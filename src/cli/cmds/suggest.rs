use crate::cli::{resolve_db_path, Cli, OutputFormat, SuggestArgs};
use crate::output;
use anyhow::Result;

pub fn suggest(args: &SuggestArgs, cli: &Cli) -> Result<()> {
    use crate::analysis::suggest;

    let db_path = resolve_db_path(cli.db.clone())?;
    let db = super::open_db_or_exit(cli, &db_path);

    let function_id = match crate::storage::resolve_function_or_semantic(
        &db,
        &args.symbol,
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

    let report =
        suggest::compute_suggestions(&db, function_id, &args.symbol, args.file.as_deref())?;

    match cli.output {
        OutputFormat::Human => {
            println!("Suggestions for: {}", report.symbol);
            if let Some(ref fp) = report.file_path {
                println!("  File: {}", fp);
            }
            println!();
            for s in &report.suggestions {
                println!(
                    "  [{}] {} ({})",
                    s.severity.to_uppercase(),
                    s.message,
                    s.kind
                );
                if let Some(ref detail) = s.detail {
                    println!("    {}", detail);
                }
            }
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
