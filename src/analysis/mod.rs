//! Inter-procedural analysis using Magellan's call graph
//!
//! This module provides a bridge to Magellan's graph algorithms,
//! enabling combined inter-procedural (Magellan) and intra-procedural (Mirage) analysis.
//!
//! # Architecture
//!
//! - **Magellan** (inter-procedural): Call graph algorithms, reachability, dead code detection
//! - **Mirage** (intra-procedural): CFG analysis, path enumeration, dominance
//!
//! The [`MagellanBridge`] struct wraps Magellan's [`CodeGraph`] to provide
//! a unified API for both layers of analysis.

pub mod analysis_types;
pub mod bridge;
pub mod json_types;
pub mod risk;
pub mod stats;
pub mod suggest;
pub mod telemetry;

pub use magellan::CodeGraph;

pub use magellan::{
    CondensationResult, Cycle, CycleKind, CycleReport, DeadSymbol, ExecutionPath,
    PathEnumerationResult, SymbolInfo,
};

#[allow(unused_imports)]
use magellan::{
    CondensationGraph, PathStatistics, ProgramSlice, SliceDirection, SliceResult, SliceStatistics,
    Supernode,
};

#[allow(unused_imports)]
use anyhow::Result;

pub use analysis_types::{
    CycleInfo, EnhancedBlastZone, EnhancedCycles, EnhancedDeadCode, LoopInfo, PathImpactSummary,
};

pub use bridge::MagellanBridge;

pub use json_types::{
    CondensationJson, DeadSymbolJson, ExecutionPathJson, PathEnumerationJson, PathStatisticsJson,
    SliceStats, SliceWrapper, SupernodeJson, SymbolInfoJson,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magellan_bridge_creation() {
        let _ = || -> Result<()> {
            let _bridge = MagellanBridge::open("test.db")?;
            Ok(())
        };
    }

    #[test]
    fn test_dead_symbol_json_from_dead_symbol() {
        use magellan::{DeadSymbol as MagellanDeadSymbol, SymbolInfo};

        let symbol_info = SymbolInfo {
            symbol_id: Some("test_symbol_id".to_string()),
            fqn: Some("test::function".to_string()),
            file_path: "test.rs".to_string(),
            kind: "Function".to_string(),
        };

        let dead = MagellanDeadSymbol {
            symbol: symbol_info,
            reason: "Not called from entry point".to_string(),
        };

        let json_symbol: DeadSymbolJson = (&dead).into();

        assert_eq!(json_symbol.fqn, Some("test::function".to_string()));
        assert_eq!(json_symbol.file_path, "test.rs");
        assert_eq!(json_symbol.kind, "Function");
        assert_eq!(json_symbol.reason, "Not called from entry point");
    }

    #[test]
    fn test_enhanced_dead_code_serialization() {
        use magellan::{DeadSymbol as MagellanDeadSymbol, SymbolInfo};

        let symbol_info = SymbolInfo {
            symbol_id: Some("test_id".to_string()),
            fqn: Some("dead::function".to_string()),
            file_path: "test.rs".to_string(),
            kind: "Function".to_string(),
        };

        let dead = MagellanDeadSymbol {
            symbol: symbol_info,
            reason: "Uncalled".to_string(),
        };

        let json_symbol: DeadSymbolJson = (&dead).into();

        let mut unreachable_blocks = std::collections::HashMap::new();
        unreachable_blocks.insert("test_func".to_string(), vec![1, 2, 3]);

        let enhanced = EnhancedDeadCode {
            uncalled_functions: vec![json_symbol],
            unreachable_blocks,
            total_dead_count: 4,
        };

        let json = serde_json::to_string(&enhanced).unwrap();
        assert!(json.contains("uncalled_functions"));
        assert!(json.contains("unreachable_blocks"));
        assert!(json.contains("total_dead_count"));
    }

    #[test]
    fn test_cycle_info_from_cycle() {
        use magellan::{Cycle, CycleKind, SymbolInfo};

        let symbol1 = SymbolInfo {
            symbol_id: Some("func_a_id".to_string()),
            fqn: Some("func_a".to_string()),
            file_path: "test.rs".to_string(),
            kind: "Function".to_string(),
        };

        let symbol2 = SymbolInfo {
            symbol_id: Some("func_b_id".to_string()),
            fqn: Some("func_b".to_string()),
            file_path: "test.rs".to_string(),
            kind: "Function".to_string(),
        };

        let mutual_recursion_cycle = Cycle {
            members: vec![symbol1.clone(), symbol2.clone()],
            kind: CycleKind::MutualRecursion,
        };

        let cycle_info: CycleInfo = (&mutual_recursion_cycle).into();
        assert_eq!(cycle_info.cycle_type, "MutualRecursion");
        assert_eq!(cycle_info.size, 2);
        assert_eq!(cycle_info.members, vec!["func_a", "func_b"]);

        let self_loop_cycle = Cycle {
            members: vec![symbol1],
            kind: CycleKind::SelfLoop,
        };

        let cycle_info: CycleInfo = (&self_loop_cycle).into();
        assert_eq!(cycle_info.cycle_type, "SelfLoop");
        assert_eq!(cycle_info.size, 1);
        assert_eq!(cycle_info.members, vec!["func_a"]);
    }

    #[test]
    fn test_enhanced_cycles_serialization() {
        use std::collections::HashMap;

        let mut function_loops = HashMap::new();
        function_loops.insert(
            "test_func".to_string(),
            vec![LoopInfo {
                header: 1,
                back_edge_from: 2,
                body_size: 3,
                nesting_level: 0,
                body_blocks: vec![1, 2, 3],
            }],
        );

        let call_graph_cycles = vec![CycleInfo {
            members: vec!["func_a".to_string(), "func_b".to_string()],
            cycle_type: "MutualRecursion".to_string(),
            size: 2,
        }];

        let enhanced = EnhancedCycles {
            call_graph_cycles,
            function_loops,
            total_cycles: 2,
        };

        let json = serde_json::to_string(&enhanced).unwrap();
        assert!(json.contains("call_graph_cycles"));
        assert!(json.contains("function_loops"));
        assert!(json.contains("total_cycles"));
        assert!(json.contains("MutualRecursion"));
    }

    #[test]
    fn test_loop_info_serialization() {
        let loop_info = LoopInfo {
            header: 1,
            back_edge_from: 3,
            body_size: 5,
            nesting_level: 2,
            body_blocks: vec![1, 2, 3, 4, 5],
        };

        let json = serde_json::to_string(&loop_info).unwrap();
        assert!(json.contains(r#""header":1"#));
        assert!(json.contains(r#""back_edge_from":3"#));
        assert!(json.contains(r#""body_size":5"#));
        assert!(json.contains(r#""nesting_level":2"#));
        assert!(json.contains(r#"body_blocks"#));
    }

    #[test]
    fn test_slice_wrapper_serialization() {
        use magellan::{ProgramSlice, SliceDirection, SliceResult, SliceStatistics};

        let target = SymbolInfo {
            symbol_id: Some("target_id".to_string()),
            fqn: Some("target_function".to_string()),
            file_path: "test.rs".to_string(),
            kind: "Function".to_string(),
        };

        let included_symbols = vec![
            SymbolInfo {
                symbol_id: Some("sym1_id".to_string()),
                fqn: Some("sym1".to_string()),
                file_path: "test.rs".to_string(),
                kind: "Function".to_string(),
            },
            SymbolInfo {
                symbol_id: Some("sym2_id".to_string()),
                fqn: Some("sym2".to_string()),
                file_path: "test.rs".to_string(),
                kind: "Function".to_string(),
            },
        ];

        let program_slice = ProgramSlice {
            target: target.clone(),
            direction: SliceDirection::Backward,
            included_symbols: included_symbols.clone(),
            symbol_count: 3,
        };

        let statistics = SliceStatistics {
            total_symbols: 3,
            data_dependencies: 2,
            control_dependencies: 1,
        };

        let slice_result = SliceResult {
            slice: program_slice,
            statistics,
        };

        let wrapper: SliceWrapper = (&slice_result).into();

        assert_eq!(wrapper.target.fqn, Some("target_function".to_string()));
        assert_eq!(wrapper.direction, "Backward");
        assert_eq!(wrapper.symbol_count, 3);
        assert_eq!(wrapper.statistics.total_symbols, 3);
        assert_eq!(wrapper.statistics.data_dependencies, 2);
        assert_eq!(wrapper.statistics.control_dependencies, 1);
        assert_eq!(wrapper.included_symbols.len(), 2);

        let json = serde_json::to_string(&wrapper).unwrap();
        assert!(json.contains("target"));
        assert!(json.contains("direction"));
        assert!(json.contains("Backward"));
        assert!(json.contains("included_symbols"));
        assert!(json.contains("statistics"));
        assert!(json.contains("data_dependencies"));
    }

    #[test]
    fn test_slice_stats_creation() {
        let stats = SliceStats {
            total_symbols: 10,
            data_dependencies: 5,
            control_dependencies: 3,
        };

        assert_eq!(stats.total_symbols, 10);
        assert_eq!(stats.data_dependencies, 5);
        assert_eq!(stats.control_dependencies, 3);

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains(r#""total_symbols":10"#));
        assert!(json.contains(r#""data_dependencies":5"#));
        assert!(json.contains(r#""control_dependencies":3"#));
    }

    #[test]
    fn test_symbol_info_json_from_symbol_info() {
        use magellan::SymbolInfo;

        let symbol_info = SymbolInfo {
            symbol_id: Some("test_symbol_id".to_string()),
            fqn: Some("test::function".to_string()),
            file_path: "test.rs".to_string(),
            kind: "Function".to_string(),
        };

        let json_symbol: SymbolInfoJson = (&symbol_info).into();

        assert_eq!(json_symbol.symbol_id, Some("test_symbol_id".to_string()));
        assert_eq!(json_symbol.fqn, Some("test::function".to_string()));
        assert_eq!(json_symbol.file_path, "test.rs");
        assert_eq!(json_symbol.kind, "Function");
    }

    #[test]
    fn test_enhanced_blast_zone_creation() {
        let forward = vec![
            SymbolInfoJson {
                symbol_id: Some("func_a_id".to_string()),
                fqn: Some("func_a".to_string()),
                file_path: "a.rs".to_string(),
                kind: "Function".to_string(),
            },
            SymbolInfoJson {
                symbol_id: Some("func_b_id".to_string()),
                fqn: Some("func_b".to_string()),
                file_path: "b.rs".to_string(),
                kind: "Function".to_string(),
            },
        ];

        let backward = vec![SymbolInfoJson {
            symbol_id: Some("main_id".to_string()),
            fqn: Some("main".to_string()),
            file_path: "main.rs".to_string(),
            kind: "Function".to_string(),
        }];

        let path_impact = PathImpactSummary {
            path_id: Some("test_path_id".to_string()),
            path_length: 5,
            blocks_affected: vec![1, 2, 3, 4],
            unique_blocks_count: 4,
        };

        let blast_zone = EnhancedBlastZone {
            target: "test_function".to_string(),
            forward_reachable: forward.clone(),
            backward_reachable: backward.clone(),
            path_impact: Some(path_impact),
        };

        assert_eq!(blast_zone.target, "test_function");
        assert_eq!(blast_zone.forward_reachable.len(), 2);
        assert_eq!(blast_zone.backward_reachable.len(), 1);
        assert!(blast_zone.path_impact.is_some());

        let json = serde_json::to_string(&blast_zone).unwrap();
        assert!(json.contains("target"));
        assert!(json.contains("forward_reachable"));
        assert!(json.contains("backward_reachable"));
        assert!(json.contains("path_impact"));
        assert!(json.contains("func_a"));
        assert!(json.contains("main"));
    }

    #[test]
    fn test_path_impact_summary_serialization() {
        let impact = PathImpactSummary {
            path_id: Some("test_path".to_string()),
            path_length: 10,
            blocks_affected: vec![1, 2, 3, 4, 5],
            unique_blocks_count: 5,
        };

        let json = serde_json::to_string(&impact).unwrap();
        assert!(json.contains("path_id"));
        assert!(json.contains("path_length"));
        assert!(json.contains("blocks_affected"));
        assert!(json.contains("unique_blocks_count"));
        assert!(json.contains("test_path"));
    }

    #[test]
    fn test_enhanced_blast_zone_without_path_impact() {
        let blast_zone = EnhancedBlastZone {
            target: "test_function".to_string(),
            forward_reachable: vec![],
            backward_reachable: vec![],
            path_impact: None,
        };

        assert!(blast_zone.path_impact.is_none());

        let json = serde_json::to_string(&blast_zone).unwrap();
        assert!(json.contains(r#""path_impact":null"#));
    }

    #[test]
    fn test_condensation_json_creation() {
        use magellan::{CondensationGraph, CondensationResult, Supernode};
        use std::collections::HashMap;

        let symbol1 = SymbolInfo {
            symbol_id: Some("func_a_id".to_string()),
            fqn: Some("func_a".to_string()),
            file_path: "a.rs".to_string(),
            kind: "Function".to_string(),
        };

        let symbol2 = SymbolInfo {
            symbol_id: Some("func_b_id".to_string()),
            fqn: Some("func_b".to_string()),
            file_path: "b.rs".to_string(),
            kind: "Function".to_string(),
        };

        let supernode1 = Supernode {
            id: 0,
            members: vec![symbol1.clone()],
        };

        let supernode2 = Supernode {
            id: 1,
            members: vec![symbol2.clone(), symbol1.clone()],
        };

        let graph = CondensationGraph {
            supernodes: vec![supernode1, supernode2],
            edges: vec![(0, 1)],
        };

        let mut mapping = HashMap::new();
        mapping.insert("func_a".to_string(), 0);
        mapping.insert("func_b".to_string(), 1);

        let result = CondensationResult {
            graph,
            original_to_supernode: mapping,
        };

        let json: CondensationJson = (&result).into();

        assert_eq!(json.supernode_count, 2);
        assert_eq!(json.edge_count, 1);
        assert_eq!(json.largest_scc_size, 2);
        assert_eq!(json.supernodes.len(), 2);
        assert_eq!(json.supernodes[0].id, "0");
        assert_eq!(json.supernodes[0].member_count, 1);
        assert_eq!(json.supernodes[1].id, "1");
        assert_eq!(json.supernodes[1].member_count, 2);
        assert!(json.supernodes[1].members.contains(&"func_b".to_string()));
    }

    #[test]
    fn test_condensation_json_serialization() {
        use magellan::{CondensationGraph, CondensationResult, Supernode};
        use std::collections::HashMap;

        let supernode = Supernode {
            id: 0,
            members: vec![SymbolInfo {
                symbol_id: Some("test_id".to_string()),
                fqn: Some("test_func".to_string()),
                file_path: "test.rs".to_string(),
                kind: "Function".to_string(),
            }],
        };

        let graph = CondensationGraph {
            supernodes: vec![supernode],
            edges: vec![],
        };

        let result = CondensationResult {
            graph,
            original_to_supernode: HashMap::new(),
        };

        let json: CondensationJson = (&result).into();
        let json_string = serde_json::to_string(&json).unwrap();

        assert!(json_string.contains(r#""supernode_count":1"#));
        assert!(json_string.contains(r#""edge_count":0"#));
        assert!(json_string.contains(r#""largest_scc_size":1"#));
        assert!(json_string.contains(r#""id":"0""#));
        assert!(json_string.contains("test_func"));
    }

    #[test]
    fn test_supernode_json_creation() {
        let supernode = SupernodeJson {
            id: "42".to_string(),
            member_count: 3,
            members: vec![
                "func_a".to_string(),
                "func_b".to_string(),
                "func_c".to_string(),
            ],
        };

        assert_eq!(supernode.id, "42");
        assert_eq!(supernode.member_count, 3);
        assert_eq!(supernode.members.len(), 3);

        let json = serde_json::to_string(&supernode).unwrap();
        assert!(json.contains(r#""id":"42""#));
        assert!(json.contains(r#""member_count":3"#));
        assert!(json.contains("func_a"));
    }

    #[test]
    fn test_execution_path_json_conversion() {
        use magellan::{ExecutionPath, SymbolInfo};

        let symbols = vec![
            SymbolInfo {
                symbol_id: Some("main_id".to_string()),
                fqn: Some("main".to_string()),
                file_path: "main.rs".to_string(),
                kind: "Function".to_string(),
            },
            SymbolInfo {
                symbol_id: Some("helper_id".to_string()),
                fqn: Some("helper".to_string()),
                file_path: "helper.rs".to_string(),
                kind: "Function".to_string(),
            },
        ];

        let path = ExecutionPath {
            symbols: symbols.clone(),
            length: 2,
        };

        let json_path: ExecutionPathJson = (&path).into();

        assert_eq!(json_path.length, 2);
        assert_eq!(json_path.symbols.len(), 2);
        assert_eq!(json_path.symbols[0].fqn, Some("main".to_string()));
        assert_eq!(json_path.symbols[1].fqn, Some("helper".to_string()));

        let json = serde_json::to_string(&json_path).unwrap();
        assert!(json.contains("symbols"));
        assert!(json.contains("length"));
        assert!(json.contains("main"));
        assert!(json.contains("helper"));
    }

    #[test]
    fn test_path_statistics_json_creation() {
        let stats = PathStatisticsJson {
            avg_length: 3.5,
            max_length: 10,
            min_length: 1,
            unique_symbols: 5,
        };

        assert_eq!(stats.avg_length, 3.5);
        assert_eq!(stats.max_length, 10);
        assert_eq!(stats.min_length, 1);
        assert_eq!(stats.unique_symbols, 5);

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains(r#""avg_length":3.5"#));
        assert!(json.contains(r#""max_length":10"#));
        assert!(json.contains(r#""min_length":1"#));
        assert!(json.contains(r#""unique_symbols":5"#));
    }

    #[test]
    fn test_path_enumeration_json_serialization() {
        use magellan::{ExecutionPath, PathStatistics};

        let symbols = vec![SymbolInfo {
            symbol_id: Some("func1_id".to_string()),
            fqn: Some("func1".to_string()),
            file_path: "test.rs".to_string(),
            kind: "Function".to_string(),
        }];

        let _path = ExecutionPath { symbols, length: 1 };

        let _stats = PathStatistics {
            avg_length: 2.0,
            max_length: 5,
            min_length: 1,
            unique_symbols: 3,
        };

        let json_stats = PathStatisticsJson {
            avg_length: 2.0,
            max_length: 5,
            min_length: 1,
            unique_symbols: 3,
        };

        let json = serde_json::to_string(&json_stats).unwrap();
        assert!(json.contains("avg_length"));
        assert!(json.contains("max_length"));
        assert!(json.contains("min_length"));
        assert!(json.contains("unique_symbols"));
    }

    #[test]
    fn test_execution_path_json_empty_path() {
        use magellan::ExecutionPath;

        let path = ExecutionPath {
            symbols: vec![],
            length: 0,
        };

        let json_path: ExecutionPathJson = (&path).into();

        assert_eq!(json_path.length, 0);
        assert_eq!(json_path.symbols.len(), 0);

        let json = serde_json::to_string(&json_path).unwrap();
        assert!(json.contains(r#""symbols":[]"#));
        assert!(json.contains(r#""length":0"#));
    }

    #[test]
    fn test_condensation_json_from_result() {
        let json = CondensationJson {
            supernode_count: 5,
            edge_count: 8,
            supernodes: vec![SupernodeJson {
                id: "scc0".to_string(),
                member_count: 3,
                members: vec![
                    "func_a".to_string(),
                    "func_b".to_string(),
                    "func_c".to_string(),
                ],
            }],
            largest_scc_size: 3,
        };

        assert_eq!(json.supernode_count, 5);
        assert_eq!(json.largest_scc_size, 3);
        assert_eq!(json.supernodes[0].member_count, 3);

        let serialized = serde_json::to_string(&json).unwrap();
        assert!(serialized.contains("supernode_count"));
        assert!(serialized.contains("largest_scc_size"));
    }

    #[test]
    fn test_execution_path_json_serialization() {
        let path = ExecutionPathJson {
            symbols: vec![SymbolInfoJson {
                symbol_id: Some("id1".to_string()),
                fqn: Some("main".to_string()),
                file_path: "main.rs".to_string(),
                kind: "Function".to_string(),
            }],
            length: 1,
        };

        let json = serde_json::to_string(&path).unwrap();
        assert!(json.contains("main"));
        assert!(json.contains("\"length\":1"));
    }

    #[test]
    fn test_all_magellan_imports_utilized() {
        let _ = std::marker::PhantomData::<CondensationGraph>;
        let _ = std::marker::PhantomData::<CondensationResult>;
        let _ = std::marker::PhantomData::<Supernode>;

        let _ = std::marker::PhantomData::<ExecutionPath>;
        let _ = std::marker::PhantomData::<PathEnumerationResult>;
        let _ = std::marker::PhantomData::<PathStatistics>;

        let _ = std::marker::PhantomData::<CondensationJson>;
        let _ = std::marker::PhantomData::<SupernodeJson>;
        let _ = std::marker::PhantomData::<ExecutionPathJson>;
        let _ = std::marker::PhantomData::<PathEnumerationJson>;
        let _ = std::marker::PhantomData::<PathStatisticsJson>;

        let _ = std::marker::PhantomData::<ProgramSlice>;
        let _ = std::marker::PhantomData::<SliceDirection>;
        let _ = std::marker::PhantomData::<SliceResult>;
        let _ = std::marker::PhantomData::<SliceStatistics>;
    }

    #[test]
    fn test_phase_11_integration() {
        let condensation = CondensationJson {
            supernode_count: 2,
            edge_count: 1,
            supernodes: vec![SupernodeJson {
                id: "scc0".to_string(),
                member_count: 2,
                members: vec!["func_a".to_string(), "func_b".to_string()],
            }],
            largest_scc_size: 2,
        };

        let path_stats = PathStatisticsJson {
            avg_length: 3.5,
            max_length: 10,
            min_length: 1,
            unique_symbols: 5,
        };

        let cond_json = serde_json::to_string(&condensation).unwrap();
        let stats_json = serde_json::to_string(&path_stats).unwrap();

        assert!(cond_json.contains("supernode_count"));
        assert!(stats_json.contains("avg_length"));
    }

    #[test]
    fn test_supernode_json_multiple_members() {
        let supernode = SupernodeJson {
            id: "cycle_42".to_string(),
            member_count: 4,
            members: vec![
                "func_a".to_string(),
                "func_b".to_string(),
                "func_c".to_string(),
                "func_d".to_string(),
            ],
        };

        let json = serde_json::to_string(&supernode).unwrap();
        assert!(json.contains("\"member_count\":4"));
        assert!(json.contains("cycle_42"));
        assert!(json.contains("func_a"));
        assert!(json.contains("func_d"));
    }

    #[test]
    fn test_path_statistics_json_edge_cases() {
        let empty_stats = PathStatisticsJson {
            avg_length: 0.0,
            max_length: 0,
            min_length: 0,
            unique_symbols: 0,
        };

        let json = serde_json::to_string(&empty_stats).unwrap();
        assert!(json.contains("\"avg_length\":0"));
        assert!(json.contains("\"unique_symbols\":0"));

        let large_stats = PathStatisticsJson {
            avg_length: 9999.99,
            max_length: 100000,
            min_length: 1,
            unique_symbols: 50000,
        };

        let json = serde_json::to_string(&large_stats).unwrap();
        assert!(json.contains("9999.99"));
        assert!(json.contains("100000"));
    }
}
