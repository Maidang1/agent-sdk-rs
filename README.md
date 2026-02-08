# Agent SDK

A production-ready Rust SDK for building AI agents with comprehensive provider features, tool calling capabilities, and advanced reliability features.

## Features

### 🚀 **Reliability**
- **Retry Logic** - Exponential backoff with configurable retry policies
- **Rate Limiting** - Client-side rate limiting with sliding window
- **Timeout Configuration** - Configurable timeouts for all request stages
- **Error Handling** - Comprehensive error types with automatic retry on transient failures

### 💰 **Cost Optimization**
- **Response Caching** - Hash-based cache with TTL and LRU eviction
- **Anthropic Prompt Caching** - Support for Anthropic's prompt caching feature
- **Token Tracking** - Built-in middleware for tracking token usage

### 🛠️ **Developer Experience**
- **Middleware System** - Extensible middleware for logging, metrics, and custom processing
- **Context Window Management** - Automatic message truncation with multiple strategies
- **Builder Pattern** - Fluent API for easy configuration
- **Type Safety** - Full Rust type safety with comprehensive error handling

### 🎨 **Advanced Features**
- **Multimodal Support** - Send images along with text (URL or base64)
- **Batch Requests** - Process multiple requests concurrently
- **Embeddings API** - Create embeddings for text (OpenRouter)
- **Streaming** - Support for streaming responses
- **Tool Calling** - Built-in tool calling with validation

## Quick Start

```rust
use agent_sdk::provider::{AnthropicProvider, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = AnthropicProvider::builder()
        .api_key("your-api-key")
        .model("claude-3-5-sonnet-20241022")
        .build()?;

    let messages = vec![Message::user("What is the capital of France?")];
    let response = provider.generate(messages, None).await?;

    println!("Response: {}", response.content);
    Ok(())
}
```

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
agent-sdk = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
```

## Providers

### Anthropic (Claude)

```rust
use agent_sdk::provider::{AnthropicProvider, RetryConfig, CacheConfig};
use std::time::Duration;

let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model("claude-3-5-sonnet-20241022")
    .retry_config(RetryConfig::default())
    .cache_config(CacheConfig::default())
    .build()?;
```

**Features:**
- Claude 3.5 Sonnet, Opus, Haiku models
- Prompt caching support
- Streaming responses
- Tool calling

### OpenRouter

```rust
use agent_sdk::provider::OpenRouterProvider;

let provider = OpenRouterProvider::builder()
    .api_key(api_key)
    .model("anthropic/claude-3.5-sonnet")
    .build()?;
```

**Features:**
- Access to multiple model providers
- OpenAI-compatible format
- Embeddings API support

## Advanced Usage

### Production Configuration

```rust
use agent_sdk::provider::*;
use std::sync::Arc;
use std::time::Duration;

let token_counter = Arc::new(TokenCounterMiddleware::new());

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
        concurrent_requests: 10,
        tokens_per_minute: None,
    })
    // Cost optimization
    .cache_config(CacheConfig {
        enabled: true,
        ttl: Duration::from_secs(3600),
        max_entries: 1000,
    })
    // Observability
    .middleware(MiddlewareChain::new()
        .add(Arc::new(LoggingMiddleware::new()))
        .add(token_counter.clone()))
    .build()?;
```

### Multimodal Input

```rust
let messages = vec![
    Message::user_with_image_url(
        "What do you see in this image?",
        "https://example.com/image.jpg"
    )
];

let response = provider.generate(messages, None).await?;
```

### Batch Requests

```rust
use agent_sdk::provider::{BatchRequest, SingleRequest, execute_batch_concurrent};

let batch = BatchRequest::new(vec![
    SingleRequest::new("req1", vec![Message::user("What is 1+1?")]),
    SingleRequest::new("req2", vec![Message::user("What is 2+2?")]),
    SingleRequest::new("req3", vec![Message::user("What is 3+3?")]),
])
.with_max_concurrent(2);

let results = execute_batch_concurrent(&provider, batch).await?;
println!("Success rate: {}/{}", results.success_count(), results.responses.len());
```

### Context Window Management

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

## Tool Calling

```rust
use agent_sdk::{Agent, Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str { "calculator" }

    fn description(&self) -> &str {
        "Perform arithmetic operations"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number"},
                "b": {"type": "number"},
                "operation": {"type": "string", "enum": ["add", "sub", "mul", "div"]}
            },
            "required": ["a", "b", "operation"]
        })
    }

    async fn execute(&self, params: &Value) -> ToolResult {
        let a = params["a"].as_f64().unwrap_or(0.0);
        let b = params["b"].as_f64().unwrap_or(0.0);
        let op = params["operation"].as_str().unwrap_or("add");

        let result = match op {
            "add" => a + b,
            "sub" => a - b,
            "mul" => a * b,
            "div" if b != 0.0 => a / b,
            _ => return ToolResult::error("Invalid operation"),
        };

        ToolResult::success(result.to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = AnthropicProvider::builder()
        .api_key(api_key)
        .model("claude-3-5-sonnet-20241022")
        .build()?;

    let mut agent = Agent::new(provider);
    agent.register_tool(Box::new(CalculatorTool)).await;

    let response = agent.run("Calculate 15 * 23").await?;
    println!("Result: {}", response);

    Ok(())
}
```

## Documentation

- [Provider Features Guide](docs/PROVIDER_FEATURES.md) - Comprehensive guide to all provider features
- [Migration Guide](docs/MIGRATION_GUIDE.md) - Guide for migrating from older versions
- [Examples](examples/) - Example code for common use cases

## Examples

Run examples with:

```bash
# Basic provider usage
cargo run --example provider_features

# Tool calling
cargo run --example calculator

# Event monitoring
cargo run --example event_monitoring

# Hook system
cargo run --example hook_system
```

## Architecture

```
agent-sdk-rs/
├── src/
│   ├── agent/          # Agent implementation with tool calling
│   ├── provider/       # LLM provider implementations
│   │   ├── anthropic.rs
│   │   ├── open_router.rs
│   │   ├── client.rs   # Shared HTTP client with retry/rate limiting
│   │   ├── retry.rs    # Retry logic with exponential backoff
│   │   ├── rate_limit.rs # Rate limiting
│   │   ├── cache.rs    # Response caching
│   │   ├── middleware.rs # Middleware system
│   │   ├── context.rs  # Context window management
│   │   ├── batch.rs    # Batch request processing
│   │   └── embeddings.rs # Embeddings API
│   ├── tool/           # Tool system
│   ├── events/         # Event system
│   └── hooks/          # Hook system
└── examples/           # Example code
```

## Performance

- **Caching**: ~1ms overhead for cache lookups, significant savings on cache hits
- **Middleware**: ~0.1-1ms overhead per middleware
- **Rate Limiting**: Automatic waiting when limits are reached
- **Retry Logic**: Exponential backoff on failures (500ms to 60s)

## Testing

Run tests with:

```bash
# Run all tests
cargo test

# Run library tests only
cargo test --lib

# Run with output
cargo test -- --nocapture
```

**Test Coverage:**
- 39 unit tests covering all provider features
- Tests for retry logic, rate limiting, caching, middleware, context management, and batch processing
- All tests passing ✅

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT OR Apache-2.0

## Changelog

### v0.1.0 (Current)

**New Features:**
- ✅ Retry logic with exponential backoff
- ✅ Client-side rate limiting
- ✅ Response caching with TTL and LRU eviction
- ✅ Middleware system (logging, token counting, metrics)
- ✅ Context window management
- ✅ Multimodal/vision support
- ✅ Batch request processing
- ✅ Embeddings API support
- ✅ Anthropic prompt caching configuration
- ✅ Builder pattern for easy configuration

**Breaking Changes:**
- Message structure changed from `String` to `Vec<ContentBlock>` to support multimodal input
- Provider constructors now return `Result` instead of direct instances
- Use `Message::user()`, `Message::system()`, `Message::assistant()` convenience methods

See [Migration Guide](docs/MIGRATION_GUIDE.md) for details.

## Roadmap

- [ ] Additional providers (OpenAI, Cohere, etc.)
- [ ] Streaming support for batch requests
- [ ] Advanced prompt caching strategies
- [ ] Token usage optimization
- [ ] Distributed rate limiting
- [ ] Request prioritization
- [ ] Circuit breaker pattern
- [ ] Health check endpoints

## Support

For issues, questions, or contributions, please visit the [GitHub repository](https://github.com/yourusername/agent-sdk-rs).
