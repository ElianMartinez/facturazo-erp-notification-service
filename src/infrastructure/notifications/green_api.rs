//! Green-API WhatsApp Integration
//!
//! This module provides integration with Green-API for WhatsApp messaging
//! Documentation: https://green-api.com/en/docs/api/

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use super::evolution_api::normalize_dominican_phone;

/// Green-API client for WhatsApp messaging
///
/// Green-API uses a different URL structure:
/// - API URL: `{apiUrl}/waInstance{idInstance}/{method}/{apiTokenInstance}`
/// - Media URL: `{mediaUrl}/waInstance{idInstance}/sendFileByUpload/{apiTokenInstance}`
///
/// The media URL is used for direct file uploads (multipart/form-data)
/// which is more efficient than base64 encoding for small files.
pub struct GreenApiClient {
    api_url: String,
    /// Media URL for file uploads (e.g., "https://7105.media.greenapi.com")
    /// If not set, falls back to api_url
    media_url: Option<String>,
    instance_id: String,
    api_token: String,
    client: Client,
}

/// Request to send text message via Green-API
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GreenSendMessageRequest {
    /// Chat ID format: "1809XXXXXXX@c.us" for personal or "XXXXX@g.us" for group
    pub chat_id: String,
    /// Message text (max 20,000 characters)
    pub message: String,
    /// Optional: Quote a message by ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoted_message_id: Option<String>,
    /// Enable link preview (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_preview: Option<bool>,
}

/// Request to send file by URL via Green-API
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GreenSendFileByUrlRequest {
    /// Chat ID format: "1809XXXXXXX@c.us"
    pub chat_id: String,
    /// URL of the file to send
    pub url_file: String,
    /// File name with extension
    pub file_name: String,
    /// Optional caption (max 20,000 characters)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

/// Response from Green-API for successful operations
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GreenApiResponse {
    /// Message ID returned on success
    pub id_message: String,
}

/// Error response from Green-API
#[derive(Debug, Deserialize)]
pub struct GreenApiErrorResponse {
    pub error: Option<String>,
    pub message: Option<String>,
}

/// Account state response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GreenApiStateResponse {
    pub state_instance: String,
}

impl GreenApiClient {
    /// Create a new Green-API client
    ///
    /// # Arguments
    /// * `api_url` - Base URL (e.g., "https://api.green-api.com")
    /// * `instance_id` - Your instance ID (numeric)
    /// * `api_token` - Your API token
    pub fn new(api_url: String, instance_id: String, api_token: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60)) // Longer timeout for file uploads
            .build()
            .expect("Failed to create HTTP client");

        Self {
            api_url,
            media_url: None,
            instance_id,
            api_token,
            client,
        }
    }

    /// Create a new Green-API client with media URL for file uploads
    ///
    /// # Arguments
    /// * `api_url` - Base URL for API calls (e.g., "https://api.green-api.com")
    /// * `media_url` - Media URL for file uploads (e.g., "https://7105.media.greenapi.com")
    /// * `instance_id` - Your instance ID (numeric)
    /// * `api_token` - Your API token
    pub fn with_media_url(
        api_url: String,
        media_url: String,
        instance_id: String,
        api_token: String,
    ) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            api_url,
            media_url: Some(media_url),
            instance_id,
            api_token,
            client,
        }
    }

    /// Set the media URL for file uploads
    pub fn set_media_url(&mut self, media_url: String) {
        self.media_url = Some(media_url);
    }

    /// Build the full endpoint URL
    /// Format: {apiUrl}/waInstance{idInstance}/{method}/{apiTokenInstance}
    fn build_url(&self, method: &str) -> String {
        format!(
            "{}/waInstance{}/{}/{}",
            self.api_url, self.instance_id, method, self.api_token
        )
    }

    /// Build the media upload URL
    /// Format: {mediaUrl}/waInstance{idInstance}/sendFileByUpload/{apiTokenInstance}
    fn build_media_url(&self) -> String {
        let base = self.media_url.as_ref().unwrap_or(&self.api_url);
        format!(
            "{}/waInstance{}/sendFileByUpload/{}",
            base, self.instance_id, self.api_token
        )
    }

    /// Convert phone number to Green-API chat ID format
    /// Green-API expects: "1809XXXXXXX@c.us"
    fn to_chat_id(phone: &str) -> String {
        let normalized = normalize_dominican_phone(phone);
        format!("{}@c.us", normalized)
    }

    /// Send a text message via WhatsApp
    pub async fn send_text(&self, phone: &str, message: &str) -> Result<String> {
        let url = self.build_url("sendMessage");
        let chat_id = Self::to_chat_id(phone);

        info!("Green-API: Sending text message to {}", chat_id);

        let request = GreenSendMessageRequest {
            chat_id,
            message: message.to_string(),
            quoted_message_id: None,
            link_preview: Some(false),
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Green-API")?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            error!("Green-API error: {} - {}", status, body);
            if let Ok(err) = serde_json::from_str::<GreenApiErrorResponse>(&body) {
                let msg = err.message.or(err.error).unwrap_or(body.clone());
                anyhow::bail!("Green-API error: {}", msg);
            }
            anyhow::bail!("Green-API error: {}", body);
        }

        let api_response: GreenApiResponse =
            serde_json::from_str(&body).context("Failed to parse Green-API response")?;

        info!(
            "Green-API: Message sent successfully. ID: {}",
            api_response.id_message
        );

        Ok(api_response.id_message)
    }

    /// Send a file by URL
    pub async fn send_file_by_url(
        &self,
        phone: &str,
        file_url: &str,
        filename: &str,
        caption: Option<&str>,
    ) -> Result<String> {
        let url = self.build_url("sendFileByUrl");
        let chat_id = Self::to_chat_id(phone);

        info!("Green-API: Sending file {} to {}", filename, chat_id);

        let request = GreenSendFileByUrlRequest {
            chat_id,
            url_file: file_url.to_string(),
            file_name: filename.to_string(),
            caption: caption.map(|s| s.to_string()),
        };

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send file request to Green-API")?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            error!("Green-API error sending file: {} - {}", status, body);
            anyhow::bail!("Green-API error: {}", body);
        }

        let api_response: GreenApiResponse =
            serde_json::from_str(&body).context("Failed to parse Green-API response")?;

        info!(
            "Green-API: File sent successfully. ID: {}",
            api_response.id_message
        );

        Ok(api_response.id_message)
    }

    /// Send a file directly using multipart/form-data
    /// This is the preferred method for file uploads as it's more efficient
    /// than base64 encoding
    pub async fn send_file_bytes(
        &self,
        phone: &str,
        file_bytes: Vec<u8>,
        filename: &str,
        mimetype: &str,
        caption: Option<&str>,
    ) -> Result<String> {
        use reqwest::multipart::{Form, Part};

        let url = self.build_media_url();
        let chat_id = Self::to_chat_id(phone);

        info!(
            "Green-API: Uploading file {} ({} bytes) to {} via multipart",
            filename,
            file_bytes.len(),
            chat_id
        );

        // Create multipart form
        let file_part = Part::bytes(file_bytes)
            .file_name(filename.to_string())
            .mime_str(mimetype)
            .context("Invalid MIME type")?;

        let mut form = Form::new()
            .text("chatId", chat_id)
            .text("fileName", filename.to_string())
            .part("file", file_part);

        if let Some(cap) = caption {
            form = form.text("caption", cap.to_string());
        }

        let response = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .context("Failed to upload file to Green-API")?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            error!("Green-API error uploading file: {} - {}", status, body);
            anyhow::bail!("Green-API error: {}", body);
        }

        let api_response: GreenApiResponse =
            serde_json::from_str(&body).context("Failed to parse Green-API response")?;

        info!(
            "Green-API: File uploaded successfully via multipart. ID: {}",
            api_response.id_message
        );

        Ok(api_response.id_message)
    }

    /// Send a PDF document (convenience method)
    /// Uses direct binary upload if media_url is configured, otherwise falls back to base64
    pub async fn send_pdf(
        &self,
        phone: &str,
        pdf_bytes: Vec<u8>,
        filename: &str,
        caption: &str,
    ) -> Result<String> {
        // Use multipart upload (more efficient, no base64 overhead)
        self.send_file_bytes(phone, pdf_bytes, filename, "application/pdf", Some(caption))
            .await
    }

    /// Check if instance is connected
    pub async fn is_connected(&self) -> Result<bool> {
        let url = self.build_url("getStateInstance");

        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            let body = response.text().await?;
            if let Ok(state) = serde_json::from_str::<GreenApiStateResponse>(&body) {
                // "authorized" means connected and ready
                return Ok(state.state_instance == "authorized");
            }
        }

        Ok(false)
    }

    /// Simple wrapper for sending text (matches EvolutionAPIClient interface)
    pub async fn send_simple_text(&self, recipient: &str, message: &str) -> Result<String> {
        self.send_text(recipient, message).await
    }

    /// Simple wrapper for sending document (matches EvolutionAPIClient interface)
    /// Decodes base64 and uses multipart upload for better reliability
    pub async fn send_document(
        &self,
        recipient: &str,
        base64_content: &str,
        filename: &str,
        mimetype: &str,
    ) -> Result<String> {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(base64_content)
            .context("Failed to decode base64 content")?;
        self.send_file_bytes(recipient, bytes, filename, mimetype, None)
            .await
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

        let text_id = self.send_text(&phone, &text_message).await?;

        // Then send the PDF
        let caption = format!("Factura {} - NCF: {}", invoice_number, ncf);
        let filename = format!("{}.pdf", invoice_number);

        let pdf_id = self
            .send_pdf(&phone, pdf_bytes, &filename, &caption)
            .await?;

        Ok(format!("text:{},pdf:{}", text_id, pdf_id))
    }
}

// Implement WhatsAppProvider trait for GreenApiClient
use super::WhatsAppProvider;
use async_trait::async_trait;

#[async_trait]
impl WhatsAppProvider for GreenApiClient {
    async fn send_simple_text(&self, recipient: &str, message: &str) -> Result<String> {
        self.send_text(recipient, message).await
    }

    async fn send_file(
        &self,
        recipient: &str,
        file_bytes: Vec<u8>,
        filename: String,
        mimetype: String,
        caption: Option<String>,
    ) -> Result<String> {
        self.send_file_bytes(
            recipient,
            file_bytes,
            &filename,
            &mimetype,
            caption.as_deref(),
        )
        .await
    }

    async fn send_invoice_notification(
        &self,
        phone: String,
        invoice_number: String,
        ncf: String,
        amount: String,
        pdf_bytes: Vec<u8>,
    ) -> Result<String> {
        GreenApiClient::send_invoice_notification(
            self,
            phone,
            invoice_number,
            ncf,
            amount,
            pdf_bytes,
        )
        .await
    }

    async fn is_connected(&self) -> Result<bool> {
        GreenApiClient::is_connected(self).await
    }

    fn provider_name(&self) -> &'static str {
        "GreenAPI"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_chat_id() {
        assert_eq!(
            GreenApiClient::to_chat_id("809-555-1234"),
            "18095551234@c.us"
        );
        assert_eq!(
            GreenApiClient::to_chat_id("+1-829-555-1234"),
            "18295551234@c.us"
        );
    }

    #[test]
    fn test_build_url() {
        let client = GreenApiClient::new(
            "https://api.green-api.com".to_string(),
            "1234567890".to_string(),
            "abc123token".to_string(),
        );

        assert_eq!(
            client.build_url("sendMessage"),
            "https://api.green-api.com/waInstance1234567890/sendMessage/abc123token"
        );
    }
}
