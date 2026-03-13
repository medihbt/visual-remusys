use remusys_ir::{
    ir::*,
    typing::{FixVecType, IValType, ScalarType, TypeFormatter, ValTypeID},
};
use serde::Serialize;
use smol_str::{SmolStr, format_smolstr};
use std::str::FromStr;
use wasm_bindgen::prelude::*;

pub use crate::{dto::*, module::*};

mod cfg;
mod dfg;
mod dto;
mod mapping;
mod module;

#[macro_export]
macro_rules! fmt_jserr {
    ($($args:tt)*) => { Err(JsError::new(&format!($($args)*))) };
}

#[macro_export]
macro_rules! console_log {
    ($($args:tt)*) => {
        web_sys::console::log_1(&format!($($args)*).into());
    };
}

#[wasm_bindgen]
pub struct Api;

#[wasm_bindgen]
impl Api {
    pub fn compile_module(source_ty: &str, source: &str) -> Result<JsValue, JsError> {
        let module_info = match source_ty {
            "sysy" => module::ModuleInfo::compile_from_sysy(source),
            "ir" => module::ModuleInfo::compile_from_ir(source),
            _ => Err(JsError::new(&format!(
                "Unsupported source type: {source_ty:?}. Expected 'sysy' or 'ir'."
            ))),
        }?;
        let id_smol = ModuleInfo::insert_module(module_info)?.id;
        serialize_to_js(&ModuleBrief { id: id_smol })
            .map_err(|e| JsError::new(&format!("Failed to serialize module brief: {e}")))
    }

    pub fn type_get_name(id: &str, tyid: &str) -> Result<String, JsError> {
        let tyid = ValTypeID::from_str(tyid).map_err(|s| JsError::new(&s))?;
        match tyid {
            ValTypeID::Void => return Ok("void".into()),
            ValTypeID::Ptr => return Ok("ptr".into()),
            ValTypeID::Int(bits) => return Ok(format!("i{bits}")),
            ValTypeID::Float(fpkind) => return Ok(fpkind.get_name().into()),
            ValTypeID::FixVec(FixVecType(inner, len_log2)) => {
                let len = 1 << len_log2;
                let s = match inner {
                    ScalarType::Ptr => SmolStr::new_inline("ptr"),
                    ScalarType::Int(bits) => format_smolstr!("i{bits}"),
                    ScalarType::Float(fpkind) => SmolStr::new_inline(fpkind.get_name()),
                };
                return Ok(format!("<{len} x {s}>"));
            }
            _ => {}
        }
        module::ModuleInfo::with_module(id, |m| {
            let tctx = &m.module.tctx;
            if !tyid.is_alive(tctx) {
                return fmt_jserr!("Type with id {tyid:?} does not exist or has been deleted");
            }
            let mut write = String::new();
            let tf = TypeFormatter::new(&mut write, tctx);
            tyid.format_ir(&tf)?;
            drop(tf);
            Ok(write)
        })
    }

    pub fn get_globals_brief(id: &str) -> Result<JsValue, JsError> {
        let brief = module::ModuleInfo::with_module_mut(id, |m| m.get_globals())?;
        serialize_to_js(&brief)
            .map_err(|e| JsError::new(&format!("Failed to serialize module globals: {e}")))
    }

    pub fn load_global_obj(id: &str, global_id: &str) -> Result<JsValue, JsError> {
        let global_id = GlobalID::from_str(global_id).map_err(|s| JsError::new(&s))?;
        let obj = module::ModuleInfo::with_module(id, |m| m.make_global_obj(global_id))?;
        serialize_to_js(&obj)
            .map_err(|e| JsError::new(&format!("Failed to serialize global object: {e}")))
    }
    pub fn load_func_of_scope(module_id: &str, value_id: JsValue) -> Result<JsValue, JsError> {
        let pool_id: SourceTrackable = if let Some(s) = value_id.as_string() {
            let pool_allocated = PoolAllocatedID::from_str(&s)
                .map_err(|s| JsError::new(&format!("Invalid pool allocated ID string: {s}")))?;
            SourceTrackable::from(pool_allocated)
        } else {
            serde_wasm_bindgen::from_value(value_id)?
        };
        let obj: Option<IRPoolObjDt> = module::ModuleInfo::with_module(module_id, |info| {
            let Some(func_id) = info.try_get_func_scope(pool_id)? else {
                return Ok(None);
            };
            info.make_global_obj(func_id).map(Some)
        })?;
        serialize_to_js(&obj).map_err(JsError::from)
    }
    pub fn func_scope_of_id(module_id: &str, value_id: JsValue) -> Result<JsValue, JsError> {
        let pool_id: SourceTrackable = serde_wasm_bindgen::from_value(value_id)?;
        let id =
            module::ModuleInfo::with_module(module_id, |info| info.try_get_func_scope(pool_id))?;
        serialize_to_js(&id).map_err(JsError::from)
    }

    pub fn rename(id: &str, poolid: JsValue, new_name: &str) -> Result<(), JsError> {
        let _pool_id: SourceTrackable = serde_wasm_bindgen::from_value(poolid.clone())?;
        module::ModuleInfo::with_module_mut(id, |info| {
            info.invalidate_overview();
            Ok(())
        })?;
        todo!("Renaming not implemented yet. poolid: {poolid:?}, new_name: {new_name}");
    }

    pub fn update_func_src(id: &str, func_id: &str) -> Result<JsValue, JsError> {
        let func_id = GlobalID::from_str(func_id).map_err(|s| JsError::new(&s))?;
        let func_src = module::ModuleInfo::with_module(id, |info| info.update_func_src(func_id))?;
        serialize_to_js(&func_src).map_err(JsError::from)
    }
    pub fn update_overview_src(id: &str) -> Result<JsValue, JsError> {
        let overview = module::ModuleInfo::with_module(id, |m| m.overview_or_make())?;
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
        serialize_to_js(&src_updates).map_err(JsError::from)
    }

    pub fn get_value_used_by(id: &str, val: JsValue) -> Result<JsValue, JsError> {
        let value_dt: ValueDt = serde_wasm_bindgen::from_value(val)?;
        module::ModuleInfo::with_module(id, |info| {
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
            serialize_to_js(&used_by).map_err(JsError::from)
        })
    }

    pub fn clone_function(id: &str, func_id: &str) -> Result<JsValue, JsError> {
        let func_id = GlobalID::from_str(func_id).map_err(|s| JsError::new(&s))?;
        module::ModuleInfo::with_module_mut(id, |info| {
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

            serialize_to_js(&res).map_err(JsError::from)
        })
    }

    pub fn make_dominator_tree(module_id: &str, func_id: &str) -> Result<JsValue, JsError> {
        let func_id = GlobalID::from_str(func_id).map_err(|s| JsError::new(&s))?;
        let dt = ModuleInfo::with_module(module_id, |m| m.make_dominator_tree(func_id))?;
        serialize_to_js(&dt).map_err(JsError::from)
    }
    pub fn make_block_dfg(module_id: &str, block_id: &str) -> Result<JsValue, JsError> {
        let block_id = BlockID::from_str(block_id).map_err(|s| JsError::new(&s))?;
        let dfg = ModuleInfo::with_module(module_id, |m| m.make_block_dfg(block_id))?;
        serialize_to_js(&dfg).map_err(JsError::from)
    }
}

fn serialize_to_js<T: ?Sized + Serialize>(v: &T) -> Result<JsValue, serde_wasm_bindgen::Error> {
    let mut ser = serde_wasm_bindgen::Serializer::new();
    // 将 map-like 类型序列化为 plain object（而不是 JS Map）
    ser = ser.serialize_maps_as_objects(true);
    v.serialize(&ser)
}
