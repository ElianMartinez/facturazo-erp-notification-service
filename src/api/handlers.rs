use actix_web::{web, HttpResponse, HttpRequest, Result};
use serde_json::json;
use uuid::Uuid;
use chrono::Utc;
use bytes::Bytes;
use flate2::read::GzDecoder;
use std::io::Read;
use rdkafka::producer::FutureRecord;
use rdkafka::util::Timeout;

use crate::models::{
    DocumentRequest, DocumentResponse, DocumentStatus, DocumentType,
    InvoiceRequest, ReportRequest, DataSource, CompressionFormat, Priority
};
use crate::generators::{PdfGenerator, ExcelGenerator};
use super::state::ApiState;

/// Generate document synchronously (small documents only)
pub async fn generate_sync(
    req: HttpRequest,
    data: web::Json<DocumentRequest>,
    state: web::Data<ApiState>,
) -> Result<HttpResponse> {
    // Extract user ID for rate limiting
    let user_id = extract_user_id(&req);

    // Check rate limit
    if let Err(_) = state.rate_limiter.check_key(&user_id) {
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

    // Generate document based on type
    let result = match data.document_type {
        DocumentType::Invoice => {
            generate_invoice_sync(&data.into_inner(), &state).await
        },
        DocumentType::Report if data_size < 100_000 => { // Small reports only
            generate_report_sync(&data.into_inner(), &state).await
        },
        _ => {
            // All other types go to async queue
            return generate_async(req, web::Json(data.into_inner()), state).await;
        }
    };

    match result {
        Ok(document_url) => {
            let response = DocumentResponse {
                id: data.id,
                status: DocumentStatus::Completed,
                url: Some(document_url),
                error: None,
                processing_time_ms: start.elapsed().as_millis() as u64,
                created_at: Utc::now(),
                expires_at: None,
            };

            // Save to database
            save_document_metadata(&state.db, &response).await?;

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
    data: web::Json<DocumentRequest>,
    state: web::Data<ApiState>,
) -> Result<HttpResponse> {
    let user_id = extract_user_id(&req);

    // Check rate limit
    if let Err(_) = state.rate_limiter.check_key(&user_id) {
        return Ok(HttpResponse::TooManyRequests().json(json!({
            "error": "Rate limit exceeded",
            "retry_after": 60
        })));
    }

    // Determine topic based on priority
    let topic = match data.priority {
        Priority::High => &state.config.kafka_topic_priority,
        _ => &state.config.kafka_topic_bulk,
    };

    // Serialize request
    let payload = serde_json::to_vec(&data.into_inner())?;

    // Send to Kafka
    let delivery = state.kafka_producer.send(
        FutureRecord::to(topic)
            .key(&data.id.to_string())
            .payload(&payload),
        Timeout::After(std::time::Duration::from_secs(5)),
    ).await;

    match delivery {
        Ok(_) => {
            // Create initial response
            let response = DocumentResponse {
                id: data.id,
                status: DocumentStatus::Queued,
                url: None,
                error: None,
                processing_time_ms: 0,
                created_at: Utc::now(),
                expires_at: None,
            };

            // Save to database
            save_document_metadata(&state.db, &response).await?;

            Ok(HttpResponse::Accepted().json(json!({
                "id": data.id,
                "status": "queued",
                "estimated_time_seconds": estimate_processing_time(&data),
                "status_url": format!("/api/v1/documents/{}/status", data.id)
            })))
        },
        Err(e) => {
            tracing::error!("Failed to queue document: {:?}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "error": "Failed to queue document",
                "details": e.to_string()
            })))
        }
    }
}

/// Handle large file upload
pub async fn upload_data(
    req: HttpRequest,
    mut payload: web::Payload,
    state: web::Data<ApiState>,
) -> Result<HttpResponse> {
    use futures::StreamExt;

    let user_id = extract_user_id(&req);

    // Check rate limit
    if let Err(_) = state.rate_limiter.check_key(&user_id) {
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

/// Get document status
pub async fn get_status(
    path: web::Path<Uuid>,
    state: web::Data<ApiState>,
) -> Result<HttpResponse> {
    let document_id = path.into_inner();

    // Query database
    let status = get_document_status(&state.db, document_id).await?;

    match status {
        Some(doc) => Ok(HttpResponse::Ok().json(doc)),
        None => Ok(HttpResponse::NotFound().json(json!({
            "error": "Document not found"
        }))),
    }
}

/// Download document
pub async fn download_document(
    path: web::Path<Uuid>,
    state: web::Data<ApiState>,
) -> Result<HttpResponse> {
    let document_id = path.into_inner();

    // Get document metadata
    let doc = get_document_status(&state.db, document_id).await?;

    match doc {
        Some(doc) if doc.status == DocumentStatus::Completed => {
            if let Some(url) = doc.url {
                // Generate presigned URL
                let presigned = state.s3_client.create_presigned_url(
                    &state.config.s3_bucket_documents,
                    &url,
                    3600, // 1 hour
                ).await?;

                Ok(HttpResponse::Found()
                    .append_header(("Location", presigned))
                    .finish())
            } else {
                Ok(HttpResponse::NotFound().json(json!({
                    "error": "Document URL not found"
                })))
            }
        },
        Some(_) => Ok(HttpResponse::BadRequest().json(json!({
            "error": "Document not ready"
        }))),
        None => Ok(HttpResponse::NotFound().json(json!({
            "error": "Document not found"
        }))),
    }
}

// Helper functions

async fn generate_invoice_sync(
    request: &DocumentRequest,
    state: &ApiState,
) -> anyhow::Result<String> {
    let invoice_req: InvoiceRequest = serde_json::from_value(request.data.clone())?;

    // Generate PDF
    let pdf_generator = PdfGenerator::new(state.template_manager.clone());
    let pdf_bytes = pdf_generator.generate_invoice(&invoice_req).await?;

    // Upload to S3
    let key = format!("invoices/{}/{}.pdf", request.metadata.organization_id, request.id);
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
    let report_req: ReportRequest = serde_json::from_value(request.data.clone())?;

    // For sync, only handle inline data
    match report_req.data_source {
        DataSource::Inline { ref rows } if rows.len() < 1000 => {
            // Generate Excel
            let excel_generator = ExcelGenerator::new();
            let excel_bytes = excel_generator.generate_report(&report_req, rows.clone()).await?;

            // Upload to S3
            let key = format!("reports/{}/{}.xlsx", request.metadata.organization_id, request.id);
            let url = state.s3_client.put_object(
                &state.config.s3_bucket_documents,
                &key,
                excel_bytes,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ).await?;

            Ok(url)
        },
        _ => Err(anyhow::anyhow!("Report too large for sync processing")),
    }
}

fn extract_user_id(req: &HttpRequest) -> String {
    // Try to get from JWT claim, header, or default
    req.headers()
        .get("X-User-Id")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("anonymous")
        .to_string()
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

async fn save_document_metadata(
    db: &PgPool,
    response: &DocumentResponse,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO documents (id, status, url, error, processing_time_ms, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (id)
        DO UPDATE SET
            status = EXCLUDED.status,
            url = EXCLUDED.url,
            error = EXCLUDED.error,
            processing_time_ms = EXCLUDED.processing_time_ms,
            updated_at = NOW()
        "#,
        response.id,
        response.status.to_string(),
        response.url,
        response.error,
        response.processing_time_ms as i64,
        response.created_at
    )
    .execute(db)
    .await?;

    Ok(())
}

async fn get_document_status(
    db: &PgPool,
    document_id: Uuid,
) -> anyhow::Result<Option<DocumentResponse>> {
    let record = sqlx::query!(
        r#"
        SELECT id, status, url, error, processing_time_ms, created_at, expires_at
        FROM documents
        WHERE id = $1
        "#,
        document_id
    )
    .fetch_optional(db)
    .await?;

    Ok(record.map(|r| DocumentResponse {
        id: r.id,
        status: r.status.parse().unwrap_or(DocumentStatus::Failed),
        url: r.url,
        error: r.error,
        processing_time_ms: r.processing_time_ms.unwrap_or(0) as u64,
        created_at: r.created_at,
        expires_at: r.expires_at,
    }))
}