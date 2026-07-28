//! Scalable hotspot ranking over large symbol sets.
//!
//! Ranks functions by a composite risk score without materializing the
//! whole candidate set (or an unbounded path-enumeration tree) in memory:
//!
//! - Candidates are streamed from SQLite in bounded chunks using keyset
//!   pagination (`WHERE id > ? ORDER BY id LIMIT ?`), so peak RSS is
//!   independent of the total symbol count.
//! - Each symbol's score is computed independently per chunk, so chunk
//!   boundaries cannot change any score.
//! - A bounded top-K min-heap across chunks yields the exact global
//!   ranking — identical to scoring everything in a single pass, sorting
//!   by `(score desc, insertion seq asc)`, and truncating to `top`.
//!
//! ## Inter-procedural mode (default)
//!
//! The previous implementation enumerated call-graph execution paths from
//! the entry symbol via `magellan::CodeGraph::enumerate_paths`. That DFS
//! has no work budget and no early exit once `max_paths` is reached — the
//! cap only limits what is *stored*, not what is *traversed* — so on a
//! ~5k-symbol / ~70k-edge call graph it explores a combinatorial number of
//! walks, issuing one SQLite query per visited node. Runtime was
//! effectively unbounded (observed: 10h+ CPU on the llama-rs DB) while RSS
//! stayed moderate, i.e. a CPU blow-up, not an OOM.
//!
//! The replacement keeps the same score *shape* (`traffic * 1.0 +
//! dominance * 2.0`) but derives both factors from bounded queries:
//!
//! - `path_count` (traffic proxy): symbol-level call-graph degree
//!   (`fan_in + fan_out`), resolved through magellan's
//!   `Symbol -CALLER-> Call -CALLS-> Symbol` edge pairs.
//! - `dominance_factor`: size of the symbol's strongly connected
//!   component, computed in-memory with `petgraph::algo::tarjan_scc` over
//!   the symbol-level call edges (one query, linear algorithm).
//!
//! ## Intra-procedural mode (`--intra-procedural`)
//!
//! Per-function CFG analysis (bounded path enumeration via
//! [`PathLimits::quick_analysis`]) exactly as before, but streamed in
//! chunks through the same top-K heap.

use anyhow::Result;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

#[cfg(feature = "backend-sqlite")]
use rusqlite::Connection;

/// Default number of candidate symbols processed per chunk.
///
/// Rationale: per-symbol state is a few hundred bytes (name, path, score),
/// so even 10k-entry chunks would be cheap on the accumulation side; the
/// bound that matters is per-chunk query size and transient CFG/score
/// buffers. 500 keeps `IN`-list / keyset queries efficient (a handful of
/// round trips for a ~5k-symbol DB), bounds transient per-chunk buffers to
/// well under 1 MiB, and keeps peak RSS dominated by data we cannot chunk
/// (the SQLite page cache), not by our accumulation.
pub const DEFAULT_CHUNK_SIZE: usize = 500;

/// A single scored hotspot candidate (analysis-layer twin of the CLI's
/// `HotspotEntry`).
#[derive(Debug, Clone, PartialEq)]
pub struct HotspotScore {
    pub function: String,
    pub risk_score: f64,
    /// Traffic through the symbol: call-graph degree (`fan_in + fan_out`)
    /// in inter-procedural mode, bounded CFG path count in
    /// intra-procedural mode.
    pub path_count: usize,
    /// Coupling indicator: SCC size in inter-procedural mode, 1.0 in
    /// intra-procedural mode.
    pub dominance_factor: f64,
    /// CFG block count (intra-procedural mode only; 0 otherwise).
    pub complexity: usize,
    pub file_path: String,
}

/// Result of a ranked hotspot scan.
#[derive(Debug, Clone, Default)]
pub struct RankedHotspots {
    /// Top-`top` entries, sorted by `(risk_score desc, seq asc)` — exactly
    /// the ordering a single-pass stable sort + truncate would produce.
    pub entries: Vec<HotspotScore>,
    /// Total candidate symbols examined across all chunks.
    pub total_functions: usize,
}

/// Heap item carrying the global insertion sequence number so ties break
/// exactly like a stable sort over the single-pass stream.
struct Scored {
    score: f64,
    seq: usize,
    entry: HotspotScore,
}

impl PartialEq for Scored {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score && self.seq == other.seq
    }
}
impl Eq for Scored {}

impl PartialOrd for Scored {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Scored {
    /// `BinaryHeap` is a max-heap; we want the *worst* retained entry on
    /// top so it is evicted first. Worst = lowest score; on ties, the
    /// latest insertion (largest seq), matching stable-sort truncation.
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .score
            .total_cmp(&self.score)
            .then_with(|| self.seq.cmp(&other.seq))
    }
}

/// Bounded top-K accumulator with exact global ranking semantics.
struct TopK {
    heap: BinaryHeap<Scored>,
    cap: usize,
    seq: usize,
}

impl TopK {
    fn new(cap: usize) -> Self {
        Self {
            // Pre-allocate modestly: `top` may be usize::MAX in tests.
            heap: BinaryHeap::with_capacity(cap.saturating_add(1).min(1024)),
            cap,
            seq: 0,
        }
    }

    fn push(&mut self, entry: HotspotScore) {
        let item = Scored {
            score: entry.risk_score,
            seq: self.seq,
            entry,
        };
        self.seq += 1;
        if self.cap == 0 {
            return;
        }
        self.heap.push(item);
        if self.heap.len() > self.cap {
            self.heap.pop();
        }
    }

    /// Drain into the final ranking: score desc, ties by insertion order.
    fn into_sorted(mut self) -> Vec<HotspotScore> {
        let mut items: Vec<Scored> = Vec::with_capacity(self.heap.len());
        while let Some(item) = self.heap.pop() {
            items.push(item);
        }
        items.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.seq.cmp(&b.seq)));
        items.into_iter().map(|i| i.entry).collect()
    }
}

/// Symbol-level call-graph adjacency: fan-in/fan-out per symbol and SCC
/// sizes, computed once from bounded SQL + Tarjan's algorithm.
struct CallGraphMetrics {
    /// `fan_in + fan_out` per symbol entity id.
    degree: HashMap<i64, usize>,
    /// SCC size per symbol entity id (1 for acyclic singletons).
    scc_size: HashMap<i64, f64>,
}

#[cfg(feature = "backend-sqlite")]
fn load_call_graph_metrics(conn: &Connection) -> Result<CallGraphMetrics> {
    // Magellan models a call as Symbol -CALLER-> CallSite -CALLS-> Symbol;
    // collapse the 2-hop chain into symbol-level caller->callee edges.
    let mut stmt = conn.prepare(
        "SELECT e1.from_id, e2.to_id \
         FROM graph_edges e1 \
         JOIN graph_edges e2 ON e1.to_id = e2.from_id AND e2.edge_type = 'CALLS' \
         WHERE e1.edge_type = 'CALLER' \
         GROUP BY e1.from_id, e2.to_id",
    )?;
    let edges: Vec<(i64, i64)> = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut degree: HashMap<i64, usize> = HashMap::new();
    let mut node_ids: HashMap<i64, petgraph::graph::NodeIndex> = HashMap::new();
    let mut graph = petgraph::graph::DiGraph::<i64, ()>::new();

    for &(from, to) in &edges {
        let from_idx = *node_ids.entry(from).or_insert_with(|| graph.add_node(from));
        let to_idx = *node_ids.entry(to).or_insert_with(|| graph.add_node(to));
        graph.add_edge(from_idx, to_idx, ());
        *degree.entry(from).or_insert(0) += 1;
        *degree.entry(to).or_insert(0) += 1;
    }

    let mut scc_size: HashMap<i64, f64> = HashMap::new();
    for component in petgraph::algo::tarjan_scc(&graph) {
        let size = component.len() as f64;
        for idx in component {
            if let Some(&symbol_id) = graph.node_weight(idx) {
                scc_size.insert(symbol_id, size);
            }
        }
    }

    Ok(CallGraphMetrics { degree, scc_size })
}

/// Inter-procedural hotspot ranking, streamed in chunks.
///
/// Scores every `Symbol` entity whose call-graph degree is at least
/// `min_degree` with `degree * 1.0 + scc_size * 2.0`. Symbols with no call
/// edges have degree 0 and are skipped unless `min_degree == 0`.
#[cfg(feature = "backend-sqlite")]
pub fn rank_inter_procedural(
    conn: &Connection,
    top: usize,
    chunk_size: usize,
    min_degree: usize,
) -> Result<RankedHotspots> {
    let metrics = load_call_graph_metrics(conn)?;
    let chunk_size = chunk_size.max(1);

    let mut topk = TopK::new(top);
    let mut total_functions = 0usize;
    let mut last_id: i64 = 0;

    loop {
        let mut stmt = conn.prepare(
            "SELECT id, name, COALESCE(file_path, '') \
             FROM graph_entities \
             WHERE kind = 'Symbol' AND id > ?1 \
             ORDER BY id LIMIT ?2",
        )?;
        let chunk: Vec<(i64, String, String)> = stmt
            .query_map(rusqlite::params![last_id, chunk_size as i64], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if chunk.is_empty() {
            break;
        }
        last_id = chunk.last().map(|(id, _, _)| *id).unwrap_or(last_id);

        for (symbol_id, name, file_path) in chunk {
            total_functions += 1;
            let degree = metrics.degree.get(&symbol_id).copied().unwrap_or(0);
            if degree < min_degree {
                continue;
            }
            let dominance = metrics.scc_size.get(&symbol_id).copied().unwrap_or(1.0);
            let risk_score = degree as f64 * 1.0 + dominance * 2.0;
            topk.push(HotspotScore {
                function: name,
                risk_score,
                path_count: degree,
                dominance_factor: dominance,
                complexity: 0,
                file_path,
            });
        }
    }

    Ok(RankedHotspots {
        entries: topk.into_sorted(),
        total_functions,
    })
}

/// Intra-procedural hotspot ranking, streamed in chunks.
///
/// Per function with CFG blocks: bounded path enumeration
/// ([`PathLimits::quick_analysis`]) plus block count, scored as
/// `path_count * 0.5 + complexity * 0.1` (unchanged from the pre-chunking
/// implementation).
#[cfg(feature = "backend-sqlite")]
pub fn rank_intra_procedural(
    conn: &Connection,
    top: usize,
    chunk_size: usize,
    min_paths: usize,
) -> Result<RankedHotspots> {
    use crate::cfg::{enumerate_paths_with_context, EnumerationContext, PathLimits};
    use crate::storage::load_cfg_from_db_with_conn;

    let chunk_size = chunk_size.max(1);
    let mut topk = TopK::new(top);
    let mut total_functions = 0usize;
    let mut last_id: i64 = 0;

    loop {
        // Keyset-paginate over distinct function ids present in cfg_blocks.
        let chunk: Vec<(i64, String, String)> = {
            let mut stmt = conn.prepare(
                "SELECT cb.function_id, ge.name, COALESCE(ge.file_path, '') \
                 FROM cfg_blocks cb \
                 JOIN graph_entities ge ON cb.function_id = ge.id \
                 WHERE cb.function_id > ?1 \
                 GROUP BY cb.function_id \
                 ORDER BY cb.function_id LIMIT ?2",
            )?;
            let mapped = stmt.query_map(rusqlite::params![last_id, chunk_size as i64], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            let mut chunk_rows = Vec::new();
            for row in mapped {
                chunk_rows.push(row?);
            }
            chunk_rows
        };

        if chunk.is_empty() {
            break;
        }
        last_id = chunk.last().map(|(id, _, _)| *id).unwrap_or(last_id);

        // Drop the statement borrow before running per-function CFG queries
        // against the same connection.
        for (func_id, func_name, file_path) in chunk {
            total_functions += 1;
            if let Ok(cfg) = load_cfg_from_db_with_conn(conn, func_id) {
                let ctx = EnumerationContext::new(&cfg);
                let limits = PathLimits::quick_analysis();
                let paths = enumerate_paths_with_context(&cfg, &limits, &ctx);

                let path_count = paths.len();
                if path_count < min_paths {
                    continue;
                }

                let complexity = cfg.node_count();
                let dominance = 1.0;
                let risk_score = path_count as f64 * 0.5 + complexity as f64 * 0.1;

                topk.push(HotspotScore {
                    function: func_name,
                    risk_score,
                    path_count,
                    dominance_factor: dominance,
                    complexity,
                    file_path,
                });
            }
        }
    }

    Ok(RankedHotspots {
        entries: topk.into_sorted(),
        total_functions,
    })
}

#[cfg(all(test, feature = "backend-sqlite"))]
mod tests {
    use super::*;

    /// Build a synthetic magellan-shaped DB: `n` Symbol entities plus call
    /// sites wired as `Symbol -CALLER-> Call -CALLS-> Symbol`.
    ///
    /// Deterministic wiring with varied degrees and one non-trivial SCC:
    /// - symbols 0..3 form a cycle (SCC size 3...4 depending on n),
    /// - symbol i (i >= 4) calls symbols (i*7)%n and (i*13)%n,
    /// - some symbols (high ids) have no edges at all (degree 0).
    fn make_synthetic_db(n: usize) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE graph_entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT,
                data TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE graph_edges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_id INTEGER NOT NULL,
                to_id INTEGER NOT NULL,
                edge_type TEXT NOT NULL,
                data TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE cfg_blocks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                function_id INTEGER NOT NULL,
                kind TEXT NOT NULL,
                terminator TEXT,
                byte_start INTEGER, byte_end INTEGER,
                start_line INTEGER, start_col INTEGER,
                end_line INTEGER, end_col INTEGER
            );",
        )
        .unwrap();

        for i in 0..n {
            conn.execute(
                "INSERT INTO graph_entities (kind, name, file_path) VALUES ('Symbol', ?1, ?2)",
                rusqlite::params![format!("sym_{i}"), format!("src/file_{}.rs", i % 7)],
            )
            .unwrap();
        }

        let mut edge_id = 0i64;
        let mut add_call = |conn: &Connection, caller: i64, callee: i64| {
            edge_id += 1;
            let call_site_id = 1_000_000 + edge_id;
            conn.execute(
                "INSERT INTO graph_entities (id, kind, name) VALUES (?1, 'Call', ?2)",
                rusqlite::params![call_site_id, format!("call_{edge_id}")],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (?1, ?2, 'CALLER')",
                rusqlite::params![caller, call_site_id],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO graph_edges (from_id, to_id, edge_type) VALUES (?1, ?2, 'CALLS')",
                rusqlite::params![call_site_id, callee],
            )
            .unwrap();
        };

        if n >= 4 {
            // Cycle among symbols 1..=3 (entity ids are 1-based).
            add_call(&conn, 1, 2);
            add_call(&conn, 2, 3);
            add_call(&conn, 3, 1);
        }
        for i in 4..n {
            let caller = (i + 1) as i64; // entity ids are 1-based
                                         // Leave the last 10% of symbols degree-0.
            if i >= n - n / 10 {
                break;
            }
            add_call(&conn, caller, ((i * 7) % n + 1) as i64);
            add_call(&conn, caller, ((i * 13) % n + 1) as i64);
        }

        conn
    }

    #[test]
    fn chunked_matches_single_pass_inter_procedural() {
        // 257 symbols with chunk size 16 forces 17 chunks (>1 chunk required).
        let conn = make_synthetic_db(257);
        let chunked = rank_inter_procedural(&conn, 20, 16, 1).unwrap();
        let single = rank_inter_procedural(&conn, 20, usize::MAX, 1).unwrap();
        assert_eq!(chunked.total_functions, single.total_functions);
        assert_eq!(chunked.total_functions, 257);
        assert_eq!(chunked.entries, single.entries);
        assert!(!chunked.entries.is_empty());
    }

    #[test]
    fn chunk_of_one_matches_single_pass() {
        let conn = make_synthetic_db(64);
        let chunked = rank_inter_procedural(&conn, 10, 1, 1).unwrap();
        let single = rank_inter_procedural(&conn, 10, usize::MAX, 1).unwrap();
        assert_eq!(chunked.entries, single.entries);
        assert_eq!(chunked.entries.len(), 10);
    }

    #[test]
    fn ranking_is_score_desc_with_stable_ties() {
        let conn = make_synthetic_db(128);
        let ranked = rank_inter_procedural(&conn, 50, 8, 1).unwrap();
        for w in ranked.entries.windows(2) {
            assert!(
                w[0].risk_score >= w[1].risk_score,
                "scores must be non-increasing: {:?} vs {:?}",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn min_degree_filters_isolated_symbols() {
        let conn = make_synthetic_db(64);
        let all = rank_inter_procedural(&conn, usize::MAX, 16, 0).unwrap();
        let connected = rank_inter_procedural(&conn, usize::MAX, 16, 1).unwrap();
        assert!(all.entries.len() >= connected.entries.len());
        assert!(connected.entries.iter().all(|e| e.path_count >= 1));
    }

    #[test]
    fn scc_members_get_higher_dominance() {
        let conn = make_synthetic_db(32);
        let ranked = rank_inter_procedural(&conn, usize::MAX, 4, 1).unwrap();
        let scc_member = ranked
            .entries
            .iter()
            .find(|e| e.function == "sym_1")
            .expect("sym_1 participates in the 3-cycle");
        assert!(
            scc_member.dominance_factor >= 3.0,
            "sym_1 is in a 3-cycle, dominance should reflect SCC size: {:?}",
            scc_member
        );
    }

    #[test]
    fn topk_heap_exact_tie_break() {
        // Direct heap semantics: equal scores keep insertion order, and the
        // *latest* of the tied losers is evicted first.
        let mut topk = TopK::new(3);
        let mk = |name: &str, score: f64| HotspotScore {
            function: name.to_string(),
            risk_score: score,
            path_count: 0,
            dominance_factor: 1.0,
            complexity: 0,
            file_path: String::new(),
        };
        for (name, score) in [("a", 5.0), ("b", 5.0), ("c", 5.0), ("d", 5.0), ("e", 7.0)] {
            topk.push(mk(name, score));
        }
        let names: Vec<String> = topk.into_sorted().into_iter().map(|e| e.function).collect();
        assert_eq!(names, vec!["e", "a", "b"]);
    }

    #[test]
    fn empty_db_yields_empty_ranking() {
        let conn = make_synthetic_db(0);
        let ranked = rank_inter_procedural(&conn, 20, 16, 1).unwrap();
        assert_eq!(ranked.total_functions, 0);
        assert!(ranked.entries.is_empty());
    }

    /// Add a minimal CFG for `function_id`: entry --fallthrough--> return
    /// (1 path), or with a conditional branch (2 paths) when `branch`.
    fn add_cfg(conn: &Connection, function_id: i64, branch: bool) {
        conn.execute(
            "INSERT INTO cfg_blocks \
             (function_id, kind, terminator, byte_start, byte_end) \
             VALUES (?1, 'entry', ?2, 0, 1)",
            rusqlite::params![
                function_id,
                if branch { "conditional" } else { "fallthrough" }
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO cfg_blocks \
             (function_id, kind, terminator, byte_start, byte_end) \
             VALUES (?1, 'return', 'return', 1, 2)",
            rusqlite::params![function_id],
        )
        .unwrap();
        if branch {
            conn.execute(
                "INSERT INTO cfg_blocks \
                 (function_id, kind, terminator, byte_start, byte_end) \
                 VALUES (?1, 'return', 'return', 2, 3)",
                rusqlite::params![function_id],
            )
            .unwrap();
        }
    }

    #[test]
    fn chunked_matches_single_pass_intra_procedural() {
        // 37 functions with CFGs, chunk size 5 forces 8 chunks.
        let conn = make_synthetic_db(37);
        for i in 1..=37i64 {
            add_cfg(&conn, i, i % 3 == 0);
        }
        let chunked = rank_intra_procedural(&conn, 15, 5, 1).unwrap();
        let single = rank_intra_procedural(&conn, 15, usize::MAX, 1).unwrap();
        assert_eq!(chunked.total_functions, 37);
        assert_eq!(chunked.total_functions, single.total_functions);
        assert_eq!(chunked.entries, single.entries);
        assert!(!chunked.entries.is_empty());
        // Branching functions (2 paths, 3 blocks) must outrank linear ones
        // (1 path, 2 blocks): 1.0 + 0.3 vs 0.5 + 0.2.
        assert!(chunked
            .entries
            .windows(2)
            .all(|w| w[0].risk_score >= w[1].risk_score));
    }

    #[test]
    fn intra_min_paths_filters() {
        let conn = make_synthetic_db(10);
        for i in 1..=10i64 {
            add_cfg(&conn, i, i % 2 == 0);
        }
        let ranked = rank_intra_procedural(&conn, usize::MAX, 3, 2).unwrap();
        assert!(ranked.entries.iter().all(|e| e.path_count >= 2));
        assert_eq!(ranked.entries.len(), 5);
    }

    #[test]
    fn top_zero_yields_no_entries_but_counts() {
        let conn = make_synthetic_db(32);
        let ranked = rank_inter_procedural(&conn, 0, 16, 1).unwrap();
        assert_eq!(ranked.total_functions, 32);
        assert!(ranked.entries.is_empty());
    }
}
