# Mirage Architecture

Mirage is a control-flow analysis tool that reads Magellan code graphs and provides path-aware reasoning capabilities.

## Overview

```
┌─────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   Mirage    │────▶│   Backend        │────▶│  Magellan DB    │
│    CLI      │     │  (auto-detect)   │     │  (.geo/.db)     │
└─────────────┘     └──────────────────┘     └─────────────────┘
```

## Components

### 1. CLI Layer (`src/cli/`)

Parses commands and dispatches to the appropriate analysis module. Modular: one file per command in `src/cli/cmds/`.

### 2. Storage Layer (`src/storage/`)

Backend detection and data access:
- `Backend` — Enum with auto-detection (`Sqlite` / `Geometric`)
- `StorageTrait` — Backend-agnostic storage interface
- `SqliteStorage` — SQLite backend (default)
- `GeometricStorage` — `.geo` backend (feature-gated)
- `MirageDb` — Legacy wrapper around `Backend`

### 3. Magellan Integration (`src/integrations/magellan/`)

Contract-aware adapter:
- **Path Normalization**: All paths via `normalize_path_for_query()`
- **Ambiguity Handling**: `Unique`/`Ambiguous`/`NotFound` results
- **Symbol Resolution**: ID, FQN, path+name lookup

### 4. CFG Analysis (`src/cfg/`)

Core algorithms:
- Path enumeration (recursive, iterative, cached)
- Dominator trees (Lengauer-Tarjan)
- Post-dominators
- Natural loop detection
- Reachability analysis
- CFG diff (cross-database comparison)

### 5. Analysis Layer (`src/analysis/`)

Inter-procedural and intra-procedural analysis:
- `MagellanBridge` — wraps Magellan's CodeGraph for call graph algorithms
- `risk` — composite risk scoring (cyclomatic complexity, path count, error ratio, nesting)
- `suggest` — refactoring suggestions (split-function, flatten-nesting, extract-method, remove-dead-code)
- `stats` — aggregate code statistics (function counts, complexity distribution, dead code)
- `telemetry` — opt-in local telemetry (`--record` or `MIRAGE_TELEMETRY=1`)

## Data Flow

```
Magellan indexes source → writes .db (cfg_blocks, graph_entities, graph_edges)
     │
     ▼
Mirage opens .db → load_cfg_from_db() → petgraph DiGraph<BasicBlock, EdgeType>
     │
     ▼
cfg:: / analysis:: modules compute in memory → CLI outputs results
```

## Symbol Resolution

```
User Input (ID/FQN/name)
    │
    ▼
StorageTrait / MagellanAdapter
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
