use thiserror::Error;

#[derive(Error, Debug)]
pub enum ZorviaError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Template not found: {0}")]
    TemplateNotFound(String),

    #[error("Kubernetes error: {0}")]
    Kube(#[from] kube::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("VM not found: {0}")]
    VmNotFound(String),

    #[error("VM already exists: {0}")]
    VmExists(String),
}
