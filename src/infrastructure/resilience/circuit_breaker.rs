//! Circuit Breaker pattern implementation
//!
//! Prevents cascading failures by stopping calls to failing services

use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CircuitState {
    /// Circuit is closed - requests flow normally
    Closed = 0,
    /// Circuit is open - requests are blocked
    Open = 1,
    /// Circuit is half-open - testing if service recovered
    HalfOpen = 2,
}

impl From<u8> for CircuitState {
    fn from(v: u8) -> Self {
        match v {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: u32,
    /// Duration to keep circuit open before testing
    pub open_duration: Duration,
    /// Number of successes in half-open to close circuit
    pub success_threshold: u32,
    /// Window for counting failures
    pub failure_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            open_duration: Duration::from_secs(30),
            success_threshold: 3,
            failure_window: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker for protecting external service calls
pub struct CircuitBreaker {
    name: String,
    config: CircuitBreakerConfig,
    state: AtomicU8,
    failure_count: AtomicU64,
    success_count: AtomicU64,
    last_failure_time: RwLock<Option<Instant>>,
    opened_at: RwLock<Option<Instant>>,
}

impl CircuitBreaker {
    pub fn new(name: &str, config: CircuitBreakerConfig) -> Self {
        Self {
            name: name.to_string(),
            config,
            state: AtomicU8::new(CircuitState::Closed as u8),
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            last_failure_time: RwLock::new(None),
            opened_at: RwLock::new(None),
        }
    }

    /// Check if request is allowed
    pub fn is_allowed(&self) -> bool {
        match self.state() {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if we should transition to half-open
                if self.should_attempt_reset() {
                    self.transition_to_half_open();
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful call
    pub fn record_success(&self) {
        match self.state() {
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::SeqCst);
            }
            CircuitState::HalfOpen => {
                let count = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                if count >= self.config.success_threshold as u64 {
                    self.transition_to_closed();
                }
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed call
    pub fn record_failure(&self) {
        *self.last_failure_time.write() = Some(Instant::now());

        match self.state() {
            CircuitState::Closed => {
                let count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                if count >= self.config.failure_threshold as u64 {
                    self.transition_to_open();
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open immediately opens the circuit
                self.transition_to_open();
            }
            CircuitState::Open => {}
        }
    }

    /// Get current state
    pub fn state(&self) -> CircuitState {
        CircuitState::from(self.state.load(Ordering::SeqCst))
    }

    /// Check if circuit is closed
    pub fn is_closed(&self) -> bool {
        self.state() == CircuitState::Closed
    }

    /// Check if circuit is open
    pub fn is_open(&self) -> bool {
        self.state() == CircuitState::Open
    }

    fn should_attempt_reset(&self) -> bool {
        if let Some(opened_at) = *self.opened_at.read() {
            opened_at.elapsed() >= self.config.open_duration
        } else {
            false
        }
    }

    fn transition_to_open(&self) {
        self.state.store(CircuitState::Open as u8, Ordering::SeqCst);
        *self.opened_at.write() = Some(Instant::now());
        self.success_count.store(0, Ordering::SeqCst);
        warn!(
            circuit_breaker = %self.name,
            "Circuit breaker OPENED after {} failures",
            self.failure_count.load(Ordering::SeqCst)
        );
    }

    fn transition_to_half_open(&self) {
        self.state
            .store(CircuitState::HalfOpen as u8, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
        info!(
            circuit_breaker = %self.name,
            "Circuit breaker HALF-OPEN, testing service"
        );
    }

    fn transition_to_closed(&self) {
        self.state
            .store(CircuitState::Closed as u8, Ordering::SeqCst);
        self.failure_count.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
        *self.opened_at.write() = None;
        info!(
            circuit_breaker = %self.name,
            "Circuit breaker CLOSED, service recovered"
        );
    }

    /// Get circuit breaker metrics
    pub fn metrics(&self) -> CircuitBreakerMetrics {
        CircuitBreakerMetrics {
            name: self.name.clone(),
            state: self.state(),
            failure_count: self.failure_count.load(Ordering::SeqCst),
            success_count: self.success_count.load(Ordering::SeqCst),
        }
    }
}

/// Circuit breaker metrics
#[derive(Debug, Clone)]
pub struct CircuitBreakerMetrics {
    pub name: String,
    pub state: CircuitState,
    pub failure_count: u64,
    pub success_count: u64,
}

/// Error when circuit is open
#[derive(Debug, thiserror::Error)]
#[error("Circuit breaker '{name}' is open - service unavailable")]
pub struct CircuitOpenError {
    pub name: String,
}

/// Execute function with circuit breaker protection
pub async fn with_circuit_breaker<F, Fut, T, E>(
    cb: &CircuitBreaker,
    f: F,
) -> Result<T, CircuitBreakerError<E>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    if !cb.is_allowed() {
        return Err(CircuitBreakerError::Open(CircuitOpenError {
            name: cb.name.clone(),
        }));
    }

    match f().await {
        Ok(result) => {
            cb.record_success();
            Ok(result)
        }
        Err(e) => {
            cb.record_failure();
            Err(CircuitBreakerError::Inner(e))
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CircuitBreakerError<E> {
    #[error(transparent)]
    Open(#[from] CircuitOpenError),
    #[error(transparent)]
    Inner(E),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_initial_state() {
        let config = CircuitBreakerConfig::default();
        let cb = CircuitBreaker::new("test", config);

        assert!(cb.is_closed());
        assert!(cb.is_allowed());
    }

    #[test]
    fn test_circuit_breaker_opens_after_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::new("test", config);

        // Record failures
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_closed());

        cb.record_failure();
        assert!(cb.is_open());
        assert!(!cb.is_allowed());
    }

    #[test]
    fn test_circuit_breaker_success_resets_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::new("test", config);

        cb.record_failure();
        cb.record_failure();
        cb.record_success();

        // Failure count should be reset
        assert_eq!(cb.failure_count.load(Ordering::SeqCst), 0);
        assert!(cb.is_closed());
    }

    #[test]
    fn test_circuit_breaker_metrics() {
        let config = CircuitBreakerConfig::default();
        let cb = CircuitBreaker::new("test-service", config);

        cb.record_failure();
        cb.record_failure();

        let metrics = cb.metrics();
        assert_eq!(metrics.name, "test-service");
        assert_eq!(metrics.state, CircuitState::Closed);
        assert_eq!(metrics.failure_count, 2);
    }
}
