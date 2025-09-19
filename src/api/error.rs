use actix_web::{error::ResponseError, http::StatusCode, HttpResponse};
use std::fmt;

#[derive(Debug)]
pub struct ApiError {
    message: String,
    status_code: StatusCode,
}

impl ApiError {
    pub fn new(message: impl Into<String>, status_code: StatusCode) -> Self {
        ApiError {
            message: message.into(),
            status_code,
        }
    }

    pub fn internal_server_error(message: impl Into<String>) -> Self {
        Self::new(message, StatusCode::INTERNAL_SERVER_ERROR)
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(message, StatusCode::BAD_REQUEST)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(message, StatusCode::NOT_FOUND)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code)
            .json(serde_json::json!({
                "error": self.message,
                "status": self.status_code.as_u16()
            }))
    }

    fn status_code(&self) -> StatusCode {
        self.status_code
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::internal_server_error(err.to_string())
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        ApiError::internal_server_error(err.to_string())
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        ApiError::bad_request(err.to_string())
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        ApiError::internal_server_error(err.to_string())
    }
}

impl From<actix_web::Error> for ApiError {
    fn from(err: actix_web::Error) -> Self {
        ApiError::internal_server_error(err.to_string())
    }
}

impl From<actix_web::error::PayloadError> for ApiError {
    fn from(err: actix_web::error::PayloadError) -> Self {
        ApiError::bad_request(err.to_string())
    }
}

pub type ApiResult<T> = Result<T, ApiError>;