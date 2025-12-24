//! ERP Core message wrappers
//!
//! This module provides wrapper types for parsing messages from ERP Core
//! which uses BaseIntegrationEvent envelope with snake_case serialization.

use crate::kafka::handlers::{
    BatchNotifyRequest, GenerateAndNotifyRequest, GenerateDocumentRequest, KafkaMessage,
    SendNotificationRequest,
};
use serde::Deserialize;
use std::collections::HashMap;

/// Wrapper for messages from ERP Core with BaseIntegrationEvent envelope.
/// ERP sends messages in snake_case after the fix in KafkaMessageProducer.cs.
#[derive(Debug, Clone, Deserialize)]
pub struct ErpIntegrationEvent {
    // Fields from BaseIntegrationEvent
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub occurred_on: Option<String>,
    pub tenant_id: i64,
    #[serde(default)]
    pub event_type: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub version: Option<i32>,

    // Message type discriminator (required)
    #[serde(rename = "type")]
    pub message_type: String,

    // Common fields
    pub request_id: String,

    // DocumentGenerateRequestEvent fields
    #[serde(default)]
    pub document_type: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub template_id: Option<String>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
    #[serde(default)]
    pub store: Option<bool>,
    #[serde(default)]
    pub callback_url: Option<String>,
    #[serde(default)]
    pub notification: Option<ErpNotificationInfo>,
    #[serde(default)]
    pub metadata: Option<HashMap<String, String>>,

    // NotificationRequestEvent fields
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub recipient: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub document_id: Option<String>,

    // BatchNotificationRequestEvent fields
    #[serde(default)]
    pub recipients: Option<Vec<String>>,
    #[serde(default)]
    pub batch_id: Option<String>,
    #[serde(default)]
    pub parallel: Option<bool>,
    #[serde(default)]
    pub concurrency: Option<usize>,
}

/// Notification info for generate_and_notify messages
#[derive(Debug, Clone, Deserialize)]
pub struct ErpNotificationInfo {
    pub request_id: String,
    pub tenant_id: i64,
    pub channel: String,
    pub recipient: String,
    #[serde(default)]
    pub subject: Option<String>,
    pub message: String,
    /// Optional HTML body for email notifications.
    /// If provided, email will be sent as multipart/alternative
    /// with plain text (message) and HTML (html_body) versions.
    #[serde(default)]
    pub html_body: Option<String>,
    /// Human-readable filename for the document (e.g., "Cuadre_Caja_001.pdf")
    #[serde(default)]
    pub document_filename: Option<String>,
}

impl ErpIntegrationEvent {
    /// Convert ERP Core event to KafkaMessage format expected by handlers
    pub fn into_kafka_message(self) -> anyhow::Result<KafkaMessage> {
        let tenant_id_str = self.tenant_id.to_string();

        match self.message_type.as_str() {
            "generate_document" => Ok(KafkaMessage::GenerateDocument(GenerateDocumentRequest {
                request_id: self.request_id,
                tenant_id: tenant_id_str,
                user_id: None,
                document_type: self.document_type.unwrap_or_else(|| "invoice".to_string()),
                format: self.format.unwrap_or_else(|| "pdf".to_string()),
                template_id: self.template_id.unwrap_or_default(),
                data: self.data.unwrap_or(serde_json::json!({})),
                store: self.store.unwrap_or(true),
                callback_url: self.callback_url,
            })),

            "generate_and_notify" => {
                let notif = self.notification.ok_or_else(|| {
                    anyhow::anyhow!("generate_and_notify requires notification info")
                })?;

                let doc_request = GenerateDocumentRequest {
                    request_id: self.request_id.clone(),
                    tenant_id: tenant_id_str.clone(),
                    user_id: None,
                    document_type: self.document_type.unwrap_or_else(|| "invoice".to_string()),
                    format: self.format.unwrap_or_else(|| "pdf".to_string()),
                    template_id: self.template_id.unwrap_or_default(),
                    data: self.data.unwrap_or(serde_json::json!({})),
                    store: self.store.unwrap_or(true),
                    callback_url: self.callback_url,
                };

                let notif_request = SendNotificationRequest {
                    request_id: notif.request_id,
                    tenant_id: notif.tenant_id.to_string(),
                    channel: notif.channel,
                    recipient: notif.recipient,
                    subject: notif.subject,
                    message: notif.message,
                    html_body: notif.html_body, // Pass HTML body from ERP Core
                    document_id: None,
                    document_filename: notif.document_filename, // Pass filename from ERP Core
                    callback_url: None,
                };

                Ok(KafkaMessage::GenerateAndNotify(GenerateAndNotifyRequest {
                    request_id: self.request_id,
                    tenant_id: tenant_id_str,
                    document: doc_request,
                    notification: notif_request,
                }))
            }

            "send_notification" => {
                // Check if notification info is provided (from NotificationService with embedded notification)
                if let Some(notif) = self.notification {
                    Ok(KafkaMessage::SendNotification(SendNotificationRequest {
                        request_id: notif.request_id,
                        tenant_id: notif.tenant_id.to_string(),
                        channel: notif.channel,
                        recipient: notif.recipient,
                        subject: notif.subject,
                        message: notif.message,
                        html_body: notif.html_body,
                        document_id: None,
                        document_filename: notif.document_filename, // Pass filename from ERP Core
                        callback_url: self.callback_url,
                    }))
                } else {
                    // Fallback to top-level fields (legacy format)
                    Ok(KafkaMessage::SendNotification(SendNotificationRequest {
                        request_id: self.request_id,
                        tenant_id: tenant_id_str,
                        channel: self.channel.unwrap_or_else(|| "email".to_string()),
                        recipient: self.recipient.unwrap_or_default(),
                        subject: self.subject,
                        message: self.message.unwrap_or_default(),
                        html_body: None,
                        document_id: self.document_id,
                        document_filename: None,
                        callback_url: self.callback_url,
                    }))
                }
            }

            "batch_notify" => {
                let recipients = self
                    .recipients
                    .ok_or_else(|| anyhow::anyhow!("batch_notify requires recipients list"))?;

                Ok(KafkaMessage::BatchNotify(BatchNotifyRequest {
                    request_id: self.request_id,
                    tenant_id: tenant_id_str,
                    batch_id: self
                        .batch_id
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                    channel: self.channel.unwrap_or_else(|| "email".to_string()),
                    recipients,
                    subject: self.subject,
                    message: self.message.unwrap_or_default(),
                    document_id: self.document_id,
                    document_filename: None,
                    parallel: self.parallel.unwrap_or(true),
                    concurrency: self.concurrency,
                    callback_url: self.callback_url,
                }))
            }

            // notify_only: Legacy alias for send_notification (backward compatibility)
            "notify_only" => {
                let notif = self
                    .notification
                    .ok_or_else(|| anyhow::anyhow!("notify_only requires notification info"))?;

                Ok(KafkaMessage::SendNotification(SendNotificationRequest {
                    request_id: notif.request_id,
                    tenant_id: notif.tenant_id.to_string(),
                    channel: notif.channel,
                    recipient: notif.recipient,
                    subject: notif.subject,
                    message: notif.message,
                    html_body: notif.html_body,
                    document_id: None,
                    document_filename: notif.document_filename, // Pass filename from ERP Core
                    callback_url: self.callback_url,
                }))
            }

            other => Err(anyhow::anyhow!("Unknown ERP message type: {}", other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_erp_document_request() {
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "occurred_on": "2025-12-12T10:00:00Z",
            "tenant_id": 1,
            "event_type": "DocumentGenerateRequest",
            "source": "Core.DocumentNotification",
            "version": 1,
            "type": "generate_document",
            "request_id": "req-123",
            "document_type": "invoice",
            "format": "pdf",
            "template_id": "fiscal_invoice",
            "data": {"seller": {}, "customer": {}, "items": []},
            "store": true
        }"#;

        let event: ErpIntegrationEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.tenant_id, 1);
        assert_eq!(event.message_type, "generate_document");
        assert_eq!(event.request_id, "req-123");

        let msg = event.into_kafka_message().unwrap();
        assert!(matches!(msg, KafkaMessage::GenerateDocument(_)));

        if let KafkaMessage::GenerateDocument(req) = msg {
            assert_eq!(req.request_id, "req-123");
            assert_eq!(req.tenant_id, "1");
            assert_eq!(req.document_type, "invoice");
        }
    }

    #[test]
    fn test_parse_erp_notification_request() {
        let json = r#"{
            "tenant_id": 2,
            "event_type": "NotificationRequest",
            "source": "Core.DocumentNotification",
            "version": 1,
            "type": "send_notification",
            "request_id": "notif-456",
            "channel": "whatsapp",
            "recipient": "18091234567",
            "message": "Hello from ERP"
        }"#;

        let event: ErpIntegrationEvent = serde_json::from_str(json).unwrap();
        let msg = event.into_kafka_message().unwrap();

        assert!(matches!(msg, KafkaMessage::SendNotification(_)));

        if let KafkaMessage::SendNotification(req) = msg {
            assert_eq!(req.request_id, "notif-456");
            assert_eq!(req.tenant_id, "2");
            assert_eq!(req.channel, "whatsapp");
            assert_eq!(req.recipient, "18091234567");
        }
    }

    #[test]
    fn test_parse_erp_generate_and_notify() {
        let json = r#"{
            "tenant_id": 1,
            "type": "generate_and_notify",
            "request_id": "gen-notif-789",
            "document_type": "invoice",
            "format": "pdf",
            "template_id": "invoice_template",
            "data": {"items": []},
            "store": true,
            "notification": {
                "request_id": "notif-789",
                "tenant_id": 1,
                "channel": "email",
                "recipient": "customer@example.com",
                "subject": "Your Invoice",
                "message": "Please find your invoice attached"
            }
        }"#;

        let event: ErpIntegrationEvent = serde_json::from_str(json).unwrap();
        let msg = event.into_kafka_message().unwrap();

        assert!(matches!(msg, KafkaMessage::GenerateAndNotify(_)));
    }

    #[test]
    fn test_unknown_message_type() {
        let json = r#"{
            "tenant_id": 1,
            "type": "unknown_type",
            "request_id": "req-000"
        }"#;

        let event: ErpIntegrationEvent = serde_json::from_str(json).unwrap();
        let result = event.into_kafka_message();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown ERP message type"));
    }
}
