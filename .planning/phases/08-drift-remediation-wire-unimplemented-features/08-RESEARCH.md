# Phase 08: Drift Remediation - Wire Unimplemented Features - Research

**Researched:** 2026-02-02
**Domain:** CLI implementation, Feature wiring, Test fixing
**Confidence:** HIGH

## Summary

Phase 08 focuses on implementing stub commands, wiring unused but implemented functions to CLI commands, fixing documentation tests, and implementing placeholder flags. This is a **wiring and integration phase**, not a new feature development phase - all underlying algorithms and data structures are already implemented and tested.

The primary work involves:
1. **Stub commands**: `mirage index` and `mirage blast-zone` currently exit with error messages
2. **Unused CFG functions**: `detect_natural_loops`, `detect_if_else_patterns`, `detect_match_patterns`, `compute_dominance_frontiers`, `enumerate_paths_cached` are implemented but have no CLI commands
3. **Unused storage functions**: Path caching functions exist but aren't called from CLI
4. **Doctest failures**: 34 doctests fail due to variable naming collision with Rust's `cfg!` macro
5. **Placeholder flag**: `--show-branches` flag shows placeholder message instead of branch details

**Primary recommendation:** This is a straightforward wiring phase. Follow existing CLI command patterns, avoid new architecture, and focus on exposing existing functionality through CLI interface.

## Standard Stack

### Core (Already in Use)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.x | CLI argument parsing | Already in use, de facto standard for Rust CLIs |
| anyhow | 1.x | Error handling | Already in use, provides context for errors |
| petgraph | 0.6 | Graph algorithms | Already in use for CFG analysis |
| rusqlite | 0.32 | Database access | Already in use for path caching |
| serde | 1.x | JSON serialization | Already in use for LLM output |

### No New Dependencies

This phase requires **no new dependencies**. All required libraries are already in the codebase. The work is purely integration and wiring.

## Architecture Patterns

### CLI Command Pattern (Already Established)

**What:** All CLI commands follow the same structure in `src/cli/mod.rs`

**Pattern:**
```rust
pub fn function_name(args: ArgsStruct, cli: &Cli) -> Result<()> {
    // 1. Resolve database path
    let db_path = super::resolve_db_path(cli.db.clone())?;

    // 2. Open database with error handling
    let db = match MirageDb::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            // JSON-aware error handling
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::database_not_found(&db_path);
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Failed: {}", e));
                std::process::exit(1);
            }
        }
    };

    // 3. Call analysis function
    let results = analysis_function(args, &db);

    // 4. Output based on format
    match cli.output {
        OutputFormat::Human => { /* human output */ }
        OutputFormat::Json | OutputFormat::Pretty => { /* JSON output */ }
    }

    Ok(())
}
```

**When to use:** All new CLI commands must follow this pattern exactly.

### Error Handling Pattern

**What:** Database errors handled consistently with JSON support

**Example:** (from `src/cli/mod.rs:420-435`)
```rust
let db = match MirageDb::open(&db_path) {
    Ok(db) => db,
    Err(_e) => {
        if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
            let error = output::JsonError::database_not_found(&db_path);
            let wrapper = output::JsonResponse::new(error);
            println!("{}", wrapper.to_json());
            std::process::exit(output::EXIT_DATABASE);
        } else {
            output::error(&format!("Failed to open database: {}", db_path));
            output::info("Hint: Run 'mirage index' to create the database");
            std::process::exit(output::EXIT_DATABASE);
        }
    }
};
```

**Why:** Provides JSON output for LLM consumption while maintaining human-readable messages.

### JSON Output Pattern

**What:** Structured JSON output using `JsonResponse` wrapper

**Example:**
```rust
let response = StructuredResponse {
    function: args.function.clone(),
    total_items: items.len(),
    items,
};
let wrapper = output::JsonResponse::new(response);

match cli.output {
    OutputFormat::Json => println!("{}", wrapper.to_json()),
    OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
    _ => {}
}
```

**Why:** All JSON output includes `schema_version`, `execution_id`, `tool`, `timestamp` for LLM consumption.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CLI argument parsing | Custom parsing | clap (already in use) | Already configured, provides validation |
| Error formatting | Manual error strings | `output::JsonError` (exists) | Consistent JSON schema |
| Database connections | Direct rusqlite | `MirageDb::open()` (exists) | Handles migrations, validation |
| Path enumeration | Re-implement | `cfg::enumerate_paths()` (exists) | Already tested, cached |
| Loop detection | Re-implement | `cfg::loops::detect_natural_loops()` (exists) | Already implemented |
| Pattern detection | Re-implement | `cfg::patterns::detect_*()` (exists) | Already implemented |
| Dominance frontiers | Re-implement | `cfg::dominance_frontiers::compute_dominance_frontiers()` (exists) | Already implemented |

**Key insight:** All analysis functions already exist. This phase is purely about wiring them to CLI commands.

## Common Pitfalls

### Pitfall 1: Naming Collision with `cfg!` Macro

**What goes wrong:** Using `cfg` as variable name in doctests causes compilation errors
```rust
/// let cfg = build_cfg();  // ERROR: expected value, found macro `cfg`
/// let loops = detect_natural_loops(&cfg);
```

**Why it happens:** Rust has a built-in `cfg!` macro for conditional compilation. The parser sees `cfg` and tries to parse it as a macro invocation.

**How to avoid:** Use alternative names in doctests:
- `graph` (preferred, short and clear)
- `control_flow_graph` (verbose but unambiguous)
- `func_cfg` (function-specific)

**Warning signs:** Doctest errors with "expected value, found macro `cfg`"

### Pitfall 2: Forgetting Database Error Handling

**What goes wrong:** New CLI commands crash instead of showing helpful errors when database doesn't exist

**Why it happens:** Copying function bodies but forgetting the `match MirageDb::open()` pattern

**How to avoid:** Always copy the full database opening pattern from existing commands (see src/cli/mod.rs:420-435)

### Pitfall 3: Inconsistent JSON Output Schema

**What goes wrong:** New commands return plain JSON without `JsonResponse` wrapper

**Why it happens:** Directly serializing structs instead of wrapping them

**How to avoid:** Always use `JsonResponse::new(data)` before serializing

### Pitfall 4: Not Allowing for Test Data

**What goes wrong:** CLI commands can't be tested because they require a real database

**How to avoid:** Follow existing pattern of accepting `db_path` parameter, use in-memory databases in tests

### Pitfall 5: Ignoring Path Caching

**What goes wrong:** Path enumeration commands always re-enumerate instead of using cache

**Why it happens:** Calling `enumerate_paths()` instead of `get_or_enumerate_paths()`

**How to avoid:** Check if a cached variant exists in `cfg::paths` module before using direct enumeration

## Code Examples

### Adding a New CLI Command

**Source:** Existing pattern in src/cli/mod.rs

```rust
// 1. Add argument struct
#[derive(Parser, Debug, Clone)]
pub struct LoopsArgs {
    /// Function to analyze
    #[arg(long)]
    pub function: String,
}

// 2. Add command variant
#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    // ... existing commands ...
    /// Show natural loops in CFG
    Loops(LoopsArgs),
}

// 3. Implement command handler
pub fn loops(args: LoopsArgs, cli: &Cli) -> Result<()> {
    use crate::storage::MirageDb;
    use crate::cfg::loops::detect_natural_loops;

    let db_path = super::resolve_db_path(cli.db.clone())?;
    let db = match MirageDb::open(&db_path) {
        Ok(db) => db,
        Err(_e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::database_not_found(&db_path);
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Failed to open database: {}", db_path));
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    // Query CFG from database (pseudo-code, adjust based on actual schema)
    // let cfg = load_cfg_from_db(&db, &args.function)?;

    // Call analysis function
    let loops = detect_natural_loops(&cfg);

    // Output based on format
    match cli.output {
        OutputFormat::Human => {
            println!("Natural loops in {}:", args.function);
            for loop_ in &loops {
                println!("  Loop header: {:?}", loop_.header);
                println!("  Loop body size: {}", loop_.size());
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let response = LoopsResponse {
                function: args.function.clone(),
                loop_count: loops.len(),
                loops,
            };
            let wrapper = output::JsonResponse::new(response);

            match cli.output {
                OutputFormat::Json => println!("{}", wrapper.to_json()),
                OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                _ => {}
            }
        }
    }

    Ok(())
}
```

### Implementing --show-branches

**Current placeholder:** src/cli/mod.rs:1115
```rust
if args.show_branches {
    output::info("Branch details: Use --show-branches to see incoming edges (not yet implemented)");
}
```

**Implementation:**
```rust
if args.show_branches {
    // For each unreachable block, find its incoming edges
    for block in &blocks {
        let incoming: Vec<_> = cfg
            .edge_references()
            .filter(|edge| edge.target() == block.node_index)
            .map(|edge| {
                let source_block = &cfg[edge.source()];
                let edge_type = cfg.edge_weight(edge.id()).unwrap();
                (source_block.id, edge_type)
            })
            .collect();

        println!("    Block {} incoming edges:", block.block_id);
        for (source_id, edge_type) in incoming {
            println!("      from {} ({:?})", source_id, edge_type);
        }
        println!();
    }
}
```

### Fixing Doctest Variable Names

**Before (fails):**
```rust
/// Detect natural loops in a CFG
///
/// # Example
/// ```rust
/// let cfg = build_test_cfg();
/// let loops = detect_natural_loops(&cfg);
/// ```
```

**After (works):**
```rust
/// Detect natural loops in a CFG
///
/// # Example
/// ```rust
/// let graph = build_test_cfg();
/// let loops = detect_natural_loops(&graph);
/// ```
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Analysis functions unused | Wire to CLI commands | Phase 8 | Users can access implemented features |
| Doctest failures with `cfg` variable | Use `graph` or `func_cfg` | Phase 8 | Documentation compiles |
| Placeholder `--show-branches` | Actual incoming edge display | Phase 8 | Unreachable analysis shows branch context |
| Path caching functions unused | Wire `get_or_enumerate_paths()` | Phase 8 | Performance improvement for repeated queries |

**No deprecated features in this phase** - all work is additive wiring.

## Open Questions

### Question 1: MIR Extraction Implementation Approach

**What we know:**
- `mirage index` command stub exists at src/cli/mod.rs:407
- Charon integration module exists at src/mir/charon.rs
- ULLBC to CFG conversion exists at src/cfg/mir.rs

**What's unclear:**
- Should `mirage index` call Charon binary or use library API?
- How to handle incremental updates (--incremental flag)?
- Should we implement full rustc integration or AST fallback first?

**Recommendation:**
- Start with AST-based CFG (already implemented in src/cfg/ast.rs)
- Use `tree-sitter` to parse source files
- Build CFG without requiring Charon/rustc
- Defer MIR-specific features to Milestone 2
- This allows `mirage index` to work immediately for basic CFG construction

### Question 2: Path Caching Integration

**What we know:**
- `enumerate_paths_cached()` exists in src/cfg/paths.rs
- Storage functions (`get_cached_paths`, `store_paths`) exist in src/storage/paths.rs
- Functions are tested but not called from CLI

**What's unclear:**
- Should path caching be automatic or explicit (via flag)?
- How to handle cache invalidation when code changes?

**Recommendation:**
- Use `get_or_enumerate_paths()` which handles cache automatically
- Check function hash before re-enumerating (already implemented)
- No user-facing changes needed - internal optimization only

### Question 3: Database Schema for Storing CFGs

**What we know:**
- Database has `cfg_blocks` and `cfg_edges` tables
- No clear API for storing/loading full CFG structures

**What's unclear:**
- How to serialize petgraph DiGraph to database?
- Should we store adjacency list or edge list?

**Recommendation:**
- Store edges as (from_block_id, to_block_id, edge_type) tuples
- Reconstruct DiGraph on load using add_edge() calls
- Use existing `cfg_blocks` table for block metadata
- Keep in-memory CFG for analysis, database for persistence

## Sources

### Primary (HIGH confidence)

**Codebase inspection (all findings):**
- src/cli/mod.rs - CLI command structure, stub commands
- src/cfg/loops.rs - `detect_natural_loops()` implementation
- src/cfg/patterns.rs - `detect_if_else_patterns()`, `detect_match_patterns()` implementation
- src/cfg/dominance_frontiers.rs - `compute_dominance_frontiers()` implementation
- src/cfg/paths.rs - `enumerate_paths_cached()`, `get_or_enumerate_paths()` implementation
- src/storage/paths.rs - Path caching functions (store, get, invalidate)
- .planning/v1.0-MILESTONE-AUDIT.md - Gap analysis, technical debt inventory
- .planning/REQUIREMENTS.md - Requirements traceability

**Existing documentation:**
- scripts/CODE_ANALYSIS_CAPABILITIES.md - Feature availability matrix

### Secondary (MEDIUM confidence)

None required for this phase - all information from codebase inspection.

### Tertiary (LOW confidence)

None - no external research needed for wiring phase.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries already in use, no new dependencies
- Architecture: HIGH - Established patterns in codebase, follow existing structure
- Pitfalls: HIGH - Doctest issue documented in audit, other pitfalls from common mistakes

**Research date:** 2026-02-02
**Valid until:** 30 days (stable phase, no external dependencies)

**Database snapshot:**
- execution_id: 697feb36-29e725 (from magellan status)
- Files: 17
- Symbols: 237
- References: 413
- Calls: 62

**Key files referenced:**
- src/cli/mod.rs (stub commands at lines 407, 1268; placeholder at line 1115)
- src/cfg/mod.rs (exports for unused functions)
- src/storage/mod.rs (path caching exports)
- .planning/v1.0-MILESTONE-AUDIT.md (gap analysis)
