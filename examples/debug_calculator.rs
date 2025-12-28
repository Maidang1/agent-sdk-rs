use agent_sdk::{Agent, OpenRouterProvider, Tool, ToolResult, AgentOptions, ToolChoice};
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
        println!("Calculator tool called with params: {}", params);
        
        let a = params["a"].as_f64().unwrap_or(0.0);
        let b = params["b"].as_f64().unwrap_or(0.0);
        let operation = params["operation"].as_str().unwrap_or("add");

        println!("Executing: {} {} {}", a, operation, b);

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

        println!("Result: {}", result);
        ToolResult::success(result.to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = env::var("OPEN_ROUTER_API_KEY")
        .expect("Please set the OPEN_ROUTER_API_KEY environment variable");

    let provider = OpenRouterProvider::new(
        api_key,
        "google/gemini-2.5-flash-lite-preview-09-2025"
    );

    let mut agent = Agent::new(provider).with_options(AgentOptions {
        system_prompt: Some("You are a helpful assistant. When asked to perform calculations, you MUST use the calculator tool. Respond with JSON format exactly like this example: {\"tool_calls\": [{\"id\": \"call_1\", \"name\": \"calculator\", \"parameters\": {\"a\": 15, \"b\": 23, \"operation\": \"mul\"}}]}".into()),
        tool_choice: ToolChoice::Required,
        max_iterations: 3,
        ..Default::default()
    });

    agent.register_tool(Box::new(CalculatorTool)).await;

    println!("Agent with calculator tool ready!");
    println!("Testing: Calculate 15 * 23");

    match agent.run("Calculate 15 * 23 using the calculator tool").await {
        Ok(response) => println!("Final Response: {}", response),
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}
