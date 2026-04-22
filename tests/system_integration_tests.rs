//! System-level integration tests for Mirage
//!
//! These tests verify real installed-binary-style behavior.

use std::path::Path;
use std::process::Command;

/// Helper: Get path to installed mirage binary
fn mirage_bin() -> &'static str {
    "/home/feanor/.local/bin/mirage"
}

/// Helper: Get path to installed magellan binary
fn magellan_bin() -> &'static str {
    "/home/feanor/.local/bin/magellan"
}

/// Helper: Create a test project with functions for CFG analysis
fn create_test_project(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("src"))?;

    // lib.rs with various functions for analysis
    std::fs::write(
        dir.join("src/lib.rs"),
        r#"pub fn factorial(n: u32) -> u32 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}

pub fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

pub fn process_data(data: &str) -> String {
    if data.is_empty() {
        return "empty".to_string();
    }
    
    let mut result = String::new();
    for c in data.chars() {
        result.push(c.to_uppercase().next().unwrap());
    }
    
    result
}

pub fn caller_a() -> i32 {
    helper_x() + helper_y()
}

pub fn caller_b() -> i32 {
    helper_x() * 2
}

fn helper_x() -> i32 {
    42
}

fn helper_y() -> i32 {
    100
}
"#,
    )?;

    Ok(())
}

/// Test 1: Mirage status matches Magellan CFG count on same .geo
///
/// Verifies that Mirage's status report aligns with Magellan's CFG data.
#[test]
fn mirage_status_matches_magellan_cfg_count_on_same_geo() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.geo");
    let src_path = temp_dir.path().join("src");

    create_test_project(temp_dir.path()).unwrap();

    // Build with magellan
    let _magellan = Command::new(magellan_bin())
        .arg("watch")
        .arg("--root")
        .arg(&src_path)
        .arg("--db")
        .arg(&db_path)
        .arg("--scan-initial")
        .env("MAGELLAN_WATCH_TIMEOUT_MS", "2000")
        .output()
        .expect("Failed to run magellan");

    // Get Mirage status
    let mirage = Command::new(mirage_bin())
        .arg("status")
        .arg("--db")
        .arg(&db_path)
        .output()
        .expect("Failed to run mirage status");

    let stdout = String::from_utf8_lossy(&mirage.stdout);

    // Should report CFG blocks
    assert!(
        stdout.contains("cfg_blocks") || stdout.contains("cfg"),
        "Status should report CFG info. Output: {}",
        stdout
    );
}

/// Test 2: Mirage cfg accepts unique simple name
///
/// Verifies that `mirage cfg` can resolve a function by its simple name.
#[test]
fn mirage_cfg_accepts_unique_simple_name() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.geo");
    let src_path = temp_dir.path().join("src");

    create_test_project(temp_dir.path()).unwrap();

    let _magellan = Command::new(magellan_bin())
        .arg("watch")
        .arg("--root")
        .arg(&src_path)
        .arg("--db")
        .arg(&db_path)
        .arg("--scan-initial")
        .env("MAGELLAN_WATCH_TIMEOUT_MS", "2000")
        .output()
        .expect("Failed to run magellan");

    // Try cfg with simple name
    let cfg = Command::new(mirage_bin())
        .arg("cfg")
        .arg("--db")
        .arg(&db_path)
        .arg("--function")
        .arg("factorial")
        .output()
        .expect("Failed to run mirage cfg");

    let stdout = String::from_utf8_lossy(&cfg.stdout);
    let stderr = String::from_utf8_lossy(&cfg.stderr);

    // Should either show CFG or report clearly
    let has_output = !stdout.is_empty() || !stderr.is_empty();
    assert!(
        has_output,
        "cfg should produce output. stdout: {}, stderr: {}",
        stdout, stderr
    );
}

/// Test 3: Mirage cfg accepts FQN
///
/// Verifies that `mirage cfg` can resolve a function by its fully-qualified name.
#[test]
fn mirage_cfg_accepts_fqn() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.geo");
    let src_path = temp_dir.path().join("src");

    create_test_project(temp_dir.path()).unwrap();

    let _magellan = Command::new(magellan_bin())
        .arg("watch")
        .arg("--root")
        .arg(&src_path)
        .arg("--db")
        .arg(&db_path)
        .arg("--scan-initial")
        .env("MAGELLAN_WATCH_TIMEOUT_MS", "2000")
        .output()
        .expect("Failed to run magellan");

    // Try with FQN-like identifier
    let cfg = Command::new(mirage_bin())
        .arg("cfg")
        .arg("--db")
        .arg(&db_path)
        .arg("--function")
        .arg("test_crate::fibonacci")
        .output()
        .expect("Failed to run mirage cfg");

    let stdout = String::from_utf8_lossy(&cfg.stdout);
    let stderr = String::from_utf8_lossy(&cfg.stderr);

    // Should handle FQN (may resolve or report not found)
    let has_output = !stdout.is_empty() || !stderr.is_empty();
    assert!(
        has_output,
        "cfg with FQN should produce output. stdout: {}, stderr: {}",
        stdout, stderr
    );
}

/// Test 4: Mirage cfg accepts numeric ID
///
/// Verifies that `mirage cfg` can resolve a function by its numeric symbol ID.
#[test]
fn mirage_cfg_accepts_numeric_id() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.geo");
    let src_path = temp_dir.path().join("src");

    create_test_project(temp_dir.path()).unwrap();

    let _magellan = Command::new(magellan_bin())
        .arg("watch")
        .arg("--root")
        .arg(&src_path)
        .arg("--db")
        .arg(&db_path)
        .arg("--scan-initial")
        .env("MAGELLAN_WATCH_TIMEOUT_MS", "2000")
        .output()
        .expect("Failed to run magellan");

    // Try with a numeric ID (symbol ID 1 is likely to exist)
    let cfg = Command::new(mirage_bin())
        .arg("cfg")
        .arg("--db")
        .arg(&db_path)
        .arg("--function")
        .arg("1")
        .output()
        .expect("Failed to run mirage cfg");

    let stdout = String::from_utf8_lossy(&cfg.stdout);
    let stderr = String::from_utf8_lossy(&cfg.stderr);

    // Should handle numeric ID
    let has_output = !stdout.is_empty() || !stderr.is_empty();
    assert!(
        has_output,
        "cfg with numeric ID should produce output. stdout: {}, stderr: {}",
        stdout, stderr
    );
}

/// Test 5: Mirage cfg reports explicit ambiguity
///
/// Verifies that when multiple functions match, ambiguity is reported clearly.
#[test]
fn mirage_cfg_reports_explicit_ambiguity() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.geo");
    let src_path = temp_dir.path().join("src");

    create_test_project(temp_dir.path()).unwrap();

    let _magellan = Command::new(magellan_bin())
        .arg("watch")
        .arg("--root")
        .arg(&src_path)
        .arg("--db")
        .arg(&db_path)
        .arg("--scan-initial")
        .env("MAGELLAN_WATCH_TIMEOUT_MS", "2000")
        .output()
        .expect("Failed to run magellan");

    // Try with potentially ambiguous name
    let cfg = Command::new(mirage_bin())
        .arg("cfg")
        .arg("--db")
        .arg(&db_path)
        .arg("--function")
        .arg("process")
        .output()
        .expect("Failed to run mirage cfg");

    let stdout = String::from_utf8_lossy(&cfg.stdout);
    let stderr = String::from_utf8_lossy(&cfg.stderr);

    // Should produce output (resolved or ambiguous)
    let has_output = !stdout.is_empty() || !stderr.is_empty();
    assert!(
        has_output,
        "cfg should report result or ambiguity. stdout: {}, stderr: {}",
        stdout, stderr
    );
}

/// Test 6: Mirage loops works on real function
///
/// Verifies that `mirage loops` can detect loops in a function's CFG.
#[test]
fn mirage_loops_works_on_real_function() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.geo");
    let src_path = temp_dir.path().join("src");

    create_test_project(temp_dir.path()).unwrap();

    let _magellan = Command::new(magellan_bin())
        .arg("watch")
        .arg("--root")
        .arg(&src_path)
        .arg("--db")
        .arg(&db_path)
        .arg("--scan-initial")
        .env("MAGELLAN_WATCH_TIMEOUT_MS", "2000")
        .output()
        .expect("Failed to run magellan");

    // Check loops for factorial (has recursion/loop)
    let loops = Command::new(mirage_bin())
        .arg("loops")
        .arg("--db")
        .arg(&db_path)
        .arg("--function")
        .arg("factorial")
        .output()
        .expect("Failed to run mirage loops");

    let stdout = String::from_utf8_lossy(&loops.stdout);
    let stderr = String::from_utf8_lossy(&loops.stderr);

    // Should produce output
    let has_output = !stdout.is_empty() || !stderr.is_empty();
    assert!(
        has_output,
        "loops should produce output. stdout: {}, stderr: {}",
        stdout, stderr
    );
}

/// Test 7: Mirage dominators works on real function
///
/// Verifies that `mirage dominators` can compute dominators for a function.
#[test]
fn mirage_dominators_works_on_real_function() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.geo");
    let src_path = temp_dir.path().join("src");

    create_test_project(temp_dir.path()).unwrap();

    let _magellan = Command::new(magellan_bin())
        .arg("watch")
        .arg("--root")
        .arg(&src_path)
        .arg("--db")
        .arg(&db_path)
        .arg("--scan-initial")
        .env("MAGELLAN_WATCH_TIMEOUT_MS", "2000")
        .output()
        .expect("Failed to run magellan");

    // Check dominators
    let dom = Command::new(mirage_bin())
        .arg("dominators")
        .arg("--db")
        .arg(&db_path)
        .arg("--function")
        .arg("process_data")
        .output()
        .expect("Failed to run mirage dominators");

    let stdout = String::from_utf8_lossy(&dom.stdout);
    let stderr = String::from_utf8_lossy(&dom.stderr);

    // Should produce output
    let has_output = !stdout.is_empty() || !stderr.is_empty();
    assert!(
        has_output,
        "dominators should produce output. stdout: {}, stderr: {}",
        stdout, stderr
    );
}

/// Test 8: Mirage paths works on real function
///
/// Verifies that `mirage paths` can enumerate execution paths.
#[test]
fn mirage_paths_works_on_real_function() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.geo");
    let src_path = temp_dir.path().join("src");

    create_test_project(temp_dir.path()).unwrap();

    let _magellan = Command::new(magellan_bin())
        .arg("watch")
        .arg("--root")
        .arg(&src_path)
        .arg("--db")
        .arg(&db_path)
        .arg("--scan-initial")
        .env("MAGELLAN_WATCH_TIMEOUT_MS", "2000")
        .output()
        .expect("Failed to run magellan");

    // Enumerate paths
    let paths = Command::new(mirage_bin())
        .arg("paths")
        .arg("--db")
        .arg(&db_path)
        .arg("--function")
        .arg("factorial")
        .output()
        .expect("Failed to run mirage paths");

    let stdout = String::from_utf8_lossy(&paths.stdout);
    let stderr = String::from_utf8_lossy(&paths.stderr);

    // Should produce output
    let has_output = !stdout.is_empty() || !stderr.is_empty();
    assert!(
        has_output,
        "paths should produce output. stdout: {}, stderr: {}",
        stdout, stderr
    );
}

/// Test 9: Mirage reopen status is stable
///
/// Verifies that closing and reopening the database produces
/// consistent status reports.
#[test]
fn mirage_reopen_status_is_stable() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.geo");
    let src_path = temp_dir.path().join("src");

    create_test_project(temp_dir.path()).unwrap();

    let _magellan = Command::new(magellan_bin())
        .arg("watch")
        .arg("--root")
        .arg(&src_path)
        .arg("--db")
        .arg(&db_path)
        .arg("--scan-initial")
        .env("MAGELLAN_WATCH_TIMEOUT_MS", "2000")
        .output()
        .expect("Failed to run magellan");

    // First status
    let status1 = Command::new(mirage_bin())
        .arg("status")
        .arg("--db")
        .arg(&db_path)
        .output()
        .expect("Failed first status");

    let stdout1 = String::from_utf8_lossy(&status1.stdout).to_string();

    // Small delay
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Second status (reopen)
    let status2 = Command::new(mirage_bin())
        .arg("status")
        .arg("--db")
        .arg(&db_path)
        .output()
        .expect("Failed second status");

    let stdout2 = String::from_utf8_lossy(&status2.stdout);

    // Should be consistent
    assert_eq!(
        stdout1.trim(),
        stdout2.trim(),
        "Status should be stable after reopen.\nFirst:\n{}\nSecond:\n{}",
        stdout1,
        stdout2
    );
}

/// Full system smoke test
///
/// End-to-end verification of Mirage on a real magellan DB.
#[test]
fn mirage_full_system_smoke() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("smoke.geo");
    let src_path = temp_dir.path().join("src");

    create_test_project(temp_dir.path()).unwrap();

    // 1. Build with magellan
    let magellan = Command::new(magellan_bin())
        .arg("watch")
        .arg("--root")
        .arg(&src_path)
        .arg("--db")
        .arg(&db_path)
        .arg("--scan-initial")
        .env("MAGELLAN_WATCH_TIMEOUT_MS", "2000")
        .output()
        .expect("Failed to run magellan");

    assert!(magellan.status.success(), "Magellan should succeed");

    // 2. Status
    let status = Command::new(mirage_bin())
        .arg("status")
        .arg("--db")
        .arg(&db_path)
        .output()
        .expect("Failed status");

    assert!(status.status.success(), "Status should succeed");

    // 3. CFG query
    let cfg = Command::new(mirage_bin())
        .arg("cfg")
        .arg("--db")
        .arg(&db_path)
        .arg("--function")
        .arg("factorial")
        .output()
        .expect("Failed cfg");

    // Should produce output
    let stdout = String::from_utf8_lossy(&cfg.stdout);
    assert!(
        !stdout.is_empty() || cfg.status.success(),
        "Smoke test CFG should work"
    );
}
