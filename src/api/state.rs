use governor::{clock::DefaultClock, state::keyed::DashMapStateStore, Quota, RateLimiter};
use std::path::PathBuf;
use std::sync::Arc;

use crate::application::orchestrators::{DocumentOrchestrator, NotificationOrchestrator};
use crate::infrastructure::cache::CacheService;
use crate::infrastructure::generators::GeneratorFactory;
use crate::infrastructure::notifications::{
    EmailService, EvolutionApiClient, GreenApiClient, WhatsAppProvider,
};
use crate::infrastructure::resilience::{ConcurrencyConfig, ConcurrencyController};
use crate::infrastructure::storage::StorageService;
use crate::infrastructure::template_resolver::{AssetManager, TemplateApiClient, TemplateResolver};
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
    pub notification_orchestrator: Arc<NotificationOrchestrator>,
    pub cache_service: Arc<CacheService>,
    pub storage_service: Arc<StorageService>,
    /// Concurrency controller for resource-aware job scheduling
    pub concurrency_controller: Arc<ConcurrencyController>,
    /// Template resolver for dynamic templates from .NET backend (None if not configured)
    pub template_resolver: Option<Arc<TemplateResolver>>,
}

/// WhatsApp provider selection
#[derive(Clone, Debug, Default, PartialEq)]
pub enum WhatsAppProviderType {
    #[default]
    Evolution,
    GreenApi,
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
    // WhatsApp provider selection
    pub whatsapp_provider: WhatsAppProviderType,
    // WhatsApp EvolutionAPI configuration
    pub evolution_api_url: Option<String>,
    pub evolution_api_key: Option<String>,
    pub evolution_instance: Option<String>,
    // WhatsApp Green-API configuration
    pub green_api_url: Option<String>,
    pub green_api_media_url: Option<String>,
    pub green_api_instance_id: Option<String>,
    pub green_api_token: Option<String>,
    // Template resolution (optional - if not set, uses built-in templates only)
    pub core_api_url: Option<String>,
    pub core_service_token: Option<String>,
    pub template_cache_ttl_secs: u64,
    pub template_asset_dir: String,
    pub s3_bucket_templates: String,
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
            // WhatsApp provider defaults
            whatsapp_provider: WhatsAppProviderType::default(),
            // EvolutionAPI defaults
            evolution_api_url: None,
            evolution_api_key: None,
            evolution_instance: None,
            // Green-API defaults
            green_api_url: None,
            green_api_media_url: None,
            green_api_instance_id: None,
            green_api_token: None,
            // Template resolution defaults
            core_api_url: None,
            core_service_token: None,
            template_cache_ttl_secs: 300, // 5 minutes
            template_asset_dir: "./template-assets".to_string(),
            s3_bucket_templates: "templates".to_string(),
        }
    }
}

impl ApiState {
    pub async fn new(config: AppConfig) -> anyhow::Result<Self> {
        // Initialize S3 - check for R2 config first, fallback to AWS
        let s3_client = if let (Ok(endpoint), Ok(access_key), Ok(secret_key)) = (
            std::env::var("S3_ENDPOINT"),
            std::env::var("S3_ACCESS_KEY"),
            std::env::var("S3_SECRET_KEY"),
        ) {
            // Extract account ID from Cloudflare R2 endpoint
            // Format: https://{account_id}.r2.cloudflarestorage.com/...
            let account_id = endpoint
                .trim_start_matches("https://")
                .split('.')
                .next()
                .unwrap_or("")
                .to_string();

            tracing::info!(
                "Initializing S3 client for Cloudflare R2. Endpoint: {}, Account: {}",
                endpoint,
                account_id
            );

            Arc::new(S3Client::new_for_r2(account_id, access_key, secret_key).await?)
        } else {
            tracing::info!("Initializing S3 client with AWS credentials from environment");
            Arc::new(S3Client::new().await?)
        };

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
            tracing::warn!(
                "Email service not configured - SMTP_HOST, SMTP_USER, SMTP_PASS required"
            );
            None
        };

        // Initialize WhatsApp service based on configured provider
        let whatsapp_service: Option<Arc<dyn WhatsAppProvider>> = match config.whatsapp_provider {
            WhatsAppProviderType::Evolution => {
                if let (Some(url), Some(key), Some(instance)) = (
                    config.evolution_api_url.as_ref(),
                    config.evolution_api_key.as_ref(),
                    config.evolution_instance.as_ref(),
                ) {
                    tracing::info!(
                        "WhatsApp service configured with EvolutionAPI, instance: {}",
                        instance
                    );
                    Some(Arc::new(EvolutionApiClient::new(
                        url.clone(),
                        key.clone(),
                        instance.clone(),
                    )))
                } else {
                    tracing::warn!(
                        "EvolutionAPI selected but not configured - EVOLUTION_API_URL, EVOLUTION_API_KEY, EVOLUTION_INSTANCE required"
                    );
                    None
                }
            }
            WhatsAppProviderType::GreenApi => {
                if let (Some(url), Some(instance_id), Some(token)) = (
                    config.green_api_url.as_ref(),
                    config.green_api_instance_id.as_ref(),
                    config.green_api_token.as_ref(),
                ) {
                    // Use media URL if configured for direct file uploads
                    let client = if let Some(media_url) = config.green_api_media_url.as_ref() {
                        tracing::info!(
                            "WhatsApp service configured with Green-API, instance: {}, media_url: {}",
                            instance_id,
                            media_url
                        );
                        GreenApiClient::with_media_url(
                            url.clone(),
                            media_url.clone(),
                            instance_id.clone(),
                            token.clone(),
                        )
                    } else {
                        tracing::info!(
                            "WhatsApp service configured with Green-API, instance: {}",
                            instance_id
                        );
                        GreenApiClient::new(url.clone(), instance_id.clone(), token.clone())
                    };
                    Some(Arc::new(client))
                } else {
                    tracing::warn!(
                        "Green-API selected but not configured - GREEN_API_URL, GREEN_API_INSTANCE_ID, GREEN_API_TOKEN required"
                    );
                    None
                }
            }
        };

        // Initialize template resolver (optional - only if CORE_API_URL is configured)
        let template_resolver = if let (Some(api_url), Some(token)) = (
            config.core_api_url.as_ref(),
            config.core_service_token.as_ref(),
        ) {
            match TemplateApiClient::new(api_url.clone(), token.clone()) {
                Ok(api_client) => {
                    let asset_manager = Arc::new(AssetManager::new(
                        s3_client.clone(),
                        PathBuf::from(&config.template_asset_dir),
                        config.s3_bucket_templates.clone(),
                    ));
                    let resolver = Arc::new(TemplateResolver::new(
                        Arc::new(api_client),
                        asset_manager,
                        config.template_cache_ttl_secs,
                    ));
                    tracing::info!("Template resolver configured with backend: {}", api_url);
                    Some(resolver)
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize template API client: {}", e);
                    None
                }
            }
        } else {
            tracing::info!("Template resolver not configured - using built-in templates only");
            None
        };

        // Initialize Typst generator for dynamic templates
        let typst_generator = Arc::new(crate::infrastructure::generators::TypstGenerator::new(
            work_dir.clone(),
        ));

        // Initialize document orchestrator
        let document_orchestrator = Arc::new(DocumentOrchestrator::new(
            generator_factory.clone(),
            storage_service.clone(),
            cache_service.clone(),
            email_service.clone(),
            whatsapp_service.clone(),
            template_resolver.clone(),
            typst_generator,
        ));

        // Initialize notification orchestrator
        let notification_orchestrator = Arc::new(NotificationOrchestrator::new(
            email_service,
            whatsapp_service,
            cache_service.clone(),
        ));

        // Initialize concurrency controller with environment config
        let concurrency_config = ConcurrencyConfig::from_env();
        tracing::info!(
            "Concurrency controller initialized: max_pdf={}, max_excel={}, max_csv={}, max_total={}, adaptive={}",
            concurrency_config.max_pdf_concurrent,
            concurrency_config.max_excel_concurrent,
            concurrency_config.max_csv_concurrent,
            concurrency_config.max_total_concurrent,
            concurrency_config.adaptive_enabled
        );
        let concurrency_controller = Arc::new(ConcurrencyController::new(concurrency_config));

        // Start background resource monitoring
        let _monitor_handle = concurrency_controller.start_monitor();

        Ok(ApiState {
            s3_client,
            template_manager,
            rate_limiter,
            config: Arc::new(config),
            generator_factory,
            document_orchestrator,
            notification_orchestrator,
            cache_service,
            storage_service,
            concurrency_controller,
            template_resolver,
        })
    }
}
