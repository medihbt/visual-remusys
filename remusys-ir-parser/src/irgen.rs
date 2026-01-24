use std::collections::HashMap;

use crate::{
    ast::{
        AstNode, BrAst, FuncAst, Ident, IdentKind, InstAst, ModuleAst, Operand, RetAst, TypeAst,
        TypeValue,
    },
    sema::{FuncTypeBuilder, HashAggr, HashSparse, SemaErr, SymbolMap, TypeMap, ValuePool},
};
use remusys_ir::{
    base::APInt,
    ir::{
        BlockID, ConstData, FuncBuilder, FuncID, FuncTerminateMode, GlobalVarBuilder,
        IGlobalVarBuildable, IRBuildRes, IRBuilder, IRFocus, ISubGlobalID, ISubValueSSA,
        IValueConvert, Module, TermiBuildRes, UseID, ValueSSA, inst::RetInstID,
    },
    typing::{AggrType, IValType, ValTypeID},
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
    fn setup_func_frame(&mut self, ast_func: &FuncAst, ir_func: FuncID) {
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
        for (ir_bb, ast_bb) in bb_list.into_iter().zip(ast_iter) {
            let Some(last_inst) = ast_bb.insts.last() else {
                let name = ast_bb.name_clone();
                panic!("AST basic block %{name} has no terminator instruction");
            };
            builder.set_focus(IRFocus::Block(ir_bb));
            self.make_terminator(&bb_maps, &mut builder, last_inst)
                .unwrap();
        }
    }

    fn make_terminator(
        &mut self,
        bb_maps: &HashMap<SmolStr, BlockID>,
        builder: &mut IRBuilder<&Module>,
        ast_inst: &InstAst,
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
                builder.build_inst(|allocs, _| RetInstID::new_uninit(allocs, ty))?;
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
            }
            I::Switch(switch_ast) => todo!(),
            inst_ast => {
                panic!(
                    "Sema error: basic block ending should be a terminator instruction but got {inst_ast:?}"
                )
            }
        }
        Ok(())
    }
}
