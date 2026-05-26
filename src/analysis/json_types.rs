use magellan::{
    CondensationResult, DeadSymbol, ExecutionPath, PathEnumerationResult, SliceResult, SymbolInfo,
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DeadSymbolJson {
    pub fqn: Option<String>,
    pub file_path: String,
    pub kind: String,
    pub reason: String,
}

impl From<&DeadSymbol> for DeadSymbolJson {
    fn from(dead: &DeadSymbol) -> Self {
        Self {
            fqn: dead.symbol.fqn.clone(),
            file_path: dead.symbol.file_path.clone(),
            kind: dead.symbol.kind.clone(),
            reason: dead.reason.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolInfoJson {
    pub symbol_id: Option<String>,
    pub fqn: Option<String>,
    pub file_path: String,
    pub kind: String,
}

impl From<&SymbolInfo> for SymbolInfoJson {
    fn from(symbol: &SymbolInfo) -> Self {
        Self {
            symbol_id: symbol.symbol_id.clone(),
            fqn: symbol.fqn.clone(),
            file_path: symbol.file_path.clone(),
            kind: symbol.kind.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SliceWrapper {
    pub target: SymbolInfoJson,
    pub direction: String,
    pub included_symbols: Vec<SymbolInfoJson>,
    pub symbol_count: usize,
    pub statistics: SliceStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct SliceStats {
    pub total_symbols: usize,
    pub data_dependencies: usize,
    pub control_dependencies: usize,
}

impl From<&SliceResult> for SliceWrapper {
    fn from(result: &SliceResult) -> Self {
        let statistics = SliceStats {
            total_symbols: result.statistics.total_symbols,
            data_dependencies: result.statistics.data_dependencies,
            control_dependencies: result.statistics.control_dependencies,
        };

        SliceWrapper {
            target: (&result.slice.target).into(),
            direction: format!("{:?}", result.slice.direction),
            included_symbols: result
                .slice
                .included_symbols
                .iter()
                .map(|s| s.into())
                .collect(),
            symbol_count: result.slice.symbol_count,
            statistics,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionPathJson {
    pub symbols: Vec<SymbolInfoJson>,
    pub length: usize,
}

impl From<&ExecutionPath> for ExecutionPathJson {
    fn from(path: &ExecutionPath) -> Self {
        ExecutionPathJson {
            symbols: path.symbols.iter().map(|s| s.into()).collect(),
            length: path.length,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PathEnumerationJson {
    pub paths: Vec<ExecutionPathJson>,
    pub total_enumerated: usize,
    pub truncated: bool,
    pub statistics: PathStatisticsJson,
}

#[derive(Debug, Clone, Serialize)]
pub struct PathStatisticsJson {
    pub avg_length: f64,
    pub max_length: usize,
    pub min_length: usize,
    pub unique_symbols: usize,
}

impl From<&PathEnumerationResult> for PathEnumerationJson {
    fn from(result: &PathEnumerationResult) -> Self {
        PathEnumerationJson {
            paths: result.paths.iter().map(|p| p.into()).collect(),
            total_enumerated: result.total_enumerated,
            truncated: result.bounded_hit,
            statistics: PathStatisticsJson {
                avg_length: result.statistics.avg_length,
                max_length: result.statistics.max_length,
                min_length: result.statistics.min_length,
                unique_symbols: result.statistics.unique_symbols,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CondensationJson {
    pub supernode_count: usize,
    pub edge_count: usize,
    pub supernodes: Vec<SupernodeJson>,
    pub largest_scc_size: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SupernodeJson {
    pub id: String,
    pub member_count: usize,
    pub members: Vec<String>,
}

impl From<&CondensationResult> for CondensationJson {
    fn from(result: &CondensationResult) -> Self {
        let supernodes: Vec<SupernodeJson> = result
            .graph
            .supernodes
            .iter()
            .map(|sn| SupernodeJson {
                id: sn.id.to_string(),
                member_count: sn.members.len(),
                members: sn.members.iter().filter_map(|m| m.fqn.clone()).collect(),
            })
            .collect();

        let largest_scc_size = supernodes
            .iter()
            .map(|sn| sn.member_count)
            .max()
            .unwrap_or(0);

        CondensationJson {
            supernode_count: result.graph.supernodes.len(),
            edge_count: result.graph.edges.len(),
            supernodes,
            largest_scc_size,
        }
    }
}
