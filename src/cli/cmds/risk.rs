use crate::cli::{resolve_db_path, Cli, OutputFormat, RiskArgs};
use crate::output;
use anyhow::Result;

pub fn risk(args: &RiskArgs, cli: &Cli) -> Result<()> {
    use crate::analysis::risk;
    use crate::storage::MirageDb;

    let db_path = resolve_db_path(cli.db.clone())?;
    let db = match MirageDb::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::database_not_found(&db_path);
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Failed to open database: {}", e));
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    let function_id = match crate::cfg::resolve_function_name_with_file(
        &db,
        &args.function,
        args.file.as_deref(),
    ) {
        Ok(id) => id,
        Err(_) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::function_not_found(&args.function);
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Function '{}' not found", args.function));
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    let report = risk::compute_risk(&db, function_id, &args.function, args.file.as_deref())?;

    match cli.output {
        OutputFormat::Human => {
            println!("Risk Analysis: {}", report.function);
            if let Some(ref fp) = report.file_path {
                println!("  File: {}", fp);
            }
            println!("  Score: {:.1} ({})", report.risk_score, report.risk_level);
            println!(
                "  Cyclomatic complexity: {}",
                report.factors.cyclomatic_complexity
            );
            println!(
                "  Paths: {} ({} error)",
                report.factors.path_count, report.factors.error_path_count
            );
            println!("  Blocks: {}", report.factors.block_count);
            println!(
                "  Loops: {} (max nesting: {})",
                report.factors.loop_count, report.factors.max_nesting_depth
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
