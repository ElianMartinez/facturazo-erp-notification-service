//! PDF Services Library - Document & Notification Service
//!
//! This library provides document generation and notification services
//! with support for PDF, Excel, CSV formats and Email, WhatsApp channels.

// New architecture modules
pub mod config;
pub mod kafka;
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod common;

// Legacy modules (kept for compatibility during migration)
pub mod api;
pub mod generators;
pub mod models;
pub mod storage;
pub mod templates;

// Re-export commonly used types from legacy modules
pub use models::{
    DocumentRequest, DocumentResponse, DocumentStatus,
    InvoiceRequest, ReportRequest,
    Priority, OutputFormat,
};

pub use generators::{PdfGenerator, ExcelGenerator};
pub use templates::{TemplateEngine, TemplateData, InvoiceData, ReportData, ReceiptData};
pub use storage::s3::S3Client;

// Re-export new architecture types
pub use config::AppConfig;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");