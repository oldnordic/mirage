//! Geometric Backend Router Implementation
//!
//! This module implements the BackendRouter trait for Geometric (.geo) databases.
//! It leverages the geometric backend's spatial indexing and A* pathfinding capabilities.

use super::*;
use crate::storage::GeometricStorage;
use anyhow::Result;

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
                terminator: block.terminator,
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
            let terminator = match data.terminator.as_str() {
                "" | "return" => crate::cfg::Terminator::Return,
                "unreachable" => crate::cfg::Terminator::Unreachable,
                "goto" | "fallthrough" | "jump" => {
                    crate::cfg::Terminator::Goto { target: 0 }
                }
                "conditional" => crate::cfg::Terminator::SwitchInt {
                    targets: vec![],
                    otherwise: 0,
                },
                t => crate::cfg::Terminator::Abort(t.to_string()),
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

        // Get CFG edges from storage (not graph edges)
        let inner = self.storage.inner();
        let cfg_edges = inner.get_all_cfg_edges();

        for edge in cfg_edges {
            if let (Some(&src_idx), Some(&dst_idx)) =
                (block_map.get(&edge.src_id), block_map.get(&edge.dst_id))
            {
                // Use Fallthrough edge type
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
        // Load CFG for the function
        let cfg = self.load_cfg(function_id)?;
        
        // Use Mirage's path enumeration algorithm on the loaded CFG
        let limits = crate::cfg::PathLimits::default()
            .with_max_paths(max_paths);
        
        let paths = crate::cfg::enumerate_paths(&cfg, &limits);
        
        // Convert to ExecutionPath format
        Ok(paths
            .into_iter()
            .map(|path| {
                let path_id = path.path_id.clone();
                let len = path.len();
                ExecutionPath {
                    path_id,
                    blocks: path.blocks.iter().map(|&b| b as i64).collect(),
                    length: len,
                }
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
        // Load CFG and compute dominators
        let cfg = self.load_cfg(function_id)?;
        
        use crate::cfg::dominators::DominatorTree as CfgDominatorTree;
        
        let dom_tree = CfgDominatorTree::new(&cfg)
            .ok_or_else(|| anyhow::anyhow!("Failed to compute dominator tree - CFG may have no entry"))?;
        
        // Convert to router format
        let mut dominators = std::collections::HashMap::new();
        
        for node in cfg.node_indices() {
            let node_id = cfg.node_weight(node).map(|n| n.id as i64).unwrap_or(0);
            let dominated: Vec<i64> = dom_tree.children(node)
                .into_iter()
                .map(|child_idx| cfg.node_weight(*child_idx).map(|n| n.id as i64).unwrap_or(0))
                .collect();
            dominators.insert(node_id, dominated);
        }
        
        Ok(DominatorTree {
            function_id,
            dominators,
        })
    }

    fn get_loops(&self, function_id: i64) -> Result<Vec<NaturalLoop>> {
        // Load CFG and detect loops
        let cfg = self.load_cfg(function_id)?;
        
        use crate::cfg::loops::detect_natural_loops;
        
        let loops = detect_natural_loops(&cfg);
        
        // Convert to router format
        Ok(loops
            .into_iter()
            .map(|l| NaturalLoop {
                header: cfg[l.header].id as i64,
                blocks: l.body.iter().map(|&n| cfg[n].id as i64).collect(),
            })
            .collect())
    }

    fn find_unreachable(&self, _within_functions: bool) -> Result<Vec<UnreachableCode>> {
        // For now, return empty - this would require scanning all functions
        // and checking for unreachable blocks in each CFG
        Ok(vec![])
    }
    fn get_patterns(&self, function_id: i64) -> Result<Vec<BranchPattern>> {
        // Load CFG and detect patterns
        let cfg = self.load_cfg(function_id)?;
        
        use crate::cfg::patterns::detect_all_patterns;
        
        let (if_else_patterns, match_patterns) = detect_all_patterns(&cfg);
        
        // Convert to router format
        let mut patterns = Vec::new();
        
        for (idx, p) in if_else_patterns.iter().enumerate() {
            let mut blocks = vec![
                cfg.node_weight(p.condition).map(|n| n.id as i64).unwrap_or(0),
                cfg.node_weight(p.true_branch).map(|n| n.id as i64).unwrap_or(0),
                cfg.node_weight(p.false_branch).map(|n| n.id as i64).unwrap_or(0),
            ];
            if let Some(merge) = p.merge_point {
                blocks.push(cfg.node_weight(merge).map(|n| n.id as i64).unwrap_or(0));
            }
            patterns.push(BranchPattern {
                pattern_id: format!("ifelse_{}", idx),
                kind: "IfElse".to_string(),
                blocks,
            });
        }
        
        for (idx, p) in match_patterns.iter().enumerate() {
            let mut blocks = vec![
                cfg.node_weight(p.switch_node).map(|n| n.id as i64).unwrap_or(0),
            ];
            for target in &p.targets {
                blocks.push(cfg.node_weight(*target).map(|n| n.id as i64).unwrap_or(0));
            }
            blocks.push(cfg.node_weight(p.otherwise).map(|n| n.id as i64).unwrap_or(0));
            patterns.push(BranchPattern {
                pattern_id: format!("match_{}", idx),
                kind: "Match".to_string(),
                blocks,
            });
        }
        
        Ok(patterns)
    }

    fn get_frontiers(&self, function_id: i64) -> Result<DominanceFrontiers> {
        // Load CFG and compute dominance frontiers
        let cfg = self.load_cfg(function_id)?;
        
        use crate::cfg::dominance_frontiers::DominanceFrontiers as CfgFrontiers;
        use crate::cfg::dominators::DominatorTree;
        
        let dom_tree = DominatorTree::new(&cfg)
            .ok_or_else(|| anyhow::anyhow!("Failed to compute dominator tree"))?;
        
        let frontiers = CfgFrontiers::new(&cfg, dom_tree);
        
        // Convert to router format
        let mut frontier_map = std::collections::HashMap::new();
        
        for node in cfg.node_indices() {
            let node_id = cfg[node].id as i64;
            let node_frontier: Vec<i64> = frontiers.frontier(node)
                .iter()
                .map(|&n| cfg[n].id as i64)
                .collect();
            frontier_map.insert(node_id, node_frontier);
        }
        
        Ok(DominanceFrontiers {
            function_id,
            frontiers: frontier_map,
        })
    }

    fn find_cycles(&self) -> Result<Vec<CallCycle>> {
        // Call cycle detection is available through Magellan's GeometricBackend
        let cycles = self.storage.inner().find_call_graph_cycles();
        Ok(cycles
            .into_iter()
            .enumerate()
            .map(|(idx, cycle)| CallCycle {
                cycle_id: format!("cycle_{}", idx),
                functions: cycle.iter().map(|id| *id as i64).collect(),
            })
            .collect())
    }

    fn get_blast_zone(&self, function_id: i64, block_id: Option<i64>) -> Result<BlastZone> {
        // Use geometric backend's reachability analysis
        let inner = self.storage.inner();
        
        // Get forward reachable (affected by this function/block)
        let forward_reachable = inner.reachable_from(function_id as u64);
        
        // Get backward reachable (can reach this function)
        let backward_reachable = inner.reverse_reachable_from(function_id as u64);
        
        // Get affected blocks from CFG if block_id specified
        let affected_blocks = if let Some(bid) = block_id {
            vec![bid]
        } else {
            // Get all blocks in the function
            let cfg = self.load_cfg(function_id)?;
            cfg.node_indices().map(|n| cfg[n].id as i64).collect()
        };
        
        Ok(BlastZone {
            center_function: function_id,
            center_block: block_id,
            affected_functions: forward_reachable.iter().map(|&id| id as i64).collect(),
            affected_blocks,
        })
    }

    fn slice(&self, symbol: &str, direction: SliceDirection) -> Result<crate::router::SliceResult> {
        // Resolve the symbol
        let symbol_id = self.resolve_function(symbol)?;
        
        let affected_symbols = match direction {
            SliceDirection::Forward => {
                // Forward: what this symbol affects
                self.storage.inner()
                    .reachable_from(symbol_id as u64)
                    .into_iter()
                    .filter_map(|id| self.get_function_name(id as i64))
                    .collect()
            }
            SliceDirection::Backward => {
                // Backward: what affects this symbol
                self.storage.inner()
                    .reverse_reachable_from(symbol_id as u64)
                    .into_iter()
                    .filter_map(|id| self.get_function_name(id as i64))
                    .collect()
            }
            SliceDirection::Both => {
                let mut forward: std::collections::HashSet<String> = self.storage.inner()
                    .reachable_from(symbol_id as u64)
                    .into_iter()
                    .filter_map(|id| self.get_function_name(id as i64))
                    .collect();
                let backward: std::collections::HashSet<String> = self.storage.inner()
                    .reverse_reachable_from(symbol_id as u64)
                    .into_iter()
                    .filter_map(|id| self.get_function_name(id as i64))
                    .collect();
                forward.extend(backward);
                forward.into_iter().collect()
            }
        };
        
        Ok(crate::router::SliceResult {
            symbol: symbol.to_string(),
            direction: match direction {
                SliceDirection::Forward => "forward".to_string(),
                SliceDirection::Backward => "backward".to_string(),
                SliceDirection::Both => "both".to_string(),
            },
            affected_symbols,
        })
    }

    fn get_hotspots(&self) -> Result<Vec<Hotspot>> {
        // Get all symbols
        let symbols = self.storage.inner()
            .get_all_symbols()
            .map_err(|e| anyhow::anyhow!("Failed to get symbols: {}", e))?;
        
        let mut hotspots = Vec::new();
        
        for symbol in symbols {
            // Calculate complexity from CFG
            let complexity = if let Ok(cfg) = self.load_cfg(symbol.id as i64) {
                // Simple complexity metric: block count
                cfg.node_count() as f64
            } else {
                0.0
            };
            
            // Calculate frequency from call graph
            let frequency = self.storage.inner()
                .get_callers(symbol.id)
                .len() as f64;
            
            hotspots.push(Hotspot {
                function_id: symbol.id as i64,
                complexity,
                frequency,
            });
        }
        
        // Sort by combined risk score (complexity * frequency)
        hotspots.sort_by(|a, b| {
            let a_score = a.complexity * a.frequency;
            let b_score = b.complexity * b.frequency;
            b_score.partial_cmp(&a_score).unwrap()
        });
        
        Ok(hotspots)
    }
    fn get_hotpaths(&self, _function_id: Option<i64>) -> Result<Vec<HotPath>> {
        // For now, return empty - hot path analysis requires path frequency data
        // which isn't available in the geometric backend
        Ok(vec![])
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
        // Build ICFG from call graph and individual CFGs
        let inner = self.storage.inner();
        
        // Get direct callees
        let callees = inner.get_callees(function_id as u64);
        
        let mut nodes = Vec::new();
        let edges = Vec::new();
        let mut node_counter: i64 = 0;
        
        // Add entry function nodes
        if let Ok(cfg) = self.load_cfg(function_id) {
            for node_idx in cfg.node_indices() {
                let block_id = cfg[node_idx].id as i64;
                nodes.push(IcfgNode {
                    id: node_counter,
                    function_id,
                    block_id,
                });
                node_counter += 1;
            }
        }
        
        // Add callee function nodes and call edges
        for callee_id in callees {
            if let Ok(cfg) = self.load_cfg(callee_id as i64) {
                for node_idx in cfg.node_indices() {
                    let block_id = cfg[node_idx].id as i64;
                    nodes.push(IcfgNode {
                        id: node_counter,
                        function_id: callee_id as i64,
                        block_id,
                    });
                    node_counter += 1;
                }
            }
        }
        
        Ok(InterProceduralCfg {
            entry_function: function_id,
            nodes,
            edges,
        })
    }
    fn get_call_graph(&self) -> Result<CallGraph> {
        // Build call graph from actual call edges in the geometric backend
        let inner = self.storage.inner();
        let symbols = inner.get_all_symbols()
            .map_err(|e| anyhow::anyhow!("Failed to get symbols: {}", e))?;
        let call_edges = inner.get_call_edges();
        
        let nodes: Vec<CallGraphNode> = symbols
            .into_iter()
            .map(|s| CallGraphNode {
                id: s.id as i64,
                function_name: s.fqn,
            })
            .collect();
        
        let edges: Vec<CallGraphEdge> = call_edges
            .into_iter()
            .map(|e| CallGraphEdge {
                caller_id: e.src_symbol_id as i64,
                callee_id: e.dst_symbol_id as i64,
            })
            .collect();
        
        Ok(CallGraph { nodes, edges })
    }
}
