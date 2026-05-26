use crate::cli::{resolve_db_path, Cli, DocsArgs, OutputFormat};
use crate::output;
use anyhow::Result;

pub fn docs(args: &DocsArgs, cli: &Cli) -> Result<()> {
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
                output::error(&format!("Failed to open database: {}", db_path));
                output::error(&format!("Error details: {}", e));
                output::info("Hint: Run 'magellan watch' to create the database");
                std::process::exit(output::EXIT_DATABASE);
            }
        }
    };

    let documents = db.list_source_documents()?;

    let filtered: Vec<_> = documents
        .into_iter()
        .filter(|d| {
            if let Some(ref kind) = args.kind {
                if d.source_kind != *kind {
                    return false;
                }
            }
            if let Some(ref tag) = args.tag {
                let tags = d.tags.as_deref().unwrap_or("");
                if !tags.split(',').any(|t| t.trim() == tag) {
                    return false;
                }
            }
            true
        })
        .take(args.limit)
        .collect();

    match cli.output {
        OutputFormat::Human => {
            if filtered.is_empty() {
                println!("No source documents found.");
                if args.kind.is_some() || args.tag.is_some() {
                    output::info("Try without --kind or --tag filters");
                }
            } else {
                println!("Source Documents ({} shown):", filtered.len());
                for doc in &filtered {
                    let title = doc.title.as_deref().unwrap_or("(untitled)");
                    let tags = doc.tags.as_deref().unwrap_or("-");
                    println!(
                        "  [{}] {} ({}) tags: {}",
                        doc.id, doc.path_or_uri, title, tags
                    );
                }
            }
        }
        OutputFormat::Json => {
            let response = output::JsonResponse::new(&filtered);
            println!("{}", response.to_json());
        }
        OutputFormat::Pretty => {
            let response = output::JsonResponse::new(&filtered);
            println!("{}", response.to_pretty_json());
        }
    }

    Ok(())
}
