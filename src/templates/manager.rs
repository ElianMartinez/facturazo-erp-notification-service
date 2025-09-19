use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use anyhow::{Result, Context};
use std::path::PathBuf;

use super::{TemplateEngine, TemplateCache};
use crate::storage::s3::S3Client;

pub struct TemplateManager {
    engine: Arc<TemplateEngine>,
    cache: Arc<TemplateCache>,
    s3_client: Option<Arc<S3Client>>,
    local_path: PathBuf,
    templates: Arc<RwLock<HashMap<String, String>>>,
}

impl TemplateManager {
    pub async fn new(
        local_path: PathBuf,
        redis_url: Option<String>,
        s3_client: Option<Arc<S3Client>>,
    ) -> Result<Self> {
        let engine = Arc::new(TemplateEngine::new(local_path.to_string_lossy().to_string())?);
        let cache = Arc::new(TemplateCache::new(redis_url, 3600).await?);

        let mut manager = TemplateManager {
            engine,
            cache,
            s3_client,
            local_path,
            templates: Arc::new(RwLock::new(HashMap::new())),
        };

        // Cargar templates locales al inicio
        manager.load_local_templates().await?;

        Ok(manager)
    }

    pub async fn get_template(&self, template_id: &str) -> Result<String> {
        // 1. Check cache
        if let Some(cached) = self.cache.get(template_id).await {
            return Ok(cached.content);
        }

        // 2. Check loaded templates
        {
            let templates = self.templates.read().await;
            if let Some(content) = templates.get(template_id) {
                // Update cache
                self.cache.set(template_id, content.clone(), "1.0.0".to_string()).await?;
                return Ok(content.clone());
            }
        }

        // 3. Try to load from S3
        if let Some(s3) = &self.s3_client {
            if let Ok(content) = self.load_from_s3(s3, template_id).await {
                // Update cache and memory
                self.cache.set(template_id, content.clone(), "1.0.0".to_string()).await?;
                self.templates.write().await.insert(template_id.to_string(), content.clone());
                return Ok(content);
            }
        }

        // 4. Try to load from local filesystem
        let content = self.load_from_filesystem(template_id).await?;

        // Update cache and memory
        self.cache.set(template_id, content.clone(), "1.0.0".to_string()).await?;
        self.templates.write().await.insert(template_id.to_string(), content.clone());

        Ok(content)
    }

    async fn load_local_templates(&mut self) -> Result<()> {
        let template_dirs = vec!["typst", "jinja"];

        for dir_name in template_dirs {
            let dir_path = self.local_path.join(dir_name);
            if !dir_path.exists() {
                tracing::warn!("Template directory does not exist: {:?}", dir_path);
                continue;
            }

            let entries = std::fs::read_dir(&dir_path)
                .context(format!("Failed to read template directory: {:?}", dir_path))?;

            for entry in entries {
                let entry = entry?;
                let path = entry.path();

                if path.extension().and_then(|s| s.to_str()) == Some("typ") ||
                   path.extension().and_then(|s| s.to_str()) == Some("jinja") {
                    let template_id = path.file_stem()
                        .and_then(|s| s.to_str())
                        .ok_or_else(|| anyhow::anyhow!("Invalid template filename"))?;

                    let content = tokio::fs::read_to_string(&path).await
                        .context(format!("Failed to read template: {:?}", path))?;

                    // Load into engine and memory
                    self.engine.load_template(template_id, &content).await?;
                    self.templates.write().await.insert(template_id.to_string(), content);

                    tracing::info!("Loaded template: {}", template_id);
                }
            }
        }

        Ok(())
    }

    async fn load_from_filesystem(&self, template_id: &str) -> Result<String> {
        // Try different extensions
        let extensions = vec!["typ", "jinja", "html"];

        for ext in extensions {
            let path = self.local_path.join(format!("{}.{}", template_id, ext));
            if path.exists() {
                let content = tokio::fs::read_to_string(&path).await
                    .context(format!("Failed to read template: {:?}", path))?;

                // Load into engine
                self.engine.load_template(template_id, &content).await?;

                return Ok(content);
            }
        }

        Err(anyhow::anyhow!("Template not found: {}", template_id))
    }

    async fn load_from_s3(&self, s3: &S3Client, template_id: &str) -> Result<String> {
        let key = format!("templates/{}.typ", template_id);
        let content = s3.get_object("templates", &key).await?;

        // Load into engine
        self.engine.load_template(template_id, &content).await?;

        Ok(content)
    }

    pub async fn reload_template(&self, template_id: &str) -> Result<()> {
        // Invalidate cache
        self.cache.invalidate(template_id).await?;

        // Remove from memory
        self.templates.write().await.remove(template_id);

        // Force reload
        self.get_template(template_id).await?;

        Ok(())
    }

    pub async fn update_template(&self, template_id: &str, content: String) -> Result<()> {
        // Update in engine
        self.engine.load_template(template_id, &content).await?;

        // Update in memory
        self.templates.write().await.insert(template_id.to_string(), content.clone());

        // Update cache
        self.cache.set(template_id, content.clone(), "1.0.0".to_string()).await?;

        // Optionally save to S3
        if let Some(s3) = &self.s3_client {
            let key = format!("templates/{}.typ", template_id);
            s3.put_object("templates", &key, content.as_bytes().to_vec(), "text/plain").await?;
        }

        // Save to local filesystem
        let path = self.local_path.join(format!("{}.typ", template_id));
        tokio::fs::write(&path, &content).await
            .context(format!("Failed to save template: {:?}", path))?;

        Ok(())
    }

    pub fn get_engine(&self) -> Arc<TemplateEngine> {
        self.engine.clone()
    }

    pub async fn list_templates(&self) -> Vec<String> {
        self.templates.read().await.keys().cloned().collect()
    }
}