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
pub use document::{Document, DocumentId, DocumentStatus, DocumentType};
pub use fiscal::{Cedula, TaxId, TaxIdType, ITBIS, NCF, RNC};
pub use invoice::{CustomerData, InvoiceData, InvoiceItemData, InvoiceStatus, SellerData};
pub use notification::{Notification, NotificationChannel, NotificationStatus};
pub use shared::{DomainError, DomainResult, TenantId, UserId};
