use remusys_ir::{
    ir::{BlockID, FuncID, FuncNumberMap, ISubGlobalID, NumberOption},
    opt::{CfgBlockStat, DominatorTree},
};
use serde::Serialize;
use smol_str::{SmolStr, format_smolstr};
use wasm_bindgen::JsError;

use crate::{ModuleInfo, fmt_jserr};

#[derive(Debug, Clone, Serialize)]
pub struct DomTreeNodeDt {
    pub id: BlockID,
    pub label: SmolStr,
}
#[derive(Debug, Clone, Serialize)]
pub struct DomTreeEdgeDt {
    pub from: BlockID,
    pub to: BlockID,
}
#[derive(Debug, Serialize)]
pub struct DomTreeDt {
    pub nodes: Vec<DomTreeNodeDt>,
    pub edges: Vec<DomTreeEdgeDt>,
}

impl DomTreeDt {
    pub fn new(ir: &ModuleInfo, func: FuncID) -> Result<Self, JsError> {
        let module = ir.module();
        if !func.is_alive(module) {
            return fmt_jserr!(Err "function {func:?} is not alive");
        }
        let dt = DominatorTree::builder(module, func)?.build();
        let numbers = FuncNumberMap::new(module, func, ir.names(), NumberOption::ignore_all());
        Self::from_dt(dt, numbers)
    }

    pub fn from_dt(dt: DominatorTree, numbers: FuncNumberMap) -> Result<Self, JsError> {
        let block_name = |block: BlockID| match numbers.get_local_name(block) {
            Some(name) => format_smolstr!("%{name}"),
            None => block.to_strid(),
        };
        let mut nodes = Vec::with_capacity(dt.nodes.len());
        let mut edges = Vec::with_capacity(dt.nodes.len() - 1);
        for node in &dt.nodes {
            let CfgBlockStat::Block(block) = node.block else {
                return fmt_jserr!(Err "post-dominance not supported");
            };
            nodes.push(DomTreeNodeDt {
                id: block,
                label: block_name(block),
            });
            if let CfgBlockStat::Block(idom_block) = node.idom {
                edges.push(DomTreeEdgeDt {
                    from: idom_block,
                    to: block,
                });
            }
        }
        Ok(Self { nodes, edges })
    }
}
