use actix_web::{dev::ServiceRequest, Error, HttpMessage};
use actix_web_httpauth::extractors::bearer::{BearerAuth, Config};
use actix_web_httpauth::extractors::AuthenticationError;
use actix_web_httpauth::middleware::HttpAuthentication;

pub struct Authentication;

impl Default for Authentication {
    fn default() -> HttpAuthentication<BearerAuth, fn(ServiceRequest, BearerAuth) -> Result<ServiceRequest, (Error, ServiceRequest)>> {
        HttpAuthentication::bearer(validator)
    }
}

async fn validator(
    req: ServiceRequest,
    credentials: BearerAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    // For now, accept any token that starts with "Bearer "
    // In production, validate JWT here
    let token = credentials.token();

    if token.is_empty() {
        let config = Config::default();
        return Err((AuthenticationError::from(config).into(), req));
    }

    // Validate token (simplified for demo)
    if token.starts_with("valid_") {
        // Extract user info from token and add to request extensions
        req.extensions_mut().insert(UserInfo {
            user_id: "user123".to_string(),
            organization_id: "org456".to_string(),
        });
        Ok(req)
    } else {
        let config = Config::default();
        Err((AuthenticationError::from(config).into(), req))
    }
}

#[derive(Clone)]
pub struct UserInfo {
    pub user_id: String,
    pub organization_id: String,
}