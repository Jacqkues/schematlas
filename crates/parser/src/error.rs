use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum Error {
    /// The text is not valid in its format. `offset` is a byte offset when known.
    #[error("{message}")]
    Parse {
        message: String,
        offset: Option<usize>,
    },
    /// Valid, but not a diagram family this crate can turn into a graph yet.
    #[error("{0} diagrams can be shown but not edited as graphs yet")]
    Unsupported(String),
    /// The edit does not make sense for this diagram (missing node, duplicate id…).
    #[error("{0}")]
    InvalidEdit(String),
}
pub type Result<T> = std::result::Result<T, Error>;
