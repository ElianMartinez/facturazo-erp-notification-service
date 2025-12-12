//! Email notification service using SMTP
//!
//! This module provides email sending capabilities using SMTP

use anyhow::{Context, Result};
use lettre::message::{header, Attachment as LettreAttachment, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use tracing::{error, info};

/// SMTP email service
pub struct EmailService {
    smtp_host: String,
    smtp_port: u16,
    smtp_username: String,
    smtp_password: String,
    from_email: String,
    from_name: String,
    use_tls: bool,
}

impl EmailService {
    /// Create a new email service
    pub fn new(
        smtp_host: String,
        smtp_port: u16,
        smtp_username: String,
        smtp_password: String,
        from_email: String,
        from_name: String,
        use_tls: bool,
    ) -> Self {
        Self {
            smtp_host,
            smtp_port,
            smtp_username,
            smtp_password,
            from_email,
            from_name,
            use_tls,
        }
    }

    /// Send an email with optional attachments
    pub async fn send_email(
        &self,
        to_email: &str,
        subject: &str,
        body: &str,
        html_body: Option<&str>,
        attachments: Vec<EmailAttachment>,
    ) -> Result<String> {
        info!(
            "Sending email to {} with subject: {}, has_html: {}, attachments: {}",
            to_email,
            subject,
            html_body.is_some(),
            attachments.len()
        );

        // Build the email
        let email = Message::builder()
            .from(format!("{} <{}>", self.from_name, self.from_email).parse()?)
            .to(to_email.parse()?)
            .subject(subject);

        // Build multipart message with correct MIME structure:
        // - With HTML + attachments: mixed { alternative { text, html }, attachments }
        // - With HTML only: alternative { text, html }
        // - With text + attachments: mixed { text, attachments }
        // - With text only: mixed { text }

        let has_attachments = !attachments.is_empty();
        let has_html = html_body.is_some();

        let multipart = if has_html && has_attachments {
            // Correct structure: multipart/mixed containing multipart/alternative + attachments
            let alternative = MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .header(header::ContentType::TEXT_PLAIN)
                        .body(body.to_string()),
                )
                .singlepart(
                    SinglePart::builder()
                        .header(header::ContentType::TEXT_HTML)
                        .body(html_body.unwrap().to_string()),
                );

            // Wrap alternative in mixed, then add attachments
            let mut mixed = MultiPart::mixed().multipart(alternative);

            for attachment in attachments {
                let content_type = match attachment.content_type.as_str() {
                    "application/pdf" => "application/pdf",
                    "image/png" => "image/png",
                    "image/jpeg" => "image/jpeg",
                    _ => "application/octet-stream",
                };

                mixed = mixed.singlepart(
                    LettreAttachment::new(attachment.filename)
                        .body(attachment.data, content_type.parse()?),
                );
            }

            mixed
        } else if has_html {
            // HTML only, no attachments - use alternative directly
            MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .header(header::ContentType::TEXT_PLAIN)
                        .body(body.to_string()),
                )
                .singlepart(
                    SinglePart::builder()
                        .header(header::ContentType::TEXT_HTML)
                        .body(html_body.unwrap().to_string()),
                )
        } else {
            // Plain text with or without attachments
            let mut mixed = MultiPart::mixed().singlepart(
                SinglePart::builder()
                    .header(header::ContentType::TEXT_PLAIN)
                    .body(body.to_string()),
            );

            for attachment in attachments {
                let content_type = match attachment.content_type.as_str() {
                    "application/pdf" => "application/pdf",
                    "image/png" => "image/png",
                    "image/jpeg" => "image/jpeg",
                    _ => "application/octet-stream",
                };

                mixed = mixed.singlepart(
                    LettreAttachment::new(attachment.filename)
                        .body(attachment.data, content_type.parse()?),
                );
            }

            mixed
        };

        let email = email.multipart(multipart)?;

        // Create SMTP transport
        let mailer = if self.use_tls {
            SmtpTransport::starttls_relay(&self.smtp_host)?
                .port(self.smtp_port)
                .credentials(Credentials::new(
                    self.smtp_username.clone(),
                    self.smtp_password.clone(),
                ))
                .build()
        } else {
            SmtpTransport::relay(&self.smtp_host)?
                .port(self.smtp_port)
                .credentials(Credentials::new(
                    self.smtp_username.clone(),
                    self.smtp_password.clone(),
                ))
                .build()
        };

        // Send the email
        match mailer.send(&email) {
            Ok(response) => {
                info!("Email sent successfully to {}", to_email);
                Ok(response.message().collect::<String>())
            }
            Err(e) => {
                error!("Failed to send email: {}", e);
                Err(anyhow::anyhow!("Failed to send email: {}", e))
            }
        }
    }

    /// Send invoice email with PDF attachment
    pub async fn send_invoice_email(
        &self,
        to_email: &str,
        invoice_number: &str,
        ncf: &str,
        amount: &str,
        due_date: &str,
        pdf_bytes: Vec<u8>,
    ) -> Result<String> {
        let subject = format!("Factura {} - Le Croissant Doré", invoice_number);

        let body = format!(
            "Estimado cliente,\n\n\
            Le enviamos su factura electrónica con los siguientes detalles:\n\n\
            Número de Factura: {}\n\
            NCF: {}\n\
            Monto Total: RD$ {}\n\
            Fecha de Vencimiento: {}\n\n\
            La factura se encuentra adjunta en formato PDF.\n\n\
            Gracias por su preferencia.\n\n\
            Atentamente,\n\
            Le Croissant Doré",
            invoice_number, ncf, amount, due_date
        );

        let html_body = format!(
            r#"<!DOCTYPE html>
            <html>
            <head>
                <style>
                    body {{ font-family: Arial, sans-serif; }}
                    .header {{ background-color: #8B4513; color: white; padding: 20px; }}
                    .content {{ padding: 20px; }}
                    .details {{ background-color: #f5f5f5; padding: 15px; margin: 20px 0; }}
                    .footer {{ color: #666; font-size: 12px; padding: 20px; }}
                </style>
            </head>
            <body>
                <div class="header">
                    <h1>Le Croissant Doré</h1>
                </div>
                <div class="content">
                    <h2>Factura Electrónica</h2>
                    <p>Estimado cliente,</p>
                    <p>Le enviamos su factura electrónica con los siguientes detalles:</p>
                    <div class="details">
                        <p><strong>Número de Factura:</strong> {}</p>
                        <p><strong>NCF:</strong> {}</p>
                        <p><strong>Monto Total:</strong> RD$ {}</p>
                        <p><strong>Fecha de Vencimiento:</strong> {}</p>
                    </div>
                    <p>La factura se encuentra adjunta en formato PDF.</p>
                    <p>Gracias por su preferencia.</p>
                </div>
                <div class="footer">
                    <p>Este es un correo automático, por favor no responda a este mensaje.</p>
                </div>
            </body>
            </html>"#,
            invoice_number, ncf, amount, due_date
        );

        let attachment = EmailAttachment {
            filename: format!("{}.pdf", invoice_number),
            content_type: "application/pdf".to_string(),
            data: pdf_bytes,
        };

        self.send_email(
            to_email,
            &subject,
            &body,
            Some(&html_body),
            vec![attachment],
        )
        .await
    }

    /// Test SMTP connection
    pub async fn test_connection(&self) -> Result<bool> {
        let mailer = if self.use_tls {
            SmtpTransport::starttls_relay(&self.smtp_host)?
                .port(self.smtp_port)
                .credentials(Credentials::new(
                    self.smtp_username.clone(),
                    self.smtp_password.clone(),
                ))
                .build()
        } else {
            SmtpTransport::relay(&self.smtp_host)?
                .port(self.smtp_port)
                .credentials(Credentials::new(
                    self.smtp_username.clone(),
                    self.smtp_password.clone(),
                ))
                .build()
        };

        mailer
            .test_connection()
            .context("Failed to test SMTP connection")
    }
}

/// Email attachment
#[derive(Debug, Clone)]
pub struct EmailAttachment {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

impl EmailService {
    /// Send email with simple attachment wrapper
    pub async fn send_with_attachments(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        attachments: Vec<EmailAttachment>,
    ) -> Result<()> {
        // Convert attachments to internal format (data -> content)
        let internal_attachments: Vec<EmailAttachment> = attachments
            .into_iter()
            .map(|a| EmailAttachment {
                filename: a.filename,
                content_type: a.content_type,
                data: a.data,
            })
            .collect();

        self.send_email(to, subject, body, None, internal_attachments)
            .await?;
        Ok(())
    }
}
