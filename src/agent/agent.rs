use super::options::{AgentOptions, ToolChoice};
use crate::error::{AgentError, Result};
use crate::events::{AgentEvent, EventBus};
use crate::provider::{LlmProvider, Message, StreamResponse};
use crate::tool::{Tool, ToolCallParser, ToolExecutor, ToolRegistry, ToolResult};
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct Agent<P: LlmProvider> {
    provider: P,
    tools: ToolRegistry,
    executor: ToolExecutor,
    conversation: Vec<Message>,
    options: AgentOptions,
    event_bus: Option<Arc<EventBus>>,
}

impl<P: LlmProvider> Agent<P> {
    pub fn new(provider: P) -> Self {
        let tools = ToolRegistry::new();
        let executor = ToolExecutor::new(tools.clone());

        Self {
            provider,
            tools,
            executor,
            conversation: Vec::new(),
            options: AgentOptions::default(),
            event_bus: None,
        }
    }

    pub fn with_options(mut self, options: AgentOptions) -> Self {
        self.options = options;
        self
    }

    pub fn with_event_bus(mut self, event_bus: Arc<EventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    pub async fn register_tool(&mut self, tool: Box<dyn Tool>) {
        self.tools.register(tool).await;
    }

    fn emit_event(&self, event: AgentEvent) {
        if let Some(bus) = &self.event_bus {
            bus.emit(event);
        }
    }

    pub async fn run(&mut self, input: &str) -> Result<String> {
        self.emit_event(AgentEvent::ConversationStarted {
            input: input.to_string(),
        });

        self.conversation.clear();

        // 添加系统提示
        if let Some(system_prompt) = &self.options.system_prompt {
            self.conversation.push(Message::system(system_prompt));
        }

        // 添加工具描述
        if matches!(
            self.options.tool_choice,
            ToolChoice::Auto | ToolChoice::Required
        ) {
            let tools_desc = self.format_tools_description().await;
            if !tools_desc.is_empty() {
                let tool_prompt = format!(
                    "You have access to the following tools:\n{}\n\nTo use a tool, respond with JSON in this format:\n{{\n  \"tool_calls\": [\n    {{\n      \"id\": \"call_1\",\n      \"name\": \"tool_name\",\n      \"parameters\": {{\n        \"param1\": \"value1\"\n      }}\n    }}\n  ]\n}}",
                    tools_desc
                );
                self.conversation.push(Message::system(tool_prompt));
            }
        }

        // 添加用户输入
        self.conversation.push(Message::user(input));

        // 执行对话循环
        for _ in 0..self.options.max_iterations {
            self.emit_event(AgentEvent::LlmRequestSent {
                messages: self.conversation.clone(),
            });

            let response = match self
                .provider
                .generate(
                    self.conversation.clone(),
                    Some(self.options.generate_options.clone()),
                )
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    let error_msg = format!("LLM request failed: {}", e);
                    self.emit_event(AgentEvent::ConversationFailed {
                        error: error_msg.clone(),
                    });
                    return Err(e.into());
                }
            };

            self.emit_event(AgentEvent::LlmResponseReceived {
                content: response.content.clone(),
                model: response.model.clone(),
            });

            self.conversation
                .push(Message::assistant(&response.content));

            // 检查是否有工具调用
            let tool_calls = ToolCallParser::extract_from_content(&response.content);

            if tool_calls.is_empty() {
                self.emit_event(AgentEvent::ConversationCompleted {
                    response: response.content.clone(),
                });
                return Ok(response.content);
            }

            self.emit_event(AgentEvent::ToolCallsDetected {
                calls: tool_calls.clone(),
            });

            // 执行工具调用
            let mut results = Vec::new();
            for call in tool_calls {
                self.emit_event(AgentEvent::ToolCallStarted { call: call.clone() });

                let result = self.executor.execute_single(&call).await;

                if result.success {
                    self.emit_event(AgentEvent::ToolCallCompleted {
                        call: call.clone(),
                        result: result.clone(),
                    });
                } else {
                    self.emit_event(AgentEvent::ToolCallFailed {
                        call: call.clone(),
                        error: result.error.clone().unwrap_or_default(),
                    });
                }

                results.push(result);
            }

            let results_text = self.format_tool_results(&results);
            self.conversation
                .push(Message::user(&format!("Tool results:\n{}", results_text)));
        }

        let error_msg = "Max iterations reached".to_string();
        self.emit_event(AgentEvent::ConversationFailed {
            error: error_msg.clone(),
        });
        Err(AgentError::ParseError(error_msg))
    }

    pub async fn run_stream(&mut self, input: &str) -> Result<StreamResponse> {
        // 简化版流式实现
        let result = self.run(input).await?;

        let (tx, rx) = mpsc::channel(1);
        tokio::spawn(async move {
            let _ = tx.send(Ok(result)).await;
        });

        Ok(StreamResponse { receiver: rx })
    }

    async fn format_tools_description(&self) -> String {
        let tools = self.tools.list_tools().await;
        tools
            .iter()
            .map(|tool| format!("- {}: {}", tool.name, tool.description))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn format_tool_results(&self, results: &[ToolResult]) -> String {
        results
            .iter()
            .enumerate()
            .map(|(i, result)| {
                if result.success {
                    format!("Result {}: {}", i + 1, result.content)
                } else {
                    format!(
                        "Error {}: {}",
                        i + 1,
                        result
                            .error
                            .as_ref()
                            .unwrap_or(&"Unknown error".to_string())
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
