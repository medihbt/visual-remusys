use std::cell::RefCell;

use remusys_ir::{
    base::APInt,
    ir::{
        indexed_ir::{IndexedValue, PoolAllocatedIndex},
        *,
    },
    opt::{CfgBlockStat, CfgDfsSeq, DominatorTree},
    typing::{AggrType, FPKind, IValType, ScalarType, ValTypeID},
};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use smol_str::SmolStr;

use codecs::*;
pub mod codecs;

/// DTO of ValueSSA.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ValueDt {
    None,
    PtrNull,
    Undef(ValTypeID),
    I1(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(I64Codec),
    AP(APIntCodec),
    F32(f32),
    F64(f64),
    AggrZero(AggrType),
    Global(GlobalIndex),
    Block(BlockIndex),
    Inst(InstIndex),
    Expr(ExprIndex),
    Arg { func: GlobalIndex, index: u32 },
}

impl From<IndexedValue> for ValueDt {
    fn from(value: IndexedValue) -> Self {
        match value {
            IndexedValue::None => Self::None,
            IndexedValue::ConstData(const_data) => Self::from(const_data),
            IndexedValue::ConstExpr(expr) => Self::Expr(expr),
            IndexedValue::FuncArg(func, index) => Self::Arg { func, index },
            IndexedValue::AggrZero(ty) => Self::AggrZero(ty),
            IndexedValue::Block(block) => Self::Block(block),
            IndexedValue::Inst(inst) => Self::Inst(inst),
            IndexedValue::Global(global) => Self::Global(global),
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
            ConstData::Zero(ScalarType::Int(64)) => Self::I64(I64Codec(0)),
            ConstData::Zero(ScalarType::Int(i)) => Self::AP(APIntCodec(APInt::new_full(0u128, i))),
            ConstData::PtrNull | ConstData::Zero(ScalarType::Ptr) => Self::PtrNull,
            ConstData::Int(apint) => Self::from(apint),
            ConstData::Float(FPKind::Ieee32, f) => Self::F32(f as f32),
            ConstData::Float(FPKind::Ieee64, f) => Self::F64(f),
        }
    }
}
impl From<APInt> for ValueDt {
    fn from(value: APInt) -> Self {
        let (val, bits) = (value.as_signed(), value.bits());
        match bits {
            1 => Self::I1(val != 0),
            8 => Self::I8(val as i8),
            16 => Self::I16(val as i16),
            32 => Self::I32(val as i32),
            64 => Self::I64(I64Codec(val as i64)),
            _ => Self::AP(APIntCodec(value)),
        }
    }
}
impl From<ValueDt> for IndexedValue {
    fn from(value: ValueDt) -> Self {
        match value {
            ValueDt::None => IndexedValue::None,
            ValueDt::PtrNull => IndexedValue::ConstData(ConstData::PtrNull),
            ValueDt::Undef(ty) => IndexedValue::ConstData(ConstData::Undef(ty)),
            ValueDt::I1(i) => IndexedValue::ConstData(ConstData::Int(APInt::from(i))),
            ValueDt::I8(i) => IndexedValue::ConstData(ConstData::Int(APInt::from(i))),
            ValueDt::I16(i) => IndexedValue::ConstData(ConstData::Int(APInt::from(i))),
            ValueDt::I32(i) => IndexedValue::ConstData(ConstData::Int(APInt::from(i))),
            ValueDt::I64(I64Codec(i)) => IndexedValue::ConstData(ConstData::Int(APInt::from(i))),
            ValueDt::AP(APIntCodec(apint)) => IndexedValue::ConstData(ConstData::Int(apint)),
            ValueDt::F32(f) => IndexedValue::ConstData(ConstData::Float(FPKind::Ieee32, f as f64)),
            ValueDt::F64(f) => IndexedValue::ConstData(ConstData::Float(FPKind::Ieee64, f)),
            ValueDt::AggrZero(ty) => IndexedValue::AggrZero(ty),
            ValueDt::Global(globl) => IndexedValue::Global(globl),
            ValueDt::Block(block) => IndexedValue::Block(block),
            ValueDt::Inst(instr) => IndexedValue::Inst(instr),
            ValueDt::Expr(expr) => IndexedValue::ConstExpr(expr),
            ValueDt::Arg { func, index } => IndexedValue::FuncArg(func, index),
        }
    }
}

impl ValueDt {
    pub fn from_ir(allocs: &IRAllocs, ir_value: ValueSSA) -> Self {
        let indexed_value = IndexedValue::from_value(ir_value, allocs);
        Self::from(indexed_value)
    }
    pub fn into_ir(self, allocs: &IRAllocs) -> ValueSSA {
        let indexed_value: IndexedValue = self.into();
        indexed_value.into_value(allocs)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ArrayTypeDt {
    pub id: ValTypeID,
    pub elem: ValTypeID,
    pub len: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructTypeDt {
    pub id: ValTypeID,
    pub fields: SmallVec<[ValTypeID; 4]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasTypeDt {
    pub id: ValTypeID,
    pub name: SmolStr,
    pub aliased: AggrType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuncTypeDt {
    pub id: ValTypeID,
    pub args: SmallVec<[ValTypeID; 4]>,
    pub ret: ValTypeID,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseDt {
    pub kind: UseKind,
    pub value: ValueDt,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JTDt {
    pub kind: JumpTargetKind,
    pub block: BlockIndex,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstDt {
    pub id: InstIndex,
    pub repr: SmolStr,
    pub opcode: Opcode,
    pub ty: ValTypeID,
    pub operands: SmallVec<[UseDt; 4]>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExprDt {
    pub id: ExprIndex,
    pub repr: SmolStr,
    pub ty: ValTypeID,
    pub operands: SmallVec<[UseDt; 4]>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDt {
    pub id: BlockIndex,
    pub instrs: SmallVec<[InstIndex; 4]>,
    pub targets: SmallVec<[JTDt; 2]>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuncDt {
    pub id: GlobalIndex,
    pub linkage: Linkage,
    pub name: SmolStr,
    pub ty: ValTypeID,
    pub ret: ValTypeID,
    pub args: Vec<ValTypeID>,
    pub blocks: Option<Vec<BlockIndex>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GVarDt {
    pub id: GlobalIndex,
    pub linkage: Linkage,
    pub name: SmolStr,
    pub ty: ValTypeID,
    pub init: Option<ValueDt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeCtxDelta {
    pub structs: Vec<StructTypeDt>,
    pub aliases: Vec<AliasTypeDt>,
    pub arrays: Vec<ArrayTypeDt>,
    pub funcs: Vec<FuncTypeDt>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDelta {
    pub tctx: Option<Box<TypeCtxDelta>>,
    pub dels: Option<Box<ModuleDel>>,
    pub adds: Option<Box<ModuleAdd>>,
}
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDel {
    pub inst: SmallVec<[InstIndex; 2]>,
    pub expr: SmallVec<[ExprIndex; 2]>,
    pub globl: SmallVec<[GlobalIndex; 2]>,
    pub block: SmallVec<[BlockIndex; 2]>,
}
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ModuleAdd {
    pub inst: Vec<InstDt>,
    pub expr: Vec<ExprDt>,
    pub block: Vec<BlockDt>,
    pub func: Vec<FuncDt>,
    pub gvar: Vec<GVarDt>,
}

impl ModuleDelta {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_inst(&mut self, dt: InstDt) {
        self.adds().inst.push(dt);
    }
    pub fn add_expr(&mut self, dt: ExprDt) {
        self.adds().expr.push(dt);
    }
    pub fn add_block(&mut self, dt: BlockDt) {
        self.adds().block.push(dt);
    }
    pub fn add_func(&mut self, dt: FuncDt) {
        self.adds().func.push(dt);
    }
    pub fn add_gvar(&mut self, dt: GVarDt) {
        self.adds().gvar.push(dt);
    }
    pub fn del_inst(&mut self, id: InstIndex) {
        self.dels().inst.push(id);
    }
    pub fn del_expr(&mut self, id: ExprIndex) {
        self.dels().expr.push(id);
    }
    pub fn del_globl(&mut self, id: GlobalIndex) {
        self.dels().globl.push(id);
    }
    pub fn del_block(&mut self, id: BlockIndex) {
        self.dels().block.push(id);
    }

    fn adds(&mut self) -> &mut ModuleAdd {
        if self.adds.is_none() {
            self.adds = Some(Box::new(ModuleAdd::default()));
        }
        self.adds.as_mut().unwrap()
    }
    fn dels(&mut self) -> &mut ModuleDel {
        if self.dels.is_none() {
            self.dels = Some(Box::new(ModuleDel::default()));
        }
        self.dels.as_mut().unwrap()
    }
}

pub struct ModuleDeltaBuilder<'ir> {
    module: &'ir Module,
    stat: IRWriteModuleStat,
    delta: ModuleDelta,
}

impl<'ir> ModuleDeltaBuilder<'ir> {
    pub fn new(module: &'ir Module) -> Self {
        Self {
            module,
            stat: IRWriteModuleStat::default(),
            delta: ModuleDelta::new(),
        }
    }

    pub fn build(self) -> ModuleDelta {
        self.delta
    }

    pub fn add_global(&mut self, id: GlobalIndex) -> Result<(), String> {
        let allocs = &self.module.allocs;
        let Some(gid) = id.as_primary(allocs) else {
            return Err(format!("UAF detected for global index {id:?}"));
        };
        match gid.deref_ir(allocs) {
            GlobalObj::Func(func) => {
                self.add_func(id, func);
            }
            GlobalObj::Var(gvar) => {
                self.delta.add_gvar(GVarDt {
                    id,
                    linkage: gvar.get_linkage(allocs),
                    name: gvar.clone_name(),
                    ty: gvar.get_ptr_pointee_type(),
                    init: if gvar.is_extern(allocs) {
                        None
                    } else {
                        Some(ValueDt::from_ir(allocs, gvar.get_init(allocs)))
                    },
                });
            }
        }
        Ok(())
    }
    fn add_func(&mut self, id: GlobalIndex, func: &FuncObj) {
        let allocs = &self.module.allocs;
        self.delta.add_func(FuncDt {
            id,
            linkage: func.get_linkage(allocs),
            name: func.clone_name(),
            ty: func.get_pointee_func_type().into_ir(),
            ret: func.ret_type,
            args: {
                let mut args = Vec::with_capacity(func.args.len());
                for arg in &func.args {
                    args.push(arg.ty);
                }
                args
            },
            blocks: func.body.as_ref().map(|body| {
                let mut blocks = Vec::with_capacity(body.blocks.len());
                for (bb_id, _) in body.blocks.iter(&allocs.blocks) {
                    blocks.push(bb_id.to_indexed(allocs));
                }
                blocks
            }),
        });
    }
    fn dump_ops(&self, user: &dyn IUser) -> SmallVec<[UseDt; 4]> {
        let mut res: SmallVec<[UseDt; 4]> = SmallVec::new();
        let allocs = &self.module.allocs;
        for op in user.get_operands().iter() {
            let uop: &Use = op.deref_ir(allocs);
            res.push(UseDt {
                kind: uop.get_kind(),
                value: ValueDt::from_ir(allocs, uop.operand.get()),
            });
        }
        res
    }

    pub fn add_expr(&mut self, id: ExprIndex) -> Result<(), String> {
        let allocs = &self.module.allocs;
        let Some(expr_id) = id.as_primary(allocs) else {
            return Err(format!("UAF detected: expr id {id:?}"));
        };
        let repr = {
            let mut str = Vec::with_capacity(32);
            let mut writer = IRWriter {
                writer: RefCell::new(&mut str),
                module: self.module,
                module_stat: std::mem::take(&mut self.stat),
            };
            writer
                .fmt_expr(expr_id)
                .map_err(|e| format!("WriteIR error: {e}"))?;
            self.stat = std::mem::take(&mut writer.module_stat);
            unsafe { SmolStr::from(str::from_utf8_unchecked(str.as_slice())) }
        };
        self.delta.add_expr(ExprDt {
            id,
            repr,
            ty: expr_id.get_valtype(allocs),
            operands: self.dump_ops(expr_id.deref_ir(allocs)),
        });
        Ok(())
    }

    pub fn add_block(&mut self, id: BlockIndex) -> Result<(), String> {
        let allocs = &self.module.allocs;
        let Some(block_id) = id.as_primary(allocs) else {
            return Err(format!("UAF detected: block{id:?}"));
        };

        let insts = {
            let inst_list = block_id.get_insts(allocs);
            let mut insts: SmallVec<[InstIndex; 4]> = SmallVec::with_capacity(inst_list.len());
            for (id, inst) in inst_list.iter(&allocs.insts) {
                if let InstObj::PhiInstEnd(_) = inst {
                    continue;
                }
                let Some(idx) = id.as_indexed(allocs) else {
                    return Err(format!("invalid inst address {id:p}"));
                };
                insts.push(idx);
            }
            insts
        };

        let jts = {
            let mut jts: SmallVec<[JTDt; 2]> = SmallVec::new();
            for jt_id in block_id.get_succs(allocs) {
                let kind = jt_id.get_kind(allocs);
                let Some(bb) = jt_id.get_block(allocs) else {
                    return Err(format!(
                        "incomplete IR: JT {kind:?} of block {id:?} has no target"
                    ));
                };
                jts.push(JTDt {
                    kind,
                    block: bb.to_indexed(allocs),
                });
            }
            jts
        };

        self.delta.add_block(BlockDt {
            id,
            instrs: insts,
            targets: jts,
        });
        Ok(())
    }

    pub fn add_inst(&mut self, id: InstIndex) -> Result<(), String> {
        let allocs = &self.module.allocs;

        let Some(inst) = id.try_deref_ir(allocs) else {
            return Err(format!("UAF detected: inst{id:?}"));
        };
        let ops = self.dump_ops(inst);
        self.delta.add_inst(InstDt {
            id,
            repr: SmolStr::new("unimplemented"),
            opcode: inst.get_opcode(),
            ty: inst.get_valtype(),
            operands: ops,
        });
        Ok(())
    }

    pub fn add_value(&mut self, val: IndexedValue) -> Result<(), String> {
        match val {
            IndexedValue::ConstExpr(expr) => self.add_expr(expr),
            IndexedValue::Block(block) => self.add_block(block),
            IndexedValue::Inst(inst) => self.add_inst(inst),
            IndexedValue::Global(global) => self.add_global(global),
            _ => Ok(()),
        }
    }
    pub fn add_value_dt(&mut self, val: ValueDt) -> Result<(), String> {
        match val {
            ValueDt::Expr(expr) => self.add_expr(expr),
            ValueDt::Block(bb) => self.add_block(bb),
            ValueDt::Inst(inst) => self.add_inst(inst),
            ValueDt::Global(g) => self.add_global(g),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DominatorTreeDt {
    pub func_id: GlobalIndex,
    /// Dominator tree nodes sorted by DFS order.
    pub nodes: Vec<DominatorNodeDt>,
    /// Whether this is a post-dominator tree.
    pub is_postdom: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DominatorNodeRepr {
    VExit,
    BB(BlockIndex),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DominatorNodeDt {
    pub repr: DominatorNodeRepr,
    pub idom: Option<DominatorNodeRepr>,
    pub semidom: Option<DominatorNodeRepr>,
}

impl DominatorTreeDt {
    pub fn from_primary(allocs: &IRAllocs, dt: &DominatorTree) -> Self {
        let mut nodes = Vec::with_capacity(dt.nodes.len());
        for pnode in &dt.nodes {
            let repr = match pnode.block {
                CfgBlockStat::Virtual => DominatorNodeRepr::VExit,
                CfgBlockStat::Block(block_id) => DominatorNodeRepr::BB(block_id.to_indexed(allocs)),
            };
            let idom = match pnode.idom {
                CfgBlockStat::Virtual if pnode.idom_dfn == CfgDfsSeq::NULL_PARENT => None,
                CfgBlockStat::Virtual => Some(DominatorNodeRepr::VExit),
                CfgBlockStat::Block(block_id) => {
                    Some(DominatorNodeRepr::BB(block_id.to_indexed(allocs)))
                }
            };
            let semidom = match pnode.semidom {
                CfgBlockStat::Virtual if pnode.idom_dfn == CfgDfsSeq::NULL_PARENT => None,
                CfgBlockStat::Virtual => Some(DominatorNodeRepr::VExit),
                CfgBlockStat::Block(block_id) => {
                    Some(DominatorNodeRepr::BB(block_id.to_indexed(allocs)))
                }
            };
            nodes.push(DominatorNodeDt {
                repr,
                idom,
                semidom,
            });
        }

        Self {
            func_id: dt.func_id.to_indexed(allocs),
            nodes,
            is_postdom: dt.is_postdom(),
        }
    }

    pub fn new(allocs: &IRAllocs, func_index: GlobalIndex) -> Result<Self, String> {
        let func_id = func_index
            .as_primary(allocs)
            .ok_or_else(|| format!("UAF detected for function index {func_index:?}"))?;
        let func_id = FuncID::try_from_global(allocs, func_id)
            .ok_or_else(|| format!("global {func_index:?} is not a function"))?;
        let dt = match DominatorTree::builder(allocs, func_id) {
            Ok(builder) => builder.build(),
            Err(e) => {
                return Err(format!(
                    "failed to build dominator tree for function {func_index:?}: {e}"
                ));
            }
        };
        Ok(Self::from_primary(allocs, &dt))
    }
}
