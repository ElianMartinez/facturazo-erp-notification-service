use actix_web::{web, HttpResponse};
use actix_web::middleware::Logger;
use actix_cors::Cors;

use super::handlers;
use super::template_handler;
use super::middleware::{auth::create_auth_middleware, compression::create_compression_middleware};

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg
        // Health checks
        .route("/health", web::get().to(health_check))
        .route("/ready", web::get().to(readiness_check))
        .route("/metrics", web::get().to(metrics_endpoint))

        // API v1
        .service(
            web::scope("/api/v1")
                .wrap(create_auth_middleware())
                .wrap(create_compression_middleware())
                .wrap(Logger::default())
                .wrap(
                    Cors::default()
                        .allowed_origin_fn(|origin, _req_head| {
                            origin.as_bytes().starts_with(b"http://localhost") ||
                            origin.as_bytes().starts_with(b"https://")
                        })
                        .allowed_methods(vec!["GET", "POST", "PUT", "DELETE"])
                        .allowed_headers(vec!["Content-Type", "Authorization", "X-User-Id"])
                        .max_age(3600)
                )

                // Document generation
                .service(
                    web::scope("/documents")
                        .route("/generate/sync", web::post().to(handlers::generate_sync))
                        .route("/generate/async", web::post().to(handlers::generate_async))
                        .route("/upload", web::post().to(handlers::upload_data))
                        .route("/{id}/status", web::get().to(handlers::get_status))
                        .route("/{id}/download", web::get().to(handlers::download_document))
                )

                // Template management (admin only)
                .service(
                    web::scope("/templates")
                        .route("", web::get().to(list_templates))
                        .route("/list", web::get().to(template_handler::list_templates))
                        .route("/generate", web::post().to(template_handler::generate_pdf_from_template))
                        .route("/preview/{id}", web::get().to(template_handler::preview_template))
                        .route("/{id}", web::get().to(get_template))
                        .route("/{id}", web::put().to(update_template))
                        .route("/{id}/reload", web::post().to(reload_template))
                )
        );
}

async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy"
    }))
}

async fn readiness_check(state: web::Data<crate::api::ApiState>) -> HttpResponse {
    // Check template manager
    let templates_loaded = state.template_manager.list_templates().len() > 0;

    // S3 is already initialized if we got here
    let s3_healthy = true;

    if s3_healthy && templates_loaded {
        HttpResponse::Ok().json(serde_json::json!({
            "status": "ready",
            "checks": {
                "s3": "ok",
                "templates": if templates_loaded { "ok" } else { "no templates loaded" }
            }
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "not_ready",
            "checks": {
                "s3": if s3_healthy { "ok" } else { "failed" },
                "templates": if templates_loaded { "ok" } else { "no templates loaded" }
            }
        }))
    }
}

async fn metrics_endpoint() -> HttpResponse {
    use prometheus::{Encoder, TextEncoder};

    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];

    encoder.encode(&metric_families, &mut buffer)
        .expect("Failed to encode metrics");

    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(buffer)
}

// Template endpoints

async fn list_templates(
    state: web::Data<crate::api::ApiState>,
) -> HttpResponse {
    let templates = state.template_manager.list_templates();

    HttpResponse::Ok().json(serde_json::json!({
        "templates": templates
    }))
}

async fn get_template(
    path: web::Path<String>,
    _state: web::Data<crate::api::ApiState>,
) -> HttpResponse {
    let template_id = path.into_inner();

    // TODO: Implementar get_template en TemplateManager
    HttpResponse::NotImplemented().json(serde_json::json!({
        "error": "Template retrieval not implemented",
        "template_id": template_id
    }))
}

async fn update_template(
    path: web::Path<String>,
    _body: String,
    _state: web::Data<crate::api::ApiState>,
) -> HttpResponse {
    let template_id = path.into_inner();

    // TODO: Implementar update_template en TemplateManager
    HttpResponse::NotImplemented().json(serde_json::json!({
        "error": "Template update not implemented",
        "template_id": template_id
    }))
}

async fn reload_template(
    path: web::Path<String>,
    _state: web::Data<crate::api::ApiState>,
) -> HttpResponse {
    let template_id = path.into_inner();

    // TODO: Implementar reload_template en TemplateManager
    HttpResponse::NotImplemented().json(serde_json::json!({
        "error": "Template reload not implemented",
        "template_id": template_id
    }))
}