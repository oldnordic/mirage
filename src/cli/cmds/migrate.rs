use crate::cli::{BackendFormat, Cli, MigrateArgs, OutputFormat};
use anyhow::Result;

pub fn migrate(args: &MigrateArgs, cli: &Cli) -> Result<()> {
    use crate::storage::BackendFormat as StorageBackendFormat;

    let db_path = std::path::Path::new(&args.db);

    // Validate database exists
    if !db_path.exists() {
        return Err(anyhow::anyhow!("Database not found: {}", args.db));
    }

    // Detect actual backend format using mirage's detection
    let actual_format = StorageBackendFormat::detect(db_path)
        .map_err(|e| anyhow::anyhow!("Backend detection failed: {}", e))?;

    // Convert storage BackendFormat to cli BackendFormat for comparison
    let actual_format_cli = match actual_format {
        StorageBackendFormat::SQLite => BackendFormat::Sqlite,
        StorageBackendFormat::Geometric => BackendFormat::Geometric,
        StorageBackendFormat::Unknown => {
            return Err(anyhow::anyhow!(
                "Cannot detect backend format: unknown format"
            ));
        }
    };

    // Validate source format matches actual database
    if args.from != actual_format_cli {
        return Err(anyhow::anyhow!(
            "Source backend mismatch: expected {}, found {:?}",
            args.from,
            actual_format
        ));
    }

    // Validate source and target are different
    if args.from == args.to {
        return Err(anyhow::anyhow!(
            "Source and target backends must be different"
        ));
    }

    // Dry run: just report what would happen
    if args.dry_run {
        match cli.output {
            OutputFormat::Human => {
                println!("Dry run: would migrate {} -> {}", args.from, args.to);
                println!("Database: {}", args.db);
            }
            OutputFormat::Json | OutputFormat::Pretty => {
                let output = serde_json::json!({
                    "dry_run": true,
                    "from": args.from.to_string(),
                    "to": args.to.to_string(),
                    "database": args.db,
                });
                match cli.output {
                    OutputFormat::Json => println!("{}", serde_json::to_string(&output)?),
                    OutputFormat::Pretty => {
                        println!("{}", serde_json::to_string_pretty(&output)?)
                    }
                    _ => unreachable!(),
                }
            }
        }
        return Ok(());
    }

    // Create backup if requested
    if args.backup {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_else(|_| {
                // Fallback to simple counter if clock is before UNIX_EPOCH
                std::time::SystemTime::now()
                    .elapsed()
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            });
        let backup_path = format!("{}.backup.{}", args.db, timestamp);
        std::fs::copy(&args.db, &backup_path)
            .map_err(|e| anyhow::anyhow!("Failed to create backup: {}", e))?;
        eprintln!("Backup created: {}", backup_path);
    }

    Err(anyhow::anyhow!(
        "Backend migration is not supported by this Mirage build; use SQLite .magellan/<project>.db databases"
    ))
}
