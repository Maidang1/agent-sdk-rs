use super::{
    GenerateOptions, GenerateResponse, LlmProvider, Message, ProviderError, Result, Role, Usage,
    ProviderClient, ProviderClientBuilder, RetryConfig, RateLimitConfig, TimeoutConfig,
    MiddlewareChain, ResponseCache, CacheConfig, CacheKey, ContextWindowManager, ContextWindowConfig,
};
use futures_util::StreamExt;
use std::env;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 1024;

/// Configuration for Anthropic prompt caching
#[derive(Debug, Clone)]
pub struct PromptCacheConfig {
    /// Whether prompt caching is enabled
    pub enabled: bool,
    /// Whether to cache system messages
    pub cache_system_messages: bool,
    /// Whether to cache tool definitions
    pub cache_tool_definitions: bool,
}

impl Default for PromptCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cache_system_messages: true,
            cache_tool_definitions: true,
        }
    }
}

impl PromptCacheConfig {
    /// Create a new prompt cache configuration
    pub fn new(enabled: bool, cache_system: bool, cache_tools: bool) -> Self {
        Self {
            enabled,
            cache_system_messages: cache_system,
            cache_tool_definitions: cache_tools,
        }
    }

    /// Disable prompt caching
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            cache_system_messages: false,
            cache_tool_definitions: false,
        }
    }
}

pub struct AnthropicProvider {
    api_key: String,
    auth_token: Option<String>,
    model: String,
    client: ProviderClient,
    base_url: String,
    middleware: Option<MiddlewareChain>,
    cache: Option<ResponseCache>,
    context_manager: Option<ContextWindowManager>,
    prompt_cache_config: PromptCacheConfig,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider with default configuration
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Result<Self> {
        Self::builder()
            .api_key(api_key)
            .model(model)
            .build()
    }

    /// Create a builder for configuring the Anthropic provider
    pub fn builder() -> AnthropicProviderBuilder {
        AnthropicProviderBuilder::default()
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    fn read_auth_token_from_env() -> Option<String> {
        env::var("ANTHROPIC_AUTH_TOKEN")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    }

    fn split_system_and_messages(
        messages: Vec<Message>,
    ) -> (Option<String>, Vec<serde_json::Value>) {
        let mut system_messages = Vec::new();
        let mut chat_messages = Vec::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    // Extract text from system message content blocks
                    system_messages.push(msg.content_as_text());
                }
                Role::User | Role::Assistant => {
                    let role = match msg.role {
                        Role::User => "user",
                        Role::Assistant => "assistant",
                        Role::System => unreachable!(),
                    };

                    // Format content blocks for API
                    let content = Self::format_message_content(&msg.content);

                    chat_messages.push(serde_json::json!({
                        "role": role,
                        "content": content,
                    }));
                }
            }
        }

        let system = if system_messages.is_empty() {
            None
        } else {
            Some(system_messages.join("\n\n"))
        };

        (system, chat_messages)
    }

    fn format_message_content(content: &[super::ContentBlock]) -> serde_json::Value {
        use super::{ContentBlock, ImageSource};

        // If only one text block, return as string for simplicity
        if content.len() == 1 {
            if let ContentBlock::Text { text } = &content[0] {
                return serde_json::json!(text);
            }
        }

        // Multiple blocks or contains images - return as array
        serde_json::json!(content.iter().map(|block| match block {
            ContentBlock::Text { text } => serde_json::json!({
                "type": "text",
                "text": text,
            }),
            ContentBlock::Image { source, detail } => {
                let mut img = serde_json::json!({
                    "type": "image",
                });
                match source {
                    ImageSource::Url { url } => {
                        img["source"] = serde_json::json!({
                            "type": "url",
                            "url": url,
                        });
                    }
                    ImageSource::Base64 { media_type, data } => {
                        img["source"] = serde_json::json!({
                            "type": "base64",
                            "media_type": media_type,
                            "data": data,
                        });
                    }
                }
                if let Some(d) = detail {
                    img["detail"] = serde_json::json!(match d {
                        super::ImageDetail::Low => "low",
                        super::ImageDetail::High => "high",
                        super::ImageDetail::Auto => "auto",
                    });
                }
                img
            }
        }).collect::<Vec<_>>())
    }

    fn build_request_body_for_model(
        model: &str,
        messages: Vec<Message>,
        options: Option<GenerateOptions>,
        stream: bool,
    ) -> serde_json::Value {
        let opts = options.unwrap_or_default();
        let (system, messages_json) = Self::split_system_and_messages(messages);

        let mut body = serde_json::json!({
            "model": model,
            "messages": messages_json,
            "stream": stream,
            "max_tokens": opts.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
        });

        if let Some(system_prompt) = system {
            body["system"] = serde_json::json!(system_prompt);
        }
        if let Some(temp) = opts.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(top_p) = opts.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(stop) = opts.stop {
            body["stop_sequences"] = serde_json::json!(stop);
        }

        body
    }

    fn build_request_body(
        &self,
        messages: Vec<Message>,
        options: Option<GenerateOptions>,
        stream: bool,
    ) -> serde_json::Value {
        Self::build_request_body_for_model(&self.model, messages, options, stream)
    }

    fn map_status_error(
        status: reqwest::StatusCode,
        headers: &reqwest::header::HeaderMap,
        text: String,
    ) -> ProviderError {
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return ProviderError::AuthenticationFailed(text);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = headers
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok());
            return ProviderError::RateLimited { retry_after };
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            return ProviderError::ModelNotAvailable(text);
        }
        ProviderError::RequestFailed(format!("{}: {}", status, text))
    }

    async fn send_request(&self, body: serde_json::Value) -> Result<reqwest::Response> {
        let _guard = self.client.acquire_rate_limit().await;

        self.client.retry_policy().execute_with_retry(|| async {
            let mut request = self
                .client
                .http_client()
                .post(format!("{}/messages", self.base_url))
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header("content-type", "application/json");

            if self.api_key.trim().is_empty() && self.auth_token.is_none() {
                return Err(ProviderError::AuthenticationFailed("No API key or auth token provided".to_string()));
            }

            if !self.api_key.trim().is_empty() {
                request = request.header("x-api-key", &self.api_key);
            }

            if let Some(token) = &self.auth_token {
                request = request.header("authorization", format!("Bearer {}", token));
            }

            let response = request
                .json(&body)
                .send()
                .await
                .map_err(|e| ProviderError::RequestFailed(e.to_string()))?;

            let status = response.status();
            if !status.is_success() {
                let headers = response.headers().clone();
                let text = response.text().await.unwrap_or_default();
                return Err(Self::map_status_error(status, &headers, text));
            }

            Ok(response)
        }).await
    }

    fn parse_generate_response_with_model(
        json: serde_json::Value,
        fallback_model: &str,
    ) -> GenerateResponse {
        let content = json["content"]
            .as_array()
            .and_then(|arr| {
                arr.iter().find_map(|block| {
                    if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                        block.get("text").and_then(|v| v.as_str()).map(String::from)
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_default();

        let usage = json.get("usage").map(|u| {
            let prompt_tokens = u["input_tokens"].as_u64().unwrap_or(0) as u32;
            let completion_tokens = u["output_tokens"].as_u64().unwrap_or(0) as u32;
            Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens.saturating_add(completion_tokens),
            }
        });

        let finish_reason = json["stop_reason"].as_str().map(String::from);
        let model = json["model"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| fallback_model.to_string());

        GenerateResponse {
            content,
            usage,
            model,
            finish_reason,
        }
    }

    fn parse_generate_response(&self, json: serde_json::Value) -> Result<GenerateResponse> {
        Ok(Self::parse_generate_response_with_model(json, &self.model))
    }

    fn extract_stream_text(event_json: &serde_json::Value) -> Option<String> {
        let event_type = event_json.get("type").and_then(|v| v.as_str())?;
        if event_type == "content_block_delta"
            && event_json["delta"]["type"].as_str() == Some("text_delta")
        {
            return event_json["delta"]["text"].as_str().map(String::from);
        }
        None
    }
}

/// Builder for creating an AnthropicProvider with custom configuration
pub struct AnthropicProviderBuilder {
    api_key: Option<String>,
    auth_token: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    client_builder: ProviderClientBuilder,
    middleware: Option<MiddlewareChain>,
    cache_config: Option<CacheConfig>,
    context_config: Option<ContextWindowConfig>,
    prompt_cache_config: PromptCacheConfig,
}

impl Default for AnthropicProviderBuilder {
    fn default() -> Self {
        Self {
            api_key: None,
            auth_token: AnthropicProvider::read_auth_token_from_env(),
            model: None,
            base_url: None,
            client_builder: ProviderClient::builder(),
            middleware: None,
            cache_config: None,
            context_config: None,
            prompt_cache_config: PromptCacheConfig::default(),
        }
    }
}

impl AnthropicProviderBuilder {
    /// Set the API key
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the auth token
    pub fn auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
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

    /// Set the prompt cache configuration
    pub fn prompt_cache_config(mut self, config: PromptCacheConfig) -> Self {
        self.prompt_cache_config = config;
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

    /// Build the Anthropic provider
    pub fn build(self) -> Result<AnthropicProvider> {
        let api_key = self.api_key.ok_or_else(|| {
            ProviderError::RequestFailed("API key is required".to_string())
        })?;

        let model = self.model.ok_or_else(|| {
            ProviderError::RequestFailed("Model is required".to_string())
        })?;

        let client = self.client_builder.build()?;

        let cache = self.cache_config.map(ResponseCache::new);
        let context_manager = self.context_config.map(ContextWindowManager::new);

        Ok(AnthropicProvider {
            api_key,
            auth_token: self.auth_token,
            model,
            client,
            base_url: self.base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            middleware: self.middleware,
            cache,
            context_manager,
            prompt_cache_config: self.prompt_cache_config,
        })
    }
}

impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
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
                let body = self.build_request_body(ctx.messages.clone(), ctx.options.clone(), false);
                let response = self.send_request(body).await?;
                let json: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|e| ProviderError::ParseError(e.to_string()))?;
                self.parse_generate_response(json)
            }.await;

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
                                    if data.is_empty() {
                                        continue;
                                    }
                                    if let Ok(event_json) =
                                        serde_json::from_str::<serde_json::Value>(data)
                                    {
                                        if let Some(text) = Self::extract_stream_text(&event_json) {
                                            if tx.send(Ok(text)).await.is_err() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_system_messages_and_chat_messages() {
        let messages = vec![
            Message::system("sys A"),
            Message::user("u1"),
            Message::assistant("a1"),
            Message::system("sys B"),
        ];
        let (system, chat) = AnthropicProvider::split_system_and_messages(messages);

        assert_eq!(system.as_deref(), Some("sys A\n\nsys B"));
        assert_eq!(chat.len(), 2);
        assert_eq!(chat[0]["role"], "user");
        assert_eq!(chat[1]["role"], "assistant");
    }

    #[test]
    fn request_body_maps_stop_to_stop_sequences() {
        let body = AnthropicProvider::build_request_body_for_model(
            "claude-3-5-sonnet-20241022",
            vec![Message::system("sys"), Message::user("hello")],
            Some(GenerateOptions {
                temperature: Some(0.2),
                max_tokens: Some(42),
                top_p: Some(0.9),
                stop: Some(vec!["END".to_string()]),
            }),
            false,
        );

        assert_eq!(body["model"], "claude-3-5-sonnet-20241022");
        assert_eq!(body["max_tokens"], 42);
        assert_eq!(body["stream"], false);
        assert_eq!(body["system"], "sys");
        assert_eq!(body["stop_sequences"][0], "END");
    }

    #[test]
    fn parse_non_stream_response() {
        let json = serde_json::json!({
            "model": "claude-3-5-sonnet-20241022",
            "stop_reason": "end_turn",
            "content": [{"type":"text","text":"hello world"}],
            "usage": {"input_tokens": 11, "output_tokens": 7}
        });

        let resp = AnthropicProvider::parse_generate_response_with_model(
            json,
            "claude-3-5-sonnet-20241022",
        );
        assert_eq!(resp.content, "hello world");
        assert_eq!(resp.finish_reason.as_deref(), Some("end_turn"));
        assert_eq!(resp.usage.as_ref().map(|u| u.total_tokens), Some(18));
    }

    #[test]
    fn extract_stream_text_delta_only() {
        let text_event = serde_json::json!({
            "type":"content_block_delta",
            "delta":{"type":"text_delta","text":"abc"}
        });
        let non_text_event = serde_json::json!({
            "type":"message_start"
        });

        assert_eq!(
            AnthropicProvider::extract_stream_text(&text_event).as_deref(),
            Some("abc")
        );
        assert!(AnthropicProvider::extract_stream_text(&non_text_event).is_none());
    }

    #[test]
    fn reads_auth_token_from_env_like_runtime() {
        assert_eq!(
            AnthropicProvider::read_auth_token_from_env(),
            env::var("ANTHROPIC_AUTH_TOKEN")
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        );
    }
}
