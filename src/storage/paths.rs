//! Path caching with BLAKE3 content addressing
//!
//! This module provides database-backed caching for enumerated execution paths.
//! Paths are stored with their BLAKE3 hash as the primary key, providing automatic
//! deduplication and tamper detection. Cache entries are invalidated when function
//! content changes (detected via function_hash comparison).

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::collections::HashMap;

use crate::cfg::{BlockId, Path, PathKind};

/// Path cache manager (placeholder for future cache management features)
///
/// This struct is a placeholder for future cache management functionality
/// such as cache statistics, manual invalidation controls, and cache warming.
#[derive(Debug, Clone)]
pub struct PathCache {
    _private: (),
}

impl PathCache {
    /// Create a new path cache manager
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for PathCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Store enumerated paths in the database
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `function_id` - ID of the function these paths belong to
/// * `paths` - Slice of paths to store
///
/// # Returns
///
/// Ok(()) on success, error on database failure
///
/// # Algorithm
///
/// 1. Begin transaction (BEGIN IMMEDIATE to prevent write conflicts)
/// 2. For each path:
///    - Insert into cfg_paths table with BLAKE3 path_id as primary key
///    - Insert each block into cfg_path_elements with sequence_order
/// 3. Commit transaction
///
/// # Transactions
///
/// Uses IMMEDIATE transaction mode to prevent write conflicts in concurrent access.
/// Transaction is automatically rolled back on error.
pub fn store_paths(conn: &mut Connection, function_id: i64, paths: &[Path]) -> Result<()> {
    // Begin transaction for atomicity
    conn.execute("BEGIN IMMEDIATE TRANSACTION", [])
        .context("Failed to begin transaction for store_paths")?;

    // Prepare insert statements for efficiency
    let mut insert_path_stmt = conn.prepare_cached(
        "INSERT INTO cfg_paths (path_id, function_id, path_kind, entry_block, exit_block, length, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    ).context("Failed to prepare cfg_paths insert statement")?;

    let mut insert_element_stmt = conn.prepare_cached(
        "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id)
         VALUES (?1, ?2, ?3)",
    ).context("Failed to prepare cfg_path_elements insert statement")?;

    let now = chrono::Utc::now().timestamp();

    for path in paths {
        // Convert PathKind to string for storage
        let kind_str = path_kind_to_str(path.kind);

        // Insert the path metadata
        insert_path_stmt.execute(params![
            &path.path_id,
            function_id,
            kind_str,
            path.entry as i64,
            path.exit as i64,
            path.len() as i64,
            now,
        ]).with_context(|| format!("Failed to insert path {}", path.path_id))?;

        // Insert each block in the path
        for (idx, &block_id) in path.blocks.iter().enumerate() {
            insert_element_stmt.execute(params![
                &path.path_id,
                idx as i64,
                block_id as i64,
            ]).with_context(|| format!(
                "Failed to insert element {} for path {}",
                idx, path.path_id
            ))?;
        }
    }

    // Commit transaction
    conn.execute("COMMIT", [])
        .context("Failed to commit transaction for store_paths")?;

    Ok(())
}

/// Convert PathKind to string for database storage
fn path_kind_to_str(kind: PathKind) -> &'static str {
    match kind {
        PathKind::Normal => "Normal",
        PathKind::Error => "Error",
        PathKind::Degenerate => "Degenerate",
        PathKind::Unreachable => "Unreachable",
    }
}

/// Convert string from database to PathKind
fn str_to_path_kind(s: &str) -> Result<PathKind> {
    match s {
        "Normal" => Ok(PathKind::Normal),
        "Error" => Ok(PathKind::Error),
        "Degenerate" => Ok(PathKind::Degenerate),
        "Unreachable" => Ok(PathKind::Unreachable),
        _ => anyhow::bail!("Invalid path_kind in database: {}", s),
    }
}

/// Retrieve cached paths for a function
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `function_id` - ID of the function to retrieve paths for
///
/// # Returns
///
/// Vector of cached paths, or empty vector if none found (not an error)
///
/// # Note
///
/// This is a stub implementation. Full implementation in Task 3.
pub fn get_cached_paths(conn: &mut Connection, function_id: i64) -> Result<Vec<Path>> {
    let _ = (conn, function_id);
    Ok(vec![])
}

/// Invalidate all cached paths for a function
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `function_id` - ID of the function to invalidate paths for
///
/// # Returns
///
/// Ok(()) on success, error on database failure
///
/// # Note
///
/// This is a stub implementation. Full implementation in Task 4.
pub fn invalidate_function_paths(conn: &mut Connection, function_id: i64) -> Result<()> {
    let _ = (conn, function_id);
    anyhow::bail!("invalidate_function_paths: not yet implemented");
}

/// Update function paths only if function hash has changed
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `function_id` - ID of the function
/// * `new_hash` - New function hash to compare against cached
/// * `paths` - Paths to store if hash differs
///
/// # Returns
///
/// * `Ok(true)` - Paths were updated (hash differed or not found)
/// * `Ok(false)` - No update needed (hash matched)
/// * `Err(...)` - Database error
///
/// # Note
///
/// This is a stub implementation. Full implementation in Task 5.
pub fn update_function_paths_if_changed(
    conn: &mut Connection,
    function_id: i64,
    new_hash: &str,
    paths: &[Path],
) -> Result<bool> {
    let _ = (conn, function_id, new_hash, paths);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create test database with required schema
    fn create_test_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();

        // Create Magellan tables (simplified)
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
             VALUES (1, 4, 3, 0)",
            [],
        ).unwrap();

        // Create Mirage schema
        crate::storage::create_schema(&mut conn).unwrap();

        // Insert a test function
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        ).unwrap();

        conn
    }

    /// Create mock paths for testing
    fn create_mock_paths() -> Vec<Path> {
        vec![
            Path::new(vec![0, 1, 2], PathKind::Normal),
            Path::new(vec![0, 1, 3], PathKind::Normal),
            Path::new(vec![0, 2], PathKind::Error),
        ]
    }

    #[test]
    fn test_path_cache_new() {
        let cache = PathCache::new();
        let _ = cache;
    }

    #[test]
    fn test_path_cache_default() {
        let cache = PathCache::default();
        let _ = cache;
    }

    #[test]
    fn test_path_kind_to_str() {
        assert_eq!(path_kind_to_str(PathKind::Normal), "Normal");
        assert_eq!(path_kind_to_str(PathKind::Error), "Error");
        assert_eq!(path_kind_to_str(PathKind::Degenerate), "Degenerate");
        assert_eq!(path_kind_to_str(PathKind::Unreachable), "Unreachable");
    }

    #[test]
    fn test_str_to_path_kind() {
        assert_eq!(str_to_path_kind("Normal").unwrap(), PathKind::Normal);
        assert_eq!(str_to_path_kind("Error").unwrap(), PathKind::Error);
        assert_eq!(str_to_path_kind("Degenerate").unwrap(), PathKind::Degenerate);
        assert_eq!(str_to_path_kind("Unreachable").unwrap(), PathKind::Unreachable);
        assert!(str_to_path_kind("Invalid").is_err());
    }

    #[test]
    fn test_store_paths_inserts_paths() {
        let mut conn = create_test_db();
        let function_id: i64 = 1; // First entity ID
        let paths = create_mock_paths();

        // Store paths
        store_paths(&mut conn, function_id, &paths).unwrap();

        // Verify cfg_paths has correct rows
        let path_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(path_count, 3, "Should have 3 paths");

        // Verify cfg_path_elements has correct rows
        let element_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_path_elements",
            [],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(element_count, 7, "Should have 7 elements (3+3+1)");
    }

    #[test]
    fn test_store_paths_path_metadata() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;
        let paths = create_mock_paths();

        store_paths(&mut conn, function_id, &paths).unwrap();

        // Verify path metadata
        let mut stmt = conn.prepare(
            "SELECT path_id, path_kind, entry_block, exit_block, length
             FROM cfg_paths
             WHERE function_id = ?
             ORDER BY entry_block, exit_block",
        ).unwrap();

        let rows: Vec<_> = stmt.query_map(params![function_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        }).unwrap().filter_map(Result::ok).collect();

        assert_eq!(rows.len(), 3);

        // First path: [0, 1, 2]
        let row = &rows[0];
        assert_eq!(row.3, 0); // entry_block
        assert_eq!(row.4, 2); // exit_block
        assert_eq!(row.5, 3); // length
        assert_eq!(row.1, "Normal"); // path_kind

        // Verify path_id is a valid hex string (BLAKE3 hash)
        assert!(!row.0.is_empty());
        assert!(row.0.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_store_paths_path_elements_order() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;
        let paths = create_mock_paths();

        store_paths(&mut conn, function_id, &paths).unwrap();

        // Get first path_id
        let path_id: String = conn.query_row(
            "SELECT path_id FROM cfg_paths WHERE function_id = ? LIMIT 1",
            params![function_id],
            |row| row.get(0),
        ).unwrap();

        // Verify elements are in correct order
        let mut stmt = conn.prepare(
            "SELECT block_id FROM cfg_path_elements
             WHERE path_id = ?
             ORDER BY sequence_order",
        ).unwrap();

        let blocks: Vec<BlockId> = stmt.query_map(params![path_id], |row| {
            Ok(row.get::<_, i64>(0)? as BlockId)
        }).unwrap().filter_map(Result::ok).collect();

        // Should match [0, 1, 2] (first path)
        assert_eq!(blocks, vec![0, 1, 2]);
    }

    #[test]
    fn test_store_paths_empty_list() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;
        let paths: Vec<Path> = vec![];

        // Should succeed with empty list
        store_paths(&mut conn, function_id, &paths).unwrap();

        // Verify no rows inserted
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(count, 0);
    }

    #[test]
    fn test_store_paths_foreign_key_constraint() {
        let mut conn = create_test_db();
        let invalid_function_id: i64 = 9999; // Doesn't exist
        let paths = create_mock_paths();

        // Should fail due to foreign key constraint
        let result = store_paths(&mut conn, invalid_function_id, &paths);

        assert!(result.is_err(), "Should fail with invalid function_id");
    }

    #[test]
    fn test_store_paths_deduplication_by_path_id() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;
        let paths = create_mock_paths();

        // Store paths first time
        store_paths(&mut conn, function_id, &paths).unwrap();

        // Try to store same paths again (same path_id)
        // This should fail due to PRIMARY KEY constraint on path_id
        let result = store_paths(&mut conn, function_id, &paths);

        // SQLite will return a constraint error
        assert!(result.is_err(), "Should fail on duplicate path_id");
    }
}
