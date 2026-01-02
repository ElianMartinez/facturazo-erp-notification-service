//! PDF Services - Combined HTTP API + Kafka Worker
//!
//! This is the main entry point that runs both:
//! - HTTP API server for synchronous document generation
//! - Kafka worker for asynchronous message processing

use actix_web::{middleware, web, App, HttpServer};
use anyhow::Result;
use pdf_services::api::state::{AppConfig, WhatsAppProviderType};
use pdf_services::api::{configure_routes, ApiState};
use pdf_services::application::orchestrators::{DocumentOrchestrator, NotificationOrchestrator};
use pdf_services::infrastructure::cache::CacheService;
use pdf_services::infrastructure::generators::GeneratorFactory;
use pdf_services::infrastructure::notifications::{
    EmailService, EvolutionApiClient, GreenApiClient, WhatsAppProvider,
};
use pdf_services::infrastructure::storage::StorageService;
use pdf_services::kafka::erp_messages::ErpIntegrationEvent;
use pdf_services::kafka::handlers::{KafkaHandler, KafkaMessage};
use prometheus::Registry;
use rdkafka::client::ClientContext;
use rdkafka::config::{ClientConfig, RDKafkaLogLevel};
use rdkafka::consumer::stream_consumer::StreamConsumer;
use rdkafka::consumer::{CommitMode, Consumer, ConsumerContext, Rebalance};
use rdkafka::error::KafkaResult;
use rdkafka::message::{Headers, Message};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::TopicPartitionList;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

/// Custom consumer context for logging rebalance events
struct CustomContext;

impl ClientContext for CustomContext {}

impl ConsumerContext for CustomContext {
    fn pre_rebalance(&self, rebalance: &Rebalance) {
        info!("Kafka pre rebalance: {:?}", rebalance);
    }

    fn post_rebalance(&self, rebalance: &Rebalance) {
        info!("Kafka post rebalance: {:?}", rebalance);
    }

    fn commit_callback(&self, result: KafkaResult<()>, _offsets: &TopicPartitionList) {
        if let Err(e) = result {
            warn!("Kafka commit callback error: {}", e);
        }
    }
}

type LoggingConsumer = StreamConsumer<CustomContext>;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("Starting PDF Services (HTTP + Kafka)");

    // Initialize Prometheus metrics
    let _prometheus = Registry::new();

    // Load configuration
    let config = load_config()?;

    // Check if Kafka is enabled
    let kafka_enabled = env::var("KAFKA_ENABLED")
        .unwrap_or_else(|_| "true".to_string())
        .parse::<bool>()
        .unwrap_or(true);

    // Shared shutdown signal
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Initialize application state for HTTP server
    let state = web::Data::new(ApiState::new(config).await?);

    // Get server settings
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()?;

    info!("HTTP server will run on {}:{}", host, port);

    // Spawn HTTP server
    let http_handle = {
        let state = state.clone();
        let host = host.clone();
        let max_payload_size = state.config.max_upload_size_bytes;
        tokio::spawn(async move {
            info!("Starting HTTP server on {}:{}", host, port);
            info!(
                "Max payload size configured: {} bytes ({} MB)",
                max_payload_size,
                max_payload_size / 1_048_576
            );

            // Configure payload limits to handle large report requests
            let json_config = web::JsonConfig::default()
                .limit(max_payload_size)
                .error_handler(move |err, _req| {
                    let message = format!("JSON payload error: {}", err);
                    actix_web::error::InternalError::from_response(
                        err,
                        actix_web::HttpResponse::PayloadTooLarge().json(serde_json::json!({
                            "error": "payload_too_large",
                            "message": message,
                            "max_size_bytes": max_payload_size
                        })),
                    )
                    .into()
                });

            let payload_config = web::PayloadConfig::default().limit(max_payload_size);

            HttpServer::new(move || {
                App::new()
                    .app_data(state.clone())
                    .app_data(json_config.clone())
                    .app_data(payload_config.clone())
                    .wrap(middleware::Logger::default())
                    .wrap(middleware::NormalizePath::trim())
                    .configure(configure_routes)
            })
            .bind((host.as_str(), port))
            .expect("Failed to bind HTTP server")
            .run()
            .await
            .expect("HTTP server error");
        })
    };

    // Spawn Kafka worker if enabled
    let kafka_handle = if kafka_enabled {
        let mut shutdown_rx = shutdown_tx.subscribe();
        Some(tokio::spawn(async move {
            if let Err(e) = run_kafka_worker(&mut shutdown_rx).await {
                error!("Kafka worker error: {}", e);
            }
        }))
    } else {
        info!("Kafka worker disabled (KAFKA_ENABLED=false)");
        None
    };

    // Wait for shutdown signal
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal (Ctrl+C)");
            let _ = shutdown_tx.send(());
        }
        result = http_handle => {
            match result {
                Ok(_) => info!("HTTP server stopped"),
                Err(e) => error!("HTTP server task error: {}", e),
            }
        }
        result = async {
            if let Some(handle) = kafka_handle {
                handle.await
            } else {
                // If kafka is disabled, just wait forever
                std::future::pending::<Result<(), tokio::task::JoinError>>().await
            }
        } => {
            match result {
                Ok(_) => info!("Kafka worker stopped"),
                Err(e) => error!("Kafka worker task error: {}", e),
            }
        }
    }

    info!("PDF Services shutdown complete");
    Ok(())
}

/// Run the Kafka consumer/producer worker
async fn run_kafka_worker(shutdown_rx: &mut broadcast::Receiver<()>) -> Result<()> {
    // Support both naming conventions for Kafka brokers
    let brokers = env::var("KAFKA_BROKERS")
        .or_else(|_| env::var("KAFKA_BOOTSTRAP_SERVERS"))
        .unwrap_or_else(|_| "localhost:9092".to_string());

    // Support both naming conventions for consumer group
    let group_id = env::var("KAFKA_GROUP_ID")
        .or_else(|_| env::var("KAFKA_CONSUMER_GROUP"))
        .unwrap_or_else(|_| "pdf-service-worker".to_string());

    // Topic naming follows ERP Core convention: {env}.facturazo.ERP.{domain}.{event}
    // Topics are created by ERP Core on startup - pdf-services only consumes
    let env_prefix = env::var("KAFKA_ENV_PREFIX").unwrap_or_else(|_| "dev".to_string());
    let base_namespace = format!("{}.facturazo.ERP", env_prefix);

    // Build default topic names matching ERP Core's KafkaTopics constants
    let default_topics = format!(
        "{}.documents.generate_request,{}.notifications.dispatch_request",
        base_namespace, base_namespace
    );
    let topics_str = env::var("KAFKA_TOPICS").unwrap_or(default_topics);
    let topics: Vec<&str> = topics_str.split(',').collect();

    // Response topic for sending results back to ERP Core
    let default_response_topic = format!("{}.documents.events", base_namespace);
    let response_topic = env::var("KAFKA_RESPONSE_TOPIC").unwrap_or(default_response_topic);

    info!("Kafka worker connecting to brokers: {}", brokers);
    info!("Kafka consumer group: {}", group_id);
    info!("Kafka subscribing to topics: {:?}", topics);

    // Create Kafka consumer
    let consumer: LoggingConsumer = ClientConfig::new()
        .set("group.id", &group_id)
        .set("bootstrap.servers", &brokers)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "false")
        .set("auto.offset.reset", "earliest")
        .set_log_level(RDKafkaLogLevel::Debug)
        .create_with_context(CustomContext)?;

    consumer.subscribe(&topics)?;

    // Create Kafka producer for responses
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &brokers)
        .set("message.timeout.ms", "5000")
        .create()?;

    // Initialize services for Kafka worker
    let work_dir = PathBuf::from("./work");
    let generator_factory = Arc::new(GeneratorFactory::new(work_dir.clone()));
    let cache_service = Arc::new(CacheService::new());
    let storage_service = Arc::new(StorageService::new(
        PathBuf::from("./storage"),
        "documents".to_string(),
    ));

    // Initialize Email service if configured
    let email_service = match (
        env::var("SMTP_HOST").ok(),
        env::var("SMTP_USER").ok(),
        env::var("SMTP_PASS").ok(),
    ) {
        (Some(host), Some(user), Some(pass)) => {
            let port = env::var("SMTP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(587);
            let from_email =
                env::var("SMTP_FROM_EMAIL").unwrap_or_else(|_| "noreply@example.com".to_string());
            let from_name =
                env::var("SMTP_FROM_NAME").unwrap_or_else(|_| "PDF Service".to_string());

            info!("Kafka worker: Email service configured with host: {}", host);
            Some(Arc::new(EmailService::new(
                host, port, user, pass, from_email, from_name, true, // use TLS
            )))
        }
        _ => {
            warn!("Kafka worker: Email service not configured - SMTP_HOST, SMTP_USER, SMTP_PASS required");
            None
        }
    };

    // Initialize WhatsApp service if configured
    let whatsapp_service: Option<Arc<dyn WhatsAppProvider>> = {
        let provider_type = match env::var("WHATSAPP_PROVIDER")
            .unwrap_or_else(|_| "evolution".to_string())
            .to_lowercase()
            .as_str()
        {
            "green" | "greenapi" | "green-api" => WhatsAppProviderType::GreenApi,
            _ => WhatsAppProviderType::Evolution,
        };

        match provider_type {
            WhatsAppProviderType::Evolution => {
                match (
                    env::var("EVOLUTION_API_URL").ok(),
                    env::var("EVOLUTION_API_KEY").ok(),
                    env::var("EVOLUTION_INSTANCE").ok(),
                ) {
                    (Some(url), Some(api_key), Some(instance)) => {
                        info!(
                            "Kafka worker: WhatsApp service configured with EvolutionAPI, instance: {}",
                            instance
                        );
                        Some(Arc::new(EvolutionApiClient::new(url, api_key, instance))
                            as Arc<dyn WhatsAppProvider>)
                    }
                    _ => {
                        warn!("Kafka worker: WhatsApp service not configured - EVOLUTION_API_URL, EVOLUTION_API_KEY, EVOLUTION_INSTANCE required");
                        None
                    }
                }
            }
            WhatsAppProviderType::GreenApi => {
                match (
                    env::var("GREEN_API_URL").ok(),
                    env::var("GREEN_API_INSTANCE_ID").ok(),
                    env::var("GREEN_API_TOKEN").ok(),
                ) {
                    (Some(url), Some(instance_id), Some(token)) => {
                        let client = if let Some(media_url) = env::var("GREEN_API_MEDIA_URL").ok() {
                            info!(
                                "Kafka worker: WhatsApp service configured with Green-API, instance: {}, media_url: {}",
                                instance_id, media_url
                            );
                            GreenApiClient::with_media_url(url, media_url, instance_id, token)
                        } else {
                            info!(
                                "Kafka worker: WhatsApp service configured with Green-API, instance: {}",
                                instance_id
                            );
                            GreenApiClient::new(url, instance_id, token)
                        };
                        Some(Arc::new(client) as Arc<dyn WhatsAppProvider>)
                    }
                    _ => {
                        warn!("Kafka worker: WhatsApp service not configured - GREEN_API_URL, GREEN_API_INSTANCE_ID, GREEN_API_TOKEN required");
                        None
                    }
                }
            }
        }
    };

    // Initialize orchestrators
    let document_orchestrator = Arc::new(DocumentOrchestrator::new(
        generator_factory.clone(),
        storage_service.clone(),
        cache_service.clone(),
        email_service.clone(),
        whatsapp_service.clone(),
    ));

    let notification_orchestrator = Arc::new(NotificationOrchestrator::new(
        email_service,
        whatsapp_service,
        cache_service.clone(),
    ));

    // Create handler
    let handler = Arc::new(KafkaHandler::new(
        document_orchestrator,
        notification_orchestrator,
    ));

    info!("Kafka worker started - waiting for messages...");

    // Message processing loop
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Kafka worker received shutdown signal");
                break;
            }
            message = consumer.recv() => {
                match message {
                    Ok(msg) => {
                        let payload = match msg.payload_view::<str>() {
                            None => {
                                warn!("Empty Kafka message received");
                                continue;
                            }
                            Some(Ok(s)) => s,
                            Some(Err(e)) => {
                                error!("Error deserializing Kafka message payload: {:?}", e);
                                continue;
                            }
                        };

                        // Extract correlation ID from headers if present
                        let correlation_id = msg.headers()
                            .and_then(|h| {
                                for i in 0..h.count() {
                                    let header = h.get(i);
                                    if header.key == "correlation_id" {
                                        return header.value.and_then(|v| std::str::from_utf8(v).ok());
                                    }
                                }
                                None
                            })
                            .map(|s| s.to_string());

                        info!(
                            "Kafka message received - topic: {}, partition: {}, offset: {}, correlation_id: {:?}",
                            msg.topic(),
                            msg.partition(),
                            msg.offset(),
                            correlation_id
                        );

                        // Parse and process message - try ERP format first, then simple format
                        let kafka_msg = match serde_json::from_str::<ErpIntegrationEvent>(payload) {
                            Ok(erp_event) => {
                                info!("Parsed ERP integration event: {:?}", erp_event.event_type);
                                // Debug log to check if html_body is being received
                                if let Some(ref notif) = erp_event.notification {
                                    info!("Notification info - channel: {}, recipient: {}, has_html_body: {}",
                                        notif.channel, notif.recipient, notif.html_body.is_some());
                                    if let Some(ref html) = notif.html_body {
                                        info!("HTML body length: {} chars", html.len());
                                    }
                                }
                                match erp_event.into_kafka_message() {
                                    Ok(msg) => msg,
                                    Err(e) => {
                                        error!("Failed to convert ERP event: {}", e);
                                        continue;
                                    }
                                }
                            }
                            Err(_) => {
                                // Fallback to simple KafkaMessage format
                                match serde_json::from_str::<KafkaMessage>(payload) {
                                    Ok(msg) => msg,
                                    Err(e) => {
                                        error!("Failed to parse Kafka message: {} - Payload: {}", e, payload);
                                        continue;
                                    }
                                }
                            }
                        };

                        // Process message
                        let handler_clone = handler.clone();
                        let producer_clone = producer.clone();
                        let response_topic_clone = response_topic.clone();
                        let correlation_id_clone = correlation_id.clone();

                        match handler_clone.handle(kafka_msg).await {
                            Ok(response) => {
                                info!("Kafka message processed successfully");

                                // Send response to response topic
                                if let Ok(response_json) = serde_json::to_string(&response) {
                                    if let Some(ref cid) = correlation_id_clone {
                                        let headers = rdkafka::message::OwnedHeaders::new()
                                            .insert(rdkafka::message::Header {
                                                key: "correlation_id",
                                                value: Some(cid.as_str()),
                                            });

                                        let record_with_headers = FutureRecord::to(&response_topic_clone)
                                            .payload(&response_json)
                                            .key(cid.as_str())
                                            .headers(headers);

                                        match producer_clone.send(record_with_headers, Duration::from_secs(5)).await {
                                            Ok(_) => info!("Response sent to topic: {}", response_topic_clone),
                                            Err((e, _)) => error!("Failed to send response: {}", e),
                                        }
                                    } else {
                                        let record = FutureRecord::<str, str>::to(&response_topic_clone)
                                            .payload(&response_json);

                                        match producer_clone.send(record, Duration::from_secs(5)).await {
                                            Ok(_) => info!("Response sent to topic: {}", response_topic_clone),
                                            Err((e, _)) => error!("Failed to send response: {}", e),
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Error processing Kafka message: {}", e);

                                // Send error response
                                let error_response = serde_json::json!({
                                    "type": "error",
                                    "error": e.to_string(),
                                    "correlation_id": correlation_id
                                });

                                if let Ok(error_json) = serde_json::to_string(&error_response) {
                                    let key = correlation_id.clone().unwrap_or_default();
                                    let record = FutureRecord::to(&response_topic_clone)
                                        .payload(&error_json)
                                        .key(&key);

                                    let _ = producer_clone.send(record, Duration::from_secs(5)).await;
                                }
                            }
                        }

                        // Commit offset
                        if let Err(e) = consumer.commit_message(&msg, CommitMode::Async) {
                            error!("Failed to commit Kafka offset: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Kafka consumer error: {}", e);
                    }
                }
            }
        }
    }

    info!("Kafka worker stopped");
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
        smtp_from_name: env::var("SMTP_FROM_NAME").unwrap_or_else(|_| "PDF Service".to_string()),
        // WhatsApp provider selection
        whatsapp_provider: match env::var("WHATSAPP_PROVIDER")
            .unwrap_or_else(|_| "evolution".to_string())
            .to_lowercase()
            .as_str()
        {
            "greenapi" | "green-api" | "green_api" => WhatsAppProviderType::GreenApi,
            _ => WhatsAppProviderType::Evolution,
        },
        // WhatsApp EvolutionAPI configuration
        evolution_api_url: env::var("EVOLUTION_API_URL").ok(),
        evolution_api_key: env::var("EVOLUTION_API_KEY").ok(),
        evolution_instance: env::var("EVOLUTION_INSTANCE").ok(),
        // WhatsApp Green-API configuration
        green_api_url: env::var("GREEN_API_URL").ok(),
        green_api_media_url: env::var("GREEN_API_MEDIA_URL").ok(),
        green_api_instance_id: env::var("GREEN_API_INSTANCE_ID").ok(),
        green_api_token: env::var("GREEN_API_TOKEN").ok(),
    };

    Ok(config)
}
