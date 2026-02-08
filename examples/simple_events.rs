use agent_sdk::{Agent, OpenRouterProvider, Tool, ToolResult, AgentOptions, ToolChoice, EventBus, AgentEvent};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::{env, sync::Arc};

struct SimpleCalculatorTool;

#[async_trait]
impl Tool for SimpleCalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Perform simple arithmetic operations"
    }

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
        let a = params["a"].as_f64().unwrap();
        let b = params["b"].as_f64().unwrap();
        let operation = params["operation"].as_str().unwrap();

        let result = match operation {
            "add" => a + b,
            "sub" => a - b,
            "mul" => a * b,
            "div" => {
                if b != 0.0 {
                    a / b
                } else {
                    return ToolResult::error("Division by zero");
                }
            }
            _ => unreachable!(),
        };

        ToolResult::success(result.to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("OPEN_ROUTER_API_KEY")
        .expect("Please set the OPEN_ROUTER_API_KEY environment variable");

    // 创建事件总线
    let event_bus = Arc::new(EventBus::new(100));
    
    // 简单的事件监听器
    let mut receiver = event_bus.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = receiver.recv().await {
            match event {
                AgentEvent::ConversationStarted { input } => {
                    println!("🚀 Started: {}", input);
                }
                AgentEvent::ToolCallsDetected { calls } => {
                    println!("🔧 Found {} tool calls", calls.len());
                }
                AgentEvent::ToolCallStarted { call } => {
                    println!("⚡ Executing: {} with {}", call.name, call.parameters);
                }
                AgentEvent::ToolCallCompleted { call, result } => {
                    println!("✅ Completed: {} -> {}", call.name, result.content);
                }
                AgentEvent::ConversationCompleted { .. } => {
                    println!("🎉 Conversation completed!");
                }
                _ => {}
            }
        }
    });

    let provider = OpenRouterProvider::new(
        api_key,
        "google/gemini-2.5-flash-lite-preview-09-2025"
    )?;

    let mut agent = Agent::new(provider)
        .with_options(AgentOptions {
            system_prompt: Some("You are a calculator. When asked to calculate, use the calculator tool with exact JSON format. Example: {\"tool_calls\": [{\"id\": \"call_1\", \"name\": \"calculator\", \"parameters\": {\"a\": 15, \"b\": 23, \"operation\": \"mul\"}}]}".into()),
            tool_choice: ToolChoice::Required,
            max_iterations: 2,
            ..Default::default()
        })
        .with_event_bus(event_bus);

    agent.register_tool(Box::new(SimpleCalculatorTool)).await;

    println!("🤖 Simple event test ready!\n");

    match agent.run("Calculate 15 * 23").await {
        Ok(response) => println!("\n🎯 Result: {}", response),
        Err(e) => eprintln!("\n💥 Error: {}", e),
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    Ok(())
}
