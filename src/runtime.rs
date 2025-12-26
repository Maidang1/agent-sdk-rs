use crate::hooks::{Hooks, NoopHooks};
use crate::llm::{FinishReason, LLMClient, LLMOptions, LLMResponse};
use crate::memory::Memory;
use crate::tool::ToolRegistry;
use crate::{Message, Result};
use std::sync::Arc;

pub struct Runtime<L: LLMClient> {
    llm: L,
    tools: ToolRegistry,
    memory: Memory,
    hooks: Arc<dyn Hooks>,
    options: RuntimeOptions,
}

#[derive(Clone)]
pub struct RuntimeOptions {
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub max_iterations: usize,
    pub system_prompt: Option<String>,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            model: "gpt-4".to_string(),
            max_tokens: None,
            temperature: None,
            max_iterations: 10,
            system_prompt: None,
        }
    }
}

impl<L: LLMClient> Runtime<L> {
    pub fn new(llm: L) -> Self {
        Self {
            llm,
            tools: ToolRegistry::new(),
            memory: Memory::new(),
            hooks: Arc::new(NoopHooks),
            options: RuntimeOptions::default(),
        }
    }

    pub fn with_options(mut self, options: RuntimeOptions) -> Self {
        self.options = options;
        self
    }


    pub fn with_hooks(mut self, hooks: impl Hooks + 'static) -> Self {
        self.hooks = Arc::new(hooks);
        self
    }

    pub fn with_memory(mut self, memory: Memory) -> Self {
        self.memory = memory;
        self
    }

    pub fn register_tool(&mut self, tool: Box<dyn crate::Tool>) {
        self.tools.register(tool);
    }

    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    pub fn memory_mut(&mut self) -> &mut Memory {
        &mut self.memory
    }

    pub async fn run(&mut self, input: impl Into<String>) -> Result<String> {
        // Add system prompt if not already present
        if self.memory.messages().is_empty() {
            if let Some(ref system_prompt) = self.options.system_prompt {
                self.memory.add(Message::system(system_prompt.clone()));
            }
        }

        // Add user message
        self.memory.add(Message::user(input.into()));

        let mut iterations = 0;

        loop {
            if iterations >= self.options.max_iterations {
                return Err(anyhow::anyhow!(
                    "Max iterations ({}) reached",
                    self.options.max_iterations
                ));
            }
            iterations += 1;

            let llm_options = LLMOptions {
                model: self.options.model.clone(),
                max_tokens: self.options.max_tokens,
                temperature: self.options.temperature,
                tools: self.tools.schemas(),
            };

            self.hooks.on_llm_start(self.memory.messages().len()).await;

            let response = self.llm.chat(self.memory.messages(), &llm_options).await?;

            self.hooks.on_llm_end(&response).await;

            match response.finish_reason {
                FinishReason::Stop | FinishReason::Length => {
                    let content = response.content.unwrap_or_default();
                    self.memory.add(Message::assistant(&content));
                    return Ok(content);
                }
                FinishReason::ToolCalls => {
                    self.handle_tool_calls(&response).await?;
                }
                FinishReason::Error => {
                    return Err(anyhow::anyhow!("LLM returned error"));
                }
            }
        }
    }


    async fn handle_tool_calls(&mut self, response: &LLMResponse) -> Result<()> {
        // Add assistant message with tool calls
        let mut assistant_msg = Message::assistant(response.content.clone().unwrap_or_default());
        assistant_msg.tool_calls = Some(response.tool_calls.clone());
        self.memory.add(assistant_msg);

        // Execute each tool call
        for tool_call in &response.tool_calls {
            self.hooks.on_tool_start(tool_call).await;

            let result = self.tools.execute(tool_call).await?;

            self.hooks.on_tool_end(tool_call, &result).await;

            // Add tool result message
            let content = if result.success {
                result.content
            } else {
                result.error.unwrap_or_else(|| "Unknown error".to_string())
            };

            self.memory.add(Message::tool(&tool_call.id, content));
        }

        Ok(())
    }
}
