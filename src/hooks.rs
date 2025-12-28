use std::sync::Arc;
use crate::events::{AgentEvent, EventBus};

pub type HookFn = Arc<dyn Fn(&AgentEvent) -> bool + Send + Sync>;

pub struct HookManager {
    event_bus: Arc<EventBus>,
    hooks: Vec<HookFn>,
}

impl HookManager {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            event_bus,
            hooks: Vec::new(),
        }
    }

    pub fn add_hook<F>(&mut self, hook: F) 
    where
        F: Fn(&AgentEvent) -> bool + Send + Sync + 'static,
    {
        self.hooks.push(Arc::new(hook));
    }

    pub async fn start_monitoring(&self) {
        let mut receiver = self.event_bus.subscribe();
        let hooks = self.hooks.clone();
        
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                for hook in &hooks {
                    if !hook(&event) {
                        // Hook 返回 false 表示停止处理
                        break;
                    }
                }
            }
        });
    }

    // 预定义的 Hook 函数
    pub fn logging_hook() -> HookFn {
        Arc::new(|event| {
            match event {
                AgentEvent::ConversationStarted { input } => {
                    println!("[LOG] Conversation started: {}", input);
                }
                AgentEvent::ToolCallStarted { call } => {
                    println!("[LOG] Tool call started: {}", call.name);
                }
                AgentEvent::ToolCallCompleted { call, result } => {
                    println!("[LOG] Tool call completed: {} -> success: {}", call.name, result.success);
                }
                AgentEvent::ConversationCompleted { .. } => {
                    println!("[LOG] Conversation completed successfully");
                }
                AgentEvent::ConversationFailed { error } => {
                    println!("[LOG] Conversation failed: {}", error);
                }
                _ => {}
            }
            true // 继续处理其他 hooks
        })
    }

    pub fn metrics_hook() -> HookFn {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::Mutex;
        use std::time::Instant;
        
        static TOOL_CALLS: AtomicU64 = AtomicU64::new(0);
        static CONVERSATIONS: AtomicU64 = AtomicU64::new(0);
        static START_TIME: Mutex<Option<Instant>> = Mutex::new(None);
        
        Arc::new(|event| {
            match event {
                AgentEvent::ConversationStarted { .. } => {
                    CONVERSATIONS.fetch_add(1, Ordering::Relaxed);
                    let mut start = START_TIME.lock().unwrap();
                    if start.is_none() {
                        *start = Some(Instant::now());
                    }
                }
                AgentEvent::ToolCallStarted { .. } => {
                    TOOL_CALLS.fetch_add(1, Ordering::Relaxed);
                }
                AgentEvent::ConversationCompleted { .. } | AgentEvent::ConversationFailed { .. } => {
                    let conversations = CONVERSATIONS.load(Ordering::Relaxed);
                    let tool_calls = TOOL_CALLS.load(Ordering::Relaxed);
                    println!("[METRICS] Total conversations: {}, Total tool calls: {}", conversations, tool_calls);
                }
                _ => {}
            }
            true
        })
    }

    pub fn error_tracking_hook() -> HookFn {
        Arc::new(|event| {
            match event {
                AgentEvent::ToolCallFailed { call, error } => {
                    eprintln!("[ERROR] Tool '{}' failed: {}", call.name, error);
                    // 这里可以添加错误报告逻辑
                }
                AgentEvent::ConversationFailed { error } => {
                    eprintln!("[ERROR] Conversation failed: {}", error);
                }
                _ => {}
            }
            true
        })
    }
}
