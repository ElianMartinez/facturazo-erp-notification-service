//! Notification services infrastructure
//!
//! This module contains implementations for various notification channels

pub mod email;
pub mod erp_report_notifier;
pub mod evolution_api;

use anyhow::Result;
use async_trait::async_trait;

// Re-export main types
pub use email::EmailAttachment;
pub use email::EmailService;
pub use erp_report_notifier::{DeliveryResult, ErpReportNotifier, ErpReportNotifierBuilder};
pub use evolution_api::EvolutionAPIClient;
pub use evolution_api::EvolutionAPIClient as EvolutionApiClient; // Alias

/// Trait for notification services
#[async_trait]
pub trait NotificationService: Send + Sync {
    /// Send a notification
    async fn send(&self, recipient: &str, message: NotificationMessage) -> Result<String>;

    /// Check if service is available
    async fn health_check(&self) -> Result<bool>;
}

/// Notification message content
#[derive(Debug, Clone)]
pub struct NotificationMessage {
    pub subject: Option<String>,
    pub body: String,
    pub html_body: Option<String>,
    pub attachments: Vec<Attachment>,
}

/// Attachment for notifications
#[derive(Debug, Clone)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    pub content: Vec<u8>,
}

/// WhatsApp notification service using EvolutionAPI
pub struct WhatsAppService {
    client: EvolutionAPIClient,
}

impl WhatsAppService {
    /// Create a new WhatsApp service
    pub fn new(base_url: String, api_key: String, instance: String) -> Self {
        Self {
            client: EvolutionAPIClient::new(base_url, api_key, instance),
        }
    }

    /// Send invoice with PDF attachment
    pub async fn send_invoice(
        &self,
        phone: String,
        invoice_number: String,
        ncf: String,
        amount: String,
        pdf_bytes: Vec<u8>,
    ) -> Result<String> {
        self.client
            .send_invoice_notification(phone, invoice_number, ncf, amount, pdf_bytes)
            .await
    }
}

#[async_trait]
impl NotificationService for WhatsAppService {
    async fn send(&self, recipient: &str, message: NotificationMessage) -> Result<String> {
        // For WhatsApp, we'll send text message and attachments separately
        let request = evolution_api::SendTextRequest {
            number: evolution_api::normalize_dominican_phone(recipient),
            text: message.body,
            delay: None,
            link_preview: Some(false),
            mentioned: None,
            mentions_every_one: None,
            quoted: None,
        };

        let text_id = self.client.send_text(request).await?;

        // Send attachments if any
        for attachment in &message.attachments {
            if attachment.content_type == "application/pdf" {
                self.client
                    .send_pdf(
                        recipient.to_string(),
                        attachment.content.clone(),
                        attachment.filename.clone(),
                        message.subject.clone().unwrap_or_default(),
                    )
                    .await?;
            }
        }

        Ok(text_id)
    }

    async fn health_check(&self) -> Result<bool> {
        self.client.is_connected().await
    }
}
