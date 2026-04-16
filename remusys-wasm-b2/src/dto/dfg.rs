use remusys_ir::ir::*;
use serde::{Deserialize, Serialize, Serializer};
use smol_str::{SmolStr, ToSmolStr, format_smolstr};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use wasm_bindgen::JsError;

use crate::{ModuleInfo, dto::ValueDt, fmt_jserr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DfgNodeID {
    Inst(InstID),
    Expr(ExprID),
    Block(BlockID),
    FuncArg(GlobalID, u32),
    Global(GlobalID),
    Use(UseID),
}
impl From<UserID> for DfgNodeID {
    fn from(value: UserID) -> Self {
        match value {
            UserID::Inst(x) => Self::Inst(x),
            UserID::Expr(x) => Self::Expr(x),
            UserID::Global(x) => Self::Global(x),
        }
    }
}

impl ToSmolStr for DfgNodeID {
    fn to_smolstr(&self) -> SmolStr {
        match self {
            DfgNodeID::Inst(inst_id) => inst_id.to_strid(),
            DfgNodeID::Expr(expr_id) => expr_id.to_strid(),
            DfgNodeID::Block(block_id) => block_id.to_strid(),
            DfgNodeID::Global(global_id) => global_id.to_strid(),
            DfgNodeID::FuncArg(func_id, index) => {
                format_smolstr!("FuncArg({}, {index})", func_id.to_strid())
            }
            DfgNodeID::Use(use_id) => use_id.to_strid(),
        }
    }
}
impl Serialize for DfgNodeID {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_smolstr())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DfgNodeRole {
    /// 从其他基本块输入的数据流结点
    Income,
    /// 向其他基本块输出的数据流结点
    Outgo,
    /// Phi 结点
    Phi,
    /// 纯计算结点（不包含副作用）
    Pure,
    /// 包含副作用的计算结点
    Effect,
    /// 终结指令结点
    Terminator,
}

/// 基本块的数据流图结点
#[derive(Debug, Clone, Serialize)]
pub struct DfgNode {
    pub id: DfgNodeID,
    pub value: ValueDt,
    pub role: DfgNodeRole,
}

#[derive(Debug, Clone, Serialize)]
pub struct DfgSection {
    pub kind: DfgNodeRole,
    pub nodes: Vec<DfgNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DfgEdge {
    pub id: UseID,
    pub from: DfgNodeID,
    pub to: DfgNodeID,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlockDfg {
    pub sections: Vec<DfgSection>,
    pub edges: Vec<DfgEdge>,
}

impl BlockDfg {
    pub fn new(module: &ModuleInfo, block_id: BlockID) -> Result<Self, JsError> {
        BlockDfgBuilder::new(module.module(), block_id).build()
    }

    pub fn to_dot_text(&self) -> String {
        fn dot_escape(text: &str) -> String {
            let mut out = String::with_capacity(text.len());
            for ch in text.chars() {
                match ch {
                    '\\' => out.push_str("\\\\"),
                    '"' => out.push_str("\\\""),
                    '\n' => out.push_str("\\n"),
                    _ => out.push(ch),
                }
            }
            out
        }

        fn role_style(role: DfgNodeRole) -> (&'static str, &'static str) {
            match role {
                DfgNodeRole::Income => ("#dbeafe", "#2563eb"),
                DfgNodeRole::Outgo => ("#dcfce7", "#16a34a"),
                DfgNodeRole::Phi => ("#fef3c7", "#d97706"),
                DfgNodeRole::Pure => ("#f3f4f6", "#4b5563"),
                DfgNodeRole::Effect => ("#fee2e2", "#dc2626"),
                DfgNodeRole::Terminator => ("#ffedd5", "#ea580c"),
            }
        }

        let mut output = String::from(
            "digraph BlockDfg {\n  rankdir=LR;\n  compound=true;\n  graph [fontname=\"Helvetica\"];\n  node [shape=box, style=filled, fontname=\"Helvetica\"];\n  edge [fontname=\"Helvetica\"];\n",
        );

        let mut node_names = HashMap::new();
        for (section_idx, section) in self.sections.iter().enumerate() {
            let section_label = dot_escape(&format!("Section {section_idx}: {:?}", section.kind));
            let _ = writeln!(output, "  subgraph cluster_{section_idx} {{");
            let _ = writeln!(output, "    label=\"{section_label}\";");
            let _ = writeln!(output, "    color=\"#94a3b8\";");
            let _ = writeln!(output, "    style=rounded;");

            for (node_idx, node) in section.nodes.iter().enumerate() {
                let node_name = format!("n_{section_idx}_{node_idx}");
                let label = dot_escape(&format!(
                    "{}\\n{:?}\\n{:?}",
                    node.id.to_smolstr(),
                    node.role,
                    node.value
                ));
                let (fillcolor, border_color) = role_style(node.role);
                let _ = writeln!(
                    output,
                    "    {node_name} [label=\"{label}\", fillcolor=\"{fillcolor}\", color=\"{border_color}\"] ;"
                );
                node_names.insert(node.id, node_name);
            }

            let _ = writeln!(output, "  }}");
        }

        for edge in &self.edges {
            let Some(from) = node_names.get(&edge.from) else {
                continue;
            };
            let Some(to) = node_names.get(&edge.to) else {
                continue;
            };
            let edge_label = dot_escape(&edge.id.to_strid());
            let _ = writeln!(output, "  {from} -> {to} [label=\"{edge_label}\"];");
        }

        output.push_str("}\n");
        output
    }
}

#[derive(Debug, Clone, Copy)]
struct InstInfo {
    id: InstID,
    role: DfgNodeRole,
}

struct BlockDfgBuilder<'ir> {
    module: &'ir Module,
    sections: Vec<DfgSection>,
    node_map: HashMap<DfgNodeID, (usize, usize)>,
    inst_list: Vec<InstInfo>,
    edges: Vec<DfgEdge>,
    edge_set: HashSet<UseID>,
}

impl<'ir> BlockDfgBuilder<'ir> {
    fn new(module: &'ir Module, block: BlockID) -> Self {
        let allocs = &module.allocs;
        let mut inst_list = Vec::with_capacity(block.get_insts(allocs).len());
        for (id, inst) in block.insts_iter(allocs) {
            let Some(role) = Self::inst_role(module, inst) else {
                continue;
            };
            inst_list.push(InstInfo { id, role });
        }
        Self {
            module,
            sections: vec![
                DfgSection {
                    kind: DfgNodeRole::Income,
                    nodes: Vec::new(),
                },
                DfgSection {
                    kind: DfgNodeRole::Outgo,
                    nodes: Vec::new(),
                },
            ],
            node_map: HashMap::new(),
            inst_list,
            edges: Vec::new(),
            edge_set: HashSet::new(),
        }
    }

    fn inst_role(module: &Module, inst: &InstObj) -> Option<DfgNodeRole> {
        use InstObj::*;

        let role = match inst {
            GuideNode(_) | PhiInstEnd(_) => return None,
            Phi(_) => DfgNodeRole::Phi,
            Unreachable(_) | Ret(_) | Jump(_) | Br(_) | Switch(_) => DfgNodeRole::Terminator,
            Store(_) | AmoRmw(_) => DfgNodeRole::Effect,
            Call(call) => {
                if Self::calls_pure(module, call) {
                    DfgNodeRole::Pure
                } else {
                    DfgNodeRole::Effect
                }
            }
            _ => DfgNodeRole::Pure,
        };
        Some(role)
    }

    fn calls_pure(module: &Module, call: &inst::CallInst) -> bool {
        let ValueSSA::Global(global) = call.get_callee(module) else {
            return false;
        };
        let Some(func) = FuncID::try_from_global(module, global) else {
            return false;
        };
        func.deref_ir(module).attrs().is_func_pure()
    }

    fn add_dedup_node(
        &mut self,
        id: DfgNodeID,
        value: ValueSSA,
        role: DfgNodeRole,
        section_id: usize,
    ) -> Result<usize, JsError> {
        if let Some((existing_sec, _)) = self.node_map.get(&id) {
            return Ok(*existing_sec);
        }
        let Some(section) = self.sections.get_mut(section_id) else {
            return fmt_jserr!(Err
                "section id overflow: {section_id} >= {}",
                self.sections.len()
            );
        };
        let node_idx = section.nodes.len();
        section.nodes.push(DfgNode {
            id,
            value: ValueDt::from(value),
            role,
        });
        self.node_map.insert(id, (section_id, node_idx));
        Ok(section_id)
    }

    fn add_nodedup_node(
        &mut self,
        id: DfgNodeID,
        value: ValueSSA,
        role: DfgNodeRole,
        section_id: usize,
    ) -> Result<usize, JsError> {
        let Some(section) = self.sections.get_mut(section_id) else {
            return fmt_jserr!(Err
                "section id overflow: {section_id} >= {}",
                self.sections.len()
            );
        };
        section.nodes.push(DfgNode {
            id,
            value: ValueDt::from(value),
            role,
        });
        Ok(section_id)
    }

    fn push_internal_inst(&mut self, InstInfo { id, role }: InstInfo) {
        let reuse_last = match self.sections.last() {
            Some(last) => last.kind == role,
            None => false,
        };

        let section_id = if reuse_last {
            self.sections.len() - 1
        } else {
            self.sections.push(DfgSection {
                kind: role,
                nodes: Vec::with_capacity(1),
            });
            self.sections.len() - 1
        };

        let section = &mut self.sections[section_id];
        let node_idx = section.nodes.len();
        section.nodes.push(DfgNode {
            id: DfgNodeID::Inst(id),
            value: ValueDt::Inst(id),
            role,
        });
        self.node_map
            .insert(DfgNodeID::Inst(id), (section_id, node_idx));
    }

    fn add_edge_with_nodes(&mut self, edge: UseID) -> Result<(), JsError> {
        if self.edge_set.contains(&edge) {
            return Ok(());
        }
        let allocs = &self.module.allocs;
        let useobj = edge.deref_ir(allocs);
        let Some(user) = useobj.user.get() else {
            return fmt_jserr!(Err "internal error: dangling edge {edge:?} has no user");
        };
        let operand = useobj.operand.get();

        let user_id = DfgNodeID::from(user);
        let user_section_id = self.add_dedup_node(
            user_id,
            user.into_ir(),
            DfgNodeRole::Outgo,
            1, // Outgo section
        )?;

        // b2 规则: 只有 Inst / FuncArg 放在 Income section 并去重;
        // 其他操作数都放在 user 所在 section，作为 Income 角色且不去重。
        let (operand_id, _) = match operand {
            ValueSSA::Inst(inst_id) => (
                DfgNodeID::Inst(inst_id),
                self.add_dedup_node(
                    DfgNodeID::Inst(inst_id),
                    operand,
                    DfgNodeRole::Income,
                    0, // Income section
                )?,
            ),
            ValueSSA::FuncArg(func_id, idx) => (
                DfgNodeID::FuncArg(func_id.raw_into(), idx),
                self.add_dedup_node(
                    DfgNodeID::FuncArg(func_id.raw_into(), idx),
                    operand,
                    DfgNodeRole::Income,
                    0, // Income section
                )?,
            ),
            _ => (
                DfgNodeID::Use(edge),
                self.add_nodedup_node(
                    DfgNodeID::Use(edge),
                    operand,
                    DfgNodeRole::Income,
                    user_section_id,
                )?,
            ),
        };

        self.edges.push(DfgEdge {
            id: edge,
            from: user_id,
            to: operand_id,
        });
        self.edge_set.insert(edge);
        Ok(())
    }

    fn build(mut self) -> Result<BlockDfg, JsError> {
        let inst_list = std::mem::take(&mut self.inst_list);
        for &info in &inst_list {
            self.push_internal_inst(info);
        }

        let allocs = &self.module.allocs;
        for &InstInfo { id, .. } in &inst_list {
            for edge in id.get_operands(allocs) {
                self.add_edge_with_nodes(edge)?;
            }
            for (edge, _) in id.deref_ir(allocs).user_iter(allocs) {
                self.add_edge_with_nodes(edge)?;
            }
        }

        Ok(BlockDfg {
            sections: self.sections,
            edges: self.edges,
        })
    }
}
