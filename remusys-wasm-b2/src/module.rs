use std::{
    collections::HashMap,
    ops::Range,
    path::Path,
    str::FromStr,
    sync::atomic::{AtomicUsize, Ordering},
};

use remusys_ir::{
    SymbolStr,
    ir::{
        BlockID, FuncArgID, FuncID, GlobalID, IRNameMap, ISubGlobalID, ISubInst, ISubInstID,
        InstID, Module, UserID,
    },
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use wasm_bindgen::{JsError, JsValue, prelude::wasm_bindgen};

use crate::{
    CallGraphDt, DomTreeDt, IRDagBuilder, IRObjPathBuf, IRTree, IRTreeObjID, SourceBuf,
    dto::{IRTreeNodeDt, cfg::FuncCfgDt, defuse_graph::DefUseGraphDt, dfg::BlockDfg},
    fmt_jserr,
    rename::IRRename,
    types::{
        JsBlockDfg, JsCallGraphDt, JsDefUseGraph, JsDomTreeDt, JsFuncCfgDt, JsIRObjPath,
        JsIRTreeNodeDt, JsIRTreeNodes, JsMonacoSrcPos, JsRenameRes, JsTreeObjID,
    },
};

pub mod rename;
pub mod source_buf;

/// Monaco-compatible source position, using 1-based line and column numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonacoSrcPos {
    /// 1-based line number
    pub line: u32,
    /// 1-based column number (in UTF-16 code units)
    pub column: u32,
}
pub type MonacoSrcRange = std::ops::Range<MonacoSrcPos>;
impl Default for MonacoSrcPos {
    fn default() -> Self {
        Self { line: 1, column: 1 }
    }
}

pub type RevLocalNameMap = HashMap<SymbolStr, IRTreeObjID>;
#[wasm_bindgen]
pub struct ModuleInfo {
    source: SourceBuf,
    ir_tree: IRTree,
    module: Box<Module>,
    names: IRNameMap,
    // 局部名称映射表, 注意不负责全局对象的名称. 这么做是因为全局名称的前缀 `@` 和局部名称的前缀 `%` 不同,
    // 两个命名空间不一样, 允许同名的全局对象和局部对象.
    rev_local_names: HashMap<FuncID, RevLocalNameMap>,
    id: usize,
}

impl ModuleInfo {
    fn compile_from_ir(source: &str) -> Result<Self, JsError> {
        use remusys_ir_parser::{ModuleWithInfo, source_to_full_ir};
        let ModuleWithInfo { module, namemap } = source_to_full_ir(source)?;
        Self::from_module(module, namemap)
    }
    fn compile_from_sysy(source: &str) -> Result<Self, JsError> {
        use remusys_lang::{ModuleInfo as LangModuleInfo, translate_sysy_text_into_full_ir};
        let info = match translate_sysy_text_into_full_ir(source) {
            Ok(info) => info,
            Err(e) => return fmt_jserr!(Err "Failed to compile SysY source: {e:#?}"),
        };
        let LangModuleInfo { module, names } = info;
        Self::from_module(module, names)
    }
    fn from_module(module: impl Into<Box<Module>>, names: IRNameMap) -> Result<Self, JsError> {
        static ID: AtomicUsize = AtomicUsize::new(0);

        let module = module.into();
        let mut ir_tree = IRTree::new();
        let mut builder = IRDagBuilder::new(module.as_ref(), &names, &ir_tree);
        let root = builder.build(IRTreeObjID::Module)?;
        let source = SourceBuf::from(builder.source_buf);
        ir_tree.root = root;
        Ok(Self {
            source,
            ir_tree,
            module,
            names,
            rev_local_names: HashMap::new(),
            id: ID.fetch_add(1, Ordering::Relaxed),
        })
    }

    // Consume the test module directly. Tests must not clone Module and then reuse old IDs,
    // because cloned EntityAlloc storage can reshuffle positions/generations.
    #[cfg(test)]
    pub(crate) fn from_test_module(module: Module) -> Result<Self, JsError> {
        Self::from_module(module, IRNameMap::default())
    }

    pub fn serialize<T: Serialize, V: From<JsValue>>(value: &T) -> Result<V, JsError> {
        let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
        value
            .serialize(&serializer)
            .map_err(|e| fmt_jserr!("Serialization error: {e:#?}"))
            .map(|v| v.into())
    }
    pub fn deserialize<T: DeserializeOwned>(value: impl Into<JsValue>) -> Result<T, JsError> {
        serde_wasm_bindgen::from_value(value.into())
            .map_err(|e| fmt_jserr!("Deserialization error: {e:#?}"))
    }

    pub fn module(&self) -> &Module {
        &self.module
    }
    pub fn ir_tree(&self) -> &IRTree {
        &self.ir_tree
    }
    pub fn source(&self) -> &SourceBuf {
        &self.source
    }
    pub fn names(&self) -> &IRNameMap {
        &self.names
    }

    pub fn rev_local_names(&mut self, func: FuncID) -> Result<&mut RevLocalNameMap, JsError> {
        let Self {
            module,
            names,
            rev_local_names,
            ..
        } = self;
        use std::collections::hash_map::Entry;
        match rev_local_names.entry(func) {
            Entry::Occupied(map) => Ok(map.into_mut()),
            Entry::Vacant(entry) => {
                let map = Self::make_rev_local_names(module, names, func)?;
                Ok(entry.insert(map))
            }
        }
    }
    pub(super) fn make_rev_local_names(
        module: &Module,
        names: &IRNameMap,
        func: FuncID,
    ) -> Result<RevLocalNameMap, JsError> {
        let mut rev_map = HashMap::new();
        let allocs = &module.allocs;
        if func.is_extern(allocs) {
            let name = func.get_name(allocs);
            return fmt_jserr!(Err "cannot make reverse local name map for extern function: @{name:?}");
        }

        for i in 0..func.args(allocs).len() as u32 {
            let arg_id = FuncArgID(func, i);
            if let Some(arg_name) = names.get_local_name(arg_id) {
                rev_map.insert(arg_name.clone(), IRTreeObjID::FuncArg(func.raw_into(), i));
            }
        }

        for (bb_id, bb) in func.blocks_iter(allocs) {
            if let Some(bb_name) = names.get_local_name(bb_id) {
                rev_map.insert(bb_name.clone(), IRTreeObjID::Block(bb_id));
            }
            for (inst_id, _) in bb.insts_iter(allocs) {
                if let Some(inst_name) = names.get_local_name(inst_id) {
                    rev_map.insert(inst_name.clone(), IRTreeObjID::Inst(inst_id));
                }
            }
        }
        Ok(rev_map)
    }

    fn global_strid_as_func(&self, global_id: &str) -> Result<FuncID, JsError> {
        let global_id = match GlobalID::from_str(global_id) {
            Ok(id) => id,
            Err(e) => return fmt_jserr!(Err "invalid global id: {global_id:?}, error: {e:#?}"),
        };
        match FuncID::try_from_global(&self.module, global_id) {
            Some(func_id) => Ok(func_id),
            None => fmt_jserr!(Err "global id does not refer to a function: {global_id:?}"),
        }
    }
    pub fn path_vec_of_tree_object(&self, obj: IRTreeObjID) -> Result<Vec<IRTreeObjID>, JsError> {
        fn path_of_block(module: &Module, block_id: BlockID) -> Result<Vec<IRTreeObjID>, JsError> {
            let Some(block) = block_id.try_deref_ir(module) else {
                return fmt_jserr!(Err "invalid block id: {block_id:?}");
            };
            let Some(func_id) = block.get_parent_func() else {
                return fmt_jserr!(Err "block does not belong to any function: {block_id:?}");
            };
            Ok(vec![
                IRTreeObjID::Module,
                IRTreeObjID::Global(func_id.raw_into()),
                IRTreeObjID::Block(block_id),
            ])
        }
        fn path_of_inst(module: &Module, inst_id: InstID) -> Result<Vec<IRTreeObjID>, JsError> {
            let Some(inst) = inst_id.try_deref_ir(module) else {
                return fmt_jserr!(Err "invalid instruction id: {inst_id:?}");
            };
            let Some(block) = inst.get_parent() else {
                return fmt_jserr!(Err "instruction does not belong to any block: {inst_id:?}");
            };
            let mut res = path_of_block(module, block)?;
            res.push(IRTreeObjID::Inst(inst_id));
            Ok(res)
        }
        let path = match obj {
            IRTreeObjID::Module => vec![IRTreeObjID::Module],
            IRTreeObjID::Global(global_id) => {
                vec![IRTreeObjID::Module, IRTreeObjID::Global(global_id)]
            }
            IRTreeObjID::FuncArg(global_id, idx) => vec![
                IRTreeObjID::Module,
                IRTreeObjID::Global(global_id),
                IRTreeObjID::FuncHeader(global_id),
                IRTreeObjID::FuncArg(global_id, idx),
            ],
            IRTreeObjID::Block(block_id) => path_of_block(&self.module, block_id)?,
            IRTreeObjID::Inst(inst_id) => path_of_inst(&self.module, inst_id)?,
            IRTreeObjID::BlockIdent(block_id) => {
                let mut res = path_of_block(&self.module, block_id)?;
                res.push(IRTreeObjID::BlockIdent(block_id));
                res
            }
            IRTreeObjID::FuncHeader(global_id) => vec![
                IRTreeObjID::Module,
                IRTreeObjID::Global(global_id),
                IRTreeObjID::FuncHeader(global_id),
            ],
            IRTreeObjID::JumpTarget(jt_id) => {
                let Some(jt) = jt_id.try_deref_ir(&self.module) else {
                    return fmt_jserr!(Err "invalid jump target id: {jt_id:?}");
                };
                let Some(inst) = jt.terminator.get() else {
                    return fmt_jserr!(Err "jump target does not belong to any instruction: {jt_id:?}");
                };
                let mut res = path_of_inst(&self.module, inst)?;
                res.push(IRTreeObjID::JumpTarget(jt_id));
                res
            }
            IRTreeObjID::Use(use_id) => {
                let Some(use_obj) = use_id.try_deref_ir(&self.module) else {
                    return fmt_jserr!(Err "invalid use id: {use_id:?}");
                };
                let Some(UserID::Inst(inst)) = use_obj.user.get() else {
                    return Ok(vec![]);
                };
                let mut res = path_of_inst(&self.module, inst)?;
                res.push(IRTreeObjID::Use(use_id));
                res
            }
        };
        Ok(path)
    }
}

#[wasm_bindgen]
impl ModuleInfo {
    /// 从给定的源代码编译出一个 ModuleInfo. `ty` 参数指定了源代码的类型, 目前支持 "ir" 和 "sysy".
    ///
    /// @param {"ir" | "sysy"} ty - 源代码的类型, 可以是 "ir" 或 "sysy".
    pub fn compile_from(ty: &str, source: &str, filename: &str) -> Result<Self, JsError> {
        let mut res = match ty {
            "ir" => Self::compile_from_ir(source),
            "sysy" => Self::compile_from_sysy(source),
            ty => fmt_jserr!(Err "unsupported source type: {ty}"),
        }?;
        let module_name = Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("input");
        res.module.name = module_name.to_string();
        Ok(res)
    }

    /// 导出当前 ModuleInfo 中的源代码文本. 这个文本可以用来在 Monaco 编辑器中显示和编辑.
    pub fn dump_source(&self) -> String {
        self.source.to_string()
    }

    /// 获取当前 Module 的临时唯一 ID.
    pub fn get_id(&self) -> usize {
        self.id
    }

    /// 给定一个 Monaco 编辑器中的位置, 返回对应的 IR 对象路径. 这个路径可以用来在 JS 侧定位和高亮显示对应的 IR 对象.
    ///
    /// @param {MonacoSrcPos} pos - Monaco 编辑器中的位置, 包含 line 和 column 两个字段, 都是 1-based 的.
    pub fn path_of_srcpos(&self, pos: JsMonacoSrcPos) -> Result<JsIRObjPath, JsError> {
        let monaco_pos: MonacoSrcPos = Self::deserialize(pos)?;
        let byte_pos = self.source.monaco_pos_to_byte(monaco_pos)?;
        let obj_path = self.ir_tree.locate_obj_path(byte_pos)?;
        Self::serialize(&obj_path.as_slice())
    }

    /// IR Tree 加载器: 给定一个 IR 对象路径, 返回对应树结点的信息（包括它代表的 IR 对象、它的类型、它在源代码中的范围等）.
    ///
    /// @param {IRTreeObjID[]} path - 对象路径, 从模块全局对象开始, 每个对象都是前一个对象的直接子对象.
    /// @return {IRTreeNodeDt} 树结点信息.
    pub fn path_get_node(&self, path: JsIRObjPath) -> Result<JsIRTreeNodeDt, JsError> {
        let obj_path: IRObjPathBuf = Self::deserialize(path)?;
        let node_path = self.ir_tree.resolve_path(&obj_path)?;
        let Some(node_id) = node_path.last() else {
            return fmt_jserr!(Err "object path cannot be empty");
        };
        let node_obj = node_id.obj(&self.ir_tree);
        let byte_range = self.ir_tree.get_path_source_range(&node_path)?;
        let monaco_range = self.source.byte_range_to_monaco(byte_range)?;
        let node = IRTreeNodeDt {
            obj: node_obj,
            kind: node_obj.get_class(self)?,
            label: node_obj.get_name(self)?,
            src_range: monaco_range,
        };
        Self::serialize(&node)
    }

    /// IR Tree 加载器: 通过结点的 path 加载对应树结点的子结点.
    ///
    /// @param {IRTreeObjID[]} path - 对象路径, 从模块全局对象开始, 每个对象都是前一个对象的直接子对象.
    /// @return {IRTreeNodeDt[]} 子结点信息数组.
    pub fn ir_tree_get_children(&self, path: JsIRObjPath) -> Result<JsIRTreeNodes, JsError> {
        let obj_path: IRObjPathBuf = Self::deserialize(path)?;
        let mut node_path = self.ir_tree.resolve_path(&obj_path)?;
        let Some(last) = node_path.pop() else {
            return fmt_jserr!(Err "object path cannot be empty");
        };
        let children_id = last.children(&self.ir_tree);
        let Range { start, .. } = self.ir_tree.get_path_source_range(&node_path)?;
        let mut res = Vec::with_capacity(children_id.len());
        for child_id in children_id {
            let child_delta = child_id.pos_delta(&self.ir_tree);
            let child_pos_begin = start.advance(child_delta.start);
            let child_pos_end = start.advance(child_delta.end);
            let child_range = self
                .source
                .byte_range_to_monaco(child_pos_begin..child_pos_end)?;
            let obj_id = child_id.obj(&self.ir_tree);
            res.push(IRTreeNodeDt {
                kind: obj_id.get_class(self)?,
                obj: obj_id,
                src_range: child_range,
                label: obj_id.get_name(self)?,
            });
        }
        Self::serialize(&res)
    }

    /// 重命名一个 IR 对象（函数、基本块、指令等）. JS 侧需要废弃所有缓存, 重新构建 IRDag 和相关数据结构.
    ///
    /// @param {IRTreeObjID[]} path - 对象路径.
    /// @param {string} new_name - 新名称, 不需要带前缀 `%` 或 `@`.
    pub fn rename(&mut self, path: JsIRObjPath, new_name: &str) -> Result<JsRenameRes, JsError> {
        let obj_path: IRObjPathBuf = Self::deserialize(path)?;
        let Some(last) = obj_path.last().cloned() else {
            return fmt_jserr!(Err "object path cannot be empty");
        };
        IRRename::new(self, last)
            .rename(new_name)
            .and_then(|x| Self::serialize(&x))
    }

    /// 通过一个 IR 对象 ID 获取它在 IR Tree 中的路径.
    ///
    /// 对于可能出现一对多映射的情况, 会返回一个错误.
    ///
    /// @param {IRTreeObjID} object_id - IR 对象 ID, 可以是模块、函数、基本块、指令等对象的 ID.
    pub fn path_of_tree_object(&self, object_id: JsTreeObjID) -> Result<JsIRObjPath, JsError> {
        let object: IRTreeObjID = Self::deserialize(object_id)?;
        let path = self.path_vec_of_tree_object(object)?;
        if path.is_empty() {
            return fmt_jserr!(Err "object is not part of the IR tree: {object:?}");
        }
        Self::serialize(&path)
    }

    /// 通过一个 IR 对象 ID 获取它的作用域.
    ///
    /// @param {IRTreeObjID} object_id - IR 对象 ID, 可以是模块、函数、基本块、指令等对象的 ID.
    pub fn get_object_scope(&self, object_id: JsTreeObjID) -> Result<String, JsError> {
        let object: IRTreeObjID = Self::deserialize(object_id)?;
        let path = self.path_vec_of_tree_object(object)?;
        if path.len() < 2 {
            return fmt_jserr!(Err "object does not have a scope: {object:?}");
        }
        let IRTreeObjID::Global(scope) = path[1] else {
            return fmt_jserr!(Err "object does not have a global scope: {object:?}");
        };
        Ok(scope.to_strid().to_string())
    }
}

/// Module -- the graph maker
#[wasm_bindgen]
impl ModuleInfo {
    /// 获取指定函数的控制流图.
    ///
    /// @param {GlobalID} func_id - 函数的全局 ID, 内存池索引的序列化版本, 和函数的名称表示一点关系也没有.
    /// @return {FuncCfgDt} 函数的控制流图数据.
    pub fn get_func_cfg(&self, func_id: &str) -> Result<JsFuncCfgDt, JsError> {
        let func_id = self.global_strid_as_func(func_id)?;
        FuncCfgDt::new(&self.module, &self.names, func_id)
            .and_then(|cfg_dt| Self::serialize(&cfg_dt))
    }

    /// 获取指定函数的支配树. 注意目前只支持支配树, 不支持后支配树.
    ///
    /// @param {GlobalID} func_id - 函数的全局 ID, 内存池索引的序列化版本, 和函数的名称表示一点关系也没有.
    /// @return {DomTreeDt} 函数的支配树数据.
    pub fn get_func_dom_tree(&self, func_id: &str) -> Result<JsDomTreeDt, JsError> {
        let func_id = self.global_strid_as_func(func_id)?;
        DomTreeDt::new(&self.module, func_id).and_then(|dt| Self::serialize(&dt))
    }

    /// 获取指定基本块的数据流图. 目前只支持单个基本块内的局部数据流, 不包含跨基本块的参数传递等数据流.
    ///
    /// @param {BlockID} block_id - 基本块的 ID, 内存池索引的序列化版本, 和基本块的名称表示一点关系也没有.
    /// @return {BlockDfgDt} 基本块的数据流图数据.
    pub fn get_block_dfg(&self, block_id: &str) -> Result<JsBlockDfg, JsError> {
        let block_id = match BlockID::from_str(block_id) {
            Ok(id) => id,
            Err(e) => return fmt_jserr!(Err "invalid block id: {block_id:?}, error: {e:#?}"),
        };
        BlockDfg::new(self, block_id).and_then(|dfg| Self::serialize(&dfg))
    }

    /// 获取以某个指令为中心的 Def-Use 图. 这个图包含了该指令的所有直接操作数和所有直接用户, 以及它们之间的连接关系.
    /// 注意这个图可能包含多条重边, 因为一个指令可能多次使用同一个操作数, 也可能被同一个用户多次使用.
    /// 目前这个图只包含直接的 Def-Use 关系, 不包含跨基本块的数据流关系
    pub fn get_def_use_graph(&self, inst_id: &str) -> Result<JsDefUseGraph, JsError> {
        let inst_id = match InstID::from_str(inst_id) {
            Ok(id) => id,
            Err(e) => return fmt_jserr!(Err "invalid instruction id: {inst_id:?}, error: {e:#?}"),
        };
        DefUseGraphDt::new(self, inst_id).and_then(|dg| Self::serialize(&dg))
    }

    /// 获取整个模块的函数调用图. 这个调用图是一个有向图, 会合并重边.
    ///
    /// @return {CallGraphDt} 模块的函数调用图数据.
    pub fn get_call_graph(&self) -> Result<JsCallGraphDt, JsError> {
        CallGraphDt::new(self).and_then(|cg| Self::serialize(&cg))
    }
}
