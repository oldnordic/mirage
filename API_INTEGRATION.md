# Mirage API Integration with Magellan

## Backend Opening

```rust
use magellan::graph::GeometricBackend;

let backend = GeometricBackend::open(db_path)?;
```

**Rules:**
- File must be valid `.geo` format
- Backend handles file locking
- Read-only connections are safe

## Path Normalization

All paths normalized before queries:

```rust
use mirage_analyzer::integrations::magellan::normalize_path_for_query;

let normalized = normalize_path_for_query("./src/lib.rs");
// Result: "src/lib.rs"
```

## Symbol Lookup

### Unique Match
```rust
SymbolLookupResult::Unique(info)
```

### Ambiguous Match
```rust
SymbolLookupResult::Ambiguous { path, name, candidates }
```

### Not Found
```rust
SymbolLookupResult::NotFound
```

## Graph Query Operations

- `get_cfg_blocks(function_id)` — CFG blocks
- `reachable_from(symbol_id)` — Forward reachable symbols
- `reverse_reachable_from(symbol_id)` — Reverse reachable
- `get_callers(symbol_id)` — Direct callers
- `get_callees(symbol_id)` — Direct callees
- `find_call_graph_cycles()` — Find SCCs

## Version Requirements

- **Magellan**: 3.0+
- **sqlitegraph**: 2.0+
- **GeoGraphDB**: Optional (for spatial features)
