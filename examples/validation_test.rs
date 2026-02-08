use agent_sdk::{Agent, OpenRouterProvider, Tool, ToolResult, AgentOptions, ToolChoice};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::env;

struct StrictCalculatorTool;

#[async_trait]
impl Tool for StrictCalculatorTool {
    fn name(&self) -> &str {
        "strict_calculator"
    }

    fn description(&self) -> &str {
        "Perform arithmetic operations with strict parameter validation"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "a": {
                    "type": "number", 
                    "description": "First number"
                },
                "b": {
                    "type": "number", 
                    "description": "Second number"
                },
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
        println!("✅ Parameters validated successfully, executing calculation...");
        
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
            _ => unreachable!(), // Should never reach here due to validation
        };

        ToolResult::success(format!("{} {} {} = {}", a, operation, b, result))
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
        system_prompt: Some("You are a helpful assistant. Use the strict_calculator tool for calculations. Follow the exact parameter format required.".into()),
        tool_choice: ToolChoice::Auto,
        max_iterations: 3,
        ..Default::default()
    });

    agent.register_tool(Box::new(StrictCalculatorTool)).await;

    println!("🔒 Testing Parameter Validation");
    println!("===============================\n");

    // Test 1: Valid parameters
    println!("📊 Test 1: Valid calculation");
    match agent.run("Calculate 15 * 23 using the calculator").await {
        Ok(response) => println!("✅ Response: {}\n", response),
        Err(e) => eprintln!("❌ Error: {}\n", e),
    }

    // Test 2: Invalid operation (should be caught by validation)
    println!("⚠️  Test 2: Invalid operation");
    match agent.run("Calculate 10 power 2 (use 'pow' operation)").await {
        Ok(response) => println!("Response: {}\n", response),
        Err(e) => eprintln!("❌ Error: {}\n", e),
    }

    // Test 3: Missing required parameter
    println!("⚠️  Test 3: Missing parameter");
    match agent.run("Use calculator with just a=5 and operation=add (no b parameter)").await {
        Ok(response) => println!("Response: {}\n", response),
        Err(e) => eprintln!("❌ Error: {}\n", e),
    }

    Ok(())
}
