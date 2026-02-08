use agent_sdk::provider::{
    AnthropicProvider, Message,
    RetryConfig, RateLimitConfig, TimeoutConfig, CacheConfig,
    MiddlewareChain, LoggingMiddleware, TokenCounterMiddleware,
    ContextWindowConfig, TruncationStrategy,
    BatchRequest, SingleRequest, execute_batch_concurrent,
};
use agent_sdk::LlmProvider;
use std::env;
use std::time::Duration;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Provider Features Demo ===\n");

    // Example 1: Basic usage with retry and rate limiting
    example_retry_and_rate_limiting().await?;

    // Example 2: Response caching
    example_response_caching().await?;

    // Example 3: Middleware system
    example_middleware().await?;

    // Example 4: Context window management
    example_context_window().await?;

    // Example 5: Multimodal input
    example_multimodal().await?;

    // Example 6: Batch requests
    example_batch_requests().await?;

    Ok(())
}

async fn example_retry_and_rate_limiting() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 1: Retry and Rate Limiting ---");

    let api_key = env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable not set");

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
        .rate_limit_config(RateLimitConfig {
            requests_per_minute: 50,
            tokens_per_minute: None,
            concurrent_requests: 5,
        })
        .timeout_config(TimeoutConfig {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(120),
            stream_timeout: Some(Duration::from_secs(300)),
        })
        .build()?;

    let messages = vec![Message::user("What is the capital of France?")];
    let response = provider.generate(messages, None).await?;

    println!("Response: {}", response.content);
    if let Some(usage) = response.usage {
        println!("Tokens used: {} total", usage.total_tokens);
    }
    println!();

    Ok(())
}

async fn example_response_caching() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 2: Response Caching ---");

    let api_key = env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable not set");

    let provider = AnthropicProvider::builder()
        .api_key(api_key)
        .model("claude-3-5-sonnet-20241022")
        .cache_config(CacheConfig {
            enabled: true,
            ttl: Duration::from_secs(3600), // 1 hour
            max_entries: 1000,
        })
        .build()?;

    let messages = vec![Message::user("What is 2 + 2?")];

    // First request - will hit the API
    println!("First request (cache miss)...");
    let start = std::time::Instant::now();
    let response1 = provider.generate(messages.clone(), None).await?;
    let duration1 = start.elapsed();
    println!("Response: {}", response1.content);
    println!("Time: {:?}", duration1);

    // Second request - should be cached
    println!("\nSecond request (cache hit)...");
    let start = std::time::Instant::now();
    let response2 = provider.generate(messages, None).await?;
    let duration2 = start.elapsed();
    println!("Response: {}", response2.content);
    println!("Time: {:?} (much faster!)", duration2);
    println!();

    Ok(())
}

async fn example_middleware() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 3: Middleware System ---");

    let api_key = env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable not set");

    // Create middleware chain
    let token_counter = Arc::new(TokenCounterMiddleware::new());
    let middleware = MiddlewareChain::new()
        .add(Arc::new(LoggingMiddleware::new()))
        .add(token_counter.clone());

    let provider = AnthropicProvider::builder()
        .api_key(api_key)
        .model("claude-3-5-sonnet-20241022")
        .middleware(middleware)
        .build()?;

    // Make a few requests
    for i in 1..=3 {
        let messages = vec![Message::user(format!("Count to {}", i))];
        let _response = provider.generate(messages, None).await?;
    }

    // Check token usage
    println!("\nTotal tokens used across all requests:");
    println!("  Prompt tokens: {}", token_counter.total_prompt_tokens());
    println!("  Completion tokens: {}", token_counter.total_completion_tokens());
    println!("  Total: {}", token_counter.total_tokens());
    println!();

    Ok(())
}

async fn example_context_window() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 4: Context Window Management ---");

    let api_key = env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable not set");

    let provider = AnthropicProvider::builder()
        .api_key(api_key)
        .model("claude-3-5-sonnet-20241022")
        .context_config(ContextWindowConfig {
            max_tokens: 1000, // Small window for demo
            truncation_strategy: TruncationStrategy::DropOldest,
        })
        .build()?;

    // Create a conversation with many messages
    let mut messages = vec![
        Message::system("You are a helpful assistant."),
    ];

    // Add many user/assistant exchanges
    for i in 1..=10 {
        messages.push(Message::user(format!("Message {}", i)));
        messages.push(Message::assistant(format!("Response {}", i)));
    }

    println!("Sending {} messages (will be truncated to fit context window)", messages.len());

    let response = provider.generate(messages, None).await?;
    println!("Response: {}", response.content);
    println!();

    Ok(())
}

async fn example_multimodal() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 5: Multimodal Input ---");

    let api_key = env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable not set");

    let _provider = AnthropicProvider::builder()
        .api_key(api_key)
        .model("claude-3-5-sonnet-20241022")
        .build()?;

    // Example with image URL
    let _messages = vec![
        Message::user_with_image_url(
            "What do you see in this image?",
            "https://example.com/image.jpg"
        )
    ];

    println!("Sending request with image URL...");
    println!("(Note: This will fail with example URL, use a real image URL)");

    // Uncomment to actually send the request:
    // let response = provider.generate(messages, None).await?;
    // println!("Response: {}", response.content);

    println!();

    Ok(())
}

async fn example_batch_requests() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 6: Batch Requests ---");

    let api_key = env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable not set");

    let provider = AnthropicProvider::builder()
        .api_key(api_key)
        .model("claude-3-5-sonnet-20241022")
        .build()?;

    // Create batch of requests
    let batch = BatchRequest::new(vec![
        SingleRequest::new("req1", vec![Message::user("What is 1+1?")]),
        SingleRequest::new("req2", vec![Message::user("What is 2+2?")]),
        SingleRequest::new("req3", vec![Message::user("What is 3+3?")]),
    ])
    .with_max_concurrent(2); // Process 2 at a time

    println!("Processing batch of {} requests...", batch.len());

    let results = execute_batch_concurrent(&provider, batch).await?;

    println!("\nResults:");
    for response in &results.responses {
        match &response.result {
            Ok(gen_response) => {
                println!("  {}: {}", response.id, gen_response.content);
            }
            Err(e) => {
                println!("  {}: Error - {}", response.id, e);
            }
        }
    }

    println!("\nSuccess rate: {}/{}", results.success_count(), results.responses.len());
    println!();

    Ok(())
}
