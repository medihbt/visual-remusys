use remusys_ir::ir::{ISubInstID, ISubValueSSA, InstID, UserID};
use serde::Serialize;
use wasm_bindgen::JsError;

use crate::{
    ModuleInfo, ValueDt,
    dfg::{DfgEdge, DfgNode, DfgNodeID, DfgNodeRole},
    fmt_jserr,
};

#[derive(Debug, Clone, Serialize)]
pub struct DefUseGraphDt {
    pub nodes: Vec<DfgNode>,
    pub edges: Vec<DfgEdge>,
}

impl DefUseGraphDt {
    pub fn new(ir: &ModuleInfo, inst: InstID) -> Result<Self, JsError> {
        use hashbrown::HashSet;
        let value = ValueDt::Inst(inst);
        let self_node = DfgNode {
            id: DfgNodeID::Inst(inst),
            label: value.get_name(ir.module(), ir.names())?,
            value,
            role: DfgNodeRole::Effect,
        };
        let operands = inst.get_operands(ir.module());

        let mut nodeid_set = HashSet::new();
        let mut nodes = Vec::with_capacity(operands.len());
        let mut edges = Vec::with_capacity(operands.len());

        nodeid_set.insert(self_node.id);
        nodes.push(self_node.clone());
        for use_id in operands {
            let operand = ValueDt::from(use_id.get_operand(ir.module()));
            let node_id = match operand {
                ValueDt::FuncArg(id, idx) => DfgNodeID::FuncArg(id, idx),
                ValueDt::Global(id) => DfgNodeID::Global(id),
                ValueDt::Block(id) => DfgNodeID::Block(id),
                ValueDt::Inst(id) => DfgNodeID::Inst(id),
                ValueDt::Expr(id) => DfgNodeID::Expr(id),
                _ => DfgNodeID::Use(use_id),
            };
            if nodeid_set.insert(node_id) {
                nodes.push(DfgNode {
                    id: node_id,
                    label: operand.get_name(ir.module(), ir.names())?,
                    value: operand,
                    role: DfgNodeRole::Income,
                });
            }
            edges.push(DfgEdge {
                id: use_id,
                label: use_id.get_kind(ir.module()),
                from: node_id,
                to: self_node.id,
            });
        }

        let Some(users) = inst.try_get_users(ir.module()) else {
            return fmt_jserr!(Err "inst {inst:?} should have a users list");
        };
        for (use_id, use_obj) in users.iter(&ir.module().allocs.uses) {
            let Some(user_id) = use_obj.user.get() else {
                return fmt_jserr!(Err "use {use_id:?} should have a user");
            };
            let user_value = ValueDt::from(user_id);
            let user_nodeid = match user_id {
                UserID::Expr(expr_id) => DfgNodeID::Expr(expr_id),
                UserID::Inst(inst_id) => DfgNodeID::Inst(inst_id),
                UserID::Global(global_id) => DfgNodeID::Global(global_id),
            };
            if nodeid_set.insert(user_nodeid) {
                nodes.push(DfgNode {
                    id: user_nodeid,
                    label: user_value.get_name(ir.module(), ir.names())?,
                    value: user_value,
                    role: DfgNodeRole::Outgo,
                });
            }
            edges.push(DfgEdge {
                id: use_id,
                label: use_id.get_kind(ir.module()),
                from: self_node.id,
                to: user_nodeid,
            });
        }

        Ok(Self { nodes, edges })
    }
}
