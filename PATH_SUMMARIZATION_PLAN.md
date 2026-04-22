# Milestone Plan: Path Summarization (Truth Proofs)

## 🎯 The Objective
To eliminate "Token Bloat" and LLM "Execution Guessing" by distilling raw MIR/AST data into compact, deterministic **Logic Proofs**.

---

## 🛠️ File-by-File Implementation Plan

### Phase 1: TDD Foundation
| File | Status | Description | Verification |
| :--- | :--- | :--- | :--- |
| `tests/path_summary_tdd.rs` | **Complete** | Define test cases for noise reduction, call extraction, and path linearization. | Verified (5/5 PASS) |

### Phase 2: Core Logic (The Truth Engine)
| File | Status | Description | Verification |
| :--- | :--- | :--- | :--- |
| `src/cfg/mod.rs` | **Complete** | Register the `summary` module. | `generalist` (Soundness Pass) |
| `src/cfg/summary.rs` | **Complete** | Implement `PathSummarizer` logic: filter noise, extract effects, format proofs. | `generalist` (Verified Fallbacks) |

### Phase 3: CLI & UX Integration
| File | Status | Description | Verification |
| :--- | :--- | :--- | :--- |
| `src/cli/mod.rs` | **Complete** | Add `--summarize` flag to `paths` command. | `generalist` (UX Pass) |
| `src/cli/cmds/paths.rs` | **N/A** | Logic integrated into `src/cli/mod.rs`. | `codebase_investigator` |

---

## 💡 Engineering References & Insights

### 1. Source of Insight: `ai/src/llm_bridge/context_compiler.rs`
The concept of "Graduated Facts" comes from your AI project. We are porting the logic that converts raw graph nodes into a list of confirmed behaviors that an LLM can trust without reading the source.

### 2. Source of Insight: `geometric_db_concept` (Voxelization)
We treat a MIR block as a **Logic Voxel**. Summarization is essentially **Level of Detail (LOD)** management for code context. We hide the "micro-logic" (memory management) and show the "macro-logic" (state changes and calls).

### 3. Connection to Magellan v9 Schema
This feature directly consumes the `statements` column added during our previous step. It validates that the ecosystem's "Single Source of Truth" is being utilized for reasoning.

---

## 🛡️ Subagent Verification Protocol

For every code change, a subagent must answer:
1.  **Memory Safety**: Are there any unsafe `unwrap()` calls or potential buffer overflows?
2.  **Logic Truth**: Does the summary accurately represent the underlying MIR, or is it losing critical logic signals?
3.  **Token Efficiency**: Does the summary actually reduce the token count compared to raw MIR?
4.  **Error Resilience**: Does it handle the fallback case where `statements` are missing (AST-only mode)?

---

**Next Step**: Implement `tests/path_summary_tdd.rs`.
