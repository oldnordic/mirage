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

    // Resolve the function's file path: prefer the explicit --file arg, else
    // look it up from the graph. Needed for the git-churn factor and display.
    let resolved_file = args
        .file
        .clone()
        .or_else(|| db.get_function_file(function_id));

    // Opt-in git-churn factor: count commits touching the file in the window.
    let churn_commits = if args.churn {
        risk::resolve_churn(resolved_file.as_deref(), args.churn_days)
    } else {
        None
    };

    // Both opt-in factors stay off unless explicitly requested, so default
    // output stays in lockstep with `mirage suggest` (P2 agreement).
    let report = risk::compute_risk_with_factors(
        &db,
        function_id,
        &args.function,
        resolved_file.as_deref(),
        args.coverage,
        churn_commits,
    )?;

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
            if report.factors.path_count_truncated {
                let reason = if report.factors.path_count_budget_exhausted {
                    "work budget exhausted; path sample only"
                } else {
                    "truncated at cap"
                };
                let estimate = report
                    .factors
                    .path_count_estimated
                    .map(|e| format!("~{}", e))
                    .unwrap_or_else(|| "astronomical (exceeds 2^64)".to_string());
                println!(
                    "  Paths: {} ({} error) [{}; estimated true count: {}]",
                    report.factors.path_count, report.factors.error_path_count, reason, estimate
                );
            } else {
                println!(
                    "  Paths: {} ({} error)",
                    report.factors.path_count, report.factors.error_path_count
                );
            }
            println!("  Blocks: {}", report.factors.block_count);
            println!(
                "  Loops: {} (max nesting: {})",
                report.factors.loop_count, report.factors.max_nesting_depth
            );
            if args.coverage {
                match report.factors.uncovered_ratio {
                    Some(u) => println!("  Uncovered blocks: {:.0}%", u * 100.0),
                    None => println!("  Uncovered blocks: n/a (no coverage data)"),
                }
            }
            if args.churn {
                match report.factors.churn_commits {
                    Some(c) => println!("  Churn: {} commits in last {} days", c, args.churn_days),
                    None => println!("  Churn: n/a (not a git repo or git unavailable)"),
                }
            }
        }
        OutputFormat::Json => {
            let mut response = output::JsonResponse::new(&report);
            if report.factors.path_count_truncated {
                response.truncated = Some(true);
            }
            println!("{}", response.to_json());
        }
        OutputFormat::Pretty => {
            let mut response = output::JsonResponse::new(&report);
            if report.factors.path_count_truncated {
                response.truncated = Some(true);
            }
            println!("{}", response.to_pretty_json());
        }
    }

    Ok(())
}
