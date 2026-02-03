//! MIR extraction via Charon

pub mod charon;

pub use charon::{parse_ullbc, run_charon, UllbcBody, UllbcSpan, UllbcTerminator};
