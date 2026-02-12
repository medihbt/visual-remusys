use std::{cell::RefCell, collections::HashMap, str::FromStr};

use remusys_ir::{
    ir::{GlobalIndex, ISubGlobal, ISubGlobalID, Module},
    typing::{ArchInfo, IValType, ValTypeID},
};
use remusys_ir_parser::{
    CompileErr,
    ast::{AstNode, ModuleAst},
    irgen::IRGen,
    parser::IRParser,
};
use serde::Serialize;
use smallvec::SmallVec;
use smol_str::SmolStr;
use wasm_bindgen::prelude::*;

mod dto;
mod irparse;

pub use crate::{
    dto::{codecs::*, *},
    irparse::*,
};

#[derive(Default)]
struct ModuleMap {
    map: HashMap<String, Box<Module>>,
    id_gen: usize,
}
impl ModuleMap {
    pub fn insert(&mut self, mut module: Module) -> String {
        let id = format!("module.{}", self.id_gen);
        self.id_gen += 1;
        module.name = id.clone();
        self.map.insert(id.clone(), Box::new(module));
        id
    }
}

thread_local! {
    static MODULES: RefCell<ModuleMap> = RefCell::new(ModuleMap::default());
}

macro_rules! fmt_jserr {
    (res $($arg:tt)*) => {
        Err(JsError::new(&format!($($arg)*)))
    };
    ($($arg:tt)*) => {
        JsError::new(&format!($($arg)*))
    };
}

#[derive(Serialize)]
pub struct IRTextInfo {
    pub module_id: String,
    pub src_mapping: IRSourceMappingDt,
}

fn source_to_ir(source: &str) -> Result<(Module, IRSourceMappingDt), CompileErr> {
    let mut parser = IRParser::new(source);
    let ast = ModuleAst::parse(&mut parser)?;
    let mut module = Module::new(ArchInfo::new_host(), "");
    let mut irgen = IRGen::new(source, &ast, &module);
    irgen.generate()?;
    let mapping = irgen.mapping;
    module.begin_gc().finish();
    Ok((module, IRSourceMappingDt::from_mapping(source, mapping)))
}

#[wasm_bindgen]
pub fn parse_ir_text(source: &str) -> Result<JsValue, JsError> {
    let line_map = {
        let mut line_map: SmallVec<[usize; 32]> = SmallVec::new();
        let mut pos = 0;
        for line in source.lines() {
            line_map.push(pos);
            pos += line.len();
        }
        if pos != 0 {
            line_map.push(pos);
        }
        line_map
    };

    match source_to_ir(source) {
        Ok((module, src_mapping)) => {
            let id = MODULES.with(|modules| modules.borrow_mut().insert(module));
            let info = IRTextInfo {
                module_id: id,
                src_mapping,
            };
            serde_wasm_bindgen::to_value(&info).map_err(|e| fmt_jserr!("serialization error: {e}"))
        }
        Err(e) => {
            let text = e.dump_string(source, &line_map);
            Err(JsError::new(&text))
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct GlobalInfo {
    pub name: SmolStr,
    pub is_func: bool,
    pub id: GlobalIndex,
}
#[wasm_bindgen]
pub fn load_ir_globals(id: &str) -> Result<JsValue, JsError> {
    MODULES.with_borrow(|modules| {
        let module = modules
            .map
            .get(id)
            .ok_or_else(|| JsError::new("invalid module id"))?;
        let symbols = module.symbols.borrow();
        let mut globals = Vec::with_capacity(symbols.func_pool().len() + symbols.var_pool().len());
        let allocs = &module.allocs;
        for &func_id in symbols.func_pool() {
            let name = func_id.deref_ir(allocs).clone_name();
            globals.push(GlobalInfo {
                name,
                is_func: true,
                id: func_id.to_indexed(allocs),
            });
        }
        for &gvar_id in symbols.var_pool() {
            let name = gvar_id.deref_ir(allocs).clone_name();
            globals.push(GlobalInfo {
                name,
                is_func: false,
                id: gvar_id.to_indexed(allocs),
            });
        }
        serde_wasm_bindgen::to_value(&globals)
            .map_err(|e| JsError::new(&format!("serialization error: {}", e)))
    })
}

#[wasm_bindgen]
pub fn load_value(id: &str, value: JsValue) -> Result<JsValue, JsError> {
    MODULES.with_borrow(|modules| {
        let module = modules
            .map
            .get(id)
            .ok_or_else(|| JsError::new("invalid module id"))?;
        let value_idx: ValueDt =
            serde_wasm_bindgen::from_value(value).map_err(|e| JsError::new(&format!("{e}")))?;

        let mut builder = ModuleDeltaBuilder::new(module);
        builder
            .add_value_dt(value_idx)
            .map_err(|e| JsError::new(&e))?;

        serde_wasm_bindgen::to_value(&builder.build()).map_err(|e| JsError::new(&format!("{e}")))
    })
}

#[wasm_bindgen]
pub fn get_dominator_tree(id: &str, func: JsValue) -> Result<JsValue, JsError> {
    MODULES.with_borrow(|modules| {
        let module = modules
            .map
            .get(id)
            .ok_or_else(|| JsError::new("invalid module id"))?;
        let allocs = &module.allocs;
        let func_index: GlobalIndex = serde_wasm_bindgen::from_value(func)
            .map_err(|e| JsError::new(&format!("invalid function index: {e}")))?;
        DominatorTreeDt::new(allocs, func_index)
            .and_then(|dt| {
                serde_wasm_bindgen::to_value(&dt).map_err(|e| format!("serialization error: {e}"))
            })
            .map_err(|e| JsError::new(&e))
    })
}

#[wasm_bindgen]
pub fn ir_type_get_name(id: &str, ty: &str) -> Result<String, JsError> {
    let Ok(tyid) = ValTypeID::from_str(ty) else {
        return fmt_jserr!(res "invalid type id {ty:?}");
    };
    MODULES.with_borrow(|modules| {
        let module = modules
            .map
            .get(id)
            .ok_or_else(|| JsError::new("invalid module id"))?;
        let tctx = &module.tctx;
        if tyid.try_get_size(tctx).is_none() {
            return fmt_jserr!(res "invalid type id {ty:?}");
        }
        Ok(tyid.get_display_name(tctx))
    })
}
