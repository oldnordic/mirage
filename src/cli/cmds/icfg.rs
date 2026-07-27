use crate::cli::{resolve_db_path, Cli, IcfgArgs, IcfgFormat, OutputFormat};
use anyhow::Result;

pub fn icfg(args: &IcfgArgs, cli: &Cli) -> Result<()> {
    use crate::cfg::icfg::{build_icfg, to_dot, IcfgJson, IcfgOptions};
    use crate::output::error;
    use crate::output::{EXIT_DATABASE, EXIT_NOT_FOUND};

    let db_path = resolve_db_path(cli.db.clone())?;

    // Open database (honest open-error handling via shared helper:
    // JSON modes get the standard error envelope, human mode gets details)
    let db = super::open_db_or_exit(cli, &db_path);

    // Resolve function name to ID
    let function_id = match db.resolve_function_name(&args.entry) {
        Ok(id) => id,
        Err(_) => {
            error(&format!("Function not found: {}", args.entry));
            std::process::exit(EXIT_NOT_FOUND);
        }
    };

    // Build options
    let options = IcfgOptions {
        max_depth: args.depth,
        include_return_edges: args.return_edges,
    };

    // Build ICFG
    let icfg = match build_icfg(db.storage(), db.backend(), db.path(), function_id, options) {
        Ok(icfg) => icfg,
        Err(e) => {
            error(&format!("Failed to build ICFG: {}", e));
            std::process::exit(EXIT_DATABASE);
        }
    };

    // Output based on format
    let format = args.format.unwrap_or(match cli.output {
        OutputFormat::Human => IcfgFormat::Human,
        _ => IcfgFormat::Dot,
    });

    match format {
        IcfgFormat::Dot => {
            println!("{}", to_dot(&icfg));
        }
        IcfgFormat::Json => {
            let json_repr = IcfgJson::from_icfg(&icfg);
            println!("{}", serde_json::to_string_pretty(&json_repr)?);
        }
        IcfgFormat::Human => {
            print_icfg_human(&icfg);
        }
    }

    Ok(())
}
fn print_icfg_human(icfg: &crate::cfg::icfg::Icfg) {
    use std::collections::HashSet;
    println!("Inter-Procedural CFG");
    println!("  Entry function: {}", icfg.entry_function);

    // Count unique functions
    let mut functions = HashSet::new();
    for node in icfg.graph.node_indices() {
        functions.insert(icfg.graph[node].function_id);
    }
    println!("  Functions: {}", functions.len());
    println!("  Nodes: {}", icfg.graph.node_count());
    println!("  Edges: {}", icfg.graph.edge_count());

    // Count edge types
    let mut call_count = 0;
    let mut return_count = 0;
    let mut intra_count = 0;

    for edge in icfg.graph.edge_indices() {
        match &icfg.graph[edge] {
            crate::cfg::icfg::IcfgEdge::Call { .. } => call_count += 1,
            crate::cfg::icfg::IcfgEdge::Return { .. } => return_count += 1,
            crate::cfg::icfg::IcfgEdge::IntraProcedural { .. } => intra_count += 1,
        }
    }

    println!("  Edges by type:");
    println!("    Call: {}", call_count);
    println!("    Return: {}", return_count);
    println!("    Intra-procedural: {}", intra_count);
}
