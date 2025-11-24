//! Document repository implementation with SQLite

use anyhow::Result;
use sqlx::{SqlitePool, Row};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::document::{Document, DocumentId, DocumentType, DocumentStatus};

/// SQLite implementation of DocumentRepository
pub struct SqliteDocumentRepository {
    pool: SqlitePool,
}

impl SqliteDocumentRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl super::DocumentRepository for SqliteDocumentRepository {
    async fn create(&self, document: &Document) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO documents (
                id, tenant_id, document_type, document_number,
                storage_path, storage_url, file_size, mime_type,
                status, created_at, updated_at, expires_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            document.id.as_str(),
            document.tenant_id.as_str(),
            document.document_type.as_str(),
            document.document_number,
            document.storage_path,
            document.storage_url,
            document.file_size,
            document.mime_type,
            document.status.as_str(),
            document.created_at,
            document.updated_at,
            document.expires_at
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn find_by_id(&self, tenant_id: &str, document_id: &str) -> Result<Option<Document>> {
        let row = sqlx::query!(
            r#"
            SELECT
                id, tenant_id, document_type, document_number,
                storage_path, storage_url, file_size, mime_type,
                status, error_message, created_at, updated_at, expires_at
            FROM documents
            WHERE tenant_id = ? AND id = ?
            "#,
            tenant_id,
            document_id
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let document = Document {
                    id: DocumentId::from_string(r.id),
                    tenant_id: r.tenant_id,
                    document_type: DocumentType::from_str(&r.document_type)?,
                    document_number: r.document_number,
                    storage_path: r.storage_path,
                    storage_url: r.storage_url,
                    file_size: r.file_size.map(|s| s as u64),
                    mime_type: r.mime_type.unwrap_or_else(|| "application/pdf".to_string()),
                    status: DocumentStatus::from_str(&r.status)?,
                    error_message: r.error_message,
                    created_at: DateTime::parse_from_rfc3339(&r.created_at)?.with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&r.updated_at)?.with_timezone(&Utc),
                    expires_at: r.expires_at.and_then(|e| DateTime::parse_from_rfc3339(&e).ok()).map(|dt| dt.with_timezone(&Utc)),
                };
                Ok(Some(document))
            },
            None => Ok(None)
        }
    }

    async fn find_by_number(&self, tenant_id: &str, document_type: &str, document_number: &str) -> Result<Option<Document>> {
        let row = sqlx::query!(
            r#"
            SELECT
                id, tenant_id, document_type, document_number,
                storage_path, storage_url, file_size, mime_type,
                status, error_message, created_at, updated_at, expires_at
            FROM documents
            WHERE tenant_id = ? AND document_type = ? AND document_number = ?
            "#,
            tenant_id,
            document_type,
            document_number
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let document = Document {
                    id: DocumentId::from_string(r.id),
                    tenant_id: r.tenant_id,
                    document_type: DocumentType::from_str(&r.document_type)?,
                    document_number: r.document_number,
                    storage_path: r.storage_path,
                    storage_url: r.storage_url,
                    file_size: r.file_size.map(|s| s as u64),
                    mime_type: r.mime_type.unwrap_or_else(|| "application/pdf".to_string()),
                    status: DocumentStatus::from_str(&r.status)?,
                    error_message: r.error_message,
                    created_at: DateTime::parse_from_rfc3339(&r.created_at)?.with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&r.updated_at)?.with_timezone(&Utc),
                    expires_at: r.expires_at.and_then(|e| DateTime::parse_from_rfc3339(&e).ok()).map(|dt| dt.with_timezone(&Utc)),
                };
                Ok(Some(document))
            },
            None => Ok(None)
        }
    }

    async fn update_status(&self, tenant_id: &str, document_id: &str, status: DocumentStatus) -> Result<()> {
        let error_message = if status == DocumentStatus::Failed {
            Some("Processing failed")
        } else {
            None
        };

        sqlx::query!(
            r#"
            UPDATE documents
            SET status = ?, error_message = ?, updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = ? AND id = ?
            "#,
            status.as_str(),
            error_message,
            tenant_id,
            document_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_storage(&self, tenant_id: &str, document_id: &str, storage_path: &str, storage_url: &str, file_size: u64) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE documents
            SET storage_path = ?, storage_url = ?, file_size = ?,
                status = 'completed', updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = ? AND id = ?
            "#,
            storage_path,
            storage_url,
            file_size as i64,
            tenant_id,
            document_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list(&self, tenant_id: &str, limit: i64, offset: i64) -> Result<Vec<Document>> {
        let rows = sqlx::query!(
            r#"
            SELECT
                id, tenant_id, document_type, document_number,
                storage_path, storage_url, file_size, mime_type,
                status, error_message, created_at, updated_at, expires_at
            FROM documents
            WHERE tenant_id = ?
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
            "#,
            tenant_id,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await?;

        let mut documents = Vec::new();
        for r in rows {
            let document = Document {
                id: DocumentId::from_string(r.id),
                tenant_id: r.tenant_id,
                document_type: DocumentType::from_str(&r.document_type)?,
                document_number: r.document_number,
                storage_path: r.storage_path,
                storage_url: r.storage_url,
                file_size: r.file_size.map(|s| s as u64),
                mime_type: r.mime_type.unwrap_or_else(|| "application/pdf".to_string()),
                status: DocumentStatus::from_str(&r.status)?,
                error_message: r.error_message,
                created_at: DateTime::parse_from_rfc3339(&r.created_at)?.with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&r.updated_at)?.with_timezone(&Utc),
                expires_at: r.expires_at.and_then(|e| DateTime::parse_from_rfc3339(&e).ok()).map(|dt| dt.with_timezone(&Utc)),
            };
            documents.push(document);
        }

        Ok(documents)
    }
}

impl DocumentType {
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "invoice" => Ok(DocumentType::Invoice),
            "quotation" => Ok(DocumentType::Quotation),
            "report" => Ok(DocumentType::Report),
            "receipt" => Ok(DocumentType::Receipt),
            "credit_note" => Ok(DocumentType::CreditNote),
            _ => Err(anyhow::anyhow!("Invalid document type: {}", s))
        }
    }

    fn as_str(&self) -> &str {
        match self {
            DocumentType::Invoice => "invoice",
            DocumentType::Quotation => "quotation",
            DocumentType::Report => "report",
            DocumentType::Receipt => "receipt",
            DocumentType::CreditNote => "credit_note",
        }
    }
}

impl DocumentStatus {
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "pending" => Ok(DocumentStatus::Pending),
            "processing" => Ok(DocumentStatus::Processing),
            "completed" => Ok(DocumentStatus::Completed),
            "failed" => Ok(DocumentStatus::Failed),
            "archived" => Ok(DocumentStatus::Archived),
            _ => Err(anyhow::anyhow!("Invalid document status: {}", s))
        }
    }

    fn as_str(&self) -> &str {
        match self {
            DocumentStatus::Pending => "pending",
            DocumentStatus::Processing => "processing",
            DocumentStatus::Completed => "completed",
            DocumentStatus::Failed => "failed",
            DocumentStatus::Archived => "archived",
        }
    }
}