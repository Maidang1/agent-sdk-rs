//! Event-driven agent example with EventBus subscription
//!
//! Run with: cargo run --example event_driven

use agent_sdk::{
    AgentEvent, EventBus, OpenAIClient, ProgressEvent, Result, Runtime, RuntimeOptions, Tool,
    ToolResult,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

struct WeatherTool;

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &str {
        "get_weather"
    }

    fn description(&self) -> &str {
        "Get current weather for a location"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "City name"
                }
            },
            "required": ["location"]
        })
    }

    async fn execute(&self, parameters: &Value) -> Result<ToolResult> {
        let location = parameters["location"].as_str().unwrap_or("Unknown");
        Ok(ToolResult::success(format!(
            "Weather in {}: Sunny, 22°C",
            location
        )))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

    // Create event bus
    let event_bus = Arc::new(EventBus::new(1024));

    // Subscribe to events
    let mut receiver = event_bus.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = receiver.recv().await {
            match event {
                AgentEvent::Progress(ProgressEvent::Started { agent_id, .. }) => {
                    println!("🚀 Agent {} started", agent_id);
                }
                AgentEvent::Progress(ProgressEvent::Thinking { content, .. }) => {
                    println!("💭 Thinking: {}...", &content[..content.len().min(50)]);
                }
                AgentEvent::Progress(ProgressEvent::ToolCalling { tool_call, .. }) => {
                    println!("🔧 Calling tool: {}", tool_call.name);
                }
                AgentEvent::Progress(ProgressEvent::ToolResult { result, .. }) => {
                    println!("✅ Tool result: {}", result.content);
                }
                AgentEvent::Progress(ProgressEvent::Completed { result, .. }) => {
                    println!("🎉 Completed: {}", result);
                }
                AgentEvent::Progress(ProgressEvent::Error { error, .. }) => {
                    println!("❌ Error: {}", error);
                }
                AgentEvent::Monitor(monitor) => {
                    println!("📊 Monitor: {:?}", monitor);
                }
                _ => {}
            }
        }
    });

    // Create runtime with event bus
    let llm = OpenAIClient::new(api_key);
    let options = RuntimeOptions {
        model: "gpt-4".to_string(),
        system_prompt: Some("You are a helpful weather assistant.".to_string()),
        ..Default::default()
    };

    let mut runtime = Runtime::new(llm)
        .with_options(options)
        .with_event_bus(event_bus);

    runtime.register_tool(Box::new(WeatherTool));

    // Run the agent
    let response = runtime.run("What's the weather in Tokyo?").await?;
    println!("\nFinal response: {}", response);

    Ok(())
}
