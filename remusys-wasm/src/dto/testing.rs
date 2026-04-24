#![cfg(test)]

use crate::*;
use std::path::PathBuf;
use remusys_ir::{
    ir::*,
    testing::cases::*,
};

use crate::dto::dfg::BlockDfg;

fn first_defined_func(module: &Module) -> FuncID {
    let symbols = module.symbols.borrow();
    symbols
        .func_pool()
        .iter()
        .copied()
        .find(|f| !f.is_extern(&module.allocs))
        .expect("expected a defined function in test case")
}

fn write_block_dfg_dot(module_info: &ModuleInfo, block_id: BlockID) -> PathBuf {
    let dfg = BlockDfg::new(module_info, block_id).expect("build block dfg");
    let dot_text = dfg.to_dot_text();
    let outfile = std::env::temp_dir().join(format!("block_dfg.{}.dot", block_id.to_strid()));
    std::fs::write(&outfile, dot_text).expect("write block dfg dot to file");
    outfile
}

#[test]
fn dump_block_dfg_for_each_block_from_minmax_case() {
    let module = test_case_minmax().module;
    let func = first_defined_func(&module);
    // Collect IDs before moving the module into ModuleInfo.
    // Cloning Module is unsafe here: EntityAlloc reassigns storage positions/generations,
    // so IDs from the old module may point to unrelated entities in the clone.
    let block_ids: Vec<_> = func.blocks_iter(&module.allocs).map(|(block_id, _)| block_id).collect();
    let module_info = ModuleInfo::from_test_module(module).expect("build ModuleInfo for test");

    let mut dumped = 0usize;
    for block_id in block_ids {
        let outfile = write_block_dfg_dot(&module_info, block_id);
        println!("Block DFG dot written to: {}", outfile.display());
        dumped += 1;
    }

    assert!(dumped > 0, "expected at least one block to dump DFG");
}
