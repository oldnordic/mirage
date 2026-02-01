# Mirage: Path-Aware Code Intelligence Engine

## What This Is

Mirage is the third tool in the Magellan ecosystem, providing **intra-procedural** code analysis through control-flow graphs and path enumeration. While Magellan tracks symbols/relationships and llmgrep searches them, Mirage answers "what paths actually exist through this function?" and "must this code always execute?".

Mirage operates on graphs stored in Magellan's database and materializes execution paths explicitly — no speculation, no full-file ingestion, just graph artifacts that LLMs can reason about.

## Core Value

**An agent may only speak if it can reference a graph artifact. No artifact → no output.**

Every query returns a path ID, CFG block, or dominance relationship that can be verified. This kills hallucination in code analysis.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] **MIR-01**: Extract MIR from rustc for accurate Rust CFG
- [ ] **AST-01**: Build CFG from Magellan's ast_nodes (multi-language support)
- [ ] **CFG-01**: Store cfg_blocks and cfg_edges in Magellan database
- [ ] **PATH-01**: Enumerate all execution paths through a function
- [ ] **PATH-02**: Classify paths (normal, error, degenerate, unreachable)
- [ ] **PATH-03**: Cache paths in cfg_paths table with BLAKE3 IDs
- [ ] **DOM-01**: Compute dominators (must-pass-through analysis)
- [ ] **DOM-02**: Compute post-dominators (must-exit-through analysis)
- [ ] **QUERY-01**: CLI command to show all paths for a function
- [ ] **QUERY-02**: CLI command to visualize CFG (human, dot, json)
- [ ] **QUERY-03**: CLI command to show dominance tree
- [ ] **DEAD-01**: Find unreachable code within functions
- [ ] **VERIFY-01**: Verify a path is still valid after code changes
- [ ] **INTEGRATION-01**: Extend Magellan database (not separate DB)

### Out of Scope

- **Search functionality** — llmgrep already handles this
- **Symbol discovery** — Magellan already handles this
- **Call graph** — Magellan already handles this
- **Inter-procedural analysis** — Out of scope for v1, focus on intra-function first
- **Multi-language MIR** — MIR is Rust-specific; use AST for other languages

## Context

**Ecosystem Position:**
```
Magellan (symbols) → llmgrep (search) → Mirage (paths)
                          ↓                    ↓
                   SQLiteGraph (shared storage)
```

**Existing Magellan Assets:**
- `ast_nodes` table (v1.9.0) with control flow kinds: `if_expression`, `while_expression`, `for_expression`, `loop_expression`, `match_expression`
- `graph_entities` for function symbols
- `graph_edges` for call relationships
- `code_chunks` for source snippets

**Key Decision - Hybrid Approach:**
- **AST → CFG** for structure (works for all tree-sitter languages)
- **MIR → CFG** for Rust-specific accuracy (types, borrow checker info)

**Documentation:**
- Comprehensive design docs exist in `docs/` folder
- ROADMAP.md defines 6 milestones (M0-M6)
- M0 (Foundation) is complete

## Constraints

- **Must extend Magellan database** — No separate storage; same DB enables atomic updates
- **Must reference graph artifacts** — No speculation, every output must have an ID/proof
- **Must follow Magellan CLI patterns** — `--db`, `--output human/json/pretty`, exit codes
- **Path explosion handling** — Functions with loops can have exponential paths; need limits/pruning
- **Incremental updates** — When code changes, only re-index affected functions

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| **Hybrid AST + MIR** | AST for multi-language structure, MIR for Rust accuracy | — Pending |
| **Same DB as Magellan** | Atomic updates, JOIN queries, single source of truth | ✓ Good |
| **BLAKE3 for path IDs** | Deterministic, fast, fits ecosystem pattern | ✓ Good |
| **Follow Magellan CLI patterns** | Consistency across toolset | ✓ Good |
| **No MIR extraction dependency for v1** | Start with AST → CFG, add MIR later | — Pending |

---
*Last updated: 2026-02-01 after initialization*
