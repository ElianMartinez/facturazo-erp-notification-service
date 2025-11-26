// use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer};
use anyhow::Result;
use pdf_services::api::state::AppConfig;
use pdf_services::api::{configure_routes, ApiState};
use prometheus::Registry;
use std::env;
use tracing_subscriber::EnvFilter;

#[actix_web::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Document Generator API");

    // Initialize Prometheus metrics
    let _prometheus = Registry::new();
    // Comentado temporalmente - requiere feature "process" y OS Linux
    // prometheus::default_registry().register(Box::new(
    //     prometheus::process_collector::ProcessCollector::for_self(),
    // ))?;

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
        s3_bucket_documents: env::var("S3_BUCKET_DOCUMENTS")
            .unwrap_or_else(|_| "documents".to_string()),
        s3_bucket_temp: env::var("S3_BUCKET_TEMP").unwrap_or_else(|_| "temp-uploads".to_string()),
        enable_compression: env::var("ENABLE_COMPRESSION")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true),
        // Email SMTP configuration
        smtp_host: env::var("SMTP_HOST").ok(),
        smtp_port: env::var("SMTP_PORT")
            .unwrap_or_else(|_| "587".to_string())
            .parse()
            .unwrap_or(587),
        smtp_user: env::var("SMTP_USER").ok(),
        smtp_pass: env::var("SMTP_PASS").ok(),
        smtp_from_email: env::var("SMTP_FROM_EMAIL")
            .unwrap_or_else(|_| "noreply@example.com".to_string()),
        smtp_from_name: env::var("SMTP_FROM_NAME")
            .unwrap_or_else(|_| "PDF Service".to_string()),
        // WhatsApp EvolutionAPI configuration
        evolution_api_url: env::var("EVOLUTION_API_URL").ok(),
        evolution_api_key: env::var("EVOLUTION_API_KEY").ok(),
        evolution_instance: env::var("EVOLUTION_INSTANCE").ok(),
    };

    Ok(config)
}
