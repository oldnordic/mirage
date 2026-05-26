# Mirage

[![Crates.io](https://img.shields.io/crates/v/mirage-analyzer)](https://crates.io/crates/mirage-analyzer)
[![Documentation](https://docs.rs/mirage-analyzer/badge.svg)](https://docs.rs/mirage-analyzer)

**Version:** 1.5.0

Path-aware code intelligence engine for Rust. Analyzes control-flow graphs from Magellan databases.

## Purpose

Mirage reads Magellan code graphs and provides control-flow analysis:
- Path enumeration through functions
- Dominance and post-dominance analysis
- Natural loop detection
- Dead code detection
- Call graph cycle detection
- Inter-procedural reachability
- Source document listing from graph memory tables

## Quick Start

```bash
# Install
cargo install mirage-analyzer

# Index your codebase with Magellan first
magellan watch --root ./src --db .magellan/mirage.db

# Analyze CFG
mirage --db .magellan/mirage.db paths --function "main"
mirage --db .magellan/mirage.db cfg --function "process"
mirage --db .magellan/mirage.db cycles
mirage --db .magellan/mirage.db docs --kind wiki
```

## Backends

Mirage supports Magellan's database formats:

| Backend | File Extension | Feature Flag | Status |
|---------|---------------|--------------|--------|
| SQLite | `.db` | `backend-sqlite` | Default |
| Geometric | `.geo` | `backend-geometric` | Supported |

**Note:** The SQLite backend is now default. Install with:
```bash
# Default (SQLite)
cargo install mirage-analyzer

# Geometric backend only
cargo install mirage-analyzer --features backend-geometric --no-default-features
```

## Requirements

- **[Magellan](https://github.com/oldnordic/magellan)** 3.3.3+ / Schema v14 (or v11+ for basic CFG, v13+ for source documents)
- Rust 1.70+
- Magellan database (`.db`) created by `magellan watch`

**Database Location:** Default is `.magellan/mirage.db` (auto-discovered)

## Documentation

- **[MANUAL.md](MANUAL.md)** — Command reference
- **[ARCHITECTURE.md](ARCHITECTURE.md)** — Design and integration
- **[API_INTEGRATION.md](API_INTEGRATION.md)** — Magellan contract
- **[INVARIANTS.md](INVARIANTS.md)** — Behavioral guarantees
- **[CHANGELOG.md](CHANGELOG.md)** — Version history

## License

GPL-3.0. See [LICENSE](LICENSE).
