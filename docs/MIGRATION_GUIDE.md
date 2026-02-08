# Migration Guide

This guide helps you migrate from older versions of agent-sdk-rs to the latest version with enhanced provider features.

## Overview of Changes

The latest version introduces significant enhancements to the provider system:

1. **Message Structure** - Changed to support multimodal content
2. **Provider Constructors** - Now return `Result` for better error handling
3. **Builder Pattern** - New fluent API for configuration
4. **New Features** - Retry, rate limiting, caching, middleware, and more

## Breaking Changes

### 1. Message Structure

**Before (v0.0.x):**
```rust
let message = Message {
    role: Role::User,
    content: "Hello, world!".to_string(),
};
```

**After (v0.1.0):**
```rust
// Use convenience methods
let message = Message::user("Hello, world!");

// Or for multimodal content
let message = Message::user_with_image_url(
    "What's in this image?",
    "https://example.com/image.jpg"
);
```

**Why:** The message structure now supports multimodal content (text + images) through a `Vec<ContentBlock>` instead of a simple `String`.

**Migration Steps:**
1. Replace direct struct construction with convenience methods
2. Use `Message::user()`, `Message::system()`, or `Message::assistant()`
3. For multimodal content, use `Message::user_with_image_url()` or `Message::user_with_image_base64()`

### 2. Provider Constructors

**Before (v0.0.x):**
```rust
let provider = AnthropicProvider::new(api_key, model);
```

**After (v0.1.0):**
```rust
// Simple usage
let provider = AnthropicProvider::new(api_key, model)?;

// Or use builder for more control
let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model(model)
    .build()?;
```

**Why:** Constructors now return `Result` to handle configuration errors properly.

**Migration Steps:**
1. Add `?` operator after `new()` calls
2. Or switch to builder pattern for more features
3. Handle the `Result` in your error handling flow

### 3. ProviderError Changes

**Before (v0.0.x):**
```rust
ProviderError::AuthenticationFailed
```

**After (v0.1.0):**
```rust
ProviderError::AuthenticationFailed(String)
```

**Why:** Error variants now include more context for better debugging.

**Migration Steps:**
1. Update pattern matching to handle the new error structure
2. Extract error messages from the `String` parameter

## New Features Migration

### Adding Retry Logic

**Before:**
```rust
let provider = AnthropicProvider::new(api_key, model);
// Manual retry logic required
```

**After:**
```rust
use agent_sdk::provider::RetryConfig;
use std::time::Duration;

let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model(model)
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

### Adding Rate Limiting

**Before:**
```rust
let provider = AnthropicProvider::new(api_key, model);
// No built-in rate limiting
```

**After:**
```rust
use agent_sdk::provider::RateLimitConfig;

let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model(model)
    .rate_limit_config(RateLimitConfig {
        requests_per_minute: 50,
        concurrent_requests: 10,
        tokens_per_minute: None,
    })
    .build()?;
```

### Adding Response Caching

**Before:**
```rust
let provider = AnthropicProvider::new(api_key, model);
// No built-in caching
```

**After:**
```rust
use agent_sdk::provider::CacheConfig;
use std::time::Duration;

let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model(model)
    .cache_config(CacheConfig {
        enabled: true,
        ttl: Duration::from_secs(3600),
        max_entries: 1000,
    })
    .build()?;
```

### Adding Middleware

**Before:**
```rust
let provider = AnthropicProvider::new(api_key, model);
// Manual logging required
```

**After:**
```rust
use agent_sdk::provider::{MiddlewareChain, LoggingMiddleware, TokenCounterMiddleware};
use std::sync::Arc;

let token_counter = Arc::new(TokenCounterMiddleware::new());
let middleware = MiddlewareChain::new()
    .add(Arc::new(LoggingMiddleware::new()))
    .add(token_counter.clone());

let provider = AnthropicProvider::builder()
    .api_key(api_key)
    .model(model)
    .middleware(middleware)
    .build()?;

// After requests, check token usage
println!("Total tokens: {}", token_counter.total_tokens());
```

## Step-by-Step Migration

### Step 1: Update Dependencies

Update your `Cargo.toml`:

```toml
[dependencies]
agent-sdk = "0.1.0"
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", features = ["full"] }
```

### Step 2: Update Message Construction

Find all instances of direct `Message` struct construction:

```bash
# Search for old pattern
grep -r "Message {" src/
```

Replace with convenience methods:

```rust
// Old
Message { role: Role::User, content: "text".to_string() }

// New
Message::user("text")
```

### Step 3: Update Provider Construction

Find all provider instantiations:

```bash
# Search for old pattern
grep -r "Provider::new" src/
```

Add error handling:

```rust
// Old
let provider = AnthropicProvider::new(api_key, model);

// New
let provider = AnthropicProvider::new(api_key, model)?;
```

### Step 4: Update Error Handling

Find all error pattern matching:

```bash
# Search for error handling
grep -r "ProviderError::" src/
```

Update patterns to handle new error structure:

```rust
// Old
match error {
    ProviderError::AuthenticationFailed => {
        eprintln!("Auth failed");
    }
}

// New
match error {
    ProviderError::AuthenticationFailed(msg) => {
        eprintln!("Auth failed: {}", msg);
    }
}
```

### Step 5: Add New Features (Optional)

Gradually add new features as needed:

1. Start with retry logic for reliability
2. Add rate limiting to prevent API limit errors
3. Enable caching for cost savings
4. Add middleware for observability
5. Configure context window management

## Example Migration

### Before (v0.0.x)

```rust
use agent_sdk::provider::{AnthropicProvider, Message, Role};

#[tokio::main]
async fn main() {
    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap();
    let provider = AnthropicProvider::new(api_key, "claude-3-5-sonnet-20241022");

    let messages = vec![
        Message {
            role: Role::User,
            content: "Hello!".to_string(),
        }
    ];

    match provider.generate(messages, None).await {
        Ok(response) => println!("{}", response.content),
        Err(ProviderError::AuthenticationFailed) => {
            eprintln!("Auth failed");
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

### After (v0.1.0)

```rust
use agent_sdk::provider::{
    AnthropicProvider, Message, ProviderError,
    RetryConfig, CacheConfig, MiddlewareChain, LoggingMiddleware,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")?;

    let provider = AnthropicProvider::builder()
        .api_key(api_key)
        .model("claude-3-5-sonnet-20241022")
        .retry_config(RetryConfig::default())
        .cache_config(CacheConfig::default())
        .middleware(MiddlewareChain::new()
            .add(Arc::new(LoggingMiddleware::new())))
        .build()?;

    let messages = vec![Message::user("Hello!")];

    match provider.generate(messages, None).await {
        Ok(response) => println!("{}", response.content),
        Err(ProviderError::AuthenticationFailed(msg)) => {
            eprintln!("Auth failed: {}", msg);
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}
```

## Common Migration Issues

### Issue 1: Type Mismatch on Message Content

**Error:**
```
expected `Vec<ContentBlock>`, found `String`
```

**Solution:**
Use convenience methods instead of direct struct construction:
```rust
// Don't do this
Message { role: Role::User, content: "text".to_string() }

// Do this
Message::user("text")
```

### Issue 2: Provider Constructor Returns Result

**Error:**
```
mismatched types: expected `AnthropicProvider`, found `Result<AnthropicProvider, ProviderError>`
```

**Solution:**
Add `?` operator or handle the Result:
```rust
// Option 1: Propagate error
let provider = AnthropicProvider::new(api_key, model)?;

// Option 2: Handle error
let provider = match AnthropicProvider::new(api_key, model) {
    Ok(p) => p,
    Err(e) => {
        eprintln!("Failed to create provider: {}", e);
        return;
    }
};
```

### Issue 3: Error Pattern Matching

**Error:**
```
pattern `ProviderError::AuthenticationFailed(_)` not covered
```

**Solution:**
Update pattern to include the message parameter:
```rust
match error {
    ProviderError::AuthenticationFailed(msg) => {
        eprintln!("Auth failed: {}", msg);
    }
    // ... other patterns
}
```

### Issue 4: Accessing Message Content

**Error:**
```
no field `content` on type `Message`
```

**Solution:**
Use `content_as_text()` method:
```rust
// Old
let text = message.content;

// New
let text = message.content_as_text();
```

## Testing Your Migration

After migrating, run these checks:

1. **Compile Check:**
   ```bash
   cargo check
   ```

2. **Run Tests:**
   ```bash
   cargo test
   ```

3. **Run Examples:**
   ```bash
   cargo run --example your_example
   ```

4. **Check for Deprecation Warnings:**
   ```bash
   cargo build 2>&1 | grep -i "deprecated"
   ```

## Gradual Migration Strategy

You don't have to migrate everything at once. Here's a recommended approach:

### Phase 1: Fix Breaking Changes (Required)
1. Update message construction to use convenience methods
2. Add `?` operator to provider constructors
3. Update error pattern matching

### Phase 2: Add Reliability (Recommended)
1. Enable retry logic
2. Configure timeouts
3. Add rate limiting

### Phase 3: Add Observability (Recommended)
1. Add logging middleware
2. Add token counter middleware
3. Add metrics middleware

### Phase 4: Optimize Costs (Optional)
1. Enable response caching
2. Configure Anthropic prompt caching
3. Add context window management

### Phase 5: Advanced Features (Optional)
1. Add multimodal support where needed
2. Use batch requests for multiple queries
3. Implement custom middleware

## Rollback Plan

If you encounter issues during migration:

1. **Keep Old Version:**
   ```toml
   [dependencies]
   agent-sdk = "0.0.x"  # Old version
   ```

2. **Use Feature Flags:**
   ```rust
   #[cfg(feature = "new-provider")]
   use agent_sdk::provider::AnthropicProvider;

   #[cfg(not(feature = "new-provider"))]
   use agent_sdk::provider::legacy::AnthropicProvider;
   ```

3. **Gradual Rollout:**
   - Migrate one module at a time
   - Test thoroughly before moving to the next module
   - Keep old code in a separate branch

## Getting Help

If you encounter issues during migration:

1. Check the [Provider Features Guide](PROVIDER_FEATURES.md)
2. Review the [examples](../examples/) directory
3. Search for similar issues in the GitHub repository
4. Open an issue with your migration question

## Deprecation Timeline

- **v0.1.0** (Current): Old message structure still works with deprecation warnings
- **v0.2.0** (Future): Old message structure removed, must use new structure
- **v0.3.0** (Future): Additional breaking changes may be introduced

## Summary

The migration to v0.1.0 brings significant improvements:

✅ **Better Reliability** - Automatic retry and rate limiting
✅ **Lower Costs** - Response caching and prompt caching
✅ **Better Observability** - Middleware system for logging and metrics
✅ **More Features** - Multimodal support, batch requests, embeddings

While there are breaking changes, the migration is straightforward and the benefits are substantial. Most codebases can be migrated in a few hours.
