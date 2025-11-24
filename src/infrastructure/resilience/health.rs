//! Health check system for monitoring dependencies
//!
//! Provides liveness and readiness probes for Kubernetes deployments

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::Serialize;

/// Health status for a dependency
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Health information for a single dependency
#[derive(Debug, Clone, Serialize)]
pub struct DependencyHealth {
    pub name: String,
    pub status: DependencyStatus,
    pub latency_ms: Option<u64>,
    pub message: Option<String>,
    #[serde(skip)]
    pub last_check: Option<Instant>,
}

impl DependencyHealth {
    pub fn healthy(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: DependencyStatus::Healthy,
            latency_ms: None,
            message: None,
            last_check: Some(Instant::now()),
        }
    }

    pub fn unhealthy(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            status: DependencyStatus::Unhealthy,
            latency_ms: None,
            message: Some(message.to_string()),
            last_check: Some(Instant::now()),
        }
    }

    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency_ms = Some(latency.as_millis() as u64);
        self
    }
}

/// Overall health status
#[derive(Debug, Clone, Serialize)]
pub struct HealthStatus {
    pub status: DependencyStatus,
    pub dependencies: HashMap<String, DependencyHealth>,
    pub version: String,
    pub uptime_secs: u64,
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        self.status == DependencyStatus::Healthy
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.status, DependencyStatus::Healthy | DependencyStatus::Degraded)
    }
}

/// Health check function type
pub type HealthCheckFn = Arc<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = DependencyHealth> + Send>> + Send + Sync>;

/// Health checker for all dependencies
pub struct HealthChecker {
    checks: RwLock<HashMap<String, HealthCheckFn>>,
    start_time: Instant,
    version: String,
}

impl HealthChecker {
    pub fn new() -> Self {
        Self {
            checks: RwLock::new(HashMap::new()),
            start_time: Instant::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Register a health check
    pub async fn register<F, Fut>(&self, name: &str, check: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = DependencyHealth> + Send + 'static,
    {
        let check_fn: HealthCheckFn = Arc::new(move || Box::pin(check()));
        self.checks.write().await.insert(name.to_string(), check_fn);
    }

    /// Check all dependencies
    pub async fn check_all(&self) -> HealthStatus {
        let checks = self.checks.read().await;
        let mut dependencies = HashMap::new();
        let mut overall_status = DependencyStatus::Healthy;

        for (name, check) in checks.iter() {
            let start = Instant::now();
            let mut health = check().await;
            health.latency_ms = Some(start.elapsed().as_millis() as u64);

            match health.status {
                DependencyStatus::Unhealthy => {
                    overall_status = DependencyStatus::Unhealthy;
                }
                DependencyStatus::Degraded if overall_status == DependencyStatus::Healthy => {
                    overall_status = DependencyStatus::Degraded;
                }
                _ => {}
            }

            dependencies.insert(name.clone(), health);
        }

        HealthStatus {
            status: overall_status,
            dependencies,
            version: self.version.clone(),
            uptime_secs: self.start_time.elapsed().as_secs(),
        }
    }

    /// Quick liveness check
    pub fn liveness(&self) -> bool {
        // Service is alive if it can respond
        true
    }

    /// Readiness check
    pub async fn readiness(&self) -> bool {
        let status = self.check_all().await;
        status.is_ready()
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Built-in health checks
pub mod checks {
    use super::*;

    /// Database health check
    pub async fn database_check<F, Fut>(ping: F) -> DependencyHealth
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(), anyhow::Error>>,
    {
        let start = Instant::now();
        match ping().await {
            Ok(()) => DependencyHealth::healthy("database")
                .with_latency(start.elapsed()),
            Err(e) => DependencyHealth::unhealthy("database", &e.to_string()),
        }
    }

    /// HTTP endpoint health check
    pub async fn http_check(name: &str, url: &str, timeout: Duration) -> DependencyHealth {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build();

        let client = match client {
            Ok(c) => c,
            Err(e) => return DependencyHealth::unhealthy(name, &e.to_string()),
        };

        let start = Instant::now();
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                DependencyHealth::healthy(name).with_latency(start.elapsed())
            }
            Ok(resp) => {
                DependencyHealth::unhealthy(name, &format!("HTTP {}", resp.status()))
            }
            Err(e) => DependencyHealth::unhealthy(name, &e.to_string()),
        }
    }

    /// Typst installation check
    pub async fn typst_check() -> DependencyHealth {
        use tokio::process::Command;

        let start = Instant::now();
        match Command::new("typst").arg("--version").output().await {
            Ok(output) if output.status.success() => {
                DependencyHealth::healthy("typst").with_latency(start.elapsed())
            }
            Ok(_) => DependencyHealth::unhealthy("typst", "typst command failed"),
            Err(e) => DependencyHealth::unhealthy("typst", &format!("typst not found: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_health_creation() {
        let health = DependencyHealth::healthy("test");
        assert_eq!(health.status, DependencyStatus::Healthy);
        assert_eq!(health.name, "test");

        let health = DependencyHealth::unhealthy("test", "connection failed");
        assert_eq!(health.status, DependencyStatus::Unhealthy);
        assert_eq!(health.message, Some("connection failed".to_string()));
    }

    #[test]
    fn test_health_status_checks() {
        let status = HealthStatus {
            status: DependencyStatus::Healthy,
            dependencies: HashMap::new(),
            version: "1.0.0".to_string(),
            uptime_secs: 100,
        };
        assert!(status.is_healthy());
        assert!(status.is_ready());

        let status = HealthStatus {
            status: DependencyStatus::Degraded,
            dependencies: HashMap::new(),
            version: "1.0.0".to_string(),
            uptime_secs: 100,
        };
        assert!(!status.is_healthy());
        assert!(status.is_ready());

        let status = HealthStatus {
            status: DependencyStatus::Unhealthy,
            dependencies: HashMap::new(),
            version: "1.0.0".to_string(),
            uptime_secs: 100,
        };
        assert!(!status.is_healthy());
        assert!(!status.is_ready());
    }

    #[tokio::test]
    async fn test_health_checker() {
        let checker = HealthChecker::new();

        checker.register("always_healthy", || async {
            DependencyHealth::healthy("always_healthy")
        }).await;

        let status = checker.check_all().await;
        assert!(status.is_healthy());
        assert_eq!(status.dependencies.len(), 1);
    }

    #[test]
    fn test_health_checker_liveness() {
        let checker = HealthChecker::new();
        assert!(checker.liveness());
    }
}
