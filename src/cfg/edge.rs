//! Edge type classification for CFG edges

use serde::{Deserialize, Serialize};

/// Type of control flow edge between basic blocks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeType {
    /// Conditional branch taken (true)
    TrueBranch,
    /// Conditional branch not taken (false)
    FalseBranch,
    /// Sequential fallthrough
    Fallthrough,
    /// Loop back to header
    LoopBack,
    /// Loop exit (condition false)
    LoopExit,
    /// Exception/unwind path
    Exception,
    /// Function call (normal return)
    Call,
    /// Function return (explicit)
    Return,
}

impl EdgeType {
    /// Color for DOT visualization
    pub fn dot_color(&self) -> &'static str {
        match self {
            EdgeType::TrueBranch => "green",
            EdgeType::FalseBranch => "red",
            EdgeType::Fallthrough => "black",
            EdgeType::LoopBack => "blue",
            EdgeType::LoopExit => "orange",
            EdgeType::Exception => "purple",
            EdgeType::Call => "gray",
            EdgeType::Return => "darkgray",
        }
    }

    /// Label for DOT visualization
    pub fn dot_label(&self) -> &'static str {
        match self {
            EdgeType::TrueBranch => "T",
            EdgeType::FalseBranch => "F",
            EdgeType::Fallthrough => "",
            EdgeType::LoopBack => "loop",
            EdgeType::LoopExit => "exit",
            EdgeType::Exception => "unwind",
            EdgeType::Call => "call",
            EdgeType::Return => "ret",
        }
    }
}

/// Classify edges from a simplified terminator
///
/// This is a simplified version for our core types.
/// MIR-specific classification will be added in 02-01.
pub fn classify_terminator(terminator: &crate::cfg::Terminator) -> Vec<(usize, EdgeType)> {
    use crate::cfg::Terminator::*;

    match terminator {
        Goto { target } => vec![(*target, EdgeType::Fallthrough)],
        SwitchInt { targets, otherwise } => {
            let mut edges = targets
                .iter()
                .map(|&t| (t, EdgeType::TrueBranch))
                .collect::<Vec<_>>();
            edges.push((*otherwise, EdgeType::FalseBranch));
            edges
        }
        Return => vec![],
        Unreachable => vec![],
        Call { target: Some(t), unwind } => {
            let mut edges = vec![(*t, EdgeType::Call)];
            if let Some(uw) = unwind {
                edges.push((*uw, EdgeType::Exception));
            }
            edges
        }
        Call { target: None, unwind: Some(uw) } => {
            vec![(*uw, EdgeType::Exception)]
        }
        Call { target: None, unwind: None } => vec![],
        Abort(_) => vec![],
    }
}
