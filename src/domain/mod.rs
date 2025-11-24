//! Domain layer - Data Transfer Objects (DTOs)
//!
//! This module contains only data structures for document generation.
//! NO business logic, validations or calculations - those belong to the core service.

pub mod document;
pub mod fiscal;
pub mod invoice;
pub mod notification;
pub mod shared;

// Re-export commonly used types
pub use document::{Document, DocumentId, DocumentType, DocumentStatus};
pub use fiscal::{NCF, RNC, Cedula, ITBIS, TaxId, TaxIdType};
pub use invoice::{InvoiceData, SellerData, CustomerData, InvoiceItemData, InvoiceStatus};
pub use notification::{Notification, NotificationChannel, NotificationStatus};
pub use shared::{TenantId, UserId, DomainError, DomainResult};