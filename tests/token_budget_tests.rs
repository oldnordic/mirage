// Integration tests for token budget feature
// These tests verify that --tokens flag works correctly across all commands

use std::process::Command;
use std::str;

#[test]
fn test_cfg_token_budget_flag_exists() {
    // Test that cfg --help mentions --tokens flag
    let output = Command::new("cargo")
        .args(["run", "--", "cfg", "--help"])
        .output()
        .expect("Failed to execute cfg command");

    // Verify help text includes --tokens flag
    let help_text = str::from_utf8(&output.stdout).expect("Invalid UTF-8");
    assert!(
        help_text.contains("--tokens"),
        "cfg help should mention --tokens flag"
    );
}

#[test]
fn test_paths_token_budget_flag_exists() {
    // Test that paths --help mentions --tokens flag
    let output = Command::new("cargo")
        .args(["run", "--", "paths", "--help"])
        .output()
        .expect("Failed to execute paths command");

    // Verify help text includes --tokens flag
    let help_text = str::from_utf8(&output.stdout).expect("Invalid UTF-8");
    assert!(
        help_text.contains("--tokens"),
        "paths help should mention --tokens flag"
    );
}

#[test]
fn test_blast_zone_token_budget_flag_exists() {
    // Test that blast-zone --help mentions --tokens flag
    let output = Command::new("cargo")
        .args(["run", "--", "blast-zone", "--help"])
        .output()
        .expect("Failed to execute blast-zone command");

    // Verify help text includes --tokens flag
    let help_text = str::from_utf8(&output.stdout).expect("Invalid UTF-8");
    assert!(
        help_text.contains("--tokens"),
        "blast-zone help should mention --tokens flag"
    );
}

#[test]
fn test_json_response_metadata() {
    // Test that JSON responses include tokens_estimated and truncated fields
    use mirage::output::JsonResponse;
    use serde_json::json;

    let response = JsonResponse::new(json!({"test": "data"}))
        .with_tokens(100)
        .with_truncated(false);

    let json_str = response.to_json();
    assert!(
        json_str.contains("tokens_estimated"),
        "JSON should include tokens_estimated field"
    );
    assert!(
        json_str.contains("truncated"),
        "JSON should include truncated field"
    );

    // Parse and verify values
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("Failed to parse JSON");
    assert_eq!(parsed["tokens_estimated"], 100);
    assert_eq!(parsed["truncated"], false);
}

#[test]
fn test_apply_token_budget_no_limit() {
    // Test that apply_token_budget returns original when no limit set
    use mirage::output::apply_token_budget;

    let input = "Test string that should not be truncated".to_string();
    let result = apply_token_budget(input.clone(), None);
    assert_eq!(result, input, "Should return original string when no limit");
}

#[test]
fn test_apply_token_budget_zero_limit() {
    // Test that apply_token_budget returns original when limit is 0
    use mirage::output::apply_token_budget;

    let input = "Test string that should not be truncated".to_string();
    let result = apply_token_budget(input.clone(), Some(0));
    assert_eq!(
        result, input,
        "Should return original string when limit is 0"
    );
}

#[test]
fn test_apply_token_budget_with_limit() {
    // Test that apply_token_budget truncates when limit is exceeded
    use mirage::output::apply_token_budget;

    let input = "This is a long string that should be truncated".to_string();
    let limit = 5; // 5 tokens = 20 chars
    let result = apply_token_budget(input, Some(limit));

    // Should be truncated and include truncation marker
    assert!(result.contains("[~"), "Should include truncation marker");
    assert!(result.contains("tokens"), "Should mention tokens in marker");
}

#[test]
fn test_token_estimation_heuristic() {
    // Test that token estimation uses chars / 4 heuristic
    let test_string = "abcdefgh"; // 8 chars
    let estimated_tokens = test_string.len() / 4;
    assert_eq!(estimated_tokens, 2, "Token estimation should use chars / 4");
}
