# Research Summary - Mirage

**Synthesized:** 2026-02-01

## Recommended Stack

### MIR Extraction
**Primary: Charon** (alpha-stage, purpose-built for Rust MIR extraction)
- Outputs structured JSON + `.llbc` files
- Handles complete crate including dependencies
- Provides source location information for AST-MIR mapping
- Rust library (`charon-lib`) and OCaml bindings available

**Fallback:** rustc-driver API (nightly only, unstable but full access)
- Use for features Charon doesn't support
- Revisit when StableMIR publishes (currently marked "Incomplete" for 2025 H1)

### Graph Foundation
**petgraph 0.8.3** - Industry standard, 235M+ downloads
- Directed graphs for CFG representation
- Rich algorithm library (DFS, BFS, shortest paths)
- DOT/Graphviz export for visualization

### Dominance Analysis
**domtree 0.1.0** - Clean Lengauer-Tarjan implementation with path compression
- Zero dependencies, focused API
- Consider Cranelift's dominator tree if more features needed

### Path Enumeration
**Custom implementation** using petgraph + DFS backtracking
- No suitable off-the-shelf library exists
- Must implement: loop bounding, cycle detection, path caching
- Use multi-solver support (Z3, CVC5) for feasibility checking

### Hybrid Architecture
- **AST Layer** (Magellan/tree-sitter): Function boundaries, statement structure, source locations
- **MIR Layer** (Charon): Precise CFG, borrow checker info, control flow semantics
- **Integration:** Map AST nodes to MIR basic blocks using source location info

---

## Table Stakes Features

Users will abandon a CFG analysis tool without these features. These are non-negotiable expectations for any tool claiming to do control flow analysis.

### Core CFG Construction
- Basic Block Identification - Partition code into maximal straight-line sequences
- Edge Discovery - Identify all control flow transitions (conditional, unconditional, exceptional)
- Entry/Exit Node Detection - Identify unique entry and exit nodes for reachability
- Graph Serialization - Export CFG in standard formats (DOT, GraphML, JSON)

### Reachability Analysis
- Unreachable Code Detection - Identify blocks with no path from entry
- Reachability Queries - Answer "can node A reach node B?" for any pair
- Path Existence - Determine if valid execution path exists between two points

### Dominance Relationships
- Dominance Computation - Calculate immediate dominators and dominator tree
- Post-Dominance Computation - Calculate post-dominators and post-dominator tree
- Dominance Frontier - Identify nodes where dominance relationships meet (SSA prerequisite)

### Control Structure Recovery
- Natural Loop Detection - Identify back-edges where head dominates tail
- Loop Header Identification - Determine which nodes are loop entry points
- If/Else Recovery - Identify conditional branching patterns
- Switch/Match Recovery - Identify multi-way branching structures

### Source Mapping
- AST-to-CFG Correspondence - Map CFG nodes back to source code location
- Node-to-Statement Mapping - Associate basic blocks with source statements
- Location-Preserving Artifacts - Graph exports maintain source location metadata

---

## Architecture Overview

### Pipeline Architecture

```
Source Code -> Frontend -> IR (MIR/AST) -> CFG Builder -> Path Enum / Dominance -> Queries
```

### Components (Build Order)

1. **Frontend** (Phase 1): MIR/AST extraction using Charon + tree-sitter
2. **CFG Builder** (Phase 2): IR-to-graph transformation with edge classification
3. **Path Enumerator** (Phase 3): DFS-based enumeration with loop bounding and classification
4. **Dominance Analyzer** (Phase 4): Cooper-Harvey-Kennedy algorithm for must-pass-through proofs
5. **Query Layer** (Phase 5): User-facing interface for all analysis types
6. **Verification** (Phase 6): Incremental update pipeline for cached artifacts

### Database Integration

Mirage extends the existing Magellan/SQLiteGraph database:

```
Magellan Core                    Mirage Extensions
graph_entities (id)      -->     cfg_blocks.function_id
graph_edges (from_id)    -->     cfg_edges
code_chunks              -->     cfg_paths, cfg_dominators
```

### Edge Types
- TrueBranch / FalseBranch - Conditional outcomes
- Fallthrough - Sequential execution
- LoopBack / LoopExit - Loop entry and exit
- Exception - Panic/throw (if detected)
- Call / Return - Function boundaries

---

## Key Pitfalls to Avoid

### 1. Path Explosion (Critical)
**Pitfall:** Unbounded loop unrolling or exhaustive path enumeration causes exponential explosion.

**Prevention:**
- Implement loop bounding (default 2-4 iterations like Clang)
- Use abstract interpretation for loop bodies (summarize effects)
- Set maximum path depth and total path count limits
- Never promise "complete path coverage"

### 2. Constraint Solver Bottleneck
**Pitfall:** SMT solving consumes 70-90% of execution time.

**Prevention:**
- Support multiple solvers (Z3, CVC5) for different constraint types
- Implement efficient encoding of symbolic expressions
- Cache solver results for repeated constraints
- Use lightweight feasibility checks before full solving

### 3. No State Merging
**Pitfall:** Exploring paths independently causes redundant analysis of overlapping prefixes.

**Prevention:**
- Implement static state merging at CFG join points
- Accept some precision loss for significant performance gains
- Traverse CFG in topological order

### 4. Tight Coupling to Magellan Schema
**Pitfall:** Assuming existing schema can accommodate CFG data without modification.

**Prevention:**
- Design separate CFG schema that integrates with Magellan
- Use foreign key references to existing graph_entities
- Prototype integration before committing to schema

### 5. Ignoring Async/Await Desugaring
**Pitfall:** Treating async/await as simple linear control flow is incorrect.

**Prevention:**
- Work at MIR level where async is already desugared to state machines
- Model await points as potential yield/resume
- Consider cancellation as implicit exit path

### 6. Whole-Program Re-analysis
**Pitfall:** Re-analyzing entire codebase for single-line changes causes unacceptable latency.

**Prevention:**
- Implement dependency tracking between analysis units
- Use reified computational dependencies for incremental updates
- Design for "analyze once, update results" workflow

### 7. Feasible vs Infeasible Path Confusion
**Pitfall:** Counting paths that can never execute due to conflicting constraints.

**Prevention:**
- Integrate with constraint solver (SMT) for path feasibility pruning
- Accept performance overhead to avoid wasting computation on infeasible paths
- Implement lightweight checks before full constraint solving

---

## Implications for Roadmap

### Phase Structure

The research confirms a **6-phase milestone structure** aligned with component dependencies:

| Phase | Component | Duration | Key Deliverable |
|-------|-----------|----------|-----------------|
| 1 | Foundation | 2 weeks | Schema + MIR extraction prototype |
| 2 | CFG Builder | 2 weeks | Basic block graphs with edge classification |
| 3 | Path Enumerator | 3 weeks | Enumerated paths with error classification |
| 4 | Dominance Analyzer | 2 weeks | Must-pass-through proofs |
| 5 | Advanced Analysis | 3 weeks | Dead code, blast zone, verification |
| 6 | Integration | 2 weeks | llmgrep integration, incremental updates |

### Parallelization Opportunities

- Phase 4 (Dominance) can run parallel with Phase 3 (Paths) - both depend only on Phase 2
- Phase 6 (Integration) can start once Phase 5 is partially complete

### Priority Order

1. **PathLite first:** Build function-level CFG and path enumeration before statement-level analysis
2. **Loop bounding from day one:** Don't attempt exhaustive enumeration
3. **Incremental by design:** Database schema must support change tracking from start
4. **MIR over AST for Rust:** Use Charon for precision, fall back to AST only when needed

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Charon API breakage | Fallback to rustc-driver |
| Path explosion | Hard limits on depth and count |
| Solver bottleneck | Multi-solver architecture |
| Incremental updates | Schema designed for versioning |

### Technical Debt Strategy

- Document Rust-specific limitations explicitly (dynamic dispatch, complex async)
- Leverage existing research (state merging, SSA construction) rather than re-inventing
- Study Polonius (next-gen borrow checker) for similar architectural challenges

---

## Sources

This synthesis draws from:
- STACK.md: Library evaluations, version compatibility, tool selection
- FEATURES.md: Table stakes, differentiators, anti-features by complexity
- ARCHITECTURE.md: Pipeline components, interface contracts, build order
- PITFALLS.md: Common mistakes with warning signs and prevention strategies
