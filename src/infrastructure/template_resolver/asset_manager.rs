//! Asset manager for downloading and caching template assets (images, fonts, logos)

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::storage::s3::S3Client;
use std::sync::Arc;

use super::models::TemplateAsset;

/// Manages template assets on the local filesystem for Typst compilation
pub struct AssetManager {
    s3_client: Arc<S3Client>,
    base_dir: PathBuf,
    /// S3 bucket for template assets
    bucket: String,
}

impl AssetManager {
    pub fn new(s3_client: Arc<S3Client>, base_dir: PathBuf, bucket: String) -> Self {
        Self {
            s3_client,
            base_dir,
            bucket,
        }
    }

    /// Download all assets for a template and return the local path mappings
    pub async fn ensure_assets(
        &self,
        tenant_id: i64,
        template_id: i64,
        version: i32,
        assets: &[TemplateAsset],
    ) -> Result<(HashMap<String, PathBuf>, Vec<PathBuf>)> {
        if assets.is_empty() {
            return Ok((HashMap::new(), Vec::new()));
        }

        let asset_dir = self
            .base_dir
            .join(tenant_id.to_string())
            .join(format!("{}_{}", template_id, version));

        fs::create_dir_all(&asset_dir)
            .await
            .context("Failed to create asset directory")?;

        let mut asset_paths = HashMap::new();
        let mut font_paths = Vec::new();

        for asset in assets {
            let local_path = asset_dir.join(&asset.file_name);

            // Skip download if file already exists (same version = same content)
            if !local_path.exists() {
                match self.download_asset(&asset.file_url, &local_path).await {
                    Ok(()) => {
                        tracing::debug!(
                            file = %asset.file_name,
                            "Downloaded asset to {:?}",
                            local_path
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            file = %asset.file_name,
                            error = %e,
                            "Failed to download asset, skipping"
                        );
                        continue;
                    }
                }
            }

            asset_paths.insert(asset.file_name.clone(), local_path.clone());

            // Track font directories for Typst --font-path
            if asset.asset_type == "FONT" {
                let font_dir = local_path.parent().unwrap_or(&asset_dir);
                if !font_paths.contains(&font_dir.to_path_buf()) {
                    font_paths.push(font_dir.to_path_buf());
                }
            }
        }

        Ok((asset_paths, font_paths))
    }

    /// Download a single asset from S3/R2 to a local path
    async fn download_asset(&self, file_url: &str, local_path: &Path) -> Result<()> {
        // file_url could be a full URL or an S3 key
        // If it starts with r2:// or s3://, strip the prefix to get the key
        let key = file_url
            .strip_prefix("r2://")
            .or_else(|| file_url.strip_prefix("s3://"))
            .unwrap_or(file_url);

        let bytes = self
            .s3_client
            .get_object_bytes(&self.bucket, key)
            .await
            .with_context(|| format!("Failed to download asset: {}", key))?;

        fs::write(local_path, &bytes)
            .await
            .with_context(|| format!("Failed to write asset to {:?}", local_path))?;

        Ok(())
    }

    /// Get the asset directory path for a template version
    pub fn asset_dir(&self, tenant_id: i64, template_id: i64, version: i32) -> PathBuf {
        self.base_dir
            .join(tenant_id.to_string())
            .join(format!("{}_{}", template_id, version))
    }

    /// Clean up assets for a specific template version
    pub async fn cleanup_template(&self, tenant_id: i64, template_id: i64) {
        let tenant_dir = self.base_dir.join(tenant_id.to_string());
        if !tenant_dir.exists() {
            return;
        }

        // Remove all version directories for this template
        if let Ok(mut entries) = fs::read_dir(&tenant_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with(&format!("{}_", template_id)) {
                        let _ = fs::remove_dir_all(entry.path()).await;
                        tracing::debug!("Cleaned up asset directory: {:?}", entry.path());
                    }
                }
            }
        }
    }
}
