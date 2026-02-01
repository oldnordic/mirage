# Pitfalls Research - CFG & Path Enumeration

**Researched:** 2026-02-01

This document catalogs common mistakes and pitfalls in CFG-based code analysis and path enumeration tools, drawing from academic research and production tool experiences. Each pitfall includes warning signs, prevention strategies, and the project phase that should address it.

---

## Path Explosion

### Pitfall: Unbounded Loop Unrolling

**Description:** Treating loops as if all iterations produce distinct paths, leading to exponential path proliferation. A loop with N iterations and M branches inside produces O(2^N) paths.

**Warning signs:**
- Analysis time increases exponentially with loop count
- Memory consumption grows unbounded on nested loops
- Simple functions with loops timeout or OOM
- Path counts reported in millions for small functions

**Prevention:**
- Implement loop bounding heuristics (default to 2-4 iterations like Clang's analyzer)
- Use abstract interpretation for loop bodies (summarize loop effects instead of unrolling)
- Detect simple loop patterns (for i in 0..N) and apply bounded unrolling
- Separate "loop exit" from "loop continue" paths conceptually

**Phase:** Design/Scaffold - must be decided before implementation. PathLite should explicitly document that loop analysis uses summary abstraction.

**Sources:**
- [Improved Loop Execution Modeling in the Clang Static Analyzer](https://cyber.bibl.u.szeged.hu/index.php/actcybern/article/view/4104/4011) - Clang unrolls loops 4 times by default
- [QUERYX: Symbolic Query on Decompiled Code](https://kaist-hacking.github.io/pubs/2023/han:queryx.pdf) - Loop unrolling identified as primary path explosion cause
- [Characteristic Studies of Loop Problems](https://taoxie.cs.illinois.edu/publications/ase13-loopstudy.pdf) - DSE tools bound loop iterations or use heuristics

---

### Pitfall: Exhaustive Path Enumeration Strategy

**Description:** Attempting to enumerate ALL feasible paths through a program. This is mathematically intractable for non-trivial programs.

**Warning signs:**
- Design document claims "complete path coverage"
- No mention of path selection heuristics or prioritization
- Analysis treats all paths as equally important

**Prevention:**
- Use prioritization based on branch distance metrics (CREST approach)
- Implement path decomposition - break CFGs into manageable segments
- Use partial path analysis (path prefixes/suffixes) rather than full paths
- Consider hybrid approaches (fuzzing + selective symbolic execution like Driller)

**Phase:** Design - explicit choice of path exploration strategy is foundational.

**Sources:**
- [Systematic Literature Review of Software Vulnerability](https://www.worldscientific.com/doi/10.1142/S0218194025300027) - CREST prioritizes by branch distance, not exhaustive traversal
- [Augmenting Fuzzing Through Selective Symbolic Execution](https://www.ndss-symposium.org/wp-content/uploads/2017/09/driller-augmenting-fuzzing-through-selective-symbolic-execution.pdf) - Driller: 1400+ citations for hybrid approach
- [Path Exploration Strategy for Symbolic Execution](https://dl.acm.org/doi/fullHtml/10.1145/3671016.3671403) - Multi-strategy search for path selection

---

### Pitfall: Feasible vs Infeasible Path Confusion

**Description:** Counting or analyzing paths that can never be executed due to conflicting constraints, wasting computation on unreachable code.

**Warning signs:**
- No constraint solving or semantic analysis
- Treating all branches as always-taken
- Path counts include mutually exclusive conditions

**Prevention:**
- Must integrate with constraint solver (SMT solver like Z3)
- Implement path feasibility pruning before deep exploration
- Accept that this adds performance overhead but saves more by avoiding infeasible paths
- Consider lightweight feasibility checks before full constraint solving

**Phase:** Implementation - constraint integration must be architected early.

**Sources:**
- [Symbolic Execution in Practice: A Survey](https://arxiv.org/pdf/2508.06643) - Path feasibility remains computationally expensive
- [Enhancing Static Analysis with Lightweight Symbolic Execution](https://lujie.ac.cn/files/papers/SATRACER.pdf) - Hybrid approach to mitigate overhead

---

## Performance Issues

### Pitfall: Constraint Solver Bottleneck

**Description:** SMT constraint solving becomes the dominant performance bottleneck, often consuming 70-90% of execution time. This is universally recognized as THE primary bottleneck in symbolic execution.

**Warning signs:**
- Profiling shows solver calls dominate execution time
- Increasing solver timeout doesn't improve coverage much
- Complex path conditions cause solver timeouts

**Prevention:**
- Use multi-solver support (different solvers for different constraint types)
- Implement efficient encoding of symbolic expressions
- Cache solver results for repeated constraints
- Use solver time prediction (SMTimer-style approaches)
- Consider machine learning for solver strategy selection

**Phase:** Implementation - architecture must support pluggable solvers and caching.

**Sources:**
- [Enhancing Symbolic Execution by Machine Learning](https://www.ndss-symposium.org/ndss-paper/auto-draft-41/) - "Constraint solving creates a serious performance bottleneck"
- [Multi-solver Support in Symbolic Execution](https://srg.doc.ic.ac.uk/files/papers/klee-multisolver-cav-13.pdf) - KLEE multi-solver approach, 116+ citations
- [Encoding Symbolic Expressions as Efficient Solver Queries](http://dslab.epfl.ch/blog/2015/07/26/encoding-symbolic-expressions.html) - Efficient encoding is critical

---

### Pitfall: State Merging Not Implemented

**Description:** Exploring paths independently without merging equivalent states at join points, causing redundant analysis of overlapping path prefixes.

**Warning signs:**
- Same code analyzed multiple times with similar contexts
- Path count equals number of leaf nodes in CFG (no merging)
- Memory grows linearly with path count

**Prevention:**
- Implement static state merging at CFG join points
- Traverse CFG in topological order
- Merge symbolic states when they reach merge points
- Accept some precision loss for significant performance gains

**Phase:** Implementation - state merging is a core algorithm, not an add-on.

**Sources:**
- [Efficient State Merging in Symbolic Execution](https://dslab.epfl.ch/pubs/stateMerging.pdf) - 403 citations, fundamental technique

---

### Pitfall: Whole-Program Re-analysis on Changes

**Description:** Re-analyzing entire codebase for single-line changes, causing unacceptable latency for iterative development.

**Warning signs:**
- No change tracking or dependency graph
- Analysis time doesn't correlate with change size
- Development workflow inhibited by slow feedback

**Prevention:**
- Implement dependency tracking between analysis units
- Use reified computational dependencies for incremental updates
- Cache analysis results with invalidation rules
- Design for "analyze once, update results" workflow

**Phase:** Architecture - incremental design must be built in from start.

**Sources:**
- [Incremental Static Program Analysis through Reified Computational Dependencies](https://soft.vub.ac.be/Publications/2024/vub-soft-phd-20241104-Jens%20Van%20der%20Plas.pdf) - PhD thesis on incremental analysis
- [Incrementalizing Production CodeQL Analyses](https://arxiv.org/pdf/2308.09660) - 25 citations, production approach
- [Common Threads in Incremental Data Flow Analysis](https://dl.acm.org/doi/10.1145/3768155) - ICFG incremental analysis (2025)

---

### Pitfall: No Early Exit/Timeout Strategy

**Description:** Analysis runs indefinitely without bounded resource limits, causing hangs or excessive resource consumption.

**Warning signs:**
- No timeout parameters
- No maximum path depth limits
- No memory budgets

**Prevention:**
- Implement per-function and global timeout budgets
- Set maximum path depth bounds
- Use progress monitoring and early termination
- Implement best-effort partial results on timeout

**Phase:** Implementation - resource limits are a basic requirement.

---

## Integration Challenges

### Pitfall: Call Graph Construction Without Dynamic Dispatch Resolution

**Description:** Building call graphs that ignore function pointers, trait objects, and closures, producing incomplete interprocedural analysis.

**Warning signs:**
- Call graph only contains direct function calls
- No handling of `dyn Trait` or `fn()`
- Interprocedural analysis misses indirect call targets

**Prevention:**
- Implement context-sensitive pointer analysis for call targets
- Accept incomplete call graphs for dynamic dispatch initially
- Document limitations explicitly
- Consider whole-program analysis for better precision
- Use type-based filtering for trait object calls

**Phase:** Implementation - call graph construction affects interprocedural analysis design.

**Sources:**
- [Rupta: Context-Sensitive Pointer Analysis for Rust](https://dl.acm.org/doi/10.1145/3640537.3641574) - First context-sensitive analysis for Rust call graphs
- [Verifying Dynamic Trait Objects in Rust](https://cs.wellesley.edu/~avh/dyn-trait-icse-seip-2022-preprint.pdf) - Dynamic dispatch poses verification challenges
- [A Deep Dive into Interprocedural Optimization](https://llvm.org/devmtg/2020-09/slides/A_Deep_Dive_into_Interprocedural_Optimization.pdf) - Function pointers identified as major challenge

---

### Pitfall: Ignoring Recursion in Call Graph

**Description:** Treating recursive calls as separate contexts, leading to infinite context expansion or missing analysis.

**Warning signs:**
- Stack overflow on mutual recursion
- Infinite loop in call graph traversal
- No handling of strongly connected components

**Prevention:**
- Detect strongly connected components (SCCs) in call graph
- Merge functions in SCCs for context-insensitive analysis
- Set recursion depth bounds
- Implement fixpoint iteration for recursive functions

**Phase:** Implementation - recursion handling is fundamental to termination.

**Sources:**
- [Systemizing Interprocedural Static Analysis](https://dl.acm.org/doi/fullHtml/10.1145/3466820) - SCC merging for recursion

---

### Pitfall: Database Schema Not Designed for Incremental Updates

**Description:** Database schema requires full rebuild on code changes, or doesn't support efficient querying of CFG relationships.

**Warning signs:**
- No foreign key relationships between symbols and CFG nodes
- No change tracking timestamps
- Query performance degrades with codebase size
- Schema changes require full re-indexing

**Prevention:**
- Design schema with update operations in mind
- Include versioning/change tracking in schema
- Use graph-native features for CFG queries (edges, adjacency)
- Implement incremental index updates rather than full rebuilds
- Test schema evolution with mock updates

**Phase:** Architecture - database design determines update capabilities.

**Sources:**
- [Incremental Static Analysis of Large Source Code Repositories](https://ftsrg.mit.bme.hu/thesis-works/pdfs/stein-daniel-bsc.pdf) - Big Data challenges in incremental analysis
- [Data Indexing and Common Challenges](https://dev.to/badmonster/data-indexing-and-common-challenges-32cp) - Incremental updates are challenging

---

### Pitfall: Tight Coupling to Existing Database Schema

**Description:** Assuming Magellan's current schema can accommodate CFG data without modification, leading to inefficient queries or missing relationships.

**Warning signs:**
- Treating CFG as "just another symbol type"
- No dedicated edge/relationship tables
- CFG queries require multiple joins or post-processing

**Prevention:**
- Design separate CFG schema that integrates with Magellan
- Use foreign key references to existing symbols
- Consider hybrid approach: Magellan for symbols, dedicated CFG storage for paths
- Prototype integration before committing to schema

**Phase:** Architecture - integration points must be designed.

---

## Rust-Specific Challenges

### Pitfall: Ignoring Async/Await State Machine Desugaring

**Description:** Treating async/await as simple control flow without modeling the state machine transformation, leading to incorrect path analysis.

**Warning signs:**
- Async functions analyzed as if they execute linearly
- No modeling of `.await` yield points
- Cancellation points not identified
- Path analysis doesn't account for state machine transitions

**Prevention:**
- Understand async/await desugaring to generator state machines
- Analyze MIR (Mid-level IR) where async is already desugared
- Model await points as potential yield/resume
- Consider cancellation as an implicit exit path
- Document async analysis limitations explicitly

**Phase:** Discovery - must understand MIR representation before designing CFG extraction.

**Sources:**
- [Async/Await - The Challenges Besides Syntax](https://internals.rust-lang.org/t/async-await-the-challenges-besides-syntax-cancellation/10287) - Cancellation as design challenge
- [Enabling the Rust Compiler to Reason about Fork/Join Parallelism](https://dspace.mit.edu/bitstream/handle/1721.1/156790/hilton-jhilton-meng-eecs-2024-thesis.pdf) - CFG analysis complexity with async
- [Rustc Dev Guide - The MIR](https://rustc-dev-guide.rust-lang.org/mir/index.html) - MIR explicitly models control flow

---

### Pitfall: Closure Capture Analysis Omitted

**Description:** Not analyzing closure capture semantics, leading to incorrect understanding of data flow and lifetime interactions.

**Warning signs:**
- Closures treated as opaque function calls
- No tracking of captured variables
- Move vs borrow captures not distinguished
- Closure lifetimes not propagated

**Prevention:**
- Analyze closure capture clauses (Fn, FnMut, FnOnce)
- Model captured variables as closure struct fields
- Track capture kind (borrow, mutable borrow, move)
- Understand closure desugaring to struct with call method
- Use MIR which has closure info already computed

**Phase:** Implementation - closure analysis requires understanding of Rust's capture inference.

**Sources:**
- [Closure Capture Inference](https://rustc-dev-guide.rust-lang.org/closure.html) - Official rustc documentation
- [Precise Closure Capture Clauses](https://smallcultfollowing.com/babysteps/blog/2018/04/24/rust-pattern-precise-closure-capture-clauses/) - Closure desugaring patterns
- [A General-Purpose Model of Rust's Ownership](https://arxiv.org/html/2503.21691v5) - Path-sensitive ownership analysis (2025)

---

### Pitfall: Iterator Lazy Evaluation Not Handled

**Description:** Treating iterator chains as executing immediately, missing the deferred execution semantics that affect control and data flow.

**Warning signs:**
- Iterator adaptors analyzed as separate function calls
- No modeling of lazy evaluation
- Consumer operations (collect, fold) not distinguished
- Short-circuiting operations (any, find) not modeled

**Prevention:**
- Identify iterator chains as distinct pattern
- Model lazy evaluation (no execution until consumer)
- Track side effects through iterator chain
- Recognize short-circuiting operations
- Consider special handling for common iterator patterns

**Phase:** Implementation - iterator patterns require special recognition.

**Sources:**
- [Processing a Series of Items with Iterators](https://doc.rust-lang.org/book/ch13-02-iterators.html) - Official docs on laziness
- [Compositional Reasoning about Advanced Iterator](https://www.cs.ubc.ca/~alexsumm/papers/BilyHansenMuellerSummers23.pdf) - Academic treatment of iterator chains

---

### Pitfall: Borrow Checker Semantics Not Integrated

**Description:** Building CFG without considering lifetime regions and borrow checking, leading to analysis that doesn't reflect Rust's ownership constraints.

**Warning signs:**
- No integration with MIR borrow checker information
- Lifetimes treated as annotations rather than semantic constraints
- Move semantics not tracked through CFG
- Borrow regions not derived from CFG structure

**Prevention:**
- Work at MIR level where borrow info is available
- Understand Non-Lexical Lifetimes (NLL) - lifetimes from CFG, not scopes
- Track loan activity across CFG paths
- Consider integrating with or borrowing from Polonius (next-gen borrow checker)
- Document lifetime analysis limitations

**Phase:** Discovery - must understand NLL and borrow checking before MIR-based CFG design.

**Sources:**
- [The Borrow Checker](https://rustc-dev-guide.rust-lang.org/borrow_check.html) - Official MIR-based borrow checking docs
- [Optimising the Next-Generation Borrow Checker for Rust](https://www.diva-portal.org/smash/get/diva2:1981974/FULLTEXT01.pdf) - 2025 thesis on location-sensitive analysis
- [RFC 2094 - Non-Lexical Lifetimes](https://rust-lang.github.io/rfcs/2094-nll.html) - CFG-based lifetime inference
- [Polonius GitHub](https://github.com/rust-lang/polonius) - Next-gen borrow checker reference

---

### Pitfall: SSA Construction Complexity Underestimated

**Description:** Underestimating the complexity of converting MIR (which is nearly SSA) to proper SSA form with phi nodes, leading to incorrect or incomplete dataflow analysis.

**Warning signs:**
- No phi node insertion strategy
- No dominance frontier computation
- Dataflow analysis struggles with merge points
- Variable liveness not correctly tracked

**Prevention:**
- Understand that MIR is already in SSA-like form but not pure SSA
- Implement proper SSA construction (Cytron et al. algorithm or simplified variant)
- Use dominance frontiers for phi node placement
- Consider "pruned SSA" or "near-pruned SSA" for efficiency
- Recognize that SSA construction requires forward dataflow on CFG

**Phase:** Implementation - SSA construction is a well-studied but complex transformation.

**Sources:**
- [Simple and Efficient Construction of SSA](https://www.cs.cornell.edu/courses/cs6120/2025sp/blog/efficient-ssa/) - Modern treatment (2025)
- [Verified Construction of SSA Form](https://dl.acm.org/doi/10.1145/2892208.2892211) - High implementation complexity challenge
- [Near-Pruned SSA Transformation](https://www.sciencedirect.com/science/article/abs/pii/S2590118425000103) - 2025 efficiency variant

---

## Summary Matrix

| Pitfall Category | Most Impact Phase | Early Detection Strategy |
|------------------|-------------------|--------------------------|
| Loop unrolling | Design | Check for loop handling in architecture docs |
| Exhaustive paths | Design | Look for "complete coverage" claims |
| Solver bottleneck | Implementation | Profile first prototype |
| State merging | Implementation | Check for redundant analysis |
| Incremental updates | Architecture | Verify schema has change tracking |
| Dynamic dispatch | Implementation | Test with trait objects |
| Recursion | Implementation | Test with mutually recursive functions |
| Async/await | Discovery | Review MIR output for async functions |
| Closures | Implementation | Test with captured variables |
| Iterators | Implementation | Test with iterator chains |
| Borrow checking | Discovery | Review Polonius/NLL documentation |
| SSA construction | Implementation | Verify phi node correctness |

---

## Key Takeaways for Mirage

1. **Path explosion is unavoidable** - Don't attempt exhaustive enumeration. Use bounded unrolling, summarization, and prioritization from the start.

2. **Constraint solving is THE bottleneck** - Architecture must support multiple solvers, caching, and efficient encoding.

3. **Work at MIR level** - MIR has already handled desugaring (closures, async, pattern matching) and borrow checking. Don't re-invent this.

4. **Incremental by design** - The database schema and analysis pipeline must support incremental updates from day one.

5. **Document limitations** - Rust has features (dynamic dispatch, complex async, unbounded loops) that resist complete static analysis. Be explicit about what's NOT supported.

6. **Leverage existing research** - State merging, SSA construction, and incremental analysis have decades of research. Don't innovate on solved problems.

7. **Polonius is a reference** - The next-gen borrow checker faces similar challenges. Study its architecture and tradeoffs.
