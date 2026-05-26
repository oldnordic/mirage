//! Magellan Geometric backend adapter with contract-aware integration
//!
//! This module provides a small adapter layer for interacting with Magellan's
//! .geo backend according to the updated contract (v3.0.2+). It implements:
//! - Path normalization before queries (GEO_QUERY_CONTRACT §Path Normalization)
//! - Explicit ambiguity error handling (GEO_QUERY_CONTRACT §Ambiguity Handling)
//! - Backend-neutral operations via wrapper types
//!
//! ## Contract Compliance
//!
//! This adapter enforces the following rules from GEO_QUERY_CONTRACT.md:
//!
//! 1. **No Silent First-Match**: Ambiguous lookups return explicit `Ambiguous` variants
//!    with all candidates, never silently picking the first match.
//!
//! 2. **Path Normalization**: All paths are normalized before queries using
//!    Magellan's `normalize_path()` function, ensuring consistent behavior
//!    across platforms and path formats.
//!
//! 3. **Explicit Not Found**: Missing symbols return `NotFound` rather than
//!    panicking or returning empty results.
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use mirage_analyzer::integrations::magellan::{
//!     MagellanAdapter, SymbolLookupResult, normalize_path_for_query,
//! };
//! use magellan::graph::GeometricBackend;
//!
//! # fn main() -> anyhow::Result<()> {
//! // Open the backend
//! let backend = GeometricBackend::open("code.geo")?;
//!
//! // Create adapter
//! let adapter = MagellanAdapter::new(backend);
//!
//! // Look up symbol with proper ambiguity handling
//! match adapter.lookup_symbol_by_name("src/lib.rs", "main") {
//!     SymbolLookupResult::Unique(info) => {
//!         println!("Found: {}", info.fqn.as_ref().unwrap());
//!     }
//!     SymbolLookupResult::Ambiguous { path, name, candidates } => {
//!         eprintln!("Ambiguous: Found {} symbols named '{}' in {}",
//!             candidates.len(), name, path);
//!         for (i, cand) in candidates.iter().enumerate() {
//!             eprintln!("  {}. {}", i + 1, cand.fqn.as_ref().unwrap_or(&cand.name));
//!         }
//!     }
//!     SymbolLookupResult::NotFound => {
//!         eprintln!("Symbol not found");
//!     }
//! }
//! # Ok(())
//! # }
//! ```

pub mod adapter;

pub use adapter::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_for_query() {
        let result = normalize_path_for_query("./src/lib.rs");
        assert!(result.ends_with("src/lib.rs") || result == "src/lib.rs");

        let result = normalize_path_for_query("nonexistent//lib.rs");
        assert!(!result.contains("//"));

        let result = normalize_path_for_query("nonexistent\\lib.rs");
        assert!(!result.contains("\\"));
    }

    #[test]
    fn test_paths_equivalent() {
        assert!(paths_equivalent(
            "./nonexistent/lib.rs",
            "nonexistent/lib.rs"
        ));
        assert!(paths_equivalent(
            "nonexistent//lib.rs",
            "nonexistent/lib.rs"
        ));
    }

    #[test]
    fn test_normalize_path_fallback() {
        let result = normalize_path_for_query("");
        assert!(result.is_empty() || result == "");
    }

    #[test]
    fn test_symbol_lookup_result_counts() {
        let result = SymbolLookupResult::NotFound;
        assert_eq!(result.count(), 0);
        assert!(result.is_not_found());
    }

    #[test]
    fn test_resolve_error_display() {
        let err = ResolveError::NotFound {
            identifier: "foo".to_string(),
            reason: "not found".to_string(),
        };
        assert_eq!(format!("{}", err), "Symbol 'foo' not found: not found");

        let err = ResolveError::Ambiguous {
            identifier: "foo".to_string(),
            candidates: vec![1, 2, 3],
            hint: "use FQN".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Ambiguous reference to 'foo'"));
        assert!(msg.contains("3 candidates"));
    }
}
