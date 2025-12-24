//! ERP Report Notification Service
//!
//! Provides notification capabilities for ERP reports via email and WhatsApp
//! Supports PDF, Excel, and CSV attachments

use anyhow::{Context, Result};
use tracing::{error, info, warn};

use crate::templates::erp_report_models::{
    DeliveryMethod, EmailDelivery, ErpReportPayload, OutputFormat, WhatsAppDelivery,
};

use std::sync::Arc;

use super::email::{EmailAttachment, EmailService};
use super::WhatsAppProvider;

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
///
/// Uses the `WhatsAppProvider` trait for WhatsApp integration,
/// allowing different providers (Evolution API, Green-API, etc.)
pub struct ErpReportNotifier {
    email_service: Option<EmailService>,
    whatsapp_provider: Option<Arc<dyn WhatsAppProvider>>,
}

impl ErpReportNotifier {
    /// Create a new notifier with optional email and WhatsApp services
    pub fn new(
        email_service: Option<EmailService>,
        whatsapp_provider: Option<Arc<dyn WhatsAppProvider>>,
    ) -> Self {
        Self {
            email_service,
            whatsapp_provider,
        }
    }

    /// Create a notifier with only email service
    pub fn email_only(email_service: EmailService) -> Self {
        Self {
            email_service: Some(email_service),
            whatsapp_provider: None,
        }
    }

    /// Create a notifier with only WhatsApp service
    pub fn whatsapp_only(whatsapp_provider: Arc<dyn WhatsAppProvider>) -> Self {
        Self {
            email_service: None,
            whatsapp_provider: Some(whatsapp_provider),
        }
    }

    /// Deliver a report based on the payload's delivery options
    ///
    /// # Arguments
    /// * `payload` - The report payload with delivery configuration
    /// * `document_bytes` - The generated report file bytes
    /// * `download_url` - Optional download URL. If provided, sends link instead of attachment
    ///                    (used when file size exceeds attachment limit, typically 10 MB)
    pub async fn deliver_report(
        &self,
        payload: &ErpReportPayload,
        document_bytes: Vec<u8>,
        download_url: Option<String>,
    ) -> Result<DeliveryResult> {
        let delivery = payload
            .delivery
            .as_ref()
            .context("No delivery options specified")?;

        match delivery.method {
            DeliveryMethod::Email => {
                self.deliver_via_email(payload, &delivery.email, document_bytes, download_url)
                    .await
            }
            DeliveryMethod::WhatsApp => {
                self.deliver_via_whatsapp(payload, &delivery.whatsapp, document_bytes, download_url)
                    .await
            }
            DeliveryMethod::Download | DeliveryMethod::View => {
                // These don't require notification, just return success
                Ok(DeliveryResult::new(delivery.method.clone()))
            }
        }
    }

    /// Deliver report via email
    ///
    /// If `download_url` is provided, sends a link instead of attaching the file
    /// (used for large files that exceed email attachment limits)
    async fn deliver_via_email(
        &self,
        payload: &ErpReportPayload,
        email_opts: &Option<EmailDelivery>,
        document_bytes: Vec<u8>,
        download_url: Option<String>,
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

        // Build email content - different template based on attachment vs link
        let subject = email_delivery.subject.clone().unwrap_or_else(|| {
            format!(
                "Reporte: {} - {}",
                payload.report.title, payload.report.code
            )
        });

        // Determine if we're sending attachment or download link
        let (plain_body, html_body, attachments) = if let Some(ref url) = download_url {
            // Large file - send download link instead of attachment
            info!(
                "Sending report with download link (file too large for attachment): {}",
                url
            );
            let (plain, html) =
                self.build_report_email_content_with_link(payload, &email_delivery.message, url);
            (plain, html, vec![])
        } else {
            // Normal file - attach directly
            let (plain, html) = self.build_report_email_content(payload, &email_delivery.message);
            let (filename, content_type) = self.get_attachment_info(payload);
            let attachment = EmailAttachment {
                filename,
                content_type,
                data: document_bytes.clone(),
            };
            (plain, html, vec![attachment])
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
                    attachments.clone(),
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
    ///
    /// If `download_url` is provided, sends a link instead of attaching the file
    /// (used for large files)
    async fn deliver_via_whatsapp(
        &self,
        payload: &ErpReportPayload,
        whatsapp_opts: &Option<WhatsAppDelivery>,
        document_bytes: Vec<u8>,
        download_url: Option<String>,
    ) -> Result<DeliveryResult> {
        let whatsapp_provider = self
            .whatsapp_provider
            .as_ref()
            .context("WhatsApp service not configured")?;

        let whatsapp_delivery = whatsapp_opts
            .as_ref()
            .context("WhatsApp delivery options not specified")?;

        if whatsapp_delivery.numbers.is_empty() {
            anyhow::bail!("No WhatsApp numbers specified");
        }

        let mut result = DeliveryResult::new(DeliveryMethod::WhatsApp);

        // Build message content - include download link if file is too large
        let text_message = if let Some(ref url) = download_url {
            self.build_whatsapp_text_message_with_link(payload, &whatsapp_delivery.message, url)
        } else {
            self.build_whatsapp_text_message(payload, &whatsapp_delivery.message)
        };

        // Get file info
        let (filename, _mimetype) = self.get_attachment_info(payload);

        info!(
            "Using WhatsApp provider: {}",
            whatsapp_provider.provider_name()
        );

        // Send to each number
        for number in &whatsapp_delivery.numbers {
            info!("Sending report via WhatsApp to: {}", number);

            // First send text message using the trait method
            match whatsapp_provider
                .send_simple_text(number, &text_message)
                .await
            {
                Ok(text_id) => {
                    info!("WhatsApp text sent to {}: {}", number, text_id);

                    // Only send document if we don't have a download URL (file is small enough)
                    if download_url.is_none() {
                        let caption = format!(
                            "{} - {}",
                            payload.report.title,
                            self.get_format_label(&payload.output.format)
                        );

                        // Use the trait method for sending files
                        let mimetype = self.get_mimetype(&payload.output.format);
                        match whatsapp_provider
                            .send_file(
                                number,
                                document_bytes.clone(),
                                filename.clone(),
                                mimetype,
                                Some(caption),
                            )
                            .await
                        {
                            Ok(doc_id) => {
                                info!("WhatsApp document sent to {}: {}", number, doc_id);
                                result.successful_recipients.push(number.clone());
                                result
                                    .message_ids
                                    .push(format!("text:{},doc:{}", text_id, doc_id));
                            }
                            Err(e) => {
                                warn!("Text sent but document failed for {}: {}", number, e);
                                result
                                    .failed_recipients
                                    .push((number.clone(), format!("Document failed: {}", e)));
                            }
                        }
                    } else {
                        // File was too large, we already sent the download link in the text message
                        info!(
                            "WhatsApp link sent to {}: {} (file too large for direct send)",
                            number, text_id
                        );
                        result.successful_recipients.push(number.clone());
                        result.message_ids.push(format!("text:{}", text_id));
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

        // Company/Tenant name
        let company_name = payload
            .company_info
            .as_ref()
            .map(|c| c.display_name.as_ref().unwrap_or(&c.name).clone())
            .unwrap_or_else(|| "".to_string());

        // Date range info
        let date_info = if let Some(ref range) = payload.report.date_range {
            format!("Período: {} al {}", range.from, range.to)
        } else if let Some(ref date) = payload.report.as_of_date {
            format!("Fecha de corte: {}", date)
        } else {
            "".to_string()
        };

        // Custom message or default
        let intro = custom_message
            .clone()
            .unwrap_or_else(|| "Se adjunta el reporte solicitado.".to_string());

        // Plain text version
        let plain_body = format!(
            r#"REPORTE: {}
{}
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
            if !company_name.is_empty() {
                format!("Empresa: {}\n", company_name)
            } else {
                "".to_string()
            },
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
        {}
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
            if !company_name.is_empty() {
                format!(
                    r#"<div class="subtitle" style="margin-top: 8px; font-weight: 600;">{}</div>"#,
                    company_name
                )
            } else {
                "".to_string()
            },
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

    /// Build plain text and HTML email content for report with download link
    /// Used when file is too large to attach (> 10 MB)
    fn build_report_email_content_with_link(
        &self,
        payload: &ErpReportPayload,
        custom_message: &Option<String>,
        download_url: &str,
    ) -> (String, String) {
        let report_title = &payload.report.title;
        let report_code = &payload.report.code;
        let format_label = self.get_format_label(&payload.output.format);
        let generated_at = &payload.report.generated_at;

        // Company/Tenant name
        let company_name = payload
            .company_info
            .as_ref()
            .map(|c| c.display_name.as_ref().unwrap_or(&c.name).clone())
            .unwrap_or_else(|| "".to_string());

        // Date range info
        let date_info = if let Some(ref range) = payload.report.date_range {
            format!("Período: {} al {}", range.from, range.to)
        } else if let Some(ref date) = payload.report.as_of_date {
            format!("Fecha de corte: {}", date)
        } else {
            "".to_string()
        };

        // Custom message or default
        let intro = custom_message
            .clone()
            .unwrap_or_else(|| "Su reporte ha sido generado exitosamente.".to_string());

        // Plain text version with download link
        let plain_body = format!(
            r#"REPORTE: {}
{}
{}

Detalles del reporte:
- Código: {}
- Formato: {}
- Registros: {}
{}
- Generado: {}

DESCARGAR REPORTE:
{}

⚠️ Este enlace expira en 24 horas.

---
Este es un correo automático generado por Facturazo ERP.
Por favor no responda a este mensaje."#,
            report_title,
            if !company_name.is_empty() {
                format!("Empresa: {}\n", company_name)
            } else {
                "".to_string()
            },
            intro,
            report_code,
            format_label,
            payload.data.total_records,
            if !date_info.is_empty() {
                format!("- {}\n", date_info)
            } else {
                "".to_string()
            },
            generated_at,
            download_url
        );

        // HTML version with download button
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
        .download-section {{
            background: linear-gradient(135deg, #48bb78 0%, #38a169 100%);
            border-radius: 8px;
            padding: 25px;
            text-align: center;
            margin: 25px 0;
        }}
        .download-btn {{
            display: inline-block;
            background: white;
            color: #276749;
            padding: 15px 40px;
            border-radius: 8px;
            text-decoration: none;
            font-weight: 600;
            font-size: 16px;
            box-shadow: 0 4px 6px rgba(0,0,0,0.1);
            transition: transform 0.2s;
        }}
        .download-btn:hover {{
            transform: translateY(-2px);
        }}
        .download-note {{
            color: white;
            margin-top: 15px;
            font-size: 13px;
            opacity: 0.9;
        }}
        .warning-notice {{
            background: #fffbeb;
            border: 1px solid #f6e05e;
            border-radius: 6px;
            padding: 12px 15px;
            margin: 15px 0;
            font-size: 13px;
            color: #744210;
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
        {}
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

        <div class="download-section">
            <a href="{}" class="download-btn">📥 Descargar Reporte</a>
            <p class="download-note">El archivo es demasiado grande para adjuntarlo al correo.</p>
        </div>

        <div class="warning-notice">
            ⚠️ <strong>Importante:</strong> Este enlace de descarga expira en 24 horas.
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
            if !company_name.is_empty() {
                format!(
                    r#"<div class="subtitle" style="margin-top: 8px; font-weight: 600;">{}</div>"#,
                    company_name
                )
            } else {
                "".to_string()
            },
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
            generated_at,
            download_url
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

    /// Build WhatsApp text message for report with download link
    /// Used when file is too large to send directly (> 10 MB)
    fn build_whatsapp_text_message_with_link(
        &self,
        payload: &ErpReportPayload,
        custom_message: &Option<String>,
        download_url: &str,
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

📥 *DESCARGAR REPORTE:*
{}

⚠️ _Este enlace expira en 24 horas._"#,
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
            custom_msg,
            download_url
        )
    }

    /// Get filename and content type for attachment
    fn get_attachment_info(&self, payload: &ErpReportPayload) -> (String, String) {
        let base_name = payload.output.file_name.clone().unwrap_or_else(|| {
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

    /// Get MIME type for format
    fn get_mimetype(&self, format: &OutputFormat) -> String {
        match format {
            OutputFormat::Pdf => "application/pdf".to_string(),
            OutputFormat::Xlsx => {
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string()
            }
            OutputFormat::Csv => "text/csv".to_string(),
            OutputFormat::Html => "text/html".to_string(),
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
    whatsapp_provider: Option<Arc<dyn WhatsAppProvider>>,
}

impl ErpReportNotifierBuilder {
    pub fn new() -> Self {
        Self {
            email_service: None,
            whatsapp_provider: None,
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

    /// Configure WhatsApp service with any provider that implements WhatsAppProvider
    pub fn with_whatsapp_provider(mut self, provider: Arc<dyn WhatsAppProvider>) -> Self {
        self.whatsapp_provider = Some(provider);
        self
    }

    /// Build the notifier
    pub fn build(self) -> ErpReportNotifier {
        ErpReportNotifier::new(self.email_service, self.whatsapp_provider)
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
        result
            .successful_recipients
            .push("test@example.com".to_string());

        assert!(result.is_success());
        assert!(!result.is_partial_success());
    }

    #[test]
    fn test_delivery_result_partial() {
        let mut result = DeliveryResult::new(DeliveryMethod::Email);
        result
            .successful_recipients
            .push("test1@example.com".to_string());
        result
            .failed_recipients
            .push(("test2@example.com".to_string(), "Failed".to_string()));

        assert!(!result.is_success());
        assert!(result.is_partial_success());
    }
}
