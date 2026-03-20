use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    io::Write,
    ops::Range,
    rc::Rc,
};

use remusys_ir::{
    ir::{
        ArrayExpr, AttrSet, Attribute, BlockID, BlockObj, ConstArrayData, ConstData, DataArrayExpr,
        ExprID, ExprObj, FixVec, FuncBody, FuncID, FuncNumberMap, FuncObj, GlobalVar, GlobalVarID,
        IArrayExpr, IPtrUniqueUser, IPtrValue, IRAllocs, IRNameMap, ISubExprID, ISubGlobal,
        ISubGlobalID, ISubInst, ISubValueSSA, ITerminatorInst, ITraceableValue, IUser,
        IValueConvert, InstID, InstObj, JumpTargetID, KVArrayExpr, Module as IRModule,
        NumberOption, PtrArgTargetAttr, SplatArrayExpr, StructExpr, UseID, ValueSSA, inst::*,
    },
    typing::{AggrType, IValType, ScalarType, ValTypeID},
};
use smallvec::{SmallVec, smallvec};
use smol_str::{SmolStr, format_smolstr};

use crate::{
    source_buf::IRSourceBuf,
    source_tree::{IRSrcTreePos, IRTree, IRTreeErr, IRTreeNode, IRTreeNodeID, IRTreeObjID},
};

#[derive(Debug, thiserror::Error)]
pub enum IRTreeBuildErr {
    #[error("{0}")]
    TreeErr(#[from] IRTreeErr),

    #[error("{0}")]
    IOErr(#[from] std::io::Error),

    #[error("{0}")]
    FmtErr(#[from] std::fmt::Error),
    // more custom errors...
}

pub type IRTreeBuildRes<T = ()> = Result<T, IRTreeBuildErr>;
pub type IRTreeNodeBuildRes = IRTreeBuildRes<IRTreeNodeID>;
pub type IRTreeChildren = IRTreeBuildRes<SmallVec<[IRTreeNodeID; 5]>>;

#[derive(Default)]
pub struct StatCache<'s> {
    funcs: HashMap<FuncID, Rc<FuncNumberMap<'s>>>,
    type_names: HashMap<ValTypeID, SmolStr>,
    str_literals: HashMap<ExprID, Option<SmolStr>>,
}

pub struct IRTreeBuilder<'ir, 'tree> {
    pub module: &'ir IRModule,
    pub names: &'ir IRNameMap,
    tree: &'tree mut IRTree,
    write: IRSourceBuf,
    modify_lines: Range<usize>,
    stat: RefCell<StatCache<'ir>>,
    indent: Cell<usize>,
}
impl<'ir, 'tree> IRTreeBuilder<'ir, 'tree> {
    pub fn new_modify(
        module: &'ir IRModule,
        names: &'ir IRNameMap,
        tree: &'tree mut IRTree,
        modify_lines: Range<usize>,
    ) -> Self {
        Self {
            module,
            names,
            tree,
            write: IRSourceBuf::default(),
            modify_lines,
            stat: RefCell::default(),
            indent: Cell::new(0),
        }
    }
    pub fn new(module: &'ir IRModule, names: &'ir IRNameMap, tree: &'tree mut IRTree) -> Self {
        Self::new_modify(module, names, tree, 1usize..1usize)
    }
    pub fn curr_pos(&self) -> IRSrcTreePos {
        let begin = self.modify_lines.start;
        let mut pos = self.write.end_pos();
        pos.line += begin.saturating_sub(1) as u32;
        pos
    }
    pub fn get_indent(&self) -> usize {
        self.indent.get()
    }
    pub fn set_indent(&self, indent: usize) {
        self.indent.set(indent);
    }
    pub fn add_indent(&self, delta: usize) {
        self.set_indent(self.get_indent() + delta);
    }
    pub fn sub_indent(&self, delta: usize) {
        self.set_indent(self.get_indent().saturating_sub(delta));
    }
    /// write a list of spaces for indent.
    pub fn write_indent(&mut self) -> IRTreeBuildRes {
        for _ in 0..self.get_indent() {
            self.write_str("    ")?;
        }
        Ok(())
    }
    /// write a new line and a list of spaces for indent.
    pub fn writeln_indent(&mut self) -> IRTreeBuildRes {
        self.write_str("\n")?;
        self.write_indent()
    }

    /* BEGIN: add these functions to make `write!()` macro happy.
     * NOTE that when `write!` is applied on IR tree builder, it
     * will return `IRTreeBuildErr` instead of `std::io::Error` */
    pub fn write(&mut self, buf: &[u8]) -> IRTreeBuildRes<usize> {
        self.write.write(buf).map_err(IRTreeBuildErr::IOErr)
    }
    pub fn write_str(&mut self, s: &str) -> IRTreeBuildRes {
        self.write
            .write_all(s.as_bytes())
            .map_err(IRTreeBuildErr::IOErr)
    }
    pub fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> IRTreeBuildRes {
        self.write.write_fmt(fmt).map_err(IRTreeBuildErr::IOErr)
    }
    /* END: functions to meet `write!()` macro */

    fn fmt_const_data(&mut self, data: ConstData) -> IRTreeBuildRes {
        use remusys_ir::ir::ConstData::*;
        match data {
            Undef(_) => self.write_str("undef"),
            PtrNull | Zero(ScalarType::Ptr) => self.write_str("null"),
            Zero(ScalarType::Int(1)) => self.write_str("false"),
            Zero(ScalarType::Int(_)) => self.write_str("0"),
            Zero(ScalarType::Float(_)) => self.write_str("0.0"),
            Int(apint) => write!(self, "{}", apint.as_signed()),
            Float(_, fp) => write!(self, "{fp}"),
        }
    }

    pub fn build_func(&mut self, func: FuncID) -> IRTreeNodeBuildRes {
        let allocs = &self.module.allocs;
        let mut stat = if func.is_extern(allocs) {
            IRTreeBuilderStat::of_global(self)
        } else {
            IRTreeBuilderStat::of_local(self, func)
        };
        stat.do_fmt_func(func, func.deref_ir(allocs))
    }
    pub fn update_block(&mut self, block: BlockID) -> IRTreeNodeBuildRes {
        let allocs = &self.module.allocs;
        let mut stat = match block.get_parent_func(allocs) {
            Some(func) => IRTreeBuilderStat::of_local(self, func),
            None => IRTreeBuilderStat::of_global(self),
        };
        stat.do_fmt_block(block, block.deref_ir(allocs))
    }
    pub fn update_inst(&mut self, inst: InstID) -> IRTreeNodeBuildRes {
        let allocs = &self.module.allocs;
        let mut stat = match inst.get_parent_func(allocs) {
            Some(func) => IRTreeBuilderStat::of_local(self, func),
            None => IRTreeBuilderStat::of_global(self),
        };
        // manually indent
        let orig = stat.builder.get_indent();
        stat.builder.add_indent(1);
        stat.builder.write_indent()?;
        // do not use `fmt_inst_line` because it will put a line feed
        let node_or_err = stat.do_fmt_inst(inst, inst.deref_ir(allocs));
        stat.builder.set_indent(orig); // keep the error passing through
        node_or_err
    }
    pub fn build_overview(&mut self) -> IRTreeNodeBuildRes {
        let module = self.module;
        let allocs = &module.allocs;
        let symbols = module.symbols.borrow();
        let mut children = SmallVec::new();
        for &gvar_id in symbols.var_pool().iter() {
            let mut stat = IRTreeBuilderStat::of_global(self);
            let child = stat.fmt_global_var(gvar_id, gvar_id.deref_ir(allocs))?;
            children.push(child);
            self.writeln_indent()?;
        }
        for &func_id in symbols.func_pool().iter() {
            let begin = self.curr_pos();
            let mut stat = if func_id.is_extern(allocs) {
                IRTreeBuilderStat::of_global(self)
            } else {
                IRTreeBuilderStat::of_local(self, func_id)
            };
            let fchild = stat.fmt_func_header(func_id, func_id.deref_ir(allocs))?;
            let node = IRTreeNode {
                parent: None,
                ir_obj: IRTreeObjID::Global(func_id.raw_into()),
                src_span: begin..self.curr_pos(),
                children: fchild,
                depth: 0,
            };
            children.push(IRTreeNodeID::allocate(&mut self.tree.alloc, node));
            self.writeln_indent()?;
        }

        let node = IRTreeNode {
            parent: None,
            ir_obj: IRTreeObjID::Module,
            src_span: self.curr_pos()..self.curr_pos(),
            children,
            depth: 0,
        };
        Ok(IRTreeNodeID::allocate(&mut self.tree.alloc, node))
    }
}

struct IRTreeBuilderStat<'b, 'ir, 'tree> {
    builder: &'b mut IRTreeBuilder<'ir, 'tree>,
    module: &'ir IRModule,
    names: &'ir IRNameMap,
    local: Option<Rc<FuncNumberMap<'ir>>>,
}
impl<'b, 'ir, 'tree> IRTreeBuilderStat<'b, 'ir, 'tree> {
    #[allow(dead_code)]
    pub fn of_global(builder: &'b mut IRTreeBuilder<'ir, 'tree>) -> Self {
        Self {
            module: builder.module,
            names: builder.names,
            builder,
            local: None,
        }
    }
    #[allow(dead_code)]
    pub fn of_local(builder: &'b mut IRTreeBuilder<'ir, 'tree>, func: FuncID) -> Self {
        let mut stat = builder.stat.borrow_mut();
        let entry = stat.funcs.entry(func);
        let curr_func = entry
            .or_insert_with(|| Self::new_local(builder, func))
            .clone();
        drop(stat);
        Self {
            module: builder.module,
            names: builder.names,
            builder,
            local: Some(curr_func),
        }
    }
    fn new_local(builder: &IRTreeBuilder<'ir, '_>, func: FuncID) -> Rc<FuncNumberMap<'ir>> {
        let allocs = &builder.module.allocs;
        let option = NumberOption::ignore_all();
        let numbers = FuncNumberMap::new(allocs, func, builder.names, option);
        Rc::new(numbers)
    }
    fn type_get_name(&self, ty: impl IValType) -> SmolStr {
        use remusys_ir::typing::ValTypeID::*;
        let tctx = &self.module.tctx;
        let ty = ty.into_ir();
        if matches!(ty, Void | Ptr | Int(_) | Float(_)) {
            return ty.get_display_name(tctx);
        }
        let mut stat = self.builder.stat.borrow_mut();
        stat.type_names
            .entry(ty)
            .or_insert_with(|| ty.get_display_name(tctx))
            .clone()
    }
    fn get_local_name(&self, val: impl IValueConvert) -> SmolStr {
        let val = val.into_value();
        let name = match &self.local {
            Some(local) => local.get_local_name(val),
            None => self.names.get_local_name(val),
        };
        if let Some(name) = name {
            return name;
        }
        match val {
            ValueSSA::Block(bb) => bb.to_strid(),
            ValueSSA::Inst(inst) => inst.to_strid(),
            ValueSSA::FuncArg(func, arg) => {
                format_smolstr!("FuncArg({},{arg})", func.raw_into().to_strid())
            }
            _ => format_smolstr!("<unnamed {val:?}>"),
        }
    }
    fn try_local_name(&self, val: impl IValueConvert) -> Option<SmolStr> {
        let val = val.into_value();
        match &self.local {
            Some(local) => local.get_local_name(val),
            None => self.names.get_local_name(val),
        }
    }
    #[allow(dead_code)]
    pub fn fmt_use(&mut self, use_id: UseID) -> IRTreeNodeBuildRes {
        let begin = self.builder.curr_pos();
        if let Some(node) = self.may_fmt_use(use_id)? {
            Ok(node)
        } else {
            let alloc = &self.builder.tree.alloc;
            let ir_obj = IRTreeObjID::Use(use_id);
            let src_span = begin..self.builder.curr_pos();
            Ok(IRTreeNodeID::new(alloc, ir_obj, src_span))
        }
    }
    pub fn may_fmt_use(&mut self, use_id: UseID) -> IRTreeBuildRes<Option<IRTreeNodeID>> {
        let allocs = &self.module.allocs;
        let operand = use_id.get_operand(allocs);

        let begin = self.builder.curr_pos();
        let mut children = SmallVec::new();
        let mut makes_node = false;
        match operand {
            ValueSSA::None => self.builder.write_str("none")?,
            ValueSSA::ConstData(data) => self.builder.fmt_const_data(data)?,
            ValueSSA::AggrZero(_) => self.builder.write_str("zeroinitializer")?,
            ValueSSA::Global(g) => {
                write!(self.builder, "@{}", g.get_name(allocs))?;
                makes_node = true;
            }
            ValueSSA::ConstExpr(expr_id) => {
                children = self.fmt_expr(expr_id)?;
                makes_node = !children.is_empty();
            }
            val => {
                write!(self.builder, "%{}", self.get_local_name(val))?;
                makes_node = true;
            }
        }
        if !makes_node {
            return Ok(None);
        }
        let node = IRTreeNode {
            parent: None,
            ir_obj: IRTreeObjID::Use(use_id),
            src_span: begin..self.builder.curr_pos(),
            children,
            depth: 0,
        };
        Ok(Some(IRTreeNodeID::allocate(
            &mut self.builder.tree.alloc,
            node,
        )))
    }
    #[allow(dead_code)]
    pub fn fmt_label(&mut self, jt: JumpTargetID) -> IRTreeNodeBuildRes {
        let begin = self.builder.curr_pos();
        let allocs = &self.module.allocs;
        let name = match jt.get_block(allocs) {
            Some(block) => self.get_local_name(block),
            None => SmolStr::new_inline("none"),
        };
        let builder = &mut self.builder;
        write!(builder, "%{name}")?;
        Ok(IRTreeNodeID::new(
            &builder.tree.alloc,
            IRTreeObjID::JumpTarget(jt),
            begin..builder.curr_pos(),
        ))
    }
    fn fmt_expr(&mut self, expr: ExprID) -> IRTreeChildren {
        if let Some(litstr) = self.expr_as_str(expr) {
            self.builder.write_str(&litstr)?;
            return Ok(SmallVec::new());
        }
        let allocs = &self.module.allocs;
        match expr.deref_ir(allocs) {
            ExprObj::Array(arr) => self.fmt_array(arr),
            ExprObj::DataArray(darr) => self.fmt_darray(darr).map(|_| SmallVec::new()),
            ExprObj::SplatArray(sarr) => self.fmt_sarray(sarr),
            ExprObj::KVArray(kv) => self.fmt_kvarray(kv),
            ExprObj::Struct(st) => self.fmt_struct(st),
            ExprObj::FixVec(vec) => self.fmt_fixvec(vec),
        }
    }
    fn fmt_array(&mut self, arr: &ArrayExpr) -> IRTreeChildren {
        self.fmt_aggr(
            ["[", "]"],
            arr.get_operands().as_slice(),
            AggrType::Array(arr.arrty),
        )
    }
    fn fmt_struct(&mut self, struc: &StructExpr) -> IRTreeChildren {
        let ops = struc.get_operands();
        let tctx = &self.module.tctx;
        let decorates = if struc.structty.is_packed(tctx) {
            ["<{", "}>"]
        } else {
            ["{", "}"]
        };
        self.fmt_aggr(decorates, ops.as_slice(), AggrType::Struct(struc.structty))
    }
    fn fmt_fixvec(&mut self, vec: &FixVec) -> IRTreeChildren {
        self.fmt_aggr(
            ["<", ">"],
            vec.get_operands().as_slice(),
            AggrType::FixVec(vec.vecty),
        )
    }
    fn fmt_aggr(&mut self, decorates: [&str; 2], ops: &[UseID], ty: AggrType) -> IRTreeChildren {
        let [begin_s, end_s] = decorates;
        let tctx = &self.module.tctx;
        self.builder.write_str(begin_s)?;
        let mut children = SmallVec::with_capacity(ops.len());
        for (i, &uid) in ops.iter().enumerate() {
            if i > 0 {
                self.builder.write_str(", ")?;
            }
            let elemty = self.type_get_name(ty.get_field(tctx, i));
            write!(self.builder, "{elemty} ")?;
            if let Some(node) = self.may_fmt_use(uid)? {
                children.push(node)
            }
        }
        self.builder.write_str(end_s)?;
        Ok(children)
    }
    fn fmt_darray(&mut self, darr: &DataArrayExpr) -> IRTreeBuildRes {
        // data array has no traced operand. its pure-constant elements are stored in another place
        // so data array formatting should never return an array of children
        self.builder.write_str("[ ")?;
        for i in 0..darr.data.len() {
            if i > 0 {
                self.builder.write_str(", ")?;
            }
            // Add type annotation before the value
            let elem_ty = darr.elemty.into_ir();
            write!(self.builder, "{} ", self.type_get_name(elem_ty))?;
            let value = darr.data.index_get_const(i);
            self.builder.fmt_const_data(value)?;
        }
        self.builder.write_str(" ]")?;
        Ok(())
    }
    fn fmt_sarray(&mut self, sarr: &SplatArrayExpr) -> IRTreeChildren {
        // splat array has only one operand. its elements are the repeats of this operand.
        // Every node of splat array is the repetition of the `Use` but at its own array index.
        let mut res = SmallVec::with_capacity(sarr.nelems);
        self.builder.write_str("[ ")?;
        for i in 0..sarr.nelems {
            if i > 0 {
                self.builder.write_str(", ")?;
            }
            // Add type annotation before the use
            write!(self.builder, "{} ", self.type_get_name(sarr.elemty))?;
            if let Some(node) = self.may_fmt_use(sarr.element[0])? {
                res.push(node)
            }
        }
        self.builder.write_str(" ]")?;
        Ok(res)
    }
    fn fmt_kvarray(&mut self, kvarr: &KVArrayExpr) -> IRTreeChildren {
        // K-V sparse array has a totally different syntax. follow the syntax rule and gather `use` edges.
        let mut children = SmallVec::new();
        let allocs = &self.module.allocs;
        let elemty = self.type_get_name(kvarr.elemty);
        self.builder.write_str("sparse [ ")?;
        let mut first = true;

        // Format explicit key-value pairs
        for (idx, _, use_id) in kvarr.elem_iter(allocs) {
            if first {
                first = false;
            } else {
                self.builder.write_str(", ")?;
            }
            write!(self.builder, "[{idx}] = {elemty} ")?;
            if let Some(node) = self.may_fmt_use(use_id)? {
                children.push(node)
            }
        }

        // Format default value
        if !first {
            self.builder.write_str(", ")?;
        }
        write!(self.builder, "..= {elemty} ")?;
        if let Some(node) = self.may_fmt_use(kvarr.default_use())? {
            children.push(node)
        }
        self.builder.write_str(" ]")?;
        Ok(children)
    }

    fn expr_as_str(&self, expr: ExprID) -> Option<SmolStr> {
        fn value_as_u8(v: ValueSSA) -> Option<u8> {
            v.as_apint().map(|a| a.as_unsigned() as u8)
        }
        fn bytes_as_litstr(iter: impl Iterator<Item = Option<u8>>) -> Option<SmolStr> {
            use std::io::Write;
            let mut s: SmallVec<[u8; 40]> = SmallVec::with_capacity(iter.size_hint().0 + 3);
            s.extend_from_slice(b"c\"");
            for ch in iter {
                match ch? {
                    b'"' => s.extend_from_slice(b"\\22"),
                    b'\\' => s.extend_from_slice(b"\\5c"),
                    b' ' => s.push(b' '),
                    ch if ch.is_ascii_graphic() => s.push(ch),
                    ch => write!(&mut s, "\\{ch:02x}").ok()?,
                }
            }
            s.push(b'\"');
            Some(SmolStr::from(unsafe {
                // # SAFETY: ensured valid UTF-8
                str::from_utf8_unchecked(s.as_slice())
            }))
        }
        fn array_as_litstr(module: &IRModule, arr: &impl IArrayExpr) -> Option<SmolStr> {
            let allocs = &module.allocs;
            let ValTypeID::Int(8) = arr.get_elem_type() else {
                return None;
            };
            let bytes = arr.value_iter(allocs).map(value_as_u8);
            bytes_as_litstr(bytes)
        }
        fn darray_as_litstr(darr: &DataArrayExpr) -> Option<SmolStr> {
            let ConstArrayData::I8(i8arr) = &darr.data else {
                return None;
            };
            bytes_as_litstr(i8arr.iter().map(|x| Some(*x as u8)))
        }

        let mut stat = self.builder.stat.borrow_mut();
        if let Some(s) = stat.str_literals.get(&expr) {
            return s.clone();
        }

        let allocs = &self.module.allocs;
        let s = match expr.deref_ir(allocs) {
            ExprObj::Array(a) => array_as_litstr(self.module, a),
            ExprObj::DataArray(da) => darray_as_litstr(da),
            ExprObj::SplatArray(sa) => array_as_litstr(self.module, sa),
            ExprObj::KVArray(_) => None,
            ExprObj::Struct(_) | ExprObj::FixVec(_) => None,
        };
        stat.str_literals.insert(expr, s.clone());
        s
    }

    pub fn fmt_inst_line(&mut self, id: InstID, inst: &'ir InstObj) -> IRTreeNodeBuildRes {
        self.builder.write_indent()?;
        let node = self.do_fmt_inst(id, inst)?;
        self.builder.write_str("\n")?;
        Ok(node)
    }
    pub fn do_fmt_inst(&mut self, id: InstID, inst: &'ir InstObj) -> IRTreeNodeBuildRes {
        let begin = self.builder.curr_pos();
        let allocs = &self.module.allocs;

        if let Some(name) = self.try_local_name(id) {
            write!(self.builder, "%{name} = ")?;
        }
        let children: SmallVec<[IRTreeNodeID; 5]> = match inst {
            InstObj::GuideNode(_) => smallvec![],
            InstObj::PhiInstEnd(_) => {
                self.builder.write_str("; ==== Phi Section End ====")?;
                smallvec![]
            }
            InstObj::Unreachable(_) => {
                self.builder.write_str("unreachable")?;
                smallvec![]
            }
            InstObj::Ret(ret_inst) => self.fmt_ret_inst(ret_inst)?,
            InstObj::Jump(jump_inst) => {
                self.builder.write_str("br label ")?;
                let label_node = self.fmt_label(jump_inst.target_jt())?;
                smallvec![label_node]
            }
            InstObj::Br(br_inst) => {
                self.builder.write_str("br i1 ")?;
                let cond_node = self.fmt_use(br_inst.cond_use())?;
                self.builder.write_str(", label ")?;
                let then_node = self.fmt_label(br_inst.then_jt())?;
                self.builder.write_str(", label ")?;
                let else_node = self.fmt_label(br_inst.else_jt())?;
                smallvec![cond_node, then_node, else_node]
            }
            InstObj::Switch(switch_inst) => self.fmt_switch_inst(allocs, switch_inst)?,
            InstObj::Alloca(alloca_inst) => {
                self.builder.write_str("alloca ")?;
                let ty_name = self.type_get_name(alloca_inst.pointee_ty);
                let align = alloca_inst.get_ptr_pointee_align();
                write!(self.builder, "{ty_name} , align {align}")?;
                smallvec![]
            }
            InstObj::GEP(gep_inst) => self.fmt_gep_inst(allocs, gep_inst)?,
            InstObj::Load(load_inst) => self.fmt_load_inst(load_inst)?,
            InstObj::Store(store_inst) => self.fmt_store_inst(store_inst)?,
            InstObj::AmoRmw(amo_rmw_inst) => self.fmt_amormw_inst(amo_rmw_inst)?,
            InstObj::BinOP(inst) => self.fmt_binop_inst(inst)?,
            InstObj::Call(call_inst) => self.fmt_call_inst(allocs, call_inst)?,
            InstObj::Cast(cast_inst) => {
                let opcode = cast_inst.get_opcode().get_name();
                let from_ty_name = self.type_get_name(cast_inst.from_ty);
                write!(self.builder, "{opcode} {from_ty_name} ")?;
                let from_node = self.fmt_use(cast_inst.from_use())?;
                let to_ty_name = self.type_get_name(cast_inst.get_valtype());
                write!(self.builder, " to {to_ty_name}")?;
                smallvec![from_node]
            }
            InstObj::Cmp(cmp_inst) => {
                let opcode = cmp_inst.get_opcode().get_name();
                let cond = cmp_inst.cond;
                let operand_ty_name = self.type_get_name(cmp_inst.operand_ty);
                write!(self.builder, "{opcode} {cond} {operand_ty_name} ")?;
                let lhs_node = self.fmt_use(cmp_inst.lhs_use())?;
                self.builder.write_str(", ")?;
                let rhs_node = self.fmt_use(cmp_inst.rhs_use())?;
                smallvec![lhs_node, rhs_node]
            }
            InstObj::IndexExtract(index_extract_inst) => {
                let aggr_ty_name = self.type_get_name(index_extract_inst.aggr_type.into_ir());
                let index = index_extract_inst.get_index(allocs);
                let index_ty_name = self.type_get_name(index.get_valtype(allocs));

                write!(self.builder, "extractelement {aggr_ty_name} ")?;
                let aggr_node = self.fmt_use(index_extract_inst.aggr_use())?;
                write!(self.builder, ", {index_ty_name} ")?;
                let index_node = self.fmt_use(index_extract_inst.index_use())?;
                smallvec![aggr_node, index_node]
            }
            InstObj::FieldExtract(field_extract_inst) => {
                let aggr_ty_name = self.type_get_name(field_extract_inst.aggr_type.into_ir());
                write!(self.builder, "extractvalue {aggr_ty_name} ")?;
                let aggr_node = self.fmt_use(field_extract_inst.aggr_use())?;
                for &idx in field_extract_inst.get_field_indices() {
                    write!(self.builder, ", {idx}")?;
                }
                smallvec![aggr_node]
            }
            InstObj::IndexInsert(index_insert_inst) => {
                let aggr_ty_name = self.type_get_name(index_insert_inst.get_valtype());
                let elem_ty_name = self.type_get_name(index_insert_inst.get_elem_type());
                let index = index_insert_inst.get_index(allocs);
                let index_ty_name = self.type_get_name(index.get_valtype(allocs));

                write!(self.builder, "insertelement {aggr_ty_name} ")?;
                let aggr_node = self.fmt_use(index_insert_inst.aggr_use())?;
                write!(self.builder, ", {elem_ty_name} ")?;
                let elem_node = self.fmt_use(index_insert_inst.elem_use())?;
                write!(self.builder, ", {index_ty_name} ")?;
                let index_node = self.fmt_use(index_insert_inst.index_use())?;
                smallvec![aggr_node, elem_node, index_node]
            }
            InstObj::FieldInsert(field_insert_inst) => {
                let aggr_ty_name = self.type_get_name(field_insert_inst.get_valtype());
                let elem_ty_name = self.type_get_name(field_insert_inst.get_elem_type());

                write!(self.builder, "insertvalue {aggr_ty_name} ")?;
                let aggr_node = self.fmt_use(field_insert_inst.aggr_use())?;
                write!(self.builder, ", {elem_ty_name} ")?;
                let elem_node = self.fmt_use(field_insert_inst.elem_use())?;
                for &idx in field_insert_inst.get_field_indices() {
                    write!(self.builder, ", {idx}")?;
                }
                smallvec![aggr_node, elem_node]
            }
            InstObj::Phi(phi_inst) => self.fmt_phi_inst(phi_inst)?,
            InstObj::Select(select_inst) => self.fmt_select_inst(select_inst)?,
        };
        let node = IRTreeNode {
            parent: None,
            ir_obj: IRTreeObjID::Inst(id),
            src_span: begin..self.builder.curr_pos(),
            children,
            depth: 0,
        };
        Ok(IRTreeNodeID::allocate(&mut self.builder.tree.alloc, node))
    }

    fn fmt_select_inst(&mut self, select_inst: &SelectInst) -> IRTreeChildren {
        let ty_name = self.type_get_name(select_inst.get_valtype());
        write!(self.builder, "select {ty_name}, i1 ")?;
        let cond_node = self.fmt_use(select_inst.cond_use())?;
        self.builder.write_str(", ")?;
        let then_node = self.fmt_use(select_inst.then_use())?;
        self.builder.write_str(", ")?;
        let else_node = self.fmt_use(select_inst.else_use())?;
        Ok(smallvec![cond_node, then_node, else_node])
    }

    fn fmt_phi_inst(&mut self, phi_inst: &PhiInst) -> IRTreeChildren {
        let ty_name = self.type_get_name(phi_inst.get_valtype());
        write!(self.builder, "phi {ty_name} ")?;
        let mut children = SmallVec::with_capacity(phi_inst.incoming_uses().len() * 2);
        let mut first = true;
        for [uval, ublk] in phi_inst.incoming_uses().iter() {
            self.builder.write_str(if first { " [" } else { ", [" })?;
            first = false;

            let val_node = self.fmt_use(*uval)?;
            self.builder.write_str(", label ")?;
            let blk_node = self.fmt_use(*ublk)?;
            self.builder.write_str("]")?;
            children.push(val_node);
            children.push(blk_node);
        }
        Ok(children)
    }

    fn fmt_call_inst(&mut self, allocs: &IRAllocs, inst: &CallInst) -> IRTreeChildren {
        let ret_ty_name = self.type_get_name(inst.get_valtype());
        if inst.is_vararg {
            write!(self.builder, "call {ret_ty_name} (...) ")?;
        } else {
            write!(self.builder, "call {ret_ty_name} ")?;
        }
        let callee_node = self.fmt_use(inst.callee_use())?;
        self.builder.write_str("(")?;
        let mut children = SmallVec::with_capacity(inst.operands.len());
        children.push(callee_node);
        let tctx = &self.module.tctx;
        for (i, &arg_use) in inst.arg_uses().iter().enumerate() {
            if i > 0 {
                self.builder.write_str(", ")?;
            }
            let arg_ty = match inst.callee_ty.get_args(tctx).get(i) {
                Some(arg) => *arg,
                None => arg_use.get_operand(allocs).get_valtype(allocs),
            };
            let arg_ty_name = self.type_get_name(arg_ty);
            write!(self.builder, "{arg_ty_name} ")?;
            let arg_node = self.fmt_use(arg_use)?;
            children.push(arg_node);
        }
        self.builder.write_str(")")?;
        Ok(children)
    }

    fn fmt_binop_inst(&mut self, inst: &BinOPInst) -> IRTreeChildren {
        let opcode = inst.get_opcode().get_name();
        let flags = inst.get_flags();
        let ty_name = self.type_get_name(inst.get_valtype());
        if flags.is_empty() {
            write!(self.builder, "{opcode} {ty_name} ")?;
        } else {
            write!(self.builder, "{opcode} {flags} {ty_name} ")?;
        }
        let lhs_node = self.fmt_use(inst.lhs_use())?;
        self.builder.write_str(", ")?;
        let rhs_node = self.fmt_use(inst.rhs_use())?;
        Ok(smallvec![lhs_node, rhs_node])
    }

    fn fmt_amormw_inst(&mut self, inst: &AmoRmwInst) -> IRTreeChildren {
        let subop_name = inst.subop_name();
        if inst.is_volatile {
            write!(self.builder, "atomicrmw volatile {subop_name} ptr ")?;
        } else {
            write!(self.builder, "atomicrmw {subop_name} ptr ")?;
        }
        let pointer_node = self.fmt_use(inst.pointer_use())?;
        let value_ty_name = self.type_get_name(inst.value_ty);
        write!(self.builder, ", {value_ty_name} ")?;
        let value_node = self.fmt_use(inst.value_use())?;
        if inst.scope != remusys_ir::ir::SyncScope::System {
            write!(self.builder, " syncscope(\"{}\")", inst.scope.as_str())?;
        }
        write!(self.builder, " {}", inst.ordering.as_str())?;
        if inst.align_log2 > 0 {
            write!(self.builder, ", align {}", 1 << inst.align_log2)?;
        }
        Ok(smallvec![pointer_node, value_node])
    }

    fn fmt_store_inst(&mut self, inst: &StoreInst) -> IRTreeChildren {
        let source_ty_name = self.type_get_name(inst.source_ty);
        write!(self.builder, "store {source_ty_name} ")?;
        let source_node = self.fmt_use(inst.source_use())?;
        self.builder.write_str(", ptr ")?;
        let target_node = self.fmt_use(inst.target_use())?;
        write!(self.builder, ", align {}", inst.get_operand_pointee_align())?;
        Ok(smallvec![source_node, target_node])
    }

    fn fmt_load_inst(&mut self, inst: &LoadInst) -> IRTreeChildren {
        let pointee_ty_name = self.type_get_name(inst.get_valtype());
        write!(self.builder, "load {pointee_ty_name}, ptr ")?;
        let source_node = self.fmt_use(inst.source_use())?;
        write!(self.builder, ", align {}", inst.get_operand_pointee_align())?;
        Ok(smallvec![source_node])
    }

    fn fmt_gep_inst(&mut self, allocs: &IRAllocs, inst: &GEPInst) -> IRTreeChildren {
        if inst.get_inbounds() {
            self.builder.write_str("getelementptr inbounds ")?;
        } else {
            self.builder.write_str("getelementptr ")?;
        }
        let initial_ty_name = self.type_get_name(inst.initial_ty);
        write!(self.builder, "{initial_ty_name}, ptr ")?;
        let base_node = self.fmt_use(inst.base_use())?;
        let mut children = SmallVec::with_capacity(inst.get_operands().len());
        children.push(base_node);
        for &index_use in inst.index_uses() {
            let index_ty_name =
                self.type_get_name(index_use.get_operand(allocs).get_valtype(allocs));
            self.builder.write_str(", ")?;
            write!(self.builder, "{index_ty_name} ")?;
            let index_node = self.fmt_use(index_use)?;
            children.push(index_node);
        }
        Ok(children)
    }

    fn fmt_switch_inst(&mut self, allocs: &IRAllocs, inst: &SwitchInst) -> IRTreeChildren {
        let cond_ty = self.type_get_name(inst.discrim_ty);
        write!(self.builder, "switch {cond_ty} ")?;
        let discrim_node = self.fmt_use(inst.discrim_use())?;
        self.builder.write_str(", label ")?;
        let default_node = self.fmt_label(inst.default_jt())?;
        let mut children = SmallVec::with_capacity(inst.n_jump_targets() + 1);
        children.extend([discrim_node, default_node]);
        if inst.case_jts().is_empty() {
            self.builder.write_str(" []")?;
        } else {
            self.builder.write_str(" [")?;
            self.builder.add_indent(1);
            for (case_jt, case_val, _) in inst.cases_iter(allocs) {
                self.builder.writeln_indent()?;
                write!(self.builder, "{cond_ty} {case_val}, label ")?;
                let case_node = self.fmt_label(case_jt)?;
                children.push(case_node);
            }
            self.builder.sub_indent(1);
            self.builder.writeln_indent()?;
            self.builder.write_str(" ]")?;
        }
        Ok(children)
    }

    fn fmt_ret_inst(&mut self, ret_inst: &RetInst) -> IRTreeChildren {
        let mut children = SmallVec::new();
        if ret_inst.get_valtype() == ValTypeID::Void {
            self.builder.write_str("ret void")?;
        } else {
            self.builder.write_str("ret ")?;
            let ty_name = self.type_get_name(ret_inst.get_valtype());
            write!(self.builder, "{ty_name} ")?;
            let use_node = self.fmt_use(ret_inst.retval_use())?;
            children.push(use_node);
        }
        Ok(children)
    }

    pub fn do_fmt_block(&mut self, id: BlockID, block: &'ir BlockObj) -> IRTreeNodeBuildRes {
        let allocs = &self.module.allocs;

        self.builder.write_indent()?;
        // 写入块标签（带缩进和%前缀）
        let begin = self.builder.curr_pos();
        writeln!(self.builder, "%{}", self.get_local_name(id))?;

        // 增加缩进级别以便指令缩进
        self.builder.add_indent(1);

        // 收集指令节点
        let mut children = SmallVec::new();
        for (inst_id, inst) in block.get_insts().iter(&allocs.insts) {
            let inst_node = self.fmt_inst_line(inst_id, inst)?;
            if !matches!(inst, InstObj::GuideNode(_) | InstObj::PhiInstEnd(_)) {
                children.push(inst_node); // These are not valid nodes!
            }
        }

        // 恢复缩进级别
        self.builder.sub_indent(1);

        // 创建块节点
        let node = IRTreeNode {
            parent: None,
            ir_obj: IRTreeObjID::Block(id),
            src_span: begin..self.builder.curr_pos(),
            children,
            depth: 0,
        };
        Ok(IRTreeNodeID::allocate(&mut self.builder.tree.alloc, node))
    }

    fn fmt_attr(&mut self, attr: &Attribute) -> IRTreeBuildRes {
        match attr {
            Attribute::NoUndef => self.builder.write_str("noundef"),
            Attribute::IntExt(iext) => self.builder.write_str(iext.as_str()),
            Attribute::PtrReadOnly => self.builder.write_str("readonly"),
            Attribute::PtrNoCapture => self.builder.write_str("nocapture"),
            Attribute::FuncNoReturn => self.builder.write_str("noreturn"),
            Attribute::FuncInline(inline) => self.builder.write_str(inline.as_str()),
            Attribute::FuncAlignStack(log2) => write!(self.builder, "alignstack({})", 1 << log2),
            Attribute::FuncPure => self.builder.write_str("pure"),
            Attribute::ArgPtrTarget(target) => {
                let (name, ty) = match *target {
                    PtrArgTargetAttr::ByRef(ty) => ("byref", ty),
                    PtrArgTargetAttr::ByVal(ty) => ("byval", ty),
                    PtrArgTargetAttr::DynArray(ty) => ("elementtype", ty),
                };
                let tyname = self.type_get_name(ty);
                write!(self.builder, "{name}({tyname})")
            }
            Attribute::ArgPtrDerefBytes(nbytes) => {
                write!(self.builder, "dereferenceable({})", nbytes)
            }
        }
    }
    fn fmt_attrs(&mut self, attrs: &AttrSet) -> IRTreeBuildRes {
        for attr in attrs.iter() {
            self.fmt_attr(&attr)?;
            self.builder.write_str(" ")?;
        }
        Ok(())
    }
    pub fn do_fmt_func(&mut self, id: FuncID, func: &'ir FuncObj) -> IRTreeNodeBuildRes {
        let begin = self.builder.curr_pos();
        let mut children = self.fmt_func_header(id, func)?;
        if let Some(body) = &func.body {
            children.extend(self.fmt_func_body(body)?);
        }
        let node = IRTreeNode {
            parent: None,
            ir_obj: IRTreeObjID::Global(id.raw_into()),
            src_span: begin..self.builder.curr_pos(),
            children,
            depth: 0,
        };
        Ok(IRTreeNodeID::allocate(&mut self.builder.tree.alloc, node))
    }
    pub fn fmt_func_header(&mut self, id: FuncID, func: &'ir FuncObj) -> IRTreeChildren {
        let allocs = &self.module.allocs;

        // 写入函数定义前缀 + 链接属性组合字符串
        self.builder.write_str(func.get_linkage_prefix(allocs))?;

        // 写入函数属性
        self.builder.write_str(" ")?;
        self.fmt_attrs(&func.attrs())?;

        let ret_ty = self.type_get_name(func.ret_type);
        let name = func.get_name();
        write!(self.builder, "{ret_ty} @{name}(")?;

        let mut children = SmallVec::with_capacity(func.args.len());

        for arg in &func.args {
            if arg.index > 0 {
                self.builder.write_str(", ")?;
            }
            write!(self.builder, "{} ", &self.type_get_name(arg.ty))?;
            self.fmt_attrs(&func.attrs())?;
            if func.is_extern(allocs) {
                continue; // 外部函数的参数没有名字, 直接忽略子结点
            }

            // 开始记录参数结点
            let begin_pos = self.builder.curr_pos();
            let arg_id = ValueSSA::FuncArg(id, arg.index);
            write!(self.builder, "%{}", self.get_local_name(arg_id))?;
            let arg_node = IRTreeNodeID::new(
                &self.builder.tree.alloc,
                IRTreeObjID::FuncArg(id.raw_into(), arg.index),
                begin_pos..self.builder.curr_pos(),
            );
            children.push(arg_node);
        }
        // 处理可变参数
        if func.is_vararg {
            let prompt = if func.args.is_empty() { "..." } else { ", ..." };
            self.builder.write_str(prompt)?;
        }
        self.builder.write_str(")")?;
        Ok(children)
    }
    pub fn fmt_func_body(&mut self, body: &'ir FuncBody) -> IRTreeChildren {
        let allocs = &self.module.allocs;
        let FuncBody { blocks, entry } = body;

        // 根据 IR 规范, 有函数体的函数不是外部函数, 直接写入
        let mut children = SmallVec::with_capacity(blocks.len());

        self.builder.write_str(" {")?;
        // 格式化入口块
        children.push(self.do_fmt_block(*entry, entry.deref_ir(allocs))?);
        // 格式化其他基本块
        for (bb_id, bb_obj) in blocks.iter(&allocs.blocks) {
            if bb_id == *entry {
                continue;
            }
            let node = self.do_fmt_block(bb_id, bb_obj)?;
            children.push(node);
        }

        self.builder.write_str("}\n")?;
        Ok(children)
    }

    /// Syntax:
    ///
    /// ```llvm
    /// @global_name = [linkage] [type init_value | type], align <alignment>
    /// ```
    #[allow(dead_code)]
    pub fn fmt_global_var(&mut self, id: GlobalVarID, gvar: &'ir GlobalVar) -> IRTreeNodeBuildRes {
        let allocs = &self.module.allocs;
        let begin = self.builder.curr_pos();

        let name = id.get_name(allocs);
        let prefix = gvar.get_linkage_prefix(allocs);
        let content_ty = self.type_get_name(gvar.get_ptr_pointee_type());
        write!(self.builder, "@{name} = {prefix} ")?;

        if let Some(tls) = gvar.tls_model.get() {
            write!(self.builder, "thread_local({}) ", tls.get_ir_text())?;
        }
        write!(self.builder, "{content_ty} ")?;

        let mut children = SmallVec::new();
        if !gvar.is_extern(allocs) {
            let init_node = self.fmt_use(gvar.initval[0])?;
            children.push(init_node);
        }
        write!(self.builder, ", align {}", gvar.get_ptr_pointee_align())?;

        let node = IRTreeNode {
            parent: None,
            ir_obj: IRTreeObjID::Global(id.raw_into()),
            src_span: begin..self.builder.curr_pos(),
            children,
            depth: 0,
        };
        Ok(IRTreeNodeID::allocate(&mut self.builder.tree.alloc, node))
    }
}
