//! Regression tests for E001 honest database-open errors (inventory plan B1).
//!
//! Root cause being guarded against: command handlers used to bind
//! `MirageDb::open` failures as `Err(_e)`, discard the specific schema-drift
//! error from `validate_schema_sqlite`, and unconditionally emit
//! `JsonError::database_not_found` ("Run 'magellan watch'") with exit 3 —
//! even when the file existed and the real cause was a schema version
//! mismatch. Every handler must now forward the real open error:
//!   - file truly missing  -> DatabaseNotFound + remediation (honest)
//!   - schema drift / other -> DatabaseOpenFailed carrying the underlying
//!     `validate_schema_sqlite` message (names schema + version)
//!
//! The drifted fixture mirrors the investigator's reproduction in
//! docs/INVENTORY_PLAN.md (B1): a database whose magellan_meta declares
//! schema version 5, older than MIN_MAGELLAN_SCHEMA_VERSION (7).

use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

/// Create a schema-drifted database: the file exists and is a valid SQLite
/// database, but its Magellan schema version (5) is below the minimum (7).
fn create_drifted_db() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("drifted.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();

    conn.execute(
        "CREATE TABLE magellan_meta (
            id INTEGER PRIMARY KEY,
            magellan_schema_version INTEGER NOT NULL,
            sqlitegraph_schema_version INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        )",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO magellan_meta VALUES (1, 5, 4, 0)", [])
        .unwrap();
    conn.execute("CREATE TABLE cfg_blocks (id INTEGER PRIMARY KEY)", [])
        .unwrap();
    conn.execute(
        "CREATE TABLE symbols (id INTEGER PRIMARY KEY, name TEXT)",
        [],
    )
    .unwrap();
    conn.execute("CREATE TABLE edges (a INTEGER, b INTEGER)", [])
        .unwrap();

    (dir, db_path)
}

fn mirage() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mirage"))
}

/// Sanity check on the fixture itself: the storage layer must reject the
/// drifted database with the specific schema-version error (this is the
/// message handlers are required to forward).
#[test]
fn test_drifted_db_open_error_names_schema_version() {
    let (_dir, db_path) = create_drifted_db();

    let err =
        mirage::storage::MirageDb::open(&db_path).expect_err("drifted database must fail to open");
    let msg = format!("{:#}", err);

    assert!(
        msg.contains("schema version 5 is too old (minimum 7)"),
        "open error must name schema and version, got: {}",
        msg
    );
    assert!(
        !msg.contains("not found"),
        "open error must not claim 'not found' for an existing file, got: {}",
        msg
    );
}

/// Every command handler must report a schema-drifted database as
/// DatabaseOpenFailed naming the real cause — never DatabaseNotFound.
#[test]
fn test_all_handlers_report_schema_drift_honestly_json() {
    let (_dir, db_path) = create_drifted_db();
    let db = db_path.to_str().unwrap();

    // (subcommand args). Every one of these reaches a MirageDb::open (or the
    // diff handler's Backend::detect_and_open) before doing any real work.
    let cases: Vec<Vec<&str>> = vec![
        vec!["status"],
        vec!["stats"],
        vec!["docs"],
        vec!["unreachable"],
        vec!["cfg", "--function", "foo"],
        vec!["paths", "--function", "foo"],
        vec!["dominators", "--function", "foo"],
        vec!["loops", "--function", "foo"],
        vec!["patterns", "--function", "foo"],
        vec!["frontiers", "--function", "foo"],
        vec!["hotpaths", "--function", "foo"],
        vec!["coverage", "--function", "foo"],
        vec!["risk", "--function", "foo"],
        vec!["suggest", "--symbol", "foo"],
        vec!["blast-zone", "--function", "foo"],
        vec!["verify", "--path-id", "p1"],
        vec!["icfg", "--entry", "foo"],
        vec!["cycles", "--function-loops"],
    ];

    for args in &cases {
        let output = mirage()
            .args(["--db", db, "--output", "json"])
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("failed to run mirage {:?}: {}", args, e));

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(3),
            "mirage {:?} on drifted DB must exit 3 (EXIT_DATABASE).\nstdout: {}\nstderr: {}",
            args,
            stdout,
            stderr
        );
        assert!(
            stdout.contains("DatabaseOpenFailed"),
            "mirage {:?} on drifted DB must report DatabaseOpenFailed.\nstdout: {}",
            args,
            stdout
        );
        assert!(
            stdout.contains("schema version 5 is too old"),
            "mirage {:?} must forward the real schema-drift cause.\nstdout: {}",
            args,
            stdout
        );
        assert!(
            !stdout.contains("DatabaseNotFound"),
            "mirage {:?} must not misreport an existing drifted DB as not found.\nstdout: {}",
            args,
            stdout
        );
    }
}

/// A genuinely missing file must still be reported as DatabaseNotFound with
/// the 'magellan watch' remediation — the honest pattern only relabels
/// open failures on files that exist.
#[test]
fn test_missing_db_still_reports_not_found_json() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("no_such.db");
    let db = missing.to_str().unwrap();

    for args in [&["status"][..], &["cfg", "--function", "foo"][..]] {
        let output = mirage()
            .args(["--db", db, "--output", "json"])
            .args(args)
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);

        assert_eq!(output.status.code(), Some(3), "mirage {:?}", args);
        assert!(
            stdout.contains("DatabaseNotFound"),
            "mirage {:?} on missing DB must report DatabaseNotFound.\nstdout: {}",
            args,
            stdout
        );
        assert!(
            stdout.contains("magellan watch"),
            "mirage {:?} on missing DB must keep the remediation hint.\nstdout: {}",
            args,
            stdout
        );
    }

    // diff opens its DBs via Backend::detect_and_open; a missing --before-db
    // must also be DatabaseNotFound.
    let output = mirage()
        .args([
            "--output",
            "json",
            "diff",
            "--function",
            "foo",
            "--before-db",
            db,
            "--after-db",
            db,
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(3));
    assert!(
        stdout.contains("DatabaseNotFound"),
        "diff on missing --before-db must report DatabaseNotFound.\nstdout: {}",
        stdout
    );
}

/// `diff` opens via `Backend::detect_and_open` (no schema validation, lazy
/// sqlite open), so neither schema drift nor a corrupt file trips its
/// open-error path — failures surface at first query. Whatever the failure
/// point, the reported error must name the real cause and must never claim
/// DatabaseNotFound for a file that exists.
#[test]
fn test_diff_reports_open_failure_honestly_json() {
    let dir = TempDir::new().unwrap();
    let garbage = dir.path().join("not_a_db.db");
    std::fs::write(&garbage, b"this is not a sqlite database").unwrap();
    let garbage = garbage.to_str().unwrap();

    let output = mirage()
        .args([
            "--output",
            "json",
            "diff",
            "--function",
            "foo",
            "--before-db",
            garbage,
            "--after-db",
            garbage,
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(output.status.code(), Some(3));
    assert!(
        stdout.contains("file is not a database"),
        "diff must forward the real underlying error.\nstdout: {}",
        stdout
    );
    assert!(
        !stdout.contains("DatabaseNotFound"),
        "diff must not misreport an existing file as not found.\nstdout: {}",
        stdout
    );
}

/// Human output mode must also name the real cause on stderr instead of the
/// bare "Failed to open database: <path>" + the create-the-database hint
/// (which sends the user the wrong direction on a drifted DB).
#[test]
fn test_drifted_db_human_output_names_real_cause() {
    let (_dir, db_path) = create_drifted_db();
    let db = db_path.to_str().unwrap();

    let output = mirage()
        .args(["--db", db, "cfg", "--function", "foo"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(3));
    assert!(
        stderr.contains("schema version 5 is too old"),
        "human mode must name the schema-drift cause.\nstderr: {}",
        stderr
    );
    assert!(
        !stderr.contains("to create the database"),
        "human mode must not hint at creating the database for a drifted DB.\nstderr: {}",
        stderr
    );
}

/// `hotspots` only opens a MirageDb under the (non-default) `sqlite`
/// feature; with default features it goes inter-procedural via
/// MagellanBridge and never touches MirageDb::open. Gate accordingly.
#[cfg(feature = "sqlite")]
#[test]
fn test_hotspots_reports_schema_drift_honestly_json() {
    let (_dir, db_path) = create_drifted_db();
    let db = db_path.to_str().unwrap();

    let output = mirage()
        .args(["--db", db, "--output", "json", "hotspots"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(output.status.code(), Some(3));
    assert!(stdout.contains("DatabaseOpenFailed"), "stdout: {}", stdout);
    assert!(
        stdout.contains("schema version 5 is too old"),
        "stdout: {}",
        stdout
    );
    assert!(!stdout.contains("DatabaseNotFound"), "stdout: {}", stdout);
}
