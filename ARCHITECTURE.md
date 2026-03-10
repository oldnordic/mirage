# Mirage Architecture

Mirage is a control-flow analysis tool that reads Magellan code graphs and provides path-aware reasoning capabilities.

## Overview

```
┌─────────────┐     ┌──────────────┐     ┌─────────────────┐
│   Mirage    │────▶│   Router     │────▶│  Magellan DB    │
│    CLI      │     │  (backend)   │     │  (.geo/.db)     │
└─────────────┘     └──────────────┘     └─────────────────┘
                            │
                            ▼
                     ┌──────────────┐
                     │   Magellan   │
                     │   Adapter    │
                     └──────────────┘
```

## Components

### 1. CLI Layer (`src/main.rs`)

Parses commands and dispatches to the appropriate backend router.

### 2. Router Layer (`src/router/`)

Backend-agnostic routing that detects database format and delegates:
- `GeometricRouter` — For `.geo` files
- `SqliteRouter` — For `.db` files
- `NativeV3Router` — For `.v3` files (reserved)

### 3. Magellan Integration (`src/integrations/magellan.rs`)

Contract-aware adapter with:
- **Path Normalization**: All paths via `normalize_path_for_query()`
- **Ambiguity Handling**: `Unique`/`Ambiguous`/`NotFound` results
- **Symbol Resolution**: ID, FQN, path+name lookup

### 4. Storage Layer (`src/storage/`)

Backend implementations:
- `GeometricStorage` — `.geo` backend
- `SqliteStorage` — SQLite backend
- `KvStorage` — Native-V3 backend (reserved)

### 5. CFG Analysis (`src/cfg/`)

Core algorithms:
- Path enumeration
- Dominator trees
- Post-dominators
- Natural loop detection
- Reachability analysis

## Backend Detection

```rust
pub fn detect(path: &Path) -> Result<BackendFormat> {
    if path.extension().and_then(|e| e.to_str()) == Some("geo") {
        return Ok(BackendFormat::Geometric);
    }
    // Check magic bytes for SQLite/NativeV3
}
```

## Symbol Resolution

```
User Input (ID/FQN/name)
    │
    ▼
MagellanAdapter::resolve_function_id()
    │
    ├─► Numeric ID? → Direct lookup
    ├─► FQN lookup?
    └─► Name search → Ambiguous/Unique/NotFound
```

## Ambiguity Handling

Mirage implements explicit ambiguity:
- **Never** silently picks first match
- Returns detailed `Ambiguous` errors with candidates
- Suggests disambiguation (FQN, path prefix)
