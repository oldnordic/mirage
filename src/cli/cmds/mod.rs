pub mod blast_zone;
pub mod cfg_cmd;
pub mod coverage;
pub mod cycles;
pub mod diff;
pub mod docs;
pub mod dominators;
pub mod frontiers;
pub mod hotpaths;
pub mod hotspots;
pub mod icfg;
pub mod loops_cmd;
pub mod migrate;
pub mod paths;
pub mod patterns;
pub mod risk;
pub mod slice;
pub mod stats;
pub mod status;
pub mod suggest;
pub mod unreachable;
pub mod verify;

use crate::cli::{Cli, OutputFormat};
use crate::output;
use crate::storage::MirageDb;

/// Exit honestly for a failed database open.
///
/// The open error from [`MirageDb::open`] (or any other backend open) is
/// never discarded:
/// - file truly missing → `DatabaseNotFound` + 'magellan watch' remediation
/// - anything else (schema drift, unknown format, ...) → `DatabaseOpenFailed`
///   carrying the underlying error message (e.g. from `validate_schema_sqlite`)
///
/// This is the pattern previously implemented only by `status`/`stats`;
/// it is shared by every command handler so a schema-drifted database can
/// no longer be misreported as "not found". Exit code is EXIT_DATABASE (3)
/// in all cases.
pub(crate) fn db_open_error_exit(cli: &Cli, db_path: &str, error: &anyhow::Error) -> ! {
    // Full error chain ("context: cause: root cause") so wrapped errors
    // (e.g. "Failed to open graph database") still name the real cause.
    let details = format!("{:#}", error);
    if matches!(cli.output, OutputFormat::Json | OutputFormat::Pretty) {
        let json_error = if std::path::Path::new(db_path).exists() {
            output::JsonError::database_open_failed(db_path, &details)
        } else {
            output::JsonError::database_not_found(db_path)
        };
        let wrapper = output::JsonResponse::new(json_error);
        match cli.output {
            OutputFormat::Pretty => println!("{}", wrapper.to_pretty_json()),
            _ => println!("{}", wrapper.to_json()),
        }
    } else {
        output::error(&format!("Failed to open database: {}", db_path));
        output::error(&format!("Error details: {}", details));
        if !std::path::Path::new(db_path).exists() {
            output::info("Hint: Run 'magellan watch' to create the database");
        }
    }
    std::process::exit(output::EXIT_DATABASE);
}

/// Open the Mirage database or exit with an honest error (see
/// [`db_open_error_exit`]). Shared by all command handlers.
pub(crate) fn open_db_or_exit(cli: &Cli, db_path: &str) -> MirageDb {
    match MirageDb::open(db_path) {
        Ok(db) => db,
        Err(e) => db_open_error_exit(cli, db_path, &e),
    }
}

pub use blast_zone::blast_zone;
pub use cfg_cmd::cfg;
pub use coverage::coverage;
pub use cycles::cycles;
pub use diff::diff;
pub use docs::docs;
pub use dominators::dominators;
pub use frontiers::frontiers;
pub use hotpaths::hotpaths;
pub use hotspots::hotspots;
pub use icfg::icfg;
pub use loops_cmd::loops;
pub use migrate::migrate;
pub use paths::paths;
pub use patterns::patterns;
pub use risk::risk;
pub use slice::slice;
pub use stats::stats;
pub use status::status;
pub use suggest::suggest;
pub use unreachable::unreachable;
pub use verify::verify;

#[cfg(test)]
pub(crate) use cfg_cmd::create_test_cfg;
