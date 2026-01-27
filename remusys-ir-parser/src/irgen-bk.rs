use std::{cmp::Ordering, collections::HashMap, str::FromStr};

use crate::{
    ast::{
        AstNode, BlockAst, BrAst, FuncAst, Ident, IdentKind, InstAst, LoadAst, ModuleAst, Operand,
        OperandKind, SwitchAst, SwitchCase, TypeAst, TypeAstKind, TypeValue,
    },
    sema::{FuncTypeBuilder, HashAggr, HashSparse, SemaErr, SymbolMap, TypeMap, ValuePool},
};
use remusys_ir::{
    base::APInt,
    ir::{
        BlockID, ConstData, FuncBuilder, FuncID, FuncTerminateMode, GlobalVarBuilder,
        IGlobalVarBuildable, IRBuildRes, IRBuilder, IRFocus, ISubGlobalID, ISubValueSSA, InstKind,
        Module, Opcode, UseID, ValueSSA,
        inst::{
            AllocaInstID, GEPInst, LoadInstID, PhiInst, RetInstID, StoreInstID, SwitchInstBuilder,
        },
    },
    typing::{AggrType, IValType, IntType, ValTypeID},
};
use smallvec::SmallVec;
use smol_str::SmolStr;

#[derive(Debug, thiserror::Error)]
pub enum IRGenErr {
    #[error("Semantic error: {0}")]
    SemaErr(#[from] SemaErr),

    #[error("Undefined symbol: {0:?}")]
    SymbolUndefined(Ident),

    #[error("Type mismatch: expected {expected}, found {found} at {span:?}")]
    TypeMismatch {
        expected: String,
        found: String,
        span: logos::Span,
    },
}
pub type IRGenRes<T = ()> = Result<T, IRGenErr>;

#[derive(Default)]
pub struct TranslateCtx {
    symbols: SymbolMap,
    types: TypeMap,
    values: ValuePool,
}

pub struct Translator<'a> {
    ctx: TranslateCtx,
    source: &'a str,
    ast: &'a ModuleAst,
    ir: &'a Module,
}

impl<'a> Translator<'a> {
    pub fn new(source: &'a str, ast: &'a ModuleAst, ir: &'a Module) -> Self {
        Self {
            ctx: TranslateCtx::default(),
            source,
            ast,
            ir,
        }
    }

    fn gen_type(&mut self, ty: &TypeAst) -> IRGenRes<ValTypeID> {
        let tctx = &self.ir.tctx;
        let res = self.ctx.types.map_type(tctx, ty)?;
        Ok(res)
    }
    fn gen_operand(&mut self, ty: ValTypeID, op: &Operand) -> IRGenRes<ValueSSA> {
        use crate::ast::OperandKind as Op;
        let tctx = &self.ir.tctx;
        let value = match &op.kind {
            Op::Undef => ConstData::Undef(ty).into_ir(),
            Op::Poison => ValueSSA::None,
            Op::Zeroinit => ValueSSA::AggrZero(AggrType::from_ir(ty)),
            Op::Null => ValueSSA::ConstData(ConstData::PtrNull),
            Op::Bool(b) => ValueSSA::from(APInt::from(*b)),
            Op::Int(i) => {
                let ValTypeID::Int(bits) = ty else {
                    panic!(
                        "Type mismatch: expected integer type, found {}",
                        ty.get_display_name(tctx)
                    );
                };
                ValueSSA::from(APInt::new(*i, bits))
            }
            Op::FP(fp) => {
                let ValTypeID::Float(kind) = ty else {
                    panic!(
                        "Type mismatch: expected float type, found {}",
                        ty.get_display_name(tctx)
                    );
                };
                ValueSSA::ConstData(ConstData::Float(kind, *fp))
            }
            Op::Global(gname) => {
                let ident = Ident {
                    kind: IdentKind::Global,
                    name: gname.clone(),
                    span: op.get_span(),
                };
                self.ctx
                    .symbols
                    .get(&ident)
                    .ok_or(IRGenErr::SymbolUndefined(ident))?
            }
            Op::Local(lname) => {
                let ident = Ident {
                    kind: IdentKind::Local,
                    name: lname.clone(),
                    span: op.get_span(),
                };
                self.ctx
                    .symbols
                    .get(&ident)
                    .ok_or(IRGenErr::SymbolUndefined(ident))?
            }
            Op::Bytes(bytes) => {
                let ValTypeID::Array(arrty) = ty else {
                    panic!(
                        "Type mismatch: expected array type, found {}",
                        ty.get_display_name(tctx)
                    );
                };
                self.ctx.values.map_bytes(bytes.clone(), arrty, self.ir)?
            }
            Op::Aggr(aggr) => {
                let hash_aggr = HashAggr {
                    kind: aggr.kind,
                    ty,
                    elems: {
                        let mut elems: SmallVec<[ValueSSA; 8]> = SmallVec::new();
                        for tyop in &aggr.elems {
                            let val = self.gen_type_value(tyop)?;
                            elems.push(val);
                        }
                        elems
                    },
                };
                self.ctx.values.map_aggr(hash_aggr, self.ir)?
            }
            Op::Sparse(sparse) => {
                let ValTypeID::Array(arrty) = ty else {
                    return Err(IRGenErr::TypeMismatch {
                        expected: "array type".to_string(),
                        found: ty.get_display_name(tctx),
                        span: op.get_span(),
                    });
                };
                let elemty = arrty.get_element_type(tctx);
                let hash_sparse = HashSparse {
                    ty: arrty,
                    default: self.gen_operand(elemty, &sparse.default.val)?,
                    indices: {
                        let mut indices: Vec<(usize, ValueSSA)> = Vec::new();
                        for (idx, tyop) in &sparse.elems {
                            let val = self.gen_type_value(tyop)?;
                            indices.push((*idx, val));
                        }
                        indices
                    },
                };
                self.ctx.values.map_kv_array(hash_sparse, self.ir)?
            }
        };
        Ok(value)
    }

    fn gen_type_value(&mut self, tyop: &TypeValue) -> IRGenRes<ValueSSA> {
        let ty = self.gen_type(&tyop.ty)?;
        let value = self.gen_operand(ty, &tyop.val)?;
        Ok(value)
    }
}

type FuncList<'a> = SmallVec<[(&'a FuncAst, FuncID); 16]>;

struct OperandInfo<'a> {
    op_use: UseID,
    op_type: ValTypeID,
    op: &'a Operand,
}

impl<'a> Translator<'a> {
    #[inline(never)]
    pub fn translate(&mut self) {
        // 先搭建全局框架
        let mut funcs: FuncList<'a> = FuncList::with_capacity(self.ast.funcs.len());
        self.setup_global_frame(&mut funcs);

        for (ast_func, ir_func) in funcs {
            self.ctx.symbols.reset_locals();
            self.setup_func_frame(ast_func, ir_func);
            todo!(
                "generate IR function {ir_func:?} for AST function @{}",
                &ast_func.header.name
            )
        }
    }
    /// 给 IR 搭建一个骨架, 把那些 Value 先 new 出来再考虑操作数填充的事儿.
    fn setup_global_frame(&mut self, funcs: &mut FuncList<'a>) {
        let module = self.ir;
        let (tctx, allocs) = (&module.tctx, &module.allocs);

        // 函数暂时没什么依赖, 先生成函数骨架
        for func in &self.ast.funcs {
            let func_type = {
                let mut func_type = FuncTypeBuilder::new(&mut self.ctx.types, tctx);
                func_type
                    .return_type(&func.header.ret_ty)
                    .expect("Failed to set return type");
                for arg in &func.header.args {
                    func_type
                        .add_argtype(&arg.ty)
                        .expect("Failed to add argument type");
                }
                func_type.finish()
            };
            let mut func_builder = FuncBuilder::new(tctx, func.header.name.to_string(), func_type);
            if func.header.is_declare {
                func_builder.make_extern();
            } else {
                func_builder
                    .linkage(func.header.linkage)
                    .terminate_mode(FuncTerminateMode::Unreachable);
            }
            let func_id = func_builder
                .build_id(module)
                .expect("Failed to build function ID");
            self.ctx.symbols.insert(func.header.name.clone(), func_id);
            if !func.header.is_declare {
                funcs.push((func, func_id));
            }
        }

        let mut to_fill = Vec::new();
        // 生成全局变量骨架, 有初始化的先弄一个 Zero 占位
        for glob in &self.ast.global_vars {
            let ty = self
                .gen_type(&glob.ty)
                .expect("Failed to generate global variable type");
            let mut gvar_builder = GlobalVarBuilder::new(glob.name.to_string(), ty);
            if glob.init.is_some() {
                gvar_builder
                    .initval(ValueSSA::new_zero(ty).expect("Failed to create zero value"))
                    .linkage(glob.linkage)
                    .tls_model(glob.tls_model);
            } else {
                gvar_builder.make_extern().tls_model(glob.tls_model);
            }
            let gvar_id = gvar_builder
                .build_id(module)
                .expect("Failed to build global variable ID");
            if glob.init.is_some() {
                to_fill.push((glob, ty, gvar_id));
            }
            self.ctx.symbols.insert(glob.name.clone(), gvar_id);
        }

        // 全局变量只能引用常量和全局变量(作为指针使用), 因此在这里立即填充初始化值
        for (gvar_ast, gvar_ty, gvar_ir) in to_fill {
            let initval = self
                .gen_operand(gvar_ty, gvar_ast.init.as_ref().unwrap())
                .expect("Failed to generate global variable initializer");
            gvar_ir.enable_init(allocs, initval);
        }
    }

    /// 为每个函数搭建框架——包括基本块和指令的，不预填充操作数.
    fn setup_func_frame(&mut self, ast_func: &'a FuncAst, ir_func: FuncID) {
        let module = self.ir;
        let (allocs, tctx) = (&module.allocs, &module.tctx);
        let func_obj = ir_func.deref_ir(allocs);

        // 搭建基本块框架
        let mut bb_maps: HashMap<SmolStr, BlockID> = HashMap::new();

        let entry_bb = func_obj.get_entry().unwrap();
        let Some(ast_body) = ast_func.body.as_ref() else {
            panic!(
                "Internal error: function decl {} should be filtered out",
                &ast_func.header.name
            );
        };

        let mut bb_list = Vec::with_capacity(ast_body.blocks.len());
        bb_maps.insert(ast_body.blocks[0].name_clone(), entry_bb);
        bb_list.push(entry_bb);

        let mut builder = IRBuilder::new(module);
        builder.set_focus(IRFocus::Block(entry_bb));
        for ast_bb in &ast_body.blocks[1..] {
            let block = builder
                .split_block()
                .expect("Internal error: failed to split current block");
            builder.set_focus(IRFocus::Block(block));
            bb_maps.insert(ast_bb.name_clone(), block);
            bb_list.push(block);
        }

        // 把 terminator 加上
        let ast_iter = ast_body.blocks.iter();
        let ir_iter = bb_list.iter().copied();
        let mut oplist: Vec<OperandInfo<'a>> = Vec::new();
        for (ir_bb, ast_bb) in ir_iter.clone().zip(ast_iter.clone()) {
            let Some(last_inst) = ast_bb.insts.last() else {
                let name = ast_bb.name_clone();
                panic!("AST basic block %{name} has no terminator instruction");
            };
            builder.set_focus(IRFocus::Block(ir_bb));
            self.make_terminator(&bb_maps, &mut oplist, &mut builder, last_inst)
                .unwrap();
        }

        for (ir_bb, ast_bb) in ir_iter.zip(ast_iter) {
            builder.set_focus(IRFocus::Block(ir_bb));
        }
    }

    fn make_terminator(
        &mut self,
        bb_maps: &HashMap<SmolStr, BlockID>,
        oplist: &mut Vec<OperandInfo<'a>>,
        builder: &mut IRBuilder<&Module>,
        ast_inst: &'a InstAst,
    ) -> IRBuildRes {
        use crate::ast::InstKind as I;
        match &ast_inst.kind {
            I::Unreachable => {
                builder.focus_set_unreachable()?;
            }
            I::RetVoid => {
                builder.build_inst(|allocs, _| RetInstID::new_uninit(allocs, ValTypeID::Void))?;
            }
            I::Ret(ret_ast) => {
                let ty = self.gen_type(&ret_ast.tyval.ty).expect("Type error");
                builder.build_inst(|allocs, _| {
                    let ret = RetInstID::new_uninit(allocs, ty);
                    oplist.push(OperandInfo {
                        op_use: ret.retval_use(allocs),
                        op_type: ty,
                        op: &ret_ast.tyval.val,
                    });
                    ret
                })?;
            }
            I::Jump(label) => {
                let Some(block) = bb_maps.get(&label.name) else {
                    Err(IRGenErr::SymbolUndefined(label.make_ident())).unwrap()
                };
                builder.focus_set_jump_to(*block)?;
            }
            I::Br(br_ast) => {
                let BrAst {
                    then_bb, else_bb, ..
                } = br_ast;

                let Some(then_bb) = bb_maps.get(&then_bb.name).copied() else {
                    panic!("Semantic error: cannot find symbol");
                };
                let Some(else_bb) = bb_maps.get(&else_bb.name).copied() else {
                    panic!("Semantic error: cannot find symbol");
                };
                let (_, br_inst) = builder.focus_set_branch_to(
                    ValueSSA::from(APInt::from(false)),
                    then_bb,
                    else_bb,
                )?;
                let allocs = &self.ir.allocs;
                oplist.push(OperandInfo {
                    op_use: br_inst.cond_use(allocs),
                    op_type: ValTypeID::Int(1),
                    op: &br_ast.cond.val,
                });
            }
            I::Switch(switch_ast) => {
                let SwitchAst {
                    cond,
                    default_bb,
                    cases,
                    ..
                } = switch_ast;
                let cond_ty = self
                    .gen_type(&cond.ty)
                    .expect("failed to generate cond type");
                let ValTypeID::Int(bits) = cond_ty else {
                    let tyname = cond_ty.get_display_name(builder.tctx());
                    panic!("type error for condition: requires int but got `{tyname}`");
                };
                let Some(default_bb) = bb_maps.get(&default_bb.name).copied() else {
                    let name = default_bb.name.as_str();
                    panic!("Semantic error: default basic block %{name} not found");
                };
                let mut switch_builder = SwitchInstBuilder::new(IntType(bits));
                switch_builder.default_bb(default_bb);

                let cond_range = if bits <= 64 {
                    let p2 = 1i128 << bits;
                    -p2..p2
                } else {
                    panic!("Semantic error: switch instruction do not support bits > 64")
                };
                for case in cases {
                    let SwitchCase { discrim, label } = case;
                    let TypeValue { ty, val } = discrim;
                    let case_ty = self.gen_type(ty).expect("failed to generate discrim type");
                    if case_ty != cond_ty {
                        let tctx = builder.tctx();
                        let ncase = case_ty.get_display_name(tctx);
                        let ncond = cond_ty.get_display_name(tctx);
                        panic!(
                            "Semantic error: case type != cond type. case({ncase}), cond({ncond})"
                        );
                    }
                    let OperandKind::Int(case_n) = &val.kind else {
                        let cond_src = &self.source[cond.get_span()];
                        panic!(
                            "Semantic error: condition should be integer literal but got source:\n{cond_src}"
                        )
                    };
                    if !cond_range.contains(case_n) {
                        panic!("Semantic error: case {case_n} overflow / underflow");
                    }
                    let Some(label) = bb_maps.get(&label.name).copied() else {
                        let name = label.name.as_str();
                        panic!("Semantic error: label %{name} not found");
                    };
                    switch_builder.case(*case_n as i64, label);
                }
                builder.build_inst(|allocs, _| {
                    let switch = switch_builder.build_id(allocs);
                    oplist.push(OperandInfo {
                        op_use: switch.discrim_use(allocs),
                        op_type: cond_ty,
                        op: &switch_ast.cond.val,
                    });
                    switch
                })?;
            }
            inst_ast => {
                panic!(
                    "Semantic error: basic block ending should be a terminator instruction but got {inst_ast:?}"
                )
            }
        }
        Ok(())
    }

    fn setup_block_frame(
        &mut self,
        ir_bb: BlockID,
        ast_bb: &'a BlockAst,
        oplist: &mut Vec<OperandInfo<'a>>,
        bb_maps: &HashMap<SmolStr, BlockID>,
        builder: &mut IRBuilder<&Module>,
    ) {
        use crate::ast::InstKind as I;
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        enum Mode {
            Phi,
            Inst,
            Terminator,
        }
        impl Mode {
            fn from_inst(inst: &InstAst) -> Self {
                use I::*;
                match &inst.kind {
                    Unreachable | RetVoid | Ret(_) | Jump(_) | Br(_) | Switch(_) => {
                        Self::Terminator
                    }
                    I::Phi(_) => Self::Phi,
                    _ => Self::Inst,
                }
            }
        }

        // 把 terminator 摘除掉 -- 它已经生成指令了
        let bbname = ast_bb.get_name();
        let insts: &'a [InstAst] = &ast_bb.insts[..ast_bb.insts.len() - 1];

        let mut curr_mode = Mode::Phi;
        for inst in insts {
            let inst_mode = Mode::from_inst(inst);
            match inst_mode.cmp(&curr_mode) {
                Ordering::Less => panic!(
                    "Semantic error: found {inst_mode:?} in {curr_mode:?} section of basic block %{bbname:?}"
                ),
                Ordering::Equal => {}
                Ordering::Greater => curr_mode = inst_mode,
            }
            let module = self.ir;
            let (allocs, tctx) = (&module.allocs, &module.tctx);

            use I::{Br, Jump, Ret, RetVoid, Switch, Unreachable};
            match &inst.kind {
                Unreachable | RetVoid | Ret(_) | Jump(_) | Br(_) | Switch(_) => {
                    panic!(
                        "Semantic error: terminator instruction should only appear at the end of insts"
                    )
                }
                I::Phi(phi_ast) => {
                    let ty = self
                        .gen_type(&phi_ast.ty)
                        .expect("Semantic error: failed to generate type");
                    let mut phi_builder = PhiInst::builder(allocs, ty);
                    let mut inop_map: HashMap<BlockID, &'a Operand> =
                        HashMap::with_capacity(phi_ast.incomes.len());
                    for (op, label) in &phi_ast.incomes {
                        let Some(label_bb) = bb_maps.get(&label.name).copied() else {
                            panic!("Semantic error: cannot find symbol %{}", &label.name);
                        };
                        if inop_map.insert(label_bb, op).is_some() {
                            panic!("Semantic error: PHI incoming repeated");
                        }
                        phi_builder.add_uninit_incoming(label_bb);
                    }
                    let phi_inst = phi_builder.build_id();
                    for [uval, ubb] in &*phi_inst.incoming_uses(allocs) {
                        let bb = BlockID::from_ir(ubb.get_operand(allocs));
                        let op = inop_map[&bb];
                        oplist.push(OperandInfo {
                            op_use: *uval,
                            op_type: ty,
                            op,
                        });
                    }
                    builder
                        .insert_inst(phi_inst)
                        .expect("internal error: cannot insert PHI nodes");
                }
                I::Alloca(alloca_ast) => {
                    let ty = self
                        .gen_type(&alloca_ast.ty)
                        .expect("Semantic error: failed to generate type");
                    let align_log2 = self.calc_align_log2(ty, alloca_ast.align);
                    builder
                        .build_inst(|allocs, _| AllocaInstID::new(allocs, ty, align_log2))
                        .expect("Semantic error: failed to build inst");
                }
                I::GEP(gepast) => {
                    let initial_ty = self
                        .gen_type(&gepast.init_ty)
                        .expect("Semantic error: failed to generate type");
                    let mut gep_builder = GEPInst::builder(tctx, allocs, initial_ty);
                    for tyval in &gepast.indices {
                        let TypeValue {
                            ty: index_ty,
                            val: index,
                        } = tyval;
                        let TypeAstKind::Int(bits) = &index_ty.kind else {
                            let src = &self.source[index_ty.get_span()];
                            panic!("Semantic error: type mismatch (requires int but got `{src}`)");
                        };
                        let index = if let OperandKind::Int(idx) = &index.kind {
                            ValueSSA::from(APInt::new(*idx, *bits))
                        } else {
                            ValueSSA::ConstData(ConstData::Undef(ValTypeID::Int(*bits)))
                        };
                        gep_builder.add_index(index);
                    }

                    let gep = builder
                        .build_inst(|_, _| gep_builder.build_id())
                        .expect("Semantic error: failed to build inst");

                    let ir_indices = gep.index_uses(allocs).iter().copied();
                    let ast_indices = gepast.indices.iter();
                    for (uidx, ast_idx) in ir_indices.zip(ast_indices) {
                        let ty = uidx.get_operand(allocs).get_valtype(allocs);
                        oplist.push(OperandInfo {
                            op_use: uidx,
                            op_type: ty,
                            op: &ast_idx.val,
                        });
                    }
                }
                I::Load(load_ast) => {
                    let LoadAst { ty, align, .. } = load_ast;
                    let pointee_ty = self
                        .gen_type(ty)
                        .expect("Semantic error: failed to build type");
                    let align_log2 = self.calc_align_log2(pointee_ty, *align);
                    let load_inst = builder
                        .build_inst(|allocs, tctx| {
                            LoadInstID::new_uninit(allocs, pointee_ty, align_log2)
                        })
                        .expect("Internal error: failed to build inst");
                    oplist.push(OperandInfo {
                        op_use: load_inst.source_use(allocs),
                        op_type: ValTypeID::Ptr,
                        op: &load_ast.src.val,
                    });
                }
                I::Store(store_ast) => {
                    let TypeAstKind::Ptr = &store_ast.dest.ty.kind else {
                        panic!("Semantic error: store dest type should be pointer");
                    };
                    let source_ty = self
                        .gen_type(&store_ast.val.ty)
                        .expect("Semantic error: failed to build type");
                    let align_log2 = self.calc_align_log2(source_ty, store_ast.align);
                    let store_inst = builder
                        .build_inst(|allocs, _| {
                            StoreInstID::new_uninit(allocs, source_ty, align_log2)
                        })
                        .expect("Internal error: failed to build inst");
                }
                I::Bin(bin_ast) => {
                    let opcode = Opcode::from_str(bin_ast.op.as_str())
                        .expect("Internal error: should filter the correct bin opcode");
                    assert_eq!(
                        opcode.get_kind(),
                        InstKind::BinOp,
                        "Internal error: should filter the correct bin opcode"
                    );
                }
                I::Cast(cast_ast) => {}
                I::Cmp(cmp_ast) => todo!(),
                I::Select(select_ast) => todo!(),
                I::Call(call_ast) => todo!(),
            }
        }
    }

    fn calc_align_log2(&self, ty: ValTypeID, ast_align: Option<usize>) -> u8 {
        let align = ast_align.unwrap_or(ty.get_align(&self.ir.tctx));
        if align.is_power_of_two() {
            align.ilog2() as u8
        } else {
            panic!("Semantic error: align {align} not power of two");
        }
    }
}
