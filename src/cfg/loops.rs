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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};
    use petgraph::graph::DiGraph;

    /// Create a simple loop: 0 -> 1 -> 2 -> 1
    /// Block 1 is the loop header
    fn create_simple_loop_cfg() -> Cfg {
        let mut g = DiGraph::new();

        // Block 0: entry, goes to loop header
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        // Block 1: loop header, condition goes to 2 (continue) or 3 (exit)
        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![2], otherwise: 3 },
            source_location: None,
        });

        // Block 2: loop body, goes back to header
        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec!["loop body".to_string()],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        // Block 3: exit
        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b1, b3, EdgeType::FalseBranch);
        g.add_edge(b2, b1, EdgeType::LoopBack);

        g
    }

    #[test]
    fn test_detect_simple_loop() {
        let cfg = create_simple_loop_cfg();
        let loops = detect_natural_loops(&cfg);

        assert_eq!(loops.len(), 1);

        let loop_ = &loops[0];
        assert_eq!(loop_.header.index(), 1); // Block 1 is header
        assert_eq!(loop_.back_edge.0.index(), 2); // Back edge from 2
        assert_eq!(loop_.back_edge.1.index(), 1); // to 1
        assert!(loop_.contains(NodeIndex::new(1)));
        assert!(loop_.contains(NodeIndex::new(2)));
        assert!(!loop_.contains(NodeIndex::new(0))); // Entry not in loop
        assert!(!loop_.contains(NodeIndex::new(3))); // Exit not in loop
    }

    #[test]
    fn test_find_loop_headers() {
        let cfg = create_simple_loop_cfg();
        let headers = find_loop_headers(&cfg);

        assert_eq!(headers.len(), 1);
        assert!(headers.contains(&NodeIndex::new(1)));
    }

    #[test]
    fn test_is_loop_header() {
        let cfg = create_simple_loop_cfg();

        assert!(is_loop_header(&cfg, NodeIndex::new(1)));
        assert!(!is_loop_header(&cfg, NodeIndex::new(0)));
        assert!(!is_loop_header(&cfg, NodeIndex::new(2)));
    }

    #[test]
    fn test_no_loops() {
        let mut g = DiGraph::new();

        // Linear: 0 -> 1 -> 2
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 2 },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::Fallthrough);

        let loops = detect_natural_loops(&g);
        assert!(loops.is_empty());
    }

    #[test]
    fn test_nested_loops() {
        let mut g = DiGraph::new();

        // Create nested loops structure
        // 0 (entry) -> 1 (outer header)
        // 1 -> 2 (outer body/inner header) or 4 (outer exit)
        // 2 -> 3 (inner body) or 4
        // 3 -> 2 (back edge to inner)
        // 4 -> 5 (exit)

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![2], otherwise: 4 },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![3], otherwise: 1 },
            source_location: None,
        });

        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 2 },
            source_location: None,
        });

        let b4 = g.add_node(BasicBlock {
            id: 4,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b1, b4, EdgeType::FalseBranch);
        g.add_edge(b2, b3, EdgeType::TrueBranch);
        g.add_edge(b2, b1, EdgeType::LoopBack); // Outer back edge
        g.add_edge(b3, b2, EdgeType::LoopBack); // Inner back edge

        let loops = detect_natural_loops(&g);
        assert_eq!(loops.len(), 2); // Two loops detected

        let nested = find_nested_loops(&g);
        assert_eq!(nested.len(), 1); // One nesting relationship
    }

    #[test]
    fn test_empty_cfg() {
        let cfg: Cfg = DiGraph::new();
        assert!(detect_natural_loops(&cfg).is_empty());
        assert!(find_loop_headers(&cfg).is_empty());
    }

    #[test]
    fn test_loops_containing() {
        let cfg = create_simple_loop_cfg();

        // Node in loop body
        let loops_2 = loops_containing(&cfg, NodeIndex::new(2));
        assert_eq!(loops_2.len(), 1);

        // Node not in any loop
        let loops_0 = loops_containing(&cfg, NodeIndex::new(0));
        assert_eq!(loops_0.len(), 0);
    }

    #[test]
    fn test_loop_size() {
        let cfg = create_simple_loop_cfg();
        let loops = detect_natural_loops(&cfg);

        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].size(), 2); // Header + body
    }

    #[test]
    fn test_nesting_level() {
        let cfg = create_simple_loop_cfg();
        let loops = detect_natural_loops(&cfg);

        assert_eq!(loops.len(), 1);
        // Single loop has nesting level 0 (not nested in any other loop)
        assert_eq!(loops[0].nesting_level(&loops), 0);
    }

    #[test]
    fn test_nesting_level_nested() {
        let mut g = DiGraph::new();

        // Create nested loops
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![2], otherwise: 4 },
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![3], otherwise: 1 },
            source_location: None,
        });

        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 2 },
            source_location: None,
        });

        let b4 = g.add_node(BasicBlock {
            id: 4,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b1, b4, EdgeType::FalseBranch);
        g.add_edge(b2, b3, EdgeType::TrueBranch);
        g.add_edge(b2, b1, EdgeType::LoopBack);
        g.add_edge(b3, b2, EdgeType::LoopBack);

        let loops = detect_natural_loops(&g);
        assert_eq!(loops.len(), 2);

        // Find outer and inner loops
        let outer_loop = loops.iter().find(|l| l.header.index() == 1).unwrap();
        let inner_loop = loops.iter().find(|l| l.header.index() == 2).unwrap();

        // Outer loop has level 0
        assert_eq!(outer_loop.nesting_level(&loops), 0);
        // Inner loop has level 1 (nested inside outer)
        assert_eq!(inner_loop.nesting_level(&loops), 1);
    }
}
