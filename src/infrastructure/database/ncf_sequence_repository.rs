//! NCF Sequence repository implementation with SQLite

use anyhow::Result;
use sqlx::SqlitePool;

/// SQLite implementation of NCFSequenceRepository
pub struct SqliteNCFSequenceRepository {
    pool: SqlitePool,
}

impl SqliteNCFSequenceRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

// Implementation will be added in next phase
#[async_trait::async_trait]
impl super::NCFSequenceRepository for SqliteNCFSequenceRepository {
    async fn create_sequence(
        &self,
        _tenant_id: &str,
        _serie: char,
        _tipo_code: u32,
        _max_sequence: u64,
        _expiry_date: chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        todo!("Implement create sequence")
    }

    async fn get_next_ncf(
        &self,
        _tenant_id: &str,
        _serie: char,
        _tipo_code: u32,
    ) -> Result<crate::domain::fiscal::NCF> {
        todo!("Implement get next NCF")
    }

    async fn get_active_sequence(
        &self,
        _tenant_id: &str,
        _serie: char,
        _tipo_code: u32,
    ) -> Result<Option<super::NCFSequenceInfo>> {
        todo!("Implement get active sequence")
    }
}
