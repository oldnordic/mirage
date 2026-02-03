//! MIR extraction via Charon

pub mod charon;

#[allow(unused_imports)] // Used in tests
pub use charon::{parse_ullbc, run_charon, UllbcBlock, UllbcBody, UllbcSpan, UllbcTerminator};
