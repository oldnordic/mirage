//! SQLite backend implementation for mirage storage trait
//!
//! Uses direct rusqlite queries to cfg_blocks table for CFG data.
//!
//! # Design
//!
//! - Implements `StorageTrait` for SQLite databases
//! - Uses `prepare_cached` for query performance
//! - Leverages Magellan's cfg_blocks table (schema v7+)
//! - Provides backend-agnostic `CfgBlockData` from SQL queries
//!
//! # Examples
//!
//! ```ignore
//! # use mirage_analyzer::storage::sqlite_backend::SqliteStorage;
//! # use anyhow::Result;
//! # fn main() -> Result<()> {
//! let storage = SqliteStorage::open("/path/to/codegraph.db")?;
//! let blocks = storage.get_cfg_blocks(123)?;
//! # Ok(())
//! # }
//! ```

use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::Path;

use super::{CfgBlockData, StorageTrait};

/// SQLite backend implementation
///
/// Wraps a rusqlite Connection and implements StorageTrait
/// using direct SQL queries to Magellan's cfg_blocks table.
#[derive(Debug)]
pub struct SqliteStorage {
    conn: Connection,
}

impl SqliteStorage {
    /// Open SQLite database at the given path
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to the SQLite database file
    ///
    /// # Returns
    ///
    /// * `Ok(SqliteStorage)` - Storage instance ready for queries
    /// * `Err(...)` - Error if file cannot be opened
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use mirage_analyzer::storage::sqlite_backend::SqliteStorage;
    /// # fn main() -> anyhow::Result<()> {
    /// let storage = SqliteStorage::open("codegraph.db")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)
            .map_err(|e| anyhow::anyhow!("Failed to open SQLite database: {}", e))?;
        Ok(Self { conn })
    }

    /// Get a reference to the underlying Connection
    ///
    /// This is useful for legacy code that needs direct SQL access.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

impl StorageTrait for SqliteStorage {
    /// Get CFG blocks for a function from SQLite backend
    ///
    /// Queries Magellan's cfg_blocks table for all blocks belonging
    /// to the given function_id, ordered by block ID.
    ///
    /// # Arguments
    ///
    /// * `function_id` - ID of the function in graph_entities
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<CfgBlockData>)` - Vector of CFG block data
    /// * `Err(...)` - Error if query fails
    ///
    /// # Note
    ///
    /// - Uses prepare_cached for performance on repeated calls
    /// - Returns empty Vec if function has no CFG blocks (not an error)
    fn get_cfg_blocks(&self, function_id: i64) -> Result<Vec<CfgBlockData>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, kind, terminator, byte_start, byte_end,
                    start_line, start_col, end_line, end_col
             FROM cfg_blocks
             WHERE function_id = ?
             ORDER BY id ASC"
        ).map_err(|e| anyhow::anyhow!("Failed to prepare cfg_blocks query: {}", e))?;

        let blocks = stmt.query_map(params![function_id], |row| {
            Ok(CfgBlockData {
                id: row.get(0)?,
                kind: row.get(1)?,
                terminator: row.get(2)?,
                byte_start: row.get::<_, Option<i64>>(3)?.unwrap_or(0) as u64,
                byte_end: row.get::<_, Option<i64>>(4)?.unwrap_or(0) as u64,
                start_line: row.get::<_, Option<i64>>(5)?.unwrap_or(0) as u64,
                start_col: row.get::<_, Option<i64>>(6)?.unwrap_or(0) as u64,
                end_line: row.get::<_, Option<i64>>(7)?.unwrap_or(0) as u64,
                end_col: row.get::<_, Option<i64>>(8)?.unwrap_or(0) as u64,
            })
        })
        .map_err(|e| anyhow::anyhow!("Failed to execute cfg_blocks query: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("Failed to collect cfg_blocks rows: {}", e))?;

        Ok(blocks)
    }

    /// Get entity by ID from SQLite backend
    ///
    /// Queries the graph_entities table for the entity with the given ID.
    ///
    /// # Arguments
    ///
    /// * `entity_id` - ID of the entity
    ///
    /// # Returns
    ///
    /// * `Some(GraphEntity)` - Entity if found
    /// * `None` - Entity not found
    fn get_entity(&self, entity_id: i64) -> Option<sqlitegraph::GraphEntity> {
        self.conn
            .query_row(
                "SELECT id, kind, name, file_path, data
                 FROM graph_entities
                 WHERE id = ?",
                params![entity_id],
                |row| {
                    Ok(sqlitegraph::GraphEntity {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        name: row.get(2)?,
                        file_path: row.get(3)?,
                        data: serde_json::from_str(row.get::<_, String>(4)?.as_str())
                            .unwrap_or_default(),
                    })
                },
            )
            .ok()
    }

    /// Get cached paths for a function from SQLite backend
    ///
    /// Queries the cfg_paths table for cached enumerated paths.
    ///
    /// # Arguments
    ///
    /// * `function_id` - ID of the function
    ///
    /// # Returns
    ///
    /// * `Ok(Some(paths))` - Cached paths if available
    /// * `Ok(None)` - No cached paths
    /// * `Err(...)` - Error if query fails
    ///
    /// # Note
    ///
    /// This is a placeholder for future path caching implementation.
    /// Currently returns Ok(None) for all functions.
    fn get_cached_paths(&self, _function_id: i64) -> Result<Option<Vec<crate::cfg::Path>>> {
        // TODO: Implement path caching in Phase 071 (Mirage Advanced Commands)
        // Query cfg_paths and cfg_path_elements tables
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Helper to create a test database with cfg_blocks table
    fn create_test_db() -> tempfile::NamedTempFile {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let conn = Connection::open(temp_file.path()).unwrap();

        // Create magellan_meta table
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

        // Create cfg_blocks table
        conn.execute(
            "CREATE TABLE cfg_blocks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                function_id INTEGER NOT NULL,
                kind TEXT NOT NULL,
                terminator TEXT NOT NULL,
                byte_start INTEGER,
                byte_end INTEGER,
                start_line INTEGER,
                start_col INTEGER,
                end_line INTEGER,
                end_col INTEGER,
                FOREIGN KEY (function_id) REFERENCES graph_entities(id)
            )",
            [],
        ).unwrap();

        conn.execute(
            "CREATE INDEX idx_cfg_blocks_function ON cfg_blocks(function_id)",
            [],
        ).unwrap();

        // Insert a test function
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data)
             VALUES ('Symbol', 'test_function', '/tmp/test.rs', '{\"kind\": \"Function\"}')",
            [],
        ).unwrap();

        // Insert test CFG blocks
        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                   start_line, start_col, end_line, end_col)
             VALUES (1, 'entry', 'fallthrough', 0, 10, 1, 0, 1, 10),
                    (1, 'normal', 'conditional', 10, 50, 2, 4, 5, 8),
                    (1, 'return', 'return', 50, 60, 5, 0, 5, 10)",
            [],
        ).unwrap();

        temp_file
    }

    #[test]
    fn test_sqlite_storage_open() {
        let temp_file = create_test_db();
        let result = SqliteStorage::open(temp_file.path());
        assert!(result.is_ok(), "Should open test database");
    }

    #[test]
    fn test_sqlite_storage_get_cfg_blocks() {
        let temp_file = create_test_db();
        let storage = SqliteStorage::open(temp_file.path()).unwrap();

        let blocks = storage.get_cfg_blocks(1).unwrap();
        assert_eq!(blocks.len(), 3, "Should have 3 CFG blocks");

        // Check first block (entry)
        assert_eq!(blocks[0].kind, "entry");
        assert_eq!(blocks[0].terminator, "fallthrough");
        assert_eq!(blocks[0].byte_start, 0);
        assert_eq!(blocks[0].byte_end, 10);

        // Check second block (conditional)
        assert_eq!(blocks[1].kind, "normal");
        assert_eq!(blocks[1].terminator, "conditional");

        // Check third block (return)
        assert_eq!(blocks[2].kind, "return");
        assert_eq!(blocks[2].terminator, "return");
    }

    #[test]
    fn test_sqlite_storage_get_cfg_blocks_empty() {
        let temp_file = create_test_db();
        let storage = SqliteStorage::open(temp_file.path()).unwrap();

        // Function 999 doesn't exist
        let blocks = storage.get_cfg_blocks(999).unwrap();
        assert_eq!(blocks.len(), 0, "Should return empty Vec for non-existent function");
    }

    #[test]
    fn test_sqlite_storage_get_entity() {
        let temp_file = create_test_db();
        let storage = SqliteStorage::open(temp_file.path()).unwrap();

        let entity = storage.get_entity(1);
        assert!(entity.is_some(), "Should find entity with ID 1");
        let entity = entity.unwrap();
        assert_eq!(entity.id, 1);
        assert_eq!(entity.kind, "Symbol");
        assert_eq!(entity.name, "test_function");
    }

    #[test]
    fn test_sqlite_storage_get_entity_not_found() {
        let temp_file = create_test_db();
        let storage = SqliteStorage::open(temp_file.path()).unwrap();

        let entity = storage.get_entity(999);
        assert!(entity.is_none(), "Should return None for non-existent entity");
    }
}
