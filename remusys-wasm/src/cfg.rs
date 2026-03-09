use remusys_ir::{
    ir::*,
    opt::{CfgBlockStat, DominatorTree},
};
use serde::Serialize;
use wasm_bindgen::JsError;

use crate::fmt_jserr;

#[derive(Debug, Serialize)]
pub struct DomTreeDt {
    pub nodes: Vec<BlockID>,
    pub edges: Vec<(BlockID, BlockID)>,
}

impl TryFrom<&DominatorTree> for DomTreeDt {
    type Error = JsError;

    fn try_from(value: &DominatorTree) -> Result<Self, Self::Error> {
        let mut nodes = Vec::with_capacity(value.nodes.len());
        let mut edges = Vec::with_capacity(value.nodes.len() - 1);
        for node in &value.nodes {
            let CfgBlockStat::Block(block) = node.block else {
                return fmt_jserr!("post-dominance not supported");
            };
            nodes.push(block);
            if let CfgBlockStat::Block(idom_block) = node.idom {
                edges.push((idom_block, block));
            }
        }
        Ok(Self { nodes, edges })
    }
}
impl TryFrom<DominatorTree> for DomTreeDt {
    type Error = JsError;
    fn try_from(value: DominatorTree) -> Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}
impl DomTreeDt {
    pub fn new(module: &Module, func: FuncID) -> Result<Self, JsError> {
        let allocs = &module.allocs;
        if !func.is_alive(allocs) {
            return fmt_jserr!("function {func:?} is not alive");
        }
        let dt = DominatorTree::builder(allocs, func)?;
        let dt = dt.build();
        Self::try_from(&dt)
    }
}
