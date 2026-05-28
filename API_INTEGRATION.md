# Mirage API Integration with Magellan

## Backend Opening

```rust
use mirage_analyzer::Backend;

let backend = Backend::detect_and_open(db_path)?;
```

**Rules:**
- Auto-detects `.db` (SQLite) or `.geo` (geometric) from file extension
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
- `k_hop_callees(symbol_id, depth)` — Callees within N call hops (via `SymbolNavigator`, magellan 4.2.0+)
- `k_hop_callers(symbol_id, depth)` — Callers within N call hops (via `SymbolNavigator`, magellan 4.2.0+)

## Blast-Zone JSON Response

When using `--call-depth N` with `--use-call-graph`, the JSON response includes a `depth` field per `CallGraphSymbol`:

```json
{
  "function": "index_file",
  "block_id": 0,
  "affected_symbols": [
    { "name": "helper_a", "depth": 1 },
    { "name": "helper_b", "depth": 2 },
    { "name": "process", "depth": 3 }
  ]
}
```

**Parameters:**
- `--call-depth <N>` — Limits inter-procedural traversal to N call hops. Default: 0 (unlimited reachability).

## Graph Memory Operations (Schema 13+)

- `list_source_documents()` — All source documents from `source_documents` table
- `get_documents_for_function(function_id)` — Documents referencing a function
- `get_facts_for_function(function_id)` — Candidate facts about a function

All methods degrade gracefully: return empty `Vec` when tables are missing.

## Version Requirements

- **Magellan**: 4.2.0+ via local path patch (schema v17; `SymbolNavigator` for depth-bounded analysis). Minimum schema: 7.
- **sqlitegraph**: 3.0+
- **GeoGraphDB**: Optional (for spatial features)
