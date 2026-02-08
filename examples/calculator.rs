use agent_sdk::{Agent, AgentOptions, OpenRouterProvider, Tool, ToolChoice, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::env;

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
        // 注意：参数已经通过 validate_parameters 校验，这里可以安全地使用 unwrap
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
            _ => unreachable!(), // 由于校验，这里不会到达
        };

        ToolResult::success(result.to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("OPEN_ROUTER_API_KEY")
        .expect("Please set the OPEN_ROUTER_API_KEY environment variable");

    let provider = OpenRouterProvider::new(api_key, "google/gemini-2.5-flash-lite-preview-09-2025")?;

    let mut agent = Agent::new(provider).with_options(AgentOptions {
        system_prompt: Some("You are a helpful assistant with access to tools. Always use tools when asked to perform calculations.".into()),
        tool_choice: ToolChoice::Auto,
        max_iterations: 5,
        ..Default::default()
    });

    agent.register_tool(Box::new(CalculatorTool)).await;

    println!("Agent with calculator tool ready!");
    println!("Testing: What is 15 * 23?");

    match agent.run("What is 15 * 23?").await {
        Ok(response) => println!("Response: {}", response),
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}
