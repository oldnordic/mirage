use crate::cli::{resolve_db_path, Cli, OutputFormat, SliceArgs, SliceDirectionArg};
use crate::output;
use anyhow::Result;

pub fn slice(args: &SliceArgs, cli: &Cli) -> Result<()> {
    use crate::analysis::{MagellanBridge, SliceWrapper};

    // Resolve database path
    let db_path = resolve_db_path(cli.db.clone())?;

    // Open Magellan database
    let bridge = match MagellanBridge::open(&db_path) {
        Ok(bridge) => bridge,
        Err(e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "DatabaseError",
                    &format!("Failed to open Magellan database: {}", e),
                    output::E_DATABASE_NOT_FOUND,
                );
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!("Failed to open Magellan database: {}", e));
                output::info("Note: Program slicing requires a Magellan code graph database");
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    // Perform the slice based on direction
    let slice_result: SliceWrapper = match args.direction {
        SliceDirectionArg::Backward => bridge.backward_slice(&args.symbol)?,
        SliceDirectionArg::Forward => bridge.forward_slice(&args.symbol)?,
    };

    // Output based on format
    match cli.output {
        OutputFormat::Human => {
            let mut output = String::new();
            output.push_str(&format!("Program Slice: {}\n", slice_result.direction));
            output.push('\n');

            // Target symbol
            output.push_str("Target:\n");
            output.push_str(&format!(
                "  Symbol: {}\n",
                slice_result.target.fqn.as_deref().unwrap_or(&args.symbol)
            ));
            output.push_str(&format!("  Kind: {}\n", slice_result.target.kind));
            output.push_str(&format!("  File: {}\n", slice_result.target.file_path));
            output.push('\n');

            // Statistics
            output.push_str("Statistics:\n");
            output.push_str(&format!(
                "  Total symbols in slice: {}\n",
                slice_result.symbol_count
            ));
            output.push_str(&format!(
                "  Data dependencies: {}\n",
                slice_result.statistics.data_dependencies
            ));
            output.push_str(&format!(
                "  Control dependencies: {}\n",
                slice_result.statistics.control_dependencies
            ));
            output.push('\n');

            // Included symbols (verbose only)
            if args.verbose {
                output.push_str(&format!(
                    "Included symbols ({}):\n",
                    slice_result.included_symbols.len()
                ));
                for (i, symbol) in slice_result.included_symbols.iter().enumerate() {
                    output.push_str(&format!(
                        "  {}. {}\n",
                        i + 1,
                        symbol.fqn.as_deref().unwrap_or("<unknown>")
                    ));
                    output.push_str(&format!(
                        "     Kind: {}, File: {}\n",
                        symbol.kind, symbol.file_path
                    ));
                }
            } else {
                output.push_str("Use --verbose to see all included symbols\n");
            }
            let processed = output::apply_token_budget(output, args.tokens);
            print!("{}", processed);
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let json_str = if matches!(cli.output, OutputFormat::Pretty) {
                serde_json::to_string_pretty(&slice_result).unwrap_or_default()
            } else {
                serde_json::to_string(&slice_result).unwrap_or_default()
            };
            let processed = output::apply_token_budget(json_str, args.tokens);
            let tokens_est = processed.len() / 4;
            let truncated = args.tokens.is_some_and(|t| t > 0 && tokens_est > t);
            let wrapper = output::JsonResponse::new(slice_result)
                .with_tokens(tokens_est)
                .with_truncated(truncated);
            match cli.output {
                OutputFormat::Json => println!("{}", wrapper.to_json()),
                OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                _ => unreachable!(),
            }
        }
    }

    Ok(())
}
