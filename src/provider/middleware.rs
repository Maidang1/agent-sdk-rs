use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;
use super::{Message, GenerateOptions, GenerateResponse, ProviderError, Result};

/// Context passed to middleware before a request
#[derive(Debug)]
pub struct RequestContext {
    pub messages: Vec<Message>,
    pub options: Option<GenerateOptions>,
    pub metadata: HashMap<String, String>,
}

/// Context passed to middleware after a response
#[derive(Debug)]
pub struct ResponseContext {
    pub response: GenerateResponse,
    pub metadata: HashMap<String, String>,
}

/// Middleware trait for intercepting provider requests and responses
#[async_trait]
pub trait Middleware: Send + Sync {
    /// Called before a request is sent to the provider
    async fn before_request(&self, ctx: &mut RequestContext) -> Result<()> {
        let _ = ctx;
        Ok(())
    }

    /// Called after a successful response is received
    async fn after_response(&self, ctx: &mut ResponseContext) -> Result<()> {
        let _ = ctx;
        Ok(())
    }

    /// Called when an error occurs
    async fn on_error(&self, error: &ProviderError) -> Result<()> {
        let _ = error;
        Ok(())
    }
}

/// Chain of middleware that executes in order
#[derive(Clone)]
pub struct MiddlewareChain {
    middlewares: Vec<Arc<dyn Middleware>>,
}

impl MiddlewareChain {
    /// Create a new empty middleware chain
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    /// Add a middleware to the chain
    pub fn add(mut self, middleware: Arc<dyn Middleware>) -> Self {
        self.middlewares.push(middleware);
        self
    }

    /// Execute all middleware before_request hooks
    pub async fn execute_before(&self, ctx: &mut RequestContext) -> Result<()> {
        for middleware in &self.middlewares {
            middleware.before_request(ctx).await?;
        }
        Ok(())
    }

    /// Execute all middleware after_response hooks
    pub async fn execute_after(&self, ctx: &mut ResponseContext) -> Result<()> {
        for middleware in &self.middlewares {
            middleware.after_response(ctx).await?;
        }
        Ok(())
    }

    /// Execute all middleware on_error hooks
    pub async fn execute_error(&self, error: &ProviderError) -> Result<()> {
        for middleware in &self.middlewares {
            middleware.on_error(error).await?;
        }
        Ok(())
    }
}

impl Default for MiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}

/// Built-in middleware for logging requests and responses
pub struct LoggingMiddleware {
    log_requests: bool,
    log_responses: bool,
    log_errors: bool,
}

impl LoggingMiddleware {
    /// Create a new logging middleware that logs everything
    pub fn new() -> Self {
        Self {
            log_requests: true,
            log_responses: true,
            log_errors: true,
        }
    }

    /// Create a logging middleware with custom settings
    pub fn with_config(log_requests: bool, log_responses: bool, log_errors: bool) -> Self {
        Self {
            log_requests,
            log_responses,
            log_errors,
        }
    }
}

impl Default for LoggingMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn before_request(&self, ctx: &mut RequestContext) -> Result<()> {
        if self.log_requests {
            println!("[Middleware] Request: {} messages", ctx.messages.len());
            if let Some(opts) = &ctx.options {
                println!("[Middleware] Options: temp={:?}, max_tokens={:?}",
                    opts.temperature, opts.max_tokens);
            }
        }
        Ok(())
    }

    async fn after_response(&self, ctx: &mut ResponseContext) -> Result<()> {
        if self.log_responses {
            println!("[Middleware] Response: {} chars", ctx.response.content.len());
            if let Some(usage) = &ctx.response.usage {
                println!("[Middleware] Usage: {} prompt + {} completion = {} total tokens",
                    usage.prompt_tokens, usage.completion_tokens, usage.total_tokens);
            }
        }
        Ok(())
    }

    async fn on_error(&self, error: &ProviderError) -> Result<()> {
        if self.log_errors {
            eprintln!("[Middleware] Error: {}", error);
        }
        Ok(())
    }
}

/// Built-in middleware for tracking total token usage
pub struct TokenCounterMiddleware {
    total_prompt_tokens: Arc<std::sync::atomic::AtomicU32>,
    total_completion_tokens: Arc<std::sync::atomic::AtomicU32>,
}

impl TokenCounterMiddleware {
    /// Create a new token counter middleware
    pub fn new() -> Self {
        Self {
            total_prompt_tokens: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            total_completion_tokens: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }

    /// Get the total prompt tokens used
    pub fn total_prompt_tokens(&self) -> u32 {
        self.total_prompt_tokens.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get the total completion tokens used
    pub fn total_completion_tokens(&self) -> u32 {
        self.total_completion_tokens.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get the total tokens used (prompt + completion)
    pub fn total_tokens(&self) -> u32 {
        self.total_prompt_tokens() + self.total_completion_tokens()
    }

    /// Reset all counters to zero
    pub fn reset(&self) {
        self.total_prompt_tokens.store(0, std::sync::atomic::Ordering::Relaxed);
        self.total_completion_tokens.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Default for TokenCounterMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Middleware for TokenCounterMiddleware {
    async fn after_response(&self, ctx: &mut ResponseContext) -> Result<()> {
        if let Some(usage) = &ctx.response.usage {
            self.total_prompt_tokens.fetch_add(
                usage.prompt_tokens,
                std::sync::atomic::Ordering::Relaxed,
            );
            self.total_completion_tokens.fetch_add(
                usage.completion_tokens,
                std::sync::atomic::Ordering::Relaxed,
            );
        }
        Ok(())
    }
}

/// Built-in middleware for collecting performance metrics
pub struct MetricsMiddleware {
    request_count: Arc<std::sync::atomic::AtomicU64>,
    error_count: Arc<std::sync::atomic::AtomicU64>,
    total_response_time_ms: Arc<std::sync::atomic::AtomicU64>,
}

impl MetricsMiddleware {
    /// Create a new metrics middleware
    pub fn new() -> Self {
        Self {
            request_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            error_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            total_response_time_ms: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Get the total number of requests
    pub fn request_count(&self) -> u64 {
        self.request_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get the total number of errors
    pub fn error_count(&self) -> u64 {
        self.error_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get the average response time in milliseconds
    pub fn average_response_time_ms(&self) -> f64 {
        let total = self.total_response_time_ms.load(std::sync::atomic::Ordering::Relaxed);
        let count = self.request_count();
        if count == 0 {
            0.0
        } else {
            total as f64 / count as f64
        }
    }

    /// Reset all metrics to zero
    pub fn reset(&self) {
        self.request_count.store(0, std::sync::atomic::Ordering::Relaxed);
        self.error_count.store(0, std::sync::atomic::Ordering::Relaxed);
        self.total_response_time_ms.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Default for MetricsMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Middleware for MetricsMiddleware {
    async fn before_request(&self, ctx: &mut RequestContext) -> Result<()> {
        self.request_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        ctx.metadata.insert("start_time".to_string(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
                .to_string()
        );
        Ok(())
    }

    async fn after_response(&self, ctx: &mut ResponseContext) -> Result<()> {
        if let Some(start_time_str) = ctx.metadata.get("start_time") {
            if let Ok(start_time) = start_time_str.parse::<u128>() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                let duration = (now - start_time) as u64;
                self.total_response_time_ms.fetch_add(duration, std::sync::atomic::Ordering::Relaxed);
            }
        }
        Ok(())
    }

    async fn on_error(&self, _error: &ProviderError) -> Result<()> {
        self.error_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_middleware_chain() {
        let chain = MiddlewareChain::new()
            .add(Arc::new(LoggingMiddleware::new()));

        let mut ctx = RequestContext {
            messages: vec![],
            options: None,
            metadata: HashMap::new(),
        };

        assert!(chain.execute_before(&mut ctx).await.is_ok());
    }

    #[tokio::test]
    async fn test_token_counter() {
        let counter = TokenCounterMiddleware::new();

        let mut ctx = ResponseContext {
            response: GenerateResponse {
                content: "test".to_string(),
                usage: Some(super::super::Usage {
                    prompt_tokens: 10,
                    completion_tokens: 20,
                    total_tokens: 30,
                }),
                model: "test".to_string(),
                finish_reason: None,
            },
            metadata: HashMap::new(),
        };

        counter.after_response(&mut ctx).await.unwrap();

        assert_eq!(counter.total_prompt_tokens(), 10);
        assert_eq!(counter.total_completion_tokens(), 20);
        assert_eq!(counter.total_tokens(), 30);
    }

    #[tokio::test]
    async fn test_metrics() {
        let metrics = MetricsMiddleware::new();

        let mut req_ctx = RequestContext {
            messages: vec![],
            options: None,
            metadata: HashMap::new(),
        };

        metrics.before_request(&mut req_ctx).await.unwrap();
        assert_eq!(metrics.request_count(), 1);

        let error = ProviderError::RequestFailed("test".to_string());
        metrics.on_error(&error).await.unwrap();
        assert_eq!(metrics.error_count(), 1);
    }
}
