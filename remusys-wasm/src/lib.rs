mod dto;
mod module;
mod tree;
mod types;

pub use hashbrown::{HashMap as BrownMap, HashSet as BrownSet};

pub use self::{
    dto::{call_graph::CallGraphDt, cfg::FuncCfgDt, dfg::BlockDfg, dom::DomTreeDt, *},
    module::{
        ModuleInfo, MonacoSrcPos, MonacoSrcRange, RevLocalNameMap, rename,
        source_buf::{SourceBuf, SourceLine},
    },
    tree::{
        IRObjPath, IRObjPathBuf, IRTree, IRTreeChildren, IRTreeErr, IRTreeNode, IRTreeNodeID,
        IRTreeNodePath, IRTreeNodePathBuf, IRTreeObjID, IRTreeRes, ManagedNodeID, ManagedTreeID,
        ManagedTreeNodeID, SourcePosIndex, SourceRangeIndex, builder::IRTreeBuilder,
    },
};

#[macro_export]
macro_rules! fmt_jserr {
    (Err $($arg:tt)*) => {
        if cfg!(test) || cfg!(not(target_arch = "wasm32")) {
            // 注意一下，在非 wasm 模式下 JsError 不可用，所以直接 panic.
            panic!($($arg)*);
        } else{
            Result::Err(::wasm_bindgen::JsError::new(&format!($($arg)*)))
        }
    };
    ($($arg:tt)*) => {
        ::wasm_bindgen::JsError::new(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! js_todo {
    () => {
        $crate::fmt_jserr!(Err "TODO")
    };
    ($fmt:expr $(, $($arg:tt)*)?) => {
        $crate::fmt_jserr!(Err "TODO: {}", format!($fmt $(, $($arg)*)?))
    };
}

#[macro_export]
macro_rules! js_assert {
    ($cond:expr $(,)?) => {
        if !$cond {
            $crate::fmt_jserr!(Err "assertion failed: {}", stringify!($cond))
        } else {
            Ok(())
        }
    };
    ($cond:expr, $($arg:tt)+) => {
        if !$cond {
            $crate::fmt_jserr!(Err $($arg)+)
        } else {
            Ok(())
        }
    };
}
