use std::collections::HashMap;

use remusys_ir::ir::{FuncID, GlobalID, ISubGlobalID, InstObj, Linkage, Module, ValueSSA};
use serde::Serialize;
use smol_str::{SmolStr, format_smolstr};
use wasm_bindgen::JsError;

use crate::ModuleInfo;

#[derive(Debug, Clone, Serialize)]
pub struct CallGraphDt {
    pub nodes: Vec<CallGraphNodeDt>,
    pub edges: Vec<CallGraphEdgeDt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum CallGraphNodeRole {
    /// 导出函数
    Public,
    /// 私有函数
    Private,
    /// 外部函数声明
    Extern,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallGraphNodeDt {
    pub id: GlobalID,
    pub label: SmolStr,
    pub role: CallGraphNodeRole,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallGraphEdgeDt {
    pub from: GlobalID,
    pub to: GlobalID,
}

impl CallGraphDt {
    pub fn new(ir: &ModuleInfo) -> Result<Self, JsError> {
        CallGraphBuilder::new(ir).build()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct EdgeKey(GlobalID, GlobalID);

#[derive(Default)]
struct EdgeList {
    edges: Vec<CallGraphEdgeDt>,
    edge_pos: HashMap<EdgeKey, usize>,
}

impl EdgeList {
    fn add_edge(&mut self, from: GlobalID, to: GlobalID) {
        let key = EdgeKey(from, to);
        if self.edge_pos.contains_key(&key) {
            return;
        }
        let edge = CallGraphEdgeDt { from, to };
        self.edges.push(edge);
        self.edge_pos.insert(key, self.edges.len() - 1);
    }
}

struct CallGraphBuilder<'ir> {
    module: &'ir Module,
    nodes: Vec<CallGraphNodeDt>,
    edges: EdgeList,
}

impl<'ir> CallGraphBuilder<'ir> {
    fn new(ir: &'ir ModuleInfo) -> Self {
        Self {
            module: ir.module(),
            nodes: Vec::new(),
            edges: EdgeList::default(),
        }
    }
    fn build(mut self) -> Result<CallGraphDt, JsError> {
        self.build_nodes()?;
        self.build_edges()?;
        Ok(CallGraphDt {
            nodes: self.nodes,
            edges: self.edges.edges,
        })
    }

    fn build_nodes(&mut self) -> Result<(), JsError> {
        let symbols = self.module.symbols.borrow();
        let funcs = symbols.func_pool();
        self.nodes.reserve(funcs.len());
        for &func in funcs.iter() {
            let name = func.clone_name(self.module);
            let role = match func.get_linkage(self.module) {
                Linkage::External => CallGraphNodeRole::Extern,
                Linkage::DSOLocal => CallGraphNodeRole::Public,
                Linkage::Private => CallGraphNodeRole::Private,
            };
            self.nodes.push(CallGraphNodeDt {
                id: func.raw_into(),
                label: format_smolstr!("@{name}"),
                role,
            });
        }
        self.nodes
            .sort_by(|l, r| l.role.cmp(&r.role).then_with(|| l.label.cmp(&r.label)));
        Ok(())
    }
    fn build_edges(&mut self) -> Result<(), JsError> {
        let mut edges = std::mem::take(&mut self.edges);

        let mut node_pos = HashMap::with_capacity(self.nodes.len());
        for (i, node) in self.nodes.iter().enumerate() {
            node_pos.insert(FuncID::raw_from(node.id), i);
        }

        for node in self.nodes.iter() {
            if node.role == CallGraphNodeRole::Extern {
                continue;
            }
            let func_id = FuncID::raw_from(node.id);
            for (_, block) in func_id.blocks_iter(self.module) {
                for (_, inst) in block.insts_iter(self.module) {
                    let Some(callee_func) = self.get_callee_func(inst) else {
                        continue;
                    };
                    if !node_pos.contains_key(&callee_func) {
                        continue;
                    }
                    edges.add_edge(node.id, callee_func.raw_into());
                }
            }
        }
        self.edges = edges;
        Ok(())
    }

    fn get_callee_func(&self, inst: &InstObj) -> Option<FuncID> {
        let InstObj::Call(call_inst) = inst else {
            return None;
        };
        let ValueSSA::Global(callee_global) = call_inst.get_callee(self.module) else {
            return None;
        };
        FuncID::try_from_global(self.module, callee_global)
    }
}
