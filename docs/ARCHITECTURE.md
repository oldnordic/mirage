# Architecture

**Version**: 0.1.0 (Design Phase)
**Created**: 2026-01-31

---

## Overview

Mirage is a control-flow and logic graph engine for Rust codebases. It reads AST-based CFG data extracted by Magellan, enumerates execution paths, and provides graph-based reasoning capabilities for LLM agents.

## Design Goals

1. **No Speculation**: Every output must reference a graph artifact
2. **Path-Aware**: Explicit representation of all execution paths
3. **Read-Only**: All indexing and CFG extraction handled by Magellan
4. **Integrated**: Part of the Magellan/llmgrep ecosystem

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Mirage CLI                              │
│  ┌──────────────┐  ┌──────────────────┐  ┌──────────────────┐  │
│  │   Analyzer   │  │   Query Layer    │  │   Output Layer   │  │
│  └──────┬───────┘  └────────┬─────────┘  └────────┬─────────┘  │
└─────────┼────────────────────┼─────────────────────┼────────────┘
          │                    │                     │
          ▼                    ▼                     ▼
┌─────────────────────────────────────────────────────────────┐
│                      Mirage Core                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │ CFG Loader   │  │Path Enumerator│  │  Dominance Calc  │   │
│  └──────┬───────┘  └──────┬───────┘  └────────┬─────────┘   │
└─────────┼──────────────────┼───────────────────┼─────────────┘
          │                  │                   │
          ▼                  ▼                   ▼
┌─────────────────────────────────────────────────────────────┐
│                    SQLiteGraph Layer                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │ CFG Storage  │  │  Path Cache  │  │  Result Cache    │   │
│  └──────────────┘  └──────────────┘  └──────────────────┘   │
└─────────────────────────────────────────────────────────────┘
          │                  │                   │
          ▼                  ▼                   ▼
┌─────────────────────────────────────────────────────────────┐
│                   Magellan Database                          │
│  (graph_entities, graph_edges, cfg_blocks, cfg_edges, etc.)  │
└─────────────────────────────────────────────────────────────┘
```

---

## Module Structure

```
mirage/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Library interface
│   ├── cfg/
│   │   ├── mod.rs
│   │   ├── loader.rs        # Load CFG from database
│   │   ├── block.rs         # Basic block representation
│   │   └── edge.rs          # Edge types and representation
│   ├── paths/
│   │   ├── mod.rs
│   │   ├── enumerator.rs    # Path enumeration algorithm
│   │   ├── classifier.rs    # Path semantics (normal/error)
│   │   └── cache.rs         # Path caching and invalidation
│   ├── analysis/
│   │   ├── mod.rs
│   │   ├── dominators.rs    # Dominance analysis
│   │   ├── reachability.rs  # Reachability queries
│   │   └── dead_code.rs     # Dead code detection
│   ├── query/
│   │   ├── mod.rs
│   │   ├── engine.rs        # Query execution
│   │   └── planner.rs       # Query planning
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── schema.rs        # Database schema
│   │   └── migrations.rs    # Schema migrations
│   └── cli/
│       ├── mod.rs
│       ├── cmds.rs          # CLI command definitions
│       └── output.rs        # Output formatting
├── docs/
│   ├── CONCEPT.md
│   ├── ARCHITECTURE.md
│   └── ECOSYSTEM_CONTEXT.md
└── tests/
    ├── integration/
    └── fixtures/
```

---

## Data Flow

### Data Flow

Magellan extracts CFG and Mirage analyzes it:

```
Magellan Pipeline (CFG Extraction):
1. Project Scan
   └─> Find all source files

2. AST Parsing
   └─> Parse source code with tree-sitter
   └─> Extract function definitions

3. CFG Building
   └─> Build cfg_blocks for each function
   └─> Build cfg_edges from control flow constructs
   └─> Store in database

Mirage Pipeline (Analysis):
1. Load CFG
   └─> Read cfg_blocks from database
   └─> Read cfg_edges from database
   └─> Build in-memory graph

2. Path Enumeration
   └─> Enumerate all execution paths
   └─> Classify paths (normal/error/degenerate)
   └─> Cache in cfg_paths table

3. Dominance Analysis
   └─> Compute dominators
   └─> Compute post-dominators
   └─> Store in cfg_dominators table
```

### Query Pipeline

```
1. Parse Query
   └─> CLI arguments → QueryPlan

2. Load CFG from Database
   └─> Read cfg_blocks and cfg_edges
   └─> Build in-memory CFG graph
   └─> Verify data exists (error if missing)

3. Execute Analysis
   └─> Path enumeration, dominance, loops, etc.
   └─> Cross-reference with Magellan symbols
   └─> Compute results

4. Format Output
   └─> JSON or human-readable
   └─> Include span references
```

---

## Database Schema

### Core Tables

```sql
-- AST Nodes (from MIR/tree-sitter)
CREATE TABLE ast_nodes (
    id INTEGER PRIMARY KEY,
    parent_id INTEGER REFERENCES ast_nodes(id),
    function_id INTEGER,  -- Links to graph_entities.id
    kind TEXT NOT NULL,   -- Function, Block, If, Match, Loop, Return, etc.
    byte_start INTEGER NOT NULL,
    byte_end INTEGER NOT NULL,
    text TEXT
);

CREATE INDEX idx_ast_nodes_parent ON ast_nodes(parent_id);
CREATE INDEX idx_ast_nodes_function ON ast_nodes(function_id);

-- CFG Blocks (basic blocks within functions)
CREATE TABLE cfg_blocks (
    id INTEGER PRIMARY KEY,
    function_id INTEGER NOT NULL,  -- Links to graph_entities.id
    block_kind TEXT NOT NULL,      -- Block, Branch, LoopEntry, LoopExit, Return, Call
    byte_start INTEGER,
    byte_end INTEGER,
    terminator TEXT,               -- if, match, return, call, switch, etc.
    FOREIGN KEY (function_id) REFERENCES graph_entities(id)
);

CREATE INDEX idx_cfg_blocks_function ON cfg_blocks(function_id);

-- CFG Edges (control flow between blocks)
CREATE TABLE cfg_edges (
    from_id INTEGER NOT NULL REFERENCES cfg_blocks(id),
    to_id INTEGER NOT NULL REFERENCES cfg_blocks(id),
    edge_type TEXT NOT NULL,       -- TrueBranch, FalseBranch, Fallthrough, LoopBack, Exception
    PRIMARY KEY (from_id, to_id, edge_type)
);

CREATE INDEX idx_cfg_edges_from ON cfg_edges(from_id);
CREATE INDEX idx_cfg_edges_to ON cfg_edges(to_id);

-- Path Cache (enumerated execution paths)
CREATE TABLE cfg_paths (
    path_id TEXT PRIMARY KEY,      -- BLAKE3 hash
    function_id INTEGER NOT NULL,
    path_kind TEXT NOT NULL,       -- normal, error, degenerate, unreachable
    entry_block INTEGER NOT NULL REFERENCES cfg_blocks(id),
    exit_block INTEGER NOT NULL REFERENCES cfg_blocks(id),
    length INTEGER NOT NULL,       -- Number of blocks
    created_at INTEGER NOT NULL,   -- Unix timestamp
    FOREIGN KEY (function_id) REFERENCES graph_entities(id)
);

CREATE INDEX idx_cfg_paths_function ON cfg_paths(function_id);
CREATE INDEX idx_cfg_paths_kind ON cfg_paths(path_kind);

-- Path Elements (blocks in each path)
CREATE TABLE cfg_path_elements (
    path_id TEXT NOT NULL REFERENCES cfg_paths(path_id),
    sequence_order INTEGER NOT NULL,
    block_id INTEGER NOT NULL REFERENCES cfg_blocks(id),
    PRIMARY KEY (path_id, sequence_order)
);

CREATE INDEX cfg_path_elements_block ON cfg_path_elements(block_id);

-- Dominators (dominance relationships)
CREATE TABLE cfg_dominators (
    block_id INTEGER NOT NULL REFERENCES cfg_blocks(id),
    dominator_id INTEGER NOT NULL REFERENCES cfg_blocks(id),
    is_strict BOOLEAN NOT NULL,
    PRIMARY KEY (block_id, dominator_id, is_strict)
);

-- Post-dominators (reverse dominance)
CREATE TABLE cfg_post_dominators (
    block_id INTEGER NOT NULL REFERENCES cfg_blocks(id),
    post_dominator_id INTEGER NOT NULL REFERENCES cfg_blocks(id),
    is_strict BOOLEAN NOT NULL,
    PRIMARY KEY (block_id, post_dominator_id, is_strict)
);
```

### Metadata Tables

```sql
-- Indexing metadata
CREATE TABLE mirage_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Sample values:
-- key: schema_version, value: 1
-- key: last_indexed, value: 1706745600
-- key: rustc_version, value: 1.77.0
```

---

## CFG Data Source

Magellan provides AST-based CFG for all supported languages:

**CFG Extraction (handled by Magellan):**
- tree-sitter AST parsing
- Control flow construct detection (if, match, loop, etc.)
- Basic block and edge construction
- Storage in cfg_blocks and cfg_edges tables

**Advantages of AST-based CFG:**
- Works on stable Rust (no nightly required)
- Consistent across all languages
- Sufficient for most LLM use cases
- No external binary dependencies

**For more information, see:** LIMITATIONS_AND_ROADMAP.md

---

## Path Enumeration Algorithm

### Naive Enumeration (Small Functions)

```rust
fn enumerate_paths(cfg: &CFG) -> Vec<Path> {
    let mut paths = Vec::new();
    let mut current = vec![cfg.entry_block()];

    fn dfs(block: BlockId, current: &mut Vec<BlockId>, paths: &mut Vec<Path>, cfg: &CFG) {
        current.push(block);

        if cfg.is_exit(block) {
            paths.push(current.clone());
        } else {
            for successor in cfg.successors(block) {
                dfs(successor, current, paths, cfg);
            }
        }

        current.pop();
    }

    dfs(cfg.entry_block(), &mut current, &mut paths, cfg);
    paths
}
```

### Pruned Enumeration (Large Functions)

```rust
fn enumerate_paths_pruned(cfg: &CFG, max_length: usize) -> Vec<Path> {
    // Use worklist algorithm with cycle detection
    // Prune paths exceeding max_length
    // Track visited states for loop handling
}
```

---

## Dominance Analysis

### Algorithm: Cooper, Harvey, Kennedy (2001)

```rust
fn compute_dominators(cfg: &CFG) -> HashMap<BlockId, Set<BlockId>> {
    // Iterative dataflow analysis
    // Dom(n) = {n} ∪ ∩ Dom(p) for all predecessors p of n
}
```

### Applications

- **Must-pass-through**: If X dominates all paths from entry to Y
- **Dead code**: If no path from entry reaches block
- **Loop detection**: Natural loops via back edges

---

## CLI Commands

### Status Check

```bash
mirage status                            # Check database and CFG availability
mirage status --db ./mydb.db             # Check specific database
```

### Path Queries

```bash
mirage paths --function SYMBOL           # All paths for function
mirage paths --function SYMBOL --show-errors   # Only error paths
mirage paths --function SYMBOL --max-length 10  # Prune long paths
```

### CFG Visualization

```bash
mirage cfg --function SYMBOL             # Human-readable CFG
mirage cfg --function SYMBOL --format dot > cfg.dot  # Graphviz
mirage cfg --function SYMBOL --format json          # JSON export
```

### Analysis

```bash
mirage dominators --function SYMBOL      # Show dominance tree
mirage unreachable --project PATH        # Find unreachable code
mirage wrong-branch --pattern "return"   # Detect bypassed cleanup
```

### Integration

```bash
mirage verify --path-id PATH_ID          # Verify path still valid
mirage blast-zone --function SYMBOL      # Impact analysis
```

---

## Error Handling

### Error Types

```rust
pub enum MirageError {
    // Database errors
    DatabaseError(String),
    SchemaVersionMismatch(u32, u32),

    // Query errors
    SymbolNotFound(String),
    PathNotFound(String),
    InvalidQuery(String),

    // Analysis errors
    CycleDetected(Vec<BlockId>),
    PathOverflow(usize),
}
```

### Recovery Strategy

- **Load errors**: Return helpful "run magellan watch" message
- **Query**: Return error with suggestion
- **Analysis**: Return partial results with warnings

---

## Testing Strategy

### Unit Tests

- CFG loading from database
- Path enumeration on synthetic graphs
- Dominance algorithm verification

### Integration Tests

- Load CFG from Magellan database
- Query validation
- Path enumeration on real CFGs

### Benchmarks

- Indexing time vs. crate size
- Path enumeration explosion
- Query latency for common patterns

---

## Performance Considerations

### Path Explosion

For functions with loops, path count can be exponential.

**Mitigation**:
- Cap path enumeration at configurable limit
- Use symbolic execution for loops
- Cache and incrementally update

### Incremental Updates

On code change:
1. Magellan detects and updates affected CFGs
2. Mirage invalidates cached paths
3. Paths re-enumerated on next query

### Memory Usage

- Use SQLite for path storage (not in-memory)
- Lazy load CFG blocks on query

---

*Last updated: 2026-01-31*
