# Phase 10: Magellan v2 Integration & Bugfixes - Research

**Researched:** 2026-02-03
**Domain:** Inter-procedural graph algorithms, library integration, Rust dependency management
**Confidence:** HIGH

## Summary

This phase focuses on integrating Magellan v2.0.0's graph algorithms into Mirage to enhance inter-procedural analysis capabilities. The primary integration involves adding Magellan as a library dependency and combining its call-graph reachability analysis with Mirage's intra-procedural CFG analysis.

**Key findings:**
1. **Magellan v2.0.0** is installed locally at `/home/feanor/Projects/magellan` and provides a rich library API via `magellan::CodeGraph`
2. **No compilation errors exist** - the project compiles cleanly with only warnings (unused imports)
3. **Integration opportunity**: Magellan provides 8 key algorithms (reachable, dead-code, cycles, condense, paths, slice) via `sqlitegraph` backend
4. **Dependency path**: Add via `path = "../magellan"` or `git = "https://github.com/oldnordic/magellan"` since it's not on crates.io
5. **sqlitegraph v1.3.0** is the underlying graph database library used by both Magellan and can be shared

**Primary recommendation:** Add Magellan as a local path dependency, integrate call-graph algorithms into Mirage CLI commands (especially `blast-zone`, `unreachable`, and new `slice` command).

---

## Standard Stack

### Core Dependencies

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| **magellan** | 2.0.0 (local path) | Code indexing + graph algorithms | Provides SCC detection, reachability, path enumeration on call graphs |
| **sqlitegraph** | 1.3.0 (re-export) | Graph persistence backend | Shared database format between Magellan and Mirage |
| **petgraph** | 0.8.3 (existing) | In-memory CFG algorithms | Already used by Mirage for dominance, loops, path enumeration |
| **rusqlite** | 0.32.1 (existing) | SQLite database access | Already used for MirageDb operations |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| anyhow | 1.0 | Error handling | Already used throughout codebase |
| serde/serde_json | 1.0 | Serialization | Already used for JSON output |
| blake3 | 1.5 | Hashing for path IDs | Already used for path cache keys |

### Dependency Addition

**For local development (path dependency):**
```toml
[dependencies]
magellan = { path = "../magellan" }
```

**For release (git dependency):**
```toml
[dependencies]
magellan = { git = "https://github.com/oldnordic/magellan", version = "2.0" }
```

**Note:** Magellan is NOT published on crates.io, so git or path dependency is required.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Magellan library | Call `magellan` binary via CLI | Library API is faster (no subprocess overhead), provides direct database access |
| Local path | Git dependency | Path dependency for active development, git for releases |

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── analysis/          # NEW: Inter-procedural analysis module
│   ├── mod.rs         # Public API for call-graph queries
│   ├── reachability.rs # Enhanced reachability (Magellan + Mirage)
│   ├── cycles.rs       # Combined cycle detection
│   └── slicing.rs      # Program slicing integration
├── cfg/               # EXISTING: Intra-procedural CFG
│   ├── mod.rs
│   ├── paths.rs       # Path enumeration
│   ├── dominators.rs  # Dominance analysis
│   └── loops.rs       # Natural loop detection
├── cli/               # EXISTING: CLI commands
│   └── mod.rs         # Add new commands here
└── storage/           # EXISTING: Database layer
    └── mod.rs         # Already extends Magellan schema
```

### Pattern 1: Wrapper API for Magellan Algorithms

**What:** Create a facade module that wraps Magellan's `CodeGraph` API for Mirage-specific use cases.

**When to use:** When you need to call Magellan algorithms from multiple CLI commands.

**Example:**
```rust
// src/analysis/mod.rs
use magellan::CodeGraph;
use anyhow::Result;

/// Wrapper for Magellan graph access
pub struct MagellanBridge {
    graph: CodeGraph,
}

impl MagellanBridge {
    /// Open Magellan database at the same path as Mirage DB
    pub fn open(db_path: &str) -> Result<Self> {
        let graph = CodeGraph::open(db_path)?;
        Ok(Self { graph })
    }

    /// Get inter-procedural reachable symbols
    pub fn reachable_symbols(&self, symbol_id: &str) -> Result<Vec<SymbolInfo>> {
        self.graph.reachable_symbols(symbol_id, None)
    }

    /// Find uncalled functions (dead code at call graph level)
    pub fn dead_functions(&self, entry_symbol: &str) -> Result<Vec<DeadSymbol>> {
        self.graph.dead_symbols(entry_symbol)
    }

    /// Detect mutual recursion cycles
    pub fn detect_cycles(&self) -> Result<CycleReport> {
        self.graph.detect_cycles()
    }
}
```

**Source:** Based on Magellan library API at `/home/feanor/Projects/magellan/src/graph/algorithms.rs`

### Pattern 2: Combined Analysis Results

**What:** Merge inter-procedural (Magellan) and intra-procedural (Mirage) results into unified output.

**When to use:** For `unreachable`, `blast-zone`, and `cycles` commands.

**Example:**
```rust
// Enhanced unreachable result
pub struct EnhancedDeadCode {
    // From Magellan: Uncalled functions
    pub uncalled_functions: Vec<DeadSymbol>,
    // From Mirage: Unreachable blocks within called functions
    pub unreachable_blocks: HashMap<String, Vec<BlockId>>,
}

// Combined cycles result
pub struct EnhancedCycles {
    // From Magellan: Call graph SCCs (mutual recursion)
    pub call_graph_cycles: Vec<Cycle>,
    // From Mirage: Natural loops within functions
    pub function_loops: HashMap<String, Vec<NaturalLoop>>,
}
```

### Pattern 3: Shared Database Access

**What:** Both Magellan and Mirage use the same SQLite database file.

**When to use:** Always - Mirage extends Magellan's schema with CFG tables.

**Example:**
```rust
// Open once, use for both
let db_path = "codegraph.db";
let mirage_db = MirageDb::open(db_path)?;  // Reads Magellan tables + Mirage tables
let magellan_graph = CodeGraph::open(db_path)?; // Reads Magellan tables
```

**Key insight:** The same database file contains:
- Magellan tables: `graph_entities`, `graph_edges` (symbols, calls)
- Mirage tables: `cfg_blocks`, `cfg_edges`, `cfg_paths`, `cfg_dominators`

### Anti-Patterns to Avoid

- **Binary subprocess calls:** Don't shell out to `magellan` CLI - use the library API directly
- **Duplicate database opens:** Open the database once and share connections
- **Ignoring SCC structure:** Magellan's `condense_call_graph()` returns DAG structure useful for topological analysis
- **Assuming symbols exist:** Always handle `Err` when resolving symbol IDs - may not be in call graph

---

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SCC detection (cycles) | Custom Tarjan implementation | `CodeGraph::detect_cycles()` | Uses sqlitegraph's optimized algorithm, handles entity ID translation |
| Call graph reachability | BFS/DFS on CALLS edges | `CodeGraph::reachable_symbols()` | Handles reverse edges (CALLER), boundary conditions |
| Dead code detection | Custom graph traversal | `CodeGraph::dead_symbols()` | Computes all entities minus reachable set correctly |
| Path enumeration on call graph | Recursive DFS | `CodeGraph::enumerate_paths()` | Has bounded enumeration (max_depth, max_paths, revisit_cap) |
| Program slicing | Manual reachability + data flow | `CodeGraph::backward_slice()` / `forward_slice()` | Call-graph fallback already implemented, will upgrade to CFG-based later |

**Key insight:** Magellan v2.0.0 has spent significant effort on these algorithms. Reuse them.

---

## Common Pitfalls

### Pitfall 1: Symbol ID vs Entity ID Confusion

**What goes wrong:** Magellan uses entity IDs (i64 row IDs) internally, but exposes stable symbol IDs (BLAKE3 hashes) publicly. Mixing these up causes "symbol not found" errors.

**Why it happens:** `CodeGraph` API methods accept symbol_id (String) but internally convert to entity_id for graph algorithms.

**How to avoid:**
- Always use public API methods that accept `&str` symbol IDs
- Let Magellan handle the entity ID resolution internally
- Only work with entity IDs if you're calling sqlitegraph directly

**Warning signs:** Getting rusqlite errors about missing graph_entities rows.

### Pitfall 2: Opening Database Multiple Times

**What goes wrong:** Opening the same database file with both `MirageDb` and `CodeGraph` creates separate connections. WAL mode allows concurrent reads, but write operations may conflict.

**Why it happens:** `MirageDb::open()` and `CodeGraph::open()` both create new SQLite connections.

**How to avoid:**
- For read-only operations: Safe to have both open
- For writes: Complete Mirage write before opening CodeGraph, or vice versa
- Consider using shared connections via rusqlite if needed

### Pitfall 3: Missing Call Graph Edges

**What goes wrong:** Calling `reachable_symbols()` on a symbol that exists in graph_entities but has no CALLS edges returns empty results (not error).

**Why it happens:** Magellan indexes symbols and calls separately. A symbol may exist but not participate in the call graph (e.g., a static function never called).

**How to avoid:**
- Check if symbol is in call graph before assuming reachability results
- Use `dead_symbols()` to find all symbols that ARE in call graph but unreachable
- Document that "no results" ≠ "error"

### Pitfall 4: Path Enumeration Bounds

**What goes wrong:** `enumerate_paths()` can return millions of paths in cyclic graphs, causing memory exhaustion.

**Why it happens:** Call graphs with recursion have infinite paths without bounds.

**How to avoid:**
- Always use `max_depth` and `max_paths` parameters
- Check `bounded_hit` flag in results to detect if enumeration was cut off
- Default to reasonable bounds (e.g., max_depth=50, max_paths=1000)

**Warning signs:** Query takes >10 seconds, memory usage spikes.

---

## Code Examples

### Example 1: Add Magellan Dependency

**Source:** Cargo.toml pattern

```toml
[dependencies]
# Local development (use this during Phase 10)
magellan = { path = "../magellan" }

# For release (switch to this after integration complete)
# magellan = { git = "https://github.com/oldnordic/magellan", version = "2.0" }

# Re-export sqlitegraph to use matching version
sqlitegraph = "1.3"
```

### Example 2: Query Dead Code (Combined Analysis)

**Source:** MAGELLAN_V2_INTEGRATION.md integration opportunity #1

```rust
use magellan::CodeGraph;
use crate::storage::MirageDb;

pub fn enhanced_dead_code(db_path: &str, entry_symbol: &str) -> anyhow::Result<EnhancedDeadCode> {
    // Open shared database
    let mut mirage_db = MirageDb::open(db_path)?;
    let magellan = CodeGraph::open(db_path)?;

    // Inter-procedural: uncalled functions
    let uncalled = magellan.dead_symbols(entry_symbol)?;

    // Intra-procedural: unreachable blocks within called functions
    let mut unreachable_blocks = HashMap::new();
    for dead_symbol in &uncalled {
        if let Some(function_id) = resolve_function_id_by_name(&mirage_db, &dead_symbol.symbol.fqn.as_ref().unwrap())? {
            let cfg = load_cfg_from_db(&mirage_db, function_id)?;
            let unreachable = find_unreachable_blocks(&cfg);
            unreachable_blocks.insert(dead_symbol.symbol.fqn.clone().unwrap(), unreachable);
        }
    }

    Ok(EnhancedDeadCode {
        uncalled_functions: uncalled,
        unreachable_blocks,
    })
}
```

### Example 3: Enhanced Blast Zone with Call Graph

**Source:** MAGELLAN_V2_INTEGRATION.md integration opportunity #2

```rust
// In CLI command handler
pub fn enhanced_blast_zone(db_path: &str, function_name: &str) -> anyhow::Result<BlastZoneResult> {
    let magellan = CodeGraph::open(db_path)?;

    // Get function's symbol_id from database
    let symbol_id = resolve_symbol_id(db_path, function_name)?;

    // Forward: what functions does this affect?
    let forward_reachable = magellan.reachable_symbols(&symbol_id, None)?;

    // Backward: what functions affect this?
    let backward_reachable = magellan.reverse_reachable_symbols(&symbol_id, None)?;

    // Combine with Mirage's intra-procedural paths
    let cfg = load_cfg_by_name(db_path, function_name)?;
    let paths = enumerate_paths_cached(&cfg)?;

    Ok(BlastZoneResult {
        forward_impact: forward_reachable,
        backward_impact: backward_reachable,
        intra_procedural_paths: paths.len(),
        cfg_complexity: cfg.node_count(),
    })
}
```

### Example 4: Combined Cycle Detection

**Source:** Magellan algorithms.rs + Mirage loops.rs

```rust
pub fn combined_cycles(db_path: &str) -> anyhow::Result<EnhancedCycles> {
    let magellan = CodeGraph::open(db_path)?;
    let mirage_db = MirageDb::open(db_path)?;

    // Inter-procedural: SCCs in call graph
    let call_cycles = magellan.detect_cycles()?;

    // Intra-procedural: Natural loops within functions
    let mut function_loops = HashMap::new();
    for cycle in &call_cycles.cycles {
        for member in &cycle.members {
            if let Ok(cfg) = load_cfg_by_fqn(&mirage_db, member.fqn.as_ref().unwrap()) {
                let loops = detect_natural_loops(&cfg);
                if !loops.is_empty() {
                    function_loops.insert(member.fqn.clone().unwrap(), loops);
                }
            }
        }
    }

    Ok(EnhancedCycles {
        call_graph_cycles: call_cycles.cycles,
        function_loops,
    })
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Separate Magellan (CLI) + Mirage (library) | Unified library integration | Phase 10 | No subprocess overhead, direct database access |
| Intra-procedural only | Inter + intra procedural | Phase 10 | Complete dead code detection (uncalled + unreachable blocks) |
| Path-based blast zone | Path + call graph reachability | Phase 10 | More accurate impact analysis |
| No program slicing | Call-graph based slicing | Phase 10 | Initial slicing capability (upgradable to CFG-based) |

**Current Magellan v2.0.0 capabilities:**
- O(V + E) reachability analysis
- O(V + E) dead code detection
- O(V + E) cycle detection (SCC decomposition)
- Bounded path enumeration on call graphs
- Forward/backward program slicing (call-graph fallback)
- Call graph condensation (SCC collapse to DAG)

**Deprecated/outdated:**
- Calling `magellan` binary via CLI - use library API instead
- Manual SCC implementation - use `CodeGraph::detect_cycles()`
- Custom BFS for reachability - use `CodeGraph::reachable_symbols()`

---

## Open Questions

### 1. **Q: Should Mirage depend on Magellan or vice versa?**
**What we know:** Magellan is the indexing tool, Mirage extends it with CFG data.
**What's unclear:** Which project should "own" the dependency relationship.
**Recommendation:** Mirage depends on Magellan (Mirage extends Magellan's database schema).

### 2. **Q: How to handle database migrations?**
**What we know:** Magellan has schema version 5, Mirage extends to schema version 1.
**What's unclear:** Migration strategy when Magellan updates its schema.
**Recommendation:** Document Mirage's minimum Magellan schema version in `MIN_MAGELLAN_SCHEMA_VERSION` constant (already defined).

### 3. **Q: Should we add new CLI commands or extend existing ones?**
**What we know:** MAGELLAN_V2_INTEGRATION.md suggests 8 integration opportunities.
**What's unclear:** Which warrant new commands vs flags.
**Recommendation:**
- New command: `mirage slice` (program slicing)
- Extend existing: `mirage unreachable --include-uncalled`, `mirage cycles --call-graph`

---

## Sources

### Primary (HIGH confidence)

- `/home/feanor/Projects/magellan/src/lib.rs` - Magellan library public API
- `/home/feanor/Projects/magellan/src/graph/mod.rs` - CodeGraph implementation
- `/home/feanor/Projects/magellan/src/graph/algorithms.rs` - All 8 graph algorithms with documentation
- `/home/feanor/Projects/mirage/MAGELLAN_V2_INTEGRATION.md` - 8 documented integration opportunities
- `/home/feanor/Projects/mirage/src/storage/mod.rs` - Mirage database layer (already extends Magellan schema)
- `/home/feanor/Projects/mirage/Cargo.toml` - Current dependencies

### Secondary (MEDIUM confidence)

- Context7: `/websites/rs_petgraph` - petgraph algorithms (SCC, A*)
- [Magellan on lib.rs](https://lib.rs/crates/magellan) - Project description (updated Dec 2025)
- [Graph Algorithms: From Theory to Optimization (Examples in Rust)](https://medium.com/@jordangrilly/graph-algorithms-from-theory-to-optimization-examples-in-rust-aa4ad2734255) - General context on Rust graph algorithms

### Tertiary (LOW confidence)

- WebSearch results for "Magellan 2.0" - Limited public documentation beyond source code
- [A graph data science library for Rust](https://users.rust-lang.org/t/a-graph-data-science-library-for-rust/137011) - Discussion about graph libraries (not Magellan-specific)

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Magellan source code directly examined, versions confirmed
- Architecture: HIGH - Integration patterns based on actual Magellan API
- Pitfalls: HIGH - Discovered through examination of algorithms.rs implementation

**Research date:** 2026-02-03
**Valid until:** 30 days (Magellan 2.0.0 is stable, but API may evolve)

**Magellan database context:**
- Execution ID: N/A (research only, no execution)
- Snapshot ID: N/A
- Files analyzed: Magellan library source (algorithms.rs, mod.rs, lib.rs)
