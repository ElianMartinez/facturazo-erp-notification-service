//! EvolutionAPI WhatsApp Integration
//!
//! This module provides integration with EvolutionAPI v2 for WhatsApp messaging

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

/// EvolutionAPI client for WhatsApp messaging
pub struct EvolutionAPIClient {
    base_url: String,
    api_key: String,
    instance: String,
    client: Client,
}

/// Request to send text message via WhatsApp
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendTextRequest {
    pub number: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_preview: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentioned: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentions_every_one: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted: Option<QuotedMessage>,
}

/// Request to send media (PDF, images, etc)
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMediaRequest {
    pub number: String,
    pub mediatype: MediaType,
    pub mimetype: String,
    pub caption: String,
    pub media: String, // Base64 encoded content
    pub file_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_preview: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentions_every_one: Option<bool>,
}

/// Media types supported by WhatsApp
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Document,
    Image,
    Video,
    Audio,
}

/// Quoted message for replies
#[derive(Debug, Serialize, Deserialize)]
pub struct QuotedMessage {
    pub key: MessageKey,
    pub message: MessageContent,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageKey {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageContent {
    pub conversation: String,
}

/// Response from EvolutionAPI for successful message send
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvolutionAPIResponse {
    pub key: MessageKeyResponse,
    #[serde(default)]
    pub message_type: Option<String>,
    #[serde(default)]
    pub message_timestamp: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageKeyResponse {
    pub remote_jid: String,
    pub from_me: bool,
    pub id: String,
}

/// Error response from EvolutionAPI
#[derive(Debug, Deserialize)]
pub struct EvolutionAPIErrorResponse {
    pub status: Option<u16>,
    pub error: Option<String>,
    pub response: Option<ErrorDetail>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorDetail {
    pub message: Option<String>,
}

impl EvolutionAPIClient {
    /// Create a new EvolutionAPI client
    pub fn new(base_url: String, api_key: String, instance: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            base_url,
            api_key,
            instance,
            client,
        }
    }

    /// Send a text message via WhatsApp
    pub async fn send_text(&self, request: SendTextRequest) -> Result<String> {
        let url = format!("{}/message/sendText/{}", self.base_url, self.instance);

        info!("Sending WhatsApp text message to {}", request.number);

        let response = self
            .client
            .post(&url)
            .header("apikey", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to EvolutionAPI")?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            error!("EvolutionAPI error: {} - {}", status, body);
            // Try to parse error response
            if let Ok(err_response) = serde_json::from_str::<EvolutionAPIErrorResponse>(&body) {
                let msg = err_response
                    .response
                    .and_then(|r| r.message)
                    .or(err_response.error)
                    .unwrap_or_else(|| body.clone());
                anyhow::bail!("Failed to send WhatsApp message: {}", msg);
            }
            anyhow::bail!("Failed to send WhatsApp message: {}", body);
        }

        let api_response: EvolutionAPIResponse =
            serde_json::from_str(&body).context("Failed to parse EvolutionAPI response")?;

        let message_id = api_response.key.id;

        info!("WhatsApp message sent successfully. ID: {}", message_id);

        Ok(message_id)
    }

    /// Send a PDF document via WhatsApp
    pub async fn send_pdf(
        &self,
        number: String,
        pdf_bytes: Vec<u8>,
        filename: String,
        caption: String,
    ) -> Result<String> {
        let base64_content = BASE64.encode(&pdf_bytes);

        let request = SendMediaRequest {
            number: normalize_dominican_phone(&number),
            mediatype: MediaType::Document,
            mimetype: "application/pdf".to_string(),
            caption,
            media: base64_content,
            file_name: filename,
            delay: None,
            link_preview: Some(false),
            mentions_every_one: Some(false),
        };

        self.send_media(request).await
    }

    /// Send media via WhatsApp
    pub async fn send_media(&self, request: SendMediaRequest) -> Result<String> {
        let url = format!("{}/message/sendMedia/{}", self.base_url, self.instance);

        info!(
            "Sending WhatsApp media to {}: {}",
            request.number, request.file_name
        );

        let response = self
            .client
            .post(&url)
            .header("apikey", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send media request to EvolutionAPI")?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            error!("EvolutionAPI error: {} - {}", status, body);
            if let Ok(err_response) = serde_json::from_str::<EvolutionAPIErrorResponse>(&body) {
                let msg = err_response
                    .response
                    .and_then(|r| r.message)
                    .or(err_response.error)
                    .unwrap_or_else(|| body.clone());
                anyhow::bail!("Failed to send WhatsApp media: {}", msg);
            }
            anyhow::bail!("Failed to send WhatsApp media: {}", body);
        }

        let api_response: EvolutionAPIResponse =
            serde_json::from_str(&body).context("Failed to parse EvolutionAPI response")?;

        let message_id = api_response.key.id;

        info!("WhatsApp media sent successfully. ID: {}", message_id);

        Ok(message_id)
    }

    /// Send invoice notification with PDF attachment
    pub async fn send_invoice_notification(
        &self,
        phone: String,
        invoice_number: String,
        ncf: String,
        amount: String,
        pdf_bytes: Vec<u8>,
    ) -> Result<String> {
        // First send the text notification
        let text_message = format!(
            "🧾 *FACTURA ELECTRÓNICA*\n\n\
            Su factura *{}* está lista.\n\
            NCF: *{}*\n\
            Monto: *RD$ {}*\n\n\
            A continuación recibirá el documento PDF.",
            invoice_number, ncf, amount
        );

        let text_request = SendTextRequest {
            number: normalize_dominican_phone(&phone),
            text: text_message,
            delay: None,
            link_preview: Some(false),
            mentioned: None,
            mentions_every_one: None,
            quoted: None,
        };

        // Send text message
        let text_id = self.send_text(text_request).await?;

        // Then send the PDF
        let caption = format!("Factura {} - NCF: {}", invoice_number, ncf);
        let filename = format!("{}.pdf", invoice_number);

        let pdf_id = self.send_pdf(phone, pdf_bytes, filename, caption).await?;

        Ok(format!("text:{},pdf:{}", text_id, pdf_id))
    }

    /// Check if instance is connected
    pub async fn is_connected(&self) -> Result<bool> {
        let url = format!(
            "{}/instance/connectionState/{}",
            self.base_url, self.instance
        );

        let response = self
            .client
            .get(&url)
            .header("apikey", &self.api_key)
            .send()
            .await?;

        if response.status().is_success() {
            // Parse response to check connection state
            // This is a simplified version, actual response structure may vary
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Simple wrapper to send a text message
    pub async fn send_simple_text(&self, recipient: &str, message: &str) -> Result<String> {
        let request = SendTextRequest {
            number: normalize_dominican_phone(recipient),
            text: message.to_string(),
            delay: None,
            link_preview: Some(false),
            mentioned: None,
            mentions_every_one: None,
            quoted: None,
        };
        self.send_text(request).await
    }

    /// Simple wrapper to send a document (base64 encoded)
    pub async fn send_document(
        &self,
        recipient: &str,
        base64_content: &str,
        filename: &str,
        mimetype: &str,
    ) -> Result<String> {
        let request = SendMediaRequest {
            number: normalize_dominican_phone(recipient),
            mediatype: MediaType::Document,
            mimetype: mimetype.to_string(),
            caption: filename.to_string(),
            media: base64_content.to_string(),
            file_name: filename.to_string(),
            delay: None,
            link_preview: Some(false),
            mentions_every_one: Some(false),
        };
        self.send_media(request).await
    }
}

/// Normalize Dominican Republic phone numbers
pub fn normalize_dominican_phone(phone: &str) -> String {
    let digits_only: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();

    // Check if it's a valid Dominican number (809, 829, 849)
    if digits_only.starts_with("1809")
        || digits_only.starts_with("1829")
        || digits_only.starts_with("1849")
    {
        digits_only
    } else if digits_only.starts_with("809")
        || digits_only.starts_with("829")
        || digits_only.starts_with("849")
    {
        format!("1{}", digits_only)
    } else if digits_only.starts_with("+1809")
        || digits_only.starts_with("+1829")
        || digits_only.starts_with("+1849")
    {
        digits_only.chars().skip(1).collect()
    } else {
        // Return as is if not a Dominican number
        digits_only
    }
}

/// Validate Dominican Republic phone number
pub fn is_valid_dominican_phone(phone: &str) -> bool {
    let normalized = normalize_dominican_phone(phone);

    // Should be 11 digits starting with 1809, 1829, or 1849
    if normalized.len() != 11 {
        return false;
    }

    normalized.starts_with("1809")
        || normalized.starts_with("1829")
        || normalized.starts_with("1849")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_dominican_phone() {
        assert_eq!(normalize_dominican_phone("809-555-1234"), "18095551234");
        assert_eq!(normalize_dominican_phone("+1-829-555-1234"), "18295551234");
        assert_eq!(normalize_dominican_phone("849 555 1234"), "18495551234");
        assert_eq!(normalize_dominican_phone("18095551234"), "18095551234");
    }

    #[test]
    fn test_validate_dominican_phone() {
        assert!(is_valid_dominican_phone("809-555-1234"));
        assert!(is_valid_dominican_phone("+1-829-555-1234"));
        assert!(is_valid_dominican_phone("18495551234"));
        assert!(!is_valid_dominican_phone("305-555-1234")); // Miami number
        assert!(!is_valid_dominican_phone("809555123")); // Too short
    }
}
