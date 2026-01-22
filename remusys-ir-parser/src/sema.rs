pub use type_mapping::*;
mod type_mapping {
    use crate::ast::{ModuleAst, TypeAliasItem, TypeAst, TypeAstKind};
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

    type IndexVec = SmallVec<[usize; 16]>;

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
        pub fn new(module: &'ast ModuleAst) -> Self {
            let types = &module.type_aliases;
            let index_map = types
                .iter()
                .enumerate()
                .map(|(i, t)| (t.name.clone(), i))
                .collect();
            Self { types, index_map }
        }

        pub fn sort(&self) -> SmallVec<[Arc<TypeAliasItem>; 16]> {
            let mut nodes: Vec<AliasNode> = (0..self.types.len())
                .map(|i| AliasNode::new(self, i))
                .collect();

            for i in 0..nodes.len() {
                let deps: IndexVec = nodes[i].dependencies.iter().copied().collect();
                for dep in deps {
                    nodes[dep].users.insert(i);
                }
            }

            let mut sorted = IndexVec::with_capacity(nodes.len());
            let mut no_deps: IndexVec = nodes
                .iter()
                .enumerate()
                .filter_map(|(i, n)| {
                    if n.dependencies.is_empty() {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect();

            while let Some(index) = no_deps.pop() {
                sorted.push(index);
                let users: IndexVec = nodes[index].users.iter().cloned().collect();
                for user in users {
                    nodes[user].dependencies.remove(&index);
                    if nodes[user].dependencies.is_empty() {
                        no_deps.push(user);
                    }
                }
            }

            if sorted.len() != nodes.len() {
                panic!("Cyclic dependency detected among type aliases");
            }

            sorted
                .into_iter()
                .map(|i| Arc::clone(&self.types[i]))
                .collect()
        }
    }

    struct AliasNode {
        dependencies: HashSet<usize>,
        users: HashSet<usize>,
    }

    impl AliasNode {
        pub fn new(sort: &TypeAliasSort<'_>, index: usize) -> Self {
            let alias = Arc::clone(&sort.types[index]);
            let deps = Self::collect_deps(&alias.ty, &sort.index_map);
            if deps.contains(&index) {
                panic!("Type alias '{}' cannot depend on itself", alias.name);
            }
            Self {
                dependencies: deps,
                users: HashSet::new(),
            }
        }

        fn collect_deps(ty: &TypeAst, index_map: &HashMap<SmolStr, usize>) -> HashSet<usize> {
            let mut deps = HashSet::new();
            Self::_do_collect_deps(ty, &mut deps);
            deps.into_iter()
                .map(|name| *index_map.get(&name).expect("Type alias not found"))
                .collect()
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
        pub fn new(module: &Module, ast: &ModuleAst) -> Self {
            let sorted = TypeAliasSort::new(ast).sort();
            let mut ret = Self::default();
            for alias in sorted {
                let tctx = &module.tctx;
                let TypeAliasItem { name, ty, .. } = alias.as_ref();
                let ValTypeID::Struct(struc) = ret.map_type(tctx, ty) else {
                    panic!("Type alias '{name}' must map to a struct type")
                };
                let alias_id = StructAliasID::new(tctx, name.to_string(), struc);
                ret.alias_map.insert(name.clone(), alias_id);
            }
            ret
        }

        pub fn map_type(&mut self, tctx: &TypeContext, ty: &TypeAst) -> ValTypeID {
            match &ty.kind {
                TypeAstKind::Void => ValTypeID::Void,
                TypeAstKind::Ptr => ValTypeID::Ptr,
                TypeAstKind::Int(i) => ValTypeID::Int(*i),
                TypeAstKind::FP(fpkind) => ValTypeID::Float(*fpkind),
                TypeAstKind::Array { elem, len } => {
                    let elemty = self.map_type(tctx, elem);
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
                            panic!("vector type only supports int | float | ptr but got {kind:?}")
                        }
                    };
                    if !len.is_power_of_two() {
                        panic!("vector length must be a power of two but got {len}");
                    }
                    ValTypeID::FixVec(FixVecType(elemty, len.trailing_zeros() as u8))
                }
                TypeAstKind::Alias(ident) => {
                    let name = ident.name.as_str();
                    let alias_id = self
                        .alias_map
                        .get(name)
                        .unwrap_or_else(|| panic!("Type alias '{name}' not found"));
                    ValTypeID::StructAlias(*alias_id)
                }
                TypeAstKind::Struct { elem, packed } => {
                    let mut fields: SmallVec<[ValTypeID; 8]> = SmallVec::with_capacity(elem.len());
                    for e in elem {
                        let field_ty = self.map_type(tctx, e);
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
            }
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
        pub fn return_type(&mut self, ty: &TypeAst) -> &mut Self {
            let ret_ty = self.tmap.map_type(self.tctx, ty);
            self.func.ret_ty = ret_ty;
            self
        }
        pub fn add_argtype(&mut self, ty: &TypeAst) -> &mut Self {
            let arg_ty = self.tmap.map_type(self.tctx, ty);
            self.func.param_tys.push(arg_ty);
            self
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
