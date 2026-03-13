use crate::{ValueDt, fmt_jserr};
use remusys_ir::ir::{inst::CallInst, *};
use serde::{Serialize, Serializer};
use smol_str::{ToSmolStr, format_smolstr};
use std::collections::HashMap;
use wasm_bindgen::JsError;

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
    fn to_smolstr(&self) -> smol_str::SmolStr {
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

#[derive(Debug, Clone, Serialize)]
pub struct BlockDfgDt {
    pub nodes: Vec<DfgSection>,
    pub edges: Vec<DfgEdge>,
}

impl BlockDfgDt {
    pub fn new(module: &Module, block: BlockID) -> Result<Self, JsError> {
        BlockDfgBuilder::new(module, block).build()
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum DfgSectionKind {
    Pure,
    Effect,
    Income,
    Outcome,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct DfgNode {
    pub id: DfgNodeID,
    pub value: ValueDt,
}

#[derive(Debug, Clone, Serialize)]
pub struct DfgEdge {
    pub id: UseID,
    pub section_id: Option<usize>,
    pub kind: UseKind,
    pub user: DfgNodeID,
    pub operand: DfgNodeID,
}

#[derive(Debug, Clone, Serialize)]
pub struct DfgSection {
    pub id: usize,
    pub nodes: Vec<DfgNode>,
    pub kind: DfgSectionKind,
}

impl DfgSection {
    pub const INCOME_SECTION_ID: usize = 0;
    pub const OUTCOME_SECTION_ID: usize = 1;

    fn initial() -> Vec<Self> {
        let income_section = Self {
            id: Self::INCOME_SECTION_ID,
            nodes: Vec::new(),
            kind: DfgSectionKind::Income,
        };
        let outcome_section = Self {
            id: Self::OUTCOME_SECTION_ID,
            nodes: Vec::new(),
            kind: DfgSectionKind::Outcome,
        };
        vec![income_section, outcome_section]
    }
}

#[derive(Debug, Clone, Copy)]
struct InstInfo {
    id: InstID,
    is_split: bool,
}

struct BlockDfgBuilder<'ir> {
    allocs: &'ir IRAllocs,
    sections: Vec<DfgSection>,
    node_map: HashMap<DfgNodeID, (usize, usize)>,
    inst_list: Vec<InstInfo>,
    edges: Vec<DfgEdge>,
}

impl<'ir> BlockDfgBuilder<'ir> {
    fn new(module: &'ir impl AsRef<IRAllocs>, block: BlockID) -> Self {
        let allocs = module.as_ref();
        let inst_list = {
            let mut inst_list = Vec::with_capacity(block.get_insts(allocs).len());
            for (id, inst) in block.insts_iter(allocs) {
                use remusys_ir::ir::InstObj::*;
                let is_split = match inst {
                    GuideNode(_) | PhiInstEnd(_) => continue,
                    Unreachable(_) | Ret(_) | Jump(_) | Br(_) | Switch(_) => true,
                    Store(_) | AmoRmw(_) => true,
                    Call(call_inst) => !Self::calls_pure(allocs, call_inst),
                    _ => false,
                };
                inst_list.push(InstInfo { id, is_split });
            }
            inst_list
        };
        Self {
            allocs,
            sections: DfgSection::initial(),
            node_map: HashMap::new(),
            inst_list,
            edges: Vec::new(),
        }
    }

    fn calls_pure(allocs: &IRAllocs, call: &CallInst) -> bool {
        let ValueSSA::Global(global) = call.get_callee(allocs) else {
            return false;
        };
        let Some(func) = FuncID::try_from_global(allocs, global) else {
            return false;
        };
        func.deref_ir(allocs).attrs().is_func_pure()
    }
    fn value_to_id(back_id: Option<UseID>, value: ValueSSA) -> Result<DfgNodeID, JsError> {
        let id = match value {
            ValueSSA::ConstExpr(expr_id) => DfgNodeID::Expr(expr_id),
            ValueSSA::FuncArg(func_id, index) => DfgNodeID::FuncArg(func_id.raw_into(), index),
            ValueSSA::Block(block_id) => DfgNodeID::Block(block_id),
            ValueSSA::Inst(inst_id) => DfgNodeID::Inst(inst_id),
            ValueSSA::Global(global_id) => DfgNodeID::Global(global_id),
            _ => match back_id {
                Some(uid) => DfgNodeID::Use(uid),
                None => {
                    return fmt_jserr!(
                        "internal error: {value:?} is not traceable but without a use ID"
                    );
                }
            },
        };
        Ok(id)
    }
    fn add_node_in_section(
        &mut self,
        id: DfgNodeID,
        value: ValueSSA,
        section_id: usize,
    ) -> Result<usize, JsError> {
        if let Some((sec_id, _)) = self.node_map.get(&id) {
            return Ok(*sec_id);
        }
        let node = DfgNode {
            id,
            value: ValueDt::from(value),
        };
        let Some(section) = self.sections.get_mut(section_id) else {
            return fmt_jserr!(
                "section id overflow: {section_id} >= {}",
                self.sections.len()
            );
        };
        let node_idx = section.nodes.len();
        section.nodes.push(node);
        self.node_map.insert(id, (section_id, node_idx));
        Ok(section_id)
    }
    fn add_edge_with_nodes(&mut self, edge: UseID) -> Result<(), JsError> {
        let useobj = edge.deref_ir(self.allocs);
        let Some(user) = useobj.user.get() else {
            return fmt_jserr!("internal error: dangling edge {edge:?} has no user");
        };
        let value = useobj.operand.get();
        let kind = useobj.get_kind();

        let user_id = DfgNodeID::from(user);
        let operand_id = Self::value_to_id(Some(edge), value)?;
        let user_section_id = self.add_node_in_section(user_id, user.into_ir(), DfgSection::OUTCOME_SECTION_ID)?;
        let operand_section_id = self.add_node_in_section(operand_id, value, DfgSection::INCOME_SECTION_ID)?;
        self.edges.push(DfgEdge {
            id: edge,
            kind,
            user: user_id,
            operand: operand_id,
            section_id: if user_section_id == operand_section_id {
                Some(user_section_id)
            } else {
                None
            },
        });
        Ok(())
    }
    fn push_internal_inst(&mut self, InstInfo { id, is_split }: InstInfo) {
        let last_sec = self
            .sections
            .last()
            .expect("internal error: at least 2 sections");
        let last_sec = match (is_split, last_sec.kind) {
            (false, DfgSectionKind::Pure) | (true, DfgSectionKind::Effect) => {
                self.sections.last_mut().unwrap()
            }
            _ => {
                use DfgSectionKind::*;
                self.sections.push(DfgSection {
                    id: self.sections.len(),
                    nodes: Vec::with_capacity(1),
                    kind: if is_split { Effect } else { Pure },
                });
                self.sections.last_mut().unwrap()
            }
        };
        let sec_id = last_sec.id;
        let node_id = last_sec.nodes.len();
        last_sec.nodes.push(DfgNode {
            id: DfgNodeID::Inst(id),
            value: ValueDt::Inst(id),
        });
        self.node_map.insert(DfgNodeID::Inst(id), (sec_id, node_id));
    }

    fn build(&mut self) -> Result<BlockDfgDt, JsError> {
        let inst_list = std::mem::take(&mut self.inst_list);
        for &info in &inst_list {
            self.push_internal_inst(info);
        }

        let allocs = self.allocs;
        for &InstInfo { id, .. } in &inst_list {
            for edge in id.get_operands(allocs) {
                self.add_edge_with_nodes(edge)?;
            }
            for (edge, _) in id.deref_ir(allocs).user_iter(allocs) {
                self.add_edge_with_nodes(edge)?;
            }
        }
        self.inst_list = inst_list;
        Ok(BlockDfgDt {
            nodes: std::mem::take(&mut self.sections),
            edges: std::mem::take(&mut self.edges),
        })
    }
}
