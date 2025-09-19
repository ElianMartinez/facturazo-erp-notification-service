use std::fmt;
use std::error::Error;

#[derive(Debug)]
pub enum DocumentError {
    IoError(String),
    GenerationError(String),
    TemplateError(String),
    ValidationError(String),
    ConfigError(String),
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentError::IoError(msg) => write!(f, "Error de E/S: {}", msg),
            DocumentError::GenerationError(msg) => write!(f, "Error de generación: {}", msg),
            DocumentError::TemplateError(msg) => write!(f, "Error de plantilla: {}", msg),
            DocumentError::ValidationError(msg) => write!(f, "Error de validación: {}", msg),
            DocumentError::ConfigError(msg) => write!(f, "Error de configuración: {}", msg),
        }
    }
}

impl Error for DocumentError {}

impl From<std::io::Error> for DocumentError {
    fn from(error: std::io::Error) -> Self {
        DocumentError::IoError(error.to_string())
    }
}

pub type DocumentResult<T> = Result<T, DocumentError>;