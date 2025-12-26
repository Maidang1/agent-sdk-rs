use crate::{Message, ToolCall, ToolResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Event types for the agent runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AgentEvent {
    // Progress channel events
    Progress(ProgressEvent),
    // Control channel events  
    Control(ControlEvent),
    // Monitor channel events
    Monitor(MonitorEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProgressEvent {
    Started { agent_id: String, session_id: String },
    Thinking { agent_id: String, content: String },
    ToolCalling { agent_id: String, tool_call: ToolCall },
    ToolResult { agent_id: String, tool_call_id: String, result: ToolResult },
    Message { agent_id: String, message: Message },
    Completed { agent_id: String, result: String },
    Error { agent_id: String, error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlEvent {
    Pause { agent_id: String },
    Resume { agent_id: String },
    Cancel { agent_id: String },
    Interrupt { agent_id: String, message: String },
    ToolApprovalRequired { agent_id: String, tool_call: ToolCall },
    ToolApproved { agent_id: String, tool_call_id: String },
    ToolRejected { agent_id: String, tool_call_id: String, reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MonitorEvent {
    TokenUsage { agent_id: String, input_tokens: u32, output_tokens: u32 },
    IterationCount { agent_id: String, count: usize },
    ToolExecutionTime { agent_id: String, tool_name: String, duration_ms: u64 },
    LLMLatency { agent_id: String, duration_ms: u64 },
    StateSnapshot { agent_id: String, state: serde_json::Value },
}

/// Event subscriber callback type
pub type EventCallback = Arc<dyn Fn(AgentEvent) + Send + Sync>;

/// Event bus for pub/sub communication
pub struct EventBus {
    sender: broadcast::Sender<AgentEvent>,
    callbacks: RwLock<HashMap<String, EventCallback>>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            callbacks: RwLock::new(HashMap::new()),
        }
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: AgentEvent) {
        let _ = self.sender.send(event);
    }

    /// Subscribe to events with a broadcast receiver
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }

    /// Register a named callback for events
    pub async fn on(&self, name: impl Into<String>, callback: EventCallback) {
        let mut callbacks = self.callbacks.write().await;
        callbacks.insert(name.into(), callback);
    }

    /// Remove a named callback
    pub async fn off(&self, name: &str) {
        let mut callbacks = self.callbacks.write().await;
        callbacks.remove(name);
    }

    /// Emit event to all registered callbacks
    pub async fn emit(&self, event: AgentEvent) {
        self.publish(event.clone());
        let callbacks = self.callbacks.read().await;
        for callback in callbacks.values() {
            callback(event.clone());
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            callbacks: RwLock::new(HashMap::new()),
        }
    }
}
