use anyhow::Result;
use std::path::Path;

use super::{CfgBlockData, DocumentInfo, StorageTrait};

#[cfg(feature = "backend-sqlite")]
use super::sqlite_backend::SqliteStorage;

/// Storage backend enum (Phase 069-01)
///
/// This enum wraps SqliteStorage and delegates
/// StorageTrait methods to the appropriate implementation.
///
/// Follows llmgrep's Backend pattern for consistency across tools.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Backend {
    /// SQLite storage backend (traditional, always available)
    #[cfg(feature = "backend-sqlite")]
    Sqlite(SqliteStorage),
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

        #[cfg(not(feature = "backend-sqlite"))]
        {
            let _ = sqlite_detected;
            Err(anyhow::anyhow!("No storage backend feature enabled"))
        }
    }

    /// Check if this is a SQLite backend
    pub fn is_sqlite(&self) -> bool {
        match self {
            #[cfg(feature = "backend-sqlite")]
            Backend::Sqlite(_) => true,
        }
    }

    /// Delegate get_cfg_blocks to inner backend
    pub fn get_cfg_blocks(&self, function_id: i64) -> Result<Vec<CfgBlockData>> {
        match self {
            #[cfg(feature = "backend-sqlite")]
            Backend::Sqlite(s) => s.get_cfg_blocks(function_id),
        }
    }

    /// Delegate get_entity to inner backend
    pub fn get_entity(&self, entity_id: i64) -> Option<sqlitegraph::GraphEntity> {
        match self {
            #[cfg(feature = "backend-sqlite")]
            Backend::Sqlite(s) => s.get_entity(entity_id),
        }
    }

    /// Delegate get_cached_paths to inner backend
    pub fn get_cached_paths(&self, function_id: i64) -> Result<Option<Vec<crate::cfg::Path>>> {
        match self {
            #[cfg(feature = "backend-sqlite")]
            Backend::Sqlite(s) => s.get_cached_paths(function_id),
        }
    }

    /// Delegate get_callees to inner backend
    pub fn get_callees(&self, function_id: i64) -> Result<Vec<i64>> {
        match self {
            #[cfg(feature = "backend-sqlite")]
            Backend::Sqlite(s) => s.get_callees(function_id),
        }
    }

    /// Delegate list_source_documents to inner backend
    pub fn list_source_documents(&self) -> Result<Vec<DocumentInfo>> {
        match self {
            #[cfg(feature = "backend-sqlite")]
            Backend::Sqlite(s) => s.list_source_documents(),
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
