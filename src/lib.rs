pub mod api;
pub mod generators;
pub mod models;
pub mod storage;
pub mod templates;

// Re-export commonly used types
pub use models::{
    DocumentRequest, DocumentResponse, DocumentStatus,
    InvoiceRequest, ReportRequest,
    Priority, OutputFormat,
};

pub use generators::{PdfGenerator, ExcelGenerator};
pub use templates::{TemplateEngine, TemplateData, InvoiceData, ReportData, ReceiptData};
pub use storage::s3::S3Client;