// Database storage layer extending Magellan's schema
//
// Mirage uses the same Magellan database and extends it with:
// - cfg_blocks: Basic blocks within functions (managed by Magellan v7+)
// - cfg_paths: Enumerated execution paths
// - cfg_path_elements: Blocks in each path
// - cfg_dominators: Dominance relationships
// - cfg_post_dominators: Reverse dominance
//
// Note: cfg_edges table is managed by Magellan v11+; Mirage computes
// edges in memory from terminator data and does not create/query this table.

pub mod paths;

// Backend-agnostic storage trait and implementations (Phase 069-01)
#[cfg(feature = "backend-geometric")]
pub mod geometric;
#[cfg(feature = "backend-sqlite")]
pub mod sqlite_backend;

// Also support the aliased feature names for convenience
#[cfg(all(feature = "geometric", not(feature = "backend-geometric")))]
pub mod geometric;
#[cfg(all(feature = "sqlite", not(feature = "backend-sqlite")))]
pub mod sqlite_backend;

pub mod backend;
pub mod mirage_db;
pub mod operations;
pub mod queries;
pub mod schema;

use anyhow::Result;

// Backend implementations (Phase 069-01)
#[cfg(feature = "backend-geometric")]
pub use backend::create_geometric_stub_backend;
#[cfg(feature = "backend-geometric")]
pub use geometric::GeometricStorage;
#[cfg(feature = "backend-sqlite")]
pub use sqlite_backend::SqliteStorage;

// Re-export path caching functions
#[allow(unused_imports)]
pub use paths::{
    get_cached_paths, invalidate_function_paths, store_paths, update_function_paths_if_changed,
    PathCache,
};

// Re-exports from backend
pub use backend::{Backend, BackendFormat};

// Re-exports from mirage_db
pub use mirage_db::{DatabaseStatus, MirageDb};

// Re-exports from schema
pub use schema::{create_minimal_database, create_schema, migrate_schema};

// Re-exports from operations
pub use operations::{
    load_cfg_from_db, load_cfg_from_db_with_conn, resolve_function_name,
    resolve_function_name_with_conn, resolve_function_name_with_file,
};

// Re-exports from queries
#[allow(deprecated)]
pub use queries::{
    compute_path_impact_from_db, function_exists, get_changed_functions, get_function_file,
    get_function_file_db, get_function_hash, get_function_hash_db, get_function_name,
    get_function_name_db, get_path_elements, hash_changed, store_cfg,
};

/// Row tuple from cfg_blocks table queries
/// Fields: id, kind, terminator, byte_start, byte_end,
///         start_line, start_col, end_line, end_col,
///         coord_x, coord_y, coord_z, cfg_condition
type CfgBlockRow = (
    i64,
    String,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<String>,
);

// ============================================================================
// Backend-Agnostic Storage Trait (Phase 069-01)
// ============================================================================

/// Backend-agnostic storage trait for CFG data
///
/// This trait abstracts over supported storage backends,
/// enabling runtime backend detection and zero breaking changes.
///
/// # Design
///
/// - Follows llmgrep's Backend pattern for consistency
/// - All methods take `&self` (not `&mut self`) to enable shared access
/// - Errors are returned as `anyhow::Error` for flexibility
///
/// # Examples
///
/// ```ignore
/// # use mirage_analyzer::storage::{StorageTrait, Backend};
/// # fn main() -> anyhow::Result<()> {
/// // Auto-detect and open backend
/// let backend = Backend::detect_and_open("/path/to/db")?;
///
/// // Query CFG blocks
/// let blocks = backend.get_cfg_blocks(123)?;
/// # Ok(())
/// # }
/// ```
pub trait StorageTrait {
    /// Get CFG blocks for a function
    fn get_cfg_blocks(&self, function_id: i64) -> Result<Vec<CfgBlockData>>;

    /// Get entity by ID
    fn get_entity(&self, entity_id: i64) -> Option<sqlitegraph::GraphEntity>;

    /// Get cached paths for a function (optional)
    fn get_cached_paths(&self, _function_id: i64) -> Result<Option<Vec<crate::cfg::Path>>> {
        Ok(None)
    }

    /// Get callees (functions called by the given function)
    fn get_callees(&self, _function_id: i64) -> Result<Vec<i64>> {
        Ok(Vec::new())
    }

    fn get_documents_for_function(&self, _function_id: i64) -> Result<Vec<DocumentInfo>> {
        Ok(Vec::new())
    }

    fn get_facts_for_function(&self, _function_id: i64) -> Result<Vec<FactInfo>> {
        Ok(Vec::new())
    }

    fn list_source_documents(&self) -> Result<Vec<DocumentInfo>> {
        Ok(Vec::new())
    }
}

/// CFG block data (backend-agnostic representation)
///
/// This struct represents the data returned by `StorageTrait::get_cfg_blocks`.
/// It is a simplified version of Magellan's CfgBlock that contains only the
/// fields needed by Mirage for CFG analysis.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CfgBlockData {
    /// Block ID (from cfg_blocks table)
    pub id: i64,
    /// Block kind (entry, conditional, loop, match, return, etc.)
    pub kind: String,
    /// Terminator kind (how control exits this block)
    pub terminator: String,
    /// Byte offset where block starts
    pub byte_start: u64,
    /// Byte offset where block ends
    pub byte_end: u64,
    /// Line where block starts (1-indexed)
    pub start_line: u64,
    /// Column where block starts (0-indexed)
    pub start_col: u64,
    /// Line where block ends (1-indexed)
    pub end_line: u64,
    /// Column where block ends (0-indexed)
    pub end_col: u64,
    /// 4D Spatial Coordinates
    /// X coordinate: dominator depth (control flow hierarchy level)
    pub coord_x: i64,
    /// Y coordinate: loop nesting depth (how many loops surround this block)
    pub coord_y: i64,
    /// Z coordinate: branch distance from entry point
    pub coord_z: i64,
    /// Optional #[cfg(...)] condition for feature-gated branch elimination
    pub cfg_condition: Option<String>,
}

/// Source document metadata from graph memory tables.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocumentInfo {
    pub id: i64,
    pub path_or_uri: String,
    pub source_kind: String,
    pub title: Option<String>,
    pub tags: Option<String>,
    pub wikilinks: Option<String>,
}

/// Candidate fact from graph memory tables.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FactInfo {
    pub candidate_id: String,
    pub subject_type: String,
    pub subject_key: String,
    pub predicate: String,
    pub object_type: Option<String>,
    pub object_key: Option<String>,
    pub status: String,
    pub source_document_id: i64,
}

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
