//! # Agent SDK
//!
//! A Rust SDK for building event-driven AI agents with support for:
//! - Multi-agent collaboration (AgentPool, Room)
//! - Event-driven architecture (EventBus, three-channel events)
//! - Tool approval workflows
//! - Scheduling and reminders
//! - Context management
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use agent_sdk::{OpenAIClient, Runtime, RuntimeOptions};
//!
//! let llm = OpenAIClient::new("your-api-key");
//! let options = RuntimeOptions {
//!     model: "gpt-4".to_string(),
//!     system_prompt: Some("You are a helpful assistant.".to_string()),
//!     ..Default::default()
//! };
//!
//! let mut runtime = Runtime::new(llm).with_options(options);
//! let response = runtime.run("Hello!").await?;
//! ```

pub mod agent;
pub mod agent_pool;
pub mod approval;
pub mod context;
pub mod event;
pub mod hooks;
pub mod llm;
pub mod memory;
pub mod message;
pub mod room;
pub mod runtime;
pub mod scheduler;
pub mod tool;

// Re-exports
pub use agent::AgentConfig;
pub use agent_pool::{AgentInfo, AgentPool, AgentState};
pub use approval::{ApprovalDecision, ApprovalHandler, ApprovalManager, ApprovalPolicy};
pub use context::{ContextManager, Priority, Todo, TodoStatus};
pub use event::{AgentEvent, ControlEvent, EventBus, MonitorEvent, ProgressEvent};
pub use hooks::{Hooks, LoggingHooks, NoopHooks};
pub use llm::{LLMClient, LLMOptions, LLMResponse, OpenAIClient, ToolSchema};
pub use memory::Memory;
pub use message::{Message, MessageRole};
pub use room::{Room, RoomManager, RoomMessage};
pub use runtime::{Runtime, RuntimeOptions, RuntimeState};
pub use scheduler::{ScheduledAction, ScheduledTask, Scheduler, SchedulerContext, Trigger};
pub use tool::{Tool, ToolCall, ToolRegistry, ToolResult};

pub type Result<T> = std::result::Result<T, anyhow::Error>;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{
        AgentConfig, AgentEvent, AgentInfo, AgentPool, AgentState, ApprovalDecision,
        ApprovalManager, ApprovalPolicy, ContextManager, ControlEvent, EventBus, Hooks,
        LLMClient, LLMOptions, LLMResponse, LoggingHooks, Memory, Message, MessageRole,
        MonitorEvent, NoopHooks, OpenAIClient, Priority, ProgressEvent, Result, Room,
        RoomManager, Runtime, RuntimeOptions, RuntimeState, Scheduler, Tool, ToolCall,
        ToolRegistry, ToolResult, ToolSchema,
    };
}
