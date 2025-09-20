use actix_web::{web, HttpResponse, HttpRequest, HttpMessage};
use serde_json::json;
use uuid::Uuid;
use chrono::Utc;
use flate2::read::GzDecoder;
use std::io::Read;

use crate::models::{
    DocumentRequest, DocumentResponse, DocumentStatus, DocumentType, Priority
};
use crate::generators::{PdfGenerator, ExcelGenerator};
use super::state::ApiState;
use super::error::ApiResult;

/// Generate document synchronously (small documents only)
pub async fn generate_sync(
    req: HttpRequest,
    mut data: web::Json<DocumentRequest>,
    state: web::Data<ApiState>,
) -> ApiResult<HttpResponse> {
    // Extract tenant and user info
    let (tenant_id, user_id) = crate::api::middleware::auth::extract_tenant_user(&req)
        .unwrap_or((1, 1));

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
        DocumentType::Invoice => {
            generate_invoice_sync(&data.into_inner(), &state).await
        },
        DocumentType::Report if data_size < 100_000 => { // Small reports only
            generate_report_sync(&data.into_inner(), &state).await
        },
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
        },
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
    let (tenant_id, user_id) = extract_tenant_user(&req);

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

    let (_tenant_id, user_id) = crate::api::middleware::auth::extract_tenant_user(&req)
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
    let content_encoding = req.headers()
        .get("Content-Encoding")
        .and_then(|h| h.to_str().ok());

    let decompressed = match content_encoding {
        Some("gzip") => {
            let mut decoder = GzDecoder::new(&body[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            decompressed
        },
        _ => body.to_vec(),
    };

    // Upload to S3 temp bucket
    let file_key = format!("uploads/{}/{}.json", user_id, Uuid::new_v4());
    state.s3_client.put_object(
        &state.config.s3_bucket_temp,
        &file_key,
        decompressed,
        "application/json",
    ).await?;

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
    let (_tenant_id, _user_id) = extract_tenant_user(&req);

    // For now, construct the S3 key directly
    let key = format!("documents/{}.pdf", document_id);

    // Generate presigned URL
    let presigned = state.s3_client.create_presigned_url(
        &state.config.s3_bucket_documents,
        &key,
        3600, // 1 hour
    ).await?;

    Ok(HttpResponse::Found()
        .append_header(("Location", presigned))
        .finish())
}

// Helper functions

async fn generate_invoice_sync(
    request: &DocumentRequest,
    state: &ApiState,
) -> anyhow::Result<String> {
    // Generate PDF using the generic generator with template
    let pdf_generator = PdfGenerator::new(state.template_manager.clone());
    let pdf_bytes = pdf_generator.generate(&request.template_id, request.data.clone()).await?;

    // Upload to S3
    let org_id = request.metadata.organization_id.clone()
        .unwrap_or_else(|| format!("tenant_{}", request.metadata.tenant_id));
    let key = format!("invoices/{}/{}.pdf", org_id, request.id);
    let url = state.s3_client.put_object(
        &state.config.s3_bucket_documents,
        &key,
        pdf_bytes,
        "application/pdf",
    ).await?;

    Ok(url)
}

async fn generate_report_sync(
    request: &DocumentRequest,
    state: &ApiState,
) -> anyhow::Result<String> {
    // Generate Excel using the generic generator
    let excel_generator = ExcelGenerator::new();
    let excel_bytes = excel_generator.generate(request.data.clone()).await?;

            // Upload to S3
            let org_id = request.metadata.organization_id.clone()
                .unwrap_or_else(|| format!("tenant_{}", request.metadata.tenant_id));
            let key = format!("reports/{}/{}.xlsx", org_id, request.id);
            let url = state.s3_client.put_object(
                &state.config.s3_bucket_documents,
                &key,
                excel_bytes,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ).await?;

            Ok(url)
}

pub fn extract_tenant_user(req: &HttpRequest) -> (i64, i64) {
    // Try to get from headers or extensions (set by auth middleware)
    let tenant_id = req.headers()
        .get("X-Tenant-Id")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let user_id = req.headers()
        .get("X-User-Id")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    // Alternative: get from request extensions if set by auth middleware
    if let Some(auth_info) = req.extensions().get::<AuthInfo>() {
        return (auth_info.tenant_id, auth_info.user_id);
    }

    (tenant_id, user_id)
}

#[derive(Clone, Debug)]
pub struct AuthInfo {
    pub tenant_id: i64,
    pub user_id: i64,
}

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

async fn extract_tenant_user_ids(_req: &HttpRequest) -> (i64, i64) {
    // Mock implementation for now
    (1, 1)
}

// Process document asynchronously
async fn process_document_async(
    state: web::Data<ApiState>,
    request: DocumentRequest,
) -> anyhow::Result<()> {
    let start = std::time::Instant::now();

    // Generate document based on type
    let (bytes, filename) = match request.document_type {
        DocumentType::Invoice => {
            let pdf_generator = PdfGenerator::new(state.template_manager.clone());
            let pdf_bytes = pdf_generator.generate(&request.template_id, request.data.clone()).await?;
            (pdf_bytes, format!("invoice_{}.pdf", request.id))
        },
        DocumentType::Report => {
            let excel_generator = ExcelGenerator::new();
            let excel_bytes = excel_generator.generate(request.data.clone()).await?;
            (excel_bytes, format!("report_{}.xlsx", request.id))
        },
        _ => {
            // For other types, try to use template
            let pdf_generator = PdfGenerator::new(state.template_manager.clone());
            let pdf_bytes = pdf_generator.generate(&request.template_id, request.data.clone()).await?;
            (pdf_bytes, format!("document_{}.pdf", request.id))
        }
    };

    // Upload to S3
    let s3_key = format!("{}/{}/{}",
        request.metadata.organization_id.unwrap_or_else(|| "default".to_string()),
        request.metadata.tenant_id,
        filename
    );

    state.s3_client.put_object(
        &state.config.s3_bucket_documents,
        &s3_key,
        bytes,
        "application/pdf",
    ).await?;

    let processing_time = start.elapsed().as_millis() as i64;
    tracing::info!("Document {} processed in {}ms", request.id, processing_time);

    Ok(())
}