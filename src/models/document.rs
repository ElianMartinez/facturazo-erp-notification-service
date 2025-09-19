use super::{OutputFormat, Priority};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

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
    pub tenant_id: i64,
    pub user_id: i64,
    pub organization_id: Option<String>, // Optional for backward compatibility
    pub request_time: DateTime<Utc>,
    pub ttl_seconds: Option<i64>,
    pub tags: Option<HashMap<String, String>>,
}

impl Default for DocumentMetadata {
    fn default() -> Self {
        DocumentMetadata {
            tenant_id: 0,
            user_id: 0,
            organization_id: None,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    Queued,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for DocumentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocumentStatus::Queued => write!(f, "queued"),
            DocumentStatus::Processing => write!(f, "processing"),
            DocumentStatus::Completed => write!(f, "completed"),
            DocumentStatus::Failed => write!(f, "failed"),
            DocumentStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for DocumentStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "queued" => Ok(DocumentStatus::Queued),
            "processing" => Ok(DocumentStatus::Processing),
            "completed" => Ok(DocumentStatus::Completed),
            "failed" => Ok(DocumentStatus::Failed),
            "cancelled" => Ok(DocumentStatus::Cancelled),
            _ => Err(format!("Unknown status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentStatusUpdate {
    pub id: Uuid,
    pub status: DocumentStatus,
    pub progress: Option<f32>, // 0-100
    pub message: Option<String>,
    pub updated_at: DateTime<Utc>,
}
