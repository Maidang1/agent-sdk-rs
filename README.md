# Agent SDK

A Rust SDK for building AI agents with tool support.

## Features

- Async agent execution
- Tool system with trait-based interface
- Message handling and conversation management
- Configurable agent settings

## Quick Start

```rust
use agent_sdk::{Agent, AgentConfig, Tool, ToolResult};

// Create agent configuration
let config = AgentConfig {
    name: "My Agent".to_string(),
    model: "gpt-4".to_string(),
    system_prompt: Some("You are helpful.".to_string()),
    tools: vec!["echo".to_string()],
    max_tokens: Some(1000),
    temperature: Some(0.7),
};

// Initialize agent
let mut agent = Agent::new(config);
```

## Examples

Run the basic example:

```bash
cargo run --example basic
```

## License

MIT OR Apache-2.0
