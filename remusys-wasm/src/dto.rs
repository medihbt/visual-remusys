#![allow(dead_code)]

use remusys_ir::{
    base::APInt,
    ir::{inst::*, *},
    typing::{AggrType, FPKind, IValType, ScalarType, ValTypeID},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smol_str::{SmolStr, ToSmolStr};

#[derive(Debug, Clone, Copy)]
pub struct StrI64(pub i64);
impl Serialize for StrI64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_smolstr())
    }
}
impl<'de> Deserialize<'de> for StrI64 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = SmolStr::deserialize(deserializer)?;
        s.parse::<i64>()
            .map(StrI64)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct SourcePos {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct SourceLoc {
    pub begin: SourcePos,
    pub end: SourcePos,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum RefValueDt {
    Global(GlobalID),
    Block(BlockID),
    Inst(InstID),
    Expr(ExprID),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum UserIDDt {
    Inst(InstID),
    Global(GlobalID),
    Expr(ExprID),
}

impl From<UserID> for UserIDDt {
    fn from(u: UserID) -> Self {
        match u {
            UserID::Expr(id) => UserIDDt::Expr(id),
            UserID::Inst(id) => UserIDDt::Inst(id),
            UserID::Global(id) => UserIDDt::Global(id),
        }
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
impl From<RefValueDt> for ValueDt {
    fn from(value: RefValueDt) -> Self {
        match value {
            RefValueDt::Global(id) => ValueDt::Global(id),
            RefValueDt::Block(id) => ValueDt::Block(id),
            RefValueDt::Inst(id) => ValueDt::Inst(id),
            RefValueDt::Expr(id) => ValueDt::Expr(id),
        }
    }
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
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum SourceTrackable {
    Global(GlobalID),
    Block(BlockID),
    Inst(InstID),
    Expr(ExprID),
    Use(UseID),
    JumpTarget(JumpTargetID),
    FuncArg(GlobalID, u32),
}

impl From<PoolAllocatedID> for SourceTrackable {
    fn from(value: PoolAllocatedID) -> Self {
        match value {
            PoolAllocatedID::Global(id) => SourceTrackable::Global(id),
            PoolAllocatedID::Block(id) => SourceTrackable::Block(id),
            PoolAllocatedID::Inst(id) => SourceTrackable::Inst(id),
            PoolAllocatedID::Expr(id) => SourceTrackable::Expr(id),
            PoolAllocatedID::Use(id) => SourceTrackable::Use(id),
            PoolAllocatedID::JumpTarget(id) => SourceTrackable::JumpTarget(id),
        }
    }
}

impl SourceTrackable {
    pub fn is_alive(&self, module: &Module) -> bool {
        let allocs = &module.allocs;
        match self {
            SourceTrackable::Global(id) => id.is_alive(allocs),
            SourceTrackable::Block(id) => id.is_alive(allocs),
            SourceTrackable::Inst(id) => id.is_alive(allocs),
            SourceTrackable::Expr(id) => id.is_alive(allocs),
            SourceTrackable::Use(id) => id.is_alive(allocs),
            SourceTrackable::JumpTarget(id) => id.is_alive(allocs),
            SourceTrackable::FuncArg(func_id, idx) => {
                if let Some(func_id) = FuncID::try_from_global(allocs, *func_id) {
                    let args = func_id.args(allocs);
                    (*idx as usize) < args.len()
                } else {
                    false
                }
            }
        }
    }
}

// Section: serialize-only DTOs

#[derive(Debug, Clone, Serialize)]
pub struct UseDt {
    pub id: UseID,
    pub user: UserIDDt,
    pub kind: UseKind,
    pub value: ValueDt,
    pub source_loc: Option<SourceLoc>,
}
#[derive(Debug, Clone, Serialize)]
pub struct JumpTargetDt {
    pub id: JumpTargetID,
    pub terminator: InstID,
    pub kind: JumpTargetKind,
    pub target: BlockID,
    pub source_loc: SourceLoc,
}
#[derive(Debug, Clone, Serialize)]
pub struct FuncArgDt {
    pub name: SmolStr,
    pub ty: ValTypeID,
    pub source_loc: Option<SourceLoc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalObjBase {
    pub id: GlobalID,
    pub name: SmolStr,
    pub linkage: Linkage,
    pub ty: ValTypeID,
    pub overview_loc: SourceLoc,
}
#[derive(Debug, Clone, Serialize)]
pub struct FuncObjDt {
    #[serde(flatten)]
    pub base: GlobalObjBase,
    pub args: Box<[FuncArgDt]>,
    pub ret_ty: ValTypeID,
    pub source: SmolStr,
    pub blocks: Option<Box<[BlockDt]>>,
}
#[derive(Debug, Clone, Serialize)]
pub struct GlobalVarObjDt {
    #[serde(flatten)]
    pub base: GlobalObjBase,
    pub init: ValueDt,
}
#[derive(Debug, Clone, Serialize)]
pub struct BlockDt {
    pub id: BlockID,
    pub parent: GlobalID,
    pub name: Option<SmolStr>,
    pub source_loc: SourceLoc,
    pub insts: Box<[InstDt]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstBase {
    pub id: InstID,
    pub parent: BlockID,
    pub name: Option<SmolStr>,
    pub opcode: Opcode,
    pub operands: Box<[UseDt]>,
    pub source_loc: SourceLoc,
}
pub type NormalInstDt = InstBase;

#[derive(Debug, Clone, Serialize)]
pub struct TerminatorDt {
    #[serde(flatten)]
    pub base: InstBase,
    pub succs: Box<[JumpTargetDt]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PhiIncoming {
    pub value: ValueDt,
    pub from: BlockID,
}
#[derive(Debug, Clone, Serialize)]
pub struct PhiInstDt {
    #[serde(flatten)]
    pub base: InstBase,
    pub incomings: Box<[PhiIncoming]>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "typeid")]
pub enum InstDt {
    #[serde(rename = "Inst")]
    Normal(NormalInstDt),
    #[serde(rename = "Terminator")]
    Terminator(TerminatorDt),
    #[serde(rename = "Phi")]
    Phi(PhiInstDt),
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceLocUpdate {
    pub id: SourceTrackable,
    pub new_loc: SourceLoc,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SourceUpdateScope {
    Func,
    Module,
}
#[derive(Debug, Clone, Serialize)]
pub struct SourceUpdates {
    pub scope: SourceUpdateScope,
    pub source: SmolStr,
    pub ranges: Box<[SourceLocUpdate]>,
    pub elliminated: Box<[SourceTrackable]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleBrief {
    pub id: SmolStr,
}
#[derive(Debug, Clone, Serialize)]
pub struct ModuleGlobalsBrief {
    pub overview_src: SmolStr,
    pub globals: Box<[GlobalObjBase]>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "typeid")]
pub enum IRPoolObjDt {
    Func(FuncObjDt),
    GlobalVar(GlobalVarObjDt),
    Block(BlockDt),
    Terminator(TerminatorDt),
    Inst(NormalInstDt),
    Phi(PhiInstDt),
}
impl From<FuncObjDt> for IRPoolObjDt {
    fn from(value: FuncObjDt) -> Self {
        Self::Func(value)
    }
}
impl From<GlobalVarObjDt> for IRPoolObjDt {
    fn from(value: GlobalVarObjDt) -> Self {
        Self::GlobalVar(value)
    }
}
impl From<BlockDt> for IRPoolObjDt {
    fn from(value: BlockDt) -> Self {
        Self::Block(value)
    }
}
impl From<TerminatorDt> for IRPoolObjDt {
    fn from(value: TerminatorDt) -> Self {
        Self::Terminator(value)
    }
}
impl From<NormalInstDt> for IRPoolObjDt {
    fn from(value: NormalInstDt) -> Self {
        Self::Inst(value)
    }
}
impl From<PhiInstDt> for IRPoolObjDt {
    fn from(value: PhiInstDt) -> Self {
        Self::Phi(value)
    }
}
impl From<InstDt> for IRPoolObjDt {
    fn from(value: InstDt) -> Self {
        match value {
            InstDt::Normal(inst) => Self::Inst(inst),
            InstDt::Terminator(term) => Self::Terminator(term),
            InstDt::Phi(phi) => Self::Phi(phi),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FuncCloneInfo {
    pub new_id: GlobalID,
    pub bb_map: Box<[(BlockID, BlockID)]>,
    pub inst_map: Box<[(InstID, InstID)]>,
}
