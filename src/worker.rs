use rdkafka::{
    consumer::{Consumer, StreamConsumer, CommitMode},
    Message,
    ClientConfig,
};
use tokio::sync::Semaphore;
use std::sync::Arc;
use anyhow::Result;
use std::time::Duration;
use futures::StreamExt;

use document_generator::{
    DocumentRequest, DocumentStatus, DocumentType,
    PdfGenerator, ExcelGenerator,
    TemplateManager, S3Client,
};
use document_generator::models::{ReportRequest, DataSource, InvoiceRequest};

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv::dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .init();

    tracing::info!("Starting Document Worker");

    // Load configuration
    let config = WorkerConfig::from_env()?;

    // Start worker pool
    let worker_pool = WorkerPool::new(config).await?;
    worker_pool.start().await?;

    Ok(())
}

#[derive(Clone)]
struct WorkerConfig {
    kafka_brokers: String,
    kafka_group_id: String,
    kafka_topics: Vec<String>,
    max_concurrent: usize,
    worker_threads: usize,
    database_url: String,
    redis_url: String,
    s3_bucket_documents: String,
    s3_bucket_temp: String,
}

impl WorkerConfig {
    fn from_env() -> Result<Self> {
        use std::env;

        Ok(WorkerConfig {
            kafka_brokers: env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string()),
            kafka_group_id: env::var("KAFKA_GROUP_ID").unwrap_or_else(|_| "doc-workers".to_string()),
            kafka_topics: vec![
                env::var("KAFKA_TOPIC_PRIORITY").unwrap_or_else(|_| "doc.requests.priority".to_string()),
                env::var("KAFKA_TOPIC_BULK").unwrap_or_else(|_| "doc.requests.bulk".to_string()),
            ],
            max_concurrent: env::var("MAX_CONCURRENT")
                .unwrap_or_else(|_| "20".to_string())
                .parse()?,
            worker_threads: env::var("WORKER_THREADS")
                .unwrap_or_else(|_| "4".to_string())
                .parse()?,
            database_url: env::var("DATABASE_URL")?,
            redis_url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost".to_string()),
            s3_bucket_documents: env::var("S3_BUCKET_DOCUMENTS").unwrap_or_else(|_| "documents".to_string()),
            s3_bucket_temp: env::var("S3_BUCKET_TEMP").unwrap_or_else(|_| "temp-uploads".to_string()),
        })
    }
}

struct WorkerPool {
    consumer: Arc<StreamConsumer>,
    semaphore: Arc<Semaphore>,
    template_manager: Arc<TemplateManager>,
    s3_client: Arc<S3Client>,
    db: Arc<sqlx::PgPool>,
    redis: Arc<redis::aio::ConnectionManager>,
    config: WorkerConfig,
}

impl WorkerPool {
    async fn new(config: WorkerConfig) -> Result<Self> {
        // Create Kafka consumer
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &config.kafka_brokers)
            .set("group.id", &config.kafka_group_id)
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest")
            .set("session.timeout.ms", "30000")
            .create()?;

        // Subscribe to topics
        consumer.subscribe(&config.kafka_topics.iter().map(|s| s.as_str()).collect::<Vec<_>>())?;

        // Initialize S3 client
        let s3_client = Arc::new(S3Client::new().await?);

        // Initialize database
        let db = Arc::new(sqlx::PgPool::connect(&config.database_url).await?);

        // Initialize Redis
        let redis_client = redis::Client::open(config.redis_url.clone())?;
        let redis = Arc::new(redis::aio::ConnectionManager::new(redis_client).await?);

        // Initialize template manager
        let template_manager = Arc::new(
            TemplateManager::new(
                std::path::PathBuf::from("templates"),
                Some(config.redis_url.clone()),
                Some(s3_client.clone()),
            ).await?
        );

        // Create semaphore for concurrency control
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent));

        Ok(WorkerPool {
            consumer: Arc::new(consumer),
            semaphore,
            template_manager,
            s3_client,
            db,
            redis,
            config,
        })
    }

    async fn start(self) -> Result<()> {
        let pool = Arc::new(self);

        // Spawn worker threads
        let mut handles = vec![];

        for i in 0..pool.config.worker_threads {
            let pool_clone = pool.clone();
            let handle = tokio::spawn(async move {
                tracing::info!("Worker {} started", i);
                pool_clone.process_messages().await
            });
            handles.push(handle);
        }

        // Wait for all workers
        for handle in handles {
            handle.await??;
        }

        Ok(())
    }

    async fn process_messages(&self) -> Result<()> {
        let mut stream = self.consumer.stream();

        while let Some(message) = stream.next().await {
            match message {
                Ok(msg) => {
                    let permit = self.semaphore.clone().acquire_owned().await?;
                    let pool = Arc::new(self.clone());

                    tokio::spawn(async move {
                        match pool.process_single_message(msg).await {
                            Ok(_) => {
                                tracing::info!("Message processed successfully");
                            },
                            Err(e) => {
                                tracing::error!("Error processing message: {:?}", e);
                            }
                        }
                        drop(permit);
                    });
                },
                Err(e) => {
                    tracing::error!("Kafka error: {:?}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }

        Ok(())
    }

    async fn process_single_message(&self, msg: rdkafka::message::BorrowedMessage<'_>) -> Result<()> {
        let payload = msg.payload()
            .ok_or_else(|| anyhow::anyhow!("Empty payload"))?;

        let request: DocumentRequest = serde_json::from_slice(payload)?;

        tracing::info!("Processing document: {}", request.id);

        // Update status to processing
        self.update_status(&request, DocumentStatus::Processing).await?;

        let start = std::time::Instant::now();

        // Generate document based on type
        let result = match request.document_type {
            DocumentType::Invoice => {
                self.generate_invoice(&request).await
            },
            DocumentType::Report => {
                self.generate_report(&request).await
            },
            _ => {
                self.generate_custom(&request).await
            }
        };

        match result {
            Ok(document_url) => {
                // Update status to completed
                self.save_completed_document(&request, document_url, start.elapsed()).await?;

                // Commit the message
                self.consumer.commit_message(&msg, CommitMode::Async)?;

                // Send notification if callback URL exists
                if let Some(callback_url) = &request.callback_url {
                    self.send_callback(callback_url, &request.id).await?;
                }

                Ok(())
            },
            Err(e) => {
                tracing::error!("Failed to generate document: {:?}", e);
                self.save_failed_document(&request, e.to_string()).await?;

                // Still commit to avoid reprocessing
                self.consumer.commit_message(&msg, CommitMode::Async)?;

                Err(e)
            }
        }
    }

    async fn generate_invoice(&self, request: &DocumentRequest) -> Result<String> {
        let invoice_req: InvoiceRequest = serde_json::from_value(request.data.clone())?;

        // Generate PDF
        let pdf_generator = PdfGenerator::new(self.template_manager.clone());
        let pdf_bytes = pdf_generator.generate_invoice(&invoice_req).await?;

        // Upload to S3
        let key = format!("invoices/{}/{}.pdf", request.metadata.organization_id, request.id);
        let url = self.s3_client.put_object(
            &self.config.s3_bucket_documents,
            &key,
            pdf_bytes,
            "application/pdf",
        ).await?;

        Ok(url)
    }

    async fn generate_report(&self, request: &DocumentRequest) -> Result<String> {
        let report_req: ReportRequest = serde_json::from_value(request.data.clone())?;

        // Load data based on source
        let data = match &report_req.data_source {
            DataSource::Inline { rows } => rows.clone(),
            DataSource::R2Reference { bucket, key, .. } => {
                // Fetch from S3/R2
                let content = self.s3_client.get_object(bucket, key).await?;
                serde_json::from_str(&content)?
            },
            DataSource::Compressed { format, data } => {
                // Decompress data
                use flate2::read::GzDecoder;
                use std::io::Read;

                let mut decoder = GzDecoder::new(&data[..]);
                let mut decompressed = String::new();
                decoder.read_to_string(&mut decompressed)?;
                serde_json::from_str(&decompressed)?
            },
            _ => {
                return Err(anyhow::anyhow!("Unsupported data source"));
            }
        };

        // Generate Excel or PDF based on format
        let (bytes, content_type, extension) = match request.format {
            document_generator::OutputFormat::Excel => {
                let excel_gen = ExcelGenerator::new();
                let bytes = excel_gen.generate_report(&report_req, data).await?;
                (bytes, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", "xlsx")
            },
            document_generator::OutputFormat::Pdf => {
                let pdf_gen = PdfGenerator::new(self.template_manager.clone());
                let bytes = pdf_gen.generate_report(&report_req, data).await?;
                (bytes, "application/pdf", "pdf")
            },
            _ => {
                return Err(anyhow::anyhow!("Unsupported format"));
            }
        };

        // Upload to S3
        let key = format!("reports/{}/{}.{}", request.metadata.organization_id, request.id, extension);
        let url = self.s3_client.put_object(
            &self.config.s3_bucket_documents,
            &key,
            bytes,
            content_type,
        ).await?;

        Ok(url)
    }

    async fn generate_custom(&self, request: &DocumentRequest) -> Result<String> {
        // Placeholder for custom document generation
        Err(anyhow::anyhow!("Custom document type not implemented"))
    }

    async fn update_status(&self, request: &DocumentRequest, status: DocumentStatus) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE documents
            SET status = $1, updated_at = NOW()
            WHERE id = $2
            "#,
            status.to_string(),
            request.id
        )
        .execute(self.db.as_ref())
        .await?;

        Ok(())
    }

    async fn save_completed_document(
        &self,
        request: &DocumentRequest,
        url: String,
        processing_time: Duration,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE documents
            SET status = $1, url = $2, processing_time_ms = $3, updated_at = NOW()
            WHERE id = $4
            "#,
            DocumentStatus::Completed.to_string(),
            url,
            processing_time.as_millis() as i64,
            request.id
        )
        .execute(self.db.as_ref())
        .await?;

        // Update statistics
        self.update_statistics(request, true, processing_time).await?;

        Ok(())
    }

    async fn save_failed_document(&self, request: &DocumentRequest, error: String) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE documents
            SET status = $1, error = $2, updated_at = NOW()
            WHERE id = $3
            "#,
            DocumentStatus::Failed.to_string(),
            error,
            request.id
        )
        .execute(self.db.as_ref())
        .await?;

        // Update statistics
        self.update_statistics(request, false, Duration::from_secs(0)).await?;

        Ok(())
    }

    async fn update_statistics(
        &self,
        request: &DocumentRequest,
        success: bool,
        processing_time: Duration,
    ) -> Result<()> {
        let today = chrono::Utc::now().date_naive();
        let doc_type = format!("{:?}", request.document_type);
        let format = format!("{:?}", request.format);

        if success {
            sqlx::query!(
                r#"
                INSERT INTO usage_statistics
                    (organization_id, date, document_type, format, count, total_processing_time_ms)
                VALUES ($1, $2, $3, $4, 1, $5)
                ON CONFLICT (organization_id, date, document_type, format)
                DO UPDATE SET
                    count = usage_statistics.count + 1,
                    total_processing_time_ms = usage_statistics.total_processing_time_ms + $5
                "#,
                request.metadata.organization_id,
                today,
                doc_type,
                format,
                processing_time.as_millis() as i64
            )
            .execute(self.db.as_ref())
            .await?;
        } else {
            sqlx::query!(
                r#"
                INSERT INTO usage_statistics
                    (organization_id, date, document_type, format, failed_count)
                VALUES ($1, $2, $3, $4, 1)
                ON CONFLICT (organization_id, date, document_type, format)
                DO UPDATE SET
                    failed_count = usage_statistics.failed_count + 1
                "#,
                request.metadata.organization_id,
                today,
                doc_type,
                format
            )
            .execute(self.db.as_ref())
            .await?;
        }

        Ok(())
    }

    async fn send_callback(&self, callback_url: &str, document_id: &uuid::Uuid) -> Result<()> {
        // Send webhook notification
        let client = reqwest::Client::new();
        let _ = client
            .post(callback_url)
            .json(&serde_json::json!({
                "document_id": document_id,
                "status": "completed",
                "timestamp": chrono::Utc::now()
            }))
            .timeout(Duration::from_secs(10))
            .send()
            .await;

        Ok(())
    }
}

impl Clone for WorkerPool {
    fn clone(&self) -> Self {
        WorkerPool {
            consumer: self.consumer.clone(),
            semaphore: self.semaphore.clone(),
            template_manager: self.template_manager.clone(),
            s3_client: self.s3_client.clone(),
            db: self.db.clone(),
            redis: self.redis.clone(),
            config: self.config.clone(),
        }
    }
}