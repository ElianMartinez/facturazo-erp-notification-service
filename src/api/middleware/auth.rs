use actix_web::{dev::ServiceRequest, Error, HttpMessage};
use actix_web_httpauth::extractors::bearer::{BearerAuth, Config};
use actix_web_httpauth::extractors::AuthenticationError;
use actix_web_httpauth::middleware::HttpAuthentication;
use std::future::{ready, Ready};

pub fn create_auth_middleware() -> HttpAuthentication<BearerAuth, fn(ServiceRequest, BearerAuth) -> Ready<Result<ServiceRequest, (Error, ServiceRequest)>>> {
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
        let tenant_id = parts.get(1)
            .and_then(|s| s.strip_prefix("tenant"))
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(1);
        let user_id = parts.get(2)
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
        req.extensions_mut().insert(crate::api::handlers::AuthInfo {
            tenant_id,
            user_id,
        });

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

// Helper function to extract tenant and user info from request
pub fn extract_tenant_user(req: &actix_web::HttpRequest) -> Option<(i64, i64)> {
    req.extensions().get::<UserInfo>()
        .map(|info| (info.tenant_id, info.user_id))
}