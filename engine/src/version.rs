/// MNMX engine version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the engine version string.
pub fn engine_version() -> &'static str {
    VERSION
}

/// Returns build metadata.
pub fn build_info() -> BuildInfo {
    BuildInfo {
        version: VERSION,
        rust_version: env!("CARGO_PKG_RUST_VERSION"),
    }
}

/// Build metadata.
pub struct BuildInfo {
    pub version: &'static str,
    pub rust_version: &'static str,
}
