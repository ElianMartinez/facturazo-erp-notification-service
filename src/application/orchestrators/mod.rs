//! Application orchestrators
//!
//! Orchestrators coordinate complex workflows between multiple services.
//! They receive data (already validated/calculated by core-service) and
//! generate documents/notifications.

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, warn, error, instrument};

use crate::domain::document::{DocumentFormat, DocumentType};
use crate::infrastructure::generators::{GeneratorFactory, GenerationOptions, DocumentType as GenDocType};
use crate::infrastructure::notifications::{EmailService, EvolutionApiClient};
use crate::infrastructure::storage::StorageService;
use crate::infrastructure::cache::CacheService;
use crate::application::commands::{GenerateDocumentCommand, SendNotificationCommand, Priority};

/// Document generation orchestrator
///
/// Coordinates the workflow:
/// 1. Receive pre-calculated data from core-service
/// 2. Generate document (PDF/Excel/CSV)
/// 3. Store document (optional)
/// 4. Send notification (optional)
pub struct DocumentOrchestrator {
    generator_factory: Arc<GeneratorFactory>,
    storage: Arc<StorageService>,
    cache: Arc<CacheService>,
    email_service: Option<Arc<EmailService>>,
    whatsapp_service: Option<Arc<EvolutionApiClient>>,
}

impl DocumentOrchestrator {
    pub fn new(
        generator_factory: Arc<GeneratorFactory>,
        storage: Arc<StorageService>,
        cache: Arc<CacheService>,
        email_service: Option<Arc<EmailService>>,
        whatsapp_service: Option<Arc<EvolutionApiClient>>,
    ) -> Self {
        Self {
            generator_factory,
            storage,
            cache,
            email_service,
            whatsapp_service,
        }
    }

    /// Execute document generation workflow
    #[instrument(skip(self, command), fields(tenant_id = %command.tenant_id, doc_type = ?command.document_type))]
    pub async fn execute(&self, command: GenerateDocumentCommand) -> Result<DocumentResult> {
        let start_time = std::time::Instant::now();
        info!("Starting document generation workflow");

        // 1. Map document type
        let gen_doc_type = self.map_document_type(&command.document_type);

        // 2. Prepare generation options
        let options = GenerationOptions {
            watermark: None,
            password_protect: false,
            compress: true,
            include_attachments: false,
        };

        // 3. Generate document
        info!("Generating document: {:?} as {:?}", command.document_type, command.format);
        let result = self.generator_factory
            .generate(gen_doc_type, command.format.clone(), command.data.clone(), options)
            .await?;

        let document_bytes = result.document_bytes.clone();
        let mime_type = result.mime_type.clone();
        let file_size = document_bytes.len();

        // 4. Store document if enabled
        let storage_url = if command.storage_enabled {
            let path = format!(
                "{}/{}/{}_{}.{}",
                command.tenant_id,
                command.document_type.as_str(),
                result.metadata.document_id,
                chrono::Utc::now().format("%Y%m%d_%H%M%S"),
                command.format.file_extension()
            );

            info!("Storing document at: {}", path);
            let url = self.storage.upload(&path, &document_bytes, &mime_type).await?;
            Some(url)
        } else {
            None
        };

        // 5. Cache result for quick retrieval
        let cache_key = format!("doc:{}:{}", command.tenant_id, result.metadata.document_id);
        self.cache.set(&cache_key, &document_bytes, 3600).await; // 1 hour TTL

        let elapsed = start_time.elapsed();
        info!(
            "Document generation completed in {:?}ms, size: {} bytes",
            elapsed.as_millis(),
            file_size
        );

        Ok(DocumentResult {
            document_id: result.metadata.document_id,
            document_bytes,
            mime_type,
            storage_url,
            generation_time_ms: elapsed.as_millis() as u64,
            file_size,
        })
    }

    fn map_document_type(&self, doc_type: &DocumentType) -> GenDocType {
        match doc_type {
            DocumentType::Invoice => GenDocType::Invoice,
            DocumentType::Quotation => GenDocType::Quotation,
            DocumentType::Report => GenDocType::Report,
            DocumentType::Receipt => GenDocType::Receipt,
            DocumentType::CreditNote => GenDocType::Invoice, // Credit notes use invoice template
            DocumentType::Custom(_) => GenDocType::Report,   // Default to report for custom
        }
    }
}

/// Result of document generation
#[derive(Debug, Clone)]
pub struct DocumentResult {
    pub document_id: String,
    pub document_bytes: Vec<u8>,
    pub mime_type: String,
    pub storage_url: Option<String>,
    pub generation_time_ms: u64,
    pub file_size: usize,
}

/// Notification orchestrator
///
/// Coordinates notification delivery via multiple channels
pub struct NotificationOrchestrator {
    email_service: Option<Arc<EmailService>>,
    whatsapp_service: Option<Arc<EvolutionApiClient>>,
    cache: Arc<CacheService>,
}

impl NotificationOrchestrator {
    pub fn new(
        email_service: Option<Arc<EmailService>>,
        whatsapp_service: Option<Arc<EvolutionApiClient>>,
        cache: Arc<CacheService>,
    ) -> Self {
        Self {
            email_service,
            whatsapp_service,
            cache,
        }
    }

    /// Execute notification workflow
    #[instrument(skip(self, command), fields(tenant_id = %command.tenant_id, channel = ?command.channel))]
    pub async fn execute(&self, command: SendNotificationCommand) -> Result<NotificationResult> {
        use crate::domain::notification::NotificationChannel;

        info!("Starting notification workflow");

        let result = match command.channel {
            NotificationChannel::Email => {
                self.send_email(&command).await?
            }
            NotificationChannel::Whatsapp => {
                self.send_whatsapp(&command).await?
            }
            NotificationChannel::Sms | NotificationChannel::InApp => {
                warn!("Channel {:?} not implemented yet", command.channel);
                NotificationResult {
                    notification_id: uuid::Uuid::new_v4().to_string(),
                    channel: command.channel.clone(),
                    status: NotificationStatus::Failed,
                    error_message: Some(format!("{:?} not implemented", command.channel)),
                }
            }
        };

        Ok(result)
    }

    async fn send_email(&self, command: &SendNotificationCommand) -> Result<NotificationResult> {
        let email_service = self.email_service.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Email service not configured"))?;

        // Build email content from template or direct content
        let body = if let Some(template_id) = &command.template_id {
            // In production, load template and render with vars
            format!("Template: {} with vars: {}", template_id, command.template_vars)
        } else {
            command.template_vars.get("body")
                .and_then(|b| b.as_str())
                .unwrap_or("")
                .to_string()
        };

        let subject = command.subject.clone()
            .unwrap_or_else(|| "Notification".to_string());

        // Get attachments if document_id is provided
        let attachments = if let Some(doc_id) = &command.document_id {
            // Try to get document from cache
            let cache_key = format!("doc:{}:{}", command.tenant_id, doc_id);
            if let Some(bytes) = self.cache.get::<Vec<u8>>(&cache_key).await {
                vec![crate::infrastructure::notifications::EmailAttachment {
                    filename: format!("{}.pdf", doc_id),
                    content_type: "application/pdf".to_string(),
                    data: bytes,
                }]
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        email_service.send_with_attachments(
            &command.recipient,
            &subject,
            &body,
            attachments,
        ).await?;

        Ok(NotificationResult {
            notification_id: uuid::Uuid::new_v4().to_string(),
            channel: command.channel.clone(),
            status: NotificationStatus::Sent,
            error_message: None,
        })
    }

    async fn send_whatsapp(&self, command: &SendNotificationCommand) -> Result<NotificationResult> {
        let whatsapp_service = self.whatsapp_service.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WhatsApp service not configured"))?;

        let message = command.template_vars.get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Notification");

        whatsapp_service.send_simple_text(&command.recipient, message).await?;

        // Send document if provided
        if let Some(doc_id) = &command.document_id {
            let cache_key = format!("doc:{}:{}", command.tenant_id, doc_id);
            if let Some(bytes) = self.cache.get::<Vec<u8>>(&cache_key).await {
                // Convert to base64 and send as document
                use base64::Engine;
                let base64_doc = base64::engine::general_purpose::STANDARD.encode(&bytes);
                whatsapp_service.send_document(
                    &command.recipient,
                    &base64_doc,
                    &format!("{}.pdf", doc_id),
                    "application/pdf",
                ).await?;
            }
        }

        Ok(NotificationResult {
            notification_id: uuid::Uuid::new_v4().to_string(),
            channel: command.channel.clone(),
            status: NotificationStatus::Sent,
            error_message: None,
        })
    }
}

/// Result of notification delivery
#[derive(Debug, Clone)]
pub struct NotificationResult {
    pub notification_id: String,
    pub channel: crate::domain::notification::NotificationChannel,
    pub status: NotificationStatus,
    pub error_message: Option<String>,
}

/// Notification status
#[derive(Debug, Clone, PartialEq)]
pub enum NotificationStatus {
    Pending,
    Sent,
    Delivered,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests would go here with mocked dependencies
}
