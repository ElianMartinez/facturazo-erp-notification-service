//! Shared domain module
//!
//! Contains shared value objects, errors, and domain services

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Tenant ID value object
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TenantId(String);

impl TenantId {
    pub fn new(id: String) -> Self {
        Self(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// User ID value object
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct UserId(String);

impl UserId {
    pub fn new(id: String) -> Self {
        Self(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Domain errors
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Validation failed: {0}")]
    ValidationError(String),

    #[error("Business rule violation: {0}")]
    BusinessRuleViolation(String),

    #[error("Concurrency conflict")]
    ConcurrencyConflict,

    #[error("External service error: {0}")]
    ExternalServiceError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Domain result type
pub type DomainResult<T> = Result<T, DomainError>;

/// Domain event trait
pub trait DomainEvent: Send + Sync {
    fn event_type(&self) -> &str;
    fn aggregate_id(&self) -> &str;
    fn occurred_at(&self) -> chrono::DateTime<chrono::Utc>;
}

/// Base domain event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseDomainEvent {
    pub event_id: String,
    pub event_type: String,
    pub aggregate_id: String,
    pub tenant_id: TenantId,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
    pub version: u32,
}

impl BaseDomainEvent {
    pub fn new(event_type: String, aggregate_id: String, tenant_id: TenantId) -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            event_type,
            aggregate_id,
            tenant_id,
            occurred_at: chrono::Utc::now(),
            version: 1,
        }
    }
}

impl DomainEvent for BaseDomainEvent {
    fn event_type(&self) -> &str {
        &self.event_type
    }

    fn aggregate_id(&self) -> &str {
        &self.aggregate_id
    }

    fn occurred_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.occurred_at
    }
}