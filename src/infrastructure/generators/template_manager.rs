//! Template manager with caching and versioning

use anyhow::Result;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Template manager for document generation
pub struct TemplateManager {
    template_dir: PathBuf,
    cache: RwLock<HashMap<String, CachedTemplate>>,
}

impl TemplateManager {
    pub fn new(template_dir: PathBuf) -> Self {
        Self {
            template_dir,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get template by name (thread-safe)
    pub async fn get_template(&self, name: &str) -> Result<String> {
        // Check cache first
        {
            let cache = self.cache.read();
            if let Some(cached) = cache.get(name) {
                if !cached.is_expired() {
                    return Ok(cached.content.clone());
                }
            }
        }

        // Load from disk
        let template = self.load_template_from_disk(name).await?;

        // Update cache
        {
            let mut cache = self.cache.write();
            cache.insert(name.to_string(), CachedTemplate::new(template.clone()));
        }

        Ok(template)
    }

    /// Load template from disk
    async fn load_template_from_disk(&self, name: &str) -> Result<String> {
        let file_path = self.template_dir.join(format!("{}.typ", name));

        if !file_path.exists() {
            // Try with .typst extension
            let alt_path = self.template_dir.join(format!("{}.typst", name));

            if alt_path.exists() {
                return Ok(fs::read_to_string(alt_path).await?);
            }

            // Return built-in template
            return self.get_builtin_template(name);
        }

        Ok(fs::read_to_string(file_path).await?)
    }

    /// Get built-in template
    fn get_builtin_template(&self, name: &str) -> Result<String> {
        match name {
            "invoice_fiscal" => Ok(templates::INVOICE_FISCAL.to_string()),
            "quotation" => Ok(templates::QUOTATION.to_string()),
            "report" => Ok(templates::REPORT.to_string()),
            "receipt" => Ok(templates::RECEIPT.to_string()),
            "credit_note" => Ok(templates::CREDIT_NOTE.to_string()),
            _ => Err(anyhow::anyhow!("Template not found: {}", name)),
        }
    }

    /// Save template to disk
    pub async fn save_template(&self, name: &str, content: &str) -> Result<()> {
        let file_path = self.template_dir.join(format!("{}.typ", name));

        // Ensure directory exists
        fs::create_dir_all(&self.template_dir).await?;

        // Save to disk
        fs::write(&file_path, content).await?;

        // Update cache
        {
            let mut cache = self.cache.write();
            cache.insert(name.to_string(), CachedTemplate::new(content.to_string()));
        }

        Ok(())
    }

    /// List available templates
    pub async fn list_templates(&self) -> Result<Vec<TemplateInfo>> {
        let mut templates = Vec::new();

        // List disk templates
        if self.template_dir.exists() {
            let mut entries = fs::read_dir(&self.template_dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                if path.extension().and_then(|s| s.to_str()) == Some("typ")
                    || path.extension().and_then(|s| s.to_str()) == Some("typst")
                {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        let metadata = entry.metadata().await?;
                        let modified = metadata
                            .modified()?
                            .duration_since(std::time::UNIX_EPOCH)?
                            .as_secs();

                        templates.push(TemplateInfo {
                            name: name.to_string(),
                            path: path.to_string_lossy().to_string(),
                            size: metadata.len() as usize,
                            modified: DateTime::from_timestamp(modified as i64, 0)
                                .unwrap_or_else(|| Utc::now()),
                            is_builtin: false,
                        });
                    }
                }
            }
        }

        // Add built-in templates
        for name in &[
            "invoice_fiscal",
            "quotation",
            "report",
            "receipt",
            "credit_note",
        ] {
            if !templates.iter().any(|t| t.name == *name) {
                templates.push(TemplateInfo {
                    name: name.to_string(),
                    path: format!("builtin:{}", name),
                    size: 0,
                    modified: Utc::now(),
                    is_builtin: true,
                });
            }
        }

        Ok(templates)
    }

    /// Clear cache
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        let cache = self.cache.read();
        CacheStats {
            templates_cached: cache.len(),
            total_size: cache.values().map(|t| t.content.len()).sum(),
        }
    }
}

/// Cached template
#[derive(Clone)]
struct CachedTemplate {
    content: String,
    cached_at: DateTime<Utc>,
    ttl_seconds: i64,
}

impl CachedTemplate {
    fn new(content: String) -> Self {
        Self {
            content,
            cached_at: Utc::now(),
            ttl_seconds: 3600, // 1 hour TTL
        }
    }

    fn is_expired(&self) -> bool {
        let age = Utc::now() - self.cached_at;
        age.num_seconds() > self.ttl_seconds
    }
}

/// Template information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub name: String,
    pub path: String,
    pub size: usize,
    pub modified: DateTime<Utc>,
    pub is_builtin: bool,
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub templates_cached: usize,
    pub total_size: usize,
}

/// Template versioning support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVersion {
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<String>,
    pub description: Option<String>,
    pub content_hash: String,
}

impl TemplateVersion {
    pub fn new(content: &str, created_by: Option<String>, description: Option<String>) -> Self {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        use ring::digest;

        let hash = digest::digest(&digest::SHA256, content.as_bytes());
        let content_hash = URL_SAFE_NO_PAD.encode(hash.as_ref());

        Self {
            version: Self::generate_version(),
            created_at: Utc::now(),
            created_by,
            description,
            content_hash,
        }
    }

    fn generate_version() -> String {
        format!("v{}", Utc::now().format("%Y%m%d.%H%M%S"))
    }
}

// Built-in templates
mod templates {
    // Use the invoice template from the invoice generator
    pub const INVOICE_FISCAL: &str = r#"
#set page(paper: "us-letter")
= Invoice Template
Fiscal invoice template with Dominican Republic compliance.
    "#;

    pub const QUOTATION: &str = r#"
#set page(paper: "us-letter")
= Quotation Template
Business quotation template.
    "#;

    pub const REPORT: &str = r#"
#set page(paper: "us-letter")
= Report Template
Report with tables and charts.
    "#;

    pub const RECEIPT: &str = r#"
#set page(paper: "us-letter")
= Receipt Template
Payment receipt template.
    "#;

    pub const CREDIT_NOTE: &str = r#"
#set page(paper: "us-letter")
= Credit Note Template
Credit note for returns and adjustments.
    "#;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_template_manager() {
        let temp_dir = TempDir::new().unwrap();
        let manager = TemplateManager::new(temp_dir.path().to_path_buf());

        // Save a template
        manager
            .save_template("test", "Test template content")
            .await
            .unwrap();

        // Get template (should be cached)
        let template = manager.get_template("test").await.unwrap();
        assert_eq!(template, "Test template content");

        // Check cache stats
        let stats = manager.cache_stats();
        assert_eq!(stats.templates_cached, 1);

        // List templates
        let templates = manager.list_templates().await.unwrap();
        assert!(templates.iter().any(|t| t.name == "test"));

        // Clear cache
        manager.clear_cache();
        assert_eq!(manager.cache_stats().templates_cached, 0);
    }

    #[test]
    fn test_template_versioning() {
        let version = TemplateVersion::new(
            "template content",
            Some("test_user".to_string()),
            Some("Initial version".to_string()),
        );

        assert!(version.version.starts_with("v"));
        assert!(!version.content_hash.is_empty()); // SHA256 base64 encoded
    }
}
