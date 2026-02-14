//! CFG diff algorithms for comparing control flow graphs
//!
//! This module provides functionality to compare CFGs between two snapshots,
//! detecting added/deleted/modified blocks and edges with structural similarity scores.
//!
//! # Design
//!
//! - Uses petgraph for graph representation and algorithms
//! - Computes set differences for blocks and edges
//! - Calculates structural similarity based on graph edit distance
//! - Supports both SQLite and native-v3 backends via StorageTrait
//!
//! # Examples
//!
//! ```ignore
//! # use mirage_analyzer::cfg::diff::{compute_cfg_diff, CfgDiff};
//! # use mirage_analyzer::storage::Backend;
//! # use anyhow::Result;
//! # fn main() -> Result<()> {
//! let backend = Backend::detect_and_open("codegraph.db")?;
//! let diff = compute_cfg_diff(&backend, 123, "current", "current")?;
//! println!("Similarity: {:.1}%", diff.structural_similarity * 100.0);
//! # Ok(())
//! # }
//! ```

use anyhow::Result;
use petgraph::graph::DiGraph;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::storage::{Backend, CfgBlockData};

// ============================================================================
// Diff Output Structures
// ============================================================================

/// CFG diff result comparing two snapshots of a function
///
/// Contains all changes detected between the before and after snapshots,
/// including added/deleted/modified blocks and edges, plus a similarity score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfgDiff {
    /// Function entity ID
    pub function_id: i64,
    /// Function fully-qualified name
    pub function_name: String,
    /// Before snapshot identifier (transaction ID or "current")
    pub before_snapshot: String,
    /// After snapshot identifier (transaction ID or "current")
    pub after_snapshot: String,
    /// Blocks added in after snapshot
    pub added_blocks: Vec<BlockDiff>,
    /// Blocks deleted from before snapshot
    pub deleted_blocks: Vec<BlockDiff>,
    /// Blocks modified between snapshots
    pub modified_blocks: Vec<BlockChange>,
    /// Edges added in after snapshot
    pub added_edges: Vec<EdgeDiff>,
    /// Edges deleted from before snapshot
    pub deleted_edges: Vec<EdgeDiff>,
    /// Structural similarity score (0.0 = completely different, 1.0 = identical)
    pub structural_similarity: f64,
}

/// Representation of a single block for diff purposes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct BlockDiff {
    /// Block unique identifier
    pub block_id: i64,
    /// Block kind (entry, conditional, loop, match, return, etc.)
    pub kind: String,
    /// Terminator kind (fallthrough, conditional, return, etc.)
    pub terminator: String,
    /// Source location (file:line:col format)
    pub source_location: String,
}

/// Block change detected between two snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockChange {
    /// Block unique identifier
    pub block_id: i64,
    /// Block state in before snapshot
    pub before: BlockDiff,
    /// Block state in after snapshot
    pub after: BlockDiff,
    /// Type of change detected
    pub change_type: ChangeType,
}

/// Type of change detected for a block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    /// Terminator instruction changed
    TerminatorChanged { before: String, after: String },
    /// Source location changed (block moved within file)
    SourceLocationChanged,
    /// Both terminator and location changed
    BothChanged,
    /// Block's outgoing edges changed
    EdgesChanged,
}

/// Representation of a single edge for diff purposes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct EdgeDiff {
    /// Source block ID
    pub from_block: i64,
    /// Target block ID
    pub to_block: i64,
    /// Edge type (fallthrough, true_branch, false_branch, etc.)
    pub edge_type: String,
}

// ============================================================================
// Diff Computation
// ============================================================================

/// Compute CFG diff between two snapshots
///
/// This function compares the CFG of a function at two different snapshots,
/// detecting structural changes and computing a similarity score.
///
/// # Arguments
///
/// * `backend` - Storage backend for querying CFG data
/// * `function_id` - Function entity ID to compare
/// * `before_snapshot` - Snapshot identifier for "before" state (transaction ID or "current")
/// * `after_snapshot` - Snapshot identifier for "after" state (transaction ID or "current")
///
/// # Returns
///
/// * `Ok(CfgDiff)` - Diff result with all detected changes
/// * `Err(...)` - Error if query or computation fails
///
/// # Note
///
/// The current implementation queries the current state for both snapshots.
/// Future versions will support true snapshot-based comparison using SnapshotId.
pub fn compute_cfg_diff(
    backend: &Backend,
    function_id: i64,
    before_snapshot: &str,
    after_snapshot: &str,
) -> Result<CfgDiff> {
    // Query function name
    let function_name = backend
        .get_entity(function_id)
        .map(|e| e.name.clone())
        .unwrap_or_else(|| format!("<function_{}>", function_id));

    // Query CFG blocks
    // Note: Current implementation uses current state for both snapshots
    // TODO: Add snapshot_id parameter to StorageTrait::get_cfg_blocks
    let blocks_before = backend.get_cfg_blocks(function_id)?;
    let blocks_after = backend.get_cfg_blocks(function_id)?;

    // Build block maps with sequential IDs for diff
    let before_map: HashMap<i64, BlockDiff> = blocks_before
        .iter()
        .enumerate()
        .map(|(idx, b)| {
            (
                idx as i64,
                BlockDiff {
                    block_id: idx as i64,
                    kind: b.kind.clone(),
                    terminator: b.terminator.clone(),
                    source_location: format!(
                        "{}:{}:{}-{}:{}",
                        "", /* file could be added later */
                        b.start_line, b.start_col, b.end_line, b.end_col
                    ),
                },
            )
        })
        .collect();

    let after_map: HashMap<i64, BlockDiff> = blocks_after
        .iter()
        .enumerate()
        .map(|(idx, b)| {
            (
                idx as i64,
                BlockDiff {
                    block_id: idx as i64,
                    kind: b.kind.clone(),
                    terminator: b.terminator.clone(),
                    source_location: format!(
                        "{}:{}:{}-{}:{}",
                        "", /* file could be added later */
                        b.start_line, b.start_col, b.end_line, b.end_col
                    ),
                },
            )
        })
        .collect();

    // Compute block set differences
    let before_ids: HashSet<i64> = before_map.keys().copied().collect();
    let after_ids: HashSet<i64> = after_map.keys().copied().collect();

    let added_block_ids: Vec<_> = after_ids.difference(&before_ids).copied().collect();
    let deleted_block_ids: Vec<_> = before_ids.difference(&after_ids).copied().collect();
    let common_ids: Vec<_> = before_ids.intersection(&after_ids).copied().collect();

    // Build added blocks
    let added_blocks: Vec<BlockDiff> = added_block_ids
        .iter()
        .filter_map(|id| after_map.get(id).cloned())
        .collect();

    // Build deleted blocks
    let deleted_blocks: Vec<BlockDiff> = deleted_block_ids
        .iter()
        .filter_map(|id| before_map.get(id).cloned())
        .collect();

    // Detect modified blocks
    let modified_blocks: Vec<BlockChange> = common_ids
        .iter()
        .filter_map(|id| {
            let before = before_map.get(id)?;
            let after = after_map.get(id)?;

            // Check if anything changed
            if before == after {
                return None;
            }

            // Determine change type
            let terminator_changed = before.terminator != after.terminator;
            let location_changed = before.source_location != after.source_location;

            let change_type = match (terminator_changed, location_changed) {
                (true, false) => ChangeType::TerminatorChanged {
                    before: before.terminator.clone(),
                    after: after.terminator.clone(),
                },
                (false, true) => ChangeType::SourceLocationChanged,
                (true, true) => ChangeType::BothChanged,
                (false, false) => return None, // No change
            };

            Some(BlockChange {
                block_id: *id,
                before: before.clone(),
                after: after.clone(),
                change_type,
            })
        })
        .collect();

    // For edges, we derive them from block terminators
    // since we don't have explicit edge storage yet
    let (added_edges, deleted_edges) = compute_edge_diff(&before_map, &after_map)?;

    // Calculate structural similarity
    let total_changes = added_blocks.len()
        + deleted_blocks.len()
        + modified_blocks.len()
        + added_edges.len()
        + deleted_edges.len();

    let total_blocks_before = before_map.len().max(1);
    let structural_similarity = if total_changes == 0 {
        1.0
    } else {
        // Simple heuristic: 1 - (changes / (blocks * 2))
        // Factor of 2 accounts for edges contributing to structure
        let max_changes = total_blocks_before * 2;
        let similarity_ratio = 1.0 - (total_changes as f64 / max_changes.max(1) as f64);
        similarity_ratio.max(0.0)
    };

    Ok(CfgDiff {
        function_id,
        function_name,
        before_snapshot: before_snapshot.to_string(),
        after_snapshot: after_snapshot.to_string(),
        added_blocks,
        deleted_blocks,
        modified_blocks,
        added_edges,
        deleted_edges,
        structural_similarity,
    })
}

/// Compute edge differences between two CFG snapshots
///
/// Derives edges from terminator information and compares them.
/// Uses a simplified representation since explicit edge storage is not available.
///
/// # Returns
///
/// * `(added_edges, deleted_edges)` - Tuple of added and deleted edges
fn compute_edge_diff(
    before: &HashMap<i64, BlockDiff>,
    after: &HashMap<i64, BlockDiff>,
) -> Result<(Vec<EdgeDiff>, Vec<EdgeDiff>)> {
    // Derive edges from terminators
    let before_edges = derive_edges(before);
    let after_edges = derive_edges(after);

    let before_set: HashSet<_> = before_edges.iter().collect();
    let after_set: HashSet<_> = after_edges.iter().collect();

    let added: Vec<_> = after_set
        .difference(&before_set)
        .map(|e| (*e).clone())
        .collect();

    let deleted: Vec<_> = before_set
        .difference(&after_set)
        .map(|e| (*e).clone())
        .collect();

    Ok((added, deleted))
}

/// Derive edges from block terminators
///
/// This is a simplified implementation that creates edges based on
/// terminator kind and sequential block ordering.
/// Future versions will query actual edge data from the database.
fn derive_edges(blocks: &HashMap<i64, BlockDiff>) -> Vec<EdgeDiff> {
    let mut edges = Vec::new();
    let mut block_ids: Vec<_> = blocks.keys().copied().collect();
    block_ids.sort();

    for (idx, &block_id) in block_ids.iter().enumerate() {
        let block = match blocks.get(&block_id) {
            Some(b) => b,
            None => continue,
        };

        match block.terminator.as_str() {
            "fallthrough" | "goto" => {
                // Edge to next block
                if idx + 1 < block_ids.len() {
                    edges.push(EdgeDiff {
                        from_block: block_id,
                        to_block: block_ids[idx + 1],
                        edge_type: "fallthrough".to_string(),
                    });
                }
            }
            "conditional" => {
                // Two edges: true and false branches
                if idx + 1 < block_ids.len() {
                    edges.push(EdgeDiff {
                        from_block: block_id,
                        to_block: block_ids[idx + 1],
                        edge_type: "true_branch".to_string(),
                    });
                }
                if idx + 2 < block_ids.len() {
                    edges.push(EdgeDiff {
                        from_block: block_id,
                        to_block: block_ids[idx + 2],
                        edge_type: "false_branch".to_string(),
                    });
                }
            }
            "return" | "panic" => {
                // No outgoing edges
            }
            "call" => {
                // Edge to next block (return path)
                if idx + 1 < block_ids.len() {
                    edges.push(EdgeDiff {
                        from_block: block_id,
                        to_block: block_ids[idx + 1],
                        edge_type: "call".to_string(),
                    });
                }
            }
            _ => {
                // Unknown terminator - no edges
            }
        }
    }

    edges
}

/// Convert CFG blocks to petgraph for algorithmic operations
///
/// This creates a DiGraph representation suitable for petgraph algorithms
/// like isomorphism checking and edit distance computation.
///
/// # Arguments
///
/// * `blocks` - CFG block data
///
/// # Returns
///
/// * `DiGraph<i64, ()>` - Graph where nodes are block IDs and edges are unlabeled
pub fn blocks_to_petgraph(blocks: &[CfgBlockData]) -> DiGraph<i64, ()> {
    let mut graph = DiGraph::new();

    // Add nodes
    let mut node_indices = HashMap::new();
    for (idx, _block) in blocks.iter().enumerate() {
        let block_id = idx as i64;
        let node_idx = graph.add_node(block_id);
        node_indices.insert(block_id, node_idx);
    }

    // Add edges based on terminators
    for (idx, block) in blocks.iter().enumerate() {
        let from_id = idx as i64;
        let from_idx = node_indices[&from_id];

        match block.terminator.as_str() {
            "fallthrough" | "goto" => {
                if idx + 1 < blocks.len() {
                    let to_idx = node_indices[&((idx + 1) as i64)];
                    graph.add_edge(from_idx, to_idx, ());
                }
            }
            "conditional" => {
                if idx + 1 < blocks.len() {
                    let to_idx = node_indices[&((idx + 1) as i64)];
                    graph.add_edge(from_idx, to_idx, ());
                }
                if idx + 2 < blocks.len() {
                    let to_idx = node_indices[&((idx + 2) as i64)];
                    graph.add_edge(from_idx, to_idx, ());
                }
            }
            "call" => {
                if idx + 1 < blocks.len() {
                    let to_idx = node_indices[&((idx + 1) as i64)];
                    graph.add_edge(from_idx, to_idx, ());
                }
            }
            _ => {
                // No edges for return, panic, etc.
            }
        }
    }

    graph
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::CfgBlockData;

    #[test]
    fn test_block_diff_equality() {
        let block1 = BlockDiff {
            block_id: 1,
            kind: "entry".to_string(),
            terminator: "fallthrough".to_string(),
            source_location: "1:0-10:0".to_string(),
        };

        let block2 = BlockDiff {
            block_id: 1,
            kind: "entry".to_string(),
            terminator: "fallthrough".to_string(),
            source_location: "1:0-10:0".to_string(),
        };

        assert_eq!(block1, block2);
    }

    #[test]
    fn test_blocks_to_petgraph() {
        let blocks = vec![
            CfgBlockData {
                id: 0,
                kind: "entry".to_string(),
                terminator: "fallthrough".to_string(),
                byte_start: 0,
                byte_end: 10,
                start_line: 1,
                start_col: 0,
                end_line: 2,
                end_col: 0,
            },
            CfgBlockData {
                id: 1,
                kind: "normal".to_string(),
                terminator: "return".to_string(),
                byte_start: 10,
                byte_end: 20,
                start_line: 2,
                start_col: 0,
                end_line: 3,
                end_col: 0,
            },
        ];

        let graph = blocks_to_petgraph(&blocks);

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn test_derive_edges() {
        let mut blocks = HashMap::new();

        blocks.insert(
            0,
            BlockDiff {
                block_id: 0,
                kind: "entry".to_string(),
                terminator: "fallthrough".to_string(),
                source_location: "1:0-5:0".to_string(),
            },
        );

        blocks.insert(
            1,
            BlockDiff {
                block_id: 1,
                kind: "normal".to_string(),
                terminator: "return".to_string(),
                source_location: "5:0-10:0".to_string(),
            },
        );

        let edges = derive_edges(&blocks);

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from_block, 0);
        assert_eq!(edges[0].to_block, 1);
        assert_eq!(edges[0].edge_type, "fallthrough");
    }

    #[test]
    fn test_compute_edge_diff() {
        let mut before = HashMap::new();
        before.insert(
            0,
            BlockDiff {
                block_id: 0,
                kind: "entry".to_string(),
                terminator: "fallthrough".to_string(),
                source_location: "1:0-5:0".to_string(),
            },
        );
        // Need a second block for fallthrough to target
        before.insert(
            1,
            BlockDiff {
                block_id: 1,
                kind: "normal".to_string(),
                terminator: "return".to_string(),
                source_location: "5:0-10:0".to_string(),
            },
        );

        let mut after = HashMap::new();
        after.insert(
            0,
            BlockDiff {
                block_id: 0,
                kind: "entry".to_string(),
                terminator: "return".to_string(), // Changed: no more fallthrough
                source_location: "1:0-5:0".to_string(),
            },
        );
        // Keep the second block
        after.insert(
            1,
            BlockDiff {
                block_id: 1,
                kind: "normal".to_string(),
                terminator: "return".to_string(),
                source_location: "5:0-10:0".to_string(),
            },
        );

        let (added, deleted) = compute_edge_diff(&before, &after).unwrap();

        // Before had an edge (0->1 fallthrough), after has none
        assert_eq!(added.len(), 0);
        assert_eq!(deleted.len(), 1);
    }
}
