//! Application orchestrators
//!
//! Orchestrators coordinate complex workflows between multiple services.
//! They receive data (already validated/calculated by core-service) and
//! generate documents/notifications.

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, instrument, warn};

use crate::application::commands::{GenerateDocumentCommand, SendNotificationCommand};
use crate::domain::document::DocumentType;
use crate::infrastructure::cache::CacheService;
use crate::infrastructure::generators::{
    DocumentType as GenDocType, GenerationOptions, GeneratorFactory, TypstGenerator,
};
use crate::infrastructure::notifications::{EmailService, WhatsAppProvider};
use crate::infrastructure::storage::StorageService;
use crate::infrastructure::template_resolver::TemplateResolver;

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
    #[allow(dead_code)]
    email_service: Option<Arc<EmailService>>,
    #[allow(dead_code)]
    whatsapp_service: Option<Arc<dyn WhatsAppProvider>>,
    /// Dynamic template resolver (None = use built-in templates only)
    template_resolver: Option<Arc<TemplateResolver>>,
    /// Typst generator for dynamic templates
    typst_generator: Arc<TypstGenerator>,
}

impl DocumentOrchestrator {
    pub fn new(
        generator_factory: Arc<GeneratorFactory>,
        storage: Arc<StorageService>,
        cache: Arc<CacheService>,
        email_service: Option<Arc<EmailService>>,
        whatsapp_service: Option<Arc<dyn WhatsAppProvider>>,
        template_resolver: Option<Arc<TemplateResolver>>,
        typst_generator: Arc<TypstGenerator>,
    ) -> Self {
        Self {
            generator_factory,
            storage,
            cache,
            email_service,
            whatsapp_service,
            template_resolver,
            typst_generator,
        }
    }

    /// Execute document generation workflow
    #[instrument(skip(self, command), fields(tenant_id = %command.tenant_id, doc_type = ?command.document_type))]
    pub async fn execute(&self, command: GenerateDocumentCommand) -> Result<DocumentResult> {
        let start_time = std::time::Instant::now();
        info!("Starting document generation workflow");

        // 1. Try dynamic template from backend (PDF only)
        let dynamic_result = if command.format == crate::domain::document::DocumentFormat::Pdf {
            self.try_dynamic_template(&command).await
        } else {
            None
        };

        let (document_bytes, mime_type, document_id) = if let Some(pdf_bytes) = dynamic_result {
            info!("Generated PDF from dynamic backend template");
            let doc_id = uuid::Uuid::new_v4().to_string();
            (pdf_bytes, "application/pdf".to_string(), doc_id)
        } else {
            // 2. Fallback to built-in generator
            let gen_doc_type = self.map_document_type(&command.document_type);
            let options = GenerationOptions {
                watermark: None,
                password_protect: false,
                compress: true,
                include_attachments: false,
            };

            info!(
                "Generating document: {:?} as {:?} (built-in)",
                command.document_type, command.format
            );
            let result = self
                .generator_factory
                .generate(
                    gen_doc_type,
                    command.format.clone(),
                    command.data.clone(),
                    options,
                )
                .await?;

            (
                result.document_bytes,
                result.mime_type,
                result.metadata.document_id,
            )
        };

        let file_size = document_bytes.len();

        // 3. Store document if enabled
        let storage_url = if command.storage_enabled {
            let path = format!(
                "{}/{}/{}_{}.{}",
                command.tenant_id,
                command.document_type.as_str(),
                document_id,
                chrono::Utc::now().format("%Y%m%d_%H%M%S"),
                command.format.file_extension()
            );

            info!("Storing document at: {}", path);
            let url = self
                .storage
                .upload(&path, &document_bytes, &mime_type)
                .await?;
            Some(url)
        } else {
            None
        };

        // 4. Cache result for quick retrieval
        let cache_key = format!("doc:{}:{}", command.tenant_id, document_id);
        self.cache.set(&cache_key, &document_bytes, 3600).await; // 1 hour TTL

        let elapsed = start_time.elapsed();
        info!(
            "Document generation completed in {:?}ms, size: {} bytes",
            elapsed.as_millis(),
            file_size
        );

        Ok(DocumentResult {
            document_id,
            document_bytes,
            mime_type,
            storage_url,
            generation_time_ms: elapsed.as_millis() as u64,
            file_size,
        })
    }

    /// Try to generate PDF using a dynamic template from the .NET backend.
    /// Returns None if resolver is not configured or resolution/generation fails.
    async fn try_dynamic_template(&self, command: &GenerateDocumentCommand) -> Option<Vec<u8>> {
        let resolver = self.template_resolver.as_ref()?;

        let tenant_id: i64 = command.tenant_id.parse().ok()?;
        let doc_type_code = command.document_type.to_backend_code();

        match resolver.resolve(tenant_id, doc_type_code).await {
            Ok(Some(cached)) => {
                info!(
                    template_id = cached.template_id,
                    version = cached.version,
                    "Using dynamic template from backend"
                );
                match self
                    .typst_generator
                    .generate_from_template(
                        &cached.typst_source,
                        &command.data,
                        &cached.asset_dir,
                        &cached.font_paths,
                    )
                    .await
                {
                    Ok(pdf) => Some(pdf),
                    Err(e) => {
                        warn!(error = %e, "Dynamic template compilation failed, falling back to built-in");
                        None
                    }
                }
            }
            Ok(None) => {
                // No dynamic template available, use built-in
                None
            }
            Err(e) => {
                warn!(error = %e, "Template resolution error, falling back to built-in");
                None
            }
        }
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
/// Coordinates notification delivery via multiple channels.
/// Uses the `WhatsAppProvider` trait for WhatsApp integration,
/// allowing different providers (Evolution API, Green-API, etc.)
pub struct NotificationOrchestrator {
    email_service: Option<Arc<EmailService>>,
    whatsapp_service: Option<Arc<dyn WhatsAppProvider>>,
    cache: Arc<CacheService>,
}

impl NotificationOrchestrator {
    pub fn new(
        email_service: Option<Arc<EmailService>>,
        whatsapp_service: Option<Arc<dyn WhatsAppProvider>>,
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
            NotificationChannel::Email => self.send_email(&command).await?,
            NotificationChannel::Whatsapp => self.send_whatsapp(&command).await?,
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
        let email_service = self
            .email_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Email service not configured"))?;

        // Build email content from template or direct content
        let body = if let Some(template_id) = &command.template_id {
            // In production, load template and render with vars
            format!(
                "Template: {} with vars: {}",
                template_id, command.template_vars
            )
        } else {
            command
                .template_vars
                .get("body")
                .and_then(|b| b.as_str())
                .unwrap_or("")
                .to_string()
        };

        // Get HTML body if provided
        let html_body = command
            .template_vars
            .get("html_body")
            .and_then(|b| b.as_str())
            .map(|s| s.to_string());

        let subject = command
            .subject
            .clone()
            .unwrap_or_else(|| "Notification".to_string());

        // Get attachments if document_id is provided
        let attachments = if let Some(doc_id) = &command.document_id {
            // Try to get document from cache
            let cache_key = format!("doc:{}:{}", command.tenant_id, doc_id);
            if let Some(bytes) = self.cache.get::<Vec<u8>>(&cache_key).await {
                // Use document_filename if provided, otherwise fall back to doc_id
                let filename = command
                    .document_filename
                    .clone()
                    .unwrap_or_else(|| format!("{}.pdf", doc_id));
                vec![crate::infrastructure::notifications::EmailAttachment {
                    filename,
                    content_type: "application/pdf".to_string(),
                    data: bytes,
                }]
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        // Use send_email with HTML support and optional sender name override
        email_service
            .send_email_with_from_name(
                &command.recipient,
                &subject,
                &body,
                html_body.as_deref(),
                attachments,
                command.sender_name.as_deref(),
            )
            .await?;

        Ok(NotificationResult {
            notification_id: uuid::Uuid::new_v4().to_string(),
            channel: command.channel.clone(),
            status: NotificationStatus::Sent,
            error_message: None,
        })
    }

    async fn send_whatsapp(&self, command: &SendNotificationCommand) -> Result<NotificationResult> {
        let whatsapp_service = self
            .whatsapp_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("WhatsApp service not configured"))?;

        let message = command
            .template_vars
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Notification");

        whatsapp_service
            .send_simple_text(&command.recipient, message)
            .await?;

        // Send document if provided - use multipart upload (more reliable than base64)
        if let Some(doc_id) = &command.document_id {
            let cache_key = format!("doc:{}:{}", command.tenant_id, doc_id);
            if let Some(bytes) = self.cache.get::<Vec<u8>>(&cache_key).await {
                // Use document_filename if provided, otherwise fall back to doc_id
                let filename = command
                    .document_filename
                    .clone()
                    .unwrap_or_else(|| format!("{}.pdf", doc_id));
                whatsapp_service
                    .send_file(
                        &command.recipient,
                        bytes,
                        filename,
                        "application/pdf".to_string(),
                        None,
                    )
                    .await?;
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
    #[allow(unused_imports)]
    use super::*;

    // Tests would go here with mocked dependencies
}
