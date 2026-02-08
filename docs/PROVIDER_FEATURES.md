# Provider Features Guide

This guide covers all the advanced features available in the agent-sdk-rs provider system.

## Table of Contents

1. [Reliability Features](#reliability-features)
2. [Cost Optimization](#cost-optimization)
3. [Developer Experience](#developer-experience)
4. [Advanced Features](#advanced-features)
5. [Configuration Examples](#configuration-examples)

## Reliability Features

### Retry Logic with Exponential Backoff

Automatically retry failed requests with configurable exponential backoff:

```rust
use agent_sdk::provider::{AnthropicProvider, RetryConfig};
use std::time::Duration;

let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model("claude-3-5-sonnet-20241022")
    .retry_config(RetryConfig {
        max_retries: 3,
        initial_backoff: Duration::from_millis(500),
        max_backoff: Duration::from_secs(60),
        backoff_multiplier: 2.0,
        retry_on_timeout: true,
        retry_on_rate_limit: true,
    })
    .build()?;
```

**What gets retried:**
- 502/503/504 server errors
- Timeout errors (if `retry_on_timeout` is true)
- Rate limit errors (if `retry_on_rate_limit` is true)

**What doesn't get retried:**
- Authentication failures
- Parse errors
- Model not available errors

### Rate Limiting

Client-side rate limiting to prevent hitting API limits:

```rust
use agent_sdk::provider::RateLimitConfig;

let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model("claude-3-5-sonnet-20241022")
    .rate_limit_config(RateLimitConfig {
        requests_per_minute: 50,
        tokens_per_minute: Some(100_000),
        concurrent_requests: 5,
    })
    .build()?;
```

**Features:**
- Sliding window rate limiting
- Concurrent request control with semaphores
- Optional token-based rate limiting
- Automatic waiting when limits are reached

### Timeout Configuration

Configure timeouts for different stages of the request:

```rust
use agent_sdk::provider::TimeoutConfig;
use std::time::Duration;

let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model("claude-3-5-sonnet-20241022")
    .timeout_config(TimeoutConfig {
        connect_timeout: Duration::from_secs(10),
        request_timeout: Duration::from_secs(120),
        stream_timeout: Some(Duration::from_secs(300)),
    })
    .build()?;
```

**Preset configurations:**
- `TimeoutConfig::fast()` - For quick operations
- `TimeoutConfig::slow()` - For long-running operations
- `TimeoutConfig::default()` - Balanced settings

## Cost Optimization

### Response Caching

Cache responses to reduce API costs and improve performance:

```rust
use agent_sdk::provider::CacheConfig;
use std::time::Duration;

let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model("claude-3-5-sonnet-20241022")
    .cache_config(CacheConfig {
        enabled: true,
        ttl: Duration::from_secs(3600), // 1 hour
        max_entries: 1000,
    })
    .build()?;
```

**Features:**
- Hash-based cache keys (messages + model + options)
- TTL-based expiration
- LRU eviction when at capacity
- Thread-safe with RwLock
- Cache statistics (hit rate, entry count)

**Preset configurations:**
- `CacheConfig::disabled()` - No caching
- `CacheConfig::short_lived()` - 5 minute TTL
- `CacheConfig::long_lived()` - 24 hour TTL

### Anthropic Prompt Caching

Enable Anthropic's prompt caching feature to reduce costs:

```rust
use agent_sdk::provider::anthropic::PromptCacheConfig;

let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model("claude-3-5-sonnet-20241022")
    .prompt_cache_config(PromptCacheConfig {
        enabled: true,
        cache_system_messages: true,
        cache_tool_definitions: true,
    })
    .build()?;
```

**Note:** This feature is specific to Anthropic and requires compatible models.

## Developer Experience

### Middleware System

Extensible middleware for logging, metrics, and custom processing:

```rust
use agent_sdk::provider::{
    MiddlewareChain, LoggingMiddleware, TokenCounterMiddleware, MetricsMiddleware
};
use std::sync::Arc;

let token_counter = Arc::new(TokenCounterMiddleware::new());
let middleware = MiddlewareChain::new()
    .add(Arc::new(LoggingMiddleware::new()))
    .add(token_counter.clone())
    .add(Arc::new(MetricsMiddleware::new()));

let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model("claude-3-5-sonnet-20241022")
    .middleware(middleware)
    .build()?;

// After making requests, check token usage
println!("Total tokens: {}", token_counter.total_tokens());
```

**Built-in middleware:**
- `LoggingMiddleware` - Logs requests and responses
- `TokenCounterMiddleware` - Tracks total token usage
- `MetricsMiddleware` - Collects performance metrics

**Custom middleware:**
Implement the `Middleware` trait to create custom middleware:

```rust
use agent_sdk::provider::{Middleware, RequestContext, ResponseContext, ProviderError, Result};
use async_trait::async_trait;

struct CustomMiddleware;

#[async_trait]
impl Middleware for CustomMiddleware {
    async fn before_request(&self, ctx: &mut RequestContext) -> Result<()> {
        // Process before request
        Ok(())
    }

    async fn after_response(&self, ctx: &mut ResponseContext) -> Result<()> {
        // Process after response
        Ok(())
    }

    async fn on_error(&self, error: &ProviderError) -> Result<()> {
        // Handle errors
        Ok(())
    }
}
```

### Context Window Management

Automatically manage context window limits:

```rust
use agent_sdk::provider::{ContextWindowConfig, TruncationStrategy};

let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model("claude-3-5-sonnet-20241022")
    .context_config(ContextWindowConfig {
        max_tokens: 100_000,
        truncation_strategy: TruncationStrategy::DropOldest,
    })
    .build()?;
```

**Truncation strategies:**
- `DropOldest` - Remove oldest messages first (keeps recent context)
- `DropMiddle` - Keep first and last messages, drop middle (preserves instructions and recent context)
- `Summarize` - Summarize old messages (future feature)

**Preset configurations:**
- `ContextWindowConfig::small()` - 4k tokens
- `ContextWindowConfig::medium()` - 32k tokens
- `ContextWindowConfig::large()` - 200k tokens

## Advanced Features

### Multimodal/Vision Support

Send images along with text:

```rust
use agent_sdk::provider::Message;

// Image from URL
let messages = vec![
    Message::user_with_image_url(
        "What do you see in this image?",
        "https://example.com/image.jpg"
    )
];

// Image from base64
let messages = vec![
    Message::user_with_image_base64(
        "Describe this image",
        "image/jpeg",
        base64_encoded_data
    )
];

let response = provider.generate(messages, None).await?;
```

**Supported formats:**
- Image URLs
- Base64-encoded images (JPEG, PNG, GIF, WebP)
- Multiple images per message
- Mixed text and image content

### Batch Requests

Process multiple requests concurrently:

```rust
use agent_sdk::provider::{BatchRequest, SingleRequest, execute_batch_concurrent};

let batch = BatchRequest::new(vec![
    SingleRequest::new("req1", vec![Message::user("What is 1+1?")]),
    SingleRequest::new("req2", vec![Message::user("What is 2+2?")]),
    SingleRequest::new("req3", vec![Message::user("What is 3+3?")]),
])
.with_max_concurrent(2); // Process 2 at a time

let results = execute_batch_concurrent(&provider, batch).await?;

for response in results.responses {
    match response.result {
        Ok(gen_response) => println!("{}: {}", response.id, gen_response.content),
        Err(e) => println!("{}: Error - {}", response.id, e),
    }
}

println!("Success rate: {}/{}", results.success_count(), results.responses.len());
```

**Features:**
- Concurrent execution with configurable concurrency
- Sequential execution option
- Individual error handling per request
- Batch statistics (success/error counts)

### Embeddings API

Create embeddings for text (OpenRouter only):

```rust
use agent_sdk::provider::{EmbeddingProvider, EmbeddingRequest};

let request = EmbeddingRequest::new("Hello, world!")
    .with_model("text-embedding-ada-002");

let response = provider.create_embeddings(request).await?;

for embedding in response.embeddings {
    println!("Embedding dimension: {}", embedding.len());
}
```

**Note:** Anthropic doesn't have a native embeddings API. Use OpenRouter or integrate with Voyage AI.

## Configuration Examples

### Production Configuration

Recommended settings for production use:

```rust
use agent_sdk::provider::*;
use std::sync::Arc;
use std::time::Duration;

let token_counter = Arc::new(TokenCounterMiddleware::new());
let metrics = Arc::new(MetricsMiddleware::new());

let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model("claude-3-5-sonnet-20241022")
    // Reliability
    .retry_config(RetryConfig {
        max_retries: 3,
        initial_backoff: Duration::from_millis(500),
        max_backoff: Duration::from_secs(60),
        backoff_multiplier: 2.0,
        retry_on_timeout: true,
        retry_on_rate_limit: true,
    })
    .rate_limit_config(RateLimitConfig {
        requests_per_minute: 50,
        tokens_per_minute: None,
        concurrent_requests: 10,
    })
    .timeout_config(TimeoutConfig {
        connect_timeout: Duration::from_secs(10),
        request_timeout: Duration::from_secs(120),
        stream_timeout: Some(Duration::from_secs(300)),
    })
    // Cost optimization
    .cache_config(CacheConfig {
        enabled: true,
        ttl: Duration::from_secs(3600),
        max_entries: 1000,
    })
    .prompt_cache_config(PromptCacheConfig::default())
    // Developer experience
    .middleware(MiddlewareChain::new()
        .add(Arc::new(LoggingMiddleware::new()))
        .add(token_counter.clone())
        .add(metrics.clone()))
    .context_config(ContextWindowConfig::large())
    .build()?;
```

### Development Configuration

Minimal configuration for development:

```rust
let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model("claude-3-5-sonnet-20241022")
    .middleware(MiddlewareChain::new()
        .add(Arc::new(LoggingMiddleware::new())))
    .build()?;
```

### High-Throughput Configuration

Optimized for high request volumes:

```rust
let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model("claude-3-5-sonnet-20241022")
    .retry_config(RetryConfig::aggressive())
    .rate_limit_config(RateLimitConfig {
        requests_per_minute: 100,
        tokens_per_minute: None,
        concurrent_requests: 20,
    })
    .cache_config(CacheConfig::long_lived())
    .build()?;
```

### Cost-Optimized Configuration

Minimize API costs:

```rust
let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model("claude-3-5-sonnet-20241022")
    .cache_config(CacheConfig::long_lived())
    .prompt_cache_config(PromptCacheConfig::default())
    .context_config(ContextWindowConfig::medium())
    .build()?;
```

## Best Practices

1. **Always use retry logic in production** - Network issues are common
2. **Enable caching for repeated queries** - Significant cost savings
3. **Use middleware for observability** - Track token usage and performance
4. **Configure rate limits** - Prevent hitting API limits
5. **Use context window management** - Avoid token limit errors
6. **Enable prompt caching for Anthropic** - Reduce costs for repeated prompts
7. **Use batch requests for multiple queries** - Better throughput
8. **Configure appropriate timeouts** - Balance between reliability and responsiveness

## Troubleshooting

### High latency

- Check if rate limiting is causing delays
- Reduce `requests_per_minute` if hitting API limits
- Increase `concurrent_requests` for better throughput
- Enable caching to reduce API calls

### Token limit errors

- Enable context window management
- Use `TruncationStrategy::DropOldest` or `DropMiddle`
- Monitor token usage with `TokenCounterMiddleware`

### Cache not working

- Verify `CacheConfig.enabled` is true
- Check if TTL is too short
- Ensure requests are identical (same messages, model, options)

### Rate limit errors

- Increase `initial_backoff` in retry config
- Reduce `requests_per_minute`
- Enable `retry_on_rate_limit`

## Performance Considerations

- **Caching**: Adds ~1ms overhead for cache lookups
- **Middleware**: Each middleware adds ~0.1-1ms overhead
- **Rate limiting**: May add delays when limits are reached
- **Context window management**: Adds ~1-5ms for token estimation
- **Retry logic**: Adds latency on failures (exponential backoff)

## Migration Guide

### From basic provider to enhanced provider

Before:
```rust
let provider = AnthropicProvider::new(api_key, model);
```

After:
```rust
let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model(model)
    .retry_config(RetryConfig::default())
    .cache_config(CacheConfig::default())
    .build()?;
```

### Message structure changes

Before:
```rust
let message = Message {
    role: Role::User,
    content: "Hello".to_string(),
};
```

After:
```rust
let message = Message::user("Hello");
// Or for multimodal:
let message = Message::user_with_image_url("Describe this", "https://...");
```

The old structure is no longer supported. Use the convenience methods instead.
