use anyhow::Result;
use magellan::graph::geometric_backend::GeometricBackend;
use magellan::validation::normalize_path;
use std::path::Path;

use super::adapter::{
    CycleInfo, DeadSymbolInfo, FqnLookupResult, MagellanAdapter, PathEnumerationResult,
};

pub struct GeometricBridge {
    backend: GeometricBackend,
}

impl GeometricBridge {
    pub fn open(db_path: &str) -> Result<Self> {
        let path = Path::new(db_path);
        let backend = GeometricBackend::open(path)
            .map_err(|e| anyhow::anyhow!("Failed to open .geo database: {}", e))?;
        Ok(Self { backend })
    }

    pub fn adapter(&self) -> MagellanAdapter<'_> {
        MagellanAdapter::new(&self.backend)
    }

    pub fn backend(&self) -> &GeometricBackend {
        &self.backend
    }

    pub fn reachable_symbols(
        &self,
        symbol_id_or_fqn: &str,
    ) -> Result<Vec<magellan::graph::geometric_backend::SymbolInfo>> {
        let adapter = MagellanAdapter::new(&self.backend);

        if let Ok(id) = symbol_id_or_fqn.parse::<u64>() {
            let reachable_ids = adapter.reachable_from(id);
            return Ok(reachable_ids
                .into_iter()
                .filter_map(|id| self.backend.find_symbol_by_id_info(id))
                .collect());
        }

        match adapter.lookup_symbol_by_fqn(symbol_id_or_fqn) {
            FqnLookupResult::Unique(info) => {
                let reachable_ids = adapter.reachable_from(info.id);
                Ok(reachable_ids
                    .into_iter()
                    .filter_map(|id| self.backend.find_symbol_by_id_info(id))
                    .collect())
            }
            FqnLookupResult::NotFound => {
                anyhow::bail!("Symbol '{}' not found", symbol_id_or_fqn)
            }
            FqnLookupResult::Ambiguous { .. } => {
                anyhow::bail!(
                    "Ambiguous reference to '{}', use numeric ID",
                    symbol_id_or_fqn
                )
            }
        }
    }

    pub fn dead_symbols(&self, entry_symbol_id_or_fqn: &str) -> Result<Vec<DeadSymbolInfo>> {
        let adapter = MagellanAdapter::new(&self.backend);

        let entry_id = if let Ok(id) = entry_symbol_id_or_fqn.parse::<u64>() {
            id
        } else {
            match adapter.lookup_symbol_by_fqn(entry_symbol_id_or_fqn) {
                FqnLookupResult::Unique(info) => info.id,
                FqnLookupResult::NotFound => {
                    anyhow::bail!("Entry point '{}' not found", entry_symbol_id_or_fqn)
                }
                FqnLookupResult::Ambiguous { .. } => {
                    anyhow::bail!(
                        "Ambiguous entry point '{}', use numeric ID",
                        entry_symbol_id_or_fqn
                    )
                }
            }
        };

        Ok(adapter.dead_code_from_entries(&[entry_id]))
    }

    pub fn detect_cycles(&self) -> Result<Vec<CycleInfo>> {
        let adapter = MagellanAdapter::new(&self.backend);
        Ok(adapter.find_call_graph_cycles())
    }

    pub fn enumerate_paths(
        &self,
        start_symbol_id_or_fqn: &str,
        end_symbol_id_or_fqn: Option<&str>,
        max_depth: usize,
        max_paths: usize,
    ) -> Result<PathEnumerationResult> {
        let adapter = MagellanAdapter::new(&self.backend);

        let start_id = if let Ok(id) = start_symbol_id_or_fqn.parse::<u64>() {
            id
        } else {
            match adapter.lookup_symbol_by_fqn(start_symbol_id_or_fqn) {
                FqnLookupResult::Unique(info) => info.id,
                FqnLookupResult::NotFound => {
                    anyhow::bail!("Start symbol '{}' not found", start_symbol_id_or_fqn)
                }
                FqnLookupResult::Ambiguous { .. } => {
                    anyhow::bail!(
                        "Ambiguous start symbol '{}', use numeric ID",
                        start_symbol_id_or_fqn
                    )
                }
            }
        };

        let end_id = if let Some(end) = end_symbol_id_or_fqn {
            if let Ok(id) = end.parse::<u64>() {
                Some(id)
            } else {
                match adapter.lookup_symbol_by_fqn(end) {
                    FqnLookupResult::Unique(info) => Some(info.id),
                    FqnLookupResult::NotFound => {
                        anyhow::bail!("End symbol '{}' not found", end)
                    }
                    FqnLookupResult::Ambiguous { .. } => {
                        anyhow::bail!("Ambiguous end symbol '{}', use numeric ID", end)
                    }
                }
            }
        } else {
            None
        };

        Ok(adapter.enumerate_paths(start_id, end_id, max_depth, max_paths))
    }
}

pub fn normalize_path_for_query(path: &str) -> String {
    use std::path::Path;

    let preprocessed = path.replace("//", "/").replace('\\', "/");

    match normalize_path(Path::new(&preprocessed)) {
        Ok(normalized) => normalized,
        Err(_) => preprocessed,
    }
}

pub fn paths_equivalent(path1: &str, path2: &str) -> bool {
    let norm1 = normalize_path_for_query(path1);
    let norm2 = normalize_path_for_query(path2);
    norm1 == norm2
}
