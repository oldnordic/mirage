//! Risk/suggest agreement regression tests for the flag-gated coverage/churn
//! risk factors (the P2 severity-alignment invariant).
//!
//! The coverage-inverse and git-churn risk factors ported from
//! code-review-graph (MIT, Tirth Kanani 2026) are opt-in (`--coverage`,
//! `--churn`) and OFF by default. These tests prove that:
//!
//! - with both flags OFF, `mirage risk` and `mirage suggest` produce the
//!   exact same score and severity even on a coverage-instrumented database
//!   (the unconditional coverage term in the original WIP would have broken
//!   this);
//! - `--coverage` folds the uncovered-block ratio into the score when
//!   coverage rows exist, and is gracefully skipped when they don't;
//! - `--churn` counts real git commits for the function's file and folds the
//!   saturating churn term into the score.

use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Create a synthetic SQLite database with one function whose CFG has three
/// blocks (entry -> normal -> return). When `with_coverage_rows` is true,
/// blocks 1 and 2 get coverage rows with hits and block 3 gets none — i.e.
/// 1 of 3 blocks is uncovered. When false, the (empty) coverage table has no
/// rows for the function at all.
fn create_test_db(file_path: &str, with_coverage_rows: bool) -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

    conn.execute(
        "CREATE TABLE magellan_meta (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            magellan_schema_version INTEGER NOT NULL,
            sqlitegraph_schema_version INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        )",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
         VALUES (1, 11, 3, 0)",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE graph_entities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            file_path TEXT,
            data TEXT NOT NULL
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE cfg_blocks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            function_id INTEGER NOT NULL,
            kind TEXT NOT NULL,
            terminator TEXT NOT NULL,
            byte_start INTEGER NOT NULL,
            byte_end INTEGER NOT NULL,
            start_line INTEGER NOT NULL,
            start_col INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            end_col INTEGER NOT NULL,
            cfg_hash TEXT,
            statements TEXT,
            cfg_condition TEXT
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE cfg_block_coverage (
            block_id INTEGER PRIMARY KEY,
            hit_count INTEGER NOT NULL
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE cfg_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            function_id INTEGER NOT NULL,
            source_idx INTEGER NOT NULL,
            target_idx INTEGER NOT NULL,
            edge_type TEXT NOT NULL
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO graph_entities (kind, name, file_path, data)
         VALUES ('Symbol', 'test_function', ?1, '{\"kind\":\"Function\",\"symbol_id\":\"test_function_symbol\"}')",
        [file_path],
    )
    .unwrap();

    for (id, kind, terminator, byte_start, byte_end) in [
        (1, "entry", "fallthrough", 0, 10),
        (2, "normal", "conditional", 10, 50),
        (3, "return", "return", 50, 60),
    ] {
        conn.execute(
            "INSERT INTO cfg_blocks (id, function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?1, 1, ?2, ?3, ?4, ?5, 1, 0, 5, 10)",
            rusqlite::params![id, kind, terminator, byte_start, byte_end],
        )
        .unwrap();
    }

    if with_coverage_rows {
        // Block 1: 5 hits, block 2: 10 hits, block 3: no row (uncovered).
        conn.execute(
            "INSERT INTO cfg_block_coverage (block_id, hit_count) VALUES (1, 5)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO cfg_block_coverage (block_id, hit_count) VALUES (2, 10)",
            [],
        )
        .unwrap();
    }

    conn.execute(
        "INSERT INTO cfg_edges (function_id, source_idx, target_idx, edge_type)
         VALUES (1, 0, 1, 'fallthrough')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO cfg_edges (function_id, source_idx, target_idx, edge_type)
         VALUES (1, 1, 2, 'conditional_true')",
        [],
    )
    .unwrap();

    (dir, db_path)
}

/// Run the mirage binary against `db_path` in JSON mode and parse stdout.
fn run_mirage_json(db_path: &Path, args: &[&str]) -> serde_json::Value {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_mirage"))
        .arg("--db")
        .arg(db_path)
        .arg("--output")
        .arg("json")
        .args(args)
        .output()
        .expect("failed to run mirage");

    assert!(
        output.status.success(),
        "mirage {args:?} failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).expect("mirage should emit JSON")
}

fn risk_score(report: &serde_json::Value) -> f64 {
    report["data"]["risk_score"]
        .as_f64()
        .expect("risk_score must be a number")
}

#[test]
fn test_risk_suggest_agree_on_coverage_instrumented_db_flags_off() {
    let (_dir, db_path) = create_test_db("src/test.rs", true);

    let risk = run_mirage_json(&db_path, &["risk", "--function", "test_function"]);
    let suggest = run_mirage_json(&db_path, &["suggest", "--symbol", "test_function"]);

    assert_eq!(
        risk_score(&risk),
        risk_score(&suggest),
        "default risk score must match suggest score even on a \
         coverage-instrumented DB (P2 agreement invariant)"
    );
    assert_eq!(
        risk["data"]["risk_level"], suggest["data"]["overall_severity"],
        "default risk severity must match suggest severity"
    );

    // With both flags OFF the opt-in factors must not even be reported.
    let factors = &risk["data"]["factors"];
    assert!(
        factors.get("uncovered_ratio").is_none(),
        "uncovered_ratio must be absent without --coverage"
    );
    assert!(
        factors.get("churn_commits").is_none(),
        "churn_commits must be absent without --churn"
    );
}

#[test]
fn test_coverage_flag_folds_uncovered_ratio_into_score() {
    let (_dir, db_path) = create_test_db("src/test.rs", true);

    let base = run_mirage_json(&db_path, &["risk", "--function", "test_function"]);
    let cov = run_mirage_json(
        &db_path,
        &["risk", "--function", "test_function", "--coverage"],
    );

    // 1 of 3 blocks has zero hits -> ratio 1/3 -> +COVERAGE_WEIGHT(5.0)/3.
    let ratio = cov["data"]["factors"]["uncovered_ratio"]
        .as_f64()
        .expect("--coverage must report uncovered_ratio on an instrumented DB");
    assert!(
        (ratio - 1.0 / 3.0).abs() < 1e-9,
        "uncovered_ratio must be 1/3, got {ratio}"
    );
    let delta = risk_score(&cov) - risk_score(&base);
    assert!(
        (delta - 5.0 / 3.0).abs() < 1e-9,
        "--coverage must add uncovered_ratio * 5.0 to the score, got delta {delta}"
    );
}

#[test]
fn test_coverage_flag_graceful_without_coverage_rows() {
    let (_dir, db_path) = create_test_db("src/test.rs", false);

    let base = run_mirage_json(&db_path, &["risk", "--function", "test_function"]);
    let cov = run_mirage_json(
        &db_path,
        &["risk", "--function", "test_function", "--coverage"],
    );

    assert_eq!(
        risk_score(&base),
        risk_score(&cov),
        "--coverage on an uninstrumented function must not change the score"
    );
    assert!(
        cov["data"]["factors"].get("uncovered_ratio").is_none(),
        "uncovered_ratio must stay absent when no coverage rows exist"
    );
}

#[test]
fn test_churn_flag_counts_real_git_commits() {
    // Build a real git repo with 3 commits touching churned.rs.
    let dir = TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(args)
            .env("GIT_AUTHOR_NAME", "mirage-test")
            .env("GIT_AUTHOR_EMAIL", "mirage-test@example.com")
            .env("GIT_COMMITTER_NAME", "mirage-test")
            .env("GIT_COMMITTER_EMAIL", "mirage-test@example.com")
            .status()
            .expect("failed to spawn git");
        assert!(status.success(), "git {args:?} failed");
    };
    git(&["init", "-q"]);
    let file = repo.join("churned.rs");
    for i in 0..3 {
        std::fs::write(&file, format!("// rev {i}\n")).unwrap();
        git(&["add", "churned.rs"]);
        git(&["commit", "-q", "-m", &format!("rev {i}")]);
    }

    // The function's stored file path is the churned file; risk resolves it
    // from the graph when --file is not given.
    let (_dbdir, db_path) = create_test_db(file.to_str().unwrap(), true);

    let base = run_mirage_json(&db_path, &["risk", "--function", "test_function"]);
    let churn = run_mirage_json(
        &db_path,
        &["risk", "--function", "test_function", "--churn"],
    );

    assert_eq!(
        churn["data"]["factors"]["churn_commits"].as_u64(),
        Some(3),
        "--churn must count the 3 real commits touching the file"
    );
    // 3 commits / saturation 10 * CHURN_WEIGHT 4.0 = 1.2.
    let delta = risk_score(&churn) - risk_score(&base);
    assert!(
        (delta - 1.2).abs() < 1e-9,
        "--churn must add (commits/10).min(1) * 4.0 to the score, got delta {delta}"
    );
}
