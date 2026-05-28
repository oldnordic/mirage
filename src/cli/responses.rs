use crate::analysis::DeadSymbolJson;

#[derive(serde::Serialize)]
pub(crate) struct PathsResponse {
    pub function: String,
    pub total_paths: usize,
    pub error_paths: usize,
    pub paths: Vec<PathSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct PathBlock {
    pub block_id: usize,
    pub terminator: String,
}

#[derive(serde::Serialize)]
pub(crate) struct SourceRange {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct PathSummary {
    pub path_id: String,
    pub kind: String,
    pub length: usize,
    pub blocks: Vec<PathBlock>,
    pub summary: Option<String>,
    pub source_range: Option<SourceRange>,
}

impl From<crate::cfg::Path> for PathSummary {
    fn from(path: crate::cfg::Path) -> Self {
        let length = path.len();
        let blocks: Vec<PathBlock> = path
            .blocks
            .into_iter()
            .map(|block_id| PathBlock {
                block_id,
                terminator: "Unknown".to_string(),
            })
            .collect();

        Self {
            path_id: path.path_id,
            kind: format!("{:?}", path.kind),
            length,
            blocks,
            summary: None,
            source_range: None,
        }
    }
}

impl PathSummary {
    pub fn from_with_cfg(path: crate::cfg::Path, cfg: &crate::cfg::Cfg) -> Self {
        use crate::cfg::summarize_path;

        let summary = Some(summarize_path(cfg, &path));

        let blocks: Vec<PathBlock> = path
            .blocks
            .iter()
            .map(|&block_id| {
                let node_idx = cfg.node_indices().find(|&n| cfg[n].id == block_id);

                let terminator = match node_idx {
                    Some(idx) => format!("{:?}", cfg[idx].terminator),
                    None => "Unknown".to_string(),
                };

                PathBlock {
                    block_id,
                    terminator,
                }
            })
            .collect();

        let source_range = Self::calculate_source_range(&path, cfg);

        let length = path.len();

        Self {
            path_id: path.path_id,
            kind: format!("{:?}", path.kind),
            length,
            summary,
            source_range,
            blocks,
        }
    }

    fn calculate_source_range(
        path: &crate::cfg::Path,
        cfg: &crate::cfg::Cfg,
    ) -> Option<SourceRange> {
        let first_loc = path
            .blocks
            .first()
            .and_then(|&bid| cfg.node_indices().find(|&n| cfg[n].id == bid))
            .and_then(|idx| cfg[idx].source_location.clone());

        let last_loc = path
            .blocks
            .last()
            .and_then(|&bid| cfg.node_indices().find(|&n| cfg[n].id == bid))
            .and_then(|idx| cfg[idx].source_location.clone());

        match (first_loc, last_loc) {
            (Some(first), Some(last)) => Some(SourceRange {
                file_path: first.file_path.to_string_lossy().to_string(),
                start_line: first.start_line,
                end_line: last.end_line,
            }),
            _ => None,
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct DominanceResponse {
    pub function: String,
    pub kind: String,
    pub root: Option<usize>,
    pub dominance_tree: Vec<DominatorEntry>,
    pub must_pass_through: Option<MustPassThroughResult>,
}

#[derive(serde::Serialize)]
pub(crate) struct DominatorEntry {
    pub block: usize,
    pub immediate_dominator: Option<usize>,
    pub dominated: Vec<usize>,
}

#[derive(serde::Serialize)]
pub(crate) struct MustPassThroughResult {
    pub block: usize,
    pub must_pass: Vec<usize>,
}

#[derive(serde::Serialize)]
pub(crate) struct InterProceduralDominanceResponse {
    pub function: String,
    pub kind: String,
    pub dominator_count: usize,
    pub dominators: Vec<String>,
}

#[derive(serde::Serialize)]
pub(crate) struct UnreachableResponse {
    pub function: String,
    pub total_functions: usize,
    pub functions_with_unreachable: usize,
    pub unreachable_count: usize,
    pub blocks: Vec<UnreachableBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncalled_functions: Option<Vec<DeadSymbolJson>>,
}

#[derive(serde::Serialize, Clone)]
pub(crate) struct IncomingEdge {
    pub from_block: usize,
    pub edge_type: String,
}

#[derive(serde::Serialize, Clone)]
pub(crate) struct UnreachableBlock {
    pub block_id: usize,
    pub kind: String,
    pub statements: Vec<String>,
    pub terminator: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub incoming_edges: Vec<IncomingEdge>,
}

#[derive(serde::Serialize)]
pub(crate) struct VerifyResult {
    pub path_id: String,
    pub valid: bool,
    pub found_in_cache: bool,
    pub function_id: Option<i64>,
    pub reason: String,
    pub current_paths: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct LoopsResponse {
    pub function: String,
    pub loop_count: usize,
    pub loops: Vec<LoopInfo>,
}

#[derive(serde::Serialize)]
pub(crate) struct LoopInfo {
    pub header: usize,
    pub back_edge_from: usize,
    pub body_size: usize,
    pub nesting_level: usize,
    pub body_blocks: Vec<usize>,
}

#[derive(serde::Serialize)]
pub(crate) struct PatternsResponse {
    pub function: String,
    pub if_else_count: usize,
    pub match_count: usize,
    pub if_else_patterns: Vec<IfElseInfo>,
    pub match_patterns: Vec<MatchInfo>,
}

#[derive(serde::Serialize)]
pub(crate) struct IfElseInfo {
    pub condition_block: usize,
    pub true_branch: usize,
    pub false_branch: usize,
    pub merge_point: Option<usize>,
    pub has_else: bool,
}

#[derive(serde::Serialize)]
pub(crate) struct MatchInfo {
    pub switch_block: usize,
    pub branch_count: usize,
    pub targets: Vec<usize>,
    pub otherwise: usize,
}

#[derive(serde::Serialize)]
pub(crate) struct FrontiersResponse {
    pub function: String,
    pub nodes_with_frontiers: usize,
    pub frontiers: Vec<NodeFrontier>,
}

#[derive(serde::Serialize)]
pub(crate) struct NodeFrontier {
    pub node: usize,
    pub frontier_set: Vec<usize>,
}

#[derive(serde::Serialize)]
pub(crate) struct IteratedFrontierResponse {
    pub function: String,
    pub iterated_frontier: Vec<usize>,
}

#[derive(serde::Serialize)]
pub(crate) struct BlockImpactResponse {
    pub function: String,
    pub block_id: usize,
    pub reachable_blocks: Vec<usize>,
    pub reachable_count: usize,
    pub max_depth: usize,
    pub has_cycles: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forward_impact: Option<Vec<CallGraphSymbol>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backward_impact: Option<Vec<CallGraphSymbol>>,
}

#[derive(serde::Serialize)]
pub(crate) struct PathImpactResponse {
    pub path_id: String,
    pub path_length: usize,
    pub unique_blocks_affected: Vec<usize>,
    pub impact_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forward_impact: Option<Vec<CallGraphSymbol>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backward_impact: Option<Vec<CallGraphSymbol>>,
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct CallGraphSymbol {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fqn: Option<String>,
    pub file_path: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
}

#[derive(serde::Serialize)]
pub(crate) struct HotspotsResponse {
    pub entry_point: String,
    pub total_functions: usize,
    pub hotspots: Vec<HotspotEntry>,
    pub mode: String,
}

#[derive(serde::Serialize, Clone)]
pub(crate) struct HotspotEntry {
    pub function: String,
    pub risk_score: f64,
    pub path_count: usize,
    pub dominance_factor: f64,
    pub complexity: usize,
    pub file_path: String,
}
