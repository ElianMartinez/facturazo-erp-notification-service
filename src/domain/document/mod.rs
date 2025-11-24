//! Document domain module
//!
//! Contains all document-related domain logic

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Document entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: DocumentId,
    pub tenant_id: crate::domain::shared::TenantId,
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

impl Document {
    /// Create a new document
    pub fn new(
        tenant_id: crate::domain::shared::TenantId,
        document_type: DocumentType,
        template_id: String,
        template_version: String,
        data: serde_json::Value,
        format: DocumentFormat,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: DocumentId::new(),
            tenant_id,
            document_type,
            status: DocumentStatus::Pending,
            template_id,
            template_version,
            data,
            format,
            storage_path: None,
            storage_url: None,
            size_bytes: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Mark document as processing
    pub fn mark_processing(&mut self) {
        self.status = DocumentStatus::Processing;
        self.updated_at = Utc::now();
    }

    /// Mark document as generated
    pub fn mark_generated(&mut self, size_bytes: u64) {
        self.status = DocumentStatus::Generated;
        self.size_bytes = Some(size_bytes);
        self.updated_at = Utc::now();
    }

    /// Mark document as stored
    pub fn mark_stored(&mut self, path: String, url: String) {
        self.status = DocumentStatus::Stored;
        self.storage_path = Some(path);
        self.storage_url = Some(url);
        self.updated_at = Utc::now();
    }

    /// Mark document as failed
    pub fn mark_failed(&mut self) {
        self.status = DocumentStatus::Failed;
        self.updated_at = Utc::now();
    }
}