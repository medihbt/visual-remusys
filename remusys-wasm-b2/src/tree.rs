//! IR 结点和源代码位置的双向映射关联树结构. Rust 这里掌握的信息比 JS 多得多,
//! 因此把 JS 的逻辑挪到这里来实现, JS 只负责展示和交互.
//!
//! ## 直接用 IR 对象不好吗?
//!
//! 许多源语言的 LSP 确实是这么做的, 源语言的 AST 直接和源语言的文本做双向映射,
//! 这样实现简单. 可问题是, Remusys-IR 不是树, 用不了这套方案. 重温一下,
//! Remusys-IR 的架构是这样的:
//!
//! - 模块: 主干树的根, 表示一个编译单元, 下属全局变量、函数等结点
//! - 函数: 主干树结点, 下属基本块结点、函数参数结点
//! - 基本块: 主干树结点, 下属指令结点
//! - 指令: 主干树的叶, 没有子结点
//! - 函数参数: 叶结点, 没有子结点
//! - 全局变量: 叶结点, 没有子结点
//!
//! IR 有主干树, 但这个主干树覆盖不到所有的 IR 对象——例如 IR 文本中需要分层表达的
//! 操作数不在主干树里. 那如果把它们加进主干树呢? 根本不行, 这些操作数之下会通过
//! def-use 关系形成复杂的图结构, 不是树了. 比如下面的一个 IR 指令
//!
//! ```remusys-ir
//! store
//!     [ 2 x [ 2 x i32 ] ] [             ; Expression E
//!         [ 2 x i32 ] [ i32 1, i32 2 ], ; Array A
//!         [ 2 x i32 ] [ i32 1, i32 2 ]  ; Array B
//!     ],
//!     ptr %array,
//!     align 16
//! ```
//!
//! 在真实的 IR 内存表示里面, 数组表达式 A 和 B 很有可能是同一个 `ExprID` 对象,
//! 也就是说, 该指令 `StoreSource` 位置的操作数是个 DAG. 这很显然不可接受. 不过
//! 可以证明, 如果定义叶结点为 "不需要遍历它的操作数就可以当成操作数打印出来的结点",
//! 合法的 IR 中操作数位的对象恒为 DAG, 不存在环路.
//!
//! 既然是 DAG 我们就可以想办法把操作数引用图这个不在主干树里的东西做成树了,
//! `IRSourceTree` 系列数据结构就是为了这个设计的. 不过引入这个模块也就意味着
//! 我需要重新编写一遍 IR 序列化逻辑, 工作量只会大不会小.
//!
//! ## 实现方案
//!
//! ### 内存管理方案
//!
//! 使用 `mtb-entity-slab` -- 我自己写的 slab 库，辅助设施比较齐全. 主要看中
//! `#[entity_id(IDType, ...)]` 属性宏, 可以快速定义强类型 ID, 少写样板代码.
//! 而且有分代信息, 前后端交互比较安全.
//!
//! ### 树结构
//!
//! 这个我没想好要怎么做, 现在处在做做看的状态.

use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    num::NonZeroU64,
    ops::Range,
};

use mtb_entity_slab::{EntityAlloc, GenIndex, IEntityAllocID, IPoliciedID, IndexedID, entity_id};
use remusys_ir::ir::{
    BlockID, FuncID, GlobalID, GlobalObj, ISubGlobalID, ISubInst, ISubInstID, InstID, InstObj,
    JumpTargetID, UseID,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smallvec::SmallVec;
use smol_str::{SmolStr, format_smolstr};
use wasm_bindgen::{JsError, prelude::wasm_bindgen};

use crate::{
    IRTreeNodeClass, IRTreeNodeDt, ModuleInfo,
    dto::ValueDt,
    fmt_jserr, js_assert,
    types::{JsIRObjPath, JsIRTreeNodeDt, JsIRTreeNodes, JsTreeObjID},
};

pub mod builder;
pub mod expand;
pub mod testing;

#[derive(Debug, Clone, thiserror::Error)]
pub enum IRTreeErr {
    #[error("invalid node id {0:?}")]
    InvalidID(IRTreeNodeID),
}
pub type IRTreeRes<T = ()> = Result<T, IRTreeErr>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(tag = "type", content = "value")]
pub enum IRTreeObjID {
    /// 模块全局, 主干树模式
    /// Module 没有 ID. 事实上 IRTreeObjID 是 Module 局部的, 离开的 Module
    /// 表达不了任何含义
    Module,

    /// 全局对象, IR 主干树模式
    Global(GlobalID),
    /// 函数参数, IR 主干树模式; 定义为 `(function, index)`
    FuncArg(GlobalID, u32),
    /// 基本块, IR 主干树模式
    Block(BlockID),
    /// 指令, IR 主干树模式
    Inst(InstID),

    /// 操作数边, 主干树之下, `ValueSSA` 的代理
    /// 之所以不直接存储 `ValueSSA`, 是因为 UseID 能进哈希表, 还能向上找到 user
    /// 主干树之外的 `IRTreeObjID` 可能会对应不止一个 `IRTreeNodeID`, `Use` 也一样
    Use(UseID),

    /// 控制流边, 主干树之下, `BlockID` 在控制流后继模式下的代理
    /// 之所以不直接存储 `BlockID` 是为了让边的行为和 `Use` 保持统一.
    /// JumpTargetID 虽然也在主干树之外, 但和主干树 ID 一样有唯一对应的 `IRTreeNodeID`
    JumpTarget(JumpTargetID),

    /// 全局函数的函数头, 主干树模式. 之所以单独拿出来是因为函数头这一行通常需要单独的格式化和展示.
    FuncHeader(GlobalID),

    /// 主干树结点: 基本块的开头名称这一行. 把这个单独拎出来是因为一个基本块实在是太大了,
    /// 全量更新整个基本块的源码图谱树太重了. 这个结点只负责基本块开头的名称这一行, 和基本块的指令结点
    /// 平级, 这样就可以在不更新指令结点的情况下更新基本块开头的名称这一行了.
    BlockIdent(BlockID),
}

impl IRTreeObjID {
    pub fn get_name(&self, ir: &ModuleInfo) -> Result<SmolStr, JsError> {
        let name = match self {
            IRTreeObjID::Module => format_smolstr!("Module {}", ir.module().name),
            IRTreeObjID::Global(global_id) => {
                format_smolstr!("@{}", global_id.get_name(ir.module()))
            }
            IRTreeObjID::FuncArg(global_id, idx) => {
                ValueDt::FuncArg(*global_id, *idx).get_name(ir.module(), ir.names())?
            }
            IRTreeObjID::Block(block_id) => {
                ValueDt::Block(*block_id).get_name(ir.module(), ir.names())?
            }
            IRTreeObjID::Inst(inst_id) => {
                ValueDt::Inst(*inst_id).get_name(ir.module(), ir.names())?
            }
            IRTreeObjID::Use(use_id) => {
                format_smolstr!("Use {}", use_id.get_kind(ir.module()))
            }
            IRTreeObjID::JumpTarget(jt_id) => {
                format_smolstr!("JumpTarget {}", jt_id.get_kind(ir.module()))
            }
            IRTreeObjID::FuncHeader(global_id) => {
                format_smolstr!("FuncHeader @{}", global_id.get_name(ir.module()))
            }
            IRTreeObjID::BlockIdent(block_id) => {
                let block_name = ValueDt::Block(*block_id).get_name(ir.module(), ir.names())?;
                format_smolstr!("BlockIdent %{}", block_name)
            }
        };
        Ok(name)
    }

    pub fn get_class(&self, ir: &ModuleInfo) -> Result<IRTreeNodeClass, JsError> {
        let res = match self {
            IRTreeObjID::Module => IRTreeNodeClass::Module,
            IRTreeObjID::FuncArg(..) => IRTreeNodeClass::FuncArg,
            IRTreeObjID::Block(_) => IRTreeNodeClass::Block,
            IRTreeObjID::Use(_) => IRTreeNodeClass::Use,
            IRTreeObjID::JumpTarget(_) => IRTreeNodeClass::JumpTarget,

            // nodes that are treated as their parents.
            IRTreeObjID::FuncHeader(_) => IRTreeNodeClass::Func,
            IRTreeObjID::BlockIdent(_) => IRTreeNodeClass::Block,

            // nodes that have more than one cases.
            IRTreeObjID::Global(global_id) => {
                let Some(global_obj) = global_id.try_deref_ir(ir.module()) else {
                    return fmt_jserr!(Err "global {global_id:?} does not exist");
                };
                match global_obj {
                    GlobalObj::Func(f) if f.body.is_none() => IRTreeNodeClass::ExternFunc,
                    GlobalObj::Func(_) => IRTreeNodeClass::Func,
                    GlobalObj::Var(_) => IRTreeNodeClass::GlobalVar,
                }
            }
            IRTreeObjID::Inst(inst_id) => {
                let Some(inst) = inst_id.try_deref_ir(ir.module()) else {
                    return fmt_jserr!(Err "inst {inst_id:?} does not exist");
                };
                match inst {
                    InstObj::Phi(_) => IRTreeNodeClass::PhiInst,
                    inst if inst.is_terminator() => IRTreeNodeClass::TerminatorInst,
                    _ => IRTreeNodeClass::NormalInst,
                }
            }
        };
        Ok(res)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Hash)]
#[repr(align(8))]
pub struct SourcePosIndex {
    /// 行号, 从 0 开始
    pub line: u32,
    /// 列号, 从 0 开始, 以字节为单位
    pub col_byte: u32,
}

impl SourcePosIndex {
    pub fn new(line_idx: u32, col_byte: u32) -> Self {
        Self {
            line: line_idx,
            col_byte,
        }
    }

    pub fn zero() -> Self {
        Self::new(0, 0)
    }

    /// 基于当前位置往前数 delta 个位置, 计算出新的位置.
    ///
    /// 注意, 这个 advance 不符合交换律, 因此 SourcePosIndex 没有实现 `Add` trait.
    pub fn advance(self, delta: Self) -> Self {
        if delta.line == 0 {
            Self::new(self.line, self.col_byte + delta.col_byte)
        } else {
            Self::new(self.line + delta.line, delta.col_byte)
        }
    }

    /// 计算从当前位置到目标位置的 delta. 注意, 这个 delta 不符合交换律, 因此 SourcePosIndex 没有实现 `Sub` trait.
    pub fn delta_to(self, other: Self) -> Result<Self, JsError> {
        let Self {
            line: bline,
            col_byte: bcol,
        } = self;
        let Self {
            line: eline,
            col_byte: ecol,
        } = other;
        match (eline.checked_sub(bline), ecol.checked_sub(bcol)) {
            (Some(0), Some(col_delta)) => Ok(Self::new(0, col_delta)),
            (Some(line_delta), _) => Ok(Self::new(line_delta, ecol)),
            _ => fmt_jserr!(Err "other position {other:?} is before self {self:?}"),
        }
    }
}
pub type SourceRangeIndex = Range<SourcePosIndex>;

pub type IRTreeChildren = SmallVec<[IRTreeNodeID; 4]>;
#[entity_id(IRTreeNodeID, backend = index)]
pub struct IRTreeNode {
    parent: Cell<Option<IRTreeNodeID>>,
    pub obj: IRTreeObjID,
    pub children: IRTreeChildren,
    /// 结点相对父结点在源代码中的位置. 采用相对位置是为了减少结点更新.
    pub pos_delta: SourceRangeIndex,
}

impl IRTreeNode {
    pub fn new(obj: IRTreeObjID, pos_delta: SourceRangeIndex) -> Self {
        Self {
            parent: Cell::new(None),
            obj,
            children: SmallVec::new(),
            pos_delta,
        }
    }
    pub fn with_children(
        obj: IRTreeObjID,
        pos_delta: SourceRangeIndex,
        children: IRTreeChildren,
    ) -> Self {
        Self {
            parent: Cell::new(None),
            obj,
            children,
            pos_delta,
        }
    }

    pub fn get_parent(&self) -> Option<IRTreeNodeID> {
        self.parent.get()
    }
    pub fn set_parent(&self, parent_id: IRTreeNodeID) {
        self.parent.set(Some(parent_id));
    }
}

impl Serialize for IRTreeNodeID {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let u48_value: u64 = self.into_gen_index().0.get();
        debug_assert!(
            u48_value <= (1u64 << 48),
            "IRTreeNodeID index exceeds 48 bits: {u48_value}"
        );
        serializer.serialize_u64(u48_value)
    }
}
impl<'de> Deserialize<'de> for IRTreeNodeID {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let u48_value: u64 = Deserialize::deserialize(deserializer)?;
        debug_assert!(
            u48_value <= (1u64 << 48),
            "IRTreeNodeID index exceeds 48 bits: {u48_value}"
        );
        let Some(nzu64) = NonZeroU64::new(u48_value) else {
            return Err(serde::de::Error::custom(format!(
                "invalid IRTreeNodeID index: {u48_value}"
            )));
        };
        Ok(IRTreeNodeID::from_gen_index(GenIndex::from(nzu64)))
    }
}

impl IRTreeNodeID {
    pub fn new(tree: &IRTree, obj: IRTreeObjID, pos_delta: SourceRangeIndex) -> Self {
        Self::allocate(tree, IRTreeNode::new(obj, pos_delta))
    }
    pub fn new_full(
        tree: &IRTree,
        obj: IRTreeObjID,
        pos_delta: SourceRangeIndex,
        children: impl Into<IRTreeChildren>,
    ) -> Self {
        Self::allocate(
            tree,
            IRTreeNode::with_children(obj, pos_delta, children.into()),
        )
    }
    pub fn allocate(tree: &IRTree, node: IRTreeNode) -> Self {
        let node_id = IRTreeNodeID::from_backend(IndexedID::allocate_from(&tree.alloc, node));
        let node = node_id.deref(tree);
        for child in &node.children {
            child.deref(tree).set_parent(node_id);
        }
        debug_assert!(
            tree.check_children_invariant(node_id),
            "children invariant broken when allocating node {:?}",
            node_id
        );
        node_id
    }

    pub fn deref(self, tree: &IRTree) -> &IRTreeNode {
        self.deref_alloc(&tree.alloc)
    }
    pub fn deref_mut(self, tree: &mut IRTree) -> &mut IRTreeNode {
        self.deref_alloc_mut(&mut tree.alloc)
    }

    pub fn to_strid(self) -> SmolStr {
        let index = self.into_gen_index().0.get();
        format_smolstr!("n{:x}", index)
    }

    pub fn obj(self, tree: &IRTree) -> IRTreeObjID {
        self.deref(tree).obj
    }
    pub fn get_parent(self, tree: &IRTree) -> Option<IRTreeNodeID> {
        self.deref(tree).get_parent()
    }
    pub fn children(self, tree: &IRTree) -> &[IRTreeNodeID] {
        self.deref(tree).children.as_slice()
    }
    pub fn pos_delta(self, tree: &IRTree) -> SourceRangeIndex {
        self.deref(tree).pos_delta.clone()
    }
    pub fn pos_delta_len(self, tree: &IRTree) -> Result<SourcePosIndex, JsError> {
        let delta = self.pos_delta(tree);
        delta.end.delta_to(delta.start)
    }

    /// 在主干树模式下, 根据相对该结点的 offset 找到对应的子结点 ID. 如果没有找到, 返回 None.
    pub fn find_child_by_offset(
        self,
        tree: &IRTree,
        offset: SourcePosIndex,
    ) -> Option<IRTreeNodeID> {
        let node = self.deref(tree);
        for &child_id in node.children.iter() {
            let Range { start, end } = child_id.pos_delta(tree);
            if start <= offset && offset < end {
                return Some(child_id);
            }
        }
        None
    }

    /// 树拷贝操作: 把该子树拷贝一份, 修改新子树的源码位置.
    pub fn insert_pos_delta(self, tree: &IRTree, delta: SourceRangeIndex) -> IRTreeNodeID {
        let old_children = self.children(tree);
        let new_children = {
            let mut new_children = IRTreeChildren::with_capacity(old_children.len());
            for &child_id in old_children.iter() {
                new_children.push(tree.clone_subtree(child_id));
            }
            new_children
        };
        let new_node = IRTreeNodeID::new_full(tree, self.obj(tree), delta, new_children);
        for child in new_node.children(tree) {
            child.deref(tree).set_parent(new_node);
        }
        if let Some(parent) = self.deref(tree).get_parent() {
            new_node.deref(tree).set_parent(parent);
        }
        new_node
    }
    pub fn set_pos_delta(self, tree: &mut IRTree, delta: SourceRangeIndex) {
        let node = self.deref_mut(tree);
        if delta == node.pos_delta {
            return;
        }
        node.pos_delta = delta;
    }
}

pub struct IRTree {
    /// IRTreeNodeID 的分配器, 负责管理 IRTreeNode 的内存和 ID 分配
    pub alloc: EntityAlloc<IRTreeNode>,
    /// 主干树的根结点 ID, 对应模块全局对象
    pub root: IRTreeNodeID,
    /// 每个函数下的 IRTreeNodeID, 对应函数对象. 之所以单独存储, 是为了方便快速定位函数结点, 因为函数结点下面的树可能很大.
    pub funcs: HashMap<FuncID, IRTreeNodeID>,
}

impl Default for IRTree {
    fn default() -> Self {
        let alloc = EntityAlloc::new();
        let node = IRTreeNode::new(IRTreeObjID::Module, SourceRangeIndex::default());
        let root = IRTreeNodeID::from_backend(IndexedID::allocate_from(&alloc, node));
        Self {
            alloc,
            root,
            funcs: HashMap::new(),
        }
    }
}

impl IRTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clone_subtree(&self, node_id: IRTreeNodeID) -> IRTreeNodeID {
        fn do_clone(tree: &IRTree, node_id: IRTreeNodeID) -> IRTreeNodeID {
            let node = node_id.deref(tree);
            let mut new_children = IRTreeChildren::with_capacity(node.children.len());
            for &child_id in node.children.iter() {
                new_children.push(do_clone(tree, child_id));
            }
            IRTreeNodeID::new_full(tree, node.obj, node.pos_delta.clone(), new_children)
        }
        do_clone(self, node_id)
    }

    pub fn print_to_dot(&self, root: IRTreeNodeID) -> String {
        use std::fmt::Write;
        let mut output = String::from("digraph IRDag {\n  rankdir=LR;\n  node [shape=box];\n");
        let mut visited = HashSet::new();
        let mut stack = vec![root];
        while let Some(node_id) = stack.pop() {
            if visited.contains(&node_id) {
                continue;
            }
            visited.insert(node_id);
            let node = node_id.deref(self);
            let node_id = node_id.into_gen_index().0.get();
            writeln!(output, "  node_{:x} [label=\"{:?}\"];", node_id, node.obj).unwrap();
            for &child_id in node.children.iter() {
                stack.push(child_id);
                let child_id_num = child_id.into_gen_index().0.get();
                writeln!(output, "  node_{:x} -> node_{:x};", node_id, child_id_num).unwrap();
            }
        }
        output.push_str("}\n");
        output
    }

    pub fn resolve_path(&self, obj_path: &IRObjPath) -> Result<IRTreeNodePathBuf, JsError> {
        let mut node_path = IRTreeNodePathBuf::with_capacity(obj_path.len());
        let mut current_node_id = self.root;
        let mut obj_path_iter = obj_path.iter();
        if obj_path_iter.next() != Some(&IRTreeObjID::Module) {
            return fmt_jserr!(Err "obj_path should start with Module");
        }
        for obj_id in obj_path_iter {
            let current_node = current_node_id.deref(self);
            let found_child_id = current_node
                .children
                .iter()
                .find(|child_id| child_id.deref(self).obj == *obj_id);
            if let Some(&child_id) = found_child_id {
                current_node_id = child_id;
                node_path.push(child_id);
            } else {
                return fmt_jserr!(Err "obj_path not found");
            }
        }
        Ok(node_path)
    }

    /// 根据源代码位置找到对应的结点路径. 结点路径是从根结点到目标结点的 ID 序列. 如果没有找到, 返回 Err.
    pub fn locate_node_path(&self, mut pos: SourcePosIndex) -> Result<IRTreeNodePathBuf, JsError> {
        let mut path = smallvec::smallvec![self.root];
        let mut curr = self.root;
        while let Some(child) = curr.find_child_by_offset(self, pos) {
            let Range { start, end } = child.pos_delta(self);
            if !(start <= pos && pos < end) {
                break;
            }
            pos = pos.delta_to(start)?;
            curr = child;
            path.push(child);
        }
        Ok(path)
    }
    pub fn locate_obj_path(&self, pos: SourcePosIndex) -> Result<IRObjPathBuf, JsError> {
        let node_path = self.locate_node_path(pos)?;
        let mut obj_path = IRObjPathBuf::with_capacity(node_path.len());
        for node_id in node_path {
            let node = node_id.deref(self);
            obj_path.push(node.obj);
        }
        Ok(obj_path)
    }
    pub fn get_path_source_range(
        &self,
        node_path: &IRTreeNodePath,
    ) -> Result<SourceRangeIndex, JsError> {
        let mut pos = SourcePosIndex::zero();
        let mut end = pos;
        for &node_id in node_path {
            let node = node_id.deref(self);
            pos = pos.advance(node.pos_delta.start);
            end = end.advance(node.pos_delta.end);
        }
        Ok(pos..end)
    }

    /// 检查一个父结点的直接子结点是否满足约束:
    /// 1) 按源码范围从前到后排列;
    /// 2) 相邻子结点范围互不重叠(允许首尾相接).
    pub fn check_children_invariant(&self, parent_id: IRTreeNodeID) -> bool {
        let children = parent_id.children(self);
        for win in children.windows(2) {
            let prev = win[0].pos_delta(self);
            let curr = win[1].pos_delta(self);
            if prev.end > curr.start {
                return false;
            }
        }
        true
    }

    /// 从根和函数表出发, DFS 遍历整个 DAG, 找到所有没有被根或函数表直接或间接引用的结点, 进行垃圾回收.
    pub fn gc(&mut self) {
        let mut visited = HashSet::new();
        let mut stack = vec![self.root];
        stack.extend(self.funcs.values().cloned());
        while let Some(node_id) = stack.pop() {
            if visited.contains(&node_id) {
                continue;
            }
            visited.insert(node_id);
            let node = node_id.deref(self);
            for &child_id in node.children.iter() {
                stack.push(child_id);
            }
        }
        self.alloc
            .free_if(|_, _, id| !visited.contains(&IRTreeNodeID::from_backend(id)));
    }
}

pub type IRTreeNodePathBuf = SmallVec<[IRTreeNodeID; 4]>;
pub type IRTreeNodePath = [IRTreeNodeID];

pub type IRObjPathBuf = SmallVec<[IRTreeObjID; 4]>;
pub type IRObjPath = [IRTreeObjID];

#[derive(Debug, Clone)]
#[wasm_bindgen::prelude::wasm_bindgen]
pub struct IRTreeCursor {
    module_id: usize,
    node_path: IRTreeNodePathBuf,
    source_range: Vec<SourceRangeIndex>,
}

impl IRTreeCursor {
    /// 从结点路径创建自己.
    pub fn from_node_path(ir: &ModuleInfo, node_path: impl Into<IRTreeNodePathBuf>) -> Self {
        let node_path = node_path.into();
        let mut source_range = Vec::with_capacity(node_path.len());
        for &node in &node_path {
            source_range.push(node.pos_delta(ir.ir_tree()));
        }
        let mut curr_pos = source_range[0].start;
        for range in source_range.iter_mut().skip(1) {
            let Range { start, end } = range.clone();
            let new_start = curr_pos.advance(start);
            let new_end = curr_pos.advance(end);
            *range = new_start..new_end;
            curr_pos = new_start;
        }
        Self {
            module_id: ir.get_id(),
            node_path,
            source_range,
        }
    }

    pub fn get_last(&self) -> Result<(IRTreeNodeID, SourceRangeIndex), JsError> {
        let Some(last_node) = self.node_path.last() else {
            return fmt_jserr!(Err "invalid empty path");
        };
        let Some(last_range) = self.source_range.last() else {
            return fmt_jserr!(Err "invalid empty path");
        };
        Ok((*last_node, last_range.clone()))
    }

    pub fn do_get_node(&self, ir: &ModuleInfo) -> Result<IRTreeNodeDt, JsError> {
        let (last_node, last_range) = self.get_last()?;
        let tree = ir.ir_tree();
        let obj = last_node.obj(tree);
        Ok(IRTreeNodeDt {
            obj,
            kind: obj.get_class(ir)?,
            label: obj.get_name(ir)?,
            src_range: ir.source().byte_range_to_monaco(last_range.clone())?,
        })
    }

    pub fn do_get_children(&self, ir: &ModuleInfo) -> Result<Vec<IRTreeNodeDt>, JsError> {
        let (last_node, last_range) = self.get_last()?;
        let children = last_node.children(ir.ir_tree());
        let mut ret = Vec::with_capacity(children.len());
        let begin_pos = last_range.start;
        for child in children {
            let tree = ir.ir_tree();
            let obj = child.obj(tree);
            let range_delta = child.pos_delta(tree);
            let src_start = begin_pos.advance(range_delta.start);
            let src_end = begin_pos.advance(range_delta.end);
            ret.push(IRTreeNodeDt {
                obj,
                kind: obj.get_class(ir)?,
                label: obj.get_name(ir)?,
                src_range: ir.source().byte_range_to_monaco(src_start..src_end)?,
            });
        }
        Ok(ret)
    }

    pub fn do_goto_child(&mut self, ir: &ModuleInfo, node: IRTreeNodeID) -> Result<(), JsError> {
        let (last_node, last_range) = self.get_last()?;
        let tree = ir.ir_tree();
        js_assert!(Some(last_node) == node.get_parent(tree))?;

        let range_delta = node.pos_delta(tree);
        let begin_pos = last_range.start;
        let src_start = begin_pos.advance(range_delta.start);
        let src_end = begin_pos.advance(range_delta.end);
        self.node_path.push(node);
        self.source_range.push(src_start..src_end);
        Ok(())
    }
}

#[wasm_bindgen]
impl IRTreeCursor {
    #[wasm_bindgen(constructor)]
    pub fn new_root(ir: &ModuleInfo) -> Self {
        let tree = ir.ir_tree();
        let root = tree.root;
        let source_range = root.deref(tree).pos_delta.clone();
        Self {
            module_id: ir.get_id(),
            node_path: smallvec::smallvec![root],
            source_range: vec![source_range],
        }
    }

    /// 克隆一个新的 Cursor, 共享底层的树结构, 但路径和位置独立. 这个操作很快, 因为树结构是共享的.
    pub fn clone(&self) -> Self {
        Clone::clone(self)
    }

    /// 从对象路径创建自己. 这个操作需要先把对象路径解析成结点路径, 因此可能会比较慢.
    ///
    /// @param {IRTreeObjID[]} objs - 对象路径, 从模块全局对象开始, 每个对象都是前一个对象的直接子对象.
    pub fn from_path(ir: &ModuleInfo, objs: JsIRObjPath) -> Result<Self, JsError> {
        let objs: IRObjPathBuf = ModuleInfo::deserialize(objs)?;
        let nodes = ir.ir_tree().resolve_path(&objs)?;
        Ok(Self::from_node_path(ir, nodes))
    }

    /// 断言当前 Cursor 在所有权上属于该 Module.
    pub fn assert_inside_module(&self, ir: &ModuleInfo) -> Result<(), JsError> {
        js_assert!(self.module_id == ir.get_id())
    }

    /// 获取当前结点的信息. 这个操作需要先检查所有权, 然后把结点信息序列化成 JS 对象.
    ///
    /// @returns {IRTreeNodeDt} 当前结点的信息, 包括对象 ID、结点类型、结点标签和结点对应的源代码范围.
    pub fn get_node(&self, ir: &ModuleInfo) -> Result<JsIRTreeNodeDt, JsError> {
        self.assert_inside_module(ir)?;
        self.do_get_node(ir).and_then(|x| ModuleInfo::serialize(&x))
    }

    /// 获取当前结点的直接子结点的信息列表. 这个操作需要先检查所有权, 然后把子结点信息序列化成 JS 对象.
    pub fn get_children(&self, ir: &ModuleInfo) -> Result<JsIRTreeNodes, JsError> {
        self.assert_inside_module(ir)?;
        self.do_get_children(ir)
            .and_then(|x| ModuleInfo::serialize(&x))
    }

    /// 移动到父结点. 这个操作需要先检查所有权, 然后更新路径和位置.
    pub fn goto_parent(&mut self) -> Result<(), JsError> {
        js_assert!(self.node_path.len() > 1)?;
        js_assert!(self.source_range.len() > 1)?;
        self.node_path.pop();
        self.source_range.pop();
        Ok(())
    }

    /// 移动到子结点. 这个操作需要先检查所有权, 然后检查子结点是否真的属于当前结点, 最后更新路径和位置.
    ///
    /// @param {IRTreeObjID} obj - 目标子结点的对象 ID. 这个对象必须是当前结点的直接子结点, 否则返回 Err.
    pub fn goto_child(&mut self, ir: &ModuleInfo, obj: JsTreeObjID) -> Result<(), JsError> {
        self.assert_inside_module(ir)?;
        let obj: IRTreeObjID = ModuleInfo::deserialize(obj)?;

        let (last_node, _) = self.get_last()?;
        let tree = ir.ir_tree();
        let Some(&child_node) = last_node
            .children(tree)
            .iter()
            .find(|&&child| child.obj(tree) == obj)
        else {
            return fmt_jserr!(Err "target object is not a direct child of current node: {obj:?}");
        };

        self.do_goto_child(ir, child_node)
    }

    /// 检查当前结点是否有某个对象 ID 的直接子结点. 这个操作需要先检查所有权, 然后检查子结点是否真的属于当前结点.
    pub fn has_child(&self, ir: &ModuleInfo, obj: JsTreeObjID) -> Result<bool, JsError> {
        self.assert_inside_module(ir)?;
        let obj: IRTreeObjID = ModuleInfo::deserialize(obj)?;

        let (last_node, _) = self.get_last()?;
        let tree = ir.ir_tree();
        Ok(last_node
            .children(tree)
            .iter()
            .any(|&child| child.obj(tree) == obj))
    }

    /// 根据当前结点路径, 生成对应的对象路径. 这个操作需要先检查所有权, 然后把对象路径序列化成 JS 对象.
    pub fn emit_path(&self, ir: &ModuleInfo) -> Result<JsIRObjPath, JsError> {
        self.assert_inside_module(ir)?;
        let mut obj_path = IRObjPathBuf::with_capacity(self.node_path.len());
        for &node_id in &self.node_path {
            let node = node_id.deref(ir.ir_tree());
            obj_path.push(node.obj);
        }
        ModuleInfo::serialize(&obj_path)
    }
}
