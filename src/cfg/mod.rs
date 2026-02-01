// CFG data structures shared by MIR and AST pipelines

pub mod analysis;
pub mod ast;
pub mod dominators;
pub mod edge;
pub mod export;
pub mod loops;
pub mod mir;
pub mod patterns;
pub mod post_dominators;
pub mod reachability;
pub mod source;

pub use analysis::{find_entry, find_exits, is_exit_block};
pub use dominators::{DominatorTree, compute_dominator_tree};
pub use post_dominators::{PostDominatorTree, compute_post_dominator_tree};
pub use ast::{ast_to_cfg, CFGBuilder};
pub use edge::{EdgeType, classify_terminator};
pub use export::{export_dot, export_json, CFGExport};
pub use loops::{detect_natural_loops, find_loop_headers, is_loop_header, NaturalLoop};
pub use patterns::{detect_if_else_patterns, detect_match_patterns, classify_branch, IfElsePattern, MatchPattern, BranchType, detect_all_patterns};
pub use reachability::{find_unreachable, find_reachable, is_reachable_from_entry, can_reach, can_reach_cached, ReachabilityCache};
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
