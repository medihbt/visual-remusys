use std::collections::HashMap;

use remusys_ir::ir::*;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CallGraphDt {
    pub nodes: Vec<CallGraphNode>,
    pub edges: Vec<CallGraphEdge>,
}

impl CallGraphDt {
    pub fn new(module: &Module) -> Self {
        CallGraphDirectBuilder::new(module).build()
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum CallNodeRole {
    Root,
    Live,
    Indirect,
    Unreachable,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CallGraphNode {
    pub id: GlobalID,
    pub role: CallNodeRole,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallGraphEdge {
    pub id: UseID,
    pub caller: GlobalID,
    pub callee: GlobalID,
}

#[derive(Default)]
struct Funcs {
    root: Box<[FuncID]>,
    other_func: Box<[FuncID]>,
}
impl Funcs {
    fn new(module: &Module) -> Self {
        let symtab = module.symbols.borrow();
        let allocs = &module.allocs;
        let func_pool = symtab.func_pool();
        let mut root = Vec::new();
        let mut other_func = Vec::new();
        for &f in func_pool.iter() {
            if f.get_name(allocs) == "main" || f.get_linkage(allocs) == Linkage::DSOLocal {
                root.push(f);
            } else {
                other_func.push(f);
            }
        }
        Self {
            root: root.into_boxed_slice(),
            other_func: other_func.into_boxed_slice(),
        }
    }
}

struct FuncFrame {
    func: FuncID,
    role: CallNodeRole,
}

struct CallGraphDirectBuilder<'ir> {
    module: &'ir Module,
    nodes: Vec<CallGraphNode>,
    edges: Vec<CallGraphEdge>,
    node_map: HashMap<GlobalID, usize>,
    edge_map: HashMap<UseID, usize>,
}

impl<'ir> CallGraphDirectBuilder<'ir> {
    pub fn new(module: &'ir Module) -> Self {
        Self {
            module,
            nodes: Vec::new(),
            node_map: HashMap::new(),
            edges: Vec::new(),
            edge_map: HashMap::new(),
        }
    }

    fn add_node(&mut self, id: GlobalID, role: CallNodeRole) -> usize {
        if let Some(&idx) = self.node_map.get(&id) {
            return idx;
        }
        let idx = self.nodes.len();
        self.nodes.push(CallGraphNode { id, role });
        self.node_map.insert(id, idx);
        idx
    }
    fn dump_direct_calls(&self, func: FuncID) -> Vec<(UseID, FuncID)> {
        let allocs = &self.module.allocs;
        let Some(blocks) = func.try_blocks_iter(allocs) else {
            return Vec::new();
        };
        let mut calls = Vec::new();
        for (_, block) in blocks {
            for (_, inst) in block.insts_iter(allocs) {
                let InstObj::Call(call) = inst else {
                    continue;
                };
                let ValueSSA::Global(callee) = call.get_callee(allocs) else {
                    continue;
                };
                let Some(callee) = FuncID::try_from_global(allocs, callee) else {
                    continue;
                };
                calls.push((call.callee_use(), callee));
            }
        }
        calls
    }

    fn build_one(&mut self, stack: &mut Vec<FuncFrame>) -> bool {
        let Some(FuncFrame { func, role }) = stack.pop() else {
            return false;
        };
        let callee_role = match role {
            CallNodeRole::Root => CallNodeRole::Live,
            role => role,
        };
        if self.node_map.contains_key(&func.raw_into()) {
            return true;
        } else {
            self.add_node(func.raw_into(), role);
        }

        for (edge_id, callee) in self.dump_direct_calls(func) {
            let edge_index = self.edges.len();
            self.edges.push(CallGraphEdge {
                id: edge_id,
                caller: func.raw_into(),
                callee: callee.raw_into(),
            });
            self.edge_map.insert(edge_id, edge_index);
            stack.push(FuncFrame {
                func: callee,
                role: callee_role,
            });
        }
        true
    }

    pub fn build(&mut self) -> CallGraphDt {
        let Funcs { root, other_func } = Funcs::new(self.module);

        let mut func_stack = Vec::with_capacity(root.len() + other_func.len());
        for &root_fid in root.iter() {
            func_stack.push(FuncFrame {
                func: root_fid,
                role: CallNodeRole::Root,
            });
        }
        while self.build_one(&mut func_stack) {}

        for &other_fid in other_func.iter() {
            if self.node_map.contains_key(&other_fid.raw_into()) {
                continue;
            }
            func_stack.push(FuncFrame {
                func: other_fid,
                role: CallNodeRole::Unreachable,
            });
        }
        while self.build_one(&mut func_stack) {}

        CallGraphDt {
            nodes: std::mem::take(&mut self.nodes),
            edges: std::mem::take(&mut self.edges),
        }
    }
}
