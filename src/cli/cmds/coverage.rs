use crate::cli::{resolve_db_path, Cli, CoverageArgs, OutputFormat};
use crate::output;
use anyhow::Result;

pub fn coverage(args: &CoverageArgs, cli: &Cli) -> Result<()> {
    use crate::cfg::load_cfg_from_db;
    use crate::storage::resolve_function_or_semantic;

    // Resolve database path
    let db_path = resolve_db_path(cli.db.clone())?;

    // Open database (honest open-error handling via shared helper)
    let db = super::open_db_or_exit(cli, &db_path);

    // Resolve function name/ID or semantic query to function_id (with optional file filter)
    let function_id = match resolve_function_or_semantic(
        &db,
        &args.function,
        args.semantic_query.as_deref(),
        args.file.as_deref(),
    ) {
        Ok(id) => id,
        Err(e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "FunctionNotFound",
                    &format!("{}", e),
                    output::E_CFG_ERROR,
                );
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("{}", e));
                output::info(&format!("Hint: {}", output::R_HINT_LIST_FUNCTIONS));
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    // Load CFG to map block IDs
    let cfg = match load_cfg_from_db(&db, function_id) {
        Ok(cfg) => cfg,
        Err(_e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "CgfLoadError",
                    &format!("Failed to load CFG for function '{}'", args.function),
                    output::E_CFG_ERROR,
                );
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!(
                    "Failed to load CFG for function '{}'",
                    args.function
                ));
                output::info("The function may be corrupted. Try re-running 'magellan watch'");
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    // Query coverage data (graceful fallback if tables absent)
    let coverage_rows: Vec<(usize, String, i64)> = db.conn().ok().map_or_else(Vec::new, |conn| {
        let sql = "SELECT bb.id, bb.kind, COALESCE(bc.hit_count, 0) \
                   FROM cfg_blocks bb \
                   LEFT JOIN cfg_block_coverage bc ON bb.id = bc.block_id \
                   WHERE bb.function_id = ?1 \
                   ORDER BY bb.byte_start";
        let mut stmt = match conn.prepare(sql) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map([function_id], |row| {
            Ok((
                row.get::<_, i64>(0)? as usize,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => Vec::new(),
        }
    });

    // Build a lookup from db_block_id to graph node index (via BasicBlock.id)
    let db_id_to_graph_id: std::collections::HashMap<i64, usize> = cfg
        .node_indices()
        .filter_map(|idx| {
            cfg.node_weight(idx)
                .and_then(|b| b.db_id.map(|db_id| (db_id, b.id)))
        })
        .collect();

    match cli.output {
        OutputFormat::Human => {
            println!("Coverage for function '{}'", args.function);
            println!("{}", "=".repeat(60));
            if coverage_rows.is_empty() {
                println!("No coverage data available.");
                println!("Hint: Run tests with 'cargo test' to generate coverage.");
            } else {
                for (db_id, kind, hits) in &coverage_rows {
                    let graph_id = db_id_to_graph_id
                        .get(&(*db_id as i64))
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "?".to_string());
                    println!(
                        "  Block {:>3} (graph #{}, kind={:>8}): {:>6} hits",
                        db_id, graph_id, kind, hits
                    );
                }
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            #[derive(serde::Serialize)]
            struct CoverageEntry {
                block_id: usize,
                graph_id: Option<usize>,
                kind: String,
                hit_count: i64,
            }
            let entries: Vec<CoverageEntry> = coverage_rows
                .iter()
                .map(|(db_id, kind, hits)| CoverageEntry {
                    block_id: *db_id,
                    graph_id: db_id_to_graph_id.get(&(*db_id as i64)).copied(),
                    kind: kind.to_string(),
                    hit_count: *hits,
                })
                .collect();
            let response = output::JsonResponse::new(serde_json::json!({
                "function": args.function,
                "coverage": entries,
            }));
            match cli.output {
                OutputFormat::Json => println!("{}", response.to_json()),
                OutputFormat::Pretty => println!("{}", response.to_pretty_json()),
                _ => unreachable!(),
            }
        }
    }

    Ok(())
}
