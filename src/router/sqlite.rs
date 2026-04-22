//! SQLite Backend Router Implementation
//!
//! This module implements the BackendRouter trait for SQLite databases.
//! It wraps the existing MirageDb and Storage implementations.

use super::*;
use crate::storage::MirageDb;
use anyhow::Result;
use std::path::Path;

/// SQLite backend router
pub struct SqliteRouter {
    db: MirageDb,
}

impl BackendRouter for SqliteRouter {
    fn open(db_path: &Path) -> Result<Self> {
        let db = MirageDb::open(db_path)?;
        Ok(Self { db })
    }

    fn status(&self) -> Result<DatabaseStatus> {
        let status = self.db.status()?;
        Ok(DatabaseStatus {
            cfg_blocks: status.cfg_blocks,
            cfg_paths: status.cfg_paths,
            cfg_dominators: status.cfg_dominators,
            mirage_schema_version: status.mirage_schema_version,
            magellan_schema_version: status.magellan_schema_version,
        })
    }

    fn load_cfg(&self, function_id: i64) -> Result<crate::cfg::Cfg> {
        crate::storage::load_cfg_from_db(&self.db, function_id)
    }

    fn resolve_function(&self, name_or_id: &str) -> Result<i64> {
        self.db.resolve_function_name(name_or_id)
    }

    fn get_function_name(&self, function_id: i64) -> Option<String> {
        self.db.get_function_name(function_id)
    }

    fn get_function_file(&self, function_id: i64) -> Option<String> {
        self.db.get_function_file(function_id)
    }

    fn function_exists(&self, function_id: i64) -> bool {
        // Try to resolve by ID and check if it exists
        self.db
            .resolve_function_name(&function_id.to_string())
            .is_ok()
    }

    fn enumerate_paths(&self, function_id: i64, max_paths: usize) -> Result<Vec<ExecutionPath>> {
        // Load CFG for the function
        let cfg = self.load_cfg(function_id)?;

        // Use the cfg::paths module to enumerate paths
        use crate::cfg::paths::{enumerate_paths, PathLimits};

        let limits = PathLimits {
            max_paths,
            max_length: 100,      // Default max path length
            loop_unroll_limit: 3, // Default loop unrolling
        };

        let paths = enumerate_paths(&cfg, &limits);

        // Convert cfg::paths::Path to router::ExecutionPath
        let execution_paths: Vec<ExecutionPath> = paths
            .into_iter()
            .map(|path| ExecutionPath {
                path_id: path.path_id,
                blocks: path.blocks.iter().map(|&b| b as i64).collect(),
                length: path.blocks.len(),
            })
            .collect();

        Ok(execution_paths)
    }

    fn get_cfg_blocks(&self, function_id: i64) -> Result<Vec<CfgBlockInfo>> {
        let blocks = self.db.storage().get_cfg_blocks(function_id)?;

        Ok(blocks
            .into_iter()
            .map(|b| CfgBlockInfo {
                id: b.id,
                kind: b.kind,
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

    fn get_dominators(&self, _function_id: i64) -> Result<DominatorTree> {
        // TODO: Implement using cfg::dominators module
        anyhow::bail!("Dominators not yet implemented for SQLite router")
    }

    fn get_loops(&self, _function_id: i64) -> Result<Vec<NaturalLoop>> {
        // TODO: Implement using cfg::loops module
        anyhow::bail!("Loops not yet implemented for SQLite router")
    }

    fn find_unreachable(&self, _within_functions: bool) -> Result<Vec<UnreachableCode>> {
        // TODO: Implement using analysis module
        anyhow::bail!("Unreachable code detection not yet implemented for SQLite router")
    }

    fn get_patterns(&self, _function_id: i64) -> Result<Vec<BranchPattern>> {
        // TODO: Implement using cfg::patterns module
        anyhow::bail!("Pattern detection not yet implemented for SQLite router")
    }

    fn get_frontiers(&self, _function_id: i64) -> Result<DominanceFrontiers> {
        // TODO: Implement using cfg::dominance_frontiers module
        anyhow::bail!("Dominance frontiers not yet implemented for SQLite router")
    }

    fn find_cycles(&self) -> Result<Vec<CallCycle>> {
        // TODO: Implement using analysis module
        anyhow::bail!("Cycle detection not yet implemented for SQLite router")
    }

    fn get_blast_zone(&self, _function_id: i64, _block_id: Option<i64>) -> Result<BlastZone> {
        // TODO: Implement using analysis module
        anyhow::bail!("Blast zone not yet implemented for SQLite router")
    }

    fn slice(&self, _symbol: &str, _direction: SliceDirection) -> Result<SliceResult> {
        // TODO: Implement using analysis module
        anyhow::bail!("Program slicing not yet implemented for SQLite router")
    }

    fn get_hotspots(&self) -> Result<Vec<Hotspot>> {
        // TODO: Implement using analysis module
        anyhow::bail!("Hotspots not yet implemented for SQLite router")
    }

    fn get_hotpaths(&self, _function_id: Option<i64>) -> Result<Vec<HotPath>> {
        // TODO: Implement using analysis module
        anyhow::bail!("Hot paths not yet implemented for SQLite router")
    }

    fn verify_path(&self, _path_id: &str) -> Result<PathVerification> {
        // TODO: Implement using analysis module
        anyhow::bail!("Path verification not yet implemented for SQLite router")
    }

    fn get_icfg(&self, _function_id: i64) -> Result<InterProceduralCfg> {
        // TODO: Implement using cfg::icfg module
        anyhow::bail!("ICFG not yet implemented for SQLite router")
    }

    fn get_call_graph(&self) -> Result<CallGraph> {
        // TODO: Implement using analysis module
        anyhow::bail!("Call graph not yet implemented for SQLite router")
    }
}
