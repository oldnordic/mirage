# Features Research - CFG Code Analysis

**Researched:** 2026-02-01

**Purpose:** Categorize features of CFG-based static analysis tools by table stakes, differentiators, and nice-to-haves for Mirage (a path-aware code intelligence engine for Rust).

---

## Table Stakes (Must Have)

Users will abandon a CFG analysis tool without these features. These are non-negotiable expectations for any tool claiming to do control flow analysis.

### Core CFG Construction

- **Basic Block Identification** - Partition code into maximal sequences of straight-line execution with single entry/exit - Complexity: Low
- **Edge Discovery** - Identify all control flow transitions (conditional, unconditional, exceptional) between blocks - Complexity: Low
- **Entry/Exit Node Detection** - Identify unique entry and exit nodes for reachability analysis - Complexity: Low
- **Graph Serialization** - Export CFG in standard formats (DOT, GraphML, JSON) for external tools - Complexity: Low

### Reachability Analysis

- **Unreachable Code Detection** - Identify blocks with no path from entry (code that can never execute) - Complexity: Low
- **Reachability Queries** - Answer "can node A reach node B?" for any pair of nodes - Complexity: Med
- **Path Existence** - Determine if any valid execution path exists between two points - Complexity: Med

### Dominance Relationships

- **Dominance Computation** - Calculate immediate dominators and dominator tree - Complexity: Med
- **Post-Dominance Computation** - Calculate post-dominators and post-dominator tree - Complexity: Med
- **Dominance Frontier** - Identify nodes where dominance relationships meet (prerequisite for SSA) - Complexity: Med

### Control Structure Recovery

- **Natural Loop Detection** - Identify back-edges where head dominates tail to detect loops - Complexity: Med
- **Loop Header Identification** - Determine which nodes are loop entry points - Complexity: Med
- **If/Else Recovery** - Identify conditional branching patterns - Complexity: Low
- **Switch/Match Recovery** - Identify multi-way branching structures - Complexity: Med

### Source Mapping

- **AST-to-CFG Correspondence** - Map each CFG node back to source code location (file, line, column) - Complexity: Low
- **Node-to-Statement Mapping** - Associate basic blocks with source statements - Complexity: Low
- **Location-Preserving Artifacts** - Graph exports maintain source location metadata - Complexity: Low

### Performance Characteristics

- **Linear-Time Graph Construction** - O(n) where n is code size for single function - Complexity: Med
- **Incremental Updates** - Handle code changes without full reconstruction - Complexity: High
- **Memory-Bounded Analysis** - Operate within predictable memory limits for large codebases - Complexity: Med

---

## Differentiators

Features that provide competitive advantage and distinguish advanced tools from basic CFG implementations.

### Path-Aware Analysis (Mirage's Focus)

- **Path Enumeration** - Explicitly enumerate all feasible execution paths through a function - Complexity: High
- **Path Feasibility Analysis** - Distinguish feasible from infeasible paths using constraint analysis - Complexity: High
- **Path Length Bounding** - Support for bounded path exploration (k-length, depth-limited) - Complexity: Med
- **Path Pruning Strategies** - Eliminate redundant or equivalent paths to manage explosion - Complexity: High
- **Symbolic Execution Integration** - Combine CFG with symbolic values for path-sensitive reasoning - Complexity: High

### Advanced Graph Structures

- **Control Dependence Graph (CDG)** - Explicit representation of control dependencies between nodes - Complexity: Med
- **Program Dependence Graph (PDG)** - Combined control and data dependence - Complexity: High
- **Interprocedural CFG** - Whole-program control flow across function boundaries - Complexity: High
- **Call-Sensitive Analysis** - Maintain separate CFG contexts per call site - Complexity: High
- **Context-Sensitive Analysis** - Distinguish calling contexts for precision - Complexity: Very High

### Code Property Graph Integration

- **AST + CFG Fusion** - Unified graph combining syntax and control flow - Complexity: Med
- **Multi-Layer Queries** - Query across AST, CFG, and dependency layers simultaneously - Complexity: High
- **Code Property Graph (CPG)** - Full integration of AST, CFG, and PDG per Joern specification - Complexity: Very High

### LLM-Friendly Outputs

- **Structured Path Artifacts** - Emit execution paths as structured JSON/protocols for LLM consumption - Complexity: Med
- **Path Summarization** - Generate natural language descriptions of control flow patterns - Complexity: High
- **Question-Answering Interface** - Support natural language queries about control flow - Complexity: Very High
- **Graph-to-Text Generation** - Convert CFG structures to human-readable explanations - Complexity: High

### Advanced Unreachable Code Detection

- **Path-Sensitive Unreachability** - Detect unreachable code considering specific execution contexts - Complexity: High
- **Condition Analysis** - Evaluate constant conditions to identify infeasible branches - Complexity: High
- **Dead Code Classification** - Distinguish control-flow unreachable from dead (executes but no effect) - Complexity: Med
- **Unused Variable Detection** - Identify variables defined but never used via dataflow - Complexity: High

### Irreducible Flow Handling

- **Irreducible Loop Detection** - Identify loops with multiple entry points (break natural loop assumptions) - Complexity: High
- **Structured Flow Reduction** - Transform irreducible flow to reducible form for analysis - Complexity: Very High
- **Goto Aggregation** - Handle arbitrary jumps for languages with computed gotos - Complexity: High

### Rust-Specific Advantages

- **Borrow Checker Integration** - Correlate CFG with lifetime and borrowing analysis - Complexity: Very High
- **Macro Expansion Tracking** - Map CFG nodes through macro expansion to source - Complexity: High
- **Async/Await Path Modeling** - Special handling for futures, async state machines - Complexity: High
- **Trait Method Resolution** - Interprocedural CFG across trait bounds and generics - Complexity: Very High

---

## Nice-to-Have

Useful features that can be deferred without losing core users.

### Visualization

- **Interactive Graph Visualization** - Web-based or GUI exploration of CFGs - Complexity: Med
- **Cluster/Group Layouts** - Automatic layout algorithms for large CFGs - Complexity: Med
- **Path Highlighting** - Visual indication of selected execution paths - Complexity: Low
- **Minimap Navigation** - Overview maps for navigating large graphs - Complexity: Med

### Additional Analyses

- **Cyclomatic Complexity** - Compute McCabe complexity metrics per function - Complexity: Low
- **Loop Nesting Depth** - Calculate maximum nesting of loops - Complexity: Low
- **Control Flow Metrics** - Various complexity and maintainability metrics - Complexity: Low
- **Data Flow Analysis** - Reaching definitions, liveness, available expressions - Complexity: High

### Developer Experience

- **IDE Integration** - Editor plugins for in-place CFG visualization - Complexity: High
- **Diff-Based Analysis** - Show how CFG changes between commits - Complexity: High
- **Hot Path Identification** - Highlight frequently-executed paths (with profile data) - Complexity: Med
- **Test Coverage Mapping** - Overlay test coverage on CFG nodes - Complexity: Med

### Export Formats

- **Multiple Serialization Formats** - DOT, GraphML, GEXF, JSON, custom binary - Complexity: Med
- **Image Export** - PNG, SVG renderings of CFGs - Complexity: Med
- **Animated Path Traversal** - Video/GIF of path exploration - Complexity: High

---

## Anti-Features (Deliberately NOT Build)

Features that should be actively avoided due to complexity, maintenance burden, or user harm.

### Path Explosion Traps

- **Full Unbounded Path Enumeration** - Attempting to enumerate ALL paths without bounds causes exponential explosion and unusable tools. Mitigation: Always use bounded, sampled, or summary-based path exploration.
- **Naive Symbolic Execution** - Unconstrained symbolic execution over any realistic codebase will never terminate. Mitigation: Use bounded exploration, path merging, and abstract interpretation.
- **Brute-Force Interprocedural Analysis** - Exhaustively exploring all call graph combinations is intractable. Mitigation: Use context summaries and demand-driven analysis.

### Over-Engineering Pitfalls

- **Full Abstract Interpretation Framework** - Building a complete abstract interpretation engine from scratch is a multi-year project. Mitigation: Use targeted analyses for specific questions, not general-purpose frameworks.
- **Custom Theorem Prover** - Don't build SMT solvers; integrate existing ones (Z3, CVC5) if needed.
- **Every Possible Metric** - Implementing hundreds of complexity metrics adds clutter. Focus on actionable insights.

### User Experience Anti-Patterns

- **Wall of Warnings** - Emitting hundreds of findings without prioritization causes users to ignore all output. Mitigation: Focus on high-confidence, actionable findings.
- **Unconfigurable False Positives** - Tools that cannot be tuned for specific codebases get disabled. Mitigation: Allow rule configuration and suppression.
- **Opaque Error Messages** - Generic "analysis failed" messages frustrate users. Mitigation: Always provide actionable context.

### Maintenance Burdens

- **Full Language Specification Coverage** - Trying to handle every edge case of Rust before shipping is a trap. Mitigation: Cover common patterns first, iterate on edge cases.
- **Cross-Language Generalization** - Building language-agnostic CFG upfront adds unnecessary abstraction. Mitigation: specialize in Rust, generalize later.
- **Real-Time Analysis for Large Codebases** - Promise of instant analysis on million-LOC codebases creates unrealistic expectations. Mitigation: Set clear performance expectations and use incremental analysis.

### The "Tool Builder Trap"

- **Configuration Overload** - Dozens of configuration options create decision paralysis. Mitigation: Sensible defaults, minimal configuration.
- **Plugin Architecture Before Core** - Building extensibility before the core is stable creates maintenance burden. Mitigation: Solid core first, plugins later.
- **Reimplementing Existing Tools** - Don't build your own parser, type checker, or build system. Use existing Rust tooling (rustc, cargo).

---

## Complexity Key

- **Low** - Straightforward implementation, well-known algorithms, weeks to implement
- **Med** - Moderate complexity, some research required, months to implement
- **High** - Significant complexity, open research problems, months to years
- **Very High** - Cutting-edge research area, significant risk, may not be feasible

---

## Sources

This research was synthesized from:

- [Analyzing control flow in Python - CodeQL](https://codeql.github.com/docs/codeql-language-guides/analyzing-control-flow-in-python/) - CFG basics, unreachable code detection, basic blocks
- [Code Property Graph Specification - Joern](https://cpg.joern.io/) - Multi-layer graph architecture (AST, CFG, PDG integration)
- [The Role of the Control Flow Graph in Static Analysis - Nicolo.dev](https://nicolo.dev/en/blog/role-control-flow-graph-static-analysis/) - Dominance, natural loops, CFG construction
- [Using the Soot flow analysis framework - McGill](https://www.sable.mcgill.ca/soot/tutorial/analysis/index.html) - Flow analysis framework, data flow on CFGs
- [A Critical Comparison on Six Static Analysis Tools - ScienceDirect](https://www.sciencedirect.com/science/article/pii/S0164121222002515) - Tool comparison, effectiveness metrics
- [Unreachable Code Elimination - GeeksforGeeks](https://www.geeksforgeeks.org/compiler-design/unreachable-code-elimination/) - Unreachable code techniques
- [A Survey of Symbolic Execution Techniques - USTC](http://staff.ustc.edu.cn/~bjhua/courses/theory/2021/ref/symexec.pdf) - Symbolic execution overview
- [Comparing Model Checking and Static Program Analysis - USENIX](https://www.usenix.org/event/ssv10/tech/full_papers/Vorobyov.pdf) - Analysis technique comparison
- [The Path Explosion Problem in Symbolic Execution - UU Thesis](https://studenttheses.uu.nl/bitstream/handle/20.500.12932/35856/thesis.pdf) - Path explosion challenges
- [How to Avoid the Pitfalls of Static Analysis - StackExchange](https://softwareengineering.stackexchange.com/questions/158535/how-to-avoid-the-pitfalls-of-static-analysis) - Common pitfalls
- [Program Slicing - Wikipedia](https://en.wikipedia.org/wiki/Program_slicing) - Slicing techniques for complexity reduction
- [Interprocedural Analysis - Harvard CS252](https://groups.seas.harvard.edu/courses/cs252/2011sp/slides/Lec05-Interprocedural.pdf) - Interprocedural vs intraprocedural complexity
