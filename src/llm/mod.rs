use crate::{Message, Result, ToolCall};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod openai;

pub use openai::OpenAIClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: FinishReason,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    Error,
}

#[derive(Debug, Clone)]
pub struct LLMOptions {
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub tools: Vec<ToolSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn chat(&self, messages: &[Message], options: &LLMOptions) -> Result<LLMResponse>;
}
