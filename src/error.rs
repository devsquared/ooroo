use thiserror::Error;

use crate::parse::ParseError;
use crate::CompileError;

/// Unified error type covering parsing, compilation, and I/O.
///
/// Returned by convenience methods like [`RuleSet::from_dsl()`](crate::RuleSet::from_dsl)
/// and [`RuleSet::from_file()`](crate::RuleSet::from_file).
#[derive(Debug, Error)]
pub enum OorooError {
    /// A parse error from the DSL parser.
    #[error(transparent)]
    Parse(#[from] ParseError),

    /// A compilation error from ruleset validation.
    #[error(transparent)]
    Compile(#[from] CompileError),

    /// An I/O error (e.g., reading a DSL file).
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[cfg(feature = "binary-cache")]
    #[error(transparent)]
    Serialize(#[from] crate::serial::SerializeError),

    #[cfg(feature = "binary-cache")]
    #[error(transparent)]
    Deserialize(#[from] crate::serial::DeserializeError),
}
