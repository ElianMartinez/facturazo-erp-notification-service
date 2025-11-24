//! Domain layer - Core business logic
//!
//! This module contains all domain entities, value objects, and business rules.
//! It should have no dependencies on external frameworks or infrastructure.

pub mod document;
pub mod notification;
pub mod shared;

// Re-export commonly used types
pub use document::{Document, DocumentId, DocumentType, DocumentStatus};
pub use notification::{Notification, NotificationChannel, NotificationStatus};
pub use shared::{TenantId, UserId, DomainError, DomainResult};