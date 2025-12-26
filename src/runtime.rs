use crate::approval::{ApprovalDecision, ApprovalManager};
use crate::context::ContextManager;
use crate::event::{AgentEvent, EventBus, MonitorEvent, ProgressEvent};
use crate::hooks::{Hooks, NoopHooks};
use crate::llm::{FinishReason, LLMClient, LLMOptions, LLMResponse};
use crate::memory::Memory;
use crate::scheduler::Scheduler;
use crate::tool::ToolRegistry;
use crate::{Message, Result};
use std::sync::Arc;
use std::time::Instant;

/// Agent runtime state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    Idle,
    Running,
    Paused,
    Completed,
    Error,
}

pub struct Runtime<L: LLMClient> {
    id: String,
    llm: L,
    tools: ToolRegistry,
    memory: Memory,
    hooks: Arc<dyn Hooks>,
    options: RuntimeOptions,
    state: RuntimeState,
    event_bus: Option<Arc<EventBus>>,
    context: ContextManager,
    approval_manager: Arc<ApprovalManager>,
    scheduler: Option<Arc<Scheduler>>,
}

#[derive(Clone)]
pub struct RuntimeOptions {
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub max_iterations: usize,
    pub system_prompt: Option<String>,
    pub require_tool_approval: bool,
}

impl Default for RuntimeOptions {
    fn default() -> Self {
        Self {
            model: "gpt-4".to_string(),
            max_tokens: None,
            temperature: None,
            max_iterations: 10,
            system_prompt: None,
            require_tool_approval: false,
        }
    }
}

impl<L: LLMClient> Runtime<L> {
    pub fn new(llm: L) -> Self {
        Self {
            id: format!("agent_{}", uuid_simple()),
            llm,
            tools: ToolRegistry::new(),
            memory: Memory::new(),
            hooks: Arc::new(NoopHooks),
            options: RuntimeOptions::default(),
            state: RuntimeState::Idle,
            event_bus: None,
            context: ContextManager::new(),
            approval_manager: Arc::new(ApprovalManager::new()),
            scheduler: None,
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
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

    pub fn with_event_bus(mut self, event_bus: Arc<EventBus>) -> Self {
        self.event_bus = Some(event_bus.clone());
        self.scheduler = Some(Arc::new(Scheduler::new(event_bus)));
        self
    }

    pub fn with_approval_manager(mut self, manager: Arc<ApprovalManager>) -> Self {
        self.approval_manager = manager;
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

    pub fn context(&self) -> &ContextManager {
        &self.context
    }

    pub fn state(&self) -> RuntimeState {
        self.state
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn approval_manager(&self) -> &Arc<ApprovalManager> {
        &self.approval_manager
    }

    fn emit(&self, event: AgentEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.publish(event);
        }
    }

    pub async fn run(&mut self, input: impl Into<String>) -> Result<String> {
        let start_time = Instant::now();
        self.state = RuntimeState::Running;

        // Emit start event
        self.emit(AgentEvent::Progress(ProgressEvent::Started {
            agent_id: self.id.clone(),
            session_id: format!("session_{}", uuid_simple()),
        }));

        // Add system prompt if not already present
        if self.memory.messages().is_empty() {
            if let Some(ref system_prompt) = self.options.system_prompt {
                self.memory.add(Message::system(system_prompt.clone()));
            }
        }

        // Add user message
        let user_input = input.into();
        self.memory.add(Message::user(&user_input));

        let mut iterations = 0;

        loop {
            if self.state == RuntimeState::Paused {
                // Wait for resume (in real impl, use condition variable)
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                continue;
            }

            if iterations >= self.options.max_iterations {
                self.state = RuntimeState::Error;
                self.emit(AgentEvent::Progress(ProgressEvent::Error {
                    agent_id: self.id.clone(),
                    error: format!("Max iterations ({}) reached", self.options.max_iterations),
                }));
                return Err(anyhow::anyhow!(
                    "Max iterations ({}) reached",
                    self.options.max_iterations
                ));
            }
            iterations += 1;

            // Update scheduler
            if let Some(ref scheduler) = self.scheduler {
                scheduler.tick_iteration().await;
                scheduler.update_elapsed(start_time.elapsed()).await;
                scheduler.check_triggers(&self.id).await;
            }

            // Emit iteration count
            self.emit(AgentEvent::Monitor(MonitorEvent::IterationCount {
                agent_id: self.id.clone(),
                count: iterations,
            }));

            let llm_options = LLMOptions {
                model: self.options.model.clone(),
                max_tokens: self.options.max_tokens,
                temperature: self.options.temperature,
                tools: self.tools.schemas(),
            };

            self.hooks.on_llm_start(self.memory.messages().len()).await;

            let llm_start = Instant::now();
            let response = self.llm.chat(self.memory.messages(), &llm_options).await?;
            let llm_duration = llm_start.elapsed();

            self.hooks.on_llm_end(&response).await;

            // Emit LLM latency
            self.emit(AgentEvent::Monitor(MonitorEvent::LLMLatency {
                agent_id: self.id.clone(),
                duration_ms: llm_duration.as_millis() as u64,
            }));

            // Emit thinking event
            if let Some(ref content) = response.content {
                self.emit(AgentEvent::Progress(ProgressEvent::Thinking {
                    agent_id: self.id.clone(),
                    content: content.clone(),
                }));
            }

            match response.finish_reason {
                FinishReason::Stop | FinishReason::Length => {
                    let content = response.content.unwrap_or_default();
                    self.memory.add(Message::assistant(&content));
                    self.state = RuntimeState::Completed;

                    self.emit(AgentEvent::Progress(ProgressEvent::Completed {
                        agent_id: self.id.clone(),
                        result: content.clone(),
                    }));

                    return Ok(content);
                }
                FinishReason::ToolCalls => {
                    self.handle_tool_calls(&response).await?;
                }
                FinishReason::Error => {
                    self.state = RuntimeState::Error;
                    self.emit(AgentEvent::Progress(ProgressEvent::Error {
                        agent_id: self.id.clone(),
                        error: "LLM returned error".to_string(),
                    }));
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
            // Emit tool calling event
            self.emit(AgentEvent::Progress(ProgressEvent::ToolCalling {
                agent_id: self.id.clone(),
                tool_call: tool_call.clone(),
            }));

            // Check approval if required
            if self.options.require_tool_approval {
                let decision = self.approval_manager.check(tool_call).await;
                match decision {
                    ApprovalDecision::Rejected(reason) => {
                        self.memory.add(Message::tool(
                            &tool_call.id,
                            format!("Tool execution rejected: {}", reason),
                        ));
                        continue;
                    }
                    ApprovalDecision::Pending => {
                        // Wait for approval
                        let decision = self
                            .approval_manager
                            .request_approval(tool_call.clone())
                            .await?;
                        if let ApprovalDecision::Rejected(reason) = decision {
                            self.memory.add(Message::tool(
                                &tool_call.id,
                                format!("Tool execution rejected: {}", reason),
                            ));
                            continue;
                        }
                    }
                    ApprovalDecision::Approved => {}
                }
            }

            self.hooks.on_tool_start(tool_call).await;

            let tool_start = Instant::now();
            let result = self.tools.execute(tool_call).await?;
            let tool_duration = tool_start.elapsed();

            self.hooks.on_tool_end(tool_call, &result).await;

            // Emit tool execution time
            self.emit(AgentEvent::Monitor(MonitorEvent::ToolExecutionTime {
                agent_id: self.id.clone(),
                tool_name: tool_call.name.clone(),
                duration_ms: tool_duration.as_millis() as u64,
            }));

            // Emit tool result event
            self.emit(AgentEvent::Progress(ProgressEvent::ToolResult {
                agent_id: self.id.clone(),
                tool_call_id: tool_call.id.clone(),
                result: result.clone(),
            }));

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

    /// Pause the runtime
    pub fn pause(&mut self) {
        if self.state == RuntimeState::Running {
            self.state = RuntimeState::Paused;
            self.emit(AgentEvent::Progress(ProgressEvent::Message {
                agent_id: self.id.clone(),
                message: Message::system("Agent paused"),
            }));
        }
    }

    /// Resume the runtime
    pub fn resume(&mut self) {
        if self.state == RuntimeState::Paused {
            self.state = RuntimeState::Running;
            self.emit(AgentEvent::Progress(ProgressEvent::Message {
                agent_id: self.id.clone(),
                message: Message::system("Agent resumed"),
            }));
        }
    }

    /// Cancel the runtime
    pub fn cancel(&mut self) {
        self.state = RuntimeState::Error;
        self.emit(AgentEvent::Progress(ProgressEvent::Error {
            agent_id: self.id.clone(),
            error: "Cancelled by user".to_string(),
        }));
    }
}

/// Simple UUID generator
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    format!("{:x}{:x}", duration.as_secs(), duration.subsec_nanos())
}
