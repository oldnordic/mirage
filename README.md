# Mirage

[![Crates.io](https://img.shields.io/crates/v/mirage-analyzer)](https://crates.io/crates/mirage-analyzer)
[![Documentation](https://docs.rs/mirage-analyzer/badge.svg)](https://docs.rs/mirage-analyzer)

**Version:** 1.2.0

Path-aware code intelligence engine for Rust. Analyzes control-flow graphs from Magellan databases.

## Purpose

Mirage reads Magellan code graphs and provides control-flow analysis:
- Path enumeration through functions
- Dominance and post-dominance analysis
- Natural loop detection
- Dead code detection
- Call graph cycle detection
- Inter-procedural reachability

## Quick Start

```bash
# Install
cargo install mirage-analyzer

# Index your codebase with Magellan first
magellan watch --root ./src --db code.geo

# Analyze CFG
mirage --db code.geo paths --function "my_crate::main"
mirage --db code.geo cfg --function "my_crate::process"
mirage --db code.geo cycles
```

## Backends

Mirage supports Magellan's database formats:

| Backend | File Extension | Feature Flag | Status |
|---------|---------------|--------------|--------|
| Geometric | `.geo` | `backend-geometric` | Default |
| SQLite | `.db` | `backend-sqlite` | Supported |

**Note:** The Geometric backend is now default. Install with:
```bash
# Default (Geometric)
cargo install mirage-analyzer

# SQLite backend only
cargo install mirage-analyzer --features backend-sqlite --no-default-features
```

## Requirements

- **[Magellan](https://github.com/oldnordic/magellan)** 3.0+ — Code graph indexing
- Magellan database (`.geo` or `.db`) created by `magellan watch`

## Documentation

- **[MANUAL.md](MANUAL.md)** — Command reference
- **[ARCHITECTURE.md](ARCHITECTURE.md)** — Design and integration
- **[API_INTEGRATION.md](API_INTEGRATION.md)** — Magellan contract
- **[INVARIANTS.md](INVARIANTS.md)** — Behavioral guarantees
- **[CHANGELOG.md](CHANGELOG.md)** — Version history

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
