use super::{FinishReason, LLMClient, LLMOptions, LLMResponse};
use crate::{Message, MessageRole, Result, ToolCall};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct OpenAIClient {
    client: Client,
    api_key: String,
    base_url: String,
}

impl Clone for OpenAIClient {
    fn clone(&self) -> Self {
        Self {
            client: Client::new(),
            api_key: self.api_key.clone(),
            base_url: self.base_url.clone(),
        }
    }
}

impl OpenAIClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: "https://api.openai.com/v1".to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }


    fn convert_messages(&self, messages: &[Message]) -> Vec<Value> {
        messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::Tool => "tool",
                };

                let mut obj = json!({
                    "role": role,
                    "content": msg.content.clone(),
                });

                if let Some(ref tool_calls) = msg.tool_calls {
                    let calls: Vec<Value> = tool_calls
                        .iter()
                        .map(|tc| {
                            json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": tc.parameters.to_string()
                                }
                            })
                        })
                        .collect();
                    obj["tool_calls"] = json!(calls);
                }

                if let Some(ref tool_call_id) = msg.tool_call_id {
                    obj["tool_call_id"] = json!(tool_call_id);
                }

                obj
            })
            .collect()
    }

    fn convert_tools(&self, tools: &[super::ToolSchema]) -> Vec<Value> {
        tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
    finish_reason: String,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAIToolCall {
    id: String,
    function: FunctionCall,
}

#[derive(Debug, Deserialize)]
struct FunctionCall {
    name: String,
    arguments: String,
}


#[async_trait]
impl LLMClient for OpenAIClient {
    async fn chat(&self, messages: &[Message], options: &LLMOptions) -> Result<LLMResponse> {
        let mut body = json!({
            "model": options.model,
            "messages": self.convert_messages(messages),
        });

        if let Some(max_tokens) = options.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }

        if let Some(temperature) = options.temperature {
            body["temperature"] = json!(temperature);
        }

        if !options.tools.is_empty() {
            body["tools"] = json!(self.convert_tools(&options.tools));
        }

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("OpenAI API error: {}", error_text));
        }

        let openai_response: OpenAIResponse = response.json().await?;
        let choice = openai_response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No response from OpenAI"))?;

        let tool_calls = choice
            .message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tc| ToolCall {
                id: tc.id,
                name: tc.function.name,
                parameters: serde_json::from_str(&tc.function.arguments).unwrap_or(json!({})),
            })
            .collect::<Vec<_>>();

        let finish_reason = match choice.finish_reason.as_str() {
            "stop" => FinishReason::Stop,
            "tool_calls" => FinishReason::ToolCalls,
            "length" => FinishReason::Length,
            _ => FinishReason::Error,
        };

        Ok(LLMResponse {
            content: choice.message.content,
            tool_calls,
            finish_reason,
        })
    }
}
