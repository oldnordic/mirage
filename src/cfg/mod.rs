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
pub use paths::{Path, PathKind, PathLimits, enumerate_paths, enumerate_paths_with_context, EnumerationContext, get_or_enumerate_paths};
pub use patterns::{detect_if_else_patterns, detect_match_patterns};
pub use reachability::{find_reachable_from_block, compute_path_impact, PathImpact};
pub use summary::summarize_path;
pub use mir::ullbc_to_cfg;
pub use source::{CharonSpan, SourceLocation};

use petgraph::graph::DiGraph;
use serde::{Deserialize, Serialize};

/// Control Flow Graph
pub type Cfg = DiGraph<BasicBlock, EdgeType>;

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
