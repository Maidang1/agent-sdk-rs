# Provider Enhancement Plan: Comprehensive SDK Capabilities

## Context

The agent-sdk-rs currently has a solid foundation with two LLM providers (Anthropic and OpenRouter), but lacks several features that are standard in modern AI SDKs. This plan adds four categories of enhancements to bring the SDK to production-ready status:

1. **Reliability** - Retry logic, timeouts, rate limiting
2. **Cost Optimization** - Response caching, Anthropic prompt caching
3. **Advanced Features** - Multimodal/vision support, embeddings, batch requests
4. **Developer Experience** - Middleware system, context window management, better configuration

The user has requested that all four categories be implemented with equal treatment for both providers where applicable.

## Current State Analysis

### Existing Providers
- **AnthropicProvider** (`src/provider/anthropic.rs`, 420 lines)
  - Supports API key and auth token authentication
  - Custom HTTP client with proxy fallback
  - Separates system messages from chat messages
  - Maps `stop` to `stop_sequences` for API compatibility

- **OpenRouterProvider** (`src/provider/open_router.rs`, 214 lines)
  - OpenAI-compatible format
  - Standard bearer token authentication

### Current Features ✅
- Basic messaging (System, User, Assistant roles)
- Streaming responses with SSE parsing
- Token usage tracking
- Error handling (RequestFailed, AuthenticationFailed, RateLimited, ModelNotAvailable, ParseError)
- GenerateOptions (temperature, max_tokens, top_p, stop)
- Event system with EventBus
- Hook system (logging, metrics, error tracking)
- Tool calling with validation

### Missing Features ❌
- No retry logic with exponential backoff
- No timeout configuration
- No client-side rate limiting
- No prompt caching (Anthropic feature)
- No response caching
- No vision/multimodal support
- No embeddings API
- No batch request support
- No middleware/interceptor pattern
- No context window management

## Implementation Plan

### Phase 1: Foundation & Reliability (Priority: HIGH)

**Goal:** Build core infrastructure for retry, timeout, and rate limiting that both providers will use.

#### 1.1 Create Shared HTTP Client Infrastructure

**New file:** `src/provider/client.rs`

Create a `ProviderClient` that wraps reqwest with:
- Configurable timeouts (connect, request, stream)
- Retry policy with exponential backoff
- Rate limiting (requests per minute, concurrent requests)
- Builder pattern for easy configuration

```rust
pub struct ProviderClient {
    http_client: Client,
    retry_policy: Arc<RetryPolicy>,
    rate_limiter: Arc<RateLimiter>,
}

pub struct ProviderClientBuilder {
    retry_config: RetryConfig,
    timeout_config: TimeoutConfig,
    rate_limit_config: RateLimitConfig,
    proxy: Option<String>,
}
```

#### 1.2 Implement Retry Logic

**New file:** `src/provider/retry.rs`

```rust
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub backoff_multiplier: f64,
    pub retry_on_timeout: bool,
    pub retry_on_rate_limit: bool,
}

pub struct RetryPolicy {
    // Determines if error should be retried
    // Calculates exponential backoff
    // Executes operations with retry logic
}
```

Retry on:
- 502/503 errors (server issues)
- Timeout errors (if configured)
- Rate limit errors (with respect to retry-after header)

#### 1.3 Implement Rate Limiting

**New file:** `src/provider/rate_limit.rs`

```rust
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub tokens_per_minute: Option<u32>,
    pub concurrent_requests: usize,
}

pub struct RateLimiter {
    // Uses tokio Semaphore for concurrency control
    // Tracks request times in sliding window
    // Automatically waits when rate limit reached
}
```

#### 1.4 Add Timeout Configuration

**New file:** `src/provider/timeout.rs`

```rust
pub struct TimeoutConfig {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub stream_timeout: Option<Duration>,
}
```

#### 1.5 Update Both Providers

**Modify:** `src/provider/anthropic.rs` and `src/provider/open_router.rs`

Replace custom HTTP client building with `ProviderClient`:

```rust
pub struct AnthropicProvider {
    api_key: String,
    auth_token: Option<String>,
    model: String,
    client: ProviderClient, // Changed from reqwest::Client
    base_url: String,
}

impl AnthropicProvider {
    pub fn builder() -> AnthropicProviderBuilder {
        AnthropicProviderBuilder::default()
    }
}

pub struct AnthropicProviderBuilder {
    api_key: String,
    model: String,
    base_url: Option<String>,
    client_builder: ProviderClientBuilder,
}
```

Update `send_request` to use retry policy:

```rust
async fn send_request(&self, body: serde_json::Value) -> Result<reqwest::Response> {
    let _guard = self.client.acquire_rate_limit().await;

    self.client.retry_policy().execute_with_retry(|| async {
        // Existing request logic
    }).await
}
```

#### 1.6 Update Provider Module

**Modify:** `src/provider/mod.rs`

Add new exports:

```rust
mod client;
mod retry;
mod rate_limit;
mod timeout;

pub use client::{ProviderClient, ProviderClientBuilder};
pub use retry::{RetryConfig, RetryPolicy};
pub use rate_limit::{RateLimitConfig, RateLimiter};
pub use timeout::TimeoutConfig;
```

### Phase 2: Developer Experience (Priority: HIGH)

**Goal:** Make the SDK easier to use and extend with middleware and better configuration.

#### 2.1 Implement Middleware System

**New file:** `src/provider/middleware.rs`

```rust
#[async_trait]
pub trait Middleware: Send + Sync {
    async fn before_request(&self, ctx: &mut RequestContext) -> Result<()>;
    async fn after_response(&self, ctx: &mut ResponseContext) -> Result<()>;
    async fn on_error(&self, error: &ProviderError) -> Result<()>;
}

pub struct MiddlewareChain {
    middlewares: Vec<Arc<dyn Middleware>>,
}
```

Built-in middleware:
- `LoggingMiddleware` - Logs requests/responses
- `TokenCounterMiddleware` - Tracks total token usage
- `MetricsMiddleware` - Collects performance metrics

#### 2.2 Add Context Window Management

**New file:** `src/provider/context.rs`

```rust
pub struct ContextWindowConfig {
    pub max_tokens: usize,
    pub truncation_strategy: TruncationStrategy,
}

pub enum TruncationStrategy {
    DropOldest,      // Remove oldest messages
    DropMiddle,      // Keep first and last, drop middle
    Summarize,       // Summarize old messages (future)
}

pub struct ContextWindowManager {
    // Estimates token count
    // Truncates messages if needed
    // Preserves system messages
}
```

#### 2.3 Improve Configuration with Builder Pattern

**Modify:** Both provider files to use comprehensive builders:

```rust
let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model("claude-3-5-sonnet-20241022")
    .retry_config(RetryConfig {
        max_retries: 3,
        initial_backoff: Duration::from_millis(500),
        ..Default::default()
    })
    .rate_limit_config(RateLimitConfig {
        requests_per_minute: 50,
        concurrent_requests: 5,
        ..Default::default()
    })
    .timeout_config(TimeoutConfig {
        request_timeout: Duration::from_secs(60),
        ..Default::default()
    })
    .middleware(Arc::new(LoggingMiddleware))
    .build()?;
```

#### 2.4 Integrate Middleware into Providers

**Modify:** `src/provider/anthropic.rs` and `src/provider/open_router.rs`

Add middleware field and execute middleware chain:

```rust
pub struct AnthropicProvider {
    // ... existing fields
    middleware: Option<MiddlewareChain>,
}

async fn generate(&self, messages: Vec<Message>, options: Option<GenerateOptions>) -> Result<GenerateResponse> {
    let mut ctx = RequestContext { messages, options, metadata: HashMap::new() };

    if let Some(mw) = &self.middleware {
        mw.execute_before(&mut ctx).await?;
    }

    let result = self.generate_internal(ctx.messages, ctx.options).await;

    match result {
        Ok(response) => {
            let mut resp_ctx = ResponseContext { response, metadata: HashMap::new() };
            if let Some(mw) = &self.middleware {
                mw.execute_after(&mut resp_ctx).await?;
            }
            Ok(resp_ctx.response)
        }
        Err(e) => {
            if let Some(mw) = &self.middleware {
                mw.execute_error(&e).await?;
            }
            Err(e)
        }
    }
}
```

### Phase 3: Cost Optimization (Priority: MEDIUM)

**Goal:** Reduce API costs through caching strategies.

#### 3.1 Implement Response Caching

**New file:** `src/provider/cache.rs`

```rust
pub struct CacheConfig {
    pub enabled: bool,
    pub ttl: Duration,
    pub max_entries: usize,
}

pub struct CacheKey {
    messages_hash: u64,
    model: String,
    options_hash: u64,
}

pub struct ResponseCache {
    config: CacheConfig,
    entries: Arc<RwLock<HashMap<CacheKey, CacheEntry>>>,
}
```

Features:
- Hash-based cache keys (messages + model + options)
- TTL-based expiration
- LRU eviction when at capacity
- Thread-safe with RwLock
- Cache statistics (hit rate, entry count)

#### 3.2 Add Anthropic Prompt Caching

**Modify:** `src/provider/anthropic.rs`

Add prompt cache configuration:

```rust
pub struct PromptCacheConfig {
    pub enabled: bool,
    pub cache_system_messages: bool,
    pub cache_tool_definitions: bool,
}

impl AnthropicProvider {
    fn build_request_body_with_cache(
        // ... params
        cache_config: &PromptCacheConfig,
    ) -> serde_json::Value {
        // Add cache_control blocks to system messages
        if cache_config.enabled && cache_config.cache_system_messages {
            body["system"] = json!([{
                "type": "text",
                "text": system_prompt,
                "cache_control": {"type": "ephemeral"}
            }]);
        }
    }
}
```

Track cache usage in response:

```rust
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub cache_creation_input_tokens: Option<u32>,
    pub cache_read_input_tokens: Option<u32>,
}
```

#### 3.3 Integrate Caching into Providers

**Modify:** Both providers to check cache before making requests:

```rust
async fn generate(&self, messages: Vec<Message>, options: Option<GenerateOptions>) -> Result<GenerateResponse> {
    // Check cache first
    if let Some(cache) = &self.cache {
        let key = CacheKey::from_request(&messages, &self.model, &options);
        if let Some(cached) = cache.get(&key).await {
            return Ok(cached);
        }
    }

    // Make request
    let response = self.generate_internal(messages.clone(), options.clone()).await?;

    // Store in cache
    if let Some(cache) = &self.cache {
        let key = CacheKey::from_request(&messages, &self.model, &options);
        cache.put(key, response.clone()).await;
    }

    Ok(response)
}
```

### Phase 4: Advanced Features (Priority: MEDIUM)

**Goal:** Expand provider capabilities with multimodal, embeddings, and batch support.

#### 4.1 Add Multimodal/Vision Support

**Modify:** `src/provider/mod.rs`

Change Message structure to support multiple content blocks:

```rust
#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text { text: String },
    Image {
        source: ImageSource,
        detail: Option<ImageDetail>,
    },
}

#[derive(Debug, Clone)]
pub enum ImageSource {
    Url { url: String },
    Base64 {
        media_type: String, // "image/jpeg", "image/png", etc.
        data: String,       // base64 encoded
    },
}

#[derive(Debug, Clone)]
pub enum ImageDetail {
    Low,    // Faster, cheaper
    High,   // More detailed
    Auto,   // Let API decide
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>, // Changed from String
}
```

Add convenience methods:

```rust
impl Message {
    pub fn user_text(content: impl Into<String>) -> Self { /* ... */ }

    pub fn user_with_image(text: impl Into<String>, image: ImageSource) -> Self {
        Self {
            role: Role::User,
            content: vec![
                ContentBlock::Text { text: text.into() },
                ContentBlock::Image { source: image, detail: None },
            ],
        }
    }

    pub fn user_with_image_url(text: impl Into<String>, url: impl Into<String>) -> Self {
        Self::user_with_image(text, ImageSource::Url { url: url.into() })
    }

    // Backward compatibility
    pub fn user(content: impl Into<String>) -> Self {
        Self::user_text(content)
    }

    pub fn content_as_text(&self) -> String {
        self.content.iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
```

**Modify:** `src/provider/anthropic.rs`

Update message formatting to handle content blocks:

```rust
fn format_message_content(content: &[ContentBlock]) -> serde_json::Value {
    if content.len() == 1 {
        if let ContentBlock::Text { text } = &content[0] {
            return json!(text);
        }
    }

    json!(content.iter().map(|block| match block {
        ContentBlock::Text { text } => json!({
            "type": "text",
            "text": text,
        }),
        ContentBlock::Image { source, detail } => {
            let mut img = json!({
                "type": "image",
            });
            match source {
                ImageSource::Url { url } => {
                    img["source"] = json!({
                        "type": "url",
                        "url": url,
                    });
                }
                ImageSource::Base64 { media_type, data } => {
                    img["source"] = json!({
                        "type": "base64",
                        "media_type": media_type,
                        "data": data,
                    });
                }
            }
            img
        }
    }).collect::<Vec<_>>())
}
```

**Modify:** `src/provider/open_router.rs`

Similar updates for OpenAI-compatible format.

#### 4.2 Add Embeddings Support

**New file:** `src/provider/embeddings.rs`

```rust
#[derive(Debug, Clone)]
pub struct EmbeddingRequest {
    pub input: Vec<String>,
    pub model: Option<String>,
    pub encoding_format: Option<EncodingFormat>,
}

#[derive(Debug, Clone)]
pub enum EncodingFormat {
    Float,
    Base64,
}

#[derive(Debug, Clone)]
pub struct EmbeddingResponse {
    pub embeddings: Vec<Vec<f32>>,
    pub model: String,
    pub usage: Option<EmbeddingUsage>,
}

#[derive(Debug, Clone)]
pub struct EmbeddingUsage {
    pub prompt_tokens: u32,
    pub total_tokens: u32,
}

pub trait EmbeddingProvider: Send + Sync {
    fn create_embeddings(
        &self,
        request: EmbeddingRequest,
    ) -> Pin<Box<dyn Future<Output = Result<EmbeddingResponse>> + Send + '_>>;
}
```

**Implement for OpenRouterProvider:**

```rust
impl EmbeddingProvider for OpenRouterProvider {
    fn create_embeddings(&self, request: EmbeddingRequest) -> /* ... */ {
        Box::pin(async move {
            let body = json!({
                "input": request.input,
                "model": request.model.unwrap_or_else(|| "text-embedding-ada-002".to_string()),
            });

            let response = self.client.http_client()
                .post(format!("{}/embeddings", self.base_url))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&body)
                .send()
                .await?;

            // Parse response
        })
    }
}
```

**Note:** Anthropic doesn't have native embeddings API. Could integrate with Voyage AI or return not supported error.

#### 4.3 Add Batch Request Support

**New file:** `src/provider/batch.rs`

```rust
#[derive(Debug, Clone)]
pub struct BatchRequest {
    pub requests: Vec<SingleRequest>,
    pub max_concurrent: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct SingleRequest {
    pub id: String,
    pub messages: Vec<Message>,
    pub options: Option<GenerateOptions>,
}

#[derive(Debug, Clone)]
pub struct BatchResponse {
    pub responses: Vec<SingleResponse>,
}

#[derive(Debug, Clone)]
pub struct SingleResponse {
    pub id: String,
    pub result: Result<GenerateResponse>,
}

pub trait BatchProvider: Send + Sync {
    fn generate_batch(
        &self,
        batch: BatchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<BatchResponse>> + Send + '_>>;
}
```

Default implementation using concurrent execution:

```rust
pub async fn execute_batch_concurrent<P: LlmProvider>(
    provider: &P,
    batch: BatchRequest,
) -> Result<BatchResponse> {
    use futures_util::stream::{self, StreamExt};

    let max_concurrent = batch.max_concurrent.unwrap_or(5);

    let responses = stream::iter(batch.requests)
        .map(|req| async move {
            let result = provider.generate(req.messages, req.options).await;
            SingleResponse { id: req.id, result }
        })
        .buffer_unordered(max_concurrent)
        .collect::<Vec<_>>()
        .await;

    Ok(BatchResponse { responses })
}
```

Implement for both providers using the default implementation.

### Phase 5: Integration & Testing

#### 5.1 Update Examples

**New file:** `examples/provider_features.rs`

Demonstrate all new features:
- Retry on failure
- Rate limiting
- Response caching
- Prompt caching (Anthropic)
- Multimodal input
- Embeddings
- Batch requests
- Middleware

#### 5.2 Add Tests

**New file:** `tests/provider_reliability.rs`
- Test retry logic with mock failures
- Test rate limiting
- Test timeout handling

**New file:** `tests/provider_caching.rs`
- Test cache hit/miss
- Test cache expiration
- Test Anthropic prompt caching

**New file:** `tests/provider_multimodal.rs`
- Test image URL input
- Test base64 image input
- Test mixed text/image messages

**New file:** `tests/provider_batch.rs`
- Test concurrent batch execution
- Test batch error handling

#### 5.3 Update Documentation

**Modify:** `README.md`

Add sections for:
- Reliability features (retry, timeout, rate limiting)
- Caching strategies
- Multimodal support
- Embeddings API
- Batch requests
- Middleware system
- Context window management

**New file:** `docs/PROVIDER_FEATURES.md`

Comprehensive guide to all provider features with examples.

## Critical Files to Modify

### New Files (16 files)
1. `src/provider/client.rs` - Shared HTTP client infrastructure
2. `src/provider/retry.rs` - Retry logic with exponential backoff
3. `src/provider/rate_limit.rs` - Rate limiting
4. `src/provider/timeout.rs` - Timeout configuration
5. `src/provider/cache.rs` - Response caching
6. `src/provider/middleware.rs` - Middleware system
7. `src/provider/context.rs` - Context window management
8. `src/provider/embeddings.rs` - Embeddings API
9. `src/provider/batch.rs` - Batch request support
10. `examples/provider_features.rs` - Feature demonstration
11. `tests/provider_reliability.rs` - Reliability tests
12. `tests/provider_caching.rs` - Caching tests
13. `tests/provider_multimodal.rs` - Multimodal tests
14. `tests/provider_batch.rs` - Batch tests
15. `docs/PROVIDER_FEATURES.md` - Feature documentation
16. `docs/MIGRATION_GUIDE.md` - Migration guide for existing users

### Modified Files (4 files)
1. `src/provider/mod.rs` - Add new exports, update Message structure
2. `src/provider/anthropic.rs` - Integrate new infrastructure, add prompt caching
3. `src/provider/open_router.rs` - Integrate new infrastructure
4. `README.md` - Update with new features

## Backward Compatibility

### Breaking Changes
- `Message.content` changes from `String` to `Vec<ContentBlock>`

### Migration Path
1. Provide `Message::user(text)` convenience method that creates single text block
2. Add `content_as_text()` method for backward compatibility
3. Update examples to show both old and new patterns
4. Provide migration guide

### Deprecation Strategy
- Mark old constructors as `#[deprecated]` in version 0.2.0
- Remove in version 0.3.0
- Provide clear migration messages

## Verification Strategy

### Unit Tests
- Test each component in isolation
- Mock HTTP responses for provider tests
- Test error handling paths
- Test configuration builders

### Integration Tests
- Test end-to-end flows with real API calls (optional, gated by env var)
- Test provider parity (same features work on both)
- Test middleware chain execution
- Test caching behavior

### Performance Tests
- Benchmark retry overhead
- Benchmark cache hit performance
- Benchmark batch vs sequential requests
- Measure rate limiter accuracy

### Manual Testing
- Test with real Anthropic API
- Test with real OpenRouter API
- Verify prompt caching reduces costs
- Verify multimodal input works correctly

## Success Criteria

1. ✅ Both providers support retry with exponential backoff
2. ✅ Both providers support configurable timeouts
3. ✅ Both providers support rate limiting
4. ✅ Both providers support response caching
5. ✅ Anthropic provider supports prompt caching
6. ✅ Both providers support multimodal input
7. ✅ OpenRouter provider supports embeddings
8. ✅ Both providers support batch requests
9. ✅ Middleware system works with both providers
10. ✅ Context window management prevents token limit errors
11. ✅ All tests pass
12. ✅ Documentation is complete
13. ✅ Examples demonstrate all features
14. ✅ Backward compatibility maintained with migration path

## Estimated Effort

- **Phase 1 (Reliability):** 3-4 days
- **Phase 2 (Developer Experience):** 2-3 days
- **Phase 3 (Cost Optimization):** 2-3 days
- **Phase 4 (Advanced Features):** 4-5 days
- **Phase 5 (Integration & Testing):** 2-3 days

**Total:** 13-18 days of development work

## Next Steps

After plan approval:
1. Start with Phase 1 (Reliability) - most critical for production use
2. Implement features incrementally with tests
3. Update examples as features are added
4. Document each feature as it's completed
5. Get user feedback after each phase
