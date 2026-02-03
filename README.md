# Mirage

**Path-Aware Code Intelligence Engine for Rust**

[![Crates.io](https://img.shields.io/crates/v/mirage-analyzer)](https://crates.io/crates/mirage-analyzer)
[![Documentation](https://docs.rs/mirage-analyzer/badge.svg)](https://docs.rs/mirage-analyzer)

> "An agent may only speak if it can reference a graph artifact."

## Part of the Code Intelligence Toolset

Mirage is one of five complementary tools designed to work together for comprehensive code analysis:

| Tool | Purpose | Install |
|------|---------|---------|
| **[Magellan](https://github.com/oldnordic/magellan)** | Call graph indexing and symbol navigation | `cargo install magellan` |
| **[llmgrep](https://github.com/oldnordic/llmgrep)** | Semantic code search over indexed symbols | `cargo install llmgrep` |
| **[Mirage](https://github.com/oldnordic/mirage)** | Control-flow analysis and path enumeration | `cargo install mirage-analyzer` |
| **[sqlitegraph](https://crates.io/crates/sqlitegraph)** | Shared graph database library (dependency) | Included automatically |
| **[splice](https://github.com/oldnordic/splice)** | Source code transformation with span precision | `cargo install splice` |

```
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│  Magellan   │ ───► │  llmgrep    │ ───► │   Mirage    │
│ (Call Graph)│      │  (Search)   │      │  (CFG/Paths)│
└─────────────┘      └─────────────┘      └─────────────┘
       │                    │                     │
       └────────────────────┴─────────────────────┘
                     │
              ┌──────▼──────┐
              │ sqlitegraph │
              │  (Database) │
              └─────────────┘
```

**Important:** Mirage provides its full capabilities when used together with Magellan. Inter-procedural analysis features (call graph dominance, hotspots, cross-function slicing) require Magellan's call graph data.

## What is Mirage?

Mirage is a command-line tool that extracts control-flow graphs (CFG) from Rust code via MIR, enumerates execution paths, and provides graph-based reasoning capabilities. It stores analysis results in a SQLite database for incremental updates.

### What Mirage is NOT

- ❌ A search tool (use [llmgrep](https://github.com/oldnordic/llmgrep))
- ❌ An embedding or semantic search tool
- ❌ A linter or static analysis tool
- ❌ A code completion engine
- ❌ A call graph indexer (use [Magellan](https://github.com/oldnordic/magellan))

### What Mirage IS

- ✅ CFG extraction from Rust MIR via Charon
- ✅ Path enumeration with caching
- ✅ Dominance analysis (dominators, post-dominators, frontiers)
- ✅ Loop detection (natural loops within functions)
- ✅ Dead code detection (unreachable blocks)
- ✅ Impact analysis (blast zones, program slicing)
- ✅ Inter-procedural analysis (with Magellan: call graph condensation, hotspots)

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

### 1. Index a Rust Project

```bash
mirage index --project /path/to/rust/project
```

This requires [Charon](https://github.com/AeneasVerif/charon) to be installed and in your PATH. Mirage will:
1. Extract MIR from the Rust project
2. Build CFG for each function
3. Enumerate and cache execution paths
4. Store results in `./codemcp/mirage.db` (or use `--db` to specify)

### 2. Query Execution Paths

```bash
# Show all paths through a function
mirage paths --function "my_crate::function_name"

# Show only error-returning paths
mirage paths --function "my_crate::function_name" --show-errors

# Limit path exploration depth
mirage paths --function "my_crate::function_name" --max-length 10
```

### 3. Visualize Control Flow

```bash
# Human-readable CFG
mirage cfg --function "my_crate::function_name"

# Export to Graphviz DOT
mirage cfg --function "my_crate::function_name" --format dot > cfg.dot
dot -Tpng cfg.dot -o cfg.png
```

### 4. Dominance Analysis

```bash
# Show dominance tree
mirage dominators --function "my_crate::function_name"

# Find blocks that must pass through a specific block
mirage dominators --function "my_crate::function_name" --must-pass-through 5

# Inter-procedural dominance (call graph level)
mirage dominators --function "my_crate::function_name" --inter-procedural
```

### 5. Find Dead Code

```bash
# Find unreachable code blocks
mirage unreachable

# Include uncalled functions (requires Magellan call graph)
mirage unreachable --include-uncalled
```

### 6. Impact Analysis

```bash
# What does this code affect? (blast zone from a block)
mirage blast-zone --function "my_crate::function_name" --block-id 0

# What affects this code? (backward slicing)
mirage slice --symbol "my_crate::function_name" --direction backward

# What does this affect? (forward slicing)
mirage slice --symbol "my_crate::function_name" --direction forward
```

## CLI Reference

### Global Options

| Option | Description |
|--------|-------------|
| `--db <DB>` | Path to the database (default: `./codemcp/mirage.db`) |
| `--output <FORMAT>` | Output format: `human`, `json`, `pretty` |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

### Commands

#### `index` - Index a Rust Project

```
mirage index [OPTIONS]

Options:
  --project <PROJECT>    Path to the Rust project to index
  --crate <CRATE>        Index specific crate
  --incremental           Only re-index changed functions
  --reindex <REINDEX>     Re-index only this function (by symbol_id)
```

#### `status` - Show Database Statistics

```
mirage status

Shows: function count, CFG blocks, paths, dominators stored
```

#### `paths` - Show Execution Paths

```
mirage paths --function <FUNCTION> [OPTIONS]

Options:
  --show-errors          Show only error-returning paths
  --max-length <N>       Maximum path length (default: 1000)
  --with-blocks          Show block details for each path
```

#### `cfg` - Show Control-Flow Graph

```
mirage cfg --function <FUNCTION> [OPTIONS]

Options:
  --format <FORMAT>      Output format: human, dot, json
```

#### `dominators` - Dominance Analysis

```
mirage dominators --function <FUNCTION> [OPTIONS]

Options:
  --must-pass-through <ID>  Show blocks that must pass through this block
  --post                    Show post-dominators instead
  --inter-procedural        Use call graph dominance (requires Magellan)
```

#### `loops` - Natural Loop Detection

```
mirage loops --function <FUNCTION> [OPTIONS]

Options:
  --verbose                Show detailed loop body blocks
```

#### `unreachable` - Dead Code Detection

```
mirage unreachable [OPTIONS]

Options:
  --within-functions        Group unreachable code by function
  --show-branches           Show incoming edge details
  --include-uncalled         Include uncalled functions (Magellan)
```

#### `patterns` - Branching Pattern Detection

```
mirage patterns --function <FUNCTION> [OPTIONS]

Options:
  --if-else                Show only if/else patterns
  --match                  Show only match patterns
```

#### `frontiers` - Dominance Frontiers

```
mirage frontiers --function <FUNCTION> [OPTIONS]

Options:
  --node <ID>              Show frontiers for specific node only
  --iterated               Show iterated dominance frontier
```

#### `verify` - Path Verification

```
mirage verify --path-id <PATH_ID>

Verifies a cached path is still valid after code changes.
```

#### `blast-zone` - Impact Analysis

```
mirage blast-zone [OPTIONS]

Options:
  --function <FUNCTION>    Function to analyze
  --block-id <ID>          Block ID to analyze from (default: 0)
  --path-id <PATH_ID>      Analyze impact from specific path
  --max-depth <N>           Maximum traversal depth (default: 100)
  --include-errors         Include error paths
  --use-call-graph         Use call graph for inter-procedural analysis
```

#### `cycles` - Cycle Detection

```
mirage cycles [OPTIONS]

Options:
  --call-graph             Show call graph cycles (SCCs)
  --function-loops         Show function loops (within CFG)
  --both                   Show both types (default)
  --verbose                Show cycle/loop members
```

#### `slice` - Program Slicing

```
mirage slice --symbol <SYMBOL> --direction <DIRECTION> [OPTIONS]

Options:
  --direction <DIR>        backward (what affects) or forward (what is affected)
  --verbose                Show detailed symbol information
```

#### `hotspots` - High-Risk Function Analysis

```
mirage hotspots [OPTIONS]

Options:
  --entry <SYMBOL>         Entry point (default: main)
  --top <N>                 Max hotspots to return (default: 20)
  --min-paths <N>          Minimum path count threshold
  --verbose                Show detailed metrics
  --inter-procedural        Use call graph analysis (requires Magellan)
```

## Output Formats

All commands support three output formats:

| Format | Description |
|--------|-------------|
| `human` | Readable text output (default) |
| `json` | Compact JSON for scripting |
| `pretty` | Formatted JSON with indentation |

Example:
```bash
mirage paths --function foo --output json | jq '.paths[].path_id'
```

## Database

Mirage stores analysis in a SQLite database with the following tables:

| Table | Purpose |
|-------|---------|
| `cfg_blocks` | Basic blocks within functions |
| `cfg_edges` | Control flow edges between blocks |
| `cfg_paths` | Enumerated execution paths |
| `cfg_dominators` | Dominance relationships |
| `graph_entities` | Function metadata |

The default database location is `./codemcp/mirage.db`. Use the `MIRAGE_DB` environment variable or `--db` flag to override.

## Requirements

- Rust 1.77+
- [Charon](https://github.com/AeneasVerif/charon) binary in PATH (for MIR extraction)
- **For inter-procedural features:** [Magellan](https://github.com/oldnordic/magellan) indexed database
- **Recommended for full workflow:**
  - [Magellan](https://crates.io/crates/magellan) - `cargo install magellan`
  - [llmgrep](https://crates.io/crates/llmgrep) - `cargo install llmgrep`
  - [splice](https://github.com/oldnordic/splice) - `cargo install splice`

## License

GPL-3.0-or-later

## The Complete Toolset

Mirage is part of a coordinated set of tools for AI-assisted code understanding:

| Tool | Function |
|------|----------|
| [Magellan](https://github.com/oldnordic/magellan) | Builds call graphs, indexes all symbols |
| [llmgrep](https://github.com/oldnordic/llmgrep) | Searches symbols semantically |
| [Mirage](https://github.com/oldnordic/mirage) | Analyzes control flow and execution paths |
| [sqlitegraph](https://crates.io/crates/sqlitegraph) | Shared graph database format |
| [splice](https://github.com/oldnordic/splice) | Precision code editing using spans |

These tools share a common database format and are designed to be used together by AI agents and developers.

## External Dependencies

- [Charon](https://github.com/AeneasVerif/charon) - Rust MIR extraction (required)
