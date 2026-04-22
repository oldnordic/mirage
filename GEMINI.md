# Mirage - Path-Aware Code Intelligence Engine

Mirage is a high-fidelity control-flow analysis engine for Rust, designed to provide "truth-based" reasoning about code behavior by analyzing graphs instead of text. It is part of a larger ecosystem for code intelligence designed for LLMs.

## Code Intelligence Ecosystem

The ecosystem consists of three main tools that work together using a shared Magellan database (typically `.magellan/magellan.db`):

1.  **Magellan**: The foundation. It watches the codebase and builds the code graph (symbols, references, call graphs).
2.  **llmgrep**: The search layer. Fast, deterministic search for symbols and references within the Magellan database.
3.  **Mirage**: The reasoning layer. Analyzes CFGs to enumerate execution paths, find dead code, and compute dominance.

---

## The LLM Workflow: From Discovery to Truth

For an LLM agent, the workflow is designed to eliminate "guessing" by materializing facts from the code graph.

### 1. Discovery (Magellan)
Locate the general area of interest or identify project-wide metrics.
```bash
# Get project statistics and schema info
magellan status --db .magellan/magellan.db

# Find a symbol by name (useful when you only have a partial name)
magellan find --db .magellan/magellan.db --name "process_data"

# List symbols in a specific file
magellan query --db .magellan/magellan.db --file "src/main.rs"
```

### 2. Search & Context (llmgrep)
Extract exact byte-spans and filter symbols programmatically.
```bash
# Search for a symbol with JSON output for precise parsing
llmgrep --db .magellan/magellan.db search --query "MyStruct" --output json

# Find all references (callers) of a function
llmgrep --db .magellan/magellan.db search --query "my_func" --mode references
```

### 3. Reasoning & Verification (Mirage)
Once a target is found, materialize its actual behavior.
```bash
# Enumerate all possible execution paths (Truth Engine)
mirage --db .magellan/magellan.db paths --function "my_crate::my_func" --output json

# Perform impact analysis (Blast Zone) to see what a change affects
mirage --db .magellan/magellan.db blast-zone --function "my_crate::my_func" --block-id 0

# Check for unreachable code that tree-sitter might have missed
mirage --db .magellan/magellan.db unreachable --within-functions
```

---

## Core Mandates & Invariants

- **The Golden Rule**: An agent may only speak if it can reference a graph artifact (e.g., `path_id`, `block_id`).
- **No Hallucinations**: If a path is not in the CFG, it does not exist in the code's logic.
- **Source of Truth**: Always use `.magellan/magellan.db` as the primary database.
- **Ambiguity Handling**: All tools return explicit `Ambiguous` errors if a symbol cannot be uniquely identified. Use Fully Qualified Names (FQN) to disambiguate.

## Advanced MIR Integration (Planned)

The project is moving toward **Charon-based MIR extraction** for Rust. This will replace the current AST-based (tree-sitter) extraction with compiler-verified CFGs, providing even higher fidelity for:
- Implicit `Drop` and `panic!` paths.
- Accurate `match` and `if let` desugaring.
- Precise lifetime-aware control flow.

Use the `index` command (once implemented) to trigger high-fidelity MIR indexing.
