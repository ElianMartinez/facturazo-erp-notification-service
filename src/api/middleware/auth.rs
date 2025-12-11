use actix_web::{dev::ServiceRequest, Error, HttpMessage};
use actix_web_httpauth::extractors::bearer::{BearerAuth, Config};
use actix_web_httpauth::extractors::AuthenticationError;
use actix_web_httpauth::middleware::HttpAuthentication;
use std::future::{ready, Ready};

pub fn create_auth_middleware() -> HttpAuthentication<
    BearerAuth,
    fn(ServiceRequest, BearerAuth) -> Ready<Result<ServiceRequest, (Error, ServiceRequest)>>,
> {
    HttpAuthentication::bearer(validator)
}

fn validator(
    req: ServiceRequest,
    credentials: BearerAuth,
) -> Ready<Result<ServiceRequest, (Error, ServiceRequest)>> {
    // For now, accept any token that starts with "Bearer "
    // In production, validate JWT here
    let token = credentials.token();

    if token.is_empty() {
        let config = Config::default();
        return ready(Err((AuthenticationError::from(config).into(), req)));
    }

    // Validate token (simplified for demo)
    // In production, decode JWT and extract tenant_id and user_id
    if token.starts_with("valid_") {
        // Extract tenant and user from token
        // Example: valid_tenant123_user456
        let parts: Vec<&str> = token.split('_').collect();
        let tenant_id = parts
            .get(1)
            .and_then(|s| s.strip_prefix("tenant"))
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(1);
        let user_id = parts
            .get(2)
            .and_then(|s| s.strip_prefix("user"))
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(1);

        // Add to request extensions
        req.extensions_mut().insert(UserInfo {
            tenant_id,
            user_id,
            organization_id: None,
        });

        // Also add AuthInfo for handlers
        req.extensions_mut()
            .insert(crate::api::handlers::AuthInfo { tenant_id, user_id });

        ready(Ok(req))
    } else {
        let config = Config::default();
        ready(Err((AuthenticationError::from(config).into(), req)))
    }
}

#[derive(Clone)]
pub struct UserInfo {
    pub tenant_id: i64,
    pub user_id: i64,
    pub organization_id: Option<String>,
}

/// Extract tenant and user info from request.
///
/// This function checks multiple sources in order of priority:
/// 1. Request extensions (set by auth middleware after JWT validation)
/// 2. HTTP headers (X-Tenant-Id, X-User-Id) for internal service-to-service calls
///
/// Returns `Some((tenant_id, user_id))` if found, `None` if no auth info available.
pub fn extract_tenant_user(req: &actix_web::HttpRequest) -> Option<(i64, i64)> {
    // First, try to get from extensions (set by auth middleware)
    if let Some(info) = req.extensions().get::<UserInfo>() {
        return Some((info.tenant_id, info.user_id));
    }

    // Fallback: try to get from headers (for internal service calls)
    let tenant_id = req
        .headers()
        .get("X-Tenant-Id")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok());

    let user_id = req
        .headers()
        .get("X-User-Id")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok());

    match (tenant_id, user_id) {
        (Some(t), Some(u)) => Some((t, u)),
        _ => None,
    }
}

/// Extract tenant and user info with defaults.
///
/// Same as `extract_tenant_user` but returns default values (1, 1) if no auth info found.
/// Use this for endpoints that can work without authentication.
pub fn extract_tenant_user_or_default(req: &actix_web::HttpRequest) -> (i64, i64) {
    extract_tenant_user(req).unwrap_or((1, 1))
}
