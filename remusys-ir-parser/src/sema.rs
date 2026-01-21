use smol_str::SmolStr;
use std::{collections::HashMap, sync::Arc};

use crate::ast::{ModuleAst, TypeAliasItem};

pub struct TypeAliasSort<'ast> {
    pub types: &'ast mut [Arc<TypeAliasItem>],
    pub index_map: HashMap<SmolStr, usize>,
}

impl<'ast> TypeAliasSort<'ast> {
    pub fn new(module: &'ast mut ModuleAst) -> Self {
        let types = &mut module.type_aliases;
        let index_map = types
            .iter()
            .enumerate()
            .map(|(i, t)| (t.name.clone(), i))
            .collect();
        Self { types, index_map }
    }
}
