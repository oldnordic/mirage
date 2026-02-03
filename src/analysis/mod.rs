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

/// Serializable wrapper for program slice results
///
/// Magellan's [`SliceResult`] doesn't implement Serialize, so we provide
/// a wrapper struct that can be serialized to JSON for CLI output.
#[derive(Debug, Clone, Serialize)]
pub struct SliceWrapper {
    /// Target symbol for the slice
    pub target: SymbolInfoJson,
    /// Direction of slicing
    pub direction: String, // "backward" or "forward"
    /// Symbols included in the slice
    pub included_symbols: Vec<SymbolInfoJson>,
    /// Number of symbols in the slice
    pub symbol_count: usize,
    /// Statistics about the slice
    pub statistics: SliceStats,
}

/// Statistics for program slicing
#[derive(Debug, Clone, Serialize)]
pub struct SliceStats {
    pub total_symbols: usize,
    pub data_dependencies: usize,
    pub control_dependencies: usize,
}

impl From<&SliceResult> for SliceWrapper {
    fn from(result: &SliceResult) -> Self {
        let statistics = SliceStats {
            total_symbols: result.statistics.total_symbols,
            data_dependencies: result.statistics.data_dependencies,
            control_dependencies: result.statistics.control_dependencies,
        };

        SliceWrapper {
            target: (&result.slice.target).into(),
            direction: format!("{:?}", result.slice.direction),
            included_symbols: result.slice.included_symbols.iter()
                .map(|s| s.into())
                .collect(),
            symbol_count: result.slice.symbol_count,
            statistics,
        }
    }
}

/// Serializable wrapper for inter-procedural execution paths
///
/// Represents a call chain from one function to another through the call graph.
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionPathJson {
    /// Functions in this call chain (ordered from start to end)
    pub symbols: Vec<SymbolInfoJson>,
    /// Path length (number of function calls)
    pub length: usize,
}

impl From<&ExecutionPath> for ExecutionPathJson {
    fn from(path: &ExecutionPath) -> Self {
        ExecutionPathJson {
            symbols: path.symbols.iter().map(|s| s.into()).collect(),
            length: path.length,
        }
    }
}

/// Serializable wrapper for path enumeration results
///
/// Wraps Magellan's [`PathEnumerationResult`] for CLI JSON output.
#[derive(Debug, Clone, Serialize)]
pub struct PathEnumerationJson {
    /// All discovered execution paths
    pub paths: Vec<ExecutionPathJson>,
    /// Total number of paths enumerated
    pub total_enumerated: usize,
    /// Whether enumeration was truncated due to limits
    pub truncated: bool,
    /// Statistics about enumerated paths
    pub statistics: PathStatisticsJson,
}

/// Serializable statistics for path enumeration
#[derive(Debug, Clone, Serialize)]
pub struct PathStatisticsJson {
    /// Average path length
    pub avg_length: f64,
    /// Maximum path length
    pub max_length: usize,
    /// Minimum path length
    pub min_length: usize,
}

impl From<&PathEnumerationResult> for PathEnumerationJson {
    fn from(result: &PathEnumerationResult) -> Self {
        PathEnumerationJson {
            paths: result.paths.iter().map(|p| p.into()).collect(),
            total_enumerated: result.total_enumerated,
            truncated: result.bounded_hit,
            statistics: PathStatisticsJson {
                avg_length: result.statistics.avg_length,
                max_length: result.statistics.max_length,
                min_length: result.statistics.min_length,
            },
        }
    }
}

/// Serializable wrapper for call graph condensation results
///
/// Magellan's [`CondensationResult`] doesn't implement Serialize,
/// so we provide a wrapper struct for CLI JSON output.
#[derive(Debug, Clone, Serialize)]
pub struct CondensationJson {
    /// Number of supernodes (SCCs) in the condensed graph
    pub supernode_count: usize,
    /// Number of edges between supernodes
    pub edge_count: usize,
    /// Supernodes with their member functions
    pub supernodes: Vec<SupernodeJson>,
    /// Largest SCC size (indicates tight coupling)
    pub largest_scc_size: usize,
}

/// Serializable representation of a supernode (SCC)
#[derive(Debug, Clone, Serialize)]
pub struct SupernodeJson {
    /// Supernode ID
    pub id: String,
    /// Number of functions in this SCC
    pub member_count: usize,
    /// Member function names
    pub members: Vec<String>,
}

impl From<&CondensationResult> for CondensationJson {
    fn from(result: &CondensationResult) -> Self {
        let supernodes: Vec<SupernodeJson> = result
            .graph
            .supernodes
            .iter()
            .map(|sn| SupernodeJson {
                id: sn.id.to_string(),
                member_count: sn.members.len(),
                members: sn
                    .members
                    .iter()
                    .filter_map(|m| m.fqn.clone())
                    .collect(),
            })
            .collect();

        let largest_scc_size = supernodes
            .iter()
            .map(|sn| sn.member_count)
            .max()
            .unwrap_or(0);

        CondensationJson {
            supernode_count: result.graph.supernodes.len(),
            edge_count: result.graph.edges.len(),
            supernodes,
            largest_scc_size,
        }
    }
}

/// Information about a call graph cycle
///
/// Serializable wrapper for cycle detection results.
#[derive(Debug, Clone, Serialize)]
pub struct CycleInfo {
    /// Fully-qualified names of cycle members
    pub members: Vec<String>,
    /// Cycle type classification
    pub cycle_type: String,
    /// Number of symbols in the cycle
    pub size: usize,
}

impl From<&Cycle> for CycleInfo {
    fn from(cycle: &Cycle) -> Self {
        let members: Vec<String> = cycle.members.iter()
            .map(|m| m.fqn.as_deref().unwrap_or("<unknown>").to_string())
            .collect();

        let cycle_type = match cycle.kind {
            CycleKind::MutualRecursion => "MutualRecursion",
            CycleKind::SelfLoop => "SelfLoop",
        };

        Self {
            members,
            cycle_type: cycle_type.to_string(),
            size: cycle.members.len(),
        }
    }
}

/// Information about a natural loop within a function
///
/// Represents intra-procedural loop structure detected via dominance analysis.
#[derive(Debug, Clone, Serialize)]
pub struct LoopInfo {
    /// Loop header block ID
    pub header: usize,
    /// Back edge source block ID
    pub back_edge_from: usize,
    /// Number of blocks in the loop body
    pub body_size: usize,
    /// Nesting depth (0 for outermost loops)
    pub nesting_level: usize,
    /// Block IDs in the loop body
    pub body_blocks: Vec<usize>,
}

/// Combined cycle detection report
///
/// Combines inter-procedural (call graph SCCs) and intra-procedural (natural loops)
/// cycle detection for complete cycle visibility.
#[derive(Debug, Clone, Serialize)]
pub struct EnhancedCycles {
    /// Inter-procedural: Call graph SCCs (mutual recursion)
    pub call_graph_cycles: Vec<CycleInfo>,
    /// Intra-procedural: Natural loops within functions
    pub function_loops: HashMap<String, Vec<LoopInfo>>,
    /// Total cycle count (call graph + function loops)
    pub total_cycles: usize,
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
    /// println!("{} symbols affect this function", slice_result.symbol_count);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn backward_slice(&self, symbol_id: &str) -> Result<SliceWrapper> {
        let result = self.graph.backward_slice(symbol_id)?;
        Ok((&result).into())
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
    /// [`SliceWrapper`] containing the slice and statistics
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
    /// println!("{} symbols are affected by this function", slice_result.symbol_count);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn forward_slice(&self, symbol_id: &str) -> Result<SliceWrapper> {
        let result = self.graph.forward_slice(symbol_id)?;
        Ok((&result).into())
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

    /// Enumerate paths and return JSON-serializable result
    ///
    /// Convenience method that wraps [`PathEnumerationResult`] in a
    /// JSON-serializable format for CLI output.
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
    /// JSON-serializable path enumeration result
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mirage::analysis::MagellanBridge;
    ///
    /// let bridge = MagellanBridge::open("codemcp/mirage.db")?;
    /// let result = bridge.enumerate_paths_json("main", None, 50, 100)?;
    /// println!("Found {} paths", result.total_enumerated);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn enumerate_paths_json(
        &self,
        start_symbol_id: &str,
        end_symbol_id: Option<&str>,
        max_depth: usize,
        max_paths: usize,
    ) -> Result<PathEnumerationJson> {
        let result = self.graph.enumerate_paths(start_symbol_id, end_symbol_id, max_depth, max_paths)?;
        Ok((&result).into())
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
    /// - **Inter-procedural Dominance**: Functions in root supernodes dominate downstream functions
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

    /// Condense call graph and return JSON-serializable result
    ///
    /// Convenience method that wraps [`CondensationResult`] in a
    /// JSON-serializable format for CLI output.
    ///
    /// # Returns
    ///
    /// [`CondensationJson`] with condensed DAG summary and supernode details
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mirage::analysis::MagellanBridge;
    ///
    /// let bridge = MagellanBridge::open("codemcp/mirage.db")?;
    /// let condensed = bridge.condense_call_graph_json()?;
    /// println!("Condensed to {} supernodes", condensed.supernode_count);
    /// println!("Largest SCC has {} functions", condensed.largest_scc_size);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn condense_call_graph_json(&self) -> Result<CondensationJson> {
        let result = self.graph.condense_call_graph()?;
        Ok((&result).into())
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

    #[test]
    fn test_cycle_info_from_cycle() {
        // Test CycleInfo conversion from Cycle
        use magellan::{SymbolInfo, Cycle, CycleKind};

        let symbol1 = SymbolInfo {
            symbol_id: Some("func_a_id".to_string()),
            fqn: Some("func_a".to_string()),
            file_path: "test.rs".to_string(),
            kind: "Function".to_string(),
        };

        let symbol2 = SymbolInfo {
            symbol_id: Some("func_b_id".to_string()),
            fqn: Some("func_b".to_string()),
            file_path: "test.rs".to_string(),
            kind: "Function".to_string(),
        };

        // Test MutualRecursion cycle
        let mutual_recursion_cycle = Cycle {
            members: vec![symbol1.clone(), symbol2.clone()],
            kind: CycleKind::MutualRecursion,
        };

        let cycle_info: CycleInfo = (&mutual_recursion_cycle).into();
        assert_eq!(cycle_info.cycle_type, "MutualRecursion");
        assert_eq!(cycle_info.size, 2);
        assert_eq!(cycle_info.members, vec!["func_a", "func_b"]);

        // Test SelfLoop cycle
        let self_loop_cycle = Cycle {
            members: vec![symbol1],
            kind: CycleKind::SelfLoop,
        };

        let cycle_info: CycleInfo = (&self_loop_cycle).into();
        assert_eq!(cycle_info.cycle_type, "SelfLoop");
        assert_eq!(cycle_info.size, 1);
        assert_eq!(cycle_info.members, vec!["func_a"]);
    }

    #[test]
    fn test_enhanced_cycles_serialization() {
        // Test EnhancedCycles can be serialized to JSON
        use std::collections::HashMap;

        let mut function_loops = HashMap::new();
        function_loops.insert("test_func".to_string(), vec![
            LoopInfo {
                header: 1,
                back_edge_from: 2,
                body_size: 3,
                nesting_level: 0,
                body_blocks: vec![1, 2, 3],
            }
        ]);

        let call_graph_cycles = vec![
            CycleInfo {
                members: vec!["func_a".to_string(), "func_b".to_string()],
                cycle_type: "MutualRecursion".to_string(),
                size: 2,
            }
        ];

        let enhanced = EnhancedCycles {
            call_graph_cycles,
            function_loops,
            total_cycles: 2,
        };

        // Test serialization
        let json = serde_json::to_string(&enhanced).unwrap();
        assert!(json.contains("call_graph_cycles"));
        assert!(json.contains("function_loops"));
        assert!(json.contains("total_cycles"));
        assert!(json.contains("MutualRecursion"));
    }

    #[test]
    fn test_loop_info_serialization() {
        // Test LoopInfo can be serialized to JSON
        let loop_info = LoopInfo {
            header: 1,
            back_edge_from: 3,
            body_size: 5,
            nesting_level: 2,
            body_blocks: vec![1, 2, 3, 4, 5],
        };

        // Test serialization
        let json = serde_json::to_string(&loop_info).unwrap();
        assert!(json.contains(r#""header":1"#));
        assert!(json.contains(r#""back_edge_from":3"#));
        assert!(json.contains(r#""body_size":5"#));
        assert!(json.contains(r#""nesting_level":2"#));
        assert!(json.contains(r#"body_blocks"#));
    }

    #[test]
    fn test_slice_wrapper_serialization() {
        // Test SliceWrapper can be serialized to JSON
        use magellan::{ProgramSlice, SliceDirection, SliceResult, SliceStatistics};

        let target = SymbolInfo {
            symbol_id: Some("target_id".to_string()),
            fqn: Some("target_function".to_string()),
            file_path: "test.rs".to_string(),
            kind: "Function".to_string(),
        };

        let included_symbols = vec![
            SymbolInfo {
                symbol_id: Some("sym1_id".to_string()),
                fqn: Some("sym1".to_string()),
                file_path: "test.rs".to_string(),
                kind: "Function".to_string(),
            },
            SymbolInfo {
                symbol_id: Some("sym2_id".to_string()),
                fqn: Some("sym2".to_string()),
                file_path: "test.rs".to_string(),
                kind: "Function".to_string(),
            },
        ];

        let program_slice = ProgramSlice {
            target: target.clone(),
            direction: SliceDirection::Backward,
            included_symbols: included_symbols.clone(),
            symbol_count: 3,
        };

        let statistics = SliceStatistics {
            total_symbols: 3,
            data_dependencies: 2,
            control_dependencies: 1,
        };

        let slice_result = SliceResult {
            slice: program_slice,
            statistics,
        };

        let wrapper: SliceWrapper = (&slice_result).into();

        // Test wrapper fields
        assert_eq!(wrapper.target.fqn, Some("target_function".to_string()));
        assert_eq!(wrapper.direction, "Backward");
        assert_eq!(wrapper.symbol_count, 3);
        assert_eq!(wrapper.statistics.total_symbols, 3);
        assert_eq!(wrapper.statistics.data_dependencies, 2);
        assert_eq!(wrapper.statistics.control_dependencies, 1);
        assert_eq!(wrapper.included_symbols.len(), 2);

        // Test serialization
        let json = serde_json::to_string(&wrapper).unwrap();
        assert!(json.contains("target"));
        assert!(json.contains("direction"));
        assert!(json.contains("Backward"));
        assert!(json.contains("included_symbols"));
        assert!(json.contains("statistics"));
        assert!(json.contains("data_dependencies"));
    }

    #[test]
    fn test_slice_stats_creation() {
        // Test SliceStats struct creation
        let stats = SliceStats {
            total_symbols: 10,
            data_dependencies: 5,
            control_dependencies: 3,
        };

        assert_eq!(stats.total_symbols, 10);
        assert_eq!(stats.data_dependencies, 5);
        assert_eq!(stats.control_dependencies, 3);

        // Test serialization
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains(r#""total_symbols":10"#));
        assert!(json.contains(r#""data_dependencies":5"#));
        assert!(json.contains(r#""control_dependencies":3"#));
    }

    #[test]
    fn test_symbol_info_json_from_symbol_info() {
        // Test SymbolInfoJson conversion from SymbolInfo
        use magellan::SymbolInfo;

        let symbol_info = SymbolInfo {
            symbol_id: Some("test_symbol_id".to_string()),
            fqn: Some("test::function".to_string()),
            file_path: "test.rs".to_string(),
            kind: "Function".to_string(),
        };

        let json_symbol: SymbolInfoJson = (&symbol_info).into();

        assert_eq!(json_symbol.symbol_id, Some("test_symbol_id".to_string()));
        assert_eq!(json_symbol.fqn, Some("test::function".to_string()));
        assert_eq!(json_symbol.file_path, "test.rs");
        assert_eq!(json_symbol.kind, "Function");
    }

    #[test]
    fn test_enhanced_blast_zone_creation() {
        // Test EnhancedBlastZone struct creation and serialization
        let forward = vec![
            SymbolInfoJson {
                symbol_id: Some("func_a_id".to_string()),
                fqn: Some("func_a".to_string()),
                file_path: "a.rs".to_string(),
                kind: "Function".to_string(),
            },
            SymbolInfoJson {
                symbol_id: Some("func_b_id".to_string()),
                fqn: Some("func_b".to_string()),
                file_path: "b.rs".to_string(),
                kind: "Function".to_string(),
            },
        ];

        let backward = vec![
            SymbolInfoJson {
                symbol_id: Some("main_id".to_string()),
                fqn: Some("main".to_string()),
                file_path: "main.rs".to_string(),
                kind: "Function".to_string(),
            },
        ];

        let path_impact = PathImpactSummary {
            path_id: Some("test_path_id".to_string()),
            path_length: 5,
            blocks_affected: vec![1, 2, 3, 4],
            unique_blocks_count: 4,
        };

        let blast_zone = EnhancedBlastZone {
            target: "test_function".to_string(),
            forward_reachable: forward.clone(),
            backward_reachable: backward.clone(),
            path_impact: Some(path_impact),
        };

        assert_eq!(blast_zone.target, "test_function");
        assert_eq!(blast_zone.forward_reachable.len(), 2);
        assert_eq!(blast_zone.backward_reachable.len(), 1);
        assert!(blast_zone.path_impact.is_some());

        // Test serialization
        let json = serde_json::to_string(&blast_zone).unwrap();
        assert!(json.contains("target"));
        assert!(json.contains("forward_reachable"));
        assert!(json.contains("backward_reachable"));
        assert!(json.contains("path_impact"));
        assert!(json.contains("func_a"));
        assert!(json.contains("main"));
    }

    #[test]
    fn test_path_impact_summary_serialization() {
        // Test PathImpactSummary can be serialized to JSON
        let impact = PathImpactSummary {
            path_id: Some("test_path".to_string()),
            path_length: 10,
            blocks_affected: vec![1, 2, 3, 4, 5],
            unique_blocks_count: 5,
        };

        let json = serde_json::to_string(&impact).unwrap();
        assert!(json.contains("path_id"));
        assert!(json.contains("path_length"));
        assert!(json.contains("blocks_affected"));
        assert!(json.contains("unique_blocks_count"));
        assert!(json.contains("test_path"));
    }

    #[test]
    fn test_enhanced_blast_zone_without_path_impact() {
        // Test EnhancedBlastZone without optional path_impact
        let blast_zone = EnhancedBlastZone {
            target: "test_function".to_string(),
            forward_reachable: vec![],
            backward_reachable: vec![],
            path_impact: None,
        };

        assert!(blast_zone.path_impact.is_none());

        // Test serialization with None
        let json = serde_json::to_string(&blast_zone).unwrap();
        assert!(json.contains(r#""path_impact":null"#));
    }
}
