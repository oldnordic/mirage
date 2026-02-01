# Phase 7: LLM Integration - Research

**Researched:** 2026-02-01
**Domain:** LLM-consumable JSON outputs for code analysis tools
**Confidence:** HIGH

## Summary

This phase focuses on making Mirage's outputs optimally consumable by LLMs and AI agents. Research covered four key areas: (1) LLM-friendly JSON schema design, (2) source location inclusion in tool outputs, (3) error remediation patterns, and (4) natural language generation for control flow summaries.

**Primary recommendation:** Follow SARIF 2.1.0 patterns for structured output, extend existing `JsonResponse` wrapper with LLM-specific metadata, and implement a template-based natural language generator for control flow descriptions.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `serde` | 1.0+ | JSON serialization | Already in use, battle-tested for Rust |
| `serde_json` | 1.0+ | JSON generation/deserialization | Already in use, standard for Rust JSON |
| Existing `JsonResponse<T>` | 1.0.0 | Response wrapper | Already defined in `src/output/mod.rs`, provides `schema_version`, `execution_id`, `tool`, `timestamp` |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| SARIF 2.1.0 patterns | 2.1.0 | Industry standard format | Reference for location/result structures, not full adoption |
| Template strings | Rust `format!` | NL generation | Simple control flow summaries without LLM dependency |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom JSON schema | Full SARIF 2.1.0 | SARIF is verbose (150+ properties), Mirage only needs subset for CFG paths |
| In-line remediation | Separate fix engine | In-line is simpler for v1; fix engines (Sorald, Copilot Autofix) are complex |

**Installation:** No new dependencies required. Use existing `serde` and `serde_json` already in `Cargo.toml`.

## Architecture Patterns

### Recommended Output Structure

Mirage already uses a `JsonResponse<T>` wrapper pattern that provides LLM-friendly metadata:

```rust
pub struct JsonResponse<T> {
    pub schema_version: String,    // "1.0.0" - allows schema evolution
    pub execution_id: String,      // Unique execution identifier
    pub tool: String,              // "mirage" - identifies source
    pub timestamp: String,         // ISO 8601 (RFC 3339) via chrono::Utc::now()
    pub data: T,                   // Actual response payload
}
```

**This pattern is optimal for LLMs because:**
- **Predictable structure:** LLMs can always find metadata at top level
- **Self-identifying:** `tool` field prevents confusion when multiple tools output JSON
- **Traceable:** `execution_id` allows correlating responses with logs
- **Versioned:** `schema_version` enables graceful schema evolution

### Pattern 1: Extended Path Result with Source Locations

**What:** Augment `PathSummary` to include source locations for each block

**Current structure (from `src/cli/mod.rs:228-233`):**
```rust
struct PathSummary {
    path_id: String,
    kind: String,
    length: usize,
    blocks: Vec<usize>,  // Only block IDs, no locations
}
```

**LLM-enhanced structure:**
```rust
struct PathSummary {
    path_id: String,
    kind: String,
    length: usize,
    blocks: Vec<PathBlock>,      // Changed from Vec<usize>
    summary: Option<String>,     // NEW: Natural language description
    source_range: SourceRange,   // NEW: Overall path span
}

struct PathBlock {
    block_id: usize,
    source_location: SourceLocation,
    terminator: String,
}

struct SourceRange {
    file_path: PathBuf,
    start_line: usize,
    end_line: usize,
}
```

**Example output:**
```json
{
  "schema_version": "1.0.0",
  "execution_id": "1769984076-12345",
  "tool": "mirage",
  "timestamp": "2026-02-01T23:15:30Z",
  "data": {
    "function": "process_request",
    "total_paths": 2,
    "error_paths": 1,
    "paths": [
      {
        "path_id": "abc123...",
        "kind": "Normal",
        "length": 3,
        "summary": "Entry → validate → return success",
        "source_range": {
          "file_path": "src/lib.rs",
          "start_line": 42,
          "end_line": 55
        },
        "blocks": [
          {
            "block_id": 0,
            "source_location": {
              "file_path": "src/lib.rs",
              "start_line": 42,
              "start_column": 5,
              "end_line": 44,
              "end_column": 10
            },
            "terminator": "Goto"
          }
        ]
      }
    ]
  }
}
```

### Pattern 2: Error Response with Remediation

**What:** Extend `JsonError` to include actionable remediation suggestions

**Current structure (from `src/output/mod.rs:126-133`):**
```rust
pub struct JsonError {
    pub error: String,
    pub message: String,
    pub code: String,
    pub remediation: Option<String>,  // Already exists!
}
```

**The `remediation` field already exists.** We just need to populate it consistently.

**Remediation pattern guidelines:**

| Error Category | Remediation Pattern | Example |
|----------------|---------------------|---------|
| Database not found | Suggest index command | `"Run 'mirage index' to create the database"` |
| Function not found | Suggest valid functions | `"Available functions: main, process, validate"` |
| Path explosion | Suggest limits | `"Use --max-length N to bound exploration"` |
| Invalid path_id | Suggest verify command | `"Run 'mirage verify --list' to see valid paths"` |

**Example error response:**
```json
{
  "schema_version": "1.0.0",
  "execution_id": "1769984076-12345",
  "tool": "mirage",
  "timestamp": "2026-02-01T23:15:30Z",
  "data": {
    "error": "FunctionNotFound",
    "message": "Function 'unknown_func' not found in database",
    "code": "E001",
    "remediation": "Run 'mirage cfg --list-functions' to see available functions"
  }
}
```

**Note:** The `JsonError` struct is already defined but needs to be used consistently. Check all error paths in CLI commands.

### Pattern 3: Natural Language Summarization

**What:** Generate concise descriptions of control flow paths

**Approach:** Template-based generation (no LLM dependency required)

**Template patterns for CFG structures:**

| CFG Pattern | Template | Example |
|-------------|----------|---------|
| Linear | `Entry → action₁ → ... → actionₙ → return` | `"Entry → validate → process → return"` |
| If-else | `Entry → check → (true_branch OR false_branch) → return` | `"Entry → validate → (success_path OR error_path) → return"` |
| Loop | `Entry → loop { body } → return` | `"Entry → loop { check → update } × N → return"` |
| Match | `Entry → match → (case₁ OR case₂ OR ...) → return` | `"Entry → match value → (None, Some, Other) → return"` |

**Implementation sketch:**
```rust
fn summarize_path(cfg: &Cfg, path: &Path) -> String {
    let parts: Vec<String> = path.blocks.iter()
        .map(|&bid| summarize_block(cfg, bid))
        .collect();

    format!("{}",
        if parts.len() <= 5 {
            parts.join(" → ")
        } else {
            format!("{} → ... → {} ({中间省略{} blocks})",
                parts[0], parts.last().unwrap(), parts.len() - 2)
        })
}

fn summarize_block(cfg: &Cfg, bid: BlockId) -> String {
    let block = &cfg[bid];
    match &block.terminator {
        Terminator::SwitchInt { .. } => format!("if/switch (block {})", bid),
        Terminator::Return => "return".to_string(),
        Terminator::Goto { .. } => format!("block {}", bid),
        Terminator::Call { .. } => format!("call → block {}", bid),
        _ => format!("block {}", bid),
    }
}
```

### Anti-Patterns to Avoid

- **Binary formats:** Never output binary/non-text. LLMs cannot consume it.
- **Inconsistent field names:** Use `snake_case` consistently (Rust/serde default). Avoid mixing `camelCase` and `snake_case`.
- **Over-nesting:** Keep depth ≤ 4 levels. Deep nesting confuses LLMs.
- **Verbose enums:** Use string enums sparingly. Prefer descriptive strings over numeric codes.
- **Missing metadata:** Always include `execution_id` for traceability.
- **Ambiguous errors:** Always provide remediation. An error without suggestion is a dead-end for agents.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Full SARIF schema | Complete `sarif-schema-2.1.0` implementation | Subset pattern (locations + results) | SARIF has 150+ properties; Mirage only needs ~10 for CFG paths |
| Custom JSON serialization | Manual `json!` macros | `#[derive(Serialize)]` from serde | Type-safe, less error-prone |
| LLM for summaries | Call external LLM API | Template-based generation | Faster, no external dependency, deterministic |
| Fix engine integration | Automatic code patching | In-line suggestions only | Automatic patches require complex AST manipulation (Sorald, Copilot) |

**Key insight:** Mirage's job is to **surface facts**, not fix code. Remediation should guide humans/agents, not autonomously modify source.

## Common Pitfalls

### Pitfall 1: Missing Source Locations

**What goes wrong:** Block IDs are useless without file context. Agents cannot navigate to the code.

**Why it happens:** Early prototype used only `Vec<usize>` for blocks.

**How to avoid:** Always include `SourceLocation` with block information. The `SourceLocation` struct already exists in `src/cfg/source.rs` with all necessary fields.

**Warning signs:** JSON output contains numeric IDs without corresponding file/line information.

### Pitfall 2: Inconsistent Error Codes

**What goes wrong:** Agents cannot programmatically categorize errors if codes are arbitrary.

**Why it happens:** Quick implementation uses ad-hoc error strings.

**How to avoid:** Define error code constants and use consistently. Map similar errors to same code.

**Warning signs:** Error codes like `E001`, `E002` are not defined centrally.

### Pitfall 3: Over-Verbose Summaries

**What goes wrong:** LLMs ignore or misinterpret long natural language descriptions.

**Why it happens:** Attempting to generate prose instead of structured descriptions.

**How to avoid:** Keep summaries under 80 chars. Use arrows (`→`) for flow. Use `{}` for loops.

**Warning signs:** Summary field exceeds 200 characters.

### Pitfall 4: Schema Version Drift

**What goes wrong:** LLM prompts break when JSON structure changes.

**How to avoid:** Always increment `schema_version` on breaking changes. Document changes.

**Warning signs:** `schema_version` stays at "1.0.0" despite structural changes.

## Code Examples

### Example 1: LLM-Enhanced Path Response

```rust
// Source: Extended from src/cli/mod.rs:228-233

use serde::Serialize;

/// Enhanced path summary for LLM consumption
#[derive(Serialize)]
pub struct PathSummary {
    pub path_id: String,
    pub kind: String,
    pub length: usize,
    pub summary: String,              // NEW: NL description
    pub source_range: SourceRange,    // NEW: Overall span
    pub blocks: Vec<PathBlock>,       // CHANGED: Now with locations
}

#[derive(Serialize)]
pub struct PathBlock {
    pub block_id: usize,
    pub source_location: SourceLocation,
    pub terminator: String,
}

#[derive(Serialize)]
pub struct SourceRange {
    pub file_path: String,  // Use String instead of PathBuf for JSON
    pub start_line: usize,
    pub end_line: usize,
}

/// Template-based summary generator
impl PathSummary {
    pub fn summarize(cfg: &Cfg, path: &Path) -> String {
        let block_summaries: Vec<String> = path.blocks.iter()
            .map(|&bid| format!("b{}", bid))
            .collect();

        match path.kind {
            PathKind::Normal => format!(
                "Entry → {} → return ({} blocks)",
                block_summaries.join(" → "),
                path.len()
            ),
            PathKind::Error => format!(
                "Entry → ... → error ({} blocks)",
                path.len()
            ),
            _ => format!("Path with {} blocks", path.len()),
        }
    }
}
```

### Example 2: Error with Remediation

```rust
// Source: src/output/mod.rs:126-149 (already exists, just use it)

use crate::output::JsonError;

// In CLI command handler:
pub fn paths_with_remediation(args: &PathsArgs) -> Result<()> {
    let db_path = resolve_db_path(args.db.clone())?;

    let db = match MirageDb::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            let error = JsonError::new(
                "DatabaseError",
                &format!("Failed to open database: {}", e),
                "E001"
            ).with_remediation("Run 'mirage index' to create the database");

            // Output JSON error
            let wrapper = output::JsonResponse::new(error);
            println!("{}", wrapper.to_json());
            std::process::exit(output::EXIT_DATABASE);
        }
    };

    // ... rest of command
}
```

### Example 3: Control Flow Summary Template

```rust
// Source: New module to add: src/cfg/summary.rs

/// Generate natural language summary of CFG structure
pub fn summarize_cfg(function_name: &str, cfg: &Cfg) -> String {
    let entry = find_entry(cfg).map(|id| format!("b{}", id.index()))
        .unwrap_or_else(|| "unknown".to_string());
    let exits = find_exits(cfg);
    let exit_count = exits.count();

    format!(
        "Function '{}' has {} basic blocks, {} exit(s). Entry: {}.",
        function_name,
        cfg.node_count(),
        exit_count,
        entry
    )
}

/// Describe a path in natural language
pub fn describe_path(path: &Path) -> String {
    let kind_desc = match path.kind {
        PathKind::Normal => "normal execution",
        PathKind::Error => "error handling",
        PathKind::Degenerate => "degenerate (dead end)",
        PathKind::Unreachable => "statically unreachable",
    };

    format!(
        "{}-block {} path from b{} to b{}",
        path.len(),
        kind_desc,
        path.entry,
        path.exit
    )
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Text-only CLI output | Structured JSON with metadata | SARIF 2.1.0 (2019) | Industry convergence on SARIF for static analysis |
| Ad-hoc JSON schemas | Standardized wrapper patterns | 2023-2025 | Tools like CodeQL, Semgrep adopt consistent patterns |
| No remediation | AI-powered fix suggestions | GitHub Copilot Autofix (2024) | Expectation of actionable guidance, not just errors |

**Deprecated/outdated:**
- **Plain error messages without codes:** Modern tools always include error codes for programmatic handling
- **Output without metadata:** LLMs need `execution_id` and `timestamp` for context
- **ASCII-only output:** Unicode support expected for international codebases

## Open Questions

1. **How detailed should block-level summaries be?**
   - **What we know:** LLMs prefer concise descriptions; too much detail causes confusion
   - **What's unclear:** Ideal length for `summary` field (current thinking: ≤80 chars)
   - **Recommendation:** Start with minimal templates, expand based on user feedback

2. **Should we adopt full SARIF compliance?**
   - **What we know:** SARIF 2.1.0 is the industry standard for static analysis interchange
   - **What's unclear:** Whether full compliance adds value for CFG path analysis
   - **Recommendation:** Adopt SARIF patterns for location/result structures, but don't implement full schema

3. **How to handle path explosion in summaries?**
   - **What we know:** Functions with thousands of paths cannot enumerate all
   - **What's unclear:** Summary format when paths are truncated
   - **Recommendation:** Include `truncated: true` flag and show first N paths

## Sources

### Primary (HIGH confidence)

- **SARIF 2.1.0 Specification** - [OASIS Standard](https://www.oasis-open.org/standard/sarifv2-1-os/)
  - Referenced for location/result structure patterns
  - Verified: Official specification document

- **CodeQL CLI SARIF Output** - [GitHub Docs](https://docs.github.com/code-security/codeql-cli/sarif-output)
  - Reference for how established tools structure JSON output
  - Verified: Official GitHub documentation

- **Existing Mirage codebase** - `src/output/mod.rs`, `src/cli/mod.rs`, `src/cfg/source.rs`
  - Verified `JsonResponse<T>` wrapper already provides LLM-friendly metadata
  - Verified `SourceLocation` struct exists with file/line/column support
  - Verified `PathSummary` struct exists, needs extension for LLM consumption

### Secondary (MEDIUM confidence)

- **ZeroFalse: LLM + Static Analysis** - [arXiv paper](https://arxiv.org/html/2510.02534v1) (October 2025)
  - Shows SARIF as bridge between static analyzers and LLMs
  - Verified: Academic research with practical examples

- **GitHub Copilot Autofix** - [Responsible Use Docs](https://docs.github.com/en/code-security/responsible-use/responsible-use-autofix-code-scanning)
  - Reference for remediation suggestion patterns
  - Verified: Official GitHub documentation

- **LLM Failure Modes** - [Medium article](https://medium.com/@adnanmasood/a-field-guide-to-llm-failure-modes-5ffaeeb08e80)
  - Reference for tool error handling patterns
  - Verified: Community resource with practical examples

### Tertiary (LOW confidence)

- **Control Flow Summarization Research** - Various academic papers 2023-2025
  - [SCLA: Smart Contract Summarization](https://arxiv.org/html/2402.04863v6)
  - [CP-BCS: Binary Code Summarization](https://openreview.net/forum?id=4AcHxGE6M4)
  - Note: Academic approaches vary significantly; template-based generation recommended for v1

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Using existing serde/serde_json, no new dependencies
- Architecture: HIGH - Based on existing `JsonResponse<T>` pattern already in codebase
- Pitfalls: MEDIUM - Verified against SARIF and GitHub patterns, but LLM consumption patterns still evolving

**Research date:** 2026-02-01
**Valid until:** 30 days (stable domain, but LLM tooling patterns evolve rapidly)

**Magellan Graph Context:**
- execution_id: 5
- snapshot_id: 3
- Database: `.codemcp/codegraph.db`
- Files indexed: 22
- Symbols: 610
- References: 1233
