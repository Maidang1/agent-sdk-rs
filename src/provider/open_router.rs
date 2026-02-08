use super::{
    CacheConfig, CacheKey, ContextWindowConfig, ContextWindowManager, GenerateOptions,
    GenerateResponse, LlmProvider, Message, MiddlewareChain, ProviderClient, ProviderClientBuilder,
    ProviderError, RateLimitConfig, ResponseCache, Result, RetryConfig, Role, TimeoutConfig, Usage,
};
use futures_util::StreamExt;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::mpsc;

/// OpenRouter Provider 实现
pub struct OpenRouterProvider {
    api_key: String,
    model: String,
    client: ProviderClient,
    base_url: String,
    middleware: Option<MiddlewareChain>,
    cache: Option<ResponseCache>,
    context_manager: Option<ContextWindowManager>,
}

impl OpenRouterProvider {
    /// Create a new OpenRouter provider with default configuration
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Result<Self> {
        Self::builder().api_key(api_key).model(model).build()
    }

    /// Create a builder for configuring the OpenRouter provider
    pub fn builder() -> OpenRouterProviderBuilder {
        OpenRouterProviderBuilder::default()
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    fn build_request_body(
        &self,
        messages: Vec<Message>,
        options: Option<GenerateOptions>,
        stream: bool,
    ) -> serde_json::Value {
        let opts = options.unwrap_or_default();

        let messages_json: Vec<serde_json::Value> = messages
            .into_iter()
            .map(|m| {
                let role = match m.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                };

                // Format content - OpenAI format supports both string and array
                let content = if m.content.len() == 1 {
                    // Single text block - use string format
                    if let super::ContentBlock::Text { text } = &m.content[0] {
                        serde_json::json!(text)
                    } else {
                        // Single image block - use array format
                        self.format_content_blocks(&m.content)
                    }
                } else {
                    // Multiple blocks - use array format
                    self.format_content_blocks(&m.content)
                };

                serde_json::json!({
                    "role": role,
                    "content": content,
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": &self.model,
            "messages": messages_json,
            "stream": stream,
        });

        if let Some(temp) = opts.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(max) = opts.max_tokens {
            body["max_tokens"] = serde_json::json!(max);
        }
        if let Some(top_p) = opts.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(stop) = opts.stop {
            body["stop"] = serde_json::json!(stop);
        }

        body
    }

    fn format_content_blocks(&self, content: &[super::ContentBlock]) -> serde_json::Value {
        use super::{ContentBlock, ImageSource};

        serde_json::json!(content
            .iter()
            .map(|block| match block {
                ContentBlock::Text { text } => serde_json::json!({
                    "type": "text",
                    "text": text,
                }),
                ContentBlock::Image { source, detail } => {
                    let mut img = serde_json::json!({
                        "type": "image_url",
                    });
                    match source {
                        ImageSource::Url { url } => {
                            let mut image_url = serde_json::json!({
                                "url": url,
                            });
                            if let Some(d) = detail {
                                image_url["detail"] = serde_json::json!(match d {
                                    super::ImageDetail::Low => "low",
                                    super::ImageDetail::High => "high",
                                    super::ImageDetail::Auto => "auto",
                                });
                            }
                            img["image_url"] = image_url;
                        }
                        ImageSource::Base64 { media_type, data } => {
                            let data_url = format!("data:{};base64,{}", media_type, data);
                            let mut image_url = serde_json::json!({
                                "url": data_url,
                            });
                            if let Some(d) = detail {
                                image_url["detail"] = serde_json::json!(match d {
                                    super::ImageDetail::Low => "low",
                                    super::ImageDetail::High => "high",
                                    super::ImageDetail::Auto => "auto",
                                });
                            }
                            img["image_url"] = image_url;
                        }
                    }
                    img
                }
            })
            .collect::<Vec<_>>())
    }

    async fn send_request(&self, body: serde_json::Value) -> Result<reqwest::Response> {
        let _guard = self.client.acquire_rate_limit().await;

        self.client
            .retry_policy()
            .execute_with_retry(|| async {
                let response = self
                    .client
                    .http_client()
                    .post(format!("{}/chat/completions", self.base_url))
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| ProviderError::RequestFailed(e.to_string()))?;

                let status = response.status();
                if status == reqwest::StatusCode::UNAUTHORIZED {
                    return Err(ProviderError::AuthenticationFailed(
                        "Invalid API key".to_string(),
                    ));
                }
                if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    let retry_after = response
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse().ok());
                    return Err(ProviderError::RateLimited { retry_after });
                }
                if !status.is_success() {
                    let text = response.text().await.unwrap_or_default();
                    return Err(ProviderError::RequestFailed(format!(
                        "{}: {}",
                        status, text
                    )));
                }

                Ok(response)
            })
            .await
    }
}

/// Builder for creating an OpenRouterProvider with custom configuration
pub struct OpenRouterProviderBuilder {
    api_key: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    client_builder: ProviderClientBuilder,
    middleware: Option<MiddlewareChain>,
    cache_config: Option<CacheConfig>,
    context_config: Option<ContextWindowConfig>,
}

impl Default for OpenRouterProviderBuilder {
    fn default() -> Self {
        Self {
            api_key: None,
            model: None,
            base_url: None,
            client_builder: ProviderClient::builder(),
            middleware: None,
            cache_config: None,
            context_config: None,
        }
    }
}

impl OpenRouterProviderBuilder {
    /// Set the API key
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the model
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the base URL
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set the retry configuration
    pub fn retry_config(mut self, config: RetryConfig) -> Self {
        self.client_builder = self.client_builder.retry_config(config);
        self
    }

    /// Set the timeout configuration
    pub fn timeout_config(mut self, config: TimeoutConfig) -> Self {
        self.client_builder = self.client_builder.timeout_config(config);
        self
    }

    /// Set the rate limit configuration
    pub fn rate_limit_config(mut self, config: RateLimitConfig) -> Self {
        self.client_builder = self.client_builder.rate_limit_config(config);
        self
    }

    /// Set a proxy URL
    pub fn proxy(mut self, proxy: impl Into<String>) -> Self {
        self.client_builder = self.client_builder.proxy(proxy);
        self
    }

    /// Set the middleware chain
    pub fn middleware(mut self, middleware: MiddlewareChain) -> Self {
        self.middleware = Some(middleware);
        self
    }

    /// Enable response caching with the given configuration
    pub fn cache_config(mut self, config: CacheConfig) -> Self {
        self.cache_config = Some(config);
        self
    }

    /// Enable context window management with the given configuration
    pub fn context_config(mut self, config: ContextWindowConfig) -> Self {
        self.context_config = Some(config);
        self
    }

    /// Disable retries
    pub fn no_retry(mut self) -> Self {
        self.client_builder = self.client_builder.no_retry();
        self
    }

    /// Disable rate limiting
    pub fn no_rate_limit(mut self) -> Self {
        self.client_builder = self.client_builder.no_rate_limit();
        self
    }

    /// Build the OpenRouter provider
    pub fn build(self) -> Result<OpenRouterProvider> {
        let api_key = self
            .api_key
            .ok_or_else(|| ProviderError::RequestFailed("API key is required".to_string()))?;

        let model = self
            .model
            .ok_or_else(|| ProviderError::RequestFailed("Model is required".to_string()))?;

        let client = self.client_builder.build()?;

        let cache = self.cache_config.map(ResponseCache::new);
        let context_manager = self.context_config.map(ContextWindowManager::new);

        Ok(OpenRouterProvider {
            api_key,
            model,
            client,
            base_url: self
                .base_url
                .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string()),
            middleware: self.middleware,
            cache,
            context_manager,
        })
    }
}

impl LlmProvider for OpenRouterProvider {
    fn name(&self) -> &str {
        "openrouter"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn generate(
        &self,
        messages: Vec<Message>,
        options: Option<GenerateOptions>,
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse>> + Send + '_>> {
        Box::pin(async move {
            // Apply context window management if configured
            let messages = if let Some(manager) = &self.context_manager {
                manager.truncate_if_needed(messages)
            } else {
                messages
            };

            // Check cache first
            if let Some(cache) = &self.cache {
                let key = CacheKey::from_request(&messages, &self.model, &options);
                if let Some(cached) = cache.get(&key).await {
                    return Ok(cached);
                }
            }

            // Execute middleware before_request
            let mut ctx = super::RequestContext {
                messages: messages.clone(),
                options: options.clone(),
                metadata: std::collections::HashMap::new(),
            };

            if let Some(mw) = &self.middleware {
                if let Err(e) = mw.execute_before(&mut ctx).await {
                    if let Some(mw) = &self.middleware {
                        let _ = mw.execute_error(&e).await;
                    }
                    return Err(e);
                }
            }

            // Make the actual request
            let result = async {
                let body =
                    self.build_request_body(ctx.messages.clone(), ctx.options.clone(), false);
                let response = self.send_request(body).await?;

                let json: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|e| ProviderError::ParseError(e.to_string()))?;

                let content = json["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();

                let usage = json.get("usage").map(|u| Usage {
                    prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                    completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
                    total_tokens: u["total_tokens"].as_u64().unwrap_or(0) as u32,
                });

                let finish_reason = json["choices"][0]["finish_reason"]
                    .as_str()
                    .map(String::from);

                Ok(GenerateResponse {
                    content,
                    usage,
                    model: self.model.clone(),
                    finish_reason,
                })
            }
            .await;

            match result {
                Ok(response) => {
                    // Store in cache
                    if let Some(cache) = &self.cache {
                        let key = CacheKey::from_request(&messages, &self.model, &options);
                        cache.put(key, response.clone()).await;
                    }

                    // Execute middleware after_response
                    let mut resp_ctx = super::ResponseContext {
                        response: response.clone(),
                        metadata: ctx.metadata,
                    };

                    if let Some(mw) = &self.middleware {
                        mw.execute_after(&mut resp_ctx).await?;
                    }

                    Ok(resp_ctx.response)
                }
                Err(e) => {
                    // Execute middleware on_error
                    if let Some(mw) = &self.middleware {
                        let _ = mw.execute_error(&e).await;
                    }
                    Err(e)
                }
            }
        })
    }

    fn generate_stream(
        &self,
        messages: Vec<Message>,
        options: Option<GenerateOptions>,
    ) -> Pin<Box<dyn Future<Output = Result<super::StreamResponse>> + Send + '_>> {
        Box::pin(async move {
            let body = self.build_request_body(messages, options, true);
            let response = self.send_request(body).await?;

            let (tx, rx) = mpsc::channel(100);

            tokio::spawn(async move {
                let mut stream = response.bytes_stream();
                let mut buffer = String::new();

                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(bytes) => {
                            buffer.push_str(&String::from_utf8_lossy(&bytes));

                            while let Some(line_end) = buffer.find('\n') {
                                let line = buffer[..line_end].trim().to_string();
                                buffer.drain(..=line_end);

                                if let Some(data) = line.strip_prefix("data: ") {
                                    if data == "[DONE]" {
                                        break;
                                    }

                                    if let Ok(json) =
                                        serde_json::from_str::<serde_json::Value>(data)
                                    {
                                        if let Some(content) =
                                            json["choices"][0]["delta"]["content"].as_str()
                                        {
                                            if tx.send(Ok(content.to_string())).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx
                                .send(Err(ProviderError::RequestFailed(e.to_string())))
                                .await;
                            break;
                        }
                    }
                }
            });

            Ok(super::StreamResponse { receiver: rx })
        })
    }
}
