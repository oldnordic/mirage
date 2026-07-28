use crate::cli::responses::*;
use crate::cli::{resolve_db_path, Cli, HotspotsArgs, OutputFormat};
use crate::output;
use anyhow::Result;

pub fn hotspots(args: &HotspotsArgs, cli: &Cli) -> Result<()> {
    #[cfg(feature = "backend-sqlite")]
    use crate::analysis::hotspots::{rank_inter_procedural, rank_intra_procedural, RankedHotspots};

    let db_path = resolve_db_path(cli.db.clone())?;

    // Open Mirage database for intra-procedural analysis
    // (honest open-error handling via shared helper)
    #[cfg(feature = "backend-sqlite")]
    let mut db = super::open_db_or_exit(cli, &db_path);

    let min_threshold = args.min_paths.unwrap_or(1);
    let chunk_size = args.chunk_size.max(1);

    // `--intra-procedural` forces intra-procedural analysis; otherwise the
    // default inter-procedural mode runs (falling back to intra-procedural
    // when the DB carries no call-graph edges).
    let use_inter = args.inter_procedural && !args.intra_procedural;

    #[cfg(feature = "backend-sqlite")]
    let (ranked, mode): (RankedHotspots, &str) = {
        let is_sqlite = db.is_sqlite();
        let conn = db.conn_mut()?;
        let mut mode = "intra-procedural";
        let mut ranked = RankedHotspots::default();

        if use_inter {
            match rank_inter_procedural(conn, args.top, chunk_size, min_threshold) {
                Ok(r) if !r.entries.is_empty() => {
                    ranked = r;
                    mode = "inter-procedural";
                }
                Ok(_) => {
                    output::warn(
                        "No call-graph edges found, falling back to intra-procedural analysis",
                    );
                }
                Err(e) => {
                    output::warn(&format!(
                        "Inter-procedural analysis unavailable ({e}), using intra-procedural analysis"
                    ));
                }
            }
        }

        if ranked.entries.is_empty() && mode == "intra-procedural" && is_sqlite {
            ranked = rank_intra_procedural(conn, args.top, chunk_size, min_threshold)?;
        }

        (ranked, mode)
    };

    #[cfg(not(feature = "backend-sqlite"))]
    let (ranked, mode): (crate::analysis::hotspots::RankedHotspots, &str) = {
        let _ = &db_path;
        (Default::default(), "intra-procedural")
    };

    let hotspots: Vec<HotspotEntry> = ranked
        .entries
        .iter()
        .map(|e| HotspotEntry {
            function: e.function.clone(),
            risk_score: e.risk_score,
            path_count: e.path_count,
            dominance_factor: e.dominance_factor,
            complexity: e.complexity,
            file_path: e.file_path.clone(),
        })
        .collect();

    let response = HotspotsResponse {
        entry_point: args.entry.clone(),
        total_functions: ranked.total_functions,
        hotspots: hotspots.clone(),
        mode: mode.to_string(),
    };

    match cli.output {
        OutputFormat::Human => {
            let mut output = String::new();
            output.push_str(&format!(
                "Hotspots Analysis (entry: {})\n",
                response.entry_point
            ));
            output.push('\n');

            // Add helpful hint if 0 functions found with intra-procedural mode
            if response.total_functions == 0 && response.mode == "intra-procedural" {
                output.push_str("No functions found. This may be because:\n");
                output.push_str("  1. The database hasn't been indexed yet\n");
                output.push_str("  2. You need to run: magellan watch --db <path>\n");
                output.push('\n');
            }

            output.push_str(&format!(
                "Found {} hotspots out of {} functions\n",
                hotspots.len(),
                response.total_functions
            ));
            output.push('\n');

            for (i, hotspot) in hotspots.iter().enumerate() {
                output.push_str(&format!(
                    "{}. {} (risk: {:.1})\n",
                    i + 1,
                    hotspot.function,
                    hotspot.risk_score
                ));
                if args.verbose {
                    output.push_str(&format!("   Paths: {}\n", hotspot.path_count));
                    output.push_str(&format!("   Dominance: {:.1}\n", hotspot.dominance_factor));
                    output.push_str(&format!("   Complexity: {}\n", hotspot.complexity));
                }
            }
            let processed = output::apply_token_budget(output, args.tokens);
            print!("{}", processed);
        }
        OutputFormat::Json => {
            let json_str = serde_json::to_string(&response).unwrap_or_default();
            let processed = output::apply_token_budget(json_str, args.tokens);
            let tokens_est = processed.len() / 4;
            let truncated = args.tokens.is_some_and(|t| t > 0 && tokens_est > t);
            let wrapper = output::JsonResponse::new(response)
                .with_tokens(tokens_est)
                .with_truncated(truncated);
            println!("{}", wrapper.to_json());
        }
        OutputFormat::Pretty => {
            let json_str = serde_json::to_string_pretty(&response).unwrap_or_default();
            let processed = output::apply_token_budget(json_str, args.tokens);
            let tokens_est = processed.len() / 4;
            let truncated = args.tokens.is_some_and(|t| t > 0 && tokens_est > t);
            let wrapper = output::JsonResponse::new(response)
                .with_tokens(tokens_est)
                .with_truncated(truncated);
            println!("{}", wrapper.to_pretty_json());
        }
    }

    Ok(())
}
