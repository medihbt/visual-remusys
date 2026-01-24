use remusys_ir::ir::ArrayBuildErr;
use smol_str::SmolStr;

pub use self::type_mapping::*;
mod type_mapping {
    use crate::{
        ast::{ModuleAst, TypeAliasItem, TypeAst, TypeAstKind},
        sema::{SemaErr, SemaRes},
    };
    use remusys_ir::{
        base::INullableValue,
        ir::Module,
        typing::{
            ArrayTypeID, ArrayTypeObj, FixVecType, FuncTypeID, ScalarType, StructAliasID,
            StructTypeID, TypeContext, ValTypeID,
        },
    };
    use smallvec::SmallVec;
    use smol_str::SmolStr;
    use std::{
        collections::{HashMap, HashSet},
        hash::Hash,
        sync::Arc,
    };

    /// Sorts type aliases in a module based on their dependencies.
    ///
    /// A Remusys-IR type object cannot be modified once it has been created. Therefore,
    /// type aliases must be defined before they are used. This struct provides functionality
    /// to sort type aliases in a module to ensure that all dependencies are resolved in the
    /// correct order.
    struct TypeAliasSort<'ast> {
        pub types: &'ast [Arc<TypeAliasItem>],
        pub index_map: HashMap<SmolStr, usize>,
    }

    impl<'ast> TypeAliasSort<'ast> {
        pub fn new(module: &'ast ModuleAst) -> SemaRes<Self> {
            let types = module.type_aliases.as_slice();
            let index_map = {
                let mut index_map = HashMap::with_capacity(types.len());
                for (index, tyalias) in types.iter().enumerate() {
                    if index_map.insert(tyalias.name.clone(), index).is_some() {
                        return Err(SemaErr::RedefinedTypeAlias(tyalias.name.clone()));
                    }
                }
                index_map
            };
            Ok(Self { types, index_map })
        }

        /// Find a order to define type aliases such that all dependencies are resolved.
        pub fn sort(&self) -> SemaRes<Vec<Arc<TypeAliasItem>>> {
            let mut nodes: Vec<AliasNode> = Vec::with_capacity(self.types.len());
            for i in 0..self.types.len() {
                nodes.push(AliasNode::new(self, i)?);
            }
            for i in 0..nodes.len() {
                let deps = std::mem::take(&mut nodes[i].dependencies);
                for &dep in &deps {
                    nodes[dep].users.push(i);
                }
                nodes[i].dependencies = deps;
            }
            let mut sorted = Vec::with_capacity(self.types.len());
            let mut no_deps: SmallVec<[usize; 8]> = SmallVec::new();
            for (i, n) in nodes.iter().enumerate() {
                if n.dependencies.is_empty() {
                    no_deps.push(i);
                }
            }
            while let Some(idx) = no_deps.pop() {
                sorted.push(Arc::clone(&self.types[idx]));
                for user in std::mem::take(&mut nodes[idx].users) {
                    let user_node = &mut nodes[user];
                    user_node.dependencies.remove(&idx);
                    if user_node.dependencies.is_empty() {
                        no_deps.push(user);
                    }
                }
            }
            if sorted.len() != self.types.len() {
                // There is a cycle in the type aliases.
                for node in nodes.iter() {
                    if !node.dependencies.is_empty() {
                        return Err(SemaErr::TypeCycled);
                    }
                }
            }
            Ok(sorted)
        }
    }

    struct AliasNode {
        dependencies: HashSet<usize>,
        users: SmallVec<[usize; 5]>,
    }
    impl AliasNode {
        pub fn new(sort: &TypeAliasSort<'_>, index: usize) -> SemaRes<Self> {
            let alias = Arc::clone(&sort.types[index]);
            let deps = Self::collect_deps(&alias.ty, &sort.index_map)?;
            if deps.contains(&index) {
                return Err(SemaErr::TypeSelfDepend(alias.name.clone()));
            }
            Ok(Self {
                dependencies: deps,
                users: SmallVec::new(),
            })
        }

        fn collect_deps(
            ty: &TypeAst,
            index_map: &HashMap<SmolStr, usize>,
        ) -> SemaRes<HashSet<usize>> {
            let mut deps = HashSet::new();
            Self::_do_collect_deps(ty, &mut deps);
            let mut out = HashSet::new();
            for name in deps.into_iter() {
                match index_map.get(&name) {
                    Some(&idx) => out.insert(idx),
                    None => return Err(SemaErr::TypeAliasNotFound(name)),
                };
            }
            Ok(out)
        }
        fn _do_collect_deps(ty: &TypeAst, deps: &mut HashSet<SmolStr>) {
            use crate::ast::TypeAstKind as Ty;
            match &ty.kind {
                Ty::Void | Ty::Ptr | Ty::Int(_) | Ty::FP(_) => {}
                Ty::Array { elem, .. } | Ty::Vec { elem, .. } => {
                    Self::_do_collect_deps(elem, deps);
                }
                Ty::Struct { elem, .. } => {
                    for e in elem {
                        Self::_do_collect_deps(e, deps);
                    }
                }
                Ty::Alias(ident) => {
                    deps.insert(ident.name.clone());
                }
            }
        }
    }

    #[derive(Debug)]
    struct HashArr(ArrayTypeObj);
    impl PartialEq for HashArr {
        fn eq(&self, other: &Self) -> bool {
            let Self(ArrayTypeObj { elemty, nelems, .. }) = self;
            let Self(ArrayTypeObj {
                elemty: o_elemty,
                nelems: o_nelems,
                ..
            }) = other;
            elemty == o_elemty && nelems == o_nelems
        }
    }
    impl Eq for HashArr {}
    impl Hash for HashArr {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            let Self(ArrayTypeObj { elemty, nelems, .. }) = self;
            elemty.hash(state);
            nelems.hash(state);
        }
    }
    impl HashArr {
        fn to_type(&self, tctx: &TypeContext) -> ArrayTypeID {
            let Self(arr) = self;
            unsafe { ArrayTypeID::new_nodedup(tctx, arr.elemty, arr.nelems) }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct HashStruct {
        packed: bool,
        fields: SmallVec<[ValTypeID; 8]>,
    }
    impl HashStruct {
        fn to_type(&self, tctx: &TypeContext) -> StructTypeID {
            let Self { packed, fields } = self.clone();
            unsafe { StructTypeID::new_nodedup(tctx, packed, fields) }
        }
    }

    #[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
    struct HashFunc {
        ret_ty: ValTypeID,
        param_tys: SmallVec<[ValTypeID; 8]>,
        is_vararg: bool,
    }
    impl HashFunc {
        fn to_type(&self, tctx: &TypeContext) -> FuncTypeID {
            let Self {
                ret_ty,
                param_tys,
                is_vararg,
            } = self;
            unsafe { FuncTypeID::new_nodedup(tctx, *ret_ty, *is_vararg, param_tys) }
        }
    }

    #[derive(Debug, Default)]
    pub struct TypeMap {
        alias_map: HashMap<SmolStr, StructAliasID>,
        array_map: HashMap<HashArr, ArrayTypeID>,
        struc_map: HashMap<HashStruct, StructTypeID>,
        func_map: HashMap<HashFunc, FuncTypeID>,
    }

    impl TypeMap {
        pub fn new(module: &Module, ast: &ModuleAst) -> SemaRes<Self> {
            let sorted = TypeAliasSort::new(ast)?.sort()?;
            let mut ret = Self::default();
            for alias in sorted {
                let tctx = &module.tctx;
                let TypeAliasItem { name, ty, .. } = alias.as_ref();
                let mapped = ret.map_type(tctx, ty)?;
                let ValTypeID::Struct(struc) = mapped else {
                    return Err(SemaErr::TypeAliasNotMappedToStruct(name.clone()));
                };
                let alias_id = StructAliasID::new(tctx, name.to_string(), struc);
                ret.alias_map.insert(name.clone(), alias_id);
            }
            Ok(ret)
        }

        pub fn map_type(&mut self, tctx: &TypeContext, ty: &TypeAst) -> SemaRes<ValTypeID> {
            let ty = match &ty.kind {
                TypeAstKind::Void => ValTypeID::Void,
                TypeAstKind::Ptr => ValTypeID::Ptr,
                TypeAstKind::Int(i) => ValTypeID::Int(*i),
                TypeAstKind::FP(fpkind) => ValTypeID::Float(*fpkind),
                TypeAstKind::Array { elem, len } => {
                    let elemty = self.map_type(tctx, elem)?;
                    let hash_arr = HashArr(ArrayTypeObj::new(elemty, *len));
                    if let Some(arr_id) = self.array_map.get(&hash_arr) {
                        ValTypeID::Array(*arr_id)
                    } else {
                        let arr_id = hash_arr.to_type(tctx);
                        self.array_map.insert(hash_arr, arr_id);
                        ValTypeID::Array(arr_id)
                    }
                }
                TypeAstKind::Vec { elem, len } => {
                    let elemty = match &elem.kind {
                        TypeAstKind::Ptr => ScalarType::Ptr,
                        TypeAstKind::FP(fp) => ScalarType::Float(*fp),
                        TypeAstKind::Int(i) => ScalarType::Int(*i),
                        kind => {
                            return Err(SemaErr::VectorElemTypeNotPrimtive(kind.get_name()));
                        }
                    };
                    if !len.is_power_of_two() {
                        return Err(SemaErr::VectorLengthNotPowerOfTwo(*len));
                    }
                    ValTypeID::FixVec(FixVecType(elemty, len.trailing_zeros() as u8))
                }
                TypeAstKind::Alias(ident) => {
                    let name = &ident.name;
                    match self.alias_map.get(name) {
                        Some(alias_id) => ValTypeID::StructAlias(*alias_id),
                        None => return Err(SemaErr::TypeAliasNotFound(name.clone())),
                    }
                }
                TypeAstKind::Struct { elem, packed } => {
                    let mut fields: SmallVec<[ValTypeID; 8]> = SmallVec::with_capacity(elem.len());
                    for e in elem {
                        let field_ty = self.map_type(tctx, e)?;
                        fields.push(field_ty);
                    }
                    let hash_struc = HashStruct {
                        packed: *packed,
                        fields,
                    };
                    if let Some(struc_id) = self.struc_map.get(&hash_struc) {
                        ValTypeID::Struct(*struc_id)
                    } else {
                        let struc_id = hash_struc.to_type(tctx);
                        self.struc_map.insert(hash_struc, struc_id);
                        ValTypeID::Struct(struc_id)
                    }
                }
            };
            Ok(ty)
        }
    }

    pub struct FuncTypeBuilder<'ir> {
        func: HashFunc,
        func_ty: FuncTypeID,
        tctx: &'ir TypeContext,
        tmap: &'ir mut TypeMap,
    }
    impl<'ir> FuncTypeBuilder<'ir> {
        pub fn new(tmap: &'ir mut TypeMap, tctx: &'ir TypeContext) -> Self {
            Self {
                func: HashFunc::default(),
                func_ty: FuncTypeID::new_null(),
                tctx,
                tmap,
            }
        }
        pub fn return_type(&mut self, ty: &TypeAst) -> SemaRes<&mut Self> {
            let ret_ty = self.tmap.map_type(self.tctx, ty)?;
            self.func.ret_ty = ret_ty;
            Ok(self)
        }
        pub fn add_argtype(&mut self, ty: &TypeAst) -> SemaRes<&mut Self> {
            let arg_ty = self.tmap.map_type(self.tctx, ty)?;
            self.func.param_tys.push(arg_ty);
            Ok(self)
        }
        pub fn is_vararg(&mut self, val: bool) -> &mut Self {
            self.func.is_vararg = val;
            self
        }

        pub fn finish(&mut self) -> FuncTypeID {
            if self.func_ty.is_nonnull() {
                return self.func_ty;
            }
            let func_ty = self.do_build();
            assert!(
                func_ty.is_nonnull(),
                "FuncTypeBuilder: failed to build function type"
            );
            self.func_ty = func_ty;
            func_ty
        }
        fn do_build(&mut self) -> FuncTypeID {
            if let Some(func_id) = self.tmap.func_map.get(&self.func) {
                *func_id
            } else {
                let func_id = self.func.to_type(self.tctx);
                self.tmap.func_map.insert(self.func.clone(), func_id);
                func_id
            }
        }
    }
}

pub use value_mapping::*;
mod value_mapping {
    use remusys_ir::{
        ir::{
            ArrayExprID, ConstArrayData, DataArrayExpr, DataArrayExprID, FixVecID, GlobalID,
            ISubExprID, ISubValueSSA, IValueConvert, KVArrayBuilder, Module, StructExprID,
            ValueSSA,
        },
        typing::{ArrayTypeID, FixVecType, IValType, StructTypeID, ValTypeID},
    };
    use smallvec::SmallVec;
    use smol_str::SmolStr;
    use std::{collections::HashMap, sync::Arc};

    use crate::{
        ast::{AggrKind, Ident, IdentKind},
        sema::{SemaErr, SemaRes},
    };

    pub type ValueList = SmallVec<[ValueSSA; 8]>;

    #[derive(Debug, Default)]
    pub struct SymbolMap {
        globals: HashMap<SmolStr, GlobalID>,
        locals: HashMap<SmolStr, ValueSSA>,
    }
    impl SymbolMap {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn insert(&mut self, name: SmolStr, value: impl IValueConvert) {
            let repeats = match value.into_value() {
                ValueSSA::Global(glob) => self.globals.insert(name.clone(), glob).is_some(),
                val => self.locals.insert(name.clone(), val).is_some(),
            };
            if repeats {
                panic!("Symbol '{name}' is already defined");
            }
        }

        pub fn get(&self, ident: &Ident) -> Option<ValueSSA> {
            match ident.kind {
                IdentKind::Global => self.globals.get(&ident.name).map(|g| ValueSSA::Global(*g)),
                IdentKind::Local => self.locals.get(&ident.name).copied(),
                IdentKind::Word => {
                    panic!("Word identifiers are not supported in symbol lookup")
                }
            }
        }

        pub fn reset_locals(&mut self) {
            self.locals.clear();
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct HashAggr {
        pub kind: AggrKind,
        pub ty: ValTypeID,
        pub elems: ValueList,
    }
    impl HashAggr {
        pub fn to_value(&self, module: &Module) -> SemaRes<ValueSSA> {
            match (self.kind, self.ty) {
                (AggrKind::Struct, ValTypeID::Struct(structty)) => self.to_struct(module, structty),
                (AggrKind::PackStruct, ValTypeID::Struct(structty)) => {
                    self.to_struct(module, structty)
                }
                (AggrKind::Array, ValTypeID::Array(arr)) => self.to_array(module, arr),
                (AggrKind::Vec, ValTypeID::FixVec(vecty)) => self.to_fixvec(module, vecty),
                (kind, ty) => Err(SemaErr::ValueNotMeetType(
                    format!("{kind:?}"),
                    ty.get_display_name(&module.tctx),
                )),
            }
        }

        fn to_struct(&self, module: &Module, structty: StructTypeID) -> SemaRes<ValueSSA> {
            let (allocs, tctx) = (&module.allocs, &module.tctx);
            let struc = StructExprID::new_uninit(allocs, tctx, structty);
            let fields = struc.field_uses(allocs);
            if fields.len() != self.elems.len() {
                return Err(SemaErr::AggrLengthMismatch {
                    name: "Struct field",
                    expect: fields.len(),
                    real: self.elems.len(),
                });
            }
            let field_types = structty.get_fields(tctx);
            for (i, field_use) in fields.iter().enumerate() {
                let value = self.elems[i];
                let field_type = field_types[i];
                let value_type = value.get_valtype(allocs);
                if value_type != field_type {
                    return Err(SemaErr::AggrElemTypeMismatch {
                        name: "Struct field",
                        index: i,
                        expect: field_type.get_display_name(tctx),
                        real: value_type.get_display_name(tctx),
                    });
                }
                field_use.set_operand(allocs, value);
            }
            Ok(struc.into_value())
        }
        fn to_array(&self, module: &Module, arrty: ArrayTypeID) -> SemaRes<ValueSSA> {
            let (allocs, tctx) = (&module.allocs, &module.tctx);
            let array = ArrayExprID::new_uninit(allocs, tctx, arrty);
            let elems = array.elem_uses(allocs);
            if elems.len() != self.elems.len() {
                return Err(SemaErr::AggrLengthMismatch {
                    name: "Array element",
                    expect: elems.len(),
                    real: self.elems.len(),
                });
            }
            let elem_type = arrty.get_element_type(tctx);
            for (i, elem_use) in elems.iter().enumerate() {
                let value = self.elems[i];
                let value_type = value.get_valtype(allocs);
                if value_type != elem_type {
                    return Err(SemaErr::AggrElemTypeMismatch {
                        name: "Array element",
                        index: i,
                        expect: elem_type.get_display_name(tctx),
                        real: value_type.get_display_name(tctx),
                    });
                }
                elem_use.set_operand(allocs, value);
            }
            Ok(array.into_value())
        }
        fn to_fixvec(&self, module: &Module, vecty: FixVecType) -> SemaRes<ValueSSA> {
            let (allocs, tctx) = (&module.allocs, &module.tctx);
            let (elemty, len) = {
                let FixVecType(elemty, len_log2) = vecty;
                (elemty.into_ir(), 1 << len_log2)
            };
            let vecid = FixVecID::new_uninit(allocs, vecty);
            let elems = vecid.elem_uses(allocs);
            if len != elems.len() {
                return Err(SemaErr::AggrLengthMismatch {
                    name: "Vector element",
                    expect: len,
                    real: elems.len(),
                });
            }
            for (i, elem_use) in elems.iter().enumerate() {
                let value = self.elems[i];
                let value_type = value.get_valtype(allocs);
                if value_type != elemty {
                    return Err(SemaErr::AggrElemTypeMismatch {
                        name: "Vector element",
                        index: i,
                        expect: elemty.get_display_name(tctx),
                        real: value_type.get_display_name(tctx),
                    });
                }
                elem_use.set_operand(allocs, value);
            }
            Ok(vecid.into_value())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct HashSparse {
        pub ty: ArrayTypeID,
        pub default: ValueSSA,
        pub indices: Vec<(usize, ValueSSA)>,
    }
    impl HashSparse {
        pub fn to_value(&self, module: &Module) -> SemaRes<ValueSSA> {
            self.sanity_assert(module)?;
            let (allocs, tctx) = (&module.allocs, &module.tctx);
            let mut kv_builder = KVArrayBuilder::new(tctx, allocs, self.ty);
            kv_builder.default_val(self.default);
            for &(index, value) in &self.indices {
                kv_builder.add_elem(index, value)?;
            }
            Ok(kv_builder.build_id().into_value())
        }

        fn sanity_assert(&self, module: &Module) -> SemaRes {
            if self.indices.is_empty() {
                return Ok(());
            }
            let mut prev = self.indices[0].0;
            let indices = &self.indices[1..];
            for &(index, _) in indices {
                if index <= prev {
                    return Err(SemaErr::SparseIndicesNotIncreasing);
                }
                prev = index;
            }

            let (allocs, tctx) = (&module.allocs, &module.tctx);
            let elemty = self.ty.get_element_type(tctx);
            if self.default.get_valtype(allocs) != elemty {
                return Err(SemaErr::SparseDefaultTypeMismatch {
                    expect: elemty.get_display_name(tctx),
                    real: self.default.get_valtype(allocs).get_display_name(tctx),
                });
            }
            for &(index, value) in &self.indices {
                if value.get_valtype(allocs) != elemty {
                    return Err(SemaErr::AggrElemTypeMismatch {
                        name: "Sparse array element",
                        index,
                        expect: elemty.get_display_name(tctx),
                        real: value.get_valtype(allocs).get_display_name(tctx),
                    });
                }
            }
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    pub struct ValuePool {
        aggrs: HashMap<HashAggr, ValueSSA>,
        kv_arrays: HashMap<HashSparse, ValueSSA>,
        bytes: HashMap<Arc<[u8]>, ValueSSA>,
    }

    impl ValuePool {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn map_aggr(&mut self, aggr: HashAggr, module: &Module) -> SemaRes<ValueSSA> {
            if let Some(val) = self.aggrs.get(&aggr) {
                Ok(*val)
            } else {
                let val = aggr.to_value(module)?;
                self.aggrs.insert(aggr, val);
                Ok(val)
            }
        }

        pub fn map_kv_array(&mut self, kv_array: HashSparse, module: &Module) -> SemaRes<ValueSSA> {
            if let Some(val) = self.kv_arrays.get(&kv_array) {
                Ok(*val)
            } else {
                let val = kv_array.to_value(module)?;
                self.kv_arrays.insert(kv_array, val);
                Ok(val)
            }
        }

        pub fn map_bytes(
            &mut self,
            bytes: Arc<[u8]>,
            byte_ty: ArrayTypeID,
            module: &Module,
        ) -> SemaRes<ValueSSA> {
            if let Some(val) = self.bytes.get(&bytes) {
                Ok(*val)
            } else {
                let (tctx, allocs) = (&module.tctx, &module.allocs);
                if byte_ty.get_element_type(tctx) != ValTypeID::Int(8) {
                    return Err(SemaErr::ValueNotMeetType(
                        "byte array element type".to_string(),
                        byte_ty.get_element_type(tctx).get_display_name(tctx),
                    ));
                }
                if byte_ty.get_num_elements(tctx) != bytes.len() {
                    return Err(SemaErr::AggrLengthMismatch {
                        name: "byte array",
                        expect: byte_ty.get_num_elements(tctx),
                        real: bytes.len(),
                    });
                }
                let Some(mut darray) = DataArrayExpr::new_zeroed(tctx, byte_ty) else {
                    panic!("Internal error: bytes expression should always be creatable");
                };
                let ConstArrayData::I8(ir_bytes) = &mut darray.data else {
                    panic!("Internal error: bytes expression should always be i8 array");
                };
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        bytes.as_ptr(),
                        ir_bytes.as_mut_ptr() as *mut u8,
                        bytes.len(),
                    );
                }
                let data_id = DataArrayExprID::allocate(allocs, darray).into_value();
                self.bytes.insert(bytes, data_id);
                Ok(data_id)
            }
        }
    }
}

pub use ast_modify::*;
mod ast_modify {
    use smol_str::format_smolstr;

    use crate::ast::{FuncAst, Ident, ModuleAst};

    #[derive(Default)]
    struct Counter(usize);

    impl Counter {
        fn next(&mut self) -> usize {
            let val = self.0;
            self.0 += 1;
            val
        }

        fn emit_name(&mut self, name: &str) -> Option<usize> {
            if name.is_empty() {
                return Some(self.next());
            }
            match name.parse::<usize>() {
                Ok(id) if id == self.0 => self.0 += 1,
                _ => (),
            }
            None
        }
    }

    pub fn fill_func_ids(func: &mut FuncAst) {
        let mut id_count = Counter::default();
        if func.body.is_none() {
            return;
        }
        for arg in &mut func.header.args {
            let Ident { name, .. } = &mut arg.name;
            if let Some(id) = id_count.emit_name(name) {
                *name = format_smolstr!("{id}");
            }
        }
        let body = func.body.as_mut().unwrap();
        let entry = &mut body.blocks[0];
        let Ident { name, .. } = &mut entry.label;
        if let Some(id) = id_count.emit_name(name) {
            *name = format_smolstr!("{id}");
        }
        // The blocks after entry block should have a valid name
        // so we do not process them here.
        // And so are the instructions inside blocks.
    }
    pub fn fill_module_ids(module: &mut ModuleAst) {
        for func in &mut module.funcs {
            fill_func_ids(func);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SemaErr {
    #[error("Type alias '{0}' is redefined")]
    RedefinedTypeAlias(SmolStr),

    #[error("Type alias '{0}' depends on itself")]
    TypeSelfDepend(SmolStr),

    #[error("Type system formed circular dependency involving")]
    TypeCycled,

    #[error("Type alias '{0}' not found")]
    TypeAliasNotFound(SmolStr),

    #[error("Type alias '{0}' must map to a struct type")]
    TypeAliasNotMappedToStruct(SmolStr),

    #[error("Vector element type '{0}' is not supported; only primitive types are allowed")]
    VectorElemTypeNotPrimtive(&'static str),

    #[error("Vector length '{0}' is not a power of two")]
    VectorLengthNotPowerOfTwo(usize),

    #[error("Value type kind is {0} and cannot match type {1}")]
    ValueNotMeetType(String, String),

    #[error("{name} count mismatch: expected {expect}, got {real}")]
    AggrLengthMismatch {
        name: &'static str,
        expect: usize,
        real: usize,
    },
    #[error("{name} element {index} type mismatch: expected '{expect}', got '{real}'")]
    AggrElemTypeMismatch {
        name: &'static str,
        index: usize,
        expect: String,
        real: String,
    },

    #[error("Array building failed: {0}")]
    ArrBuild(#[from] ArrayBuildErr),

    #[error("Sparse array indices is not strictly increasing")]
    SparseIndicesNotIncreasing,

    #[error("Sparse array default value type '{real}' does not match expected type '{expect}'")]
    SparseDefaultTypeMismatch { expect: String, real: String },
}
pub type SemaRes<T = ()> = Result<T, SemaErr>;
