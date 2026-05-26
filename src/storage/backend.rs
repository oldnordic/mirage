use anyhow::Result;
use std::path::Path;

use super::{CfgBlockData, DocumentInfo, StorageTrait};

#[cfg(feature = "backend-geometric")]
use super::geometric::GeometricStorage;

#[cfg(feature = "backend-sqlite")]
use super::sqlite_backend::SqliteStorage;

/// Storage backend enum (Phase 069-01)
///
/// This enum wraps SqliteStorage or GeometricStorage and delegates
/// StorageTrait methods to the appropriate implementation.
///
/// Follows llmgrep's Backend pattern for consistency across tools.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Backend {
    /// SQLite storage backend (traditional, always available)
    #[cfg(feature = "backend-sqlite")]
    Sqlite(SqliteStorage),
    /// Geometric storage backend for .geo files (Magellan 3.0+)
    #[cfg(feature = "backend-geometric")]
    Geometric(GeometricStorage),
}

impl Backend {
    /// Detect backend format from database file and open appropriate backend
    ///
    /// Uses file extension and magellan's detection for consistent backend detection.
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to the database file
    ///
    /// # Returns
    ///
    /// * `Ok(Backend)` - Appropriate backend variant
    /// * `Err(...)` - Error if detection or opening fails
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use mirage_analyzer::storage::Backend;
    /// # fn main() -> anyhow::Result<()> {
    /// let backend = Backend::detect_and_open("/path/to/codegraph.db")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn detect_and_open(db_path: &Path) -> Result<Self> {
        use magellan::migrate_backend_cmd::detect_backend_format;

        // Check for .geo extension first (Magellan 3.0+ geometric backend)
        #[cfg(feature = "backend-geometric")]
        let is_geo = db_path.extension().and_then(|e| e.to_str()) == Some("geo");

        #[cfg(feature = "backend-geometric")]
        {
            if is_geo {
                return GeometricStorage::open(db_path).map(Backend::Geometric);
            }
        }

        // For non-.geo files, use Magellan's SQLite detection.
        let sqlite_detected = detect_backend_format(db_path).is_ok();

        #[cfg(feature = "backend-sqlite")]
        {
            if sqlite_detected {
                SqliteStorage::open(db_path).map(Backend::Sqlite)
            } else {
                Err(anyhow::anyhow!(
                    "Unsupported database format; use a SQLite .db"
                ))
            }
        }

        #[cfg(not(any(feature = "backend-sqlite", feature = "backend-geometric")))]
        {
            let _ = sqlite_detected;
            Err(anyhow::anyhow!("No storage backend feature enabled"))
        }
    }

    /// Check if this is a Geometric backend
    pub fn is_geometric(&self) -> bool {
        match self {
            #[cfg(feature = "backend-geometric")]
            Backend::Geometric(_) => true,
            _ => false,
        }
    }

    /// Check if this is a SQLite backend
    pub fn is_sqlite(&self) -> bool {
        match self {
            #[cfg(feature = "backend-sqlite")]
            Backend::Sqlite(_) => true,
            #[cfg(feature = "backend-geometric")]
            Backend::Geometric(_) => false,
            #[cfg(not(feature = "backend-sqlite"))]
            _ => false,
        }
    }

    /// Delegate get_cfg_blocks to inner backend
    pub fn get_cfg_blocks(&self, function_id: i64) -> Result<Vec<CfgBlockData>> {
        match self {
            #[cfg(feature = "backend-sqlite")]
            Backend::Sqlite(s) => s.get_cfg_blocks(function_id),
            #[cfg(feature = "backend-geometric")]
            Backend::Geometric(g) => g.get_cfg_blocks(function_id),
            #[allow(unreachable_patterns)]
            _ => Err(anyhow::anyhow!("No storage backend available")),
        }
    }

    /// Delegate get_entity to inner backend
    pub fn get_entity(&self, entity_id: i64) -> Option<sqlitegraph::GraphEntity> {
        match self {
            #[cfg(feature = "backend-sqlite")]
            Backend::Sqlite(s) => s.get_entity(entity_id),
            #[cfg(feature = "backend-geometric")]
            Backend::Geometric(g) => g.get_entity(entity_id),
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    /// Delegate get_cached_paths to inner backend
    pub fn get_cached_paths(&self, function_id: i64) -> Result<Option<Vec<crate::cfg::Path>>> {
        match self {
            #[cfg(feature = "backend-sqlite")]
            Backend::Sqlite(s) => s.get_cached_paths(function_id),
            #[cfg(feature = "backend-geometric")]
            Backend::Geometric(g) => g.get_cached_paths(function_id),
            #[allow(unreachable_patterns)]
            _ => Err(anyhow::anyhow!("No storage backend available")),
        }
    }

    /// Delegate get_callees to inner backend
    pub fn get_callees(&self, function_id: i64) -> Result<Vec<i64>> {
        match self {
            #[cfg(feature = "backend-sqlite")]
            Backend::Sqlite(s) => s.get_callees(function_id),
            #[cfg(feature = "backend-geometric")]
            Backend::Geometric(g) => g.get_callees(function_id),
            #[allow(unreachable_patterns)]
            _ => Ok(Vec::new()),
        }
    }

    /// Delegate list_source_documents to inner backend
    pub fn list_source_documents(&self) -> Result<Vec<DocumentInfo>> {
        match self {
            #[cfg(feature = "backend-sqlite")]
            Backend::Sqlite(s) => s.list_source_documents(),
            #[cfg(feature = "backend-geometric")]
            Backend::Geometric(g) => g.list_source_documents(),
            #[allow(unreachable_patterns)]
            _ => Ok(Vec::new()),
        }
    }
}

// Implement StorageTrait for Backend (delegates to inner storage)
impl StorageTrait for Backend {
    fn get_cfg_blocks(&self, function_id: i64) -> Result<Vec<CfgBlockData>> {
        self.get_cfg_blocks(function_id)
    }

    fn get_entity(&self, entity_id: i64) -> Option<sqlitegraph::GraphEntity> {
        self.get_entity(entity_id)
    }

    fn get_cached_paths(&self, function_id: i64) -> Result<Option<Vec<crate::cfg::Path>>> {
        self.get_cached_paths(function_id)
    }

    fn get_callees(&self, function_id: i64) -> Result<Vec<i64>> {
        self.get_callees(function_id)
    }
}

/// Database backend format detected in a graph database file.
///
/// This is the legacy format detection enum. For new code, use the
/// `Backend` enum (with StorageTrait) which provides full backend abstraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendFormat {
    /// SQLite-based backend (default, backward compatible)
    SQLite,
    /// Geometric backend (.geo files, Magellan 3.0+)
    Geometric,
    /// Unknown or unrecognized format
    Unknown,
}

impl BackendFormat {
    /// Detect which backend format a database file uses.
    ///
    /// Checks the file header to determine if the database is SQLite format.
    /// Returns Unknown if the file doesn't exist or has an unrecognized header.
    ///
    /// **Deprecated:** Use `Backend::detect_and_open()` for new code which provides
    /// full backend abstraction, not just format detection.
    pub fn detect(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(BackendFormat::Unknown);
        }

        // Check for .geo extension first (Magellan 3.0+ geometric backend)
        if path.extension().and_then(|e| e.to_str()) == Some("geo") {
            return Ok(BackendFormat::Geometric);
        }

        let mut file = std::fs::File::open(path)?;
        let mut header = [0u8; 16];
        let bytes_read = std::io::Read::read(&mut file, &mut header)?;

        if bytes_read < header.len() {
            return Ok(BackendFormat::Unknown);
        }

        // SQLite databases start with "SQLite format 3"
        Ok(if &header[..15] == b"SQLite format 3" {
            BackendFormat::SQLite
        } else {
            BackendFormat::Unknown
        })
    }
}

/// Create a stub GraphBackend for geometric backend
///
/// Geometric backend doesn't use sqlitegraph's GraphBackend trait.
/// Instead, it provides its own query methods directly via GeometricBackend.
/// This stub is used to satisfy the MirageDb struct's graph_backend field.
///
/// Any code that tries to use GraphBackend methods on a geometric database
/// will get appropriate errors directing them to use the geometric-specific
/// methods instead.
#[cfg(feature = "backend-geometric")]
pub(super) fn create_geometric_stub_backend() -> Box<dyn GraphBackend> {
    use sqlitegraph::backend::{
        BackendDirection, BackupResult, EdgeSpec, ImportMetadata, NeighborQuery, NodeSpec,
        SnapshotMetadata,
    };
    use sqlitegraph::multi_hop::ChainStep;
    use sqlitegraph::pattern::{PatternMatch, PatternQuery};
    use sqlitegraph::{GraphBackend, GraphEntity, SqliteGraphError};

    /// Stub GraphBackend implementation for geometric backend
    /// All methods return errors since geometric uses its own API
    struct GeometricStubBackend;

    impl GraphBackend for GeometricStubBackend {
        fn insert_node(&self, _node: NodeSpec) -> Result<i64, SqliteGraphError> {
            Err(SqliteGraphError::unsupported(
                "GraphBackend operations not supported for geometric backend. Use GeometricBackend methods directly."
            ))
        }

        fn insert_edge(&self, _edge: EdgeSpec) -> Result<i64, SqliteGraphError> {
            Err(SqliteGraphError::unsupported(
                "GraphBackend operations not supported for geometric backend. Use GeometricBackend methods directly."
            ))
        }

        fn update_node(&self, _node_id: i64, _node: NodeSpec) -> Result<i64, SqliteGraphError> {
            Err(SqliteGraphError::unsupported(
                "GraphBackend operations not supported for geometric backend. Use GeometricBackend methods directly."
            ))
        }

        fn delete_entity(&self, _id: i64) -> Result<(), SqliteGraphError> {
            Err(SqliteGraphError::unsupported(
                "GraphBackend operations not supported for geometric backend. Use GeometricBackend methods directly."
            ))
        }

        fn entity_ids(&self) -> Result<Vec<i64>, SqliteGraphError> {
            Ok(vec![])
        }

        fn get_node(
            &self,
            _snapshot_id: SnapshotId,
            _id: i64,
        ) -> Result<GraphEntity, SqliteGraphError> {
            Err(SqliteGraphError::unsupported(
                "GraphBackend operations not supported for geometric backend. Use GeometricBackend methods directly."
            ))
        }

        fn neighbors(
            &self,
            _snapshot_id: SnapshotId,
            _node: i64,
            _query: NeighborQuery,
        ) -> Result<Vec<i64>, SqliteGraphError> {
            Ok(vec![])
        }

        fn bfs(
            &self,
            _snapshot_id: SnapshotId,
            _start: i64,
            _depth: u32,
        ) -> Result<Vec<i64>, SqliteGraphError> {
            Ok(vec![])
        }

        fn shortest_path(
            &self,
            _snapshot_id: SnapshotId,
            _start: i64,
            _end: i64,
        ) -> Result<Option<Vec<i64>>, SqliteGraphError> {
            Ok(None)
        }

        fn node_degree(
            &self,
            _snapshot_id: SnapshotId,
            _node: i64,
        ) -> Result<(usize, usize), SqliteGraphError> {
            Ok((0, 0))
        }

        fn k_hop(
            &self,
            _snapshot_id: SnapshotId,
            _start: i64,
            _depth: u32,
            _direction: BackendDirection,
        ) -> Result<Vec<i64>, SqliteGraphError> {
            Ok(vec![])
        }

        fn k_hop_filtered(
            &self,
            _snapshot_id: SnapshotId,
            _start: i64,
            _depth: u32,
            _direction: BackendDirection,
            _allowed_edge_types: &[&str],
        ) -> Result<Vec<i64>, SqliteGraphError> {
            Ok(vec![])
        }

        fn chain_query(
            &self,
            _snapshot_id: SnapshotId,
            _start: i64,
            _chain: &[ChainStep],
        ) -> Result<Vec<i64>, SqliteGraphError> {
            Ok(vec![])
        }

        fn pattern_search(
            &self,
            _snapshot_id: SnapshotId,
            _start: i64,
            _pattern: &PatternQuery,
        ) -> Result<Vec<PatternMatch>, SqliteGraphError> {
            Ok(vec![])
        }

        fn checkpoint(&self) -> Result<(), SqliteGraphError> {
            Ok(())
        }

        fn flush(&self) -> Result<(), SqliteGraphError> {
            Ok(())
        }

        fn backup(&self, _backup_dir: &std::path::Path) -> Result<BackupResult, SqliteGraphError> {
            Err(SqliteGraphError::unsupported(
                "Backup not supported for geometric backend",
            ))
        }

        fn snapshot_export(
            &self,
            _export_dir: &std::path::Path,
        ) -> Result<SnapshotMetadata, SqliteGraphError> {
            Err(SqliteGraphError::unsupported(
                "Snapshot export not supported for geometric backend",
            ))
        }

        fn snapshot_import(
            &self,
            _import_dir: &std::path::Path,
        ) -> Result<ImportMetadata, SqliteGraphError> {
            Err(SqliteGraphError::unsupported(
                "Snapshot import not supported for geometric backend",
            ))
        }

        fn query_nodes_by_kind(
            &self,
            _snapshot_id: SnapshotId,
            _kind: &str,
        ) -> Result<Vec<i64>, SqliteGraphError> {
            Ok(vec![])
        }

        fn query_nodes_by_name_pattern(
            &self,
            _snapshot_id: SnapshotId,
            _pattern: &str,
        ) -> Result<Vec<i64>, SqliteGraphError> {
            Ok(vec![])
        }
    }

    Box::new(GeometricStubBackend)
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_backend_detect_sqlite_header() {
        use std::io::Write;

        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let mut file = std::fs::File::create(temp_file.path()).unwrap();
        file.write_all(b"SQLite format 3\0").unwrap();
        file.sync_all().unwrap();

        let backend = BackendFormat::detect(temp_file.path()).unwrap();
        assert_eq!(
            backend,
            BackendFormat::SQLite,
            "Should detect SQLite format"
        );
    }

    #[test]
    fn test_backend_detect_nonexistent_file() {
        let backend = BackendFormat::detect(Path::new("/nonexistent/path/to/file.db")).unwrap();
        assert_eq!(
            backend,
            BackendFormat::Unknown,
            "Non-existent file should be Unknown"
        );
    }

    #[test]
    fn test_backend_detect_empty_file() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();

        let backend = BackendFormat::detect(temp_file.path()).unwrap();
        assert_eq!(
            backend,
            BackendFormat::Unknown,
            "Empty file should be Unknown"
        );
    }

    #[test]
    fn test_backend_detect_partial_header() {
        use std::io::Write;

        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let mut file = std::fs::File::create(temp_file.path()).unwrap();
        file.write_all(b"SQLite").unwrap();
        file.sync_all().unwrap();

        let backend = BackendFormat::detect(temp_file.path()).unwrap();
        assert_eq!(
            backend,
            BackendFormat::Unknown,
            "Partial header should be Unknown"
        );
    }

    #[test]
    fn test_backend_equality() {
        assert_eq!(BackendFormat::SQLite, BackendFormat::SQLite);
        assert_eq!(BackendFormat::Unknown, BackendFormat::Unknown);

        assert_ne!(BackendFormat::SQLite, BackendFormat::Unknown);
    }
}
