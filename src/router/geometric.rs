//! Geometric Backend Router Implementation
//!
//! This module implements the BackendRouter trait for Geometric (.geo) databases.
//! It leverages the geometric backend's spatial indexing and A* pathfinding capabilities.

use super::*;
use crate::storage::GeometricStorage;
use anyhow::Result;
use magellan::graph::backend::Backend;
use std::path::Path;

/// Geometric backend router
pub struct GeometricRouter {
    storage: GeometricStorage,
}

impl BackendRouter for GeometricRouter {
    fn open(db_path: &Path) -> Result<Self> {
        let storage = GeometricStorage::open(db_path)?;
        Ok(Self { storage })
    }

    fn status(&self) -> Result<DatabaseStatus> {
        // Get stats from geometric backend
        let stats = self.storage.inner().get_stats()?;

        Ok(DatabaseStatus {
            cfg_blocks: stats.cfg_block_count as i64,
            cfg_paths: 0, // Geometric computes on-demand
            cfg_dominators: 0,
            mirage_schema_version: 1,
            magellan_schema_version: 8,
        })
    }

    fn load_cfg(&self, function_id: i64) -> Result<crate::cfg::Cfg> {
        let blocks = self
            .storage
            .inner()
            .get_cfg_blocks_for_function(function_id);
        if blocks.is_empty() {
            anyhow::bail!(
                "No blocks found for function {} in geometric backend",
                function_id
            );
        }

        let mut cfg = crate::cfg::Cfg::new();

        // Add blocks first to generate NodeIndices
        let mut block_map = std::collections::HashMap::new();
        for block in blocks {
            let data = crate::storage::CfgBlockData {
                id: block.id as i64,
                kind: block.block_kind,
                terminator: Some(block.terminator),
                byte_start: block.byte_start,
                byte_end: block.byte_end,
                start_line: block.start_line,
                start_col: block.start_col,
                end_line: block.end_line,
                end_col: block.end_col,
            };

            // Convert block kind string to BlockKind enum
            let kind = match data.kind.as_str() {
                "entry" => crate::cfg::BlockKind::Entry,
                "exit" => crate::cfg::BlockKind::Exit,
                _ => crate::cfg::BlockKind::Normal,
            };

            // Convert terminator string to Terminator enum
            // Note: .geo stores terminators as lowercase strings
            let terminator = match data.terminator.as_deref() {
                Some("return") | None => crate::cfg::Terminator::Return,
                Some("unreachable") => crate::cfg::Terminator::Unreachable,
                Some("goto") | Some("fallthrough") | Some("jump") => {
                    crate::cfg::Terminator::Goto { target: 0 }
                }
                Some("conditional") => crate::cfg::Terminator::SwitchInt {
                    targets: vec![],
                    otherwise: 0,
                },
                Some(t) => crate::cfg::Terminator::Abort(t.to_string()),
            };

            // Create source location
            let source_location = crate::cfg::SourceLocation {
                file_path: std::path::PathBuf::new(), // Empty path since we don't have file info here
                byte_start: data.byte_start as usize,
                byte_end: data.byte_end as usize,
                start_line: data.start_line as usize,
                start_column: data.start_col as usize,
                end_line: data.end_line as usize,
                end_column: data.end_col as usize,
            };

            let node_idx = cfg.add_node(crate::cfg::BasicBlock {
                id: data.id as usize,
                kind,
                statements: vec![],
                terminator,
                source_location: Some(source_location),
            });
            block_map.insert(block.id, node_idx);
        }

        // Get edges from storage
        let inner = self.storage.inner();
        let edges = inner.get_all_edges();

        for edge in edges {
            if let (Some(&src_idx), Some(&dst_idx)) =
                (block_map.get(&edge.src), block_map.get(&edge.dst))
            {
                // Use Fallthrough edge type (Flow doesn't exist)
                cfg.add_edge(src_idx, dst_idx, crate::cfg::EdgeType::Fallthrough);
            }
        }

        Ok(cfg)
    }

    fn resolve_function(&self, name_or_id: &str) -> Result<i64> {
        use crate::integrations::magellan::{MagellanAdapter, ResolveError};

        // Try to parse as numeric ID first
        if let Ok(id) = name_or_id.parse::<i64>() {
            // Verify the ID exists in the database
            if self
                .storage
                .inner()
                .find_symbol_by_id_info(id as u64)
                .is_some()
            {
                return Ok(id);
            } else {
                anyhow::bail!("Function with ID '{}' not found", id);
            }
        }

        // Use MagellanAdapter for contract-compliant resolution
        let adapter = MagellanAdapter::new(self.storage.inner());

        match adapter.resolve_function_id(name_or_id) {
            Ok(id) => Ok(id as i64),
            Err(ResolveError::NotFound { identifier, reason }) => {
                anyhow::bail!("Function '{}' not found: {}", identifier, reason)
            }
            Err(ResolveError::Ambiguous {
                identifier,
                candidates,
                hint,
            }) => {
                // Build detailed ambiguity error
                let mut err_msg = format!(
                    "Ambiguous function reference to '{}': {} candidates match\n\n{}",
                    identifier,
                    candidates.len(),
                    hint
                );

                // Add candidate details
                err_msg.push_str("\n\nCandidates:\n");
                for (i, cand_id) in candidates.iter().enumerate() {
                    if let Some(info) = self.storage.inner().find_symbol_by_id_info(*cand_id) {
                        // fqn is String in geometric types, not Option<String>
                        let fqn_display = if !info.fqn.is_empty() {
                            info.fqn.as_str()
                        } else {
                            info.name.as_str()
                        };
                        err_msg.push_str(&format!(
                            "  {}. ID {}: {} in {}:{}:{}\n",
                            i + 1,
                            cand_id,
                            fqn_display,
                            info.file_path,
                            info.start_line,
                            info.start_col
                        ));
                    }
                }
                err_msg.push_str("\nUse the fully-qualified name (FQN) to disambiguate.");

                anyhow::bail!("{}", err_msg)
            }
        }
    }

    fn get_function_name(&self, function_id: i64) -> Option<String> {
        self.storage
            .inner()
            .find_symbol_by_id_info(function_id as u64)
            .map(|i| i.fqn)
    }

    fn get_function_file(&self, function_id: i64) -> Option<String> {
        self.storage
            .inner()
            .find_symbol_by_id_info(function_id as u64)
            .map(|i| i.file_path)
    }

    fn function_exists(&self, function_id: i64) -> bool {
        self.storage
            .inner()
            .find_symbol_by_id_info(function_id as u64)
            .is_some()
    }

    fn enumerate_paths(&self, function_id: i64, max_paths: usize) -> Result<Vec<ExecutionPath>> {
        let inner = self.storage.inner();
        // Entry block is typically 0
        let entry_block = 0u64;
        println!(
            "DEBUG enumerate_paths: function_id={}, entry_block={}, max_paths={}",
            function_id, entry_block, max_paths
        );
        let k_paths = inner.find_cfg_k_paths(function_id, entry_block, u64::MAX, max_paths);
        println!("DEBUG: found {} paths", k_paths.len());
        for (i, path) in k_paths.iter().enumerate() {
            println!(
                "DEBUG: path[{}]: node_ids={:?}, length={}",
                i,
                path.node_ids,
                path.node_ids.len()
            );
        }

        Ok(k_paths
            .into_iter()
            .enumerate()
            .map(|(idx, path)| ExecutionPath {
                path_id: format!("path_{}_{}", function_id, idx),
                blocks: path.node_ids.iter().map(|id| *id as i64).collect(),
                length: path.node_ids.len(),
            })
            .collect())
    }

    fn get_cfg_blocks(&self, function_id: i64) -> Result<Vec<CfgBlockInfo>> {
        let blocks = self
            .storage
            .inner()
            .get_cfg_blocks_for_function(function_id);
        Ok(blocks
            .into_iter()
            .map(|b| CfgBlockInfo {
                id: b.id as i64,
                kind: b.block_kind,
                terminator: Some(b.terminator),
                byte_start: b.byte_start,
                byte_end: b.byte_end,
                start_line: b.start_line,
                start_col: b.start_col,
                end_line: b.end_line,
                end_col: b.end_col,
            })
            .collect())
    }

    fn get_dominators(&self, function_id: i64) -> Result<DominatorTree> {
        let inner = self.storage.inner();
        let result = inner.compute_dominance(function_id, 0);
        let mut dominators: std::collections::HashMap<i64, Vec<i64>> =
            std::collections::HashMap::new();

        for (&node, &idom) in &result.idom {
            let mut dom_list = vec![idom as i64];
            // Recursively add parent dominators
            let mut current = idom;
            while let Some(&next_idom) = result.idom.get(&current) {
                if next_idom == current {
                    break;
                } // Cycle protection
                dom_list.push(next_idom as i64);
                current = next_idom;
            }
            dominators.insert(node as i64, dom_list);
        }

        Ok(DominatorTree {
            function_id,
            dominators,
        })
    }

    fn get_loops(&self, function_id: i64) -> Result<Vec<NaturalLoop>> {
        let inner = self.storage.inner();
        let result = inner.find_natural_loops(function_id, 0);
        Ok(result
            .loops
            .into_iter()
            .map(|l| NaturalLoop {
                header: l.header as i64,
                blocks: l.body.iter().map(|b| *b as i64).collect(),
            })
            .collect())
    }

    fn find_unreachable(&self, _within_functions: bool) -> Result<Vec<UnreachableCode>> {
        Ok(vec![])
    }
    fn get_patterns(&self, _function_id: i64) -> Result<Vec<BranchPattern>> {
        Ok(vec![])
    }

    fn get_frontiers(&self, function_id: i64) -> Result<DominanceFrontiers> {
        let inner = self.storage.inner();
        let result = inner.compute_dominance_frontier(function_id, 0);
        let frontiers: std::collections::HashMap<i64, Vec<i64>> = result
            .frontier
            .into_iter()
            .map(|(k, v)| (k as i64, v.iter().map(|b| *b as i64).collect()))
            .collect();
        Ok(DominanceFrontiers {
            function_id,
            frontiers,
        })
    }

    fn find_cycles(&self) -> Result<Vec<CallCycle>> {
        Ok(vec![])
    }

    fn get_blast_zone(&self, function_id: i64, block_id: Option<i64>) -> Result<BlastZone> {
        let inner = self.storage.inner();
        let affected_blocks: Vec<i64> = if let Some(bid) = block_id {
            inner
                .get_reachable_from(function_id, bid as u64)
                .into_iter()
                .map(|b| b as i64)
                .collect()
        } else {
            vec![]
        };
        Ok(BlastZone {
            center_function: function_id,
            center_block: block_id,
            affected_functions: vec![function_id],
            affected_blocks,
        })
    }

    fn slice(&self, symbol: &str, direction: SliceDirection) -> Result<crate::router::SliceResult> {
        let inner = self.storage.inner();
        let function_id = self.resolve_function(symbol)?;
        let symbol_id = function_id as u64;
        let (nodes, dir_str) = match direction {
            SliceDirection::Forward => {
                (inner.forward_slice(function_id, symbol_id).nodes, "forward")
            }
            SliceDirection::Backward => (
                inner.backward_slice(function_id, symbol_id).nodes,
                "backward",
            ),
            SliceDirection::Both => (inner.full_slice(function_id, symbol_id).nodes, "both"),
        };
        Ok(crate::router::SliceResult {
            symbol: symbol.to_string(),
            direction: dir_str.to_string(),
            affected_symbols: nodes.iter().map(|b| b.to_string()).collect(),
        })
    }

    fn get_hotspots(&self) -> Result<Vec<Hotspot>> {
        Ok(vec![])
    }
    fn get_hotpaths(&self, function_id: Option<i64>) -> Result<Vec<HotPath>> {
        if let Some(fid) = function_id {
            let paths = self.enumerate_paths(fid, 10)?;
            Ok(paths
                .into_iter()
                .map(|p| HotPath {
                    path: p.blocks,
                    frequency: 1.0,
                })
                .collect())
        } else {
            Ok(vec![])
        }
    }

    fn verify_path(&self, path_id: &str) -> Result<PathVerification> {
        let parts: Vec<&str> = path_id.split('_').collect();
        if parts.len() >= 2 {
            if let Ok(fid) = parts[1].parse::<i64>() {
                let valid = self.function_exists(fid);
                return Ok(PathVerification {
                    path_id: path_id.to_string(),
                    valid,
                    issues: if valid {
                        vec![]
                    } else {
                        vec!["Function not found".to_string()]
                    },
                });
            }
        }
        Ok(PathVerification {
            path_id: path_id.to_string(),
            valid: false,
            issues: vec!["Invalid ID".to_string()],
        })
    }

    fn get_icfg(&self, function_id: i64) -> Result<InterProceduralCfg> {
        Ok(InterProceduralCfg {
            entry_function: function_id,
            nodes: vec![IcfgNode {
                id: 0,
                function_id,
                block_id: 0,
            }],
            edges: vec![],
        })
    }
    fn get_call_graph(&self) -> Result<CallGraph> {
        Ok(CallGraph {
            nodes: vec![],
            edges: vec![],
        })
    }
}
