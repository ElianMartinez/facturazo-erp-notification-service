//! Kafka Worker - Processes messages from Kafka topics
//!
//! This worker runs independently and processes document generation
//! and notification requests from Kafka.

use anyhow::Result;
use rdkafka::client::ClientContext;
use rdkafka::config::{ClientConfig, RDKafkaLogLevel};
use rdkafka::consumer::stream_consumer::StreamConsumer;
use rdkafka::consumer::{CommitMode, Consumer, ConsumerContext, Rebalance};
use rdkafka::error::KafkaResult;
use rdkafka::message::{Headers, Message};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::TopicPartitionList;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::broadcast;
use tracing::{error, info, warn, Instrument};

use pdf_services::application::orchestrators::{DocumentOrchestrator, NotificationOrchestrator};
use pdf_services::infrastructure::cache::CacheService;
use pdf_services::infrastructure::generators::GeneratorFactory;
use pdf_services::infrastructure::notifications::{
    EmailService, EvolutionApiClient, GreenApiClient, WhatsAppProvider,
};
use pdf_services::infrastructure::storage::StorageService;
use pdf_services::kafka::erp_messages::ErpIntegrationEvent;
use pdf_services::kafka::handlers::{KafkaHandler, KafkaMessage};
use std::path::PathBuf;

/// Custom consumer context for logging rebalance events
struct CustomContext;

impl ClientContext for CustomContext {}

impl ConsumerContext for CustomContext {
    fn pre_rebalance(&self, rebalance: &Rebalance) {
        info!("Pre rebalance: {:?}", rebalance);
    }

    fn post_rebalance(&self, rebalance: &Rebalance) {
        info!("Post rebalance: {:?}", rebalance);
    }

    fn commit_callback(&self, result: KafkaResult<()>, _offsets: &TopicPartitionList) {
        match result {
            Ok(_) => {}
            Err(e) => warn!("Commit callback error: {}", e),
        }
    }
}

type LoggingConsumer = StreamConsumer<CustomContext>;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting Kafka Worker");

    // Load configuration
    dotenv::dotenv().ok();

    let brokers = env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string());
    let group_id =
        env::var("KAFKA_GROUP_ID").unwrap_or_else(|_| "pdf-services-consumer".to_string());

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

    info!("Connecting to Kafka brokers: {}", brokers);
    info!("Consumer group: {}", group_id);
    info!("Subscribing to topics: {:?}", topics);

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

    // Initialize services
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

            info!("Email service configured with host: {}", host);
            Some(Arc::new(EmailService::new(
                host, port, user, pass, from_email, from_name, true, // use TLS
            )))
        }
        _ => {
            warn!("Email service not configured - SMTP_HOST, SMTP_USER, SMTP_PASS required");
            None
        }
    };

    // Initialize WhatsApp service - supports both Evolution API and Green-API
    let whatsapp_service: Option<Arc<dyn WhatsAppProvider>> = {
        let provider_type = env::var("WHATSAPP_PROVIDER")
            .unwrap_or_else(|_| "evolution".to_string())
            .to_lowercase();

        match provider_type.as_str() {
            "greenapi" | "green-api" | "green_api" => {
                match (
                    env::var("GREEN_API_URL").ok(),
                    env::var("GREEN_API_INSTANCE_ID").ok(),
                    env::var("GREEN_API_TOKEN").ok(),
                ) {
                    (Some(url), Some(instance_id), Some(token)) => {
                        // Use media URL if available for direct file uploads
                        let client = if let Ok(media_url) = env::var("GREEN_API_MEDIA_URL") {
                            info!(
                                "WhatsApp service configured with Green-API at: {} (media: {})",
                                url, media_url
                            );
                            GreenApiClient::with_media_url(url, media_url, instance_id, token)
                        } else {
                            info!("WhatsApp service configured with Green-API at: {}", url);
                            GreenApiClient::new(url, instance_id, token)
                        };
                        Some(Arc::new(client) as Arc<dyn WhatsAppProvider>)
                    }
                    _ => {
                        warn!("Green-API selected but not configured - GREEN_API_URL, GREEN_API_INSTANCE_ID, GREEN_API_TOKEN required");
                        None
                    }
                }
            }
            _ => {
                match (
                    env::var("EVOLUTION_API_URL").ok(),
                    env::var("EVOLUTION_API_KEY").ok(),
                    env::var("EVOLUTION_INSTANCE").ok(),
                ) {
                    (Some(url), Some(api_key), Some(instance)) => {
                        info!("WhatsApp service configured with EvolutionAPI at: {}", url);
                        Some(Arc::new(EvolutionApiClient::new(url, api_key, instance))
                            as Arc<dyn WhatsAppProvider>)
                    }
                    _ => {
                        warn!("WhatsApp service not configured - EVOLUTION_API_URL, EVOLUTION_API_KEY, EVOLUTION_INSTANCE required");
                        None
                    }
                }
            }
        }
    };

    // Initialize template resolver (if configured)
    let template_resolver = if let (Ok(api_url), Ok(token)) = (
        env::var("PDF_CORE_API_URL"),
        env::var("PDF_CORE_SERVICE_TOKEN"),
    ) {
        use pdf_services::infrastructure::template_resolver::{
            AssetManager, TemplateApiClient, TemplateResolver,
        };
        match TemplateApiClient::new(api_url.clone(), token) {
            Ok(api_client) => {
                let s3_client = Arc::new(
                    pdf_services::storage::s3::S3Client::new()
                        .await
                        .expect("S3 client required for template resolver"),
                );
                let asset_dir = env::var("PDF_TEMPLATE_ASSET_DIR")
                    .unwrap_or_else(|_| "./template-assets".to_string());
                let bucket =
                    env::var("S3_BUCKET_TEMPLATES").unwrap_or_else(|_| "templates".to_string());
                let ttl: u64 = env::var("PDF_TEMPLATE_CACHE_TTL_SECS")
                    .unwrap_or_else(|_| "300".to_string())
                    .parse()
                    .unwrap_or(300);
                let resolver = Arc::new(TemplateResolver::new(
                    Arc::new(api_client),
                    Arc::new(AssetManager::new(
                        s3_client,
                        PathBuf::from(asset_dir),
                        bucket,
                    )),
                    ttl,
                ));
                info!("Template resolver configured with backend: {}", api_url);
                Some(resolver)
            }
            Err(e) => {
                warn!("Failed to init template API client: {}", e);
                None
            }
        }
    } else {
        None
    };

    let typst_generator =
        Arc::new(pdf_services::infrastructure::generators::TypstGenerator::new(work_dir.clone()));

    // Initialize orchestrators
    let document_orchestrator = Arc::new(DocumentOrchestrator::new(
        generator_factory.clone(),
        storage_service.clone(),
        cache_service.clone(),
        email_service.clone(),
        whatsapp_service.clone(),
        template_resolver.clone(),
        typst_generator,
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
        template_resolver,
    ));

    // Shutdown signal
    let (shutdown_tx, mut shutdown_rx) = broadcast::channel::<()>(1);

    // Spawn signal handler
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        let _ = signal::ctrl_c().await;
        info!("Received shutdown signal");
        let _ = shutdown_tx_clone.send(());
    });

    info!("Kafka Worker started - waiting for messages...");

    // Message processing loop
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Shutting down Kafka Worker...");
                break;
            }
            message = consumer.recv() => {
                match message {
                    Ok(msg) => {
                        let payload = match msg.payload_view::<str>() {
                            None => {
                                warn!("Empty message received");
                                continue;
                            }
                            Some(Ok(s)) => s,
                            Some(Err(e)) => {
                                error!("Error deserializing message payload: {:?}", e);
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
                            "Received message from topic: {}, partition: {}, offset: {}, correlation_id: {:?}",
                            msg.topic(),
                            msg.partition(),
                            msg.offset(),
                            correlation_id
                        );

                        // Parse and process message - try ERP format first, then simple format
                        let kafka_msg = match serde_json::from_str::<ErpIntegrationEvent>(payload) {
                            Ok(erp_event) => {
                                info!("Parsed ERP integration event: {:?}", erp_event.event_type);
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

                        // Process message within a span that includes correlation_id
                        // This ensures ALL logs within processing have the correlation_id
                        let handler_clone = handler.clone();
                        let producer_clone = producer.clone();
                        let response_topic_clone = response_topic.clone();
                        let correlation_id_for_span = correlation_id.clone().unwrap_or_else(|| "none".to_string());
                        let correlation_id_clone = correlation_id.clone();

                        // Create a span with the correlation_id - all nested logs will include it
                        let processing_span = tracing::info_span!(
                            "process_message",
                            correlation_id = %correlation_id_for_span,
                            topic = %msg.topic(),
                            partition = %msg.partition(),
                            offset = %msg.offset()
                        );

                        // Process message within the span
                        async {
                            info!("Starting message processing");

                            match handler_clone.handle(kafka_msg).await {
                                Ok(response) => {
                                    info!("Message processed successfully");

                                    // Send response to response topic
                                    if let Ok(response_json) = serde_json::to_string(&response) {
                                        // Add correlation ID header if present and send
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
                                    error!("Error processing message: {}", e);

                                    // Send error response
                                    let error_response = serde_json::json!({
                                        "type": "error",
                                        "error": e.to_string(),
                                        "correlation_id": correlation_id_clone
                                    });

                                    if let Ok(error_json) = serde_json::to_string(&error_response) {
                                        let key = correlation_id_clone.clone().unwrap_or_default();
                                        let record = FutureRecord::to(&response_topic_clone)
                                            .payload(&error_json)
                                            .key(&key);

                                        let _ = producer_clone.send(record, Duration::from_secs(5)).await;
                                    }
                                }
                            }
                        }
                        .instrument(processing_span)
                        .await;

                        // Commit offset
                        if let Err(e) = consumer.commit_message(&msg, CommitMode::Async) {
                            error!("Failed to commit offset: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Kafka error: {}", e);
                    }
                }
            }
        }
    }

    info!("Kafka Worker stopped");
    Ok(())
}
