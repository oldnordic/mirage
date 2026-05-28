# Mirage User Manual

Version 1.6.0

---

## Overview

Mirage is a path-aware code intelligence tool. It reads Magellan code graphs and analyzes control-flow graphs to enumerate execution paths, detect dead code, and compute dominance relationships.

**Core Principle:** An agent may only speak if it can reference a graph artifact.

## Part of the Code Intelligence Toolset

Mirage is one of five complementary tools designed to work together:

| Tool | Purpose | Install |
|------|---------|---------|
| [Magellan](https://github.com/oldnordic/magellan) | Call graph indexing and symbol navigation | `cargo install magellan` |
| [llmgrep](https://github.com/oldnordic/llmgrep) | Semantic code search over indexed symbols | `cargo install llmgrep` |
| [Mirage](https://github.com/oldnordic/mirage) | Control-flow analysis and path enumeration | `cargo install mirage-analyzer` |
| [sqlitegraph](https://crates.io/crates/sqlitegraph) | Shared graph database library (dependency) | Included automatically |
| [splice](https://github.com/oldnordic/splice) | Source code transformation with span precision | `cargo install splice` |

**Important:** Mirage provides its full capabilities when used together with Magellan. Inter-procedural analysis features (call graph dominance, hotspots, cross-function slicing) require Magellan's call graph data.

---

## Getting Started

### Installation

```bash
# From crates.io (binary installs as 'mirage')
cargo install mirage-analyzer

# From source
git clone https://github.com/oldnordic/mirage
cd mirage
cargo install --path .
```

### Requirements

- **Magellan 3.3.3+ / Schema v14** (or v11+ for basic CFG, v13+ for source documents)
  - For CFG extraction and 4D spatial coordinates
  - Run `magellan watch --root ./src --db .magellan/mirage.db` first
- **Rust 1.70+**

### Magellan v10 → v11 Migration

If you're upgrading from Magellan v10 to v11, follow these steps:

```bash
# 1. Check your current Magellan schema version
mirage status --db .magellan/mirage.db
# Look for: "Schema version: 1 (Magellan: X)"

# 2. If it shows v10 or earlier, rebuild your Magellan database
rm .magellan/mirage.db
magellan watch --root ./src --db .magellan/mirage.db --scan-initial

# 3. Verify the new schema
mirage status --db .magellan/mirage.db
# Should show: "Schema version: 1 (Magellan: 11)"

# 4. Clear old path caches (they use function_hash, v11 uses cfg_hash)
# This is automatic - Mirage will rebuild caches on first run
mirage paths --function "some_function" --db .magellan/mirage.db

# 5. Verify 4D coordinates are available
mirage cfg --function "main" --output json --db .magellan/mirage.db | jq '.data.blocks[0]'
# Should include: coord_x, coord_y, coord_z, coord_t
```

**What changed in Magellan v11:**
- Added `coord_t` column for temporal/type metadata
- Changed from `function_hash` to `cfg_hash` for cache invalidation
- Better cache invalidation when CFG structure changes

**Backward compatibility:** Mirage 1.2.4+ works with both v10 and v11 databases, but v11 is recommended for the full 4D coordinate experience.

### Full Workflow Setup

For the complete code intelligence workflow:

```bash
# Install all tools
cargo install magellan llmgrep mirage-analyzer splice

# In your project directory
cd /path/to/rust/project

# Start the call graph indexer (magellan) - this creates CFG data
magellan watch --root ./src --db .magellan/mirage.db

# Search symbols (llmgrep)
llmgrep search --query "function_name" --db .magellan/mirage.db

# Analyze paths (mirage)
mirage paths --function "main" --db .magellan/mirage.db
```

### First Usage

```bash
# 1. Navigate to your Rust project
cd /path/to/rust/project

# 2. Create database with Magellan (this extracts CFG data)
magellan watch --root ./src --db .magellan/mirage.db

# 3. Analyze with Mirage
mirage status --db .magellan/mirage.db
mirage paths --function "main" --db .magellan/mirage.db
mirage cfg --function "main" --db .magellan/mirage.db
```

---

## Global Options

These options apply to all commands:

| Option | Description | Default |
|--------|-------------|---------|
| `--db <PATH>` | Path to SQLite database | `.magellan/mirage.db` |
| `--output <FORMAT>` | Output: `human`, `json`, `pretty` | `human` |

Set the database path with environment variable:
```bash
export MIRAGE_DB=/custom/path/mirage.db
```

---

## Commands Reference

### `status` - Database Statistics

Show what's stored in the database.

```bash
mirage status
```

**Output:**
```
Database Statistics
==================
Functions:    45
CFG Blocks:   387
Paths:        1,234
Dominators:   Calculated on-demand
```

---

### `paths` - Execution Paths

Show all execution paths through a function.

```bash
mirage paths --function "function_name"
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function symbol ID or simple name |
| `--file <PATH>` | File path to disambiguate duplicate names (optional) |
| `--show-errors` | Show only error-returning paths |
| `--max-length <N>` | Prune paths longer than N (default: 1000) |
| `--with-blocks` | Include block details in output |

**Output (human):**
```
Paths: function_name
====================================

Found 3 paths (1 error, 2 normal)

Path #1: Normal (length 3)
  Entry → Block1 → Block3 → Exit

Path #2: Normal (length 2)
  Entry → Block2 → Exit

Path #3: Error (length 2)
  Entry → Block1 → Panic
```

**JSON Output:**
```json
{
  "function": "function_name",
  "total_paths": 3,
  "paths": [
    {
      "path_id": "abc123...",
      "kind": "normal",
      "length": 3,
      "blocks": [...]
    }
  ]
}
```

---

### `cfg` - Control-Flow Graph

Display the control-flow graph for a function.

```bash
mirage cfg --function "function_name"
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function to display |
| `--file <PATH>` | File path to disambiguate duplicate names (optional) |
| `--format <FORMAT>` | `human`, `dot`, or `json` |

**Human Output:**
```
CFG: function_name
=============================

Block 0 (Entry)
├── Terminator: Goto(Block1)
└── Outgoing: [Block1]

Block 1
├── Terminator: SwitchInt(var, targets: [Block2, Block3])
└── Outgoing: [Block2, Block3, Block4]

...
```

**DOT Export (for Graphviz):**
```bash
mirage cfg --function foo --format dot > cfg.dot
dot -Tpng cfg.dot -o cfg.png
```

---

### `dominators` - Dominance Analysis

Compute which code MUST execute on any path from entry to exit.

```bash
mirage dominators --function "function_name"
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function to analyze |
| `--file <PATH>` | File path to disambiguate duplicate names (optional) |
| `--must-pass-through <ID>` | Show blocks dominated by this block |
| `--post` | Show post-dominators (reverse) |
| `--inter-procedural` | Use call graph dominance (requires Magellan) |

**What is Dominance?**
- Block A dominates Block B if ALL paths from entry to B must pass through A
- Useful for proving code MUST execute (e.g., validation happens before use)

**Output:**
```
Dominators: function_name
======================================

Block 0 (Entry)
├── Immediate: ─
└── Dominates: Block1, Block2, Block3

Block 1
├── Immediate: Block0
└── Dominates: Block2

Must-pass-through Block1:
  - Block2 (via Block1 → Block2)
```

---

### `loops` - Natural Loop Detection

Find loops in the control-flow graph.

```bash
mirage loops --function "function_name"
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function to analyze |
| `--file <PATH>` | File path to disambiguate duplicate names (optional) |
| `--verbose` | Show loop body block IDs |

**What is a Natural Loop?**
A back-edge (N → H) where H dominates N indicates a loop with header H.

**Output:**
```
Loops: function_name
================================

Found 2 loops

Loop #1: Header Block5
├── Back edge from: Block7
├── Body size: 3 blocks
└── Nesting level: 1 (outermost)

Loop #2: Header Block8
├── Back edge from: Block8 (self-loop)
├── Body size: 1 block
└── Nesting level: 2 (nested in Loop #1)
```

---

### `unreachable` - Dead Code Detection

Find code blocks that cannot be reached from any entry point.

```bash
mirage unreachable
```

| Option | Description |
|--------|-------------|
| `--within-functions` | Group by function |
| `--show-branches` | Show incoming edge details |
| `--include-uncalled` | Include uncalled functions (Magellan) |

**Output:**
```
Unreachable Code
=================

Function: obsolete_module
  Block 12: Line 45 (dead code after return)
  Block 13: Line 50 (unreachable branch)

Total: 2 unreachable blocks in 1 function(s)
```

---

### `patterns` - Branching Patterns

Detect if/else and match patterns in the CFG.

```bash
mirage patterns --function "function_name"
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function to analyze |
| `--file <PATH>` | File path to disambiguate duplicate names (optional) |
| `--if-else` | Show only if/else patterns |
| `--match` | Show only match patterns |

**Output:**
```
Patterns: function_name
=====================================

If/Else Patterns:
  Pattern #1: Block1
    ├── True branch: Block2
    └── False branch: Block3
    └── Merge point: Block4

Match Patterns:
  Pattern #1: Block5
    ├── Arms: Block6, Block7, Block8
    └── Merge point: Block9
```

---

### `frontiers` - Dominance Frontiers

Compute dominance frontiers (used for SSA placement).

```bash
mirage frontiers --function "function_name"
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function to analyze |
| `--file <PATH>` | File path to disambiguate duplicate names (optional) |
| `--node <ID>` | Show frontiers for specific node only |
| `--iterated` | Show iterated dominance frontier |

**What is a Dominance Frontier?**
The set of nodes where a dominator's dominance ends. Used for phi variable placement in SSA.

---

### `verify` - Path Verification

Verify a cached path is still valid after code changes.

```bash
mirage verify --path-id "abc123def456..."
```

| Option | Description |
|--------|-------------|
| `--path-id <ID>` | Path ID to verify |

**Output:**
```
Path Verification
=================

Path ID: abc123def456...
Status: VALID

The path still exists in the current CFG.
```

---

### `blast-zone` - Impact Analysis

Show what code is affected by changes to a specific block or path.

```bash
mirage blast-zone --function "function_name" --block-id 0
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function containing the block |
| `--file <PATH>` | File path to disambiguate duplicate names (optional) |
| `--block-id <ID>` | Block ID to analyze from (default: 0) |
| `--path-id <ID>` | Analyze impact from specific path |
| `--max-depth <N>` | Maximum traversal depth (default: 100) |
| `--include-errors` | Include error paths in analysis |
| `--use-call-graph` | Use call graph for inter-procedural impact |
| `--call-depth <N>` | Limit inter-procedural traversal to N call hops (default: 0 = unlimited reachability) |

**What is a Blast Zone?**
The set of all code reachable from a given point. Changing code in the blast zone affects all downstream execution.

**Depth-aware inter-procedural analysis:**
Use `--call-depth N` with `--use-call-graph` to limit call graph traversal to N hops. This uses depth-aware BFS through the call graph, showing propagation depth for each affected symbol.

```bash
# Depth-limited blast zone (3 call hops)
mirage blast-zone --function "index_file" --use-call-graph --call-depth 3
```

When `--call-depth` is 0 (default), blast-zone uses existing unlimited reachability. When > 0, each affected symbol is annotated with its depth in the call chain.

**Output (without --call-depth):**
```
Blast Zone: function_name:Block0
==============================================

Intra-Procedural Impact (CFG):
  Block1 → Block2 → Block3
  Block1 → Block4 → Exit

Affected functions: 1 (within same function)
```

**Output (with --call-depth 3):**
```
Blast Zone: function_name:Block0
==============================================

Inter-Procedural Impact (call graph, depth ≤ 3):
  [d1] helper_a → helper_b
  [d2] helper_b → process
  [d3] process → commit

Affected functions: 4 (across 3 call hops)
```

---

### `cycles` - Cycle Detection

Find cycles in code at both call graph and CFG levels.

```bash
mirage cycles
```

| Option | Description |
|--------|-------------|
| `--call-graph` | Show call graph cycles (SCCs) |
| `--function-loops` | Show function loops (within CFG) |
| `--both` | Show both types (default) |
| `--verbose` | Show cycle members |

**Output:**
```
Cycles Detected
===============

Call Graph Cycles (Inter-Procedural):
  SCC #1: 2 functions
    ├── foo
    └── bar
    (mutual recursion)

Function Loops (Intra-Procedural):
  foo::process
    └── Loop at Block5 (self-loop)
```

---

### `icfg` - Inter-Procedural CFG

Build an inter-procedural control flow graph combining the entry function with its callees via call/return edges.

```bash
mirage icfg --entry "main"
```

| Option | Description |
|--------|-------------|
| `--entry <NAME>` | Entry function symbol ID or name |
| `--depth <N>` | Maximum call graph traversal depth (default: 3) |
| `--return-edges <BOOL>` | Include return edges (default: true) |
| `--format <FORMAT>` | `dot`, `json`, or `human` |

**What is an ICFG?**
An ICFG (Inter-procedural Control Flow Graph) connects multiple function CFGs with call edges (from caller to callee entry) and return edges (from callee exit back to caller). It enables whole-program path analysis.

**DOT Export (for Graphviz):**
```bash
mirage icfg --entry "main" --format dot > icfg.dot
dot -Tpng icfg.dot -o icfg.png
```

**JSON Output:**
```bash
mirage icfg --entry "main" --format json
```

---

### `diff` - CFG Diff

Compare control-flow graphs between two Magellan database snapshots (e.g., before and after a code change).

```bash
mirage diff --function "main" --before-db old.db --after-db new.db
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function to compare (symbol ID or name) |
| `--before-db <PATH>` | Path to "before" database |
| `--after-db <PATH>` | Path to "after" database |
| `--show-edges` | Show edge differences |
| `--verbose` | Show detailed block changes |

**Workflow:**
1. Run `magellan watch` on the old code, save the `.db`
2. Make code changes
3. Run `magellan watch` on the new code, save to a different `.db`
4. Run `mirage diff --before-db old.db --after-db new.db --function "name"`

**Output:**
```
CFG Diff: main
  Before: old.db
  After: new.db
  Similarity: 85.3%

Added blocks (2):
  + Block 5: conditional @ 42:0-48:0
  + Block 6: return @ 48:0-50:0

Deleted blocks (1):
  - Block 4: return @ 38:0-40:0
```

---

### `slice` - Program Slicing

Compute backward or forward program slices.

```bash
mirage slice --symbol "function_name" --direction backward
```

| Option | Description |
|--------|-------------|
| `--symbol <NAME>` | Symbol to slice |
| `--direction <DIR>` | `backward` (what affects) or `forward` (what is affected) |
| `--verbose` | Show detailed symbol info |

**What is Slicing?**
- **Backward slice:** All code that affects this symbol
- **Forward slice:** All code that this symbol affects

---

### `hotpaths` - Most-Traversed Paths

Identify the most frequently traversed execution paths through a function.

```bash
mirage hotpaths --function "main" --top 10
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function to analyze |
| `--top <N>` | Number of hot paths to return (default: 10) |
| `--rationale` | Show rationale for hotness scores |
| `--min-score <SCORE>` | Minimum hotness threshold (0.0 to 1.0) |

**Output:**
```
Hot Paths: main
================

Found 5 hot paths

1. Path length 3 (hotness: 0.85)
   Entry → Block1 → Block2 → Exit
   Rationale: Dominates 12 other paths

2. Path length 2 (hotness: 0.72)
   Entry → Block3 → Exit
   Rationale: Shared prefix with 8 paths
```

---

### `hotspots` - High-Risk Functions

Identify high-risk functions using path counts, call dominance, and complexity.

```bash
mirage hotspots --entry main --top 10
```

| Option | Description |
|--------|-------------|
| `--entry <SYMBOL>` | Entry point for analysis (default: main) |
| `--top <N>` | Max hotspots to return (default: 20) |
| `--min-paths <N>` | Minimum path count threshold |
| `--verbose` | Show detailed metrics |
| `--inter-procedural` | Use call graph analysis (requires Magellan) |

**Risk Score Calculation:**
- Combines path count, SCC size (coupling), and complexity
- Higher score = higher risk

**Output:**
```
Hotspots Analysis (entry: main)
================================

Found 10 hotspots out of 45 functions

1. process_request (risk: 42.5)
   Paths: 15  Dominance: 3.0  Complexity: 12

2. handle_error (risk: 38.2)
   Paths: 8  Dominance: 2.0  Complexity: 8
```

---

### `migrate` - Database Migration

Migrate a database between storage backends.

```bash
mirage migrate --from sqlite --to geometric --db .magellan/mirage.db
```

| Option | Description |
|--------|-------------|
| `--db <PATH>` | Database path to migrate |
| `--backup` | Create backup before migrating |
| `--dry-run` | Detect format only, do not migrate |

---

### `docs` - Source Documents

List source documents from Magellan's graph memory tables (requires schema 13+).

```bash
mirage docs --db .magellan/mirage.db
```

| Option | Description |
|--------|-------------|
| `--kind <KIND>` | Filter by source kind (wiki, code, message, etc.) |
| `--tag <TAG>` | Filter by tag |
| `--limit <N>` | Maximum results (default: 50) |

**Examples:**
```bash
# List all source documents
mirage docs --db .magellan/mirage.db

# Filter by kind
mirage docs --db .magellan/mirage.db --kind wiki

# Filter by tag, JSON output
mirage docs --db .magellan/mirage.db --tag rust --output json
```

Graceful degradation: returns "No source documents found" when the `source_documents` table is missing (pre-schema-13 databases).

---

## Output Formats

All commands support three output formats:

### Human (default)
Readable text with color and formatting:
```bash
mirage paths --function foo
```

### JSON
Compact JSON for scripting:
```bash
mirage paths --function foo --output json | jq '.paths | length'
```

### Pretty
Formatted JSON with indentation:
```bash
mirage paths --function foo --output pretty
```

---

## Database Schema

| Table | Description |
|-------|-------------|
| `graph_entities` | Functions and their metadata |
| `cfg_blocks` | Basic blocks within functions |
| `cfg_edges` | Control flow edges |
| `cfg_paths` | Enumerated execution paths |
| `cfg_dominators` | Dominance relationships |

---

## Tips & Tricks

### Chaining Commands

Use JSON output to pipe between commands:
```bash
mirage paths --function foo --output json | jq '.paths[].path_id' | xargs -I {} mirage verify --path-id {}
```

### Working with Large Codebases

For large projects, use specific function targeting and path limits:
```bash
mirage paths --function foo --max-length 50
```

---

## Troubleshooting

### "No such function in database"
The function hasn't been indexed yet. Run `magellan watch` first.

### "Magellan database not available"
Inter-procedural features require Magellan. Run `magellan watch` first or omit those flags.

---

## See Also

### Companion Tools

- [Magellan](https://github.com/oldnordic/magellan) - Call graph indexer
- [llmgrep](https://github.com/oldnordic/llmgrep) - Semantic code search
- [sqlitegraph](https://crates.io/crates/sqlitegraph) - Shared graph database library
- [splice](https://github.com/oldnordic/splice) - Precision code editing with spans

### Documentation

- [README.md](README.md) - Project overview and quick start
