# Charon MIR Integration & LLM-Optimization Plan

## Ecosystem Context & The Goal

The Code Intelligence ecosystem (Magellan, llmgrep, splice, Mirage) is designed to provide high-fidelity, deterministic context to LLMs, eliminating "hallucinations" and the need for LLMs to guess how code executes. 

**The Goal**: Move from "AI that reads text" to "AI that understands a verified logic graph."

---

## 1. LLM-Optimization Requirements (Context Compression)

To save tokens and remove guessing, all tools must prioritize **distilled facts** over raw data.

### A. Symbol-ID-First Communication
- **Constraint**: LLMs should prefer the **BLAKE3 symbol_id** (stable hash) over names. 
- **Workflow**: `magellan find` returns `id: a1b2`. All subsequent calls to `mirage`, `llmgrep`, and `splice` use `a1b2` for 100% deterministic lookup.

### B. Path Summarization (Mirage)
- **Problem**: Raw MIR/CFG paths are too long.
- **Solution**: Mirage will implement a `--summarize` flag that converts a CFG path into a high-level logic sequence.
- **Example**: `[Entry] -> [Check Auth] -> [DB Write] -> [Return]` instead of raw block IDs.

### C. Token-Aware Context Snippets (llmgrep)
- **Constraint**: Implementation of `--context-budget <TOKENS>`.
- **Solution**: llmgrep will return only the most relevant lines of a function (e.g., the specific `match` arm) to keep the LLM's context window lean.

---

## 2. Advanced 4D Spatiotemporal Reasoning (The Vision)

Integrating concepts from the `geometric_db_concept` project (Dual Octrees, Minecraft Block Streaming) into the `geometric` backend.

### A. 4D Coordinate System (X, Y, Z, T)
- **X (Dominator Depth)**: Measures structural hierarchy.
- **Y (Loop Nesting)**: Measures iterative complexity.
- **Z (Branch Count)**: Measures decision density.
- **T (Time/Version)**: Measures evolution (Git commits) or execution flow (Trace history).

### B. Minecraft Block Streaming (Context Voxelization)
- **Concept**: Treat the codebase as a 3D/4D voxel world.
- **Streaming**: LLMs "stream" only the relevant "chunks" (spatial volumes of code) instead of loading entire files.
- **Efficiency**: Only load code that exists in a specific "complexity volume" (e.g., "all high-loop-nesting blocks in the auth module").

### C. Dual Octree Traversal
- **Algorithm**: Use O(log N) Dual Octree joins to find spatiotemporal correlations.
- **Query**: "Find all blocks with Z > 5 (high branching) that have intersected with a bug-fix commit (T) in the last month."

### D. Progression vs. Regression (Time Travel)
- **Regression**: Look back at previous CFG states to understand how a bug was introduced.
- **Progression**: Predictive pathfinding to see if a proposed code change will intersect with known failure "worldlines" in the future cone.

---

## 3. File-by-File Implementation Plan (Updated)

### 1. `Cargo.toml`
- Add `geographdb-core` (path-based) and `charon` support dependencies.
- Enable `geometric-backend` by default.

### 2. `src/mir/translator.rs` (New)
- **Task**: Map Charon LLBC to Mirage CFG.
- **Spatiotemporal**: Assign initial 3D (X,Y,Z) coordinates to blocks based on dominator/loop analysis.

### 3. `src/storage/sqlite_backend.rs` & `src/storage/geometric.rs`
- **SQLite**: Add `statements` (TEXT) and `symbol_id` (TEXT) columns.
- **Geometric**: Implement **4D Octree** support using `memoria_spatial_core`. Integrate the "Minecraft Block" voxelization for streaming context to LLMs.

### 4. `src/cli/cmds/index.rs` (New)
- **Task**: Orchestrate the full 4D indexing.
- **Step**: Run `charon` -> Translate -> Assign Spatial Coordinates -> Commit to Magellan DB.

---

## 4. The "Golden Rule" Proof
An LLM may only suggest a code change if it can provide a **"Mirage Proof"**:
> "I am changing line 45 because Mirage Path #8 (Regression T-3) proves that `user_ptr` becomes null if the `timeout` branch is taken."
