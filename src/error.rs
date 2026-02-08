use std::path::PathBuf;

/// Error types for the geoipsed library.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// MMDB database file was not found at the expected path.
    #[error("database not found: {path}")]
    DatabaseNotFound { path: PathBuf },

    /// IP address lookup failed in the MMDB database.
    #[error("lookup failed for {ip}")]
    LookupFailed {
        ip: String,
        #[source]
        source: maxminddb::MaxMindDBError,
    },

    /// Template string is malformed.
    #[error("invalid template: {reason}")]
    InvalidTemplate { reason: String },

    /// No IP address patterns were selected for extraction.
    #[error("no IP patterns selected")]
    NoPatterns,

    /// Provider was not found in the registry.
    #[error("unknown provider: {name}")]
    UnknownProvider { name: String },

    /// Provider has not been initialized.
    #[error("provider not initialized: {name}")]
    ProviderNotInitialized { name: String },

    /// An I/O error occurred.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// A regex compilation error.
    #[error("regex error: {0}")]
    Regex(#[from] regex_automata::meta::BuildError),
}

/// Convenience type alias for Results using the library error.
pub type Result<T> = std::result::Result<T, Error>;
