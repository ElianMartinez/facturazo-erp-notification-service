use std::sync::Arc;
use governor::{Quota, RateLimiter, clock::DefaultClock, state::keyed::DashMapStateStore};

use crate::templates::TemplateManager;
use crate::storage::s3::S3Client;

// Key format: "tenant_id:user_id"
pub type KeyedRateLimiter = Arc<RateLimiter<String, DashMapStateStore<String>, DefaultClock>>;

#[derive(Clone)]
pub struct ApiState {
    pub s3_client: Arc<S3Client>,
    pub template_manager: Arc<TemplateManager>,
    pub rate_limiter: KeyedRateLimiter,
    pub config: Arc<AppConfig>,
}

#[derive(Clone)]
pub struct AppConfig {
    pub max_sync_size_bytes: usize,
    pub max_upload_size_bytes: usize,
    pub rate_limit_per_minute: u32,
    pub rate_limit_burst: u32,
    pub sync_timeout_ms: u64,
    pub s3_bucket_documents: String,
    pub s3_bucket_temp: String,
    pub enable_compression: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            max_sync_size_bytes: 1_048_576,      // 1MB
            max_upload_size_bytes: 104_857_600,  // 100MB
            rate_limit_per_minute: 100,
            rate_limit_burst: 20,
            sync_timeout_ms: 5000,
            s3_bucket_documents: "documents".to_string(),
            s3_bucket_temp: "temp-uploads".to_string(),
            enable_compression: true,
        }
    }
}

impl ApiState {
    pub async fn new(config: AppConfig) -> anyhow::Result<Self> {
        // Initialize S3
        let s3_client = Arc::new(S3Client::new().await?);

        // Initialize template manager
        let template_manager = Arc::new(TemplateManager::new(
            "templates".to_string(),
            "output".to_string()
        ));

        // Initialize rate limiter
        let quota = Quota::per_minute(std::num::NonZeroU32::new(config.rate_limit_per_minute).unwrap())
            .allow_burst(std::num::NonZeroU32::new(config.rate_limit_burst).unwrap());
        let rate_limiter = Arc::new(RateLimiter::dashmap_with_clock(quota, &DefaultClock::default()));

        Ok(ApiState {
            s3_client,
            template_manager,
            rate_limiter,
            config: Arc::new(config),
        })
    }
}