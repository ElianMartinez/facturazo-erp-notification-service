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
    // Check database connection
    let db_healthy = sqlx::query("SELECT 1")
        .fetch_one(&state.db)
        .await
        .is_ok();

    // Check Redis connection
    let redis_healthy = {
        let mut redis = state.redis.clone();
        redis::cmd("PING")
            .query_async::<_, String>(&mut redis)
            .await
            .is_ok()
    };

    // Check Kafka
    let kafka_healthy = true; // TODO: Implement actual check

    if db_healthy && redis_healthy && kafka_healthy {
        HttpResponse::Ok().json(serde_json::json!({
            "status": "ready",
            "checks": {
                "database": "ok",
                "redis": "ok",
                "kafka": "ok"
            }
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "not_ready",
            "checks": {
                "database": if db_healthy { "ok" } else { "failed" },
                "redis": if redis_healthy { "ok" } else { "failed" },
                "kafka": if kafka_healthy { "ok" } else { "failed" }
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
    let templates = state.template_manager.list_templates().await;

    HttpResponse::Ok().json(serde_json::json!({
        "templates": templates
    }))
}

async fn get_template(
    path: web::Path<String>,
    state: web::Data<crate::api::ApiState>,
) -> HttpResponse {
    let template_id = path.into_inner();

    match state.template_manager.get_template(&template_id).await {
        Ok(content) => HttpResponse::Ok()
            .content_type("text/plain")
            .body(content),
        Err(e) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Template not found",
            "details": e.to_string()
        }))
    }
}

async fn update_template(
    path: web::Path<String>,
    body: String,
    state: web::Data<crate::api::ApiState>,
) -> HttpResponse {
    let template_id = path.into_inner();

    match state.template_manager.update_template(&template_id, body).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "status": "updated",
            "template_id": template_id
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Failed to update template",
            "details": e.to_string()
        }))
    }
}

async fn reload_template(
    path: web::Path<String>,
    state: web::Data<crate::api::ApiState>,
) -> HttpResponse {
    let template_id = path.into_inner();

    match state.template_manager.reload_template(&template_id).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "status": "reloaded",
            "template_id": template_id
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Failed to reload template",
            "details": e.to_string()
        }))
    }
}