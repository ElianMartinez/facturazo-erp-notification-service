//! Application layer - Use cases and orchestration
//!
//! This layer contains application services, commands, queries, and orchestrators
//! that coordinate between domain and infrastructure layers.

pub mod commands;
pub mod queries;
pub mod orchestrators;

use std::sync::Arc;
use anyhow::Result;

/// Application services container
pub struct ApplicationServices {
    pub document_service: Arc<dyn DocumentService>,
    pub notification_service: Arc<dyn NotificationService>,
}

/// Document service trait
#[async_trait::async_trait]
pub trait DocumentService: Send + Sync {
    /// Generate a document
    async fn generate_document(
        &self,
        request: commands::GenerateDocumentCommand,
    ) -> Result<crate::domain::document::Document>;

    /// Get document status
    async fn get_document_status(
        &self,
        document_id: &str,
    ) -> Result<crate::domain::document::DocumentStatus>;

    /// Process batch documents
    async fn process_batch(
        &self,
        request: commands::BatchProcessCommand,
    ) -> Result<Vec<crate::domain::document::Document>>;
}

/// Notification service trait
#[async_trait::async_trait]
pub trait NotificationService: Send + Sync {
    /// Send a notification
    async fn send_notification(
        &self,
        request: commands::SendNotificationCommand,
    ) -> Result<crate::domain::notification::Notification>;

    /// Get notification status
    async fn get_notification_status(
        &self,
        notification_id: &str,
    ) -> Result<crate::domain::notification::NotificationStatus>;

    /// Send batch notifications
    async fn send_batch_notifications(
        &self,
        requests: Vec<commands::SendNotificationCommand>,
    ) -> Result<Vec<crate::domain::notification::Notification>>;
}