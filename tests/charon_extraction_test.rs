use mirage::mir::charon_llbc::Crate;
use std::fs;

#[test]
fn test_parse_add_llbc() {
    let json_content = fs::read_to_string("tests/data/add.llbc").expect("Failed to read test data");
    let result: Crate =
        serde_json::from_str(&json_content).expect("Failed to deserialize LLBC JSON");

    assert_eq!(result.crate_name, "test_crate");
    assert_eq!(result.fun_decls.len(), 1);

    let fun = &result.fun_decls[0];
    assert_eq!(fun.name, vec!["test_crate", "add"]);
    assert_eq!(fun.body.as_ref().unwrap().arg_count, 2);
}

#[test]
fn test_parse_if_else_llbc() {
    let json_content =
        fs::read_to_string("tests/data/if_else.llbc").expect("Failed to read test data");
    let result: Crate =
        serde_json::from_str(&json_content).expect("Failed to deserialize LLBC JSON");

    assert_eq!(result.fun_decls.len(), 1);
    let fun = &result.fun_decls[0];
    let body = fun.body.as_ref().unwrap();
    let unstructured = body.unstructured.as_ref().unwrap();

    assert_eq!(unstructured.blocks.len(), 3);
    assert!(matches!(
        unstructured.blocks[0].terminator,
        mirage::mir::charon_llbc::Terminator::SwitchInt { .. }
    ));
}

#[test]
fn test_translate_if_else() {
    use mirage::mir::translator::translate_function;

    let json_content =
        fs::read_to_string("tests/data/if_else.llbc").expect("Failed to read test data");
    let result: Crate =
        serde_json::from_str(&json_content).expect("Failed to deserialize LLBC JSON");

    let fun = &result.fun_decls[0];
    let cfg = translate_function(fun, &result.files);

    // Check nodes
    assert_eq!(cfg.node_count(), 3, "Should have 3 blocks");

    // Check edges
    let edges: Vec<_> = cfg.edge_references().collect();
    assert_eq!(edges.len(), 2, "Should have 2 edges from the switch");
}
