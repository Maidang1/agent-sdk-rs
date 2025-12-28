use super::options::{AgentOptions, ToolChoice};
use crate::error::{AgentError, Result};
use crate::provider::{LlmProvider, Message, StreamResponse};
use crate::tool::{Tool, ToolCallParser, ToolExecutor, ToolRegistry, ToolResult};
use tokio::sync::mpsc;

pub struct Agent<P: LlmProvider> {
    provider: P,
    tools: ToolRegistry,
    executor: ToolExecutor,
    conversation: Vec<Message>,
    options: AgentOptions,
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
        }
    }

    pub fn with_options(mut self, options: AgentOptions) -> Self {
        self.options = options;
        self
    }

    pub async fn register_tool(&mut self, tool: Box<dyn Tool>) {
        self.tools.register(tool).await;
    }

    pub async fn run(&mut self, input: &str) -> Result<String> {
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
            let response = self
                .provider
                .generate(
                    self.conversation.clone(),
                    Some(self.options.generate_options.clone()),
                )
                .await?;

            self.conversation
                .push(Message::assistant(&response.content));

            // 检查是否有工具调用
            let tool_calls = ToolCallParser::extract_from_content(&response.content);

            if tool_calls.is_empty() {
                return Ok(response.content);
            }

            // 执行工具调用
            let results = self.executor.execute_calls(tool_calls).await;
            let results_text = self.format_tool_results(&results);

            self.conversation
                .push(Message::user(&format!("Tool results:\n{}", results_text)));
        }

        Err(AgentError::ParseError("Max iterations reached".into()))
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
