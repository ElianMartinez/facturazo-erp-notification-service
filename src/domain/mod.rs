//! Domain layer - Core business logic
//!
//! This module contains all domain entities, value objects, and business rules.
//! It should have no dependencies on external frameworks or infrastructure.

pub mod document;
pub mod fiscal;
pub mod invoice;
pub mod notification;
pub mod shared;

// Re-export commonly used types
pub use document::{Document, DocumentId, DocumentType, DocumentStatus};
pub use fiscal::{NCF, RNC, Cedula, ITBIS, TipoComprobante, FiscalError};
pub use invoice::{Invoice, InvoiceId, InvoiceStatus, InvoiceItem, Currency, PaymentTerms};
pub use notification::{Notification, NotificationChannel, NotificationStatus};
pub use shared::{TenantId, UserId, DomainError, DomainResult};