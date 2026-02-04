// CFG data structures shared by MIR and AST pipelines

pub mod analysis;
pub mod ast;
pub mod dominance_frontiers;
pub mod dominators;
pub mod edge;
pub mod export;
pub mod loops;
pub mod mir;
pub mod paths;
pub mod patterns;
pub mod post_dominators;
pub mod reachability;
pub mod source;
pub mod summary;

pub use analysis::{find_entry, find_exits};
pub use crate::storage::{load_cfg_from_db, resolve_function_name};
pub use dominance_frontiers::compute_dominance_frontiers;
pub use dominators::DominatorTree;
pub use post_dominators::PostDominatorTree;
pub use edge::EdgeType;
pub use export::{export_dot, export_json, CFGExport};
pub use loops::detect_natural_loops;
#[allow(unused_imports)] // Used in tests within the module
pub use paths::{Path, PathKind, PathLimits, enumerate_paths, enumerate_paths_cached, enumerate_paths_cached_with_context, enumerate_paths_with_context, EnumerationContext, get_or_enumerate_paths};
pub use patterns::{detect_if_else_patterns, detect_match_patterns};
pub use reachability::{find_reachable_from_block, compute_path_impact, PathImpact};
pub use summary::summarize_path;
pub use mir::ullbc_to_cfg;
pub use source::{CharonSpan, SourceLocation};

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
/// - "fallthrough" -> Single EdgeType::Fallthrough edge to next sequential block
/// - "conditional" -> Two edges: TrueBranch (next block), FalseBranch (block after that)
/// - "goto" -> Single EdgeType::Fallthrough edge to next sequential block
/// - "return" | "panic" -> No outgoing edges (exit block)
/// - "break" | "continue" -> No edges (loop control - handled in analysis phase)
/// - "call" -> EdgeType::Call edge to next sequential block
///
/// # Notes
///
/// - This is a simplified implementation that assumes sequential block ordering
/// - Future enhancement: use proper target resolution for goto/call terminators
/// - Loop back-edges will be detected during loop analysis phase
pub fn build_edges_from_terminators(
    graph: &mut Cfg,
    blocks: &[(i64, String, Option<String>, Option<i64>, Option<i64>,
              Option<i64>, Option<i64>, Option<i64>, Option<i64>)],
    _db_id_to_node: &HashMap<i64, usize>,
) -> Result<()> {
    use petgraph::graph::NodeIndex;

    // For each block, analyze terminator to find successors
    for (idx, (_, _kind, terminator_opt, _, _, _, _, _, _)) in blocks.iter().enumerate() {
        let terminator = terminator_opt.as_deref().unwrap_or("");

        match terminator {
            "fallthrough" => {
                // Edge to next sequential block
                if idx + 1 < blocks.len() {
                    graph.add_edge(
                        NodeIndex::new(idx),
                        NodeIndex::new(idx + 1),
                        EdgeType::Fallthrough,
                    );
                }
            }
            "conditional" => {
                // Two edges: true and false branches
                // True branch is next block (if), false is after that (else/end)
                if idx + 1 < blocks.len() {
                    graph.add_edge(
                        NodeIndex::new(idx),
                        NodeIndex::new(idx + 1),
                        EdgeType::TrueBranch,
                    );
                }
                if idx + 2 < blocks.len() {
                    graph.add_edge(
                        NodeIndex::new(idx),
                        NodeIndex::new(idx + 2),
                        EdgeType::FalseBranch,
                    );
                }
            }
            "goto" => {
                // Find target by analyzing control flow structure
                // For now, fallthrough to next (will be refined with proper target resolution)
                if idx + 1 < blocks.len() {
                    graph.add_edge(
                        NodeIndex::new(idx),
                        NodeIndex::new(idx + 1),
                        EdgeType::Fallthrough,
                    );
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
                if idx + 1 < blocks.len() {
                    graph.add_edge(
                        NodeIndex::new(idx),
                        NodeIndex::new(idx + 1),
                        EdgeType::Call,
                    );
                }
            }
            _ => {
                // Unknown terminator - no edge
            }
        }
    }
    Ok(())
}

/// Basic block in a CFG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicBlock {
    /// Unique identifier within the function
    pub id: BlockId,
    /// Block kind (entry, normal, exit)
    pub kind: BlockKind,
    /// Statements in this block (simplified for now)
    pub statements: Vec<String>,
    /// Terminator instruction
    pub terminator: Terminator,
    /// Source location for this block (if available)
    pub source_location: Option<SourceLocation>,
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
    Goto { target: BlockId },
    SwitchInt { targets: Vec<BlockId>, otherwise: BlockId },
    Return,
    Unreachable,
    Call { target: Option<BlockId>, unwind: Option<BlockId> },
    Abort(String),
}
