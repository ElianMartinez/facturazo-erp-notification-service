//! Invoice repository implementation with SQLite

use anyhow::Result;
use sqlx::SqlitePool;

/// SQLite implementation of InvoiceRepository
pub struct SqliteInvoiceRepository {
    pool: SqlitePool,
}

impl SqliteInvoiceRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

// Implementation will be added in next phase
#[async_trait::async_trait]
impl super::InvoiceRepository for SqliteInvoiceRepository {
    async fn create(&self, _invoice: &crate::domain::invoice::InvoiceData) -> Result<String> {
        todo!("Implement invoice creation")
    }

    async fn find_by_id(
        &self,
        _tenant_id: &str,
        _invoice_id: &str,
    ) -> Result<Option<crate::domain::invoice::InvoiceData>> {
        todo!("Implement find by id")
    }

    async fn find_by_ncf(&self, _ncf: &str) -> Result<Option<crate::domain::invoice::InvoiceData>> {
        todo!("Implement find by NCF")
    }

    async fn find_by_number(
        &self,
        _tenant_id: &str,
        _invoice_number: &str,
    ) -> Result<Option<crate::domain::invoice::InvoiceData>> {
        todo!("Implement find by number")
    }

    async fn update_status(
        &self,
        _tenant_id: &str,
        _invoice_id: &str,
        _status: crate::domain::invoice::InvoiceStatus,
    ) -> Result<()> {
        todo!("Implement status update")
    }

    async fn mark_paid(
        &self,
        _tenant_id: &str,
        _invoice_id: &str,
        _payment_method: &str,
        _paid_date: chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        todo!("Implement mark paid")
    }

    async fn list_overdue(
        &self,
        _tenant_id: &str,
    ) -> Result<Vec<crate::domain::invoice::InvoiceData>> {
        todo!("Implement list overdue")
    }

    async fn get_next_invoice_number(&self, _tenant_id: &str) -> Result<String> {
        todo!("Implement get next invoice number")
    }
}
