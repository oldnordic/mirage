use crate::cli::{resolve_db_path, Cli, OutputFormat, StatsArgs};
use crate::output;
use anyhow::Result;

pub fn stats(_args: &StatsArgs, cli: &Cli) -> Result<()> {
    use crate::analysis::stats;
    use crate::storage::MirageDb;

    let db_path = resolve_db_path(cli.db.clone())?;
    let db = match MirageDb::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = if std::path::Path::new(&db_path).exists() {
                    output::JsonError::database_open_failed(&db_path, &e.to_string())
                } else {
                    output::JsonError::database_not_found(&db_path)
                };
                let wrapper = output::JsonResponse::new(error);
                match cli.output {
                    OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                    _ => println!("{}", wrapper.to_json()),
                }
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Failed to open database: {}", e));
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    let report = stats::gather_stats(&db)?;

    match cli.output {
        OutputFormat::Human => {
            println!("Code Statistics:");
            println!(
                "  Functions: {} total, {} with CFG, {} without",
                report.total_functions, report.functions_with_cfg, report.functions_without_cfg
            );
            println!(
                "  Blocks: {} total, avg {:.1} per function, max {} ({})",
                report.total_blocks,
                report.avg_blocks_per_function,
                report.max_blocks,
                report.max_blocks_function.as_deref().unwrap_or("N/A")
            );
            println!("  Paths: {} cached", report.total_paths);
            println!("  Dead code blocks: {}", report.dead_code_count);
            println!("  Coverage gaps: {}", report.coverage_gap_count);
            println!(
                "  Complexity: {} low, {} medium, {} high, {} critical",
                report.complexity_distribution.low,
                report.complexity_distribution.medium,
                report.complexity_distribution.high,
                report.complexity_distribution.critical
            );
        }
        OutputFormat::Json => {
            let response = output::JsonResponse::new(&report);
            println!("{}", response.to_json());
        }
        OutputFormat::Pretty => {
            let response = output::JsonResponse::new(&report);
            println!("{}", response.to_pretty_json());
        }
    }

    Ok(())
}
