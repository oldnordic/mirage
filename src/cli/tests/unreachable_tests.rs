// ============================================================================

use crate::cfg::reachability::find_unreachable;
use crate::cfg::{BasicBlock, BlockKind, Cfg, EdgeType, Terminator};
use crate::cli::responses::*;
use crate::cli::*;
use petgraph::graph::DiGraph;

/// Helper to create a test CFG with an unreachable block
fn create_cfg_with_unreachable() -> Cfg {
    let mut g = DiGraph::new();

    // Block 0: entry, goes to 1
    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec!["let x = 1".to_string()],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });

    // Block 1: normal, goes to 2
    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec!["if x > 0".to_string()],
        terminator: Terminator::SwitchInt {
            targets: vec![2],
            otherwise: 3,
        },
        source_location: None,
    });

    // Block 2: exit (reachable)
    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec!["return true".to_string()],
        terminator: Terminator::Return,
        source_location: None,
    });

    // Block 3: exit (reachable)
    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec!["return false".to_string()],
        terminator: Terminator::Return,
        source_location: None,
    });

    // Block 4: unreachable (no edges to it)
    let _b4 = g.add_node(BasicBlock {
        id: 4,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec!["unreachable code".to_string()],
        terminator: Terminator::Unreachable,
        source_location: None,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::TrueBranch);
    g.add_edge(b1, b3, EdgeType::FalseBranch);

    g
}

/// Test that unreachable blocks are detected
#[test]
fn test_unreachable_detects_dead_code() {
    let cfg = create_cfg_with_unreachable();
    let unreachable_indices = find_unreachable(&cfg);

    // Should find exactly 1 unreachable block (block 4)
    assert_eq!(
        unreachable_indices.len(),
        1,
        "Should find exactly 1 unreachable block"
    );

    // Verify it's block 4
    let block_id = cfg.node_weight(unreachable_indices[0]).unwrap().id;
    assert_eq!(block_id, 4, "Unreachable block should be block 4");
}

/// Test that UnreachableResponse struct serializes correctly
#[test]
fn test_unreachable_response_serialization() {
    use crate::output::JsonResponse;

    let response = UnreachableResponse {
        uncalled_functions: None,
        function: "test_func".to_string(),
        total_functions: 1,
        functions_with_unreachable: 1,
        unreachable_count: 1,
        blocks: vec![UnreachableBlock {
            block_id: 4,
            kind: "Exit".to_string(),
            statements: vec!["unreachable code".to_string()],
            terminator: "Unreachable".to_string(),
            incoming_edges: vec![],
        }],
    };

    let wrapper = JsonResponse::new(response);
    let json = wrapper.to_json();

    assert!(json.contains("\"function\":\"test_func\""));
    assert!(json.contains("\"unreachable_count\":1"));
    assert!(json.contains("\"block_id\":4"));
    assert!(json.contains("\"kind\":\"Exit\""));
}

/// Test that empty unreachable response is handled correctly
#[test]
fn test_unreachable_empty_response() {
    use crate::output::JsonResponse;

    let response = UnreachableResponse {
        uncalled_functions: None,
        function: "test_func".to_string(),
        total_functions: 1,
        functions_with_unreachable: 0,
        unreachable_count: 0,
        blocks: vec![],
    };

    let wrapper = JsonResponse::new(response);
    let json = wrapper.to_json();

    assert!(json.contains("\"unreachable_count\":0"));
    assert!(json.contains("\"functions_with_unreachable\":0"));
}

/// Test that UnreachableBlock struct contains expected fields
#[test]
fn test_unreachable_block_fields() {
    let block = UnreachableBlock {
        block_id: 5,
        kind: "Normal".to_string(),
        statements: vec!["stmt1".to_string(), "stmt2".to_string()],
        terminator: "Return".to_string(),
        incoming_edges: vec![],
    };

    assert_eq!(block.block_id, 5);
    assert_eq!(block.kind, "Normal");
    assert_eq!(block.statements.len(), 2);
    assert_eq!(block.terminator, "Return");
}

/// Test UnreachableArgs flags
#[test]
fn test_unreachable_args_flags() {
    let args_with = UnreachableArgs {
        include_uncalled: false,
        within_functions: true,
        show_branches: true,
    };

    let args_without = UnreachableArgs {
        include_uncalled: false,
        within_functions: false,
        show_branches: false,
    };

    assert!(args_with.within_functions);
    assert!(args_with.show_branches);
    assert!(!args_without.within_functions);
    assert!(!args_without.show_branches);
}

/// Test that create_test_cfg has no unreachable blocks
#[test]
fn test_test_cfg_fully_reachable() {
    let cfg = cmds::create_test_cfg();
    let unreachable_indices = find_unreachable(&cfg);

    // Test CFG should have no unreachable blocks
    assert_eq!(
        unreachable_indices.len(),
        0,
        "Test CFG should have no unreachable blocks"
    );
}

/// Test that --show-branches includes incoming edge details
#[test]
fn test_unreachable_show_branches_with_edges() {
    use crate::cfg::reachability::find_unreachable;
    use petgraph::visit::EdgeRef;

    // Create a CFG with an unreachable block that HAS incoming edges
    // This simulates a block that's only reachable from an unreachable source
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec!["let x = 1".to_string()],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec!["if x > 0".to_string()],
        terminator: Terminator::SwitchInt {
            targets: vec![2],
            otherwise: 3,
        },
        source_location: None,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec!["return true".to_string()],
        terminator: Terminator::Return,
        source_location: None,
    });

    // b3 and b4 are both unreachable, but b4 has an incoming edge from b3
    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec!["unreachable branch".to_string()],
        terminator: Terminator::Goto { target: 4 },
        source_location: None,
    });

    let b4 = g.add_node(BasicBlock {
        id: 4,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec!["unreachable code".to_string()],
        terminator: Terminator::Unreachable,
        source_location: None,
    });

    // Only connect entry to b1, making b3 and b4 unreachable
    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::TrueBranch);
    // b3 -> b4 edge exists, but both blocks are unreachable
    g.add_edge(b3, b4, EdgeType::Fallthrough);

    // Build UnreachableBlock structs with show_branches=true
    let unreachable_indices = find_unreachable(&g);
    let blocks: Vec<UnreachableBlock> = unreachable_indices
        .iter()
        .map(|&idx| {
            let block = &g[idx];
            let kind_str = format!("{:?}", block.kind);
            let terminator_str = format!("{:?}", block.terminator);

            // Collect incoming edges
            let incoming_edges: Vec<IncomingEdge> = g
                .edge_references()
                .filter(|edge| edge.target() == idx)
                .map(|edge| {
                    let source_block = &g[edge.source()];
                    let edge_type = g.edge_weight(edge.id()).unwrap();
                    IncomingEdge {
                        from_block: source_block.id,
                        edge_type: format!("{:?}", edge_type),
                    }
                })
                .collect();

            UnreachableBlock {
                block_id: block.id,
                kind: kind_str,
                statements: block.statements.clone(),
                terminator: terminator_str,
                incoming_edges,
            }
        })
        .collect();

    // Should find 2 unreachable blocks (3 and 4)
    assert_eq!(blocks.len(), 2);

    // Block 3 should have no incoming edges (isolated unreachable code)
    let block3 = blocks.iter().find(|b| b.block_id == 3).unwrap();
    assert_eq!(block3.incoming_edges.len(), 0);

    // Block 4 should have 1 incoming edge from block 3
    let block4 = blocks.iter().find(|b| b.block_id == 4).unwrap();
    assert_eq!(block4.incoming_edges.len(), 1);
    assert_eq!(block4.incoming_edges[0].from_block, 3);
    assert_eq!(block4.incoming_edges[0].edge_type, "Fallthrough");
}

/// Test that --show-branches JSON output includes incoming_edges field
#[test]
fn test_unreachable_show_branches_json_output() {
    use crate::cfg::reachability::find_unreachable;
    use crate::output::JsonResponse;
    use petgraph::visit::EdgeRef;

    // Create the same CFG as above
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec!["let x = 1".to_string()],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec!["if x > 0".to_string()],
        terminator: Terminator::SwitchInt {
            targets: vec![2],
            otherwise: 3,
        },
        source_location: None,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec!["return true".to_string()],
        terminator: Terminator::Return,
        source_location: None,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec!["unreachable branch".to_string()],
        terminator: Terminator::Goto { target: 4 },
        source_location: None,
    });

    let b4 = g.add_node(BasicBlock {
        id: 4,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec!["unreachable code".to_string()],
        terminator: Terminator::Unreachable,
        source_location: None,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::TrueBranch);
    g.add_edge(b3, b4, EdgeType::Fallthrough);

    // Build UnreachableBlock structs with incoming edges
    let unreachable_indices = find_unreachable(&g);
    let blocks: Vec<UnreachableBlock> = unreachable_indices
        .iter()
        .map(|&idx| {
            let block = &g[idx];
            UnreachableBlock {
                block_id: block.id,
                kind: format!("{:?}", block.kind),
                statements: block.statements.clone(),
                terminator: format!("{:?}", block.terminator),
                incoming_edges: g
                    .edge_references()
                    .filter(|edge| edge.target() == idx)
                    .map(|edge| {
                        let source_block = &g[edge.source()];
                        let edge_type = g.edge_weight(edge.id()).unwrap();
                        IncomingEdge {
                            from_block: source_block.id,
                            edge_type: format!("{:?}", edge_type),
                        }
                    })
                    .collect(),
            }
        })
        .collect();

    let response = UnreachableResponse {
        function: "test".to_string(),
        total_functions: 1,
        functions_with_unreachable: 1,
        unreachable_count: 2,
        blocks,
        uncalled_functions: None,
    };

    let wrapper = JsonResponse::new(response);
    let json = wrapper.to_json();

    // Verify JSON contains incoming_edges field
    assert!(json.contains("\"incoming_edges\""));
    // Verify block 4 has an incoming edge from block 3
    assert!(json.contains("\"from_block\":3"));
    assert!(json.contains("\"edge_type\":\"Fallthrough\""));
}

/// Test that IncomingEdge struct serializes correctly
#[test]
fn test_incoming_edge_serialization() {
    let edge = IncomingEdge {
        from_block: 5,
        edge_type: "TrueBranch".to_string(),
    };

    let serialized = serde_json::to_string(&edge).unwrap();
    assert!(serialized.contains("\"from_block\":5"));
    assert!(serialized.contains("\"edge_type\":\"TrueBranch\""));
}
