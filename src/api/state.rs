use std::sync::Arc;
use rdkafka::producer::FutureProducer;
use redis::aio::ConnectionManager;
use sqlx::PgPool;
use governor::{Quota, RateLimiter, clock::DefaultClock, state::keyed::DashMapStateStore};

use crate::templates::TemplateManager;
use crate::storage::s3::S3Client;

pub type KeyedRateLimiter = Arc<RateLimiter<String, DashMapStateStore<String>, DefaultClock>>;

#[derive(Clone)]
pub struct ApiState {
    pub kafka_producer: Arc<FutureProducer>,
    pub redis: ConnectionManager,
    pub s3_client: Arc<S3Client>,
    pub db: PgPool,
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
    pub kafka_topic_priority: String,
    pub kafka_topic_bulk: String,
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
            kafka_topic_priority: "doc.requests.priority".to_string(),
            kafka_topic_bulk: "doc.requests.bulk".to_string(),
            s3_bucket_documents: "documents".to_string(),
            s3_bucket_temp: "temp-uploads".to_string(),
            enable_compression: true,
        }
    }
}

impl ApiState {
    pub async fn new(config: AppConfig) -> anyhow::Result<Self> {
        // Initialize Kafka producer
        let kafka_producer = create_kafka_producer()?;

        // Initialize Redis
        let redis_client = redis::Client::open(
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string())
        )?;
        let redis = ConnectionManager::new(redis_client).await?;

        // Initialize S3
        let s3_client = Arc::new(S3Client::new().await?);

        // Initialize database
        let db = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;

        // Initialize template manager
        let template_manager = Arc::new(
            TemplateManager::new(
                std::path::PathBuf::from("templates"),
                Some(std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string())),
                Some(s3_client.clone()),
            ).await?
        );

        // Initialize rate limiter
        let quota = Quota::per_minute(std::num::NonZeroU32::new(config.rate_limit_per_minute).unwrap())
            .allow_burst(std::num::NonZeroU32::new(config.rate_limit_burst).unwrap());
        let rate_limiter = Arc::new(RateLimiter::dashmap_with_clock(quota, &DefaultClock::default()));

        Ok(ApiState {
            kafka_producer: Arc::new(kafka_producer),
            redis,
            s3_client,
            db,
            template_manager,
            rate_limiter,
            config: Arc::new(config),
        })
    }
}

fn create_kafka_producer() -> anyhow::Result<FutureProducer> {
    use rdkafka::ClientConfig;

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string()))
        .set("message.timeout.ms", "5000")
        .set("compression.type", "snappy")
        .set("queue.buffering.max.messages", "100000")
        .set("queue.buffering.max.kbytes", "1048576")
        .set("batch.num.messages", "10000")
        .create()?;

    Ok(producer)
}