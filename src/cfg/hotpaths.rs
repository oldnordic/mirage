//! Hot path detection using static CFG heuristics
//!
//! This module provides functionality for identifying the most-traversed
//! execution paths through a function's control flow graph using static
//! analysis heuristics rather than runtime profiling data.
//!
//! # Hot Path Detection
//!
//! Hot paths are execution paths that are likely to be frequently executed
//! based on static analysis of the control flow graph. The heuristics used
//! include:
//!
//! 1. **Loop nesting factor**: Paths within loops are 2^depth hotter
//! 2. **Dominator factor**: Dominant blocks (on all paths) increase hotness
//! 3. **Early exit penalty**: Paths that exit early are cooler
//!
//! These heuristics provide a reasonable approximation of runtime hotness
//! without requiring actual profiling data, making them useful for static
//! analysis tools.

use anyhow::Result;
use petgraph::algo::dominators;
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::{
    BasicBlock, BlockId, Cfg, EdgeType, Path, Terminator,
    loops::NaturalLoop,
};

/// Hot path with computed hotness score
///
/// Represents a single execution path through a function's CFG
/// that has been ranked by its estimated execution frequency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotPath {
    /// Unique identifier for this path
    pub path_id: String,
    /// Ordered block IDs in execution order
    pub blocks: Vec<BlockId>,
    /// Computed hotness score (higher = more frequently executed)
    pub hotness_score: f64,
    /// Human-readable rationale for the hotness score
    pub rationale: Vec<String>,
}

/// Hot path analysis options
///
/// Configuration options for hot path detection and ranking.
#[derive(Debug, Clone)]
pub struct HotpathsOptions {
    /// Maximum number of hot paths to return
    pub top_n: usize,
    /// Whether to include rationale in the output
    pub include_rationale: bool,
}

impl Default for HotpathsOptions {
    fn default() -> Self {
        Self {
            top_n: 10,
            include_rationale: true,
        }
    }
}

/// Compute hot paths for a function using static heuristics
///
/// # Heuristics
///
/// 1. **Loop nesting factor**: Paths in loops are 2^depth hotter
/// 2. **Dominator factor**: Dominant blocks (on all paths) increase hotness
/// 3. **Early exit penalty**: Paths that exit early are cooler
///
/// Formula: `hotness = loop_factor * dom_factor * exit_factor`
///
/// # Arguments
///
/// * `graph` - The control flow graph
/// * `paths` - All enumerated paths through the function
/// * `entry_id` - Entry block node index
/// * `natural_loops` - Detected natural loops in the CFG
/// * `options` - Analysis options
///
/// # Returns
///
/// * `Ok(Vec<HotPath>)` - Top N hottest paths with scores
///
/// # Examples
///
/// ```ignore
/// # use mirage::cfg::hotpaths::{compute_hot_paths, HotpathsOptions};
/// # use petgraph::graph::DiGraph;
/// # let graph = DiGraph::new();
/// # let paths = vec![];
/// # let entry = petgraph::graph::NodeIndex::new(0);
/// # let loops = vec![];
/// let options = HotpathsOptions::default();
/// let hot_paths = compute_hot_paths(&graph, &paths, entry, &loops, options)?;
/// ```
pub fn compute_hot_paths(
    graph: &Cfg,
    paths: &[Path],
    entry_id: NodeIndex,
    natural_loops: &[NaturalLoop],
    options: HotpathsOptions,
) -> Result<Vec<HotPath>> {
    // Compute dominator tree
    let dom_tree = dominators::simple_fast(graph, entry_id);

    // Get all dominant blocks (blocks that dominate all reachable blocks)
    let dominant_blocks: HashSet<NodeIndex> = dom_tree
        .dominators(entry_id)
        .iter()
        .copied()
        .collect();

    // Compute hotness for each path
    let mut hot_paths: Vec<HotPath> = paths.iter().map(|path| {
        let mut hotness = 1.0;
        let mut rationale = Vec::new();

        // Loop nesting factor
        let loop_depth = compute_loop_depth(natural_loops, &path.blocks);
        if loop_depth > 0 {
            let loop_factor = 2.0_f64.powi(loop_depth as i32);
            hotness *= loop_factor;
            rationale.push(format!("Loop depth {} (×{})", loop_depth, loop_factor));
        }

        // Dominator factor (count dominant blocks in path)
        let dominant_count = path.blocks
            .iter()
            .filter(|b| dominant_blocks.contains(&NodeIndex::new(**b)))
            .count();
        if dominant_count > 0 {
            let dom_factor = 1.0 + (dominant_count as f64 * 0.5);
            hotness *= dom_factor;
            rationale.push(format!("{} dominant blocks (×{})", dominant_count, dom_factor));
        }

        // Early exit penalty (path ends in return block)
        if let Some(&last_block) = path.blocks.last() {
            let last_node = NodeIndex::new(last_block);
            if let Some(block) = graph.node_weight(last_node) {
                if block.terminator == Terminator::Return {
                    // Check if this is "early" (shorter than average path length)
                    let avg_len = paths.iter()
                        .map(|p| p.blocks.len())
                        .sum::<usize>() as f64 / paths.len() as f64;
                    if path.blocks.len() < avg_len as usize && path.blocks.len() > 1 {
                        hotness *= 0.5;
                        rationale.push("Early exit (×0.5)".to_string());
                    }
                }
            }
        }

        HotPath {
            path_id: path.path_id.clone(),
            blocks: path.blocks.clone(),
            hotness_score: hotness,
            rationale: if options.include_rationale { rationale } else { vec![] },
        }
    }).collect();

    // Sort by hotness descending
    hot_paths.sort_by(|a, b| {
        b.hotness_score.partial_cmp(&a.hotness_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Return top N
    hot_paths.truncate(options.top_n);

    Ok(hot_paths)
}

/// Compute maximum loop nesting depth for a path
///
/// Returns the maximum nesting level of loops that contain
/// any block in the path.
///
/// # Arguments
///
/// * `loops` - All detected natural loops
/// * `path_blocks` - Block IDs in the path
///
/// # Returns
///
/// Maximum loop nesting depth (0 = no loops)
fn compute_loop_depth(loops: &[NaturalLoop], path_blocks: &[BlockId]) -> usize {
    path_blocks
        .iter()
        .map(|block_id| {
            loops
                .iter()
                .filter(|l| l.body.contains(&NodeIndex::new(*block_id)))
                .count()
        })
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::{BlockKind, EdgeType, Terminator};

    /// Create a simple CFG with a loop for testing
    fn create_loop_cfg() -> (Cfg, NodeIndex, Vec<NaturalLoop>) {
        let mut graph = DiGraph::new();

        // Block 0: entry
        let b0 = graph.add_node(BasicBlock {
            id: 0,
            kind: BlockKind::Entry,
            statements: vec![],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        // Block 1: loop header
        let b1 = graph.add_node(BasicBlock {
            id: 1,
            kind: BlockKind::Normal,
            statements: vec![],
            terminator: Terminator::SwitchInt { targets: vec![2], otherwise: 3 },
            source_location: None,
        });

        // Block 2: loop body
        let b2 = graph.add_node(BasicBlock {
            id: 2,
            kind: BlockKind::Normal,
            statements: vec!["loop body".to_string()],
            terminator: Terminator::Goto { target: 1 },
            source_location: None,
        });

        // Block 3: exit
        let b3 = graph.add_node(BasicBlock {
            id: 3,
            kind: BlockKind::Exit,
            statements: vec![],
            terminator: Terminator::Return,
            source_location: None,
        });

        graph.add_edge(b0, b1, EdgeType::Fallthrough);
        graph.add_edge(b1, b2, EdgeType::TrueBranch);
        graph.add_edge(b1, b3, EdgeType::FalseBranch);
        graph.add_edge(b2, b1, EdgeType::Fallthrough);

        // Create a mock loop
        let loop_body = [b1, b2].iter().copied().collect();
        let natural_loop = NaturalLoop {
            header: b1,
            back_edge: (b2, b1),
            body: loop_body,
        };

        (graph, b0, vec![natural_loop])
    }

    #[test]
    fn test_compute_loop_depth() {
        let (_graph, _entry, loops) = create_loop_cfg();

        // Path through loop: [0, 1, 2, 1, 3]
        let path_blocks = vec![0, 1, 2, 1, 3];
        let depth = compute_loop_depth(&loops, &path_blocks);
        assert_eq!(depth, 1, "Path through loop should have depth 1");

        // Path avoiding loop: [0, 1, 3]
        let path_blocks = vec![0, 1, 3];
        let depth = compute_loop_depth(&loops, &path_blocks);
        assert_eq!(depth, 1, "Path through header should have depth 1");
    }

    #[test]
    fn test_hotpaths_options_default() {
        let options = HotpathsOptions::default();
        assert_eq!(options.top_n, 10);
        assert!(options.include_rationale);
    }

    #[test]
    fn test_compute_hot_paths_empty() {
        let graph = DiGraph::new();
        let entry = NodeIndex::new(0);
        let paths: Vec<Path> = vec![];
        let loops: Vec<NaturalLoop> = vec![];

        let options = HotpathsOptions::default();
        let result = compute_hot_paths(&graph, &paths, entry, &loops, options);

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_compute_hot_paths_basic() {
        use crate::cfg::PathKind;

        let (graph, entry, loops) = create_loop_cfg();

        // Create some test paths
        let paths = vec![
            Path::new(vec![0, 1, 2, 1, 3], PathKind::Normal), // Through loop
            Path::new(vec![0, 1, 3], PathKind::Normal),       // Direct exit
        ];

        let options = HotpathsOptions {
            top_n: 10,
            include_rationale: true,
        };

        let result = compute_hot_paths(&graph, &paths, entry, &loops, options);

        assert!(result.is_ok());
        let hot_paths = result.unwrap();

        // The path through the loop should be hotter
        assert_eq!(hot_paths.len(), 2);
        assert!(hot_paths[0].hotness_score > hot_paths[1].hotness_score);

        // First path should have loop rationale
        assert!(!hot_paths[0].rationale.is_empty());
        assert!(hot_paths[0].rationale.iter().any(|r| r.contains("Loop")));
    }
}
