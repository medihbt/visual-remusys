use remusys_ir::{
    ir::{BlockID, FuncID, IRNameMap, ISubGlobalID, JumpTargetID, JumpTargetKind, Module},
    opt::CfgDfsSeq,
};
use serde::Serialize;
use smol_str::SmolStr;
use wasm_bindgen::JsError;

use crate::fmt_jserr;

#[derive(Debug, Clone, Serialize)]
pub struct FuncCfgDt {
    pub nodes: Vec<CfgNodeDt>,
    pub edges: Vec<CfgEdgeDt>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum CfgNodeRole {
    Entry,
    Branch,
    Exit,
}

#[derive(Debug, Clone, Serialize)]
pub struct CfgNodeDt {
    pub role: CfgNodeRole,
    pub block: BlockID,
    pub label: SmolStr,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum CfgEdgeDfsRole {
    Tree,
    Back,
    SelfRing,
    Forward,
    Cross,
}

#[derive(Debug, Clone, Serialize)]
pub struct CfgEdgeDt {
    pub id: JumpTargetID,
    pub from: BlockID,
    pub to: BlockID,
    pub dfs_role: CfgEdgeDfsRole,
    pub jt_kind: JumpTargetKind,
}

impl FuncCfgDt {
    pub fn new(module: &Module, names: &IRNameMap, func: FuncID) -> Result<Self, JsError> {
        let allocs = &module.allocs;
        let Some(body) = func.get_body(allocs) else {
            return fmt_jserr!(Err "function @{} is external", func.get_name(allocs));
        };
        let entry = body.entry;

        let edge_role = EdgeRoleJudge::new(module, func)?;

        let mut nodes = Vec::with_capacity(edge_role.node_len());
        let mut edges = Vec::new();

        for (bb_id, block) in body.blocks.iter(&allocs.blocks) {
            let label = match names.get_local_name(bb_id) {
                Some(name) => name,
                None => bb_id.to_strid(),
            };

            let role = if bb_id == entry {
                CfgNodeRole::Entry
            } else if bb_id.get_succs(allocs).is_empty() {
                CfgNodeRole::Exit
            } else {
                CfgNodeRole::Branch
            };
            nodes.push(CfgNodeDt {
                role,
                block: bb_id,
                label,
            });

            let succs = block.get_succs(allocs);
            edges.reserve(succs.len());
            for jt_id in succs {
                let jt = jt_id.deref_ir(allocs);
                let Some(to) = jt.block.get() else {
                    return fmt_jserr!(Err "jump target {jt_id:?} does not point to a block");
                };
                edges.push(CfgEdgeDt {
                    id: jt_id,
                    from: bb_id,
                    to,
                    dfs_role: edge_role.role(bb_id, to),
                    jt_kind: jt.get_kind(),
                });
            }
        }

        Ok(Self { nodes, edges })
    }
}

struct EdgeRoleJudge {
    pre_dfs: CfgDfsSeq,
    post_dfs: CfgDfsSeq,
}

impl EdgeRoleJudge {
    fn new(module: &Module, func: FuncID) -> Result<Self, JsError> {
        Ok(Self {
            pre_dfs: CfgDfsSeq::new_pre(module, func)?,
            post_dfs: CfgDfsSeq::new_post(module, func)?,
        })
    }

    fn role(&self, from: BlockID, to: BlockID) -> CfgEdgeDfsRole {
        let Self { pre_dfs, post_dfs } = self;
        if from == to {
            return CfgEdgeDfsRole::SelfRing;
        }

        let from_pre = pre_dfs.block_dfn(from);
        let to_pre = pre_dfs.block_dfn(to);

        // Tree edge requires direct DFS parent relation, not only ancestor relation.
        if pre_dfs.nodes[to_pre].parent == from_pre {
            return CfgEdgeDfsRole::Tree;
        }

        let from_post = post_dfs.block_dfn(from);
        let to_post = post_dfs.block_dfn(to);

        // In this post-order indexing, ancestors appear later than descendants.
        let to_is_ancestor_of_from = to_pre < from_pre && to_post > from_post;
        if to_is_ancestor_of_from {
            return CfgEdgeDfsRole::Back;
        }

        let to_is_descendant_of_from = from_pre < to_pre && from_post > to_post;
        if to_is_descendant_of_from {
            return CfgEdgeDfsRole::Forward;
        }

        CfgEdgeDfsRole::Cross
    }

    fn node_len(&self) -> usize {
        self.pre_dfs.nodes.len()
    }
}
