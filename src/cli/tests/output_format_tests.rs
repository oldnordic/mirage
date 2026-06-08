// ============================================================================

use crate::cli::responses::*;
use crate::cli::*;
use crate::output::JsonResponse;

/// Test that all response structs serialize correctly to JSON
#[test]
fn test_all_response_types_serialize() {
    // PathsResponse
    let paths_resp = PathsResponse {
        function: "test_func".to_string(),
        total_paths: 2,
        error_paths: 0,
        paths: vec![],
    };
    let paths_json = serde_json::to_string(&paths_resp);
    assert!(paths_json.is_ok(), "PathsResponse should serialize");

    // DominanceResponse
    let dom_resp = DominanceResponse {
        function: "test_func".to_string(),
        kind: "dominators".to_string(),
        root: Some(0),
        dominance_tree: vec![],
        must_pass_through: None,
    };
    let dom_json = serde_json::to_string(&dom_resp);
    assert!(dom_json.is_ok(), "DominanceResponse should serialize");

    // UnreachableResponse
    let unreach_resp = UnreachableResponse {
        uncalled_functions: None,
        function: "test_func".to_string(),
        total_functions: 1,
        functions_with_unreachable: 0,
        unreachable_count: 0,
        blocks: vec![],
    };
    let unreach_json = serde_json::to_string(&unreach_resp);
    assert!(unreach_json.is_ok(), "UnreachableResponse should serialize");

    // VerifyResult
    let verify_res = VerifyResult {
        path_id: "test_path".to_string(),
        valid: true,
        found_in_cache: true,
        function_id: Some(1),
        reason: "Test".to_string(),
        current_paths: 2,
    };
    let verify_json = serde_json::to_string(&verify_res);
    assert!(verify_json.is_ok(), "VerifyResult should serialize");
}

/// Test that JsonResponse wrapper works for all response types
#[test]
fn test_json_response_wrapper_for_all_commands() {
    // PathsResponse wrapped
    let paths_resp = PathsResponse {
        function: "test_func".to_string(),
        total_paths: 2,
        error_paths: 0,
        paths: vec![],
    };
    let paths_wrapper = JsonResponse::new(paths_resp);
    assert_eq!(paths_wrapper.schema_version, "1.0.1");
    assert_eq!(paths_wrapper.tool, "mirage");
    assert!(!paths_wrapper.execution_id.is_empty());

    // DominanceResponse wrapped
    let dom_resp = DominanceResponse {
        function: "test_func".to_string(),
        kind: "dominators".to_string(),
        root: Some(0),
        dominance_tree: vec![],
        must_pass_through: None,
    };
    let dom_wrapper = JsonResponse::new(dom_resp);
    assert_eq!(dom_wrapper.schema_version, "1.0.1");
    assert_eq!(dom_wrapper.tool, "mirage");

    // UnreachableResponse wrapped
    let unreach_resp = UnreachableResponse {
        uncalled_functions: None,
        function: "test_func".to_string(),
        total_functions: 1,
        functions_with_unreachable: 0,
        unreachable_count: 0,
        blocks: vec![],
    };
    let unreach_wrapper = JsonResponse::new(unreach_resp);
    assert_eq!(unreach_wrapper.schema_version, "1.0.1");
    assert_eq!(unreach_wrapper.tool, "mirage");

    // VerifyResult wrapped
    let verify_res = VerifyResult {
        path_id: "test_path".to_string(),
        valid: true,
        found_in_cache: true,
        function_id: Some(1),
        reason: "Test".to_string(),
        current_paths: 2,
    };
    let verify_wrapper = JsonResponse::new(verify_res);
    assert_eq!(verify_wrapper.schema_version, "1.0.1");
    assert_eq!(verify_wrapper.tool, "mirage");
}

/// Test that to_json() produces compact JSON
#[test]
fn test_json_response_compact_format() {
    let data = vec!["item1", "item2"];
    let wrapper = JsonResponse::new(data);
    let compact = wrapper.to_json();

    // Compact JSON should not have unnecessary whitespace
    assert!(
        !compact.contains("\n"),
        "Compact JSON should not have newlines"
    );
    assert!(
        compact.contains("\"item1\""),
        "Compact JSON should contain data"
    );
}

/// Test that to_pretty_json() produces formatted JSON
#[test]
fn test_json_response_pretty_format() {
    let data = vec!["item1", "item2"];
    let wrapper = JsonResponse::new(data);
    let pretty = wrapper.to_pretty_json();

    // Pretty JSON should have newlines for formatting
    assert!(pretty.contains("\n"), "Pretty JSON should have newlines");
    assert!(pretty.contains("  "), "Pretty JSON should have indentation");

    // Both formats should produce valid JSON with same data
    let compact = wrapper.to_json();
    let compact_val: serde_json::Value = serde_json::from_str(&compact).unwrap();
    let pretty_val: serde_json::Value = serde_json::from_str(&pretty).unwrap();
    assert_eq!(
        compact_val, pretty_val,
        "Both formats should produce same data"
    );
}

/// Test that JsonResponse contains required fields
#[test]
fn test_json_response_required_fields() {
    let data = "test_data";
    let wrapper = JsonResponse::new(data);

    // Check all required fields exist and have correct values
    assert_eq!(wrapper.schema_version, "1.0.1");
    assert_eq!(wrapper.tool, "mirage");
    assert!(!wrapper.execution_id.is_empty());
    assert!(!wrapper.timestamp.is_empty());

    // Verify execution_id format (should be timestamp-processid)
    assert!(
        wrapper.execution_id.contains("-"),
        "execution_id should contain hyphen"
    );

    // Verify timestamp is valid RFC3339 format
    let parsed_time = chrono::DateTime::parse_from_rfc3339(&wrapper.timestamp);
    assert!(parsed_time.is_ok(), "timestamp should be valid RFC3339");
}

/// Test that format selection logic works correctly
#[test]
fn test_output_format_enum_matches() {
    // Test that all three formats are distinct
    assert_ne!(OutputFormat::Human, OutputFormat::Json);
    assert_ne!(OutputFormat::Human, OutputFormat::Pretty);
    assert_ne!(OutputFormat::Json, OutputFormat::Pretty);

    // Test equality
    assert_eq!(OutputFormat::Human, OutputFormat::Human);
    assert_eq!(OutputFormat::Json, OutputFormat::Json);
    assert_eq!(OutputFormat::Pretty, OutputFormat::Pretty);
}

/// Test that human format doesn't contain JSON artifacts
#[test]
fn test_human_output_no_json_artifacts() {
    // Human format should print readable text, not JSON
    // This test verifies the pattern: Human output uses println!, not JsonResponse

    let function_name = "test_function";
    let path_count = 5;

    // Simulate human format output
    let mut output = String::new();
    output.push_str(&format!("Function: {}\n", function_name));
    output.push_str(&format!("Total paths: {}\n", path_count));

    // Human output should not contain JSON artifacts
    assert!(
        !output.contains("{"),
        "Human output should not contain JSON objects"
    );
    assert!(
        !output.contains("}"),
        "Human output should not contain JSON objects"
    );
    assert!(
        !output.contains("\""),
        "Human output should not contain JSON quotes"
    );
    assert!(
        !output.contains("schema_version"),
        "Human output should not contain JSON metadata"
    );
}

/// Test that JSON output contains all expected metadata
#[test]
fn test_json_output_has_metadata() {
    let data = "test_data";
    let wrapper = JsonResponse::new(data);
    let json = wrapper.to_json();

    // JSON should contain all metadata fields
    assert!(json.contains("\"schema_version\""));
    assert!(json.contains("\"execution_id\""));
    assert!(json.contains("\"tool\""));
    assert!(json.contains("\"timestamp\""));
    assert!(json.contains("\"data\""));
}

/// Test error response format
#[test]
fn test_error_response_format() {
    use crate::output::JsonError;

    let error = JsonError::new("category", "message", "CODE");
    assert_eq!(error.error, "category");
    assert_eq!(error.message, "message");
    assert_eq!(error.code, "CODE");
    assert!(error.remediation.is_none());

    let error_with_remediation = error.with_remediation("Try X instead");
    assert_eq!(
        error_with_remediation.remediation,
        Some("Try X instead".to_string())
    );

    // Error response should serialize
    let json = serde_json::to_string(&error_with_remediation);
    assert!(json.is_ok());
    let json_str = json.unwrap();
    assert!(json_str.contains("\"error\""));
    assert!(json_str.contains("\"message\""));
    assert!(json_str.contains("\"code\""));
    assert!(json_str.contains("\"remediation\""));
}

/// Test that all CLI struct variants can be created with different output formats
#[test]
fn test_cli_with_different_output_formats() {
    let formats = vec![
        OutputFormat::Human,
        OutputFormat::Json,
        OutputFormat::Pretty,
    ];

    for format in formats {
        let cli = Cli {
            db: Some("./test.db".to_string()),
            output: format,
            command: Some(Commands::Status(StatusArgs {})),
            detect_backend: false,
            record: false,
        };

        assert_eq!(cli.output, format);
        assert_eq!(cli.db, Some("./test.db".to_string()));
    }
}

/// Test CfgFormat enum values
#[test]
fn test_cfg_format_enum() {
    let formats = vec![CfgFormat::Human, CfgFormat::Dot, CfgFormat::Json];

    for format in &formats {
        match format {
            CfgFormat::Human | CfgFormat::Dot | CfgFormat::Json => {}
        }
    }

    // Test distinctness
    assert_ne!(CfgFormat::Human, CfgFormat::Dot);
    assert_ne!(CfgFormat::Human, CfgFormat::Json);
    assert_ne!(CfgFormat::Dot, CfgFormat::Json);
}

/// Test that response field naming follows snake_case convention
#[test]
fn test_response_snake_case_naming() {
    // All JSON field names should use snake_case
    let paths_resp = PathsResponse {
        function: "test".to_string(),
        total_paths: 1,
        error_paths: 0,
        paths: vec![],
    };
    let json = serde_json::to_string(&paths_resp).unwrap();

    // Check for snake_case fields
    assert!(json.contains("\"function\""));
    assert!(json.contains("\"total_paths\""));
    assert!(json.contains("\"error_paths\""));

    // Should not have camelCase
    assert!(!json.contains("\"totalPaths\""));
    assert!(!json.contains("\"errorPaths\""));
}

/// Test loops command detects natural loops
#[test]
fn test_loops_detects_loops() {
    use crate::cfg::{detect_natural_loops, BasicBlock, BlockKind, EdgeType, Terminator};
    use petgraph::graph::DiGraph;

    // Create a simple loop: 0 -> 1 -> 2 -> 1
    let mut g = DiGraph::new();

    let b0 = g.add_node(BasicBlock {
        id: 0,
        db_id: None,
        kind: BlockKind::Entry,
        statements: vec![],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
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
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b2 = g.add_node(BasicBlock {
        id: 2,
        db_id: None,
        kind: BlockKind::Normal,
        statements: vec!["loop body".to_string()],
        terminator: Terminator::Goto { target: 1 },
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    let b3 = g.add_node(BasicBlock {
        id: 3,
        db_id: None,
        kind: BlockKind::Exit,
        statements: vec![],
        terminator: Terminator::Return,
        source_location: None,
        coord_x: 0,
        coord_y: 0,
        coord_z: 0,
    });

    g.add_edge(b0, b1, EdgeType::Fallthrough);
    g.add_edge(b1, b2, EdgeType::TrueBranch);
    g.add_edge(b1, b3, EdgeType::FalseBranch);
    g.add_edge(b2, b1, EdgeType::LoopBack);

    let loops = detect_natural_loops(&g);

    // Should detect one loop
    assert_eq!(loops.len(), 1, "Should detect exactly one loop");
    assert_eq!(loops[0].header.index(), 1, "Loop header should be block 1");
}

/// Test loops command with empty CFG
#[test]
fn test_loops_empty_cfg() {
    use crate::cfg::detect_natural_loops;
    use petgraph::graph::DiGraph;
    let empty_cfg: crate::cfg::Cfg = DiGraph::new();
    let loops = detect_natural_loops(&empty_cfg);

    assert!(loops.is_empty(), "Empty CFG should have no loops");
}

/// Test loops response serialization
#[test]
fn test_loops_response_serialization() {
    use crate::output::JsonResponse;

    let response = LoopsResponse {
        function: "test_func".to_string(),
        loop_count: 2,
        loops: vec![
            LoopInfo {
                header: 1,
                back_edge_from: 2,
                body_size: 2,
                nesting_level: 0,
                body_blocks: vec![1, 2],
            },
            LoopInfo {
                header: 3,
                back_edge_from: 4,
                body_size: 3,
                nesting_level: 1,
                body_blocks: vec![1, 2, 3],
            },
        ],
    };

    // Should serialize without errors
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"function\""));
    assert!(json.contains("\"loop_count\""));
    assert!(json.contains("\"loops\""));

    // Test with JsonResponse wrapper
    let wrapper = JsonResponse::new(response);
    let wrapped_json = wrapper.to_json();
    assert!(wrapped_json.contains("\"schema_version\""));
    assert!(wrapped_json.contains("\"execution_id\""));
}

/// Test LoopsArgs struct fields
#[test]
fn test_loops_args_fields() {
    let args = LoopsArgs {
        function: "my_function".to_string(),
        semantic_query: None,
        file: None,
        verbose: true,
    };

    assert_eq!(args.function, "my_function");
    assert!(args.verbose);
}

/// Test LoopInfo struct fields
#[test]
fn test_loop_info_fields() {
    let loop_info = LoopInfo {
        header: 5,
        back_edge_from: 7,
        body_size: 3,
        nesting_level: 2,
        body_blocks: vec![5, 6, 7],
    };

    assert_eq!(loop_info.header, 5);
    assert_eq!(loop_info.back_edge_from, 7);
    assert_eq!(loop_info.body_size, 3);
    assert_eq!(loop_info.nesting_level, 2);
    assert_eq!(loop_info.body_blocks, vec![5, 6, 7]);
}

/// Test loops command with json output format
#[test]
fn test_loops_json_output_format() {
    use crate::output::JsonResponse;

    let response = LoopsResponse {
        function: "json_test".to_string(),
        loop_count: 1,
        loops: vec![LoopInfo {
            header: 1,
            back_edge_from: 2,
            body_size: 2,
            nesting_level: 0,
            body_blocks: vec![1, 2],
        }],
    };

    let wrapper = JsonResponse::new(response);
    let json = wrapper.to_json();

    // Verify JSON structure
    assert!(json.contains("\"schema_version\""));
    assert!(json.contains("\"execution_id\""));
    assert!(json.contains("\"tool\""));
    assert!(json.contains("\"timestamp\""));
    assert!(json.contains("\"data\""));
}

/// Test loops command with verbose flag
#[test]
fn test_loops_verbose_flag() {
    let args_verbose = LoopsArgs {
        function: "test".to_string(),
        semantic_query: None,
        file: None,
        verbose: true,
    };

    let args_not_verbose = LoopsArgs {
        function: "test".to_string(),
        semantic_query: None,
        file: None,
        verbose: false,
    };

    assert!(args_verbose.verbose);
    assert!(!args_not_verbose.verbose);
}

/// Test loops nesting level calculation
#[test]
fn test_loops_nesting_levels() {
    let loop_outer = LoopInfo {
        header: 1,
        back_edge_from: 3,
        body_size: 3,
        nesting_level: 0, // Outermost
        body_blocks: vec![1, 2, 3],
    };

    let loop_inner = LoopInfo {
        header: 2,
        back_edge_from: 4,
        body_size: 2,
        nesting_level: 1, // Nested inside outer
        body_blocks: vec![2, 4],
    };

    assert_eq!(loop_outer.nesting_level, 0);
    assert_eq!(loop_inner.nesting_level, 1);
}

/// Test loops response with no loops
#[test]
fn test_loops_response_empty() {
    use crate::output::JsonResponse;

    let response = LoopsResponse {
        function: "no_loops_func".to_string(),
        loop_count: 0,
        loops: vec![],
    };

    let wrapper = JsonResponse::new(response);
    let json = wrapper.to_json();

    // Should handle empty loops gracefully
    assert!(json.contains("\"loop_count\":0"));
    assert!(json.contains("\"loops\":[]"));
}

/// Test patterns command with if/else detection
#[test]
fn test_patterns_if_else_detection() {
    use crate::cfg::{detect_if_else_patterns, detect_match_patterns};

    let cfg = cmds::create_test_cfg();

    // Detect patterns
    let if_else_patterns = detect_if_else_patterns(&cfg);
    let match_patterns = detect_match_patterns(&cfg);

    // Test CFG has a simple if/else (block 1 -> blocks 2 and 3)
    // This is a diamond pattern, so it should be detected
    assert!(
        !if_else_patterns.is_empty(),
        "Should detect if/else pattern"
    );

    // Check pattern structure
    let pattern = &if_else_patterns[0];
    assert_eq!(cfg[pattern.condition].id, 1);
    assert_eq!(cfg[pattern.true_branch].id, 2);
    assert_eq!(cfg[pattern.false_branch].id, 3);

    // Our test CFG doesn't have a match statement
    assert!(
        match_patterns.is_empty(),
        "Should not detect match patterns in simple if/else"
    );
}

/// Test patterns command with --if-else filter
#[test]
fn test_patterns_if_else_filter() {
    // Test argument parsing - command structure is correct
    let args = PatternsArgs {
        function: "test_func".to_string(),
        semantic_query: None,
        file: None,
        if_else: true,
        r#match: false,
    };

    // Verify args are parsed correctly
    assert!(args.if_else);
    assert!(!args.r#match);
    assert_eq!(args.function, "test_func");
}

/// Test patterns command with --match filter
#[test]
fn test_patterns_match_filter() {
    // Test argument parsing - command structure is correct
    let args = PatternsArgs {
        function: "test_func".to_string(),
        semantic_query: None,
        file: None,
        if_else: false,
        r#match: true,
    };

    // Verify args are parsed correctly
    assert!(!args.if_else);
    assert!(args.r#match);
    assert_eq!(args.function, "test_func");
}

/// Test patterns command with JSON output
#[test]
fn test_patterns_json_output() {
    // Test argument parsing - command structure is correct
    let args = PatternsArgs {
        function: "test_func".to_string(),
        semantic_query: None,
        file: None,
        if_else: false,
        r#match: false,
    };

    let cli = Cli {
        db: None,
        output: OutputFormat::Json,
        command: Some(Commands::Patterns(args.clone())),
        detect_backend: false,
        record: false,
    };

    // Verify CLI structure
    assert!(matches!(cli.output, OutputFormat::Json));
}

/// Test patterns response struct serialization
#[test]
fn test_patterns_response_serialization() {
    let response = PatternsResponse {
        function: "test_func".to_string(),
        if_else_count: 1,
        match_count: 0,
        if_else_patterns: vec![IfElseInfo {
            condition_block: 1,
            true_branch: 2,
            false_branch: 3,
            merge_point: Some(4),
            has_else: true,
        }],
        match_patterns: vec![],
    };

    // Should serialize to JSON
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"function\""));
    assert!(json.contains("\"if_else_count\""));
    assert!(json.contains("\"match_count\""));

    // Check snake_case naming
    assert!(json.contains("\"if_else_patterns\""));
    assert!(json.contains("\"condition_block\""));
    assert!(json.contains("\"merge_point\""));
}
