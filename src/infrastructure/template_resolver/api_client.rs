//! HTTP client for the .NET Facturazo ERP Core backend

use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;

use super::models::{ApiResponse, ResolvedTemplate};

/// Client for the Facturazo ERP Core template resolution API
pub struct TemplateApiClient {
    http: Client,
    base_url: String,
    service_token: String,
}

impl TemplateApiClient {
    pub fn new(base_url: String, service_token: String) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(3))
            .connect_timeout(Duration::from_secs(2))
            .pool_max_idle_per_host(5)
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            http,
            base_url,
            service_token,
        })
    }

    /// Resolve the active template for a tenant + document type code
    pub async fn resolve(
        &self,
        tenant_id: i64,
        document_type_code: &str,
    ) -> Result<ResolvedTemplate> {
        let url = format!(
            "{}/core/api/templates/resolve?document_type_code={}",
            self.base_url, document_type_code
        );

        tracing::debug!(
            tenant_id,
            document_type_code,
            "Resolving template from backend: {}",
            url
        );

        let response = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.service_token))
            .header("X-Tenant-Id", tenant_id.to_string())
            .send()
            .await
            .context("Failed to call template resolve API")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Template resolve API returned {}: {}",
                status.as_u16(),
                body
            );
        }

        let api_response: ApiResponse<ResolvedTemplate> = response
            .json()
            .await
            .context("Failed to parse template resolve response")?;

        if !api_response.success {
            let error_msg = api_response
                .error
                .map(|e| format!("{}: {}", e.code, e.message))
                .unwrap_or_else(|| "Unknown error".to_string());
            anyhow::bail!("Template resolve failed: {}", error_msg);
        }

        api_response
            .data
            .ok_or_else(|| anyhow::anyhow!("Template resolve returned success but no data"))
    }
}
