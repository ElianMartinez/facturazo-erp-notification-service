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

// ERP Report types
use pdf_services::templates::{
    AgingBucket, ColumnAlign, ColumnDefinition, ColumnType, DataStructureType, DateRange,
    DeliveryMethod, DeliveryOptions, EmailDelivery, ErpReportPayload, GroupingConfig, OutputFormat,
    OutputOptions, PageMargins, PageOrientation, PageSize, ReportDataSet, ReportGroup, ReportInfo,
    ReportMetadata, TypstTemplate, WhatsAppDelivery,
};
use std::collections::HashMap;

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

fn get_smtp_host() -> String {
    std::env::var("SMTP_HOST").unwrap_or_else(|_| "smtp.gmail.com".to_string())
}

fn get_smtp_port() -> u16 {
    std::env::var("SMTP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(587)
}

fn get_smtp_username() -> String {
    std::env::var("SMTP_USER").unwrap_or_else(|_| "".to_string())
}

fn get_smtp_password() -> String {
    std::env::var("SMTP_PASS").unwrap_or_else(|_| "".to_string())
}

fn get_smtp_from_email() -> String {
    std::env::var("SMTP_FROM_EMAIL").unwrap_or_else(|_| "noreply@facturazo.com".to_string())
}

fn get_smtp_from_name() -> String {
    std::env::var("SMTP_FROM_NAME").unwrap_or_else(|_| "Facturazo ERP".to_string())
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
    /// Test ERP report generation with sample data
    ErpReport {
        /// Report variant (ncf_summary, ar_balance, etc.)
        #[arg(short, long, default_value = "ncf_summary")]
        variant: String,
        /// Page orientation (portrait, landscape)
        #[arg(short, long, default_value = "landscape")]
        orientation: String,
        /// Number of test records to generate
        #[arg(short, long, default_value = "50")]
        records: i32,
        /// Output format (pdf, xlsx, csv)
        #[arg(short, long, default_value = "pdf")]
        format: String,
        /// Load payload from JSON file (optional)
        #[arg(short, long)]
        json: Option<String>,
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
    /// Send ERP report via email
    SendReportEmail {
        /// Report variant (ncf_summary, ar_balance, etc.)
        #[arg(short, long, default_value = "ncf_summary")]
        variant: String,
        /// Output format (pdf, xlsx, csv)
        #[arg(short, long, default_value = "pdf")]
        format: String,
        /// Number of test records to generate
        #[arg(short, long, default_value = "30")]
        records: i32,
        /// Email recipient
        #[arg(short, long)]
        email: String,
        /// Custom subject (optional)
        #[arg(short, long)]
        subject: Option<String>,
    },
    /// Send ERP report via WhatsApp
    SendReportWhatsapp {
        /// Report variant (ncf_summary, ar_balance, etc.)
        #[arg(short, long, default_value = "ncf_summary")]
        variant: String,
        /// Output format (pdf, xlsx, csv)
        #[arg(short, long, default_value = "pdf")]
        format: String,
        /// Number of test records to generate
        #[arg(short, long, default_value = "30")]
        records: i32,
        /// WhatsApp phone number
        #[arg(short, long)]
        phone: String,
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
        Commands::ErpReport {
            variant,
            orientation,
            records,
            format,
            json,
        } => test_erp_report(&variant, &orientation, records, &format, json).await?,
        Commands::Http { url } => test_http(&url).await?,
        Commands::Whatsapp { action } => handle_whatsapp(action).await?,
        Commands::SendReportEmail {
            variant,
            format,
            records,
            email,
            subject,
        } => send_report_via_email(&variant, &format, records, &email, subject).await?,
        Commands::SendReportWhatsapp {
            variant,
            format,
            records,
            phone,
        } => send_report_via_whatsapp(&variant, &format, records, &phone).await?,
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

async fn test_erp_report(
    variant: &str,
    orientation: &str,
    records: i32,
    format: &str,
    json_path: Option<String>,
) -> Result<()> {
    use pdf_services::infrastructure::generators::typst_generator::TypstGenerator;
    use pdf_services::infrastructure::generators::ErpReportCsvGenerator;
    use pdf_services::infrastructure::generators::ErpReportExcelGenerator;
    use pdf_services::templates::templates::ErpReportTemplate;

    println!("\n📊 Testing ERP Report Generation\n{}", "=".repeat(50));

    let work_dir = PathBuf::from("./temp");
    std::fs::create_dir_all(&work_dir)?;

    // Build payload - from JSON file or generate sample
    let (payload, output_name) = if let Some(ref path) = json_path {
        println!("   Loading from JSON: {}", path);
        let json_content = std::fs::read_to_string(path)?;
        let payload: ErpReportPayload = serde_json::from_str(&json_content)?;
        let name = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("custom");
        (payload, name.to_string())
    } else {
        println!("   Variant: {}", variant);
        println!("   Orientation: {}", orientation);
        println!("   Records: {}", records);
        let payload = match variant {
            "ncf_summary" => build_ncf_summary_payload(records, orientation),
            "ar_balance" => build_ar_balance_payload(records, orientation),
            _ => build_generic_payload(variant, records, orientation),
        };
        (payload, format!("erp_{}", variant))
    };

    println!("   Format: {}", format.to_uppercase());

    // Check processing mode
    let mode = payload.get_processing_mode();
    println!("   Processing Mode: {:?}", mode);
    println!("   Est. Time: {}s", payload.estimate_processing_time_secs());

    // Generate based on format
    match format.to_lowercase().as_str() {
        "pdf" => {
            // Generate PDF using Typst template
            let template = ErpReportTemplate::new();
            let json_value = serde_json::to_value(&payload)?;

            println!("\n   Validating payload...");
            template.validate(&json_value)?;

            println!("   Generating Typst content...");
            let typst_content = template.generate(&json_value)?;

            // Save Typst content for debugging
            let typst_path = format!("{}.typ", output_name);
            std::fs::write(&typst_path, &typst_content)?;
            println!("   Typst saved: {}", typst_path);

            // Generate PDF
            println!("   Generating PDF...");
            let generator = TypstGenerator::new(work_dir);

            match generator.generate_pdf(&typst_content).await {
                Ok(bytes) => {
                    let output_path = format!("{}.pdf", output_name);
                    std::fs::write(&output_path, &bytes)?;
                    println!("\n✅ PDF generated successfully!");
                    println!("   Output: {}", output_path);
                    println!(
                        "   Size: {} bytes ({:.2} KB)",
                        bytes.len(),
                        bytes.len() as f64 / 1024.0
                    );
                }
                Err(e) => {
                    println!("\n❌ PDF generation failed: {}", e);
                    println!("   Check {} for Typst syntax errors", typst_path);
                }
            }
        }
        "xlsx" | "excel" => {
            // Generate Excel
            println!("\n   Generating Excel...");
            let generator = ErpReportExcelGenerator::new();

            match generator.generate(&payload).await {
                Ok(bytes) => {
                    let output_path = format!("{}.xlsx", output_name);
                    std::fs::write(&output_path, &bytes)?;
                    println!("\n✅ Excel generated successfully!");
                    println!("   Output: {}", output_path);
                    println!(
                        "   Size: {} bytes ({:.2} KB)",
                        bytes.len(),
                        bytes.len() as f64 / 1024.0
                    );
                }
                Err(e) => {
                    println!("\n❌ Excel generation failed: {}", e);
                }
            }
        }
        "csv" => {
            // Generate CSV
            println!("\n   Generating CSV...");
            let generator = ErpReportCsvGenerator::new();

            match generator.generate(&payload).await {
                Ok(bytes) => {
                    let output_path = format!("{}.csv", output_name);
                    std::fs::write(&output_path, &bytes)?;
                    println!("\n✅ CSV generated successfully!");
                    println!("   Output: {}", output_path);
                    println!(
                        "   Size: {} bytes ({:.2} KB)",
                        bytes.len(),
                        bytes.len() as f64 / 1024.0
                    );

                    // Preview first few lines
                    let content = String::from_utf8_lossy(&bytes);
                    let preview: Vec<&str> = content.lines().take(5).collect();
                    println!("\n   Preview (first 5 lines):");
                    for line in preview {
                        println!("   {}", line);
                    }
                }
                Err(e) => {
                    println!("\n❌ CSV generation failed: {}", e);
                }
            }
        }
        _ => {
            println!("\n❌ Unknown format: {}. Use pdf, xlsx, or csv.", format);
        }
    }

    Ok(())
}

fn build_ncf_summary_payload(records: i32, orientation: &str) -> ErpReportPayload {
    // Generate sample rows
    let mut rows: Vec<HashMap<String, serde_json::Value>> = Vec::new();
    let ncf_types = vec![
        ("E31", "Factura de Credito Fiscal"),
        ("E32", "Factura de Consumo"),
        ("E33", "Nota de Debito"),
        ("E34", "Nota de Credito"),
    ];

    let mut grand_total = HashMap::new();
    let mut total_amount = 0.0f64;
    let mut total_itbis = 0.0f64;
    let mut total_records = 0i32;

    for i in 0..records {
        let (ncf_type, ncf_name) = &ncf_types[i as usize % ncf_types.len()];
        let amount = 1000.0 + (i as f64 * 123.45);
        let itbis = amount * 0.18;

        total_amount += amount;
        total_itbis += itbis;
        total_records += 1;

        let mut row = HashMap::new();
        row.insert(
            "invoiceDate".to_string(),
            serde_json::json!(format!("2025-12-{:02}", (i % 28) + 1)),
        );
        row.insert(
            "encf".to_string(),
            serde_json::json!(format!("{}00000{:04}", ncf_type, i + 1)),
        );
        row.insert(
            "customerRnc".to_string(),
            serde_json::json!(format!("1010{:05}", i)),
        );
        row.insert(
            "customerName".to_string(),
            serde_json::json!(format!("Cliente {} S.R.L.", i + 1)),
        );
        row.insert(
            "tipoVenta".to_string(),
            serde_json::json!(if i % 3 == 0 { "CREDITO" } else { "CONTADO" }),
        );
        row.insert("total".to_string(), serde_json::json!(amount + itbis));
        row.insert("totalTax".to_string(), serde_json::json!(itbis));
        row.insert("itbis18".to_string(), serde_json::json!(itbis));
        row.insert("itbis16".to_string(), serde_json::json!(0.0));
        row.insert("exemptAmount".to_string(), serde_json::json!(0.0));
        row.insert("ncfType".to_string(), serde_json::json!(ncf_type));
        row.insert("ncfTypeName".to_string(), serde_json::json!(ncf_name));
        row.insert(
            "section".to_string(),
            serde_json::json!(if *ncf_type == "E34" {
                "DEVOLUCIONES"
            } else {
                "VENTAS"
            }),
        );
        rows.push(row);
    }

    grand_total.insert("recordCount".to_string(), serde_json::json!(total_records));
    grand_total.insert(
        "total".to_string(),
        serde_json::json!(total_amount + total_itbis),
    );
    grand_total.insert("subtotal".to_string(), serde_json::json!(total_amount));
    grand_total.insert("totalItbis".to_string(), serde_json::json!(total_itbis));
    grand_total.insert("itbis18".to_string(), serde_json::json!(total_itbis));

    // Group rows by section
    let ventas_rows: Vec<HashMap<String, serde_json::Value>> = rows
        .iter()
        .filter(|r| r.get("section").and_then(|v| v.as_str()) != Some("DEVOLUCIONES"))
        .cloned()
        .collect();
    let devoluciones_rows: Vec<HashMap<String, serde_json::Value>> = rows
        .iter()
        .filter(|r| r.get("section").and_then(|v| v.as_str()) == Some("DEVOLUCIONES"))
        .cloned()
        .collect();

    let mut ventas_subtotal = HashMap::new();
    let ventas_total: f64 = ventas_rows
        .iter()
        .filter_map(|r| r.get("total").and_then(|v| v.as_f64()))
        .sum();
    ventas_subtotal.insert("total".to_string(), serde_json::json!(ventas_total));
    ventas_subtotal.insert(
        "recordCount".to_string(),
        serde_json::json!(ventas_rows.len()),
    );

    let mut devoluciones_subtotal = HashMap::new();
    let devoluciones_total: f64 = devoluciones_rows
        .iter()
        .filter_map(|r| r.get("total").and_then(|v| v.as_f64()))
        .sum();
    devoluciones_subtotal.insert("total".to_string(), serde_json::json!(devoluciones_total));
    devoluciones_subtotal.insert(
        "recordCount".to_string(),
        serde_json::json!(devoluciones_rows.len()),
    );

    let groups = vec![
        ReportGroup {
            level: 0,
            key: "VENTAS".to_string(),
            label: "VENTAS".to_string(),
            record_count: ventas_rows.len() as i32,
            subtotal: Some(ventas_subtotal),
            rows: ventas_rows,
            sub_groups: None,
            default_expanded: true,
        },
        ReportGroup {
            level: 0,
            key: "DEVOLUCIONES".to_string(),
            label: "DEVOLUCIONES".to_string(),
            record_count: devoluciones_rows.len() as i32,
            subtotal: Some(devoluciones_subtotal),
            rows: devoluciones_rows,
            sub_groups: None,
            default_expanded: true,
        },
    ];

    ErpReportPayload {
        document_type: "REPORT".to_string(),
        tenant_id: 1,
        user_id: 1,
        correlation_id: None,
        report: ReportInfo {
            code: "NCF_SUMMARY".to_string(),
            variant: Some("DETAILED".to_string()),
            title: "Facturas Emitidas por NCF - Detallado".to_string(),
            subtitle: Some("Ventas".to_string()),
            breadcrumb: Some("Ventas / Reportes / Facturas por NCF / Detallado".to_string()),
            generated_at: chrono::Utc::now().to_rfc3339(),
            user_name: Some("Administrador Del Sistema".to_string()),
            date_range: Some(DateRange {
                from: "2025-12-01".to_string(),
                to: "2025-12-15".to_string(),
            }),
            as_of_date: None,
        },
        metadata: ReportMetadata {
            columns: vec![
                ColumnDefinition {
                    key: "invoiceDate".to_string(),
                    label: "Fecha".to_string(),
                    column_type: ColumnType::Date,
                    format: Some("dd/MM/yyyy".to_string()),
                    width: Some(90),
                    align: Some(ColumnAlign::Center),
                    sortable: true,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "encf".to_string(),
                    label: "eNCF".to_string(),
                    column_type: ColumnType::String,
                    format: None,
                    width: Some(140),
                    align: Some(ColumnAlign::Left),
                    sortable: false,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "customerRnc".to_string(),
                    label: "RNC Cliente".to_string(),
                    column_type: ColumnType::String,
                    format: None,
                    width: Some(100),
                    align: Some(ColumnAlign::Left),
                    sortable: false,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "customerName".to_string(),
                    label: "Nombre Cliente".to_string(),
                    column_type: ColumnType::String,
                    format: None,
                    width: Some(180),
                    align: Some(ColumnAlign::Left),
                    sortable: false,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "tipoVenta".to_string(),
                    label: "Tipo Venta".to_string(),
                    column_type: ColumnType::String,
                    format: None,
                    width: Some(80),
                    align: Some(ColumnAlign::Center),
                    sortable: false,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "total".to_string(),
                    label: "Monto Total".to_string(),
                    column_type: ColumnType::Currency,
                    format: Some("#,##0.00".to_string()),
                    width: Some(110),
                    align: Some(ColumnAlign::Right),
                    sortable: true,
                    highlight: Some("primary".to_string()),
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "totalTax".to_string(),
                    label: "Total ITBIS".to_string(),
                    column_type: ColumnType::Currency,
                    format: Some("#,##0.00".to_string()),
                    width: Some(100),
                    align: Some(ColumnAlign::Right),
                    sortable: false,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
            ],
            grouping: Some(GroupingConfig {
                enabled: true,
                field: Some("section".to_string()),
                label_field: Some("section".to_string()),
                label_prefix: None,
                show_subtotals: true,
                subtotal_label: Some("Subtotal".to_string()),
                subtotal_fields: vec!["total".to_string(), "totalTax".to_string()],
                levels: None,
            }),
            show_grand_total: true,
            grand_total_fields: vec![
                "total".to_string(),
                "totalTax".to_string(),
                "recordCount".to_string(),
            ],
            has_running_balance: false,
            show_opening_balance: false,
            aging_buckets: None,
        },
        data: ReportDataSet {
            structure_type: DataStructureType::Grouped,
            rows: None,
            groups: Some(groups),
            opening_balance: None,
            grand_total: Some(grand_total),
            summary: None,
            total_records: records,
            totals: None,
        },
        output: OutputOptions {
            format: OutputFormat::Pdf,
            page_size: Some(PageSize::Letter),
            orientation: Some(if orientation == "landscape" {
                PageOrientation::Landscape
            } else {
                PageOrientation::Portrait
            }),
            scale: 100,
            margins: Some(PageMargins {
                top: 12.0,
                bottom: 12.0,
                left: 10.0,
                right: 10.0,
            }),
            include_header: true,
            include_footer: true,
            show_logo: false,
            file_name: Some("ncf_summary_report.pdf".to_string()),
        },
        delivery: None,
        company_info: None,
    }
}

fn build_ar_balance_payload(records: i32, orientation: &str) -> ErpReportPayload {
    let mut rows: Vec<HashMap<String, serde_json::Value>> = Vec::new();
    let mut total_balance = 0.0f64;

    for i in 0..records {
        let balance = 5000.0 + (i as f64 * 234.56);
        total_balance += balance;

        let mut row = HashMap::new();
        row.insert(
            "clientCode".to_string(),
            serde_json::json!(format!("CLI-{:04}", i + 1)),
        );
        row.insert(
            "clientName".to_string(),
            serde_json::json!(format!("Empresa {} S.R.L.", i + 1)),
        );
        row.insert(
            "documentNumber".to_string(),
            serde_json::json!(format!("FAC-{:06}", i + 1)),
        );
        row.insert(
            "documentDate".to_string(),
            serde_json::json!(format!("2025-11-{:02}", (i % 28) + 1)),
        );
        row.insert(
            "dueDate".to_string(),
            serde_json::json!(format!("2025-12-{:02}", (i % 28) + 1)),
        );
        row.insert(
            "originalAmount".to_string(),
            serde_json::json!(balance * 1.2),
        );
        row.insert("balance".to_string(), serde_json::json!(balance));
        row.insert(
            "current".to_string(),
            serde_json::json!(if i % 4 == 0 { balance } else { 0.0 }),
        );
        row.insert(
            "days30".to_string(),
            serde_json::json!(if i % 4 == 1 { balance } else { 0.0 }),
        );
        row.insert(
            "days60".to_string(),
            serde_json::json!(if i % 4 == 2 { balance } else { 0.0 }),
        );
        row.insert(
            "days90".to_string(),
            serde_json::json!(if i % 4 == 3 { balance } else { 0.0 }),
        );
        rows.push(row);
    }

    let mut grand_total = HashMap::new();
    grand_total.insert("balance".to_string(), serde_json::json!(total_balance));
    grand_total.insert("recordCount".to_string(), serde_json::json!(records));

    ErpReportPayload {
        document_type: "REPORT".to_string(),
        tenant_id: 1,
        user_id: 1,
        correlation_id: None,
        report: ReportInfo {
            code: "AR_BALANCE_ANALYSIS".to_string(),
            variant: Some("DETAILED".to_string()),
            title: "Analisis de Saldos por Cobrar".to_string(),
            subtitle: Some("Cuentas por Cobrar".to_string()),
            breadcrumb: Some("Cuentas por Cobrar / Reportes / Analisis de Saldos".to_string()),
            generated_at: chrono::Utc::now().to_rfc3339(),
            user_name: Some("Administrador Del Sistema".to_string()),
            date_range: None,
            as_of_date: Some("2025-12-15".to_string()),
        },
        metadata: ReportMetadata {
            columns: vec![
                ColumnDefinition {
                    key: "clientCode".to_string(),
                    label: "Codigo".to_string(),
                    column_type: ColumnType::String,
                    format: None,
                    width: Some(80),
                    align: Some(ColumnAlign::Left),
                    sortable: false,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "clientName".to_string(),
                    label: "Cliente".to_string(),
                    column_type: ColumnType::String,
                    format: None,
                    width: Some(200),
                    align: Some(ColumnAlign::Left),
                    sortable: true,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "documentNumber".to_string(),
                    label: "Documento".to_string(),
                    column_type: ColumnType::String,
                    format: None,
                    width: Some(100),
                    align: Some(ColumnAlign::Left),
                    sortable: false,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "dueDate".to_string(),
                    label: "Vencimiento".to_string(),
                    column_type: ColumnType::Date,
                    format: Some("dd/MM/yyyy".to_string()),
                    width: Some(90),
                    align: Some(ColumnAlign::Center),
                    sortable: true,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "balance".to_string(),
                    label: "Saldo".to_string(),
                    column_type: ColumnType::Currency,
                    format: Some("#,##0.00".to_string()),
                    width: Some(110),
                    align: Some(ColumnAlign::Right),
                    sortable: true,
                    highlight: Some("primary".to_string()),
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "current".to_string(),
                    label: "Corriente".to_string(),
                    column_type: ColumnType::Currency,
                    format: Some("#,##0.00".to_string()),
                    width: Some(100),
                    align: Some(ColumnAlign::Right),
                    sortable: false,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "days30".to_string(),
                    label: "1-30 dias".to_string(),
                    column_type: ColumnType::Currency,
                    format: Some("#,##0.00".to_string()),
                    width: Some(100),
                    align: Some(ColumnAlign::Right),
                    sortable: false,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "days60".to_string(),
                    label: "31-60 dias".to_string(),
                    column_type: ColumnType::Currency,
                    format: Some("#,##0.00".to_string()),
                    width: Some(100),
                    align: Some(ColumnAlign::Right),
                    sortable: false,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "days90".to_string(),
                    label: "61+ dias".to_string(),
                    column_type: ColumnType::Currency,
                    format: Some("#,##0.00".to_string()),
                    width: Some(100),
                    align: Some(ColumnAlign::Right),
                    sortable: false,
                    highlight: Some("danger".to_string()),
                    hidden: false,
                    hide_in_print: false,
                },
            ],
            grouping: None,
            show_grand_total: true,
            grand_total_fields: vec![
                "balance".to_string(),
                "current".to_string(),
                "days30".to_string(),
                "days60".to_string(),
                "days90".to_string(),
            ],
            has_running_balance: false,
            show_opening_balance: false,
            aging_buckets: Some(vec![
                AgingBucket {
                    name: "Corriente".to_string(),
                    days_from: 0,
                    days_to: Some(0),
                },
                AgingBucket {
                    name: "1-30 dias".to_string(),
                    days_from: 1,
                    days_to: Some(30),
                },
                AgingBucket {
                    name: "31-60 dias".to_string(),
                    days_from: 31,
                    days_to: Some(60),
                },
                AgingBucket {
                    name: "61+ dias".to_string(),
                    days_from: 61,
                    days_to: None,
                },
            ]),
        },
        data: ReportDataSet {
            structure_type: DataStructureType::Flat,
            rows: Some(rows),
            groups: None,
            opening_balance: None,
            grand_total: Some(grand_total),
            summary: None,
            total_records: records,
            totals: None,
        },
        output: OutputOptions {
            format: OutputFormat::Pdf,
            page_size: Some(PageSize::Letter),
            orientation: Some(if orientation == "landscape" {
                PageOrientation::Landscape
            } else {
                PageOrientation::Portrait
            }),
            scale: 95,
            margins: Some(PageMargins {
                top: 10.0,
                bottom: 10.0,
                left: 8.0,
                right: 8.0,
            }),
            include_header: true,
            include_footer: true,
            show_logo: false,
            file_name: Some("ar_balance_report.pdf".to_string()),
        },
        delivery: None,
        company_info: None,
    }
}

fn build_generic_payload(variant: &str, records: i32, orientation: &str) -> ErpReportPayload {
    let mut rows: Vec<HashMap<String, serde_json::Value>> = Vec::new();

    for i in 0..records {
        let mut row = HashMap::new();
        row.insert("id".to_string(), serde_json::json!(i + 1));
        row.insert(
            "name".to_string(),
            serde_json::json!(format!("Item {}", i + 1)),
        );
        row.insert(
            "value".to_string(),
            serde_json::json!(100.0 + (i as f64 * 10.5)),
        );
        row.insert(
            "date".to_string(),
            serde_json::json!(format!("2025-12-{:02}", (i % 28) + 1)),
        );
        rows.push(row);
    }

    ErpReportPayload {
        document_type: "REPORT".to_string(),
        tenant_id: 1,
        user_id: 1,
        correlation_id: None,
        report: ReportInfo {
            code: variant.to_uppercase(),
            variant: None,
            title: format!("Reporte: {}", variant),
            subtitle: None,
            breadcrumb: Some(format!("Reportes / {}", variant)),
            generated_at: chrono::Utc::now().to_rfc3339(),
            user_name: Some("Test User".to_string()),
            date_range: Some(DateRange {
                from: "2025-12-01".to_string(),
                to: "2025-12-15".to_string(),
            }),
            as_of_date: None,
        },
        metadata: ReportMetadata {
            columns: vec![
                ColumnDefinition {
                    key: "id".to_string(),
                    label: "ID".to_string(),
                    column_type: ColumnType::Integer,
                    format: None,
                    width: Some(60),
                    align: Some(ColumnAlign::Center),
                    sortable: false,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "name".to_string(),
                    label: "Nombre".to_string(),
                    column_type: ColumnType::String,
                    format: None,
                    width: Some(200),
                    align: Some(ColumnAlign::Left),
                    sortable: true,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "value".to_string(),
                    label: "Valor".to_string(),
                    column_type: ColumnType::Currency,
                    format: Some("#,##0.00".to_string()),
                    width: Some(120),
                    align: Some(ColumnAlign::Right),
                    sortable: true,
                    highlight: Some("primary".to_string()),
                    hidden: false,
                    hide_in_print: false,
                },
                ColumnDefinition {
                    key: "date".to_string(),
                    label: "Fecha".to_string(),
                    column_type: ColumnType::Date,
                    format: Some("dd/MM/yyyy".to_string()),
                    width: Some(100),
                    align: Some(ColumnAlign::Center),
                    sortable: true,
                    highlight: None,
                    hidden: false,
                    hide_in_print: false,
                },
            ],
            grouping: None,
            show_grand_total: true,
            grand_total_fields: vec!["value".to_string()],
            has_running_balance: false,
            show_opening_balance: false,
            aging_buckets: None,
        },
        data: ReportDataSet {
            structure_type: DataStructureType::Flat,
            rows: Some(rows),
            groups: None,
            opening_balance: None,
            grand_total: None,
            summary: None,
            total_records: records,
            totals: None,
        },
        output: OutputOptions {
            format: OutputFormat::Pdf,
            page_size: Some(PageSize::Letter),
            orientation: Some(if orientation == "landscape" {
                PageOrientation::Landscape
            } else {
                PageOrientation::Portrait
            }),
            scale: 100,
            margins: None,
            include_header: true,
            include_footer: true,
            show_logo: false,
            file_name: Some(format!("{}_report.pdf", variant)),
        },
        delivery: None,
        company_info: None,
    }
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

/// Generate report document bytes based on format
async fn generate_report_bytes(
    payload: &ErpReportPayload,
    format: &str,
) -> Result<(Vec<u8>, String, String)> {
    use pdf_services::infrastructure::generators::typst_generator::TypstGenerator;
    use pdf_services::infrastructure::generators::ErpReportCsvGenerator;
    use pdf_services::infrastructure::generators::ErpReportExcelGenerator;
    use pdf_services::templates::templates::ErpReportTemplate;

    let work_dir = PathBuf::from("./temp");
    std::fs::create_dir_all(&work_dir)?;

    match format.to_lowercase().as_str() {
        "pdf" => {
            let template = ErpReportTemplate::new();
            let json_value = serde_json::to_value(payload)?;
            template.validate(&json_value)?;
            let typst_content = template.generate(&json_value)?;
            let generator = TypstGenerator::new(work_dir);
            let bytes = generator.generate_pdf(&typst_content).await?;
            Ok((bytes, "application/pdf".to_string(), "pdf".to_string()))
        }
        "xlsx" | "excel" => {
            let generator = ErpReportExcelGenerator::new();
            let bytes = generator.generate(payload).await?;
            Ok((
                bytes,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
                "xlsx".to_string(),
            ))
        }
        "csv" => {
            let generator = ErpReportCsvGenerator::new();
            let bytes = generator.generate(payload).await?;
            Ok((bytes, "text/csv".to_string(), "csv".to_string()))
        }
        _ => anyhow::bail!("Unsupported format: {}", format),
    }
}

async fn send_report_via_email(
    variant: &str,
    format: &str,
    records: i32,
    email: &str,
    subject: Option<String>,
) -> Result<()> {
    use pdf_services::infrastructure::notifications::ErpReportNotifierBuilder;

    println!("\n📧 Send ERP Report via Email\n{}", "=".repeat(50));
    println!("   Variant: {}", variant);
    println!("   Format: {}", format.to_uppercase());
    println!("   Records: {}", records);
    println!("   To: {}", email);

    // Check SMTP config
    let smtp_user = get_smtp_username();
    if smtp_user.is_empty() {
        println!("\n❌ SMTP not configured!");
        println!("   Set the following environment variables:");
        println!("   - SMTP_HOST (default: smtp.gmail.com)");
        println!("   - SMTP_PORT (default: 587)");
        println!("   - SMTP_USER");
        println!("   - SMTP_PASS");
        println!("   - SMTP_FROM_EMAIL (default: noreply@facturazo.com)");
        println!("   - SMTP_FROM_NAME (default: Facturazo ERP)");
        return Ok(());
    }

    println!("\n   SMTP Host: {}", get_smtp_host());
    println!("   SMTP User: {}", smtp_user);

    // Build payload with delivery options
    let mut payload = match variant {
        "ncf_summary" => build_ncf_summary_payload(records, "landscape"),
        "ar_balance" => build_ar_balance_payload(records, "landscape"),
        _ => build_generic_payload(variant, records, "landscape"),
    };

    // Set output format
    payload.output.format = match format.to_lowercase().as_str() {
        "xlsx" | "excel" => OutputFormat::Xlsx,
        "csv" => OutputFormat::Csv,
        _ => OutputFormat::Pdf,
    };

    // Set delivery options
    payload.delivery = Some(DeliveryOptions {
        method: DeliveryMethod::Email,
        email: Some(EmailDelivery {
            recipients: vec![email.to_string()],
            subject,
            message: Some(
                "Este reporte fue generado automáticamente por Facturazo ERP.".to_string(),
            ),
        }),
        whatsapp: None,
    });

    // Generate document
    println!("\n   Generating {} document...", format.to_uppercase());
    let (doc_bytes, _content_type, _ext) = generate_report_bytes(&payload, format).await?;
    println!(
        "   Generated: {} bytes ({:.2} KB)",
        doc_bytes.len(),
        doc_bytes.len() as f64 / 1024.0
    );

    // Create notifier and send
    println!("\n   Sending email...");
    let notifier = ErpReportNotifierBuilder::new()
        .with_email(
            get_smtp_host(),
            get_smtp_port(),
            get_smtp_username(),
            get_smtp_password(),
            get_smtp_from_email(),
            get_smtp_from_name(),
            true, // use TLS
        )
        .build();

    match notifier.deliver_report(&payload, doc_bytes, None).await {
        Ok(result) => {
            println!("\n✅ Email sent successfully!");
            println!("   Recipients: {:?}", result.successful_recipients);
            if !result.message_ids.is_empty() {
                println!("   Message IDs: {:?}", result.message_ids);
            }
            if !result.failed_recipients.is_empty() {
                println!("   Failed: {:?}", result.failed_recipients);
            }
        }
        Err(e) => {
            println!("\n❌ Email failed: {}", e);
        }
    }

    Ok(())
}

async fn send_report_via_whatsapp(
    variant: &str,
    format: &str,
    records: i32,
    phone: &str,
) -> Result<()> {
    use pdf_services::infrastructure::notifications::evolution_api::is_valid_dominican_phone;
    use pdf_services::infrastructure::notifications::{
        ErpReportNotifierBuilder, EvolutionApiClient, WhatsAppProvider,
    };
    use std::sync::Arc;

    println!("\n📱 Send ERP Report via WhatsApp\n{}", "=".repeat(50));
    println!("   Variant: {}", variant);
    println!("   Format: {}", format.to_uppercase());
    println!("   Records: {}", records);
    println!("   To: {}", phone);

    // Validate phone
    if !is_valid_dominican_phone(phone) {
        println!(
            "\n⚠️  Warning: '{}' may not be a valid Dominican phone number",
            phone
        );
        println!("   Expected format: 809-XXX-XXXX, 829-XXX-XXXX, or 849-XXX-XXXX");
    }

    let api_url = get_whatsapp_api_url();
    let api_key = get_whatsapp_api_key();
    let instance = get_whatsapp_instance();

    println!("\n   WhatsApp API: {}", api_url);
    println!("   Instance: {}", instance);

    // Create WhatsApp provider (using Evolution API for local testing)
    let whatsapp_provider: Arc<dyn WhatsAppProvider> =
        Arc::new(EvolutionApiClient::new(api_url, api_key, instance));

    // Build payload with delivery options
    let mut payload = match variant {
        "ncf_summary" => build_ncf_summary_payload(records, "landscape"),
        "ar_balance" => build_ar_balance_payload(records, "landscape"),
        _ => build_generic_payload(variant, records, "landscape"),
    };

    // Set output format
    payload.output.format = match format.to_lowercase().as_str() {
        "xlsx" | "excel" => OutputFormat::Xlsx,
        "csv" => OutputFormat::Csv,
        _ => OutputFormat::Pdf,
    };

    // Set delivery options
    payload.delivery = Some(DeliveryOptions {
        method: DeliveryMethod::WhatsApp,
        email: None,
        whatsapp: Some(WhatsAppDelivery {
            numbers: vec![phone.to_string()],
            message: Some("Reporte generado por Facturazo ERP".to_string()),
        }),
    });

    // Generate document
    println!("\n   Generating {} document...", format.to_uppercase());
    let (doc_bytes, _content_type, _ext) = generate_report_bytes(&payload, format).await?;
    println!(
        "   Generated: {} bytes ({:.2} KB)",
        doc_bytes.len(),
        doc_bytes.len() as f64 / 1024.0
    );

    // Create notifier and send
    println!("\n   Sending via WhatsApp...");
    let notifier = ErpReportNotifierBuilder::new()
        .with_whatsapp_provider(whatsapp_provider)
        .build();

    match notifier.deliver_report(&payload, doc_bytes, None).await {
        Ok(result) => {
            println!("\n✅ WhatsApp sent successfully!");
            println!("   Recipients: {:?}", result.successful_recipients);
            if !result.message_ids.is_empty() {
                println!("   Message IDs: {:?}", result.message_ids);
            }
            if !result.failed_recipients.is_empty() {
                println!("   Failed: {:?}", result.failed_recipients);
            }
        }
        Err(e) => {
            println!("\n❌ WhatsApp failed: {}", e);
        }
    }

    Ok(())
}
