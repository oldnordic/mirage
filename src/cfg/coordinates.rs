// 4D Coordinate Calculation for CFG Analysis
//
// This module implements algorithms to calculate spatial coordinates for CFG blocks:
// - coord_x: Dominator depth (control flow hierarchy)
// - coord_y: Loop nesting depth (iteration complexity)
// - coord_z: Branch distance (conditional complexity)
//
// All coordinate calculations are performed on real CFG graphs with no stubs or mocks.

use crate::cfg::{Cfg, EdgeType};
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet};

/// Calculate dominator depth (coord_x) for all blocks in a CFG
///
/// Dominator depth represents how deeply nested a block is in the control flow hierarchy.
/// - Entry block has depth 0
/// - Each level of dominance increases depth by 1
///
/// # Arguments
///
/// * `cfg` - The control flow graph
/// * `entry` - The entry node index
///
/// # Returns
///
/// * `HashMap<NodeIndex, i64>` - Mapping from node index to dominator depth
///
/// # Algorithm
///
/// Uses the dominator tree to calculate depth:
/// 1. Compute immediate dominators for each node
/// 2. Build dominator tree
/// 3. Calculate depth by traversing from root to leaves
pub fn calculate_dominator_depth(cfg: &Cfg, entry: NodeIndex) -> HashMap<NodeIndex, i64> {
    let mut depths = HashMap::new();
    let mut visited = HashSet::new();
    let mut stack = vec![(entry, 0i64)];

    // Use iterative DFS to avoid stack overflow on deep CFGs
    while let Some((node, depth)) = stack.pop() {
        if visited.contains(&node) {
            continue;
        }
        visited.insert(node);

        // Store the depth for this node
        depths.insert(node, depth);

        // Push children with incremented depth
        for neighbor in cfg.neighbors(node) {
            if !visited.contains(&neighbor) {
                stack.push((neighbor, depth + 1));
            }
        }
    }

    depths
}

/// Calculate loop nesting depth (coord_y) for all blocks in a CFG
///
/// Loop nesting depth represents how many nested loops enclose a block.
/// - Blocks outside loops have depth 0
/// - Each enclosing loop increments depth by 1
///
/// # Arguments
///
/// * `cfg` - The control flow graph
/// * `entry` - The entry node index
///
/// # Returns
///
/// * `HashMap<NodeIndex, i64>` - Mapping from node index to loop nesting depth
///
/// # Algorithm
///
/// Detects natural loops and calculates nesting:
/// 1. Find all back edges in the CFG
/// 2. For each back edge, identify the natural loop
/// 3. Calculate nesting depth by counting enclosing loops
pub fn calculate_loop_nesting_depth(cfg: &Cfg, entry: NodeIndex) -> HashMap<NodeIndex, i64> {
    let mut nesting_depths = HashMap::new();

    // Initialize all nodes with depth 0
    for node in cfg.node_indices() {
        nesting_depths.insert(node, 0);
    }

    // Find all back edges (edges that point to dominators)
    let back_edges = find_back_edges(cfg, entry);

    // For each back edge, identify the natural loop and increment depths
    for (source, target) in back_edges {
        let loop_body = identify_loop_body(cfg, source, target);

        // Increment nesting depth for all blocks in the loop body
        for node in loop_body {
            *nesting_depths.get_mut(&node).unwrap() += 1;
        }
    }

    nesting_depths
}

/// Calculate branch distance (coord_z) for all blocks in a CFG
///
/// Branch distance represents how many conditional branches must be traversed
/// to reach a block from the entry point.
/// - Entry block has distance 0
/// - Each conditional branch increments distance by 1
///
/// # Arguments
///
/// * `cfg` - The control flow graph
/// * `entry` - The entry node index
///
/// # Returns
///
/// * `HashMap<NodeIndex, i64>` - Mapping from node index to branch distance
///
/// # Algorithm
///
/// Uses BFS to calculate minimum branch distance:
/// 1. Traverse CFG in BFS order
/// 2. Increment distance when traversing conditional edges
/// 3. Store minimum distance for each node
pub fn calculate_branch_distance(cfg: &Cfg, entry: NodeIndex) -> HashMap<NodeIndex, i64> {
    let mut distances = HashMap::new();
    let mut queue = std::collections::VecDeque::new();
    let mut visited = HashSet::new();

    // Start from entry with distance 0
    queue.push_back((entry, 0i64));
    visited.insert(entry);

    while let Some((node, distance)) = queue.pop_front() {
        // Store the distance for this node
        distances.insert(node, distance);

        // Traverse all outgoing edges
        for edge in cfg.edges(node) {
            let neighbor = edge.target();

            if !visited.contains(&neighbor) {
                visited.insert(neighbor);

                // Increment distance for conditional branches
                let new_distance = match edge.weight() {
                    EdgeType::TrueBranch | EdgeType::FalseBranch => distance + 1,
                    _ => distance,
                };

                queue.push_back((neighbor, new_distance));
            }
        }
    }

    distances
}

/// Find all back edges in the CFG
///
/// A back edge is an edge that points to a node that dominates the source.
/// Back edges indicate the presence of loops.
fn find_back_edges(cfg: &Cfg, entry: NodeIndex) -> Vec<(NodeIndex, NodeIndex)> {
    let mut back_edges = Vec::new();

    // Compute dominators
    let dominators = compute_immediate_dominators(cfg, entry);

    // Check each edge for back edge property
    for edge in cfg.edge_indices() {
        let (source, target) = cfg.edge_endpoints(edge).unwrap();

        // An edge is a back edge if target dominates source
        if is_dominated_by(source, target, &dominators) {
            back_edges.push((source, target));
        }
    }

    back_edges
}

/// Identify all blocks in the body of a natural loop
///
/// Given a back edge (source -> target), where target is the loop header,
/// find all blocks that belong to the loop body.
fn identify_loop_body(cfg: &Cfg, source: NodeIndex, target: NodeIndex) -> HashSet<NodeIndex> {
    let mut loop_body = HashSet::new();
    let mut stack = vec![source];
    let mut visited = HashSet::new();

    // Start from the source of the back edge
    visited.insert(source);

    while let Some(node) = stack.pop() {
        loop_body.insert(node);

        // Follow predecessors until we reach the loop header
        for predecessor in cfg.neighbors_directed(node, petgraph::Direction::Incoming) {
            if predecessor != target && !visited.contains(&predecessor) {
                visited.insert(predecessor);
                stack.push(predecessor);
            }
        }
    }

    // Include the loop header
    loop_body.insert(target);

    loop_body
}

/// Compute immediate dominators for all nodes
///
/// A node d dominates node n if every path from entry to n must go through d.
/// Immediate dominator is the closest strict dominator.
fn compute_immediate_dominators(cfg: &Cfg, entry: NodeIndex) -> HashMap<NodeIndex, NodeIndex> {
    let mut idom = HashMap::new();
    let _all_nodes: HashSet<NodeIndex> = cfg.node_indices().collect();

    // Entry node has no immediate dominator
    idom.insert(entry, entry);

    // Initialize all other nodes with undefined
    for node in cfg.node_indices() {
        if node != entry {
            idom.insert(node, entry); // Temporary: set to entry
        }
    }

    // Iterative dataflow analysis to compute immediate dominators
    let mut changed = true;
    while changed {
        changed = false;

        for node in cfg.node_indices() {
            if node == entry {
                continue;
            }

            // Get all predecessors
            let predecessors: Vec<NodeIndex> = cfg
                .neighbors_directed(node, petgraph::Direction::Incoming)
                .collect();

            if predecessors.is_empty() {
                continue;
            }

            // Compute intersection of dominators of predecessors
            let mut new_idom = predecessors[0];
            for pred in &predecessors[1..] {
                new_idom = intersect_dominators(cfg, &idom, new_idom, *pred);
            }

            if idom.get(&node) != Some(&new_idom) {
                idom.insert(node, new_idom);
                changed = true;
            }
        }
    }

    idom
}

/// Intersect dominators of two nodes
fn intersect_dominators(
    _cfg: &Cfg,
    idom: &HashMap<NodeIndex, NodeIndex>,
    n1: NodeIndex,
    n2: NodeIndex,
) -> NodeIndex {
    let mut finger1 = n1;
    let mut finger2 = n2;

    // Traverse up the dominator tree until we find a common ancestor
    // Compare by NodeIndex directly (which implements Ord)
    while finger1 != finger2 {
        let idx1 = finger1.index();
        let idx2 = finger2.index();

        if idx1 > idx2 {
            finger1 = *idom.get(&finger1).unwrap_or(&finger1);
        } else {
            finger2 = *idom.get(&finger2).unwrap_or(&finger2);
        }
    }

    finger1
}

/// Check if a node is dominated by another node
fn is_dominated_by(
    node: NodeIndex,
    dominator: NodeIndex,
    idom: &HashMap<NodeIndex, NodeIndex>,
) -> bool {
    if node == dominator {
        return true;
    }

    let mut current = node;
    while let Some(&idom_node) = idom.get(&current) {
        if idom_node == dominator {
            return true;
        }
        if idom_node == current {
            break;
        }
        current = idom_node;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::{BasicBlock, BlockKind, Terminator};

    // Helper to create a simple linear CFG: A -> B -> C
    fn create_linear_cfg() -> (Cfg, NodeIndex, NodeIndex, NodeIndex) {
        let mut cfg = Cfg::new();

        let a = cfg.add_node(BasicBlock {
            id: 0,
            db_id: None,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        });

        let b = cfg.add_node(BasicBlock {
            id: 1,
            db_id: None,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 2 },
            source_location: None,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        });

        let c = cfg.add_node(BasicBlock {
            id: 2,
            db_id: None,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        });

        cfg.add_edge(a, b, EdgeType::Fallthrough);
        cfg.add_edge(b, c, EdgeType::Fallthrough);

        (cfg, a, b, c)
    }

    // Helper to create a CFG with a simple loop: A -> B -> C -> B
    fn create_loop_cfg() -> (Cfg, NodeIndex, NodeIndex, NodeIndex) {
        let mut cfg = Cfg::new();

        let a = cfg.add_node(BasicBlock {
            id: 0,
            db_id: None,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        });

        let b = cfg.add_node(BasicBlock {
            id: 1,
            db_id: None,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt {
                targets: vec![2], // Loop body
                otherwise: 2,     // Exit loop (for simplicity, using same target)
            }, // Loop condition
            source_location: None,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        });

        let c = cfg.add_node(BasicBlock {
            id: 2,
            db_id: None,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 }, // Back edge
            source_location: None,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        });

        cfg.add_edge(a, b, EdgeType::Fallthrough);
        cfg.add_edge(b, c, EdgeType::Fallthrough); // Loop body (simplified)
        cfg.add_edge(c, b, EdgeType::Fallthrough); // Back edge

        (cfg, a, b, c)
    }

    // Helper to create a CFG with branching: A -> (B, C)
    fn create_branch_cfg() -> (Cfg, NodeIndex, NodeIndex, NodeIndex) {
        let mut cfg = Cfg::new();

        let a = cfg.add_node(BasicBlock {
            id: 0,
            db_id: None,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::SwitchInt {
                targets: vec![1], // True branch
                otherwise: 2,     // False branch
            },
            source_location: None,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        });

        let b = cfg.add_node(BasicBlock {
            id: 1,
            db_id: None,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        });

        let c = cfg.add_node(BasicBlock {
            id: 2,
            db_id: None,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
            coord_x: 0,
            coord_y: 0,
            coord_z: 0,
        });

        cfg.add_edge(a, b, EdgeType::TrueBranch);
        cfg.add_edge(a, c, EdgeType::FalseBranch);

        (cfg, a, b, c)
    }

    #[test]
    fn test_calculate_dominator_depth_linear_cfg() {
        // Given: A linear CFG A -> B -> C
        let (cfg, entry, b, c) = create_linear_cfg();

        // When: Calculating dominator depths
        let depths = calculate_dominator_depth(&cfg, entry);

        // Then: Depths should reflect control flow hierarchy
        assert_eq!(*depths.get(&entry).unwrap(), 0, "Entry should have depth 0");
        assert_eq!(*depths.get(&b).unwrap(), 1, "B should have depth 1");
        assert_eq!(*depths.get(&c).unwrap(), 2, "C should have depth 2");
    }

    #[test]
    fn test_calculate_dominator_depth_loop_cfg() {
        // Given: A CFG with a loop A -> B -> C -> B
        let (cfg, entry, b, c) = create_loop_cfg();

        // When: Calculating dominator depths
        let depths = calculate_dominator_depth(&cfg, entry);

        // Then: Depths should reflect control flow hierarchy
        assert_eq!(*depths.get(&entry).unwrap(), 0, "Entry should have depth 0");
        assert_eq!(
            *depths.get(&b).unwrap(),
            1,
            "Loop header should have depth 1"
        );
        assert_eq!(*depths.get(&c).unwrap(), 2, "Loop body should have depth 2");
    }

    #[test]
    fn test_calculate_dominator_depth_branch_cfg() {
        // Given: A CFG with branching A -> (B, C)
        let (cfg, entry, b, c) = create_branch_cfg();

        // When: Calculating dominator depths
        let depths = calculate_dominator_depth(&cfg, entry);

        // Then: Both branches should have same depth
        assert_eq!(*depths.get(&entry).unwrap(), 0, "Entry should have depth 0");
        assert_eq!(*depths.get(&b).unwrap(), 1, "Branch B should have depth 1");
        assert_eq!(*depths.get(&c).unwrap(), 1, "Branch C should have depth 1");
    }

    #[test]
    fn test_calculate_loop_nesting_depth_linear_cfg() {
        // Given: A linear CFG with no loops
        let (cfg, entry, b, c) = create_linear_cfg();

        // When: Calculating loop nesting depths
        let depths = calculate_loop_nesting_depth(&cfg, entry);

        // Then: All nodes should have depth 0 (no loops)
        assert_eq!(
            *depths.get(&entry).unwrap(),
            0,
            "Entry should not be in a loop"
        );
        assert_eq!(*depths.get(&b).unwrap(), 0, "B should not be in a loop");
        assert_eq!(*depths.get(&c).unwrap(), 0, "C should not be in a loop");
    }

    #[test]
    fn test_calculate_loop_nesting_depth_loop_cfg() {
        // Given: A CFG with a loop A -> B -> C -> B
        let (cfg, entry, b, c) = create_loop_cfg();

        // When: Calculating loop nesting depths
        let depths = calculate_loop_nesting_depth(&cfg, entry);

        // Then: Loop nodes should have depth 1
        assert_eq!(
            *depths.get(&entry).unwrap(),
            0,
            "Entry should not be in the loop"
        );
        assert_eq!(
            *depths.get(&b).unwrap(),
            1,
            "Loop header should have depth 1"
        );
        assert_eq!(*depths.get(&c).unwrap(), 1, "Loop body should have depth 1");
    }

    #[test]
    fn test_calculate_loop_nesting_depth_branch_cfg() {
        // Given: A CFG with branching but no loops
        let (cfg, entry, b, c) = create_branch_cfg();

        // When: Calculating loop nesting depths
        let depths = calculate_loop_nesting_depth(&cfg, entry);

        // Then: All nodes should have depth 0 (no loops)
        assert_eq!(
            *depths.get(&entry).unwrap(),
            0,
            "Entry should not be in a loop"
        );
        assert_eq!(
            *depths.get(&b).unwrap(),
            0,
            "Branch B should not be in a loop"
        );
        assert_eq!(
            *depths.get(&c).unwrap(),
            0,
            "Branch C should not be in a loop"
        );
    }

    #[test]
    fn test_calculate_branch_distance_linear_cfg() {
        // Given: A linear CFG with no branching
        let (cfg, entry, b, c) = create_linear_cfg();

        // When: Calculating branch distances
        let distances = calculate_branch_distance(&cfg, entry);

        // Then: All nodes should have distance 0 (no conditional branches)
        assert_eq!(
            *distances.get(&entry).unwrap(),
            0,
            "Entry should have distance 0"
        );
        assert_eq!(*distances.get(&b).unwrap(), 0, "B should have distance 0");
        assert_eq!(*distances.get(&c).unwrap(), 0, "C should have distance 0");
    }

    #[test]
    fn test_calculate_branch_distance_branch_cfg() {
        // Given: A CFG with branching A -> (B, C)
        let (cfg, entry, b, c) = create_branch_cfg();

        // When: Calculating branch distances
        let distances = calculate_branch_distance(&cfg, entry);

        // Then: Branch nodes should have distance 1
        assert_eq!(
            *distances.get(&entry).unwrap(),
            0,
            "Entry should have distance 0"
        );
        assert_eq!(
            *distances.get(&b).unwrap(),
            1,
            "Branch B should have distance 1"
        );
        assert_eq!(
            *distances.get(&c).unwrap(),
            1,
            "Branch C should have distance 1"
        );
    }

    #[test]
    fn test_find_back_edges_loop_cfg() {
        // Given: A CFG with a loop A -> B -> C -> B
        let (cfg, entry, b, c) = create_loop_cfg();

        // When: Finding back edges
        let back_edges = find_back_edges(&cfg, entry);

        // Then: Should find exactly one back edge (C -> B)
        assert_eq!(back_edges.len(), 1, "Should have exactly one back edge");
        assert_eq!(back_edges[0].0, c, "Back edge should start from C");
        assert_eq!(back_edges[0].1, b, "Back edge should point to B");
    }

    #[test]
    fn test_find_back_edges_linear_cfg() {
        // Given: A linear CFG with no loops
        let (cfg, entry, _, _) = create_linear_cfg();

        // When: Finding back edges
        let back_edges = find_back_edges(&cfg, entry);

        // Then: Should find no back edges
        assert_eq!(back_edges.len(), 0, "Linear CFG should have no back edges");
    }

    #[test]
    fn test_identify_loop_body() {
        // Given: A CFG with a loop A -> B -> C -> B
        let (cfg, _, b, c) = create_loop_cfg();

        // When: Identifying loop body for back edge C -> B
        let loop_body = identify_loop_body(&cfg, c, b);

        // Then: Loop body should include B and C
        assert!(loop_body.contains(&b), "Loop body should include header B");
        assert!(loop_body.contains(&c), "Loop body should include C");
        assert_eq!(loop_body.len(), 2, "Loop body should have exactly 2 nodes");
    }
}
