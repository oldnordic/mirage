// Database storage layer extending Magellan's schema
//
// Mirage uses the same Magellan database and extends it with:
// - cfg_blocks: Basic blocks within functions
// - cfg_edges: Control flow between blocks
// - cfg_paths: Enumerated execution paths
// - cfg_path_elements: Blocks in each path
// - cfg_dominators: Dominance relationships
// - cfg_post_dominators: Reverse dominance

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::Path;

/// Mirage schema version
pub const MIRAGE_SCHEMA_VERSION: i32 = 1;

/// Magellan schema version we require
pub const REQUIRED_MAGELLAN_SCHEMA_VERSION: i32 = 4;

/// SQLiteGraph schema version we require
pub const REQUIRED_SQLITEGRAPH_SCHEMA_VERSION: i32 = 3;

/// Database connection wrapper
pub struct MirageDb {
    conn: Connection,
}

impl MirageDb {
    /// Open database at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            anyhow::bail!("Database not found: {}", path.display());
        }

        let mut conn = Connection::open(path)
            .context("Failed to open database")?;

        // Verify schema and run migrations if needed
        let mirage_version: i32 = conn.query_row(
            "SELECT mirage_schema_version FROM mirage_meta WHERE id = 1",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        if mirage_version > MIRAGE_SCHEMA_VERSION {
            anyhow::bail!(
                "Database schema version {} is newer than supported version {}.
                 Please update Mirage.",
                mirage_version, MIRAGE_SCHEMA_VERSION
            );
        }

        // Check Magellan schema compatibility
        let magellan_version: i32 = conn.query_row(
            "SELECT magellan_schema_version FROM magellan_meta WHERE id = 1",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        if magellan_version != REQUIRED_MAGELLAN_SCHEMA_VERSION {
            anyhow::bail!(
                "Magellan schema version {} is incompatible with required version {}.
                 Please update Magellan.",
                magellan_version, REQUIRED_MAGELLAN_SCHEMA_VERSION
            );
        }

        // Run migrations if we're behind current version
        if mirage_version < MIRAGE_SCHEMA_VERSION {
            migrate_schema(&mut conn)?;
        }

        Ok(Self { conn })
    }

    /// Get a reference to the underlying connection
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Get a mutable reference to the underlying connection
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

/// A schema migration
struct Migration {
    version: i32,
    description: &'static str,
    up: fn(&mut Connection) -> Result<()>,
}

/// Get all registered migrations
fn migrations() -> Vec<Migration> {
    // No migrations yet - framework is ready for future schema changes
    vec![]
}

/// Run schema migrations to bring database up to current version
pub fn migrate_schema(conn: &mut Connection) -> Result<()> {
    let current_version: i32 = conn.query_row(
        "SELECT mirage_schema_version FROM mirage_meta WHERE id = 1",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    if current_version >= MIRAGE_SCHEMA_VERSION {
        // Already at or above current version
        return Ok(());
    }

    // Get migrations that need to run
    let pending: Vec<_> = migrations()
        .into_iter()
        .filter(|m| m.version > current_version && m.version <= MIRAGE_SCHEMA_VERSION)
        .collect();

    for migration in pending {
        // Run migration
        (migration.up)(conn)
            .with_context(|| format!("Failed to run migration v{}: {}", migration.version, migration.description))?;

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
pub fn create_schema(conn: &mut Connection) -> Result<()> {
    // Create mirage_meta table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS mirage_meta (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            mirage_schema_version INTEGER NOT NULL,
            magellan_schema_version INTEGER NOT NULL,
            rustc_version TEXT,
            created_at INTEGER NOT NULL
        )",
        [],
    )?;

    // Create cfg_blocks table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cfg_blocks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            function_id INTEGER NOT NULL,
            block_kind TEXT NOT NULL,
            byte_start INTEGER,
            byte_end INTEGER,
            terminator TEXT,
            function_hash TEXT,
            FOREIGN KEY (function_id) REFERENCES graph_entities(id)
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_cfg_blocks_function ON cfg_blocks(function_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_cfg_blocks_function_hash ON cfg_blocks(function_hash)",
        [],
    )?;

    // Create cfg_edges table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cfg_edges (
            from_id INTEGER NOT NULL,
            to_id INTEGER NOT NULL,
            edge_type TEXT NOT NULL,
            PRIMARY KEY (from_id, to_id, edge_type),
            FOREIGN KEY (from_id) REFERENCES cfg_blocks(id),
            FOREIGN KEY (to_id) REFERENCES cfg_blocks(id)
        )",
        [],
    )?;

    conn.execute("CREATE INDEX IF NOT EXISTS idx_cfg_edges_from ON cfg_edges(from_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_cfg_edges_to ON cfg_edges(to_id)", [])?;

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
            FOREIGN KEY (function_id) REFERENCES graph_entities(id),
            FOREIGN KEY (entry_block) REFERENCES cfg_blocks(id),
            FOREIGN KEY (exit_block) REFERENCES cfg_blocks(id)
        )",
        [],
    )?;

    conn.execute("CREATE INDEX IF NOT EXISTS idx_cfg_paths_function ON cfg_paths(function_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_cfg_paths_kind ON cfg_paths(path_kind)", [])?;

    // Create cfg_path_elements table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cfg_path_elements (
            path_id TEXT NOT NULL,
            sequence_order INTEGER NOT NULL,
            block_id INTEGER NOT NULL,
            PRIMARY KEY (path_id, sequence_order),
            FOREIGN KEY (path_id) REFERENCES cfg_paths(path_id),
            FOREIGN KEY (block_id) REFERENCES cfg_blocks(id)
        )",
        [],
    )?;

    conn.execute("CREATE INDEX IF NOT EXISTS cfg_path_elements_block ON cfg_path_elements(block_id)", [])?;

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

/// Database status information
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatabaseStatus {
    pub cfg_blocks: i64,
    pub cfg_edges: i64,
    pub cfg_paths: i64,
    pub cfg_dominators: i64,
    pub mirage_schema_version: i32,
    pub magellan_schema_version: i32,
}

impl MirageDb {
    /// Get database statistics
    pub fn status(&self) -> Result<DatabaseStatus> {
        let cfg_blocks: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM cfg_blocks",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        let cfg_edges: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM cfg_edges",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        let cfg_paths: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM cfg_paths",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        let cfg_dominators: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM cfg_dominators",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        let mirage_schema_version: i32 = self.conn.query_row(
            "SELECT mirage_schema_version FROM mirage_meta WHERE id = 1",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        let magellan_schema_version: i32 = self.conn.query_row(
            "SELECT magellan_schema_version FROM magellan_meta WHERE id = 1",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        Ok(DatabaseStatus {
            cfg_blocks,
            cfg_edges,
            cfg_paths,
            cfg_dominators,
            mirage_schema_version,
            magellan_schema_version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        // First create the Magellan tables (simplified)
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

        // Insert Magellan meta
        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, ?, ?, ?)",
            params![REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, 0],
        ).unwrap();

        // Create Mirage schema
        create_schema(&mut conn).unwrap();

        // Verify tables exist
        let table_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name LIKE 'cfg_%'",
            [],
            |row| row.get(0),
        ).unwrap();

        assert!(table_count >= 5); // cfg_blocks, cfg_edges, cfg_paths, cfg_path_elements, cfg_dominators
    }

    #[test]
    fn test_migrate_schema_from_version_0() {
        let mut conn = Connection::open_in_memory().unwrap();

        // Create Magellan tables
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
             VALUES (1, ?, ?, ?)",
            params![REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, 0],
        ).unwrap();

        // Create Mirage schema at version 0 (no mirage_meta yet)
        create_schema(&mut conn).unwrap();

        // Verify version is 1
        let version: i32 = conn.query_row(
            "SELECT mirage_schema_version FROM mirage_meta WHERE id = 1",
            [],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(version, MIRAGE_SCHEMA_VERSION);
    }

    #[test]
    fn test_migrate_schema_no_op_when_current() {
        let mut conn = Connection::open_in_memory().unwrap();

        // Create Magellan tables
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
             VALUES (1, ?, ?, ?)",
            params![REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, 0],
        ).unwrap();

        // Create Mirage schema
        create_schema(&mut conn).unwrap();

        // Migration should be a no-op - already at current version
        migrate_schema(&mut conn).unwrap();

        // Verify version is still 1
        let version: i32 = conn.query_row(
            "SELECT mirage_schema_version FROM mirage_meta WHERE id = 1",
            [],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(version, MIRAGE_SCHEMA_VERSION);
    }

    #[test]
    fn test_fk_constraint_cfg_blocks() {
        let mut conn = Connection::open_in_memory().unwrap();

        // Enable foreign key enforcement (SQLite requires this)
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

        // Create Magellan tables
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
             VALUES (1, ?, ?, ?)",
            params![REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, 0],
        ).unwrap();

        // Create Mirage schema
        create_schema(&mut conn).unwrap();

        // Insert a graph entity (function)
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            params!("function", "test_func", "test.rs", "{}"),
        ).unwrap();

        let function_id: i64 = conn.last_insert_rowid();

        // Attempt to insert cfg_blocks with invalid function_id (should fail)
        let invalid_result = conn.execute(
            "INSERT INTO cfg_blocks (function_id, block_kind, byte_start, byte_end, terminator, function_hash) VALUES (?, ?, ?, ?, ?, ?)",
            params!(9999, "entry", 0, 10, "ret", "abc123"),
        );

        // Should fail with foreign key constraint error
        assert!(invalid_result.is_err(), "Insert with invalid function_id should fail");

        // Insert valid cfg_blocks with correct function_id (should succeed)
        let valid_result = conn.execute(
            "INSERT INTO cfg_blocks (function_id, block_kind, byte_start, byte_end, terminator, function_hash) VALUES (?, ?, ?, ?, ?, ?)",
            params!(function_id, "entry", 0, 10, "ret", "abc123"),
        );

        assert!(valid_result.is_ok(), "Insert with valid function_id should succeed");

        // Verify the insert worked
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(count, 1, "Should have exactly one cfg_block entry");
    }
}
