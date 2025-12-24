use actix_web::{web, HttpRequest, HttpResponse};
use chrono::Utc;
use flate2::read::GzDecoder;
use serde_json::json;
use std::io::Read;
use uuid::Uuid;

use super::error::ApiResult;
use super::state::ApiState;
use crate::application::commands::{GenerateDocumentCommand, SendNotificationCommand};
use crate::domain::document::{DocumentFormat, DocumentType as DomainDocType};
use crate::domain::notification::{NotificationChannel, NotificationPriority};
use crate::models::{
    DocumentRequest, DocumentResponse, DocumentStatus, DocumentType, NotificationConfig, Priority,
};

use crate::api::middleware::auth::{extract_tenant_user, extract_tenant_user_or_default};

/// Generate document synchronously (small documents only)
pub async fn generate_sync(
    req: HttpRequest,
    mut data: web::Json<DocumentRequest>,
    state: web::Data<ApiState>,
) -> ApiResult<HttpResponse> {
    // Extract tenant and user info
    let (tenant_id, user_id) = extract_tenant_user_or_default(&req);

    // Update metadata with tenant and user info
    data.metadata.tenant_id = tenant_id;
    data.metadata.user_id = user_id;

    // Check rate limit using tenant:user key
    let rate_limit_key = format!("{}:{}", tenant_id, user_id);
    if let Err(_) = state.rate_limiter.check_key(&rate_limit_key) {
        return Ok(HttpResponse::TooManyRequests().json(json!({
            "error": "Rate limit exceeded",
            "retry_after": 60
        })));
    }

    // Check document size
    let data_size = serde_json::to_vec(&data.data)?.len();
    if data_size > state.config.max_sync_size_bytes {
        // Redirect to async
        return generate_async(req, data, state).await;
    }

    let start = std::time::Instant::now();

    // Clone id before consuming data
    let document_id = data.id;
    let document_type = data.document_type.clone();

    // Generate document based on type
    let result = match document_type {
        DocumentType::Invoice => generate_invoice_sync(&data.into_inner(), &state).await,
        DocumentType::Report if data_size < 100_000 => {
            // Small reports only
            generate_report_sync(&data.into_inner(), &state).await
        }
        _ => {
            // All other types go to async queue
            return generate_async(req, data, state).await;
        }
    };

    match result {
        Ok(document_url) => {
            let response = DocumentResponse {
                id: document_id,
                status: DocumentStatus::Completed,
                url: Some(document_url),
                error: None,
                processing_time_ms: start.elapsed().as_millis() as u64,
                created_at: Utc::now(),
                expires_at: None,
            };

            // Save to database
            // Document metadata would be saved to cache/S3 in production

            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to generate document: {:?}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "error": "Failed to generate document",
                "details": e.to_string()
            })))
        }
    }
}

/// Queue document for async generation
pub async fn generate_async(
    req: HttpRequest,
    mut data: web::Json<DocumentRequest>,
    state: web::Data<ApiState>,
) -> ApiResult<HttpResponse> {
    let (tenant_id, user_id) = extract_tenant_user_or_default(&req);

    // Update metadata with tenant and user info
    data.metadata.tenant_id = tenant_id;
    data.metadata.user_id = user_id;

    // Check rate limit using tenant:user key
    let rate_limit_key = format!("{}:{}", tenant_id, user_id);
    if let Err(_) = state.rate_limiter.check_key(&rate_limit_key) {
        return Ok(HttpResponse::TooManyRequests().json(json!({
            "error": "Rate limit exceeded",
            "retry_after": 60
        })));
    }

    // Clone id before consuming data
    let document_id = data.id;

    // In a production system, this would queue the job to a background worker
    // For now, we'll process it inline using tokio::spawn
    let state_clone = state.clone();
    let data_clone = data.into_inner();

    tokio::spawn(async move {
        // Process the document asynchronously
        match process_document_async(state_clone, data_clone).await {
            Ok(_) => tracing::info!("Document {} processed successfully", document_id),
            Err(e) => tracing::error!("Failed to process document {}: {}", document_id, e),
        }
    });

    Ok(HttpResponse::Accepted().json(json!({
        "id": document_id,
        "status": "processing",
        "estimated_time_seconds": 30,
        "status_url": format!("/api/v1/documents/{}/status", document_id)
    })))
}

/// Handle large file upload
pub async fn upload_data(
    req: HttpRequest,
    mut payload: web::Payload,
    state: web::Data<ApiState>,
) -> ApiResult<HttpResponse> {
    use futures::StreamExt;

    let (_tenant_id, user_id) = extract_tenant_user(&req)
        .ok_or_else(|| actix_web::error::ErrorUnauthorized("No auth info"))?;

    // Check rate limit
    if let Err(_) = state.rate_limiter.check_key(&user_id.to_string()) {
        return Ok(HttpResponse::TooManyRequests().json(json!({
            "error": "Rate limit exceeded",
            "retry_after": 60
        })));
    }

    // Read body with size limit
    let mut body = web::BytesMut::new();
    let max_size = state.config.max_upload_size_bytes;

    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        if (body.len() + chunk.len()) > max_size {
            return Ok(HttpResponse::PayloadTooLarge().json(json!({
                "error": "File too large",
                "max_size_mb": max_size / 1_048_576
            })));
        }
        body.extend_from_slice(&chunk);
    }

    // Check if compressed
    let content_encoding = req
        .headers()
        .get("Content-Encoding")
        .and_then(|h| h.to_str().ok());

    let decompressed = match content_encoding {
        Some("gzip") => {
            let mut decoder = GzDecoder::new(&body[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            decompressed
        }
        _ => body.to_vec(),
    };

    // Upload to S3 temp bucket
    let file_key = format!("uploads/{}/{}.json", user_id, Uuid::new_v4());
    state
        .s3_client
        .put_object(
            &state.config.s3_bucket_temp,
            &file_key,
            decompressed,
            "application/json",
        )
        .await?;

    // Return reference
    Ok(HttpResponse::Ok().json(json!({
        "status": "uploaded",
        "data_reference": {
            "bucket": state.config.s3_bucket_temp,
            "key": file_key,
            "expires_in": 86400
        }
    })))
}

/// Get document status (placeholder - no longer using database)
pub async fn get_status(
    _req: HttpRequest,
    _path: web::Path<Uuid>,
    _state: web::Data<ApiState>,
) -> ApiResult<HttpResponse> {
    // This would need to check S3 or a cache service
    Ok(HttpResponse::Ok().json(json!({
        "status": "completed",
        "message": "Status tracking not implemented in this version"
    })))
}

/// Download document (simplified version)
pub async fn download_document(
    req: HttpRequest,
    path: web::Path<Uuid>,
    state: web::Data<ApiState>,
) -> ApiResult<HttpResponse> {
    let document_id = path.into_inner();
    let (_tenant_id, _user_id) = extract_tenant_user_or_default(&req);

    // For now, construct the S3 key directly
    let key = format!("documents/{}.pdf", document_id);

    // Generate presigned URL
    let presigned = state
        .s3_client
        .create_presigned_url(
            &state.config.s3_bucket_documents,
            &key,
            3600, // 1 hour
        )
        .await?;

    Ok(HttpResponse::Found()
        .append_header(("Location", presigned))
        .finish())
}

// Helper functions

async fn generate_invoice_sync(
    request: &DocumentRequest,
    state: &ApiState,
) -> anyhow::Result<String> {
    // Build command for orchestrator
    let command = GenerateDocumentCommand {
        tenant_id: request.metadata.tenant_id.to_string(),
        user_id: Some(request.metadata.user_id.to_string()),
        document_type: DomainDocType::Invoice,
        format: DocumentFormat::Pdf,
        data: request.data.clone(),
        template_id: request.template_id.clone(),
        template_version: "latest".to_string(),
        storage_enabled: true,
        notification_enabled: request.notification.is_some(),
        priority: crate::application::commands::Priority::Normal,
    };

    // Use orchestrator to generate and store
    let result = state.document_orchestrator.execute(command).await?;

    // Send notifications if configured
    if let Some(ref notification_config) = request.notification {
        send_notifications(
            state,
            notification_config,
            &result.document_id,
            &request.metadata.tenant_id.to_string(),
        )
        .await;
    }

    // Return URL or file path
    result
        .storage_url
        .ok_or_else(|| anyhow::anyhow!("No storage URL returned"))
}

async fn generate_report_sync(
    request: &DocumentRequest,
    state: &ApiState,
) -> anyhow::Result<String> {
    // Build command for orchestrator - reports default to Excel
    let command = GenerateDocumentCommand {
        tenant_id: request.metadata.tenant_id.to_string(),
        user_id: Some(request.metadata.user_id.to_string()),
        document_type: DomainDocType::Report,
        format: DocumentFormat::Excel,
        data: request.data.clone(),
        template_id: request.template_id.clone(),
        template_version: "latest".to_string(),
        storage_enabled: true,
        notification_enabled: request.notification.is_some(),
        priority: crate::application::commands::Priority::Normal,
    };

    // Use orchestrator to generate and store
    let result = state.document_orchestrator.execute(command).await?;

    // Send notifications if configured
    if let Some(ref notification_config) = request.notification {
        send_notifications(
            state,
            notification_config,
            &result.document_id,
            &request.metadata.tenant_id.to_string(),
        )
        .await;
    }

    // Return URL or file path
    result
        .storage_url
        .ok_or_else(|| anyhow::anyhow!("No storage URL returned"))
}

// NOTE: extract_tenant_user moved to middleware/auth.rs for centralization

#[derive(Clone, Debug)]
pub struct AuthInfo {
    pub tenant_id: i64,
    pub user_id: i64,
}

#[allow(dead_code)]
fn estimate_processing_time(request: &DocumentRequest) -> u64 {
    match (&request.document_type, &request.priority) {
        (DocumentType::Invoice, Priority::High) => 30,
        (DocumentType::Invoice, _) => 60,
        (DocumentType::Report, Priority::High) => 120,
        (DocumentType::Report, _) => 300,
        _ => 180,
    }
}

// Database helper functions removed - would use cache/S3 in production

// Process document asynchronously using orchestrator
async fn process_document_async(
    state: web::Data<ApiState>,
    request: DocumentRequest,
) -> anyhow::Result<()> {
    let start = std::time::Instant::now();

    // Map document type to domain type and format
    let (doc_type, format) = match &request.document_type {
        DocumentType::Invoice => (DomainDocType::Invoice, DocumentFormat::Pdf),
        DocumentType::Report => (DomainDocType::Report, DocumentFormat::Excel),
        DocumentType::Receipt => (DomainDocType::Receipt, DocumentFormat::Pdf),
        DocumentType::Certificate => (
            DomainDocType::Custom("certificate".to_string()),
            DocumentFormat::Pdf,
        ),
        DocumentType::Statement => (
            DomainDocType::Custom("statement".to_string()),
            DocumentFormat::Pdf,
        ),
        DocumentType::Custom(name) => (DomainDocType::Custom(name.clone()), DocumentFormat::Pdf),
    };

    // Build command for orchestrator
    let command = GenerateDocumentCommand {
        tenant_id: request.metadata.tenant_id.to_string(),
        user_id: Some(request.metadata.user_id.to_string()),
        document_type: doc_type,
        format,
        data: request.data.clone(),
        template_id: request.template_id.clone(),
        template_version: "latest".to_string(),
        storage_enabled: true,
        notification_enabled: false,
        priority: crate::application::commands::Priority::Normal,
    };

    // Use orchestrator to generate and store
    let result = state.document_orchestrator.execute(command).await?;

    let processing_time = start.elapsed().as_millis() as i64;
    tracing::info!(
        "Document {} processed in {}ms, size: {} bytes",
        request.id,
        processing_time,
        result.file_size
    );

    Ok(())
}

/// Helper function to send notifications via WhatsApp and/or Email
async fn send_notifications(
    state: &ApiState,
    config: &NotificationConfig,
    document_id: &str,
    tenant_id: &str,
) {
    // Send WhatsApp notification
    if let Some(ref whatsapp) = config.whatsapp {
        let message = whatsapp
            .message
            .clone()
            .unwrap_or_else(|| format!("📄 Tu documento {} está listo.", document_id));

        let command = SendNotificationCommand {
            tenant_id: tenant_id.to_string(),
            channel: NotificationChannel::Whatsapp,
            recipient: whatsapp.phone.clone(),
            subject: None,
            template_id: None,
            template_vars: json!({ "message": message }),
            priority: NotificationPriority::Normal,
            document_id: Some(document_id.to_string()),
            document_filename: None, // Will use document_id as fallback
            sender_name: None,       // WhatsApp doesn't use sender name
            attachments: vec![],
        };

        match state.notification_orchestrator.execute(command).await {
            Ok(result) => {
                tracing::info!(
                    "WhatsApp notification sent to {}: {:?}",
                    whatsapp.phone,
                    result.status
                );
            }
            Err(e) => {
                tracing::error!("Failed to send WhatsApp notification: {}", e);
            }
        }
    }

    // Send Email notification
    if let Some(ref email) = config.email {
        let subject = email
            .subject
            .clone()
            .unwrap_or_else(|| format!("Documento {} disponible", document_id));
        let body = email
            .body
            .clone()
            .unwrap_or_else(|| format!("Tu documento {} está listo para descargar.", document_id));

        let command = SendNotificationCommand {
            tenant_id: tenant_id.to_string(),
            channel: NotificationChannel::Email,
            recipient: email.to.clone(),
            subject: Some(subject),
            template_id: None,
            template_vars: json!({ "body": body }),
            priority: NotificationPriority::Normal,
            document_id: Some(document_id.to_string()),
            document_filename: None, // Will use document_id as fallback
            sender_name: None,       // Uses default SMTP_FROM_NAME
            attachments: vec![],
        };

        match state.notification_orchestrator.execute(command).await {
            Ok(result) => {
                tracing::info!(
                    "Email notification sent to {}: {:?}",
                    email.to,
                    result.status
                );
            }
            Err(e) => {
                tracing::error!("Failed to send email notification: {}", e);
            }
        }
    }
}
