//! Geometric backend storage implementation for Mirage
//!
//! This module provides integration with Magellan's GeometricBackend for .geo database files.
//! It wraps the geometric backend and implements the StorageTrait for CFG analysis.

#![cfg(feature = "backend-geometric")]

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use magellan::graph::geometric_backend::{
    CfgBlock as MagellanCfgBlock, GeometricBackend as MagellanGeometricBackend, SymbolInfo,
};

use crate::storage::{CfgBlockData, StorageTrait};

/// Geometric storage backend for Mirage
///
/// Wraps Magellan's GeometricBackend and provides Mirage-specific
/// storage operations for CFG analysis.
pub struct GeometricStorage {
    inner: MagellanGeometricBackend,
    db_path: PathBuf,
}

impl std::fmt::Debug for GeometricStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeometricStorage")
            .field("db_path", &self.db_path)
            .finish_non_exhaustive()
    }
}

impl GeometricStorage {
    /// Open a geometric database file
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to the .geo database file
    ///
    /// # Returns
    ///
    /// * `Ok(GeometricStorage)` - Opened storage backend
    /// * `Err(...)` - Error if file doesn't exist or isn't a valid geometric database
    pub fn open(db_path: &Path) -> Result<Self> {
        // Validate file exists
        if !db_path.exists() {
            anyhow::bail!("Database not found: {}", db_path.display());
        }

        // Validate .geo extension
        match db_path.extension().and_then(|e| e.to_str()) {
            Some("geo") => {}
            _ => {
                anyhow::bail!("File does not have .geo extension: {}", db_path.display());
            }
        }

        // Open Magellan geometric backend
        let inner = MagellanGeometricBackend::open(db_path)
            .map_err(|e| anyhow::anyhow!("Failed to open geometric backend: {}", e))?;

        Ok(Self {
            inner,
            db_path: db_path.to_path_buf(),
        })
    }

    /// Create a new geometric database file
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path where the .geo database will be created
    ///
    /// # Returns
    ///
    /// * `Ok(GeometricStorage)` - Created storage backend
    /// * `Err(...)` - Error if creation fails
    #[allow(dead_code)]
    pub fn create(db_path: &Path) -> Result<Self> {
        let inner = MagellanGeometricBackend::create(db_path)
            .map_err(|e| anyhow::anyhow!("Failed to create geometric backend: {}", e))?;

        Ok(Self {
            inner,
            db_path: db_path.to_path_buf(),
        })
    }

    /// Get a reference to the inner Magellan geometric backend
    pub fn inner(&self) -> &MagellanGeometricBackend {
        &self.inner
    }

    /// Find symbols by name and return detailed info
    ///
    /// This is a convenience method that wraps the Magellan backend's
    /// find_symbols_by_name_info method.
    pub fn find_symbols_by_name(&self, name: &str) -> Vec<SymbolInfo> {
        self.inner.find_symbols_by_name_info(name)
    }

    /// Find a symbol by its fully qualified name
    pub fn find_symbol_by_fqn(&self, fqn: &str) -> Option<SymbolInfo> {
        self.inner.find_symbol_by_fqn_info(fqn)
    }

    /// Find a symbol ID by name and file path
    ///
    /// Returns Some(id) if a unique match is found, None if not found or ambiguous.
    pub fn find_symbol_id_by_name_and_path(&self, name: &str, path: &str) -> Option<u64> {
        self.inner.find_symbol_id_by_name_and_path(name, path)
    }

    /// Get CFG blocks for a function directly from the backend
    pub fn get_cfg_blocks_for_function(&self, function_id: i64) -> Vec<MagellanCfgBlock> {
        self.inner.get_cfg_blocks_for_function(function_id)
    }

    /// Get all CFG edges from the backend
    pub fn get_all_edges(&self) -> Vec<geographdb_core::storage::EdgeRec> {
        self.inner.get_all_edges()
    }

    /// Complete FQN prefix for autocomplete functionality
    pub fn complete_fqn_prefix(&self, prefix: &str, limit: usize) -> Vec<String> {
        self.inner.complete_fqn_prefix(prefix, limit)
    }

    /// Get database statistics
    pub fn get_stats(&self) -> Result<GeometricStats> {
        // Reload from disk to ensure we have the latest persisted data
        // This is necessary because the geometric backend caches data in memory
        // and another process (like magellan watch) may have written new data
        self.inner.reload_from_disk()?;

        // Now get fresh stats from the reloaded cache
        let stats = self.inner.get_geometric_stats();

        Ok(GeometricStats {
            symbol_count: stats.symbol_count as usize,
            cfg_block_count: stats.cfg_block_count as usize,
        })
    }
}

/// Statistics for the GeometricDB backend
#[derive(Debug, Clone)]
pub struct GeometricStats {
    pub symbol_count: usize,
    pub cfg_block_count: usize,
}

impl StorageTrait for GeometricStorage {
    fn get_cfg_blocks(&self, function_id: i64) -> Result<Vec<CfgBlockData>> {
        let blocks = self.inner.get_cfg_blocks_for_function(function_id);

        tracing::debug!(
            "get_cfg_blocks for function_id {}: found {} blocks",
            function_id,
            blocks.len()
        );

        // Convert geographdb_core::CfgBlock to Mirage's CfgBlockData
        Ok(blocks
            .into_iter()
            .map(|block| {
                CfgBlockData {
                    id: block.id as i64,
                    kind: block.block_kind,
                    terminator: block.terminator,
                    byte_start: block.byte_start,
                    byte_end: block.byte_end,
                    start_line: block.start_line,
                    start_col: block.start_col,
                    end_line: block.end_line,
                    end_col: block.end_col,
                    // Map geometric backend fields to 4D spatial coordinates
                    // X = dominator_depth, Y = loop_nesting, Z = branch_count
                    coord_x: block.dominator_depth as i64,
                    coord_y: block.loop_nesting as i64,
                    coord_z: block.branch_count as i64,
                }
            })
            .collect())
    }

    fn get_entity(&self, entity_id: i64) -> Option<sqlitegraph::GraphEntity> {
        // Geometric backend uses different entity storage
        // Try to find the symbol by ID
        if let Some(info) = self.inner.find_symbol_by_id_info(entity_id as u64) {
            // Convert SymbolInfo to GraphEntity
            // Note: GraphEntity only has id, kind, name, file_path, data fields
            // Additional info goes into the data field as JSON
            let data = serde_json::json!({
                "fqn": info.fqn,
                "byte_start": info.byte_start,
                "byte_end": info.byte_end,
                "start_line": info.start_line,
                "start_col": info.start_col,
                "end_line": info.end_line,
                "end_col": info.end_col,
            });
            Some(sqlitegraph::GraphEntity {
                id: entity_id,
                name: info.name,
                kind: format!("{:?}", info.kind),
                file_path: Some(info.file_path),
                data,
            })
        } else {
            None
        }
    }

    fn get_cached_paths(&self, _function_id: i64) -> Result<Option<Vec<crate::cfg::Path>>> {
        // Path caching not implemented for Geometric backend yet
        Ok(None)
    }
}

/// Convert a terminator string to the Terminator enum
fn convert_terminator_to_enum(terminator: &str) -> crate::cfg::Terminator {
    match terminator {
        "Return" => crate::cfg::Terminator::Return,
        "Unreachable" => crate::cfg::Terminator::Unreachable,
        "Fallthrough" => crate::cfg::Terminator::Goto { target: 0 }, // Placeholder
        "Conditional" => crate::cfg::Terminator::SwitchInt {
            targets: vec![],
            otherwise: 0,
        },
        _ => crate::cfg::Terminator::Abort(terminator.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_geo_db() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let geo_path = temp_dir.path().join("test.geo");

        // Create valid geometric database
        let _backend = MagellanGeometricBackend::create(&geo_path)
            .expect("Failed to create test geo database");

        (temp_dir, geo_path)
    }

    #[test]
    fn test_geometric_storage_open_valid_file() {
        let (_temp_dir, geo_path) = create_test_geo_db();
        let result = GeometricStorage::open(&geo_path);
        assert!(result.is_ok(), "Should open valid .geo file");
    }

    #[test]
    fn test_geometric_storage_open_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let geo_path = temp_dir.path().join("nonexistent.geo");

        let result = GeometricStorage::open(&geo_path);
        assert!(result.is_err(), "Should fail for nonexistent file");
    }

    #[test]
    fn test_geometric_storage_open_wrong_extension() {
        let temp_dir = TempDir::new().unwrap();
        let wrong_path = temp_dir.path().join("test.db");
        std::fs::write(&wrong_path, b"dummy").unwrap();

        let result = GeometricStorage::open(&wrong_path);
        assert!(result.is_err(), "Should fail for non-.geo file");
    }

    #[test]
    fn test_geometric_storage_create_and_open() {
        let temp_dir = TempDir::new().unwrap();
        let geo_path = temp_dir.path().join("new.geo");

        // Create
        let result = GeometricStorage::create(&geo_path);
        assert!(result.is_ok(), "Should create new .geo file");

        // Open
        let result = GeometricStorage::open(&geo_path);
        assert!(result.is_ok(), "Should open created .geo file");
    }

    #[test]
    fn test_geometric_storage_find_symbols_empty() {
        let (_temp_dir, geo_path) = create_test_geo_db();
        let storage = GeometricStorage::open(&geo_path).unwrap();

        let symbols = storage.find_symbols_by_name("nonexistent");
        assert!(
            symbols.is_empty(),
            "Should return empty for nonexistent symbol"
        );
    }

    #[test]
    fn test_geometric_storage_complete_prefix_empty() {
        let (_temp_dir, geo_path) = create_test_geo_db();
        let storage = GeometricStorage::open(&geo_path).unwrap();

        let completions = storage.complete_fqn_prefix("test", 10);
        assert!(
            completions.is_empty(),
            "Should return empty for empty database"
        );
    }
}
