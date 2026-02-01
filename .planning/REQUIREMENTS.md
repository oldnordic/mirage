# Requirements: Mirage

**Defined:** 2026-02-01
**Core Value:** An agent may only speak if it can reference a graph artifact. No artifact → no output.

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

- [ ] **REACH-01**: System detects unreachable code blocks (no path from entry)
- [ ] **REACH-02**: System answers reachability queries (can node A reach node B?)
- [ ] **REACH-03**: System determines path existence between two points

### Dominance Relationships

- [ ] **DOM-01**: System computes immediate dominators for all nodes
- [ ] **DOM-02**: System computes dominator tree
- [ ] **DOM-03**: System computes post-dominators
- [ ] **DOM-04**: System computes dominance frontiers

### Control Structure Recovery

- [ ] **CTRL-01**: System detects natural loops (back-edge where head dominates tail)
- [ ] **CTRL-02**: System identifies loop header nodes
- [ ] **CTRL-03**: System recovers if/else branching patterns
- [ ] **CTRL-04**: System recovers match/expression branching patterns

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
- [ ] **DB-05**: System foreign keys cfg_blocks.function_id → graph_entities.id
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

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| All v1 requirements | TBD | Pending |

**Coverage:**
- v1 requirements: 47 total
- Mapped to phases: TBD
- Unmapped: 47 ⚠

---
*Requirements defined: 2026-02-01*
