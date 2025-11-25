//! Document DTOs - Data structures for document tracking
//!
//! Simple tracking of generated documents. NO business logic.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Document metadata for tracking generated PDFs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: DocumentId,
    pub tenant_id: String,
    pub document_type: DocumentType,
    pub status: DocumentStatus,
    pub template_id: String,
    pub template_version: String,
    pub data: serde_json::Value,
    pub format: DocumentFormat,
    pub storage_path: Option<String>,
    pub storage_url: Option<String>,
    pub size_bytes: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Document ID value object
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DocumentId(String);

impl DocumentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn from_string(id: String) -> Self {
        Self(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Document types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    Invoice,
    Report,
    Receipt,
    Quotation,
    CreditNote,
    Custom(String),
}

impl DocumentType {
    pub fn as_str(&self) -> &str {
        match self {
            DocumentType::Invoice => "invoice",
            DocumentType::Report => "report",
            DocumentType::Receipt => "receipt",
            DocumentType::Quotation => "quotation",
            DocumentType::CreditNote => "credit_note",
            DocumentType::Custom(s) => s,
        }
    }
}

/// Document status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    Pending,
    Processing,
    Generated,
    Stored,
    Failed,
    Delivered,
}

/// Document format
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentFormat {
    Pdf,
    Excel,
    Csv,
    Html,
}

impl DocumentFormat {
    pub fn as_str(&self) -> &str {
        match self {
            DocumentFormat::Pdf => "pdf",
            DocumentFormat::Excel => "excel",
            DocumentFormat::Csv => "csv",
            DocumentFormat::Html => "html",
        }
    }

    pub fn file_extension(&self) -> &str {
        match self {
            DocumentFormat::Pdf => "pdf",
            DocumentFormat::Excel => "xlsx",
            DocumentFormat::Csv => "csv",
            DocumentFormat::Html => "html",
        }
    }

    pub fn mime_type(&self) -> &str {
        match self {
            DocumentFormat::Pdf => "application/pdf",
            DocumentFormat::Excel => {
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            }
            DocumentFormat::Csv => "text/csv",
            DocumentFormat::Html => "text/html",
        }
    }
}

// No business logic methods - Document is just a DTO
