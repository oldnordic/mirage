# Mirage: Control-Flow & Logic Graph Engine

## Concept

A **Path-Aware Code Intelligence Engine** that operates on graphs, not text. It materializes behavior explicitly: paths, proofs, counterexamples.

**NOT:**
- A search tool (llmgrep already does this)
- An embedding tool
- Static analysis / linting
- Semantic search

**IS:**
- Path enumeration and verification
- Graph-based reasoning about code behavior
- Truth engine that materializes facts for LLM consumption

## Why This Matters

Current LLM code analysis fails because:
1. **Full file ingestion** → token explosions, "I skimmed it"
2. **Speculative reasoning** → "it probably does X"
3. **Text-based inference** → misses structural truths

Mirage fixes all three by:
1. **No full reads** — agents query graph artifacts only
2. **No speculation** — output must reference graph artifacts
3. **Structural truth** — paths come from explicit graph traversal

---

# Ecosystem Context

Mirage is the third tool in the Magellan ecosystem:

| Tool | Scope | Storage | Purpose |
|------|-------|---------|---------|
| **Magellan** | Inter-procedural (symbols) | `graph_entities`, `graph_edges` | Code graph navigation |
| **llmgrep** | Query over Magellan | Read-only CLI | Semantic code search |
| **Mirage** | Intra-procedural (CFG) + Paths | `cfg_blocks`, `cfg_edges`, `cfg_paths` | Path enumeration |

## Foundation: SQLiteGraph

All three tools share SQLiteGraph as the persistence layer:

- **Dual Backend**: SQLite or Native V2 (clustered adjacency)
- **ACID Transactions**: Full rollback support
- **MVCC Snapshots**: Read isolation
- **Pub/Sub Events**: In-process change notification (Native V2)
- **HNSW Vector Search**: For embedding-based queries

Mirage will use SQLiteGraph to store:
- AST nodes (from tree-sitter or rustc)
- CFG blocks and edges
- Enumerated execution paths
- Dominator relationships

---

# Design Philosophy

## Foundation First → CFG from Magellan

**Mirage is a read-only analyzer** that works with CFG data extracted by Magellan:

```
Magellan: source → AST → CFG blocks/edges
Mirage: CFG → paths → dominance → loops
```

- Magellan parses source code with tree-sitter
- Magellan builds AST-based CFG for all languages
- Mirage loads CFG from database
- Mirage computes paths, dominance, and loops

This separation of concerns keeps Mirage focused on analysis.

---

# Architecture

## Phase 1: Read CFG from Magellan (Foundation)

### Data Source

Magellan provides AST-based CFG for all supported languages. Mirage reads this data and computes:

| Feature | Source |
|---------|--------|
| cfg_blocks | Magellan (populated by `magellan watch`) |
| cfg_edges | Magellan (populated by `magellan watch`) |
| cfg_paths | Mirage (computed on demand) |
| cfg_dominators | Mirage (computed on demand) |

### Output Schema

```sql
-- AST Nodes: tree-sitter or rustc AST
ast_nodes(
    id INTEGER PRIMARY KEY,
    parent_id INTEGER,
    kind TEXT,              -- Function, Block, If, Match, Loop, etc.
    byte_start INTEGER,
    byte_end INTEGER,
    text TEXT
)

-- CFG Blocks: basic blocks within functions
cfg_blocks(
    id INTEGER PRIMARY KEY,
    function_id INTEGER,    -- links to graph_entities
    block_kind TEXT,        -- Block|Branch|LoopEntry|LoopExit|Return|Call
    byte_start INTEGER,
    byte_end INTEGER,
    terminator TEXT         -- "if", "match", "return", "call", etc.
)

-- CFG Edges: control flow between blocks
cfg_edges(
    from_id INTEGER,
    to_id INTEGER,
    edge_type TEXT,         -- TrueBranch|FalseBranch|Fallthrough|LoopBack|Exception
    PRIMARY KEY (from_id, to_id, edge_type)
)

-- Path Cache: enumerated execution paths
cfg_paths(
    path_id TEXT PRIMARY KEY,      -- BLAKE3 hash of path signature
    function_id INTEGER,
    path_kind TEXT,                -- normal|error|degenerate|unreachable
    entry_block INTEGER,
    exit_block INTEGER,
    length INTEGER                 -- number of blocks in path
)

-- Path Elements: blocks in each path
cfg_path_elements(
    path_id TEXT,
    sequence_order INTEGER,
    block_id INTEGER,
    PRIMARY KEY (path_id, sequence_order)
)

-- Dominance: computed dominator relationships
cfg_dominators(
    block_id INTEGER,
    dominator_id INTEGER,          -- block that dominates this one
    is_strict BOOLEAN             -- true = strict dominator
)

-- Post-dominators: reverse dominance (for "must exit through")
cfg_post_dominators(
    block_id INTEGER,
    post_dominator_id INTEGER,
    is_strict BOOLEAN
)
```

## Phase 2: Path Enumeration (Mirage)

1. Enumerate all paths through CFG
2. Tag paths by semantics (normal/error/degenerate)
3. Cache paths in `cfg_paths` table
4. Incremental updates on code change

## Phase 3: Analysis Queries (Mirage)

```bash
# Show all execution paths through a function
mirage paths --function "parse_request" --show-errors

# Prove that validation happens before use
mirage dominators --function "use_resource" --must-pass-through "validate"

# Find dead code within functions
mirage unreachable --within-functions --show-branches

# Detect bypassed error handling
mirage wrong-branch --pattern "return" --before "cleanup"
```

---

# The Analyzer Pipeline

## Phase 1: Graph Construction (Magellan)

```
source → AST → CFG blocks/edges
```

**Tools**: tree-sitter, SQLiteGraph (persistence)

**Command**: `magellan watch --root ./src --db ./mirage.db`

## Phase 2: Graph Traversal (Machine)

Enumerate paths, prune impossible, tag semantics.

**Output**: Path IDs, not text.

## Phase 3: Structural Verification (Machine)

- llmgrep confirms symbols exist
- llmgrep confirms calls are real
- Magellan confirms ownership/structure

If this fails → stop. LLM is not allowed to speak.

## Phase 4: LLM Reasoning (Last, Constrained)

LLM may explain, suggest, describe — but ONLY with:
- Path ID
- CFG proof
- Reachability proof

**LLM cannot invent paths.**

---

# The Golden Rule

> **An agent may only speak if it can reference a graph artifact.**
>
> **No artifact → no output.**

This single rule kills hallucination.

---

# CLI Design

```bash
# Path commands
mirage paths --function SYMBOL [--show-errors] [--max-length N]
mirage cfg --function SYMBOL [--format dot|json]
mirage dominators --function SYMBOL --must-pass-through BLOCK
mirage unreachable --within-functions
mirage wrong-branch [--pattern PATTERN]

# Status and verification
mirage status                       # Check database and CFG availability
mirage verify --path-id ID          # Verify path still valid
mirage blast-zone --function SYMBOL # Impact analysis using paths
```

---

# Naming: Mirage

**Why "Mirage"?**

- Suggests "seeing what's not visible" (hidden paths)
- Short, memorable, CLI-friendly
- No conflicts with existing tools
- Fits the explorer theme: **Magellan → llmgrep → Mirage**

---

# References

- [Magellan](https://github.com/oldnordic/magellan) - Code graph navigation and symbol analysis
- [llmgrep](https://github.com/oldnordic/llmgrep) - Semantic code search powered by embeddings
- [SQLiteGraph](https://github.com/oldnordic/sqlitegraph) - Embedded graph database with dual backend

---

*Created: 2026-01-31*
