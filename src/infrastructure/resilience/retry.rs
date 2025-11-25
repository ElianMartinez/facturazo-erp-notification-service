//! Retry logic with exponential backoff
//!
//! Automatically retries failed operations with configurable backoff

use std::future::Future;
use std::time::Duration;
use tracing::{debug, warn};

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub multiplier: f64,
    /// Add jitter to prevent thundering herd
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
            jitter: true,
        }
    }
}

/// Retry policy
pub struct RetryPolicy {
    config: RetryConfig,
}

impl RetryPolicy {
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Calculate delay for attempt number
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base_delay = self.config.initial_delay.as_millis() as f64
            * self.config.multiplier.powi(attempt as i32);

        let delay_ms = base_delay.min(self.config.max_delay.as_millis() as f64);

        let final_delay = if self.config.jitter {
            // Add up to 25% jitter
            let jitter = rand::random::<f64>() * 0.25 * delay_ms;
            delay_ms + jitter
        } else {
            delay_ms
        };

        Duration::from_millis(final_delay as u64)
    }

    /// Check if should retry
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.config.max_attempts
    }
}

/// Execute with retry logic
pub async fn with_retry<F, Fut, T, E>(
    config: &RetryConfig,
    operation_name: &str,
    mut f: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let policy = RetryPolicy::new(config.clone());
    let mut attempt = 0;

    loop {
        match f().await {
            Ok(result) => {
                if attempt > 0 {
                    debug!(
                        operation = %operation_name,
                        attempt = attempt + 1,
                        "Operation succeeded after retry"
                    );
                }
                return Ok(result);
            }
            Err(e) => {
                if !policy.should_retry(attempt) {
                    warn!(
                        operation = %operation_name,
                        attempts = attempt + 1,
                        error = %e,
                        "Operation failed after all retries"
                    );
                    return Err(e);
                }

                let delay = policy.delay_for_attempt(attempt);
                warn!(
                    operation = %operation_name,
                    attempt = attempt + 1,
                    max_attempts = config.max_attempts,
                    delay_ms = delay.as_millis(),
                    error = %e,
                    "Operation failed, retrying"
                );

                tokio::time::sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

/// Retry with condition
pub async fn with_retry_if<F, Fut, T, E, P>(
    config: &RetryConfig,
    operation_name: &str,
    mut f: F,
    should_retry: P,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
    P: Fn(&E) -> bool,
{
    let policy = RetryPolicy::new(config.clone());
    let mut attempt = 0;

    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if !policy.should_retry(attempt) || !should_retry(&e) {
                    return Err(e);
                }

                let delay = policy.delay_for_attempt(attempt);
                warn!(
                    operation = %operation_name,
                    attempt = attempt + 1,
                    delay_ms = delay.as_millis(),
                    error = %e,
                    "Operation failed, retrying"
                );

                tokio::time::sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

/// Builder for retry configuration
pub struct RetryBuilder {
    config: RetryConfig,
}

impl RetryBuilder {
    pub fn new() -> Self {
        Self {
            config: RetryConfig::default(),
        }
    }

    pub fn max_attempts(mut self, attempts: u32) -> Self {
        self.config.max_attempts = attempts;
        self
    }

    pub fn initial_delay(mut self, delay: Duration) -> Self {
        self.config.initial_delay = delay;
        self
    }

    pub fn max_delay(mut self, delay: Duration) -> Self {
        self.config.max_delay = delay;
        self
    }

    pub fn multiplier(mut self, multiplier: f64) -> Self {
        self.config.multiplier = multiplier;
        self
    }

    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.config.jitter = jitter;
        self
    }

    pub fn build(self) -> RetryConfig {
        self.config
    }
}

impl Default for RetryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_retry_policy_delay_calculation() {
        let config = RetryConfig {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
            jitter: false,
            ..Default::default()
        };
        let policy = RetryPolicy::new(config);

        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(400));
    }

    #[test]
    fn test_retry_policy_max_delay() {
        let config = RetryConfig {
            initial_delay: Duration::from_millis(1000),
            max_delay: Duration::from_millis(2000),
            multiplier: 10.0,
            jitter: false,
            ..Default::default()
        };
        let policy = RetryPolicy::new(config);

        // Should be capped at max_delay
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(2000));
    }

    #[tokio::test]
    async fn test_with_retry_succeeds_first_try() {
        let config = RetryConfig::default();
        let counter = AtomicU32::new(0);

        let result: Result<i32, &str> = with_retry(&config, "test", || async {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(42)
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_with_retry_succeeds_after_failures() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(1),
            jitter: false,
            ..Default::default()
        };
        let counter = AtomicU32::new(0);

        let result: Result<i32, &str> = with_retry(&config, "test", || async {
            let count = counter.fetch_add(1, Ordering::SeqCst);
            if count < 2 {
                Err("failed")
            } else {
                Ok(42)
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_retry_builder() {
        let config = RetryBuilder::new()
            .max_attempts(5)
            .initial_delay(Duration::from_millis(50))
            .max_delay(Duration::from_secs(5))
            .multiplier(1.5)
            .with_jitter(false)
            .build();

        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.initial_delay, Duration::from_millis(50));
        assert_eq!(config.max_delay, Duration::from_secs(5));
        assert_eq!(config.multiplier, 1.5);
        assert!(!config.jitter);
    }
}
