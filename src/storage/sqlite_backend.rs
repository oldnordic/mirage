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

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path as StdPath;

use super::{CfgBlockData, DocumentInfo, FactInfo, StorageTrait};
use crate::cfg::Path;

/// Convert string from database to PathKind
fn str_to_path_kind(s: &str) -> Result<crate::cfg::PathKind> {
    match s {
        "Normal" => Ok(crate::cfg::PathKind::Normal),
        "Error" => Ok(crate::cfg::PathKind::Error),
        "Degenerate" => Ok(crate::cfg::PathKind::Degenerate),
        "Unreachable" => Ok(crate::cfg::PathKind::Unreachable),
        _ => anyhow::bail!("Unknown path kind: {}", s),
    }
}

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
    pub fn open(db_path: &StdPath) -> Result<Self> {
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
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT id, kind, terminator, byte_start, byte_end,
                    start_line, start_col, end_line, end_col,
                    coord_x, coord_y, coord_z
             FROM cfg_blocks
             WHERE function_id = ?
             ORDER BY id ASC",
            )
            .map_err(|e| anyhow::anyhow!("Failed to prepare cfg_blocks query: {}", e))?;

        let blocks = stmt
            .query_map(params![function_id], |row| {
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
                    // 4D spatial coordinates from Magellan's cfg_blocks table
                    coord_x: row.get::<_, Option<i64>>(9)?.unwrap_or(0),
                    coord_y: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
                    coord_z: row.get::<_, Option<i64>>(11)?.unwrap_or(0),
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
    fn get_cached_paths(&self, function_id: i64) -> Result<Option<Vec<Path>>> {
        // Query cfg_paths table for all paths of this function
        let mut stmt = self
            .conn
            .prepare(
                "SELECT path_id, path_kind, entry_block, exit_block
             FROM cfg_paths
             WHERE function_id = ?1",
            )
            .context("Failed to prepare cfg_paths query")?;

        let path_rows = stmt
            .query_map(params![function_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .context("Failed to execute cfg_paths query")?;

        let mut paths = Vec::new();

        for path_row in path_rows {
            let (path_id, kind_str, entry, exit) = path_row?;
            let kind = str_to_path_kind(&kind_str)
                .with_context(|| format!("Invalid path kind: {}", kind_str))?;

            // Query cfg_path_elements for blocks in this path
            let mut elem_stmt = self
                .conn
                .prepare(
                    "SELECT block_id
                 FROM cfg_path_elements
                 WHERE path_id = ?1
                 ORDER BY sequence_order ASC",
                )
                .context("Failed to prepare cfg_path_elements query")?;

            let block_rows = elem_stmt
                .query_map(params![&path_id], |row| row.get::<_, i64>(0))
                .context("Failed to execute cfg_path_elements query")?;

            let mut blocks = Vec::new();
            for block_row in block_rows {
                let block_id: i64 = block_row?;
                // BlockId in Path is usize, convert from i64
                blocks.push(block_id as usize);
            }

            paths.push(Path {
                path_id,
                blocks,
                kind,
                entry: entry as usize,
                exit: exit as usize,
            });
        }

        if paths.is_empty() {
            Ok(None)
        } else {
            Ok(Some(paths))
        }
    }

    fn get_callees(&self, function_id: i64) -> Result<Vec<i64>> {
        // Magellan schema: functions have CALLER edges to call-site entities,
        // and call-site entities have CALLS edges to callee functions.
        let mut stmt = self
            .conn
            .prepare(
                "SELECT DISTINCT g2.to_id
                 FROM graph_edges g1
                 JOIN graph_edges g2 ON g1.to_id = g2.from_id
                 WHERE g1.from_id = ? AND g1.edge_type = 'CALLER'
                   AND g2.edge_type = 'CALLS'",
            )
            .map_err(|e| anyhow::anyhow!("Failed to prepare get_callees query: {}", e))?;

        let callees = stmt
            .query_map(params![function_id], |row| row.get::<_, i64>(0))
            .map_err(|e| anyhow::anyhow!("Failed to execute get_callees query: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("Failed to collect callee rows: {}", e))?;

        Ok(callees)
    }

    fn get_documents_for_function(&self, function_id: i64) -> Result<Vec<DocumentInfo>> {
        let tables_ok: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('source_documents', 'candidate_facts')",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            == 2;

        if !tables_ok {
            return Ok(Vec::new());
        }

        let func_name: Option<String> = self
            .conn
            .query_row(
                "SELECT name FROM graph_entities WHERE id = ?",
                rusqlite::params![function_id],
                |row| row.get(0),
            )
            .optional()
            .context("query function name")?
            .flatten();

        let name = match func_name {
            Some(n) => n,
            None => return Ok(Vec::new()),
        };

        self.conn
            .prepare(
                "SELECT DISTINCT sd.id, sd.path_or_uri, sd.source_kind, sd.title, sd.tags, sd.wikilinks
                 FROM source_documents sd
                 INNER JOIN candidate_facts cf ON cf.source_document_id = sd.id
                 WHERE cf.subject_key = ? AND cf.subject_type = 'function'
                 ORDER BY sd.path_or_uri",
            )?
            .query_map(rusqlite::params![name], |row| {
                Ok(DocumentInfo {
                    id: row.get(0)?,
                    path_or_uri: row.get(1)?,
                    source_kind: row.get(2)?,
                    title: row.get(3)?,
                    tags: row.get(4)?,
                    wikilinks: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("collect document rows: {}", e))
    }

    fn get_facts_for_function(&self, function_id: i64) -> Result<Vec<FactInfo>> {
        let table_exists: bool = self
            .conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='candidate_facts'",
                [],
                |row| row.get::<_, i32>(0),
            )
            .optional()
            .context("check candidate_facts table")?
            .is_some();

        if !table_exists {
            return Ok(Vec::new());
        }

        let func_name: Option<String> = self
            .conn
            .query_row(
                "SELECT name FROM graph_entities WHERE id = ?",
                rusqlite::params![function_id],
                |row| row.get(0),
            )
            .optional()
            .context("query function name")?
            .flatten();

        let name = match func_name {
            Some(n) => n,
            None => return Ok(Vec::new()),
        };

        self.conn
            .prepare(
                "SELECT candidate_id, subject_type, subject_key, predicate,
                        object_type, object_key, status, source_document_id
                 FROM candidate_facts
                 WHERE subject_key = ? AND subject_type = 'function'
                 ORDER BY predicate",
            )?
            .query_map(rusqlite::params![name], |row| {
                Ok(FactInfo {
                    candidate_id: row.get(0)?,
                    subject_type: row.get(1)?,
                    subject_key: row.get(2)?,
                    predicate: row.get(3)?,
                    object_type: row.get(4)?,
                    object_key: row.get(5)?,
                    status: row.get(6)?,
                    source_document_id: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("collect fact rows: {}", e))
    }

    fn list_source_documents(&self) -> Result<Vec<DocumentInfo>> {
        let table_exists: bool = self
            .conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='source_documents'",
                [],
                |row| row.get::<_, i32>(0),
            )
            .optional()
            .context("check source_documents table")?
            .is_some();

        if !table_exists {
            return Ok(Vec::new());
        }

        self.conn
            .prepare(
                "SELECT id, path_or_uri, source_kind, title, tags, wikilinks
                 FROM source_documents ORDER BY path_or_uri",
            )?
            .query_map([], |row| {
                Ok(DocumentInfo {
                    id: row.get(0)?,
                    path_or_uri: row.get(1)?,
                    source_kind: row.get(2)?,
                    title: row.get(3)?,
                    tags: row.get(4)?,
                    wikilinks: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("collect document rows: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        )
        .unwrap();

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
        )
        .unwrap();

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
                coord_x INTEGER DEFAULT 0,
                coord_y INTEGER DEFAULT 0,
                coord_z INTEGER DEFAULT 0,
                FOREIGN KEY (function_id) REFERENCES graph_entities(id)
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE INDEX idx_cfg_blocks_function ON cfg_blocks(function_id)",
            [],
        )
        .unwrap();

        // Create cfg_paths table
        conn.execute(
            "CREATE TABLE cfg_paths (
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
        )
        .unwrap();

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_cfg_paths_function ON cfg_paths(function_id)",
            [],
        )
        .unwrap();

        // Create cfg_path_elements table
        conn.execute(
            "CREATE TABLE cfg_path_elements (
                path_id TEXT NOT NULL,
                sequence_order INTEGER NOT NULL,
                block_id INTEGER NOT NULL,
                PRIMARY KEY (path_id, sequence_order),
                FOREIGN KEY (path_id) REFERENCES cfg_paths(path_id)
            )",
            [],
        )
        .unwrap();

        // Insert a test function
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data)
             VALUES ('Symbol', 'test_function', '/tmp/test.rs', '{\"kind\": \"Function\"}')",
            [],
        )
        .unwrap();

        // Insert test CFG blocks
        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                   start_line, start_col, end_line, end_col)
             VALUES (1, 'entry', 'fallthrough', 0, 10, 1, 0, 1, 10),
                    (1, 'normal', 'conditional', 10, 50, 2, 4, 5, 8),
                    (1, 'return', 'return', 50, 60, 5, 0, 5, 10)",
            [],
        )
        .unwrap();

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
        assert_eq!(
            blocks.len(),
            0,
            "Should return empty Vec for non-existent function"
        );
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
        assert!(
            entity.is_none(),
            "Should return None for non-existent entity"
        );
    }

    #[test]
    fn test_sqlite_storage_get_cached_paths_none_when_empty() {
        let temp_file = create_test_db();
        let storage = SqliteStorage::open(temp_file.path()).unwrap();

        // No cached paths for function 1
        let paths = storage.get_cached_paths(1).unwrap();
        assert!(paths.is_none(), "Should return None when no cached paths");
    }

    #[test]
    fn test_sqlite_storage_get_cached_paths_with_data() {
        let temp_file = create_test_db();
        let conn = Connection::open(temp_file.path()).unwrap();

        // Insert a test path into cfg_paths
        conn.execute(
            "INSERT INTO cfg_paths (path_id, function_id, path_kind, entry_block, exit_block, length, created_at)
             VALUES ('test_path_123', 1, 'Normal', 100, 102, 3, 1000)",
            [],
        ).unwrap();

        // Insert path elements into cfg_path_elements
        conn.execute(
            "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id) VALUES
             ('test_path_123', 0, 100),
             ('test_path_123', 1, 101),
             ('test_path_123', 2, 102)",
            [],
        )
        .unwrap();

        let storage = SqliteStorage::open(temp_file.path()).unwrap();
        let paths = storage.get_cached_paths(1).unwrap();

        assert!(
            paths.is_some(),
            "Should return Some when cached paths exist"
        );
        let paths = paths.unwrap();
        assert_eq!(paths.len(), 1, "Should have 1 path");

        let path = &paths[0];
        assert_eq!(path.path_id, "test_path_123");
        assert_eq!(path.blocks, vec![100, 101, 102]);
        assert_eq!(path.kind, crate::cfg::PathKind::Normal);
        assert_eq!(path.entry, 100);
        assert_eq!(path.exit, 102);
    }
}
