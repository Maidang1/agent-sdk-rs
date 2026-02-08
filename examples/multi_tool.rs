use agent_sdk::{Agent, OpenRouterProvider, Tool, ToolResult, AgentOptions, ToolChoice};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::env;

// 计算器工具
struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Perform arithmetic operations (add, sub, mul, div)"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "a": {"type": "number", "description": "First number"},
                "b": {"type": "number", "description": "Second number"},
                "operation": {
                    "type": "string", 
                    "enum": ["add", "sub", "mul", "div"],
                    "description": "Operation to perform"
                }
            },
            "required": ["a", "b", "operation"]
        })
    }

    async fn execute(&self, params: &Value) -> ToolResult {
        let a = params["a"].as_f64().unwrap_or(0.0);
        let b = params["b"].as_f64().unwrap_or(0.0);
        let operation = params["operation"].as_str().unwrap_or("add");

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
            _ => return ToolResult::error("Unknown operation"),
        };

        ToolResult::success(result.to_string())
    }
}

// 文本处理工具
struct TextTool;

#[async_trait]
impl Tool for TextTool {
    fn name(&self) -> &str {
        "text_processor"
    }

    fn description(&self) -> &str {
        "Process text (uppercase, lowercase, reverse, count_words)"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {"type": "string", "description": "Text to process"},
                "operation": {
                    "type": "string",
                    "enum": ["uppercase", "lowercase", "reverse", "count_words"],
                    "description": "Operation to perform on text"
                }
            },
            "required": ["text", "operation"]
        })
    }

    async fn execute(&self, params: &Value) -> ToolResult {
        // 参数已校验，可以安全使用
        let text = params["text"].as_str().unwrap();
        let operation = params["operation"].as_str().unwrap();

        let result = match operation {
            "uppercase" => text.to_uppercase(),
            "lowercase" => text.to_lowercase(),
            "reverse" => text.chars().rev().collect(),
            "count_words" => text.split_whitespace().count().to_string(),
            _ => unreachable!(), // 校验确保不会到达
        };

        ToolResult::success(result)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("OPEN_ROUTER_API_KEY")
        .expect("Please set the OPEN_ROUTER_API_KEY environment variable");

    let provider = OpenRouterProvider::new(
        api_key,
        "google/gemini-2.5-flash-lite-preview-09-2025"
    )?;

    let mut agent = Agent::new(provider).with_options(AgentOptions {
        system_prompt: Some("You are a helpful assistant with access to tools. When asked to perform calculations or text processing, use the appropriate tools. Always respond with JSON format for tool calls.".into()),
        tool_choice: ToolChoice::Auto,
        max_iterations: 5,
        ..Default::default()
    });

    // 注册多个工具
    agent.register_tool(Box::new(CalculatorTool)).await;
    agent.register_tool(Box::new(TextTool)).await;

    println!("🤖 Multi-tool Agent Ready!");
    println!("Available tools: calculator, text_processor");
    println!();

    // 测试计算
    println!("📊 Testing calculation: What is (15 * 23) + (100 / 4)?");
    match agent.run("Calculate (15 * 23) + (100 / 4) step by step").await {
        Ok(response) => println!("✅ Response: {}\n", response),
        Err(e) => eprintln!("❌ Error: {}\n", e),
    }

    // 测试文本处理
    println!("📝 Testing text processing: Process 'Hello World'");
    match agent.run("Convert 'Hello World' to uppercase and then reverse it").await {
        Ok(response) => println!("✅ Response: {}\n", response),
        Err(e) => eprintln!("❌ Error: {}\n", e),
    }

    // 测试组合使用
    println!("🔄 Testing combined usage:");
    match agent.run("Count the words in 'The quick brown fox jumps' and then multiply that count by 7").await {
        Ok(response) => println!("✅ Response: {}", response),
        Err(e) => eprintln!("❌ Error: {}", e),
    }

    Ok(())
}
