//! Kafka message handlers
//!
//! Handlers process incoming Kafka messages and route them to orchestrators

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};

use crate::application::commands::{GenerateDocumentCommand, Priority, SendNotificationCommand};
use crate::application::orchestrators::{DocumentOrchestrator, NotificationOrchestrator};
use crate::domain::document::{DocumentFormat, DocumentType};
use crate::domain::notification::NotificationChannel;

/// Kafka message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KafkaMessage {
    /// Generate a document (PDF, Excel, CSV)
    GenerateDocument(GenerateDocumentRequest),
    /// Send a notification (Email, WhatsApp, SMS)
    SendNotification(SendNotificationRequest),
    /// Generate document and send notification
    GenerateAndNotify(GenerateAndNotifyRequest),
    /// Batch document generation
    BatchGenerate(BatchGenerateRequest),
    /// Batch notifications to multiple recipients
    BatchNotify(BatchNotifyRequest),
    /// Generate document and notify multiple recipients
    GenerateAndNotifyMany(GenerateAndNotifyManyRequest),
}

/// Request to generate a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateDocumentRequest {
    pub request_id: String,
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub document_type: String,
    pub format: String,
    pub template_id: String,
    pub data: serde_json::Value,
    pub store: bool,
    pub callback_url: Option<String>,
}

/// Request to send a notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendNotificationRequest {
    pub request_id: String,
    pub tenant_id: String,
    pub channel: String,
    pub recipient: String,
    pub subject: Option<String>,
    pub message: String,
    /// Optional HTML body for email notifications
    pub html_body: Option<String>,
    pub document_id: Option<String>,
    pub callback_url: Option<String>,
}

/// Request to generate and notify
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateAndNotifyRequest {
    pub request_id: String,
    pub tenant_id: String,
    pub document: GenerateDocumentRequest,
    pub notification: SendNotificationRequest,
}

/// Request for batch generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchGenerateRequest {
    pub request_id: String,
    pub tenant_id: String,
    pub batch_id: String,
    pub documents: Vec<GenerateDocumentRequest>,
    pub parallel: bool,
    pub callback_url: Option<String>,
}

/// Request for batch notifications to multiple recipients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchNotifyRequest {
    pub request_id: String,
    pub tenant_id: String,
    pub batch_id: String,
    pub channel: String,
    /// Multiple recipients - can be phone numbers or emails
    pub recipients: Vec<String>,
    pub subject: Option<String>,
    pub message: String,
    /// Optional: attach a document to all notifications
    pub document_id: Option<String>,
    /// Process in parallel (recommended for large batches)
    pub parallel: bool,
    /// Max concurrent sends (default: 10)
    pub concurrency: Option<usize>,
    pub callback_url: Option<String>,
}

/// Request to generate one document and notify multiple recipients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateAndNotifyManyRequest {
    pub request_id: String,
    pub tenant_id: String,
    pub document: GenerateDocumentRequest,
    pub notification: NotificationTemplate,
}

/// Notification template for multiple recipients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationTemplate {
    pub channel: String,
    /// Multiple recipients
    pub recipients: Vec<String>,
    pub subject: Option<String>,
    pub message: String,
    /// Process in parallel
    pub parallel: bool,
    /// Max concurrent sends (default: 10)
    pub concurrency: Option<usize>,
}

/// Kafka message handler
pub struct KafkaHandler {
    document_orchestrator: Arc<DocumentOrchestrator>,
    notification_orchestrator: Arc<NotificationOrchestrator>,
}

impl KafkaHandler {
    pub fn new(
        document_orchestrator: Arc<DocumentOrchestrator>,
        notification_orchestrator: Arc<NotificationOrchestrator>,
    ) -> Self {
        Self {
            document_orchestrator,
            notification_orchestrator,
        }
    }

    /// Handle incoming Kafka message
    #[instrument(skip(self, message), fields(message_type = ?std::mem::discriminant(&message)))]
    pub async fn handle(&self, message: KafkaMessage) -> Result<HandlerResponse> {
        match message {
            KafkaMessage::GenerateDocument(req) => self.handle_generate_document(req).await,
            KafkaMessage::SendNotification(req) => self.handle_send_notification(req).await,
            KafkaMessage::GenerateAndNotify(req) => self.handle_generate_and_notify(req).await,
            KafkaMessage::BatchGenerate(req) => self.handle_batch_generate(req).await,
            KafkaMessage::BatchNotify(req) => self.handle_batch_notify(req).await,
            KafkaMessage::GenerateAndNotifyMany(req) => {
                self.handle_generate_and_notify_many(req).await
            }
        }
    }

    async fn handle_generate_document(
        &self,
        req: GenerateDocumentRequest,
    ) -> Result<HandlerResponse> {
        info!("Handling GenerateDocument: {}", req.request_id);

        let command = GenerateDocumentCommand {
            tenant_id: req.tenant_id.clone(),
            user_id: req.user_id,
            document_type: parse_document_type(&req.document_type),
            template_id: req.template_id,
            template_version: "latest".to_string(),
            data: req.data,
            format: parse_format(&req.format),
            storage_enabled: req.store,
            notification_enabled: false,
            priority: Priority::Normal,
        };

        let result = self.document_orchestrator.execute(command).await?;

        Ok(HandlerResponse::DocumentGenerated {
            request_id: req.request_id,
            document_id: result.document_id,
            storage_url: result.storage_url,
            file_size: result.file_size,
            generation_time_ms: result.generation_time_ms,
        })
    }

    async fn handle_send_notification(
        &self,
        req: SendNotificationRequest,
    ) -> Result<HandlerResponse> {
        info!("Handling SendNotification: {}", req.request_id);

        // Build template_vars with body and optional html_body
        let mut template_vars = serde_json::json!({
            "message": req.message,
            "body": req.message
        });

        // Add html_body if provided
        if let Some(html) = &req.html_body {
            template_vars["html_body"] = serde_json::Value::String(html.clone());
        }

        let command = SendNotificationCommand {
            tenant_id: req.tenant_id.clone(),
            channel: parse_channel(&req.channel),
            recipient: req.recipient,
            subject: req.subject,
            template_id: None,
            template_vars,
            priority: crate::domain::notification::NotificationPriority::Normal,
            document_id: req.document_id,
            attachments: vec![],
        };

        let result = self.notification_orchestrator.execute(command).await?;

        Ok(HandlerResponse::NotificationSent {
            request_id: req.request_id,
            notification_id: result.notification_id,
            status: format!("{:?}", result.status),
        })
    }

    async fn handle_generate_and_notify(
        &self,
        req: GenerateAndNotifyRequest,
    ) -> Result<HandlerResponse> {
        info!("Handling GenerateAndNotify: {}", req.request_id);

        // 1. Generate document
        let doc_result = self.handle_generate_document(req.document.clone()).await?;

        // 2. Get document_id from result
        let document_id = match &doc_result {
            HandlerResponse::DocumentGenerated { document_id, .. } => document_id.clone(),
            _ => return Err(anyhow::anyhow!("Unexpected response type")),
        };

        // 3. Send notification with document attached
        let mut notification_req = req.notification;
        notification_req.document_id = Some(document_id.clone());

        let notif_result = self.handle_send_notification(notification_req).await?;

        Ok(HandlerResponse::GenerateAndNotifyCompleted {
            request_id: req.request_id,
            document_id,
            notification_id: match notif_result {
                HandlerResponse::NotificationSent {
                    notification_id, ..
                } => notification_id,
                _ => "unknown".to_string(),
            },
        })
    }

    async fn handle_batch_generate(&self, req: BatchGenerateRequest) -> Result<HandlerResponse> {
        info!(
            "Handling BatchGenerate: {} with {} documents",
            req.batch_id,
            req.documents.len()
        );

        let mut results = Vec::new();
        let mut failed = 0;

        if req.parallel {
            // Process in parallel
            let futures: Vec<_> = req
                .documents
                .iter()
                .map(|doc| self.handle_generate_document(doc.clone()))
                .collect();

            let batch_results = futures::future::join_all(futures).await;

            for result in batch_results {
                match result {
                    Ok(resp) => results.push(resp),
                    Err(e) => {
                        error!("Batch document generation failed: {}", e);
                        failed += 1;
                    }
                }
            }
        } else {
            // Process sequentially
            for doc in req.documents {
                match self.handle_generate_document(doc).await {
                    Ok(resp) => results.push(resp),
                    Err(e) => {
                        error!("Batch document generation failed: {}", e);
                        failed += 1;
                    }
                }
            }
        }

        Ok(HandlerResponse::BatchCompleted {
            request_id: req.request_id,
            batch_id: req.batch_id,
            total: results.len() + failed,
            successful: results.len(),
            failed,
        })
    }

    /// Handle batch notifications to multiple recipients
    async fn handle_batch_notify(&self, req: BatchNotifyRequest) -> Result<HandlerResponse> {
        info!(
            "Handling BatchNotify: {} to {} recipients",
            req.batch_id,
            req.recipients.len()
        );

        let concurrency = req.concurrency.unwrap_or(10);
        let mut successful = 0;
        let mut failed = 0;
        let mut notification_ids = Vec::new();

        if req.parallel {
            // Process in parallel with concurrency limit using chunks
            for chunk in req.recipients.chunks(concurrency) {
                let futures: Vec<_> = chunk
                    .iter()
                    .enumerate()
                    .map(|(i, recipient)| {
                        let notif_req = SendNotificationRequest {
                            request_id: format!("{}-{}", req.request_id, i),
                            tenant_id: req.tenant_id.clone(),
                            channel: req.channel.clone(),
                            recipient: recipient.clone(),
                            subject: req.subject.clone(),
                            message: req.message.clone(),
                            html_body: None,
                            document_id: req.document_id.clone(),
                            callback_url: None,
                        };
                        self.handle_send_notification(notif_req)
                    })
                    .collect();

                let chunk_results = futures::future::join_all(futures).await;

                for result in chunk_results {
                    match result {
                        Ok(HandlerResponse::NotificationSent {
                            notification_id, ..
                        }) => {
                            successful += 1;
                            notification_ids.push(notification_id);
                        }
                        Ok(_) => successful += 1,
                        Err(e) => {
                            error!("Batch notification failed: {}", e);
                            failed += 1;
                        }
                    }
                }
            }
        } else {
            // Process sequentially
            for (i, recipient) in req.recipients.iter().enumerate() {
                let notif_req = SendNotificationRequest {
                    request_id: format!("{}-{}", req.request_id, i),
                    tenant_id: req.tenant_id.clone(),
                    channel: req.channel.clone(),
                    recipient: recipient.clone(),
                    subject: req.subject.clone(),
                    message: req.message.clone(),
                    html_body: None,
                    document_id: req.document_id.clone(),
                    callback_url: None,
                };

                match self.handle_send_notification(notif_req).await {
                    Ok(HandlerResponse::NotificationSent {
                        notification_id, ..
                    }) => {
                        successful += 1;
                        notification_ids.push(notification_id);
                    }
                    Ok(_) => successful += 1,
                    Err(e) => {
                        error!("Batch notification to {} failed: {}", recipient, e);
                        failed += 1;
                    }
                }
            }
        }

        Ok(HandlerResponse::BatchNotifyCompleted {
            request_id: req.request_id,
            batch_id: req.batch_id,
            total: successful + failed,
            successful,
            failed,
            notification_ids,
        })
    }

    /// Handle generate document and notify multiple recipients
    async fn handle_generate_and_notify_many(
        &self,
        req: GenerateAndNotifyManyRequest,
    ) -> Result<HandlerResponse> {
        info!(
            "Handling GenerateAndNotifyMany: {} to {} recipients",
            req.request_id,
            req.notification.recipients.len()
        );

        // 1. Generate document first
        let doc_result = self.handle_generate_document(req.document.clone()).await?;

        let document_id = match &doc_result {
            HandlerResponse::DocumentGenerated { document_id, .. } => document_id.clone(),
            _ => return Err(anyhow::anyhow!("Unexpected response type")),
        };

        // 2. Send notifications to all recipients
        let batch_notify_req = BatchNotifyRequest {
            request_id: format!("{}-notify", req.request_id),
            tenant_id: req.tenant_id.clone(),
            batch_id: format!("{}-batch", req.request_id),
            channel: req.notification.channel,
            recipients: req.notification.recipients,
            subject: req.notification.subject,
            message: req.notification.message,
            document_id: Some(document_id.clone()),
            parallel: req.notification.parallel,
            concurrency: req.notification.concurrency,
            callback_url: None,
        };

        let notify_result = self.handle_batch_notify(batch_notify_req).await?;

        let (notifications_sent, notifications_failed) = match &notify_result {
            HandlerResponse::BatchNotifyCompleted {
                successful, failed, ..
            } => (*successful, *failed),
            _ => (0, 0),
        };

        Ok(HandlerResponse::GenerateAndNotifyManyCompleted {
            request_id: req.request_id,
            document_id,
            total_recipients: notifications_sent + notifications_failed,
            notifications_sent,
            notifications_failed,
        })
    }
}

/// Handler response types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HandlerResponse {
    DocumentGenerated {
        request_id: String,
        document_id: String,
        storage_url: Option<String>,
        file_size: usize,
        generation_time_ms: u64,
    },
    NotificationSent {
        request_id: String,
        notification_id: String,
        status: String,
    },
    GenerateAndNotifyCompleted {
        request_id: String,
        document_id: String,
        notification_id: String,
    },
    BatchCompleted {
        request_id: String,
        batch_id: String,
        total: usize,
        successful: usize,
        failed: usize,
    },
    /// Response for batch notifications
    BatchNotifyCompleted {
        request_id: String,
        batch_id: String,
        total: usize,
        successful: usize,
        failed: usize,
        notification_ids: Vec<String>,
    },
    /// Response for generate and notify many
    GenerateAndNotifyManyCompleted {
        request_id: String,
        document_id: String,
        total_recipients: usize,
        notifications_sent: usize,
        notifications_failed: usize,
    },
}

// Helper functions
fn parse_document_type(s: &str) -> DocumentType {
    match s.to_lowercase().as_str() {
        "invoice" => DocumentType::Invoice,
        "quotation" => DocumentType::Quotation,
        "report" => DocumentType::Report,
        "receipt" => DocumentType::Receipt,
        other => DocumentType::Custom(other.to_string()),
    }
}

fn parse_format(s: &str) -> DocumentFormat {
    match s.to_lowercase().as_str() {
        "pdf" => DocumentFormat::Pdf,
        "excel" | "xlsx" => DocumentFormat::Excel,
        "csv" => DocumentFormat::Csv,
        "html" => DocumentFormat::Html,
        _ => DocumentFormat::Pdf,
    }
}

fn parse_channel(s: &str) -> NotificationChannel {
    match s.to_lowercase().as_str() {
        "email" => NotificationChannel::Email,
        "whatsapp" | "wa" => NotificationChannel::Whatsapp,
        "sms" => NotificationChannel::Sms,
        _ => NotificationChannel::Email,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message() {
        let json = r#"{
            "type": "generate_document",
            "request_id": "req-123",
            "tenant_id": "tenant-1",
            "document_type": "invoice",
            "format": "pdf",
            "template_id": "invoice_fiscal",
            "data": {"seller": {}, "customer": {}, "items": []},
            "store": true
        }"#;

        let message: KafkaMessage = serde_json::from_str(json).unwrap();

        match message {
            KafkaMessage::GenerateDocument(req) => {
                assert_eq!(req.request_id, "req-123");
                assert_eq!(req.document_type, "invoice");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_parse_document_type() {
        assert!(matches!(
            parse_document_type("invoice"),
            DocumentType::Invoice
        ));
        assert!(matches!(
            parse_document_type("INVOICE"),
            DocumentType::Invoice
        ));
        assert!(matches!(
            parse_document_type("quotation"),
            DocumentType::Quotation
        ));
        assert!(matches!(
            parse_document_type("custom_type"),
            DocumentType::Custom(_)
        ));
    }

    #[test]
    fn test_parse_format() {
        assert!(matches!(parse_format("pdf"), DocumentFormat::Pdf));
        assert!(matches!(parse_format("excel"), DocumentFormat::Excel));
        assert!(matches!(parse_format("xlsx"), DocumentFormat::Excel));
        assert!(matches!(parse_format("csv"), DocumentFormat::Csv));
    }
}
