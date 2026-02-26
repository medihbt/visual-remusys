use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};
use wasm_bindgen::prelude::*;

use remusys_ir::ir::{
    FuncID, GlobalObj, IPtrValue, IRNameMap, IRSerializer, ISubGlobal, ISubGlobalID, Module,
    SerializeIR,
};
use smol_str::SmolStr;

use crate::dto::{GlobalObjBase, ModuleBrief, ModuleGlobalsBrief, SourceLoc, SourcePos};

mod dto;
mod mapping {
    use crate::dto::{SourceLoc, SourcePos};

    pub struct LineBuf<'s>(Vec<&'s str>);

    impl<'a> From<&'a str> for LineBuf<'a> {
        fn from(s: &'a str) -> Self {
            LineBuf(s.lines().collect())
        }
    }

    impl<'s> LineBuf<'s> {
        pub fn map_pos(&self, line_base1: usize, col_nchars: usize) -> usize {
            let line_idx = line_base1.saturating_sub(1);
            if let Some(line) = self.0.get(line_idx) {
                line.chars().take(col_nchars).map(|c| c.len_utf16()).sum()
            } else {
                col_nchars
            }
        }
        pub fn correct_pos(&self, pos: SourcePos) -> SourcePos {
            SourcePos {
                line: pos.line,
                column: self.map_pos(pos.line, pos.column),
            }
        }
        pub fn correct_loc(&self, loc: SourceLoc) -> SourceLoc {
            SourceLoc {
                begin: self.correct_pos(loc.begin),
                end: self.correct_pos(loc.end),
            }
        }
    }
}

struct ModuleInfo {
    module: Module,
    names: IRNameMap,
}

thread_local! {
    static MODULES: RefCell<HashMap<SmolStr, ModuleInfo>>
        = RefCell::new(HashMap::new());
    static MODULE_COUNTER: Cell<usize> = const { Cell::new(0) };
}

impl ModuleInfo {
    pub fn with_module<R>(name: &str, f: impl FnOnce(&ModuleInfo) -> R) -> Option<R> {
        MODULES.with_borrow(|modules| modules.get(name).map(f))
    }
    fn next_id() -> usize {
        MODULE_COUNTER.with(|counter| {
            let id = counter.get();
            counter.set(id + 1);
            id
        })
    }

    pub fn get_globals(&self) -> Result<ModuleGlobalsBrief, JsError> {
        let mut ser = IRSerializer::new_buffered(&self.module, &self.names);
        ser.enable_srcmap();

        let mut globals = self.build_globals_bases(&mut ser)?;

        let overview_src = ser.extract_string();
        self.normalize_overview_columns(&overview_src, &mut globals);

        Ok(ModuleGlobalsBrief {
            overview_src,
            globals: globals.into_boxed_slice(),
        })
    }

    fn build_globals_bases<W: std::io::Write>(
        &self,
        ser: &mut IRSerializer<'_, '_, W>,
    ) -> Result<Vec<GlobalObjBase>, JsError> {
        let symtab = self.module.symbols.borrow();
        let mut globals = Vec::with_capacity(symtab.exported().len());
        let allocs = &self.module.allocs;

        for (name, &id) in symtab.exported() {
            let obj = id.deref_ir(allocs);
            let ir_srcrange = match obj {
                GlobalObj::Func(_) => ser.fmt_func_header(FuncID::raw_from(id)),
                GlobalObj::Var(_) => ser.fmt_global(id).map(|()| {
                    let srcmap = ser
                        .source_map()
                        .expect("internal error: source map not available");
                    srcmap
                        .index_get_range(id)
                        .copied()
                        .expect("internal error: global without source location")
                }),
            };
            let (ir_begin, ir_end) = ir_srcrange
                .map_err(|e| JsError::new(&format!("Failed to serialize global '{name}': {e}")))?;
            let base = GlobalObjBase {
                id,
                name: name.clone(),
                linkage: obj.get_linkage(allocs),
                ty: obj.get_ptr_pointee_type(),
                overview_loc: SourceLoc {
                    begin: SourcePos {
                        line: ir_begin.line,
                        // store char-based columns first
                        column: ir_begin.column_nchars,
                    },
                    end: SourcePos {
                        line: ir_end.line,
                        column: ir_end.column_nchars,
                    },
                },
            };
            globals.push(base);
        }

        Ok(globals)
    }

    fn normalize_overview_columns(&self, overview_src: &str, globals: &mut [GlobalObjBase]) {
        use mapping::LineBuf;
        let lb = LineBuf::from(overview_src);

        for obj in globals.iter_mut() {
            obj.overview_loc = lb.correct_loc(obj.overview_loc);
        }
    }

    pub fn compile_from_sysy(source: &str) -> Result<Self, JsError> {
        let module = remusys_lang::translate_sysy_text_into_ir(source)
            .map_err(|e| JsError::new(&format!("Failed to compile SysY source: {e}")))?;
        let names = IRNameMap::new();
        Ok(Self { module, names })
    }
    pub fn compile_from_ir(source: &str) -> Result<Self, JsError> {
        let module = remusys_ir_parser::source_to_ir(source)
            .map_err(|e| JsError::new(&format!("Failed to compile IR source: {e}")))?;
        let names = IRNameMap::new();
        Ok(Self { module, names })
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

    pub fn get_module_globals(id: &str) -> Result<JsValue, JsError> {
        let brief = match ModuleInfo::with_module(id, |info| info.get_globals()) {
            Some(Ok(brief)) => brief,
            Some(Err(e)) => return Err(e),
            None => return Err(JsError::new(&format!("Module with id '{}' not found", id))),
        };
        serde_wasm_bindgen::to_value(&brief)
            .map_err(|e| JsError::new(&format!("Failed to serialize module globals: {e}")))
    }
}
