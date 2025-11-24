//! Local testing CLI
//!
//! Run: cargo run --bin local-test
//!
//! Environment variables:
//!   KAFKA_BROKERS - Kafka broker address (default: localhost:9092)
//!   MINIO_ENDPOINT - MinIO endpoint (default: http://localhost:9000)
//!   API_URL - API server URL (default: http://localhost:8080)

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

// Direct imports from rdkafka for admin
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;

fn get_kafka_brokers() -> String {
    std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string())
}

fn get_minio_endpoint() -> String {
    std::env::var("MINIO_ENDPOINT").unwrap_or_else(|_| "http://localhost:9000".to_string())
}

fn get_api_url() -> String {
    std::env::var("API_URL").unwrap_or_else(|_| "http://localhost:8080".to_string())
}

fn get_whatsapp_api_url() -> String {
    std::env::var("PDF_NOTIFICATION_WHATSAPP_API_URL")
        .unwrap_or_else(|_| "http://5.161.120.166:8080".to_string())
}

fn get_whatsapp_api_key() -> String {
    std::env::var("PDF_NOTIFICATION_WHATSAPP_API_KEY")
        .unwrap_or_else(|_| "mySuperSecretKey123".to_string())
}

fn get_whatsapp_instance() -> String {
    std::env::var("PDF_NOTIFICATION_WHATSAPP_INSTANCE_NAME")
        .unwrap_or_else(|_| "FACTURAZO-ERP-DEV".to_string())
}

#[derive(Parser)]
#[command(name = "local-test")]
#[command(about = "Local testing CLI for PDF Services")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check all services health
    Health,
    /// Kafka operations
    Kafka {
        #[command(subcommand)]
        action: KafkaAction,
    },
    /// Test document generation
    Generate {
        /// Document type (invoice, report, quotation)
        #[arg(short, long, default_value = "invoice")]
        doc_type: String,
    },
    /// Test HTTP API
    Http {
        /// API endpoint to test
        #[arg(short, long, default_value = "http://localhost:8080")]
        url: String,
    },
    /// WhatsApp operations via EvolutionAPI
    Whatsapp {
        #[command(subcommand)]
        action: WhatsappAction,
    },
}

#[derive(Subcommand)]
enum KafkaAction {
    /// Check Kafka connection
    Check,
    /// List all topics
    ListTopics,
    /// Create required topics
    CreateTopics,
    /// Get cluster info
    ClusterInfo,
    /// Send test message
    Send {
        #[arg(short, long)]
        topic: String,
        #[arg(short, long)]
        message: String,
    },
}

#[derive(Subcommand)]
enum WhatsappAction {
    /// Check EvolutionAPI connection status
    Status,
    /// Send a test text message
    SendText {
        /// Phone number (Dominican format: 809-XXX-XXXX)
        #[arg(short, long)]
        phone: String,
        /// Message to send
        #[arg(short, long, default_value = "Prueba desde PDF Services")]
        message: String,
    },
    /// Send a test PDF document
    SendPdf {
        /// Phone number (Dominican format: 809-XXX-XXXX)
        #[arg(short, long)]
        phone: String,
        /// Caption for the document
        #[arg(short, long, default_value = "Documento de prueba")]
        caption: String,
    },
    /// Show current configuration
    Config,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Health => check_health().await?,
        Commands::Kafka { action } => handle_kafka(action).await?,
        Commands::Generate { doc_type } => test_generation(&doc_type).await?,
        Commands::Http { url } => test_http(&url).await?,
        Commands::Whatsapp { action } => handle_whatsapp(action).await?,
    }

    Ok(())
}

async fn check_health() -> Result<()> {
    println!("\n🏥 Health Check\n{}", "=".repeat(50));

    // Check Typst
    print!("📄 Typst: ");
    match tokio::process::Command::new("typst")
        .arg("--version")
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            println!("✅ OK ({})", version.trim());
        }
        _ => {
            println!("❌ Not installed");
        }
    }

    // Check Kafka
    let kafka_brokers = get_kafka_brokers();
    print!("📨 Kafka ({}): ", kafka_brokers);
    match check_kafka_connection(&kafka_brokers).await {
        Ok(true) => println!("✅ OK"),
        Ok(false) => println!("⚠️ Unreachable"),
        Err(e) => println!("❌ {}", e),
    }

    // Check MinIO/S3
    let minio_endpoint = get_minio_endpoint();
    print!("💾 MinIO ({}): ", minio_endpoint);
    match reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?
        .get(format!("{}/minio/health/live", minio_endpoint))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => println!("✅ OK"),
        _ => println!("⚠️ Not running (optional)"),
    }

    println!("\n{}", "=".repeat(50));
    Ok(())
}

async fn check_kafka_connection(brokers: &str) -> Result<bool> {
    let admin: AdminClient<DefaultClientContext> = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .create()?;

    match admin.inner().fetch_metadata(None, Duration::from_secs(5)) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

async fn handle_kafka(action: KafkaAction) -> Result<()> {
    let brokers = get_kafka_brokers();

    match action {
        KafkaAction::Check => {
            println!("\n🔍 Checking Kafka connection to {}...", brokers);

            match check_kafka_connection(&brokers).await? {
                true => {
                    println!("✅ Kafka is reachable!");

                    let admin: AdminClient<DefaultClientContext> = ClientConfig::new()
                        .set("bootstrap.servers", brokers)
                        .create()?;

                    let metadata = admin.inner().fetch_metadata(None, Duration::from_secs(5))?;

                    println!("\n📊 Cluster Info:");
                    println!("   Brokers: {}", metadata.brokers().len());
                    for broker in metadata.brokers() {
                        println!(
                            "   - {}:{} (id: {})",
                            broker.host(),
                            broker.port(),
                            broker.id()
                        );
                    }
                    let topic_count = metadata
                        .topics()
                        .iter()
                        .filter(|t| !t.name().starts_with("__"))
                        .count();
                    println!("   Topics: {}", topic_count);
                }
                false => {
                    println!("❌ Kafka is not reachable");
                    println!("\n💡 Make sure Kafka is running:");
                    println!("   docker-compose up -d kafka");
                }
            }
        }

        KafkaAction::ListTopics => {
            println!("\n📋 Listing Kafka topics...");

            let admin: AdminClient<DefaultClientContext> = ClientConfig::new()
                .set("bootstrap.servers", brokers)
                .create()?;

            let metadata = admin
                .inner()
                .fetch_metadata(None, Duration::from_secs(10))?;

            let topics: Vec<_> = metadata
                .topics()
                .iter()
                .filter(|t| !t.name().starts_with("__"))
                .collect();

            if topics.is_empty() {
                println!("   (no topics found)");
            } else {
                for topic in topics {
                    println!(
                        "   📁 {} ({} partitions)",
                        topic.name(),
                        topic.partitions().len()
                    );
                }
            }
        }

        KafkaAction::CreateTopics => {
            println!("\n🔧 Creating required Kafka topics...\n");

            let admin: AdminClient<DefaultClientContext> = ClientConfig::new()
                .set("bootstrap.servers", brokers)
                .create()?;

            let topics = vec![
                ("document-generate-request", 3),
                ("document-batch-request", 3),
                ("notification-dispatch-request", 3),
                ("document-events", 6),
                ("document-dlq", 1),
            ];

            for (name, partitions) in &topics {
                let new_topic = NewTopic::new(name, *partitions, TopicReplication::Fixed(1));
                let opts = AdminOptions::new().operation_timeout(Some(Duration::from_secs(10)));

                match admin.create_topics(&[new_topic], &opts).await {
                    Ok(results) => {
                        for result in results {
                            match result {
                                Ok(_) => println!("   ✅ Created: {}", name),
                                Err((_, err)) => {
                                    if err.to_string().contains("TopicAlreadyExists") {
                                        println!("   ✓ Exists: {}", name);
                                    } else {
                                        println!("   ❌ Failed {}: {}", name, err);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => println!("   ❌ Error: {}", e),
                }
            }

            println!("\n📋 Current topics:");
            let metadata = admin.inner().fetch_metadata(None, Duration::from_secs(5))?;
            for topic in metadata.topics() {
                if !topic.name().starts_with("__") {
                    println!("   ✓ {}", topic.name());
                }
            }
        }

        KafkaAction::ClusterInfo => {
            println!("\n📊 Kafka Cluster Info\n");

            let admin: AdminClient<DefaultClientContext> = ClientConfig::new()
                .set("bootstrap.servers", brokers)
                .create()?;

            let metadata = admin
                .inner()
                .fetch_metadata(None, Duration::from_secs(10))?;

            println!("Brokers ({}):", metadata.brokers().len());
            for broker in metadata.brokers() {
                println!(
                    "  - ID: {}, Host: {}:{}",
                    broker.id(),
                    broker.host(),
                    broker.port()
                );
            }

            let topics: Vec<_> = metadata
                .topics()
                .iter()
                .filter(|t| !t.name().starts_with("__"))
                .collect();
            println!("\nTopics ({}):", topics.len());
            for topic in topics {
                println!(
                    "  - {} ({} partitions)",
                    topic.name(),
                    topic.partitions().len()
                );
            }
        }

        KafkaAction::Send { topic, message } => {
            println!("\n📤 Sending message to topic '{}'...", topic);

            use rdkafka::producer::{FutureProducer, FutureRecord};

            let producer: FutureProducer = ClientConfig::new()
                .set("bootstrap.servers", brokers)
                .set("message.timeout.ms", "5000")
                .create()?;

            let record = FutureRecord::to(&topic).payload(&message).key("test-key");

            match producer.send(record, Duration::from_secs(5)).await {
                Ok((partition, offset)) => {
                    println!("✅ Message sent!");
                    println!("   Partition: {}", partition);
                    println!("   Offset: {}", offset);
                }
                Err((err, _)) => {
                    println!("❌ Failed to send: {}", err);
                }
            }
        }
    }

    Ok(())
}

async fn test_generation(doc_type: &str) -> Result<()> {
    use pdf_services::infrastructure::generators::typst_generator::TypstGenerator;

    println!("\n📄 Testing {} generation...\n", doc_type);

    let work_dir = PathBuf::from("./temp");
    std::fs::create_dir_all(&work_dir)?;
    let generator = TypstGenerator::new(work_dir);

    // Simple test content
    let content = format!(
        r#"
#set page(paper: "a4", margin: 2cm)
#set text(font: "Helvetica", size: 11pt)

= Test Document

Generated by PDF Services

== Details
- Type: {}
- Date: #datetime.today().display()
- Status: Success

#v(1cm)

This is a test document generated locally.
    "#,
        doc_type
    );

    let output_path = format!("test_output_{}.pdf", doc_type);

    match generator.generate_pdf(&content).await {
        Ok(bytes) => {
            std::fs::write(&output_path, &bytes)?;
            println!("✅ Generated successfully!");
            println!("   Output: {}", output_path);
            println!("   Size: {} bytes", bytes.len());
        }
        Err(e) => {
            println!("❌ Generation failed: {}", e);
        }
    }

    Ok(())
}

async fn test_http(_base_url: &str) -> Result<()> {
    let base_url = get_api_url();
    println!("\n🌐 Testing HTTP API at {}\n", base_url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    // Test health endpoint
    print!("GET /health: ");
    match client.get(format!("{}/health", base_url)).send().await {
        Ok(resp) => {
            println!("✅ {}", resp.status());
        }
        Err(e) => {
            println!("❌ {}", e);
            println!("\n💡 Make sure the API server is running:");
            println!("   cargo run --bin pdf-services");
            return Ok(());
        }
    }

    // Test API info
    print!("GET /api/v1/info: ");
    match client.get(format!("{}/api/v1/info", base_url)).send().await {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                let body = resp.text().await?;
                println!("✅ {}", status);
                println!("   Response: {}", body);
            } else {
                println!("⚠️ {}", status);
            }
        }
        Err(e) => println!("❌ {}", e),
    }

    Ok(())
}

async fn handle_whatsapp(action: WhatsappAction) -> Result<()> {
    use pdf_services::infrastructure::notifications::evolution_api::{
        is_valid_dominican_phone, EvolutionAPIClient,
    };

    let api_url = get_whatsapp_api_url();
    let api_key = get_whatsapp_api_key();
    let instance = get_whatsapp_instance();

    match action {
        WhatsappAction::Config => {
            println!(
                "\n📱 WhatsApp (EvolutionAPI) Configuration\n{}",
                "=".repeat(50)
            );
            println!("   API URL:  {}", api_url);
            println!("   API Key:  {}...", &api_key[..api_key.len().min(10)]);
            println!("   Instance: {}", instance);
            println!("\n💡 Set environment variables to override:");
            println!("   PDF_NOTIFICATION_WHATSAPP_API_URL");
            println!("   PDF_NOTIFICATION_WHATSAPP_API_KEY");
            println!("   PDF_NOTIFICATION_WHATSAPP_INSTANCE_NAME");
        }

        WhatsappAction::Status => {
            println!("\n📱 Checking EvolutionAPI Status...\n");
            println!("   URL: {}", api_url);
            println!("   Instance: {}", instance);

            let client = EvolutionAPIClient::new(api_url, api_key, instance);

            print!("\n   Connection: ");
            match client.is_connected().await {
                Ok(true) => println!("✅ Connected"),
                Ok(false) => println!("⚠️ Not connected"),
                Err(e) => println!("❌ Error: {}", e),
            }
        }

        WhatsappAction::SendText { phone, message } => {
            println!("\n📱 Sending WhatsApp Text Message\n{}", "=".repeat(50));

            // Validate phone number
            if !is_valid_dominican_phone(&phone) {
                println!(
                    "⚠️  Warning: '{}' may not be a valid Dominican phone number",
                    phone
                );
                println!("   Expected format: 809-XXX-XXXX, 829-XXX-XXXX, or 849-XXX-XXXX");
            }

            println!("   To: {}", phone);
            println!("   Message: {}", message);
            println!("   Instance: {}", instance);

            let client = EvolutionAPIClient::new(api_url, api_key, instance);

            print!("\n   Sending... ");
            match client.send_simple_text(&phone, &message).await {
                Ok(msg_id) => {
                    println!("✅ Sent!");
                    println!("   Message ID: {}", msg_id);
                }
                Err(e) => {
                    println!("❌ Failed");
                    println!("   Error: {}", e);
                }
            }
        }

        WhatsappAction::SendPdf { phone, caption } => {
            println!("\n📱 Sending WhatsApp PDF Document\n{}", "=".repeat(50));

            // Validate phone number
            if !is_valid_dominican_phone(&phone) {
                println!(
                    "⚠️  Warning: '{}' may not be a valid Dominican phone number",
                    phone
                );
            }

            // Generate a simple test PDF first
            println!("   Generating test PDF...");

            let work_dir = PathBuf::from("./temp");
            std::fs::create_dir_all(&work_dir)?;

            use pdf_services::infrastructure::generators::typst_generator::TypstGenerator;
            let generator = TypstGenerator::new(work_dir);

            let content = r#"
#set page(paper: "a4", margin: 2cm)
#set text(font: "Helvetica", size: 11pt)

= PDF Services Test Document

Generated by *PDF Services* via WhatsApp.

== Details
- Date: #datetime.today().display()
- Type: Test Document
- Status: Success

#v(1cm)

This is a test document sent via WhatsApp using EvolutionAPI.
            "#;

            let pdf_bytes = generator.generate_pdf(content).await?;
            println!("   PDF generated: {} bytes", pdf_bytes.len());

            println!("   To: {}", phone);
            println!("   Caption: {}", caption);

            let client = EvolutionAPIClient::new(api_url, api_key, instance);

            print!("\n   Sending PDF... ");
            match client
                .send_pdf(phone, pdf_bytes, "test_document.pdf".to_string(), caption)
                .await
            {
                Ok(msg_id) => {
                    println!("✅ Sent!");
                    println!("   Message ID: {}", msg_id);
                }
                Err(e) => {
                    println!("❌ Failed");
                    println!("   Error: {}", e);
                }
            }
        }
    }

    Ok(())
}
