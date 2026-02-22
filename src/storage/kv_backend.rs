//! Native-V3 KV backend implementation for mirage storage trait
//!
//! Uses V3Backend's native KV store for CFG data stored as JSON.
//!
//! # Design
//!
//! - Implements `StorageTrait` for native-v3 databases
//! - KV key format: `cfg:func:{function_id}`
//! - Provides backend-agnostic `CfgBlockData` from KV store
//!
//! # Examples
//!
//! ```ignore
//! # use mirage_analyzer::storage::kv_backend::KvStorage;
//! # use anyhow::Result;
//! # fn main() -> Result<()> {
//! let storage = KvStorage::open("/path/to/codegraph.db")?;
//! let blocks = storage.get_cfg_blocks(123)?;
//! # Ok(())
//! # }
//! ```
//!
//! Note: V3Backend is used directly rather than the GraphBackend trait
//! to access V3's native KV store capabilities.

use anyhow::Result;
use std::path::Path;

use sqlitegraph::backend::native::v3::V3Backend;
use sqlitegraph::backend::native::v3::kv_store::types::KvValue;

use super::{CfgBlockData, StorageTrait};

/// KV key format for CFG blocks: cfg:func:{function_id}
fn cfg_key(function_id: i64) -> String {
    format!("cfg:func:{}", function_id)
}

/// Native-V3 KV backend implementation
///
/// Wraps a V3Backend and implements StorageTrait
/// using V3's native KV store for CFG data.
pub struct KvStorage {
    /// V3 backend instance
    backend: V3Backend,
}

impl std::fmt::Debug for KvStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KvStorage")
            .field("backend", &"<V3Backend>")
            .finish()
    }
}

impl KvStorage {
    /// Open native-v3 database at the given path
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to the native-v3 database file
    ///
    /// # Returns
    ///
    /// * `Ok(KvStorage)` - Storage instance ready for queries
    /// * `Err(...)` - Error if file cannot be opened
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use mirage_analyzer::storage::kv_backend::KvStorage;
    /// # fn main() -> anyhow::Result<()> {
    /// let storage = KvStorage::open("codegraph.db")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn open(db_path: &Path) -> Result<Self> {
        let backend = V3Backend::open(db_path)
            .map_err(|e| anyhow::anyhow!("Failed to open native-v3 database: {}", e))?;
        Ok(Self { backend })
    }

    /// Get a reference to the underlying V3Backend
    ///
    /// This is useful for queries beyond the StorageTrait API.
    pub fn backend(&self) -> &V3Backend {
        &self.backend
    }
}

impl StorageTrait for KvStorage {
    /// Get CFG blocks for a function from KV backend
    ///
    /// Uses the KV store with key format `cfg:func:{function_id}` to load
    /// CFG blocks as JSON.
    ///
    /// # Arguments
    ///
    /// * `function_id` - ID of the function in graph_entities
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<CfgBlockData>)` - Vector of CFG block data
    /// * `Err(...)` - Error if query fails
    ///
    /// # Note
    ///
    /// - Returns empty Vec if function has no CFG blocks (not an error)
    fn get_cfg_blocks(&self, function_id: i64) -> Result<Vec<CfgBlockData>> {
        // Use kv_get_v3 to retrieve CFG blocks from KV store
        let key = cfg_key(function_id);
        
        match self.backend.kv_get_v3(sqlitegraph::SnapshotId::current(), key.as_bytes()) {
            Some(kv_value) => {
                // Parse JSON from KvValue
                let json_str: String = match kv_value {
                    KvValue::Json(json) => json.to_string(),
                    KvValue::String(s) => s,
                    KvValue::Bytes(b) => {
                        String::from_utf8_lossy(&b).to_string()
                    }
                    _ => {
                        return Err(anyhow::anyhow!(
                            "CFG data for function {} is not in JSON format", function_id
                        ));
                    }
                };
                
                // Deserialize JSON to Vec<CfgBlockData>
                match serde_json::from_str::<Vec<CfgBlockData>>(&json_str) {
                    Ok(blocks) => Ok(blocks),
                    Err(e) => {
                        Err(anyhow::anyhow!(
                            "Failed to parse CFG data for function {}: {}", function_id, e
                        ))
                    }
                }
            }
            None => {
                // No CFG blocks stored for this function
                Ok(Vec::new())
            }
        }
    }

    /// Get entity by ID from KV backend
    ///
    /// Uses V3Backend's get_node to query entity data.
    ///
    /// # Arguments
    ///
    /// * `entity_id` - ID of the entity
    ///
    /// # Returns
    ///
    /// * `Some(GraphEntity)` - Entity if found
    /// * `None` - Entity not found
    fn get_entity(&self, entity_id: i64) -> Option<sqlitegraph::GraphEntity> {
        // V3Backend implements GraphBackend, so we can use get_node with a snapshot
        use sqlitegraph::GraphBackend;
        self.backend.get_node(sqlitegraph::SnapshotId::current(), entity_id).ok()
    }

    /// Get cached paths for a function from KV backend
    ///
    /// Uses KV store for path caching with key format `cfg:paths:{function_id}`.
    ///
    /// # Arguments
    ///
    /// * `function_id` - ID of the function
    ///
    /// # Returns
    ///
    /// * `Ok(Some(paths))` - Cached paths if available
    /// * `Ok(None)` - No cached paths
    /// * `Err(...)` - Error if query fails
    ///
    /// # Note
    ///
    /// This is a placeholder for future path caching implementation.
    /// Currently returns Ok(None) for all functions.
    fn get_cached_paths(&self, _function_id: i64) -> Result<Option<Vec<crate::cfg::Path>>> {
        // TODO: Implement path caching in Phase 071 (Mirage Advanced Commands)
        // Use KV store with key format: cfg:paths:{function_id}
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests require a native-v3 database file
    // which is complex to set up in unit tests. For now, we test
    // the API surface and provide compile-time verification.

    #[test]
    fn test_kv_storage_trait_bounds() {
        // This test verifies that KvStorage implements StorageTrait
        // at compile time. If it compiles, the trait is implemented.
        fn assert_storage_trait<T: StorageTrait>(_t: &T) {}
        let _ = assert_storage_trait::<KvStorage>;
    }

    #[test]
    fn test_cfg_block_data_fields() {
        // Verify CfgBlockData has the expected fields
        let block = CfgBlockData {
            id: 0,
            kind: "entry".to_string(),
            terminator: "fallthrough".to_string(),
            byte_start: 0,
            byte_end: 10,
            start_line: 1,
            start_col: 0,
            end_line: 1,
            end_col: 10,
        };

        assert_eq!(block.id, 0);
        assert_eq!(block.kind, "entry");
        assert_eq!(block.terminator, "fallthrough");
        assert_eq!(block.byte_start, 0);
        assert_eq!(block.byte_end, 10);
    }
}
