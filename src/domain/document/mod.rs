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
    Custom(String),
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

// No business logic methods - Document is just a DTO