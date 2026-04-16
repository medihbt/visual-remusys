#![cfg(test)]

use crate::*;
use remusys_ir::{
    ir::{inst::*, *},
    testing::cases::*,
};

fn first_defined_func(module: &Module) -> FuncID {
    let symbols = module.symbols.borrow();
    symbols
        .func_pool()
        .iter()
        .copied()
        .find(|f| !f.is_extern(&module.allocs))
        .expect("expected a defined function in test case")
}

fn find_inst_in_func(
    module: &Module,
    func: FuncID,
    pred: impl Fn(&InstObj) -> bool,
) -> Option<InstID> {
    let allocs = &module.allocs;
    for (_, block) in func.blocks_iter(allocs) {
        for (inst_id, inst) in block.get_insts().iter(&allocs.insts) {
            if pred(inst) {
                return Some(inst_id);
            }
        }
    }
    None
}

fn build_inst_node(module: &Module, func: FuncID, inst_id: InstID) -> (IRTree, IRTreeNodeID, String) {
    let dag = IRTree::new();
    let names = IRNameMap::default();
    let mut builder = IRDagBuilder::new(module, &names, &dag);
    builder.curr_scope = Some(func);
    let node = builder
        .build(IRTreeObjID::Inst(inst_id))
        .expect("format inst to IRDag node");
    let source = builder.source_buf;
    (dag, node, source)
}

#[test]
fn fmt_inst_call_from_minmax_case() {
    let module = test_case_minmax().module;
    let func = first_defined_func(&module);
    let call_inst = find_inst_in_func(&module, func, |inst| matches!(inst, InstObj::Call(_)))
        .expect("expected at least one call instruction");

    let (dag, node, source) = build_inst_node(&module, func, call_inst);
    let node_ref = node.deref(&dag);

    assert_eq!(node_ref.obj, IRTreeObjID::Inst(call_inst));
    assert!(source.contains("call"));
    assert!(
        !node_ref.children.is_empty(),
        "call should have use children"
    );
    assert!(node_ref.pos_delta.start <= node_ref.pos_delta.end);
}

#[test]
fn fmt_inst_branch_from_cfg_case() {
    let module = test_case_cfg_deep_while_br().module;
    let func = first_defined_func(&module);
    let br_inst = find_inst_in_func(&module, func, |inst| matches!(inst, InstObj::Br(_)))
        .expect("expected at least one conditional branch");

    let (dag, node, source) = build_inst_node(&module, func, br_inst);
    let node_ref = node.deref(&dag);

    assert_eq!(node_ref.obj, IRTreeObjID::Inst(br_inst));
    assert!(source.contains("br i1"));
    assert_eq!(
        node_ref.children.len(),
        3,
        "br must have cond/then/else nodes"
    );
    assert!(node_ref.pos_delta.start <= node_ref.pos_delta.end);
}

#[test]
fn fmt_inst_ret_from_cfg_case() {
    let module = test_case_cfg_deep_while_br().module;
    let func = first_defined_func(&module);
    let ret_inst = find_inst_in_func(&module, func, |inst| matches!(inst, InstObj::Ret(_)))
        .expect("expected at least one return instruction");

    let (dag, node, source) = build_inst_node(&module, func, ret_inst);
    let node_ref = node.deref(&dag);

    println!("{}", dag.print_to_dot(node));

    assert_eq!(node_ref.obj, IRTreeObjID::Inst(ret_inst));
    assert!(source.contains("ret"));
    assert!(
        !node_ref.children.is_empty(),
        "ret i32 should have one use child"
    );
    assert!(node_ref.pos_delta.start <= node_ref.pos_delta.end);
}

#[test]
fn fmt_module_source() {
    let module = test_case_minmax().module;
    let dag = IRTree::new();
    let names = IRNameMap::default();
    let mut builder = IRDagBuilder::new(&module, &names, &dag);
    let node = builder
        .build(IRTreeObjID::Module)
        .expect("format module to IRDag node");
    let source = builder.source_buf;

    println!("{}", source);
    let graph_dot = dag.print_to_dot(node);
    let outfile = std::env::temp_dir().join("module_dag.dot");
    std::fs::write(&outfile, graph_dot).expect("write graph dot to file");
    println!("Graph dot written to: {}", outfile.display());
}
