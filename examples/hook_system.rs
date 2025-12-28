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

        // 模拟一些处理时间
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

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
    tokio::spawn(async move {
        println!("🎧 Hook system started...\n");
        
        while let Ok(event) = receiver.recv().await {
            match event {
                AgentEvent::ConversationStarted { input } => {
                    println!("[LOG] Conversation started: {}", input);
                }
                AgentEvent::ToolCallStarted { call } => {
                    println!("🔥 [CUSTOM] About to execute tool: {}", call.name);
                    println!("[LOG] Tool call started: {}", call.name);
                }
                AgentEvent::ToolCallCompleted { call, result } => {
                    println!("[LOG] Tool call completed: {} -> success: {}", call.name, result.success);
                }
                AgentEvent::ToolCallFailed { call, error } => {
                    eprintln!("[ERROR] Tool '{}' failed: {}", call.name, error);
                }
                AgentEvent::LlmResponseReceived { content, .. } => {
                    if content.contains("tool_calls") {
                        println!("🎯 [CUSTOM] LLM wants to use tools!");
                    }
                }
                AgentEvent::ConversationCompleted { .. } => {
                    println!("[LOG] Conversation completed successfully");
                }
                AgentEvent::ConversationFailed { error } => {
                    println!("[LOG] Conversation failed: {}", error);
                    eprintln!("[ERROR] Conversation failed: {}", error);
                }
                _ => {}
            }
        }
    });

    let provider = OpenRouterProvider::new(
        api_key,
        "google/gemini-2.5-flash-lite-preview-09-2025"
    );

    let mut agent = Agent::new(provider)
        .with_options(AgentOptions {
            system_prompt: Some("You are a helpful calculator assistant. Use the calculator tool with exact parameters: a, b, and operation.".into()),
            tool_choice: ToolChoice::Auto,
            max_iterations: 3,
            ..Default::default()
        })
        .with_event_bus(event_bus);

    agent.register_tool(Box::new(CalculatorTool)).await;

    println!("🤖 Agent with Hook system ready!\n");

    // 测试多个计算
    let tests = vec![
        "What is 15 * 23?",
        "Calculate 100 / 4",
        "What is 50 + 25?",
    ];

    for (i, test) in tests.iter().enumerate() {
        println!("\n📊 Test {}: {}", i + 1, test);
        match agent.run(test).await {
            Ok(response) => println!("✅ Result: {}", response),
            Err(e) => eprintln!("❌ Error: {}", e),
        }
        
        // 短暂延迟以便观察事件
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    // 等待所有事件处理完成
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    Ok(())
}
