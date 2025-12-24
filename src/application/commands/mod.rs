//! Application commands (CQRS pattern)
//!
//! Commands represent actions that change the state of the system

use serde::{Deserialize, Serialize};
use validator::Validate;

/// Command to generate a document
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct GenerateDocumentCommand {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub document_type: crate::domain::document::DocumentType,
    pub template_id: String,
    pub template_version: String,
    pub data: serde_json::Value,
    pub format: crate::domain::document::DocumentFormat,
    pub storage_enabled: bool,
    pub notification_enabled: bool,
    pub priority: Priority,
}

/// Command to send a notification
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SendNotificationCommand {
    pub tenant_id: String,
    pub channel: crate::domain::notification::NotificationChannel,
    #[validate(email)]
    pub recipient: String,
    pub subject: Option<String>,
    pub template_id: Option<String>,
    pub template_vars: serde_json::Value,
    pub priority: crate::domain::notification::NotificationPriority,
    pub document_id: Option<String>,
    /// Human-readable filename for the document attachment (e.g., "Factura_001.pdf")
    /// If not provided, falls back to document_id
    #[serde(default)]
    pub document_filename: Option<String>,
    pub attachments: Vec<AttachmentData>,
}

/// Command to process a batch of documents
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct BatchProcessCommand {
    pub tenant_id: String,
    pub batch_id: String,
    pub documents: Vec<BatchDocumentItem>,
    pub parallel_workers: u32,
    pub continue_on_error: bool,
    pub notification_on_complete: bool,
}

/// Batch document item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchDocumentItem {
    pub document_id: String,
    pub template_id: String,
    pub data: serde_json::Value,
}

/// Attachment data for notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentData {
    pub filename: String,
    pub content_type: String,
    pub url: Option<String>,
    pub base64: Option<String>,
}

/// Priority for commands
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    Normal,
    High,
    Urgent,
}
