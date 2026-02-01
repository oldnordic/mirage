# Stack Research - Rust CFG & Path Enumeration

**Researched:** 2026-02-01

## MIR Extraction Options

### Option 1: Charon
**Version:** No stable release yet (alpha software, active development as of 2025)
**Repository:** [AeneasVerif/charon](https://github.com/AeneasVerif/charon)
**Paper:** [Charon: An Analysis Framework for Rust](https://arxiv.org/html/2410.18042v2)

**Pros:**
- Purpose-built for extracting Rust crate contents into analyzable format (JSON + `.llbc` files)
- Handles complete crate including dependencies
- Simplified MIR bodies with source location information
- Rust library (`charon-lib`) for parsing and manipulation
- OCaml bindings available (`charon-ml`)
- Actively developed by Aeneas verification project

**Cons:**
- Alpha software - API has breaking changes planned
- Does not support all Rust edge cases correctly
- Single-purpose extraction tool, not a general-purpose CFG library
- JSON-based format may require additional parsing for graph operations

**Confidence:** High (for research/analysis focus), Medium (for production stability)

---

### Option 2: rustc-driver API (Direct)
**Version:** Tied to rustc version (nightly only)
**Docs:** [rustc-dev-guide](https://rustc-dev-guide.rust-lang.org/rustc-driver/intro.html)

**Pros:**
- Full access to MIR at any compilation stage
- Can run custom compiler passes
- Access to complete compiler internals (HIR, MIR, THIR, etc.)
- Maximum flexibility for analysis

**Cons:**
- Unstable API - breaks between compiler versions
- Requires nightly toolchain
- Heavy dependency on rustc internals
- Complex setup and maintenance burden
- Not published on crates.io

**Confidence:** Low (for stable tooling), High (for compiler research)

---

### Option 3: StableMIR (Future - Not Ready)
**Version:** Not yet published on crates.io (2025 H1 project goal - **Incomplete**)
**Docs:** [Rust Project Goals 2025](https://rust-lang.github.io/rust-project-goals/2025h1/stable-mir.html)

**Pros:**
- Stable interface to rustc MIR
- Published to crates.io for easy dependency management
- Shielded from internal compiler changes
- Official Rust project goal

**Cons:**
- **NOT YET AVAILABLE** - marked "Incomplete" as of late 2025
- Cannot be used for production tools in 2026 Q1
- API not finalized

**Confidence:** Low (availability), High (future potential)

---

### Option 4: rustc -Z dump-mir (CLI)
**Version:** Available in nightly rustc
**Docs:** [MIR Debugging](https://rustc-dev-guide.rust-lang.org/mir/debugging.html)

**Pros:**
- Simple CLI invocation, no code required
- Outputs human-readable MIR
- Can target specific passes with flags

**Cons:**
- Text output requires parsing
- Not programmatic
- Requires nightly toolchain
- No structured data output

**Confidence:** Medium (for debugging), Low (for tool integration)

---

### Option 5: Miri (MIR Interpreter)
**Version:** Actively developed, [rust-lang/miri](https://github.com/rust-lang/miri)
**Paper:** [Miri: Practical Undefined Behavior Detection for Rust](https://dl.acm.org/doi/pdf/10.1145/3776690)

**Pros:**
- Production-quality MIR interpreter
- Integrates with cargo (`cargo miri`)
- Excellent for runtime verification
- Memory safety checking

**Cons:**
- Focused on interpretation, not CFG extraction
- Not designed for static analysis
- Heavy runtime overhead

**Confidence:** High (for UB detection), Low (for CFG construction)

---

## CFG Construction

### Libraries

#### petgraph
**Version:** 0.8.3 (September 2025)
**Repository:** [petgraph/petgraph](https://github.com/petgraph/petgraph)
**Crates.io:** [petgraph](https://crates.io/crates/petgraph)

**Pros:**
- De facto standard graph library in Rust (235M+ all-time downloads)
- Supports directed/undirected graphs
- DOT/Graphviz export for visualization
- Rich algorithm library (DFS, BFS, shortest paths, etc.)
- Actively maintained

**Cons:**
- General-purpose, not CFG-specific
- No built-in dominator analysis
- No path enumeration utilities

**Confidence:** High - Use as foundational graph data structure

---

#### spirt
**Version:** 0.3.0+
**Repository:** Part of [rust-gpu](https://github.com/embarkstudios/rust-gpu)
**Docs:** [ControlFlowGraph docs](https://docs.rs/spirt/latest/spirt/cfg/struct.ControlFlowGraph.html)

**Pros:**
- Purpose-built `ControlFlowGraph` struct
- Designed for compiler IR (SPIR-V derived)
- Handles control regions and instructions
- SSA-aware

**Cons:**
- Research project for shader compilation
- Not Rust-MIR-specific
- Limited documentation for general CFG use

**Confidence:** Medium (for reference), Low (as primary dependency)

---

#### cpg-rs
**Repository:** [gbrigandi/cpg-rs](https://github.com/gbrigandi/cpg-rs)
**Crates.io:** [cpg-rs](https://crates.io/crates/cpg-rs)

**Pros:**
- Code Property Graph includes CFG + PDG (Program Dependence Graph)
- Designed for static analysis and vulnerability detection
- Complete data structures for CPG representation

**Cons:**
- Limited documentation
- May be abandoned or minimally maintained
- Not specialized for Rust/MIR

**Confidence:** Low

---

#### cfg-traits
**Version:** 0.2.3 (July 2025)
**Crates.io:** [cfg-traits](https://crates.io/crates/cfg-traits)

**Pros:**
- Abstractions for working with CFGs
- Trait-based design for flexibility

**Cons:**
- Minimal documentation
- Very small download count
- Trait-only, no concrete implementations

**Confidence:** Low

---

## Dominance Analysis

### Libraries

#### domtree
**Version:** 0.1.0
**Crates.io:** [domtree](https://crates.io/crates/domtree)

**Pros:**
- Generic implementation of dominator tree calculation
- Based on **Lengauer-Tarjan algorithm** with path compression
- Zero dependencies
- Clean, focused API

**Cons:**
- Early version (0.1.0)
- Minimal documentation
- No post-dominator support explicitly mentioned

**Confidence:** Medium - Best available dominator tree crate

---

#### cranelift_codegen::dominator_tree
**Docs:** [cranelift_codegen](https://docs.rs/cranelift-codegen/latest/craneliff_codegen/dominator_tree/index.html)

**Pros:**
- Production-quality code (used in Cranelift)
- Implements Semi-NCA algorithm
- Battle-tested

**Cons:**
- Part of large Cranelift dependency
- Internal API, not standalone
- Tied to Cranelift's IR

**Confidence:** High (quality), Medium (as dependency)

---

#### rustc_internal::dominators
**Module:** `rustc_data_structures::graph::dominators`
**Docs:** [nightly-rustc docs](https://doc.rust-lang.org/beta/nightly-rustc/rustc_data_structures/graph/dominators/index.html)

**Pros:**
- Used by rustc itself
- Optimized for MIR/CFG analysis

**Cons:**
- Nightly-only internal API
- Not usable as stable dependency

**Confidence:** High (algorithm), Low (usability)

---

## Path Enumeration

### Algorithms

#### DFS with Backtracking
**Reference:** [Stack Overflow: Find all paths in a graph with DFS](https://stackoverflow.com/questions/9803143/find-all-paths-in-a-graph-with-dfs)

**Pros:**
- Simple to implement
- Works with any directed graph
- Well-documented algorithm

**Cons:**
- Exponential path count in worst case
- Must handle cycles (visited set per path, not global)
- Can be infinite for graphs with loops (need loop bounds)

**Confidence:** High - Standard approach, must implement ourselves

---

#### Loop-Aware Path Enumeration
**Reference:** [IIT Madras - Program Analysis](https://www.cse.iitm.ac.in/~rupesh/teaching/pa/jan17/cribes/1.pdf)

**Key Insight:** "As we cannot enumerate all the possible paths, we have to make a conservative approximation"

**Pros:**
- Handles loops with bounded unrolling
- Practical for static analysis

**Cons:**
- No standard library
- Must design approximation strategy

**Confidence:** High - Need custom implementation

---

### Available Rust Implementations

#### graph-algorithms-rs
**Repository:** [slavik-pastushenko/graph-algorithms-rs](https://github.com/slavik-pastushenko/graph-algorithms-rs)

**Pros:**
- DFS implementation included
- Rust-native

**Cons:**
- Not specifically for path enumeration
- May not handle CFG-specific concerns

**Confidence:** Medium - Reference only

---

## Recommendations

### Primary Stack

**MIR Extraction:** **Charon** (with fallback to rustc-driver)
- Rationale: Charon is designed specifically for this use case, outputs structured data, and has both Rust and OCaml libraries
- Fallback: rustc-driver for features Charon doesn't support
- Note: Revisit when StableMIR publishes to crates.io

**Graph Foundation:** **petgraph 0.8.3**
- Rationale: Industry standard, actively maintained, rich algorithm library
- Use `DiGraph` or `Graph` with directed edges for CFG

**Dominance Analysis:** **domtree 0.1.0** (consider cranelift if more features needed)
- Rationale: Clean Lengauer-Tarjan implementation, minimal dependencies
- Consider extracting algorithm from rustc internals if domtree insufficient

**Path Enumeration:** **Custom implementation using petgraph + DFS backtracking**
- Rationale: No suitable off-the-shelf library; algorithm is well-understood
- Must implement: loop bounding, cycle detection, path caching

### Secondary Options

**Alternative to Charon:** Direct rustc-driver + custom MIR traversal
- Use when: Charon doesn't support required Rust feature
- Trade-off: More maintenance but full control

**Alternative to domtree:** Extract from rustc_data_structures or implement Lengauer-Tarjan from scratch
- Use when: Need post-dominators or advanced features
- Reference: [UT Austin explanation of Lengauer-Tarjan](https://www.cs.utexas.edu/~misra/Lengauer+Tarjan.pdf)

### Avoid

**StableMIR** for production in 2026 - not yet published
**cpg-rs** - not actively maintained, not Rust-specific
**cfg-traits** - trait-only, no implementations
**spirt** - wrong domain (shader compilation), not Rust-MIR
**-Z dump-mir parsing** - fragile text parsing, not programmatic

### Architecture Notes

For Mirage's hybrid approach (AST for structure, MIR for accuracy):

1. **AST Layer:** Use existing Magellan/tree-sitter for:
   - Function boundaries
   - Statement-level structure
   - Source location mapping

2. **MIR Layer:** Use Charon for:
   - Precise CFG structure
   - Borrow checker information
   - Control flow semantics

3. **Integration:** Map AST nodes to MIR basic blocks using source location info from Charon output

### Version Compatibility Matrix

| Component | Compatible With | Notes |
|-----------|-----------------|-------|
| Charon | Nightly rustc | Alpha, breaking changes expected |
| petgraph 0.8.3 | Rust 1.70+ | MSRV 1.70 |
| domtree 0.1.0 | Rust 2018+ | Standard edition |
| rustc-driver | Nightly only | Must match rustc version |

### Future Watch

1. **StableMIR on crates.io** - Will be game-changing when available (2026?)
2. **Charon maturity** - Watch for 1.0 release or stability announcements
3. **domtree enhancements** - Post-dominator support, additional algorithms
