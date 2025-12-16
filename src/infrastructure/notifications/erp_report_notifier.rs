//! ERP Report Notification Service
//!
//! Provides notification capabilities for ERP reports via email and WhatsApp
//! Supports PDF, Excel, and CSV attachments

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use tracing::{error, info, warn};

use crate::templates::erp_report_models::{
    DeliveryMethod, DeliveryOptions, EmailDelivery, ErpReportPayload, OutputFormat, WhatsAppDelivery,
};

use super::email::{EmailAttachment, EmailService};
use super::evolution_api::{EvolutionAPIClient, MediaType, SendMediaRequest, SendTextRequest};

/// Result of delivering a report
#[derive(Debug, Clone)]
pub struct DeliveryResult {
    pub method: DeliveryMethod,
    pub successful_recipients: Vec<String>,
    pub failed_recipients: Vec<(String, String)>, // (recipient, error_message)
    pub message_ids: Vec<String>,
}

impl DeliveryResult {
    pub fn new(method: DeliveryMethod) -> Self {
        Self {
            method,
            successful_recipients: Vec::new(),
            failed_recipients: Vec::new(),
            message_ids: Vec::new(),
        }
    }

    pub fn is_success(&self) -> bool {
        !self.successful_recipients.is_empty() && self.failed_recipients.is_empty()
    }

    pub fn is_partial_success(&self) -> bool {
        !self.successful_recipients.is_empty() && !self.failed_recipients.is_empty()
    }
}

/// ERP Report Notifier - handles delivery of generated reports
pub struct ErpReportNotifier {
    email_service: Option<EmailService>,
    whatsapp_client: Option<EvolutionAPIClient>,
}

impl ErpReportNotifier {
    /// Create a new notifier with optional email and WhatsApp services
    pub fn new(
        email_service: Option<EmailService>,
        whatsapp_client: Option<EvolutionAPIClient>,
    ) -> Self {
        Self {
            email_service,
            whatsapp_client,
        }
    }

    /// Create a notifier with only email service
    pub fn email_only(email_service: EmailService) -> Self {
        Self {
            email_service: Some(email_service),
            whatsapp_client: None,
        }
    }

    /// Create a notifier with only WhatsApp service
    pub fn whatsapp_only(whatsapp_client: EvolutionAPIClient) -> Self {
        Self {
            email_service: None,
            whatsapp_client: Some(whatsapp_client),
        }
    }

    /// Deliver a report based on the payload's delivery options
    pub async fn deliver_report(
        &self,
        payload: &ErpReportPayload,
        document_bytes: Vec<u8>,
    ) -> Result<DeliveryResult> {
        let delivery = payload
            .delivery
            .as_ref()
            .context("No delivery options specified")?;

        match delivery.method {
            DeliveryMethod::Email => {
                self.deliver_via_email(payload, &delivery.email, document_bytes)
                    .await
            }
            DeliveryMethod::WhatsApp => {
                self.deliver_via_whatsapp(payload, &delivery.whatsapp, document_bytes)
                    .await
            }
            DeliveryMethod::Download | DeliveryMethod::View => {
                // These don't require notification, just return success
                Ok(DeliveryResult::new(delivery.method.clone()))
            }
        }
    }

    /// Deliver report via email
    async fn deliver_via_email(
        &self,
        payload: &ErpReportPayload,
        email_opts: &Option<EmailDelivery>,
        document_bytes: Vec<u8>,
    ) -> Result<DeliveryResult> {
        let email_service = self
            .email_service
            .as_ref()
            .context("Email service not configured")?;

        let email_delivery = email_opts
            .as_ref()
            .context("Email delivery options not specified")?;

        if email_delivery.recipients.is_empty() {
            anyhow::bail!("No email recipients specified");
        }

        let mut result = DeliveryResult::new(DeliveryMethod::Email);

        // Build email content
        let subject = email_delivery.subject.clone().unwrap_or_else(|| {
            format!(
                "Reporte: {} - {}",
                payload.report.title,
                payload.report.code
            )
        });

        let (plain_body, html_body) = self.build_report_email_content(payload, &email_delivery.message);

        // Build attachment
        let (filename, content_type) = self.get_attachment_info(payload);
        let attachment = EmailAttachment {
            filename,
            content_type,
            data: document_bytes.clone(),
        };

        // Send to each recipient
        for recipient in &email_delivery.recipients {
            info!("Sending report email to: {}", recipient);

            match email_service
                .send_email(
                    recipient,
                    &subject,
                    &plain_body,
                    Some(&html_body),
                    vec![attachment.clone()],
                )
                .await
            {
                Ok(message_id) => {
                    info!("Email sent successfully to {}: {}", recipient, message_id);
                    result.successful_recipients.push(recipient.clone());
                    result.message_ids.push(message_id);
                }
                Err(e) => {
                    error!("Failed to send email to {}: {}", recipient, e);
                    result
                        .failed_recipients
                        .push((recipient.clone(), e.to_string()));
                }
            }
        }

        if result.successful_recipients.is_empty() {
            anyhow::bail!("Failed to send email to all recipients");
        }

        Ok(result)
    }

    /// Deliver report via WhatsApp
    async fn deliver_via_whatsapp(
        &self,
        payload: &ErpReportPayload,
        whatsapp_opts: &Option<WhatsAppDelivery>,
        document_bytes: Vec<u8>,
    ) -> Result<DeliveryResult> {
        let whatsapp_client = self
            .whatsapp_client
            .as_ref()
            .context("WhatsApp service not configured")?;

        let whatsapp_delivery = whatsapp_opts
            .as_ref()
            .context("WhatsApp delivery options not specified")?;

        if whatsapp_delivery.numbers.is_empty() {
            anyhow::bail!("No WhatsApp numbers specified");
        }

        let mut result = DeliveryResult::new(DeliveryMethod::WhatsApp);

        // Build message content
        let text_message = self.build_whatsapp_text_message(payload, &whatsapp_delivery.message);

        // Get file info
        let (filename, mimetype) = self.get_attachment_info(payload);
        let base64_content = BASE64.encode(&document_bytes);

        // Build caption
        let caption = format!(
            "{} - {}",
            payload.report.title,
            self.get_format_label(&payload.output.format)
        );

        // Send to each number
        for number in &whatsapp_delivery.numbers {
            info!("Sending report via WhatsApp to: {}", number);

            // First send text message
            let text_request = SendTextRequest {
                number: super::evolution_api::normalize_dominican_phone(number),
                text: text_message.clone(),
                delay: None,
                link_preview: Some(false),
                mentioned: None,
                mentions_every_one: None,
                quoted: None,
            };

            match whatsapp_client.send_text(text_request).await {
                Ok(text_id) => {
                    info!("WhatsApp text sent to {}: {}", number, text_id);

                    // Now send the document
                    let media_request = SendMediaRequest {
                        number: super::evolution_api::normalize_dominican_phone(number),
                        mediatype: MediaType::Document,
                        mimetype: mimetype.clone(),
                        caption: caption.clone(),
                        media: base64_content.clone(),
                        file_name: filename.clone(),
                        delay: Some(1000), // Small delay after text
                        link_preview: Some(false),
                        mentions_every_one: Some(false),
                    };

                    match whatsapp_client.send_media(media_request).await {
                        Ok(doc_id) => {
                            info!("WhatsApp document sent to {}: {}", number, doc_id);
                            result.successful_recipients.push(number.clone());
                            result.message_ids.push(format!("text:{},doc:{}", text_id, doc_id));
                        }
                        Err(e) => {
                            warn!(
                                "Text sent but document failed for {}: {}",
                                number, e
                            );
                            result
                                .failed_recipients
                                .push((number.clone(), format!("Document failed: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to send WhatsApp to {}: {}", number, e);
                    result
                        .failed_recipients
                        .push((number.clone(), e.to_string()));
                }
            }
        }

        if result.successful_recipients.is_empty() {
            anyhow::bail!("Failed to send WhatsApp to all numbers");
        }

        Ok(result)
    }

    /// Build plain text and HTML email content for report
    fn build_report_email_content(
        &self,
        payload: &ErpReportPayload,
        custom_message: &Option<String>,
    ) -> (String, String) {
        let report_title = &payload.report.title;
        let report_code = &payload.report.code;
        let format_label = self.get_format_label(&payload.output.format);
        let generated_at = &payload.report.generated_at;

        // Date range info
        let date_info = if let Some(ref range) = payload.report.date_range {
            format!("Período: {} al {}", range.from, range.to)
        } else if let Some(ref date) = payload.report.as_of_date {
            format!("Fecha de corte: {}", date)
        } else {
            "".to_string()
        };

        // Custom message or default
        let intro = custom_message.clone().unwrap_or_else(|| {
            "Se adjunta el reporte solicitado.".to_string()
        });

        // Plain text version
        let plain_body = format!(
            r#"REPORTE: {}

{}

Detalles del reporte:
- Código: {}
- Formato: {}
- Registros: {}
{}
- Generado: {}

El archivo se encuentra adjunto a este correo.

---
Este es un correo automático generado por Facturazo ERP.
Por favor no responda a este mensaje."#,
            report_title,
            intro,
            report_code,
            format_label,
            payload.data.total_records,
            if !date_info.is_empty() {
                format!("- {}\n", date_info)
            } else {
                "".to_string()
            },
            generated_at
        );

        // HTML version
        let html_body = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body {{
            font-family: 'Segoe UI', Arial, sans-serif;
            line-height: 1.6;
            color: #333;
            max-width: 600px;
            margin: 0 auto;
            padding: 20px;
        }}
        .header {{
            background: linear-gradient(135deg, #1a365d 0%, #2c5282 100%);
            color: white;
            padding: 25px;
            text-align: center;
            border-radius: 8px 8px 0 0;
        }}
        .header h1 {{
            margin: 0;
            font-size: 24px;
            font-weight: 600;
        }}
        .header .subtitle {{
            margin-top: 5px;
            opacity: 0.9;
            font-size: 14px;
        }}
        .content {{
            padding: 25px;
            background: #ffffff;
            border: 1px solid #e2e8f0;
            border-top: none;
        }}
        .intro {{
            font-size: 16px;
            margin-bottom: 20px;
        }}
        .details {{
            background-color: #f7fafc;
            padding: 20px;
            border-radius: 6px;
            margin: 20px 0;
            border-left: 4px solid #3182ce;
        }}
        .details-row {{
            display: flex;
            justify-content: space-between;
            padding: 8px 0;
            border-bottom: 1px solid #e2e8f0;
        }}
        .details-row:last-child {{
            border-bottom: none;
        }}
        .details-label {{
            font-weight: 600;
            color: #4a5568;
        }}
        .details-value {{
            color: #2d3748;
        }}
        .badge {{
            display: inline-block;
            padding: 4px 12px;
            border-radius: 20px;
            font-size: 12px;
            font-weight: 600;
            text-transform: uppercase;
        }}
        .badge-pdf {{ background: #fed7d7; color: #c53030; }}
        .badge-xlsx {{ background: #c6f6d5; color: #276749; }}
        .badge-csv {{ background: #bee3f8; color: #2b6cb0; }}
        .attachment-notice {{
            background: #ebf8ff;
            border: 1px solid #90cdf4;
            border-radius: 6px;
            padding: 15px;
            text-align: center;
            margin: 20px 0;
        }}
        .attachment-notice svg {{
            width: 24px;
            height: 24px;
            fill: #3182ce;
        }}
        .footer {{
            color: #718096;
            font-size: 12px;
            padding: 20px;
            text-align: center;
            background: #f7fafc;
            border-radius: 0 0 8px 8px;
            border: 1px solid #e2e8f0;
            border-top: none;
        }}
    </style>
</head>
<body>
    <div class="header">
        <h1>{}</h1>
        <div class="subtitle">{}</div>
    </div>
    <div class="content">
        <p class="intro">{}</p>

        <div class="details">
            <div class="details-row">
                <span class="details-label">Formato</span>
                <span class="details-value"><span class="badge badge-{}">{}</span></span>
            </div>
            <div class="details-row">
                <span class="details-label">Total de registros</span>
                <span class="details-value">{}</span>
            </div>
            {}
            <div class="details-row">
                <span class="details-label">Generado</span>
                <span class="details-value">{}</span>
            </div>
        </div>

        <div class="attachment-notice">
            <p style="margin: 0;"><strong>El archivo se encuentra adjunto a este correo.</strong></p>
        </div>
    </div>
    <div class="footer">
        <p>Este es un correo automático generado por <strong>Facturazo ERP</strong>.</p>
        <p>Por favor no responda a este mensaje.</p>
    </div>
</body>
</html>"#,
            report_title,
            report_code,
            intro,
            format_label.to_lowercase(),
            format_label,
            payload.data.total_records,
            if !date_info.is_empty() {
                format!(
                    r#"<div class="details-row">
                <span class="details-label">{}</span>
                <span class="details-value">{}</span>
            </div>"#,
                    if payload.report.date_range.is_some() {
                        "Período"
                    } else {
                        "Fecha de corte"
                    },
                    if let Some(ref range) = payload.report.date_range {
                        format!("{} al {}", range.from, range.to)
                    } else {
                        payload.report.as_of_date.clone().unwrap_or_default()
                    }
                )
            } else {
                "".to_string()
            },
            generated_at
        );

        (plain_body, html_body)
    }

    /// Build WhatsApp text message for report
    fn build_whatsapp_text_message(
        &self,
        payload: &ErpReportPayload,
        custom_message: &Option<String>,
    ) -> String {
        let format_label = self.get_format_label(&payload.output.format);
        let format_emoji = self.get_format_emoji(&payload.output.format);

        // Date range info
        let date_info = if let Some(ref range) = payload.report.date_range {
            format!("📅 *Período:* {} al {}", range.from, range.to)
        } else if let Some(ref date) = payload.report.as_of_date {
            format!("📅 *Fecha de corte:* {}", date)
        } else {
            "".to_string()
        };

        let custom_msg = custom_message
            .clone()
            .map(|m| format!("\n\n{}", m))
            .unwrap_or_default();

        format!(
            r#"📊 *REPORTE GENERADO*

*{}*
_{}_

{} *Formato:* {}
📁 *Registros:* {}
{}{}

A continuación recibirá el documento."#,
            payload.report.title,
            payload.report.code,
            format_emoji,
            format_label,
            payload.data.total_records,
            if !date_info.is_empty() {
                format!("{}\n", date_info)
            } else {
                "".to_string()
            },
            custom_msg
        )
    }

    /// Get filename and content type for attachment
    fn get_attachment_info(&self, payload: &ErpReportPayload) -> (String, String) {
        let base_name = payload
            .output
            .file_name
            .clone()
            .unwrap_or_else(|| {
                format!(
                    "{}_{}_{}",
                    payload.report.code,
                    payload
                        .report
                        .variant
                        .clone()
                        .unwrap_or_else(|| "REPORT".to_string()),
                    chrono::Local::now().format("%Y%m%d_%H%M%S")
                )
            });

        match payload.output.format {
            OutputFormat::Pdf => (format!("{}.pdf", base_name), "application/pdf".to_string()),
            OutputFormat::Xlsx => (
                format!("{}.xlsx", base_name),
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
            ),
            OutputFormat::Csv => (format!("{}.csv", base_name), "text/csv".to_string()),
            OutputFormat::Html => (format!("{}.html", base_name), "text/html".to_string()),
        }
    }

    /// Get human-readable format label
    fn get_format_label(&self, format: &OutputFormat) -> String {
        match format {
            OutputFormat::Pdf => "PDF".to_string(),
            OutputFormat::Xlsx => "Excel".to_string(),
            OutputFormat::Csv => "CSV".to_string(),
            OutputFormat::Html => "HTML".to_string(),
        }
    }

    /// Get emoji for format
    fn get_format_emoji(&self, format: &OutputFormat) -> &str {
        match format {
            OutputFormat::Pdf => "📄",
            OutputFormat::Xlsx => "📊",
            OutputFormat::Csv => "📋",
            OutputFormat::Html => "🌐",
        }
    }
}

/// Builder for creating ErpReportNotifier with services from configuration
pub struct ErpReportNotifierBuilder {
    email_service: Option<EmailService>,
    whatsapp_client: Option<EvolutionAPIClient>,
}

impl ErpReportNotifierBuilder {
    pub fn new() -> Self {
        Self {
            email_service: None,
            whatsapp_client: None,
        }
    }

    /// Configure email service
    pub fn with_email(
        mut self,
        smtp_host: String,
        smtp_port: u16,
        smtp_username: String,
        smtp_password: String,
        from_email: String,
        from_name: String,
        use_tls: bool,
    ) -> Self {
        self.email_service = Some(EmailService::new(
            smtp_host,
            smtp_port,
            smtp_username,
            smtp_password,
            from_email,
            from_name,
            use_tls,
        ));
        self
    }

    /// Configure WhatsApp service
    pub fn with_whatsapp(
        mut self,
        base_url: String,
        api_key: String,
        instance: String,
    ) -> Self {
        self.whatsapp_client = Some(EvolutionAPIClient::new(base_url, api_key, instance));
        self
    }

    /// Build the notifier
    pub fn build(self) -> ErpReportNotifier {
        ErpReportNotifier::new(self.email_service, self.whatsapp_client)
    }
}

impl Default for ErpReportNotifierBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_attachment_info_pdf() {
        let notifier = ErpReportNotifier::new(None, None);

        let json = r#"{
            "tenantId": 1,
            "userId": 1,
            "report": { "code": "TEST", "title": "Test", "generatedAt": "2025-01-01" },
            "metadata": { "columns": [] },
            "data": { "structureType": "Flat", "totalRecords": 10 },
            "output": { "format": "Pdf", "fileName": "my_report" }
        }"#;
        let payload: ErpReportPayload = serde_json::from_str(json).unwrap();

        let (filename, content_type) = notifier.get_attachment_info(&payload);
        assert_eq!(filename, "my_report.pdf");
        assert_eq!(content_type, "application/pdf");
    }

    #[test]
    fn test_get_attachment_info_xlsx() {
        let notifier = ErpReportNotifier::new(None, None);

        let json = r#"{
            "tenantId": 1,
            "userId": 1,
            "report": { "code": "TEST", "title": "Test", "generatedAt": "2025-01-01" },
            "metadata": { "columns": [] },
            "data": { "structureType": "Flat", "totalRecords": 10 },
            "output": { "format": "Xlsx", "fileName": "my_report" }
        }"#;
        let payload: ErpReportPayload = serde_json::from_str(json).unwrap();

        let (filename, content_type) = notifier.get_attachment_info(&payload);
        assert_eq!(filename, "my_report.xlsx");
        assert!(content_type.contains("spreadsheetml"));
    }

    #[test]
    fn test_delivery_result_success() {
        let mut result = DeliveryResult::new(DeliveryMethod::Email);
        result.successful_recipients.push("test@example.com".to_string());

        assert!(result.is_success());
        assert!(!result.is_partial_success());
    }

    #[test]
    fn test_delivery_result_partial() {
        let mut result = DeliveryResult::new(DeliveryMethod::Email);
        result.successful_recipients.push("test1@example.com".to_string());
        result.failed_recipients.push(("test2@example.com".to_string(), "Failed".to_string()));

        assert!(!result.is_success());
        assert!(result.is_partial_success());
    }
}
