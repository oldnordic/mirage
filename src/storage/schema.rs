use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use super::{MIRAGE_SCHEMA_VERSION, REQUIRED_MAGELLAN_SCHEMA_VERSION};
#[cfg(all(test, feature = "sqlite"))]
use super::{REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, TEST_MAGELLAN_SCHEMA_VERSION};

/// A schema migration
struct Migration {
    version: i32,
    description: &'static str,
    up: fn(&mut Connection) -> Result<()>,
}

/// Get all registered migrations
fn migrations() -> Vec<Migration> {
    vec![]
}

/// Run schema migrations to bring database up to current version
pub fn migrate_schema(conn: &mut Connection) -> Result<()> {
    let current_version: i32 = conn
        .query_row(
            "SELECT mirage_schema_version FROM mirage_meta WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if current_version >= MIRAGE_SCHEMA_VERSION {
        return Ok(());
    }

    // Get migrations that need to run
    let pending: Vec<_> = migrations()
        .into_iter()
        .filter(|m| m.version > current_version && m.version <= MIRAGE_SCHEMA_VERSION)
        .collect();

    for migration in pending {
        // Run migration
        (migration.up)(conn).with_context(|| {
            format!(
                "Failed to run migration v{}: {}",
                migration.version, migration.description
            )
        })?;

        // Update version
        conn.execute(
            "UPDATE mirage_meta SET mirage_schema_version = ? WHERE id = 1",
            params![migration.version],
        )?;
    }

    // Ensure we're at the final version
    if current_version < MIRAGE_SCHEMA_VERSION {
        conn.execute(
            "UPDATE mirage_meta SET mirage_schema_version = ? WHERE id = 1",
            params![MIRAGE_SCHEMA_VERSION],
        )?;
    }

    Ok(())
}

/// Create Mirage schema tables in an existing Magellan database
///
/// The magellan_schema_version parameter should be the actual version
/// from the magellan_meta table, not MIN_MAGELLAN_SCHEMA_VERSION.
pub fn create_schema(conn: &mut Connection, _magellan_schema_version: i32) -> Result<()> {
    // Create mirage_meta table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS mirage_meta (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            mirage_schema_version INTEGER NOT NULL,
            magellan_schema_version INTEGER NOT NULL,
            compiler_version TEXT,
            created_at INTEGER NOT NULL
        )",
        [],
    )?;

    // Create cfg_blocks table matching the current Magellan contract.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cfg_blocks (
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
            cfg_condition TEXT,
            FOREIGN KEY (function_id) REFERENCES graph_entities(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_cfg_blocks_function ON cfg_blocks(function_id)",
        [],
    )?;

    // cfg_edges table is managed by Magellan v11+ with schema:
    //   (id, function_id, source_idx, target_idx, edge_type)
    // Mirage computes edges in memory via build_edges_from_terminators().

    // Create cfg_paths table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cfg_paths (
            path_id TEXT PRIMARY KEY,
            function_id INTEGER NOT NULL,
            path_kind TEXT NOT NULL,
            entry_block INTEGER NOT NULL,
            exit_block INTEGER NOT NULL,
            length INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (function_id) REFERENCES graph_entities(id)
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_cfg_paths_function ON cfg_paths(function_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_cfg_paths_kind ON cfg_paths(path_kind)",
        [],
    )?;

    // Create cfg_path_elements table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cfg_path_elements (
            path_id TEXT NOT NULL,
            sequence_order INTEGER NOT NULL,
            block_id INTEGER NOT NULL,
            PRIMARY KEY (path_id, sequence_order),
            FOREIGN KEY (path_id) REFERENCES cfg_paths(path_id)
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS cfg_path_elements_block ON cfg_path_elements(block_id)",
        [],
    )?;

    // Create cfg_dominators table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cfg_dominators (
            block_id INTEGER NOT NULL,
            dominator_id INTEGER NOT NULL,
            is_strict BOOLEAN NOT NULL,
            PRIMARY KEY (block_id, dominator_id, is_strict),
            FOREIGN KEY (block_id) REFERENCES cfg_blocks(id),
            FOREIGN KEY (dominator_id) REFERENCES cfg_blocks(id)
        )",
        [],
    )?;

    // Create cfg_post_dominators table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cfg_post_dominators (
            block_id INTEGER NOT NULL,
            post_dominator_id INTEGER NOT NULL,
            is_strict BOOLEAN NOT NULL,
            PRIMARY KEY (block_id, post_dominator_id, is_strict),
            FOREIGN KEY (block_id) REFERENCES cfg_blocks(id),
            FOREIGN KEY (post_dominator_id) REFERENCES cfg_blocks(id)
        )",
        [],
    )?;

    // Initialize mirage_meta
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT OR REPLACE INTO mirage_meta (id, mirage_schema_version, magellan_schema_version, created_at)
         VALUES (1, ?, ?, ?)",
        params![MIRAGE_SCHEMA_VERSION, REQUIRED_MAGELLAN_SCHEMA_VERSION, now],
    )?;

    Ok(())
}

#[cfg(all(test, feature = "sqlite"))]
pub(crate) fn create_test_db_with_schema() -> rusqlite::Connection {
    use rusqlite::Connection;

    let mut conn = Connection::open_in_memory().expect("test setup: in-memory DB always succeeds");

    conn.execute(
        "CREATE TABLE magellan_meta (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            magellan_schema_version INTEGER NOT NULL,
            sqlitegraph_schema_version INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        )",
        [],
    )
    .expect("test setup: CREATE TABLE magellan_meta");

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
    .expect("test setup: CREATE TABLE graph_entities");

    conn.execute(
        "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
         VALUES (1, ?, ?, ?)",
        rusqlite::params![7, 3, 0],
    ).expect("test setup: INSERT magellan_meta");

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
            cfg_condition TEXT,
            FOREIGN KEY (function_id) REFERENCES graph_entities(id)
        )",
        [],
    )
    .expect("test setup: CREATE TABLE cfg_blocks");

    conn.execute(
        "CREATE TABLE graph_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_id INTEGER NOT NULL,
            to_id INTEGER NOT NULL,
            edge_type TEXT NOT NULL,
            data TEXT
        )",
        [],
    )
    .expect("test setup: CREATE TABLE graph_edges");

    create_schema(&mut conn, TEST_MAGELLAN_SCHEMA_VERSION).expect("test setup: create_schema");

    conn.execute("PRAGMA foreign_keys = ON", [])
        .expect("test setup: PRAGMA foreign_keys");

    conn
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_create_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
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
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, ?, ?, ?)",
            params![REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, 0],
        ).unwrap();

        create_schema(&mut conn, TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name LIKE 'cfg_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert!(table_count >= 4);
    }

    #[test]
    fn test_migrate_schema_from_version_0() {
        let mut conn = Connection::open_in_memory().unwrap();

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
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, ?, ?, ?)",
            params![REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, 0],
        ).unwrap();

        create_schema(&mut conn, TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        let version: i32 = conn
            .query_row(
                "SELECT mirage_schema_version FROM mirage_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(version, MIRAGE_SCHEMA_VERSION);
    }

    #[test]
    fn test_migrate_schema_no_op_when_current() {
        let mut conn = Connection::open_in_memory().unwrap();

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
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, ?, ?, ?)",
            params![REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, 0],
        ).unwrap();

        create_schema(&mut conn, TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        migrate_schema(&mut conn).unwrap();

        let version: i32 = conn
            .query_row(
                "SELECT mirage_schema_version FROM mirage_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(version, MIRAGE_SCHEMA_VERSION);
    }

    #[test]
    fn test_fk_constraint_cfg_blocks() {
        let mut conn = Connection::open_in_memory().unwrap();

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
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, ?, ?, ?)",
            params![REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, 0],
        ).unwrap();

        create_schema(&mut conn, TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            params!("function", "test_func", "test.rs", "{}"),
        )
        .unwrap();

        let function_id: i64 = conn.last_insert_rowid();

        let invalid_result = conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params!(9999, "entry", "return", 0, 10, 1, 0, 1, 10),
        );

        assert!(
            invalid_result.is_err(),
            "Insert with invalid function_id should fail"
        );

        let valid_result = conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params!(function_id, "entry", "return", 0, 10, 1, 0, 1, 10),
        );

        assert!(
            valid_result.is_ok(),
            "Insert with valid function_id should succeed"
        );

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
                params![function_id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(count, 1, "Should have exactly one cfg_block entry");
    }
}
