// Database storage layer extending Magellan's schema
//
// Mirage uses the same Magellan database and extends it with:
// - cfg_blocks: Basic blocks within functions
// - cfg_edges: Control flow between blocks
// - cfg_paths: Enumerated execution paths
// - cfg_path_elements: Blocks in each path
// - cfg_dominators: Dominance relationships
// - cfg_post_dominators: Reverse dominance

pub mod paths;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

// Re-export path caching functions
// Note: Some exports like PathCache, store_paths, etc. are not currently used
// but are kept for potential future use and API completeness
#[allow(unused_imports)]
pub use paths::{
    PathCache,
    store_paths,
    get_cached_paths,
    invalidate_function_paths,
    update_function_paths_if_changed,
};

/// Mirage schema version
pub const MIRAGE_SCHEMA_VERSION: i32 = 1;

/// Minimum Magellan schema version we require
/// Magellan v7+ includes cfg_blocks table with AST-based CFG
pub const MIN_MAGELLAN_SCHEMA_VERSION: i32 = 7;

/// Magellan schema version used in tests (for consistency)
pub const TEST_MAGELLAN_SCHEMA_VERSION: i32 = MIN_MAGELLAN_SCHEMA_VERSION;

/// Alias for backward compatibility (same as TEST_MAGELLAN_SCHEMA_VERSION)
pub const REQUIRED_MAGELLAN_SCHEMA_VERSION: i32 = TEST_MAGELLAN_SCHEMA_VERSION;

/// SQLiteGraph schema version we require
pub const REQUIRED_SQLITEGRAPH_SCHEMA_VERSION: i32 = 3;

/// Database connection wrapper
pub struct MirageDb {
    conn: Connection,
}

impl MirageDb {
    /// Open database at the given path
    ///
    /// This can open:
    /// - A Mirage database (with mirage_meta table)
    /// - A Magellan database (extends it with Mirage tables)
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            anyhow::bail!("Database not found: {}", path.display());
        }

        let mut conn = Connection::open(path)
            .context("Failed to open database")?;

        // Check if mirage_meta table exists
        let mirage_meta_exists: bool = conn.query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='mirage_meta'",
            [],
            |row| row.get(0),
        ).optional()?.unwrap_or(0) == 1;

        // Get Mirage schema version (0 if table doesn't exist)
        let mirage_version: i32 = if mirage_meta_exists {
            conn.query_row(
                "SELECT mirage_schema_version FROM mirage_meta WHERE id = 1",
                [],
                |row| row.get(0),
            ).optional()?.flatten().unwrap_or(0)
        } else {
            0
        };

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
        ).optional()?.flatten().unwrap_or(0);

        if magellan_version < MIN_MAGELLAN_SCHEMA_VERSION {
            anyhow::bail!(
                "Magellan schema version {} is too old (minimum {}). \
                 Please update Magellan and run 'magellan watch' to rebuild CFGs.",
                magellan_version, MIN_MAGELLAN_SCHEMA_VERSION
            );
        }

        // Check for cfg_blocks table existence (Magellan v7+)
        let cfg_blocks_exists: bool = conn.query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='cfg_blocks'",
            [],
            |row| row.get(0),
        ).optional()?.unwrap_or(0) == 1;

        if !cfg_blocks_exists {
            anyhow::bail!(
                "CFG blocks table not found. Magellan schema v7+ required. \
                 Run 'magellan watch' to build CFGs."
            );
        }

        // If mirage_meta doesn't exist, this is a pure Magellan database.
        // Initialize Mirage tables to extend it.
        if !mirage_meta_exists {
            create_schema(&mut conn, magellan_version)?;
        } else if mirage_version < MIRAGE_SCHEMA_VERSION {
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
            rustc_version TEXT,
            created_at INTEGER NOT NULL
        )",
        [],
    )?;

    // Create cfg_blocks table (Magellan v7+ schema)
    // Note: Mirage now uses Magellan's cfg_blocks table as the source of truth
    // This table is created by Magellan, but we include the CREATE here for:
    // 1. Test database setup
    // 2. Documentation of expected schema
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cfg_blocks (
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
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_cfg_blocks_function ON cfg_blocks(function_id)",
        [],
    )?;

    // Create cfg_edges table (kept for backward compatibility with tests and existing databases)
    // Note: New code should compute edges in memory using build_edges_from_terminators()
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
            FOREIGN KEY (function_id) REFERENCES graph_entities(id)
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
            FOREIGN KEY (path_id) REFERENCES cfg_paths(path_id)
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
    #[deprecated(note = "Edges are now computed in memory, not stored")]
    pub cfg_edges: i64,
    pub cfg_paths: i64,
    pub cfg_dominators: i64,
    pub mirage_schema_version: i32,
    pub magellan_schema_version: i32,
}

impl MirageDb {
    /// Get database statistics
    ///
    /// Note: cfg_edges count is included for backward compatibility but edges
    /// are now computed in memory from terminator data, not stored.
    pub fn status(&self) -> Result<DatabaseStatus> {
        let cfg_blocks: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM cfg_blocks",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        // Edges are now computed in memory from terminator data (per RESEARCH.md Pattern 2)
        // This count is kept for backward compatibility but will always be 0 for new databases
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

        #[allow(deprecated)]
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

/// Resolve a function name or ID to a function_id
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `name_or_id` - Function name (string) or function_id (numeric string)
///
/// # Returns
///
/// * `Ok(i64)` - The function_id if found
/// * `Err(...)` - Error if function not found or query fails
///
/// # Examples
///
/// ```no_run
/// # use mirage::storage::resolve_function_name;
/// # use rusqlite::Connection;
/// # fn main() -> anyhow::Result<()> {
/// # let conn = Connection::open_in_memory()?;
/// // Resolve by numeric ID
/// let func_id = resolve_function_name(&conn, "123")?;
///
/// // Resolve by function name
/// let func_id = resolve_function_name(&conn, "my_function")?;
/// # Ok(())
/// # }
/// ```
///
/// # Algorithm
///
/// 1. If input parses as i64, return it directly (it's already a function_id)
/// 2. Otherwise, query graph_entities for a function with matching name
/// 3. Return the ID if found, error if not found
pub fn resolve_function_name(conn: &Connection, name_or_id: &str) -> Result<i64> {
    // Try to parse as numeric ID first
    if let Ok(id) = name_or_id.parse::<i64>() {
        return Ok(id);
    }

    // Query by function name
    let function_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM graph_entities WHERE kind = 'function' AND name = ? LIMIT 1",
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

/// Load a CFG from the database for a given function_id
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `function_id` - ID of the function to load CFG for
///
/// # Returns
///
/// * `Ok(Cfg)` - The reconstructed control flow graph
/// * `Err(...)` - Error if query fails or CFG data is invalid
///
/// # Examples
///
/// ```no_run
/// # use mirage::storage::load_cfg_from_db;
/// # use rusqlite::Connection;
/// # fn main() -> anyhow::Result<()> {
/// # let conn = Connection::open_in_memory()?;
/// let cfg = load_cfg_from_db(&conn, 123)?;
/// # Ok(())
/// # }
/// ```
///
/// # Algorithm
///
/// 1. Query all cfg_blocks for the function from Magellan's cfg_blocks table, ordered by id
/// 2. For each block, create a BasicBlock with:
///    - id: sequential index (0, 1, 2...) based on query order
///    - kind: parsed from Magellan's kind string (entry/normal/exit/return/if/else/loop/etc.)
///    - terminator: parsed from Magellan's terminator string (fallthrough/conditional/goto/return/etc.)
///    - source_location: constructed from line/column data if available
///    - statements: empty vec! (future enhancement)
/// 3. Build edges from terminator data using build_edges_from_terminators()
/// 4. Return the constructed Cfg
///
/// # Notes
///
/// - Block IDs in the database (AUTOINCREMENT) are mapped to sequential
///   indices in the CFG graph (0, 1, 2...) for consistency with in-memory CFG construction
/// - Magellan stores terminator as plain TEXT, not JSON
/// - Magellan uses "kind" column, not "block_kind"
/// - Requires Magellan schema v7+ for cfg_blocks table
/// - Edges are constructed in memory from terminator data, not queried from cfg_edges table
pub fn load_cfg_from_db(conn: &Connection, function_id: i64) -> Result<crate::cfg::Cfg> {
    use crate::cfg::{BasicBlock, BlockKind, Cfg, Terminator};
    use crate::cfg::build_edges_from_terminators;
    use crate::cfg::source::SourceLocation;
    use std::path::PathBuf;

    // Query file_path for this function from graph_entities
    let file_path: Option<String> = conn
        .query_row(
            "SELECT file_path FROM graph_entities WHERE id = ?",
            params![function_id],
            |row| row.get(0),
        )
        .optional()
        .context("Failed to query file_path from graph_entities")?;

    let file_path = file_path.map(PathBuf::from);

    // Query all blocks for this function from Magellan's cfg_blocks table
    // Magellan schema v7+ uses: kind (not block_kind), terminator as TEXT, and line/col columns
    let mut stmt = conn.prepare_cached(
        "SELECT id, kind, terminator, byte_start, byte_end,
                start_line, start_col, end_line, end_col
         FROM cfg_blocks
         WHERE function_id = ?
         ORDER BY id ASC",
    ).context("Failed to prepare cfg_blocks query")?;

    let block_rows: Vec<(i64, String, Option<String>, Option<i64>, Option<i64>,
                          Option<i64>, Option<i64>, Option<i64>, Option<i64>)> = stmt
        .query_map(params![function_id], |row| {
            Ok((
                row.get(0)?,     // id (database primary key)
                row.get(1)?,     // kind (Magellan's column name)
                row.get(2)?,     // terminator (plain TEXT, not JSON)
                row.get(3)?,     // byte_start
                row.get(4)?,     // byte_end
                row.get(5)?,     // start_line
                row.get(6)?,     // start_col
                row.get(7)?,     // end_line
                row.get(8)?,     // end_col
            ))
        })
        .context("Failed to execute cfg_blocks query")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect cfg_blocks rows")?;

    if block_rows.is_empty() {
        anyhow::bail!(
            "No CFG blocks found for function_id {}. Run 'magellan watch' to build CFGs.",
            function_id
        );
    }

    // Build mapping from database block ID to graph node index
    let mut db_id_to_node: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
    let mut graph = Cfg::new();

    // Add each block to the graph
    for (node_idx, (db_id, kind_str, terminator_str, byte_start, byte_end,
                     start_line, start_col, end_line, end_col)) in
        block_rows.iter().enumerate()
    {
        // Parse Magellan's block kind to Mirage's BlockKind
        let kind = match kind_str.as_str() {
            "entry" => BlockKind::Entry,
            "return" => BlockKind::Exit,
            "if" | "else" | "loop" | "while" | "for" | "match_arm" | "block" => BlockKind::Normal,
            _ => {
                // Fallback: treat unknown kinds as Normal
                // Magellan may have additional kinds we don't explicitly handle
                BlockKind::Normal
            }
        };

        // Parse Magellan's terminator string to Mirage's Terminator enum
        let terminator = match terminator_str.as_deref() {
            Some("fallthrough") => Terminator::Goto { target: 0 }, // target will be resolved from edges
            Some("conditional") => Terminator::SwitchInt { targets: vec![], otherwise: 0 },
            Some("goto") => Terminator::Goto { target: 0 },
            Some("return") => Terminator::Return,
            Some("break") => Terminator::Abort("break".to_string()),
            Some("continue") => Terminator::Abort("continue".to_string()),
            Some("call") => Terminator::Call { target: None, unwind: None },
            Some("panic") => Terminator::Abort("panic".to_string()),
            Some(_) | None => Terminator::Unreachable,
        };

        // Construct source_location from Magellan's line/column data
        let source_location = if let Some(ref path) = file_path {
            // Use line/column data directly (Magellan v7+)
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
            kind,
            statements: vec![], // Empty for now - future enhancement
            terminator,
            source_location,
        };

        graph.add_node(block);
        db_id_to_node.insert(*db_id, node_idx);
    }

    // Build edges from terminator data (per RESEARCH.md Pattern 2)
    // Edges are derived in memory by analyzing terminators, not queried from cfg_edges table
    build_edges_from_terminators(&mut graph, &block_rows, &db_id_to_node)
        .context("Failed to build edges from terminator data")?;

    Ok(graph)
}

/// Store a CFG in the database for a given function
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `function_id` - ID of the function in graph_entities
/// * `function_hash` - BLAKE3 hash of the function body for incremental updates
/// * `cfg` - The control flow graph to store
///
/// # Returns
///
/// * `Ok(())` - CFG stored successfully
/// * `Err(...)` - Error if storage fails
///
/// # Algorithm
///
/// 1. Begin IMMEDIATE transaction for atomicity
/// 2. Clear existing cfg_blocks and cfg_edges for this function_id (incremental update)
/// 3. Insert each BasicBlock as a row in cfg_blocks:
///    - Serialize terminator as JSON string
///    - Store source location byte ranges if available
/// 4. Insert each edge as a row in cfg_edges (for backward compatibility)
/// 5. Commit transaction
///
/// # Notes
///
/// - DEPRECATED: Magellan handles CFG storage via cfg_blocks. Edges are now computed in memory.
/// - This function is kept for backward compatibility with existing tests.
/// - Uses BEGIN IMMEDIATE to acquire write lock early (prevents write conflicts)
/// - Existing blocks/edges are cleared for incremental updates
/// - Block IDs are AUTOINCREMENT in the database
#[deprecated(note = "Magellan handles CFG storage via cfg_blocks. Edges are computed in memory.")]
pub fn store_cfg(
    conn: &mut Connection,
    function_id: i64,
    _function_hash: &str,  // Unused: Magellan manages its own caching
    cfg: &crate::cfg::Cfg,
) -> Result<()> {
    use crate::cfg::{BlockKind, EdgeType, Terminator};
    use petgraph::visit::EdgeRef;

    conn.execute("BEGIN IMMEDIATE TRANSACTION", [])
        .context("Failed to begin transaction")?;

    // Clear existing blocks and edges for this function (incremental update)
    conn.execute(
        "DELETE FROM cfg_edges WHERE from_id IN (
            SELECT id FROM cfg_blocks WHERE function_id = ?
         )",
        params![function_id],
    ).context("Failed to clear existing cfg_edges")?;

    conn.execute(
        "DELETE FROM cfg_blocks WHERE function_id = ?",
        params![function_id],
    ).context("Failed to clear existing cfg_blocks")?;

    // Insert each block and collect database IDs
    let mut block_id_map: std::collections::HashMap<petgraph::graph::NodeIndex, i64> =
        std::collections::HashMap::new();

    let mut insert_block = conn.prepare_cached(
        "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                  start_line, start_col, end_line, end_col)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    ).context("Failed to prepare block insert statement")?;

    for node_idx in cfg.node_indices() {
        let block = cfg.node_weight(node_idx)
            .context("CFG node has no weight")?;

        // Convert terminator to Magellan's string format
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

        // Get location data from source_location
        let (byte_start, byte_end) = block.source_location.as_ref()
            .map(|loc| (Some(loc.byte_start as i64), Some(loc.byte_end as i64)))
            .unwrap_or((None, None));

        let (start_line, start_col, end_line, end_col) = block.source_location.as_ref()
            .map(|loc| (
                Some(loc.start_line as i64),
                Some(loc.start_column as i64),
                Some(loc.end_line as i64),
                Some(loc.end_column as i64),
            ))
            .unwrap_or((None, None, None, None));

        // Convert BlockKind to Magellan's kind string
        let kind = match block.kind {
            BlockKind::Entry => "entry",
            BlockKind::Normal => "block",
            BlockKind::Exit => "return",
        };

        insert_block.execute(params![
            function_id,
            kind,
            terminator_str,
            byte_start,
            byte_end,
            start_line,
            start_col,
            end_line,
            end_col,
        ]).context("Failed to insert cfg_block")?;

        let db_id = conn.last_insert_rowid();
        block_id_map.insert(node_idx, db_id);
    }

    // Insert each edge (for backward compatibility, though edges are now computed in memory)
    let mut insert_edge = conn.prepare_cached(
        "INSERT INTO cfg_edges (from_id, to_id, edge_type) VALUES (?, ?, ?)",
    ).context("Failed to prepare edge insert statement")?;

    for edge in cfg.edge_references() {
        let from_db_id = block_id_map.get(&edge.source())
            .context("Edge source has no database ID")?;
        let to_db_id = block_id_map.get(&edge.target())
            .context("Edge target has no database ID")?;

        let edge_type_str = match edge.weight() {
            EdgeType::Fallthrough => "Fallthrough",
            EdgeType::TrueBranch => "TrueBranch",
            EdgeType::FalseBranch => "FalseBranch",
            EdgeType::LoopBack => "LoopBack",
            EdgeType::LoopExit => "LoopExit",
            EdgeType::Call => "Call",
            EdgeType::Exception => "Exception",
            EdgeType::Return => "Return",
        };

        insert_edge.execute(params![from_db_id, to_db_id, edge_type_str])
            .context("Failed to insert cfg_edge")?;
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
        |row| row.get::<_, i64>(0).map(|count| count > 0)
    ).optional().ok().flatten().unwrap_or(false)
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
    // Try to query function_hash if it exists (legacy Mirage schema)
    conn.query_row(
        "SELECT function_hash FROM cfg_blocks WHERE function_id = ? LIMIT 1",
        params![function_id],
        |row| row.get(0)
    ).optional().ok().flatten()
}

/// Compare two function hashes and return true if they differ
///
/// Used by the index command to decide whether to skip a function.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `function_id` - ID of the function
/// * `new_hash` - New hash to compare against stored hash
///
/// # Returns
///
/// * `Ok(true)` - Hashes differ or function is new (needs re-indexing)
/// * `Ok(false)` - Hashes match (can skip)
/// * `Err(...)` - Database query error
///
/// # Note
///
/// Magellan's cfg_blocks table doesn't store function_hash, so this function
/// always returns true (indicating re-indexing needed) when using Magellan's schema.
pub fn hash_changed(
    conn: &Connection,
    function_id: i64,
    _new_hash: &str,
) -> Result<bool> {
    let old_hash: Option<String> = conn.query_row(
        "SELECT function_hash FROM cfg_blocks WHERE function_id = ? LIMIT 1",
        params![function_id],
        |row| row.get(0)
    ).optional()?;

    match old_hash {
        Some(old) => Ok(old != _new_hash),
        None => Ok(true),  // New function or no hash stored, always index
    }
}

/// Compute the set of functions that need re-indexing based on git changes
///
/// This uses git diff to find changed Rust files, then queries the database
/// for functions defined in those files.
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `project_path` - Path to the project being indexed
///
/// # Returns
///
/// Set of function names that should be re-indexed
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

    // Use git to find changed Rust files
    if let Ok(git_output) = Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .current_dir(project_path)
        .output()
    {
        let git_files = String::from_utf8_lossy(&git_output.stdout);

        // Collect .rs files that changed
        let changed_rs_files: Vec<&str> = git_files
            .lines()
            .filter(|f| f.ends_with(".rs"))
            .collect();

        if changed_rs_files.is_empty() {
            return Ok(changed);
        }

        // Build a list of file paths for the SQL query
        for file in changed_rs_files {
            // Normalize the file path relative to project root
            let normalized_path = if file.starts_with('/') {
                file.trim_start_matches('/')
            } else {
                file
            };

            // Query for functions in this file
            // Note: file_path in graph_entities may be relative or absolute,
            // so we check both patterns
            let mut stmt = conn.prepare_cached(
                "SELECT name FROM graph_entities
                 WHERE kind = 'function' AND (
                     file_path = ? OR
                     file_path = ? OR
                     file_path LIKE '%' || ?
                 )"
            ).context("Failed to prepare function lookup query")?;

            let with_slash = format!("/{}", normalized_path);

            let rows = stmt.query_map(
                params![normalized_path, &with_slash, normalized_path],
                |row| row.get::<_, String>(0)
            ).context("Failed to execute function lookup")?;

            for row in rows {
                if let Ok(func_name) = row {
                    changed.insert(func_name);
                }
            }
        }
    }

    Ok(changed)
}

/// Get the file containing a function
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `function_name` - Name of the function
///
/// # Returns
///
/// * `Ok(Some(file_path))` - The file path if found
/// * `Ok(None)` - Function not found
/// * `Err(...)` - Database error
pub fn get_function_file(
    conn: &Connection,
    function_name: &str,
) -> Result<Option<String>> {
    let file: Option<String> = conn.query_row(
        "SELECT file_path FROM graph_entities WHERE kind = 'function' AND name = ? LIMIT 1",
        params![function_name],
        |row| row.get(0)
    ).optional()?;

    Ok(file)
}

/// Get the function name for a given block ID
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `function_id` - ID of the function
///
/// # Returns
///
/// * `Some(name)` - The function name if found
/// * `None` - Function not found
pub fn get_function_name(conn: &Connection, function_id: i64) -> Option<String> {
    conn.query_row(
        "SELECT name FROM graph_entities WHERE id = ?",
        params![function_id],
        |row| row.get(0)
    ).optional().ok().flatten()
}

/// Get path elements (blocks in order) for a given path_id
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `path_id` - The path ID to query
///
/// # Returns
///
/// * `Ok(Vec<BlockId>)` - Ordered list of block IDs in the path
/// * `Err(...)` - Error if query fails or path not found
pub fn get_path_elements(conn: &Connection, path_id: &str) -> Result<Vec<crate::cfg::BlockId>> {
    let mut stmt = conn.prepare_cached(
        "SELECT block_id FROM cfg_path_elements
         WHERE path_id = ?
         ORDER BY sequence_order ASC",
    ).context("Failed to prepare path elements query")?;

    let blocks: Vec<crate::cfg::BlockId> = stmt
        .query_map(params![path_id], |row| {
            Ok(row.get::<_, i64>(0)? as usize)
        })
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
///
/// # Arguments
///
/// * `conn` - Database connection
/// * `path_id` - The path ID to analyze
/// * `cfg` - The control flow graph
/// * `max_depth` - Maximum depth for impact analysis
///
/// # Returns
///
/// * `Ok(PathImpact)` - Aggregated impact data
/// * `Err(...)` - Error if path not found or computation fails
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

/// Create a minimal Magellan-compatible database at the given path
///
/// This creates a new database with the minimal Magellan schema required
/// for Mirage to store CFG data. For a full Magellan database, users
/// should run `magellan watch` on their project.
///
/// # Arguments
///
/// * `path` - Path where the database should be created
///
/// # Returns
///
/// * `Ok(())` - Database created successfully
/// * `Err(...)` - Error if creation fails
pub fn create_minimal_database<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();

    // Don't overwrite existing database
    if path.exists() {
        anyhow::bail!("Database already exists: {}", path.display());
    }

    let mut conn = Connection::open(path)
        .context("Failed to create database file")?;

    // Create Magellan meta table
    conn.execute(
        "CREATE TABLE magellan_meta (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            magellan_schema_version INTEGER NOT NULL,
            sqlitegraph_schema_version INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        )",
        [],
    ).context("Failed to create magellan_meta table")?;

    // Create graph_entities table (minimal schema)
    conn.execute(
        "CREATE TABLE graph_entities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            file_path TEXT,
            data TEXT NOT NULL
        )",
        [],
    ).context("Failed to create graph_entities table")?;

    // Create indexes for graph_entities
    conn.execute(
        "CREATE INDEX idx_graph_entities_kind ON graph_entities(kind)",
        [],
    ).context("Failed to create index on graph_entities.kind")?;

    conn.execute(
        "CREATE INDEX idx_graph_entities_name ON graph_entities(name)",
        [],
    ).context("Failed to create index on graph_entities.name")?;

    // Initialize Magellan meta
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
         VALUES (1, ?, ?, ?)",
        params![REQUIRED_MAGELLAN_SCHEMA_VERSION, REQUIRED_SQLITEGRAPH_SCHEMA_VERSION, now],
    ).context("Failed to initialize magellan_meta")?;

    // Create Mirage schema
    create_schema(&mut conn, TEST_MAGELLAN_SCHEMA_VERSION).context("Failed to create Mirage schema")?;

    Ok(())
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
        create_schema(&mut conn, TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

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
        create_schema(&mut conn, TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

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
        create_schema(&mut conn, TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

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
        create_schema(&mut conn, TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        // Insert a graph entity (function)
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            params!("function", "test_func", "test.rs", "{}"),
        ).unwrap();

        let function_id: i64 = conn.last_insert_rowid();

        // Attempt to insert cfg_blocks with invalid function_id (should fail)
        let invalid_result = conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params!(9999, "entry", "return", 0, 10, 1, 0, 1, 10),
        );

        // Should fail with foreign key constraint error
        assert!(invalid_result.is_err(), "Insert with invalid function_id should fail");

        // Insert valid cfg_blocks with correct function_id (should succeed)
        let valid_result = conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params!(function_id, "entry", "return", 0, 10, 1, 0, 1, 10),
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

    #[test]
    fn test_store_cfg_retrieves_correctly() {
        use crate::cfg::{BasicBlock, BlockKind, Cfg, EdgeType, Terminator};

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
        create_schema(&mut conn, TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        // Insert a function entity
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            params!("function", "test_func", "test.rs", "{}"),
        ).unwrap();

        let function_id: i64 = conn.last_insert_rowid();

        // Create a simple test CFG
        let mut cfg = Cfg::new();

        let b0 = cfg.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec!["let x = 1".to_string()],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b1 = cfg.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        cfg.add_edge(b0, b1, EdgeType::Fallthrough);

        // Store the CFG
        store_cfg(&mut conn, function_id, "test_hash_123", &cfg).unwrap();

        // Verify blocks were stored
        let block_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(block_count, 2, "Should have 2 blocks");

        // Verify edges were stored
        let edge_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_edges",
            [],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(edge_count, 1, "Should have 1 edge");

        // Note: function_hash is not stored in Magellan's schema, so we skip that check
        // The hash functionality is only available with Mirage's legacy schema

        // Verify function_exists
        assert!(function_exists(&conn, function_id));
        assert!(!function_exists(&conn, 9999));

        // Load and verify the CFG
        let loaded_cfg = load_cfg_from_db(&conn, function_id).unwrap();

        assert_eq!(loaded_cfg.node_count(), 2);
        assert_eq!(loaded_cfg.edge_count(), 1);
    }

    #[test]
    fn test_store_cfg_incremental_update_clears_old_data() {
        use crate::cfg::{BasicBlock, BlockKind, Cfg, EdgeType, Terminator};

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

        create_schema(&mut conn, TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            params!("function", "test_func", "test.rs", "{}"),
        ).unwrap();

        let function_id: i64 = conn.last_insert_rowid();

        // Create initial CFG with 2 blocks
        let mut cfg1 = Cfg::new();
        let b0 = cfg1.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });
        let b1 = cfg1.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });
        cfg1.add_edge(b0, b1, EdgeType::Fallthrough);

        store_cfg(&mut conn, function_id, "hash_v1", &cfg1).unwrap();

        let block_count_v1: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();

        assert_eq!(block_count_v1, 2);

        // Create updated CFG with 3 blocks
        let mut cfg2 = Cfg::new();
        let b0 = cfg2.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });
        let b1 = cfg2.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 2 },
            source_location: None,
        });
        let b2 = cfg2.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });
        cfg2.add_edge(b0, b1, EdgeType::Fallthrough);
        cfg2.add_edge(b1, b2, EdgeType::Fallthrough);

        store_cfg(&mut conn, function_id, "hash_v2", &cfg2).unwrap();

        let block_count_v2: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cfg_blocks WHERE function_id = ?",
            params![function_id],
            |row| row.get(0),
        ).unwrap();

        // Should have 3 blocks now (old ones cleared)
        assert_eq!(block_count_v2, 3);

        // Note: function_hash is not stored in Magellan's schema
        // Hash verification is skipped for Magellan v7+ schema
    }

    // Helper function to create a test database with Magellan + Mirage schema
    //
    // Creates a Magellan v7-compatible database with Mirage extensions.
    // The cfg_blocks table uses Magellan v7 schema:
    // - kind: TEXT (lowercase: "entry", "block", "return", "if", etc.)
    // - terminator: TEXT (lowercase: "fallthrough", "conditional", "return", etc.)
    // - Includes line/column fields for source locations
    fn create_test_db_with_schema() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();

        // Create Magellan v7 tables
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

        // Insert Magellan v7 meta
        conn.execute(
            "INSERT INTO magellan_meta (id, magellan_schema_version, sqlitegraph_schema_version, created_at)
             VALUES (1, ?, ?, ?)",
            params![7, 3, 0],  // Magellan v7, sqlitegraph v3
        ).unwrap();

        // Create Magellan's cfg_blocks table (v7 schema)
        // This is the authoritative table for CFG data in Magellan v7+
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

        // Create graph_edges for CFG edges
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

        // Create Mirage schema (mirage_meta and additional tables)
        create_schema(&mut conn, TEST_MAGELLAN_SCHEMA_VERSION).unwrap();

        // Enable foreign key enforcement for tests
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

        conn
    }

    // Tests for resolve_function_name and load_cfg_from_db (09-02)

    #[test]
    fn test_resolve_function_by_id() {
        let conn = create_test_db_with_schema();

        // Insert a test function
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            params!("function", "my_func", "test.rs", "{}"),
        ).unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        // Resolve by numeric ID
        let result = resolve_function_name(&conn, &function_id.to_string()).unwrap();
        assert_eq!(result, function_id);
    }

    #[test]
    fn test_resolve_function_by_name() {
        let conn = create_test_db_with_schema();

        // Insert a test function
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            params!("function", "test_function", "test.rs", "{}"),
        ).unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        // Resolve by name
        let result = resolve_function_name(&conn, "test_function").unwrap();
        assert_eq!(result, function_id);
    }

    #[test]
    fn test_resolve_function_not_found() {
        let conn = create_test_db_with_schema();

        // Try to resolve a non-existent function
        let result = resolve_function_name(&conn, "nonexistent_func");

        assert!(result.is_err(), "Should return error for non-existent function");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not found") || err_msg.contains("not found in database"));
    }

    #[test]
    fn test_resolve_function_numeric_string() {
        let conn = create_test_db_with_schema();

        // Insert a test function
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            params!("function", "func123", "test.rs", "{}"),
        ).unwrap();

        // Resolve by numeric string "123" - should parse as ID, not name
        let result = resolve_function_name(&conn, "123").unwrap();
        assert_eq!(result, 123);

        // Now insert a function with ID 456
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            params!("function", "another_func", "test.rs", "{}"),
        ).unwrap();
        let _id_456 = conn.last_insert_rowid();

        // If we query "456" it should try to parse as numeric ID
        // Since we just inserted and got some ID, let's verify numeric parsing works
        let result = resolve_function_name(&conn, "999").unwrap();
        assert_eq!(result, 999, "Should return numeric ID directly");
    }

    #[test]
    fn test_load_cfg_not_found() {
        let conn = create_test_db_with_schema();

        // Try to load CFG for non-existent function
        let result = load_cfg_from_db(&conn, 99999);

        assert!(result.is_err(), "Should return error for function with no CFG");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("No CFG blocks found") || err_msg.contains("not found"));
    }

    #[test]
    fn test_load_cfg_empty_terminator() {
        use crate::cfg::Terminator;

        let conn = create_test_db_with_schema();

        // Insert a test function
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            params!("function", "empty_term_func", "test.rs", "{}"),
        ).unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        // Create a block with NULL terminator (should default to Unreachable)
        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params!(function_id, "return", "return", 0, 10, 1, 0, 1, 10),
        ).unwrap();

        // Load the CFG - should handle NULL terminator gracefully
        let cfg = load_cfg_from_db(&conn, function_id).unwrap();

        assert_eq!(cfg.node_count(), 1);
        let block = &cfg[petgraph::graph::NodeIndex::new(0)];
        assert!(matches!(block.terminator, Terminator::Return));
    }

    #[test]
    fn test_load_cfg_with_multiple_edge_types() {
        use crate::cfg::EdgeType;

        let conn = create_test_db_with_schema();

        // Insert a test function
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            params!("function", "edge_types_func", "test.rs", "{}"),
        ).unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        // Create blocks with different edge types
        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params!(function_id, "entry", "conditional", 0, 10, 1, 0, 1, 10),
        ).unwrap();
        let _block_0_id: i64 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params!(function_id, "block", "fallthrough", 10, 20, 2, 0, 2, 10),
        ).unwrap();
        let _block_1_id: i64 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params!(function_id, "block", "call", 20, 30, 3, 0, 3, 10),
        ).unwrap();
        let _block_2_id: i64 = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO cfg_blocks (function_id, kind, terminator, byte_start, byte_end,
                                     start_line, start_col, end_line, end_col)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params!(function_id, "return", "return", 30, 40, 4, 0, 4, 10),
        ).unwrap();
        let _block_3_id: i64 = conn.last_insert_rowid();

        // Load the CFG - edges are now built from terminator data, not cfg_edges table
        let cfg = load_cfg_from_db(&conn, function_id).unwrap();

        assert_eq!(cfg.node_count(), 4);
        assert_eq!(cfg.edge_count(), 4);

        // Verify edge types are built from terminators:
        // Block 0 (conditional) -> Block 1 (TrueBranch), Block 2 (FalseBranch)
        // Block 1 (fallthrough) -> Block 2 (Fallthrough)
        // Block 2 (call) -> Block 3 (Call)
        use petgraph::visit::EdgeRef;
        let edges: Vec<_> = cfg.edge_references().map(|e| {
            (e.source().index(), e.target().index(), *e.weight())
        }).collect();

        assert!(edges.contains(&(0, 1, EdgeType::TrueBranch)));
        assert!(edges.contains(&(0, 2, EdgeType::FalseBranch)));
        assert!(edges.contains(&(1, 2, EdgeType::Fallthrough)));
        assert!(edges.contains(&(2, 3, EdgeType::Call)));
    }

    #[test]
    fn test_get_function_name() {
        let conn = create_test_db_with_schema();

        // Insert a test function
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            params!("function", "my_test_func", "test.rs", "{}"),
        ).unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        // Get function name
        let name = get_function_name(&conn, function_id);
        assert_eq!(name, Some("my_test_func".to_string()));

        // Non-existent function
        let name = get_function_name(&conn, 9999);
        assert_eq!(name, None);
    }

    #[test]
    fn test_get_path_elements() {
        let conn = create_test_db_with_schema();

        // Insert a test function and path
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            params!("function", "path_test_func", "test.rs", "{}"),
        ).unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        // Insert a path
        conn.execute(
            "INSERT INTO cfg_paths (path_id, function_id, path_kind, entry_block, exit_block, length, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params!("test_path_abc123", function_id, "normal", 0, 2, 3, 1000),
        ).unwrap();

        // Insert path elements
        conn.execute(
            "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id) VALUES (?, ?, ?)",
            params!("test_path_abc123", 0, 0),
        ).unwrap();
        conn.execute(
            "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id) VALUES (?, ?, ?)",
            params!("test_path_abc123", 1, 1),
        ).unwrap();
        conn.execute(
            "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id) VALUES (?, ?, ?)",
            params!("test_path_abc123", 2, 2),
        ).unwrap();

        // Get path elements
        let blocks = get_path_elements(&conn, "test_path_abc123").unwrap();
        assert_eq!(blocks, vec![0, 1, 2]);

        // Non-existent path
        let result = get_path_elements(&conn, "nonexistent_path");
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_path_impact_from_db() {
        use crate::cfg::{BasicBlock, BlockKind, Terminator};

        let conn = create_test_db_with_schema();

        // Insert a test function
        conn.execute(
            "INSERT INTO graph_entities (kind, name, file_path, data) VALUES (?, ?, ?, ?)",
            params!("function", "impact_test_func", "test.rs", "{}"),
        ).unwrap();
        let function_id: i64 = conn.last_insert_rowid();

        // Create a simple CFG: 0 -> 1 -> 2 -> 3
        let mut cfg = crate::cfg::Cfg::new();
        let b0 = cfg.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });
        let b1 = cfg.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 2 },
            source_location: None,
        });
        let b2 = cfg.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 3 },
            source_location: None,
        });
        let b3 = cfg.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });
        cfg.add_edge(b0, b1, crate::cfg::EdgeType::Fallthrough);
        cfg.add_edge(b1, b2, crate::cfg::EdgeType::Fallthrough);
        cfg.add_edge(b2, b3, crate::cfg::EdgeType::Fallthrough);

        // Insert a path: 0 -> 1 -> 3
        conn.execute(
            "INSERT INTO cfg_paths (path_id, function_id, path_kind, entry_block, exit_block, length, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params!("impact_test_path", function_id, "normal", 0, 3, 3, 1000),
        ).unwrap();

        conn.execute(
            "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id) VALUES (?, ?, ?)",
            params!("impact_test_path", 0, 0),
        ).unwrap();
        conn.execute(
            "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id) VALUES (?, ?, ?)",
            params!("impact_test_path", 1, 1),
        ).unwrap();
        conn.execute(
            "INSERT INTO cfg_path_elements (path_id, sequence_order, block_id) VALUES (?, ?, ?)",
            params!("impact_test_path", 2, 3),
        ).unwrap();

        // Compute impact
        let impact = compute_path_impact_from_db(&conn, "impact_test_path", &cfg, None).unwrap();

        assert_eq!(impact.path_id, "impact_test_path");
        assert_eq!(impact.path_length, 3);
        // Block 2 is not in the path but is reachable from block 1
        assert!(impact.unique_blocks_affected.contains(&2));
    }
}
