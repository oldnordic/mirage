# Ecosystem Context

This document captures the current state of the Magellan toolset that Mirage will integrate with.

---

## The Magellan Toolset

```
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│  Magellan   │ ───► │  llmgrep    │ ───► │   Mirage    │
│ (Indexer)   │      │  (Search)   │      │ (Paths)     │
└─────────────┘      └─────────────┘      └─────────────┘
       │                    │                     │
       └────────────────────┴─────────────────────┘
                     │
              ┌──────▼──────┐
              │ SQLiteGraph │
              │ (Storage)   │
              └─────────────┘
```

### Tool Responsibilities

| Tool | Scope | Storage | Purpose |
|------|-------|---------|---------|
| **Magellan** | Inter-procedural (symbols) | `graph_entities`, `graph_edges` | Code graph navigation, symbol discovery |
| **llmgrep** | Query over Magellan | Read-only CLI | Semantic code search with metrics |
| **Mirage** | Intra-procedural (CFG) | `cfg_blocks`, `cfg_edges`, `cfg_paths` | Path enumeration, verification |
| **SQLiteGraph** | Persistence layer | Dual backend | Graph storage, ACID transactions |

---

## SQLiteGraph: Current State (v1.2.7)

### Features

- **Dual Backend Architecture**: SQLite or Native V2 (clustered adjacency)
- **ACID Transactions**: Atomicity, Consistency, Isolation, Durability
- **MVCC Snapshots**: Read isolation with snapshot views
- **Pub/Sub Events**: In-process event notification (Native V2 only)
- **HNSW Vector Search**: Hierarchical Navigable Small World for ANN
- **Graph Algorithms**: PageRank, Betweenness Centrality, Label Propagation, Louvain

### Backend Selection

| Use Case | Recommended Backend |
|----------|-------------------|
| Write-Heavy Workloads | Native V2 (1.3-3.2x faster inserts) |
| Star-Pattern Graphs | Native V2 (clustered adjacency) |
| Chain-Depth Traversals | SQLite (V2 has 2-10x regression) |
| Enterprise Applications | SQLite (tooling ecosystem) |
| Vector Search | Native V2 (HNSW integration) |

### Key Schema

```sql
-- Entities: Symbols, files, code chunks
graph_entities(id, kind, name, file_path, data)

-- Edges: Relationships between entities
graph_edges(from_id, to_id, edge_type)

-- Labels: For filtering by language, kind
graph_labels(entity_id, label)

-- Code chunks: Pre-stored snippets
code_chunks(file_path, byte_start, byte_end, content, content_hash)

-- Metrics: File and symbol level complexity
file_metrics(file_path, symbol_count, loc, fan_in, fan_out, complexity_score)
symbol_metrics(symbol_id, loc, fan_in, fan_out, cyclomatic_complexity)
```

### API Patterns

```rust
// SQLite Backend
use sqlitegraph::{SqliteGraph, GraphEntity};
let graph = SqliteGraph::open_in_memory()?;

// Native V2 Backend
use sqlitegraph::{GraphConfig, open_graph, NodeSpec};
let cfg = GraphConfig::native();
let graph = open_graph("graph.db", &cfg)?;
```

---

## Magellan: Current State (v1.8.0)

### Philosophy

> **"Text is more tokens. Facts are answers."**

Magellan exists to give LLMs **answers, not search results**.

### Capabilities

| Feature | Command | Output |
|---------|---------|--------|
| Find symbols | `find --name` | Symbol locations |
| Find references | `refs --direction in/out` | Call sites |
| List files | `files --symbols` | Files with symbol counts |
| Query by label | `label --list` | Language/kind filtering |
| Get source code | `get` | Symbol content |

### Database Schema

```sql
graph_entities         -- Nodes (symbols with file_path, kind, data, symbol_id)
graph_edges            -- Edges (from_id, to_id, edge_type)
graph_labels           -- Labels (entity_id, label)
graph_properties       -- Properties (entity_id, key, value)
code_chunks            -- Pre-stored code snippets
file_metrics           -- File-level metrics
symbol_metrics        -- Symbol-level metrics
```

### Edge Types

- `CALLER` - Function calls another function
- `CALLS` - Function calls another
- `DEFINES` - Symbol defines another
- `REFERENCES` - General references

### Labels

**Languages**: `rust`, `python`, `javascript`, `typescript`, `c`, `cpp`, `java`, ...

**Kinds**: `fn`, `method`, `struct`, `class`, `enum`, `interface`, `module`, ...

### Symbol ID Format

```
symbol_id = SHA256(language + ":" + fqn + ":" + span_id)[0:16]
```

16 hex characters (64 bits) for stable identification.

---

## llmgrep: Current State (v1.1.0)

### Purpose

Read-only query engine over the Magellan code map with deterministic, schema-aligned output.

### Key Flags

| Flag | Description |
|------|-------------|
| `--symbol-id <ID>` | Search by BLAKE3 SymbolId |
| `--fqn <PATTERN>` | Filter by FQN pattern |
| `--exact-fqn <FQN>` | Exact FQN match |
| `--min-complexity <N>` | Minimum complexity score |
| `--max-complexity <N>` | Maximum complexity score |
| `--min-fan-in <N>` | Minimum incoming references |
| `--min-fan-out <N>` | Minimum outgoing calls |
| `--language <LANG>` | Filter by language |
| `--kind <KIND>` | Filter by symbol kind |
| `--sort-by complexity` | Sort by cyclomatic_complexity |
| `--sort-by fan-in` | Sort by incoming references |
| `--sort-by fan-out` | Sort by outgoing calls |

### JSON Output Schema

```json
{
  "name": "symbol_name",
  "kind": "fn",
  "kind_normalized": "fn",
  "language": "rust",
  "symbol_id": "abc123...",
  "canonical_fqn": "project::src/lib.rs::fn symbol_name",
  "display_fqn": "project::lib::symbol_name",
  "metrics": {
    "fan_in": 5,
    "fan_out": 3,
    "cyclomatic_complexity": 1
  },
  "content_hash": "sha256hash...",
  "span": {
    "file_path": "src/lib.rs",
    "byte_start": 100,
    "byte_end": 200,
    "start_line": 10,
    "end_line": 20
  },
  "snippet": "source code here..."
}
```

### Modes

- `--mode symbols` - Find symbols by name/pattern
- `--mode references` - Find references to symbols
- `--mode calls` - Find call relationships

### Available Scripts

1. **llmgrep-workflow.sh** - Main workflow: search, refs, calls, check-wire, hotspots
2. **call-chain.sh** - Forward/backward call analysis
3. **blast-zone.sh** - Impact analysis

---

## Integration Strategy for Mirage

### 1. Shared Database

Mirage will extend the existing Magellan database with new tables:

```sql
-- Mirage tables (extending Magellan schema)
cfg_blocks(...)
cfg_edges(...)
cfg_paths(...)
cfg_dominators(...)
```

### 2. Symbol Linkage

Mirage `function_id` will reference Magellan `symbol_id`:

```sql
cfg_blocks.function_id → graph_entities.id (WHERE kind = 'Function')
```

### 3. Query Integration

```bash
# Use Magellan to find function
mirage paths --function "$(llmgrep search --query 'parse' --output json | jq -r '.[0].symbol_id')"

# Use Mirage paths for impact analysis
mirage blast-zone --symbol SYMBOL_ID --max-depth 3
```

### 4. Verification Workflow

1. **Magellan**: Confirms symbol exists, has valid span
2. **Mirage**: Enumerates paths, verifies dominance
3. **llmgrep**: Confirms all references in paths are real

---

## Conventions to Follow

### ID Formats

All tools use 16 hex characters (64 bits) for stable IDs:

```rust
// Symbol ID
symbol_id = SHA256(language + ":" + fqn + ":" + span_id)[0:16]

// Path ID (Mirage)
path_id = BLAKE3(function_id + ":" + path_signature)[0:16]
```

### Span Conventions

- **byte_start**: UTF-8 byte offset (inclusive)
- **byte_end**: UTF-8 byte offset (exclusive)
- **start_line**: 1-indexed
- **start_col**: 0-indexed
- **end_line**: 1-indexed
- **end_col**: 0-indexed (exclusive)

### FQN Variants

| Field | Format | Purpose |
|-------|--------|---------|
| `canonical_fqn` | `{crate}::{file_path}::{kind} {name}` | Unambiguous identity |
| `display_fqn` | `{crate}::{module}::{name}` | Human-readable |

---

## Existing Research

Note: Mirage previously considered using Charon for MIR extraction but decided against it due to:
- External binary dependency
- Nightly Rust requirement
- Research project status (not production-ready)

**Current direction:** Use AST-based CFG from Magellan for all languages.

For details, see: LIMITATIONS_AND_ROADMAP.md

---

*Last updated: 2026-01-31*
