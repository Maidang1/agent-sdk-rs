use agent_sdk::{Agent, OpenRouterProvider, Tool, ToolResult, AgentOptions, ToolChoice, EventBus, AgentEvent};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::{env, sync::Arc};

struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Perform arithmetic operations"
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
    
    // 启动事件监听器
    let mut receiver = event_bus.subscribe();
    let event_listener = tokio::spawn(async move {
        println!("🎧 Event listener started...\n");
        
        while let Ok(event) = receiver.recv().await {
            match event {
                AgentEvent::ConversationStarted { input } => {
                    println!("🚀 Conversation started with input: '{}'", input);
                }
                AgentEvent::LlmRequestSent { messages } => {
                    println!("📤 LLM request sent with {} messages", messages.len());
                }
                AgentEvent::LlmResponseReceived { content, model } => {
                    println!("📥 LLM response received from {}: '{}'", model, 
                        if content.len() > 50 { 
                            format!("{}...", &content[..50]) 
                        } else { 
                            content 
                        }
                    );
                }
                AgentEvent::ToolCallsDetected { calls } => {
                    println!("🔧 Detected {} tool call(s)", calls.len());
                    for call in calls {
                        println!("   - {}: {}", call.name, call.parameters);
                    }
                }
                AgentEvent::ToolCallStarted { call } => {
                    println!("⚡ Starting tool call: {} with params {}", call.name, call.parameters);
                }
                AgentEvent::ToolCallCompleted { call, result } => {
                    println!("✅ Tool call completed: {} -> {}", call.name, result.content);
                }
                AgentEvent::ToolCallFailed { call, error } => {
                    println!("❌ Tool call failed: {} -> {}", call.name, error);
                }
                AgentEvent::ConversationCompleted { response } => {
                    println!("🎉 Conversation completed with response: '{}'", response);
                }
                AgentEvent::ConversationFailed { error } => {
                    println!("💥 Conversation failed: {}", error);
                }
            }
        }
    });

    let provider = OpenRouterProvider::new(
        api_key,
        "google/gemini-2.5-flash-lite-preview-09-2025"
    );

    let mut agent = Agent::new(provider)
        .with_options(AgentOptions {
            system_prompt: Some("You are a helpful calculator assistant. When using the calculator tool, you must provide parameters 'a', 'b', and 'operation'. For example: {\"a\": 15, \"b\": 23, \"operation\": \"mul\"}".into()),
            tool_choice: ToolChoice::Auto,
            max_iterations: 3,
            ..Default::default()
        })
        .with_event_bus(event_bus);

    agent.register_tool(Box::new(CalculatorTool)).await;

    println!("🤖 Agent with event monitoring ready!\n");

    // 测试计算
    match agent.run("What is (15 * 23) + (100 / 4)?").await {
        Ok(response) => println!("\n🎯 Final result: {}", response),
        Err(e) => eprintln!("\n💥 Error: {}", e),
    }

    // 等待事件监听器完成
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    event_listener.abort();

    Ok(())
}
