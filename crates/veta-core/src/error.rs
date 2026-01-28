use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("validation error: {0}")]
    Validation(String),

    #[error("database error: {0}")]
    Database(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("internal error: {0}")]
    Internal(String),
}
