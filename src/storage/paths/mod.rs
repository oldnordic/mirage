//! Path caching with BLAKE3 content addressing
//!
//! This module provides database-backed caching for enumerated execution paths.
//! Paths are stored with their BLAKE3 hash as the primary key, providing automatic
//! deduplication and tamper detection. Cache entries are invalidated when function
//! content changes (detected via function_hash comparison).

mod storage;

pub use storage::{
    get_cached_paths, invalidate_function_paths, store_paths, store_paths_batch,
    update_function_paths_if_changed,
};

#[cfg(test)]
use storage::{path_kind_to_str, str_to_path_kind};

/// Path cache manager (placeholder for future cache management features)
///
/// This struct is a placeholder for future cache management functionality
/// such as cache statistics, manual invalidation controls, and cache warming.
#[derive(Debug, Clone)]
pub struct PathCache {
    _private: (),
}

impl PathCache {
    /// Create a new path cache manager
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for PathCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
