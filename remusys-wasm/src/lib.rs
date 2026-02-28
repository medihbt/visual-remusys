use remusys_ir::ir::*;
use smol_str::SmolStr;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    str::FromStr,
};
use wasm_bindgen::prelude::*;

use crate::{dto::*, mapping::StrLines};
mod dto;

mod mapping {
    use super::*;

    pub struct StrLines<'a>(Vec<&'a str>);

    impl<'a> From<&'a str> for StrLines<'a> {
        fn from(s: &'a str) -> Self {
            Self(s.lines().collect())
        }
    }
    impl<'a> StrLines<'a> {
        pub fn map_pos(&self, pos: SourcePos) -> SourcePos {
            let line_idx = pos.line.saturating_sub(1);
            let line_src = self.0.get(line_idx).copied().unwrap_or("");
            let col = line_src
                .chars()
                .take(pos.column)
                .map(|c| c.len_utf16())
                .sum();
            SourcePos {
                line: pos.line,
                column: col,
            }
        }

        pub fn map_loc(&self, loc: SourceLoc) -> SourceLoc {
            SourceLoc {
                begin: self.map_pos(loc.begin),
                end: self.map_pos(loc.end),
            }
        }

        pub fn map_range(&self, range: IRSourceRange) -> SourceLoc {
            let (begin_pos, end_pos) = range;
            self.map_loc(SourceLoc {
                begin: SourcePos {
                    line: begin_pos.line,
                    column: begin_pos.column_nchars,
                },
                end: SourcePos {
                    line: end_pos.line,
                    column: end_pos.column_nchars,
                },
            })
        }
    }
}

struct ModuleInfo {
    module: Box<Module>,
    names: IRNameMap,
    overview: RefCell<Option<Rc<OverviewInfo>>>,
}

struct OverviewInfo {
    src: SmolStr,
    global_map: HashMap<GlobalID, IRSourceRange>,
    lines: Box<[usize]>,
}

impl OverviewInfo {
    pub fn correct_pos(&self, pos: SourcePos) -> SourcePos {
        let line_idx = pos.line.saturating_sub(1);
        let start_byte = self.lines.get(line_idx).copied().unwrap_or(self.src.len());
        let end_byte = self
            .lines
            .get(line_idx + 1)
            .copied()
            .unwrap_or(self.src.len());
        let line_src = &self.src[start_byte..end_byte];
        let col = line_src
            .chars()
            .take(pos.column)
            .map(|c| c.len_utf16())
            .sum();
        SourcePos {
            line: pos.line,
            column: col,
        }
    }
    pub fn correct_loc(&self, loc: SourceLoc) -> SourceLoc {
        SourceLoc {
            begin: self.correct_pos(loc.begin),
            end: self.correct_pos(loc.end),
        }
    }
    pub fn map_range_to_loc(&self, range: IRSourceRange) -> SourceLoc {
        let (begin_pos, end_pos) = range;
        self.correct_loc(SourceLoc {
            begin: SourcePos {
                line: begin_pos.line,
                column: begin_pos.column_nchars,
            },
            end: SourcePos {
                line: end_pos.line,
                column: end_pos.column_nchars,
            },
        })
    }
}

thread_local! {
    static MODULES: RefCell<HashMap<SmolStr, ModuleInfo>>
        = RefCell::new(HashMap::new());
    static MODULE_COUNTER: Cell<usize> = const { Cell::new(0) };
}

macro_rules! fmt_jserr {
    ($($args:tt)*) => { Err(JsError::new(&format!($($args)*))) };
}

impl ModuleInfo {
    fn new(module: Module) -> Self {
        Self {
            module: Box::new(module),
            names: IRNameMap::new(),
            overview: RefCell::new(None),
        }
    }

    pub fn compile_from_sysy(source: &str) -> Result<Self, JsError> {
        let module = remusys_lang::translate_sysy_text_into_ir(source)
            .map_err(|e| JsError::new(&format!("Failed to compile SysY source: {e}")))?;
        Ok(Self::new(module))
    }
    pub fn compile_from_ir(source: &str) -> Result<Self, JsError> {
        let module = remusys_ir_parser::source_to_ir(source)
            .map_err(|e| JsError::new(&format!("Failed to compile IR source: {e}")))?;
        Ok(Self::new(module))
    }

    pub fn with_module<R>(
        name: &str,
        f: impl FnOnce(&ModuleInfo) -> Result<R, JsError>,
    ) -> Result<R, JsError> {
        let res = MODULES.with_borrow(|modules| modules.get(name).map(f));
        match res {
            Some(Ok(r)) => Ok(r),
            Some(Err(e)) => Err(e),
            None => Err(JsError::new(&format!(
                "Module with id '{}' not found",
                name
            ))),
        }
    }
    pub fn with_module_mut<R>(
        id: &str,
        f: impl FnOnce(&mut ModuleInfo) -> Result<R, JsError>,
    ) -> Result<R, JsError> {
        let res = MODULES.with_borrow_mut(|modules| modules.get_mut(id).map(f));
        match res {
            Some(Ok(r)) => Ok(r),
            Some(Err(e)) => Err(e),
            None => Err(JsError::new(&format!("Module with id '{id}' not found"))),
        }
    }

    fn next_id() -> usize {
        MODULE_COUNTER.with(|counter| {
            let id = counter.get();
            counter.set(id + 1);
            id
        })
    }

    pub fn invalidate_overview(&self) {
        self.overview.take();
    }
    pub fn overview_or_make(&self) -> IRWriteRes<Rc<OverviewInfo>> {
        let mut overview = self.overview.borrow_mut();
        match &*overview {
            Some(ov) => Ok(ov.clone()),
            None => {
                let new_ov = self.make_overview()?;
                *overview = Some(new_ov.clone());
                Ok(new_ov)
            }
        }
    }
    pub fn make_overview(&self) -> IRWriteRes<Rc<OverviewInfo>> {
        let symtab = self.module.symbols.borrow();
        let mut func_pos = Vec::with_capacity(symtab.exported().len());
        let mut ser = IRSerializer::new_buffered(&self.module, &self.names);
        ser.enable_srcmap();

        for &id in symtab.exported().values() {
            let obj = id.deref_ir(&self.module.allocs);
            match obj {
                GlobalObj::Var(_) => {
                    ser.fmt_global(id)?;
                }
                GlobalObj::Func(_) => {
                    let range = ser.fmt_func_header(FuncID::raw_from(id))?;
                    func_pos.push((id, range));
                }
            }
        }
        let mut srcmap = ser
            .dump_srcmap()
            .expect("internal error: source map not available");
        let overview_src = ser.extract_string();
        for (id, range) in func_pos {
            srcmap.insert_range(id, range);
        }
        let mut lines = vec![0];
        let mut offset: usize = 0;
        for l in overview_src.split_inclusive('\n') {
            offset += l.len();
            lines.push(offset);
        }
        Ok(Rc::new(OverviewInfo {
            src: SmolStr::new(overview_src.as_str()),
            global_map: srcmap.globals,
            lines: lines.into_boxed_slice(),
        }))
    }

    pub fn get_globals(&mut self) -> Result<ModuleGlobalsBrief, JsError> {
        let symtab = self.module.symbols.borrow();
        let mut globals = Vec::with_capacity(symtab.exported().len());
        for (name, &id) in symtab.exported() {
            let base = self.make_global_base(id, name.clone())?;
            globals.push(base);
        }
        Ok(ModuleGlobalsBrief {
            overview_src: self.overview_or_make()?.src.clone(),
            globals: globals.into_boxed_slice(),
        })
    }
    fn make_global_base(&self, id: GlobalID, name: SmolStr) -> Result<GlobalObjBase, JsError> {
        let overview = self.overview_or_make()?;
        let Some(range) = overview.global_map.get(&id) else {
            return fmt_jserr!("Source location for global with id {id:?} not found in overview");
        };
        let overview_loc = overview.map_range_to_loc(*range);
        let obj = id.deref_ir(&self.module.allocs);
        Ok(GlobalObjBase {
            id,
            name,
            linkage: obj.get_linkage(&self.module.allocs),
            ty: obj.get_ptr_pointee_type(),
            overview_loc,
        })
    }

    pub fn make_global_obj(&self, id: GlobalID) -> Result<IRPoolObjDt, JsError> {
        let obj = id.deref_ir(&self.module.allocs);
        let base = self.make_global_base(id, obj.clone_name())?;
        match obj {
            GlobalObj::Func(func) => self.make_func_obj(func, base).map(IRPoolObjDt::Func),
            GlobalObj::Var(var) => self.make_var_obj(var, base).map(IRPoolObjDt::GlobalVar),
        }
    }

    pub fn update_func_src(&self, func_id: GlobalID) -> Result<SourceUpdates, JsError> {
        if !func_id.is_alive(&self.module.allocs) {
            return fmt_jserr!("Function with id {func_id:?} does not exist or has been deleted");
        }
        let Some(func_id) = FuncID::try_from_global(&self.module.allocs, func_id) else {
            return fmt_jserr!("global id {func_id:?} is not a function");
        };
        let mut func_ser = FuncSerializer::new_buffered(&self.module, func_id, &self.names);
        func_ser.enable_srcmap().fmt_func(func_id)?;
        let srcmap = func_ser
            .dump_srcmap()
            .expect("internal error: source map not available");
        let source = func_ser.extract_string();
        let strlines = StrLines::from(source.as_str());

        Ok(SourceUpdates {
            scope: SourceUpdateScope::Func,
            source: SmolStr::new(source.as_str()),
            ranges: {
                let nlocs = srcmap.funcargs[&func_id].len()
                    + srcmap.blocks.len()
                    + srcmap.insts.len()
                    + srcmap.uses.len()
                    + srcmap.jts.len();
                let mut loc_updates = Vec::with_capacity(nlocs);
                for (i, range) in srcmap.funcargs[&func_id].iter().enumerate() {
                    let Some(range) = *range else {
                        continue;
                    };
                    let loc = strlines.map_range(range);
                    loc_updates.push(SourceLocUpdate {
                        id: SourceTrackable::FuncArg(func_id.raw_into(), i as u32),
                        new_loc: loc,
                    });
                }

                for (&bb_id, &range) in &srcmap.blocks {
                    let loc = strlines.map_range(range);
                    loc_updates.push(SourceLocUpdate {
                        id: SourceTrackable::Block(bb_id),
                        new_loc: loc,
                    });
                }
                for (&inst_id, &range) in &srcmap.insts {
                    let loc = strlines.map_range(range);
                    loc_updates.push(SourceLocUpdate {
                        id: SourceTrackable::Inst(inst_id),
                        new_loc: loc,
                    });
                }
                for (&use_id, &range) in &srcmap.uses {
                    let loc = strlines.map_range(range);
                    loc_updates.push(SourceLocUpdate {
                        id: SourceTrackable::Use(use_id),
                        new_loc: loc,
                    });
                }
                for (&jt_id, &range) in &srcmap.jts {
                    let loc = strlines.map_range(range);
                    loc_updates.push(SourceLocUpdate {
                        id: SourceTrackable::JumpTarget(jt_id),
                        new_loc: loc,
                    });
                }

                loc_updates.into_boxed_slice()
            },
            elliminated: Box::new([]),
        })
    }

    fn make_var_obj(
        &self,
        var: &GlobalVar,
        base: GlobalObjBase,
    ) -> Result<GlobalVarObjDt, JsError> {
        Ok(GlobalVarObjDt {
            base,
            init: var.get_init(&self.module.allocs).into(),
        })
    }
    fn make_func_obj(&self, func: &FuncObj, base: GlobalObjBase) -> Result<FuncObjDt, JsError> {
        let func_id = FuncID::raw_from(base.id);
        let mut args = Vec::with_capacity(func.args.len());
        let mut func_ser = FuncSerializer::new_buffered(&self.module, func_id, &self.names);
        func_ser.enable_srcmap().fmt_func(func_id)?;

        let srcmap = func_ser
            .dump_srcmap()
            .expect("internal error: `enable_srcmap` was called but source map not available");
        let name_map = func_ser.get_numbers();
        let func_src = func_ser.extract_string();
        let src_lines = StrLines::from(func_src.as_str());
        let allocs = &self.module.allocs;

        for arg in &func.args {
            let arg_id = FuncArgID(func_id, arg.index);
            args.push(FuncArgDt {
                name: name_map.get_local_name(arg_id).unwrap(),
                ty: arg.ty,
                source_loc: srcmap
                    .funcarg_get_range(arg_id)
                    .map(|r| src_lines.map_range(r))
                    .unwrap(),
            });
        }
        let blocks = if let Some(blocks) = func.get_blocks() {
            let mut block_dts = Vec::with_capacity(blocks.len());
            for (bb_id, bb) in blocks.iter(&allocs.blocks) {
                block_dts.push(self.make_block_obj(bb_id, bb, &srcmap, &name_map, &src_lines)?);
            }
            Some(block_dts.into_boxed_slice())
        } else {
            None
        };

        Ok(FuncObjDt {
            base,
            args: args.into_boxed_slice(),
            ret_ty: func.ret_type,
            source: SmolStr::new(func_src.as_str()),
            blocks,
        })
    }

    fn make_block_obj(
        &self,
        bb_id: BlockID,
        bb: &BlockObj,
        srcmap: &SourceRangeMap,
        nummap: &FuncNumberMap,
        src_lines: &StrLines<'_>,
    ) -> Result<BlockDt, JsError> {
        let mut inst_dts = Vec::with_capacity(bb.get_insts().len());
        let allocs = &self.module.allocs;
        for (inst_id, inst) in bb.insts_iter(allocs) {
            if matches!(inst, InstObj::PhiInstEnd(_)) {
                continue;
            }
            inst_dts.push(self.make_inst_obj(inst_id, inst, srcmap, nummap, src_lines)?);
        }
        Ok(BlockDt {
            id: bb_id,
            name: nummap.get_local_name(bb_id),
            source_loc: srcmap
                .index_get_range(bb_id)
                .map(|r| src_lines.map_range(*r))
                .unwrap(),
            insts: inst_dts.into_boxed_slice(),
        })
    }

    fn make_inst_obj(
        &self,
        inst_id: InstID,
        inst: &InstObj,
        srcmap: &SourceRangeMap,
        nummap: &FuncNumberMap,
        src_lines: &StrLines<'_>,
    ) -> Result<InstDt, JsError> {
        let inst_base = InstBase {
            id: inst_id,
            name: nummap.get_local_name(inst_id),
            opcode: inst.get_opcode(),
            operands: {
                let mut ops = Vec::with_capacity(inst.get_operands().len());
                for uid in inst.operands_iter() {
                    let dt = self.make_use_dt(uid, srcmap, src_lines)?;
                    ops.push(dt);
                }
                ops.into_boxed_slice()
            },
            source_loc: srcmap
                .index_get_range(inst_id)
                .map(|r| src_lines.map_range(*r))
                .unwrap(),
        };

        let allocs = &self.module.allocs;
        match inst {
            InstObj::Phi(phi) => {
                let phi_dt = PhiInstDt {
                    base: inst_base,
                    incomings: {
                        let mut incomings = Vec::with_capacity(phi.incoming_uses().len());
                        for [uval, ubb] in phi.incoming_uses().iter() {
                            let from = BlockID::from_ir(ubb.get_operand(allocs));
                            let value = ValueDt::from(uval.get_operand(allocs));
                            incomings.push(PhiIncoming { value, from });
                        }
                        incomings.into_boxed_slice()
                    },
                };
                Ok(InstDt::Phi(phi_dt))
            }
            x if x.is_terminator() => {
                let termi = TerminatorDt {
                    base: inst_base,
                    succs: {
                        let jts = x.try_get_jts().unwrap_or(JumpTargets::Fix(&[]));
                        let mut succs = Vec::with_capacity(jts.len());
                        for jt in jts.iter() {
                            let jt_dt = self.make_jt_dt(*jt, srcmap, src_lines)?;
                            succs.push(jt_dt);
                        }
                        succs.into_boxed_slice()
                    },
                };
                Ok(InstDt::Terminator(termi))
            }
            _ => Ok(InstDt::Normal(inst_base)),
        }
    }

    fn make_use_dt(
        &self,
        use_id: UseID,
        srcmap: &SourceRangeMap,
        src_lines: &StrLines<'_>,
    ) -> Result<UseDt, JsError> {
        let use_obj = use_id.deref_ir(&self.module.allocs);
        Ok(UseDt {
            id: use_id,
            kind: use_obj.get_kind(),
            value: use_obj.operand.get().into(),
            source_loc: srcmap
                .index_get_range(use_id)
                .map(|r| src_lines.map_range(*r))
                .ok_or_else(|| {
                    JsError::new(&format!(
                        "Source location for use with id {use_id:?} not found in source map"
                    ))
                })?,
        })
    }
    fn make_jt_dt(
        &self,
        jt_id: JumpTargetID,
        srcmap: &SourceRangeMap,
        src_lines: &StrLines<'_>,
    ) -> Result<JumpTargetDt, JsError> {
        let jt_obj = jt_id.deref_ir(&self.module.allocs);
        let target = jt_obj.block.get().ok_or_else(|| {
            JsError::new(&format!(
                "Jump target with id {jt_id:?} has invalid block operand"
            ))
        })?;
        Ok(JumpTargetDt {
            id: jt_id,
            kind: jt_obj.get_kind(),
            target,
            source_loc: srcmap
                .index_get_range(jt_id)
                .map(|r| src_lines.map_range(*r))
                .ok_or_else(|| {
                    JsError::new(&format!(
                        "Source location for jump target with id {jt_id:?} not found in source map"
                    ))
                })?,
        })
    }
}

#[wasm_bindgen]
pub struct Api;

#[wasm_bindgen]
impl Api {
    pub fn compile_module(source_ty: &str, source: &str) -> Result<JsValue, JsError> {
        let mut module_info = match source_ty {
            "sysy" => ModuleInfo::compile_from_sysy(source),
            "ir" => ModuleInfo::compile_from_ir(source),
            _ => Err(JsError::new(&format!(
                "Unsupported source type: {source_ty:?}. Expected 'sysy' or 'ir'."
            ))),
        }?;
        let id = format!("module_{}", ModuleInfo::next_id());
        module_info.module.name = id.clone();
        let id_smol = SmolStr::from(id);
        MODULES.with_borrow_mut(|modules| {
            modules.insert(id_smol.clone(), module_info);
        });
        serde_wasm_bindgen::to_value(&ModuleBrief { id: id_smol })
            .map_err(|e| JsError::new(&format!("Failed to serialize module brief: {e}")))
    }

    pub fn get_globals_brief(id: &str) -> Result<JsValue, JsError> {
        let brief = ModuleInfo::with_module_mut(id, |m| m.get_globals())?;
        serde_wasm_bindgen::to_value(&brief)
            .map_err(|e| JsError::new(&format!("Failed to serialize module globals: {e}")))
    }

    pub fn load_global_obj(id: &str, global_id: &str) -> Result<JsValue, JsError> {
        let global_id = GlobalID::from_str(global_id).map_err(|s| JsError::new(&s))?;
        let obj = ModuleInfo::with_module(id, |m| m.make_global_obj(global_id))?;
        serde_wasm_bindgen::to_value(&obj)
            .map_err(|e| JsError::new(&format!("Failed to serialize global object: {e}")))
    }

    pub fn rename(id: &str, poolid: JsValue, new_name: &str) -> Result<(), JsError> {
        let _pool_id: SourceTrackable = serde_wasm_bindgen::from_value(poolid.clone())?;
        ModuleInfo::with_module_mut(id, |info| {
            info.invalidate_overview();
            Ok(())
        })?;
        todo!("Renaming not implemented yet. poolid: {poolid:?}, new_name: {new_name}");
    }

    pub fn update_func_src(id: &str, func_id: &str) -> Result<JsValue, JsError> {
        let func_id = GlobalID::from_str(func_id).map_err(|s| JsError::new(&s))?;
        let func_src = ModuleInfo::with_module(id, |info| info.update_func_src(func_id))?;
        serde_wasm_bindgen::to_value(&func_src).map_err(JsError::from)
    }
    pub fn update_overview_src(id: &str) -> Result<JsValue, JsError> {
        let overview =
            ModuleInfo::with_module(id, |m| m.overview_or_make().map_err(JsError::from))?;
        let src_updates = SourceUpdates {
            scope: SourceUpdateScope::Module,
            source: overview.src.clone(),
            ranges: {
                let mut ranges = Vec::with_capacity(overview.global_map.len());
                for (&id, &range) in &overview.global_map {
                    ranges.push(SourceLocUpdate {
                        id: SourceTrackable::Global(id),
                        new_loc: overview.map_range_to_loc(range),
                    });
                }
                ranges.into_boxed_slice()
            },
            elliminated: Box::new([]),
        };
        serde_wasm_bindgen::to_value(&src_updates).map_err(JsError::from)
    }

    pub fn get_value_used_by(id: &str, val: JsValue) -> Result<JsValue, JsError> {
        let value_dt: ValueDt = serde_wasm_bindgen::from_value(val)?;
        ModuleInfo::with_module(id, |info| {
            let allocs = &info.module.allocs;
            let Some(value) = value_dt.into_value(&info.module) else {
                return fmt_jserr!(
                    "Value operand {value_dt:?} does not correspond to a valid IR value"
                );
            };
            let mut used_by = Vec::new();
            if let Some(ival) = value.try_get_users(allocs) {
                for (uid, _) in ival.iter(&allocs.uses) {
                    used_by.push(uid);
                }
            }
            serde_wasm_bindgen::to_value(&used_by).map_err(JsError::from)
        })
    }

    pub fn clone_function(id: &str, func_id: &str) -> Result<JsValue, JsError> {
        let func_id = GlobalID::from_str(func_id).map_err(|s| JsError::new(&s))?;
        ModuleInfo::with_module_mut(id, |info| {
            let Some(func_id) = FuncID::try_from_global(&info.module.allocs, func_id) else {
                return fmt_jserr!("global id {id:?} is not a function");
            };
            let mut builder = FuncClone::new(&mut info.module, func_id)?;
            builder.keep_recurse(true);
            let cloned = builder.finish()?;
            let new_func = cloned.new_func;

            let res = FuncCloneInfo {
                new_id: new_func.raw_into(),
                bb_map: cloned
                    .blocks
                    .iter()
                    .map(|(old, new)| (*old, *new))
                    .collect(),
                inst_map: cloned.insts.iter().map(|(old, new)| (*old, *new)).collect(),
            };

            serde_wasm_bindgen::to_value(&res).map_err(JsError::from)
        })
    }
}
