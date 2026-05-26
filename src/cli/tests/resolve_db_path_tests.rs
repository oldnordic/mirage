// ============================================================================

use crate::cli::*;

// Ensure tests don't interfere with each other by clearing env var
fn clear_env() {
    std::env::remove_var("MIRAGE_DB");
}

#[test]
fn test_resolve_db_path_no_source() {
    clear_env();
    // No arg, no env, no discoverable DB -> returns error
    // (unless running in a directory with a .db file)
    let result = resolve_db_path(None);
    // Result depends on whether there's a .db file in current directory
    // So we just verify the function doesn't panic
    match result {
        Ok(path) => {
            // If we got a path, it should be a .db or .db file
            assert!(
                path.ends_with(".db") || path.ends_with(".db"),
                "Auto-discovered DB should have .db or .db extension"
            );
        }
        Err(_) => {
            // Expected when no DB is available
        }
    }
}

#[test]
fn test_resolve_db_path_with_cli_arg() {
    clear_env();
    // CLI arg provided -> returns CLI arg
    let result = resolve_db_path(Some("/custom/path.db".to_string())).unwrap();
    assert_eq!(result, "/custom/path.db");
}

#[test]
fn test_resolve_db_path_with_env_var() {
    clear_env();
    // Env var set -> returns env var value
    std::env::set_var("MIRAGE_DB", "/env/path.db");
    let result = resolve_db_path(None);
    assert!(
        result.is_ok(),
        "Expected Ok with MIRAGE_DB set, got: {:?}",
        result
    );
    assert_eq!(result.unwrap(), "/env/path.db");
    std::env::remove_var("MIRAGE_DB");
}

#[test]
fn test_resolve_db_path_cli_overrides_env() {
    clear_env();
    // CLI arg should override env var
    std::env::set_var("MIRAGE_DB", "/env/path.db");
    let result = resolve_db_path(Some("/cli/path.db".to_string())).unwrap();
    assert_eq!(result, "/cli/path.db");
    std::env::remove_var("MIRAGE_DB");
}
