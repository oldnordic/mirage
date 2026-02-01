# Architecture Research - CFG Analysis Systems

**Researched:** 2026-02-01

## Executive Summary

CFG-based code analysis systems follow a well-established architectural pattern with clear separation between frontend (source transformation), intermediate representation (IR), and backend (analysis). Research shows successful tools organize around a **pipeline architecture** where each component has a single responsibility and well-defined interfaces.

For Mirage, this means:

1. **Frontend**: MIR/AST extraction (source code to IR)
2. **Middle**: CFG construction (IR to control-flow graph)
3. **Backend**: Path enumeration, dominance, queries (graph to answers)

This research confirms the existing Mirage design is sound and identifies specific interface boundaries that should guide implementation.

---

## System Components

### 1. Frontend: Source-to-IR Transformation

**Purpose:** Transform source code into a structured intermediate representation suitable for CFG construction.

**Inputs:**
- Source code files (.rs, .py, .js, etc.)
- Project structure (crate/module definitions)

**Outputs:**
- AST nodes (tree-sitter or rustc AST)
- MIR (for Rust specifically)
- Symbol table mapping AST nodes to graph_entities

**Dependencies:**
- tree-sitter parsers (multi-language)
- rustc/rustc-driver (Rust MIR)
- Magellan graph_entities (for symbol linkage)

**Interface Contract:**
```rust
trait Frontend {
    // Parse source into AST nodes
    fn parse(&self, source: &SourceFile) -> Result<Vec<ASTNode>>;

    // Extract IR (MIR for Rust, AST for others)
    fn extract_ir(&self, function_id: SymbolId) -> Result<IntermediateRep>;

    // Link to existing Magellan symbols
    fn link_symbols(&self, ast: &ASTNode) -> Result<SymbolId>;
}
```

**Key Architectural Decision:** Hybrid approach
- **AST path**: Works for all tree-sitter languages, fast, but less precise
- **MIR path**: Rust-specific, slower, but includes types and borrow checker info

---

### 2. CFG Builder: IR-to-Graph Transformation

**Purpose:** Transform AST/MIR into control-flow graph representation with basic blocks and edges.

**Inputs:**
- AST nodes (from Frontend)
- MIR basic blocks (from rustc)
- Function symbol ID (from graph_entities)

**Outputs:**
- cfg_blocks records (basic blocks)
- cfg_edges records (control flow transitions)
- Entry/exit block identification

**Dependencies:**
- Frontend component (must complete first)
- graph_entities table (for function linkage)

**Interface Contract:**
```rust
trait CFGBuilder {
    // Build CFG for a single function
    fn build_cfg(&self, function_id: SymbolId, ir: &IntermediateRep) -> Result<CFG>;

    // Identify entry and exit blocks
    fn find_entry_exit(&self, cfg: &CFG) -> Result<(BlockId, BlockId)>;

    // Classify edge types
    fn classify_edge(&self, from: BlockId, to: BlockId, terminator: &Terminator) -> EdgeType;

    // Persist to database
    fn store(&self, cfg: &CFG) -> Result<()>;
}
```

**Edge Types:**
| Type | Description | Example |
|------|-------------|---------|
| TrueBranch | Condition evaluates to true | `if x > 0 { ... }` |
| FalseBranch | Condition evaluates to false | `if x > 0 { ... } else { ... }` |
| Fallthrough | Sequential execution | End of block, next statement |
| LoopBack | Return to loop entry | `while cond { ... }` |
| LoopExit | Exit loop when condition false | `for x in iter { break }` |
| Exception | Panic/throw (if detected) | `panic!()`, `?` operator |
| Call | Function call (may return) | `foo()` |
| Return | Function exit | `return`, `Result::Err` |

---

### 3. Path Enumerator: Graph-to-Paths Transformation

**Purpose:** Enumerate all execution paths through CFG, classify them by semantics, and cache results.

**Inputs:**
- cfg_blocks (from CFG Builder)
- cfg_edges (from CFG Builder)
- Configuration (max path length, prune loops)

**Outputs:**
- cfg_paths records (enumerated paths with BLAKE3 IDs)
- cfg_path_elements records (blocks in each path)
- Path classification (normal, error, degenerate, unreachable)

**Dependencies:**
- CFG Builder (must complete first)
- cfg_blocks table
- cfg_edges table

**Interface Contract:**
```rust
trait PathEnumerator {
    // Enumerate all paths through a CFG
    fn enumerate(&self, cfg: &CFG, options: &EnumOptions) -> Result<Vec<Path>>;

    // Classify path by semantics
    fn classify(&self, path: &Path, cfg: &CFG) -> PathKind;

    // Generate stable path ID
    fn path_id(&self, path: &Path) -> String;

    // Cache paths in database
    fn store(&self, paths: &[Path]) -> Result<()>;

    // Invalidate cached paths on function change
    fn invalidate(&self, function_id: SymbolId) -> Result<()>;
}

enum PathKind {
    Normal,      // Reaches normal exit
    Error,       // Exits via Err, panic, or error return
    Degenerate,  // Contains infinite loop or unreachable
    Unreachable, // No execution reaches this path
}

struct EnumOptions {
    max_length: usize,     // Max blocks per path (default: 100)
    max_paths: usize,      // Max paths to enumerate (default: 10000)
    prune_loops: bool,     // Limit loop unrolling (default: true)
    max_iterations: usize, // Max loop iterations (default: 3)
}
```

**Path Explosion Mitigation:**
Research from WCET analysis and CFG literature identifies these strategies:

1. **Path length limiting** - Cap at configurable maximum
2. **Loop bounding** - Treat loops as 0, 1, N iterations (abstract interpretation)
3. **Symbolic execution** - Represent loop effects symbolically
4. **Pruning** - Remove dominated paths early
5. **Implicit enumeration** - Use ILP formulations (from [Theiling 2002](https://publikationen.sulb.uni-saarland.de/bitstream/20.500.11880/25834/1/HenrikTheiling_ProfDrReinhardWilhelm.pdf))

---

### 4. Dominance Analyzer: Structural Relationship Computation

**Purpose:** Compute dominator and post-dominator relationships for "must-pass-through" proofs.

**Inputs:**
- cfg_blocks
- cfg_edges
- Entry/exit blocks (from CFG Builder)

**Outputs:**
- cfg_dominators records (dominance relationships)
- cfg_post_dominators records (reverse dominance)
- Dominance frontiers (for advanced analyses)

**Dependencies:**
- CFG Builder (must complete first)

**Interface Contract:**
```rust
trait DominanceAnalyzer {
    // Compute dominators using Cooper-Harvey-Kennedy algorithm
    fn compute_dominators(&self, cfg: &CFG) -> Result<DominatorTree>;

    // Compute post-dominators (reverse CFG)
    fn compute_post_dominators(&self, cfg: &CFG) -> Result<PostDominatorTree>;

    // Check if block A dominates block B
    fn dominates(&self, a: BlockId, b: BlockId) -> bool;

    // Check if all paths from entry to B pass through A
    fn must_pass_through(&self, entry: BlockId, target: BlockId, check: BlockId) -> bool;

    // Compute dominance frontiers (for SSA, dataflow)
    fn dominance_frontier(&self, cfg: &CFG) -> Result<HashMap<BlockId, Vec<BlockId>>>;

    // Store results
    fn store(&self, dominators: &DominatorTree) -> Result<()>;
}
```

**Algorithm Choice:** Cooper-Harvey-Kennedy (2001)
- **Why:** Simple, fast, works well with iterative dataflow
- **Complexity:** O(N^2) worst case, O(N log N) typical
- **Source:** [A Simple, Fast Dominance Algorithm](https://www.cs.tufts.edu/~nr/cs257/archive/keith-cooper/dom14.pdf)

**Applications:**
- **Must-pass-through proofs**: Verify validation always executes before use
- **Loop detection**: Natural loops via back edges
- **Dead code**: Blocks with no path from entry
- **SSA construction**: Dominance frontiers enable phi-node placement

---

### 5. Query Layer: User-Facing Interface

**Purpose:** Execute user queries and return results in human-readable or machine-parsable format.

**Inputs:**
- Query parameters (function ID, path kind, block ID, etc.)
- Database (all Mirage tables)

**Outputs:**
- Formatted results (human, JSON, dot, etc.)
- Proofs/counterexamples for verification queries

**Dependencies:**
- All other components (frontend, CFG builder, path enumerator, dominance analyzer)
- Magellan graph_entities (for symbol resolution)
- llmgrep (for symbol discovery integration)

**Interface Contract:**
```rust
trait QueryEngine {
    // Path queries
    fn paths_for_function(&self, function_id: SymbolId, filter: &PathFilter) -> Result<Vec<Path>>;
    fn path_by_id(&self, path_id: &str) -> Result<Path>;
    fn error_paths(&self, function_id: SymbolId) -> Result<Vec<Path>>;

    // CFG queries
    fn cfg_for_function(&self, function_id: SymbolId) -> Result<CFG>;
    fn blocks_for_function(&self, function_id: SymbolId) -> Result<Vec<Block>>;

    // Dominance queries
    fn dominators(&self, block_id: BlockId) -> Result<Vec<BlockId>>;
    fn must_pass_through(&self, entry: BlockId, target: BlockId, check: BlockId) -> Result<bool>;

    // Analysis queries
    fn unreachable_code(&self, function_id: SymbolId) -> Result<Vec<Block>>;
    fn dead_branches(&self, function_id: SymbolId) -> Result<Vec<Edge>>;
    fn blast_zone(&self, symbol_id: SymbolId, max_depth: usize) -> Result<BlastZone>;
}
```

---

### 6. Verification Component: Artifact Validation

**Purpose:** Verify that cached artifacts (paths, CFGs) remain valid after code changes.

**Inputs:**
- Path ID or CFG to verify
- Current state of database
- Changed file/function list

**Outputs:**
- Verification result (valid/invalid)
- Updated artifacts (if invalid but reconstructible)
- Deletion command (if invalid and unreconstructible)

**Dependencies:**
- All Mirage tables
- Magellan change tracking (if available)

**Interface Contract:**
```rust
trait Verifier {
    // Verify a single path is still valid
    fn verify_path(&self, path_id: &str) -> Result<VerificationResult>;

    // Verify all paths for a function
    fn verify_function(&self, function_id: SymbolId) -> Result<Vec<VerificationResult>>;

    // Re-enumerate invalid paths
    fn reenumerate(&self, function_id: SymbolId) -> Result<()>;

    // Batch verification after code change
    fn verify_batch(&self, changed_functions: Vec<SymbolId>) -> Result<VerificationReport>;
}

enum VerificationResult {
    Valid,           // Path still exists
    InvalidDeleted,  // Path no longer exists (block/edge removed)
    InvalidModified, // Path changed (needs re-enumeration)
    Recreated,       // Path was invalid, successfully recreated
}
```

---

## Data Flow

### Indexing Pipeline (Write Path)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          INDEXING PIPELINE                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Source Code                                                               │
│      │                                                                      │
│      ▼                                                                      │
│  ┌─────────────────┐                                                        │
│  │   FRONTEND      │  ◄── tree-sitter, rustc MIR                          │
│  │  - Parse AST    │                                                        │
│  │  - Extract MIR  │                                                        │
│  │  - Link symbols │                                                        │
│  └────────┬────────┘                                                        │
│           │                                                                 │
│           ▼                                                                 │
│  ┌─────────────────┐                                                        │
│  │   CFG BUILDER   │  ◄── AST/MIR -> cfg_blocks, cfg_edges                │
│  │  - Basic blocks │                                                        │
│  │  - Edge types   │                                                        │
│  │  - Entry/exit   │                                                        │
│  └────────┬────────┘                                                        │
│           │                                                                 │
│           ├──────────────────────┬──────────────────────┐                   │
│           ▼                      ▼                      ▼                   │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐             │
│  │ PATH ENUMERATOR │  │ DOMINANCE       │  │     QUERY       │ (parallel)   │
│  │ - Enumerate     │  │ ANALYZER        │  │ - Ready to      │             │
│  │ - Classify      │  │ - Dominators    │  │   serve         │             │
│  │ - Cache paths   │  │ - Post-domin    │  │                 │             │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Query Pipeline (Read Path)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           QUERY PIPELINE                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  CLI / API Request                                                          │
│      │                                                                      │
│      ▼                                                                      │
│  ┌─────────────────┐                                                        │
│  │  QUERY PARSER   │  ◄── Parse CLI args, extract filters                  │
│  └────────┬────────┘                                                        │
│           │                                                                 │
│           ▼                                                                 │
│  ┌─────────────────┐                                                        │
│  │  QUERY ENGINE   │  ◄── Route to appropriate handler                    │
│  └────────┬────────┘                                                        │
│           │                                                                 │
│           ├──────────────┬──────────────┬──────────────┐                    │
│           ▼              ▼              ▼              ▼                    │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐                │
│  │  Path     │  │   CFG     │  │ Dominance │  │ Analysis  │                │
│  │  Queries  │  │  Queries  │  │  Queries  │  │  Queries  │                │
│  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘                │
│        │             │             │             │                          │
│        └─────────────┴─────────────┴─────────────┘                          │
│                      │                                                      │
│                      ▼                                                      │
│         ┌──────────────────────┐                                           │
│         │    RESULT FORMAT     │  ◄── human, json, dot                     │
│         └──────────────────────┘                                           │
│                      │                                                      │
│                      ▼                                                      │
│              Output to User                                                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Verification Pipeline (Incremental Updates)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       VERIFICATION PIPELINE                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Code Change Detected                                                       │
│      │                                                                      │
│      ▼                                                                      │
│  ┌─────────────────┐                                                        │
│  │   AFFECTED      │  ◄── Identify changed functions                       │
│  │   FUNCTION SCAN │                                                        │
│  └────────┬────────┘                                                        │
│           │                                                                 │
│           ▼                                                                 │
│  ┌─────────────────┐                                                        │
│  │   INVALIDATE    │  ◄── Mark cached artifacts invalid                   │
│  │   CACHE         │                                                        │
│  └────────┬────────┘                                                        │
│           │                                                                 │
│           ▼                                                                 │
│  ┌─────────────────┐                                                        │
│  │   RE-ENUMERATE  │  ◄── Only for changed functions                       │
│  │   PATHS         │                                                        │
│  └────────┬────────┘                                                        │
│           │                                                                 │
│           ▼                                                                 │
│  ┌─────────────────┐                                                        │
│  │   VERIFY        │  ◄── Cross-reference with Magellan                    │
│  │   ARTIFACTS     │                                                        │
│  └─────────────────┘                                                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Integration with Magellan

### Shared Database Approach

Mirage extends the existing Magellan/SQLiteGraph database rather than creating separate storage. This enables:

1. **Atomic updates**: CFG and symbols update together
2. **JOIN queries**: Combine CFG info with symbol metadata
3. **Single source of truth**: No divergence between tools

### Table Relationships

```
Magellan Core                    Mirage Extensions
─────────────                    ┌───────────────────────────────────────────┐
│ graph_entities │──────────────►│ cfg_blocks.function_id                 │
│ (id)           │               │   └── Links to Function symbols         │
└────────────────┘               │                                          │
                                 │ cfg_paths.function_id                   │
┌─────────────────┐              │   └── Links to Function symbols         │
│ graph_edges     │              │                                          │
│ (from_id,       │              │ cfg_dominators                          │
│  to_id)         │              │ cfg_post_dominators                     │
└─────────────────┘              │                                          │
                                 │ cfg_path_elements                        │
┌─────────────────┐              │   └── References cfg_blocks             │
│ code_chunks     │              └───────────────────────────────────────────┘
│ (file_path,     │
│  byte_start)    │              Integration Points:
└─────────────────┘              1. Symbol Discovery: llmgrep finds symbol_id
                                 2. CFG Query: Mirage uses symbol_id to get CFG
                                 3. Span Lookup: Mirage uses byte_start/End
                                    to fetch code_chunks
```

### Cross-Tool Workflows

**Workflow 1: Find and Analyze**
```bash
# Step 1: Find symbol using llmgrep
SYMBOL_ID=$(llmgrep search --query "parse_request" --output json | jq -r '.[0].symbol_id')

# Step 2: Get all paths through function
mirage paths --function $SYMBOL_ID --show-errors
```

**Workflow 2: Verify and Validate**
```bash
# Step 1: Check if validation is wired
llmgrep refs --symbol-id $VALIDATION_ID --direction in

# Step 2: Prove validation dominates usage
mirage dominators --function $USAGE_ID --must-pass-through $VALIDATION_ID
```

**Workflow 3: Impact Analysis**
```bash
# Step 1: Find all callers (Magellan/llmgrep)
CALLERS=$(llmgrep search --mode calls --direction in --query $SYMBOL)

# Step 2: For each caller, get error paths
for caller in $CALLERS; do
    mirage paths --function $caller --show-errors
done
```

---

## Build Order

Based on component dependencies and research on static analysis tool architecture:

### Phase 1: Foundation (Milestone 1)
**Duration:** ~2 weeks

**Components:**
1. Database schema extension (`mirage_meta`, `cfg_blocks`, `cfg_edges`)
2. Frontend skeleton (tree-sitter integration)
3. MIR extraction prototype (single file support)

**Why first:** Infrastructure must exist before analysis can run.

**Acceptance:**
```bash
mirage index --file test.rs
echo "✓ MIR extracted"
echo "✓ CFG blocks created"
```

---

### Phase 2: CFG Construction (Milestone 2)
**Duration:** ~2 weeks

**Components:**
1. CFG Builder implementation
2. Edge classification
3. Entry/exit detection
4. Basic query support (`mirage cfg --function SYMBOL`)

**Dependencies:** Phase 1 complete

**Why second:** All downstream analysis depends on correct CFG.

**Acceptance:**
```bash
mirage cfg --function main --format dot | dot -Tpng -o cfg.png
echo "✓ CFG visualized correctly"
```

---

### Phase 3: Path Enumeration (Milestone 3)
**Duration:** ~3 weeks

**Components:**
1. Path enumerator (DFS-based)
2. Path classifier (normal/error/degenerate)
3. Path caching (`cfg_paths`, `cfg_path_elements`)
4. Path queries (`mirage paths --function SYMBOL`)

**Dependencies:** Phase 2 complete

**Why third:** Paths are primary value proposition; enables all downstream analysis.

**Acceptance:**
```bash
mirage paths --function process --show-errors
echo "✓ Error paths enumerated"
```

---

### Phase 4: Dominance Analysis (Milestone 4)
**Duration:** ~2 weeks

**Components:**
1. Cooper-Harvey-Kennedy algorithm implementation
2. Post-dominator computation
3. Dominance storage (`cfg_dominators`, `cfg_post_dominators`)
4. Dominance queries (`mirage dominators --must-pass-through`)

**Dependencies:** Phase 2 complete (can run in parallel with Phase 3)

**Why fourth:** Enables "must-pass-through" proofs and advanced analyses.

**Acceptance:**
```bash
mirage dominators --function use_input --must-pass-through validate
echo "✓ Proof: all paths pass through validate"
```

---

### Phase 5: Advanced Analysis (Milestone 5)
**Duration:** ~3 weeks

**Components:**
1. Dead code detection (unreachable blocks)
2. Wrong branch detection (bypassed cleanup/error handling)
3. Blast zone analysis (path-based impact)
4. Verification component

**Dependencies:** Phases 3 and 4 complete

**Why fifth:** Builds on paths and dominance to provide higher-level insights.

**Acceptance:**
```bash
mirage unreachable --within-functions
echo "✓ Dead code found"
```

---

### Phase 6: Integration & Polish (Milestone 6)
**Duration:** ~2 weeks

**Components:**
1. llmgrep integration (path data in search results)
2. Incremental update pipeline
3. CLI consistency with Magellan
4. Documentation and examples

**Dependencies:** All previous phases complete

**Why last:** Integration depends on all core features working.

**Acceptance:**
```bash
# Cross-tool workflow
llmgrep search --query "parse" | jq -r '.[0].symbol_id' | xargs mirage paths --function
echo "✓ Integrated workflow works"
```

---

## Parallelization Opportunities

Some components can be developed in parallel after dependencies are met:

| Phase | Can Parallel With | Shared Dependency |
|-------|-------------------|-------------------|
| Phase 4 | Phase 3 | CFG Builder (Phase 2) |
| Phase 5 (dead code) | Phase 4 (partially) | CFG Builder |
| Phase 6 (llmgrep integration) | Phase 5 | Query interface |

---

## Interface Boundaries for Incremental Development

### 1. Frontend Interface
```rust
// File: mirage/src/frontend/mod.rs
pub trait IRExtractor {
    fn extract(&self, source: &SourceFile) -> Result<Vec<IRNode>>;
    fn link_function(&self, node: &IRNode) -> Result<SymbolId>;
}
```

**Implementation sequence:**
- Step 1: Define trait
- Step 2: Tree-sitter implementation (AST only)
- Step 3: MIR extraction (Rust-specific)
- Step 4: Multi-language support

---

### 2. CFG Interface
```rust
// File: mirage/src/cfg/mod.rs
pub trait CFGBuilder {
    fn build(&self, ir: &[IRNode], function_id: SymbolId) -> Result<CFG>;
    fn store(&self, cfg: &CFG) -> Result<()>;
}

pub struct CFG {
    pub blocks: Vec<Block>,
    pub edges: Vec<Edge>,
    pub entry: BlockId,
    pub exit: BlockId,
}
```

**Implementation sequence:**
- Step 1: Define CFG struct
- Step 2: Build from AST nodes (simple cases)
- Step 3: Handle control flow (if, match, loop)
- Step 4: Handle complex cases (async, try operators)

---

### 3. Path Interface
```rust
// File: mirage/src/paths/mod.rs
pub trait PathEnumerator {
    fn enumerate(&self, cfg: &CFG, options: &Options) -> Result<Vec<Path>>;
    fn store(&self, paths: &[Path]) -> Result<()>;
}

pub struct Path {
    pub id: String,        // BLAKE3 hash
    pub function_id: SymbolId,
    pub kind: PathKind,
    pub blocks: Vec<BlockId>,
}
```

**Implementation sequence:**
- Step 1: Naive DFS (no loop handling)
- Step 2: Cycle detection
- Step 3: Path limiting
- Step 4: Loop bounding
- Step 5: Classification

---

### 4. Dominance Interface
```rust
// File: mirage/src/analysis/dominators.rs
pub trait DominanceAnalyzer {
    fn compute(&self, cfg: &CFG) -> Result<DominatorTree>;
    fn dominates(&self, a: BlockId, b: BlockId) -> bool;
}

pub struct DominatorTree {
    pub strict_dominators: HashMap<BlockId, HashSet<BlockId>>,
    pub immediate_dominators: HashMap<BlockId, Option<BlockId>>,
}
```

**Implementation sequence:**
- Step 1: Iterative algorithm
- Step 2: Post-dominators
- Step 3: Frontiers
- Step 4: Query helpers

---

## Key Architectural Insights from Research

### Insight 1: Pipeline Architecture is Standard

Static analysis tools universally use a **pipeline architecture** ([Unveiling the Power of IRs](https://arxiv.org/html/2405.12841v1), 2024):

```
Source → Frontend → IR → Analysis → Results
```

**Implication for Mirage:** The existing design aligns with industry practice. No major restructuring needed.

---

### Insight 2: IR Choice is Critical

Research emphasizes that **intermediate representation quality determines analysis capability** ([Static Analysis Using IRs](https://par.nsf.gov/servlets/purl/10519999), 2023):

- High-level IR: Better for semantic understanding
- Low-level IR: Better for precision
- Multi-level IR: Best of both (SAIL project)

**Implication for Mirage:** Hybrid approach (AST + MIR) is correct. AST for structure, MIR for Rust-specific precision.

---

### Insight 3: Path Explosion is the Hard Problem

All literature on CFG analysis identifies **path explosion** as the primary challenge ([Theiling 2002](https://publikationen.sulb.uni-saarland.de/bitstream/20.500.11880/25834/1/HenrikTheiling_ProfDrReinhardWilhelm.pdf)).

**Solutions identified:**
1. Loop bounding (limit iterations)
2. Path length limiting
3. Symbolic execution
4. Implicit enumeration (IPET)

**Implication for Mirage:** Implement loop bounding from day one. Don't attempt exhaustive enumeration.

---

### Insight 4: Dominance Algorithms Are Well-Understood

The [Cooper-Harvey-Kennedy algorithm](https://www.cs.tufts.edu/~nr/cs257/archive/keith-cooper/dom14.pdf) (2001) is the standard approach.

**Implication for Mirage:** Use the known algorithm. No research needed here.

---

### Insight 5: Integration Enables New Capabilities

Tools that integrate with existing ecosystems ([CodeChecker](https://www.arxiv.org/pdf/2408.02220), 2024) provide more value than standalone tools.

**Implication for Mirage:** The Magellan/llmgrep integration is a key differentiator. Prioritize this in Phase 6.

---

## References

### Academic Papers
1. Cooper, Harvey, Kennedy. [A Simple, Fast Dominance Algorithm](https://www.cs.tufts.edu/~nr/cs257/archive/keith-cooper/dom14.pdf). 2001.
2. Theiling, Henrik. [Control Flow Graphs for Real-Time System Analysis](https://publikationen.sulb.uni-saarland.de/bitstream/20.500.11880/25834/1/HenrikTheiling_ProfDrReinhardWilhelm.pdf). 2002.
3. Dwyer, MB. [A Flexible Architecture for Building Data Flow Analyzers](https://dl.acm.org/doi/pdf/10.5555/227726.227843). 1996.

### Recent Research (2024-2025)
4. [Unveiling the Power of Intermediate Representations](https://arxiv.org/html/2405.12841v1). May 2024.
5. [Static Analysis Using Intermediate Representations](https://par.nsf.gov/servlets/purl/10519999). 2023.
6. [CodeChecker Integration Platform](https://www.arxiv.org/pdf/2408.02220). August 2024.

### Educational Resources
7. [The Role of the Control Flow Graph in Static Analysis](https://nicolo.dev/en/blog/role-control-flow-graph-static-analysis/). October 2023.
8. Cornell University. [ECE 6775: Control Flow Lecture Notes](https://www.csl.cornell.edu/courses/ece6775/pdf/lecture08.pdf).
9. [Intermediate Representation Overview](https://www.emergentmind.com/topics/intermediate-representation-ir). November 2025.

### Tool Documentation
10. Magellan Project. [GitHub Repository](https://github.com/oldnordic/magellan).
11. Charon. [Rust MIR/CFG Extraction](https://github.com/AeneasVerif/charon).
12. SQLiteGraph. [Embedded Graph Database](https://github.com/oldnordic/sqlitegraph).

---

*Last updated: 2026-02-01*
