use crate::cfg::{BlockId, Cfg};
use petgraph::graph::NodeIndex;
use std::collections::HashSet;

/// Configurable limits for path enumeration
///
/// Prevents exponential explosion of paths in complex CFGs and
/// ensures termination in the presence of loops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathLimits {
    /// Maximum number of blocks per path
    pub max_length: usize,
    /// Maximum number of paths to enumerate
    pub max_paths: usize,
    /// Loop iterations to unroll before stopping
    pub loop_unroll_limit: usize,
}

impl Default for PathLimits {
    fn default() -> Self {
        Self {
            max_length: 1000,
            max_paths: 10000,
            loop_unroll_limit: 3,
        }
    }
}

impl PathLimits {
    /// Create new path limits with custom values
    pub fn new(max_length: usize, max_paths: usize, loop_unroll_limit: usize) -> Self {
        Self {
            max_length,
            max_paths,
            loop_unroll_limit,
        }
    }

    /// Create limits with a custom maximum path length
    pub fn with_max_length(mut self, max_length: usize) -> Self {
        self.max_length = max_length;
        self
    }

    /// Create limits with a custom maximum path count
    pub fn with_max_paths(mut self, max_paths: usize) -> Self {
        self.max_paths = max_paths;
        self
    }

    /// Create limits with a custom loop unroll limit
    pub fn with_loop_unroll_limit(mut self, loop_unroll_limit: usize) -> Self {
        self.loop_unroll_limit = loop_unroll_limit;
        self
    }

    /// Quick analysis preset for fast, approximate path enumeration
    ///
    /// Use this for:
    /// - Initial code exploration
    /// - IDE/integration features where responsiveness matters
    /// - Large codebases where thorough analysis would be too slow
    ///
    /// Tradeoffs:
    /// - May miss some paths due to lower limits
    /// - Loop unrolling is minimal (2 iterations)
    /// - Completes in <100ms for typical functions
    pub fn quick_analysis() -> Self {
        Self {
            max_length: 100,
            max_paths: 1000,
            loop_unroll_limit: 2,
        }
    }

    /// Thorough analysis preset for comprehensive path enumeration
    ///
    /// Use this for:
    /// - Final analysis before deployment
    /// - Security-critical code paths
    /// - Test coverage validation
    ///
    /// Tradeoffs:
    /// - Higher limits produce more complete results
    /// - May take several seconds on complex functions
    /// - Still bounded to prevent infinite loops
    pub fn thorough() -> Self {
        Self {
            max_length: 10000,
            max_paths: 100000,
            loop_unroll_limit: 5,
        }
    }
}

/// Compute BLAKE3 hash of a block sequence
///
/// Used to generate unique identifiers for paths. The hash includes
/// the path length to prevent collisions between different sequences
/// that might otherwise hash to the same value.
///
/// # Arguments
///
/// * `blocks` - Slice of block IDs in execution order
///
/// # Returns
///
/// Hex string representing the BLAKE3 hash
pub fn hash_path(blocks: &[BlockId]) -> String {
    let mut hasher = blake3::Hasher::new();

    hasher.update(&blocks.len().to_le_bytes());

    for &block_id in blocks {
        hasher.update(&block_id.to_le_bytes());
    }

    hasher.finalize().to_hex().to_string()
}

/// Pre-computed context for path enumeration
///
/// Contains analysis results that are shared across all path enumerations.
/// Computing this context once and reusing it is much more efficient than
/// recomputing for each enumeration call.
#[derive(Debug, Clone)]
pub struct EnumerationContext {
    /// Blocks reachable from the entry node
    pub reachable_blocks: HashSet<BlockId>,
    /// Loop header nodes (for bounding loop iterations)
    pub loop_headers: HashSet<NodeIndex>,
    /// Exit nodes (valid path termination points)
    pub exits: HashSet<NodeIndex>,
}

impl EnumerationContext {
    /// Create a new enumeration context by analyzing the CFG
    pub fn new(cfg: &Cfg) -> Self {
        let reachable_nodes = crate::cfg::reachability::find_reachable(cfg);
        let reachable_blocks: HashSet<BlockId> =
            reachable_nodes.iter().map(|&idx| cfg[idx].id).collect();

        let loop_headers = crate::cfg::loops::find_loop_headers(cfg);

        let mut exits: HashSet<NodeIndex> =
            crate::cfg::analysis::find_exits(cfg).into_iter().collect();

        if exits.is_empty() {
            for node in cfg.node_indices() {
                if cfg.neighbors(node).next().is_none() {
                    exits.insert(node);
                }
            }
        }

        Self {
            reachable_blocks,
            loop_headers,
            exits,
        }
    }

    /// Get the number of reachable blocks
    pub fn reachable_count(&self) -> usize {
        self.reachable_blocks.len()
    }

    /// Get the number of loop headers
    pub fn loop_count(&self) -> usize {
        self.loop_headers.len()
    }

    /// Get the number of exit nodes
    pub fn exit_count(&self) -> usize {
        self.exits.len()
    }

    /// Check if a block is reachable
    pub fn is_reachable(&self, block_id: BlockId) -> bool {
        self.reachable_blocks.contains(&block_id)
    }

    /// Check if a node is a loop header
    pub fn is_loop_header(&self, node: NodeIndex) -> bool {
        self.loop_headers.contains(&node)
    }

    /// Check if a node is an exit
    pub fn is_exit(&self, node: NodeIndex) -> bool {
        self.exits.contains(&node)
    }
}
