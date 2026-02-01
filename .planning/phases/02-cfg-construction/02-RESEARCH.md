# Phase 2: CFG Construction - Research

**Researched:** 2026-02-01
**Domain:** Control Flow Graph construction from MIR (Rust) and AST (multi-language)
**Confidence:** HIGH

## Summary

This phase focuses on building Control Flow Graphs (CFGs) from intermediate representations. For Rust code, Mirage will use Charon to extract MIR (Mid-level Intermediate Representation) which already contains basic blocks and control flow edges. For non-Rust code, tree-sitter AST will be used with a custom CFG construction algorithm.

**Primary recommendation:** Use Charon's ULLBC (Unstructured Low-Level Borrow Calculus) for Rust CFG extraction—it provides pre-built basic blocks with terminators. For non-Rust languages, implement a standard leader-based basic block identification algorithm with visitor pattern over tree-sitter AST.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| petgraph | 0.8.3 | Graph data structure | Industry standard (235M+ downloads), rich algorithm library, DOT export |
| charon | alpha | MIR extraction for Rust | Purpose-built for Rust MIR extraction, outputs ULLBC/LLBC with CFG |
| tree-sitter | latest | AST parsing for non-Rust | Multi-language parsing, already used in Magellan |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde | latest | JSON serialization | Export CFG for tool integration |
| domtree | 0.1.0 | Dominance analysis (future phase) | Phase 4: Dominator tree computation |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Charon | rustc-driver direct | Charon provides cleaned-up ULLBC; rustc-driver requires nightly and complex setup |
| petgraph | spirt | petgraph is general-purpose and stable; spirt is research-grade for shader compilation |
| tree-sitter | ast-grep | tree-sitter has better multi-language support and Rust integration |

**Installation:**
```bash
# Core dependencies
cargo add petgraph serde

# For MIR extraction (Rust only)
# Charon is run as external binary, produces JSON
# See: https://github.com/AeneasVerif/charon
```

## Architecture Patterns

### Charon-Based CFG Construction (Rust)

**What:** Charon extracts MIR from Rust crates and outputs ULLBC (Unstructured Low-Level Borrow Calculus) which is already a CFG.

**When to use:** All Rust code analysis.

**Key data structures from Charon:**

```rust
// Source: https://arxiv.org/html/2410.18042v3

// Charon's output structure
pub struct TranslatedCrate {
    pub crate_name: String,
    pub files: Vector<FileId, File>,
    pub fun_decls: Vector<FunDeclId, FunDecl>,
    // ... other declarations
}

pub struct FunDecl {
    pub id: FunDeclId,
    pub meta: ItemMeta,
    pub signature: FunSig,
    pub body: Result<Body, Opaque>,
}

// LLBC statements - CFG is embedded in ULLBC
pub enum StatementKind {
    Assign(Place, Rvalue),
    Call(Call),
    Abort(AbortKind),      // panic
    Switch(Switch),        // branching
    Loop(Block),           // loops
    Return,
    Nop,
    Drop(Place),
    Break(usize),
    Continue(usize),
    // ... omitted
}

pub struct Statement {
    pub span: Span,           // Source location
    pub kind: StatementKind,
    pub comments: Vec<String>,
}
```

**Integration flow:**
1. Run `charon` binary (outputs `.llbc` or `.ullbc` JSON file)
2. Parse JSON with `serde_json::from_reader::<charon_lib::export::CrateData>(file)`
3. Extract ULLBC body which contains basic blocks and terminators
4. Convert ULLBC to petgraph::Graph for Mirage's internal representation

### AST-Based CFG Construction (Non-Rust)

**What:** Build CFG from tree-sitter AST using standard compiler algorithms.

**When to use:** Non-Rust languages, fallback when Charon fails.

**Algorithm: Leader-based basic block identification**

```
Leaders are identified as:
1. First instruction in function (ENTRY)
2. Target of any branch (conditional, unconditional)
3. Instruction immediately following a branch

Basic blocks:
- Maximal sequence of instructions from leader to next leader (exclusive)
- Last instruction in block is always a terminator
```

**Implementation pattern:**

```rust
// Visitor pattern over tree-sitter AST
pub struct CFGBuilder<'a> {
    graph: DiGraph<BasicBlock, EdgeType>,
    entry: Option<NodeIndex>,
    exits: Vec<NodeIndex>,
    source_map: HashMap<NodeIndex, SourceLocation>,
}

impl<'a> CFGBuilder<'a> {
    pub fn from_function(node: &Node) -> Result<Self> {
        let mut builder = CFGBuilder::new();

        // Find entry block (first statement)
        let entry = builder.visit_block(&node);
        builder.entry = Some(entry);

        // Recursively build CFG
        builder.visit_statements(&node);

        // Identify exits (return, unreachable)
        builder.find_exits();

        Ok(builder)
    }

    fn visit_statements(&mut self, node: &Node) {
        match node.kind() {
            "if_statement" => self.handle_if(node),
            "while_statement" => self.handle_while(node),
            "for_statement" => self.handle_for(node),
            "match_statement" => self.handle_match(node),
            "return_statement" => self.handle_return(node),
            _ => self.sequential(node),
        }
    }
}
```

### Edge Types Classification

| Type | Description | MIR Equivalent | AST Source |
|------|-------------|----------------|------------|
| TrueBranch | Condition evaluates true | `SwitchInt` to target | `if` condition true branch |
| FalseBranch | Condition evaluates false | `SwitchInt` otherwise | `if` else branch |
| Fallthrough | Sequential execution | `Goto` | Next statement |
| LoopBack | Return to loop header | `Goto` to header | `while`, `for` continue |
| LoopExit | Exit loop when false | `SwitchInt` otherwise | `while`, `for` break |
| Exception | Panic/throw | `UnwindResume`, `Assert` | `panic!`, `?` operator |
| Call | Function call (returns) | `Call` with target | Function call |
| Return | Function exit | `Return` | `return` statement |

### Recommended Project Structure

```
src/
├── cfg/
│   ├── mod.rs              # CFG builder trait and common types
│   ├── mir.rs              # Charon/ULLBC to CFG conversion
│   ├── ast.rs              # Tree-sitter AST to CFG conversion
│   ├── edge.rs             # Edge type classification
│   └── export.rs           # DOT/JSON serialization
├── mir/
│   └── charon.rs           # Charon process spawning and output parsing
└── storage/
    └── schema.rs           # Database tables (extended from Phase 1)
```

### Anti-Patterns to Avoid

- **Inline Charon dependency:** Don't link directly against `charon-lib`—it requires nightly rustc. Use Charon as external binary, parse JSON output.
- **Re-implementing MIR traversal:** Charon's ULLBC already provides basic blocks. Don't reconstruct from MIR.
- **Ignoring source spans:** Both ULLBC and tree-sitter provide source location. Always map CFG nodes back to source.
- **Assuming single exit:** Functions can have multiple exits (early returns, panics). Always track exit nodes as a set.
- **Missing unwind edges:** Rust has explicit panic paths. Include `unwind` edges from terminators.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Graph data structure | Custom adjacency list | petgraph::Graph | 235M+ downloads, battle-tested, DOT export |
| JSON serialization | Custom serialization | serde | De facto standard, works with petgraph |
| MIR extraction | Custom rustc-driver | Charon binary | 18kLoC of cleanup logic, actively maintained |
| DOT export | Custom DOT writer | petgraph::dot::Dot | Built-in, configurable output |
| AST parsing | Custom parsers | tree-sitter | Multi-language, already in Magellan |

**Key insight:** The core value of Mirage is in the analysis (reachability, paths), not in IR extraction or graph representation. Use existing, mature tools.

## Common Pitfalls

### Pitfall 1: Confusing LLBC with ULLBC

**What goes wrong:** LLBC is a structured AST (control flow reconstructed). ULLBC is the CFG. For CFG construction, use ULLBC.

**Why it happens:** Charon paper emphasizes LLBC but CFG needs the unstructured form.

**How to avoid:**
```rust
// Request ULLBC output (not LLBC)
// ULLBC contains basic blocks directly
// LLBC requires Relooper-like algorithm to reconstruct CFG (don't do this)
```

**Warning signs:** Trying to identify basic blocks in LLBC's structured statements.

### Pitfall 2: Missing Unwind Edges

**What goes wrong:** CFG doesn't include panic/unwind paths, making analysis incorrect for error handling.

**Why it happens:** Rust's `Result` and `?` operator can panic. Charon's ULLBC includes `unwind` edges.

**How to avoid:** Always check terminators for `unwind` field:
```rust
// From rustc MIR TerminatorKind (what Charon is based on)
// Each terminator may have an unwind target:
// - `UnwindAction::Cleanup(block)` -> explicit cleanup block
// - `UnwindAction::Continue` -> continue unwinding
// - `UnwindAction::Terminate` -> abort
```

**Warning signs:** All paths through function end at a single return.

### Pitfall 3: Incorrect Entry/Exit Detection

**What goes wrong:** Entry node not the first block, or missing multiple exit nodes.

**Why it happens:** Functions can have multiple returns, early exits, panics.

**How to avoid:**
- **Entry:** First basic block in function body (uniquely identified)
- **Exits:** All blocks with `Return` terminator, `UnwindTerminate`, `Unreachable` at function end

**Algorithm:**
```rust
fn find_entry(body: &Body) -> BlockId {
    body.blocks.first().unwrap().id
}

fn find_exits(body: &Body) -> Vec<BlockId> {
    body.blocks.iter()
        .filter(|b| matches!(b.terminator.kind,
            TerminatorKind::Return |
            TerminatorKind::UnwindTerminate(_) |
            TerminatorKind::Unreachable))
        .map(|b| b.id)
        .collect()
}
```

### Pitfall 4: Source Location Loss

**What goes wrong:** CFG nodes not mapped back to source code.

**Why it happens:** Intermediate representations may lose original source spans.

**How to avoid:** Both Charon ULLBC and tree-sitter provide spans:
- Charon: `Statement.span` contains file, line, column
- Tree-sitter: `Node.range()` returns byte offsets

**Store in database:**
```sql
CREATE TABLE cfg_blocks (
    id INTEGER PRIMARY KEY,
    function_id INTEGER NOT NULL,
    block_index INTEGER NOT NULL,  -- Order in function

    -- Source mapping (from ULLBC/AST)
    file_path TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    start_column INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    end_column INTEGER NOT NULL,

    FOREIGN KEY (function_id) REFERENCES graph_entities(id)
);
```

### Pitfall 5: SwitchInt Representation

**What goes wrong:** Multi-way branching (match, switch) incorrectly modeled as multiple if-else.

**Why it happens:** `SwitchInt` in MIR represents all multi-way branches (match on integer, enum, bool).

**How to avoid:** Model `SwitchInt` as a single node with multiple successors:
```rust
// From rustc MIR TerminatorKind
SwitchInt {
    discr: Operand<'tcx>,        // Value being switched on
    targets: SwitchTargets,       // Map value -> block
}
// Access successors: targets.iter()
```

### Pitfall 6: DOT Export Performance

**What goes wrong:** DOT export for large functions is slow and creates huge files.

**Why it happens:** petgraph's `Dot` formatter is intended for debugging, not production.

**How to avoid:**
- Use custom formatters with `Dot::with_attr_getters`
- Filter graphs before export (e.g., only show reachable blocks)
- Consider JSON export for large CFGs

## Code Examples

### MIR Terminator Edge Discovery

```rust
// Source: https://doc.rust-lang.org/beta/nightly-rustc/rustc_middle/mir/enum.TerminatorKind.html

// Rust MIR TerminatorKind variants and their successors:
//
// Goto { target } -> [target]
// SwitchInt { targets } -> [all target blocks]
// Return -> [] (EXIT)
// Unreachable -> [] (EXIT)
// Call { target, unwind } -> [target] + optional [unwind]
// TailCall { } -> [] (EXIT)
// Assert { target, unwind } -> [target] + optional [unwind]
// Yield { resume, drop } -> [resume] + optional [drop]
// Drop { target, unwind } -> [target] + optional [unwind]
// FalseEdge { real_target, imaginary_target } -> [real_target] (imaginary for borrowck)

// Edge classification from terminator:
pub fn classify_edges(terminator: &TerminatorKind) -> Vec<(BlockId, EdgeType)> {
    match terminator {
        TerminatorKind::Goto { target } => vec![(*target, EdgeType::Fallthrough)],

        TerminatorKind::SwitchInt { targets, .. } => {
            targets.iter()
                .map(|(_, tgt)| (tgt, EdgeType::TrueBranch))
                .chain([(targets.otherwise(), EdgeType::FalseBranch)])
                .collect()
        }

        TerminatorKind::Return => vec![],  // Exit

        TerminatorKind::Call { target: Some(tgt), unwind, .. } => {
            let mut edges = vec![(*tgt, EdgeType::Call)];
            if let UnwindAction::Cleanup(cleanup) = unwind {
                edges.push((cleanup, EdgeType::Exception));
            }
            edges
        }

        // ... handle other variants
        _ => vec![],
    }
}
```

### petgraph DOT Export with Custom Labels

```rust
// Source: https://docs.rs/petgraph/latest/petgraph/dot/struct.Dot.html

use petgraph::{Graph, dot::{Dot, Config}};

type CFG = Graph<&str, &str>;

fn export_cfg_dot(cfg: &CFG) -> String {
    format!("{:?}", Dot::with_attr_getters(
        cfg,
        &[Config::EdgeNoLabel],
        // Edge attributes (color by type)
        &|_, (edge)| {
            match *edge.weight() {
                "true" => "color = green, label = \"T\"".to_string(),
                "false" => "color = red, label = \"F\"".to_string(),
                "fallthrough" => "color = black, style = dashed".to_string(),
                "exception" => "color = orange, label = \"panic\"".to_string(),
                _ => String::new(),
            }
        },
        // Node attributes (include block ID and source location)
        &|_, (node, weight)| {
            format!("label = \"{}\\n{}\"", node.index(), weight)
        },
    ))
}

// Output example:
// digraph {
//     0 [label = "0\nentry"]
//     1 [label = "1\ncheck"]
//     2 [label = "2\ndone"]
//     0 -> 1 [color = black, style = dashed]
//     1 -> 2 [color = green, label = "T"]
//     1 -> 2 [color = red, label = "F"]
// }
```

### JSON Export Schema

```rust
// CFG JSON export format for tool integration
#[derive(Serialize)]
pub struct CFGExport {
    pub function_id: String,
    pub function_name: String,
    pub entry: usize,
    pub exits: Vec<usize>,
    pub blocks: Vec<BlockExport>,
    pub edges: Vec<EdgeExport>,
}

#[derive(Serialize)]
pub struct BlockExport {
    pub id: usize,
    pub statements: Vec<String>,  // Human-readable
    pub terminator: String,
    pub source_location: SourceLocation,
}

#[derive(Serialize)]
pub struct EdgeExport {
    pub from: usize,
    pub to: usize,
    pub kind: String,  // "true_branch", "false_branch", etc.
}
```

### Charon Integration

```rust
// Run Charon and parse output
use std::process::Command;
use std::io::BufReader;

pub fn extract_mir(crate_path: &Path) -> Result<charon_lib::export::CrateData> {
    // Run charon binary
    let output = Command::new("charon")
        .current_dir(crate_path)
        .arg("--output-format=json")
        .output()?;

    // Parse JSON
    let reader = BufReader::new(output.stdout.as_slice());
    let crate_data: charon_lib::export::CrateData =
        serde_json::from_reader(reader)?;

    Ok(crate_data)
}

// Convert ULLBC to petgraph
pub fn ullbc_to_cfg(body: &charon_lib::llbc::Body) -> CFG {
    let mut graph = Graph::new();

    // Add blocks (ULLBC already has basic blocks)
    let block_indices: Vec<_> = body.blocks.iter()
        .map(|b| graph.add_node(b))
        .collect();

    // Add edges from terminators
    for (i, block) in body.blocks.iter().enumerate() {
        let from = block_indices[i];
        for (target, kind) in extract_successors(&block.terminator) {
            let to = block_indices[target];
            graph.add_edge(from, to, kind);
        }
    }

    graph
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Direct MIR traversal via rustc-driver | Charon ULLBC extraction | 2024-2025 | No need for nightly compiler, simplified API |
| Text-based MIR parsing | Structured JSON output | Charon 2024 | No fragile regex parsing, typed data structures |
| Single-language analysis | Multi-language via tree-sitter | 2020s | Single toolchain for polyglot codebases |

**Deprecated/outdated:**
- **rustc -Z dump-mir parsing**: Text output parsing is fragile. Use Charon for structured data.
- **Custom AST parsers**: tree-sitter provides production-ready parsers for 40+ languages.
- **StableMIR (as of 2025)**: Still incomplete. Charon is the working solution.

## Open Questions

1. **Charon binary distribution**
   - What we know: Charon is alpha, no stable release
   - What's unclear: How to distribute Charon binary with Mirage
   - Recommendation: Document Charon as external dependency, provide install instructions

2. **AST CFG for complex constructs**
   - What we know: Basic if/while/for is straightforward
   - What's unclear: match guards, async/await desugaring
   - Recommendation: Start with simple constructs, extend as needed

3. **Incremental CFG updates**
   - What we know: Database schema supports function-level updates
   - What's unclear: How to detect which functions changed for re-analysis
   - Recommendation: Use file modification time + hash comparison

## Sources

### Primary (HIGH confidence)

- [Charon: An Analysis Framework for Rust](https://arxiv.org/html/2410.18042v3) - Complete ULLBC/LLBC specification, API structure
- [rustc_middle::mir::TerminatorKind](https://doc.rust-lang.org/beta/nightly-rustc/rustc_middle/mir/enum.TerminatorKind.html) - Official MIR terminator variants
- [petgraph::dot::Dot documentation](https://docs.rs/petgraph/latest/petgraph/dot/struct.Dot.html) - DOT export API
- [Graphviz DOT Language](https://graphviz.org/doc/info/lang.html) - DOT format specification
- [tree-climber GitHub](https://github.com/bstee615/tree-climber) - Reference implementation for AST-based CFG

### Secondary (MEDIUM confidence)

- [AeneasVerif/charon GitHub](https://github.com/AeneasVerif/charon) - Charon source and usage
- [Miri: Practical Undefined Behavior Detection for Rust (2026)](https://research.ralfj.de/papers/2026-popl-miri.pdf) - MIR CFG fundamentals
- [Cornell CS6120: Global Analysis & SSA](https://www.cs.cornell.edu/courses/cs6120/2020fa/lesson/5/) - Dominance frontiers
- [Wikipedia: Dominator (graph theory)](https://en.wikipedia.org/wiki/Dominator_(graph_theory)) - Basic dominance definitions

### Tertiary (LOW confidence)

- Various StackOverflow discussions on CFG construction
- Academic papers on specific CFG algorithms (verify before implementing)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Charon and petgraph are well-documented with official sources
- Architecture: HIGH - Based on Charon paper which includes complete data structure definitions
- Pitfalls: HIGH - Verified against official MIR documentation

**Research date:** 2026-02-01
**Valid until:** 90 days (Charon is alpha, API may change)
