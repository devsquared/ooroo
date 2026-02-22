use thiserror::Error;

use crate::parse::ParseError;
use crate::CompileError;

/// Unified error type covering parsing, compilation, and I/O.
///
/// Returned by convenience methods like [`RuleSet::from_dsl()`](crate::RuleSet::from_dsl)
/// and [`RuleSet::from_file()`](crate::RuleSet::from_file).
#[derive(Debug, Error)]
pub enum OorooError {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    Compile(#[from] CompileError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[cfg(feature = "binary-cache")]
    #[error(transparent)]
    Serialize(#[from] crate::serial::SerializeError),

    #[cfg(feature = "binary-cache")]
    #[error(transparent)]
    Deserialize(#[from] crate::serial::DeserializeError),
}
