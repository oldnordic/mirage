//! Charon ULLBC extraction and parsing

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

/// Run charon binary and capture ULLBC JSON output
pub fn run_charon(crate_path: &Path) -> Result<String> {
    let output = Command::new("charon")
        .current_dir(crate_path)
        .args(["--output-format", "json"])
        .output()
        .context("Failed to run charon binary. Install from: https://github.com/AeneasVerif/charon")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Charon failed: {}", stderr);
    }

    Ok(String::from_utf8(output.stdout)?)
}

/// Parse Charon ULLBC JSON output
pub fn parse_ullbc(json: &str) -> Result<UllbcData> {
    serde_json::from_str(json).context("Failed to parse Charon ULLBC JSON")
}

/// Simplified ULLBC data structures
///
/// These are minimal structures for parsing Charon output.
/// Full Charon types are much larger - we extract what we need for CFG.
#[derive(Debug, Deserialize, Serialize)]
pub struct UllbcData {
    pub crate_name: String,
    pub functions: Vec<UllbcFunction>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UllbcFunction {
    pub id: String,
    pub name: String,
    pub body: Option<UllbcBody>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UllbcBody {
    pub blocks: Vec<UllbcBlock>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UllbcBlock {
    pub id: usize,
    pub statements: Vec<String>,  // Simplified
    pub terminator: UllbcTerminator,
    /// Source location span (optional, depends on Charon output)
    #[serde(default)]
    pub span: Option<UllbcSpan>,
}

/// Source span from ULLBC
#[derive(Debug, Deserialize, Serialize)]
pub struct UllbcSpan {
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind")]
pub enum UllbcTerminator {
    Goto { target: usize },
    SwitchInt { targets: Vec<usize>, otherwise: usize },
    Return,
    Unreachable,
    Call { target: Option<usize>, unwind: Option<usize> },
    Abort { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_ullbc() {
        let json = r#"{
            "crate_name": "test",
            "functions": [{
                "id": "fn1",
                "name": "test_fn",
                "body": {
                    "blocks": [
                        {
                            "id": 0,
                            "statements": [],
                            "terminator": {"kind": "Return"}
                        }
                    ]
                }
            }]
        }"#;

        let data = parse_ullbc(json).unwrap();
        assert_eq!(data.crate_name, "test");
        assert_eq!(data.functions.len(), 1);
        assert_eq!(data.functions[0].name, "test_fn");
    }

    #[test]
    fn test_parse_ullbc_with_goto() {
        let json = r#"{
            "crate_name": "test",
            "functions": [{
                "id": "fn1",
                "name": "goto_fn",
                "body": {
                    "blocks": [
                        {
                            "id": 0,
                            "statements": ["let x = 1"],
                            "terminator": {"kind": "Goto", "target": 1}
                        },
                        {
                            "id": 1,
                            "statements": [],
                            "terminator": {"kind": "Return"}
                        }
                    ]
                }
            }]
        }"#;

        let data = parse_ullbc(json).unwrap();
        let body = data.functions[0].body.as_ref().unwrap();
        assert_eq!(body.blocks.len(), 2);
        assert_eq!(body.blocks[0].id, 0);
        assert_eq!(body.blocks[1].id, 1);
        assert!(matches!(body.blocks[0].terminator, UllbcTerminator::Goto { target: 1 }));
    }

    #[test]
    fn test_parse_ullbc_with_switch() {
        let json = r#"{
            "crate_name": "test",
            "functions": [{
                "id": "fn1",
                "name": "switch_fn",
                "body": {
                    "blocks": [
                        {
                            "id": 0,
                            "statements": [],
                            "terminator": {
                                "kind": "SwitchInt",
                                "targets": [1, 2],
                                "otherwise": 3
                            }
                        },
                        {
                            "id": 1,
                            "statements": [],
                            "terminator": {"kind": "Return"}
                        },
                        {
                            "id": 2,
                            "statements": [],
                            "terminator": {"kind": "Return"}
                        },
                        {
                            "id": 3,
                            "statements": [],
                            "terminator": {"kind": "Return"}
                        }
                    ]
                }
            }]
        }"#;

        let data = parse_ullbc(json).unwrap();
        let body = data.functions[0].body.as_ref().unwrap();
        assert!(matches!(
            &body.blocks[0].terminator,
            UllbcTerminator::SwitchInt { targets, otherwise: 3 } if targets.len() == 2
        ));
    }

    #[test]
    fn test_parse_ullbc_with_call_and_unwind() {
        let json = r#"{
            "crate_name": "test",
            "functions": [{
                "id": "fn1",
                "name": "call_fn",
                "body": {
                    "blocks": [
                        {
                            "id": 0,
                            "statements": [],
                            "terminator": {
                                "kind": "Call",
                                "target": 1,
                                "unwind": 2
                            }
                        },
                        {
                            "id": 1,
                            "statements": [],
                            "terminator": {"kind": "Return"}
                        },
                        {
                            "id": 2,
                            "statements": [],
                            "terminator": {"kind": "Return"}
                        }
                    ]
                }
            }]
        }"#;

        let data = parse_ullbc(json).unwrap();
        let body = data.functions[0].body.as_ref().unwrap();
        assert!(matches!(
            &body.blocks[0].terminator,
            UllbcTerminator::Call { target: Some(1), unwind: Some(2) }
        ));
    }
}
