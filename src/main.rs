use actix_web::{web, App, HttpServer, middleware};
use actix_cors::Cors;
use document_generator::api::{ApiState, configure_routes};
use document_generator::api::state::AppConfig;
use tracing_subscriber::EnvFilter;
use prometheus::Registry;
use std::env;
use anyhow::Result;

#[actix_web::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .init();

    tracing::info!("Starting Document Generator API");

    // Initialize Prometheus metrics
    let prometheus = Registry::new();
    prometheus::default_registry()
        .register(Box::new(prometheus::process_collector::ProcessCollector::for_self()))?;

    // Load configuration
    let config = load_config()?;

    // Initialize application state
    let state = web::Data::new(ApiState::new(config).await?);

    // Get server settings
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()?;

    tracing::info!("Starting server on {}:{}", host, port);

    // Start HTTP server
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            .wrap(middleware::NormalizePath::trim())
            .configure(configure_routes)
    })
    .bind((host.as_str(), port))?
    .run()
    .await?;

    Ok(())
}

fn load_config() -> Result<AppConfig> {
    let config = AppConfig {
        max_sync_size_bytes: env::var("MAX_SYNC_SIZE_BYTES")
            .unwrap_or_else(|_| "1048576".to_string())
            .parse()?,
        max_upload_size_bytes: env::var("MAX_UPLOAD_SIZE_BYTES")
            .unwrap_or_else(|_| "104857600".to_string())
            .parse()?,
        rate_limit_per_minute: env::var("RATE_LIMIT_PER_MINUTE")
            .unwrap_or_else(|_| "100".to_string())
            .parse()?,
        rate_limit_burst: env::var("RATE_LIMIT_BURST")
            .unwrap_or_else(|_| "20".to_string())
            .parse()?,
        sync_timeout_ms: env::var("SYNC_TIMEOUT_MS")
            .unwrap_or_else(|_| "5000".to_string())
            .parse()?,
        kafka_topic_priority: env::var("KAFKA_TOPIC_PRIORITY")
            .unwrap_or_else(|_| "doc.requests.priority".to_string()),
        kafka_topic_bulk: env::var("KAFKA_TOPIC_BULK")
            .unwrap_or_else(|_| "doc.requests.bulk".to_string()),
        s3_bucket_documents: env::var("S3_BUCKET_DOCUMENTS")
            .unwrap_or_else(|_| "documents".to_string()),
        s3_bucket_temp: env::var("S3_BUCKET_TEMP")
            .unwrap_or_else(|_| "temp-uploads".to_string()),
        enable_compression: env::var("ENABLE_COMPRESSION")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true),
    };

    Ok(config)
}