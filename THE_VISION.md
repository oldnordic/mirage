# The Vision: 4D Spatiotemporal Code Intelligence

This document outlines the revolutionary shift from **Text-Based AI Engineering** to **Verified Logic Navigation** using the Magellan-Mirage ecosystem.

## 1. The Core Problem: The "Hallucination Gap"

LLMs currently process code as text. This leads to three fundamental failures:
1.  **Guessing Execution**: LLMs assume they know the control flow but miss implicit drops, panics, and complex match desugaring.
2.  **Context Bloat**: To "understand" a function, an LLM often reads the entire file, wasting thousands of tokens on irrelevant boilerplate.
3.  **Ambiguity**: Searching for a name like `new()` in a large project returns dozens of results, forcing the LLM to guess which one is relevant.

## 2. The Solution: Spatiotemporal Logic Graphs

We treat the codebase as a **4D Voxel World** where logic is positioned in space-time.

### The 4D Coordinate System (X, Y, Z, T)
- **X: Dominator Depth (Structural Hierarchy)** — How deep is this block in the logic tree?
- **Y: Loop Nesting (Iterative Complexity)** — How many levels of loops must be traversed to reach this?
- **Z: Branch Count (Decision Density)** — How many decisions lead to this point?
- **T: Time / Version (Evolution)** — Which Git commit or execution trace does this state belong to?

### Minecraft-Style Block Streaming
Instead of reading files, the AI "streams" voxels of logic. If an AI is investigating a bug in the "Auth" module, it only requests blocks within a specific spatial volume (e.g., "high-complexity branches in the login flow"). 

**Gain**: 80-90% reduction in context window usage.

## 3. The Ecosystem Tools

- **Magellan**: The **SSoT (Single Source of Truth)**. It indexes symbols into a BLAKE3-hashed graph and maintains the 4D spatial index.
- **llmgrep**: The **Surgical Scalpel**. It uses HNSW vector search to find logic by "meaning" and extracts exact byte-spans within a specific token budget.
- **Mirage**: The **Truth Engine**. It extracts MIR (via Charon) to provide 100% accurate paths. It "proves" that a path is reachable or that code is dead.
- **splice**: The **Verified Editor**. It applies changes to exact byte-ranges confirmed by the graph, ensuring no "accidental" edits to surrounding code.

## 4. Why This Wins (The Gains)

### For the Developer
- **Zero-Bug Refactoring**: `splice` + `Mirage` ensures that a rename or move doesn't break hidden execution paths.
- **High-Fidelity Documentation**: Automatically generate docs that explain *behavior* (what the code does) rather than just *syntax* (what it looks like).

### For the LLM (The "AI Peer")
- **Anti-Hallucination**: The LLM is prohibited from suggesting a change unless it can cite a `path_id` or `block_id`.
- **Deterministic Communication**: Tools speak in `symbol_ids`. No more "Ambiguous symbol" errors.
- **Context Compression**: `mirage --summarize` turns 500 lines of code into 5 lines of logical "Truth."

## 5. How to Use the Future

### Discovery & Search
```bash
# Find the unique ID of a function via semantic search
llmgrep --db .magellan/magellan.db search --query "handle user logout" --output json

# Result: { "symbol_id": "b3f4...", "name": "logout" }
```

### High-Fidelity Indexing
```bash
# Generate the MIR-based Truth Graph
mirage index --project . --rust
```

### Truth-Based Reasoning
```bash
# Get a summarized logic path for a specific function ID
mirage paths --id b3f4... --summarize

# Result: [Entry] -> [Verify Session] -> [Drop SessionKey] -> [Redirect]
```

### Verified Editing
```bash
# Rename symbol across the whole graph with blast-zone safety
splice rename --id b3f4... --to "terminate_session" --verify-paths
```

---

**The Golden Rule**: An agent may only speak if it can reference a graph artifact. We don't guess code; we navigate reality.
