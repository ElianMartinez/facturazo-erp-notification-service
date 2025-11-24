//! Database infrastructure layer
//!
//! SQLite with SQLx for persistence

use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::time::Duration;
use anyhow::Result;

// Temporarily disable repositories until database is set up
// pub mod document_repository;
pub mod invoice_repository;
pub mod notification_repository;
pub mod ncf_sequence_repository;

// pub use document_repository::SqliteDocumentRepository;
pub use invoice_repository::SqliteInvoiceRepository;
pub use notification_repository::SqliteNotificationRepository;
pub use ncf_sequence_repository::SqliteNCFSequenceRepository;

/// Database connection pool
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Create new database connection
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(25)
            .min_connections(5)
            .acquire_timeout(Duration::from_secs(3))
            .idle_timeout(Duration::from_secs(600))
            .connect(database_url)
            .await?;

        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await?;

        Ok(Self { pool })
    }

    /// Get pool reference
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Health check
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await?;
        Ok(())
    }
}

/// Repository trait for documents
#[async_trait::async_trait]
pub trait DocumentRepository: Send + Sync {
    async fn create(&self, document: &crate::domain::document::Document) -> Result<()>;
    async fn find_by_id(&self, tenant_id: &str, document_id: &str) -> Result<Option<crate::domain::document::Document>>;
    async fn find_by_number(&self, tenant_id: &str, document_type: &str, document_number: &str) -> Result<Option<crate::domain::document::Document>>;
    async fn update_status(&self, tenant_id: &str, document_id: &str, status: crate::domain::document::DocumentStatus) -> Result<()>;
    async fn update_storage(&self, tenant_id: &str, document_id: &str, storage_path: &str, storage_url: &str, file_size: u64) -> Result<()>;
    async fn list(&self, tenant_id: &str, limit: i64, offset: i64) -> Result<Vec<crate::domain::document::Document>>;
}

/// Repository trait for invoices
#[async_trait::async_trait]
pub trait InvoiceRepository: Send + Sync {
    async fn create(&self, invoice: &crate::domain::invoice::InvoiceData) -> Result<String>;
    async fn find_by_id(&self, tenant_id: &str, invoice_id: &str) -> Result<Option<crate::domain::invoice::InvoiceData>>;
    async fn find_by_ncf(&self, ncf: &str) -> Result<Option<crate::domain::invoice::InvoiceData>>;
    async fn find_by_number(&self, tenant_id: &str, invoice_number: &str) -> Result<Option<crate::domain::invoice::InvoiceData>>;
    async fn update_status(&self, tenant_id: &str, invoice_id: &str, status: crate::domain::invoice::InvoiceStatus) -> Result<()>;
    async fn mark_paid(&self, tenant_id: &str, invoice_id: &str, payment_method: &str, paid_date: chrono::DateTime<chrono::Utc>) -> Result<()>;
    async fn list_overdue(&self, tenant_id: &str) -> Result<Vec<crate::domain::invoice::InvoiceData>>;
    async fn get_next_invoice_number(&self, tenant_id: &str) -> Result<String>;
}

/// Repository trait for notifications
#[async_trait::async_trait]
pub trait NotificationRepository: Send + Sync {
    async fn create(&self, notification: &crate::domain::notification::Notification) -> Result<()>;
    async fn find_by_id(&self, tenant_id: &str, notification_id: &str) -> Result<Option<crate::domain::notification::Notification>>;
    async fn update_status(&self, tenant_id: &str, notification_id: &str, status: crate::domain::notification::NotificationStatus) -> Result<()>;
    async fn list_pending(&self, tenant_id: &str, channel: Option<&str>) -> Result<Vec<crate::domain::notification::Notification>>;
}

/// Repository trait for NCF sequences
#[async_trait::async_trait]
pub trait NCFSequenceRepository: Send + Sync {
    async fn create_sequence(&self, tenant_id: &str, serie: char, tipo_code: u32, max_sequence: u64, expiry_date: chrono::DateTime<chrono::Utc>) -> Result<()>;
    async fn get_next_ncf(&self, tenant_id: &str, serie: char, tipo_code: u32) -> Result<crate::domain::fiscal::NCF>;
    async fn get_active_sequence(&self, tenant_id: &str, serie: char, tipo_code: u32) -> Result<Option<NCFSequenceInfo>>;
}

/// NCF Sequence information
#[derive(Debug, Clone)]
pub struct NCFSequenceInfo {
    pub serie: char,
    pub tipo_code: u32,
    pub current_sequence: u64,
    pub max_sequence: u64,
    pub expiry_date: chrono::DateTime<chrono::Utc>,
    pub remaining: u64,
}