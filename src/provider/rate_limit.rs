use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Semaphore, RwLock};

/// Configuration for rate limiting
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum number of requests per minute
    pub requests_per_minute: u32,
    /// Optional maximum tokens per minute
    pub tokens_per_minute: Option<u32>,
    /// Maximum number of concurrent requests
    pub concurrent_requests: usize,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 60,
            tokens_per_minute: None,
            concurrent_requests: 10,
        }
    }
}

impl RateLimitConfig {
    /// Create a new rate limit configuration
    pub fn new(requests_per_minute: u32, concurrent_requests: usize) -> Self {
        Self {
            requests_per_minute,
            tokens_per_minute: None,
            concurrent_requests,
        }
    }

    /// Create a configuration with no rate limiting
    pub fn unlimited() -> Self {
        Self {
            requests_per_minute: u32::MAX,
            tokens_per_minute: None,
            concurrent_requests: 1000,
        }
    }

    /// Create a conservative rate limit configuration
    pub fn conservative() -> Self {
        Self {
            requests_per_minute: 30,
            tokens_per_minute: None,
            concurrent_requests: 5,
        }
    }

    /// Create an aggressive rate limit configuration
    pub fn aggressive() -> Self {
        Self {
            requests_per_minute: 120,
            tokens_per_minute: None,
            concurrent_requests: 20,
        }
    }
}

/// Rate limiter using sliding window and semaphore for concurrency control
#[derive(Debug)]
pub struct RateLimiter {
    config: RateLimitConfig,
    /// Semaphore for controlling concurrent requests
    semaphore: Arc<Semaphore>,
    /// Sliding window of request timestamps
    request_times: Arc<RwLock<Vec<Instant>>>,
    /// Sliding window of token usage
    token_usage: Arc<RwLock<Vec<(Instant, u32)>>>,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(config.concurrent_requests)),
            request_times: Arc::new(RwLock::new(Vec::new())),
            token_usage: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    /// Acquire a permit to make a request, waiting if necessary
    pub async fn acquire(&self) -> RateLimitGuard {
        // Acquire semaphore permit for concurrency control
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("Semaphore closed");

        // Wait for rate limit window if needed
        self.wait_for_rate_limit().await;

        // Record this request
        let now = Instant::now();
        let mut times = self.request_times.write().await;
        times.push(now);

        RateLimitGuard {
            _permit: permit,
            rate_limiter: self.clone(),
        }
    }

    /// Wait until we're within the rate limit window
    async fn wait_for_rate_limit(&self) {
        loop {
            let now = Instant::now();
            let window_start = now - Duration::from_secs(60);

            // Clean up old entries and count recent requests
            let mut times = self.request_times.write().await;
            times.retain(|&time| time > window_start);

            let recent_requests = times.len() as u32;

            if recent_requests < self.config.requests_per_minute {
                break;
            }

            // Calculate how long to wait
            if let Some(oldest) = times.first() {
                let wait_duration = Duration::from_secs(60) - now.duration_since(*oldest);
                drop(times); // Release lock before sleeping

                // Log rate limit wait (optional, only if tracing is available)
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    "Rate limit reached ({}/{}), waiting {:?}",
                    recent_requests,
                    self.config.requests_per_minute,
                    wait_duration
                );

                tokio::time::sleep(wait_duration).await;
            } else {
                break;
            }
        }

        // Check token rate limit if configured
        if let Some(max_tokens) = self.config.tokens_per_minute {
            self.wait_for_token_limit(max_tokens).await;
        }
    }

    /// Wait until we're within the token rate limit window
    async fn wait_for_token_limit(&self, max_tokens: u32) {
        loop {
            let now = Instant::now();
            let window_start = now - Duration::from_secs(60);

            let mut usage = self.token_usage.write().await;
            usage.retain(|(time, _)| *time > window_start);

            let recent_tokens: u32 = usage.iter().map(|(_, tokens)| tokens).sum();

            if recent_tokens < max_tokens {
                break;
            }

            // Calculate how long to wait
            if let Some((oldest_time, _)) = usage.first() {
                let wait_duration = Duration::from_secs(60) - now.duration_since(*oldest_time);
                drop(usage); // Release lock before sleeping

                // Log token rate limit wait (optional, only if tracing is available)
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    "Token rate limit reached ({}/{}), waiting {:?}",
                    recent_tokens,
                    max_tokens,
                    wait_duration
                );

                tokio::time::sleep(wait_duration).await;
            } else {
                break;
            }
        }
    }

    /// Record token usage for rate limiting
    pub async fn record_tokens(&self, tokens: u32) {
        if self.config.tokens_per_minute.is_some() {
            let mut usage = self.token_usage.write().await;
            usage.push((Instant::now(), tokens));
        }
    }

    /// Get current rate limit statistics
    pub async fn stats(&self) -> RateLimitStats {
        let now = Instant::now();
        let window_start = now - Duration::from_secs(60);

        let times = self.request_times.read().await;
        let recent_requests = times.iter().filter(|&&time| time > window_start).count() as u32;

        let usage = self.token_usage.read().await;
        let recent_tokens: u32 = usage
            .iter()
            .filter(|(time, _)| *time > window_start)
            .map(|(_, tokens)| tokens)
            .sum();

        RateLimitStats {
            requests_in_window: recent_requests,
            requests_per_minute_limit: self.config.requests_per_minute,
            tokens_in_window: if self.config.tokens_per_minute.is_some() {
                Some(recent_tokens)
            } else {
                None
            },
            tokens_per_minute_limit: self.config.tokens_per_minute,
            available_permits: self.semaphore.available_permits(),
            max_concurrent: self.config.concurrent_requests,
        }
    }
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            semaphore: Arc::clone(&self.semaphore),
            request_times: Arc::clone(&self.request_times),
            token_usage: Arc::clone(&self.token_usage),
        }
    }
}

/// Guard that releases rate limit resources when dropped
pub struct RateLimitGuard {
    _permit: tokio::sync::OwnedSemaphorePermit,
    rate_limiter: RateLimiter,
}

impl RateLimitGuard {
    /// Record token usage for this request
    pub async fn record_tokens(&self, tokens: u32) {
        self.rate_limiter.record_tokens(tokens).await;
    }
}

/// Statistics about current rate limit usage
#[derive(Debug, Clone)]
pub struct RateLimitStats {
    /// Number of requests in the current window
    pub requests_in_window: u32,
    /// Maximum requests per minute
    pub requests_per_minute_limit: u32,
    /// Number of tokens used in the current window (if tracking)
    pub tokens_in_window: Option<u32>,
    /// Maximum tokens per minute (if configured)
    pub tokens_per_minute_limit: Option<u32>,
    /// Number of available concurrent request permits
    pub available_permits: usize,
    /// Maximum concurrent requests
    pub max_concurrent: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_concurrent_limit() {
        let limiter = RateLimiter::new(RateLimitConfig {
            requests_per_minute: 1000,
            tokens_per_minute: None,
            concurrent_requests: 2,
        });

        let _guard1 = limiter.acquire().await;
        let _guard2 = limiter.acquire().await;

        let stats = limiter.stats().await;
        assert_eq!(stats.available_permits, 0);
    }

    #[tokio::test]
    async fn test_rate_limit_stats() {
        let limiter = RateLimiter::new(RateLimitConfig {
            requests_per_minute: 60,
            tokens_per_minute: Some(10000),
            concurrent_requests: 5,
        });

        let guard = limiter.acquire().await;
        guard.record_tokens(100).await;

        let stats = limiter.stats().await;
        assert_eq!(stats.requests_in_window, 1);
        assert_eq!(stats.tokens_in_window, Some(100));
        assert_eq!(stats.available_permits, 4);
    }
}
