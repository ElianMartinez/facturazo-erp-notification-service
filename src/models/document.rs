use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use std::collections::HashMap;
use super::{Priority, OutputFormat};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRequest {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub template_id: String,
    pub document_type: DocumentType,
    pub data: serde_json::Value,
    pub priority: Priority,
    pub format: OutputFormat,
    pub callback_url: Option<String>,
    pub metadata: DocumentMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    Invoice,
    Report,
    Certificate,
    Statement,
    Receipt,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub user_id: String,
    pub organization_id: String,
    pub request_time: DateTime<Utc>,
    pub ttl_seconds: Option<i64>,
    pub tags: Option<HashMap<String, String>>,
}

impl Default for DocumentMetadata {
    fn default() -> Self {
        DocumentMetadata {
            user_id: String::new(),
            organization_id: String::new(),
            request_time: Utc::now(),
            ttl_seconds: Some(86400), // 24 hours
            tags: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentResponse {
    pub id: Uuid,
    pub status: DocumentStatus,
    pub url: Option<String>,
    pub error: Option<String>,
    pub processing_time_ms: u64,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    Queued,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentStatusUpdate {
    pub id: Uuid,
    pub status: DocumentStatus,
    pub progress: Option<f32>, // 0-100
    pub message: Option<String>,
    pub updated_at: DateTime<Utc>,
}