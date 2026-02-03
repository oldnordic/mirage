//! Inter-procedural analysis using Magellan's call graph
//!
//! This module provides a bridge to Magellan's graph algorithms,
//! enabling combined inter-procedural (Magellan) and intra-procedural (Mirage) analysis.
//!
//! # Architecture
//!
//! - **Magellan** (inter-procedural): Call graph algorithms, reachability, dead code detection
//! - **Mirage** (intra-procedural): CFG analysis, path enumeration, dominance
//!
//! The [`MagellanBridge`] struct wraps Magellan's [`CodeGraph`] to provide
//! a unified API for both layers of analysis.

use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;

// Re-export key types from Magellan for convenience
pub use magellan::CodeGraph;

// Re-export algorithm result types for ergonomic API
pub use magellan::{
    Cycle, CycleKind, CycleReport, CondensationGraph, CondensationResult, DeadSymbol,
    ExecutionPath, PathEnumerationResult, PathStatistics, ProgramSlice, SliceDirection,
    SliceResult, SliceStatistics, Supernode, SymbolInfo,
};

/// Serializable wrapper for [`DeadSymbol`]
///
/// Magellan's [`DeadSymbol`] doesn't implement Serialize, so we provide
/// a wrapper struct that can be serialized to JSON for CLI output.
#[derive(Debug, Clone, Serialize)]
pub struct DeadSymbolJson {
    /// Fully-qualified name of the dead symbol
    pub fqn: Option<String>,
    /// File path containing the symbol
    pub file_path: String,
    /// Symbol kind (Function, Method, Class, etc.)
    pub kind: String,
    /// Reason why this symbol is unreachable/dead
    pub reason: String,
}

impl From<&DeadSymbol> for DeadSymbolJson {
    fn from(dead: &DeadSymbol) -> Self {
        Self {
            fqn: dead.symbol.fqn.clone(),
            file_path: dead.symbol.file_path.clone(),
            kind: dead.symbol.kind.clone(),
            reason: dead.reason.clone(),
        }
    }
}

/// Serializable wrapper for [`SymbolInfo`]
///
/// Magellan's [`SymbolInfo`] doesn't implement Serialize, so we provide
/// a wrapper struct that can be serialized to JSON for CLI output.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolInfoJson {
    /// Stable symbol ID (32-char BLAKE3 hash)
    pub symbol_id: Option<String>,
    /// Fully-qualified name of the symbol
    pub fqn: Option<String>,
    /// File path containing the symbol
    pub file_path: String,
    /// Symbol kind (Function, Method, Class, etc.)
    pub kind: String,
}

impl From<&SymbolInfo> for SymbolInfoJson {
    fn from(symbol: &SymbolInfo) -> Self {
        Self {
            symbol_id: symbol.symbol_id.clone(),
            fqn: symbol.fqn.clone(),
            file_path: symbol.file_path.clone(),
            kind: symbol.kind.clone(),
        }
    }
}

/// Enhanced dead code detection combining Magellan and Mirage analysis
///
/// Combines inter-procedural dead code detection (uncalled functions from Magellan)
/// with intra-procedural dead code detection (unreachable blocks within functions from Mirage).
///
/// # Fields
///
/// - `uncalled_functions`: Functions never called from entry point (Magellan)
/// - `unreachable_blocks`: Unreachable blocks within functions (Mirage)
/// - `total_dead_count`: Total count of dead code items
#[derive(Debug, Clone, Serialize)]
pub struct EnhancedDeadCode {
    /// From Magellan: Functions never called from entry
    pub uncalled_functions: Vec<DeadSymbolJson>,
    /// From Mirage: Unreachable blocks within functions (function_name -> block_ids)
    pub unreachable_blocks: HashMap<String, Vec<usize>>,
    /// Total count of dead code items
    pub total_dead_count: usize,
}

/// Enhanced blast zone combining call graph and CFG impact analysis
///
/// This struct provides a comprehensive impact analysis by combining:
/// - **Inter-procedural impact** (call graph): Which functions are affected
/// - **Intra-procedural impact** (CFG): Which blocks/paths are affected within functions
#[derive(Debug, Clone, Serialize)]
pub struct EnhancedBlastZone {
    /// Target function/block being analyzed
    pub target: String,
    /// Forward: What functions this affects (via call graph)
    pub forward_reachable: Vec<SymbolInfoJson>,
    /// Backward: What functions affect this (reverse call graph)
    pub backward_reachable: Vec<SymbolInfoJson>,
    /// Intra-procedural: Path-based impact within function
    pub path_impact: Option<PathImpactSummary>,
}

/// Summary of path-based impact within a function
///
/// This represents the CFG-level impact analysis for blocks within a single function.
#[derive(Debug, Clone, Serialize)]
pub struct PathImpactSummary {
    /// Path ID being analyzed
    pub path_id: Option<String>,
    /// Path length in blocks
    pub path_length: usize,
    /// Block IDs affected by the path
    pub blocks_affected: Vec<usize>,
    /// Count of unique blocks affected
    pub unique_blocks_count: usize,
}

/// Bridge to Magellan's inter-procedural graph algorithms
///
/// Wraps [`CodeGraph`] to provide access to call graph algorithms including:
/// - Reachability analysis (forward/reverse)
/// - Dead code detection
/// - Cycle detection (mutual recursion)
/// - Program slicing
/// - Path enumeration
///
/// # Example
///
/// ```no_run
/// use mirage::analysis::MagellanBridge;
///
/// // Open existing Magellan database
/// let bridge = MagellanBridge::open("codemcp/mirage.db")?;
///
/// // Find all functions reachable from main
/// let reachable = bridge.reachable_symbols("main")?;
/// println!("Found {} reachable functions", reachable.len());
///
/// // Find dead code unreachable from entry points
/// let dead = bridge.graph().dead_symbols("main")?;
/// println!("Found {} dead symbols", dead.len());
/// # Ok::<(), anyhow::Error>(())
/// ```
pub struct MagellanBridge {
    /// Underlying Magellan code graph
    graph: CodeGraph,
}

impl MagellanBridge {
    /// Open a Magellan database for inter-procedural analysis
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to the Magellan database file (typically `codemcp/mirage.db`)
    ///
    /// # Returns
    ///
    /// A [`MagellanBridge`] instance ready for analysis
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mirage::analysis::MagellanBridge;
    ///
    /// let bridge = MagellanBridge::open("codemcp/mirage.db")?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn open(db_path: &str) -> Result<Self> {
        let graph = CodeGraph::open(db_path)?;
        Ok(Self { graph })
    }

    /// Get a reference to the underlying Magellan [`CodeGraph`]
    ///
    /// Provides direct access to all Magellan algorithms for advanced use cases.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mirage::analysis::MagellanBridge;
    ///
    /// let bridge = MagellanBridge::open("codemcp/mirage.db")?;
    ///
    /// // Access full CodeGraph API
    /// let cycles = bridge.graph().detect_cycles()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn graph(&self) -> &CodeGraph {
        &self.graph
    }

    /// Find all symbols reachable from a given symbol (forward reachability)
    ///
    /// Computes the transitive closure of the call graph starting from the
    /// specified symbol. This is useful for:
    /// - Impact analysis (what does changing this symbol affect?)
    /// - Test coverage (what code does this test exercise?)
    /// - Dependency tracing
    ///
    /// # Arguments
    ///
    /// * `symbol_id` - Stable symbol ID (32-char BLAKE3 hash) or FQN
    ///
    /// # Returns
    ///
    /// Vector of [`SymbolInfo`] for reachable symbols, sorted deterministically
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mirage::analysis::MagellanBridge;
    ///
    /// let bridge = MagellanBridge::open("codemcp/mirage.db")?;
    ///
    /// // Find all functions called from main (directly or indirectly)
    /// let reachable = bridge.reachable_symbols("main")?;
    /// for symbol in reachable {
    ///     println!("  - {}", symbol.fqn.as_deref().unwrap_or("?"));
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn reachable_symbols(&self, symbol_id: &str) -> Result<Vec<SymbolInfo>> {
        self.graph.reachable_symbols(symbol_id, None)
    }

    /// Find all symbols that can reach a given symbol (reverse reachability)
    ///
    /// Computes the reverse transitive closure of the call graph. Returns all
    /// symbols from which the specified symbol can be reached (i.e., all callers).
    /// This is useful for:
    /// - Bug isolation (what code affects this symbol?)
    /// - Refactoring safety (what needs to be updated?)
    /// - Root cause analysis
    ///
    /// # Arguments
    ///
    /// * `symbol_id` - Stable symbol ID (32-char BLAKE3 hash) or FQN
    ///
    /// # Returns
    ///
    /// Vector of [`SymbolInfo`] for symbols that can reach the target
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mirage::analysis::MagellanBridge;
    ///
    /// let bridge = MagellanBridge::open("codemcp/mirage.db")?;
    ///
    /// // Find all functions that call 'helper_function'
    /// let callers = bridge.reverse_reachable_symbols("helper_function")?;
    /// println!("{} functions call this", callers.len());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn reverse_reachable_symbols(&self, symbol_id: &str) -> Result<Vec<SymbolInfo>> {
        self.graph.reverse_reachable_symbols(symbol_id, None)
    }

    /// Find dead code unreachable from an entry point symbol
    ///
    /// Identifies all symbols in the call graph that cannot be reached from
    /// the specified entry point (e.g., `main`, `test_main`).
    ///
    /// # Limitations
    ///
    /// - Only considers the call graph
    /// - Symbols called via reflection, function pointers, or dynamic dispatch
    ///   may be incorrectly flagged
    /// - Test functions and platform-specific code may appear as dead code
    ///
    /// # Arguments
    ///
    /// * `entry_symbol_id` - Stable symbol ID of the entry point (e.g., main function)
    ///
    /// # Returns
    ///
    /// Vector of [`DeadSymbol`] for unreachable symbols
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mirage::analysis::MagellanBridge;
    ///
    /// let bridge = MagellanBridge::open("codemcp/mirage.db")?;
    ///
    /// // Find all functions unreachable from main
    /// let dead = bridge.dead_symbols("main")?;
    /// for dead_symbol in &dead {
    ///     println!("Dead: {} ({})",
    ///         dead_symbol.symbol.fqn.as_deref().unwrap_or("?"),
    ///         dead_symbol.reason);
    /// }
    ///
    /// // Convert to JSON-serializable format
    /// let json_symbols: Vec<DeadSymbolJson> = dead.iter().map(|d| d.into()).collect();
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn dead_symbols(&self, entry_symbol_id: &str) -> Result<Vec<DeadSymbol>> {
        self.graph.dead_symbols(entry_symbol_id)
    }

    /// Detect cycles in the call graph using SCC decomposition
    ///
    /// Finds all strongly connected components (SCCs) with more than one member,
    /// which indicate cycles or mutual recursion in the call graph.
    ///
    /// # Returns
    ///
    /// [`CycleReport`] containing all detected cycles
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mirage::analysis::MagellanBridge;
    ///
    /// let bridge = MagellanBridge::open("codemcp/mirage.db")?;
    ///
    /// let report = bridge.detect_cycles()?;
    /// println!("Found {} cycles", report.total_count);
    /// for cycle in &report.cycles {
    ///     println!("Cycle with {} members:", cycle.members.len());
    ///     for member in &cycle.members {
    ///         println!("  - {}", member.fqn.as_deref().unwrap_or("?"));
    ///     }
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn detect_cycles(&self) -> Result<CycleReport> {
        self.graph.detect_cycles()
    }

    /// Compute a backward program slice (what affects this symbol)
    ///
    /// Returns all symbols that can affect the target symbol through the call graph.
    /// This is useful for bug isolation.
    ///
    /// # Note
    ///
    /// Current implementation uses call-graph reachability as a fallback.
    /// Full CFG-based program slicing will be available in future versions.
    ///
    /// # Arguments
    ///
    /// * `symbol_id` - Stable symbol ID or FQN to slice from
    ///
    /// # Returns
    ///
    /// [`SliceResult`] containing the slice and statistics
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mirage::analysis::MagellanBridge;
    ///
    /// let bridge = MagellanBridge::open("codemcp/mirage.db")?;
    ///
    /// // Find what affects 'helper_function'
    /// let slice_result = bridge.backward_slice("helper_function")?;
    /// println!("{} symbols affect this function", slice_result.slice.symbol_count);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn backward_slice(&self, symbol_id: &str) -> Result<SliceResult> {
        self.graph.backward_slice(symbol_id)
    }

    /// Compute a forward program slice (what this symbol affects)
    ///
    /// Returns all symbols that the target symbol can affect through the call graph.
    /// This is useful for refactoring safety.
    ///
    /// # Note
    ///
    /// Current implementation uses call-graph reachability as a fallback.
    /// Full CFG-based program slicing will be available in future versions.
    ///
    /// # Arguments
    ///
    /// * `symbol_id` - Stable symbol ID or FQN to slice from
    ///
    /// # Returns
    ///
    /// [`SliceResult`] containing the slice and statistics
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mirage::analysis::MagellanBridge;
    ///
    /// let bridge = MagellanBridge::open("codemcp/mirage.db")?;
    ///
    /// // Find what 'main_function' affects
    /// let slice_result = bridge.forward_slice("main_function")?;
    /// println!("{} symbols are affected by this function", slice_result.slice.symbol_count);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn forward_slice(&self, symbol_id: &str) -> Result<SliceResult> {
        self.graph.forward_slice(symbol_id)
    }

    /// Enumerate execution paths from a starting symbol
    ///
    /// Finds all execution paths from `start_symbol_id` to `end_symbol_id` (if provided)
    /// or all paths starting from `start_symbol_id` (if end_symbol_id is None).
    ///
    /// Path enumeration uses bounded DFS to prevent infinite traversal in cyclic graphs.
    ///
    /// # Arguments
    ///
    /// * `start_symbol_id` - Starting symbol ID or FQN
    /// * `end_symbol_id` - Optional ending symbol ID or FQN
    /// * `max_depth` - Maximum path depth (default: 100)
    /// * `max_paths` - Maximum number of paths to return (default: 1000)
    ///
    /// # Returns
    ///
    /// [`PathEnumerationResult`] with all discovered paths and statistics
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mirage::analysis::MagellanBridge;
    ///
    /// let bridge = MagellanBridge::open("codemcp/mirage.db")?;
    ///
    /// // Find all paths from main to any leaf function
    /// let result = bridge.enumerate_paths("main", None, 50, 100)?;
    ///
    /// println!("Found {} paths", result.total_enumerated);
    /// println!("Average length: {:.2}", result.statistics.avg_length);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn enumerate_paths(
        &self,
        start_symbol_id: &str,
        end_symbol_id: Option<&str>,
        max_depth: usize,
        max_paths: usize,
    ) -> Result<PathEnumerationResult> {
        self.graph
            .enumerate_paths(start_symbol_id, end_symbol_id, max_depth, max_paths)
    }

    /// Condense the call graph by collapsing SCCs into supernodes
    ///
    /// Creates a condensation DAG by collapsing each strongly connected component
    /// into a single "supernode". The resulting graph is always acyclic.
    ///
    /// # Use Cases
    ///
    /// - **Topological Sorting**: Condensation graph is a DAG
    /// - **Mutual Recursion Detection**: Large supernodes indicate tight coupling
    /// - **Impact Analysis**: Changing one symbol affects its entire SCC
    ///
    /// # Returns
    ///
    /// [`CondensationResult`] with the condensed DAG and symbol-to-supernode mapping
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mirage::analysis::MagellanBridge;
    ///
    /// let bridge = MagellanBridge::open("codemcp/mirage.db")?;
    ///
    /// let condensed = bridge.condense_call_graph()?;
    ///
    /// println!("Condensed to {} supernodes", condensed.graph.supernodes.len());
    /// println!("Condensed graph has {} edges", condensed.graph.edges.len());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn condense_call_graph(&self) -> Result<CondensationResult> {
        self.graph.condense_call_graph()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magellan_bridge_creation() {
        // Test that MagellanBridge can be created (requires database)
        // This is a compile-time test - actual database integration tested in later plans
        let _ = || -> Result<()> {
            let _bridge = MagellanBridge::open("test.db")?;
            Ok(())
        };
    }

    #[test]
    fn test_dead_symbol_json_from_dead_symbol() {
        // Test DeadSymbolJson conversion from DeadSymbol
        use magellan::{SymbolInfo, DeadSymbol as MagellanDeadSymbol};

        let symbol_info = SymbolInfo {
            symbol_id: Some("test_symbol_id".to_string()),
            fqn: Some("test::function".to_string()),
            file_path: "test.rs".to_string(),
            kind: "Function".to_string(),
        };

        let dead = MagellanDeadSymbol {
            symbol: symbol_info,
            reason: "Not called from entry point".to_string(),
        };

        let json_symbol: DeadSymbolJson = (&dead).into();

        assert_eq!(json_symbol.fqn, Some("test::function".to_string()));
        assert_eq!(json_symbol.file_path, "test.rs");
        assert_eq!(json_symbol.kind, "Function");
        assert_eq!(json_symbol.reason, "Not called from entry point");
    }

    #[test]
    fn test_enhanced_dead_code_serialization() {
        // Test EnhancedDeadCode can be serialized to JSON
        use magellan::{SymbolInfo, DeadSymbol as MagellanDeadSymbol};

        let symbol_info = SymbolInfo {
            symbol_id: Some("test_id".to_string()),
            fqn: Some("dead::function".to_string()),
            file_path: "test.rs".to_string(),
            kind: "Function".to_string(),
        };

        let dead = MagellanDeadSymbol {
            symbol: symbol_info,
            reason: "Uncalled".to_string(),
        };

        let json_symbol: DeadSymbolJson = (&dead).into();

        let mut unreachable_blocks = std::collections::HashMap::new();
        unreachable_blocks.insert("test_func".to_string(), vec![1, 2, 3]);

        let enhanced = EnhancedDeadCode {
            uncalled_functions: vec![json_symbol],
            unreachable_blocks,
            total_dead_count: 4,
        };

        // Test serialization
        let json = serde_json::to_string(&enhanced).unwrap();
        assert!(json.contains("uncalled_functions"));
        assert!(json.contains("unreachable_blocks"));
        assert!(json.contains("total_dead_count"));
    }
}
