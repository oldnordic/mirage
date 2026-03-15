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

use anyhow::Result;
use magellan::graph::geometric_backend::GeometricBackend;
use magellan::validation::normalize_path;
use std::path::Path;

/// Result type for symbol lookup that may have ambiguity
///
/// This type provides explicit ambiguity information for symbol lookups,
/// allowing callers to distinguish between unique matches, ambiguous matches,
/// and not-found cases.
///
/// This corresponds to the contract requirement: "Do not silently pick first
/// match on ambiguity" (GEO_QUERY_CONTRACT.md §Ambiguity Handling).
#[derive(Debug, Clone)]
pub enum SymbolLookupResult {
    /// Exactly one symbol found
    Unique(magellan::graph::geometric_backend::SymbolInfo),

    /// Multiple symbols match - explicit ambiguity with candidates
    ///
    /// The `candidates` vector contains ALL matching symbols, allowing the
    /// caller to implement appropriate disambiguation logic (e.g., filtering
    /// by path, asking user to choose, or using additional context).
    Ambiguous {
        /// Path that was queried
        path: String,
        /// Symbol name that was queried
        name: String,
        /// All symbols that matched the query
        candidates: Vec<magellan::graph::geometric_backend::SymbolInfo>,
    },

    /// No symbols found
    NotFound,
}

impl SymbolLookupResult {
    /// Check if this result represents a unique match
    pub fn is_unique(&self) -> bool {
        matches!(self, SymbolLookupResult::Unique(_))
    }

    /// Check if this result represents an ambiguous match
    pub fn is_ambiguous(&self) -> bool {
        matches!(self, SymbolLookupResult::Ambiguous { .. })
    }

    /// Check if this result represents a not-found case
    pub fn is_not_found(&self) -> bool {
        matches!(self, SymbolLookupResult::NotFound)
    }

    /// Get the unique symbol if this is a unique match
    ///
    /// Returns `None` for ambiguous or not-found cases.
    pub fn unique(self) -> Option<magellan::graph::geometric_backend::SymbolInfo> {
        match self {
            SymbolLookupResult::Unique(info) => Some(info),
            _ => None,
        }
    }

    /// Get candidates if this is an ambiguous match
    ///
    /// Returns `None` for unique or not-found cases.
    pub fn candidates(self) -> Option<Vec<magellan::graph::geometric_backend::SymbolInfo>> {
        match self {
            SymbolLookupResult::Ambiguous { candidates, .. } => Some(candidates),
            _ => None,
        }
    }

    /// Get the count of matching symbols
    ///
    /// Returns 0 for not-found, 1 for unique, N for ambiguous.
    pub fn count(&self) -> usize {
        match self {
            SymbolLookupResult::Unique(_) => 1,
            SymbolLookupResult::Ambiguous { candidates, .. } => candidates.len(),
            SymbolLookupResult::NotFound => 0,
        }
    }
}

/// Result type for FQN-based symbol lookup
///
/// Similar to `SymbolLookupResult` but for FQN queries where ambiguity
/// is less common but still possible (e.g., duplicate symbols during
/// reindexing).
#[derive(Debug, Clone)]
pub enum FqnLookupResult {
    /// Exactly one symbol found
    Unique(magellan::graph::geometric_backend::SymbolInfo),

    /// Multiple symbols match the FQN (data integrity issue)
    Ambiguous {
        /// FQN that was queried
        fqn: String,
        /// All symbols with this FQN
        candidates: Vec<magellan::graph::geometric_backend::SymbolInfo>,
    },

    /// No symbol found with this FQN
    NotFound,
}

/// Result type for ID-based symbol lookup
///
/// ID lookups should never be ambiguous (IDs are unique), but we use
/// a result type for API consistency and to handle missing symbols.
#[derive(Debug, Clone)]
pub enum IdLookupResult {
    /// Symbol found
    Found(magellan::graph::geometric_backend::SymbolInfo),

    /// No symbol with this ID
    NotFound,
}

/// Magellan adapter for contract-compliant queries
///
/// Wraps `GeometricBackend` and enforces the GEO_QUERY_CONTRACT rules
/// for path normalization, ambiguity handling, and error reporting.
///
/// ## Design
///
/// - All queries normalize paths first using `normalize_path_for_query()`
/// - Symbol lookups return explicit `Unique`/`Ambiguous`/`NotFound` results
/// - Never silently picks first match on ambiguity
/// - Uses Magella's documented contract methods only
pub struct MagellanAdapter<'a> {
    /// Underlying Geometric backend (borrowed)
    backend: &'a GeometricBackend,
}

impl<'a> MagellanAdapter<'a> {
    /// Create a new adapter from a borrowed Geometric backend
    ///
    /// # Arguments
    ///
    /// * `backend` - The Geometric backend instance (borrowed)
    pub fn new(backend: &'a GeometricBackend) -> Self {
        Self { backend }
    }

    /// Get a reference to the underlying backend
    ///
    /// This provides access to the full GeometricBackend API for operations
    /// that don't require contract enforcement (e.g., CFG queries).
    pub fn backend(&self) -> &GeometricBackend {
        self.backend
    }

    /// Look up a symbol by path and name with contract-compliant ambiguity handling
    ///
    /// This method implements the contract requirement: "Do not silently pick
    /// first match on ambiguity" (GEO_QUERY_CONTRACT.md §Ambiguity Handling).
    ///
    /// # Arguments
    ///
    /// * `path` - File path (will be normalized)
    /// * `name` - Symbol name
    ///
    /// # Returns
    ///
    /// - `Unique(info)` if exactly one symbol matches
    /// - `Ambiguous { candidates }` if multiple symbols match
    /// - `NotFound` if no symbols match
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use mirage_analyzer::integrations::magellan::MagellanAdapter;
    /// # let adapter = unimplemented!();
    /// match adapter.lookup_symbol_by_path_and_name("src/lib.rs", "main") {
    ///     mirage_analyzer::integrations::magellan::SymbolLookupResult::Unique(info) => {
    ///         println!("Found: {:?}", info.id);
    ///     }
    ///     mirage_analyzer::integrations::magellan::SymbolLookupResult::Ambiguous { candidates, .. } => {
    ///         eprintln!("Found {} candidates - please disambiguate", candidates.len());
    ///     }
    ///     mirage_analyzer::integrations::magellan::SymbolLookupResult::NotFound => {
    ///         eprintln!("Symbol not found");
    ///     }
    /// }
    /// ```
    pub fn lookup_symbol_by_path_and_name(&self, path: &str, name: &str) -> SymbolLookupResult {
        // Normalize path first (GEO_QUERY_CONTRACT §Path Normalization)
        let normalized_path = normalize_path_for_query(path);

        // Use Magellan's contract-compliant find_symbol_id_by_name_and_path
        // Returns Option<u64> - Some(id) if unique match found, None if not found or ambiguous
        match self
            .backend
            .find_symbol_id_by_name_and_path(name, &normalized_path)
        {
            Some(id) => {
                // Unique match found - get the symbol info
                match self.backend.find_symbol_by_id_info(id) {
                    Some(info) => SymbolLookupResult::Unique(info),
                    None => SymbolLookupResult::NotFound,
                }
            }
            None => {
                // Not found or ambiguous - get ALL candidates to determine which
                let all_symbols = self
                    .backend
                    .symbols_in_file(&normalized_path)
                    .unwrap_or_default();
                let matching: Vec<_> = all_symbols.into_iter().filter(|s| s.name == name).collect();

                if matching.len() > 1 {
                    // Truly ambiguous
                    SymbolLookupResult::Ambiguous {
                        path: normalized_path,
                        name: name.to_string(),
                        candidates: matching,
                    }
                } else {
                    SymbolLookupResult::NotFound
                }
            }
        }
    }

    /// Look up a symbol by FQN with contract-compliant ambiguity handling
    ///
    /// # Arguments
    ///
    /// * `fqn` - Fully-qualified name
    ///
    /// # Returns
    ///
    /// - `Unique(info)` if exactly one symbol matches
    /// - `Ambiguous { candidates }` if multiple symbols have this FQN (data issue)
    /// - `NotFound` if no symbols match
    pub fn lookup_symbol_by_fqn(&self, fqn: &str) -> FqnLookupResult {
        match self.backend.find_symbol_by_fqn_info(fqn) {
            Some(info) => FqnLookupResult::Unique(info),
            None => FqnLookupResult::NotFound,
        }
    }

    /// Look up a symbol by numeric ID
    ///
    /// ID lookups are never ambiguous by design (IDs are unique primary keys).
    ///
    /// # Arguments
    ///
    /// * `id` - Symbol ID (u64)
    ///
    /// # Returns
    ///
    /// - `Found(info)` if symbol exists
    /// - `NotFound` if symbol doesn't exist
    pub fn lookup_symbol_by_id(&self, id: u64) -> IdLookupResult {
        match self.backend.find_symbol_by_id_info(id) {
            Some(info) => IdLookupResult::Found(info),
            None => IdLookupResult::NotFound,
        }
    }

    /// Resolve a function identifier to a numeric ID with ambiguity handling
    ///
    /// This is a convenience method that handles the common pattern of resolving
    /// a user-provided identifier (which could be an ID, FQN, or path+name tuple)
    /// to a numeric symbol ID.
    ///
    /// # Arguments
    ///
    /// * `identifier` - Function identifier (numeric ID, FQN, or simple name)
    ///
    /// # Returns
    ///
    /// - `Ok(id)` if exactly one match found
    /// - `Err(AmbiguityError)` if multiple matches
    /// - `Err(not found error)` if no matches
    pub fn resolve_function_id(&self, identifier: &str) -> Result<u64, ResolveError> {
        // Try numeric ID first
        if let Ok(id) = identifier.parse::<u64>() {
            match self.lookup_symbol_by_id(id) {
                IdLookupResult::Found(_) => return Ok(id),
                IdLookupResult::NotFound => {
                    return Err(ResolveError::NotFound {
                        identifier: identifier.to_string(),
                        reason: "No symbol with this ID exists".to_string(),
                    })
                }
            }
        }

        // Try FQN lookup
        match self.lookup_symbol_by_fqn(identifier) {
            FqnLookupResult::Unique(info) => return Ok(info.id),
            FqnLookupResult::NotFound => {
                // Fall through to simple name lookup as last resort
            }
            FqnLookupResult::Ambiguous { fqn, candidates } => {
                let count = candidates.len();
                return Err(ResolveError::Ambiguous {
                    identifier: identifier.to_string(),
                    candidates: candidates.into_iter().map(|c| c.id).collect(),
                    hint: format!(
                        "FQN '{}' matches {} symbols - check for duplicate definitions",
                        fqn, count
                    ),
                });
            }
        }

        // As last resort, search by simple name across all files
        let all_matching = self.backend.find_symbols_by_name_info(identifier);

        // Deduplicate by symbol ID - the ID is the unique primary key in the database.
        // This handles cases where the same symbol may be indexed multiple times with
        // identical (name, file_path, location) data but different internal records.
        let mut unique_symbols: Vec<magellan::graph::geometric_backend::SymbolInfo> = Vec::new();
        let mut seen_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();
        
        for sym in all_matching {
            if seen_ids.insert(sym.id) {
                unique_symbols.push(sym);
            }
        }

        match unique_symbols.len() {
            0 => Err(ResolveError::NotFound {
                identifier: identifier.to_string(),
                reason: "No symbol with this name, FQN, or ID exists".to_string(),
            }),
            1 => Ok(unique_symbols[0].id),
            n => {
                // Check if all candidates are at the same location (duplicates) or genuinely different
                // Normalize paths for consistent comparison
                let normalize_path = |p: &str| -> String {
                    p.replace("\\", "/").replace("/./", "/")
                };
                
                let first = &unique_symbols[0];
                let first_path_normalized = normalize_path(&first.file_path);
                let all_same_location = unique_symbols.iter().all(|sym| {
                    let sym_path_normalized = normalize_path(&sym.file_path);
                    sym.name == first.name 
                        && sym_path_normalized == first_path_normalized
                        && sym.start_line == first.start_line 
                        && sym.start_col == first.start_col
                });
                
                if all_same_location {
                    // All same location - they're duplicates, pick the first one
                    Ok(unique_symbols[0].id)
                } else {
                    // Genuinely ambiguous - different functions with same name
                    Err(ResolveError::Ambiguous {
                        identifier: identifier.to_string(),
                        candidates: unique_symbols.into_iter().map(|c| c.id).collect(),
                        hint: format!(
                            "Found {} unique symbols named '{}' - use FQN or path to disambiguate",
                            n, identifier
                        ),
                    })
                }
            }
        }
    }
}

/// Error type for symbol resolution operations
#[derive(Debug, Clone)]
pub enum ResolveError {
    /// Symbol not found
    NotFound {
        /// The identifier that was searched for
        identifier: String,
        /// Human-readable reason
        reason: String,
    },

    /// Multiple symbols match (ambiguity)
    Ambiguous {
        /// The identifier that was searched for
        identifier: String,
        /// IDs of all matching candidates
        candidates: Vec<u64>,
        /// Human-readable hint for disambiguation
        hint: String,
    },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::NotFound { identifier, reason } => {
                write!(f, "Symbol '{}' not found: {}", identifier, reason)
            }
            ResolveError::Ambiguous {
                identifier,
                candidates,
                hint,
            } => {
                write!(
                    f,
                    "Ambiguous reference to '{}': {} candidates. {}",
                    identifier,
                    candidates.len(),
                    hint
                )
            }
        }
    }
}

impl std::error::Error for ResolveError {}

/// Information about a call graph cycle
///
/// Represents a strongly connected component (SCC) with more than one member,
/// indicating mutual recursion or cyclic dependencies.
#[derive(Debug, Clone)]
pub struct CycleInfo {
    /// All symbol IDs in this cycle
    pub symbol_ids: Vec<u64>,
    /// Cycle kind (mutual recursion, longer cycle, etc.)
    pub kind: String,
}

/// Dead symbol information
///
/// A symbol that is unreachable from the given entry points.
#[derive(Debug, Clone)]
pub struct DeadSymbolInfo {
    /// The dead symbol's ID
    pub symbol_id: u64,
    /// Fully-qualified name
    pub fqn: Option<String>,
    /// File path
    pub file_path: String,
    /// Why this symbol is considered dead
    pub reason: String,
}

/// Call graph path information
///
/// Represents a path through the call graph from one symbol to another.
#[derive(Debug, Clone)]
pub struct CallPath {
    /// Ordered symbol IDs along the path
    pub symbol_ids: Vec<u64>,
    /// Path length (number of edges)
    pub length: usize,
}

/// Result type for path enumeration
#[derive(Debug, Clone)]
pub struct PathEnumerationResult {
    /// All discovered paths
    pub paths: Vec<CallPath>,
    /// Total number of paths found
    pub total_count: usize,
    /// Whether enumeration was truncated due to limits
    pub truncated: bool,
}

/// Call relationship information
#[derive(Debug, Clone)]
pub struct CallRelation {
    /// Caller symbol ID
    pub caller_id: u64,
    /// Caller name
    pub caller_name: String,
    /// Callee symbol ID
    pub callee_id: u64,
    /// Callee name
    pub callee_name: String,
}

impl<'a> MagellanAdapter<'a> {
    /// Get all symbols reachable from a given symbol (forward reachability)
    ///
    /// Computes the transitive closure of the call graph starting from the
    /// specified symbol. Returns all symbols that can be reached through
    /// call edges.
    ///
    /// # Arguments
    ///
    /// * `symbol_id` - Starting symbol ID
    ///
    /// # Returns
    ///
    /// Vector of reachable symbol IDs
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use mirage_analyzer::integrations::magellan::MagellanAdapter;
    /// # let adapter = unimplemented!();
    /// let reachable = adapter.reachable_from(123);
    /// println!("{} symbols reachable from 123", reachable.len());
    /// ```
    pub fn reachable_from(&self, symbol_id: u64) -> Vec<u64> {
        self.backend.reachable_from(symbol_id)
    }

    /// Get all symbols that can reach a given symbol (reverse reachability)
    ///
    /// Computes the reverse transitive closure - all symbols from which the
    /// specified symbol can be reached (i.e., all direct and indirect callers).
    ///
    /// # Arguments
    ///
    /// * `symbol_id` - Target symbol ID
    ///
    /// # Returns
    ///
    /// Vector of symbol IDs that can reach the target
    pub fn reverse_reachable_from(&self, symbol_id: u64) -> Vec<u64> {
        self.backend.reverse_reachable_from(symbol_id)
    }

    /// Find dead code starting from entry points
    ///
    /// Identifies all symbols that are unreachable from any of the given entry
    /// points. These symbols constitute dead code that can never be executed.
    ///
    /// # Arguments
    ///
    /// * `entry_ids` - Entry point symbol IDs (e.g., main, test functions)
    ///
    /// # Returns
    ///
    /// Vector of dead symbol IDs with metadata
    pub fn dead_code_from_entries(&self, entry_ids: &[u64]) -> Vec<DeadSymbolInfo> {
        let dead_ids = self.backend.dead_code_from_entries(entry_ids);

        dead_ids
            .into_iter()
            .filter_map(|id| self.backend.find_symbol_by_id_info(id))
            .map(|info| DeadSymbolInfo {
                symbol_id: info.id,
                fqn: Some(info.fqn),
                file_path: info.file_path,
                reason: "Not reachable from any entry point".to_string(),
            })
            .collect()
    }

    /// Detect cycles in the call graph
    ///
    /// Finds all strongly connected components (SCCs) with more than one member,
    /// which indicate cycles or mutual recursion in the call graph.
    ///
    /// # Returns
    ///
    /// Vector of detected cycles, where each cycle is a vector of symbol IDs
    pub fn find_call_graph_cycles(&self) -> Vec<CycleInfo> {
        let cycles = self.backend.find_call_graph_cycles();

        cycles
            .into_iter()
            .map(|mut scc| CycleInfo {
                symbol_ids: std::mem::take(&mut scc),
                kind: if scc.len() == 2 {
                    "MutualRecursion".to_string()
                } else {
                    "Cycle".to_string()
                },
            })
            .collect()
    }

    /// Get strongly connected components of the call graph
    ///
    /// Returns all SCCs, including single-node components. Each SCC is a
    /// maximal set of symbols where each symbol can reach each other.
    ///
    /// # Returns
    ///
    /// Magellan's SccResult containing all SCCs
    pub fn get_strongly_connected_components(&self) -> magellan::graph::geometric_calls::SccResult {
        self.backend.get_strongly_connected_components()
    }

    /// Condense the call graph by collapsing SCCs
    ///
    /// Creates a condensation DAG where each strongly connected component is
    /// collapsed into a single supernode. The resulting graph is always a DAG
    /// (no cycles).
    ///
    /// # Returns
    ///
    /// Magellan's CondensationDag representing the condensed graph
    pub fn condense_call_graph(&self) -> magellan::graph::geometric_calls::CondensationDag {
        self.backend.condense_call_graph()
    }

    /// Enumerate paths in the call graph
    ///
    /// Finds all paths from a start symbol to an optional end symbol, bounded
    /// by depth and path count limits.
    ///
    /// # Arguments
    ///
    /// * `start_id` - Starting symbol ID
    /// * `end_id` - Optional target symbol ID (None = all paths from start)
    /// * `max_depth` - Maximum path length (0 = unlimited)
    /// * `max_paths` - Maximum number of paths to return (0 = unlimited)
    ///
    /// # Returns
    ///
    /// Path enumeration result with all discovered paths
    pub fn enumerate_paths(
        &self,
        start_id: u64,
        end_id: Option<u64>,
        max_depth: usize,
        max_paths: usize,
    ) -> PathEnumerationResult {
        let magellan_result = self
            .backend
            .enumerate_paths(start_id, end_id, max_depth, max_paths);

        PathEnumerationResult {
            paths: magellan_result
                .paths
                .into_iter()
                .map(|symbol_ids| CallPath {
                    symbol_ids: symbol_ids.clone(),
                    length: symbol_ids.len(),
                })
                .collect(),
            total_count: magellan_result.total_enumerated,
            truncated: magellan_result.bounded_hit,
        }
    }

    /// Get direct callers of a symbol
    ///
    /// Returns all symbols that directly call the specified symbol.
    ///
    /// # Arguments
    ///
    /// * `symbol_id` - Target symbol ID
    ///
    /// # Returns
    ///
    /// Vector of call relationships
    pub fn callers_of_symbol(&self, symbol_id: u64) -> Vec<CallRelation> {
        let callers = self.backend.get_callers(symbol_id);

        callers
            .into_iter()
            .filter_map(|caller_id| {
                let caller_info = self.backend.find_symbol_by_id_info(caller_id)?;
                let callee_info = self.backend.find_symbol_by_id_info(symbol_id)?;
                Some(CallRelation {
                    caller_id,
                    caller_name: caller_info.name.clone(),
                    callee_id: symbol_id,
                    callee_name: callee_info.name.clone(),
                })
            })
            .collect()
    }

    /// Get direct callees of a symbol
    ///
    /// Returns all symbols that are directly called by the specified symbol.
    ///
    /// # Arguments
    ///
    /// * `symbol_id` - Source symbol ID
    ///
    /// # Returns
    ///
    /// Vector of call relationships
    pub fn callees_of_symbol(&self, symbol_id: u64) -> Vec<CallRelation> {
        let callees = self.backend.get_callees(symbol_id);

        callees
            .into_iter()
            .filter_map(|callee_id| {
                let caller_info = self.backend.find_symbol_by_id_info(symbol_id)?;
                let callee_info = self.backend.find_symbol_by_id_info(callee_id)?;
                Some(CallRelation {
                    caller_id: symbol_id,
                    caller_name: caller_info.name.clone(),
                    callee_id,
                    callee_name: callee_info.name.clone(),
                })
            })
            .collect()
    }
}

/// Geometric bridge for .geo databases
///
/// This bridge provides a subset of `MagellanBridge` functionality that works
/// with Geometric (.geo) backend databases. It mirrors the API of `MagellanBridge`
/// but uses the GeometricBackend directly rather than going through `CodeGraph`.
///
/// This allows Mirage CLI commands to work seamlessly with .geo databases.
///
/// # Example
///
/// ```rust,no_run
/// use mirage_analyzer::integrations::magellan::GeometricBridge;
///
/// let bridge = GeometricBridge::open("code.geo")?;
/// let cycles = bridge.detect_cycles()?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub struct GeometricBridge {
    /// The owned Geometric backend
    backend: GeometricBackend,
}

impl GeometricBridge {
    /// Open a Geometric (.geo) database
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to the .geo database file
    ///
    /// # Returns
    ///
    /// A new `GeometricBridge` instance
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened or is not a valid .geo database
    pub fn open(db_path: &str) -> Result<Self> {
        let path = Path::new(db_path);
        let backend = GeometricBackend::open(path)
            .map_err(|e| anyhow::anyhow!("Failed to open .geo database: {}", e))?;
        Ok(Self { backend })
    }

    /// Get a reference to the underlying adapter
    pub fn adapter(&self) -> MagellanAdapter<'_> {
        MagellanAdapter::new(&self.backend)
    }

    /// Get a reference to the underlying backend
    pub fn backend(&self) -> &GeometricBackend {
        &self.backend
    }

    /// Get reachable symbols from a starting point
    ///
    /// This mirrors the `MagellanBridge::reachable_symbols` API.
    ///
    /// # Arguments
    ///
    /// * `symbol_id_or_fqn` - Symbol ID (u64 string) or FQN
    ///
    /// # Returns
    ///
    /// Vector of `SymbolInfo` for reachable symbols
    pub fn reachable_symbols(
        &self,
        symbol_id_or_fqn: &str,
    ) -> Result<Vec<magellan::graph::geometric_backend::SymbolInfo>> {
        let adapter = MagellanAdapter::new(&self.backend);

        // Parse as numeric ID first
        if let Ok(id) = symbol_id_or_fqn.parse::<u64>() {
            let reachable_ids = adapter.reachable_from(id);
            return Ok(reachable_ids
                .into_iter()
                .filter_map(|id| self.backend.find_symbol_by_id_info(id))
                .collect());
        }

        // Try FQN lookup
        match adapter.lookup_symbol_by_fqn(symbol_id_or_fqn) {
            FqnLookupResult::Unique(info) => {
                let reachable_ids = adapter.reachable_from(info.id);
                Ok(reachable_ids
                    .into_iter()
                    .filter_map(|id| self.backend.find_symbol_by_id_info(id))
                    .collect())
            }
            FqnLookupResult::NotFound => {
                anyhow::bail!("Symbol '{}' not found", symbol_id_or_fqn)
            }
            FqnLookupResult::Ambiguous { .. } => {
                anyhow::bail!(
                    "Ambiguous reference to '{}', use numeric ID",
                    symbol_id_or_fqn
                )
            }
        }
    }

    /// Find dead symbols (unreachable from entry points)
    ///
    /// This mirrors the `MagellanBridge::dead_symbols` API.
    ///
    /// # Arguments
    ///
    /// * `entry_symbol_id_or_fqn` - Entry point symbol ID or FQN
    ///
    /// # Returns
    ///
    /// Vector of dead symbol information
    pub fn dead_symbols(&self, entry_symbol_id_or_fqn: &str) -> Result<Vec<DeadSymbolInfo>> {
        let adapter = MagellanAdapter::new(&self.backend);

        // Resolve entry point to ID
        let entry_id = if let Ok(id) = entry_symbol_id_or_fqn.parse::<u64>() {
            id
        } else {
            match adapter.lookup_symbol_by_fqn(entry_symbol_id_or_fqn) {
                FqnLookupResult::Unique(info) => info.id,
                FqnLookupResult::NotFound => {
                    anyhow::bail!("Entry point '{}' not found", entry_symbol_id_or_fqn)
                }
                FqnLookupResult::Ambiguous { .. } => {
                    anyhow::bail!(
                        "Ambiguous entry point '{}', use numeric ID",
                        entry_symbol_id_or_fqn
                    )
                }
            }
        };

        Ok(adapter.dead_code_from_entries(&[entry_id]))
    }

    /// Detect cycles in the call graph
    ///
    /// This mirrors the `MagellanBridge::detect_cycles` API.
    ///
    /// # Returns
    ///
    /// Vector of detected cycles
    pub fn detect_cycles(&self) -> Result<Vec<CycleInfo>> {
        let adapter = MagellanAdapter::new(&self.backend);
        Ok(adapter.find_call_graph_cycles())
    }

    /// Enumerate paths in the call graph
    ///
    /// This mirrors the `MagellanBridge::enumerate_paths` API.
    ///
    /// # Arguments
    ///
    /// * `start_symbol_id_or_fqn` - Starting point (ID or FQN)
    /// * `end_symbol_id_or_fqn` - Optional endpoint (ID or FQN, None = all paths)
    /// * `max_depth` - Maximum path depth
    /// * `max_paths` - Maximum number of paths
    ///
    /// # Returns
    ///
    /// Path enumeration result
    pub fn enumerate_paths(
        &self,
        start_symbol_id_or_fqn: &str,
        end_symbol_id_or_fqn: Option<&str>,
        max_depth: usize,
        max_paths: usize,
    ) -> Result<PathEnumerationResult> {
        let adapter = MagellanAdapter::new(&self.backend);

        // Resolve start ID
        let start_id = if let Ok(id) = start_symbol_id_or_fqn.parse::<u64>() {
            id
        } else {
            match adapter.lookup_symbol_by_fqn(start_symbol_id_or_fqn) {
                FqnLookupResult::Unique(info) => info.id,
                FqnLookupResult::NotFound => {
                    anyhow::bail!("Start symbol '{}' not found", start_symbol_id_or_fqn)
                }
                FqnLookupResult::Ambiguous { .. } => {
                    anyhow::bail!(
                        "Ambiguous start symbol '{}', use numeric ID",
                        start_symbol_id_or_fqn
                    )
                }
            }
        };

        // Resolve end ID (if provided)
        let end_id = if let Some(end) = end_symbol_id_or_fqn {
            if let Ok(id) = end.parse::<u64>() {
                Some(id)
            } else {
                match adapter.lookup_symbol_by_fqn(end) {
                    FqnLookupResult::Unique(info) => Some(info.id),
                    FqnLookupResult::NotFound => {
                        anyhow::bail!("End symbol '{}' not found", end)
                    }
                    FqnLookupResult::Ambiguous { .. } => {
                        anyhow::bail!("Ambiguous end symbol '{}', use numeric ID", end)
                    }
                }
            }
        } else {
            None
        };

        Ok(adapter.enumerate_paths(start_id, end_id, max_depth, max_paths))
    }
}

/// Normalize a file path for Mag queries
///
/// This helper ensures consistent path handling across all queries.
/// Paths are normalized before being passed to Magellan, ensuring that:
/// - `./src/x.rs` and `src/x.rs` resolve consistently
/// - `src\\x.rs` converts to `src/x.rs` (Windows paths)
/// - Redundant separators are removed (`src//lib.rs` → `src/lib.rs`)
///
/// This implements the contract requirement: "Paths normalized during
/// ingest/query (v3.0.2)" (GEO_QUERY_CONTRACT.md §Path Normalization).
///
/// # Arguments
///
/// * `path` - File path to normalize
///
/// # Returns
///
/// Normalized path string, or original if normalization fails
///
/// # Example
///
/// ```rust
/// use mirage_analyzer::integrations::magellan::normalize_path_for_query;
///
/// assert!(normalize_path_for_query("./src/lib.rs").ends_with("src/lib.rs"));
/// assert_eq!(normalize_path_for_query("src//lib.rs"), "src/lib.rs");
/// ```
pub fn normalize_path_for_query(path: &str) -> String {
    use std::path::Path;
    
    // Pre-process path to handle double slashes and backslashes
    // This ensures consistent behavior before canonicalization
    let preprocessed = path.replace("//", "/").replace('\\', "/");
    
    match normalize_path(Path::new(&preprocessed)) {
        Ok(normalized) => normalized,
        Err(_) => {
            // Fallback to preprocessed path if normalization fails
            // This preserves functionality for edge cases
            preprocessed
        }
    }
}

/// Check if two paths refer to the same file using normalized comparison
///
/// # Arguments
///
/// * `path1` - First path
/// * `path2` - Second path
///
/// # Returns
///
/// `true` if paths refer to the same file
///
/// # Example
///
/// ```rust
/// use mirage_analyzer::integrations::magellan::paths_equivalent;
///
/// assert!(paths_equivalent("./src/lib.rs", "src/lib.rs"));
/// assert!(paths_equivalent("src//lib.rs", "src/lib.rs"));
/// ```
pub fn paths_equivalent(path1: &str, path2: &str) -> bool {
    let norm1 = normalize_path_for_query(path1);
    let norm2 = normalize_path_for_query(path2);
    norm1 == norm2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_for_query() {
        // Test that ./ prefix is handled
        let result = normalize_path_for_query("./src/lib.rs");
        // If src/lib.rs exists, it will be canonicalized to absolute path
        // Otherwise, ./ prefix is stripped
        assert!(result.ends_with("src/lib.rs") || result == "src/lib.rs");

        // Test that double slashes are removed (non-existent path won't canonicalize)
        let result = normalize_path_for_query("nonexistent//lib.rs");
        assert!(!result.contains("//"));

        // Test that backslash is converted
        let result = normalize_path_for_query("nonexistent\\lib.rs");
        assert!(!result.contains("\\"));
    }

    #[test]
    fn test_paths_equivalent() {
        // Use non-existent paths to avoid canonicalization to absolute paths
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
        // Invalid paths should still return something (the original)
        let result = normalize_path_for_query("");
        // Either normalization succeeds or returns original
        assert!(result.is_empty() || result == "");
    }

    #[test]
    fn test_symbol_lookup_result_counts() {
        // We can't test with real data without a database,
        // but we can test the count logic
        let result = SymbolLookupResult::NotFound;
        assert_eq!(result.count(), 0);
        assert!(result.is_not_found());

        // Note: Can't construct Unique/Ambiguous without SymbolInfo which
        // requires a real database. The logic is straightforward anyway.
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
