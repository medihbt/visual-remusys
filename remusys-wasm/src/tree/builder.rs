use std::{collections::HashMap, fmt::Write, ops::Range, rc::Rc, str};

use remusys_ir::{
    SymbolStr,
    ir::{inst::*, *},
    typing::*,
};
use smallvec::{SmallVec, smallvec};
use smol_str::{SmolStr, format_smolstr};
use wasm_bindgen::JsError;

use crate::{
    IRTree, IRTreeChildren, IRTreeNode, IRTreeNodeID, IRTreeObjID, SourcePosIndex, fmt_jserr,
    js_assert,
};

#[derive(Debug, Clone)]
struct TreeMapNode {
    id: IRTreeNodeID,
    src: Range<usize>,
}

pub struct IRTreeBuilder<'ir, 'name> {
    pub source_buf: String,
    pub indent: usize,
    pub curr_pos: SourcePosIndex,
    pub module: &'ir Module,
    pub names: &'name IRNameMap,
    pub tree: &'ir IRTree,
    pub curr_scope: Option<FuncID>,
    scopes: HashMap<FuncID, Rc<FuncNumberMap<'name>>>,
    types: HashMap<ValTypeID, SymbolStr>,
    pos_stack: Vec<SourcePosIndex>,
    tree_map: HashMap<UseID, TreeMapNode>,
    expr_str: HashMap<ExprID, Option<SmolStr>>,
}

impl<'ir, 'name> std::fmt::Write for IRTreeBuilder<'ir, 'name> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.source_buf.push_str(s);
        self.curr_pos = Self::bump_pos(self.curr_pos, s);
        Ok(())
    }
}

impl<'ir, 'name> IRTreeBuilder<'ir, 'name> {
    pub fn new(module: &'ir Module, names: &'name IRNameMap, tree: &'ir IRTree) -> Self {
        Self {
            source_buf: String::new(),
            tree_map: HashMap::new(),
            types: HashMap::new(),
            curr_pos: SourcePosIndex::zero(),
            indent: 0,
            module,
            names,
            pos_stack: vec![SourcePosIndex::zero()],
            tree,
            expr_str: HashMap::new(),
            scopes: HashMap::new(),
            curr_scope: None,
        }
    }

    pub fn begin_pos(&mut self) {
        self.pos_stack.push(self.curr_pos);
    }
    pub fn end_pos(&mut self) {
        self.pos_stack.pop();
    }
    pub fn relative_pos(&self) -> Result<SourcePosIndex, JsError> {
        let Some(last_pos) = self.pos_stack.last() else {
            return fmt_jserr!(Err "Internal error: pos_stack is empty");
        };
        last_pos.delta_to(self.curr_pos)
    }

    fn make_numbers(&mut self, func: FuncID) -> Result<Rc<FuncNumberMap<'name>>, JsError> {
        if let Some(num_map) = self.scopes.get(&func) {
            return Ok(num_map.clone());
        }
        let allocs = &self.module.allocs;
        if func.is_extern(allocs) {
            return fmt_jserr!(Err "External function {func:?} is not supported");
        }
        let num_map = FuncNumberMap::new(allocs, func, self.names, NumberOption::ignore_all());
        let num_map = Rc::new(num_map);
        self.scopes.insert(func, num_map.clone());
        Ok(num_map)
    }

    pub fn build(&mut self, obj: IRTreeObjID) -> Result<IRTreeNodeID, JsError> {
        let res = match obj {
            IRTreeObjID::Module => self.fmt_module(),
            IRTreeObjID::Global(globl) => self.fmt_global(globl),
            IRTreeObjID::FuncArg(func, idx) => {
                let allocs = &self.module.allocs;
                let Some(func) = FuncID::try_from_global(allocs, func) else {
                    return fmt_jserr!(Err "FunctionID {func:?} does not exist");
                };
                self.fmt_func_arg(func, idx)
            }
            IRTreeObjID::Block(bb) => self.fmt_block(bb),
            IRTreeObjID::Inst(inst) => self.fmt_inst(inst),
            IRTreeObjID::Use(u) => self.fmt_use(u),
            IRTreeObjID::JumpTarget(jt) => self.fmt_label(jt),
            IRTreeObjID::BlockIdent(bb) => self.fmt_block_label_line(bb),
            IRTreeObjID::FuncHeader(func) => {
                let allocs = &self.module.allocs;
                let Some(func) = FuncID::try_from_global(allocs, func) else {
                    return fmt_jserr!(Err "FunctionID {func:?} does not exist");
                };
                let func_obj = func.deref_ir(allocs);
                self.fmt_func_header(func, func_obj)
            }
        }?;
        js_assert!(
            self.pos_stack.len() == 1,
            "Internal error: pos_stack should have exactly one element at the end of build, but has {}",
            self.pos_stack.len()
        )?;
        Ok(res)
    }

    fn type_name(&mut self, ty: impl IValType) -> SymbolStr {
        let tctx = &self.module.tctx;
        let ty = ty.into_ir();
        match ty {
            ValTypeID::Void => SymbolStr::new_inline("void"),
            ValTypeID::Ptr => SymbolStr::new_inline("ptr"),
            ValTypeID::Int(_) | ValTypeID::Float(_) => ty.get_display_name(tctx),
            _ => self
                .types
                .entry(ty)
                .or_insert_with(|| ty.get_display_name(tctx))
                .clone(),
        }
    }
    fn fmt_type(&mut self, ty: impl IValType) -> Result<(), JsError> {
        let name = self.type_name(ty);
        self.write_str(&name)?;
        Ok(())
    }

    fn wrap_and_indent(&mut self) {
        self.source_buf.reserve(1 + self.indent * 4);
        let _ = self.write_char('\n');
        for _ in 0..self.indent {
            let _ = self.write_str("    ");
        }
    }

    fn write_indent(&mut self) -> Result<(), JsError> {
        for _ in 0..self.indent {
            self.write_str("    ")?;
        }
        Ok(())
    }

    fn writeln_indent(&mut self) -> Result<(), JsError> {
        self.write_str("\n")?;
        self.write_indent()
    }

    fn bump_pos(mut pos: SourcePosIndex, s: &str) -> SourcePosIndex {
        for c in s.chars() {
            if c == '\n' {
                pos.line += 1;
                pos.col_byte = 0;
            } else {
                pos.col_byte += c.len_utf8() as u32;
            }
        }
        pos
    }

    fn src_push_range(&mut self, src: Range<usize>) {
        self.source_buf.extend_from_within(src.clone());
        self.curr_pos = Self::bump_pos(self.curr_pos, &self.source_buf[src]);
    }

    fn fmt_module(&mut self) -> Result<IRTreeNodeID, JsError> {
        let module = self.module;
        let symbols = module.symbols.borrow();
        let begin = self.relative_pos()?;
        let mut children = IRTreeChildren::with_capacity(symbols.exported().len());
        let exported = symbols.exported().values();
        for (i, &gid) in exported.enumerate() {
            if i > 0 {
                self.wrap_and_indent();
            } else {
                self.write_indent()?;
            }
            let child = self.fmt_global(gid)?;
            children.push(child);
            self.write_char('\n')?;
        }
        let node = IRTreeNode::with_children(
            self.tree,
            IRTreeObjID::Module,
            begin..self.relative_pos()?,
            children,
        );
        let node_id = IRTreeNodeID::allocate(self.tree, node);
        Ok(node_id)
    }
    fn fmt_global(&mut self, globl: GlobalID) -> Result<IRTreeNodeID, JsError> {
        let allocs = &self.module.allocs;
        let Some(obj) = globl.try_deref_ir(allocs) else {
            return fmt_jserr!(Err "GlobalID {globl:?} does not exist");
        };
        match obj {
            GlobalObj::Var(gvar) => self.fmt_global_var(globl, gvar),
            GlobalObj::Func(func_obj) => {
                let Some(func_id) = FuncID::try_from_global(allocs, globl) else {
                    return fmt_jserr!(Err "GlobalID {globl:?} is not a function");
                };
                self.fmt_func(func_id, func_obj)
            }
        }
    }

    fn fmt_inst(&mut self, inst_id: InstID) -> Result<IRTreeNodeID, JsError> {
        let allocs = &self.module.allocs;
        let Some(inst) = inst_id.try_deref_ir(allocs) else {
            return fmt_jserr!(Err "Instruction {inst_id:?} does not exist");
        };
        match self.do_fmt_inst(inst_id, inst)? {
            Some(inst) => Ok(inst),
            None => fmt_jserr!(Err "Instruction {inst_id:?} is not supported"),
        }
    }

    fn fmt_block(&mut self, block: BlockID) -> Result<IRTreeNodeID, JsError> {
        let allocs = &self.module.allocs;
        let Some(obj) = block.try_deref_ir(allocs) else {
            return fmt_jserr!(Err "BlockID {block:?} does not exist");
        };
        self.do_fmt_block(block, obj)
    }

    fn fmt_block_label_line(&mut self, label: BlockID) -> Result<IRTreeNodeID, JsError> {
        let name = self.get_local_name(label)?;
        let begin_pos = self.relative_pos()?;
        write!(self, "{name}:")?;
        let end_pos = self.relative_pos()?;
        let node = IRTreeNode::with_children(
            self.tree,
            IRTreeObjID::BlockIdent(label),
            begin_pos..end_pos,
            IRTreeChildren::new(),
        );
        Ok(IRTreeNodeID::allocate(self.tree, node))
    }
    fn fmt_func_arg(&mut self, func: FuncID, arg_idx: u32) -> Result<IRTreeNodeID, JsError> {
        let allocs = &self.module.allocs;
        let Some(obj) = func.try_deref_ir(allocs) else {
            return fmt_jserr!(Err "FunctionID {func:?} does not exist");
        };
        if obj.is_extern(allocs) {
            return fmt_jserr!(Err "External function {func:?} is not supported");
        };
        let begin_pos = self.relative_pos()?;
        let arg_name = self.get_local_name(FuncArgID(func, arg_idx))?;
        write!(self, "%{arg_name}")?;
        let end_pos = self.relative_pos()?;
        let node = IRTreeNode::with_children(
            self.tree,
            IRTreeObjID::FuncArg(func.raw_into(), arg_idx),
            begin_pos..end_pos,
            IRTreeChildren::new(),
        );
        Ok(IRTreeNodeID::allocate(self.tree, node))
    }

    fn fmt_use(&mut self, use_id: UseID) -> Result<IRTreeNodeID, JsError> {
        let begin_pos = self.relative_pos()?;
        let begin_byte = self.source_buf.len();

        // 如果这个 use_id 已经在 tree_map 里了, 就直接复用它的 pos_delta.
        if let Some(node) = self.tree_map.get(&use_id) {
            let node = node.clone();
            let len = node.id.pos_delta_len(self.tree)?;
            let end_pos = begin_pos.advance(len);
            self.src_push_range(node.src.clone());
            let new_node = node.id.insert_pos_delta(self.tree, begin_pos..end_pos);
            return Ok(new_node.leak());
        }

        self.begin_pos();
        let children = (|| -> Result<IRTreeChildren, JsError> {
            let mut children = IRTreeChildren::new();
            let allocs = &self.module.allocs;
            match use_id.get_operand(allocs) {
                ValueSSA::None => self.write_str("none")?,
                ValueSSA::AggrZero(_) => self.write_str("zeroinitializer")?,
                ValueSSA::ConstData(data) => self.fmt_const_data(data)?,
                ValueSSA::ConstExpr(expr_id) => children = self.fmt_expr(expr_id)?,
                ValueSSA::FuncArg(func_id, arg_idx) => {
                    let name = self.get_local_name(FuncArgID(func_id, arg_idx))?;
                    write!(self, "%{name}")?;
                }
                ValueSSA::Block(block_id) => {
                    let name = self.get_local_name(ValueSSA::Block(block_id))?;
                    write!(self, "%{name}")?;
                }
                ValueSSA::Inst(inst_id) => {
                    let name = self.get_local_name(ValueSSA::Inst(inst_id))?;
                    write!(self, "%{name}")?;
                }
                ValueSSA::Global(global_id) => {
                    let name = global_id.clone_name(allocs);
                    write!(self, "@{name}")?;
                }
            }
            Ok(children)
        })();
        self.end_pos();
        let children = children?;
        let end_pos = self.relative_pos()?;
        let end_byte = self.source_buf.len();
        let node = IRTreeNode::with_children(
            self.tree,
            IRTreeObjID::Use(use_id),
            begin_pos..end_pos,
            children,
        );
        let node_id = IRTreeNodeID::allocate(self.tree, node);
        self.tree_map.insert(
            use_id,
            TreeMapNode {
                id: node_id,
                src: begin_byte..end_byte,
            },
        );
        Ok(node_id)
    }

    fn fmt_const_data(&mut self, data: ConstData) -> Result<(), JsError> {
        use remusys_ir::ir::ConstData::*;
        match data {
            Undef(_) => self.write_str("undef")?,
            PtrNull | Zero(ScalarType::Ptr) => self.write_str("null")?,
            Zero(ScalarType::Int(1)) => self.write_str("false")?,
            Zero(ScalarType::Int(_)) => self.write_str("0")?,
            Zero(ScalarType::Float(_)) => self.write_str("0.0")?,
            Int(apint) => write!(self, "{}", apint.as_signed())?,
            Float(_, fp) => write!(self, "{fp}")?,
        }
        Ok(())
    }

    fn fmt_array(&mut self, arr: &ArrayExpr) -> Result<IRTreeChildren, JsError> {
        self.fmt_aggr(
            ["[", "]"],
            arr.get_operands().as_slice(),
            AggrType::Array(arr.arrty),
        )
    }

    fn fmt_struct(&mut self, struc: &StructExpr) -> Result<IRTreeChildren, JsError> {
        let ops = struc.get_operands();
        let tctx = &self.module.tctx;
        let decorates = if struc.structty.is_packed(tctx) {
            ["<{", "}>"]
        } else {
            ["{", "}"]
        };
        self.fmt_aggr(decorates, ops.as_slice(), AggrType::Struct(struc.structty))
    }

    fn fmt_fixvec(&mut self, vec: &FixVec) -> Result<IRTreeChildren, JsError> {
        self.fmt_aggr(
            ["<", ">"],
            vec.get_operands().as_slice(),
            AggrType::FixVec(vec.vecty),
        )
    }

    fn fmt_aggr(
        &mut self,
        decorates: [&str; 2],
        ops: &[UseID],
        ty: AggrType,
    ) -> Result<IRTreeChildren, JsError> {
        let [begin_s, end_s] = decorates;
        let tctx = &self.module.tctx;
        self.write_str(begin_s)?;
        let mut children = IRTreeChildren::with_capacity(ops.len());
        for (i, &uid) in ops.iter().enumerate() {
            if i > 0 {
                self.write_str(", ")?;
            }
            let elem_ty = self.type_name(ty.get_field(tctx, i));
            write!(self, "{elem_ty} ")?;
            let child = self.fmt_use(uid)?;
            children.push(child);
        }
        self.write_str(end_s)?;
        Ok(children)
    }

    fn fmt_darray(&mut self, darr: &DataArrayExpr) -> Result<IRTreeChildren, JsError> {
        self.write_str("[ ")?;
        for i in 0..darr.data.len() {
            if i > 0 {
                self.write_str(", ")?;
            }
            let elem_ty = self.type_name(darr.elemty.into_ir());
            write!(self, "{} ", elem_ty)?;
            let value = darr.data.index_get_const(i);
            self.fmt_const_data(value)?;
        }
        self.write_str(" ]")?;
        Ok(IRTreeChildren::new())
    }

    fn fmt_sarray(&mut self, sarr: &SplatArrayExpr) -> Result<IRTreeChildren, JsError> {
        let mut children = IRTreeChildren::with_capacity(sarr.nelems);
        self.write_str("[ ")?;
        for i in 0..sarr.nelems {
            if i > 0 {
                self.write_str(", ")?;
            }
            let elem_ty = self.type_name(sarr.elemty.into_ir());
            write!(self, "{elem_ty} ")?;
            let child = self.fmt_use(sarr.element[0])?;
            children.push(child);
        }
        self.write_str(" ]")?;
        Ok(children)
    }

    fn fmt_kvarray(&mut self, kvarr: &KVArrayExpr) -> Result<IRTreeChildren, JsError> {
        let mut children = IRTreeChildren::new();
        let allocs = &self.module.allocs;
        let elem_ty = self.type_name(kvarr.elemty);
        self.write_str("sparse [ ")?;
        let mut first = true;

        for (idx, _, use_id) in kvarr.elem_iter(allocs) {
            if first {
                first = false;
            } else {
                self.write_str(", ")?;
            }
            write!(self, "[{idx}] = {elem_ty} ")?;
            let child = self.fmt_use(use_id)?;
            children.push(child);
        }

        if !first {
            self.write_str(", ")?;
        }
        write!(self, "..= {elem_ty} ")?;
        let child = self.fmt_use(kvarr.default_use())?;
        children.push(child);
        self.write_str(" ]")?;
        Ok(children)
    }

    fn fmt_expr(&mut self, expr_id: ExprID) -> Result<IRTreeChildren, JsError> {
        if let Some(litstr) = self.expr_as_string(expr_id) {
            self.write_str(&litstr)?;
            return Ok(IRTreeChildren::new());
        }
        let allocs = &self.module.allocs;
        match expr_id.deref_ir(allocs) {
            ExprObj::Array(arr) => self.fmt_array(arr),
            ExprObj::DataArray(darr) => self.fmt_darray(darr),
            ExprObj::SplatArray(sarr) => self.fmt_sarray(sarr),
            ExprObj::KVArray(kv) => self.fmt_kvarray(kv),
            ExprObj::Struct(st) => self.fmt_struct(st),
            ExprObj::FixVec(vec) => self.fmt_fixvec(vec),
        }
    }

    fn expr_as_string(&mut self, expr_id: ExprID) -> Option<SmolStr> {
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
            s.push(b'"');
            Some(SmolStr::from(unsafe {
                // SAFETY: the constructed byte sequence is valid UTF-8.
                str::from_utf8_unchecked(s.as_slice())
            }))
        }

        fn array_as_litstr(module: &Module, arr: &impl IArrayExpr) -> Option<SmolStr> {
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

        if let Some(s) = self.expr_str.get(&expr_id) {
            return s.clone();
        }

        let allocs = &self.module.allocs;
        let s = match expr_id.deref_ir(allocs) {
            ExprObj::Array(a) => array_as_litstr(self.module, a),
            ExprObj::DataArray(da) => darray_as_litstr(da),
            ExprObj::SplatArray(sa) => array_as_litstr(self.module, sa),
            ExprObj::KVArray(_) => None,
            ExprObj::Struct(_) | ExprObj::FixVec(_) => None,
        };
        self.expr_str.insert(expr_id, s.clone());
        s
    }

    fn fmt_global_var(&mut self, id: GlobalID, gvar: &GlobalVar) -> Result<IRTreeNodeID, JsError> {
        let allocs = &self.module.allocs;
        let begin_pos = self.relative_pos()?;

        self.begin_pos();
        let children = (|| -> Result<IRTreeChildren, JsError> {
            let name = id.clone_name(allocs);
            let prefix = gvar.get_linkage_prefix(allocs);
            let content_ty = self.type_name(gvar.get_ptr_pointee_type());
            write!(self, "@{name} = {prefix} ")?;

            if let Some(tls) = gvar.tls_model.get() {
                write!(self, "thread_local({}) ", tls.get_ir_text())?;
            }
            write!(self, "{content_ty} ")?;

            let mut children = IRTreeChildren::new();
            if !gvar.is_extern(allocs) {
                let init_node = self.fmt_use(gvar.initval[0])?;
                children.push(init_node);
            }
            write!(self, ", align {}", gvar.get_ptr_pointee_align())?;
            Ok(children)
        })();
        self.end_pos();
        let children = children?;

        let end_pos = self.relative_pos()?;
        let node = IRTreeNode::with_children(
            self.tree,
            IRTreeObjID::Global(id.raw_into()),
            begin_pos..end_pos,
            children,
        );
        Ok(IRTreeNodeID::allocate(self.tree, node))
    }

    fn fmt_attr(&mut self, attr: &Attribute) -> Result<(), JsError> {
        match attr {
            Attribute::NoUndef => self.write_str("noundef")?,
            Attribute::IntExt(iext) => self.write_str(iext.as_str())?,
            Attribute::PtrReadOnly => self.write_str("readonly")?,
            Attribute::PtrNoCapture => self.write_str("nocapture")?,
            Attribute::FuncNoReturn => self.write_str("noreturn")?,
            Attribute::FuncInline(inline) => self.write_str(inline.as_str())?,
            Attribute::FuncAlignStack(log2) => write!(self, "alignstack({})", 1 << log2)?,
            Attribute::FuncPure => self.write_str("pure")?,
            Attribute::ArgPtrTarget(target) => {
                let (name, ty) = match *target {
                    PtrArgTargetAttr::ByRef(ty) => ("byref", ty),
                    PtrArgTargetAttr::ByVal(ty) => ("byval", ty),
                    PtrArgTargetAttr::DynArray(ty) => ("elementtype", ty),
                };
                let tyname = self.type_name(ty);
                write!(self, "{name}({tyname})")?;
            }
            Attribute::ArgPtrDerefBytes(nbytes) => {
                write!(self, "dereferenceable({})", nbytes)?;
            }
        }
        Ok(())
    }

    fn fmt_attrs(&mut self, attrs: &AttrSet) -> Result<(), JsError> {
        for attr in attrs.iter() {
            self.fmt_attr(&attr)?;
            self.write_str(" ")?;
        }
        Ok(())
    }

    fn fmt_func_header(&mut self, id: FuncID, func: &FuncObj) -> Result<IRTreeNodeID, JsError> {
        // extern 函数参数没有局部名字; 非 extern 参数有名字并可建立 FuncArg 子结点。
        let allocs = &self.module.allocs;
        let begin_pos = self.relative_pos()?;

        let cap = if func.body.is_none() {
            0
        } else {
            func.args.len()
        };
        let mut children = IRTreeChildren::with_capacity(cap);
        self.begin_pos();

        write!(self, "{} ", func.get_linkage_prefix(allocs))?;
        self.fmt_attrs(&func.attrs())?;
        let ret_ty = self.type_name(func.ret_type);
        let name = id.clone_name(allocs);
        write!(self, "{ret_ty} @{name}(")?;

        for arg in &func.args {
            if arg.index > 0 {
                self.write_str(", ")?;
            }
            self.fmt_type(arg.ty)?;
            self.fmt_attrs(&func.attrs())?;

            if func.is_extern(allocs) {
                continue;
            }

            let arg_begin = self.relative_pos()?;
            let arg_name = self.get_local_name(FuncArgID(id, arg.index))?;
            write!(self, " %{arg_name}")?;
            let arg_end = self.relative_pos()?;
            let arg_node = IRTreeNode::with_children(
                self.tree,
                IRTreeObjID::FuncArg(id.raw_into(), arg.index),
                arg_begin..arg_end,
                IRTreeChildren::new(),
            );
            children.push(IRTreeNodeID::allocate(self.tree, arg_node));
        }
        if func.is_vararg {
            let prompt = if func.args.is_empty() { "..." } else { ", ..." };
            self.write_str(prompt)?;
        }
        self.write_str(")")?;

        self.end_pos();
        let end_pos = self.relative_pos()?;
        let node = IRTreeNode::with_children(
            self.tree,
            IRTreeObjID::FuncHeader(id.raw_into()),
            begin_pos..end_pos,
            children,
        );
        Ok(IRTreeNodeID::allocate(self.tree, node))
    }

    fn fmt_func(&mut self, id: FuncID, func: &FuncObj) -> Result<IRTreeNodeID, JsError> {
        let begin_pos = self.relative_pos()?;
        self.begin_pos();
        let scope = self.curr_scope.replace(id);
        let header_node = self.fmt_func_header(id, func)?;
        let children = match &func.body {
            None => smallvec![header_node],
            Some(body) => self.fmt_func_body(header_node, body)?,
        };
        self.curr_scope = scope;
        self.end_pos();
        let end_pos = self.relative_pos()?;
        let node = IRTreeNode::with_children(
            self.tree,
            IRTreeObjID::Global(id.raw_into()),
            begin_pos..end_pos,
            children,
        );
        Ok(IRTreeNodeID::allocate(self.tree, node))
    }
    fn fmt_func_body(
        &mut self,
        header: IRTreeNodeID,
        body: &FuncBody,
    ) -> Result<IRTreeChildren, JsError> {
        let allocs = &self.module.allocs;
        let mut children = IRTreeChildren::with_capacity(1 + body.blocks.len());
        children.push(header);
        self.write_str("{")?;
        self.wrap_and_indent();
        for (bb_id, bb) in body.blocks.iter(&allocs.blocks) {
            let bb_node = self.do_fmt_block(bb_id, bb)?;
            children.push(bb_node);
        }
        self.write_str("}")?;
        Ok(children)
    }
}

impl<'ir, 'name> IRTreeBuilder<'ir, 'name> {
    fn get_local_name(&mut self, val: impl IValueConvert) -> Result<SmolStr, JsError> {
        if let Some(name) = self.try_local_name(val)? {
            return Ok(name);
        }
        let name = match val.into_value() {
            ValueSSA::Block(bb) => bb.to_strid(),
            ValueSSA::Inst(inst) => inst.to_strid(),
            ValueSSA::FuncArg(func, arg) => {
                format_smolstr!("FuncArg({},{arg})", func.raw_into().to_strid())
            }
            val => format_smolstr!("<unnamed {val:?}>"),
        };
        Ok(name)
    }

    fn try_local_name(&mut self, val: impl IValueConvert) -> Result<Option<SmolStr>, JsError> {
        let value = val.into_value();
        if let Some(local_name) = self.names.get_local_name(value) {
            return Ok(Some(local_name));
        }
        let scope = match self.curr_scope {
            Some(func) => func,
            None => return Ok(None),
        };
        let num_map = self.make_numbers(scope)?;
        Ok(num_map.get_local_name(value))
    }

    fn fmt_label(&mut self, jt: JumpTargetID) -> Result<IRTreeNodeID, JsError> {
        let begin_pos = self.relative_pos()?;
        let allocs = &self.module.allocs;
        let name = match jt.get_block(allocs) {
            Some(block) => self.get_local_name(ValueSSA::Block(block))?,
            None => SmolStr::new_inline("none"),
        };
        write!(self, "%{name}")?;
        let node = IRTreeNode::with_children(
            self.tree,
            IRTreeObjID::JumpTarget(jt),
            begin_pos..self.relative_pos()?,
            IRTreeChildren::new(),
        );
        Ok(IRTreeNodeID::allocate(self.tree, node))
    }

    fn do_fmt_inst(
        &mut self,
        inst_id: InstID,
        inst: &InstObj,
    ) -> Result<Option<IRTreeNodeID>, JsError> {
        let allocs = &self.module.allocs;
        let begin_pos = self.relative_pos()?;
        self.begin_pos();

        if let Some(name) = self.try_local_name(inst_id)? {
            write!(self, "%{name} = ")?;
        }
        let mut should_emit_node = true;
        let children = match inst {
            InstObj::GuideNode(_) => {
                should_emit_node = false;
                smallvec![]
            }
            InstObj::PhiInstEnd(_) => {
                self.write_str("; ==== Phi Section End ====")?;
                should_emit_node = false;
                smallvec![]
            }
            InstObj::Unreachable(_) => {
                self.write_str("unreachable")?;
                smallvec![]
            }
            InstObj::Ret(ret_inst) => self.fmt_ret_inst(ret_inst)?,
            InstObj::Jump(jump_inst) => {
                self.write_str("br label ")?;
                let label_node = self.fmt_label(jump_inst.target_jt())?;
                smallvec![label_node]
            }
            InstObj::Br(br_inst) => {
                self.write_str("br i1 ")?;
                let cond_node = self.fmt_use(br_inst.cond_use())?;
                self.write_str(", label ")?;
                let then_node = self.fmt_label(br_inst.then_jt())?;
                self.write_str(", label ")?;
                let else_node = self.fmt_label(br_inst.else_jt())?;
                smallvec![cond_node, then_node, else_node]
            }
            InstObj::Switch(switch_inst) => self.fmt_switch_inst(allocs, switch_inst)?,
            InstObj::Alloca(alloca_inst) => {
                self.write_str("alloca ")?;
                let ty_name = self.type_name(alloca_inst.pointee_ty);
                let align = alloca_inst.get_ptr_pointee_align();
                write!(self, "{ty_name} , align {align}")?;
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
                let from_ty_name = self.type_name(cast_inst.from_ty);
                write!(self, "{opcode} {from_ty_name} ")?;
                let from_node = self.fmt_use(cast_inst.from_use())?;
                let to_ty_name = self.type_name(cast_inst.get_valtype());
                write!(self, " to {to_ty_name}")?;
                smallvec![from_node]
            }
            InstObj::Cmp(cmp_inst) => {
                let opcode = cmp_inst.get_opcode().get_name();
                let cond = cmp_inst.cond;
                let operand_ty_name = self.type_name(cmp_inst.operand_ty);
                write!(self, "{opcode} {cond} {operand_ty_name} ")?;
                let lhs_node = self.fmt_use(cmp_inst.lhs_use())?;
                self.write_str(", ")?;
                let rhs_node = self.fmt_use(cmp_inst.rhs_use())?;
                smallvec![lhs_node, rhs_node]
            }
            InstObj::IndexExtract(index_extract_inst) => {
                let aggr_ty_name = self.type_name(index_extract_inst.aggr_type.into_ir());
                let index = index_extract_inst.get_index(allocs);
                let index_ty_name = self.type_name(index.get_valtype(allocs));

                write!(self, "extractelement {aggr_ty_name} ")?;
                let aggr_node = self.fmt_use(index_extract_inst.aggr_use())?;
                write!(self, ", {index_ty_name} ")?;
                let index_node = self.fmt_use(index_extract_inst.index_use())?;
                smallvec![aggr_node, index_node]
            }
            InstObj::FieldExtract(field_extract_inst) => {
                let aggr_ty_name = self.type_name(field_extract_inst.aggr_type.into_ir());
                write!(self, "extractvalue {aggr_ty_name} ")?;
                let aggr_node = self.fmt_use(field_extract_inst.aggr_use())?;
                for &idx in field_extract_inst.get_field_indices() {
                    write!(self, ", {idx}")?;
                }
                smallvec![aggr_node]
            }
            InstObj::IndexInsert(index_insert_inst) => {
                let aggr_ty_name = self.type_name(index_insert_inst.get_valtype());
                let elem_ty_name = self.type_name(index_insert_inst.get_elem_type());
                let index = index_insert_inst.get_index(allocs);
                let index_ty_name = self.type_name(index.get_valtype(allocs));

                write!(self, "insertelement {aggr_ty_name} ")?;
                let aggr_node = self.fmt_use(index_insert_inst.aggr_use())?;
                write!(self, ", {elem_ty_name} ")?;
                let elem_node = self.fmt_use(index_insert_inst.elem_use())?;
                write!(self, ", {index_ty_name} ")?;
                let index_node = self.fmt_use(index_insert_inst.index_use())?;
                smallvec![aggr_node, elem_node, index_node]
            }
            InstObj::FieldInsert(field_insert_inst) => {
                let aggr_ty_name = self.type_name(field_insert_inst.get_valtype());
                let elem_ty_name = self.type_name(field_insert_inst.get_elem_type());

                write!(self, "insertvalue {aggr_ty_name} ")?;
                let aggr_node = self.fmt_use(field_insert_inst.aggr_use())?;
                write!(self, ", {elem_ty_name} ")?;
                let elem_node = self.fmt_use(field_insert_inst.elem_use())?;
                for &idx in field_insert_inst.get_field_indices() {
                    write!(self, ", {idx}")?;
                }
                smallvec![aggr_node, elem_node]
            }
            InstObj::Phi(phi_inst) => self.fmt_phi_inst(phi_inst)?,
            InstObj::Select(select_inst) => self.fmt_select_inst(select_inst)?,
        };
        self.end_pos();
        let end_pos = self.relative_pos()?;

        if !should_emit_node {
            return Ok(None);
        }

        let node = IRTreeNode::with_children(
            self.tree,
            IRTreeObjID::Inst(inst_id),
            begin_pos..end_pos,
            children,
        );
        let node_id = IRTreeNodeID::allocate(self.tree, node);
        Ok(Some(node_id))
    }

    fn fmt_select_inst(&mut self, select_inst: &SelectInst) -> Result<IRTreeChildren, JsError> {
        let ty_name = self.type_name(select_inst.get_valtype());
        write!(self, "select {ty_name}, i1 ")?;
        let cond_node = self.fmt_use(select_inst.cond_use())?;
        self.write_str(", ")?;
        let then_node = self.fmt_use(select_inst.then_use())?;
        self.write_str(", ")?;
        let else_node = self.fmt_use(select_inst.else_use())?;
        let mut children = IRTreeChildren::with_capacity(3);
        children.push(cond_node);
        children.push(then_node);
        children.push(else_node);
        Ok(children)
    }

    fn fmt_phi_inst(&mut self, phi_inst: &PhiInst) -> Result<IRTreeChildren, JsError> {
        let ty_name = self.type_name(phi_inst.get_valtype());
        write!(self, "phi {ty_name} ")?;
        let mut children = IRTreeChildren::with_capacity(phi_inst.incoming_uses().len() * 2);
        let mut first = true;
        for [uval, ublk] in phi_inst.incoming_uses().iter() {
            self.write_str(if first { " [" } else { ", [" })?;
            first = false;

            let val_node = self.fmt_use(*uval)?;
            self.write_str(", label ")?;
            let blk_node = self.fmt_use(*ublk)?;
            self.write_str("]")?;
            children.push(val_node);
            children.push(blk_node);
        }
        Ok(children)
    }

    fn fmt_call_inst(
        &mut self,
        allocs: &IRAllocs,
        inst: &CallInst,
    ) -> Result<IRTreeChildren, JsError> {
        let ret_ty_name = self.type_name(inst.get_valtype());
        if inst.is_vararg {
            write!(self, "call {ret_ty_name} (...) ")?;
        } else {
            write!(self, "call {ret_ty_name} ")?;
        }
        let callee_node = self.fmt_use(inst.callee_use())?;
        self.write_str("(")?;
        let mut children = IRTreeChildren::with_capacity(inst.operands.len());
        children.push(callee_node);
        let tctx = &self.module.tctx;
        for (i, &arg_use) in inst.arg_uses().iter().enumerate() {
            if i > 0 {
                self.write_str(", ")?;
            }
            let arg_ty = match inst.callee_ty.get_args(tctx).get(i) {
                Some(arg) => *arg,
                None => arg_use.get_operand(allocs).get_valtype(allocs),
            };
            let arg_ty_name = self.type_name(arg_ty);
            write!(self, "{arg_ty_name} ")?;
            let arg_node = self.fmt_use(arg_use)?;
            children.push(arg_node);
        }
        self.write_str(")")?;
        Ok(children)
    }

    fn fmt_binop_inst(&mut self, inst: &BinOPInst) -> Result<IRTreeChildren, JsError> {
        let opcode = inst.get_opcode().get_name();
        let flags = inst.get_flags();
        let ty_name = self.type_name(inst.get_valtype());
        if flags.is_empty() {
            write!(self, "{opcode} {ty_name} ")?;
        } else {
            write!(self, "{opcode} {flags} {ty_name} ")?;
        }
        let lhs_node = self.fmt_use(inst.lhs_use())?;
        self.write_str(", ")?;
        let rhs_node = self.fmt_use(inst.rhs_use())?;
        let mut children = IRTreeChildren::with_capacity(2);
        children.push(lhs_node);
        children.push(rhs_node);
        Ok(children)
    }

    fn fmt_amormw_inst(&mut self, inst: &AmoRmwInst) -> Result<IRTreeChildren, JsError> {
        let subop_name = inst.subop_name();
        if inst.is_volatile {
            write!(self, "atomicrmw volatile {subop_name} ptr ")?;
        } else {
            write!(self, "atomicrmw {subop_name} ptr ")?;
        }
        let pointer_node = self.fmt_use(inst.pointer_use())?;
        let value_ty_name = self.type_name(inst.value_ty);
        write!(self, ", {value_ty_name} ")?;
        let value_node = self.fmt_use(inst.value_use())?;
        if inst.scope != remusys_ir::ir::SyncScope::System {
            write!(self, " syncscope(\"{}\")", inst.scope.as_str())?;
        }
        write!(self, " {}", inst.ordering.as_str())?;
        if inst.align_log2 > 0 {
            write!(self, ", align {}", 1 << inst.align_log2)?;
        }
        let mut children = IRTreeChildren::with_capacity(2);
        children.push(pointer_node);
        children.push(value_node);
        Ok(children)
    }

    fn fmt_store_inst(&mut self, inst: &StoreInst) -> Result<IRTreeChildren, JsError> {
        let source_ty_name = self.type_name(inst.source_ty);
        write!(self, "store {source_ty_name} ")?;
        let source_node = self.fmt_use(inst.source_use())?;
        self.write_str(", ptr ")?;
        let target_node = self.fmt_use(inst.target_use())?;
        write!(self, ", align {}", inst.get_operand_pointee_align())?;
        let mut children = IRTreeChildren::with_capacity(2);
        children.push(source_node);
        children.push(target_node);
        Ok(children)
    }

    fn fmt_load_inst(&mut self, inst: &LoadInst) -> Result<IRTreeChildren, JsError> {
        let pointee_ty_name = self.type_name(inst.get_valtype());
        write!(self, "load {pointee_ty_name}, ptr ")?;
        let source_node = self.fmt_use(inst.source_use())?;
        write!(self, ", align {}", inst.get_operand_pointee_align())?;
        let mut children = IRTreeChildren::with_capacity(1);
        children.push(source_node);
        Ok(children)
    }

    fn fmt_gep_inst(
        &mut self,
        allocs: &IRAllocs,
        inst: &GEPInst,
    ) -> Result<IRTreeChildren, JsError> {
        if inst.get_inbounds() {
            self.write_str("getelementptr inbounds ")?;
        } else {
            self.write_str("getelementptr ")?;
        }
        let initial_ty_name = self.type_name(inst.initial_ty);
        write!(self, "{initial_ty_name}, ptr ")?;
        let base_node = self.fmt_use(inst.base_use())?;
        let mut children = IRTreeChildren::with_capacity(inst.get_operands().len());
        children.push(base_node);
        for &index_use in inst.index_uses() {
            let index_ty_name = self.type_name(index_use.get_operand(allocs).get_valtype(allocs));
            self.write_str(", ")?;
            write!(self, "{index_ty_name} ")?;
            let index_node = self.fmt_use(index_use)?;
            children.push(index_node);
        }
        Ok(children)
    }

    fn fmt_switch_inst(
        &mut self,
        allocs: &IRAllocs,
        inst: &SwitchInst,
    ) -> Result<IRTreeChildren, JsError> {
        let cond_ty = self.type_name(inst.discrim_ty);
        write!(self, "switch {cond_ty} ")?;
        let discrim_node = self.fmt_use(inst.discrim_use())?;
        self.write_str(", label ")?;
        let default_node = self.fmt_label(inst.default_jt())?;
        let mut children = IRTreeChildren::with_capacity(inst.n_jump_targets() + 1);
        children.push(discrim_node);
        children.push(default_node);
        if inst.case_jts().is_empty() {
            self.write_str(" []")?;
        } else {
            self.write_str(" [")?;
            self.indent += 1;
            for (case_jt, case_val, _) in inst.cases_iter(allocs) {
                self.writeln_indent()?;
                write!(self, "{cond_ty} {case_val}, label ")?;
                let case_node = self.fmt_label(case_jt)?;
                children.push(case_node);
            }
            self.indent = self.indent.saturating_sub(1);
            self.writeln_indent()?;
            self.write_str(" ]")?;
        }
        Ok(children)
    }

    fn fmt_ret_inst(&mut self, ret_inst: &RetInst) -> Result<IRTreeChildren, JsError> {
        if ret_inst.get_valtype() == ValTypeID::Void {
            self.write_str("ret void")?;
            Ok(IRTreeChildren::new())
        } else {
            self.write_str("ret ")?;
            let ty_name = self.type_name(ret_inst.get_valtype());
            write!(self, "{ty_name} ")?;
            let use_node = self.fmt_use(ret_inst.retval_use())?;
            Ok(smallvec![use_node])
        }
    }
}

impl<'ir, 'name> IRTreeBuilder<'ir, 'name> {
    fn do_fmt_block(
        &mut self,
        block_id: BlockID,
        block: &BlockObj,
    ) -> Result<IRTreeNodeID, JsError> {
        let begin_pos = self.relative_pos()?;
        let mut children: SmallVec<[IRTreeNodeID; 4]> =
            IRTreeChildren::with_capacity(1 + block.get_insts().len());

        self.begin_pos();

        let label_node = self.fmt_block_label_line(block_id)?;
        children.push(label_node);

        let allocs = &self.module.allocs;
        self.indent += 1;
        for (inst_id, inst) in block.insts_iter(allocs) {
            self.wrap_and_indent();
            if let Some(inst_node) = self.do_fmt_inst(inst_id, inst)? {
                children.push(inst_node)
            }
        }
        self.indent -= 1;
        self.wrap_and_indent();
        self.end_pos();

        let end_pos = self.relative_pos()?;
        let node = IRTreeNode::with_children(
            self.tree,
            IRTreeObjID::Block(block_id),
            begin_pos..end_pos,
            children,
        );
        let node_id = IRTreeNodeID::allocate(self.tree, node);
        Ok(node_id)
    }
}
