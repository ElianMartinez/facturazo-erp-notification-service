//! Storage layer for document persistence
//!
//! Currently implements local file storage, will add R2 support

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;

/// Storage trait for document persistence
#[async_trait::async_trait]
pub trait Storage: Send + Sync {
    async fn store(&self, key: &str, data: &[u8]) -> Result<String>;
    async fn retrieve(&self, key: &str) -> Result<Vec<u8>>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn exists(&self, key: &str) -> Result<bool>;
}

/// Local file storage implementation
pub struct LocalStorage {
    base_path: String,
}

impl LocalStorage {
    pub fn new(base_path: String) -> Self {
        Self { base_path }
    }
}

#[async_trait::async_trait]
impl Storage for LocalStorage {
    async fn store(&self, key: &str, data: &[u8]) -> Result<String> {
        let path = Path::new(&self.base_path).join(key);

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&path, data).await?;
        Ok(path.to_string_lossy().to_string())
    }

    async fn retrieve(&self, key: &str) -> Result<Vec<u8>> {
        let path = Path::new(&self.base_path).join(key);
        Ok(fs::read(&path).await?)
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let path = Path::new(&self.base_path).join(key);
        fs::remove_file(&path).await?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let path = Path::new(&self.base_path).join(key);
        Ok(path.exists())
    }
}

/// Storage service wrapper for orchestrators
pub struct StorageService {
    storage: Arc<dyn Storage>,
    base_url: String,
}

impl StorageService {
    pub fn new(base_path: PathBuf, base_url: String) -> Self {
        Self {
            storage: Arc::new(LocalStorage::new(base_path.to_string_lossy().to_string())),
            base_url,
        }
    }

    /// Upload a file and return its URL
    pub async fn upload(&self, path: &str, data: &[u8], _content_type: &str) -> Result<String> {
        let stored_path = self.storage.store(path, data).await?;

        // Return URL (in production this would be a proper URL, for local it's the file path)
        if self.base_url.is_empty() {
            Ok(format!("file://{}", stored_path))
        } else {
            Ok(format!("{}/{}", self.base_url, path))
        }
    }

    /// Download a file
    pub async fn download(&self, path: &str) -> Result<Vec<u8>> {
        self.storage.retrieve(path).await
    }

    /// Delete a file
    pub async fn delete(&self, path: &str) -> Result<()> {
        self.storage.delete(path).await
    }

    /// Check if a file exists
    pub async fn exists(&self, path: &str) -> Result<bool> {
        self.storage.exists(path).await
    }
}
