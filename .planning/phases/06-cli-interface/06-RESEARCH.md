# Phase 6: CLI Interface - Research

**Researched:** 2026-02-01
**Domain:** Command-line interface design, Rust CLI patterns, user experience
**Confidence:** HIGH

## Summary

Phase 6 focuses on implementing the user-facing CLI interface for Mirage. This phase builds on all previous phases (database foundation, CFG construction, reachability, dominance, path enumeration) to expose these capabilities through a well-designed command-line interface following Magellan's established patterns.

The CLI is already partially implemented with stub commands in `src/cli/mod.rs`. This phase focuses on connecting those stubs to the actual analysis capabilities implemented in Phases 2-5, with proper output formatting, error handling, and user experience.

**Key challenges addressed:**
- Connecting CLI commands to backend analysis capabilities
- Supporting multiple output formats (human, JSON, pretty JSON)
- Efficient path display for large result sets
- Path verification after code changes
- Following Magellan's CLI patterns for consistency

**Primary recommendation:** Implement CLI commands incrementally, starting with path queries (already have enumeration), then CFG display, dominance analysis, unreachable code detection, and path verification. Use clap's derive API with ValueEnum for output formats. Follow Magellan's JsonResponse wrapper pattern for JSON output.

## Standard Stack

The CLI interface builds on Mirage's existing dependencies:

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| **clap** | 4.5 | CLI argument parsing, subcommands, value enums | Industry-standard Rust CLI framework with derive API, already in use |
| **anyhow** | 1.0 | Error handling and propagation | Already in use, provides Context for error chaining |
| **serde** | 1.0 | JSON serialization for response types | Already in use for cfg export |
| **serde_json** | 1.0 | JSON output formatting | Already in use |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **atty** | 0.2 | Terminal detection for colored output | Already in use in output module |
| **chrono** | 0.4 | Timestamp generation for JsonResponse | Already in use |
| **rusqlite** | 0.32 | Database queries for all commands | Already in use from Phase 1 |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| clap derive API | clap builder API | Derive API is more concise for static commands; builder is better for dynamic commands (not needed here) |
| JsonResponse wrapper | Raw JSON output | Wrapper provides schema versioning and execution ID tracking for tool integration |
|atty| is-terminal | atty is older but widely used; is-terminal is newer but both work fine |

**Installation:**
No new dependencies. All required libraries already installed from previous phases.

## Architecture Patterns

### CLI Module Structure (Existing)

```
src/
├── main.rs              # Entry point, command dispatch
├── cli/
│   └── mod.rs           # CLI argument definitions, command handlers (stubs)
├── cfg/
│   ├── paths.rs         # Path enumeration (Phase 5, complete)
│   ├── dominators.rs    # Dominance analysis (Phase 4, complete)
│   ├── reachability.rs  # Unreachable code detection (Phase 3, complete)
│   ├── export.rs        # DOT/JSON export (Phase 2, complete)
│   └── mod.rs           # Re-exports
├── storage/
│   ├── paths.rs         # Path caching (Phase 5, complete)
│   └── mod.rs           # Database connection (Phase 1, complete)
└── output/
    └── mod.rs           # Output formatting utilities (complete)
```

### Pattern 1: Clap Derive API with Subcommands

**What:** Use `#[derive(Parser)]` and `#[derive(Subcommand)]` for CLI structure.

**When to use:** All CLI command definitions.

**Why:** Derive API provides:
- Type-safe argument parsing
- Automatic `--help` generation
- Compile-time verification
- Clean separation of concerns

**Example from existing code:**

```rust
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug, Clone)]
#[command(name = "mirage")]
#[command(author, version, about)]
pub struct Cli {
    /// Path to the Magellan/Mirage database
    #[arg(global = true, long, env = "MIRAGE_DB")]
    pub db: Option<String>,

    /// Output format
    #[arg(global = true, long, value_enum, default_value_t = OutputFormat::Human)]
    pub output: OutputFormat,

    #[command(subcommand)]
    pub command: Commands,
}

/// Output format options
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text output
    Human,
    /// Compact JSON for programmatic consumption
    Json,
    /// Formatted JSON with indentation
    Pretty,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Show all execution paths through a function
    Paths(PathsArgs),
    /// Show control-flow graph for a function
    Cfg(CfgArgs),
    /// Show dominance relationships for a function
    Dominators(DominatorsArgs),
    /// Find unreachable code within functions
    Unreachable(UnreachableArgs),
    /// Verify a path is still valid
    Verify(VerifyArgs),
    // ... other commands
}

#[derive(Parser, Debug, Clone)]
pub struct PathsArgs {
    /// Function symbol ID or name
    #[arg(long)]
    pub function: String,

    /// Show only error paths
    #[arg(long)]
    pub show_errors: bool,

    /// Maximum path length (for pruning)
    #[arg(long)]
    pub max_length: Option<usize>,

    /// Show block details for each path
    #[arg(long)]
    pub with_blocks: bool,
}
```

### Pattern 2: Command Handler with (args, &Cli) Signature

**What:** Commands needing global context receive both args and Cli reference.

**When to use:** Commands that need global options (db path, output format).

**Why:** This pattern enables:
- Access to global options (db, output format)
- Clean separation between local and global arguments
- Consistent with Magellan's pattern

**Example from existing `cfg` command:**

```rust
pub fn cfg(args: &CfgArgs, cli: &Cli) -> Result<()> {
    use crate::cfg::{export_dot, export_json};

    // For now, create a test CFG
    // In future, we'll load from database
    let cfg = create_test_cfg();

    // Determine output format
    let format = args.format.unwrap_or(match cli.output {
        OutputFormat::Human => CfgFormat::Human,
        OutputFormat::Json => CfgFormat::Json,
        OutputFormat::Pretty => CfgFormat::Json,
    });

    match format {
        CfgFormat::Human | CfgFormat::Dot => {
            let dot = export_dot(&cfg);
            println!("{}", dot);
        }
        CfgFormat::Json => {
            let export = export_json(&cfg, &args.function);
            let json = serde_json::to_string_pretty(&export)?;
            println!("{}", json);
        }
    }

    Ok(())
}
```

### Pattern 3: JsonResponse Wrapper for JSON Output

**What:** Wrap all JSON responses in `JsonResponse<T>` with metadata.

**When to use:** All JSON output (not human mode).

**Why:** Wrapper provides:
- Schema version for parsing stability
- Execution ID for traceability
- Tool name identification
- Timestamp for debugging

**Example from existing output module:**

```rust
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
            schema_version: "1.0.0".to_string(),
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
```

### Pattern 4: Database Path Resolution Priority

**What:** Resolve database path from CLI arg > env var > default.

**When to use:** All commands that need database access.

**Why:** Provides:
- User override via CLI
- Configuration via environment
- Sensible default for convenience

**Example from existing code:**

```rust
/// Resolve the database path from multiple sources
///
/// Priority: CLI arg > MIRAGE_DB env var > default "./codemcp.db"
/// This follows Magellan's pattern for database path resolution.
pub fn resolve_db_path(cli_db: Option<String>) -> anyhow::Result<String> {
    match cli_db {
        Some(path) => Ok(path),
        None => std::env::var("MIRAGE_DB")
            .or_else(|_| Ok("./codemcp.db".to_string())),
    }
}
```

### Pattern 5: Three-Tier Output Format Handling

**What:** Support human, compact JSON, and pretty JSON output.

**When to use:** All commands with data output.

**Why:** Provides:
- Human-readable for interactive use
- Compact JSON for piping to other tools
- Pretty JSON for debugging/development

**Example from status command:**

```rust
pub fn status(_args: StatusArgs, cli: &Cli) -> Result<()> {
    use crate::storage::MirageDb;

    let db_path = super::resolve_db_path(cli.db.clone())?;
    let db = match MirageDb::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            output::error(&format!("Failed to open database: {}", e));
            output::info("Hint: Run 'mirage index' to create the database");
            std::process::exit(output::EXIT_DATABASE);
        }
    };

    let status = db.status()?;

    match cli.output {
        OutputFormat::Human => {
            println!("Mirage Database Status:");
            println!("  Schema version: {} (Magellan: {})",
                status.mirage_schema_version,
                status.magellan_schema_version);
            println!("  cfg_blocks: {}", status.cfg_blocks);
            println!("  cfg_edges: {}", status.cfg_edges);
            println!("  cfg_paths: {}", status.cfg_paths);
            println!("  cfg_dominators: {}", status.cfg_dominators);
        }
        OutputFormat::Json => {
            let response = output::JsonResponse::new(status);
            println!("{}", response.to_json());
        }
        OutputFormat::Pretty => {
            let response = output::JsonResponse::new(status);
            println!("{}", response.to_pretty_json());
        }
    }

    Ok(())
}
```

### Anti-Patterns to Avoid

- **Mixing output logic with business logic:** Keep formatting separate from analysis.
- **Hardcoding database paths:** Always use `resolve_db_path()` for consistency.
- **Ignoring global output format:** Commands must respect `--output` flag.
- **Inconsistent error handling:** Use `output::error()` and proper exit codes.
- **Returning raw structs without JsonResponse wrapper:** JSON mode requires metadata.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CLI argument parsing | Manual env var parsing, string splitting | `clap` derive API | Handles validation, help generation, errors automatically |
| JSON serialization | Manual format strings | `serde_json` + `JsonResponse` wrapper | Handles escaping, types, versioning |
| Terminal detection | Manual termios checks | `atty` (already in use) | Cross-platform, handles edge cases |
| Error messages | println! to stderr | `output::error()` with exit codes | Consistent formatting, proper exit codes |
| Color output | Manual ANSI codes | `output` module color constants | Handles terminal detection |

**Key insight:** CLI UX is subtle. Wrong terminal detection, broken pipe handling, or inconsistent error messages create poor user experience. Use established patterns.

## Common Pitfalls

### Pitfall 1: Database Connection Errors Without Context

**What goes wrong:** "database is locked" or "no such table" errors without explanation.

**Why it happens:** Not checking if database exists before opening, not providing hints.

**How to avoid:**

```rust
// WRONG: Bare error
let db = MirageDb::open(&db_path)?;

// CORRECT: Contextual error with hint
let db = match MirageDb::open(&db_path) {
    Ok(db) => db,
    Err(e) => {
        output::error(&format!("Failed to open database: {}", e));
        output::info("Hint: Run 'mirage index' to create the database");
        std::process::exit(output::EXIT_DATABASE);
    }
};
```

**Warning signs:** Users reporting cryptic database errors, confusion about first-time setup.

### Pitfall 2: Ignoring Global Output Format

**What goes wrong:** Command outputs JSON when `--output human` is set, or vice versa.

**Why it happens:** Not checking `cli.output` before formatting.

**How to avoid:**

```rust
// WRONG: Always prints human output
pub fn paths(args: PathsArgs) -> Result<()> {
    for path in paths {
        println!("Path: {:?}", path.blocks);
    }
    Ok(())
}

// CORRECT: Respect global output format
pub fn paths(args: PathsArgs, cli: &Cli) -> Result<()> {
    let paths = get_paths(&args)?;

    match cli.output {
        OutputFormat::Human => print_paths_human(&paths),
        OutputFormat::Json => print_paths_json(&paths),
        OutputFormat::Pretty => print_paths_pretty(&paths),
    }
    Ok(())
}
```

**Warning signs:** Inconsistent output formats between commands, user confusion.

### Pitfall 3: Blocking on Large Path Enumerations

**What goes wrong:** CLI hangs for seconds enumerating millions of paths.

**Why it happens:** Not applying limits, not checking path count before enumeration.

**How to avoid:**

```rust
// WRONG: Unbounded enumeration
let paths = enumerate_paths(&cfg, &PathLimits::default());

// CORRECT: Apply user limits and estimate first
let limits = PathLimits {
    max_length: args.max_length.unwrap_or(1000),
    max_paths: args.max_paths.unwrap_or(10000),
    ..Default::default()
};

// Estimate first to warn user
if let Some(estimate) = estimate_path_count(&cfg, &limits) {
    if estimate > limits.max_paths {
        output::warn(&format!(
            "Function may have >{} paths. Consider using --max-paths or --max-length.",
            limits.max_paths
        ));
    }
}

let paths = enumerate_paths(&cfg, &limits);
```

**Warning signs:** Commands taking >10 seconds, OOM errors, user complaints about hangs.

### Pitfall 4: Missing Path Validation

**What goes wrong:** `mirage verify --path-id XYZ` doesn't check if path still exists.

**Why it happens:** Not re-running enumeration to verify path validity.

**How to avoid:**

```rust
// WRONG: Just check if path_id exists in database
pub fn verify(args: VerifyArgs) -> Result<()> {
    let exists = db.path_exists(&args.path_id)?;
    println!("{}", exists);
    Ok(())
}

// CORRECT: Re-enumerate and check if path still valid
pub fn verify(args: VerifyArgs) -> Result<()> {
    let cached = db.get_path(&args.path_id)?;

    // Re-enumerate function to verify path still exists
    let current_paths = enumerate_paths(&cfg, &PathLimits::default())?;

    let still_valid = current_paths.iter().any(|p| p.path_id == args.path_id);

    let result = VerifyResponse {
        path_id: args.path_id,
        valid: still_valid,
        found_in_cache: cached.is_some(),
    };

    match cli.output {
        OutputFormat::Human => println!("Path {}: {}", args.path_id,
            if still_valid { "valid" } else { "invalid" }),
        OutputFormat::Json => println!("{}", serde_json::to_string(&result)?),
        // ...
    }

    Ok(())
}
```

**Warning signs:** Verify command always returns true even after code changes.

### Pitfall 5: Inconsistent Exit Codes

**What goes wrong:** All errors exit with code 1, making it hard to distinguish error types.

**Why it happens:** Using `std::process::exit(1)` everywhere or `?` without context.

**How to avoid:**

```rust
// Already defined in output module
pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_ERROR: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_DATABASE: i32 = 3;
pub const EXIT_FILE_NOT_FOUND: i32 = 4;
pub const EXIT_VALIDATION: i32 = 5;

// Usage
let db = match MirageDb::open(&db_path) {
    Ok(db) => db,
    Err(_) => std::process::exit(output::EXIT_DATABASE),
};
```

**Warning signs:** Shell scripts can't distinguish between usage errors and database errors.

## Code Examples

Verified patterns from official sources:

### Clap Subcommand with Global Options

```rust
// Source: /websites/rs_clap - Subcommands with clap Derive

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Global flag available to all subcommands
    #[arg(global = true, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Adds files to myapp
    Add {
        /// Name of the item to add
        name: Option<String>
    },
}

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Add { name } => {
            if cli.verbose {
                println!("Adding {:?}", name);
            }
        }
    }
}
```

### ValueEnum for Output Format

```rust
// Source: /websites/rs_clap - ValueEnum documentation

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text output
    Human,
    /// Compact JSON for programmatic consumption
    Json,
    /// Formatted JSON with indentation
    Pretty,
}

// Used in argument definition
#[arg(long, value_enum, default_value_t = OutputFormat::Human)]
pub output: OutputFormat,
```

### JsonResponse Wrapper Pattern

```rust
// Source: Existing Mirage code (Magellan-compatible)

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PathsResponse {
    pub function_id: String,
    pub function_name: Option<String>,
    pub path_count: usize,
    pub paths: Vec<PathSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PathSummary {
    pub path_id: String,
    pub kind: String,  // "Normal", "Error", etc.
    pub length: usize,
    pub entry: usize,
    pub exit: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<usize>>,
}

// Usage in command
pub fn paths(args: PathsArgs, cli: &Cli) -> Result<()> {
    let paths = get_paths(&args)?;
    let response = PathsResponse { /* ... */ };

    match cli.output {
        OutputFormat::Json => {
            let wrapped = JsonResponse::new(response);
            println!("{}", wrapped.to_json());
        }
        OutputFormat::Pretty => {
            let wrapped = JsonResponse::new(response);
            println!("{}", wrapped.to_pretty_json());
        }
        OutputFormat::Human => {
            // Human-readable formatting
            println!("Found {} paths for {}", paths.len(), args.function);
        }
    }

    Ok(())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual string parsing for CLI | clap derive API | 2019+ | Type-safe, compile-time verified, auto-generated help |
| Custom JSON formats | Standardized JsonResponse wrapper | 2020s+ | Tool interoperability, schema versioning |
| Single output format | Multiple format support (human/json/pretty) | 2010s+ | Both human-friendly and machine-readable |
| Inconsistent error codes | Standardized exit codes | POSIX | Scriptable error handling |

**Deprecated/outdated:**
- **Manual argument parsing:** Prone to bugs, no help generation. Use clap.
- **println! for structured output:** Can't be parsed by tools. Use JsonResponse for JSON mode.
- **Ignoring terminal capabilities:** Always printing colors looks bad in logs. Use `atty` detection.

## Open Questions

Things that couldn't be fully resolved:

1. **Optimal path display for large result sets**
   - What we know: Pagination isn't standard in CLI tools, most just dump all results.
   - What's unclear: Whether to implement `--limit` and `--offset` for path queries.
   - Recommendation: Skip pagination for Phase 6, add `--limit` flag if users request it.

2. **Path verification semantics after code changes**
   - What we know: Path can become invalid if blocks are added/removed.
   - What's unclear: Whether "valid" means "same path_id exists" or "path still reaches same exit".
   - Recommendation: Simple validity check (path_id exists) for Phase 6. Semantic validity can be added later.

3. **Interactive vs non-interactive error messages**
   - What we know: Attty detection exists, but some tools pipe to pager (less) which changes terminal detection.
   - What's unclear: Whether to detect pager and adjust output.
   - Recommendation: Skip pager detection for Phase 6. Attty is sufficient for most cases.

4. **DOT output streaming for large CFGs**
   - What we know: Large functions can generate 1000+ line DOT files.
   - What's unclear: Whether to stream DOT output incrementally vs build full string.
   - Recommendation: Build full string (current approach). Streaming optimization only if profiling shows need.

## Sources

### Primary (HIGH confidence)

- **/websites/rs_clap** - clap command-line argument parser
  - `#[derive(Parser)]` - Derive API for CLI structure
  - `#[derive(Subcommand)]` - Subcommand enumeration
  - `#[derive(ValueEnum)]` - Type-safe enum arguments
  - `#[arg(global = true)]` - Global flag syntax
  - Topics fetched: Subcommands, derive API, ValueEnum, global options, output formatting

- **Existing Mirage codebase** - /home/feanor/Projects/mirage
  - `src/cli/mod.rs` - CLI argument definitions (complete, just needs implementation)
  - `src/output/mod.rs` - Output utilities, JsonResponse wrapper
  - `src/storage/mod.rs` - Database connection, status query
  - Verified: All CLI stubs exist, database layer works, output utilities ready

- **Magellan codebase** - /home/feanor/Projects/magellan
  - `src/files_cmd.rs` - Example of simple list command with JSON output
  - `src/query_cmd.rs` - Example of complex query with format handling
  - `src/output/command.rs` - JsonResponse wrapper pattern, Span model
  - Verified: Output patterns, error handling, exit code conventions

### Secondary (MEDIUM confidence)

- [Rust CLI Book - Command Line Arguments in Rust](https://rust-cli.github.io/book/index.html)
  - What was checked: clap patterns, output conventions, error handling
  - Verified: clap derive API is standard approach, exit code patterns

### Tertiary (LOW confidence)

- None - All research based on official documentation and existing codebase

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - clap 4.5 is documented, already in use, existing code compiles
- Architecture: HIGH - Existing CLI structure is complete, just needs handler implementation
- Output formatting: HIGH - JsonResponse wrapper exists and is tested
- Database integration: HIGH - All storage APIs complete from Phases 1-5
- Command patterns: HIGH - Magellan provides working examples to follow

**Research date:** 2026-02-01
**Valid until:** 2026-03-01 (30 days - stable domain, CLI patterns are well-established)

**Magellan snapshot:**
- Not applicable - research based on code reading, not execution

**Planner readiness:** This research provides sufficient detail for gsd-planner to create executable plans for Phase 6. All CLI stubs exist, backend APIs are complete, output patterns are defined. The phase is primarily about connecting existing pieces with proper formatting and error handling.
