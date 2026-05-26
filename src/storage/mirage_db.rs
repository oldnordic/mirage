use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use sqlitegraph::{open_graph, GraphBackend, GraphConfig, SnapshotId};
use std::path::Path;

use super::{
    Backend, BackendFormat, CfgBlockRow, MIN_MAGELLAN_SCHEMA_VERSION, MIRAGE_SCHEMA_VERSION,
};

#[cfg(feature = "backend-geometric")]
use super::backend::create_geometric_stub_backend;

#[cfg(feature = "backend-sqlite")]
use super::schema::{create_schema, migrate_schema};

#[cfg(feature = "backend-sqlite")]
use super::operations::resolve_function_name_sqlite;

use super::operations::load_cfg_from_rows;

/// Database connection wrapper
///
/// Uses Backend enum for CFG queries (Phase 069-02) and GraphBackend for entity queries.
/// This dual-backend approach allows gradual migration from direct Connection usage.
pub struct MirageDb {
    /// Storage backend for CFG queries (Phase 069-02)
    /// Wraps either SqliteStorage or KvStorage for backend-agnostic CFG access.
    storage: Backend,

    /// Backend-agnostic graph interface for entity queries
    /// Used for entity_ids(), get_node(), kv_get() and other GraphBackend operations.
    graph_backend: Box<dyn GraphBackend>,

    /// Snapshot ID for consistent reads
    snapshot_id: SnapshotId,

    // SQLite-specific connection (only available with sqlite feature)
    // DEPRECATED: Use storage field instead for new code
    #[cfg(feature = "backend-sqlite")]
    conn: Option<Connection>,
}

impl std::fmt::Debug for MirageDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MirageDb")
            .field("snapshot_id", &self.snapshot_id)
            .field("storage", &self.storage)
            .field("graph_backend", &"<GraphBackend>")
            .finish()
    }
}

impl MirageDb {
    /// Open database at the given path
    ///
    /// This can open:
    /// - A Mirage database (with mirage_meta table)
    /// - A Magellan database (extends it with Mirage tables)
    ///
    /// Phase 069-02: Uses Backend::detect_and_open() for CFG queries
    /// and open_graph() for entity queries (GraphBackend).
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            anyhow::bail!("Database not found: {}", path.display());
        }

        // Phase 069-02: Use Backend::detect_and_open() for storage layer
        let storage = Backend::detect_and_open(path).context("Failed to open storage backend")?;

        // Detect backend format from file header for GraphBackend creation
        let detected_backend =
            BackendFormat::detect(path).context("Failed to detect backend format")?;

        // Handle geometric backend specially - it doesn't use GraphBackend
        #[cfg(feature = "backend-geometric")]
        if detected_backend == BackendFormat::Geometric {
            let snapshot_id = SnapshotId::current();

            let graph_backend = create_geometric_stub_backend();

            #[cfg(feature = "backend-sqlite")]
            let conn = None;

            return Ok(Self {
                storage,
                graph_backend,
                snapshot_id,
                #[cfg(feature = "backend-sqlite")]
                conn,
            });
        }

        // Select appropriate GraphConfig based on detected backend
        let cfg = match detected_backend {
            BackendFormat::SQLite => GraphConfig::sqlite(),
            BackendFormat::Geometric => GraphConfig::native(),
            BackendFormat::Unknown => {
                anyhow::bail!(
                    "Unknown database format: {}. Cannot determine backend.",
                    path.display()
                );
            }
        };

        // Use open_graph factory to create GraphBackend for entity queries
        let graph_backend = open_graph(path, &cfg).context("Failed to open graph database")?;

        let snapshot_id = SnapshotId::current();

        // For SQLite backend, open Connection and validate schema
        #[cfg(feature = "backend-sqlite")]
        let conn = {
            let mut conn = Connection::open(path).context("Failed to open SQLite connection")?;
            Self::validate_schema_sqlite(&mut conn, path)?;
            Some(conn)
        };

        Ok(Self {
            storage,
            graph_backend,
            snapshot_id,
            #[cfg(feature = "backend-sqlite")]
            conn,
        })
    }

    /// Validate database schema for SQLite backend
    #[cfg(feature = "backend-sqlite")]
    fn validate_schema_sqlite(conn: &mut Connection, _path: &Path) -> Result<()> {
        // Check if mirage_meta table exists
        let mirage_meta_exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='mirage_meta'",
                [],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0)
            == 1;

        // Get Mirage schema version (0 if table doesn't exist)
        let mirage_version: i32 = if mirage_meta_exists {
            conn.query_row(
                "SELECT mirage_schema_version FROM mirage_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?
            .flatten()
            .unwrap_or(0)
        } else {
            0
        };

        if mirage_version > MIRAGE_SCHEMA_VERSION {
            anyhow::bail!(
                "Database schema version {} is newer than supported version {}.
                 Please update Mirage.",
                mirage_version,
                MIRAGE_SCHEMA_VERSION
            );
        }

        // Check Magellan schema compatibility
        let magellan_version: i32 = conn
            .query_row(
                "SELECT magellan_schema_version FROM magellan_meta WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?
            .flatten()
            .unwrap_or(0);

        if magellan_version < MIN_MAGELLAN_SCHEMA_VERSION {
            anyhow::bail!(
                "Magellan schema version {} is too old (minimum {}). \
                 Please update Magellan and run 'magellan watch' to rebuild CFGs.",
                magellan_version,
                MIN_MAGELLAN_SCHEMA_VERSION
            );
        }

        // Check for cfg_blocks table existence (Magellan v7+)
        let cfg_blocks_exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='cfg_blocks'",
                [],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0)
            == 1;

        if !cfg_blocks_exists {
            anyhow::bail!(
                "CFG blocks table not found. Magellan schema v7+ required. \
                 Run 'magellan watch' to build CFGs."
            );
        }

        // If mirage_meta doesn't exist, this is a pure Magellan database.
        // Initialize Mirage tables to extend it.
        if !mirage_meta_exists {
            create_schema(conn, magellan_version)?;
        } else if mirage_version < MIRAGE_SCHEMA_VERSION {
            migrate_schema(conn)?;
        }

        Ok(())
    }

    /// Get a reference to the underlying Connection (SQLite backend only)
    ///
    /// Phase 069-02: DEPRECATED - Use storage() for CFG queries, backend() for entity queries.
    #[cfg(feature = "backend-sqlite")]
    pub fn conn(&self) -> Result<&Connection, anyhow::Error> {
        self.conn.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Direct Connection access deprecated. Use storage() for CFG queries or backend() for entity queries."
            )
        })
    }

    /// Get a mutable reference to the underlying Connection (SQLite backend only)
    ///
    /// Phase 069-02: DEPRECATED - Use storage() for CFG queries, backend() for entity queries.
    #[cfg(feature = "backend-sqlite")]
    pub fn conn_mut(&mut self) -> Result<&mut Connection, anyhow::Error> {
        self.conn.as_mut().ok_or_else(|| {
            anyhow::anyhow!(
                "Direct Connection access deprecated. Use storage() for CFG queries or backend() for entity queries."
            )
        })
    }

    /// Get a reference to the storage backend for CFG queries
    ///
    /// Phase 069-02: Use this to access CFG-specific storage operations
    /// like get_cfg_blocks(), get_entity(), and get_cached_paths().
    ///
    /// This is the preferred way to access CFG data in new code.
    pub fn storage(&self) -> &Backend {
        &self.storage
    }

    /// Get a reference to the backend-agnostic GraphBackend interface
    ///
    /// Use this for entity queries (entity_ids, get_node, kv_get, etc.).
    /// Phase 069-02: This now returns the GraphBackend used for entity queries,
    /// while storage() provides the Backend enum for CFG queries.
    pub fn backend(&self) -> &dyn GraphBackend {
        self.graph_backend.as_ref()
    }

    /// Check if the database backend is SQLite
    ///
    /// This is useful for runtime checks when certain features
    /// are only available with specific backends (e.g., path caching).
    #[cfg(feature = "backend-sqlite")]
    pub fn is_sqlite(&self) -> bool {
        self.conn.is_some()
    }

    /// List source documents from graph memory tables
    pub fn list_source_documents(&self) -> Result<Vec<super::DocumentInfo>> {
        self.storage.list_source_documents()
    }

    /// Get database statistics
    ///
    /// Note: cfg_edges count is included for backward compatibility but edges
    /// are now computed in memory from terminator data, not stored.
    #[cfg(feature = "backend-sqlite")]
    pub fn status(&self) -> Result<DatabaseStatus> {
        match self.conn.as_ref() {
            Some(conn) => {
                let cfg_blocks: i64 = conn
                    .query_row("SELECT COUNT(*) FROM cfg_blocks", [], |row| row.get(0))
                    .unwrap_or(0);

                let cfg_edges: i64 = conn
                    .query_row("SELECT COUNT(*) FROM cfg_edges", [], |row| row.get(0))
                    .unwrap_or(0);

                let cfg_paths: i64 = conn
                    .query_row("SELECT COUNT(*) FROM cfg_paths", [], |row| row.get(0))
                    .unwrap_or(0);

                let cfg_dominators: i64 = conn
                    .query_row("SELECT COUNT(*) FROM cfg_dominators", [], |row| row.get(0))
                    .unwrap_or(0);

                let mirage_schema_version: i32 = conn
                    .query_row(
                        "SELECT mirage_schema_version FROM mirage_meta WHERE id = 1",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);

                let magellan_schema_version: i32 = conn
                    .query_row(
                        "SELECT magellan_schema_version FROM magellan_meta WHERE id = 1",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);

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
            None => self.status_via_storage(),
        }
    }

    /// Helper function to get status via storage backend (for non-SQLite backends)
    #[cfg(feature = "backend-sqlite")]
    fn status_via_storage(&self) -> Result<DatabaseStatus> {
        #[cfg(feature = "backend-geometric")]
        {
            if let Backend::Geometric(ref geometric) = self.storage {
                let stats = geometric.get_stats()?;
                return Ok(DatabaseStatus {
                    cfg_blocks: stats.cfg_block_count as i64,
                    cfg_edges: 0,
                    cfg_paths: 0,
                    cfg_dominators: 0,
                    mirage_schema_version: MIRAGE_SCHEMA_VERSION,
                    magellan_schema_version: MIN_MAGELLAN_SCHEMA_VERSION,
                });
            }
        }

        #[allow(deprecated)]
        Ok(DatabaseStatus {
            cfg_blocks: 0,
            cfg_edges: 0,
            cfg_paths: 0,
            cfg_dominators: 0,
            mirage_schema_version: MIRAGE_SCHEMA_VERSION,
            magellan_schema_version: MIN_MAGELLAN_SCHEMA_VERSION,
        })
    }

    /// Get database statistics (geometric backend)
    ///
    /// Uses GeometricBackend methods to query symbol and CFG data.
    #[cfg(all(feature = "backend-geometric", not(feature = "backend-sqlite")))]
    pub fn status(&self) -> Result<DatabaseStatus> {
        let cfg_blocks: i64 = if let Backend::Geometric(ref geometric) = self.storage {
            0
        } else {
            0
        };

        let cfg_edges: i64 = 0;
        let cfg_paths: i64 = 0;
        let cfg_dominators: i64 = 0;

        let mirage_schema_version = MIRAGE_SCHEMA_VERSION;
        let magellan_schema_version = MIN_MAGELLAN_SCHEMA_VERSION;

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

    /// Resolve a function name or ID to a function_id (backend-agnostic)
    ///
    /// This method works with both SQLite and geometric backends.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use mirage_analyzer::storage::MirageDb;
    /// # fn main() -> anyhow::Result<()> {
    /// # let db = MirageDb::open("test.db")?;
    /// let func_id = db.resolve_function_name("123")?;
    /// let func_id = db.resolve_function_name("my_function")?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "backend-sqlite")]
    pub fn resolve_function_name(&self, name_or_id: &str) -> Result<i64> {
        self.resolve_function_name_with_file(name_or_id, None)
    }

    /// Resolve a function name or ID to a function_id with optional file filter
    ///
    /// For SQLite backend: queries the graph_entities table
    /// For geometric backend: uses GraphBackend::get_node
    #[cfg(feature = "backend-sqlite")]
    pub fn resolve_function_name_with_file(
        &self,
        name_or_id: &str,
        file_filter: Option<&str>,
    ) -> Result<i64> {
        // Try to parse as numeric ID first
        if let Ok(id) = name_or_id.parse::<i64>() {
            return Ok(id);
        }

        // Check if we have a SQLite connection (geometric backend has conn=None)
        if let Ok(conn) = self.conn() {
            resolve_function_name_sqlite(conn, name_or_id, file_filter)
        } else {
            // For geometric backend, use the storage backend directly
            #[cfg(feature = "backend-geometric")]
            {
                if let Backend::Geometric(ref geometric) = self.storage {
                    return self.resolve_function_name_geometric(name_or_id);
                }
            }
            anyhow::bail!("No database connection available for function resolution")
        }
    }

    /// Normalize path for deduplication purposes
    /// Converts paths to a canonical form for comparison
    #[cfg(feature = "backend-geometric")]
    fn normalize_path_for_dedup(path: &str) -> String {
        let path = path.replace('\\', "/");
        let path = path.strip_prefix("./").unwrap_or(&path);
        if let Some(idx) = path.find("/src/") {
            path[idx + 1..].to_string()
        } else {
            path.to_string()
        }
    }

    /// Resolve function name for geometric backend
    ///
    /// Accepts:
    /// - Numeric ID (e.g., "12345")
    /// - Full Qualified Name (FQN): magellan::/path/to/file.rs::FunctionName
    /// - Simple name (e.g., "FunctionName") - must be unique
    #[cfg(feature = "backend-geometric")]
    fn resolve_function_name_geometric(&self, name_or_id: &str) -> Result<i64> {
        if let Ok(id) = name_or_id.parse::<i64>() {
            if let Backend::Geometric(ref geometric) = self.storage {
                if geometric
                    .inner()
                    .find_symbol_by_id_info(id as u64)
                    .is_some()
                {
                    return Ok(id);
                }
            }
            anyhow::bail!("Function with ID '{}' not found", id);
        }

        if let Some(fqn_data) = Self::parse_fqn(name_or_id) {
            return self.resolve_function_by_fqn(fqn_data);
        }

        if let Backend::Geometric(ref geometric) = self.storage {
            let all_symbols = geometric.find_symbols_by_name(name_or_id);
            if all_symbols.is_empty() {
                anyhow::bail!("Function '{}' not found", name_or_id);
            }

            let mut unique_symbols: Vec<magellan::graph::geometric_backend::SymbolInfo> =
                Vec::new();
            let mut seen_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();

            for sym in all_symbols {
                if seen_ids.insert(sym.id) {
                    unique_symbols.push(sym);
                }
            }

            if unique_symbols.len() > 1 {
                let first = &unique_symbols[0];
                let first_path_normalized = Self::normalize_path_for_dedup(&first.file_path);
                let all_same_location = unique_symbols.iter().all(|sym| {
                    let sym_path_normalized = Self::normalize_path_for_dedup(&sym.file_path);
                    sym.name == first.name
                        && sym_path_normalized == first_path_normalized
                        && sym.start_line == first.start_line
                        && sym.start_col == first.start_col
                });

                if !all_same_location {
                    anyhow::bail!(
                        "Ambiguous function reference to '{}': {} unique candidates found\n\nCandidates:\n{}\n\nUse full qualified name: magellan::/path/to/file.rs::{}",
                        name_or_id,
                        unique_symbols.len(),
                        unique_symbols.iter().map(|s| {
                            format!("  - {} ({}:{}:{})", s.name, s.file_path, s.start_line, s.start_col)
                        }).collect::<Vec<_>>().join("\n"),
                        name_or_id
                    );
                }
            }
            Ok(unique_symbols[0].id as i64)
        } else {
            anyhow::bail!("Geometric backend not available")
        }
    }

    /// Parse FQN format: magellan::/path/to/file.rs::Function symbol_name
    /// Returns (file_path, symbol_name) if valid FQN
    #[cfg(feature = "backend-geometric")]
    fn parse_fqn(name: &str) -> Option<(&str, &str)> {
        if !name.starts_with("magellan::") {
            return None;
        }

        let after_prefix = &name[10..];

        if let Some(last_sep_pos) = after_prefix.rfind("::") {
            let file_path = &after_prefix[..last_sep_pos];
            let name_part = &after_prefix[last_sep_pos + 2..];

            let symbol_name = if let Some(space_pos) = name_part.find(' ') {
                &name_part[space_pos + 1..]
            } else {
                name_part
            };

            if !file_path.is_empty() && !symbol_name.is_empty() {
                return Some((file_path, symbol_name));
            }
        }

        None
    }

    /// Resolve function by FQN (file path + symbol name)
    #[cfg(feature = "backend-geometric")]
    fn resolve_function_by_fqn(&self, fqn_data: (&str, &str)) -> Result<i64> {
        let (file_path, symbol_name) = fqn_data;

        if let Backend::Geometric(ref geometric) = self.storage {
            match geometric.find_symbol_id_by_name_and_path(symbol_name, file_path) {
                Some(id) => Ok(id as i64),
                None => {
                    let all_symbols = geometric.find_symbols_by_name(symbol_name);
                    let normalized_target = Self::normalize_path_for_dedup(file_path);

                    let matching_symbols: Vec<_> = all_symbols
                        .into_iter()
                        .filter(|sym| {
                            let sym_path_normalized =
                                Self::normalize_path_for_dedup(&sym.file_path);
                            sym_path_normalized == normalized_target
                        })
                        .collect();

                    if matching_symbols.is_empty() {
                        anyhow::bail!(
                            "Function '{}' not found in file '{}'",
                            symbol_name,
                            file_path
                        );
                    } else {
                        anyhow::bail!(
                            "Multiple functions named '{}' found in file '{}' ({} matches). Use numeric ID instead.",
                            symbol_name,
                            file_path,
                            matching_symbols.len()
                        );
                    }
                }
            }
        } else {
            anyhow::bail!("Geometric backend not available")
        }
    }

    /// Load a CFG from the database (backend-agnostic)
    ///
    /// For SQLite backend: uses SQL query on cfg_blocks table
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use mirage_analyzer::storage::MirageDb;
    /// # fn main() -> anyhow::Result<()> {
    /// # let db = MirageDb::open("test.db")?;
    /// let cfg = db.load_cfg(123)?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "backend-sqlite")]
    pub fn load_cfg(&self, function_id: i64) -> Result<crate::cfg::Cfg> {
        let blocks = self.storage().get_cfg_blocks(function_id)?;

        if blocks.is_empty() {
            anyhow::bail!(
                "No CFG blocks found for function_id {}. Run 'magellan watch' to build CFGs.",
                function_id
            );
        }

        let file_path = self.get_function_file(function_id);

        let block_rows: Vec<CfgBlockRow> = blocks
            .into_iter()
            .enumerate()
            .map(|(idx, b)| {
                (
                    idx as i64,
                    b.kind,
                    Some(b.terminator),
                    Some(b.byte_start as i64),
                    Some(b.byte_end as i64),
                    Some(b.start_line as i64),
                    Some(b.start_col as i64),
                    Some(b.end_line as i64),
                    Some(b.end_col as i64),
                    Some(b.coord_x),
                    Some(b.coord_y),
                    Some(b.coord_z),
                    b.cfg_condition,
                )
            })
            .collect();

        let cfg_edges: Vec<(i64, i64, String)> = if let Ok(conn) = self.conn() {
            match conn.prepare_cached(
                "SELECT source_idx, target_idx, edge_type
                 FROM cfg_edges
                 WHERE function_id = ?
                 ORDER BY source_idx, target_idx",
            ) {
                Ok(mut stmt) => {
                    match stmt.query_map(params![function_id], |row| {
                        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                    }) {
                        Ok(rows) => rows.collect::<Result<Vec<_>, _>>().unwrap_or_default(),
                        Err(_) => vec![],
                    }
                }
                Err(_) => vec![],
            }
        } else {
            vec![]
        };

        load_cfg_from_rows(
            block_rows,
            file_path.map(std::path::PathBuf::from),
            cfg_edges,
        )
    }

    /// Get the function name for a given function_id (backend-agnostic)
    ///
    /// For SQLite backend: queries the graph_entities table
    /// For Geometric backend: uses GraphBackend::get_node
    pub fn get_function_name(&self, function_id: i64) -> Option<String> {
        let snapshot = SnapshotId::current();
        self.backend()
            .get_node(snapshot, function_id)
            .ok()
            .and_then(|entity| {
                if entity.kind == "Symbol"
                    && entity.data.get("kind").and_then(|v| v.as_str()) == Some("Function")
                {
                    Some(entity.name)
                } else {
                    None
                }
            })
    }

    /// Get the file path for a given function_id (backend-agnostic)
    pub fn get_function_file(&self, function_id: i64) -> Option<String> {
        let snapshot = SnapshotId::current();
        self.backend()
            .get_node(snapshot, function_id)
            .ok()
            .and_then(|entity| entity.file_path)
    }

    /// Check if a function has CFG blocks (SQLite backend)
    #[cfg(feature = "backend-sqlite")]
    pub fn function_exists(&self, function_id: i64) -> bool {
        use crate::storage::function_exists;
        self.conn()
            .map(|conn| function_exists(conn, function_id))
            .unwrap_or(false)
    }

    /// Get the function hash for path caching (SQLite backend)
    #[cfg(feature = "backend-sqlite")]
    pub fn get_function_hash(&self, function_id: i64) -> Option<String> {
        use crate::storage::get_function_hash;
        self.conn()
            .map(|conn| get_function_hash(conn, function_id))
            .ok()
            .flatten()
    }
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

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_open_database_old_magellan_schema() {
        let db_file = tempfile::NamedTempFile::new().unwrap();
        {
            let conn = Connection::open(db_file.path()).unwrap();
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
                 VALUES (1, 6, 3, 0)",
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
            )
            .unwrap();
        }

        let result = MirageDb::open(db_file.path());
        assert!(result.is_err(), "Should fail with old Magellan schema");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("too old") || err_msg.contains("minimum"),
            "Error should mention schema too old: {}",
            err_msg
        );
        assert!(
            err_msg.contains("magellan watch"),
            "Error should suggest running magellan watch: {}",
            err_msg
        );
    }
}
