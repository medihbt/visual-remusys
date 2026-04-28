use remusys_ir::{
    ir::{
        BlockID, FuncID, FuncNumberMap, IRNameMap, ISubGlobalID, JumpTargetID, JumpTargetKind,
        Module, NumberOption,
    },
    opt::CfgDfsSeq,
};
use serde::Serialize;
use smallvec::SmallVec;
use smol_str::{SmolStr, format_smolstr};
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
        let numbers = FuncNumberMap::new(module, func, names, NumberOption::ignore_all());

        let mut nodes = Vec::with_capacity(edge_role.node_len());
        let mut edges = Vec::new();

        for (bb_id, block) in body.blocks.iter(&allocs.blocks) {
            let label = match numbers.get_local_name(bb_id) {
                Some(name) => format_smolstr!("%{name}"),
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
    tree: DfsTree,
}

impl EdgeRoleJudge {
    fn new(module: &Module, func: FuncID) -> Result<Self, JsError> {
        let pre_dfs = CfgDfsSeq::new_pre(module, func)?;
        let tree = DfsTree::from(pre_dfs);
        Ok(Self { tree })
    }

    fn role(&self, from: BlockID, to: BlockID) -> CfgEdgeDfsRole {
        if from == to {
            return CfgEdgeDfsRole::SelfRing;
        }

        let from_pre = self.tree.block_dfn(from);
        let to_pre = self.tree.block_dfn(to);

        // Tree edge requires direct DFS parent relation, not only ancestor relation.
        if self.tree.parent(to_pre) == from_pre {
            return CfgEdgeDfsRole::Tree;
        }

        let from_post = self.tree.treepost_dfn(from_pre);
        let to_post = self.tree.treepost_dfn(to_pre);

        // 祖先在后序遍历中出现得更晚（post-dfn 更大）。
        // pre-dfn 与 post-dfn 均来自同一棵 DFS 树，保证了语义一致性。
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
        self.tree.node_len()
    }
}

#[derive(Default, Clone)]
struct DfsTreeNode {
    treepost_dfn: usize,
    children: SmallVec<[usize; 4]>,
}

/// 原来的 pre-dfs + post-dfs 可能导致构建出的两个 DFS 树不一致, 这里统一采用前序 DFS 树.
/// 后序 DFN 数值通过直接在这棵树上遍历得到.
struct DfsTree {
    seq: CfgDfsSeq,
    nodes: Vec<DfsTreeNode>,
}

impl From<CfgDfsSeq> for DfsTree {
    fn from(value: CfgDfsSeq) -> Self {
        let mut nodes = vec![DfsTreeNode::default(); value.nodes.len()];
        for (dfn, seq_node) in value.nodes.iter().enumerate() {
            if seq_node.parent != CfgDfsSeq::NULL_PARENT {
                nodes[seq_node.parent].children.push(dfn);
            }
        }
        DfsTree::build_treepost(&value, nodes.as_mut_slice());
        Self { seq: value, nodes }
    }
}

impl DfsTree {
    pub fn block_dfn(&self, block: BlockID) -> usize {
        self.seq.block_dfn(block)
    }

    pub fn parent(&self, dfn: usize) -> usize {
        self.seq.nodes[dfn].parent
    }

    pub fn treepost_dfn(&self, dfn: usize) -> usize {
        self.nodes[dfn].treepost_dfn
    }

    pub fn node_len(&self) -> usize {
        self.seq.nodes.len()
    }

    /// 对 `nodes` 这棵树做后序遍历, 然后填充 `nodes[].treepost_dfn`
    fn build_treepost(_dfs_seq: &CfgDfsSeq, nodes: &mut [DfsTreeNode]) {
        if nodes.is_empty() {
            return;
        }
        // 根结点在 pre-dfn 0（入口基本块总是第一个被访问）。
        // 迭代后序遍历：(pre_dfn, children_visited)
        let mut stk = vec![(0usize, false)];
        let mut post_dfn: usize = 0;
        while let Some((dfn, visited)) = stk.pop() {
            if visited {
                nodes[dfn].treepost_dfn = post_dfn;
                post_dfn += 1;
            } else {
                stk.push((dfn, true));
                // 逆序压入子结点，保证左->右的处理顺序
                for i in (0..nodes[dfn].children.len()).rev() {
                    stk.push((nodes[dfn].children[i], false));
                }
            }
        }
    }
}
