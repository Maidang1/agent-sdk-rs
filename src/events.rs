use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub enum AgentEvent {
    ConversationStarted {
        input: String,
    },
    LlmRequestSent {
        messages: Vec<crate::provider::Message>,
    },
    LlmResponseReceived {
        content: String,
        model: String,
    },
    ToolCallsDetected {
        calls: Vec<crate::tool::ToolCall>,
    },
    ToolCallStarted {
        call: crate::tool::ToolCall,
    },
    ToolCallCompleted {
        call: crate::tool::ToolCall,
        result: crate::tool::ToolResult,
    },
    ToolCallFailed {
        call: crate::tool::ToolCall,
        error: String,
    },
    ConversationCompleted {
        response: String,
    },
    ConversationFailed {
        error: String,
    },
}

pub type EventHandler = Arc<dyn Fn(&AgentEvent) + Send + Sync>;

pub struct EventBus {
    sender: broadcast::Sender<AgentEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn emit(&self, event: AgentEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}
