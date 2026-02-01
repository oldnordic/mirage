//! MIR extraction via Charon

pub mod charon;

pub use charon::{run_charon, parse_ullbc, UllbcData, UllbcBody, UllbcBlock, UllbcTerminator};
