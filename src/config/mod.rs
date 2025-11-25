//! Configuration module for the Document & Notification Service
//!
//! This module handles all configuration loading and validation
//! from environment variables, config files, and defaults.

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Main application configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub kafka: KafkaConfig,
    pub storage: StorageConfig,
    pub cache: CacheConfig,
    pub notification: NotificationConfig,
}

/// HTTP Server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: Option<usize>,
    pub max_connections: usize,
    pub timeout_ms: u64,
}

/// Database configuration (SQLite)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    pub path: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
    pub wal_mode: bool,
}

/// Kafka configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KafkaConfig {
    pub brokers: String,
    pub group_id: String,
    pub client_id: String,
    pub topics: KafkaTopics,
    pub security: Option<KafkaSecurity>,
}

/// Kafka topics configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KafkaTopics {
    pub document_request: String,
    pub document_batch: String,
    pub notification_dispatch: String,
    pub events: String,
    pub dlq: String,
}

/// Kafka security configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KafkaSecurity {
    pub protocol: String,
    pub sasl_mechanism: Option<String>,
    pub sasl_username: Option<String>,
    pub sasl_password: Option<String>,
}

/// Storage configuration (Cloudflare R2)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub url_expiry_seconds: u32,
}

/// Cache configuration (In-memory)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheConfig {
    pub max_capacity: u64,
    pub time_to_live_seconds: u64,
    pub time_to_idle_seconds: u64,
}

/// Notification configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotificationConfig {
    pub email: EmailConfig,
    pub whatsapp: WhatsAppConfig,
}

/// Email configuration (SMTP)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub from_email: String,
    pub from_name: String,
    pub use_tls: bool,
}

/// WhatsApp configuration (EvolutionAPI)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WhatsAppConfig {
    pub api_url: String,
    pub api_key: String,
    pub instance_name: String,
    pub webhook_url: Option<String>,
}

impl AppConfig {
    /// Load configuration from environment and config files
    pub fn load() -> Result<Self> {
        // Load .env file if exists
        dotenv::dotenv().ok();

        // Build configuration from various sources
        let config = config::Config::builder()
            // Start with default values
            .add_source(config::Config::try_from(&Self::default())?)
            // Add environment variables with PDF_ prefix
            .add_source(config::Environment::with_prefix("PDF").separator("_"))
            // Add config file if exists
            .add_source(config::File::with_name("config/default").required(false))
            .add_source(config::File::with_name("config/local").required(false))
            .build()?;

        // Deserialize into our config struct
        let app_config: AppConfig = config.try_deserialize()?;

        // Validate configuration
        app_config.validate()?;

        Ok(app_config)
    }

    /// Validate configuration values
    fn validate(&self) -> Result<()> {
        // Add validation logic here
        if self.server.port == 0 {
            anyhow::bail!("Server port cannot be 0");
        }

        if self.database.path.is_empty() {
            anyhow::bail!("Database path cannot be empty");
        }

        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                workers: None,
                max_connections: 5000,
                timeout_ms: 30000,
            },
            database: DatabaseConfig {
                path: "data/app.db".to_string(),
                max_connections: 5,
                min_connections: 1,
                connection_timeout_seconds: 30,
                idle_timeout_seconds: 600,
                wal_mode: true,
            },
            kafka: KafkaConfig {
                brokers: "localhost:9092".to_string(),
                group_id: "document-service".to_string(),
                client_id: "document-service-01".to_string(),
                topics: KafkaTopics {
                    document_request: "document-generate-request".to_string(),
                    document_batch: "document-batch-request".to_string(),
                    notification_dispatch: "notification-dispatch-request".to_string(),
                    events: "document-events".to_string(),
                    dlq: "document-dlq".to_string(),
                },
                security: None,
            },
            storage: StorageConfig {
                endpoint: "https://r2.cloudflarestorage.com".to_string(),
                bucket: "documents".to_string(),
                region: "auto".to_string(),
                access_key_id: "".to_string(),
                secret_access_key: "".to_string(),
                url_expiry_seconds: 3600,
            },
            cache: CacheConfig {
                max_capacity: 10000,
                time_to_live_seconds: 3600,
                time_to_idle_seconds: 600,
            },
            notification: NotificationConfig {
                email: EmailConfig {
                    smtp_host: "smtp.gmail.com".to_string(),
                    smtp_port: 587,
                    smtp_username: "".to_string(),
                    smtp_password: "".to_string(),
                    from_email: "noreply@example.com".to_string(),
                    from_name: "Document Service".to_string(),
                    use_tls: true,
                },
                whatsapp: WhatsAppConfig {
                    api_url: "http://localhost:8081".to_string(),
                    api_key: "".to_string(),
                    instance_name: "default".to_string(),
                    webhook_url: None,
                },
            },
        }
    }
}
