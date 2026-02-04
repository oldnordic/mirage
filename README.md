# Mirage

**Path-Aware Code Intelligence Engine for Rust**

[![Crates.io](https://img.shields.io/crates/v/mirage-analyzer)](https://crates.io/crates/mirage-analyzer)
[![Documentation](https://docs.rs/mirage-analyzer/badge.svg)](https://docs.rs/mirage-analyzer)

> "An agent may only speak if it can reference a graph artifact."

## ⚠️ Requires Magellan

**Mirage requires [Magellan](https://github.com/oldnordic/magellan) to function.**

Magellan provides the AST-based control flow graph (CFG) data that Mirage analyzes. You must run `magellan watch` on your codebase before using Mirage.

```bash
# Install Magellan first
cargo install magellan

# Watch your project (builds CFG)
magellan watch --root ./src --db .codemcp/codegraph.db

# Now Mirage can analyze
mirage status
```

## The Code Intelligence Toolset

Mirage is part of a coordinated toolset built on [sqlitegraph](https://github.com/oldnordic/sqlitegraph). All tools share a common SQLite graph database and are designed to work together for AI-assisted code understanding.

```
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│  Magellan   │ ───► │  llmgrep    │ ───► │   Mirage    │
│(Symbols &   │      │ (Semantic   │      │(CFG & Paths)│
│  Call Graph)│      │  Search)    │      │             │
└─────────────┘      └─────────────┘      └─────────────┘
       │                    │                     │
       └────────────────────┴─────────────────────┘
                     │
              ┌──────▼──────┐
              │ sqlitegraph │
              │  (Database) │
              └─────────────┘
                     │
              ┌──────▼──────┐
              │   splice    │
              │(Edit using  │
              │   spans)    │
              └─────────────┘
```

| Tool | Purpose | Repository | Install |
|------|---------|------------|---------|
| **sqlitegraph** | Graph database foundation | [github.com/oldnordic/sqlitegraph](https://github.com/oldnordic/sqlitegraph) | `cargo add sqlitegraph` |
| **Magellan** | Call graph indexing, symbol navigation | [github.com/oldnordic/magellan](https://github.com/oldnordic/magellan) | `cargo install magellan` |
| **llmgrep** | Semantic code search | [github.com/oldnordic/llmgrep](https://github.com/oldnordic/llmgrep) | `cargo install llmgrep` |
| **Mirage** | CFG analysis, path enumeration | [github.com/oldnordic/mirage](https://github.com/oldnordic/mirage) | `cargo install mirage-analyzer` |
| **splice** | Precision code editing | [github.com/oldnordic/splice](https://github.com/oldnordic/splice) | `cargo install splice` |

## What is Mirage?

Mirage analyzes control-flow graphs extracted by Magellan to answer questions like:
- What execution paths exist through this function?
- What code MUST execute on any path from entry to exit?
- Which blocks are unreachable (dead code)?
- What is the impact of changing this code?

### What Mirage is NOT

- ❌ A search tool (use [llmgrep](https://github.com/oldnordic/llmgrep))
- ❌ An embedding or semantic search tool
- ❌ A linter or static analysis tool
- ❌ A code completion engine
- ❌ A call graph indexer (use [Magellan](https://github.com/oldnordic/magellan))

### What Mirage IS

- ✅ CFG analysis from Magellan's AST-based data
- ✅ Path enumeration with BLAKE3 caching
- ✅ Dominance analysis (dominators, post-dominators, frontiers)
- ✅ Loop detection (natural loops within functions)
- ✅ Dead code detection (unreachable blocks)
- ✅ Impact analysis (blast zones, program slicing)
- ✅ Inter-procedural analysis (hotspots, call graph condensation)

## Installation

```bash
cargo install mirage-analyzer
```

The binary is installed as `mirage`:

```bash
mirage --help
```

Or build from source:

```bash
git clone https://github.com/oldnordic/mirage
cd mirage
cargo install --path .
```

## Quick Start

### 1. Install the Toolset

```bash
# Install all tools for complete workflow
cargo install magellan        # Call graph & CFG extraction (REQUIRED)
cargo install llmgrep         # Semantic search
cargo install mirage-analyzer # Path-aware analysis
cargo install splice          # Precision editing
```

### 2. Index Your Project

```bash
# Magellan watches your source and builds CFG
magellan watch --root ./src --db .codemcp/codegraph.db
```

Magellan will:
1. Parse your source code with tree-sitter
2. Build AST-based control flow graphs for each function
3. Store everything in a SQLite database

### 3. Analyze with Mirage

```bash
# Check database status
mirage status

# Show all execution paths through a function
mirage paths --function "my_crate::main"

# Find unreachable code
mirage unreachable

# Visualize control flow
mirage cfg --function "my_crate::main" --format dot | dot -Tpng -o cfg.png
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `status` | Database statistics and CFG availability |
| `paths` | Enumerate execution paths through a function |
| `cfg` | Visualize control-flow graph (human/dot/json) |
| `dominators` | Dominance relationships (must-pass-through analysis) |
| `loops` | Detect natural loops within functions |
| `unreachable` | Find dead code (unreachable blocks) |
| `patterns` | Recover if/else and match branching patterns |
| `frontiers` | Compute dominance frontiers |
| `verify` | Verify cached path is still valid |
| `blast-zone` | Impact analysis from a block or path |
| `cycles` | Detect call graph SCCs and function loops |
| `slice` | Program slicing (backward/forward) |
| `hotspots` | Risk scoring based on paths and dominance |

### Global Options

| Option | Description |
|--------|-------------|
| `--db <DB>` | Path to database (default: `.codemcp/codegraph.db`) |
| `--output <FORMAT>` | Output: `human`, `json`, `pretty` |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

## Examples

### Path Enumeration

```bash
# All paths through a function
mirage paths --function "my_crate::process"

# Only error-returning paths
mirage paths --function "my_crate::process" --show-errors

# Limit exploration depth
mirage paths --function "my_crate::process" --max-length 10

# JSON output for scripting
mirage paths --function "my_crate::process" --output json | jq '.paths[].path_id'
```

### Dominance Analysis

```bash
# Show dominance tree
mirage dominators --function "my_crate::process"

# What MUST execute before this block?
mirage dominators --function "my_crate::process" --must-pass-through 5

# Post-dominators (what must execute after)
mirage dominators --function "my_crate::process" --post

# Inter-procedural (call graph level)
mirage dominators --function "my_crate::main" --inter-procedural
```

### Impact Analysis

```bash
# What does this block affect?
mirage blast-zone --function "my_crate::process" --block-id 0

# What affects this function? (backward slice)
mirage slice --symbol "my_crate::process" --direction backward

# What does this function affect? (forward slice)
mirage slice --symbol "my_crate::process" --direction forward

# High-risk functions
mirage hotspots --entry main --top 10
```

## Database Schema

Mirage extends the Magellan database with:

| Table | Purpose |
|-------|---------|
| `cfg_blocks` | Basic blocks within functions (from Magellan) |
| `cfg_paths` | Enumerated execution paths with BLAKE3 IDs |
| `cfg_dominators` | Dominance relationships |

The default database location is `.codemcp/codegraph.db`. This is shared with Magellan.

## Requirements

- **Rust 1.77+**
- **[Magellan](https://github.com/oldnordic/magellan)** — Required for CFG extraction
  ```bash
  cargo install magellan
  magellan watch --root ./src --db .codemcp/codegraph.db
  ```
- **[sqlitegraph](https://crates.io/crates/sqlitegraph)** — Included automatically as dependency

## License

GPL-3.0-or-later

## Related Projects

- [sqlitegraph](https://github.com/oldnordic/sqlitegraph) — Graph database library
- [Magellan](https://github.com/oldnordic/magellan) — Call graph indexer
- [llmgrep](https://github.com/oldnordic/llmgrep) — Semantic search
- [splice](https://github.com/oldnordic/splice) — Precision editing
