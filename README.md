# Mirage

[![Crates.io](https://img.shields.io/crates/v/mirage-analyzer)](https://crates.io/crates/mirage-analyzer)
[![Documentation](https://docs.rs/mirage-analyzer/badge.svg)](https://docs.rs/mirage-analyzer)

**Version:** 1.0.2

Path-aware code intelligence engine for Rust. Analyzes control-flow graphs to find execution paths, dead code, and impact zones.

**Positioning:** CFG analysis tool for codebases indexed by Magellan. Answers questions like "what code MUST execute on any path from entry to exit?"

## Purpose

**Mirage analyzes control-flow graphs from Magellan.** Built for:

- **Developers** — Find dead code, understand execution paths, analyze impact of changes
- **AI Assistants** — Structured path analysis for code reasoning
- **Tooling** — Scriptable CFG analysis with precise block-level data

## Features

- **Path enumeration** — Find all execution paths through a function
- **Dominance analysis** — Compute dominators, post-dominators, dominance frontiers
- **Loop detection** — Identify natural loops within functions
- **Dead code detection** — Find unreachable blocks
- **Impact analysis** — Blast zones, program slicing
- **Hotspots** — Risk scoring based on path counts and complexity

## Quick Start

```bash
# Install
cargo install mirage-analyzer

# Requires Magellan database (create first)
magellan watch --root ./src --db code.v3

# Check database status
mirage status

# Show execution paths through a function
mirage paths --function "my_crate::process"

# Find unreachable code
mirage unreachable

# Visualize control flow
mirage cfg --function "my_crate::main" --format dot
```

## Installation

```bash
cargo install mirage-analyzer
```

Or build from source with Native-V3 backend:

```bash
# Native-V3 backend (recommended - fastest)
cargo install mirage-analyzer --features native-v3 --no-default-features
```

## Backends

| Feature | Description | File | Best For |
|---------|-------------|------|----------|
| `native-v3` | **High-performance binary backend** | `.v3` | Production (recommended) |
| (default) | SQLite backend | `.db` | Compatibility |

Both backends provide identical CFG analysis results.

## Requirements

- **[Magellan](https://github.com/oldnordic/magellan)** 2.4.3+ — Required for CFG extraction
- **[sqlitegraph](https://crates.io/crates/sqlitegraph)** 2.0.3+ — Included automatically

## Documentation

- **[MANUAL.md](MANUAL.md)** — Complete command reference and examples
- **[CHANGELOG.md](CHANGELOG.md)** — Version history

## What Mirage Does NOT Do

- ❌ Search code (use [llmgrep](https://github.com/oldnordic/llmgrep))
- ❌ Index code (use [Magellan](https://github.com/oldnordic/magellan))
- ❌ Type checking or semantic analysis
- ❌ Code editing (use [splice](https://github.com/oldnordic/splice))

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
