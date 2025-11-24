//! Application queries (CQRS pattern)
//!
//! Queries represent read operations that don't change state

use serde::{Deserialize, Serialize};

/// Query to get document status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetDocumentStatusQuery {
    pub tenant_id: String,
    pub document_id: String,
}

/// Query to list notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListNotificationsQuery {
    pub tenant_id: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub status_filter: Option<crate::domain::notification::NotificationStatus>,
}

/// Query to get batch progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBatchProgressQuery {
    pub tenant_id: String,
    pub batch_id: String,
}
