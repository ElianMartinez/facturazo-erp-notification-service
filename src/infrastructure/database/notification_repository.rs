//! Notification repository implementation with SQLite

use anyhow::Result;
use sqlx::SqlitePool;

/// SQLite implementation of NotificationRepository
pub struct SqliteNotificationRepository {
    pool: SqlitePool,
}

impl SqliteNotificationRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

// Implementation will be added in next phase
#[async_trait::async_trait]
impl super::NotificationRepository for SqliteNotificationRepository {
    async fn create(&self, _notification: &crate::domain::notification::Notification) -> Result<()> {
        todo!("Implement notification creation")
    }

    async fn find_by_id(&self, _tenant_id: &str, _notification_id: &str) -> Result<Option<crate::domain::notification::Notification>> {
        todo!("Implement find by id")
    }

    async fn update_status(&self, _tenant_id: &str, _notification_id: &str, _status: crate::domain::notification::NotificationStatus) -> Result<()> {
        todo!("Implement status update")
    }

    async fn list_pending(&self, _tenant_id: &str, _channel: Option<&str>) -> Result<Vec<crate::domain::notification::Notification>> {
        todo!("Implement list pending")
    }
}