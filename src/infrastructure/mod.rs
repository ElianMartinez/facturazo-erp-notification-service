//! Infrastructure layer
//!
//! Contains implementations of external services and repositories

pub mod database;
pub mod cache;
pub mod storage;
pub mod generators;
pub mod notifications;
pub mod persistence;
pub mod observability;
pub mod kafka;
pub mod resilience;

use anyhow::Result;
use std::sync::Arc;

/// Infrastructure container
pub struct Infrastructure {
    pub database: Arc<database::Database>,
    pub cache: Arc<cache::CacheManager>,
    pub resilience: Arc<resilience::ResilienceManager>,
}

impl Infrastructure {
    /// Create new infrastructure container
    pub async fn new(config: &crate::config::AppConfig) -> Result<Self> {
        // Initialize database
        let database = Arc::new(
            database::Database::new(&config.database.path).await?
        );

        // Initialize cache
        let cache = Arc::new(cache::CacheManager::new());

        // Initialize resilience manager
        let resilience = Arc::new(resilience::ResilienceManager::new(
            resilience::ResilienceConfig::default()
        ));

        Ok(Self {
            database,
            cache,
            resilience,
        })
    }

    /// Health check for all infrastructure services
    pub async fn health_check(&self) -> Result<()> {
        // Check database
        self.database.health_check().await?;

        // Cache is always available (in-memory)

        Ok(())
    }

    /// Get health status with all dependencies
    pub async fn health_status(&self) -> resilience::HealthStatus {
        self.resilience.health_status().await
    }

    /// Shutdown infrastructure services
    pub async fn shutdown(&self) -> Result<()> {
        // Cleanup resources
        Ok(())
    }
}