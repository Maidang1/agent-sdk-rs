# Agent SDK

A Rust SDK for building AI agents with tool calling capabilities.

## Features

- **LLM Provider Abstraction**: Unified interface for different LLM providers
- **Tool System**: Extensible tool registration and execution
- **Agent Runtime**: Core agent with conversation management
- **Tool Call Parsing**: Support for JSON and XML tool call formats
- **Async/Await**: Full async support with tokio

## Quick Start

```rust
use agent_sdk::{Agent, OpenRouterProvider, Tool, ToolResult, AgentOptions, ToolChoice};
use async_trait::async_trait;
use serde_json::{json, Value};

struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str { "calculator" }
    fn description(&self) -> &str { "Perform arithmetic operations" }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number"},
                "b": {"type": "number"},
                "operation": {"type": "string", "enum": ["add", "sub", "mul", "div"]}
            },
            "required": ["a", "b", "operation"]
        })
    }
    
    async fn execute(&self, params: &Value) -> ToolResult {
        let a = params["a"].as_f64().unwrap_or(0.0);
        let b = params["b"].as_f64().unwrap_or(0.0);
        let op = params["operation"].as_str().unwrap_or("add");
        
        let result = match op {
            "add" => a + b,
            "sub" => a - b,
            "mul" => a * b,
            "div" => if b != 0.0 { a / b } else { return ToolResult::error("Division by zero") },
            _ => return ToolResult::error("Unknown operation"),
        };
        
        ToolResult::success(result.to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = OpenRouterProvider::new(
        std::env::var("OPEN_ROUTER_API_KEY")?,
        "google/gemini-2.5-flash-lite-preview-09-2025"
    );
    
    let mut agent = Agent::new(provider).with_options(AgentOptions {
        system_prompt: Some("You are a helpful assistant with access to tools.".into()),
        tool_choice: ToolChoice::Auto,
        ..Default::default()
    });
    
    agent.register_tool(Box::new(CalculatorTool)).await;
    
    let response = agent.run("What is 42 * 17?").await?;
    println!("{}", response);
    Ok(())
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Agent                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │ Conversation│  │ Tool Registry│  │ Tool Executor│          │
│  │ Management  │  │             │  │             │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                    LLM Provider                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │ OpenRouter  │  │   OpenAI    │  │   Custom    │          │
│  │ Provider    │  │  Provider   │  │  Provider   │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
└─────────────────────────────────────────────────────────────┘
```

## Examples

```bash
# Basic calculator tool
cargo run --example calculator

# Multiple tools working together
cargo run --example multi_tool

# Test tool call parsing
cargo run --example test_parser
```

## Tool Call Format

The agent supports JSON format for tool calls:

```json
{
  "tool_calls": [
    {
      "id": "call_1",
      "name": "calculator",
      "parameters": {
        "a": 10,
        "b": 5,
        "operation": "add"
      }
    }
  ]
}
```

## Current Implementation

### Core Components
- **LlmProvider**: Trait for LLM integration (OpenRouter implemented)
- **Tool**: Trait for tool implementation with async execution
- **Agent**: Main runtime with conversation management
- **ToolRegistry**: Thread-safe tool storage and management
- **ToolCallParser**: Extracts tool calls from LLM responses

### Supported Features
- ✅ Basic tool calling with JSON format
- ✅ Multiple tool registration
- ✅ Async tool execution
- ✅ Error handling and recovery
- ✅ Conversation context management
- ✅ OpenRouter provider integration

### Planned Features
- 🔄 Event-driven architecture
- 🔄 Tool approval workflows
- 🔄 Multi-agent collaboration
- 🔄 Scheduling and reminders
- 🔄 Additional LLM providers

## License

MIT OR Apache-2.0
