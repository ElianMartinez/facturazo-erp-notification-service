//! ERP Report API Handler
//!
//! Handles HTTP endpoints for ERP report generation (PDF, Excel, CSV)

use actix_web::{web, HttpRequest, HttpResponse};
use serde_json::json;
use std::path::PathBuf;
use std::time::Instant;
use uuid::Uuid;

use super::error::ApiResult;
use super::middleware::auth::extract_tenant_user_or_default;
use super::state::ApiState;
use crate::templates::erp_report_models::{ErpReportPayload, OutputFormat};
use crate::templates::templates::ErpReportTemplate;
use crate::templates::TypstTemplate;

/// Generate ERP report synchronously
/// POST /api/v1/reports/generate/sync
pub async fn generate_erp_report_sync(
    req: HttpRequest,
    payload: web::Json<ErpReportPayload>,
    state: web::Data<ApiState>,
) -> ApiResult<HttpResponse> {
    let start = Instant::now();
    let mut payload = payload.into_inner();

    // Extract tenant and user info from request
    let (tenant_id, user_id) = extract_tenant_user_or_default(&req);

    // Update payload with tenant/user from request if not set
    if payload.tenant_id == 0 {
        payload.tenant_id = tenant_id;
    }
    if payload.user_id == 0 {
        payload.user_id = user_id;
    }

    // Check rate limit
    let rate_limit_key = format!("report:{}:{}", payload.tenant_id, payload.user_id);
    if state.rate_limiter.check_key(&rate_limit_key).is_err() {
        return Ok(HttpResponse::TooManyRequests().json(json!({
            "error": "Rate limit exceeded",
            "retry_after": 60
        })));
    }

    // Note: The /sync endpoint always processes synchronously
    // The caller (Core-Service) decides whether to use sync or async endpoint
    // based on the number of records (threshold: 1000)

    // Generate report ID
    let report_id = Uuid::new_v4().to_string();

    tracing::info!(
        "Generating ERP report sync: code={}, variant={:?}, format={:?}, records={}",
        payload.report.code,
        payload.report.variant,
        payload.output.format,
        payload.data.total_records
    );

    // Generate based on output format
    let result = match payload.output.format {
        OutputFormat::Pdf => generate_pdf(&payload).await,
        OutputFormat::Xlsx => generate_excel(&payload).await,
        OutputFormat::Csv => generate_csv(&payload).await,
        OutputFormat::Html => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "error": "HTML format not yet implemented for reports"
            })));
        }
    };

    match result {
        Ok((bytes, mime_type, extension)) => {
            let processing_time_ms = start.elapsed().as_millis() as u64;

            // Store in S3 if configured
            let storage_url = if !state.config.s3_bucket_documents.is_empty() {
                let file_key = format!(
                    "reports/{}/{}/{}.{}",
                    payload.tenant_id,
                    chrono::Utc::now().format("%Y/%m/%d"),
                    report_id,
                    extension
                );

                match state
                    .s3_client
                    .put_object(
                        &state.config.s3_bucket_documents,
                        &file_key,
                        bytes.clone(),
                        mime_type,
                    )
                    .await
                {
                    Ok(_) => {
                        // Generate presigned URL
                        match state
                            .s3_client
                            .create_presigned_url(
                                &state.config.s3_bucket_documents,
                                &file_key,
                                3600, // 1 hour
                            )
                            .await
                        {
                            Ok(url) => Some(url),
                            Err(e) => {
                                tracing::warn!("Failed to create presigned URL: {}", e);
                                None
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to upload to S3: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            tracing::info!(
                "ERP report generated: id={}, format={:?}, size={} bytes, time={}ms",
                report_id,
                payload.output.format,
                bytes.len(),
                processing_time_ms
            );

            // Return response with download URL or inline bytes
            if let Some(url) = storage_url {
                Ok(HttpResponse::Ok().json(json!({
                    "success": true,
                    "reportId": report_id,
                    "status": "completed",
                    "downloadUrl": url,
                    "format": format!("{:?}", payload.output.format).to_lowercase(),
                    "mimeType": mime_type,
                    "fileSize": bytes.len(),
                    "processingTimeMs": processing_time_ms
                })))
            } else {
                // Return bytes inline (base64 encoded)
                use base64::Engine;
                let base64_content = base64::engine::general_purpose::STANDARD.encode(&bytes);

                Ok(HttpResponse::Ok().json(json!({
                    "success": true,
                    "reportId": report_id,
                    "status": "completed",
                    "content": base64_content,
                    "format": format!("{:?}", payload.output.format).to_lowercase(),
                    "mimeType": mime_type,
                    "fileSize": bytes.len(),
                    "processingTimeMs": processing_time_ms
                })))
            }
        }
        Err(e) => {
            tracing::error!("Failed to generate ERP report: {:?}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "error": "Failed to generate report",
                "details": e.to_string()
            })))
        }
    }
}

/// Generate ERP report and stream bytes directly
/// POST /api/v1/reports/generate/stream
/// Returns binary content directly with appropriate headers
pub async fn generate_erp_report_stream(
    req: HttpRequest,
    payload: web::Json<ErpReportPayload>,
    state: web::Data<ApiState>,
) -> ApiResult<HttpResponse> {
    let start = Instant::now();
    let mut payload = payload.into_inner();

    // Extract tenant and user info from request
    let (tenant_id, user_id) = extract_tenant_user_or_default(&req);

    if payload.tenant_id == 0 {
        payload.tenant_id = tenant_id;
    }
    if payload.user_id == 0 {
        payload.user_id = user_id;
    }

    // Check rate limit
    let rate_limit_key = format!("report:{}:{}", payload.tenant_id, payload.user_id);
    if state.rate_limiter.check_key(&rate_limit_key).is_err() {
        return Ok(HttpResponse::TooManyRequests().json(json!({
            "error": "Rate limit exceeded",
            "retry_after": 60
        })));
    }

    let report_id = Uuid::new_v4().to_string();

    tracing::info!(
        "Generating ERP report stream: code={}, variant={:?}, format={:?}, records={}",
        payload.report.code,
        payload.report.variant,
        payload.output.format,
        payload.data.total_records
    );

    // Generate based on output format
    let result = match payload.output.format {
        OutputFormat::Pdf => generate_pdf(&payload).await,
        OutputFormat::Xlsx => generate_excel(&payload).await,
        OutputFormat::Csv => generate_csv(&payload).await,
        OutputFormat::Html => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "error": "HTML format not yet implemented for reports"
            })));
        }
    };

    match result {
        Ok((bytes, mime_type, extension)) => {
            let processing_time_ms = start.elapsed().as_millis() as u64;

            // Generate filename
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let filename = format!(
                "{}_{}.{}",
                payload.report.code.to_lowercase().replace('_', "-"),
                timestamp,
                extension
            );

            tracing::info!(
                "ERP report stream generated: id={}, format={:?}, size={} bytes, time={}ms",
                report_id,
                payload.output.format,
                bytes.len(),
                processing_time_ms
            );

            // Return binary stream directly
            Ok(HttpResponse::Ok()
                .content_type(mime_type)
                .append_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"{}\"", filename),
                ))
                .append_header(("Content-Length", bytes.len().to_string()))
                .append_header(("X-Report-Id", report_id))
                .append_header(("X-Processing-Time-Ms", processing_time_ms.to_string()))
                .body(bytes))
        }
        Err(e) => {
            tracing::error!("Failed to generate ERP report stream: {:?}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "error": "Failed to generate report",
                "details": e.to_string()
            })))
        }
    }
}

/// Queue ERP report for async generation
/// POST /api/v1/reports/generate/async
pub async fn generate_erp_report_async(
    req: HttpRequest,
    payload: web::Json<ErpReportPayload>,
    state: web::Data<ApiState>,
) -> ApiResult<HttpResponse> {
    let mut payload = payload.into_inner();

    // Extract tenant and user info
    let (tenant_id, user_id) = extract_tenant_user_or_default(&req);
    if payload.tenant_id == 0 {
        payload.tenant_id = tenant_id;
    }
    if payload.user_id == 0 {
        payload.user_id = user_id;
    }

    // Check rate limit
    let rate_limit_key = format!("report:{}:{}", payload.tenant_id, payload.user_id);
    if state.rate_limiter.check_key(&rate_limit_key).is_err() {
        return Ok(HttpResponse::TooManyRequests().json(json!({
            "error": "Rate limit exceeded",
            "retry_after": 60
        })));
    }

    let report_id = Uuid::new_v4().to_string();
    let estimated_time = payload.estimate_processing_time_secs();

    tracing::info!(
        "Queueing ERP report async: id={}, code={}, records={}, estimated_time={}s",
        report_id,
        payload.report.code,
        payload.data.total_records,
        estimated_time
    );

    // Clone for async task
    let state_clone = state.clone();
    let report_id_clone = report_id.clone();

    // Spawn background task
    tokio::spawn(async move {
        match process_report_async(state_clone, payload, report_id_clone.clone()).await {
            Ok(url) => {
                tracing::info!("Async report {} completed: {}", report_id_clone, url);
            }
            Err(e) => {
                tracing::error!("Async report {} failed: {}", report_id_clone, e);
            }
        }
    });

    Ok(HttpResponse::Accepted().json(json!({
        "success": true,
        "reportId": report_id,
        "status": "processing",
        "estimatedTimeSeconds": estimated_time,
        "statusUrl": format!("/api/v1/reports/{}/status", report_id)
    })))
}

/// Get report status
/// GET /api/v1/reports/{id}/status
pub async fn get_report_status(
    _req: HttpRequest,
    path: web::Path<String>,
    _state: web::Data<ApiState>,
) -> ApiResult<HttpResponse> {
    let report_id = path.into_inner();

    // In production, this would check a cache/database for status
    Ok(HttpResponse::Ok().json(json!({
        "reportId": report_id,
        "status": "unknown",
        "message": "Status tracking requires cache/database implementation"
    })))
}

/// Download report
/// GET /api/v1/reports/{id}/download
pub async fn download_report(
    _req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ApiState>,
) -> ApiResult<HttpResponse> {
    let report_id = path.into_inner();

    // Try to find the report in S3 with various extensions
    for ext in ["pdf", "xlsx", "csv"] {
        // Try finding by date pattern (current month)
        let now = chrono::Utc::now();
        let key = format!("reports/1/{}/{}.{}", now.format("%Y/%m/%d"), report_id, ext);

        if let Ok(url) = state
            .s3_client
            .create_presigned_url(&state.config.s3_bucket_documents, &key, 3600)
            .await
        {
            return Ok(HttpResponse::Found()
                .append_header(("Location", url))
                .finish());
        }
    }

    Ok(HttpResponse::NotFound().json(json!({
        "error": "Report not found",
        "reportId": report_id
    })))
}

// ============================================
// Internal helper functions
// ============================================

/// Generate PDF from ERP report payload
async fn generate_pdf(
    payload: &ErpReportPayload,
) -> anyhow::Result<(Vec<u8>, &'static str, &'static str)> {
    use crate::infrastructure::generators::typst_generator::TypstGenerator;

    // Create template and generate Typst content
    let template = ErpReportTemplate::new();
    let json_value = serde_json::to_value(payload)?;
    template.validate(&json_value)?;
    let typst_content = template.generate(&json_value)?;

    // Generate PDF using Typst
    let work_dir = PathBuf::from("./temp");
    std::fs::create_dir_all(&work_dir)?;
    let generator = TypstGenerator::new(work_dir);
    let pdf_bytes = generator.generate_pdf(&typst_content).await?;

    Ok((pdf_bytes, "application/pdf", "pdf"))
}

/// Generate Excel from ERP report payload
async fn generate_excel(
    payload: &ErpReportPayload,
) -> anyhow::Result<(Vec<u8>, &'static str, &'static str)> {
    use crate::infrastructure::generators::erp_report_excel_generator::ErpReportExcelGenerator;

    let generator = ErpReportExcelGenerator::new();
    let excel_bytes = generator.generate(payload).await?;

    Ok((
        excel_bytes,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "xlsx",
    ))
}

/// Generate CSV from ERP report payload
async fn generate_csv(
    payload: &ErpReportPayload,
) -> anyhow::Result<(Vec<u8>, &'static str, &'static str)> {
    use crate::infrastructure::generators::erp_report_csv_generator::ErpReportCsvGenerator;

    let generator = ErpReportCsvGenerator::new();
    let csv_bytes = generator.generate(payload).await?;

    Ok((csv_bytes, "text/csv; charset=utf-8", "csv"))
}

/// Process report asynchronously
async fn process_report_async(
    state: web::Data<ApiState>,
    payload: ErpReportPayload,
    report_id: String,
) -> anyhow::Result<String> {
    let start = Instant::now();

    // Generate based on format
    let (bytes, mime_type, extension) = match payload.output.format {
        OutputFormat::Pdf => generate_pdf(&payload).await?,
        OutputFormat::Xlsx => generate_excel(&payload).await?,
        OutputFormat::Csv => generate_csv(&payload).await?,
        OutputFormat::Html => {
            return Err(anyhow::anyhow!("HTML format not implemented"));
        }
    };

    // Upload to S3
    let file_key = format!(
        "reports/{}/{}/{}.{}",
        payload.tenant_id,
        chrono::Utc::now().format("%Y/%m/%d"),
        report_id,
        extension
    );

    state
        .s3_client
        .put_object(
            &state.config.s3_bucket_documents,
            &file_key,
            bytes.clone(),
            mime_type,
        )
        .await?;

    // Generate presigned URL
    let url = state
        .s3_client
        .create_presigned_url(
            &state.config.s3_bucket_documents,
            &file_key,
            86400, // 24 hours
        )
        .await?;

    let processing_time = start.elapsed().as_millis();
    tracing::info!(
        "Async report {} completed: size={} bytes, time={}ms",
        report_id,
        bytes.len(),
        processing_time
    );

    // Handle delivery if configured
    if payload.delivery.is_some() {
        if let Err(e) = deliver_report(&payload, &bytes, extension).await {
            tracing::warn!("Delivery failed for report {}: {}", report_id, e);
        }
    }

    Ok(url)
}

/// Deliver report via configured channels
async fn deliver_report(
    payload: &ErpReportPayload,
    bytes: &[u8],
    extension: &str,
) -> anyhow::Result<()> {
    use crate::infrastructure::notifications::ErpReportNotifierBuilder;
    use crate::templates::erp_report_models::DeliveryMethod;

    let delivery = payload
        .delivery
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No delivery options configured"))?;

    // Build notifier with available services
    let mut builder = ErpReportNotifierBuilder::new();

    // Configure email if needed
    if delivery.email.is_some() {
        if let (Ok(host), Ok(user), Ok(pass)) = (
            std::env::var("SMTP_HOST"),
            std::env::var("SMTP_USER"),
            std::env::var("SMTP_PASSWORD"),
        ) {
            let port = std::env::var("SMTP_PORT")
                .unwrap_or_else(|_| "587".to_string())
                .parse()
                .unwrap_or(587);

            let from_email =
                std::env::var("SMTP_FROM_EMAIL").unwrap_or_else(|_| "noreply@facturazo.com".into());
            let from_name =
                std::env::var("SMTP_FROM_NAME").unwrap_or_else(|_| "Facturazo ERP".into());

            builder = builder.with_email(host, port, user, pass, from_email, from_name, true);
        }
    }

    // Configure WhatsApp if needed
    if delivery.whatsapp.is_some() {
        if let (Ok(base_url), Ok(instance), Ok(api_key)) = (
            std::env::var("EVOLUTION_API_URL"),
            std::env::var("EVOLUTION_INSTANCE"),
            std::env::var("EVOLUTION_API_KEY"),
        ) {
            builder = builder.with_whatsapp(base_url, api_key, instance);
        }
    }

    let notifier = builder.build();

    // Deliver based on method using the unified deliver_report method
    match delivery.method {
        DeliveryMethod::Email | DeliveryMethod::WhatsApp => {
            // Create a modified payload with file_name hint
            let mut delivery_payload = payload.clone();
            let file_name = format!(
                "{}_{}.{}",
                payload.report.code.to_lowercase().replace('_', "-"),
                chrono::Utc::now().format("%Y%m%d"),
                extension
            );

            // Set file_name in output options if possible
            delivery_payload.output.file_name = Some(file_name);

            notifier
                .deliver_report(&delivery_payload, bytes.to_vec())
                .await?;
        }
        _ => {} // Download/View don't need delivery
    }

    Ok(())
}
