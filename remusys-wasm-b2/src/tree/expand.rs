use hashbrown::HashMap as BrownMap;
use serde::Serialize;
use smol_str::SmolStr;
use wasm_bindgen::{JsError, prelude::wasm_bindgen};

use crate::{
    IRObjPath, IRObjPathBuf, IRTree, IRTreeNodeClass, IRTreeNodeID, IRTreeObjID, ModuleInfo,
    fmt_jserr, js_assert,
    types::{JsGuideNodeData, JsIRObjPath},
};

struct Node {
    ir_object: IRTreeObjID,
    expand_children: BrownMap<IRTreeObjID, Node>,
}

impl Node {
    fn can_expand(&self) -> bool {
        use IRTreeObjID::*;
        matches!(
            self.ir_object,
            Module | Global(_) | FuncHeader(_) | Block(_) | Inst(_)
        )
    }

    fn expand(&mut self, tree: &IRTree, self_node: IRTreeNodeID) -> bool {
        if !self.can_expand() {
            return false;
        }
        let children = self_node.children(tree);
        let mut new_expand_children = BrownMap::with_capacity(children.len());
        for child in children {
            let ir_object = child.obj(tree);
            let node = Node {
                ir_object,
                expand_children: BrownMap::new(),
            };
            new_expand_children.insert(ir_object, node);
        }
        self.expand_children = new_expand_children;
        true
    }

    fn dfs_expand(&mut self, tree: &IRTree, self_node: IRTreeNodeID) {
        // 这个 DFS 最多就 6 层, 没有栈溢出的风险. Rust 的 borrow checker 不让我
        // 在显式的栈里存储那么多个 &mut Node (考虑到这些 Node 存在父子关系, 生命周期
        // 都是同一个), 做不了非递归的 DFS.
        if !self.can_expand() {
            return;
        }
        let children = self_node.children(tree);
        let mut new_expand_children = BrownMap::with_capacity(children.len());
        for &child in children {
            let ir_object = child.obj(tree);
            let mut node = Node {
                ir_object,
                expand_children: BrownMap::new(),
            };
            node.dfs_expand(tree, child);
            new_expand_children.insert(ir_object, node);
        }
        self.expand_children = new_expand_children;
    }
}

#[wasm_bindgen]
pub struct IRExpandTree {
    module_id: usize,
    root: Node,
}

impl IRExpandTree {
    /// 获取 path 所示的结点, 不尝试任何展开操作. 如果这个结点不存在或者 path 中的某一层父结点没展开, 就返回 None.
    fn get_node(&self, ir: &ModuleInfo, path: &IRObjPath) -> Result<Option<&Node>, JsError> {
        let mut node = &self.root;
        let ir_tree = ir.ir_tree();
        let mut tree_node = ir_tree.root;
        for &obj in path {
            let tree_children = tree_node.children(ir_tree);
            let Some(&tree_child) = tree_children
                .iter()
                .find(|&child| child.obj(ir_tree) == obj)
            else {
                return Ok(None);
            };
            if node.expand_children.is_empty() {
                return Ok(None);
            }

            let Some(node_child) = node.expand_children.get(&obj) else {
                return Ok(None);
            };
            node = node_child;
            tree_node = tree_child;
        }
        Ok(Some(node))
    }
    fn node_mut_expanded(
        &mut self,
        ir: &ModuleInfo,
        path: &IRObjPath,
    ) -> Result<(&mut Node, IRTreeNodeID), JsError> {
        let mut node = &mut self.root;
        let ir_tree = ir.ir_tree();
        let mut tree_node = ir_tree.root;
        for &obj in path {
            let tree_children = tree_node.children(ir_tree);
            let Some(&tree_child) = tree_children
                .iter()
                .find(|&child| child.obj(ir_tree) == obj)
            else {
                return fmt_jserr!(Err "IR object {:?} not found in IR tree", obj);
            };
            if node.expand_children.is_empty() && !node.expand(ir_tree, tree_node) {
                return fmt_jserr!(Err "IR object {:?} cannot be expanded", node.ir_object);
            }

            let Some(node_child) = node.expand_children.get_mut(&obj) else {
                return fmt_jserr!(Err "IR object {:?} not found in expand tree", obj);
            };
            node = node_child;
            tree_node = tree_child;
        }
        Ok((node, tree_node))
    }

    fn do_expand(&mut self, ir: &ModuleInfo, path: &IRObjPath) -> Result<(), JsError> {
        js_assert!(ir.get_id() == self.module_id, "Module ID mismatch")?;

        let ir_tree = ir.ir_tree();
        let (set_node, tree_node) = self.node_mut_expanded(ir, path)?;
        // Now expand set node itself.
        set_node.expand(ir_tree, tree_node);
        Ok(())
    }
    fn do_dfs_expand(&mut self, ir: &ModuleInfo, path: &IRObjPath) -> Result<(), JsError> {
        js_assert!(ir.get_id() == self.module_id, "Module ID mismatch")?;

        let ir_tree = ir.ir_tree();
        let (set_node, tree_node) = self.node_mut_expanded(ir, path)?;
        // Now expand set node itself and all its descendants.
        set_node.dfs_expand(ir_tree, tree_node);
        Ok(())
    }
    fn do_collapse(&mut self, ir: &ModuleInfo, path: &IRObjPath) -> Result<(), JsError> {
        js_assert!(ir.get_id() == self.module_id, "Module ID mismatch")?;

        let (set_node, _) = self.node_mut_expanded(ir, path)?;
        set_node.expand_children.clear();
        Ok(())
    }

    fn do_load_tree(&mut self, ir: &ModuleInfo) -> Result<IRGuideNodeDt, JsError> {
        let ir_tree = ir.ir_tree();
        let (new_root, guide_root) =
            Self::build_node_intersection(ir, ir_tree, ir_tree.root, &self.root)?;
        self.root = new_root;
        Ok(guide_root)
    }
    fn build_node_intersection(
        ir: &ModuleInfo,
        ir_tree: &IRTree,
        tree_node: IRTreeNodeID,
        old_node: &Node,
    ) -> Result<(Node, IRGuideNodeDt), JsError> {
        let ir_object = tree_node.obj(ir_tree);
        let mut new_node = Node {
            ir_object,
            expand_children: BrownMap::new(),
        };

        // 约定: expand_children 非空表示该结点在旧状态下是展开的.
        let mut dt_children = Vec::new();
        if !old_node.expand_children.is_empty() {
            for &tree_child in tree_node.children(ir_tree) {
                let child_obj = tree_child.obj(ir_tree);
                let Some(old_child) = old_node.expand_children.get(&child_obj) else {
                    // 取交集: 仅保留同时出现在当前 IR 与旧展开树中的结点.
                    continue;
                };

                let (new_child, child_dt) =
                    Self::build_node_intersection(ir, ir_tree, tree_child, old_child)?;
                new_node.expand_children.insert(child_obj, new_child);
                dt_children.push(child_dt);
            }
        }

        let guide_node = IRGuideNodeDt {
            id: tree_node.to_strid(),
            ir_object,
            label: ir_object.get_name(ir)?,
            kind: ir_object.get_class(ir)?,
            focus_class: GuideFocusClass::NotFocused,
            children: if dt_children.is_empty() {
                None
            } else {
                Some(dt_children)
            },
        };

        Ok((new_node, guide_node))
    }
}

#[wasm_bindgen]
impl IRExpandTree {
    pub fn new(ir: &ModuleInfo) -> Self {
        let tree = ir.ir_tree();
        let root = tree.root;
        let mut expand_children = BrownMap::new();
        for child in root.children(tree) {
            let ir_object = child.obj(tree);
            expand_children.insert(
                ir_object,
                Node {
                    ir_object,
                    expand_children: BrownMap::new(),
                },
            );
        }

        let root_node = Node {
            ir_object: IRTreeObjID::Module,
            expand_children,
        };
        Self {
            module_id: ir.get_id(),
            root: root_node,
        }
    }

    /// 展开一层 path 所示的结点. 如果这个 path 还没向下探到子结点就发现其中的一层父结点没展开,
    /// 那会顺着这个父结点一直往下展开到 path 的下一层.
    ///
    /// @param ir - 模块信息, 用来访问 IR 树等相关信息. 注意这个参数必须和构造函数里传入的模块信息是同一个, 否则会返回错误.
    /// @param {IRTreeObjID[]} path - 结点路径, 类型 `IRTreeObjID[]`.
    /// @return {void} 成功时什么都不返回.
    ///
    /// @throws Error 如果遇到了不能展开的结点, 或者其他异常情况, 就抛出一个 Error.
    pub fn expand_one(&mut self, ir: &ModuleInfo, path: JsIRObjPath) -> Result<(), JsError> {
        let path: IRObjPathBuf = ModuleInfo::deserialize(path)?;
        self.do_expand(ir, &path)
    }

    /// 展开 path 所示的结点和它下面的所有后代结点. 这个函数的行为和 `expand_one` 类似, 但是它会递归地展开所有后代结点.
    ///
    /// @param {IRTreeObjID[]} path - 结点路径, 类型 `IRTreeObjID[]`.
    pub fn expand_all(&mut self, ir: &ModuleInfo, path: JsIRObjPath) -> Result<(), JsError> {
        let path: IRObjPathBuf = ModuleInfo::deserialize(path)?;
        self.do_dfs_expand(ir, &path)
    }

    /// 收起 path 所示的结点. 这个函数会把这个结点下面的所有后代结点都收起来, 不管它们之前是什么状态.
    ///
    /// @param {IRTreeObjID[]} path - 结点路径, 类型 `IRTreeObjID[]`.
    pub fn collapse(&mut self, ir: &ModuleInfo, path: JsIRObjPath) -> Result<(), JsError> {
        let path: IRObjPathBuf = ModuleInfo::deserialize(path)?;
        self.do_collapse(ir, &path)
    }

    /// 判断 path 所示的结点是否已经展开了.
    ///
    /// @param {IRTreeObjID[]} path - 结点路径, 类型 `IRTreeObjID[]`.
    pub fn path_expanded(&self, ir: &ModuleInfo, path: JsIRObjPath) -> Result<bool, JsError> {
        let path: IRObjPathBuf = ModuleInfo::deserialize(path)?;
        let Some(node) = self.get_node(ir, &path)? else {
            return Ok(false);
        };
        Ok(!node.expand_children.is_empty())
    }

    /// 按照这个 ExpandTree 里记录的展开状态, 从 IR 树里加载出一个新的树形结构.
    ///
    /// @param {IRTreeObjID[]} focus_path - 焦点路径.
    pub fn load_tree(
        &mut self,
        ir: &ModuleInfo,
        focus_path: JsIRObjPath,
    ) -> Result<JsGuideNodeData, JsError> {
        js_assert!(ir.get_id() == self.module_id, "Module ID mismatch")?;
        let focus_path: IRObjPathBuf = ModuleInfo::deserialize(focus_path)?;
        let focus_obj = *focus_path.last().unwrap();
        let mut root_node = self.do_load_tree(ir)?;
        // 标记 focus path 上的结点.
        let mut focus_node = &mut root_node;
        for &obj in &focus_path {
            let Some(children) = &mut focus_node.children else {
                break;
            };
            let Some(child) = children.iter_mut().find(|child| child.ir_object == obj) else {
                break;
            };
            let child_obj = child.ir_object;
            child.focus_class = if child_obj == focus_obj {
                GuideFocusClass::FocusNode
            } else if matches!(child_obj, IRTreeObjID::Global(_)) {
                GuideFocusClass::FocusScope
            } else {
                GuideFocusClass::FocusParent
            };
            focus_node = child;
        }
        ModuleInfo::serialize(&root_node)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub enum GuideFocusClass {
    #[default]
    NotFocused,
    FocusNode,
    FocusParent,
    FocusScope,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IRGuideNodeDt {
    pub id: SmolStr,
    pub ir_object: IRTreeObjID,
    pub label: SmolStr,
    pub kind: IRTreeNodeClass,
    pub focus_class: GuideFocusClass,
    pub children: Option<Vec<IRGuideNodeDt>>,
    // parent: always None because it makes ring reference and is not needed in frontend.
}
