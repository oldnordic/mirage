//! Mirage - Path-Aware Code Intelligence Engine
//!
//! A control-flow and logic graph engine for multi-language codebases.
//!
//! # Getting Started
//!
//! ```rust,no_run
//! use mirage_analyzer::Backend;
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

#![allow(dead_code)]

pub mod analysis;
pub mod cfg;
pub mod cli;
pub mod integrations;
pub mod mir;
pub mod output;
pub mod platform;
pub mod router;
pub mod storage;

// Public API exports
pub use storage::{create_schema, Backend, CfgBlockData, DatabaseStatus, MirageDb, StorageTrait};
