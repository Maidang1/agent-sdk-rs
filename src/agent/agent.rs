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
        if self.tools_enabled() {
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
            let tool_calls = self.process_tool_calls(&response.content).await?;

            if tool_calls.is_empty() {
                if matches!(self.options.tool_choice, ToolChoice::Required) {
                    let error_msg =
                        "ToolChoice::Required is set but model response contains no tool calls"
                            .to_string();
                    self.emit_event(AgentEvent::ConversationFailed {
                        error: error_msg.clone(),
                    });
                    return Err(AgentError::ParseError(error_msg));
                }

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
        if !self.tools_enabled() {
            self.conversation.clear();
            if let Some(system_prompt) = &self.options.system_prompt {
                self.conversation.push(Message::system(system_prompt));
            }
            self.conversation.push(Message::user(input));

            self.emit_event(AgentEvent::LlmRequestSent {
                messages: self.conversation.clone(),
            });

            return self
                .provider
                .generate_stream(
                    self.conversation.clone(),
                    Some(self.options.generate_options.clone()),
                )
                .await
                .map_err(Into::into);
        }

        // 工具模式仍走 run() 聚合后返回单 chunk
        let result = self.run(input).await?;

        let (tx, rx) = mpsc::channel(1);
        tokio::spawn(async move {
            let _ = tx.send(Ok(result)).await;
        });

        Ok(StreamResponse { receiver: rx })
    }

    async fn format_tools_description(&self) -> String {
        let tools = self.tools.list_tools().await;
        let target_tool = match &self.options.tool_choice {
            ToolChoice::Specific(name) => Some(name.as_str()),
            _ => None,
        };

        tools
            .iter()
            .filter(|tool| target_tool.map(|name| tool.name == name).unwrap_or(true))
            .map(|tool| format!("- {}: {}", tool.name, tool.description))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn tools_enabled(&self) -> bool {
        !matches!(self.options.tool_choice, ToolChoice::None)
    }

    async fn process_tool_calls(&self, content: &str) -> Result<Vec<crate::tool::ToolCall>> {
        if !self.tools_enabled() {
            return Ok(Vec::new());
        }

        let mut calls = ToolCallParser::extract_from_content(content);
        if let ToolChoice::Specific(expected_name) = &self.options.tool_choice {
            if calls.iter().any(|call| call.name != *expected_name) {
                return Err(AgentError::ParseError(format!(
                    "ToolChoice::Specific({}) only allows this tool to be called",
                    expected_name
                )));
            }

            calls.retain(|call| call.name == *expected_name);
        }

        Ok(calls)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{GenerateOptions, GenerateResponse, Usage};
    use std::future::Future;
    use std::pin::Pin;

    struct MockProvider {
        content: String,
    }

    impl LlmProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }

        fn model(&self) -> &str {
            "mock-model"
        }

        fn generate(
            &self,
            _messages: Vec<Message>,
            _options: Option<GenerateOptions>,
        ) -> Pin<Box<dyn Future<Output = crate::provider::Result<GenerateResponse>> + Send + '_>>
        {
            Box::pin(async move {
                Ok(GenerateResponse {
                    content: self.content.clone(),
                    usage: Some(Usage::default()),
                    model: self.model().to_string(),
                    finish_reason: Some("stop".to_string()),
                })
            })
        }

        fn generate_stream(
            &self,
            _messages: Vec<Message>,
            _options: Option<GenerateOptions>,
        ) -> Pin<Box<dyn Future<Output = crate::provider::Result<StreamResponse>> + Send + '_>>
        {
            Box::pin(async move {
                let (tx, rx) = mpsc::channel(2);
                let content = self.content.clone();
                tokio::spawn(async move {
                    let _ = tx.send(Ok(content)).await;
                });
                Ok(StreamResponse { receiver: rx })
            })
        }

        fn health_check(
            &self,
        ) -> Pin<Box<dyn Future<Output = crate::provider::Result<()>> + Send + '_>> {
            Box::pin(async { Ok(()) })
        }
    }

    #[tokio::test]
    async fn tool_choice_none_ignores_tool_call_payload() {
        let provider = MockProvider {
            content: r#"{"tool_calls":[{"name":"calculator","parameters":{"a":1,"b":2}}]}"#
                .to_string(),
        };

        let mut agent = Agent::new(provider).with_options(AgentOptions {
            tool_choice: ToolChoice::None,
            max_iterations: 1,
            ..Default::default()
        });

        let result = agent.run("hi").await.expect("run should succeed");
        assert!(result.contains("tool_calls"));
    }

    #[tokio::test]
    async fn tool_choice_required_returns_error_without_tool_call() {
        let provider = MockProvider {
            content: "plain answer without tool call".to_string(),
        };

        let mut agent = Agent::new(provider).with_options(AgentOptions {
            tool_choice: ToolChoice::Required,
            max_iterations: 1,
            ..Default::default()
        });

        let err = agent.run("hi").await.expect_err("should fail");
        assert!(err
            .to_string()
            .contains("ToolChoice::Required is set but model response contains no tool calls"));
    }

    #[tokio::test]
    async fn tool_choice_specific_rejects_other_tool_names() {
        let provider = MockProvider {
            content: r#"{"tool_calls":[{"name":"other_tool","parameters":{}}]}"#.to_string(),
        };

        let mut agent = Agent::new(provider).with_options(AgentOptions {
            tool_choice: ToolChoice::Specific("calculator".to_string()),
            max_iterations: 1,
            ..Default::default()
        });

        let err = agent.run("hi").await.expect_err("should fail");
        assert!(err
            .to_string()
            .contains("ToolChoice::Specific(calculator) only allows this tool to be called"));
    }

    #[tokio::test]
    async fn run_stream_uses_provider_stream_when_tools_disabled() {
        let provider = MockProvider {
            content: "streamed content".to_string(),
        };

        let mut agent = Agent::new(provider).with_options(AgentOptions {
            tool_choice: ToolChoice::None,
            ..Default::default()
        });

        let mut stream = agent.run_stream("hi").await.expect("stream should start");
        let chunk = stream
            .receiver
            .recv()
            .await
            .expect("should receive chunk")
            .expect("chunk should be ok");
        assert_eq!(chunk, "streamed content");
    }
}
