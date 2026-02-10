// Output formatting utilities following Magellan's patterns

use std::io::IsTerminal;

// Colors for terminal output (when supported)
pub const RED: &str = "\x1b[0;31m";
pub const GREEN: &str = "\x1b[0;32m";
pub const YELLOW: &str = "\x1b[1;33m";
pub const BLUE: &str = "\x1b[0;34m";
pub const CYAN: &str = "\x1b[0;36m";
pub const MAGENTA: &str = "\x1b[0;35m";
pub const BOLD: &str = "\x1b[1m";
pub const NC: &str = "\x1b[0m"; // No Color

/// Check if stdout is a terminal (for color output)
#[inline]
pub fn is_terminal() -> bool {
    std::io::stdout().is_terminal()
}

/// Print info message
pub fn info(msg: &str) {
    let color = if is_terminal() { GREEN } else { "" };
    let reset = if is_terminal() { NC } else { "" };
    println!("{}[INFO]{} {}", color, reset, msg);
}

/// Print warning message
pub fn warn(msg: &str) {
    let color = if is_terminal() { YELLOW } else { "" };
    let reset = if is_terminal() { NC } else { "" };
    eprintln!("{}[WARN]{} {}", color, reset, msg);
}

/// Print error message
pub fn error(msg: &str) {
    let color = if is_terminal() { RED } else { "" };
    let reset = if is_terminal() { NC } else { "" };
    eprintln!("{}[ERROR]{} {}", color, reset, msg);
}

/// Print success message
pub fn success(msg: &str) {
    let color = if is_terminal() { MAGENTA } else { "" };
    let reset = if is_terminal() { NC } else { "" };
    println!("{}[OK]{} {}", color, reset, msg);
}

/// Print section header
pub fn header(msg: &str) {
    let bold = if is_terminal() { BOLD } else { "" };
    let reset = if is_terminal() { NC } else { "" };
    println!("{}===>{} {}", bold, reset, msg);
    println!();
}

/// Print command being executed
pub fn cmd(cmd: &str) {
    let color = if is_terminal() { CYAN } else { "" };
    let reset = if is_terminal() { NC } else { "" };
    eprintln!("{}[CMD]{} {}", color, reset, cmd);
}

/// Exit codes (matching Magellan's conventions)
pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_ERROR: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_DATABASE: i32 = 3;
pub const EXIT_FILE_NOT_FOUND: i32 = 4;
pub const EXIT_VALIDATION: i32 = 5;
pub const EXIT_NOT_FOUND: i32 = 6;

/// Exit with usage error
pub fn exit_usage(msg: &str) -> ! {
    error(msg);
    std::process::exit(EXIT_USAGE);
}

/// Exit with file not found error
pub fn exit_file_not_found(path: &str) -> ! {
    error(&format!("File not found: {}", path));
    std::process::exit(EXIT_FILE_NOT_FOUND);
}

/// Exit with database error
pub fn exit_database(msg: &str) -> ! {
    error(&format!("Database error: {}", msg));
    std::process::exit(EXIT_DATABASE);
}

// ============================================================================
// Error Codes and Remediation
// ============================================================================

/// Error codes for JSON error responses
pub const E_DATABASE_NOT_FOUND: &str = "E001";
pub const E_FUNCTION_NOT_FOUND: &str = "E002";
pub const E_BLOCK_NOT_FOUND: &str = "E003";
pub const E_PATH_NOT_FOUND: &str = "E004";
pub const E_PATH_EXPLOSION: &str = "E005";
pub const E_INVALID_INPUT: &str = "E006";
pub const E_CFG_ERROR: &str = "E007";

/// Common remediation messages
pub const R_HINT_INDEX: &str = "Run 'magellan watch' to create the database";
pub const R_HINT_LIST_FUNCTIONS: &str = "Run 'mirage cfg --list-functions' to see available functions";
pub const R_HINT_MAX_LENGTH: &str = "Use --max-length N to bound path exploration";
pub const R_HINT_VERIFY_PATH: &str = "Run 'mirage verify --list' to see valid paths";

/// JSON output wrapper (following Magellan's response format)
#[derive(Debug, Clone, serde::Serialize)]
pub struct JsonResponse<T> {
    pub schema_version: String,
    pub execution_id: String,
    pub tool: String,
    pub timestamp: String,
    pub data: T,
}

impl<T: serde::Serialize> JsonResponse<T> {
    pub fn new(data: T) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = chrono::Utc::now().to_rfc3339();
        let exec_id = format!("{:x}-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            std::process::id()
        );

        JsonResponse {
            schema_version: "1.0.1".to_string(),
            execution_id: exec_id,
            tool: "mirage".to_string(),
            timestamp,
            data,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn to_pretty_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// Error response format for JSON mode
#[derive(Debug, Clone, serde::Serialize)]
pub struct JsonError {
    pub error: String,
    pub message: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

impl JsonError {
    pub fn new(category: &str, message: &str, code: &str) -> Self {
        JsonError {
            error: category.to_string(),
            message: message.to_string(),
            code: code.to_string(),
            remediation: None,
        }
    }

    pub fn with_remediation(mut self, remediation: &str) -> Self {
        self.remediation = Some(remediation.to_string());
        self
    }

    /// Database not found error with remediation
    pub fn database_not_found(path: &str) -> Self {
        Self::new(
            "DatabaseNotFound",
            &format!("Database not found: {}", path),
            E_DATABASE_NOT_FOUND
        ).with_remediation(R_HINT_INDEX)
    }

    /// Function not found error with remediation
    pub fn function_not_found(name: &str) -> Self {
        Self::new(
            "FunctionNotFound",
            &format!("Function '{}' not found in database", name),
            E_FUNCTION_NOT_FOUND
        ).with_remediation(R_HINT_LIST_FUNCTIONS)
    }

    /// Block not found error
    pub fn block_not_found(id: usize) -> Self {
        Self::new(
            "BlockNotFound",
            &format!("Block {} not found in CFG", id),
            E_BLOCK_NOT_FOUND
        )
    }

    /// Path not found error
    pub fn path_not_found(id: &str) -> Self {
        Self::new(
            "PathNotFound",
            &format!("Path '{}' not found or no longer valid", id),
            E_PATH_NOT_FOUND
        ).with_remediation("Run 'mirage verify --path-id ID' to check path validity")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_response() {
        let data = vec!["item1", "item2"];
        let response = JsonResponse::new(data);
        let json = response.to_json();
        assert!(json.contains("\"tool\":\"mirage\""));
        assert!(json.contains("\"data\":[\"item1\",\"item2\"]"));
    }
}
