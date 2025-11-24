//! Storage layer for document persistence
//!
//! Currently implements local file storage, will add R2 support

use anyhow::Result;
use std::path::Path;
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