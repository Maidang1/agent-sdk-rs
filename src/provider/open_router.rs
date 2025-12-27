use super::{
    GenerateOptions, GenerateResponse, LlmProvider, Message, ProviderError, Result, Role, Usage,
};
use futures_util::StreamExt;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::mpsc;

/// OpenRouter Provider 实现
pub struct OpenRouterProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
    base_url: String,
}

impl OpenRouterProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            client: reqwest::Client::new(),
            base_url: "https://openrouter.ai/api/v1".into(),
        }
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
                serde_json::json!({
                    "role": match m.role {
                        Role::System => "system",
                        Role::User => "user",
                        Role::Assistant => "assistant",
                    },
                    "content": m.content
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

    async fn send_request(&self, body: serde_json::Value) -> Result<reqwest::Response> {
        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::RequestFailed(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ProviderError::AuthenticationFailed);
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
            return Err(ProviderError::RequestFailed(format!("{}: {}", status, text)));
        }

        Ok(response)
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
            let body = self.build_request_body(messages, options, false);
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
