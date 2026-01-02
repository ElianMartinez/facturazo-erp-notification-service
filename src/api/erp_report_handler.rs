//! ERP Report API Handler
//!
//! Handles HTTP endpoints for ERP report generation (PDF, Excel, CSV)
//! with adaptive concurrency control to prevent server overload.

use actix_web::{web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::time::Instant;
use tracing::Instrument;
use uuid::Uuid;

use super::error::ApiResult;
use super::middleware::auth::extract_tenant_user_or_default;
use super::state::ApiState;
use crate::infrastructure::resilience::{ConcurrencyError, JobClassification, JobType};
use crate::templates::erp_report_models::{ErpReportPayload, OutputFormat};
use crate::templates::templates::ErpReportTemplate;
use crate::templates::TypstTemplate;

// ============================================
// Job Status Tracking
// ============================================

/// Status of an async report job
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatus {
    pub report_id: String,
    pub status: JobState,
    pub download_url: Option<String>,
    pub file_size: Option<u64>,
    pub processing_time_ms: Option<u64>,
    pub error: Option<String>,
    pub details: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub format: Option<String>,
    pub mime_type: Option<String>,
}

/// Job state enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Processing,
    Completed,
    Failed,
}

impl JobStatus {
    /// Create a new job in processing state
    pub fn new_processing(report_id: String) -> Self {
        Self {
            report_id,
            status: JobState::Processing,
            download_url: None,
            file_size: None,
            processing_time_ms: None,
            error: None,
            details: None,
            created_at: Utc::now(),
            completed_at: None,
            format: None,
            mime_type: None,
        }
    }

    /// Mark job as completed
    pub fn complete(
        mut self,
        download_url: Option<String>,
        file_size: u64,
        processing_time_ms: u64,
        format: String,
        mime_type: String,
    ) -> Self {
        self.status = JobState::Completed;
        self.download_url = download_url;
        self.file_size = Some(file_size);
        self.processing_time_ms = Some(processing_time_ms);
        self.completed_at = Some(Utc::now());
        self.format = Some(format);
        self.mime_type = Some(mime_type);
        self
    }

    /// Mark job as failed
    pub fn fail(mut self, error: String, details: Option<String>) -> Self {
        self.status = JobState::Failed;
        self.error = Some(error);
        self.details = details;
        self.completed_at = Some(Utc::now());
        self
    }
}

/// Cache key prefix for job status
const JOB_STATUS_CACHE_PREFIX: &str = "job_status:";

/// TTL for job status in cache (24 hours)
const JOB_STATUS_TTL_SECS: u64 = 86400;

/// Maximum file size to attach directly to email (10 MB)
/// Files larger than this will be uploaded to S3 and a download link sent instead
const MAX_ATTACHMENT_SIZE_BYTES: usize = 10 * 1024 * 1024; // 10 MB

// ============================================
// Job Classification Helpers
// ============================================

/// Classify a report job for concurrency control
fn classify_job(payload: &ErpReportPayload) -> JobClassification {
    let job_type = match payload.output.format {
        OutputFormat::Pdf => JobType::Pdf,
        OutputFormat::Xlsx => JobType::Excel,
        OutputFormat::Csv => JobType::Csv,
        OutputFormat::Html => JobType::Pdf, // Treat HTML as PDF for now
    };

    // Convert i32 to usize safely
    let record_count = payload.data.total_records.max(0) as usize;
    JobClassification::new(job_type, record_count)
}

/// Convert concurrency error to HTTP response
fn concurrency_error_response(error: ConcurrencyError) -> HttpResponse {
    match error {
        ConcurrencyError::ResourceExhausted { reason } => {
            tracing::warn!("Report rejected - resources exhausted: {}", reason);
            HttpResponse::ServiceUnavailable().json(json!({
                "error": "server_overloaded",
                "message": "Server is currently overloaded. Please try again later.",
                "details": reason,
                "retry_after": 30
            }))
        }
        ConcurrencyError::QueueFull { job_type } => {
            tracing::warn!("Report rejected - queue full for {:?}", job_type);
            HttpResponse::ServiceUnavailable().json(json!({
                "error": "queue_full",
                "message": "Too many reports in queue. Please try again later.",
                "job_type": format!("{:?}", job_type),
                "retry_after": 60
            }))
        }
        ConcurrencyError::Timeout => {
            tracing::warn!("Report rejected - timeout waiting for resources");
            HttpResponse::GatewayTimeout().json(json!({
                "error": "timeout",
                "message": "Timeout waiting for available resources",
                "retry_after": 30
            }))
        }
        ConcurrencyError::Backpressure => HttpResponse::ServiceUnavailable().json(json!({
            "error": "backpressure",
            "message": "System is under heavy load. Please try again later.",
            "retry_after": 60
        })),
        _ => HttpResponse::InternalServerError().json(json!({
            "error": "concurrency_error",
            "message": "Failed to acquire resources for report generation"
        })),
    }
}

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

    // Generate report ID and get correlation_id for distributed tracing
    let report_id = Uuid::new_v4().to_string();
    let correlation_id = payload
        .correlation_id
        .clone()
        .unwrap_or_else(|| report_id.clone());

    // Classify job for concurrency control
    let classification = classify_job(&payload);
    tracing::info!(
        "Job classification: type={:?}, size={:?}, records={}, estimated_ram={}MB",
        classification.job_type,
        classification.job_size,
        classification.record_count,
        classification.estimated_ram_mb
    );

    // Acquire concurrency permit
    let permit = match state
        .concurrency_controller
        .acquire_permit(&report_id, classification, payload.tenant_id as u64)
        .await
    {
        Ok(permit) => permit,
        Err(e) => {
            return Ok(concurrency_error_response(e));
        }
    };

    // Clone values needed outside the async block
    let report_id_for_completion = report_id.clone();
    let state_for_completion = state.clone();

    // Create a span for the entire request - all logs will include correlation_id
    let request_span = tracing::info_span!(
        "erp_report_sync",
        correlation_id = %correlation_id,
        report_id = %report_id,
        tenant_id = %payload.tenant_id,
        report_code = %payload.report.code
    );

    // Process within the span so all logs have correlation_id
    let result = async move {
        tracing::info!(
            "Generating ERP report sync: variant={:?}, format={:?}, records={}",
            payload.report.variant,
            payload.output.format,
            payload.data.total_records
        );

        // Debug: Log output options to verify deserialization
        tracing::debug!(
            "Output options: page_size={:?}, orientation={:?}, scale={}, margins={:?}, include_header={}, include_footer={}, show_logo={}",
            payload.output.page_size,
            payload.output.orientation,
            payload.output.scale,
            payload.output.margins,
            payload.output.include_header,
            payload.output.include_footer,
            payload.output.show_logo
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
    .instrument(request_span)
    .await;

    // Mark job as completed/failed and release permit
    let success = result
        .as_ref()
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    state_for_completion
        .concurrency_controller
        .complete_job(&report_id_for_completion, success);

    // Permit is dropped here, releasing the semaphore slot
    drop(permit);

    result
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

    // Generate report ID and get correlation_id for distributed tracing
    let report_id = Uuid::new_v4().to_string();
    let correlation_id = payload
        .correlation_id
        .clone()
        .unwrap_or_else(|| report_id.clone());

    // Classify job for concurrency control
    let classification = classify_job(&payload);

    // Acquire concurrency permit
    let permit = match state
        .concurrency_controller
        .acquire_permit(&report_id, classification, payload.tenant_id as u64)
        .await
    {
        Ok(permit) => permit,
        Err(e) => {
            return Ok(concurrency_error_response(e));
        }
    };

    // Clone values needed outside the async block
    let report_id_for_completion = report_id.clone();
    let state_for_completion = state.clone();

    // Create a span for the entire request - all logs will include correlation_id
    let request_span = tracing::info_span!(
        "erp_report_stream",
        correlation_id = %correlation_id,
        report_id = %report_id,
        tenant_id = %payload.tenant_id,
        report_code = %payload.report.code
    );

    let result = async move {
        tracing::info!(
            "Generating ERP report stream: variant={:?}, format={:?}, records={}",
            payload.report.variant,
            payload.output.format,
            payload.data.total_records
        );

        // Debug: Log output options to verify deserialization
        tracing::debug!(
            "Stream output options: page_size={:?}, orientation={:?}, scale={}, margins={:?}",
            payload.output.page_size,
            payload.output.orientation,
            payload.output.scale,
            payload.output.margins
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
    .instrument(request_span)
    .await;

    // Mark job as completed/failed and release permit
    let success = result
        .as_ref()
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    state_for_completion
        .concurrency_controller
        .complete_job(&report_id_for_completion, success);
    drop(permit);

    result
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

    // Use correlation_id from payload or generate one
    let correlation_id = payload
        .correlation_id
        .clone()
        .unwrap_or_else(|| report_id.clone());

    // Classify job for concurrency control
    let classification = classify_job(&payload);
    tracing::info!(
        correlation_id = %correlation_id,
        report_id = %report_id,
        report_code = %payload.report.code,
        records = payload.data.total_records,
        estimated_time_secs = estimated_time,
        job_type = ?classification.job_type,
        job_size = ?classification.job_size,
        "Queueing ERP report for async processing"
    );

    // Check if system can accept the job (pre-check without acquiring permit)
    if !state.concurrency_controller.is_healthy() {
        let metrics = state.concurrency_controller.metrics();
        tracing::warn!(
            report_id = %report_id,
            ram_percent = metrics.ram_percent,
            load = metrics.load_1min,
            active_jobs = metrics.active_jobs,
            "Rejecting async job - system unhealthy"
        );
        return Ok(HttpResponse::ServiceUnavailable().json(json!({
            "error": "server_overloaded",
            "message": "Server is currently overloaded. Please try again later.",
            "retry_after": 30,
            "metrics": {
                "ram_percent": metrics.ram_percent,
                "load_1min": metrics.load_1min,
                "active_jobs": metrics.active_jobs
            }
        })));
    }

    // Save initial job status to cache
    let job_status = JobStatus::new_processing(report_id.clone());
    let cache_key = format!("{}{}", JOB_STATUS_CACHE_PREFIX, report_id);
    state
        .cache_service
        .set(&cache_key, &job_status, JOB_STATUS_TTL_SECS)
        .await;

    tracing::debug!(
        report_id = %report_id,
        cache_key = %cache_key,
        "Saved initial job status to cache"
    );

    // Clone for async task
    let state_clone = state.clone();
    let report_id_clone = report_id.clone();
    let correlation_id_clone = correlation_id.clone();
    let tenant_id = payload.tenant_id as u64;

    // Spawn background task with concurrency control
    tokio::spawn(async move {
        // Acquire permit inside the spawned task
        let permit = match state_clone
            .concurrency_controller
            .acquire_permit(&report_id_clone, classification, tenant_id)
            .await
        {
            Ok(permit) => permit,
            Err(e) => {
                tracing::error!(
                    correlation_id = %correlation_id_clone,
                    report_id = %report_id_clone,
                    "Failed to acquire permit: {:?}", e
                );

                // Update job status to failed
                let cache_key = format!("{}{}", JOB_STATUS_CACHE_PREFIX, report_id_clone);
                if let Some(job_status) =
                    state_clone.cache_service.get::<JobStatus>(&cache_key).await
                {
                    let updated_status =
                        job_status.fail("Server overloaded".to_string(), Some(format!("{:?}", e)));
                    state_clone
                        .cache_service
                        .set(&cache_key, &updated_status, JOB_STATUS_TTL_SECS)
                        .await;
                }
                return;
            }
        };

        let result =
            process_report_async(state_clone.clone(), payload, report_id_clone.clone()).await;

        // Mark job completion
        let success = result.is_ok();
        state_clone
            .concurrency_controller
            .complete_job(&report_id_clone, success);
        drop(permit);

        match result {
            Ok((download_url, file_size, processing_time_ms, format, mime_type)) => {
                tracing::info!(
                    correlation_id = %correlation_id_clone,
                    report_id = %report_id_clone,
                    download_url = ?download_url,
                    file_size = file_size,
                    "Async report completed"
                );

                // Update job status to completed
                let cache_key = format!("{}{}", JOB_STATUS_CACHE_PREFIX, report_id_clone);
                if let Some(job_status) =
                    state_clone.cache_service.get::<JobStatus>(&cache_key).await
                {
                    let updated_status = job_status.complete(
                        download_url,
                        file_size,
                        processing_time_ms,
                        format,
                        mime_type,
                    );
                    state_clone
                        .cache_service
                        .set(&cache_key, &updated_status, JOB_STATUS_TTL_SECS)
                        .await;
                    tracing::debug!(report_id = %report_id_clone, "Updated job status to completed");
                }
            }
            Err(e) => {
                tracing::error!(
                    correlation_id = %correlation_id_clone,
                    report_id = %report_id_clone,
                    "Async report failed: {}", e
                );

                // Update job status to failed
                let cache_key = format!("{}{}", JOB_STATUS_CACHE_PREFIX, report_id_clone);
                if let Some(job_status) =
                    state_clone.cache_service.get::<JobStatus>(&cache_key).await
                {
                    let updated_status = job_status
                        .fail("Failed to generate report".to_string(), Some(e.to_string()));
                    state_clone
                        .cache_service
                        .set(&cache_key, &updated_status, JOB_STATUS_TTL_SECS)
                        .await;
                    tracing::debug!(report_id = %report_id_clone, "Updated job status to failed");
                }
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
    state: web::Data<ApiState>,
) -> ApiResult<HttpResponse> {
    let report_id = path.into_inner();
    let cache_key = format!("{}{}", JOB_STATUS_CACHE_PREFIX, report_id);

    tracing::debug!(
        report_id = %report_id,
        cache_key = %cache_key,
        "Getting report status from cache"
    );

    // Get job status from cache
    match state.cache_service.get::<JobStatus>(&cache_key).await {
        Some(job_status) => {
            tracing::info!(
                report_id = %report_id,
                status = ?job_status.status,
                "Found job status in cache"
            );

            // Map status to response format expected by Core-Service
            let status_str = match job_status.status {
                JobState::Processing => "processing",
                JobState::Completed => "completed",
                JobState::Failed => "failed",
            };

            Ok(HttpResponse::Ok().json(json!({
                "success": job_status.status == JobState::Completed,
                "reportId": job_status.report_id,
                "status": status_str,
                "downloadUrl": job_status.download_url,
                "fileSize": job_status.file_size.unwrap_or(0),
                "processingTimeMs": job_status.processing_time_ms.unwrap_or(0),
                "format": job_status.format,
                "mimeType": job_status.mime_type,
                "error": job_status.error,
                "details": job_status.details,
                "createdAt": job_status.created_at.to_rfc3339(),
                "completedAt": job_status.completed_at.map(|dt| dt.to_rfc3339())
            })))
        }
        None => {
            tracing::warn!(
                report_id = %report_id,
                "Job status not found in cache"
            );

            Ok(HttpResponse::NotFound().json(json!({
                "success": false,
                "reportId": report_id,
                "status": "not_found",
                "error": "Report job not found or expired"
            })))
        }
    }
}

/// Download report
/// GET /api/v1/reports/{id}/download
pub async fn download_report(
    _req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<ApiState>,
) -> ApiResult<HttpResponse> {
    let report_id = path.into_inner();
    let temp_dir = PathBuf::from("./temp/reports");

    // Get job status to determine mime type and format
    let job_cache_key = format!("{}{}", JOB_STATUS_CACHE_PREFIX, report_id);
    let (mime_type, extension) =
        if let Some(job_status) = state.cache_service.get::<JobStatus>(&job_cache_key).await {
            let mime = job_status
                .mime_type
                .unwrap_or_else(|| "application/octet-stream".to_string());
            let ext = job_status.format.unwrap_or_else(|| "bin".to_string());

            // If it's an S3 presigned URL, redirect to it
            if let Some(download_url) = &job_status.download_url {
                if download_url.starts_with("http") {
                    tracing::info!(
                        report_id = %report_id,
                        "Redirecting to S3 download URL"
                    );
                    return Ok(HttpResponse::Found()
                        .append_header(("Location", download_url.clone()))
                        .finish());
                }
            }

            (mime, ext)
        } else {
            // Default to PDF if no job status found
            ("application/pdf".to_string(), "pdf".to_string())
        };

    // Check for local file (files <= 10MB)
    let file_path = temp_dir.join(format!("{}.{}", report_id, extension));
    if file_path.exists() {
        tracing::info!(
            report_id = %report_id,
            file_path = %file_path.display(),
            "Serving report from local file"
        );

        // Read file bytes
        match std::fs::read(&file_path) {
            Ok(bytes) => {
                let size = bytes.len();

                // Delete the file after reading (consume once)
                if let Err(e) = std::fs::remove_file(&file_path) {
                    tracing::warn!(
                        report_id = %report_id,
                        "Failed to delete temp file after serving: {:?}", e
                    );
                } else {
                    tracing::debug!(report_id = %report_id, "Deleted temp file after serving");
                }

                // Return bytes directly with appropriate headers
                return Ok(HttpResponse::Ok()
                    .content_type(mime_type)
                    .append_header((
                        "Content-Disposition",
                        format!(
                            "attachment; filename=\"report-{}.{}\"",
                            report_id, extension
                        ),
                    ))
                    .append_header(("Content-Length", size.to_string()))
                    .body(bytes));
            }
            Err(e) => {
                tracing::error!(report_id = %report_id, "Failed to read temp file: {:?}", e);
            }
        }
    }

    // Fallback: Try to find the report in S3 with various extensions
    for ext in ["pdf", "xlsx", "csv"] {
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
/// Returns: (download_url, file_size, processing_time_ms, format, mime_type)
async fn process_report_async(
    state: web::Data<ApiState>,
    payload: ErpReportPayload,
    report_id: String,
) -> anyhow::Result<(Option<String>, u64, u64, String, String)> {
    let start = Instant::now();

    // Use correlation_id from payload or fallback to report_id
    let correlation_id = payload
        .correlation_id
        .clone()
        .unwrap_or_else(|| report_id.clone());

    // Generate based on format
    tracing::info!(
        correlation_id = %correlation_id,
        report_code = %payload.report.code,
        format = ?payload.output.format,
        "Step 1: Generating report"
    );

    let (bytes, mime_type, extension) = match payload.output.format {
        OutputFormat::Pdf => generate_pdf(&payload).await.map_err(|e| {
            tracing::error!(correlation_id = %correlation_id, "PDF generation failed: {:?}", e);
            e
        })?,
        OutputFormat::Xlsx => generate_excel(&payload).await.map_err(|e| {
            tracing::error!(correlation_id = %correlation_id, "Excel generation failed: {:?}", e);
            e
        })?,
        OutputFormat::Csv => generate_csv(&payload).await.map_err(|e| {
            tracing::error!(correlation_id = %correlation_id, "CSV generation failed: {:?}", e);
            e
        })?,
        OutputFormat::Html => {
            return Err(anyhow::anyhow!("HTML format not implemented"));
        }
    };

    let file_size = bytes.len();
    tracing::info!(
        correlation_id = %correlation_id,
        size_bytes = file_size,
        "Step 1 complete: Generated report"
    );

    // Upload to S3 only if file is too large (> 10MB)
    // For smaller files, we store bytes in cache for direct download
    let needs_s3_upload = file_size > MAX_ATTACHMENT_SIZE_BYTES;

    // Upload to S3 if needed (file > 10MB)
    let download_url = if needs_s3_upload {
        tracing::info!(
            correlation_id = %correlation_id,
            file_size = file_size,
            threshold = MAX_ATTACHMENT_SIZE_BYTES,
            "File exceeds 10MB, uploading to S3"
        );

        // Get environment prefix for S3 path
        let env_prefix = std::env::var("KAFKA_ENV_PREFIX").unwrap_or_else(|_| "dev".to_string());

        // Generate filename from report code: NCF_SUMMARY -> ncf-summary_2025-12-17_19-30-45.pdf
        let report_name = payload.report.code.to_lowercase().replace('_', "-");
        let timestamp = chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S");

        // S3 key format: {env}/{tenant_id}/{report-name}_{datetime}.{extension}
        let file_key = format!(
            "{}/{}/{}_{}.{}",
            env_prefix, payload.tenant_id, report_name, timestamp, extension
        );

        tracing::info!(
            correlation_id = %correlation_id,
            bucket = %state.config.s3_bucket_documents,
            key = %file_key,
            "Step 2: Uploading to S3"
        );

        if let Err(e) = state
            .s3_client
            .put_object(
                &state.config.s3_bucket_documents,
                &file_key,
                bytes.clone(),
                mime_type,
            )
            .await
        {
            tracing::error!(correlation_id = %correlation_id, "S3 upload failed: {:?}", e);
            return Err(e);
        }
        tracing::info!(correlation_id = %correlation_id, "Step 2 complete: S3 upload successful");

        // Generate presigned URL
        tracing::info!(correlation_id = %correlation_id, "Step 3: Generating presigned URL");
        let url = match state
            .s3_client
            .create_presigned_url(
                &state.config.s3_bucket_documents,
                &file_key,
                86400, // 24 hours
            )
            .await
        {
            Ok(url) => {
                tracing::info!(correlation_id = %correlation_id, "Step 3 complete: Presigned URL generated");
                url
            }
            Err(e) => {
                tracing::error!(correlation_id = %correlation_id, "Presigned URL generation failed: {:?}", e);
                return Err(e);
            }
        };

        Some(url)
    } else {
        // File is small enough (<= 10MB) - store locally for direct download endpoint
        // Also will be attached directly to email/WhatsApp (not just a link)
        let temp_dir = PathBuf::from("./temp/reports");
        std::fs::create_dir_all(&temp_dir).ok();

        let file_path = temp_dir.join(format!("{}.{}", report_id, extension));
        tracing::info!(
            correlation_id = %correlation_id,
            file_size = file_size,
            file_path = %file_path.display(),
            threshold = MAX_ATTACHMENT_SIZE_BYTES,
            "File <= 10MB, storing locally. Will be sent as direct attachment via email/WhatsApp"
        );

        // Write bytes to file for the download endpoint
        if let Err(e) = std::fs::write(&file_path, &bytes) {
            tracing::error!(correlation_id = %correlation_id, "Failed to write temp file: {:?}", e);
            // Don't fail - the file will still be sent as attachment
        }

        // Return internal download URL (for status endpoint and manual download)
        Some(format!("/api/v1/reports/{}/download", report_id))
    };

    // Determine if we should pass download_url to delivery
    // For small files (<= 10MB), we want to attach directly via email/WhatsApp (pass None)
    // For large files (> 10MB), we pass the S3 URL so recipients get a download link instead
    let delivery_download_url = if needs_s3_upload {
        download_url.clone()
    } else {
        None // Small files: attach directly, don't send a link
    };

    let processing_time = start.elapsed().as_millis();
    tracing::info!(
        correlation_id = %correlation_id,
        report_code = %payload.report.code,
        size_bytes = file_size,
        processing_time_ms = processing_time,
        uploaded_to_s3 = needs_s3_upload,
        "Report processing completed"
    );

    // Handle delivery if configured
    // Note: We use delivery_download_url (None for small files) so that small files
    // are attached directly to email/WhatsApp instead of sending a download link
    if payload.delivery.is_some() {
        if let Err(e) = deliver_report(
            &payload,
            &bytes,
            extension,
            delivery_download_url.as_deref(),
        )
        .await
        {
            tracing::warn!(
                correlation_id = %correlation_id,
                "Delivery failed: {}", e
            );
        }
    }

    // Return job completion info: (download_url, file_size, processing_time_ms, format, mime_type)
    Ok((
        download_url,
        file_size as u64,
        processing_time as u64,
        extension.to_string(),
        mime_type.to_string(),
    ))
}

/// Deliver report via configured channels
///
/// # Arguments
/// * `payload` - The report payload with delivery options
/// * `bytes` - The generated report bytes
/// * `extension` - File extension (pdf, xlsx, csv)
/// * `download_url` - Optional S3 download URL. If provided, sends link instead of attachment
///                    (used when file size exceeds MAX_ATTACHMENT_SIZE_BYTES)
async fn deliver_report(
    payload: &ErpReportPayload,
    bytes: &[u8],
    extension: &str,
    download_url: Option<&str>,
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
        // SMTP_PASS is used for compatibility with .env files, fallback to SMTP_PASSWORD
        if let (Ok(host), Ok(user), Ok(pass)) = (
            std::env::var("SMTP_HOST"),
            std::env::var("SMTP_USER"),
            std::env::var("SMTP_PASS").or_else(|_| std::env::var("SMTP_PASSWORD")),
        ) {
            let port = std::env::var("SMTP_PORT")
                .unwrap_or_else(|_| "587".to_string())
                .parse()
                .unwrap_or(587);

            let from_email =
                std::env::var("SMTP_FROM_EMAIL").unwrap_or_else(|_| "noreply@facturazo.com".into());
            let from_name =
                std::env::var("SMTP_FROM_NAME").unwrap_or_else(|_| "Facturazo ERP".into());

            tracing::info!(
                "Email service configured: host={}, port={}, from={}",
                host,
                port,
                from_email
            );
            builder = builder.with_email(host, port, user, pass, from_email, from_name, true);
        } else {
            tracing::warn!("Email delivery requested but SMTP config not found. Required: SMTP_HOST, SMTP_USER, SMTP_PASS");
        }
    }

    // Configure WhatsApp if needed - supports both Evolution API and Green-API
    if delivery.whatsapp.is_some() {
        // Check which provider to use (default to Evolution API for backwards compatibility)
        let provider_type = std::env::var("WHATSAPP_PROVIDER")
            .unwrap_or_else(|_| "evolution".to_string())
            .to_lowercase();

        let whatsapp_provider: Option<
            std::sync::Arc<dyn crate::infrastructure::notifications::WhatsAppProvider>,
        > = match provider_type.as_str() {
            "greenapi" | "green-api" | "green_api" => {
                // Try Green-API configuration
                if let (Ok(url), Ok(instance_id), Ok(token)) = (
                    std::env::var("GREEN_API_URL"),
                    std::env::var("GREEN_API_INSTANCE_ID"),
                    std::env::var("GREEN_API_TOKEN"),
                ) {
                    // Use media URL if available for direct file uploads
                    let client = if let Ok(media_url) = std::env::var("GREEN_API_MEDIA_URL") {
                        tracing::info!(
                            "WhatsApp service configured with Green-API: url={}, instance={}, media={}",
                            url,
                            instance_id,
                            media_url
                        );
                        crate::infrastructure::notifications::GreenApiClient::with_media_url(
                            url,
                            media_url,
                            instance_id,
                            token,
                        )
                    } else {
                        tracing::info!(
                            "WhatsApp service configured with Green-API: url={}, instance={}",
                            url,
                            instance_id
                        );
                        crate::infrastructure::notifications::GreenApiClient::new(
                            url,
                            instance_id,
                            token,
                        )
                    };
                    Some(std::sync::Arc::new(client))
                } else {
                    tracing::warn!("Green-API selected but config not found. Required: GREEN_API_URL, GREEN_API_INSTANCE_ID, GREEN_API_TOKEN");
                    None
                }
            }
            _ => {
                // Default to Evolution API
                if let (Ok(base_url), Ok(instance), Ok(api_key)) = (
                    std::env::var("EVOLUTION_API_URL"),
                    std::env::var("EVOLUTION_INSTANCE"),
                    std::env::var("EVOLUTION_API_KEY"),
                ) {
                    tracing::info!(
                        "WhatsApp service configured with EvolutionAPI: url={}, instance={}",
                        base_url,
                        instance
                    );
                    Some(std::sync::Arc::new(
                        crate::infrastructure::notifications::EvolutionApiClient::new(
                            base_url, api_key, instance,
                        ),
                    ))
                } else {
                    tracing::warn!("EvolutionAPI selected but config not found. Required: EVOLUTION_API_URL, EVOLUTION_INSTANCE, EVOLUTION_API_KEY");
                    None
                }
            }
        };

        if let Some(provider) = whatsapp_provider {
            builder = builder.with_whatsapp_provider(provider);
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

            // If we have a download URL (file too large for attachment), pass it to notifier
            // Otherwise, the file will be attached directly
            notifier
                .deliver_report(
                    &delivery_payload,
                    bytes.to_vec(),
                    download_url.map(|s| s.to_string()),
                )
                .await?;
        }
        _ => {} // Download/View don't need delivery
    }

    Ok(())
}
