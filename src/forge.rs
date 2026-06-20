//! High-level API for external consumers (forge, agents, tool integrations).
//!
//! This module provides convenience functions that wrap `MirageDb`, `Backend`,
//! and `MagellanBridge` into single-call functions for common operations.
//!
//! # Example
//!
//! ```no_run
//! use std::path::Path;
//! use mirage::forge;
//!
//! let db = Path::new(".magellan/myproject.db");
//!
//! // Get CFG for a function by name
//! let cfg = forge::get_function_cfg("my_function", None, db).unwrap();
//! println!(
//!     "{} blocks, {} edges",
//!     cfg.cfg.node_count(),
//!     cfg.cfg.edge_count()
//! );
//!
//! // Detect cycles in the call graph
//! let cycles = forge::detect_cycles(db).unwrap();
//! println!("{} cycles found", cycles.cycles.len());
//!
//! // Find dead symbols (unreachable from entry points)
//! let dead = forge::find_dead_symbols("main", db).unwrap();
//! println!("{} dead symbols", dead.len());
//! ```

use crate::analysis::MagellanBridge;
use crate::cfg::{Cfg, Path as CfgPath};
use crate::storage::{Backend, MirageDb};
use anyhow::Result;
use std::path::Path;

/// CFG result for a function.
#[derive(Debug, Clone)]
pub struct FunctionCfgResult {
    /// Function name.
    pub name: String,
    /// Function ID in the database.
    pub function_id: i64,
    /// The control flow graph.
    pub cfg: Cfg,
}

/// Cycle report from call graph analysis.
#[derive(Debug, Clone)]
pub struct CycleReport {
    /// Detected cycles (each cycle is a list of symbol names).
    pub cycles: Vec<Vec<String>>,
}

/// Dead symbol information.
#[derive(Debug, Clone)]
pub struct DeadSymbolInfo {
    /// Symbol name.
    pub name: String,
    /// Symbol kind.
    pub kind: String,
    /// File path.
    pub file_path: String,
}

/// Resolve a function name to its ID, optionally filtered by file.
///
/// Uses `SymbolNavigator` for name-based resolution.
///
/// # Errors
///
/// Returns an error if the function cannot be found.
pub fn resolve_function(name: &str, file_filter: Option<&str>, db_path: &Path) -> Result<i64> {
    let db = MirageDb::open(db_path)?;
    db.resolve_function_name_with_file(name, file_filter)
}

/// Get the CFG for a function by name.
///
/// Resolves the function name to an ID, loads CFG blocks, and constructs
/// a complete control flow graph.
///
/// # Arguments
///
/// * `name` - Function name (or numeric ID as string)
/// * `file_filter` - Optional file path substring to disambiguate
/// * `db_path` - Path to the Magellan database
///
/// # Errors
///
/// Returns an error if the function is not found or has no CFG data.
pub fn get_function_cfg(
    name: &str,
    file_filter: Option<&str>,
    db_path: &Path,
) -> Result<FunctionCfgResult> {
    let db = MirageDb::open(db_path)?;
    let function_id = db.resolve_function_name_with_file(name, file_filter)?;
    let cfg = db.load_cfg(function_id)?;

    Ok(FunctionCfgResult {
        name: name.to_string(),
        function_id,
        cfg,
    })
}

/// Get cached execution paths for a function.
///
/// Returns pre-computed paths if available in the database.
///
/// # Errors
///
/// Returns an error if the function is not found.
pub fn get_function_paths(
    name: &str,
    file_filter: Option<&str>,
    db_path: &Path,
) -> Result<Option<Vec<CfgPath>>> {
    let backend = Backend::detect_and_open(db_path)?;
    let db = MirageDb::open(db_path)?;
    let function_id = db.resolve_function_name_with_file(name, file_filter)?;
    backend.get_cached_paths(function_id)
}

/// Get callees (outgoing calls) for a function.
///
/// Returns the function IDs of all functions called from the specified function.
///
/// # Errors
///
/// Returns an error if the function is not found.
pub fn get_callees(name: &str, file_filter: Option<&str>, db_path: &Path) -> Result<Vec<i64>> {
    let backend = Backend::detect_and_open(db_path)?;
    let db = MirageDb::open(db_path)?;
    let function_id = db.resolve_function_name_with_file(name, file_filter)?;
    backend.get_callees(function_id)
}

/// Detect cycles in the call graph.
///
/// Uses magellan's cycle detection algorithm to find all strongly connected
/// components with more than one node.
///
/// # Errors
///
/// Returns an error if the database cannot be opened.
pub fn detect_cycles(db_path: &Path) -> Result<CycleReport> {
    let bridge = MagellanBridge::open(db_path.to_str().unwrap_or("."))?;
    let result = bridge.detect_cycles()?;

    let cycles = result
        .cycles
        .into_iter()
        .map(|cycle| {
            cycle
                .members
                .into_iter()
                .filter_map(|info| info.fqn)
                .collect()
        })
        .collect();

    Ok(CycleReport { cycles })
}

/// Find dead symbols (unreachable from the given entry point).
///
/// Computes forward reachability from the entry symbol and returns
/// all symbols that are NOT reachable.
///
/// # Arguments
///
/// * `entry_symbol` - The entry point symbol (e.g. "main")
/// * `db_path` - Path to the Magellan database
///
/// # Errors
///
/// Returns an error if the database cannot be opened.
pub fn find_dead_symbols(entry_symbol: &str, db_path: &Path) -> Result<Vec<DeadSymbolInfo>> {
    let bridge = MagellanBridge::open(db_path.to_str().unwrap_or("."))?;
    let dead = bridge.dead_symbols(entry_symbol)?;

    Ok(dead
        .into_iter()
        .map(|d| DeadSymbolInfo {
            name: d.symbol.fqn.clone().unwrap_or_default(),
            kind: d.symbol.kind.clone(),
            file_path: d.symbol.file_path.clone(),
        })
        .collect())
}

/// Find all symbols reachable from a given symbol (forward reachability).
///
/// # Arguments
///
/// * `symbol_id` - The BLAKE3 symbol ID to start from
/// * `db_path` - Path to the Magellan database
///
/// # Errors
///
/// Returns an error if the database cannot be opened.
pub fn reachable_symbols(symbol_id: &str, db_path: &Path) -> Result<Vec<DeadSymbolInfo>> {
    let bridge = MagellanBridge::open(db_path.to_str().unwrap_or("."))?;
    let reachable = bridge.reachable_symbols(symbol_id)?;

    Ok(reachable
        .into_iter()
        .map(|r| DeadSymbolInfo {
            name: r.fqn.clone().unwrap_or_default(),
            kind: r.kind.clone(),
            file_path: r.file_path.clone(),
        })
        .collect())
}

/// Get database status.
///
/// Returns basic statistics about the database contents.
///
/// # Errors
///
/// Returns an error if the database cannot be opened.
pub fn database_status(db_path: &Path) -> Result<crate::storage::DatabaseStatus> {
    let db = MirageDb::open(db_path)?;
    db.status()
}
