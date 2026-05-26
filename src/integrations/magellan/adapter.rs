use anyhow::Result;
use magellan::graph::geometric_backend::GeometricBackend;

#[derive(Debug, Clone)]
pub enum SymbolLookupResult {
    Unique(magellan::graph::geometric_backend::SymbolInfo),

    Ambiguous {
        path: String,
        name: String,
        candidates: Vec<magellan::graph::geometric_backend::SymbolInfo>,
    },

    NotFound,
}

impl SymbolLookupResult {
    pub fn is_unique(&self) -> bool {
        matches!(self, SymbolLookupResult::Unique(_))
    }

    pub fn is_ambiguous(&self) -> bool {
        matches!(self, SymbolLookupResult::Ambiguous { .. })
    }

    pub fn is_not_found(&self) -> bool {
        matches!(self, SymbolLookupResult::NotFound)
    }

    pub fn unique(self) -> Option<magellan::graph::geometric_backend::SymbolInfo> {
        match self {
            SymbolLookupResult::Unique(info) => Some(info),
            _ => None,
        }
    }

    pub fn candidates(self) -> Option<Vec<magellan::graph::geometric_backend::SymbolInfo>> {
        match self {
            SymbolLookupResult::Ambiguous { candidates, .. } => Some(candidates),
            _ => None,
        }
    }

    pub fn count(&self) -> usize {
        match self {
            SymbolLookupResult::Unique(_) => 1,
            SymbolLookupResult::Ambiguous { candidates, .. } => candidates.len(),
            SymbolLookupResult::NotFound => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum FqnLookupResult {
    Unique(magellan::graph::geometric_backend::SymbolInfo),

    Ambiguous {
        fqn: String,
        candidates: Vec<magellan::graph::geometric_backend::SymbolInfo>,
    },

    NotFound,
}

#[derive(Debug, Clone)]
pub enum IdLookupResult {
    Found(magellan::graph::geometric_backend::SymbolInfo),

    NotFound,
}

pub struct MagellanAdapter<'a> {
    backend: &'a GeometricBackend,
}

impl<'a> MagellanAdapter<'a> {
    pub fn new(backend: &'a GeometricBackend) -> Self {
        Self { backend }
    }

    pub fn backend(&self) -> &GeometricBackend {
        self.backend
    }

    pub fn lookup_symbol_by_path_and_name(&self, path: &str, name: &str) -> SymbolLookupResult {
        let normalized_path = super::normalize_path_for_query(path);

        match self
            .backend
            .find_symbol_id_by_name_and_path(name, &normalized_path)
        {
            Some(id) => match self.backend.find_symbol_by_id_info(id) {
                Some(info) => SymbolLookupResult::Unique(info),
                None => SymbolLookupResult::NotFound,
            },
            None => {
                let all_symbols = self
                    .backend
                    .symbols_in_file(&normalized_path)
                    .unwrap_or_default();
                let matching: Vec<_> = all_symbols.into_iter().filter(|s| s.name == name).collect();

                if matching.len() > 1 {
                    SymbolLookupResult::Ambiguous {
                        path: normalized_path,
                        name: name.to_string(),
                        candidates: matching,
                    }
                } else {
                    SymbolLookupResult::NotFound
                }
            }
        }
    }

    pub fn lookup_symbol_by_fqn(&self, fqn: &str) -> FqnLookupResult {
        match self.backend.find_symbol_by_fqn_info(fqn) {
            Some(info) => FqnLookupResult::Unique(info),
            None => FqnLookupResult::NotFound,
        }
    }

    pub fn lookup_symbol_by_id(&self, id: u64) -> IdLookupResult {
        match self.backend.find_symbol_by_id_info(id) {
            Some(info) => IdLookupResult::Found(info),
            None => IdLookupResult::NotFound,
        }
    }

    pub fn resolve_function_id(&self, identifier: &str) -> Result<u64, ResolveError> {
        if let Ok(id) = identifier.parse::<u64>() {
            match self.lookup_symbol_by_id(id) {
                IdLookupResult::Found(_) => return Ok(id),
                IdLookupResult::NotFound => {
                    return Err(ResolveError::NotFound {
                        identifier: identifier.to_string(),
                        reason: "No symbol with this ID exists".to_string(),
                    })
                }
            }
        }

        match self.lookup_symbol_by_fqn(identifier) {
            FqnLookupResult::Unique(info) => return Ok(info.id),
            FqnLookupResult::NotFound => {}
            FqnLookupResult::Ambiguous { fqn, candidates } => {
                let count = candidates.len();
                return Err(ResolveError::Ambiguous {
                    identifier: identifier.to_string(),
                    candidates: candidates.into_iter().map(|c| c.id).collect(),
                    hint: format!(
                        "FQN '{}' matches {} symbols - check for duplicate definitions",
                        fqn, count
                    ),
                });
            }
        }

        let all_matching = self.backend.find_symbols_by_name_info(identifier);

        let mut unique_symbols: Vec<magellan::graph::geometric_backend::SymbolInfo> = Vec::new();
        let mut seen_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();

        for sym in all_matching {
            if seen_ids.insert(sym.id) {
                unique_symbols.push(sym);
            }
        }

        match unique_symbols.len() {
            0 => Err(ResolveError::NotFound {
                identifier: identifier.to_string(),
                reason: "No symbol with this name, FQN, or ID exists".to_string(),
            }),
            1 => Ok(unique_symbols[0].id),
            n => {
                let normalize_path =
                    |p: &str| -> String { p.replace("\\", "/").replace("/./", "/") };

                let first = &unique_symbols[0];
                let first_path_normalized = normalize_path(&first.file_path);
                let all_same_location = unique_symbols.iter().all(|sym| {
                    let sym_path_normalized = normalize_path(&sym.file_path);
                    sym.name == first.name
                        && sym_path_normalized == first_path_normalized
                        && sym.start_line == first.start_line
                        && sym.start_col == first.start_col
                });

                if all_same_location {
                    Ok(unique_symbols[0].id)
                } else {
                    Err(ResolveError::Ambiguous {
                        identifier: identifier.to_string(),
                        candidates: unique_symbols.into_iter().map(|c| c.id).collect(),
                        hint: format!(
                            "Found {} unique symbols named '{}' - use FQN or path to disambiguate",
                            n, identifier
                        ),
                    })
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ResolveError {
    NotFound {
        identifier: String,
        reason: String,
    },

    Ambiguous {
        identifier: String,
        candidates: Vec<u64>,
        hint: String,
    },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::NotFound { identifier, reason } => {
                write!(f, "Symbol '{}' not found: {}", identifier, reason)
            }
            ResolveError::Ambiguous {
                identifier,
                candidates,
                hint,
            } => {
                write!(
                    f,
                    "Ambiguous reference to '{}': {} candidates. {}",
                    identifier,
                    candidates.len(),
                    hint
                )
            }
        }
    }
}

impl std::error::Error for ResolveError {}

#[derive(Debug, Clone)]
pub struct CycleInfo {
    pub symbol_ids: Vec<u64>,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub struct DeadSymbolInfo {
    pub symbol_id: u64,
    pub fqn: Option<String>,
    pub file_path: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct CallPath {
    pub symbol_ids: Vec<u64>,
    pub length: usize,
}

#[derive(Debug, Clone)]
pub struct PathEnumerationResult {
    pub paths: Vec<CallPath>,
    pub total_count: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone)]
pub struct CallRelation {
    pub caller_id: u64,
    pub caller_name: String,
    pub callee_id: u64,
    pub callee_name: String,
}

impl<'a> MagellanAdapter<'a> {
    pub fn reachable_from(&self, symbol_id: u64) -> Vec<u64> {
        self.backend.reachable_from(symbol_id)
    }

    pub fn reverse_reachable_from(&self, symbol_id: u64) -> Vec<u64> {
        self.backend.reverse_reachable_from(symbol_id)
    }

    pub fn dead_code_from_entries(&self, entry_ids: &[u64]) -> Vec<DeadSymbolInfo> {
        let dead_ids = self.backend.dead_code_from_entries(entry_ids);

        dead_ids
            .into_iter()
            .filter_map(|id| self.backend.find_symbol_by_id_info(id))
            .map(|info| DeadSymbolInfo {
                symbol_id: info.id,
                fqn: Some(info.fqn),
                file_path: info.file_path,
                reason: "Not reachable from any entry point".to_string(),
            })
            .collect()
    }

    pub fn find_call_graph_cycles(&self) -> Vec<CycleInfo> {
        let cycles = self.backend.find_call_graph_cycles();

        cycles
            .into_iter()
            .map(|mut scc| CycleInfo {
                symbol_ids: std::mem::take(&mut scc),
                kind: if scc.len() == 2 {
                    "MutualRecursion".to_string()
                } else {
                    "Cycle".to_string()
                },
            })
            .collect()
    }

    pub fn get_strongly_connected_components(&self) -> magellan::graph::geometric_calls::SccResult {
        self.backend.get_strongly_connected_components()
    }

    pub fn condense_call_graph(&self) -> magellan::graph::geometric_calls::CondensationDag {
        self.backend.condense_call_graph()
    }

    pub fn enumerate_paths(
        &self,
        start_id: u64,
        end_id: Option<u64>,
        max_depth: usize,
        max_paths: usize,
    ) -> PathEnumerationResult {
        let magellan_result = self
            .backend
            .enumerate_paths(start_id, end_id, max_depth, max_paths);

        PathEnumerationResult {
            paths: magellan_result
                .paths
                .into_iter()
                .map(|symbol_ids| CallPath {
                    symbol_ids: symbol_ids.clone(),
                    length: symbol_ids.len(),
                })
                .collect(),
            total_count: magellan_result.total_enumerated,
            truncated: magellan_result.bounded_hit,
        }
    }

    pub fn callers_of_symbol(&self, symbol_id: u64) -> Vec<CallRelation> {
        let callers = self.backend.get_callers(symbol_id);

        callers
            .into_iter()
            .filter_map(|caller_id| {
                let caller_info = self.backend.find_symbol_by_id_info(caller_id)?;
                let callee_info = self.backend.find_symbol_by_id_info(symbol_id)?;
                Some(CallRelation {
                    caller_id,
                    caller_name: caller_info.name.clone(),
                    callee_id: symbol_id,
                    callee_name: callee_info.name.clone(),
                })
            })
            .collect()
    }

    pub fn callees_of_symbol(&self, symbol_id: u64) -> Vec<CallRelation> {
        let callees = self.backend.get_callees(symbol_id);

        callees
            .into_iter()
            .filter_map(|callee_id| {
                let caller_info = self.backend.find_symbol_by_id_info(symbol_id)?;
                let callee_info = self.backend.find_symbol_by_id_info(callee_id)?;
                Some(CallRelation {
                    caller_id: symbol_id,
                    caller_name: caller_info.name.clone(),
                    callee_id,
                    callee_name: callee_info.name.clone(),
                })
            })
            .collect()
    }
}
