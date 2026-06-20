//! Mirage - Path-Aware Code Intelligence Engine
//!
//! A control-flow and logic graph engine for multi-language codebases.
//!
//! For high-level convenience functions, see the [`forge`] module.
//!
//! # Getting Started
//!
//! ```rust,no_run
//! use mirage::Backend;
//! use std::path::Path;
//!
//! // Auto-detect and open the database backend
//! let backend = Backend::detect_and_open(Path::new("codegraph.db"))?;
//!
//! // Query CFG blocks
//! let blocks = backend.get_cfg_blocks(123)?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Backend Support
//!
//! Mirage supports SQLite storage with optional geometric analysis:
//! - **SQLite**: Default backend, backward compatible with Magellan v7+
//!
//! The backend is automatically detected from the database file format.
//!
//! # Public API
//!
//! - [`Backend`] - Enum wrapping storage backends with auto-detection
//! - [`StorageTrait`] - Backend-agnostic storage interface
//! - [`MirageDb`] - Legacy database connection (wraps Backend internally)
//! - [`forge`] - High-level API for external consumers

pub mod analysis;
pub mod cfg;
pub mod cli;
pub mod forge;
pub mod integrations;
pub mod output;
pub mod platform;
pub mod query;
pub mod storage;

// Public API exports
pub use storage::{
    create_schema, Backend, CfgBlockData, DatabaseStatus, DocumentInfo, FactInfo, MirageDb,
    StorageTrait,
};

// Forge convenience function re-exports
pub use forge::{
    database_status, detect_cycles, find_dead_symbols, get_callees, get_function_cfg,
    get_function_paths, reachable_symbols, resolve_function, CycleReport, DeadSymbolInfo,
    FunctionCfgResult,
};
