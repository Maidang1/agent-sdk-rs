use crate::{ToolCall, ToolResult};
use async_trait::async_trait;

#[async_trait]
pub trait Hooks: Send + Sync {
    async fn on_tool_start(&self, _tool_call: &ToolCall) {}
    async fn on_tool_end(&self, _tool_call: &ToolCall, _result: &ToolResult) {}
    async fn on_error(&self, _error: &anyhow::Error) {}
    async fn on_llm_start(&self, _message_count: usize) {}
    async fn on_llm_end(&self, _response: &crate::llm::LLMResponse) {}
}

pub struct NoopHooks;

#[async_trait]
impl Hooks for NoopHooks {}

pub struct LoggingHooks;

#[async_trait]
impl Hooks for LoggingHooks {
    async fn on_tool_start(&self, tool_call: &ToolCall) {
        println!("[Hook] Tool start: {}", tool_call.name);
    }

    async fn on_tool_end(&self, tool_call: &ToolCall, result: &ToolResult) {
        println!(
            "[Hook] Tool end: {} success={}",
            tool_call.name, result.success
        );
    }

    async fn on_error(&self, error: &anyhow::Error) {
        eprintln!("[Hook] Error: {}", error);
    }

    async fn on_llm_start(&self, message_count: usize) {
        println!("[Hook] LLM start with {} messages", message_count);
    }

    async fn on_llm_end(&self, response: &crate::llm::LLMResponse) {
        println!("[Hook] LLM end: {:?}", response.finish_reason);
    }
}
