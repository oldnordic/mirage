// CFG data structures shared by MIR and AST pipelines

pub mod analysis;
pub mod ast;
pub mod coordinates;
pub mod diff;
pub mod dominance_frontiers;
pub mod dominators;
pub mod edge;
pub mod export;
pub mod git_utils;
pub mod hotpaths;
pub mod icfg;
pub mod loops;
pub mod paths;
pub mod patterns;
pub mod post_dominators;
pub mod reachability;
pub mod source;
pub mod summary;

// Re-export for public API convenience
pub use crate::storage::{
    load_cfg_from_db, resolve_function_name, resolve_function_name_with_file,
};
#[allow(unused_imports)]
pub use analysis::{find_entry, find_exits};

#[cfg(feature = "sqlite")]
pub use crate::storage::{load_cfg_from_db_with_conn, resolve_function_name_with_conn};
pub use dominance_frontiers::compute_dominance_frontiers;
pub use dominators::DominatorTree;
pub use edge::EdgeType;
pub use export::{export_dot, export_json, CFGExport};
#[allow(unused_imports)] // Public API re-export, used directly by consumers
pub use hotpaths::{compute_hot_paths, HotPath, HotpathsOptions};
pub use loops::detect_natural_loops;
#[allow(unused_imports)] // Used in tests within the module
pub use paths::{
    enumerate_paths, enumerate_paths_cached, enumerate_paths_cached_with_context,
    enumerate_paths_incremental, enumerate_paths_iterative, enumerate_paths_with_context,
    enumerate_paths_with_metadata, get_or_enumerate_paths, EnumerationContext, EnumerationStats,
    IncrementalPathsResult, LimitsHit, Path, PathEnumerationResult, PathKind, PathLimits,
};
pub use patterns::{detect_if_else_patterns, detect_match_patterns};
pub use post_dominators::PostDominatorTree;
pub use reachability::{compute_path_impact, find_reachable_from_block, PathImpact};
pub use source::SourceLocation;
pub use summary::summarize_path;

use anyhow::Result;
use petgraph::graph::DiGraph;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Control Flow Graph
pub type Cfg = DiGraph<BasicBlock, EdgeType>;

/// Build CFG edges from Magellan's terminator strings
///
/// This function constructs edges in memory by analyzing each block's terminator
/// and determining successor blocks. Per RESEARCH.md Pattern 2, edges are derived
/// from terminator data rather than queried from the database.
///
/// # Arguments
///
/// * `graph` - The CFG graph to add edges to (already populated with nodes)
/// * `blocks` - Block data from Magellan's cfg_blocks table
/// * `db_id_to_node` - Mapping from database block IDs to graph node indices
///
/// # Returns
///
/// * `Ok(())` - Edges constructed successfully
/// * `Err(...)` - Error if construction fails
///
/// # Edge Construction Rules
///
/// - "fallthrough" -> Single EdgeType::Fallthrough edge to next block by byte order
/// - "conditional" -> Two edges: TrueBranch (next block), FalseBranch (block after that)
/// - "goto" -> Single EdgeType::Fallthrough edge to next block by byte order
/// - "return" | "panic" -> No outgoing edges (exit block)
/// - "break" | "continue" -> No edges (loop control - handled in analysis phase)
/// - "call" -> EdgeType::Call edge to next block by byte order
///
/// # Notes
///
/// - Uses byte offsets to determine control flow order (not sequential indices)
/// - Blocks are sorted by byte_start to determine execution order
/// - Loop back-edges will be detected during loop analysis phase
pub fn build_edges_from_terminators(
    graph: &mut Cfg,
    blocks: &[(
        i64,
        String,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
    )],
    db_id_to_node: &HashMap<i64, usize>,
) -> Result<()> {
    use petgraph::graph::NodeIndex;

    // Sort blocks by byte_start to get execution order
    // This is crucial because block IDs are not necessarily in control flow order
    let mut blocks_with_idx: Vec<(
        usize,
        &(
            i64,
            String,
            Option<String>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
        ),
    )> = blocks.iter().enumerate().collect();
    blocks_with_idx.sort_by_key(|(_, (_, _, _, byte_start, _, _, _, _, _, _, _, _))| *byte_start);

    // Build a map from position in sorted order to node index
    let mut sorted_pos_to_node: HashMap<usize, usize> = HashMap::new();
    for (sorted_pos, (_original_idx, (db_id, _, _, _, _, _, _, _, _, _, _, _))) in
        blocks_with_idx.iter().enumerate()
    {
        if let Some(&node_idx) = db_id_to_node.get(db_id) {
            sorted_pos_to_node.insert(sorted_pos, node_idx);
        }
    }

    // For each block in sorted order, analyze terminator to find successors
    for (sorted_pos, (_original_idx, (_, _kind, terminator_opt, _, _, _, _, _, _, _, _, _))) in
        blocks_with_idx.iter().enumerate()
    {
        let terminator = terminator_opt.as_deref().unwrap_or("");
        let current_node = *sorted_pos_to_node.get(&sorted_pos).ok_or_else(|| {
            anyhow::anyhow!("Block at position {} not found in node map", sorted_pos)
        })?;

        match terminator {
            "fallthrough" | "goto" => {
                // Edge to next block in byte order
                if sorted_pos + 1 < blocks_with_idx.len() {
                    if let Some(&target_node) = sorted_pos_to_node.get(&(sorted_pos + 1)) {
                        graph.add_edge(
                            NodeIndex::new(current_node),
                            NodeIndex::new(target_node),
                            EdgeType::Fallthrough,
                        );
                    }
                }
            }
            "conditional" => {
                // Two edges: true and false branches
                // True branch is next block in byte order (if branch)
                if sorted_pos + 1 < blocks_with_idx.len() {
                    if let Some(&true_target) = sorted_pos_to_node.get(&(sorted_pos + 1)) {
                        graph.add_edge(
                            NodeIndex::new(current_node),
                            NodeIndex::new(true_target),
                            EdgeType::TrueBranch,
                        );
                    }
                }
                // False branch is block after next (else/end)
                if sorted_pos + 2 < blocks_with_idx.len() {
                    if let Some(&false_target) = sorted_pos_to_node.get(&(sorted_pos + 2)) {
                        graph.add_edge(
                            NodeIndex::new(current_node),
                            NodeIndex::new(false_target),
                            EdgeType::FalseBranch,
                        );
                    }
                }
            }
            "return" | "panic" => {
                // No outgoing edges (exit block)
            }
            "break" | "continue" => {
                // Loop exit/back edges - will need proper target resolution
                // For now, no edge (will be refined with loop analysis)
            }
            "call" => {
                // Function call - edge to next block (normal return path)
                if sorted_pos + 1 < blocks_with_idx.len() {
                    if let Some(&target_node) = sorted_pos_to_node.get(&(sorted_pos + 1)) {
                        graph.add_edge(
                            NodeIndex::new(current_node),
                            NodeIndex::new(target_node),
                            EdgeType::Call,
                        );
                    }
                }
            }
            _ => {
                // Unknown terminator - no edge
            }
        }
    }
    Ok(())
}

/// Build CFG edges from Magellan's cfg_edges table data
///
/// This function constructs edges in memory by reading the cfg_edges table,
/// which stores edges as vector indices within the function's block list.
///
/// # Arguments
///
/// * `graph` - The CFG graph to add edges to (already populated with nodes)
/// * `edges` - Edge rows from cfg_edges table: (source_idx, target_idx, edge_type)
/// * `index_to_node` - Mapping from vector index (0,1,2...) to graph node index
///
/// # Returns
///
/// * `Ok(())` - Edges constructed successfully
/// * `Err(...)`` - Error if index mapping fails
///
/// # Edge Type Mapping
///
/// | Magellan edge_type | Mirage EdgeType |
/// |-------------------|-----------------|
/// | fallthrough | Fallthrough |
/// | conditional_true | TrueBranch |
/// | conditional_false | FalseBranch |
/// | back_edge | LoopBack |
/// | call | Call |
/// | return | Return |
/// | jump | Fallthrough (break/continue fallback) |
pub fn build_edges_from_cfg_edges(
    graph: &mut Cfg,
    edges: &[(i64, i64, String)],
    index_to_node: &std::collections::HashMap<usize, usize>,
) -> Result<()> {
    use petgraph::graph::NodeIndex;

    for (source_idx, target_idx, edge_type_str) in edges {
        let source_node = *index_to_node
            .get(&(*source_idx as usize))
            .ok_or_else(|| anyhow::anyhow!("Source index {} not found in block map", source_idx))?;
        let target_node = *index_to_node
            .get(&(*target_idx as usize))
            .ok_or_else(|| anyhow::anyhow!("Target index {} not found in block map", target_idx))?;

        let edge_type = match edge_type_str.as_str() {
            "fallthrough" => EdgeType::Fallthrough,
            "conditional_true" => EdgeType::TrueBranch,
            "conditional_false" => EdgeType::FalseBranch,
            "back_edge" => EdgeType::LoopBack,
            "call" => EdgeType::Call,
            "return" => EdgeType::Return,
            "jump" => EdgeType::Fallthrough,
            _ => EdgeType::Fallthrough,
        };

        graph.add_edge(
            NodeIndex::new(source_node),
            NodeIndex::new(target_node),
            edge_type,
        );
    }

    Ok(())
}

/// Basic block in a CFG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicBlock {
    /// Unique identifier within the function (graph node index)
    pub id: BlockId,
    /// Original database block ID from cfg_blocks (for coverage/schema lookups)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_id: Option<i64>,
    /// Block kind (entry, normal, exit)
    pub kind: BlockKind,
    /// Statements in this block (simplified for now)
    pub statements: Vec<String>,
    /// Terminator instruction
    pub terminator: Terminator,
    /// Source location for this block (if available)
    pub source_location: Option<SourceLocation>,
    /// 4D Spatial Coordinates
    /// X coordinate: dominator depth (control flow hierarchy level)
    pub coord_x: i64,
    /// Y coordinate: loop nesting depth (how many loops surround this block)
    pub coord_y: i64,
    /// Z coordinate: branch distance from entry point
    pub coord_z: i64,
}

/// Block identifier
pub type BlockId = usize;

/// Block classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockKind {
    Entry,
    Normal,
    Exit,
}

/// Terminator instruction (simplified representation)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Terminator {
    Goto {
        target: BlockId,
    },
    SwitchInt {
        targets: Vec<BlockId>,
        otherwise: BlockId,
    },
    Return,
    Unreachable,
    Call {
        target: Option<BlockId>,
        unwind: Option<BlockId>,
    },
    Abort(String),
}
