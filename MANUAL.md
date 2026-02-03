# Mirage User Manual

Version 0.1.0

---

## Overview

Mirage is a path-aware code intelligence tool for Rust. It extracts control-flow graphs (CFG) from Rust MIR, enumerates execution paths, and provides graph-based analysis capabilities.

**Core Principle:** An agent may only speak if it can reference a graph artifact.

---

## Getting Started

### Installation

```bash
# From source
git clone https://github.com/oldnordic/mirage
cd mirage
cargo install --path .

# Or using cargo install
cargo install mirage
```

### Requirements

- **Charon:** Install [Charon](https://github.com/AeneasVerif/charon) and ensure it's in your PATH
- **Magellan (optional):** For inter-procedural features, run `magellan watch` on your project first

### First Usage

```bash
# 1. Navigate to your Rust project
cd /path/to/rust/project

# 2. Index the project
mirage index --project .

# 3. Query analysis results
mirage paths --function "my_crate::main"
mirage cfg --function "my_crate::main"
```

---

## Global Options

These options apply to all commands:

| Option | Description | Default |
|--------|-------------|---------|
| `--db <PATH>` | Path to SQLite database | `./codemcp/mirage.db` |
| `--output <FORMAT>` | Output: `human`, `json`, `pretty` | `human` |

Set the database path with environment variable:
```bash
export MIRAGE_DB=/custom/path/mirage.db
```

---

## Commands Reference

### `index` - Index a Rust Project

Extracts MIR from Rust source and builds control-flow graphs.

```bash
mirage index --project /path/to/project
```

| Option | Description |
|--------|-------------|
| `--project <PATH>` | Path to Rust project root |
| `--crate <NAME>` | Index only this crate |
| `--incremental` | Only re-index changed functions |
| `--reindex <ID>` | Re-index specific function by symbol_id |

**What it does:**
1. Runs `charon` on the project to extract MIR
2. Converts MIR to CFG for each function
3. Enumerates execution paths with BLAKE3 hashing
4. Stores results in SQLite database

**Output:**
```
Indexing /path/to/project
[████████████████████] 100% (45/45 functions)

Processed: 45  Updated: 45  Skipped: 0  Errors: 0
```

---

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
mirage paths --function "my_crate::function_name"
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function symbol ID or fully qualified name |
| `--show-errors` | Show only error-returning paths |
| `--max-length <N>` | Prune paths longer than N (default: 1000) |
| `--with-blocks` | Include block details in output |

**Output (human):**
```
Paths: my_crate::function_name
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
  "function": "my_crate::function_name",
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
mirage cfg --function "my_crate::function_name"
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function to display |
| `--format <FORMAT>` | `human`, `dot`, or `json` |

**Human Output:**
```
CFG: my_crate::function_name
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
mirage dominators --function "my_crate::function_name"
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function to analyze |
| `--must-pass-through <ID>` | Show blocks dominated by this block |
| `--post` | Show post-dominators (reverse) |
| `--inter-procedural` | Use call graph dominance (requires Magellan) |

**What is Dominance?**
- Block A dominates Block B if ALL paths from entry to B must pass through A
- Useful for proving code MUST execute (e.g., validation happens before use)

**Output:**
```
Dominators: my_crate::function_name
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
mirage loops --function "my_crate::function_name"
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function to analyze |
| `--verbose` | Show loop body block IDs |

**What is a Natural Loop?**
A back-edge (N → H) where H dominates N indicates a loop with header H.

**Output:**
```
Loops: my_crate::function_name
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

Function: my_crate::obsolete_module
  Block 12: Line 45 (dead code after return)
  Block 13: Line 50 (unreachable branch)

Total: 2 unreachable blocks in 1 function(s)
```

---

### `patterns` - Branching Patterns

Detect if/else and match patterns in the CFG.

```bash
mirage patterns --function "my_crate::function_name"
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function to analyze |
| `--if-else` | Show only if/else patterns |
| `--match` | Show only match patterns |

**Output:**
```
Patterns: my_crate::function_name
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
mirage frontiers --function "my_crate::function_name"
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function to analyze |
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
mirage blast-zone --function "my_crate::function_name" --block-id 0
```

| Option | Description |
|--------|-------------|
| `--function <NAME>` | Function containing the block |
| `--block-id <ID>` | Block ID to analyze from (default: 0) |
| `--path-id <ID>` | Analyze impact from specific path |
| `--max-depth <N>` | Maximum traversal depth (default: 100) |
| `--include-errors` | Include error paths in analysis |
| `--use-call-graph` | Use call graph for inter-procedural impact |

**What is a Blast Zone?**
The set of all code reachable from a given point. Changing code in the blast zone affects all downstream execution.

**Output:**
```
Blast Zone: my_crate::function_name:Block0
==============================================

Intra-Procedural Impact (CFG):
  Block1 → Block2 → Block3
  Block1 → Block4 → Exit

Affected functions: 1 (within same function)
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

### `slice` - Program Slicing

Compute backward or forward program slices.

```bash
mirage slice --symbol "my_crate::function_name" --direction backward
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

### Incremental Updates

After making changes, re-index only what changed:
```bash
mirage index --project . --incremental
```

### Chaining Commands

Use JSON output to pipe between commands:
```bash
mirage paths --function foo --output json | jq '.paths[].path_id' | xargs -I {} mirage verify --path-id {}
```

### Working with Large Codebases

For large projects, use specific crate targeting:
```bash
mirage index --project . --crate my_crate
```

---

## Troubleshooting

### "charon: command not found"
Install Charon from https://github.com/AeneasVerif/charon and ensure it's in your PATH.

### "No such function in database"
The function hasn't been indexed yet. Run `mirage index` first.

### "Magellan database not available"
Inter-procedural features require Magellan. Run `magellan watch` first or omit those flags.

---

## See Also

- [README.md](README.md) - Project overview
- [Magellan](https://github.com/oldnordic/magellan) - Call graph indexer
- [llmgrep](https://github.com/oldnordic/llmgrep) - Semantic search
