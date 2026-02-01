//! Natural loop detection using dominance analysis

use crate::cfg::Cfg;
use crate::cfg::analysis::find_entry;
use petgraph::algo::dominators::simple_fast;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use std::collections::{HashSet, VecDeque};

/// A natural loop detected in the CFG
///
/// A natural loop has a single entry point (the header) and
/// is identified by a back-edge where the header dominates the tail.
#[derive(Debug, Clone)]
pub struct NaturalLoop {
    /// Loop header node (single entry point)
    pub header: NodeIndex,
    /// Back edge (tail -> header) that identifies this loop
    pub back_edge: (NodeIndex, NodeIndex),
    /// All nodes in the loop body (including header)
    pub body: HashSet<NodeIndex>,
}

impl NaturalLoop {
    /// Check if a node is in the loop body
    pub fn contains(&self, node: NodeIndex) -> bool {
        self.body.contains(&node)
    }

    /// Get the number of nodes in the loop body
    pub fn size(&self) -> usize {
        self.body.len()
    }

    /// Get the loop depth (nesting level) relative to other loops
    ///
    /// Returns 0 for outermost loops, 1 for loops nested inside one outer loop, etc.
    pub fn nesting_level(&self, all_loops: &[NaturalLoop]) -> usize {
        let mut level = 0;
        for other in all_loops {
            if other.header != self.header && other.body.contains(&self.header) {
                level = level.max(other.nesting_level(all_loops) + 1);
            }
        }
        level
    }
}

/// Detect all natural loops in a CFG
///
/// Uses the dominance-based definition: a back-edge (N -> H) where
/// H dominates N. The loop consists of H plus all nodes that can
/// reach N without going through H.
///
/// Returns an empty vec if:
/// - CFG has no entry (empty graph)
/// - No back-edges exist (no loops)
///
/// # Example
/// ```rust
/// let loops = detect_natural_loops(&cfg);
/// for loop_ in &loops {
///     println!("Loop header: {:?}", loop_.header);
///     println!("Loop body size: {}", loop_.size());
/// }
/// ```
pub fn detect_natural_loops(cfg: &Cfg) -> Vec<NaturalLoop> {
    let entry = match find_entry(cfg) {
        Some(e) => e,
        None => return vec![],
    };

    // Compute dominators using Cooper et al. algorithm
    let dominators = simple_fast(cfg, entry);

    let mut loops = Vec::new();

    // Find all back edges: (N -> H) where H dominates N
    for edge in cfg.edge_references() {
        let tail = edge.source();
        let header = edge.target();

        // Check if this is a back edge (header dominates tail)
        // Header dominates tail if header is in tail's dominator set
        if let Some(mut tail_dominators) = dominators.dominators(tail) {
            if tail_dominators.any(|d| d == header) {
                let body = compute_loop_body(cfg, header, tail);
                loops.push(NaturalLoop {
                    header,
                    back_edge: (tail, header),
                    body,
                });
            }
        }
    }

    loops
}

/// Compute loop body from back edge (tail -> header)
///
/// The body includes:
/// - The header
/// - The tail
/// - All nodes that can reach the tail without going through the header
///
/// This is the standard algorithm for finding nodes in a natural loop.
fn compute_loop_body(cfg: &Cfg, header: NodeIndex, tail: NodeIndex) -> HashSet<NodeIndex> {
    let mut body = HashSet::new();
    let mut worklist = VecDeque::new();

    worklist.push_back(tail);

    while let Some(node) = worklist.pop_front() {
        if node == header {
            continue;
        }

        if body.contains(&node) {
            continue;
        }

        body.insert(node);

        // Add all predecessors of this node that can reach it without going through header
        for pred in cfg.neighbors_directed(node, petgraph::Direction::Incoming) {
            if pred != header && !body.contains(&pred) {
                worklist.push_back(pred);
            }
        }
    }

    body.insert(header); // Always include header
    body
}

/// Find all loop headers in the CFG
///
/// A node is a loop header if it's the target of a back-edge.
///
/// # Example
/// ```rust
/// let headers = find_loop_headers(&cfg);
/// for header in headers {
///     println!("Node {:?} is a loop header", header);
/// }
/// ```
pub fn find_loop_headers(cfg: &Cfg) -> HashSet<NodeIndex> {
    detect_natural_loops(cfg)
        .into_iter()
        .map(|loop_| loop_.header)
        .collect()
}

/// Check if a node is a loop header
///
/// Returns true if the node is the target of any back-edge.
pub fn is_loop_header(cfg: &Cfg, node: NodeIndex) -> bool {
    find_loop_headers(cfg).contains(&node)
}

/// Get all loops that contain a given node
///
/// A node may be in multiple loop bodies due to nesting.
pub fn loops_containing(cfg: &Cfg, node: NodeIndex) -> Vec<NaturalLoop> {
    detect_natural_loops(cfg)
        .into_iter()
        .filter(|loop_| loop_.body.contains(&node))
        .collect()
}

/// Find nested loops (loops inside other loops)
///
/// Returns pairs of (outer_loop, inner_loop) where inner is nested inside outer.
pub fn find_nested_loops(cfg: &Cfg) -> Vec<(NaturalLoop, NaturalLoop)> {
    let loops = detect_natural_loops(cfg);
    let mut nested = Vec::new();

    for (i, outer) in loops.iter().enumerate() {
        for inner in loops.iter().skip(i + 1) {
            // Inner is nested if its header is in outer's body
            if outer.body.contains(&inner.header) {
                nested.push((outer.clone(), inner.clone()));
            }
        }
    }

    nested
}
