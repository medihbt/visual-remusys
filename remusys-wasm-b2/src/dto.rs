use serde::{Deserialize, Serialize};
use smol_str::{SmolStr, ToSmolStr, format_smolstr};
use wasm_bindgen::JsError;

use crate::{IRTreeObjID, MonacoSrcRange, fmt_jserr};

pub mod call_graph;
pub mod cfg;
pub mod dfg;
pub mod dom;
pub mod testing;

use remusys_ir::{
    base::APInt,
    ir::*,
    typing::{AggrType, FPKind, IValType, ScalarType, ValTypeID},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StrI64(pub i64);

impl Serialize for StrI64 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.to_smolstr().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StrI64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = SmolStr::deserialize(deserializer)?;
        let value = s.parse::<i64>().map_err(serde::de::Error::custom)?;
        Ok(StrI64(value))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ValueDt {
    None,
    Undef(ValTypeID),
    PtrNull,
    I1(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(StrI64),
    APInt(APInt),
    F32(f32),
    F64(f64),
    ZeroInit(AggrType),
    FuncArg(GlobalID, u32),

    // Flatten: RefValueDt
    Global(GlobalID),
    Block(BlockID),
    Inst(InstID),
    Expr(ExprID),
}
impl From<ConstData> for ValueDt {
    fn from(value: ConstData) -> Self {
        match value {
            ConstData::Undef(ty) => Self::Undef(ty),
            ConstData::Zero(ScalarType::Float(FPKind::Ieee32)) => Self::F32(0.0f32),
            ConstData::Zero(ScalarType::Float(FPKind::Ieee64)) => Self::F64(0.0f64),
            ConstData::Zero(ScalarType::Int(1)) => Self::I1(false),
            ConstData::Zero(ScalarType::Int(8)) => Self::I8(0),
            ConstData::Zero(ScalarType::Int(16)) => Self::I16(0),
            ConstData::Zero(ScalarType::Int(32)) => Self::I32(0),
            ConstData::Zero(ScalarType::Int(64)) => Self::I64(StrI64(0)),
            ConstData::Zero(ScalarType::Int(x)) => Self::APInt(APInt::new(0, x)),
            ConstData::Zero(ScalarType::Ptr) => Self::PtrNull,
            ConstData::PtrNull => Self::PtrNull,
            ConstData::Int(apint) => match apint.bits() {
                1 => Self::I1(apint.is_nonzero()),
                8 => Self::I8(apint.as_signed() as i8),
                16 => Self::I16(apint.as_signed() as i16),
                32 => Self::I32(apint.as_signed() as i32),
                64 => Self::I64(StrI64(apint.as_signed() as i64)),
                _ => Self::APInt(apint),
            },
            ConstData::Float(FPKind::Ieee32, f) => Self::F32(f as f32),
            ConstData::Float(FPKind::Ieee64, f) => Self::F64(f),
        }
    }
}
impl From<ValueSSA> for ValueDt {
    fn from(value: ValueSSA) -> Self {
        match value {
            ValueSSA::None => Self::None,
            ValueSSA::ConstData(data) => Self::from(data),
            ValueSSA::ConstExpr(expr) => Self::Expr(expr),
            ValueSSA::AggrZero(aggrty) => Self::ZeroInit(aggrty),
            ValueSSA::FuncArg(func, idx) => Self::FuncArg(func.raw_into(), idx),
            ValueSSA::Block(block_id) => Self::Block(block_id),
            ValueSSA::Inst(inst_id) => Self::Inst(inst_id),
            ValueSSA::Global(global_id) => Self::Global(global_id),
        }
    }
}
impl ValueDt {
    pub fn into_value(self, module: &Module) -> Option<ValueSSA> {
        fn intval<T: Into<APInt>>(value: T) -> ValueSSA {
            ValueSSA::ConstData(ConstData::Int(value.into()))
        }
        let (tctx, allocs) = (&module.tctx, &module.allocs);
        let val = match self {
            ValueDt::None => ValueSSA::None,
            ValueDt::Undef(ty) if ty.is_alive(tctx) => ValueSSA::ConstData(ConstData::Undef(ty)),
            ValueDt::PtrNull => ValueSSA::ConstData(ConstData::PtrNull),
            ValueDt::I1(b) => intval(b),
            ValueDt::I8(i) => intval(i),
            ValueDt::I16(i) => intval(i),
            ValueDt::I32(i) => intval(i),
            ValueDt::I64(str_i64) => intval(str_i64.0),
            ValueDt::APInt(apint) => intval(apint),
            ValueDt::F32(f) => ValueSSA::ConstData(ConstData::Float(FPKind::Ieee32, f as f64)),
            ValueDt::F64(f) => ValueSSA::ConstData(ConstData::Float(FPKind::Ieee64, f)),
            ValueDt::ZeroInit(aggr_type) if aggr_type.into_ir().is_alive(tctx) => {
                ValueSSA::AggrZero(aggr_type)
            }
            ValueDt::Global(global_id) if global_id.is_alive(allocs) => ValueSSA::Global(global_id),
            ValueDt::Block(block_id) if block_id.is_alive(allocs) => ValueSSA::Block(block_id),
            ValueDt::Inst(inst_id) if inst_id.is_alive(allocs) => ValueSSA::Inst(inst_id),
            ValueDt::Expr(expr_id) if expr_id.is_alive(allocs) => ValueSSA::ConstExpr(expr_id),
            ValueDt::FuncArg(func_id, idx) if func_id.is_alive(allocs) => {
                let func_id = FuncID::try_from_global(allocs, func_id)?;
                if (idx as usize) >= func_id.args(allocs).len() {
                    return None;
                }
                ValueSSA::FuncArg(func_id, idx)
            }
            _ => return None,
        };
        Some(val)
    }

    pub fn get_name(&self, module: &Module, names: &IRNameMap) -> Result<SmolStr, JsError> {
        let name = match self {
            ValueDt::None => SmolStr::new("none"),
            ValueDt::Undef(ty) => {
                format_smolstr!("{} undef", ty.get_display_name(&module.tctx))
            }
            ValueDt::PtrNull => SmolStr::new("ptr null"),
            ValueDt::I1(true) => SmolStr::new("true"),
            ValueDt::I1(false) => SmolStr::new("false"),
            ValueDt::I8(v) => format_smolstr!("i8 {v}"),
            ValueDt::I16(v) => format_smolstr!("i16 {v}"),
            ValueDt::I32(v) => format_smolstr!("i32 {v}"),
            ValueDt::I64(StrI64(v)) => format_smolstr!("i64 {v}"),
            ValueDt::APInt(apint) => {
                format_smolstr!("i{} {}", apint.bits(), apint.as_signed())
            }
            ValueDt::F32(v) => format_smolstr!("f32 {v}"),
            ValueDt::F64(v) => format_smolstr!("f64 {v}"),
            ValueDt::ZeroInit(_) => SmolStr::new("zeroinitializer"),
            ValueDt::FuncArg(glob_id, idx) => {
                let Some(func_id) = FuncID::try_from_global(module, *glob_id) else {
                    return fmt_jserr!(Err "function {glob_id:?} does not exist");
                };
                match names.get_local_name(FuncArgID(func_id, *idx)) {
                    Some(name) => format_smolstr!("%{name}"),
                    None => format_smolstr!("@{}.arg{idx}", func_id.get_name(module)),
                }
            }
            ValueDt::Global(global_id) => {
                let Some(obj) = global_id.try_deref_ir(module) else {
                    return fmt_jserr!(Err "global {global_id:?} does not exist");
                };
                format_smolstr!("@{}", obj.get_name())
            }
            ValueDt::Block(block_id) => match names.get_local_name(*block_id) {
                Some(name) => format_smolstr!("%{name}"),
                None => block_id.to_strid(),
            },
            ValueDt::Inst(inst_id) => match names.get_local_name(*inst_id) {
                Some(name) => format_smolstr!("%{name}"),
                None => inst_id.to_strid(),
            },
            ValueDt::Expr(expr_id) => match names.get_local_name(*expr_id) {
                Some(name) => format_smolstr!("%{name}"),
                None => expr_id.to_strid(),
            },
        };
        Ok(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum IRTreeNodeClass {
    Module,
    GlobalVar,
    ExternFunc,
    Func,
    FuncArg,
    Block,
    PhiInst,
    NormalInst,
    TerminatorInst,
    Use,
    JumpTarget,
}

#[derive(Debug, Clone, Serialize)]
pub struct IRTreeNodeDt {
    pub obj: IRTreeObjID,
    pub kind: IRTreeNodeClass,
    pub label: SmolStr,
    pub src_range: MonacoSrcRange,
}
