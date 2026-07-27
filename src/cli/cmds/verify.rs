use crate::cli::responses::*;
use crate::cli::{resolve_db_path, Cli, OutputFormat, VerifyArgs};
use crate::output;
use anyhow::Result;

pub fn verify(args: &VerifyArgs, cli: &Cli) -> Result<()> {
    use crate::cfg::{enumerate_paths, load_cfg_from_db, PathLimits};
    use rusqlite::OptionalExtension;

    // Resolve database path
    let db_path = resolve_db_path(cli.db.clone())?;

    // Open database (honest open-error handling via shared helper)
    let db = super::open_db_or_exit(cli, &db_path);

    let path_id = &args.path_id;

    // Path verification requires SQLite backend (path caching)
    if !db.is_sqlite() {
        let msg = "Path verification requires SQLite backend with path caching.";
        if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
            let error = output::JsonError::new("UnsupportedBackend", msg, output::E_INVALID_INPUT);
            let wrapper = output::JsonResponse::new(error);
            println!("{}", wrapper.to_json());
            std::process::exit(output::EXIT_USAGE);
        } else {
            output::error(msg);
            output::info("retired binary backend backend does not support path caching.");
            std::process::exit(output::EXIT_USAGE);
        }
    }

    // Check if path exists in cache by querying cfg_paths table
    let cached_path_info: Option<(String, i64, String)> = db
        .conn()?
        .query_row(
            "SELECT path_id, function_id, path_kind FROM cfg_paths WHERE path_id = ?1",
            rusqlite::params![path_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .unwrap_or(None);

    let (found_in_cache, function_id, _path_kind) = match cached_path_info {
        Some((_id, fid, kind)) => (true, fid, kind),
        None => {
            // Path not found in cache
            let result = VerifyResult {
                path_id: path_id.clone(),
                valid: false,
                found_in_cache: false,
                function_id: None,
                reason: "Path not found in cache".to_string(),
                current_paths: 0,
            };

            match cli.output {
                OutputFormat::Human => {
                    println!("Path ID {}: not found in cache", path_id);
                    println!("  The path may have been invalidated or never existed.");
                }
                OutputFormat::Json | OutputFormat::Pretty => {
                    let wrapper = output::JsonResponse::new(result);
                    match cli.output {
                        OutputFormat::Json => println!("{}", wrapper.to_json()),
                        OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                        _ => unreachable!(),
                    }
                }
            }
            return Ok(());
        }
    };

    // Path exists in cache - verify it still exists in current enumeration
    // Load CFG from database for this function
    let cfg = match load_cfg_from_db(&db, function_id) {
        Ok(cfg) => cfg,
        Err(_e) => {
            if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
                let error = output::JsonError::new(
                    "CgfLoadError",
                    &format!("Failed to load CFG for function_id {}", function_id),
                    output::E_CFG_ERROR,
                );
                let wrapper = output::JsonResponse::new(error);
                println!("{}", wrapper.to_json());
                std::process::exit(output::EXIT_DATABASE);
            } else {
                output::error(&format!(
                    "Failed to load CFG for function_id {}",
                    function_id
                ));
                output::info("The function data may be corrupted. Try re-running 'magellan watch'");
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    // Re-enumerate paths to check if the path still exists
    let limits = PathLimits::default();
    let current_paths = enumerate_paths(&cfg, &limits);
    let current_path_count = current_paths.len();

    // Check if any enumerated path has the same path_id
    let path_still_valid = current_paths.iter().any(|p| &p.path_id == path_id);

    let reason = if path_still_valid {
        "Path found in current enumeration".to_string()
    } else {
        "Path no longer exists in current enumeration (code may have changed)".to_string()
    };

    let result = VerifyResult {
        path_id: path_id.clone(),
        valid: path_still_valid,
        found_in_cache,
        function_id: Some(function_id),
        reason,
        current_paths: current_path_count,
    };

    match cli.output {
        OutputFormat::Human => {
            println!(
                "Path ID {}: {}",
                path_id,
                if result.valid { "valid" } else { "invalid" }
            );
            println!(
                "  Found in cache: {}",
                if found_in_cache { "yes" } else { "no" }
            );
            println!("  Status: {}", result.reason);
            println!("  Current total paths: {}", current_path_count);
            if !path_still_valid {
                println!();
                output::info("The path may have been invalidated by code changes.");
                output::info("Consider re-running path enumeration to update the cache.");
            }
        }
        OutputFormat::Json | OutputFormat::Pretty => {
            let wrapper = output::JsonResponse::new(result);
            match cli.output {
                OutputFormat::Json => println!("{}", wrapper.to_json()),
                OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
                _ => unreachable!(),
            }
        }
    }

    Ok(())
}
