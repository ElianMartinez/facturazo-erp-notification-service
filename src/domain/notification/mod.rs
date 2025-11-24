//! Notification domain module
//!
//! Contains all notification-related domain logic

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Notification entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: NotificationId,
    pub tenant_id: crate::domain::shared::TenantId,
    pub channel: NotificationChannel,
    pub status: NotificationStatus,
    pub recipient: String,
    pub subject: Option<String>,
    pub content: NotificationContent,
    pub priority: NotificationPriority,
    pub document_id: Option<crate::domain::document::DocumentId>,
    pub attempts: u32,
    pub max_attempts: u32,
    pub created_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
}

/// Notification ID value object
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NotificationId(String);

impl NotificationId {
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

/// Notification channels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    Email,
    #[serde(alias = "WhatsApp", alias = "whatsapp")]
    Whatsapp,
    InApp,
    Sms,
}

/// Notification status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationStatus {
    Pending,
    Sending,
    Sent,
    Delivered,
    Failed,
    Bounced,
}

/// Notification priority
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationPriority {
    Low,
    Normal,
    High,
    Urgent,
}

/// Notification content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationContent {
    pub template_id: Option<String>,
    pub template_vars: serde_json::Value,
    pub body: Option<String>,
    pub html_body: Option<String>,
    pub attachments: Vec<Attachment>,
}

/// Attachment for notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    pub source: AttachmentSource,
    pub size_bytes: Option<u64>,
}

/// Attachment source
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentSource {
    Url(String),
    Base64(String),
    FilePath(String),
}

impl Notification {
    /// Create a new notification
    pub fn new(
        tenant_id: crate::domain::shared::TenantId,
        channel: NotificationChannel,
        recipient: String,
        content: NotificationContent,
        priority: NotificationPriority,
    ) -> Self {
        Self {
            id: NotificationId::new(),
            tenant_id,
            channel,
            status: NotificationStatus::Pending,
            recipient,
            subject: None,
            content,
            priority,
            document_id: None,
            attempts: 0,
            max_attempts: 3,
            created_at: Utc::now(),
            sent_at: None,
            delivered_at: None,
        }
    }

    /// Link notification to a document
    pub fn with_document(mut self, document_id: crate::domain::document::DocumentId) -> Self {
        self.document_id = Some(document_id);
        self
    }

    /// Set subject for email notifications
    pub fn with_subject(mut self, subject: String) -> Self {
        self.subject = Some(subject);
        self
    }

    /// Mark notification as sending
    pub fn mark_sending(&mut self) {
        self.status = NotificationStatus::Sending;
        self.attempts += 1;
        self.sent_at = Some(Utc::now());
    }

    /// Mark notification as sent
    pub fn mark_sent(&mut self) {
        self.status = NotificationStatus::Sent;
    }

    /// Mark notification as delivered
    pub fn mark_delivered(&mut self) {
        self.status = NotificationStatus::Delivered;
        self.delivered_at = Some(Utc::now());
    }

    /// Mark notification as failed
    pub fn mark_failed(&mut self) {
        self.status = NotificationStatus::Failed;
    }

    /// Check if can retry
    pub fn can_retry(&self) -> bool {
        self.attempts < self.max_attempts && self.status == NotificationStatus::Failed
    }
}