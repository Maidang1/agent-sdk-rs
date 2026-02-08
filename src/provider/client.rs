use std::sync::Arc;
use reqwest::Client;
use crate::provider::{Result, ProviderError};
use super::retry::{RetryConfig, RetryPolicy};
use super::rate_limit::{RateLimitConfig, RateLimiter, RateLimitGuard};
use super::timeout::TimeoutConfig;

/// Shared HTTP client with retry, rate limiting, and timeout support
#[derive(Debug, Clone)]
pub struct ProviderClient {
    http_client: Client,
    retry_policy: Arc<RetryPolicy>,
    rate_limiter: Arc<RateLimiter>,
}

impl ProviderClient {
    /// Create a new provider client with the given configuration
    pub fn new(
        http_client: Client,
        retry_policy: RetryPolicy,
        rate_limiter: RateLimiter,
    ) -> Self {
        Self {
            http_client,
            retry_policy: Arc::new(retry_policy),
            rate_limiter: Arc::new(rate_limiter),
        }
    }

    /// Get a reference to the underlying HTTP client
    pub fn http_client(&self) -> &Client {
        &self.http_client
    }

    /// Get a reference to the retry policy
    pub fn retry_policy(&self) -> &RetryPolicy {
        &self.retry_policy
    }

    /// Get a reference to the rate limiter
    pub fn rate_limiter(&self) -> &RateLimiter {
        &self.rate_limiter
    }

    /// Acquire a rate limit permit
    pub async fn acquire_rate_limit(&self) -> RateLimitGuard {
        self.rate_limiter.acquire().await
    }

    /// Create a builder for configuring a provider client
    pub fn builder() -> ProviderClientBuilder {
        ProviderClientBuilder::default()
    }
}

/// Builder for creating a ProviderClient with custom configuration
#[derive(Debug)]
pub struct ProviderClientBuilder {
    retry_config: RetryConfig,
    timeout_config: TimeoutConfig,
    rate_limit_config: RateLimitConfig,
    proxy: Option<String>,
    user_agent: Option<String>,
}

impl Default for ProviderClientBuilder {
    fn default() -> Self {
        Self {
            retry_config: RetryConfig::default(),
            timeout_config: TimeoutConfig::default(),
            rate_limit_config: RateLimitConfig::default(),
            proxy: None,
            user_agent: Some(format!(
                "agent-sdk-rs/{}",
                env!("CARGO_PKG_VERSION")
            )),
        }
    }
}

impl ProviderClientBuilder {
    /// Set the retry configuration
    pub fn retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// Set the timeout configuration
    pub fn timeout_config(mut self, config: TimeoutConfig) -> Self {
        self.timeout_config = config;
        self
    }

    /// Set the rate limit configuration
    pub fn rate_limit_config(mut self, config: RateLimitConfig) -> Self {
        self.rate_limit_config = config;
        self
    }

    /// Set a proxy URL
    pub fn proxy(mut self, proxy: impl Into<String>) -> Self {
        self.proxy = Some(proxy.into());
        self
    }

    /// Set a custom user agent
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Disable retries
    pub fn no_retry(mut self) -> Self {
        self.retry_config = RetryConfig::none();
        self
    }

    /// Disable rate limiting
    pub fn no_rate_limit(mut self) -> Self {
        self.rate_limit_config = RateLimitConfig::unlimited();
        self
    }

    /// Build the provider client
    pub fn build(self) -> Result<ProviderClient> {
        let mut client_builder = Client::builder()
            .connect_timeout(self.timeout_config.connect_timeout)
            .timeout(self.timeout_config.request_timeout);

        // Avoid reading system proxy settings in environments where it may panic
        // (e.g. headless CI/macOS sandbox without a dynamic store).
        if self.proxy.is_none() {
            client_builder = client_builder.no_proxy();
        }

        if let Some(user_agent) = self.user_agent {
            client_builder = client_builder.user_agent(user_agent);
        }

        if let Some(proxy_url) = self.proxy {
            let proxy = reqwest::Proxy::all(&proxy_url)
                .map_err(|e| ProviderError::RequestFailed(format!("Invalid proxy: {}", e)))?;
            client_builder = client_builder.proxy(proxy);
        }

        let http_client = client_builder
            .build()
            .map_err(|e| ProviderError::RequestFailed(format!("Failed to build HTTP client: {}", e)))?;

        let retry_policy = RetryPolicy::new(self.retry_config);
        let rate_limiter = RateLimiter::new(self.rate_limit_config);

        Ok(ProviderClient::new(http_client, retry_policy, rate_limiter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default() {
        let client = ProviderClient::builder().build();
        assert!(client.is_ok());
    }

    #[test]
    fn test_builder_custom_config() {
        let client = ProviderClient::builder()
            .retry_config(RetryConfig::aggressive())
            .timeout_config(TimeoutConfig::fast())
            .rate_limit_config(RateLimitConfig::conservative())
            .build();
        assert!(client.is_ok());
    }

    #[test]
    fn test_builder_no_retry() {
        let client = ProviderClient::builder()
            .no_retry()
            .build();
        assert!(client.is_ok());
    }

    #[test]
    fn test_builder_with_proxy() {
        let client = ProviderClient::builder()
            .proxy("http://localhost:8080")
            .build();
        assert!(client.is_ok());
    }
}
