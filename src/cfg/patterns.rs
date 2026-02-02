//! Branching pattern recovery from CFG shapes

use crate::cfg::{BlockId, Cfg, Terminator};
use crate::cfg::analysis::is_branch_point;
use crate::cfg::EdgeType;
use petgraph::graph::NodeIndex;
use std::collections::HashSet;

/// Classification of a node's branching structure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchType {
    /// No branching (0 or 1 successor) - linear control flow
    Linear,
    /// Two-way conditional branch (if/else pattern)
    Conditional,
    /// Multi-way branch (match/switch)
    MultiWay,
    /// Unknown or complex branching
    Unknown,
}

/// Represents an if/else structure detected in the CFG
#[derive(Debug, Clone)]
pub struct IfElsePattern {
    /// Condition node (branch point)
    pub condition: NodeIndex,
    /// True branch target
    pub true_branch: NodeIndex,
    /// False branch target
    pub false_branch: NodeIndex,
    /// Merge point (where branches reconverge)
    /// None if branches don't merge (e.g., early return)
    pub merge_point: Option<NodeIndex>,
}

impl IfElsePattern {
    /// Check if this is a complete if/else (branches merge)
    pub fn has_else(&self) -> bool {
        self.merge_point.is_some()
    }

    /// Get the number of blocks in this pattern
    pub fn size(&self) -> usize {
        2 + if self.merge_point.is_some() { 1 } else { 0 }
    }
}

/// Represents a match/switch structure detected in the CFG
#[derive(Debug, Clone)]
pub struct MatchPattern {
    /// Switch node (contains SwitchInt terminator)
    pub switch_node: NodeIndex,
    /// Branch targets (excluding default/otherwise)
    pub targets: Vec<NodeIndex>,
    /// Default/otherwise branch
    pub otherwise: NodeIndex,
}

impl MatchPattern {
    /// Get total number of branches
    pub fn branch_count(&self) -> usize {
        self.targets.len() + 1 // +1 for otherwise
    }

    /// Check if this match is exhaustive (all branches defined)
    /// This is a simplified check - true exhaustiveness requires type info
    pub fn has_explicit_default(&self) -> bool {
        // In our representation, otherwise always exists
        // A more sophisticated version would check if it's reachable
        true
    }
}

/// Classify a node's branching structure
///
/// Returns the type of control flow at this node based on
/// outgoing edges and terminator type.
///
/// # Example
/// ```rust,no_run
/// # use mirage::cfg::patterns::{classify_branch, BranchType};
/// # use mirage::cfg::Cfg;
/// # let graph: Cfg = unimplemented!();
/// for node in graph.node_indices() {
///     match classify_branch(&graph, node) {
///         BranchType::Conditional => println!("if/else at {:?}", node),
///         BranchType::MultiWay => println!("match at {:?}", node),
///         _ => {}
///     }
/// }
/// ```
pub fn classify_branch(cfg: &Cfg, node: NodeIndex) -> BranchType {
    let successors: Vec<_> = cfg.neighbors(node).collect();

    match successors.len() {
        0 | 1 => BranchType::Linear,
        2 => {
            // Check if it's a diamond pattern (if/else)
            let merge = find_common_successor(cfg, successors[0], successors[1]);
            if merge.is_some() {
                BranchType::Conditional
            } else {
                // Could be if without else, if with early return
                BranchType::Unknown
            }
        }
        3.. => {
            // Multi-way branch - check for SwitchInt
            if let Some(block) = cfg.node_weight(node) {
                if matches!(block.terminator, Terminator::SwitchInt { .. }) {
                    BranchType::MultiWay
                } else {
                    BranchType::Unknown
                }
            } else {
                BranchType::Unknown
            }
        }
    }
}

/// Find common successor of two nodes (merge point)
///
/// Returns the first node reachable from both n1 and n2 (excluding
/// the nodes themselves). This identifies where branches reconverge.
fn find_common_successor(cfg: &Cfg, n1: NodeIndex, n2: NodeIndex) -> Option<NodeIndex> {
    // Collect reachable nodes from n1 (excluding n1 and n2)
    let mut reachable_from_n1 = HashSet::new();
    let mut worklist = vec![n1];

    while let Some(node) = worklist.pop() {
        // Skip n1 and n2 themselves - we want their successors
        if node == n1 || node == n2 {
            for succ in cfg.neighbors(node) {
                if succ != n1 && succ != n2 && !reachable_from_n1.contains(&succ) {
                    worklist.push(succ);
                    reachable_from_n1.insert(succ);
                }
            }
            continue;
        }

        if !reachable_from_n1.insert(node) {
            continue;
        }
        for succ in cfg.neighbors(node) {
            if succ != n1 && succ != n2 && !reachable_from_n1.contains(&succ) {
                worklist.push(succ);
            }
        }
    }

    // Check nodes reachable from n2
    let mut visited = HashSet::new();
    let mut worklist = vec![n2];

    while let Some(node) = worklist.pop() {
        if node == n1 || node == n2 {
            for succ in cfg.neighbors(node) {
                if succ != n1 && succ != n2 && !visited.contains(&succ) {
                    if reachable_from_n1.contains(&succ) {
                        return Some(succ);
                    }
                    visited.insert(succ);
                    worklist.push(succ);
                }
            }
            continue;
        }

        if reachable_from_n1.contains(&node) {
            return Some(node);
        }

        if !visited.insert(node) {
            continue;
        }
        for succ in cfg.neighbors(node) {
            if succ != n1 && succ != n2 && !visited.contains(&succ) {
                worklist.push(succ);
            }
        }
    }

    None
}

/// Detect if/else patterns by looking for diamond structures
///
/// A diamond structure is:
/// - A branch point with 2 successors
/// - Both successors eventually merge to a common point
/// - NOT a multi-way SwitchInt (that's a match, not if/else)
///
/// Note: This distinguishes if/else from match by checking if the SwitchInt
/// has more than 1 target (if/else has 1 target + otherwise = 2 branches,
/// match has 2+ targets + otherwise = 3+ branches).
///
/// Returns an empty vec if no patterns found.
///
/// # Example
/// ```rust,no_run
/// # use mirage::cfg::patterns::detect_if_else_patterns;
/// # let graph = unimplemented!();
/// let patterns = detect_if_else_patterns(&graph);
/// for pattern in patterns {
///     println!("if/else at {:?}, merges at {:?}", pattern.condition, pattern.merge_point);
/// }
/// ```
pub fn detect_if_else_patterns(cfg: &Cfg) -> Vec<IfElsePattern> {
    let mut patterns = Vec::new();

    for branch in cfg.node_indices().filter(|&n| is_branch_point(cfg, n)) {
        let successors: Vec<_> = cfg.neighbors(branch).collect();

        if successors.len() == 2 {
            // Exclude multi-way SwitchInt terminators (3+ branches) - those are matches
            // If/else uses SwitchInt with 1 target (2 branches total)
            if let Some(block) = cfg.node_weight(branch) {
                if let Terminator::SwitchInt { targets, .. } = &block.terminator {
                    if targets.len() > 1 {
                        // This is a match, not if/else
                        continue;
                    }
                }
            }

            // Check for diamond pattern (merge point)
            let merge_point = find_common_successor(cfg, successors[0], successors[1]);

            // Determine which branch is true/false based on edge type
            let (true_branch, false_branch) = order_branches_by_edge_type(cfg, branch, successors[0], successors[1]);

            patterns.push(IfElsePattern {
                condition: branch,
                true_branch,
                false_branch,
                merge_point,
            });
        }
    }

    patterns
}

/// Order branches as (true, false) based on edge type
///
/// Uses EdgeType to determine which successor is the true branch
/// and which is the false branch.
fn order_branches_by_edge_type(
    cfg: &Cfg,
    from: NodeIndex,
    succ1: NodeIndex,
    succ2: NodeIndex,
) -> (NodeIndex, NodeIndex) {
    let edge1_type = cfg.find_edge(from, succ1).and_then(|e| cfg.edge_weight(e).copied());
    let edge2_type = cfg.find_edge(from, succ2).and_then(|e| cfg.edge_weight(e).copied());

    match (edge1_type, edge2_type) {
        (Some(EdgeType::TrueBranch), _) => (succ1, succ2),
        (_, Some(EdgeType::TrueBranch)) => (succ2, succ1),
        (Some(EdgeType::FalseBranch), _) => (succ2, succ1),
        (_, Some(EdgeType::FalseBranch)) => (succ1, succ2),
        _ => (succ1, succ2), // Default order if unclear
    }
}

/// Detect match patterns by looking for SwitchInt terminators
///
/// SwitchInt terminators with 2+ targets indicate multi-way branches (match/switch).
/// Two-way SwitchInt (1 target + otherwise) represents if/else, not match.
///
/// The pattern includes all branch targets plus the default/otherwise.
///
/// Returns an empty vec if no patterns found.
///
/// # Example
/// ```rust,no_run
/// # use mirage::cfg::patterns::detect_match_patterns;
/// # let graph = unimplemented!();
/// let patterns = detect_match_patterns(&graph);
/// for pattern in patterns {
///     println!("match at {:?} with {} branches", pattern.switch_node, pattern.branch_count());
/// }
/// ```
pub fn detect_match_patterns(cfg: &Cfg) -> Vec<MatchPattern> {
    let mut patterns = Vec::new();

    for node in cfg.node_indices() {
        if let Some(block) = cfg.node_weight(node) {
            if let Terminator::SwitchInt { targets, otherwise } = &block.terminator {
                // Only detect multi-way matches (2+ targets = 3+ branches)
                // Single-target SwitchInt (2 branches) is if/else, not match
                if targets.len() < 2 {
                    continue;
                }

                // Convert BlockIds to NodeIndices
                let target_indices: Vec<_> = targets
                    .iter()
                    .filter_map(|&id| find_node_by_id(cfg, id))
                    .collect();

                if let Some(otherwise_idx) = find_node_by_id(cfg, *otherwise) {
                    patterns.push(MatchPattern {
                        switch_node: node,
                        targets: target_indices,
                        otherwise: otherwise_idx,
                    });
                }
            }
        }
    }

    patterns
}

/// Helper: find NodeIndex by BlockId
fn find_node_by_id(cfg: &Cfg, id: BlockId) -> Option<NodeIndex> {
    cfg.node_indices()
        .find(|&n| cfg.node_weight(n).map_or(false, |b| b.id == id))
}

/// Get all branching patterns in the CFG
///
/// Returns both if/else and match patterns for a complete view
/// of control flow structure.
pub fn detect_all_patterns(cfg: &Cfg) -> (Vec<IfElsePattern>, Vec<MatchPattern>) {
    (
        detect_if_else_patterns(cfg),
        detect_match_patterns(cfg),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::{BasicBlock, BlockKind, EdgeType, Terminator};
    use petgraph::graph::DiGraph;

    /// Create a diamond pattern CFG (if/else)
    fn create_diamond_cfg() -> Cfg {
        let mut g = DiGraph::new();

        // Block 0: entry, goes to 1
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        // Block 1: if condition
        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![2], otherwise: 3 },
            source_location: None,
        });

        // Block 2: true branch
        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec!["true branch".to_string()],
            terminator: Terminator::Goto { target: 4 },
            source_location: None,
        });

        // Block 3: false branch
        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Normal,
            statements: vec!["false branch".to_string()],
            terminator: Terminator::Goto { target: 4 },
            source_location: None,
        });

        // Block 4: merge point
        let b4 = g.add_node(BasicBlock {
            id: 4,
            kind: BlockKind::Exit,
            statements: vec!["merge".to_string()],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b1, b3, EdgeType::FalseBranch);
        g.add_edge(b2, b4, EdgeType::Fallthrough);
        g.add_edge(b3, b4, EdgeType::Fallthrough);

        g
    }

    #[test]
    fn test_detect_if_else_diamond() {
        let cfg = create_diamond_cfg();
        let patterns = detect_if_else_patterns(&cfg);

        assert_eq!(patterns.len(), 1);

        let pattern = &patterns[0];
        assert_eq!(pattern.condition.index(), 1);
        assert_eq!(pattern.true_branch.index(), 2);
        assert_eq!(pattern.false_branch.index(), 3);
        assert_eq!(pattern.merge_point, Some(NodeIndex::new(4)));
        assert!(pattern.has_else());
    }

    #[test]
    fn test_classify_branch() {
        let cfg = create_diamond_cfg();

        assert_eq!(classify_branch(&cfg, NodeIndex::new(0)), BranchType::Linear);
        assert_eq!(classify_branch(&cfg, NodeIndex::new(1)), BranchType::Conditional);
        assert_eq!(classify_branch(&cfg, NodeIndex::new(2)), BranchType::Linear);
        assert_eq!(classify_branch(&cfg, NodeIndex::new(4)), BranchType::Linear);
    }

    #[test]
    fn test_detect_match_patterns() {
        let mut g = DiGraph::new();

        // Block 0: match with 3 branches
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![1, 2], otherwise: 3 },
            source_location: None,
        });

        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Exit,
            statements: vec!["case 1".to_string()],
            terminator: Terminator::Return,
            source_location: None,
        });

        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Exit,
            statements: vec!["case 2".to_string()],
            terminator: Terminator::Return,
            source_location: None,
        });

        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Exit,
            statements: vec!["default".to_string()],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::TrueBranch);
        g.add_edge(b0, b2, EdgeType::TrueBranch);
        g.add_edge(b0, b3, EdgeType::FalseBranch);

        let patterns = detect_match_patterns(&g);
        assert_eq!(patterns.len(), 1);

        let pattern = &patterns[0];
        assert_eq!(pattern.switch_node.index(), 0);
        assert_eq!(pattern.targets.len(), 2);
        assert_eq!(pattern.otherwise.index(), 3);
        assert_eq!(pattern.branch_count(), 3);
    }

    #[test]
    fn test_classify_multiway() {
        let mut g = DiGraph::new();

        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![1, 2], otherwise: 3 },
            source_location: None,
        });

        for i in 1..=3 {
            g.add_node(BasicBlock {
                id: i,
                kind: BlockKind::Exit,
                statements: vec![],
                terminator: Terminator::Return,
                source_location: None,
            });
        }

        for i in 1..=3 {
            g.add_edge(b0, NodeIndex::new(i), EdgeType::TrueBranch);
        }

        assert_eq!(classify_branch(&g, NodeIndex::new(0)), BranchType::MultiWay);
    }

    #[test]
    fn test_detect_all_patterns() {
        let mut g = DiGraph::new();

        // Create CFG with if/else and multi-way match

        // Entry
        let b0 = g.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        // If/else at block 1
        let b1 = g.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![2], otherwise: 3 },
            source_location: None,
        });

        // True branch (leads to match)
        let b2 = g.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec![],
            // Multi-way match with 2 targets (3 branches total)
            terminator: Terminator::SwitchInt { targets: vec![4, 5], otherwise: 6 },
            source_location: None,
        });

        // False branch
        let b3 = g.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 7 },
            source_location: None,
        });

        // Match branches
        let b4 = g.add_node(BasicBlock {
            id: 4,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 7 },
            source_location: None,
        });

        let b5 = g.add_node(BasicBlock {
            id: 5,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 7 },
            source_location: None,
        });

        let b6 = g.add_node(BasicBlock {
            id: 6,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 7 },
            source_location: None,
        });

        // Merge point
        let b7 = g.add_node(BasicBlock {
            id: 7,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        g.add_edge(b0, b1, EdgeType::Fallthrough);
        g.add_edge(b1, b2, EdgeType::TrueBranch);
        g.add_edge(b1, b3, EdgeType::FalseBranch);
        g.add_edge(b2, b4, EdgeType::TrueBranch);
        g.add_edge(b2, b5, EdgeType::TrueBranch);
        g.add_edge(b2, b6, EdgeType::FalseBranch);
        g.add_edge(b3, b7, EdgeType::Fallthrough);
        g.add_edge(b4, b7, EdgeType::Fallthrough);
        g.add_edge(b5, b7, EdgeType::Fallthrough);
        g.add_edge(b6, b7, EdgeType::Fallthrough);

        let (if_patterns, match_patterns) = detect_all_patterns(&g);

        // Should detect 1 if/else (at block 1) - all branches merge at 7
        assert_eq!(if_patterns.len(), 1);
        assert_eq!(if_patterns[0].condition.index(), 1);

        // Should detect 1 match (at block 2) - has 2 targets (3 branches)
        assert_eq!(match_patterns.len(), 1);
        assert_eq!(match_patterns[0].switch_node.index(), 2);
        assert_eq!(match_patterns[0].targets.len(), 2);
        assert_eq!(match_patterns[0].branch_count(), 3);
    }

    #[test]
    fn test_empty_cfg() {
        let cfg: Cfg = DiGraph::new();
        assert!(detect_if_else_patterns(&cfg).is_empty());
        assert!(detect_match_patterns(&cfg).is_empty());
    }

    #[test]
    fn test_linear_cfg_no_patterns() {
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

        assert!(detect_if_else_patterns(&g).is_empty());
        assert!(detect_match_patterns(&g).is_empty());
    }
}
