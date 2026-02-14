// Platform detection and feature flags
//
// Windows support is opt-in and explicit. This avoids silent behavior changes,
// bug reports from unsupported paths, and keeps trust with existing users.

#[cfg(feature = "windows")]
pub const IS_WINDOWS: bool = true;

#[cfg(not(feature = "windows"))]
pub const IS_WINDOWS: bool = false;

#[cfg(feature = "unix")]
pub const IS_UNIX: bool = true;

#[cfg(not(feature = "unix"))]
pub const IS_UNIX: bool = false;

/// Warn users about Windows limitations on first run
pub fn check_platform_support() {
    if IS_WINDOWS {
        eprintln!("=== Windows Support Notice ===");
        eprintln!("Windows support is analysis-only:");
        eprintln!("  - No file watching (use manual reindex via Magellan)");
        eprintln!("  - No auto-index");
        eprintln!("  - No background processes");
        eprintln!();
        eprintln!("Run 'magellan watch' before using Mirage on Windows.");
        eprintln!("==================================");
    }
}
