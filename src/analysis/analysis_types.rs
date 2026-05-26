use magellan::{Cycle, CycleKind};
use serde::Serialize;
use std::collections::HashMap;

use super::json_types::{DeadSymbolJson, SymbolInfoJson};

#[derive(Debug, Clone, Serialize)]
pub struct CycleInfo {
    pub members: Vec<String>,
    pub cycle_type: String,
    pub size: usize,
}

impl From<&Cycle> for CycleInfo {
    fn from(cycle: &Cycle) -> Self {
        let members: Vec<String> = cycle
            .members
            .iter()
            .map(|m| m.fqn.as_deref().unwrap_or("<unknown>").to_string())
            .collect();

        let cycle_type = match cycle.kind {
            CycleKind::MutualRecursion => "MutualRecursion",
            CycleKind::SelfLoop => "SelfLoop",
        };

        Self {
            members,
            cycle_type: cycle_type.to_string(),
            size: cycle.members.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LoopInfo {
    pub header: usize,
    pub back_edge_from: usize,
    pub body_size: usize,
    pub nesting_level: usize,
    pub body_blocks: Vec<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnhancedCycles {
    pub call_graph_cycles: Vec<CycleInfo>,
    pub function_loops: HashMap<String, Vec<LoopInfo>>,
    pub total_cycles: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnhancedDeadCode {
    pub uncalled_functions: Vec<DeadSymbolJson>,
    pub unreachable_blocks: HashMap<String, Vec<usize>>,
    pub total_dead_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnhancedBlastZone {
    pub target: String,
    pub forward_reachable: Vec<SymbolInfoJson>,
    pub backward_reachable: Vec<SymbolInfoJson>,
    pub path_impact: Option<PathImpactSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PathImpactSummary {
    pub path_id: Option<String>,
    pub path_length: usize,
    pub blocks_affected: Vec<usize>,
    pub unique_blocks_count: usize,
}
