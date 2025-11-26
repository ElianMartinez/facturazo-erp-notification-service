use governor::{clock::DefaultClock, state::keyed::DashMapStateStore, Quota, RateLimiter};
use std::path::PathBuf;
use std::sync::Arc;

use crate::application::orchestrators::DocumentOrchestrator;
use crate::infrastructure::cache::CacheService;
use crate::infrastructure::generators::GeneratorFactory;
use crate::infrastructure::notifications::{EmailService, EvolutionApiClient};
use crate::infrastructure::storage::StorageService;
use crate::storage::s3::S3Client;
use crate::templates::TemplateManager;

// Key format: "tenant_id:user_id"
pub type KeyedRateLimiter = Arc<RateLimiter<String, DashMapStateStore<String>, DefaultClock>>;

#[derive(Clone)]
pub struct ApiState {
    pub s3_client: Arc<S3Client>,
    pub template_manager: Arc<TemplateManager>,
    pub rate_limiter: KeyedRateLimiter,
    pub config: Arc<AppConfig>,
    pub generator_factory: Arc<GeneratorFactory>,
    pub document_orchestrator: Arc<DocumentOrchestrator>,
    pub cache_service: Arc<CacheService>,
    pub storage_service: Arc<StorageService>,
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
    // Email SMTP configuration
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    pub smtp_user: Option<String>,
    pub smtp_pass: Option<String>,
    pub smtp_from_email: String,
    pub smtp_from_name: String,
    // WhatsApp EvolutionAPI configuration
    pub evolution_api_url: Option<String>,
    pub evolution_api_key: Option<String>,
    pub evolution_instance: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            max_sync_size_bytes: 1_048_576,     // 1MB
            max_upload_size_bytes: 104_857_600, // 100MB
            rate_limit_per_minute: 100,
            rate_limit_burst: 20,
            sync_timeout_ms: 5000,
            s3_bucket_documents: "documents".to_string(),
            s3_bucket_temp: "temp-uploads".to_string(),
            enable_compression: true,
            // Email defaults
            smtp_host: None,
            smtp_port: 587,
            smtp_user: None,
            smtp_pass: None,
            smtp_from_email: "noreply@example.com".to_string(),
            smtp_from_name: "PDF Service".to_string(),
            // WhatsApp defaults
            evolution_api_url: None,
            evolution_api_key: None,
            evolution_instance: None,
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
            "output".to_string(),
        ));

        // Initialize rate limiter
        let quota =
            Quota::per_minute(std::num::NonZeroU32::new(config.rate_limit_per_minute).unwrap())
                .allow_burst(std::num::NonZeroU32::new(config.rate_limit_burst).unwrap());
        let rate_limiter = Arc::new(RateLimiter::dashmap_with_clock(
            quota,
            &DefaultClock::default(),
        ));

        // Initialize new infrastructure components
        let work_dir = PathBuf::from("./work");
        let generator_factory = Arc::new(GeneratorFactory::new(work_dir.clone()));
        let cache_service = Arc::new(CacheService::new());
        let storage_service = Arc::new(StorageService::new(
            PathBuf::from("./storage"),
            config.s3_bucket_documents.clone(),
        ));

        // Initialize Email service if configured
        let email_service = if let (Some(host), Some(user), Some(pass)) = (
            config.smtp_host.as_ref(),
            config.smtp_user.as_ref(),
            config.smtp_pass.as_ref(),
        ) {
            tracing::info!("Email service configured with host: {}", host);
            Some(Arc::new(EmailService::new(
                host.clone(),
                config.smtp_port,
                user.clone(),
                pass.clone(),
                config.smtp_from_email.clone(),
                config.smtp_from_name.clone(),
                true, // use TLS
            )))
        } else {
            tracing::warn!("Email service not configured - SMTP_HOST, SMTP_USER, SMTP_PASS required");
            None
        };

        // Initialize WhatsApp service if configured
        let whatsapp_service = if let (Some(url), Some(key), Some(instance)) = (
            config.evolution_api_url.as_ref(),
            config.evolution_api_key.as_ref(),
            config.evolution_instance.as_ref(),
        ) {
            tracing::info!("WhatsApp service configured with instance: {}", instance);
            Some(Arc::new(EvolutionApiClient::new(
                url.clone(),
                key.clone(),
                instance.clone(),
            )))
        } else {
            tracing::warn!("WhatsApp service not configured - EVOLUTION_API_URL, EVOLUTION_API_KEY, EVOLUTION_INSTANCE required");
            None
        };

        // Initialize document orchestrator
        let document_orchestrator = Arc::new(DocumentOrchestrator::new(
            generator_factory.clone(),
            storage_service.clone(),
            cache_service.clone(),
            email_service,
            whatsapp_service,
        ));

        Ok(ApiState {
            s3_client,
            template_manager,
            rate_limiter,
            config: Arc::new(config),
            generator_factory,
            document_orchestrator,
            cache_service,
            storage_service,
        })
    }
}
