use std::time::Duration;
use std::future::Future;
use std::pin::Pin;
use crate::provider::{Result, ProviderError};

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial backoff duration before first retry
    pub initial_backoff: Duration,
    /// Maximum backoff duration between retries
    pub max_backoff: Duration,
    /// Multiplier for exponential backoff (typically 2.0)
    pub backoff_multiplier: f64,
    /// Whether to retry on timeout errors
    pub retry_on_timeout: bool,
    /// Whether to retry on rate limit errors
    pub retry_on_rate_limit: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            retry_on_timeout: true,
            retry_on_rate_limit: true,
        }
    }
}

impl RetryConfig {
    /// Create a new retry configuration with custom values
    pub fn new(max_retries: u32, initial_backoff: Duration) -> Self {
        Self {
            max_retries,
            initial_backoff,
            ..Default::default()
        }
    }

    /// Create a configuration with no retries
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            ..Default::default()
        }
    }

    /// Create a configuration with aggressive retries
    pub fn aggressive() -> Self {
        Self {
            max_retries: 5,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            retry_on_timeout: true,
            retry_on_rate_limit: true,
        }
    }
}

/// Policy for handling retries with exponential backoff
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    config: RetryConfig,
}

impl RetryPolicy {
    /// Create a new retry policy with the given configuration
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Determine if an error should be retried
    pub fn should_retry(&self, error: &ProviderError, attempt: u32) -> bool {
        if attempt >= self.config.max_retries {
            return false;
        }

        match error {
            // Always retry server errors
            ProviderError::RequestFailed(msg) => {
                msg.contains("502") || msg.contains("503") || msg.contains("504")
            }
            // Retry rate limits if configured
            ProviderError::RateLimited { .. } => self.config.retry_on_rate_limit,
            // Retry timeouts if configured
            ProviderError::RequestFailed(msg) if msg.contains("timeout") => {
                self.config.retry_on_timeout
            }
            // Don't retry authentication or parse errors
            ProviderError::AuthenticationFailed(_) | ProviderError::ParseError(_) => false,
            // Don't retry model not available
            ProviderError::ModelNotAvailable(_) => false,
            // Don't retry other errors by default
            ProviderError::Other(_) => false,
        }
    }

    /// Calculate the backoff duration for a given attempt
    pub fn calculate_backoff(&self, attempt: u32) -> Duration {
        let backoff_ms = self.config.initial_backoff.as_millis() as f64
            * self.config.backoff_multiplier.powi(attempt as i32);

        let backoff = Duration::from_millis(backoff_ms as u64);

        // Cap at max_backoff
        if backoff > self.config.max_backoff {
            self.config.max_backoff
        } else {
            backoff
        }
    }

    /// Execute an operation with retry logic
    pub async fn execute_with_retry<F, Fut, T>(&self, mut operation: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut attempt = 0;

        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    if !self.should_retry(&error, attempt) {
                        return Err(error);
                    }

                    let backoff = self.calculate_backoff(attempt);

                    // Log retry attempt (optional, only if tracing is available)
                    #[cfg(feature = "tracing")]
                    tracing::debug!(
                        "Retry attempt {} after error: {:?}. Waiting {:?}",
                        attempt + 1,
                        error,
                        backoff
                    );

                    tokio::time::sleep(backoff).await;
                    attempt += 1;
                }
            }
        }
    }

    /// Execute an operation with retry logic, allowing inspection of retry attempts
    pub async fn execute_with_retry_and_callback<F, Fut, T, C>(
        &self,
        mut operation: F,
        mut on_retry: C,
    ) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T>>,
        C: FnMut(u32, &ProviderError, Duration),
    {
        let mut attempt = 0;

        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    if !self.should_retry(&error, attempt) {
                        return Err(error);
                    }

                    let backoff = self.calculate_backoff(attempt);
                    on_retry(attempt + 1, &error, backoff);

                    tokio::time::sleep(backoff).await;
                    attempt += 1;
                }
            }
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new(RetryConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_retry_server_errors() {
        let policy = RetryPolicy::default();

        assert!(policy.should_retry(
            &ProviderError::RequestFailed("502 Bad Gateway".to_string()),
            0
        ));
        assert!(policy.should_retry(
            &ProviderError::RequestFailed("503 Service Unavailable".to_string()),
            0
        ));
    }

    #[test]
    fn test_should_not_retry_auth_errors() {
        let policy = RetryPolicy::default();

        assert!(!policy.should_retry(
            &ProviderError::AuthenticationFailed("Invalid API key".to_string()),
            0
        ));
    }

    #[test]
    fn test_should_not_retry_after_max_attempts() {
        let policy = RetryPolicy::new(RetryConfig {
            max_retries: 2,
            ..Default::default()
        });

        assert!(!policy.should_retry(
            &ProviderError::RequestFailed("502 Bad Gateway".to_string()),
            2
        ));
    }

    #[test]
    fn test_exponential_backoff() {
        let policy = RetryPolicy::new(RetryConfig {
            initial_backoff: Duration::from_millis(100),
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(10),
            ..Default::default()
        });

        assert_eq!(policy.calculate_backoff(0), Duration::from_millis(100));
        assert_eq!(policy.calculate_backoff(1), Duration::from_millis(200));
        assert_eq!(policy.calculate_backoff(2), Duration::from_millis(400));
        assert_eq!(policy.calculate_backoff(3), Duration::from_millis(800));
    }

    #[test]
    fn test_backoff_capped_at_max() {
        let policy = RetryPolicy::new(RetryConfig {
            initial_backoff: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            max_backoff: Duration::from_secs(5),
            ..Default::default()
        });

        assert_eq!(policy.calculate_backoff(10), Duration::from_secs(5));
    }
}
