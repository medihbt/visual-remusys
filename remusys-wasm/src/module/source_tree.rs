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
    collections::HashMap,
    num::NonZeroU64,
    ops::{Range, RangeInclusive},
};

use mtb_entity_slab::{IEntityAllocID, IPoliciedID, IndexedID};
use remusys_ir::{
    base::FixBitSet,
    ir::*,
    mtb_entity_slab::{GenIndex, entity_id},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smallvec::{SmallVec, smallvec};

use crate::{
    source_buf::IRSourceBuf,
    source_tree_builder::{IRTreeBuildRes, IRTreeBuilder, IRTreeNodeBuildRes},
};

#[derive(Debug, Clone, thiserror::Error)]
pub enum IRTreeErr {
    #[error("invalid node id {0:?}")]
    InvalidID(IRTreeNodeID),
}
pub type IRTreeRes<T = ()> = Result<T, IRTreeErr>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Hash)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct IRSrcTreePos {
    /// line ID starting from 1
    pub line: u32,
    /// column UTF-16 offset starting from 1
    pub col_u16: u32,
}
pub type IRSrcTreeSpan = Range<IRSrcTreePos>;

impl Default for IRSrcTreePos {
    fn default() -> Self {
        Self {
            line: 1,
            col_u16: 1,
        }
    }
}

pub struct IRTreeDelta {
    pub delta_src: IRSourceBuf,
    pub del_lines: RangeInclusive<usize>,
}

#[derive(Debug, Clone)]
#[entity_id(IRTreeNodeID, allocator_type = IRTreeAlloc, backend = index)]
pub struct IRTreeNode {
    pub parent: Option<IRTreeNodeID>,
    pub ir_obj: IRTreeObjID,
    pub depth: u32,
    pub src_span: Range<IRSrcTreePos>,
    pub children: SmallVec<[IRTreeNodeID; 5]>,
}
impl Serialize for IRTreeNodeID {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // In WASM32, IndexedID only has 48 valid bits (16-bit generation + 32-bit
        // real index), which is less than 52-bit requirement of Javascript
        serializer.serialize_u64(self.into_gen_index().0.get())
    }
}
impl<'de> Deserialize<'de> for IRTreeNodeID {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let inner = NonZeroU64::deserialize(deserializer)?;
        if inner.get() >= (1u64 << 48) {
            return Err(Error::custom(format!("id {inner:x} has more than 48 bits")));
        }
        Ok(Self::from_gen_index(GenIndex(inner)))
    }
}
impl IRTreeNodeID {
    pub fn new(alloc: &IRTreeAlloc, ir_obj: IRTreeObjID, src_span: Range<IRSrcTreePos>) -> Self {
        use mtb_entity_slab::IEntityAllocID;
        let val = IRTreeNode {
            parent: None,
            ir_obj,
            src_span,
            children: SmallVec::new(),
            depth: 0,
        };
        Self(IndexedID::allocate_from(alloc, val))
    }
    pub fn allocate(alloc: &mut IRTreeAlloc, obj: IRTreeNode) -> Self {
        let children: SmallVec<[Self; 16]> = SmallVec::from_slice(&obj.children);
        let ret = Self(IndexedID::allocate_from(alloc, obj));
        for c in children {
            c.set_parent(alloc, ret);
        }
        ret
    }

    pub fn get_parent(self, alloc: &IRTreeAlloc) -> Option<Self> {
        self.deref_alloc(alloc).parent
    }
    pub fn set_parent(self, alloc: &mut IRTreeAlloc, parent: Self) {
        self.deref_alloc_mut(alloc).parent = Some(parent);
    }
    pub fn get_ir_obj(self, alloc: &IRTreeAlloc) -> IRTreeObjID {
        self.deref_alloc(alloc).ir_obj
    }
    pub fn get_src_span(self, alloc: &IRTreeAlloc) -> Range<IRSrcTreePos> {
        self.deref_alloc(alloc).src_span.clone()
    }
    pub fn set_src_span(self, alloc: &mut IRTreeAlloc, span: Range<IRSrcTreePos>) {
        self.deref_alloc_mut(alloc).src_span = span;
    }
    pub fn get_children(self, alloc: &IRTreeAlloc) -> &[Self] {
        self.deref_alloc(alloc).children.as_slice()
    }
    pub fn get_depth(self, alloc: &IRTreeAlloc) -> u32 {
        self.deref_alloc(alloc).depth
    }
}

pub struct IRTree {
    /// 包含所有结点对象的对象池
    pub alloc: IRTreeAlloc,

    /// 为了减少源码更新、结点更新开销, Visual Remusys 的对象树其实是一片森林.
    ///
    /// 有一个表示模块全局的源码 Overview 视图和一组表示函数局部的源码 LocalView
    /// 视图.
    ///
    /// overview 视图不会显示函数定义的函数体.
    pub overview_id: IRTreeNodeID,

    /// 每函数定义一份的局部源码视图.
    ///
    /// 局部源码视图不会显示其他全局对象定义.
    pub local_views: HashMap<FuncID, IRTreeNodeID>,
}
impl Default for IRTree {
    fn default() -> Self {
        Self::new_empty()
    }
}
impl IRTree {
    pub fn new_empty() -> Self {
        let alloc = IRTreeAlloc::new();
        let root_id = IRTreeNodeID::new(&alloc, IRTreeObjID::Module, Range::default());
        Self {
            alloc,
            overview_id: root_id,
            local_views: HashMap::new(),
        }
    }
    pub fn with_overview<'ir>(module: &'ir Module, names: &'ir IRNameMap) -> IRTreeBuildRes<Self> {
        let mut res = Self::new_empty();
        let root = IRTreeBuilder::new(module, names, &mut res).build_overview()?;
        res.overview_id = root;
        Ok(res)
    }
    fn build_dep(&mut self, root: IRTreeNodeID, root_dep: u32) {
        let alloc = &mut self.alloc;
        let mut stk: SmallVec<[_; 16]> = smallvec![(root, root_dep)];
        while let Some((node_id, depth)) = stk.pop() {
            let node = node_id.deref_alloc_mut(alloc);
            node.depth = depth;
            for &child in node.children.iter() {
                stk.push((child, depth + 1));
            }
        }
    }

    pub fn gc_mark_sweep(&mut self) {
        let mut live_set = FixBitSet::<4>::with_len(self.alloc.len());
        let mut stack = Vec::with_capacity(16);
        stack.push(self.overview_id);
        while let Some(node_id) = stack.pop() {
            live_set.enable(node_id.into_gen_index().real_index());
            stack.extend_from_slice(node_id.get_children(&self.alloc));
        }
        self.alloc
            .free_if(|_, _, inner| !live_set.get(inner.get_order()));
    }

    fn update_line_map(&mut self, mut focus: IRTreeNodeID, line_delta: isize) {
        if line_delta == 0 {
            return;
        }
        let alloc = &mut self.alloc;

        while let Some(parent) = focus.get_parent(alloc) {
            let mut span = parent.get_src_span(alloc);
            span.end.line = (span.end.line as isize + line_delta) as u32;
            parent.set_src_span(alloc, span);

            let sibling = parent.get_children(alloc);
            let mut stk: SmallVec<[_; 16]> = SmallVec::new();
            for &child in sibling.iter().rev() {
                if child == focus {
                    break;
                }
                stk.push(child);
            }

            while let Some(child) = stk.pop() {
                let mut span = child.get_src_span(alloc);
                span.start.line = (span.start.line as isize + line_delta) as u32;
                span.end.line = (span.end.line as isize + line_delta) as u32;
                child.set_src_span(alloc, span);
                stk.extend_from_slice(child.get_children(alloc));
            }
            focus = parent;
        }
    }

    fn find_editable(&self, mut node_id: IRTreeNodeID) -> Option<IRTreeNodeID> {
        loop {
            let node = node_id.deref_alloc(&self.alloc);
            match node.ir_obj {
                IRTreeObjID::Block(_)
                | IRTreeObjID::Inst(_)
                | IRTreeObjID::Global(_)
                | IRTreeObjID::Module => return Some(node_id),
                _ => {}
            }
            let parent = node.parent?;
            node_id = parent;
        }
    }
}
