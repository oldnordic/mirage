use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

use super::CfgBlockRow;

#[cfg(feature = "backend-sqlite")]
pub(super) fn resolve_function_name_sqlite(
    conn: &Connection,
    name_or_id: &str,
    file_filter: Option<&str>,
) -> Result<i64> {
    let function_id_by_symbol: Option<i64> = conn
        .query_row(
            "SELECT id FROM graph_entities
             WHERE kind = 'Symbol'
             AND json_extract(data, '$.kind') = 'Function'
             AND json_extract(data, '$.symbol_id') = ?
             LIMIT 1",
            params![name_or_id],
            |row| row.get(0),
        )
        .optional()
        .context(format!(
            "Failed to query function with symbol_id '{}'",
            name_or_id
        ))?;

    if let Some(id) = function_id_by_symbol {
        return Ok(id);
    }

    let function_id: Option<i64> = if let Some(file_path) = file_filter {
        let pattern = format!("%{}%", file_path);
        conn.query_row(
            "SELECT id FROM graph_entities
             WHERE kind = 'Symbol'
             AND json_extract(data, '$.kind') = 'Function'
             AND name = ?
             AND file_path LIKE ?
             LIMIT 1",
            params![name_or_id, pattern],
            |row| row.get(0),
        )
        .optional()
        .context(format!(
            "Failed to query function with name '{}' in file '{}'",
            name_or_id, file_path
        ))?
    } else {
        conn.query_row(
            "SELECT id FROM graph_entities
             WHERE kind = 'Symbol'
             AND json_extract(data, '$.kind') = 'Function'
             AND name = ?
             LIMIT 1",
            params![name_or_id],
            |row| row.get(0),
        )
        .optional()
        .context(format!(
            "Failed to query function with name '{}'",
            name_or_id
        ))?
    };

    function_id.context(format!(
        "Function '{}' not found in database. Run 'magellan watch' to index functions.",
        name_or_id
    ))
}

/// Return the set of active Cargo feature names from `magellan_meta.project_metadata`.
///
/// Returns an empty set when the column is absent, NULL, or unparseable — which
/// keeps the filter conservative (unknown condition → live).
#[cfg(feature = "backend-sqlite")]
fn get_active_features_from_conn(conn: &Connection) -> std::collections::HashSet<String> {
    let meta_json: Option<String> = conn
        .query_row(
            "SELECT project_metadata FROM magellan_meta WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .unwrap_or(None)
        .flatten();

    let mut features = std::collections::HashSet::new();
    if let Some(json) = meta_json {
        #[derive(serde::Deserialize)]
        struct Meta {
            features: Option<std::collections::HashMap<String, serde_json::Value>>,
        }
        if let Ok(meta) = serde_json::from_str::<Meta>(&json) {
            if let Some(map) = meta.features {
                for key in map.into_keys() {
                    features.insert(key);
                }
            }
        }
    }
    features
}

/// Evaluate a simple `#[cfg(...)]` condition against a set of active features.
///
/// Mirrors `magellan::evaluate_cfg_condition`. Supports `feature = "name"`,
/// `all(...)`, `any(...)`, and `not(...)`.
/// Unknown conditions conservatively return `true` (block is live).
fn evaluate_cfg_condition(
    condition: &str,
    active_features: &std::collections::HashSet<String>,
) -> bool {
    let condition = condition.trim();

    if let Some(name) = condition
        .strip_prefix("feature = \"")
        .and_then(|s| s.strip_suffix('"'))
    {
        return active_features.contains(name);
    }

    if let Some(inner) = condition
        .strip_prefix("all(")
        .and_then(|s| s.strip_suffix(')'))
    {
        return inner
            .split(',')
            .all(|c| evaluate_cfg_condition(c.trim(), active_features));
    }

    if let Some(inner) = condition
        .strip_prefix("any(")
        .and_then(|s| s.strip_suffix(')'))
    {
        return inner
            .split(',')
            .any(|c| evaluate_cfg_condition(c.trim(), active_features));
    }

    if let Some(inner) = condition
        .strip_prefix("not(")
        .and_then(|s| s.strip_suffix(')'))
    {
        return !evaluate_cfg_condition(inner.trim(), active_features);
    }

    true
}

/// Load CFG blocks from SQLite backend
///
/// This helper function loads CFG blocks using SQL queries from the cfg_blocks table.
#[cfg(feature = "backend-sqlite")]
fn load_cfg_from_sqlite(conn: &Connection, function_id: i64) -> Result<crate::cfg::Cfg> {
    use std::path::PathBuf;

    let file_path: Option<String> = conn
        .query_row(
            "SELECT file_path FROM graph_entities WHERE id = ?",
            params![function_id],
            |row| row.get(0),
        )
        .optional()
        .context("Failed to query file_path from graph_entities")?;

    let file_path = file_path.map(PathBuf::from);

    let has_cfg_condition: bool = conn
        .query_row(
            "SELECT 1 FROM pragma_table_info('cfg_blocks') WHERE name='cfg_condition' LIMIT 1",
            [],
            |_| Ok(true),
        )
        .optional()
        .unwrap_or(None)
        .unwrap_or(false);

    let query = if has_cfg_condition {
        "SELECT id, kind, terminator, byte_start, byte_end,
             start_line, start_col, end_line, end_col,
             coord_x, coord_y, coord_z, cfg_condition
         FROM cfg_blocks WHERE function_id = ? ORDER BY id ASC"
    } else {
        "SELECT id, kind, terminator, byte_start, byte_end,
             start_line, start_col, end_line, end_col,
             coord_x, coord_y, coord_z, NULL
         FROM cfg_blocks WHERE function_id = ? ORDER BY id ASC"
    };

    let mut stmt = conn
        .prepare_cached(query)
        .context("Failed to prepare cfg_blocks query")?;

    let block_rows: Vec<CfgBlockRow> = stmt
        .query_map(params![function_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
                row.get(12)?,
            ))
        })
        .context("Failed to execute cfg_blocks query")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect cfg_blocks rows")?;

    let active_features = get_active_features_from_conn(conn);
    let block_rows: Vec<CfgBlockRow> = block_rows
        .into_iter()
        .filter(|(.., cfg_condition)| {
            cfg_condition
                .as_ref()
                .map(|c| evaluate_cfg_condition(c, &active_features))
                .unwrap_or(true)
        })
        .collect();

    if block_rows.is_empty() {
        anyhow::bail!(
            "No CFG blocks found for function_id {}. Run 'magellan watch' to build CFGs.",
            function_id
        );
    }

    let edges: Vec<(i64, i64, String)> = match conn.prepare_cached(
        "SELECT source_idx, target_idx, edge_type
             FROM cfg_edges
             WHERE function_id = ?
             ORDER BY source_idx, target_idx",
    ) {
        Ok(mut stmt) => stmt
            .query_map(params![function_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .context("Failed to query cfg_edges")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to collect cfg_edges rows")?,
        Err(_) => Vec::new(),
    };

    load_cfg_from_rows(block_rows, file_path, edges)
}

/// Common CFG loading logic used by the SQLite backend
///
/// This function takes pre-fetched block rows and builds the CFG structure.
pub(super) fn load_cfg_from_rows(
    block_rows: Vec<CfgBlockRow>,
    file_path: Option<std::path::PathBuf>,
    cfg_edges: Vec<(i64, i64, String)>,
) -> Result<crate::cfg::Cfg> {
    use crate::cfg::source::SourceLocation;
    use crate::cfg::{build_edges_from_cfg_edges, build_edges_from_terminators};
    use crate::cfg::{BasicBlock, BlockKind, Cfg, Terminator};
    use std::collections::HashMap;

    let mut db_id_to_node: HashMap<i64, usize> = HashMap::new();
    let mut graph = Cfg::new();

    for (
        node_idx,
        (
            db_id,
            kind_str,
            terminator_str,
            byte_start,
            byte_end,
            start_line,
            start_col,
            end_line,
            end_col,
            coord_x,
            coord_y,
            coord_z,
            _cfg_condition,
        ),
    ) in block_rows.iter().enumerate()
    {
        let kind = match kind_str.as_str() {
            "entry" => BlockKind::Entry,
            "return" => BlockKind::Exit,
            "if" | "else" | "loop" | "while" | "for" | "match_arm" | "block" => BlockKind::Normal,
            _ => BlockKind::Normal,
        };

        let terminator = match terminator_str.as_deref() {
            Some("fallthrough") => Terminator::Goto { target: 0 },
            Some("conditional") => Terminator::SwitchInt {
                targets: vec![],
                otherwise: 0,
            },
            Some("goto") => Terminator::Goto { target: 0 },
            Some("return") => Terminator::Return,
            Some("break") => Terminator::Abort("break".to_string()),
            Some("continue") => Terminator::Abort("continue".to_string()),
            Some("call") => Terminator::Call {
                target: None,
                unwind: None,
            },
            Some("panic") => Terminator::Abort("panic".to_string()),
            Some(_) | None => Terminator::Unreachable,
        };

        let source_location = if let Some(ref path) = file_path {
            let sl = start_line.and_then(|l| start_col.map(|c| (l as usize, c as usize)));
            let el = end_line.and_then(|l| end_col.map(|c| (l as usize, c as usize)));

            match (sl, el, byte_start, byte_end) {
                (Some((start_l, start_c)), Some((end_l, end_c)), Some(bs), Some(be)) => {
                    Some(SourceLocation {
                        file_path: path.clone(),
                        byte_start: *bs as usize,
                        byte_end: *be as usize,
                        start_line: start_l,
                        start_column: start_c,
                        end_line: end_l,
                        end_column: end_c,
                    })
                }
                _ => None,
            }
        } else {
            None
        };

        let block = BasicBlock {
            id: node_idx,
            db_id: Some(*db_id),
            kind,
            statements: vec![],
            terminator,
            source_location,
            coord_x: coord_x.unwrap_or(0),
            coord_y: coord_y.unwrap_or(0),
            coord_z: coord_z.unwrap_or(0),
        };

        graph.add_node(block);
        db_id_to_node.insert(*db_id, node_idx);
    }

    let mut index_to_node: HashMap<usize, usize> = HashMap::new();
    for (idx, (db_id, _, _, _, _, _, _, _, _, _, _, _, _)) in block_rows.iter().enumerate() {
        if let Some(&node_idx) = db_id_to_node.get(db_id) {
            index_to_node.insert(idx, node_idx);
        }
    }

    if !cfg_edges.is_empty() {
        build_edges_from_cfg_edges(&mut graph, &cfg_edges, &index_to_node)
            .context("Failed to build edges from cfg_edges")?;
    } else {
        build_edges_from_terminators(&mut graph, &block_rows, &db_id_to_node)
            .context("Failed to build edges from terminator data")?;
    }

    Ok(graph)
}

/// Resolve a function name or ID to a function_id (backend-agnostic)
///
/// This is the main entry point for resolving function names. It works with both
/// SQLite and geometric backends.
///
/// # Examples
///
/// ```no_run
/// # use mirage_analyzer::storage::{resolve_function_name, MirageDb};
/// # fn main() -> anyhow::Result<()> {
/// # let db = MirageDb::open("test.db")?;
/// let func_id = resolve_function_name(&db, "123")?;
/// let func_id = resolve_function_name(&db, "my_function")?;
/// # Ok(())
/// # }
/// ```
pub fn resolve_function_name(db: &super::MirageDb, name_or_id: &str) -> Result<i64> {
    db.resolve_function_name(name_or_id)
}

/// Resolve a function name or ID to a function_id with optional file filter
///
/// This is a helper function that delegates to `MirageDb::resolve_function_name_with_file`.
/// Use this for a backend-agnostic API.
///
/// # Examples
///
/// ```no_run
/// # use mirage_analyzer::storage::{resolve_function_name_with_file, MirageDb};
/// # fn main() -> anyhow::Result<()> {
/// # let db = MirageDb::open("test.db")?;
/// let func_id = resolve_function_name_with_file(&db, "process", Some("src/lib.rs"))?;
/// # Ok(())
/// # }
/// ```
pub fn resolve_function_name_with_file(
    db: &super::MirageDb,
    name_or_id: &str,
    file_filter: Option<&str>,
) -> Result<i64> {
    db.resolve_function_name_with_file(name_or_id, file_filter)
}

/// Resolve a function name or ID to a function_id (SQLite backend, legacy)
///
/// This is the legacy function that takes a direct Connection reference.
/// For new code supporting both backends, use `resolve_function_name` which takes `&MirageDb`.
#[cfg(feature = "backend-sqlite")]
pub fn resolve_function_name_with_conn(conn: &Connection, name_or_id: &str) -> Result<i64> {
    if let Ok(id) = name_or_id.parse::<i64>() {
        return Ok(id);
    }

    let function_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM graph_entities
             WHERE kind = 'Symbol'
             AND json_extract(data, '$.kind') = 'Function'
             AND name = ?
             LIMIT 1",
            params![name_or_id],
            |row| row.get(0),
        )
        .optional()
        .context(format!(
            "Failed to query function with name '{}'",
            name_or_id
        ))?;

    function_id.context(format!(
        "Function '{}' not found in database. Run 'magellan watch' to index functions.",
        name_or_id
    ))
}

/// Load a CFG from the database for a given function_id (backend-agnostic)
///
/// # Examples
///
/// ```no_run
/// # use mirage_analyzer::storage::{load_cfg_from_db, MirageDb};
/// # fn main() -> anyhow::Result<()> {
/// # let db = MirageDb::open("test.db")?;
/// let cfg = load_cfg_from_db(&db, 123)?;
/// # Ok(())
/// # }
/// ```
pub fn load_cfg_from_db(db: &super::MirageDb, function_id: i64) -> Result<crate::cfg::Cfg> {
    db.load_cfg(function_id)
}

/// Load a CFG from the database for a given function_id (SQLite backend)
///
/// This is the legacy function that takes a direct Connection reference.
/// For new code supporting both backends, use `load_cfg_from_db` which takes `&MirageDb`.
#[cfg(feature = "backend-sqlite")]
pub fn load_cfg_from_db_with_conn(conn: &Connection, function_id: i64) -> Result<crate::cfg::Cfg> {
    load_cfg_from_sqlite(conn, function_id)
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn create_test_db_with_schema() -> Connection {
        crate::storage::schema::create_test_db_with_schema()
    }

    #[test]
    fn test_resolve_function_by_id() {
        let conn = create_test_db_with_schema();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "my_func", "test.rs", "{}"),
        )
        .unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        let result = resolve_function_name_with_conn(&conn, &function_id.to_string()).unwrap();
        assert_eq!(result, function_id);
    }

    #[test]
    fn test_resolve_function_by_name() {
        let conn = create_test_db_with_schema();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!(
                "Symbol",
                "test_function",
                "test.rs",
                r#"{"kind":"Function"}"#
            ),
        )
        .unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        let result = resolve_function_name_with_conn(&conn, "test_function").unwrap();
        assert_eq!(result, function_id);
    }

    #[test]
    fn test_resolve_function_not_found() {
        let conn = create_test_db_with_schema();

        let result = resolve_function_name_with_conn(&conn, "nonexistent_func");

        assert!(
            result.is_err(),
            "Should return error for non-existent function"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not found") || err_msg.contains("not found in database"));
    }

    #[test]
    fn test_resolve_function_numeric_string() {
        let conn = create_test_db_with_schema();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "func123", "test.rs", "{}"),
        )
        .unwrap();

        let result = resolve_function_name_with_conn(&conn, "123").unwrap();
        assert_eq!(result, 123);

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "another_func", "test.rs", "{}"),
        )
        .unwrap();
        let _id_456 = conn.last_insert_rowid();

        let result = resolve_function_name_with_conn(&conn, "999").unwrap();
        assert_eq!(result, 999, "Should return numeric ID directly");
    }

    #[test]
    fn test_load_cfg_not_found() {
        let conn = create_test_db_with_schema();

        let result = load_cfg_from_db_with_conn(&conn, 99999);

        assert!(
            result.is_err(),
            "Should return error for function with no CFG"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("No CFG blocks found") || err_msg.contains("not found"));
    }

    #[test]
    fn test_load_cfg_empty_terminator() {
        use crate::cfg::Terminator;

        let conn = create_test_db_with_schema();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "empty_term_func", "test.rs", "{}"),
        )
        .unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params!(function_id, "return", "return", 0, 10, 1, 0, 1, 10),
        )
        .unwrap();

        let cfg = load_cfg_from_db_with_conn(&conn, function_id).unwrap();

        assert_eq!(cfg.node_count(), 1);
        let block = &cfg[petgraph::graph::NodeIndex::new(0)];
        assert!(matches!(block.terminator, Terminator::Return));
    }

    #[test]
    fn test_load_cfg_with_multiple_edge_types() {
        use crate::cfg::EdgeType;

        let conn = create_test_db_with_schema();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "edge_types_func", "test.rs", "{}"),
        )
        .unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params!(function_id, "entry", "conditional", 0, 10, 1, 0, 1, 10),
        )
        .unwrap();
        let _block_0_id: i64 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params!(function_id, "block", "fallthrough", 10, 20, 2, 0, 2, 10),
        )
        .unwrap();
        let _block_1_id: i64 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params!(function_id, "block", "call", 20, 30, 3, 0, 3, 10),
        )
        .unwrap();
        let _block_2_id: i64 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params!(function_id, "return", "return", 30, 40, 4, 0, 4, 10),
        )
        .unwrap();
        let _block_3_id: i64 = conn.last_insert_rowid();

        let cfg = load_cfg_from_db_with_conn(&conn, function_id).unwrap();

        assert_eq!(cfg.node_count(), 4);
        assert_eq!(cfg.edge_count(), 4);

        use petgraph::visit::EdgeRef;
        let edges: Vec<_> = cfg
            .edge_references()
            .map(|e| (e.source().index(), e.target().index(), *e.weight()))
            .collect();

        assert!(edges.contains(&(0, 1, EdgeType::TrueBranch)));
        assert!(edges.contains(&(0, 2, EdgeType::FalseBranch)));
        assert!(edges.contains(&(1, 2, EdgeType::Fallthrough)));
        assert!(edges.contains(&(2, 3, EdgeType::Call)));
    }

    #[test]
    fn test_load_cfg_missing_cfg_blocks_table() {
        let conn = Connection::open_in_memory().unwrap();

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
            rusqlite::params![6, 3, 0],
        ).unwrap();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "test_func", "test.rs", "{}"),
        )
        .unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        let result = load_cfg_from_db_with_conn(&conn, function_id);
        assert!(result.is_err(), "Should fail when cfg_blocks table missing");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("cfg_blocks") || err_msg.contains("prepare"),
            "Error should mention cfg_blocks or prepare: {}",
            err_msg
        );
    }

    #[test]
    fn test_load_cfg_function_not_found() {
        let conn = create_test_db_with_schema();

        let result = load_cfg_from_db_with_conn(&conn, 99999);
        assert!(result.is_err(), "Should fail for non-existent function");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("No CFG blocks found") || err_msg.contains("not found"),
            "Error should mention missing CFG: {}",
            err_msg
        );
        assert!(
            err_msg.contains("magellan watch"),
            "Error should suggest running magellan watch: {}",
            err_msg
        );
    }

    #[test]
    fn test_load_cfg_empty_blocks() {
        let conn = create_test_db_with_schema();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "func_without_cfg", "test.rs", "{}"),
        )
        .unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        let result = load_cfg_from_db_with_conn(&conn, function_id);
        assert!(result.is_err(), "Should fail when no CFG blocks exist");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("No CFG blocks found"),
            "Error should mention no CFG blocks: {}",
            err_msg
        );
        assert!(
            err_msg.contains("magellan watch"),
            "Error should suggest running magellan watch: {}",
            err_msg
        );
    }

    #[test]
    fn test_resolve_function_missing_with_helpful_message() {
        let conn = create_test_db_with_schema();

        let result = resolve_function_name_with_conn(&conn, "nonexistent_function");
        assert!(result.is_err(), "Should fail for non-existent function");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not found") || err_msg.contains("not found in database"),
            "Error should mention function not found: {}",
            err_msg
        );
    }

    #[test]
    fn test_load_cfg_drops_dead_cfg_condition_blocks() {
        let conn = create_test_db_with_schema();

        conn.execute("ALTER TABLE cfg_blocks ADD COLUMN cfg_condition TEXT", [])
            .unwrap();

        conn.execute(
            "ALTER TABLE magellan_meta ADD COLUMN project_metadata TEXT",
            [],
        )
        .unwrap();

        let metadata_json =
            r#"{"name":"test","version":"0.1.0","features":{"enabled_feat":[]},"dependencies":[]}"#;
        conn.execute(
            "UPDATE magellan_meta SET project_metadata = ? WHERE id = 1",
            rusqlite::params![metadata_json],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            rusqlite::params!("function", "cfg_branch_func", "test.rs", "{}"),
        )
        .unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col, cfg_condition)
             VALUES (?, 'entry', 'fallthrough', 0, 10, 1, 0, 1, 10, NULL)",
            rusqlite::params![function_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col, cfg_condition)
             VALUES (?, 'block', 'return', 11, 20, 2, 0, 2, 10, 'feature = \"enabled_feat\"')",
            rusqlite::params![function_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col, cfg_condition)
             VALUES (?, 'block', 'return', 21, 30, 3, 0, 3, 10, 'feature = \"disabled_feat\"')",
            rusqlite::params![function_id],
        )
        .unwrap();

        let cfg = load_cfg_from_db_with_conn(&conn, function_id).unwrap();

        assert_eq!(
            cfg.node_count(),
            2,
            "dead cfg_condition block should be filtered out; expected 2 live blocks, got {}",
            cfg.node_count()
        );
    }
}
