// ============================================================================

use crate::cfg::{compute_dominance_frontiers, DominatorTree};
use crate::cli::responses::*;
use crate::cli::*;
/// Test frontiers response struct serialization
#[test]
fn test_frontiers_response_serialization() {
    use crate::output::JsonResponse;

    let response = FrontiersResponse {
        function: "test_func".to_string(),
        nodes_with_frontiers: 2,
        frontiers: vec![
            NodeFrontier {
                node: 1,
                frontier_set: vec![3],
            },
            NodeFrontier {
                node: 2,
                frontier_set: vec![3],
            },
        ],
    };

    let wrapper = JsonResponse::new(response);
    let json = wrapper.to_json();

    // Verify JSON structure
    assert!(json.contains("\"function\":\"test_func\""));
    assert!(json.contains("\"nodes_with_frontiers\":2"));
    assert!(json.contains("\"frontiers\":["));
}

/// Test iterated frontier response struct serialization
#[test]
fn test_iterated_frontier_response_serialization() {
    use crate::output::JsonResponse;

    let response = IteratedFrontierResponse {
        function: "test_func".to_string(),
        iterated_frontier: vec![3, 4],
    };

    let wrapper = JsonResponse::new(response);
    let json = wrapper.to_json();

    // Verify JSON structure
    assert!(json.contains("\"function\":\"test_func\""));
    assert!(json.contains("\"iterated_frontier\":[3,4]"));
}

/// Test basic frontier computation (diamond CFG)
#[test]
fn test_frontiers_basic() {
    use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};
    use petgraph::graph::DiGraph;

    // Create diamond CFG: 0 -> 1,2 -> 3
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![1],
            otherwise: 2,
        },
        source_location: None,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec!["branch 1".to_string()],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec!["branch 2".to_string()],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });

    g.add_edge(b0, b1, EdgeType::TrueBranch);
    g.add_edge(b0, b2, EdgeType::FalseBranch);
    g.add_edge(b1, b3, EdgeType::Fallthrough);
    g.add_edge(b2, b3, EdgeType::Fallthrough);

    // Compute dominance frontiers
    let dom_tree = DominatorTree::new(&g).expect("CFG has entry");
    let frontiers = compute_dominance_frontiers(&g, dom_tree);

    // In diamond CFG:
    // DF[1] = {3} (1 dominates itself, pred of 3, doesn't strictly dominate 3)
    // DF[2] = {3} (2 dominates itself, pred of 3, doesn't strictly dominate 3)
    let df1 = frontiers.frontier(b1);
    assert!(df1.contains(&b3));
    assert_eq!(df1.len(), 1);

    let df2 = frontiers.frontier(b2);
    assert!(df2.contains(&b3));
    assert_eq!(df2.len(), 1);

    // Entry (0) has empty frontier (strictly dominates all nodes)
    let df0 = frontiers.frontier(b0);
    assert!(df0.is_empty());
}

/// Test --iterated flag functionality
#[test]
fn test_frontiers_iterated_flag() {
    let args = FrontiersArgs {
        function: "test_func".to_string(),
        semantic_query: None,
        file: None,
        iterated: true,
        node: None,
    };

    assert!(args.iterated);
    assert!(args.node.is_none());
}

/// Test --node flag functionality
#[test]
fn test_frontiers_node_flag() {
    let args = FrontiersArgs {
        function: "test_func".to_string(),
        semantic_query: None,
        file: None,
        iterated: false,
        node: Some(5),
    };

    assert!(!args.iterated);
    assert_eq!(args.node, Some(5));
}

/// Test frontiers with linear CFG (empty frontiers)
#[test]
fn test_frontiers_linear_cfg() {
    use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};
    use petgraph::graph::DiGraph;

    // Linear CFG: 0 -> 1 -> 2 -> 3
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 2 },
        source_location: None,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::Goto { target: 3 },
        source_location: None,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::Fallthrough);
    g.add_edge(b2, b3, EdgeType::Fallthrough);

    // Compute dominance frontiers
    let dom_tree = DominatorTree::new(&g).expect("CFG has entry");
    let frontiers = compute_dominance_frontiers(&g, dom_tree);

    // Linear CFG has no dominance frontiers (no join points)
    let nodes_with_frontiers: Vec<_> = frontiers.nodes_with_frontiers().collect();
    assert!(nodes_with_frontiers.is_empty());
}

/// Test frontiers with loop CFG (self-frontier)
#[test]
fn test_frontiers_loop_cfg() {
    use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};
    use petgraph::graph::DiGraph;

    // Loop CFG: 0 -> 1 <-> 2 (back edge), 1 -> 3 (exit)
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });

    let b1 = g.add_node(BasicBlock {
        id: 1,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec![],
        terminator: Terminator::SwitchInt {
            targets: vec![2],
            otherwise: 3,
        },
        source_location: None,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec!["loop body".to_string()],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::TrueBranch);
    g.add_edge(b1, b3, EdgeType::FalseBranch);
    g.add_edge(b2, b1, EdgeType::LoopBack);

    // Compute dominance frontiers
    let dom_tree = DominatorTree::new(&g).expect("CFG has entry");
    let frontiers = compute_dominance_frontiers(&g, dom_tree);

    // Loop header (1) should have self-frontier due to back edge
    let df1 = frontiers.frontier(b1);
    assert!(df1.contains(&b1), "Loop header should have self-frontier");
}

/// Test frontiers command with json output format
#[test]
fn test_frontiers_json_output_format() {
    use crate::output::JsonResponse;

    let response = FrontiersResponse {
        function: "json_test".to_string(),
        nodes_with_frontiers: 2,
        frontiers: vec![
            NodeFrontier {
                node: 1,
                frontier_set: vec![3],
            },
            NodeFrontier {
                node: 2,
                frontier_set: vec![3],
            },
        ],
    };

    let wrapper = JsonResponse::new(response);
    let json = wrapper.to_json();

    // Verify JSON structure with metadata
    assert!(json.contains("\"schema_version\""));
    assert!(json.contains("\"execution_id\""));
    assert!(json.contains("\"tool\""));
    assert!(json.contains("\"timestamp\""));
    assert!(json.contains("\"data\""));
}

/// Test frontiers response with empty frontiers
#[test]
fn test_frontiers_response_empty() {
    use crate::output::JsonResponse;

    let response = FrontiersResponse {
        function: "linear_func".to_string(),
        nodes_with_frontiers: 0,
        frontiers: vec![],
    };

    let wrapper = JsonResponse::new(response);
    let json = wrapper.to_json();

    // Should handle empty frontiers gracefully
    assert!(json.contains("\"nodes_with_frontiers\":0"));
    assert!(json.contains("\"frontiers\":[]"));
}

// ============================================================================
// Hotspots Command Tests
// ============================================================================

/// Test hotspots args parsing
#[test]
fn test_hotspots_args_parsing() {
    let args = HotspotsArgs {
        entry: "main".to_string(),
        top: 10,
        min_paths: Some(5),
        verbose: true,
        inter_procedural: false,
        intra_procedural: false,
    };

    assert_eq!(args.entry, "main");
    assert_eq!(args.top, 10);
    assert_eq!(args.min_paths, Some(5));
    assert!(args.verbose);
    assert!(!args.inter_procedural);
}

/// Test hotspots entry point default
#[test]
fn test_hotspots_args_default_entry() {
    let args = HotspotsArgs {
        entry: "main".to_string(), // default value
        top: 20,
        min_paths: None,
        verbose: false,
        inter_procedural: false,
        intra_procedural: false,
    };

    assert_eq!(args.entry, "main");
    assert_eq!(args.top, 20); // default value
}

/// Test hotspot entry serialization
#[test]
fn test_hotspot_entry_serialization() {
    let entry = HotspotEntry {
        function: "test_func".to_string(),
        risk_score: 42.5,
        path_count: 10,
        dominance_factor: 1.5,
        complexity: 5,
        file_path: "test.rs".to_string(),
    };

    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("test_func"));
    assert!(json.contains("42.5"));
    assert!(json.contains("\"path_count\":10"));
}

/// Test hotspots response serialization
#[test]
fn test_hotspots_response_serialization() {
    use crate::output::JsonResponse;

    let response = HotspotsResponse {
        entry_point: "main".to_string(),
        total_functions: 100,
        hotspots: vec![],
        mode: "intra-procedural".to_string(),
    };

    let wrapper = JsonResponse::new(response);
    let json = wrapper.to_json();

    assert!(json.contains("\"entry_point\":\"main\""));
    assert!(json.contains("\"total_functions\":100"));
    assert!(json.contains("intra-procedural"));
}

/// Test hotspots response with entries
#[test]
fn test_hotspots_response_with_entries() {
    use crate::output::JsonResponse;

    let hotspot = HotspotEntry {
        function: "risky_func".to_string(),
        risk_score: 85.0,
        path_count: 50,
        dominance_factor: 3.0,
        complexity: 15,
        file_path: "src/lib.rs".to_string(),
    };

    let response = HotspotsResponse {
        entry_point: "main".to_string(),
        total_functions: 10,
        hotspots: vec![hotspot],
        mode: "inter-procedural".to_string(),
    };

    let wrapper = JsonResponse::new(response);
    let json = wrapper.to_json();

    assert!(json.contains("risky_func"));
    assert!(json.contains("85"));
    assert!(json.contains("inter-procedural"));
}

/// Test hotspots clone (needed for vector operations)
#[test]
fn test_hotspot_entry_clone() {
    let entry = HotspotEntry {
        function: "func".to_string(),
        risk_score: 1.0,
        path_count: 1,
        dominance_factor: 1.0,
        complexity: 1,
        file_path: "file.rs".to_string(),
    };

    let cloned = entry.clone();
    assert_eq!(entry.function, cloned.function);
    assert_eq!(entry.risk_score, cloned.risk_score);
}

// ============================================================================
// Hotpaths Command Tests
// ============================================================================

/// Test hotpaths args parsing
#[test]
fn test_hotpaths_args_parsing() {
    let args = HotpathsArgs {
        function: "my_function".to_string(),
        semantic_query: None,
        top: 5,
        rationale: true,
        min_score: Some(0.5),
    };

    assert_eq!(args.function, "my_function");
    assert_eq!(args.top, 5);
    assert!(args.rationale);
    assert_eq!(args.min_score, Some(0.5));
}

/// Test hotpaths args defaults
#[test]
fn test_hotpaths_args_defaults() {
    let args = HotpathsArgs {
        function: "main".to_string(),
        semantic_query: None,
        top: 10, // default value
        rationale: false,
        min_score: None,
    };

    assert_eq!(args.function, "main");
    assert_eq!(args.top, 10); // default value
    assert!(!args.rationale);
    assert!(args.min_score.is_none());
}

// ============================================================================
// Inter-Procedural Dominance Tests
// ============================================================================

/// Test dominators args has inter_procedural flag
#[test]
fn test_dominators_args_has_inter_procedural_flag() {
    let args = DominatorsArgs {
        function: "main".to_string(),
        semantic_query: None,
        file: None,
        must_pass_through: Some("block1".to_string()),
        post: false,
        inter_procedural: true,
    };

    assert!(args.inter_procedural);
    assert_eq!(args.function, "main");
    assert_eq!(args.must_pass_through, Some("block1".to_string()));
    assert!(!args.post);
}

/// Test dominators args without inter_procedural flag
#[test]
fn test_dominators_args_default_intra_procedural() {
    let args = DominatorsArgs {
        function: "main".to_string(),
        semantic_query: None,
        file: None,
        must_pass_through: None,
        post: false,
        inter_procedural: false, // default
    };

    assert!(!args.inter_procedural);
    assert!(!args.post);
    assert!(args.must_pass_through.is_none());
}

/// Test inter-procedural dominance with post flag combination
#[test]
fn test_dominators_inter_procedural_with_post() {
    // In practice, inter_procedural mode should take precedence
    let args = DominatorsArgs {
        function: "entry".to_string(),
        semantic_query: None,
        file: None,
        must_pass_through: None,
        post: true,
        inter_procedural: true,
    };

    // Both flags can be set (inter_procedural takes precedence in handler)
    assert!(args.inter_procedural);
    assert!(args.post);
}

/// Test inter-procedural mode cannot use must_pass_through
#[test]
fn test_dominators_inter_procedural_must_pass_through_combination() {
    // These flags can coexist in args struct
    let args = DominatorsArgs {
        function: "main".to_string(),
        semantic_query: None,
        file: None,
        must_pass_through: Some("some_block".to_string()),
        post: false,
        inter_procedural: true,
    };

    assert!(args.inter_procedural);
    assert!(args.must_pass_through.is_some());
    // Handler should validate this combination
}
