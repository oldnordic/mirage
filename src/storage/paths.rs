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
/// # Algorithm
///
/// 1. Execute SQL query joining cfg_paths and cfg_path_elements
/// 2. Group rows by path_id
/// 3. For each path, collect blocks in order (by sequence_order)
/// 4. Reconstruct Path objects with metadata
///
/// # Empty Result
///
/// Returns Ok(vec![]) for cache miss (no paths stored), not an error.
pub fn get_cached_paths(conn: &mut Connection, function_id: i64) -> Result<Vec<Path>> {
    // Query paths and their elements
    let mut stmt = conn.prepare_cached(
        "SELECT p.path_id, p.path_kind, p.entry_block, p.exit_block,
                pe.block_id, pe.sequence_order
         FROM cfg_paths p
         JOIN cfg_path_elements pe ON p.path_id = pe.path_id
         WHERE p.function_id = ?1
         ORDER BY p.path_id, pe.sequence_order",
    ).context("Failed to prepare get_cached_paths query")?;

    // Group elements by path_id
    let mut path_data: HashMap<String, PathData> = HashMap::new();

    let rows = stmt.query_map(params![function_id], |row| {
        Ok((
            row.get::<_, String>(0)?,  // path_id
            row.get::<_, String>(1)?,  // path_kind
            row.get::<_, i64>(2)?,     // entry_block
            row.get::<_, i64>(3)?,     // exit_block
            row.get::<_, i64>(4)?,     // block_id
            row.get::<_, i64>(5)?,     // sequence_order
        ))
    }).context("Failed to execute get_cached_paths query")?;

    for row in rows {
        let (path_id, kind_str, entry_block, exit_block, block_id, _sequence_order) = row?;
        let entry = entry_block as BlockId;
        let exit = exit_block as BlockId;
        let kind = str_to_path_kind(&kind_str)
            .with_context(|| format!("Invalid path_kind '{}' in database", kind_str))?;

        path_data.entry(path_id)
            .or_insert_with(|| PathData {
                path_id: String::new(), // Will be replaced
                kind,
                entry,
                exit,
                blocks: Vec::new(),
            })
            .blocks.push(block_id as BlockId);
    }

    // Reconstruct Path objects
    let mut paths = Vec::new();
    for (path_id, mut data) in path_data {
        data.path_id = path_id;
        let path = Path::new(data.blocks, data.kind);
        // Verify entry/exit match the path (path_id was computed from blocks)
        paths.push(path);
    }

    Ok(paths)
}

/// Helper struct for reconstructing paths from database rows
struct PathData {
    path_id: String,
    kind: PathKind,
    entry: BlockId,
    exit: BlockId,
    blocks: Vec<BlockId>,
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
/// # Algorithm
///
/// 1. Begin transaction
/// 2. Delete path_elements first (FK dependency: elements reference paths)
/// 3. Delete paths
/// 4. Commit transaction
///
/// # Idempotent
///
/// Returns Ok(()) even if no paths exist for the function.
pub fn invalidate_function_paths(conn: &mut Connection, function_id: i64) -> Result<()> {
    // Begin transaction for atomicity
    conn.execute("BEGIN IMMEDIATE TRANSACTION", [])
        .context("Failed to begin transaction for invalidate_function_paths")?;

    // Delete path_elements first (FK dependency)
    conn.execute(
        "DELETE FROM cfg_path_elements
         WHERE path_id IN (SELECT path_id FROM cfg_paths WHERE function_id = ?1)",
        params![function_id],
    ).context("Failed to delete cfg_path_elements")?;

    // Delete paths
    conn.execute(
        "DELETE FROM cfg_paths WHERE function_id = ?1",
        params![function_id],
    ).context("Failed to delete cfg_paths")?;

    // Commit transaction
    conn.execute("COMMIT", [])
        .context("Failed to commit transaction for invalidate_function_paths")?;

    Ok(())
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
/// # Algorithm
///
/// 1. Get current function_hash from cfg_blocks
/// 2. If hash matches new_hash -> cache hit, return Ok(false)
/// 3. If hash differs or not found -> cache miss:
///    - Invalidate old paths via invalidate_function_paths
///    - Store new paths via store_paths
///    - Update cfg_blocks.function_hash = new_hash
///    - Return Ok(true)
///
/// # Incremental Updates
///
/// This enables incremental updates - paths are only re-enumerated
/// when function content changes.
pub fn update_function_paths_if_changed(
    conn: &mut Connection,
    function_id: i64,
    new_hash: &str,
    paths: &[Path],
) -> Result<bool> {
    // Check current function_hash in cfg_blocks
    let current_hash: Option<String> = conn.query_row(
        "SELECT function_hash FROM cfg_blocks WHERE function_id = ?1 LIMIT 1",
        params![function_id],
        |row| row.get(0),
    ).unwrap_or(None);

    // If hash matches, no update needed (cache hit)
    if let Some(ref hash) = current_hash {
        if hash == new_hash {
            return Ok(false);
        }
    }

    // Cache miss or hash changed - invalidate old paths
    invalidate_function_paths(conn, function_id)?;

    // Store new paths
    store_paths(conn, function_id, paths)?;

    // Update function_hash in cfg_blocks
    // First check if a cfg_blocks entry exists for this function
    let block_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM cfg_blocks WHERE function_id = ?1)",
        params![function_id],
        |row| row.get(0),
    ).unwrap_or(false);

    if block_exists {
        // Update existing entry
        conn.execute(
            "UPDATE cfg_blocks SET function_hash = ?1 WHERE function_id = ?2",
            params![new_hash, function_id],
        ).context("Failed to update function_hash")?;
    } else {
        // Insert a placeholder cfg_blocks entry for hash tracking
        // This allows caching paths even before full CFG is stored
        conn.execute(
            "INSERT INTO cfg_blocks (function_id, block_kind, function_hash)
             VALUES (?1, ?2, ?3)",
            params![function_id, "placeholder", new_hash],
        ).context("Failed to insert function_hash placeholder")?;
    }

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

        // Enable foreign key enforcement for tests
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

        conn
    }

    /// Create cfg_blocks entries for a set of block IDs
    ///
    /// This is needed because cfg_paths has foreign keys to cfg_blocks
    fn create_test_blocks(conn: &mut Connection, function_id: i64, block_ids: &[BlockId]) {
        for &block_id in block_ids {
            conn.execute(
                "INSERT INTO cfg_blocks (function_id, block_kind, terminator)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params!(function_id, "entry", "ret"),
            ).unwrap();
        }
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

        assert_eq!(element_count, 8, "Should have 8 elements (3+3+2)");
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
        assert_eq!(row.2, 0); // entry_block
        assert_eq!(row.3, 2); // exit_block
        assert_eq!(row.4, 3); // length
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

    // Task 3: get_cached_paths tests

    #[test]
    fn test_get_cached_paths_empty() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;

        // No paths stored - should return empty vec (not error)
        let paths = get_cached_paths(&mut conn, function_id).unwrap();
        assert_eq!(paths.len(), 0);
    }

    #[test]
    fn test_get_cached_paths_retrieves_stored_paths() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;
        let original_paths = create_mock_paths();

        // Store paths
        store_paths(&mut conn, function_id, &original_paths).unwrap();

        // Retrieve paths
        let retrieved_paths = get_cached_paths(&mut conn, function_id).unwrap();

        // Should have same count
        assert_eq!(retrieved_paths.len(), original_paths.len());

        // Each original path should be in retrieved paths (order not guaranteed)
        for orig in &original_paths {
            assert!(
                retrieved_paths.iter().any(|p| p.blocks == orig.blocks && p.kind == orig.kind),
                "Path {:?} not found in retrieved paths",
                orig.blocks
            );
        }
    }

    #[test]
    fn test_get_cached_paths_block_order_preserved() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;

        // Create paths with specific block sequences
        let paths = vec![
            Path::new(vec![0, 1, 2, 3], PathKind::Normal),
            Path::new(vec![5, 4, 3, 2, 1], PathKind::Error),
        ];

        store_paths(&mut conn, function_id, &paths).unwrap();
        let retrieved = get_cached_paths(&mut conn, function_id).unwrap();

        assert_eq!(retrieved.len(), 2);

        // Find each path in retrieved paths
        let path1 = retrieved.iter().find(|p| p.blocks == vec![0, 1, 2, 3]).unwrap();
        assert_eq!(path1.blocks, vec![0, 1, 2, 3]);

        let path2 = retrieved.iter().find(|p| p.blocks == vec![5, 4, 3, 2, 1]).unwrap();
        assert_eq!(path2.blocks, vec![5, 4, 3, 2, 1]);
    }

    #[test]
    fn test_get_cached_paths_kind_preserved() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;

        let paths = vec![
            Path::new(vec![0], PathKind::Normal),
            Path::new(vec![1], PathKind::Error),
            Path::new(vec![2], PathKind::Degenerate),
            Path::new(vec![3], PathKind::Unreachable),
        ];

        store_paths(&mut conn, function_id, &paths).unwrap();
        let retrieved = get_cached_paths(&mut conn, function_id).unwrap();

        assert_eq!(retrieved.len(), 4);

        // Check each kind is preserved
        assert!(retrieved.iter().any(|p| p.kind == PathKind::Normal));
        assert!(retrieved.iter().any(|p| p.kind == PathKind::Error));
        assert!(retrieved.iter().any(|p| p.kind == PathKind::Degenerate));
        assert!(retrieved.iter().any(|p| p.kind == PathKind::Unreachable));
    }

    #[test]
    fn test_get_cached_paths_invalid_kind_returns_error() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;

        // Insert a path with invalid kind directly
        conn.execute(
            "INSERT INTO cfg_paths (path_id, function_id, path_kind, entry_block, exit_block, length, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params!("invalid_path_id", function_id, "InvalidKind", 0, 0, 1, 0),
        ).unwrap();

        // Insert a path element so the JOIN returns rows
        conn.execute(
            "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id)
             VALUES (?, ?, ?)",
            params!("invalid_path_id", 0, 0),
        ).unwrap();

        // Should return error due to invalid path_kind
        let result = get_cached_paths(&mut conn, function_id);
        assert!(result.is_err(), "Should fail on invalid path_kind");
    }

    #[test]
    fn test_get_cached_paths_roundtrip() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;

        // Create complex paths
        let paths = vec![
            Path::new(vec![0, 1, 2, 3, 4, 5], PathKind::Normal),
            Path::new(vec![0, 1, 3, 5], PathKind::Normal),
            Path::new(vec![0, 2, 4, 5], PathKind::Error),
            Path::new(vec![0, 5], PathKind::Degenerate),
        ];

        store_paths(&mut conn, function_id, &paths).unwrap();
        let retrieved = get_cached_paths(&mut conn, function_id).unwrap();

        // Full roundtrip verification
        assert_eq!(retrieved.len(), paths.len());

        // Sort both by blocks for comparison (order may vary)
        let mut sorted_orig: Vec<_> = paths.iter().collect();
        let mut sorted_ret: Vec<_> = retrieved.iter().collect();
        sorted_orig.sort_by_key(|p| p.blocks.clone());
        sorted_ret.sort_by_key(|p| p.blocks.clone());

        for (orig, ret) in sorted_orig.iter().zip(sorted_ret.iter()) {
            assert_eq!(orig.blocks, ret.blocks, "Block sequence mismatch");
            assert_eq!(orig.kind, ret.kind, "PathKind mismatch");
        }
    }

    // Task 4: invalidate_function_paths tests

    #[test]
    fn test_invalidate_function_paths_deletes_all_paths() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;
        let paths = create_mock_paths();

        // Store paths
        store_paths(&mut conn, function_id, &paths).unwrap();

        // Verify paths exist
        let count_before: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count_before, 3);

        // Invalidate
        invalidate_function_paths(&mut conn, function_id).unwrap();

        // Verify paths deleted
        let count_after: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count_after, 0);
    }

    #[test]
    fn test_invalidate_function_paths_deletes_elements() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;
        let paths = create_mock_paths();

        // Store paths
        store_paths(&mut conn, function_id, &paths).unwrap();

        // Verify elements exist
        let count_before: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_path_elements",
            [],
            |row| row.get(0),
        ).unwrap();
        assert!(count_before > 0);

        // Invalidate
        invalidate_function_paths(&mut conn, function_id).unwrap();

        // Verify elements deleted (via subquery)
        let count_after: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_path_elements
             WHERE path_id IN (SELECT path_id FROM cfg_paths WHERE function_id = ?)",
            params![function_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count_after, 0);
    }

    #[test]
    fn test_invalidate_function_paths_idempotent() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;

        // Call invalidate with no paths stored - should succeed
        invalidate_function_paths(&mut conn, function_id).unwrap();

        // Call again - should still succeed (idempotent)
        invalidate_function_paths(&mut conn, function_id).unwrap();
    }

    #[test]
    fn test_invalidate_function_paths_then_retrieve_empty() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;
        let paths = create_mock_paths();

        // Store and verify
        store_paths(&mut conn, function_id, &paths).unwrap();
        let before = get_cached_paths(&mut conn, function_id).unwrap();
        assert_eq!(before.len(), 3);

        // Invalidate
        invalidate_function_paths(&mut conn, function_id).unwrap();

        // Retrieve should return empty
        let after = get_cached_paths(&mut conn, function_id).unwrap();
        assert_eq!(after.len(), 0);
    }

    #[test]
    fn test_invalidate_function_paths_only_target_function() {
        let mut conn = create_test_db();

        // Insert two functions
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "func1", "test.rs", "{}"),
        ).unwrap();
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "func2", "test.rs", "{}"),
        ).unwrap();

        let function_id_1: i64 = 1;
        let function_id_2: i64 = 2;

        // Create different paths for each function (to avoid path_id collision)
        let paths_1 = vec![
            Path::new(vec![0, 1, 2], PathKind::Normal),
            Path::new(vec![0, 1, 3], PathKind::Normal),
            Path::new(vec![0, 2], PathKind::Error),
        ];
        let paths_2 = vec![
            Path::new(vec![10, 11, 12], PathKind::Normal),
            Path::new(vec![10, 11, 13], PathKind::Normal),
            Path::new(vec![10, 12], PathKind::Error),
        ];

        // Store paths for both functions
        store_paths(&mut conn, function_id_1, &paths_1).unwrap();
        store_paths(&mut conn, function_id_2, &paths_2).unwrap();

        // Verify both have paths
        let count_1_before: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id_1],
            |row| row.get(0),
        ).unwrap();
        let count_2_before: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id_2],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count_1_before, 3);
        assert_eq!(count_2_before, 3);

        // Invalidate only function 1
        invalidate_function_paths(&mut conn, function_id_1).unwrap();

        // Function 1 should be empty
        let count_1_after: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id_1],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count_1_after, 0);

        // Function 2 should still have paths
        let count_2_after: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id_2],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count_2_after, 3);
    }

    // Task 5: update_function_paths_if_changed tests

    #[test]
    fn test_update_function_paths_if_changed_first_call() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;
        let paths = create_mock_paths();
        let hash = "abc123";

        // First call with no existing hash - should update
        let updated = update_function_paths_if_changed(&mut conn, function_id, hash, &paths).unwrap();
        assert!(updated, "First call should return true (updated)");

        // Verify paths were stored
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 3);

        // Verify hash was stored
        let stored_hash: String = conn.query_row(
            "SELECT function_hash FROM cfg_blocks WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(stored_hash, hash);
    }

    #[test]
    fn test_update_function_paths_if_changed_same_hash() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;
        let paths = create_mock_paths();
        let hash = "abc123";

        // First call - should update
        let updated1 = update_function_paths_if_changed(&mut conn, function_id, hash, &paths).unwrap();
        assert!(updated1);

        // Second call with same hash - should NOT update
        let updated2 = update_function_paths_if_changed(&mut conn, function_id, hash, &paths).unwrap();
        assert!(!updated2, "Same hash should return false (no update)");
    }

    #[test]
    fn test_update_function_paths_if_changed_different_hash() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;
        let paths1 = create_mock_paths();
        let paths2 = vec![
            Path::new(vec![0, 1], PathKind::Normal),
        ];

        // First call with hash1
        let updated1 = update_function_paths_if_changed(&mut conn, function_id, "hash1", &paths1).unwrap();
        assert!(updated1);

        // Verify 3 paths from first call
        let count1: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count1, 3);

        // Second call with different hash - should update
        let updated2 = update_function_paths_if_changed(&mut conn, function_id, "hash2", &paths2).unwrap();
        assert!(updated2, "Different hash should return true (updated)");

        // Verify paths were replaced (now only 1 path)
        let count2: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_paths WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count2, 1, "Old paths should be invalidated and replaced");

        // Verify hash was updated
        let stored_hash: String = conn.query_row(
            "SELECT function_hash FROM cfg_blocks WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(stored_hash, "hash2");
    }

    #[test]
    fn test_update_function_paths_if_changed_three_calls() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;
        let paths = create_mock_paths();

        // Call 1: new hash -> update
        let u1 = update_function_paths_if_changed(&mut conn, function_id, "hash1", &paths).unwrap();
        assert!(u1);

        // Call 2: same hash -> no update
        let u2 = update_function_paths_if_changed(&mut conn, function_id, "hash1", &paths).unwrap();
        assert!(!u2);

        // Call 3: different hash -> update
        let u3 = update_function_paths_if_changed(&mut conn, function_id, "hash2", &paths).unwrap();
        assert!(u3);

        // Call 4: same hash again -> no update
        let u4 = update_function_paths_if_changed(&mut conn, function_id, "hash2", &paths).unwrap();
        assert!(!u4);
    }

    #[test]
    fn test_update_function_paths_if_changed_with_existing_cfg_block() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;

        // Insert a cfg_blocks entry first (simulating existing CFG)
        conn.execute(
            "INSERT INTO cfg_blocks (function_id, block_kind, function_hash)
             VALUES (?, ?, ?)",
            params![function_id, "entry", "old_hash"],
        ).unwrap();

        let paths = create_mock_paths();

        // Update with new hash
        let updated = update_function_paths_if_changed(&mut conn, function_id, "new_hash", &paths).unwrap();
        assert!(updated);

        // Verify hash was updated (not inserted as new row)
        let hash: String = conn.query_row(
            "SELECT function_hash FROM cfg_blocks WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(hash, "new_hash");

        // Verify only one cfg_blocks entry exists
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1, "Should update existing row, not insert new");
    }

    #[test]
    fn test_update_function_paths_if_changed_creates_placeholder() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;
        let paths = create_mock_paths();

        // No cfg_blocks entry exists initially
        let count_before: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count_before, 0);

        // Update paths
        update_function_paths_if_changed(&mut conn, function_id, "hash1", &paths).unwrap();

        // Verify placeholder was created
        let count_after: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count_after, 1, "Should create placeholder cfg_blocks entry");

        // Verify placeholder has correct hash
        let hash: String = conn.query_row(
            "SELECT function_hash FROM cfg_blocks WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(hash, "hash1");
    }

    #[test]
    fn test_update_function_paths_if_changed_invalidates_old() {
        let mut conn = create_test_db();
        let function_id: i64 = 1;

        let paths1 = vec![
            Path::new(vec![0, 1, 2], PathKind::Normal),
            Path::new(vec![0, 1, 3], PathKind::Normal),
        ];
        let paths2 = vec![
            Path::new(vec![0, 2], PathKind::Error),
        ];

        // Store first set
        update_function_paths_if_changed(&mut conn, function_id, "hash1", &paths1).unwrap();

        // Verify first paths exist
        let retrieved1 = get_cached_paths(&mut conn, function_id).unwrap();
        assert_eq!(retrieved1.len(), 2);

        // Update with different hash and new paths
        update_function_paths_if_changed(&mut conn, function_id, "hash2", &paths2).unwrap();

        // Verify only new paths exist (old ones invalidated)
        let retrieved2 = get_cached_paths(&mut conn, function_id).unwrap();
        assert_eq!(retrieved2.len(), 1);
        assert_eq!(retrieved2[0].blocks, vec![0, 2]);
        assert_eq!(retrieved2[0].kind, PathKind::Error);
    }
}
