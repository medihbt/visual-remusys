use remusys_ir::{
    ir::*,
    opt::{CfgBlockStat, CfgDfsSeq, DominatorTree},
};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use wasm_bindgen::JsError;

use crate::{ModuleInfo, fmt_jserr};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CfgNodeKind {
    Entry,
    Control,
    Exit,
    Unreachable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CfgEdgeClass {
    SelfRing,
    Tree,
    Back,
    Forward,
    Cross,
    Unreachable,
}

#[derive(Debug, Serialize)]
pub struct CfgNode {
    pub id: BlockID,
    pub label: SmolStr,
    pub kind: CfgNodeKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CfgEdge {
    pub id: JumpTargetID,
    pub kind: JumpTargetKind,
    pub from: BlockID,
    pub to: BlockID,
    /// 该边是否为 CFG 中的关键边（critical edge）. 会显示为双线.
    pub is_critical: bool,
    /// 边的分类. 该分类仅供前端展示使用, 不会影响 CFG 的结构.
    pub edge_class: CfgEdgeClass,
}

#[derive(Debug, Serialize)]
pub struct FuncCfgDt {
    pub nodes: Vec<CfgNode>,
    pub edges: Vec<CfgEdge>,
}

impl FuncCfgDt {
    /// 构造函数 CFG. 该函数会对 CFG 中的边进行分类, 并标记关键边. 该函数的时间复杂度为 O(|blocks| + |edges|).
    ///
    /// ## 分类依据
    ///
    /// - `Tree`: 如果 `to` 是 `from` 的子树节点, 则该边为树边.
    /// - `Back`: 如果 `to` 是 `from` 的祖先节点, 则该边为回边.
    /// - `Forward`: 如果 `to` 是 `from` 的非子树后代节点, 则该边为前向边.
    /// - `Cross`: 其他情况, 则该边为交叉边.
    /// - `SelfRing`: 如果 `to` 与 `from` 相同, 则该边为自环边.
    /// - `Unreachable`: 如果 `from` 是不可达的, 则该边为不可达边.
    pub fn new(mctx: &ModuleInfo, func: FuncID) -> Result<Self, JsError> {
        let allocs = &mctx.module.allocs;
        let names = &mctx.names;
        let Some(body) = func.get_body(allocs) else {
            return fmt_jserr!("function @{} is external", func.get_name(allocs));
        };
        let entry = body.entry;

        let pre_dfs = CfgDfsSeq::new_pre(allocs, func)?;
        let post_dfs = CfgDfsSeq::new_post(allocs, func)?;
        let mut nodes = Vec::with_capacity(pre_dfs.nodes.len());
        let mut edges = Vec::new();

        let is_critical = |from: BlockID, to: BlockID| {
            let from_succs = from.get_succs(allocs);
            let to_preds = to.get_preds(allocs);
            from_succs.len() > 1 && to_preds.is_multiple(&allocs.jts)
        };
        let edge_class = |from: BlockID, to: BlockID| {
            let from_pre = pre_dfs.block_dfn(from);
            let to_pre = pre_dfs.block_dfn(to);

            // Tree edge requires direct DFS parent relation, not only ancestor relation.
            if pre_dfs.nodes[to_pre].parent == from_pre {
                return CfgEdgeClass::Tree;
            }

            let from_post = post_dfs.block_dfn(from);
            let to_post = post_dfs.block_dfn(to);

            // In this codebase's post-order indexing, ancestors appear later than descendants.
            let to_is_ancestor_of_from = to_pre < from_pre && to_post > from_post;
            if to_is_ancestor_of_from {
                return CfgEdgeClass::Back;
            }

            let to_is_descendant_of_from = from_pre < to_pre && from_post > to_post;
            if to_is_descendant_of_from {
                return CfgEdgeClass::Forward;
            }

            CfgEdgeClass::Cross
        };
        for (bb_id, block) in body.blocks.iter(&allocs.blocks) {
            let label = match names.get_local_name(bb_id) {
                Some(name) => name,
                None => bb_id.to_strid(),
            };
            let kind = if bb_id == entry {
                CfgNodeKind::Entry
            } else if !pre_dfs.block_reachable(bb_id) {
                CfgNodeKind::Unreachable
            } else if bb_id.get_succs(allocs).is_empty() {
                CfgNodeKind::Exit
            } else {
                CfgNodeKind::Control
            };
            nodes.push(CfgNode {
                id: bb_id,
                label,
                kind,
            });

            let succs = block.get_succs(allocs);
            edges.reserve(succs.len());

            for jt_id in succs {
                let jt = jt_id.deref_ir(allocs);
                let Some(to) = jt.block.get() else {
                    return fmt_jserr!("jump target {jt_id:?} does not point to a block");
                };
                let edge_class = if to == bb_id {
                    CfgEdgeClass::SelfRing
                } else if kind == CfgNodeKind::Unreachable {
                    CfgEdgeClass::Unreachable
                } else {
                    edge_class(bb_id, to)
                };
                edges.push(CfgEdge {
                    id: jt_id,
                    kind: jt.get_kind(),
                    from: bb_id,
                    to,
                    is_critical: is_critical(bb_id, to),
                    edge_class,
                });
            }
        }

        Ok(Self { nodes, edges })
    }
}
