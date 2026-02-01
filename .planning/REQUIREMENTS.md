# Requirements: Mirage

**Defined:** 2026-02-01
**Core Value:** An agent may only speak if it can reference a graph artifact. No artifact -> no output.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### CFG Construction

- [ ] **CFG-01**: System identifies basic blocks in functions (maximal straight-line sequences)
- [ ] **CFG-02**: System discovers all control flow edges between blocks (conditional, unconditional, exceptional)
- [ ] **CFG-03**: System detects entry and exit nodes for each function
- [ ] **CFG-04**: System exports CFG in DOT format for visualization
- [ ] **CFG-05**: System exports CFG in JSON format for tool integration
- [ ] **CFG-06**: System maps CFG nodes back to source locations (file, line, column)

### Reachability Analysis

- [x] **REACH-01**: System detects unreachable code blocks (no path from entry)
- [x] **REACH-02**: System answers reachability queries (can node A reach node B?)
- [x] **REACH-03**: System determines path existence between two points

### Dominance Relationships

- [x] **DOM-01**: System computes immediate dominators for all nodes
- [x] **DOM-02**: System computes dominator tree
- [x] **DOM-03**: System computes post-dominators
- [x] **DOM-04**: System computes dominance frontiers

### Control Structure Recovery

- [x] **CTRL-01**: System detects natural loops (back-edge where head dominates tail)
- [x] **CTRL-02**: System identifies loop header nodes
- [x] **CTRL-03**: System recovers if/else branching patterns
- [x] **CTRL-04**: System recovers match/expression branching patterns

### Path Enumeration

- [ ] **PATH-01**: System enumerates all feasible execution paths through a function
- [ ] **PATH-02**: System classifies paths (normal, error, degenerate, unreachable)
- [ ] **PATH-03**: System implements path length bounding to prevent explosion
- [ ] **PATH-04**: System distinguishes feasible from infeasible paths
- [ ] **PATH-05**: System caches enumerated paths with BLAKE3 IDs
- [ ] **PATH-06**: System assigns unique path ID for each enumerated path

### MIR Integration

- [ ] **MIR-01**: System extracts MIR from rustc using Charon
- [ ] **MIR-02**: System builds CFG from MIR (Rust-specific, accurate)
- [ ] **MIR-03**: System falls back to AST-based CFG for non-Rust code

### Database Integration

- [ ] **DB-01**: System extends Magellan database with cfg_blocks table
- [ ] **DB-02**: System extends Magellan database with cfg_edges table
- [ ] **DB-03**: System extends Magellan database with cfg_paths table
- [ ] **DB-04**: System extends Magellan database with cfg_dominators table
- [ ] **DB-05**: System foreign keys cfg_blocks.function_id -> graph_entities.id
- [ ] **DB-06**: System supports incremental updates on code change

### CLI Interface

- [ ] **CLI-01**: `mirage paths --function SYMBOL` shows all paths for function
- [ ] **CLI-02**: `mirage paths --show-errors` shows only error paths
- [ ] **CLI-03**: `mirage paths --max-length N` bounds path exploration
- [ ] **CLI-04**: `mirage cfg --function SYMBOL` shows human-readable CFG
- [ ] **CLI-05**: `mirage cfg --format dot` exports Graphviz DOT
- [ ] **CLI-06**: `mirage cfg --format json` exports JSON
- [ ] **CLI-07**: `mirage dominators --function SYMBOL` shows dominance tree
- [ ] **CLI-08**: `mirage dominators --must-pass-through BLOCK` proves mandatory execution
- [ ] **CLI-09**: `mirage unreachable` finds unreachable code blocks
- [ ] **CLI-10**: `mirage verify --path-id ID` verifies path still valid
- [ ] **CLI-11**: `mirage status` shows database statistics
- [ ] **CLI-12**: All commands support `--output json|pretty|human`

### LLM Integration

- [ ] **LLM-01**: Path queries return structured JSON with path IDs
- [ ] **LLM-02**: Path results include block sequence and source locations
- [ ] **LLM-03**: Error responses include remediation suggestions
- [ ] **LLM-04**: System provides natural language summaries of control flow

### Performance

- [ ] **PERF-01**: CFG construction completes in O(n) time where n is code size
- [ ] **PERF-02**: Path enumeration respects configurable depth and count limits
- [ ] **PERF-03**: Database updates are incremental (function-level granularity)

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Interprocedural Analysis

- **INTER-01**: System builds interprocedural CFG across function boundaries
- **INTER-02**: System maintains call-sensitive CFG contexts
- **INTER-03**: System performs context-sensitive analysis

### Advanced Visualizations

- **VIZ-01**: Interactive graph visualization in web UI
- **VIZ-02**: Path highlighting in source code view
- **VIZ-03**: CFG animation for execution paths

### Advanced Rust Features

- **RUST-01**: Borrow checker integration via Polonius
- **RUST-02**: Async/await state machine analysis
- **RUST-03**: Closure capture analysis
- **RUST-04**: Trait resolution tracking

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Full SMT theorem proving | Excessive complexity, use lightweight feasibility checks instead |
| Exhaustive path enumeration without bounds | Path explosion makes this intractable |
| Custom IR design | Use existing MIR from rustc instead |
| Separate database storage | Must extend Magellan database for atomic updates |
| Multi-language MIR | MIR is Rust-specific; use AST for other languages |
| Full abstract interpretation | Over-engineering for v1, use bounded analysis instead |

## Traceability

Which phases cover which requirements.

| Requirement | Phase | Status |
|-------------|-------|--------|
| CFG-01 | Phase 2 | Pending |
| CFG-02 | Phase 2 | Pending |
| CFG-03 | Phase 2 | Pending |
| CFG-04 | Phase 2 | Pending |
| CFG-05 | Phase 2 | Pending |
| CFG-06 | Phase 2 | Pending |
| REACH-01 | Phase 3 | Complete |
| REACH-02 | Phase 3 | Complete |
| REACH-03 | Phase 3 | Complete |
| DOM-01 | Phase 4 | Complete |
| DOM-02 | Phase 4 | Complete |
| DOM-03 | Phase 4 | Complete |
| DOM-04 | Phase 4 | Complete |
| CTRL-01 | Phase 3 | Complete |
| CTRL-02 | Phase 3 | Complete |
| CTRL-03 | Phase 3 | Complete |
| CTRL-04 | Phase 3 | Complete |
| PATH-01 | Phase 5 | Pending |
| PATH-02 | Phase 5 | Pending |
| PATH-03 | Phase 5 | Pending |
| PATH-04 | Phase 5 | Pending |
| PATH-05 | Phase 5 | Pending |
| PATH-06 | Phase 5 | Pending |
| MIR-01 | Phase 2 | Pending |
| MIR-02 | Phase 2 | Pending |
| MIR-03 | Phase 2 | Pending |
| DB-01 | Phase 1 | Complete |
| DB-02 | Phase 1 | Complete |
| DB-03 | Phase 1 | Complete |
| DB-04 | Phase 1 | Complete |
| DB-05 | Phase 1 | Complete |
| DB-06 | Phase 1 | Complete |
| CLI-01 | Phase 6 | Pending |
| CLI-02 | Phase 6 | Pending |
| CLI-03 | Phase 6 | Pending |
| CLI-04 | Phase 6 | Pending |
| CLI-05 | Phase 6 | Pending |
| CLI-06 | Phase 6 | Pending |
| CLI-07 | Phase 6 | Pending |
| CLI-08 | Phase 6 | Pending |
| CLI-09 | Phase 6 | Pending |
| CLI-10 | Phase 6 | Pending |
| CLI-11 | Phase 6 | Pending |
| CLI-12 | Phase 6 | Pending |
| LLM-01 | Phase 7 | Pending |
| LLM-02 | Phase 7 | Pending |
| LLM-03 | Phase 7 | Pending |
| LLM-04 | Phase 7 | Pending |
| PERF-01 | Phase 5 | Pending |
| PERF-02 | Phase 5 | Pending |
| PERF-03 | Phase 5 | Pending |

**Coverage:**
- v1 requirements: 51 total
- Mapped to phases: 51/51 (100%)
- Unmapped: 0

---
*Requirements defined: 2026-02-01*
*Roadmap created: 2026-02-01*
