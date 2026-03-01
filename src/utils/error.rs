use thiserror::Error;

#[derive(Error, Debug)]
pub enum ZorviaError {
    #[error("VM not found: {0}")]
    VmNotFound(String),

    #[error("VM already exists: {0}")]
    VmExists(String),
}
