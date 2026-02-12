use std::{collections::HashMap, str::FromStr};

use logos::Span;
use remusys_ir::{
    base::APInt,
    ir::{indexed_ir::PoolAllocatedIndex, inst::*, *},
    typing::*,
};
use smallvec::SmallVec;
use smol_str::{SmolStr, ToSmolStr, format_smolstr};

use crate::{
    ast::*,
    mapping::{IRFuncSrcMapping, IRSourceMapping},
    sema::*,
};

#[derive(Debug, thiserror::Error)]
pub enum IRGenErrKind {
    #[error("semantic error: {0}")]
    SemaErr(#[from] SemaErr),

    #[error("undefined symbol: {0}")]
    SymbolUndefined(SmolStr),

    #[error("redefined global symbol: {0}")]
    GlobalRedefined(SmolStr),

    #[error("redefined basic block: {0}")]
    BlockRedefined(SmolStr),

    #[error("redefined local symbol: {0}; NOTE: '%SYM = ' is always a definition, not assignment")]
    LocalRedefined(SmolStr),

    #[error("basic block not terminated: {0}")]
    BlockNotTerminated(SmolStr),

    #[error("type mismatch: expected {expect}, found {found}")]
    TypeMismatch { expect: SmolStr, found: SmolStr },

    #[error("value mismatch: expected {expect}, found '{found}'")]
    ValueMismatch { expect: SmolStr, found: SmolStr },

    #[error("instruction {inst_name} in wrong section: required {required:?}, found {found:?}")]
    InstInWrongSection {
        inst_name: SmolStr,
        required: InstSection,
        found: InstSection,
    },

    #[error("alignment {0} is not a power of 2")]
    AlignNotPwrOf2(usize),

    #[error("IR building error: {0}")]
    IRBuild(IRBuildError),

    #[error("GEP unpacking error: {0}")]
    GEPUnpack(GEPUnpackErr),
}
#[derive(Debug, thiserror::Error)]
#[error("{kind} at {span:?}")]
pub struct IRGenErr {
    pub kind: IRGenErrKind,
    pub span: Span,
}
impl From<IRGenErrKind> for IRGenErr {
    fn from(kind: IRGenErrKind) -> Self {
        Self { kind, span: 0..0 }
    }
}
impl From<SemaErr> for IRGenErr {
    fn from(kind: SemaErr) -> Self {
        IRGenErrKind::SemaErr(kind).into()
    }
}
impl IRGenErr {
    pub fn print(&self, source: &str) {
        println!("IR generation error at {:?}:", self.span);
        let snippet = &source[self.span.clone()];
        println!("  {}", snippet);
        println!("Error: {}", self.kind);
    }

    fn insert_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    fn sema(span: Span, err: SemaErr) -> Self {
        IRGenErr {
            kind: IRGenErrKind::SemaErr(err),
            span,
        }
    }

    fn type_mismatch_err<T>(
        span: Span,
        tctx: &TypeContext,
        expected: impl ToSmolStr,
        found: ValTypeID,
    ) -> IRGenRes<T> {
        Err(IRGenErr {
            kind: IRGenErrKind::TypeMismatch {
                expect: expected.to_smolstr(),
                found: found.get_display_name(tctx).to_smolstr(),
            },
            span,
        })
    }

    fn symbol_undef_err<T>(span: Span, ident: impl ToSmolStr) -> IRGenRes<T> {
        Err(IRGenErr {
            kind: IRGenErrKind::SymbolUndefined(ident.to_smolstr()),
            span,
        })
    }
    fn symbol_undef(span: Span, ident: impl ToSmolStr) -> Self {
        IRGenErr {
            kind: IRGenErrKind::SymbolUndefined(ident.to_smolstr()),
            span,
        }
    }
    fn ident_undef(ident: &Ident) -> Self {
        IRGenErr {
            kind: IRGenErrKind::SymbolUndefined(ident.name.clone()),
            span: ident.get_span(),
        }
    }

    fn global_redef(span: Span, name: SmolStr) -> Self {
        IRGenErr {
            kind: IRGenErrKind::GlobalRedefined(name),
            span,
        }
    }
    fn block_redef(span: Span, name: SmolStr) -> Self {
        IRGenErr {
            kind: IRGenErrKind::BlockRedefined(name),
            span,
        }
    }
    fn block_redef_err<T>(span: Span, name: SmolStr) -> IRGenRes<T> {
        Err(IRGenErr::block_redef(span, name))
    }
    fn block_not_terminated_err<T>(span: Span, name: SmolStr) -> IRGenRes<T> {
        Err(IRGenErr {
            kind: IRGenErrKind::BlockNotTerminated(name),
            span,
        })
    }
    fn local_redef(ident: &Ident) -> Self {
        IRGenErr {
            kind: IRGenErrKind::LocalRedefined(ident.name.clone()),
            span: ident.get_span(),
        }
    }

    fn ir_build(span: Span, err: IRBuildError) -> Self {
        IRGenErr {
            kind: IRGenErrKind::IRBuild(err),
            span,
        }
    }
    fn value_mismatch(span: Span, expected: impl ToSmolStr, source: &str) -> Self {
        IRGenErr {
            kind: IRGenErrKind::ValueMismatch {
                expect: expected.to_smolstr(),
                found: source[span.clone()].to_smolstr(),
            },
            span,
        }
    }
    fn value_mismatch_err<T>(
        op: &dyn AstNode,
        expected: impl ToSmolStr,
        source: &str,
    ) -> IRGenRes<T> {
        Err(IRGenErr::value_mismatch(op.get_span(), expected, source))
    }

    fn inst_in_wrong_section_err<T>(inst: &InstAst, required: InstSection) -> IRGenRes<T> {
        let inst_name = match inst.get_id() {
            Some(id) => id.name.clone(),
            None => SmolStr::new_static("<unnamed>"),
        };
        Err(IRGenErr {
            kind: IRGenErrKind::InstInWrongSection {
                inst_name,
                required,
                found: inst.get_section(),
            },
            span: inst.get_span(),
        })
    }

    fn align_not_pwr_of2(span: Span, align: usize) -> Self {
        IRGenErr {
            kind: IRGenErrKind::AlignNotPwrOf2(align),
            span,
        }
    }
    fn align_not_pwr_of2_err<T>(span: Span, align: usize) -> IRGenRes<T> {
        Err(IRGenErr::align_not_pwr_of2(span, align))
    }

    fn gep_unpack(span: Span, err: GEPUnpackErr) -> Self {
        IRGenErr {
            kind: IRGenErrKind::GEPUnpack(err),
            span,
        }
    }
}
pub type IRGenRes<T = ()> = Result<T, IRGenErr>;

pub struct IRGen<'a> {
    source: &'a str,
    ast: &'a ModuleAst,
    ir: &'a Module,
    symbols: SymbolMap,
    types: TypeMap,
    values: ValuePool,
    pub mapping: IRSourceMapping,
}

impl<'a> IRGen<'a> {
    pub fn new(src: &'a str, ast: &'a ModuleAst, ir: &'a Module) -> Self {
        Self {
            source: src,
            ast,
            ir,
            symbols: SymbolMap::default(),
            types: TypeMap::default(),
            values: ValuePool::default(),
            mapping: IRSourceMapping::default(),
        }
    }

    fn tear_module(&self) -> (&'a IRAllocs, &'a TypeContext) {
        (&self.ir.allocs, &self.ir.tctx)
    }
    fn gen_type(&mut self, ty: &TypeAst) -> IRGenRes<ValTypeID> {
        self.types
            .map_type(&self.ir.tctx, ty)
            .map_err(|e| IRGenErr::from(e).insert_span(ty.get_span()))
    }
    fn gen_value(&mut self, ty: ValTypeID, op: &Operand) -> IRGenRes<ValueSSA> {
        use crate::ast::OperandKind as OP;
        let tctx = &self.ir.tctx;
        let value = match &op.kind {
            OP::Undef => ValueSSA::new_undef(ty),
            OP::Poison => ValueSSA::None,
            OP::Zeroinit => ValueSSA::AggrZero(AggrType::from_ir(ty)),
            OP::Null => ValueSSA::ConstData(ConstData::PtrNull),
            OP::Bool(b) => ValueSSA::from(APInt::from(*b)),
            OP::Int(i) => {
                let ValTypeID::Int(bits) = ty else {
                    return IRGenErr::type_mismatch_err(op.get_span(), tctx, "integer type", ty);
                };
                ValueSSA::from(APInt::new_full(*i as u128, bits))
            }
            OP::FP(fp) => {
                let ValTypeID::Float(kind) = ty else {
                    return IRGenErr::type_mismatch_err(op.get_span(), tctx, "FP type", ty);
                };
                ValueSSA::ConstData(ConstData::Float(kind, *fp))
            }
            OP::Global(gname) => {
                let ident = Ident::global(op.get_span(), gname.clone());
                self.symbols
                    .get(&ident)
                    .ok_or(IRGenErr::ident_undef(&ident))?
            }
            OP::Local(lname) => {
                let ident = Ident::local(op.get_span(), lname.clone());
                self.symbols
                    .get(&ident)
                    .ok_or(IRGenErr::ident_undef(&ident))?
            }
            OP::Bytes(bytes) => {
                let ValTypeID::Array(byte_ty) = ty else {
                    return IRGenErr::type_mismatch_err(op.get_span(), tctx, "array type", ty);
                };
                if bytes.len() != byte_ty.get_num_elements(tctx) {
                    let len = format_smolstr!("[{} x i8]", byte_ty.get_num_elements(tctx));
                    return IRGenErr::type_mismatch_err(op.get_span(), tctx, len, ty);
                }
                self.values
                    .map_bytes(bytes.clone(), byte_ty, self.ir)
                    .map_err(|e| IRGenErr::sema(op.get_span(), e))?
            }
            OP::Aggr(aggr) => {
                let mut elems: SmallVec<[ValueSSA; 8]> = SmallVec::new();
                for tyop in &aggr.elems {
                    let val = self.gen_type_value(tyop)?;
                    elems.push(val);
                }
                let hash_aggr = HashAggr {
                    kind: aggr.kind,
                    ty,
                    elems,
                };
                self.values
                    .map_aggr(hash_aggr, self.ir)
                    .map_err(|e| IRGenErr::sema(op.get_span(), e))?
            }
            OP::Sparse(sparse) => {
                let ValTypeID::Array(arrty) = ty else {
                    return IRGenErr::type_mismatch_err(op.get_span(), tctx, "array type", ty);
                };
                let elemty = arrty.get_element_type(tctx);
                let mut indices: Vec<(usize, ValueSSA)> = Vec::new();
                for (idx, tyop) in &sparse.elems {
                    let val = self.gen_type_value(tyop)?;
                    indices.push((*idx, val));
                }
                let hash_sparse = HashSparse {
                    ty: arrty,
                    default: self.gen_value(elemty, &sparse.default.val)?,
                    indices,
                };
                self.values
                    .map_kv_array(hash_sparse, self.ir)
                    .map_err(|e| IRGenErr::sema(op.get_span(), e))?
            }
        };
        Ok(value)
    }
    fn gen_value_or_undef(&mut self, ty: ValTypeID, op: &Operand) -> IRGenRes<ValueSSA> {
        match self.gen_value(ty, op) {
            Ok(v) => Ok(v),
            Err(e) => match e.kind {
                IRGenErrKind::SymbolUndefined(..) => Ok(ValueSSA::new_undef(ty)),
                _ => Err(e),
            },
        }
    }
    fn gen_type_value(&mut self, tyop: &TypeValue) -> IRGenRes<ValueSSA> {
        let ty = self.gen_type(&tyop.ty)?;
        self.gen_value(ty, &tyop.val)
    }
}

type FuncList<'a> = SmallVec<[(&'a FuncAst, FuncID); 16]>;

struct OperandInfo<'a> {
    op_use: UseID,
    op_type: ValTypeID,
    op: &'a Operand,
}

struct GVarInfo<'a> {
    id: GlobalVarID,
    ty: ValTypeID,
    ast: &'a GlobalVarAst,
}

impl<'a> IRGen<'a> {
    pub fn generate(&mut self) -> IRGenRes {
        let mut funcs: FuncList<'a> = FuncList::with_capacity(self.ast.funcs.len());
        self.setup_global_frame(&mut funcs)?;

        for (func_ast, func_ir) in funcs {
            self.symbols.reset_locals();

            // 先把函数参数塞进符号表
            for (index, arg) in func_ast.header.args.iter().enumerate() {
                let arg_value = ValueSSA::FuncArg(func_ir, index as u32);
                let name = arg.name.name.clone();
                self.symbols
                    .insert(name.clone(), arg_value)
                    .map_err(|_| IRGenErr::symbol_undef(arg.get_span(), name))?;
            }
            self.generate_func(func_ast, func_ir)?;
        }
        Ok(())
    }

    fn setup_global_frame(&mut self, funcs: &mut FuncList<'a>) -> IRGenRes {
        let allocs = &self.ir.allocs;
        self.mapping.funcs.reserve(self.ast.funcs.len());
        for func in &self.ast.funcs {
            let func_id = self.build_func_metadata(funcs, func)?;
            self.mapping.funcs.push(IRFuncSrcMapping {
                head_span: func.header.get_span(),
                full_span: func.get_span(),
                id: GlobalIndex::from_primary(func_id.raw_into(), allocs),
                args: Vec::from_iter(func.header.args.iter().map(|a| a.get_span())),
            });
        }
        let mut global_defs = Vec::with_capacity(self.ast.global_vars.len());

        self.mapping.gvars.reserve(self.ast.global_vars.len());
        // 生成全局变量骨架, 有初始化的先弄一个 Zero 占位
        for glob in &self.ast.global_vars {
            let gid = self.build_gvar_metadata(&mut global_defs, glob)?;
            self.mapping.gvars.push((
                glob.get_span(),
                GlobalIndex::from_primary(gid.raw_into(), allocs),
            ));
        }

        for info in global_defs {
            let GVarInfo { id, ty, ast } = info;
            if let Some(init) = &ast.init {
                let initval = self.gen_value(ty, init)?;
                id.enable_init(allocs, initval);
                self.mapping.uses.push((
                    init.get_span(),
                    UseIndex::from_primary(id.init_use(allocs), allocs),
                ));
            }
        }
        Ok(())
    }

    fn build_func_metadata(
        &mut self,
        funcs: &mut FuncList<'a>,
        func: &'a FuncAst,
    ) -> IRGenRes<FuncID> {
        let tctx = &self.ir.tctx;
        let functype = {
            let mut builder = FuncTypeBuilder::new(&mut self.types, tctx);
            let ret_ty = &func.header.ret_ty;
            builder
                .return_type(ret_ty)
                .map_err(|e| IRGenErr::sema(ret_ty.get_span(), e))?;
            for arg in &func.header.args {
                builder
                    .add_argtype(&arg.ty)
                    .map_err(|e| IRGenErr::sema(arg.get_span(), e))?;
            }
            builder.finish()
        };
        let FuncAst { header, .. } = func;
        let mut builder = FuncBuilder::new(tctx, header.name.as_str(), functype);
        if header.is_declare {
            builder.make_extern();
        } else {
            builder
                .linkage(header.linkage)
                .terminate_mode(FuncTerminateMode::Unreachable);
        }
        let func_id = builder
            .build_id(self.ir)
            .map_err(|_| IRGenErr::global_redef(header.get_span(), header.name.clone()))?;

        self.symbols
            .insert(header.name.clone(), func_id)
            .map_err(|_| IRGenErr::global_redef(header.get_span(), header.name.clone()))?;
        if !header.is_declare {
            funcs.push((func, func_id));
        }
        Ok(func_id)
    }

    fn build_gvar_metadata(
        &mut self,
        global_defs: &mut Vec<GVarInfo<'a>>,
        glob: &'a GlobalVarAst,
    ) -> IRGenRes<GlobalID> {
        let ty = self.gen_type(&glob.ty)?;
        let mut gvar_builder = GlobalVarBuilder::new(glob.name.to_string(), ty);
        if glob.init.is_some() {
            let zvalue =
                ValueSSA::new_zero(ty).expect("internal error: failed to create zero value");
            gvar_builder
                .initval(zvalue)
                .linkage(glob.linkage)
                .tls_model(glob.tls_model);
        } else {
            gvar_builder.make_extern().tls_model(glob.tls_model);
        }
        let gvar_id = gvar_builder
            .build_id(self.ir)
            .map_err(|_| IRGenErr::global_redef(glob.get_span(), glob.name.to_smolstr()))?;
        if glob.init.is_some() {
            global_defs.push(GVarInfo {
                id: gvar_id,
                ty,
                ast: glob,
            });
        }
        self.symbols
            .insert(glob.name.clone(), gvar_id)
            .map_err(|_| IRGenErr::global_redef(glob.get_span(), glob.name.clone()))?;
        Ok(gvar_id.raw_into())
    }

    fn generate_func(&mut self, func_ast: &'a FuncAst, func_ir: FuncID) -> IRGenRes {
        let mut func_gen = FuncGen::new(self, func_ast, func_ir);
        func_gen.generate()
    }
}

struct FuncGen<'a: 't, 't> {
    irgen: &'t mut IRGen<'a>,
    func_ast: &'a FuncAst,
    func_ir: FuncID,
    bb_map: HashMap<SmolStr, BlockID>,
    use_queue: Vec<OperandInfo<'a>>,
}

impl<'a: 't, 't> FuncGen<'a, 't> {
    pub fn new(irgen: &'t mut IRGen<'a>, func_ast: &'a FuncAst, func_ir: FuncID) -> Self {
        let Some(body) = &func_ast.body else {
            panic!("internal error: cannot build FuncGen for declare-only function");
        };
        Self {
            irgen,
            func_ast,
            func_ir,
            bb_map: HashMap::with_capacity(body.blocks.len()),
            use_queue: Vec::new(),
        }
    }
    pub fn generate(&mut self) -> IRGenRes {
        self.setup_func_layout()?;
        while let Some(opinfo) = self.use_queue.pop() {
            let OperandInfo {
                op_use,
                op_type,
                op,
            } = opinfo;
            let val = self.irgen.gen_value(op_type, op)?;
            op_use.set_operand(&self.irgen.ir.allocs, val);
        }
        Ok(())
    }

    /// 搭建好函数的基本框架——基本块、指令布局. 此时不会填充操作数。
    fn setup_func_layout(&mut self) -> IRGenRes {
        let allocs = &self.irgen.ir.allocs;
        let func_obj = self.func_ir.deref_ir(allocs);

        // 这里直接 `unwrap()` 是安全的，因为 FuncGen 只能为有函数体的函数构造
        let entry_bb = func_obj.get_entry().unwrap();
        let ast_body = self.func_ast.body.as_ref().unwrap();
        if ast_body.blocks.is_empty() {
            panic!("internal error: function body has no basic blocks");
        }

        let mut ir_builder = IRBuilder::new(self.irgen.ir);
        let mut bb_list: SmallVec<[_; 11]> = SmallVec::with_capacity(ast_body.blocks.len());

        self.bb_map
            .insert(ast_body.blocks[0].name_clone(), entry_bb);
        bb_list.push(entry_bb);
        ir_builder.set_focus(IRFocus::Block(entry_bb));

        for ast_bb in &ast_body.blocks[1..] {
            let block = ir_builder
                .split_block()
                .expect("Internal error: failed to split current block");
            ir_builder.set_focus(IRFocus::Block(block));
            if self.bb_map.insert(ast_bb.name_clone(), block).is_some() {
                return IRGenErr::block_redef_err(ast_bb.get_span(), ast_bb.name_clone());
            }
            bb_list.push(block);
        }

        self.irgen.mapping.blocks.reserve(ast_body.blocks.len());

        // 加入基本块终止指令. 要求终止指令在基本块末尾.
        let ast_iter = ast_body.blocks.iter();
        let ir_iter = bb_list.iter().copied();
        for (ir_bb, ast_bb) in ir_iter.zip(ast_iter) {
            // Push basic block source position into mapping
            self.irgen
                .mapping
                .blocks
                .push((ast_bb.get_span(), BlockIndex::from_primary(ir_bb, allocs)));

            ir_builder.set_focus(IRFocus::Block(ir_bb));
            let Some(last_inst) = ast_bb.insts.last() else {
                let name = ast_bb.name_clone();
                return IRGenErr::block_not_terminated_err(ast_bb.get_span(), name);
            };
            self.make_terminator(&mut ir_builder, ast_bb, last_inst)?;

            let insts = &ast_bb.insts[..ast_bb.insts.len() - 1];
            let mut section = InstSection::Phi;
            for ast_inst in insts {
                let inst_section = ast_inst.get_section();
                if inst_section < section {
                    return IRGenErr::inst_in_wrong_section_err(ast_inst, section);
                } else if inst_section > section {
                    section = inst_section;
                }
                if inst_section == InstSection::Terminator {
                    /* 指令被不恰当地提前结束了: 一个基本块里只允许一个终止指令 */
                    return IRGenErr::inst_in_wrong_section_err(ast_inst, section);
                }
                let inst = self.make_and_insert_inst(&mut ir_builder, ast_inst)?;
                if let Some(id) = ast_inst.get_id() {
                    self.irgen
                        .symbols
                        .insert(id.name.clone(), inst)
                        .map_err(|_| IRGenErr::local_redef(id))?;
                }
            }
        }

        Ok(())
    }

    fn get_label(&self, label: &Label) -> IRGenRes<BlockID> {
        let name = &label.name;
        match self.bb_map.get(name) {
            Some(bb) => Ok(*bb),
            None => IRGenErr::symbol_undef_err(label.get_span(), name),
        }
    }

    fn make_align(&self, ty: ValTypeID, align: Option<usize>) -> IRGenRes<u8> {
        let tctx = &self.irgen.ir.tctx;
        let align_log2 = {
            let align = align.unwrap_or(ty.get_align(tctx));
            if align.is_power_of_two() {
                align.trailing_zeros() as u8
            } else {
                return IRGenErr::align_not_pwr_of2_err(0..0, align);
            }
        };
        Ok(align_log2)
    }

    fn push_use(&mut self, u: UseID, op: &'a Operand) {
        let val = u.get_operand(&self.irgen.ir.allocs);
        self.push_use_by_value(u, val, op);
    }
    fn push_use_by_value(&mut self, u: UseID, val: ValueSSA, op: &'a Operand) {
        self.irgen.mapping.uses.push((
            op.get_span(),
            UseIndex::from_primary(u, &self.irgen.ir.allocs),
        ));
        let ValueSSA::ConstData(ConstData::Undef(ty)) = val else {
            return;
        };
        self.use_queue.push(OperandInfo {
            op_use: u,
            op_type: ty,
            op,
        });
    }

    fn make_terminator(
        &mut self,
        ir_builder: &mut IRBuilder<&'a Module>,
        ast_bb: &'a BlockAst,
        ast_inst: &'a InstAst,
    ) -> IRGenRes<InstID> {
        use crate::ast::InstKind as I;
        let map_build_err = {
            let span = ast_inst.get_span();
            move |e: IRBuildError| IRGenErr::ir_build(span, e)
        };
        let allocs = &self.irgen.ir.allocs;
        let termi: InstID = match &ast_inst.kind {
            I::Unreachable => ir_builder
                .focus_set_unreachable()
                .map_err(map_build_err)?
                .1
                .raw_into(),
            I::RetVoid => ir_builder
                .build_inst(|allocs, _| RetInstID::new_uninit(allocs, ValTypeID::Void))
                .map_err(map_build_err)?
                .raw_into(),
            I::Ret(ret_ast) => {
                let ty = self.irgen.gen_type(&ret_ast.tyval.ty)?;
                let ret_inst = ir_builder
                    .build_inst(|allocs, _| RetInstID::new_uninit(allocs, ty))
                    .map_err(map_build_err)?;
                self.use_queue.push(OperandInfo {
                    op_use: ret_inst.retval_use(allocs),
                    op_type: ty,
                    op: &ret_ast.tyval.val,
                });
                ret_inst.raw_into()
            }
            I::Jump(label) => ir_builder
                .focus_set_jump_to(self.get_label(label)?)
                .map_err(map_build_err)?
                .1
                .raw_into(),
            I::Br(br) => {
                let then_bb = self.get_label(&br.then_bb)?;
                let else_bb = self.get_label(&br.else_bb)?;
                let cond = self
                    .irgen
                    .gen_value_or_undef(ValTypeID::Int(1), &br.cond.val)?;
                // old terminator placeholder disposed
                let (_, brinst) = ir_builder
                    .focus_set_branch_to(cond, then_bb, else_bb)
                    .map_err(map_build_err)?;
                self.push_use_by_value(brinst.cond_use(allocs), cond, &br.cond.val);
                brinst.raw_into()
            }
            I::Switch(switch) => self.make_switch(ir_builder, switch)?.raw_into(),
            _ => {
                let name = ast_bb.name_clone();
                return IRGenErr::block_not_terminated_err(ast_inst.get_span(), name);
            }
        };

        Ok(termi)
    }

    fn make_switch(
        &mut self,
        ir_builder: &mut IRBuilder<&'a Module>,
        switch: &'a SwitchAst,
    ) -> IRGenRes<SwitchInstID> {
        let map_build_err = {
            let span = switch.get_span();
            move |e: IRBuildError| IRGenErr::ir_build(span, e)
        };
        let allocs = &self.irgen.ir.allocs;
        let cond_ty = self.irgen.gen_type(&switch.cond.ty)?;
        let ValTypeID::Int(bits) = cond_ty else {
            return IRGenErr::type_mismatch_err(
                switch.cond.get_span(),
                &self.irgen.ir.tctx,
                "integer type",
                cond_ty,
            );
        };
        let cond_val = self.irgen.gen_value_or_undef(cond_ty, &switch.cond.val)?;

        let mut switch_builder = SwitchInstBuilder::new(IntType(bits));
        switch_builder.default_bb(self.get_label(&switch.default_bb)?);
        for case in &switch.cases {
            let case_val = match &case.discrim.val.kind {
                OperandKind::Int(i) => {
                    let case_ap = APInt::new_full(*i as u128, bits);
                    if *i > 0 {
                        case_ap.as_unsigned() as i64
                    } else {
                        case_ap.as_signed() as i64
                    }
                }
                _ => {
                    return IRGenErr::value_mismatch_err(
                        &case.discrim.val,
                        "integer constant",
                        self.irgen.source,
                    );
                }
            };
            let case_bb = self.get_label(&case.label)?;
            switch_builder.case(case_val, case_bb);
        }

        // old terminator placeholder disposed
        let switch_inst = ir_builder
            .build_inst(|allocs, _| switch_builder.build_id(allocs))
            .map_err(map_build_err)?;
        self.push_use_by_value(switch_inst.discrim_use(allocs), cond_val, &switch.cond.val);
        Ok(switch_inst)
    }

    fn map_build_err(ast: &dyn AstNode) -> impl (FnOnce(IRBuildError) -> IRGenErr) + 'static {
        let span = ast.get_span();
        move |e: IRBuildError| IRGenErr::ir_build(span, e)
    }
    fn make_and_insert_inst(
        &mut self,
        ir_builder: &mut IRBuilder<&'a Module>,
        ast_inst: &'a InstAst,
    ) -> IRGenRes<InstID> {
        use crate::ast::InstKind as I;
        match &ast_inst.kind {
            I::Unreachable | I::RetVoid | I::Ret(_) | I::Jump(_) | I::Br(_) | I::Switch(_) => {
                panic!("internal error: terminator instruction should not be built here")
            }
            I::Phi(phi_ast) => self.make_phi(ir_builder, phi_ast),
            I::Alloca(alloca_ast) => self.make_alloca(ir_builder, alloca_ast),
            I::GEP(gepast) => self.make_gep(ir_builder, gepast),
            I::Load(load_ast) => self.make_load(ir_builder, load_ast),
            I::Store(store_ast) => self.make_store(ir_builder, store_ast),
            I::Bin(bin_ast) => self.make_binary(ir_builder, bin_ast),
            I::Cast(cast_ast) => self.make_cast(ir_builder, cast_ast),
            I::Cmp(cmp_ast) => self.make_cmp(ir_builder, cmp_ast),
            I::Select(select_ast) => self.make_select(ir_builder, select_ast),
            I::Call(call_ast) => self.make_call(ir_builder, call_ast),
        }
    }
    fn make_phi(
        &mut self,
        ir_builder: &mut IRBuilder<&'a Module>,
        phi_ast: &'a PhiAst,
    ) -> IRGenRes<InstID> {
        let ty = self.irgen.gen_type(&phi_ast.ty)?;
        let allocs = &self.irgen.ir.allocs;
        let mut phi_builder = PhiInst::builder(allocs, ty);
        let mut ops: HashMap<BlockID, &'a Operand> = HashMap::new();
        for (income_op, income_bb) in &phi_ast.incomes {
            let bb_id = self.get_label(&Label {
                span: income_bb.get_span(),
                name: income_bb.name.clone(),
            })?;
            let income_val = self.irgen.gen_value_or_undef(ty, income_op)?;
            phi_builder.add_incoming(bb_id, income_val);
            if let ValueSSA::ConstData(ConstData::Undef(_)) = income_val {
                ops.insert(bb_id, income_op);
            }
        }
        let phi_inst = phi_builder.build_id();
        ir_builder
            .insert_inst(phi_inst)
            .map_err(Self::map_build_err(phi_ast))?;
        for &[uval, ubb] in &*phi_inst.incoming_uses(allocs) {
            let income_op = ops[&BlockID::from_ir(ubb.get_operand(allocs))];
            self.irgen
                .mapping
                .uses
                .push((income_op.get_span(), UseIndex::from_primary(uval, allocs)));
            let ValueSSA::ConstData(ConstData::Undef(_)) = uval.get_operand(allocs) else {
                continue;
            };
            self.use_queue.push(OperandInfo {
                op_use: uval,
                op_type: ty,
                op: income_op,
            });
        }
        Ok(phi_inst.raw_into())
    }
    fn make_alloca(
        &mut self,
        ir_builder: &mut IRBuilder<&'a Module>,
        alloca: &'a AllocaAst,
    ) -> IRGenRes<InstID> {
        let ty = self.irgen.gen_type(&alloca.ty)?;
        let allocs = &self.irgen.ir.allocs;
        let align_log2 = self.make_align(ty, alloca.align)?;
        let alloca_inst = AllocaInstID::new(allocs, ty, align_log2);
        ir_builder
            .insert_inst(alloca_inst.raw_into())
            .map_err(Self::map_build_err(alloca))?;
        Ok(alloca_inst.raw_into())
    }
    fn make_gep(
        &mut self,
        ir_builder: &mut IRBuilder<&'a Module>,
        gep: &'a GEPAst,
    ) -> IRGenRes<InstID> {
        let (allocs, tctx) = self.irgen.tear_module();
        let init_ty = self.irgen.gen_type(&gep.init_ty)?;
        let init_ptr = self
            .irgen
            .gen_value_or_undef(ValTypeID::Ptr, &gep.initptr.val)?;
        let mut gep_builder = GEPInstID::builder(tctx, allocs, init_ty);
        gep_builder.inbounds(gep.inbounds).base_ptr(init_ptr);
        for index in &gep.indices {
            let index_ty = self.irgen.gen_type(&index.ty)?;
            let ValTypeID::Int(_) = index_ty else {
                return IRGenErr::type_mismatch_err(
                    index.get_span(),
                    tctx,
                    "integer type",
                    index_ty,
                );
            };
            let index_val = self.irgen.gen_value_or_undef(index_ty, &index.val)?;
            gep_builder
                .try_add_index(index_val)
                .map_err(|e| IRGenErr::gep_unpack(gep.get_span(), e))?;
        }
        let gep_inst = gep_builder.build_id();
        ir_builder
            .insert_inst(gep_inst)
            .map_err(Self::map_build_err(gep))?;

        for (idx, u) in gep_inst.get_operands(allocs).into_iter().enumerate() {
            let ValueSSA::ConstData(ConstData::Undef(ty)) = u.get_operand(allocs) else {
                continue;
            };
            let op = if idx == 0 {
                &gep.initptr.val
            } else {
                &gep.indices[idx - 1].val
            };
            self.use_queue.push(OperandInfo {
                op_use: u,
                op_type: ty,
                op,
            });
        }
        Ok(gep_inst.raw_into())
    }
    fn make_load(
        &mut self,
        ir_builder: &mut IRBuilder<&'a Module>,
        load: &'a LoadAst,
    ) -> IRGenRes<InstID> {
        let ty = self.irgen.gen_type(&load.ty)?;
        let src = self
            .irgen
            .gen_value_or_undef(ValTypeID::Ptr, &load.src.val)?;

        let allocs = &self.irgen.ir.allocs;
        let align_log2 = self.make_align(ty, load.align)?;
        let load_inst = LoadInstID::new_uninit(allocs, ty, align_log2);
        load_inst.set_source(allocs, src);
        ir_builder
            .insert_inst(load_inst)
            .map_err(Self::map_build_err(load))?;

        self.push_use_by_value(load_inst.source_use(allocs), src, &load.src.val);
        Ok(load_inst.raw_into())
    }
    fn make_store(
        &mut self,
        ir_builder: &mut IRBuilder<&'a Module>,
        store_ast: &'a StoreAst,
    ) -> IRGenRes<InstID> {
        let val_ty = self.irgen.gen_type(&store_ast.val.ty)?;
        let source = self.irgen.gen_value_or_undef(val_ty, &store_ast.val.val)?;
        let target = self
            .irgen
            .gen_value_or_undef(ValTypeID::Ptr, &store_ast.dest.val)?;

        let allocs = &self.irgen.ir.allocs;
        let align_log2 = self.make_align(val_ty, store_ast.align)?;
        let store_inst = StoreInstID::new(allocs, source, target, align_log2);

        self.push_use_by_value(store_inst.source_use(allocs), source, &store_ast.val.val);
        self.push_use_by_value(store_inst.target_use(allocs), target, &store_ast.dest.val);
        ir_builder
            .insert_inst(store_inst)
            .map_err(Self::map_build_err(store_ast))?;
        Ok(store_inst.raw_into())
    }
    fn make_binary(
        &mut self,
        ir_builder: &mut IRBuilder<&'a Module>,
        bin_ast: &'a BinAst,
    ) -> IRGenRes<InstID> {
        let opcode = Opcode::from_str(&bin_ast.op)
            .expect("internal error: binary operator should be valid in parser");
        let lhs_ty = self.irgen.gen_type(&bin_ast.lhs.ty)?;
        let lhs_val = self.irgen.gen_value_or_undef(lhs_ty, &bin_ast.lhs.val)?;
        let rhs_val = self.irgen.gen_value_or_undef(lhs_ty, &bin_ast.rhs)?;

        let binop = ir_builder
            .build_inst(|allocs, _| BinOPInstID::new(allocs, opcode, lhs_val, rhs_val))
            .map_err(Self::map_build_err(bin_ast))?;
        let allocs = &self.irgen.ir.allocs;
        self.push_use_by_value(binop.lhs_use(allocs), lhs_val, &bin_ast.lhs.val);
        self.push_use_by_value(binop.rhs_use(allocs), rhs_val, &bin_ast.rhs);
        binop.set_flags(allocs, bin_ast.flags);
        Ok(binop.raw_into())
    }

    fn make_cast(
        &mut self,
        ir_builder: &mut IRBuilder<&'a Module>,
        cast_ast: &'a CastAst,
    ) -> IRGenRes<InstID> {
        let src_ty = self.irgen.gen_type(&cast_ast.tyval.ty)?;
        let dest_ty = self.irgen.gen_type(&cast_ast.dest_ty)?;
        let src_val = self.irgen.gen_value_or_undef(src_ty, &cast_ast.tyval.val)?;
        let opcode = Opcode::from_str(&cast_ast.op)
            .expect("internal error: cast operator should be valid in parser");
        let cast_inst = ir_builder
            .build_inst(|allocs, _| CastInstID::new(allocs, opcode, src_val, dest_ty))
            .map_err(Self::map_build_err(cast_ast))?;
        let allocs = &self.irgen.ir.allocs;
        self.push_use_by_value(cast_inst.from_use(allocs), src_val, &cast_ast.tyval.val);
        Ok(cast_inst.raw_into())
    }

    fn make_cmp(
        &mut self,
        ir_builder: &mut IRBuilder<&'a Module>,
        cmp_ast: &'a CmpAst,
    ) -> Result<InstID, IRGenErr> {
        let ty = self.irgen.gen_type(&cmp_ast.lhs.ty)?;
        let lhs_val = self.irgen.gen_value_or_undef(ty, &cmp_ast.lhs.val)?;
        let rhs_val = self.irgen.gen_value_or_undef(ty, &cmp_ast.rhs)?;

        let op = Opcode::from_str(&cmp_ast.op)
            .expect("internal error: cmp operator should be valid in parser");
        let cmp_inst = ir_builder
            .build_inst(|allocs, _| {
                let cmp = CmpInstID::new_uninit(allocs, op, cmp_ast.cond, ty);
                cmp.set_lhs(allocs, lhs_val);
                cmp.set_rhs(allocs, rhs_val);
                self.push_use(cmp.lhs_use(allocs), &cmp_ast.lhs.val);
                self.push_use(cmp.rhs_use(allocs), &cmp_ast.rhs);
                cmp
            })
            .map_err(Self::map_build_err(cmp_ast))?;
        Ok(cmp_inst.raw_into())
    }

    fn make_select(
        &mut self,
        ir_builder: &mut IRBuilder<&'a Module>,
        select_ast: &'a SelectAst,
    ) -> Result<InstID, IRGenErr> {
        let cond_val = self
            .irgen
            .gen_value_or_undef(ValTypeID::Int(1), &select_ast.cond.val)?;
        let ty = self.irgen.gen_type(&select_ast.then_val.ty)?;
        let then_val = self
            .irgen
            .gen_value_or_undef(ty, &select_ast.then_val.val)?;
        let else_val = self.irgen.gen_value_or_undef(ty, &select_ast.else_val)?;

        let select_inst = ir_builder
            .build_inst(|allocs, _| {
                let select = SelectInstID::new(allocs, cond_val, then_val, else_val);
                self.push_use(select.cond_use(allocs), &select_ast.cond.val);
                self.push_use(select.then_use(allocs), &select_ast.then_val.val);
                self.push_use(select.else_use(allocs), &select_ast.else_val);
                select
            })
            .map_err(Self::map_build_err(select_ast))?;
        Ok(select_inst.raw_into())
    }

    fn make_call(
        &mut self,
        ir_builder: &mut IRBuilder<&'a Module>,
        call_ast: &'a CallAst,
    ) -> Result<InstID, IRGenErr> {
        let (allocs, tctx) = self.irgen.tear_module();

        let ret_ty = self.irgen.gen_type(&call_ast.ret_ty)?;
        let callee = self
            .irgen
            .gen_value_or_undef(ValTypeID::Ptr, &call_ast.func)?;
        let (arg_tys, arg_vals) = {
            let len = call_ast.args.len();
            let mut arg_tys: SmallVec<[ValTypeID; 8]> = SmallVec::with_capacity(len);
            let mut arg_vals: SmallVec<[ValueSSA; 8]> = SmallVec::with_capacity(len);

            for tyval in &call_ast.args {
                let TypeValue { ty, val } = tyval;
                let arg_ty = self.irgen.gen_type(ty)?;
                let arg_val = self.irgen.gen_value_or_undef(arg_ty, val)?;
                arg_tys.push(arg_ty);
                arg_vals.push(arg_val);
            }
            (arg_tys, arg_vals)
        };

        let callee_ty = {
            let mut functy_builder = FuncTypeBuilder::new(&mut self.irgen.types, tctx);
            functy_builder
                .is_vararg(call_ast.is_vararg)
                .ir_return_type(ret_ty)
                .reserve_args(arg_tys.len());
            for &arg_ty in &arg_tys {
                functy_builder.ir_add_argtype(arg_ty);
            }
            functy_builder.finish()
        };
        let mut call_builder = CallInst::builder(tctx, callee_ty);
        call_builder
            .callee(callee)
            .is_tail_call(call_ast.is_tail)
            .with_args(&arg_vals);
        let call_inst = ir_builder
            .build_inst(|allocs, _| call_builder.build_id(allocs))
            .map_err(Self::map_build_err(call_ast))?;

        for (idx, opuse) in call_inst.get_operands(allocs).iter().enumerate() {
            let op = if idx == 0 {
                &call_ast.func
            } else {
                &call_ast.args[idx - 1].val
            };
            self.push_use(*opuse, op);
        }
        Ok(call_inst.raw_into())
    }
}
