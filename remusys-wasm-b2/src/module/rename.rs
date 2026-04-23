use std::collections::HashMap;

use remusys_ir::{SymbolStr, ir::*};
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsError;

use crate::{IRTreeBuilder, IRTree, IRTreeObjID, ModuleInfo, RevLocalNameMap, SourceBuf, fmt_jserr};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RenameRes {
    Renamed,
    NoChange,
    GlobalNameConflict { name: SymbolStr },
    LocalNameConflict { name: SymbolStr },
    UnnamedObject,
}

pub struct IRRename<'ir> {
    module: &'ir mut Module,
    names: &'ir mut IRNameMap,
    rev_names: &'ir mut HashMap<FuncID, RevLocalNameMap>,
    tree: &'ir mut IRTree,
    source: &'ir mut SourceBuf,
    object: IRTreeObjID,
}

impl<'ir> IRRename<'ir> {
    pub fn new(module_info: &'ir mut ModuleInfo, object: IRTreeObjID) -> Self {
        let ModuleInfo {
            module,
            names,
            ir_tree: dag,
            source,
            rev_local_names: rev_names,
            ..
        } = module_info;
        Self {
            module,
            names,
            rev_names,
            tree: dag,
            source,
            object,
        }
    }

    /// ## 返回
    ///
    /// - `true`: 重命名成功, 已经修改了 IRDag 和相关数据结构. JS 侧需要废弃所有缓存, 重新构建 IRDag 和相关数据结构.
    /// - `false`: 重命名失败, 什么都没做. JS 侧不需要做任何事情.
    /// - `Err`: 出现错误, 可能是输入不合法等. JS 侧可以弹出错误提示等.
    pub fn rename(&mut self, new_name: &str) -> Result<RenameRes, JsError> {
        match self.object {
            IRTreeObjID::Module => {
                self.module.name = new_name.to_string();
                Ok(RenameRes::NoChange) // 模块重命名不影响 IRDag 结构, 不需要 JS 侧重建 IRDag 和相关数据结构.
            }
            IRTreeObjID::Global(global_id) | IRTreeObjID::FuncHeader(global_id) => {
                self.rename_global(global_id, new_name)
            }
            IRTreeObjID::FuncArg(func, idx) => self.rename_arg(func, idx, new_name),
            IRTreeObjID::Block(block_id) | IRTreeObjID::BlockIdent(block_id) => {
                self.rename_block(block_id, new_name)
            }
            IRTreeObjID::JumpTarget(jt_id) => {
                let Some(block_id) = jt_id.get_block(&self.module.allocs) else {
                    return fmt_jserr!(Err "jump target does not refer to a block: {:?}", jt_id);
                };
                self.rename_block(block_id, new_name)
            }
            IRTreeObjID::Inst(inst_id) => self.rename_inst(inst_id, new_name),
            IRTreeObjID::Use(use_id) => self.rename_use(use_id, new_name),
        }
    }

    fn revmap_mut(&mut self, func: FuncID) -> Result<&mut RevLocalNameMap, JsError> {
        let Self {
            module,
            names,
            rev_names,
            ..
        } = self;
        use std::collections::hash_map::Entry;
        match rev_names.entry(func) {
            Entry::Occupied(map) => Ok(map.into_mut()),
            Entry::Vacant(entry) => {
                let map = ModuleInfo::make_rev_local_names(module, names, func)?;
                Ok(entry.insert(map))
            }
        }
    }

    fn rename_global(&mut self, id: GlobalID, new_name: &str) -> Result<RenameRes, JsError> {
        let symbols = self.module.symbols.get_mut();
        if id.get_name(&self.module.allocs) == new_name {
            return Ok(RenameRes::NoChange);
        }
        if symbols.get_symbol_by_name(new_name).is_some() {
            return Ok(RenameRes::GlobalNameConflict {
                name: new_name.into(),
            });
        }
        if symbols.is_id_exported(id) {
            symbols.unexport_symbol(id, &self.module.allocs);
        }
        id.deref_ir_mut(&mut self.module.allocs).common_mut().name = new_name.into();
        id.export(self.module)
            .map_err(|e| fmt_jserr!("Another global object {e:?} has name {new_name}"))?;
        self.full_update()?;
        Ok(RenameRes::Renamed)
    }
    fn rename_arg(
        &mut self,
        func_id: GlobalID,
        arg_idx: u32,
        new_name: &str,
    ) -> Result<RenameRes, JsError> {
        let Some(func_id) = FuncID::try_from_global(&self.module.allocs, func_id) else {
            return fmt_jserr!(Err "object is not a function: {:?}", func_id);
        };
        let old_name = self.names.get_local_name(FuncArgID(func_id, arg_idx));
        if old_name.as_deref() == Some(new_name) {
            return Ok(RenameRes::NoChange);
        }
        let rev_map = self.revmap_mut(func_id)?;
        if rev_map.contains_key(new_name) {
            return Ok(RenameRes::LocalNameConflict {
                name: new_name.into(),
            });
        }
        if let Some(old_name) = old_name {
            rev_map.remove(&old_name);
        }
        rev_map.insert(
            new_name.into(),
            IRTreeObjID::FuncArg(func_id.raw_into(), arg_idx),
        );
        self.names
            .set_func_arg(func_id, arg_idx as usize, new_name.into());
        self.full_update()?;
        Ok(RenameRes::Renamed)
    }
    fn rename_block(&mut self, block_id: BlockID, new_name: &str) -> Result<RenameRes, JsError> {
        let allocs = &self.module.allocs;
        let Some(func_id) = block_id.get_parent_func(allocs) else {
            return fmt_jserr!(Err "block does not belong to any function: {:?}", block_id);
        };
        let old_name = self.names.get_local_name(block_id);
        if old_name.as_deref() == Some(new_name) {
            return Ok(RenameRes::NoChange);
        }
        let rev_map = self.revmap_mut(func_id)?;
        if rev_map.contains_key(new_name) {
            return Ok(RenameRes::LocalNameConflict {
                name: new_name.into(),
            });
        }
        if let Some(old_name) = old_name {
            rev_map.remove(&old_name);
        }
        rev_map.insert(new_name.into(), IRTreeObjID::Block(block_id));
        self.names.insert_block(block_id, new_name.into());
        self.full_update()?;
        Ok(RenameRes::Renamed)
    }
    fn rename_inst(&mut self, inst_id: InstID, new_name: &str) -> Result<RenameRes, JsError> {
        let allocs = &self.module.allocs;
        let Some(func_id) = inst_id.get_parent_func(allocs) else {
            return fmt_jserr!(Err "instruction does not belong to any function: {:?}", inst_id);
        };
        let old_name = self.names.get_local_name(inst_id);
        if old_name.as_deref() == Some(new_name) {
            return Ok(RenameRes::NoChange);
        }
        let rev_map = self.revmap_mut(func_id)?;
        if rev_map.contains_key(new_name) {
            return Ok(RenameRes::LocalNameConflict {
                name: new_name.into(),
            });
        }
        if let Some(old_name) = old_name {
            rev_map.remove(&old_name);
        }
        rev_map.insert(new_name.into(), IRTreeObjID::Inst(inst_id));
        self.names.insert_inst(inst_id, new_name.into());
        self.full_update()?;
        Ok(RenameRes::Renamed)
    }
    fn rename_use(&mut self, use_id: UseID, new_name: &str) -> Result<RenameRes, JsError> {
        let allocs = &self.module.allocs;
        match use_id.get_operand(allocs) {
            ValueSSA::FuncArg(func_id, idx) => self.rename_arg(func_id.raw_into(), idx, new_name),
            ValueSSA::Block(block_id) => self.rename_block(block_id, new_name),
            ValueSSA::Inst(inst_id) => self.rename_inst(inst_id, new_name),
            ValueSSA::Global(global_id) => self.rename_global(global_id, new_name),
            _ => Ok(RenameRes::UnnamedObject), // 我问你没有身份的对象哪来的名字
        }
    }

    /// 权宜之计, 先实现一个全量更新的函数, 先改名字, 然后重建整个 IRDag 和相关数据结构. 之后再优化成增量更新.
    fn full_update(&mut self) -> Result<(), JsError> {
        let allocs = &self.module.allocs;

        let mut builder = IRTreeBuilder::new(self.module, self.names, self.tree);
        let new_root = builder.build(IRTreeObjID::Module)?;
        *self.source = SourceBuf::from(builder.source_buf.as_str());
        self.tree.root = new_root;
        let mut funcs = HashMap::new();
        for id in new_root.children(self.tree) {
            let IRTreeObjID::Global(gid) = id.obj(self.tree) else {
                continue;
            };
            let Some(func_id) = FuncID::try_from_global(allocs, gid) else {
                continue;
            };
            if !func_id.is_extern(allocs) {
                funcs.insert(func_id, *id);
            }
        }
        self.tree.funcs = funcs;
        Ok(())
    }
}
