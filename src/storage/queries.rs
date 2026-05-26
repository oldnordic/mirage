use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

use super::MirageDb;

/// Store a CFG in the database for a given function
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `function_id` - ID of the function in graph_entities
/// * `function_hash` - BLAKE3 hash of the function body for incremental updates
/// * `cfg` - The control flow graph to store
///
/// # Notes
///
/// - DEPRECATED: Magellan handles CFG storage via cfg_blocks. Edges are computed in memory.
/// - This function is kept for backward compatibility with existing tests.
/// - cfg_edges table is managed by Magellan v11+; Mirage does not create or query it.
/// - Uses BEGIN IMMEDIATE to acquire write lock early (prevents write conflicts)
/// - Existing blocks are cleared for incremental updates
/// - Block IDs are AUTOINCREMENT in the database
#[deprecated(note = "Magellan handles CFG storage via cfg_blocks. Edges are computed in memory.")]
#[allow(deprecated)]
pub fn store_cfg(
    conn: &mut Connection,
    function_id: i64,
    _function_hash: &str,
    cfg: &crate::cfg::Cfg,
) -> Result<()> {
    use crate::cfg::{BlockKind, Terminator};

    conn.execute("BEGIN IMMEDIATE TRANSACTION", [])
        .context("Failed to begin transaction")?;

    conn.execute(
        "DELETE FROM cfg_blocks WHERE function_id = ?",
        params![function_id],
    )
    .context("Failed to clear existing cfg_blocks")?;

    let mut block_id_map: std::collections::HashMap<petgraph::graph::NodeIndex, i64> =
        std::collections::HashMap::new();

    let mut insert_block = conn
        .prepare_cached(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                  start_line, start_col, end_line, end_col,
                                  coord_x, coord_y, coord_z)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .context("Failed to prepare block insert statement")?;

    for node_idx in cfg.node_indices() {
        let block = cfg
            .node_weight(node_idx)
            .context("CFG node has no weight")?;

        let terminator_str = match &block.terminator {
            Terminator::Goto { .. } => "goto",
            Terminator::SwitchInt { .. } => "conditional",
            Terminator::Return => "return",
            Terminator::Call { .. } => "call",
            Terminator::Abort(msg) if msg == "break" => "break",
            Terminator::Abort(msg) if msg == "continue" => "continue",
            Terminator::Abort(msg) if msg == "panic" => "panic",
            _ => "fallthrough",
        };

        let (byte_start, byte_end) = block
            .source_location
            .as_ref()
            .map(|loc| (Some(loc.byte_start as i64), Some(loc.byte_end as i64)))
            .unwrap_or((None, None));

        let (start_line, start_col, end_line, end_col) = block
            .source_location
            .as_ref()
            .map(|loc| {
                (
                    Some(loc.start_line as i64),
                    Some(loc.start_column as i64),
                    Some(loc.end_line as i64),
                    Some(loc.end_column as i64),
                )
            })
            .unwrap_or((None, None, None, None));

        let kind = match block.kind {
            BlockKind::Entry => "entry",
            BlockKind::Normal => "block",
            BlockKind::Exit => "return",
        };

        insert_block
            .execute(params![
                function_id,
                kind,
                terminator_str,
                byte_start,
                byte_end,
                start_line,
                start_col,
                end_line,
                end_col,
                block.coord_x,
                block.coord_y,
                block.coord_z,
            ])
            .context("Failed to insert cfg_block")?;

        let db_id = conn.last_insert_rowid();
        block_id_map.insert(node_idx, db_id);
    }

    conn.execute("COMMIT", [])
        .context("Failed to commit transaction")?;

    Ok(())
}

/// Check if a function is already indexed in the database
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `function_id` - ID of the function to check
///
/// # Returns
///
/// * `true` - Function has CFG blocks stored
/// * `false` - Function not indexed
pub fn function_exists(conn: &Connection, function_id: i64) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
        params![function_id],
        |row| row.get::<_, i64>(0).map(|count| count > 0),
    )
    .optional()
    .ok()
    .flatten()
    .unwrap_or(false)
}

/// Get the stored hash for a function
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `function_id` - ID of the function
///
/// # Returns
///
/// * `Some(hash)` - The stored BLAKE3 hash if function exists
/// * `None` - Function not found or no hash stored
///
/// # Note
///
/// Magellan's cfg_blocks table doesn't store function_hash, so this function
/// always returns None when using Magellan's schema. The hash functionality
/// is only available when using Mirage's legacy schema.
pub fn get_function_hash(conn: &Connection, function_id: i64) -> Option<String> {
    let cfg_hash: Option<String> = conn
        .query_row(
            "SELECT cfg_hash FROM cfg_blocks WHERE function_id = ? LIMIT 1",
            params![function_id],
            |row| row.get(0),
        )
        .optional()
        .ok()
        .flatten();

    if cfg_hash.is_some() {
        return cfg_hash;
    }

    conn.query_row(
        "SELECT json_extract(data, '$.symbol_id') FROM graph_entities WHERE id = ? LIMIT 1",
        params![function_id],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()
    .ok()
    .flatten()
    .flatten()
}

/// Compare two function hashes and return true if they differ
///
/// Used by the index command to decide whether to skip a function.
///
/// # Note
///
/// Compare stored cfg_hash against new hash to detect function changes.
/// Returns true if hashes differ or no hash is found (indicating re-indexing needed).
pub fn hash_changed(conn: &Connection, function_id: i64, _new_hash: &str) -> Result<bool> {
    let old_hash: Option<String> = conn
        .query_row(
            "SELECT cfg_hash FROM cfg_blocks WHERE function_id = ? LIMIT 1",
            params![function_id],
            |row| row.get(0),
        )
        .optional()?;

    match old_hash {
        Some(old) => Ok(old != _new_hash),
        None => Ok(true),
    }
}

/// Compute the set of functions that need re-indexing based on git changes
///
/// This uses git diff to find changed Rust files, then queries the database
/// for functions defined in those files.
///
/// # Notes
///
/// - Uses `git diff --name-only HEAD` to detect changed files
/// - Only considers .rs files
/// - Returns functions from changed files based on graph_entities table
pub fn get_changed_functions(
    conn: &Connection,
    project_path: &std::path::Path,
) -> Result<std::collections::HashSet<String>> {
    use std::collections::HashSet;
    use std::process::Command;

    let mut changed = HashSet::new();

    if let Ok(git_output) = Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .current_dir(project_path)
        .output()
    {
        let git_files = String::from_utf8_lossy(&git_output.stdout);

        let changed_rs_files: Vec<&str> =
            git_files.lines().filter(|f| f.ends_with(".rs")).collect();

        if changed_rs_files.is_empty() {
            return Ok(changed);
        }

        for file in changed_rs_files {
            let normalized_path = if file.starts_with('/') {
                file.trim_start_matches('/')
            } else {
                file
            };

            let mut stmt = conn
                .prepare_cached(
                    "SELECT name FROM graph_entities
                 WHERE kind = 'function' AND (
                     file_path = ? OR
                     file_path = ? OR
                     file_path LIKE '%' || ?
                 )",
                )
                .context("Failed to prepare function lookup query")?;

            let with_slash = format!("/{}", normalized_path);

            let rows = stmt
                .query_map(
                    params![normalized_path, &with_slash, normalized_path],
                    |row| row.get::<_, String>(0),
                )
                .context("Failed to execute function lookup")?;

            for func_name in rows.flatten() {
                changed.insert(func_name);
            }
        }
    }

    Ok(changed)
}

/// Get the file containing a function
pub fn get_function_file(conn: &Connection, function_name: &str) -> Result<Option<String>> {
    let file: Option<String> = conn
        .query_row(
            "SELECT file_path FROM graph_entities WHERE kind = 'function' AND name = ? LIMIT 1",
            params![function_name],
            |row| row.get(0),
        )
        .optional()?;

    Ok(file)
}

/// Get the function name for a given block ID
pub fn get_function_name(conn: &Connection, function_id: i64) -> Option<String> {
    conn.query_row(
        "SELECT name FROM graph_entities WHERE id = ?",
        params![function_id],
        |row| row.get(0),
    )
    .optional()
    .ok()
    .flatten()
}

/// Get path elements (blocks in order) for a given path_id
pub fn get_path_elements(conn: &Connection, path_id: &str) -> Result<Vec<crate::cfg::BlockId>> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT block_id FROM cfg_path_elements
         WHERE path_id = ?
         ORDER BY sequence_order ASC",
        )
        .context("Failed to prepare path elements query")?;

    let blocks: Vec<crate::cfg::BlockId> = stmt
        .query_map(params![path_id], |row| Ok(row.get::<_, i64>(0)? as usize))
        .context("Failed to execute path elements query")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect path elements")?;

    if blocks.is_empty() {
        anyhow::bail!("Path '{}' not found in cache", path_id);
    }

    Ok(blocks)
}

/// Compute path impact from the database
///
/// This loads the path's blocks from the database and computes
/// the impact by aggregating reachable blocks from each path block.
pub fn compute_path_impact_from_db(
    conn: &Connection,
    path_id: &str,
    cfg: &crate::cfg::Cfg,
    max_depth: Option<usize>,
) -> Result<crate::cfg::PathImpact> {
    let path_blocks = get_path_elements(conn, path_id)?;

    let mut impact = crate::cfg::compute_path_impact(cfg, &path_blocks, max_depth);
    impact.path_id = path_id.to_string();

    Ok(impact)
}

/// Get the function name for a given function_id (backend-agnostic)
///
/// This is the main entry point for getting function names. It works with both
/// SQLite and geometric backends.
///
/// # Examples
///
/// ```no_run
/// # use mirage_analyzer::storage::{get_function_name_db, MirageDb};
/// # fn main() -> anyhow::Result<()> {
/// # let db = MirageDb::open("test.db")?;
/// if let Some(name) = get_function_name_db(&db, 123) {
///     println!("Function: {}", name);
/// }
/// # Ok(())
/// # }
/// ```
pub fn get_function_name_db(db: &MirageDb, function_id: i64) -> Option<String> {
    db.get_function_name(function_id)
}

/// Get the file path for a given function_id (backend-agnostic)
///
/// This is the main entry point for getting function file paths. It works with both
/// SQLite and geometric backends.
///
/// # Examples
///
/// ```no_run
/// # use mirage_analyzer::storage::{get_function_file_db, MirageDb};
/// # fn main() -> anyhow::Result<()> {
/// # let db = MirageDb::open("test.db")?;
/// if let Some(path) = get_function_file_db(&db, 123) {
///     println!("File: {}", path);
/// }
/// # Ok(())
/// # }
/// ```
pub fn get_function_file_db(db: &MirageDb, function_id: i64) -> Option<String> {
    db.get_function_file(function_id)
}

/// Get the function hash for path caching (backend-agnostic)
///
/// For SQLite backend: returns the stored hash if available
/// For geometric backend: always returns None (Magellan manages its own caching)
///
/// # Examples
///
/// ```no_run
/// # use mirage_analyzer::storage::{get_function_hash_db, MirageDb};
/// # fn main() -> anyhow::Result<()> {
/// # let db = MirageDb::open("test.db")?;
/// if let Some(hash) = get_function_hash_db(&db, 123) {
///     println!("Hash: {}", hash);
/// }
/// # Ok(())
/// # }
/// ```
pub fn get_function_hash_db(db: &MirageDb, function_id: i64) -> Option<String> {
    db.get_function_hash(function_id)
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn create_test_db_with_schema() -> Connection {
        crate::storage::schema::create_test_db_with_schema()
    }

    #[test]
    #[allow(deprecated)]
    fn test_store_cfg_retrieves_correctly() {
        use crate::cfg::{BasicBlock, BlockKind, Cfg, EdgeType, Terminator};

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
            rusqlite::params![super::super::REQUIRED_MAGELLAN_SCHEMA_VERSION, super::super::REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, 0],
        ).unwrap();

        super::super::schema::create_schema(&mut conn, super::super::TEST_MAGELLAN_SCHEMA_VERSION)
            .unwrap();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        )
        .unwrap();

        let function_id: i64 = conn.last_insert_rowid();

        let mut cfg = Cfg::new();

        let b0 = cfg.add_node(BasicBlock {
            id: 0,
            db_id: None,
            kind: BlockKind::Entry,
            statements: vec!["let x = 1".to_string()],
            terminator: Terminator::Goto { target: 1 },
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
            source_location: None,
        });

        let b1 = cfg.add_node(BasicBlock {
            id: 1,
            db_id: None,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Return,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
            source_location: None,
        });

        cfg.add_edge(b0, b1, EdgeType::Fallthrough);

        store_cfg(&mut conn, function_id, "test_hash_123", &cfg).unwrap();

        let block_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
                rusqlite::params![function_id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(block_count, 2, "Should have 2 blocks");

        assert!(function_exists(&conn, function_id));
        assert!(!function_exists(&conn, 9999));

        let loaded_cfg =
            super::super::operations::load_cfg_from_db_with_conn(&conn, function_id).unwrap();

        assert_eq!(loaded_cfg.node_count(), 2);
        assert_eq!(loaded_cfg.edge_count(), 1);
    }

    #[test]
    #[allow(deprecated)]
    fn test_store_cfg_incremental_update_clears_old_data() {
        use crate::cfg::{BasicBlock, BlockKind, Cfg, EdgeType, Terminator};

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
            rusqlite::params![super::super::REQUIRED_MAGELLAN_SCHEMA_VERSION, super::super::REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, 0],
        ).unwrap();

        super::super::schema::create_schema(&mut conn, super::super::TEST_MAGELLAN_SCHEMA_VERSION)
            .unwrap();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        )
        .unwrap();

        let function_id: i64 = conn.last_insert_rowid();

        let mut cfg1 = Cfg::new();
        let b0 = cfg1.add_node(BasicBlock {
            id: 0,
            db_id: None,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
            source_location: None,
        });
        let b1 = cfg1.add_node(BasicBlock {
            id: 1,
            db_id: None,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
            source_location: None,
        });
        cfg1.add_edge(b0, b1, EdgeType::Fallthrough);

        store_cfg(&mut conn, function_id, "hash_v1", &cfg1).unwrap();

        let block_count_v1: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
                rusqlite::params![function_id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(block_count_v1, 2);

        let mut cfg2 = Cfg::new();
        let b0 = cfg2.add_node(BasicBlock {
            id: 0,
            db_id: None,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
            source_location: None,
        });
        let b1 = cfg2.add_node(BasicBlock {
            id: 1,
            db_id: None,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 2 },
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
            source_location: None,
        });
        let b2 = cfg2.add_node(BasicBlock {
            id: 2,
            db_id: None,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
            source_location: None,
        });
        cfg2.add_edge(b0, b1, EdgeType::Fallthrough);
        cfg2.add_edge(b1, b2, EdgeType::Fallthrough);

        store_cfg(&mut conn, function_id, "hash_v3", &cfg2).unwrap();

        let block_count_v3: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
                rusqlite::params![function_id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(block_count_v3, 3);
    }

    #[test]
    fn test_get_function_name() {
        let conn = create_test_db_with_schema();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "my_test_func", "test.rs", "{}"),
        )
        .unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        let name = get_function_name(&conn, function_id);
        assert_eq!(name, Some("my_test_func".to_string()));

        let name = get_function_name(&conn, 9999);
        assert_eq!(name, None);
    }

    #[test]
    fn test_get_path_elements() {
        let conn = create_test_db_with_schema();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "path_test_func", "test.rs", "{}"),
        )
        .unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO cfgPaths (path_id, function_id, path_kind, entry_block, exit_block, length, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params!("test_path_abc123", function_id, "normal", 0, 2, 3, 1000),
        ).unwrap_or_else(|_e| {
            conn.execute(
                "INSERT INTO cfg_paths (path_id, function_id, path_kind, entry_block, exit_block, length, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params!("test_path_abc123", function_id, "normal", 0, 2, 3, 1000),
            ).unwrap()
        });

        conn.execute(
            "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id) VALUES (?, ?, ?)",
            rusqlite::params!("test_path_abc123", 0, 0),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id) VALUES (?, ?, ?)",
            rusqlite::params!("test_path_abc123", 1, 1),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id) VALUES (?, ?, ?)",
            rusqlite::params!("test_path_abc123", 2, 2),
        )
        .unwrap();

        let blocks = get_path_elements(&conn, "test_path_abc123").unwrap();
        assert_eq!(blocks, vec![0, 1, 2]);

        let result = get_path_elements(&conn, "nonexistent_path");
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_path_impact_from_db() {
        use crate::cfg::{BasicBlock, BlockKind, Terminator};

        let conn = create_test_db_with_schema();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "impact_test_func", "test.rs", "{}"),
        )
        .unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        let mut cfg = crate::cfg::Cfg::new();
        let b0 = cfg.add_node(BasicBlock {
            id: 0,
            db_id: None,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
            source_location: None,
        });
        let b1 = cfg.add_node(BasicBlock {
            id: 1,
            db_id: None,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 2 },
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
            source_location: None,
        });
        let b2 = cfg.add_node(BasicBlock {
            id: 2,
            db_id: None,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 3 },
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
            source_location: None,
        });
        let b3 = cfg.add_node(BasicBlock {
            id: 3,
            db_id: None,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
            source_location: None,
        });
        cfg.add_edge(b0, b1, crate::cfg::EdgeType::Fallthrough);
        cfg.add_edge(b1, b2, crate::cfg::EdgeType::Fallthrough);
        cfg.add_edge(b2, b3, crate::cfg::EdgeType::Fallthrough);

        conn.execute(
            "INSERT INTO cfg_paths (path_id, function_id, path_kind, entry_block, exit_block, length, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params!("impact_test_path", function_id, "normal", 0, 3, 3, 1000),
        ).unwrap();

        conn.execute(
            "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id) VALUES (?, ?, ?)",
            rusqlite::params!("impact_test_path", 0, 0),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id) VALUES (?, ?, ?)",
            rusqlite::params!("impact_test_path", 1, 1),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id) VALUES (?, ?, ?)",
            rusqlite::params!("impact_test_path", 2, 3),
        )
        .unwrap();

        let impact = compute_path_impact_from_db(&conn, "impact_test_path", &cfg, None).unwrap();

        assert_eq!(impact.path_id, "impact_test_path");
        assert_eq!(impact.path_length, 3);
        assert!(impact.unique_blocks_affected.contains(&2));
    }
}
