//! Rate limiting implementation
//!
//! Controls request rates per tenant and endpoint using token bucket algorithm

use std::num::NonZeroU32;
use std::sync::Arc;
use dashmap::DashMap;
use governor::{Quota, RateLimiter as GovernorLimiter, clock::DefaultClock, state::{InMemoryState, NotKeyed}};
use crate::infrastructure::resilience::RateLimitError;

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimiterConfig {
    /// Requests per second per tenant
    pub requests_per_second: u32,
    /// Burst capacity
    pub burst_size: u32,
    /// Requests per second for specific endpoints
    pub endpoint_limits: Vec<(String, u32)>,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 100,
            burst_size: 50,
            endpoint_limits: vec![
                ("/api/v1/generate/sync".to_string(), 10),
                ("/api/v1/generate/async".to_string(), 50),
            ],
        }
    }
}

/// Simple rate limiter
pub struct RateLimiter {
    limiter: GovernorLimiter<NotKeyed, InMemoryState, DefaultClock>,
}

impl RateLimiter {
    pub fn new(requests_per_second: u32, burst_size: u32) -> Self {
        let quota = Quota::per_second(NonZeroU32::new(requests_per_second).unwrap_or(NonZeroU32::new(1).unwrap()))
            .allow_burst(NonZeroU32::new(burst_size).unwrap_or(NonZeroU32::new(1).unwrap()));

        Self {
            limiter: GovernorLimiter::direct(quota),
        }
    }

    /// Check if request is allowed (non-blocking)
    pub fn check(&self) -> bool {
        self.limiter.check().is_ok()
    }

    /// Wait until request is allowed (blocking)
    pub async fn wait(&self) {
        self.limiter.until_ready().await;
    }
}

/// Rate limiter per tenant
pub struct TenantRateLimiter {
    config: RateLimiterConfig,
    tenant_limiters: DashMap<String, Arc<RateLimiter>>,
    endpoint_limiters: DashMap<String, Arc<RateLimiter>>,
}

impl TenantRateLimiter {
    pub fn new(config: RateLimiterConfig) -> Self {
        let endpoint_limiters = DashMap::new();

        // Pre-create endpoint limiters
        for (endpoint, rps) in &config.endpoint_limits {
            endpoint_limiters.insert(
                endpoint.clone(),
                Arc::new(RateLimiter::new(*rps, *rps / 2 + 1)),
            );
        }

        Self {
            config,
            tenant_limiters: DashMap::new(),
            endpoint_limiters,
        }
    }

    /// Check rate limit for tenant
    pub fn check(&self, tenant_id: &str) -> Result<(), RateLimitError> {
        let limiter = self.get_or_create_tenant_limiter(tenant_id);

        if limiter.check() {
            Ok(())
        } else {
            Err(RateLimitError::Exceeded {
                tenant_id: tenant_id.to_string(),
                retry_after_secs: 1,
            })
        }
    }

    /// Check rate limit for tenant and endpoint
    pub fn check_with_endpoint(&self, tenant_id: &str, endpoint: &str) -> Result<(), RateLimitError> {
        // Check tenant limit first
        self.check(tenant_id)?;

        // Then check endpoint limit
        if let Some(limiter) = self.endpoint_limiters.get(endpoint) {
            if !limiter.check() {
                return Err(RateLimitError::Exceeded {
                    tenant_id: tenant_id.to_string(),
                    retry_after_secs: 1,
                });
            }
        }

        Ok(())
    }

    /// Wait until request is allowed
    pub async fn wait(&self, tenant_id: &str) {
        let limiter = self.get_or_create_tenant_limiter(tenant_id);
        limiter.wait().await;
    }

    fn get_or_create_tenant_limiter(&self, tenant_id: &str) -> Arc<RateLimiter> {
        self.tenant_limiters
            .entry(tenant_id.to_string())
            .or_insert_with(|| {
                Arc::new(RateLimiter::new(
                    self.config.requests_per_second,
                    self.config.burst_size,
                ))
            })
            .clone()
    }

    /// Get current number of tracked tenants
    pub fn tenant_count(&self) -> usize {
        self.tenant_limiters.len()
    }

    /// Clear limiters for inactive tenants (call periodically)
    pub fn cleanup_inactive(&self) {
        // In a real implementation, track last access time and remove old entries
        // For now, just keep all limiters
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_allows_within_limit() {
        let limiter = RateLimiter::new(10, 5);

        // Should allow initial requests within burst
        for _ in 0..5 {
            assert!(limiter.check());
        }
    }

    #[test]
    fn test_tenant_rate_limiter() {
        let config = RateLimiterConfig {
            requests_per_second: 10,
            burst_size: 5,
            endpoint_limits: vec![],
        };
        let limiter = TenantRateLimiter::new(config);

        // First few requests should succeed
        assert!(limiter.check("tenant-1").is_ok());
        assert!(limiter.check("tenant-1").is_ok());

        // Different tenant has own limit
        assert!(limiter.check("tenant-2").is_ok());
    }

    #[test]
    fn test_tenant_count() {
        let config = RateLimiterConfig::default();
        let limiter = TenantRateLimiter::new(config);

        limiter.check("tenant-1").ok();
        limiter.check("tenant-2").ok();
        limiter.check("tenant-3").ok();

        assert_eq!(limiter.tenant_count(), 3);
    }
}
