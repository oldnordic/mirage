use crate::cli::{resolve_db_path, Cli, OutputFormat, RiskArgs};
use crate::output;
use anyhow::Result;

pub fn risk(args: &RiskArgs, cli: &Cli) -> Result<()> {
    use crate::analysis::risk;

    let db_path = resolve_db_path(cli.db.clone())?;
    let db = super::open_db_or_exit(cli, &db_path);

    let function_id = match crate::storage::resolve_function_or_semantic(
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
