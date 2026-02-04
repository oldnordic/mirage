# Phase 12: Magellan CFG Integration - Research

**Researched:** 2026-02-04
**Domain:** Control Flow Graph extraction and database schema migration
**Confidence:** HIGH

## Summary

This phase replaces Mirage's Charon-based MIR extraction with Magellan's AST-based CFG extraction. The key finding is that **Magellan v2.1+ (schema v7) now provides `cfg_blocks` table**, which stores basic block information extracted from AST nodes. This eliminates the need for Charon as an external binary dependency.

**Primary recommendation:** Read CFG data directly from Magellan's `cfg_blocks` table and construct edges from block terminators. Remove all Charon-related code including `src/mir/charon.rs` and the MIR-based `ullbc_to_cfg` conversion.

**Key insight:** Magellan's CFG is AST-based (not MIR-based), which means:
- Pros: No external dependency, works on stable Rust, multi-language support
- Cons: Less precise than MIR (no macro expansion, generic monomorphization, async desugaring)

This is an **acceptable trade-off** because:
1. AST-based CFG is sufficient for most analyses (path enumeration, dominance, complexity)
2. Removes a complex external dependency (Charon)
3. Unifies the extraction pipeline (single source of truth)
4. Multi-language support via tree-sitter

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| **magellan** | 2.1+ | Graph indexing with CFG blocks | Provides cfg_blocks table (schema v7) |
| **sqlitegraph** | 1.3 | Graph database storage | Shared database format |
| **rusqlite** | 0.31 | SQLite access | Direct database queries |
| **petgraph** | 0.8 | In-memory CFG representation | Graph algorithms for analysis |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **tree-sitter** | 0.22 | AST parsing | Already used by Magellan internally |
| **serde** | 1.0 | Serialization | Terminator storage as JSON |
| **anyhow** | 1.0 | Error handling | Standard error propagation |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Magellan CFG | Charon MIR | Charon requires external binary, nightly Rust, single-language only |
| Direct SQL query | ORM (Diesel) | SQL is simpler for read-only queries, less overhead |
| cfg_blocks only | cfg_blocks + cfg_edges | Magellan stores blocks; edges derived from terminators |

**Installation:**
```bash
# Already satisfied - Mirage depends on magellan 2.0
# Upgrade to 2.1+ when available for cfg_blocks support
cargo update magellan
```

## Architecture Patterns

### Current Architecture (Charon-based)

```
┌─────────────────────────────────────────────────────────────┐
│  Mirage Index Command                                       │
│  1. Run charon binary (external process)                   │
│  2. Parse ULLBC JSON output                                 │
│  3. Convert ULLBC to Mirage CFG                             │
│  4. Store in Mirage's cfg_blocks/cfg_edges tables          │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  External: Charon Binary                                    │
│  - Requires nightly Rust                                   │
│  - Extracts MIR from rustc                                  │
│  - Outputs ULLBC JSON                                       │
└─────────────────────────────────────────────────────────────┘
```

### Target Architecture (Magellan-based)

```
┌─────────────────────────────────────────────────────────────┐
│  Magellan Watch (already runs separately)                   │
│  1. Parse source files with tree-sitter                     │
│  2. Extract symbols and AST nodes                           │
│  3. Build cfg_blocks from AST                               │
│  4. Store in shared database                                 │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  Mirage (reads from Magellan database)                      │
│  1. Read cfg_blocks from database                           │
│  2. Construct edges from terminators                        │
│  3. Build in-memory Cfg (petgraph)                          │
│  4. Run analysis (paths, dominators, etc.)                  │
└─────────────────────────────────────────────────────────────┘
```

### Pattern 1: Schema Migration

**What:** Mirage has its own `cfg_blocks` table. Magellan v7 also has `cfg_blocks`.

**When to use:** When reading CFG data, use Magellan's table directly.

**Migration approach:**
```rust
// OLD: Read from Mirage's cfg_blocks
let blocks = conn.prepare(
    "SELECT id, block_kind, byte_start, byte_end, terminator
     FROM cfg_blocks WHERE function_id = ?"
)?;

// NEW: Read from Magellan's cfg_blocks (same schema!)
let blocks = conn.prepare(
    "SELECT id, kind, terminator, byte_start, byte_end,
            start_line, start_col, end_line, end_col
     FROM cfg_blocks WHERE function_id = ?"
)?;
```

**Schema differences:**

| Field | Magellan v7 | Mirage v1 | Mapping |
|-------|-------------|-----------|---------|
| id | id | id | Same (auto-increment) |
| kind | kind | block_kind | Different name |
| terminator | terminator | terminator | Same |
| byte_start | byte_start | byte_start | Same |
| byte_end | byte_end | byte_end | Same |
| start_line | start_line | (missing) | New in Magellan |
| start_col | start_col | (missing) | New in Magellan |
| end_line | end_line | (missing) | New in Magellan |
| end_col | end_col | (missing) | New in Magellan |

### Pattern 2: Edge Construction from Terminators

**What:** Magellan stores blocks with terminators; edges are derived from terminator information.

**When to use:** When building in-memory CFG for analysis.

**Example:**
```rust
// Source: Magellan cfg_extractor.rs
// Terminator kinds: "fallthrough", "conditional", "goto", "return", etc.

// Build edges from terminator data
match block.terminator.as_str() {
    "fallthrough" => {
        // Single edge to next block (sequential)
        graph.add_edge(from_idx, to_idx, EdgeType::Fallthrough);
    }
    "conditional" => {
        // Two edges: true and false branches
        graph.add_edge(from_idx, true_idx, EdgeType::TrueBranch);
        graph.add_edge(from_idx, false_idx, EdgeType::FalseBranch);
    }
    "goto" => {
        // Unconditional jump
        graph.add_edge(from_idx, target_idx, EdgeType::Fallthrough);
    }
    "return" => {
        // No outgoing edges (exit block)
    }
    // ... other terminators
}
```

### Anti-Patterns to Avoid

- **Don't keep Charon integration:** "Just in case we need MIR later" - If MIR is needed in future, use stable_mir (not Charon)
- **Don't duplicate cfg_blocks:** Mirage should not maintain its own cfg_blocks table
- **Don't run separate indexing:** `mirage index` should not re-extract what Magellan already provides

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CFG extraction | Custom AST parser | Magellan's CfgExtractor | Already handles all Rust constructs |
| Database schema | Custom migration | Use Magellan's schema directly | Shared database, single source of truth |
| Edge construction | Complex logic | Derive from terminators | Terminators encode all edge information |
| Error handling | Custom codes | anyhow::Context | Standard error propagation with context |

**Key insight:** Magellan's v7 schema already provides everything needed. Don't reinvent.

## Common Pitfalls

### Pitfall 1: Assuming cfg_edges Table Exists

**What goes wrong:** Code assumes `cfg_edges` table exists and contains pre-built edges.

**Why it happens:** Mirage's current schema has `cfg_edges`, but Magellan v7 does NOT (edges are in graph_edges with edge_type='CFG_BLOCK').

**How to avoid:** Read edges from `graph_edges WHERE edge_type = 'CFG_BLOCK'` or derive from terminators.

**Warning signs:**
- Query fails with "no such table: cfg_edges"
- Edge count is zero when blocks exist

### Pitfall 2: Block Kind Name Mismatch

**What goes wrong:** Code looks for `block_kind` column but Magellan uses `kind`.

**Why it happens:** Different schema naming conventions.

**How to avoid:** Use Magellan's column names (`kind`, not `block_kind`).

**Detection:**
```sql
-- Check schema before querying
PRAGMA table_info(cfg_blocks);
```

### Pitfall 3: Missing Source Location Data

**What goes wrong:** Code expects `byte_start`/`byte_end` but Magellan also provides line/column.

**Why it happens:** Mirage's old schema was byte-only; Magellan v7 includes line/column.

**How to avoid:** Read all location fields for richer source mapping.

### Pitfall 4: Terminator Format Differences

**What goes wrong:** Code expects JSON terminator but Magellan stores plain text.

**Why it happens:** Mirage stores terminators as JSON for Charon compatibility; Magellan uses simple strings.

**Detection:**
- `serde_json::from_str` fails on Magellan terminator values
- Terminator values are lowercase strings like "fallthrough", "conditional"

**Solution:** Map Magellan terminator strings to Mirage's Terminator enum.

## Code Examples

### Reading CFG from Magellan's Database

```rust
// Source: Derived from Magellan cfg_ops.rs and Mirage storage.rs

use rusqlite::params;
use petgraph::graph::DiGraph;

pub fn load_cfg_from_magellan(
    conn: &Connection,
    function_id: i64,
) -> Result<Cfg> {
    // Query blocks from Magellan's cfg_blocks table
    let mut stmt = conn.prepare_cached(
        "SELECT id, kind, terminator, byte_start, byte_end,
                start_line, start_col, end_line, end_col
         FROM cfg_blocks
         WHERE function_id = ?
         ORDER BY id ASC",
    )?;

    let block_rows: Vec<(i64, String, String, i64, i64, i64, i64, i64, i64)> =
        stmt.query_map(params![function_id], |row| {
            Ok((
                row.get(0)?, // id
                row.get(1)?, // kind
                row.get(2)?, // terminator
                row.get(3)?, // byte_start
                row.get(4)?, // byte_end
                row.get(5)?, // start_line
                row.get(6)?, // start_col
                row.get(7)?, // end_line
                row.get(8)?, // end_col
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if block_rows.is_empty() {
        anyhow::bail!(
            "No CFG blocks found for function_id {}. \
             Run 'magellan watch' to build CFGs.",
            function_id
        );
    }

    // Build CFG from blocks
    let mut graph = Cfg::new();
    let mut db_id_to_node = std::collections::HashMap::new();

    for (node_idx, (db_id, kind_str, terminator_str, byte_start, byte_end,
                     start_line, start_col, end_line, end_col)) in
        block_rows.iter().enumerate()
    {
        let kind = map_magellan_kind(kind_str)?;
        let terminator = map_magellan_terminator(terminator_str)?;

        let block = BasicBlock {
            id: node_idx,
            kind,
            statements: vec![], // Empty for AST-based CFG
            terminator,
            source_location: Some(SourceLocation {
                file_path: get_function_file(conn, function_id)?,
                byte_start: *byte_start as usize,
                byte_end: *byte_end as usize,
                start_line: *start_line as usize,
                start_column: *start_col as usize,
                end_line: *end_line as usize,
                end_column: *end_col as usize,
            }),
        };

        graph.add_node(block);
        db_id_to_node.insert(*db_id, node_idx);
    }

    // Build edges from terminators
    // (See Pattern 2 above)
    build_edges_from_terminators(&mut graph, &block_rows, &db_id_to_node)?;

    Ok(graph)
}

fn map_magellan_kind(kind: &str) -> Result<BlockKind> {
    match kind {
        "entry" => Ok(BlockKind::Entry),
        "if" | "else" | "loop" | "while" | "for" | "match_arm" | "block" => {
            Ok(BlockKind::Normal)
        }
        "return" => Ok(BlockKind::Exit),
        _ => anyhow::bail!("Unknown block kind: {}", kind),
    }
}

fn map_magellan_terminator(term: &str) -> Result<Terminator> {
    match term {
        "fallthrough" => Ok(Terminator::Goto { target: 0 }), // Target resolved later
        "conditional" => Ok(Terminator::SwitchInt {
            targets: vec![],
            otherwise: 0,
        }),
        "goto" => Ok(Terminator::Goto { target: 0 }),
        "return" => Ok(Terminator::Return),
        "break" => Ok(Terminator::Abort("break".to_string())),
        "continue" => Ok(Terminator::Abort("continue".to_string())),
        "call" => Ok(Terminator::Call {
            target: Some(0),
            unwind: None,
        }),
        "panic" => Ok(Terminator::Abort("panic".to_string())),
        _ => anyhow::bail!("Unknown terminator: {}", term),
    }
}
```

### Removing Charon Integration

```rust
// BEFORE: src/mir/charon.rs (DELETE THIS FILE)
// This entire module is no longer needed

// BEFORE: src/cli/mod.rs - index command
use crate::mir::{run_charon, parse_ullbc};
use crate::cfg::ullbc_to_cfg;

// Remove all Charon-related code:
// - run_charon() call
// - parse_ullbc() call
// - ullbc_to_cfg() call
// - Auto-install prompt for Charon

// AFTER: Simplified index command
pub fn index(args: &IndexArgs, cli: &Cli) -> Result<()> {
    let db_path = super::resolve_db_path(cli.db.clone())?;

    // Just verify Magellan has indexed the project
    let magellan_db_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cfg_blocks",
        [],
        |row| row.get(0),
    )?;

    if magellan_db_count == 0 {
        anyhow::bail!(
            "No CFG data found. Run 'magellan watch --db {}' first.",
            db_path
        );
    }

    output::success(&format!(
        "Using Magellan CFG data ({} blocks loaded)",
        magellan_db_count
    ));

    Ok(())
}
```

### Mapping Between Schemas

```rust
// Schema mapping adapter

/// Adapter for reading Magellan's cfg_blocks as Mirage's BasicBlock
pub struct MagellanCfgAdapter {
    conn: Connection,
}

impl MagellanCfgAdapter {
    pub fn load_function_cfg(&self, function_id: i64) -> Result<Cfg> {
        // Magellan v7 schema:
        // - kind (not block_kind)
        // - terminator as plain text (not JSON)
        // - Includes line/column info

        let blocks = self.query_blocks(function_id)?;

        // Map to Mirage's internal representation
        let cfg = self.blocks_to_cfg(blocks)?;

        Ok(cfg)
    }

    fn query_blocks(&self, function_id: i64) -> Result<Vec<MagellanBlock>> {
        // Direct SQL query to Magellan's cfg_blocks
        // No ORM, no abstraction - just the data
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, kind, terminator, byte_start, byte_end,
                    start_line, start_col, end_line, end_col
             FROM cfg_blocks
             WHERE function_id = ?
             ORDER BY id",
        )?;

        // ... query execution
        Ok(vec![])
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Charon MIR extraction | Magellan AST-based CFG | Phase 12 | Removes external dependency |
| Separate cfg_blocks tables | Shared Magellan schema | Phase 12 | Single source of truth |
| JSON ULLBC parsing | Direct SQL reads | Phase 12 | Simpler, faster |
| charon binary required | Only magellan required | Phase 12 | Easier installation |

**Deprecated/outdated:**
- **Charon integration:** `src/mir/charon.rs` - can be deleted
- **ULLBC parsing:** `src/mir/charon.rs::parse_ullbc` - can be deleted
- **MIR-to-CFG conversion:** `src/cfg/mir.rs::ullbc_to_cfg` - can be deleted
- **Auto-install Charon prompt:** In `src/cli/mod.rs::index` - can be removed

## Open Questions

### 1. Edge Storage Format (MEDIUM Confidence)

**What we know:** Magellan stores edges in `graph_edges` with `edge_type='CFG_BLOCK'`. Mirage has separate `cfg_edges` table.

**What's unclear:** Should we:
- Option A: Read edges from Magellan's `graph_edges`?
- Option B: Derive edges from terminators in memory?
- Option C: Keep Mirage's `cfg_edges` table for cached edges?

**Recommendation:** **Option B** - Derive edges from terminators. This is:
- Simpler (no edge table maintenance)
- More flexible (can analyze different edge interpretations)
- Consistent with how CFG algorithms work (terminators define edges)

### 2. Schema Version Requirement (HIGH Confidence)

**What we know:** Magellan schema v7 added `cfg_blocks`. Mirage currently requires Magellan schema v4.

**What's unclear:** Should we:
- Option A: Bump minimum to v7 (hard requirement)?
- Option B: Make CFG optional (graceful degradation)?

**Recommendation:** **Option A with fallback** - Require v7 for CFG features, but provide clear error message:
```rust
let magellan_version: i64 = conn.query_row(
    "SELECT magellan_schema_version FROM magellan_meta WHERE id = 1",
    [],
    |row| row.get(0),
)?;

if magellan_version < 7 {
    anyhow::bail!(
        "Magellan schema v{} is too old for CFG analysis. \
         Please update Magellan: cargo install magellan --force",
        magellan_version
    );
}
```

### 3. Test Fixture Migration (MEDIUM Confidence)

**What we know:** Current tests use Charon ULLBC fixtures.

**What's unclear:** Should we:
- Option A: Create Magellan-based fixtures?
- Option B: Mock database queries?
- Option C: Use real project indexing?

**Recommendation:** **Option A** - Create test databases with Magellon-extracted CFG. Use simple Rust functions as test cases.

## Sources

### Primary (HIGH confidence)

- **Magellan source code:**
  - `/home/feanor/Projects/magellan/src/graph/db_compat.rs` - Schema v7 definition
  - `/home/feanor/Projects/magellan/src/graph/cfg_ops.rs` - CFG block operations
  - `/home/feanor/Projects/magellan/src/graph/cfg_extractor.rs` - AST-based CFG extraction
  - `/home/feanor/Projects/magellan/src/graph/schema.rs` - CfgBlock struct definition
- **Mirage source code:**
  - `/home/feanor/Projects/mirage/src/storage/mod.rs` - Database operations
  - `/home/feanor/Projects/mirage/src/mir/charon.rs` - Charon integration (to be removed)
  - `/home/feanor/Projects/mirage/src/cfg/mir.rs` - ULLBC-to-CFG conversion (to be removed)
  - `/home/feanor/Projects/mirage/src/cli/mod.rs` - Index command (to be simplified)
- **Magellan documentation:**
  - `/home/feanor/Projects/magellan/docs/MIRAGE_CFG_SCHEMA.md` - Schema specification
  - `/home/feanor/Projects/magellan/docs/CFG_LIMITATIONS.md` - AST-based CFG limitations

### Secondary (MEDIUM confidence)

- **Integration design:**
  - `/home/feanor/Projects/mirage/MAGELLAN_V2_INTEGRATION.md` - Integration opportunities
  - `/home/feanor/Projects/magellan/docs/SCHEMA_REFERENCE.md` - Schema conventions

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** - Based on actual source code inspection
- Architecture: **HIGH** - Schema differences verified directly from databases
- Pitfalls: **HIGH** - Edge case patterns identified from current codebase
- Implementation approach: **MEDIUM** - Some open questions about edge handling

**Research date:** 2026-02-04
**Valid until:** 30 days (Magellan schema is stable at v7, but verify before final implementation)

---

*Phase 12 Research Complete*
