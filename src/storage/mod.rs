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
pub use paths::{
    PathCache,
    store_paths,
    get_cached_paths,
    invalidate_function_paths,
    update_function_paths_if_changed,
};

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
        "Function '{}' not found in database. Run 'mirage index' to index functions.",
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
/// 1. Query all cfg_blocks for the function, ordered by id
/// 2. For each block, create a BasicBlock with:
///    - id: sequential index (0, 1, 2...) based on query order
///    - kind: parsed from BlockKind string (Entry/Normal/Exit)
///    - terminator: deserialized from JSON string
///    - source_location: None (future enhancement)
///    - statements: empty vec! (future enhancement)
/// 3. Query all cfg_edges connecting blocks for this function
/// 4. Map database block IDs to graph node indices
/// 5. Add edges to the graph with parsed EdgeType
/// 6. Return the constructed Cfg
///
/// # Notes
///
/// - Block IDs in the database (AUTOINCREMENT) are mapped to sequential
///   indices in the CFG graph (0, 1, 2...) for consistency with in-memory CFG construction
/// - Terminator is stored as JSON in the database and deserialized via serde_json
pub fn load_cfg_from_db(conn: &Connection, function_id: i64) -> Result<crate::cfg::Cfg> {
    use crate::cfg::{BasicBlock, BlockKind, Cfg, EdgeType};
    use petgraph::graph::NodeIndex;

    // Query all blocks for this function
    let mut stmt = conn.prepare_cached(
        "SELECT id, block_kind, byte_start, byte_end, terminator
         FROM cfg_blocks
         WHERE function_id = ?
         ORDER BY id ASC",
    ).context("Failed to prepare cfg_blocks query")?;

    let block_rows: Vec<(i64, String, Option<i64>, Option<i64>, Option<String>)> = stmt
        .query_map(params![function_id], |row| {
            Ok((
                row.get(0)?,     // id (database primary key)
                row.get(1)?,     // block_kind
                row.get(2)?,     // byte_start
                row.get(3)?,     // byte_end
                row.get(4)?,     // terminator (JSON string)
            ))
        })
        .context("Failed to execute cfg_blocks query")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect cfg_blocks rows")?;

    if block_rows.is_empty() {
        anyhow::bail!(
            "No CFG blocks found for function_id {}. Run 'mirage index' to build CFGs.",
            function_id
        );
    }

    // Build mapping from database block ID to graph node index
    let mut db_id_to_node: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
    let mut graph = Cfg::new();

    // Add each block to the graph
    for (node_idx, (db_id, kind_str, _byte_start, _byte_end, terminator_json)) in
        block_rows.iter().enumerate()
    {
        // Parse block kind
        let kind = match kind_str.as_str() {
            "Entry" => BlockKind::Entry,
            "Exit" => BlockKind::Exit,
            "Normal" => BlockKind::Normal,
            _ => anyhow::bail!("Invalid block_kind '{}'", kind_str),
        };

        // Deserialize terminator from JSON
        let terminator = if let Some(json) = terminator_json {
            serde_json::from_str(json)
                .with_context(|| format!("Failed to deserialize terminator: {}", json))?
        } else {
            // Default terminator for blocks without one (shouldn't happen in valid CFGs)
            crate::cfg::Terminator::Unreachable
        };

        let block = BasicBlock {
            id: node_idx,
            kind,
            statements: vec![], // Empty for now - future enhancement
            terminator,
            source_location: None, // Future enhancement: load from source_location table
        };

        graph.add_node(block);
        db_id_to_node.insert(*db_id, node_idx);
    }

    // Query all edges for this function's blocks
    let mut stmt = conn.prepare_cached(
        "SELECT e.from_id, e.to_id, e.edge_type
         FROM cfg_edges e
         INNER JOIN cfg_blocks b1 ON e.from_id = b1.id
         INNER JOIN cfg_blocks b2 ON e.to_id = b2.id
         WHERE b1.function_id = ? OR b2.function_id = ?",
    ).context("Failed to prepare cfg_edges query")?;

    let edge_rows: Vec<(i64, i64, String)> = stmt
        .query_map(params![function_id, function_id], |row| {
            Ok((
                row.get(0)?, // from_id (database block ID)
                row.get(1)?, // to_id (database block ID)
                row.get(2)?, // edge_type
            ))
        })
        .context("Failed to execute cfg_edges query")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect cfg_edges rows")?;

    // Add edges to the graph
    for (from_db_id, to_db_id, edge_type_str) in edge_rows {
        let from_idx = db_id_to_node.get(&from_db_id).context(format!(
            "Edge references unknown from_id {}",
            from_db_id
        ))?;
        let to_idx = db_id_to_node.get(&to_db_id).context(format!(
            "Edge references unknown to_id {}",
            to_db_id
        ))?;

        // Parse edge type
        let edge_type = match edge_type_str.as_str() {
            "Fallthrough" => EdgeType::Fallthrough,
            "TrueBranch" => EdgeType::TrueBranch,
            "FalseBranch" => EdgeType::FalseBranch,
            "LoopBack" => EdgeType::LoopBack,
            "LoopExit" => EdgeType::LoopExit,
            "Call" => EdgeType::Call,
            "Return" => EdgeType::Return,
            "Exception" => EdgeType::Exception,
            _ => anyhow::bail!("Invalid edge_type '{}'", edge_type_str),
        };

        graph.add_edge(
            NodeIndex::new(*from_idx),
            NodeIndex::new(*to_idx),
            edge_type,
        );
    }

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
/// 4. Insert each edge as a row in cfg_edges:
///    - Use database row IDs from inserted blocks
///    - Serialize edge type as string
/// 5. Commit transaction
///
/// # Notes
///
/// - Uses BEGIN IMMEDIATE to acquire write lock early (prevents write conflicts)
/// - Existing blocks/edges are cleared for incremental updates
/// - Block IDs are AUTOINCREMENT in the database
pub fn store_cfg(
    conn: &mut Connection,
    function_id: i64,
    function_hash: &str,
    cfg: &crate::cfg::Cfg,
) -> Result<()> {
    use crate::cfg::{BlockKind, EdgeType};
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
        "INSERT INTO cfg_blocks (function_id, block_kind, byte_start, byte_end, terminator, function_hash)
         VALUES (?, ?, ?, ?, ?, ?)",
    ).context("Failed to prepare block insert statement")?;

    for node_idx in cfg.node_indices() {
        let block = cfg.node_weight(node_idx)
            .context("CFG node has no weight")?;

        // Serialize terminator as JSON
        let terminator_json = serde_json::to_string(&block.terminator)
            .context("Failed to serialize terminator")?;

        // Get byte range from source location
        let (byte_start, byte_end) = block.source_location.as_ref()
            .map(|loc| (Some(loc.byte_start), Some(loc.byte_end)))
            .unwrap_or((None, None));

        // Convert BlockKind to string
        let block_kind = match block.kind {
            BlockKind::Entry => "Entry",
            BlockKind::Normal => "Normal",
            BlockKind::Exit => "Exit",
        };

        insert_block.execute(params![
            function_id,
            block_kind,
            byte_start,
            byte_end,
            terminator_json,
            function_hash,
        ]).context("Failed to insert cfg_block")?;

        let db_id = conn.last_insert_rowid();
        block_id_map.insert(node_idx, db_id);
    }

    // Insert each edge
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
pub fn get_function_hash(conn: &Connection, function_id: i64) -> Option<String> {
    conn.query_row(
        "SELECT function_hash FROM cfg_blocks WHERE function_id = ? LIMIT 1",
        params![function_id],
        |row| row.get(0)
    ).optional().ok().flatten()
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
        create_schema(&mut conn).unwrap();

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

        // Verify function_hash was stored
        let stored_hash = get_function_hash(&conn, function_id);
        assert_eq!(stored_hash, Some("test_hash_123".to_string()));

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

        create_schema(&mut conn).unwrap();

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

        // Verify hash was updated
        let stored_hash = get_function_hash(&conn, function_id);
        assert_eq!(stored_hash, Some("hash_v2".to_string()));
    }
}
