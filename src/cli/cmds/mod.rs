//! CLI command implementations
//!
//! Each command is in its own module for maintainability.
//! Maximum file size: 600 LOC (exceptions up to 1000 LOC)

pub mod status;
pub mod paths;
pub mod cfg;
pub mod dominators;
pub mod loops;
pub mod unreachable;
pub mod verify;
pub mod blast_zone;
pub mod cycles;
pub mod slice;
pub mod hotspots;
pub mod hotpaths;
pub mod patterns;
pub mod frontiers;
pub mod diff;
pub mod icfg;
// pub mod complex_blocks;
// pub mod spatial_analysis;
pub mod migrate;

// // Re-export command handler functions
// pub use complex_blocks::complex_blocks;
// pub use spatial_analysis::spatial_analysis;
