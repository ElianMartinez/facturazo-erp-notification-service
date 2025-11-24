//! Resilience patterns for external service calls
//!
//! This module provides:
//! - Circuit Breaker: Prevents cascading failures
//! - Rate Limiter: Controls request rates per tenant/endpoint
//! - Retry: Automatic retries with exponential backoff
//! - Health Checks: Monitor dependency health

mod circuit_breaker;
pub mod health;
mod rate_limiter;
mod retry;

pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
pub use health::{DependencyHealth, DependencyStatus, HealthChecker, HealthStatus};
pub use rate_limiter::{RateLimiter, RateLimiterConfig, TenantRateLimiter};
pub use retry::{with_retry, RetryConfig, RetryPolicy};

use anyhow::Result;
use std::sync::Arc;

/// Resilience configuration
#[derive(Debug, Clone)]
pub struct ResilienceConfig {
    pub circuit_breaker: CircuitBreakerConfig,
    pub rate_limiter: RateLimiterConfig,
    pub retry: RetryConfig,
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            circuit_breaker: CircuitBreakerConfig::default(),
            rate_limiter: RateLimiterConfig::default(),
            retry: RetryConfig::default(),
        }
    }
}

/// Resilience manager - coordinates all resilience patterns
pub struct ResilienceManager {
    pub circuit_breakers: CircuitBreakerRegistry,
    pub rate_limiters: Arc<TenantRateLimiter>,
    pub health_checker: Arc<HealthChecker>,
    config: ResilienceConfig,
}

impl ResilienceManager {
    pub fn new(config: ResilienceConfig) -> Self {
        Self {
            circuit_breakers: CircuitBreakerRegistry::new(config.circuit_breaker.clone()),
            rate_limiters: Arc::new(TenantRateLimiter::new(config.rate_limiter.clone())),
            health_checker: Arc::new(HealthChecker::new()),
            config,
        }
    }

    /// Get circuit breaker for a service
    pub fn circuit_breaker(&self, service: &str) -> Arc<CircuitBreaker> {
        self.circuit_breakers.get_or_create(service)
    }

    /// Check rate limit for tenant
    pub fn check_rate_limit(&self, tenant_id: &str) -> Result<(), RateLimitError> {
        self.rate_limiters.check(tenant_id)
    }

    /// Get health status
    pub async fn health_status(&self) -> HealthStatus {
        self.health_checker.check_all().await
    }
}

/// Registry for circuit breakers per service
pub struct CircuitBreakerRegistry {
    breakers: dashmap::DashMap<String, Arc<CircuitBreaker>>,
    config: CircuitBreakerConfig,
}

impl CircuitBreakerRegistry {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: dashmap::DashMap::new(),
            config,
        }
    }

    pub fn get_or_create(&self, service: &str) -> Arc<CircuitBreaker> {
        self.breakers
            .entry(service.to_string())
            .or_insert_with(|| Arc::new(CircuitBreaker::new(service, self.config.clone())))
            .clone()
    }

    pub fn get(&self, service: &str) -> Option<Arc<CircuitBreaker>> {
        self.breakers.get(service).map(|r| r.clone())
    }
}

/// Rate limit error
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded for tenant {tenant_id}. Retry after {retry_after_secs} seconds")]
    Exceeded {
        tenant_id: String,
        retry_after_secs: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resilience_manager_creation() {
        let config = ResilienceConfig::default();
        let manager = ResilienceManager::new(config);

        // Get circuit breaker
        let cb = manager.circuit_breaker("test-service");
        assert!(cb.is_closed());
    }

    #[test]
    fn test_circuit_breaker_registry() {
        let config = CircuitBreakerConfig::default();
        let registry = CircuitBreakerRegistry::new(config);

        let cb1 = registry.get_or_create("service-a");
        let cb2 = registry.get_or_create("service-a");
        let cb3 = registry.get_or_create("service-b");

        // Same service returns same instance
        assert!(Arc::ptr_eq(&cb1, &cb2));
        // Different service returns different instance
        assert!(!Arc::ptr_eq(&cb1, &cb3));
    }
}
