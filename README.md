# Agent SDK

A Rust SDK for building event-driven AI agents. Inspired by [kode-agent-sdk](https://github.com/shareAI-lab/kode-agent-sdk).

## Features

- **Event-Driven Architecture**: Three-channel event system (Progress, Control, Monitor)
- **Multi-Agent Collaboration**: AgentPool, Room messaging, Safe Fork, Lineage tracking
- **Tool Approval Workflows**: Configurable policies, allowlist/blocklist, custom handlers
- **Scheduling & Reminders**: Iteration-based triggers, time-based triggers, custom conditions
- **Context Management**: Variables, metadata, Todo tracking
- **LLM Integration**: OpenAI-compatible API, extensible LLMClient trait

## Quick Start

```rust
use agent_sdk::{OpenAIClient, Runtime, RuntimeOptions, Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str { "calculator" }
    fn description(&self) -> &str { "Perform arithmetic" }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "a": { "type": "number" },
                "b": { "type": "number" },
                "op": { "type": "string" }
            }
        })
    }
    async fn execute(&self, params: &Value) -> agent_sdk::Result<ToolResult> {
        let a = params["a"].as_f64().unwrap_or(0.0);
        let b = params["b"].as_f64().unwrap_or(0.0);
        let result = match params["op"].as_str().unwrap_or("add") {
            "add" => a + b,
            "sub" => a - b,
            "mul" => a * b,
            "div" => a / b,
            _ => 0.0,
        };
        Ok(ToolResult::success(result.to_string()))
    }
}

#[tokio::main]
async fn main() -> agent_sdk::Result<()> {
    let llm = OpenAIClient::new(std::env::var("OPENAI_API_KEY")?);
    let mut runtime = Runtime::new(llm).with_options(RuntimeOptions {
        model: "gpt-4".to_string(),
        system_prompt: Some("You are a calculator assistant.".to_string()),
        ..Default::default()
    });
    runtime.register_tool(Box::new(CalculatorTool));
    
    let response = runtime.run("What is 42 * 17?").await?;
    println!("{}", response);
    Ok(())
}
```

## Event-Driven Architecture

Subscribe to agent events for real-time monitoring:

```rust
use agent_sdk::{EventBus, AgentEvent, ProgressEvent};
use std::sync::Arc;

let event_bus = Arc::new(EventBus::new(1024));
let mut receiver = event_bus.subscribe();

tokio::spawn(async move {
    while let Ok(event) = receiver.recv().await {
        match event {
            AgentEvent::Progress(ProgressEvent::ToolCalling { tool_call, .. }) => {
                println!("Calling: {}", tool_call.name);
            }
            AgentEvent::Monitor(monitor) => {
                println!("Monitor: {:?}", monitor);
            }
            _ => {}
        }
    }
});

let runtime = Runtime::new(llm).with_event_bus(event_bus);
```

## Tool Approval Workflow

Control tool execution with approval policies:

```rust
use agent_sdk::{ApprovalManager, ApprovalPolicy};

let approval = Arc::new(ApprovalManager::new());

// Auto-approve safe tools
approval.set_tool_policy("read_file", ApprovalPolicy::AutoApprove).await;

// Block dangerous tools
approval.set_tool_policy("delete_file", ApprovalPolicy::AutoReject("Blocked".into())).await;

// Require manual approval for others
approval.set_policy(ApprovalPolicy::RequireApproval).await;

let runtime = Runtime::new(llm)
    .with_options(RuntimeOptions { require_tool_approval: true, ..Default::default() })
    .with_approval_manager(approval);
```

## Multi-Agent Collaboration

Create agent pools and collaboration rooms:

```rust
use agent_sdk::{AgentPool, Room, EventBus};

let event_bus = Arc::new(EventBus::new(1024));
let pool: AgentPool<OpenAIClient> = AgentPool::new(event_bus.clone());

// Create agents
pool.create_agent("researcher", "Researcher", llm1, None).await?;
pool.create_agent("writer", "Writer", llm2, None).await?;

// Fork an agent (inherits context)
pool.fork_agent("researcher", "researcher_v2", llm3).await?;

// Create collaboration room
let room = Room::new("project", "Project Room", event_bus);
room.join("researcher").await;
room.join("writer").await;
room.send("researcher", "Found relevant data").await;
```

## Scheduling

Set up reminders and triggers:

```rust
use agent_sdk::{Scheduler, Trigger, ScheduledTask, ScheduledAction};
use std::time::Duration;

let scheduler = Scheduler::new(event_bus);

// Remind after N iterations
scheduler.remind_after_iterations("check", 5, "Check progress").await;

// Remind at interval
scheduler.remind_at_interval("timer", Duration::from_secs(60), "One minute passed").await;

// Custom trigger
scheduler.add_task(ScheduledTask {
    id: "auto_pause".into(),
    trigger: Trigger::AfterIterations(10),
    action: ScheduledAction::Pause,
    repeat: false,
    last_triggered: None,
}).await;
```

## Examples

```bash
# Basic usage
cargo run --example basic

# Event-driven with EventBus
cargo run --example event_driven

# Multi-agent collaboration
cargo run --example multi_agent

# Tool approval workflow
cargo run --example approval_workflow

# Scheduler and reminders
cargo run --example scheduler
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        EventBus                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                   │
│  │ Progress │  │ Control  │  │ Monitor  │                   │
│  │ Channel  │  │ Channel  │  │ Channel  │                   │
│  └──────────┘  └──────────┘  └──────────┘                   │
└─────────────────────────────────────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        ▼                   ▼                   ▼
┌───────────────┐   ┌───────────────┐   ┌───────────────┐
│   AgentPool   │   │   Scheduler   │   │   Approval    │
│               │   │               │   │   Manager     │
└───────────────┘   └───────────────┘   └───────────────┘
        │
        ▼
┌───────────────────────────────────────────────────────────┐
│                        Runtime                             │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐      │
│  │ Memory  │  │ Context │  │  Tools  │  │  Hooks  │      │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘      │
└───────────────────────────────────────────────────────────┘
        │
        ▼
┌───────────────┐
│   LLMClient   │
│   (OpenAI)    │
└───────────────┘
```

## License

MIT OR Apache-2.0
