// Integration tests for Mirage database layer
//
// These tests verify that the database schema works correctly with real Magellan databases
// and all Phase 1 requirements are satisfied.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};

/// Helper to create a minimal Magellan v7 database for testing
///
/// This provides a real Magellan v7 database environment for testing Mirage's
/// schema extensions. The database includes:
/// - magellan_meta table with schema version 7 (Magellan v7)
/// - graph_entities table for storing functions
/// - cfg_blocks table with Magellan v7 schema (kind, terminator as lowercase TEXT)
/// - graph_edges table for edge storage
fn create_test_magellan_db() -> tempfile::NamedTempFile {
    let db = tempfile::NamedTempFile::new().unwrap();
    let conn = Connection::open(db.path()).unwrap();

    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

    // Create magellan_meta table (Magellan schema version 7)
    conn.execute(
        "CREATE TABLE magellan_meta (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            magellan_schema_version INTEGER NOT NULL,
            sqlitegraph_schema_version INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        )",
        [],
    ).unwrap();

    // Insert Magellan v7 meta
    conn.execute(
        "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
         VALUES (1, 7, 3, 0)",
        [],
    ).unwrap();

    // Create graph_entities table
    conn.execute(
        "CREATE TABLE graph_entities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            file_path TEXT,
            data TEXT NOT NULL
        )",
        [],
    ).unwrap();

    // Create Magellan v7 cfg_blocks table
    // Uses lowercase "kind" and "terminator" columns (not block_kind, not JSON)
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
            FOREIGN KEY (function_id) REFERENCES graph_entities(id)
        )",
        [],
    ).unwrap();

    // Create graph_edges table for edge storage
    conn.execute(
        "CREATE TABLE graph_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_id INTEGER NOT NULL,
            to_id INTEGER NOT NULL,
            edge_type TEXT NOT NULL,
            data TEXT
        )",
        [],
    ).unwrap();

    db
}

/// Helper to get a list of all index names for a given table
fn get_index_names(conn: &Connection, table_pattern: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='index' AND tbl_name LIKE ? ORDER BY name"
    )?;

    let indexes = stmt.query_map([table_pattern], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(indexes)
}

/// Check if a table exists in the database
fn table_exists(conn: &Connection, table_name: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
        [table_name],
        |row| row.get::<_, i64>(0),
    ).unwrap_or(0) > 0
}

/// Check if an index exists in the database
fn index_exists(conn: &Connection, index_name: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?",
        [index_name],
        |row| row.get::<_, i64>(0),
    ).unwrap_or(0) > 0
}

/// Helper to insert a test function into graph_entities
fn insert_test_function(conn: &Connection, name: &str, file_path: &str) -> i64 {
    conn.execute(
        "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
        ("function", name, file_path, "{}"),
    ).unwrap();
    conn.last_insert_rowid()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mirage_analyzer::storage::{create_schema, MirageDb, MIRAGE_SCHEMA_VERSION, REQUIRED_MAGELLAN_SCHEMA_VERSION};

    #[test]
    fn test_magellan_db_setup() {
        // Verify our helper creates a valid Magellan database
        let db_file = create_test_magellan_db();
        let conn = Connection::open(db_file.path()).unwrap();

        // Check magellan_meta exists
        assert!(table_exists(&conn, "magellan_meta"));

        // Check graph_entities exists
        assert!(table_exists(&conn, "graph_entities"));

        // Check cfg_blocks exists (Magellan v7 table)
        assert!(table_exists(&conn, "cfg_blocks"));
        // Check graph_edges exists
        assert!(table_exists(&conn, "graph_edges"));

        // Verify Magellan schema version
        let version: i32 = conn.query_row(
            "SELECT magellan_schema_version FROM magellan_meta WHERE id = 1",
            [],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(version, 7, "Magellan schema version should be 7");

        // Verify we can insert test data
        let function_id = insert_test_function(&conn, "test_func", "test.rs");
        assert!(function_id > 0, "Should have a valid function_id");

        // Verify the insert worked
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM graph_entities WHERE id = ?",
            [function_id],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(count, 1);
    }

    #[test]
    fn test_schema_creation_in_magellan_db() {
        // Create a real Magellan database with schema version 4
        let db_file = create_test_magellan_db();
        let mut conn = Connection::open(db_file.path()).unwrap();

        // Enable foreign keys for this connection
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

        // Call create_schema() to add Mirage tables
        create_schema(&mut conn, 7).unwrap();

        // Verify all Mirage tables exist
        let mirage_tables = vec![
            "cfg_blocks",
            "cfg_edges",
            "cfg_paths",
            "cfg_path_elements",
            "cfg_dominators",
            "cfg_post_dominators",
            "mirage_meta",
        ];

        for table in mirage_tables {
            assert!(
                table_exists(&conn, table),
                "Table {} should exist after schema creation",
                table
            );
        }

        // Verify indexes were created
        let cfg_indexes = get_index_names(&conn, "cfg_%").unwrap();
        assert!(!cfg_indexes.is_empty(), "Should have indexes for cfg_* tables");

        // Check specific indexes exist
        assert!(
            index_exists(&conn, "idx_cfg_blocks_function"),
            "Index idx_cfg_blocks_function should exist"
        );
        // Note: idx_cfg_blocks_function_hash doesn't exist in Magellan v7 (no function_hash column)
        assert!(
            index_exists(&conn, "idx_cfg_edges_from"),
            "Index idx_cfg_edges_from should exist"
        );
        assert!(
            index_exists(&conn, "idx_cfg_edges_to"),
            "Index idx_cfg_edges_to should exist"
        );
        assert!(
            index_exists(&conn, "idx_cfg_paths_function"),
            "Index idx_cfg_paths_function should exist"
        );

        // Verify mirage_meta has correct schema versions
        let mirage_version: i32 = conn.query_row(
            "SELECT mirage_schema_version FROM mirage_meta WHERE id = 1",
            [],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(mirage_version, MIRAGE_SCHEMA_VERSION);

        let magellan_version: i32 = conn.query_row(
            "SELECT magellan_schema_version FROM mirage_meta WHERE id = 1",
            [],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(magellan_version, REQUIRED_MAGELLAN_SCHEMA_VERSION);
    }

    #[test]
    fn test_foreign_key_enforcement() {
        // Create Magellan database
        let db_file = create_test_magellan_db();
        let mut conn = Connection::open(db_file.path()).unwrap();

        // Enable foreign keys (required for SQLite)
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

        // Verify foreign keys are enabled
        let fk_enabled: i32 = conn.query_row("PRAGMA foreign_keys", [], |row| row.get(0)).unwrap();
        assert_eq!(fk_enabled, 1, "Foreign keys should be enabled");

        // Create Mirage schema
        create_schema(&mut conn, 7).unwrap();

        // Insert a function into graph_entities
        let function_id = insert_test_function(&conn, "test_function", "src/test.rs");

        // Insert valid cfg_blocks referencing that function_id -> should succeed
        // Uses Magellan v7 schema: kind (not block_kind), terminator as TEXT, line/col fields
        let valid_result = conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (function_id, "entry", "return", 0, 10, 1, 0, 1, 10),
        );

        assert!(valid_result.is_ok(), "Insert with valid function_id should succeed");

        // Get the block_id for edge tests
        let block_id: i64 = conn.last_insert_rowid();

        // Attempt to insert cfg_blocks with non-existent function_id -> should fail
        let invalid_result = conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (9999i64, "entry", "return", 0, 10, 1, 0, 1, 10),
        );

        assert!(
            invalid_result.is_err(),
            "Insert with non-existent function_id should fail due to FK constraint"
        );

        // Insert another block for edge testing
        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (function_id, "block", "fallthrough", 11, 20, 2, 0, 2, 10),
        ).unwrap();

        let block_id_2: i64 = conn.last_insert_rowid();

        // Insert cfg_edges referencing valid block IDs -> should succeed
        let valid_edge = conn.execute(
            "INSERT INTO cfg_edges (from_id, to_id, edge_type)
             VALUES (?, ?, ?)",
            (block_id, block_id_2, "fallthrough"),
        );

        assert!(valid_edge.is_ok(), "Insert with valid block IDs should succeed");

        // Attempt to insert cfg_edges with invalid block IDs -> should fail
        let invalid_edge = conn.execute(
            "INSERT INTO cfg_edges (from_id, to_id, edge_type)
             VALUES (?, ?, ?)",
            (9999i64, block_id_2, "fallthrough"),
        );

        assert!(
            invalid_edge.is_err(),
            "Insert with invalid from_id should fail due to FK constraint"
        );

        let invalid_edge_2 = conn.execute(
            "INSERT INTO cfg_edges (from_id, to_id, edge_type)
             VALUES (?, ?, ?)",
            (block_id, 8888i64, "fallthrough"),
        );

        assert!(
            invalid_edge_2.is_err(),
            "Insert with invalid to_id should fail due to FK constraint"
        );
    }

    #[test]
    fn test_incremental_update_tracking() {
        // Create Magellan database with Mirage schema
        let db_file = create_test_magellan_db();
        let mut conn = Connection::open(db_file.path()).unwrap();
        create_schema(&mut conn, 7).unwrap();

        // Insert a function
        let function_id = insert_test_function(&conn, "changing_function", "src/lib.rs");

        // Insert cfg_blocks with Magellan v7 schema (no function_hash column)
        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (function_id, "entry", "fallthrough", 0, 10, 1, 0, 1, 10),
        ).unwrap();

        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (function_id, "block", "return", 11, 20, 2, 0, 2, 10),
        ).unwrap();

        // Query for blocks for this function
        let count_initial: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
            (function_id,),
            |row| row.get(0),
        ).unwrap();

        assert_eq!(count_initial, 2, "Should have 2 blocks initially");

        // Note: Magellan v7 doesn't use function_hash for incremental updates
        // Hash-based change detection is handled differently in Magellan's system
        // This test now verifies basic block insertion/counting

        // Verify query by non-existent function returns 0
        let count_different: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
            (9999i64,),
            |row| row.get(0),
        ).unwrap();

        assert_eq!(count_different, 0, "Should have 0 blocks for non-existent function");

        // Test the needs_reanalysis helper function (adapted for Magellan v7)
        // In Magellan v7, function_exists check is sufficient
        assert!(!needs_reanalysis(&conn, function_id, "any_hash").unwrap(),
                "Should not need reanalysis when function has blocks");

        assert!(needs_reanalysis(&conn, 9999, "any_hash").unwrap(),
               "Should need reanalysis when function has no blocks");
    }

    /// Helper function that demonstrates the incremental workflow with Magellan v7:
    /// Returns true if the function needs to be re-analyzed
    ///
    /// In Magellan v7, there's no function_hash column in cfg_blocks.
    /// Instead, we check if the function has any CFG blocks at all.
    fn needs_reanalysis(conn: &Connection, function_id: i64, _new_hash: &str) -> Result<bool> {
        // Query for existing CFG blocks for this function
        let block_count: Option<i64> = conn.query_row(
            "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
            [function_id],
            |row| row.get(0),
        ).optional()?;

        // Return true if no blocks found (needs indexing), false if blocks exist
        Ok(match block_count {
            Some(count) => count == 0,
            None => true,
        })
    }

    #[test]
    fn test_migration_framework() {
        // Test 1: Create database at schema version 0 (no mirage_meta)
        let db_file = tempfile::NamedTempFile::new().unwrap();
        let mut conn = Connection::open(db_file.path()).unwrap();

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

        // Create Magellan meta (Mirage schema version implicitly 0)
        conn.execute(
            "CREATE TABLE magellan_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                magellan_schema_version INTEGER NOT NULL,
                sqlitegraph_schema_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT,
                data TEXT NOT NULL
            )",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, 7, 3, 0)",  // Magellan v7
            [],
        ).unwrap();

        // Run create_schema() -> should create mirage_meta with version 1
        create_schema(&mut conn, 7).unwrap();

        // Verify version is 1
        let version: i32 = conn.query_row(
            "SELECT mirage_schema_version FROM mirage_meta WHERE id = 1",
            [],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(version, MIRAGE_SCHEMA_VERSION,
                   "Schema should be at version {} after creation", MIRAGE_SCHEMA_VERSION);

        // Test 2: Run create_schema() again -> should detect version 1, do nothing
        // (No error should occur)
        create_schema(&mut conn, 7).unwrap();

        let version_2: i32 = conn.query_row(
            "SELECT mirage_schema_version FROM mirage_meta WHERE id = 1",
            [],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(version_2, MIRAGE_SCHEMA_VERSION,
                   "Schema version should remain {} after second call", MIRAGE_SCHEMA_VERSION);

        // Test 3: MirageDb::open() with newer schema should error
        let db_file_newer = tempfile::NamedTempFile::new().unwrap();
        {
            let mut conn_newer = Connection::open(db_file_newer.path()).unwrap();

            // Create Magellan tables
            conn_newer.execute(
                "CREATE TABLE magellan_meta (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    magellan_schema_version INTEGER NOT NULL,
                    sqlitegraph_schema_version INTEGER NOT NULL,
                    created_at INTEGER NOT NULL
                )",
                [],
            ).unwrap();

            conn_newer.execute(
                "CREATE TABLE graph_entities (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    kind TEXT NOT NULL,
                    name TEXT NOT NULL,
                    file_path TEXT,
                    data TEXT NOT NULL
                )",
                [],
            ).unwrap();

            conn_newer.execute(
                "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
                 VALUES (1, 7, 3, 0)",  // Magellan v7
                [],
            ).unwrap();

            // Create Mirage schema at version 1
            create_schema(&mut conn_newer, 7).unwrap();

            // Manually bump to version 2 (simulating a newer database)
            conn_newer.execute(
                "UPDATE mirage_meta SET mirage_schema_version = ? WHERE id = 1",
                [2i32],
            ).unwrap();
        }

        // Try to open with MirageDb (should fail with version error)
        let result = MirageDb::open(db_file_newer.path());

        assert!(result.is_err(),
                "Opening a database with schema version 2 should fail when we only support version 1");

        if let Err(e) = result {
            let err = e.to_string();
            assert!(err.contains("newer than supported version"),
                    "Error message should mention newer version: {}", err);
        }
    }

    #[test]
    fn test_open_nonexistent_database() {
        // MirageDb::open() should fail gracefully for non-existent databases
        let result = MirageDb::open("/path/that/does/not/exist.db");

        assert!(result.is_err(), "Should fail to open non-existent database");

        if let Err(e) = result {
            let err = e.to_string();
            assert!(err.contains("not found"),
                    "Error should mention 'not found': {}", err);
        }
    }

    #[test]
    fn test_magellan_schema_compatibility() {
        // Test with wrong Magellan schema version
        let db_file = tempfile::NamedTempFile::new().unwrap();
        {
            let conn = Connection::open(db_file.path()).unwrap();

            // Create Magellan meta with wrong version (6 instead of 7)
            conn.execute(
                "CREATE TABLE magellan_meta (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    magellan_schema_version INTEGER NOT NULL,
                    sqlitegraph_schema_version INTEGER NOT NULL,
                    created_at INTEGER NOT NULL
                )",
                [],
            ).unwrap();

            conn.execute(
                "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
                 VALUES (1, 6, 3, 0)",  // Version 6 is too old (need v7+)
                [],
            ).unwrap();
        }

        let result = MirageDb::open(db_file.path());

        assert!(result.is_err(),
                "Should fail with incompatible Magellan schema version");

        if let Err(e) = result {
            let err = e.to_string();
            assert!(err.contains("too old") || err.contains("incompatible"),
                    "Error should mention 'too old' or 'incompatible': {}", err);
        }
    }

    #[test]
    fn test_full_workflow() {
        // Integration test: Full workflow from DB creation to status query
        let db_file = create_test_magellan_db();
        let mut conn = Connection::open(db_file.path()).unwrap();
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

        // Create Mirage schema
        create_schema(&mut conn, 7).unwrap();

        // Insert test data using Magellan v7 schema
        let function_id = insert_test_function(&conn, "full_test_func", "src/full.rs");

        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (function_id, "entry", "fallthrough", 0, 10, 1, 0, 1, 10),
        ).unwrap();

        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (function_id, "block", "return", 11, 20, 2, 0, 2, 10),
        ).unwrap();

        // Note: Edges are computed in memory in Magellan v7, not stored in cfg_edges
        // The cfg_edges table exists for backward compatibility but is unused

        // Use MirageDb to get status
        drop(conn);
        let db = MirageDb::open(db_file.path()).unwrap();
        let status = db.status().unwrap();

        // Verify status reflects our test data
        assert_eq!(status.cfg_blocks, 2, "Should have 2 cfg_blocks");
        assert_eq!(status.mirage_schema_version, 1, "Mirage schema should be v1");
        assert_eq!(status.magellan_schema_version, 7, "Magellan schema should be v7");
    }
}
