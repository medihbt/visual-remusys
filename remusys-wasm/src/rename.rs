use remusys_ir::ir::*;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;

use crate::{ModuleInfo, SourceTrackable};

#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
pub enum RenameErr {
    #[error("ID {0:?} is invalid or does not exist")]
    InvalidID(SourceTrackable),

    #[error("The new name '{0:?}' is not a valid identifier")]
    InvalidName(SmolStr),

    #[error("The new name '{name}' is already used by {existed:?}")]
    NameRepeated {
        name: SmolStr,
        existed: SourceTrackable,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RenameDelta {
    pub invalidate_overview: bool,
    pub invalidated: Vec<GlobalID>,
}

/// 函数作用域内的所有名称信息，供 JS 前端管理作用域和查重
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FunctionNames {
    /// 参数名称映射：参数索引 -> 名称
    pub args: HashMap<u32, SmolStr>,
    /// 指令名称映射：指令ID -> 名称
    pub insts: HashMap<InstID, SmolStr>,
    /// 基本块名称映射：基本块ID -> 名称
    pub blocks: HashMap<BlockID, SmolStr>,
}

pub type RenameRes = Result<RenameDelta, RenameErr>;

impl ModuleInfo {
    /// 在重命名操作前清理名称映射中的无效条目
    fn gc_names(&mut self) {
        self.names.gc(&self.module.allocs);
    }

    pub fn rename(&mut self, id: SourceTrackable, new_name: &str) -> RenameRes {
        // 首先清理无效的名称映射
        self.gc_names();

        if !id.is_alive(&self.module) {
            return Err(RenameErr::InvalidID(id));
        }
        if !utils::name_is_ir_ident(new_name) {
            return Err(RenameErr::InvalidName(SmolStr::new(new_name)));
        }
        let renamed = match id {
            SourceTrackable::Global(global_id) => self.rename_global(global_id, new_name),
            SourceTrackable::Block(block_id) => self.rename_block(block_id, new_name),
            SourceTrackable::Inst(inst_id) => self.rename_inst(inst_id, new_name),
            SourceTrackable::Use(use_id) => self.rename_use(use_id, new_name),
            SourceTrackable::JumpTarget(jt_id) => {
                let Some(block) = jt_id.get_block(&self.module.allocs) else {
                    return Err(RenameErr::InvalidID(id));
                };
                self.rename_block(block, new_name)
            }
            SourceTrackable::FuncArg(func, index) => {
                let Some(func_id) = FuncID::try_from_global(&self.module.allocs, func) else {
                    return Err(RenameErr::InvalidID(id));
                };
                self.rename_func_arg(func_id, index, new_name)
            }
            SourceTrackable::Expr(_) => Self::no_need_to_rename(),
        }?;

        if renamed.invalidate_overview {
            self.invalidate_overview();
        }
        Ok(renamed)
    }

    /// 重命名全局对象 (函数或者全局变量)
    ///
    /// 涉及的作用域:
    ///
    /// - Module 全局作用域
    /// - 该全局对象如果是函数, 则涉及 Function 作用域
    /// - 该全局对象的引用处所处的作用域. 比如, 一个函数被其他函数调用, 那该函数重命名时, 调用处的缓存一并被废弃
    ///
    /// 修改的 Module 区域
    ///
    /// - GlobalID 自己所指的全局对象
    /// - 如果 GlobalID 被导出了 (放到 module.symtab.exported), 则需要卸载并重新导出 (因为导出表里是名字到 ID 的映射)
    fn rename_global(&mut self, id: GlobalID, name: &str) -> RenameRes {
        let allocs = &self.module.allocs;

        // 如果名称相同，无需操作
        if id.get_name(allocs) == name {
            return Self::no_need_to_rename();
        }

        // 检查名称是否已被其他全局对象使用
        if let Some(existing_id) = self.module.get_global_by_name(name)
            && existing_id != id
        {
            return Err(RenameErr::NameRepeated {
                name: SmolStr::new(name),
                existed: SourceTrackable::Global(existing_id),
            });
        }

        // 处理导出状态
        let was_exported = id.unexport(&self.module);
        if was_exported {
            // 重新导出新名称
            id.rename_and_export(name, &mut self.module)
                .expect("failed to rename and export global");
        } else {
            // 只修改名称，不涉及导出表
            id.deref_ir_mut(&mut self.module.allocs).common_mut().name = SmolStr::new(name);
        }

        // 使引用该全局对象的作用域失效
        let mut delta = self.invalidate_user_scopes(ValueSSA::Global(id))?;
        delta.invalidate_overview = true;
        Ok(delta)
    }

    fn rename_block(&mut self, id: BlockID, name: &str) -> RenameRes {
        let name_str = SmolStr::new(name);

        // 检查当前名称是否相同
        if let Some(old_name) = self.names.blocks.get(&id)
            && old_name == &name_str
        {
            return Self::no_need_to_rename();
        }

        // 检查名称在父函数内是否已被使用
        if let Some(parent_func) = id.get_parent_func(&self.module.allocs)
            && self.is_name_used_in_function(
                parent_func,
                &name_str,
                Some(SourceTrackable::Block(id)),
            )
        {
            return Err(RenameErr::NameRepeated {
                name: name_str.clone(),
                existed: SourceTrackable::Block(id),
            });
        }

        // 更新名称映射
        self.names.blocks.insert(id, name_str);

        // 使引用该基本块的作用域失效
        self.invalidate_user_scopes(ValueSSA::Block(id))
    }

    fn rename_inst(&mut self, id: InstID, name: &str) -> RenameRes {
        let name_str = SmolStr::new(name);

        // 检查当前名称是否相同
        if let Some(old_name) = self.names.insts.get(&id)
            && old_name == &name_str
        {
            return Self::no_need_to_rename();
        }

        // 检查名称在父函数内是否已被使用
        if let Some(parent_func) = id.get_parent_func(&self.module.allocs)
            && self.is_name_used_in_function(
                parent_func,
                &name_str,
                Some(SourceTrackable::Inst(id)),
            )
        {
            return Err(RenameErr::NameRepeated {
                name: name_str.clone(),
                existed: SourceTrackable::Inst(id),
            });
        }

        // 更新名称映射
        self.names.insts.insert(id, name_str);

        // 使引用该指令的作用域失效
        self.invalidate_user_scopes(ValueSSA::Inst(id))
    }

    fn rename_func_arg(&mut self, func: FuncID, index: u32, name: &str) -> RenameRes {
        let name_str = SmolStr::new(name);
        let allocs = &self.module.allocs;

        // 验证参数索引有效
        let func_obj = func.deref_ir(allocs);
        let arg_count = func_obj.args.len() as u32;
        if index >= arg_count {
            return Err(RenameErr::InvalidID(SourceTrackable::FuncArg(
                func.raw_into(),
                index,
            )));
        }

        // 在修改前先检查名称是否在函数内已被使用
        let name_check_result = {
            if self.is_name_used_in_function(
                func,
                &name_str,
                Some(SourceTrackable::FuncArg(func.raw_into(), index)),
            ) {
                return Err(RenameErr::NameRepeated {
                    name: name_str.clone(),
                    existed: SourceTrackable::FuncArg(func.raw_into(), index),
                });
            }
            true
        };

        if !name_check_result {
            return Ok(RenameDelta::default());
        }

        // 获取或创建函数参数名称数组
        let args = self
            .names
            .funcs
            .entry(func)
            .or_insert_with(|| vec![None; arg_count as usize].into_boxed_slice());

        // 确保数组大小正确
        if args.len() <= index as usize {
            let mut new_args = args.to_vec();
            new_args.resize(index as usize + 1, None);
            *args = new_args.into_boxed_slice();
        }

        // 检查当前名称是否相同
        if let Some(old_name) = &args[index as usize]
            && old_name == &name_str
        {
            return Self::no_need_to_rename();
        }

        // 更新名称
        args[index as usize] = Some(name_str);

        // 使引用该函数参数的作用域失效
        self.invalidate_user_scopes(ValueSSA::FuncArg(func, index))
    }

    fn rename_use(&mut self, id: UseID, name: &str) -> RenameRes {
        match id.get_operand(&self.module.allocs) {
            ValueSSA::Inst(inst) => self.rename_inst(inst, name),
            ValueSSA::Global(glob) => self.rename_global(glob, name),
            ValueSSA::Block(bb) => self.rename_block(bb, name),
            ValueSSA::FuncArg(func, idx) => self.rename_func_arg(func, idx, name),
            _ => Ok(RenameDelta::default()), // 其他类型的 ValueSSA 不需要重命名
        }
    }

    fn no_need_to_rename() -> RenameRes {
        Ok(RenameDelta::default())
    }

    /// 检查名称在函数内是否已被使用
    fn is_name_used_in_function(
        &self,
        func: FuncID,
        name: &SmolStr,
        exclude: Option<SourceTrackable>,
    ) -> bool {
        // 检查参数名称
        if let Some(args) = self.names.funcs.get(&func) {
            for (i, arg_name) in args.iter().enumerate() {
                if arg_name.as_ref() == Some(name) {
                    if let Some(SourceTrackable::FuncArg(ex_func, ex_idx)) = exclude
                        && let Some(ex_func_id) =
                            FuncID::try_from_global(&self.module.allocs, ex_func)
                        && ex_func_id == func
                        && ex_idx as usize == i
                    {
                        continue; // 排除自身
                    }
                    return true;
                }
            }
        }

        // 检查指令名称
        for (inst_id, inst_name) in &self.names.insts {
            if inst_name == name {
                // 检查指令是否属于该函数
                if let Some(parent_func) = inst_id.get_parent_func(&self.module.allocs)
                    && parent_func == func
                {
                    if let Some(SourceTrackable::Inst(ex_inst)) = exclude
                        && ex_inst == *inst_id
                    {
                        continue; // 排除自身
                    }
                    return true;
                }
            }
        }

        // 检查基本块名称
        for (block_id, block_name) in &self.names.blocks {
            if block_name == name {
                // 检查基本块是否属于该函数
                if let Some(parent_func) = block_id.get_parent_func(&self.module.allocs)
                    && parent_func == func
                {
                    if let Some(SourceTrackable::Block(ex_block)) = exclude
                        && ex_block == *block_id
                    {
                        continue; // 排除自身
                    }
                    return true;
                }
            }
        }

        false
    }

    /// 使引用指定值的所有作用域失效
    fn invalidate_user_scopes(&self, value: ValueSSA) -> RenameRes {
        use remusys_ir::ir::{ISubGlobalID, ISubInstID, ITraceableValue};

        let allocs = &self.module.allocs;
        let mut invalidated = Vec::new();

        // 获取值的可追踪表示
        let Some(traceable) = value.as_dyn_traceable(allocs) else {
            // 该值类型不支持追踪用户，直接返回
            return Ok(RenameDelta::default());
        };

        // 遍历所有使用该值的位置
        for (use_id, _) in traceable.user_iter(allocs) {
            if let Some(user_id) = use_id.get_user(allocs) {
                match user_id {
                    UserID::Inst(inst_id) => {
                        // 指令使用者：找到所在函数
                        if let Some(parent_func_id) = inst_id.get_parent_func(allocs) {
                            invalidated.push(parent_func_id.raw_into());
                        }
                    }
                    // UserID::Block variant does not exist
                    // 基本块不能直接作为UserID，通过其他方式处理
                    UserID::Expr(_expr_id) => {
                        // 表达式使用者：目前暂不处理，因为ExprID没有get_parent方法
                        // 表达式通常属于指令或常量，不影响函数作用域失效
                    }
                    UserID::Global(global_id) => {
                        // 全局对象使用者（如全局变量初始化）
                        invalidated.push(global_id);
                    } // _ 分支已被上述模式覆盖，不会到达
                }
            }
        }

        // 如果重命名的是函数本身，需要特殊处理调用者
        if let ValueSSA::Global(global_id) = value
            && let Some(func_id) = FuncID::try_from_global(allocs, global_id)
        {
            // 函数被重命名：所有调用该函数的函数都需要失效
            let func_obj = func_id.deref_ir(allocs);
            let users = func_obj.users();

            for (use_id, _) in users.iter(&allocs.uses) {
                if let Some(user_id) = use_id.get_user(allocs)
                    && let UserID::Inst(inst_id) = user_id
                    && let Some(parent_func_id) = inst_id.get_parent_func(allocs)
                {
                    invalidated.push(parent_func_id.raw_into());
                }
            }
        }

        // 去重
        invalidated.sort_unstable();
        invalidated.dedup();

        Ok(RenameDelta {
            invalidate_overview: false,
            invalidated,
        })
    }

    /// 获取给定 SourceTrackable 的当前名称（如果存在）
    pub fn get_current_name(&self, id: SourceTrackable) -> Option<SmolStr> {
        match id {
            SourceTrackable::Global(global_id) => {
                if global_id.is_alive(&self.module.allocs) {
                    Some(SmolStr::new(global_id.get_name(&self.module.allocs)))
                } else {
                    None
                }
            }
            SourceTrackable::Block(block_id) => self.names.blocks.get(&block_id).cloned(),
            SourceTrackable::Inst(inst_id) => self.names.insts.get(&inst_id).cloned(),
            SourceTrackable::FuncArg(func, index) => {
                if let Some(func_id) = FuncID::try_from_global(&self.module.allocs, func) {
                    self.names
                        .funcs
                        .get(&func_id)
                        .and_then(|args| args.get(index as usize))
                        .and_then(|opt| opt.clone())
                } else {
                    None
                }
            }
            SourceTrackable::Use(use_id) => {
                // Use 的名称取决于其操作数
                match use_id.get_operand(&self.module.allocs) {
                    ValueSSA::Inst(inst) => self.names.insts.get(&inst).cloned(),
                    ValueSSA::Global(glob) => {
                        if glob.is_alive(&self.module.allocs) {
                            Some(SmolStr::new(glob.get_name(&self.module.allocs)))
                        } else {
                            None
                        }
                    }
                    ValueSSA::Block(bb) => self.names.blocks.get(&bb).cloned(),
                    ValueSSA::FuncArg(func, idx) => self
                        .names
                        .funcs
                        .get(&func)
                        .and_then(|args| args.get(idx as usize))
                        .and_then(|opt| opt.clone()),
                    _ => None,
                }
            }
            SourceTrackable::JumpTarget(jt_id) => {
                // JumpTarget 的名称取决于其目标基本块
                if let Some(block_id) = jt_id.get_block(&self.module.allocs) {
                    self.names.blocks.get(&block_id).cloned()
                } else {
                    None
                }
            }
            SourceTrackable::Expr(_) => None, // 表达式没有持久化名称
        }
    }

    /// 检查名称在给定上下文中是否可用
    /// 如果 available 为 true 且没有错误，表示名称可用
    /// 如果 available 为 false，error 会说明原因
    pub fn check_name_availability(
        &self,
        id: SourceTrackable,
        new_name: &str,
    ) -> Result<bool, RenameErr> {
        // 注意：这里不调用 gc_names() 因为需要 &mut self
        // 调用者应确保在重命名操作前已经清理过无效名称映射

        if !id.is_alive(&self.module) {
            return Err(RenameErr::InvalidID(id));
        }
        if !utils::name_is_ir_ident(new_name) {
            return Err(RenameErr::InvalidName(SmolStr::new(new_name)));
        }

        match id {
            SourceTrackable::Global(global_id) => {
                // 检查全局名称是否被其他全局对象使用
                if let Some(existing_id) = self.module.get_global_by_name(new_name)
                    && existing_id != global_id
                {
                    return Err(RenameErr::NameRepeated {
                        name: SmolStr::new(new_name),
                        existed: SourceTrackable::Global(existing_id),
                    });
                }
                Ok(true)
            }
            SourceTrackable::Block(block_id) => {
                let name_str = SmolStr::new(new_name);
                // 检查名称在父函数内是否已被使用
                if let Some(parent_func) = block_id.get_parent_func(&self.module.allocs)
                    && self.is_name_used_in_function(
                        parent_func,
                        &name_str,
                        Some(SourceTrackable::Block(block_id)),
                    )
                {
                    return Err(RenameErr::NameRepeated {
                        name: name_str,
                        existed: SourceTrackable::Block(block_id),
                    });
                }
                Ok(true)
            }
            SourceTrackable::Inst(inst_id) => {
                let name_str = SmolStr::new(new_name);
                // 检查名称在父函数内是否已被使用
                if let Some(parent_func) = inst_id.get_parent_func(&self.module.allocs)
                    && self.is_name_used_in_function(
                        parent_func,
                        &name_str,
                        Some(SourceTrackable::Inst(inst_id)),
                    )
                {
                    return Err(RenameErr::NameRepeated {
                        name: name_str,
                        existed: SourceTrackable::Inst(inst_id),
                    });
                }
                Ok(true)
            }
            SourceTrackable::FuncArg(func, index) => {
                let name_str = SmolStr::new(new_name);
                // 检查名称在函数内是否已被使用
                if let Some(func_id) = FuncID::try_from_global(&self.module.allocs, func)
                    && self.is_name_used_in_function(
                        func_id,
                        &name_str,
                        Some(SourceTrackable::FuncArg(func, index)),
                    )
                {
                    return Err(RenameErr::NameRepeated {
                        name: name_str,
                        existed: SourceTrackable::FuncArg(func, index),
                    });
                }
                Ok(true)
            }
            SourceTrackable::Use(use_id) => {
                // Use 的名称检查取决于其操作数类型
                match use_id.get_operand(&self.module.allocs) {
                    ValueSSA::Inst(inst) => {
                        self.check_name_availability(SourceTrackable::Inst(inst), new_name)
                    }
                    ValueSSA::Global(glob) => {
                        self.check_name_availability(SourceTrackable::Global(glob), new_name)
                    }
                    ValueSSA::Block(bb) => {
                        self.check_name_availability(SourceTrackable::Block(bb), new_name)
                    }
                    ValueSSA::FuncArg(func, idx) => self.check_name_availability(
                        SourceTrackable::FuncArg(func.raw_into(), idx),
                        new_name,
                    ),
                    _ => Ok(true), // 其他类型不需要名称检查
                }
            }
            SourceTrackable::JumpTarget(jt_id) => {
                // JumpTarget 的名称检查取决于其目标基本块
                if let Some(block_id) = jt_id.get_block(&self.module.allocs) {
                    self.check_name_availability(SourceTrackable::Block(block_id), new_name)
                } else {
                    Ok(true)
                }
            }
            SourceTrackable::Expr(_) => Ok(true), // 表达式不需要重命名
        }
    }

    /// 获取函数作用域内的所有名称信息，供 JS 前端管理作用域和查重
    pub fn get_function_names(&self, func: FuncID) -> Result<FunctionNames, RenameErr> {
        let allocs = &self.module.allocs;

        // 检查函数是否存在
        if !func.is_alive(allocs) {
            return Err(RenameErr::InvalidID(SourceTrackable::Global(
                func.raw_into(),
            )));
        }

        let mut result = FunctionNames::default();

        // 收集参数名称
        if let Some(args) = self.names.funcs.get(&func) {
            for (i, arg_name) in args.iter().enumerate() {
                if let Some(name) = arg_name {
                    result.args.insert(i as u32, name.clone());
                }
            }
        }

        // 收集指令名称
        for (inst_id, inst_name) in &self.names.insts {
            if let Some(parent_func) = inst_id.get_parent_func(allocs)
                && parent_func == func
            {
                result.insts.insert(*inst_id, inst_name.clone());
            }
        }

        // 收集基本块名称
        for (block_id, block_name) in &self.names.blocks {
            if let Some(parent_func) = block_id.get_parent_func(allocs)
                && parent_func == func
            {
                result.blocks.insert(*block_id, block_name.clone());
            }
        }

        Ok(result)
    }

    // fn invalidate_overview 已经在 module.rs 中定义，这里不需要重复定义
}

mod utils {
    /// matches pattern: Identifier, sometimes with '.'
    ///
    /// Regex: `[0-9A-Za-z_]+(\.[0-9A-Za-z_]+)*`
    pub fn name_is_ir_ident(name: &str) -> bool {
        let name_bytes = name.as_bytes();
        if name_bytes.is_empty() {
            return false;
        }

        // Check first character must be alphanumeric or underscore
        let first = name_bytes[0];
        if !(first.is_ascii_alphanumeric() || first == b'_') {
            return false;
        }

        let mut last_was_dot = false;
        // Iterate through remaining bytes
        for &ch in &name_bytes[1..] {
            match ch {
                b'.' if last_was_dot => return false, // Consecutive dots
                b'.' => last_was_dot = true,
                b'_' => last_was_dot = false,
                ch if ch.is_ascii_alphanumeric() => last_was_dot = false,
                _ => return false, // invalid character or not ASCII
            }
        }

        // Must not end with a dot
        !last_was_dot
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_name_is_ir_ident() {
            // Valid identifiers
            assert!(name_is_ir_ident("foo"));
            assert!(name_is_ir_ident("foo123"));
            assert!(name_is_ir_ident("_foo"));
            assert!(name_is_ir_ident("foo_bar"));
            assert!(name_is_ir_ident("FooBar"));
            assert!(name_is_ir_ident("foo.bar"));
            assert!(name_is_ir_ident("foo.bar.baz"));
            assert!(name_is_ir_ident("foo123.bar_456"));
            assert!(name_is_ir_ident("_foo._bar"));
            assert!(name_is_ir_ident("123")); // digits allowed at start
            assert!(name_is_ir_ident("123.456"));
            assert!(name_is_ir_ident("abc.123"));
            assert!(name_is_ir_ident("_123"));
            assert!(name_is_ir_ident("foo._bar")); // underscore after dot allowed
            assert!(name_is_ir_ident("a.b.c.d.e.f"));

            // Invalid identifiers
            assert!(!name_is_ir_ident("")); // empty
            assert!(!name_is_ir_ident(".")); // single dot
            assert!(!name_is_ir_ident("foo.")); // ends with dot
            assert!(!name_is_ir_ident(".foo")); // starts with dot
            assert!(!name_is_ir_ident("foo..bar")); // consecutive dots
            assert!(!name_is_ir_ident("foo bar")); // space
            assert!(!name_is_ir_ident("foo-bar")); // hyphen
            assert!(!name_is_ir_ident("foo@bar")); // special char
            assert!(!name_is_ir_ident("foo.bar.")); // ends with dot after multiple parts
            assert!(!name_is_ir_ident("foo. bar")); // space after dot
            assert!(!name_is_ir_ident("foo.bar ")); // trailing space
            assert!(!name_is_ir_ident(" foo.bar")); // leading space
            assert!(!name_is_ir_ident("foo.bär")); // non-ASCII
            assert!(!name_is_ir_ident("foo.")); // single component ending with dot
            assert!(!name_is_ir_ident("...")); // only dots
        }
    }
}
