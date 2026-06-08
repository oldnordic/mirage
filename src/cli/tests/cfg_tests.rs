// ============================================================================

use crate::cfg::{export_dot, export_json};
use crate::cli::*;

/// Test that DOT format output contains expected Graphviz DOT syntax
#[test]
fn test_cfg_dot_format() {
    let cfg = cmds::create_test_cfg();
    let dot = export_dot(&cfg);

    // Verify basic Graphviz DOT structure
    assert!(
        dot.contains("digraph CFG"),
        "DOT output should contain 'digraph CFG'"
    );
    assert!(
        dot.contains("rankdir=TB"),
        "DOT output should contain rankdir attribute"
    );
    assert!(
        dot.contains("node [shape=box"),
        "DOT output should contain node shape attribute"
    );
    assert!(
        dot.contains("}"),
        "DOT output should end with closing brace"
    );

    // Verify edge syntax
    assert!(dot.contains("->"), "DOT output should contain edge arrows");
}

/// Test that JSON format output is valid and contains expected structure
#[test]
fn test_cfg_json_format() {
    let cfg = cmds::create_test_cfg();
    let function_name = "test_function";
    let export = export_json(&cfg, function_name, None);

    // Verify function name is included
    assert_eq!(
        export.function_name, function_name,
        "JSON export should include function name"
    );

    // Verify structure
    assert!(
        export.entry.is_some(),
        "JSON export should have an entry block"
    );
    assert!(
        !export.exits.is_empty(),
        "JSON export should have exit blocks"
    );
    assert!(!export.blocks.is_empty(), "JSON export should have blocks");
    assert!(!export.edges.is_empty(), "JSON export should have edges");

    // Verify JSON can be serialized
    let json_str = serde_json::to_string(&export);
    assert!(
        json_str.is_ok(),
        "JSON export should be serializable to JSON"
    );

    // Verify JSON contains function name
    let json = json_str.unwrap();
    assert!(
        json.contains(function_name),
        "JSON output should contain function name"
    );
    assert!(
        json.contains("\"entry\""),
        "JSON output should contain entry field"
    );
    assert!(
        json.contains("\"exits\""),
        "JSON output should contain exits field"
    );
    assert!(
        json.contains("\"blocks\""),
        "JSON output should contain blocks field"
    );
    assert!(
        json.contains("\"edges\""),
        "JSON output should contain edges field"
    );
}

/// Test that function name is correctly passed to export_json()
#[test]
fn test_cfg_function_name_in_export() {
    let cfg = cmds::create_test_cfg();

    // Test with different function names
    let test_names = vec!["my_function", "TestFunc", "module::submodule::function"];

    for name in test_names {
        let export = export_json(&cfg, name, None);
        assert_eq!(
            export.function_name, name,
            "Function name should be preserved in export"
        );
    }
}

/// Test format fallback when args.format is None (should use cli.output)
#[test]
fn test_cfg_format_fallback() {
    // Test that CfgFormat::Human is used when cli.output is Human
    let cli_human = Cli {
        db: None,
        output: OutputFormat::Human,
        command: Some(Commands::Cfg(CfgArgs {
            function: "test".to_string(),
            semantic_query: None,
            file: None,
            format: None,
        })),
        detect_backend: false,
        record: false,
    };

    let cfg_args = match &cli_human.command {
        Some(Commands::Cfg(args)) => args,
        _ => panic!("Expected Cfg command"),
    };

    // Simulate the format resolution logic from cfg()
    let resolved_format = cfg_args.format.unwrap_or(match cli_human.output {
        OutputFormat::Human => CfgFormat::Human,
        OutputFormat::Json => CfgFormat::Json,
        OutputFormat::Pretty => CfgFormat::Json,
    });

    assert_eq!(
        resolved_format,
        CfgFormat::Human,
        "Should fall back to Human format"
    );

    // Test that CfgFormat::Json is used when cli.output is Json
    let cli_json = Cli {
        db: None,
        output: OutputFormat::Json,
        command: Some(Commands::Cfg(CfgArgs {
            function: "test".to_string(),
            semantic_query: None,
            file: None,
            format: None,
        })),
        detect_backend: false,
        record: false,
    };

    let cfg_args_json = match &cli_json.command {
        Some(Commands::Cfg(args)) => args,
        _ => panic!("Expected Cfg command"),
    };

    let resolved_format_json = cfg_args_json.format.unwrap_or(match cli_json.output {
        OutputFormat::Human => CfgFormat::Human,
        OutputFormat::Json => CfgFormat::Json,
        OutputFormat::Pretty => CfgFormat::Json,
    });

    assert_eq!(
        resolved_format_json,
        CfgFormat::Json,
        "Should fall back to Json format"
    );
}

/// Test that JsonResponse wrapper wraps CFGExport correctly
#[test]
fn test_cfg_json_response_wrapper() {
    use crate::output::JsonResponse;

    let cfg = cmds::create_test_cfg();
    let export = export_json(&cfg, "wrapped_function", None);
    let response = JsonResponse::new(export);

    // Verify JsonResponse structure
    assert_eq!(response.schema_version, "1.0.1");
    assert_eq!(response.tool, "mirage");
    assert!(!response.execution_id.is_empty());
    assert!(!response.timestamp.is_empty());

    // Verify can be serialized
    let json = response.to_json();
    assert!(json.contains("\"schema_version\""));
    assert!(json.contains("\"execution_id\""));
    assert!(json.contains("\"tool\":\"mirage\""));
    assert!(json.contains("\"data\""));
    assert!(json.contains("wrapped_function"));
}

/// Test DOT format contains expected block information
#[test]
fn test_cfg_dot_block_info() {
    let cfg = cmds::create_test_cfg();
    let dot = export_dot(&cfg);

    // Check for ENTRY block marker (green fill)
    assert!(
        dot.contains("lightgreen"),
        "DOT should mark entry block with green"
    );

    // Check for EXIT block marker (coral fill)
    assert!(
        dot.contains("lightcoral"),
        "DOT should mark exit blocks with coral"
    );

    // Check for block labels
    assert!(dot.contains("Block"), "DOT should contain block labels");
}

/// Test DOT format contains expected edge information
#[test]
fn test_cfg_dot_edge_info() {
    let cfg = cmds::create_test_cfg();
    let dot = export_dot(&cfg);

    // Check for edge colors (TrueBranch=green, FalseBranch=red)
    assert!(
        dot.contains("color=green"),
        "DOT should show true branch edges in green"
    );
    assert!(
        dot.contains("color=red"),
        "DOT should show false branch edges in red"
    );
}
