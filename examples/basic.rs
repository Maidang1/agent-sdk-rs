use agent_sdk::{
    OpenAIClient, Result, Runtime, RuntimeOptions, Tool, ToolResult,
};
use async_trait::async_trait;
use serde_json::{json, Value};

struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Perform basic arithmetic operations"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["add", "subtract", "multiply", "divide"]
                },
                "a": { "type": "number" },
                "b": { "type": "number" }
            },
            "required": ["operation", "a", "b"]
        })
    }

    async fn execute(&self, parameters: &Value) -> Result<ToolResult> {
        let op = parameters["operation"].as_str().unwrap_or("");
        let a = parameters["a"].as_f64().unwrap_or(0.0);
        let b = parameters["b"].as_f64().unwrap_or(0.0);

        let result = match op {
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b == 0.0 {
                    return Ok(ToolResult::error("Division by zero"));
                }
                a / b
            }
            _ => return Ok(ToolResult::error("Unknown operation")),
        };

        Ok(ToolResult::success(format!("{}", result)))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

    let llm = OpenAIClient::new(api_key);

    let options = RuntimeOptions {
        model: "gpt-4".to_string(),
        system_prompt: Some("You are a helpful assistant with access to a calculator.".to_string()),
        ..Default::default()
    };

    let mut runtime = Runtime::new(llm).with_options(options);
    runtime.register_tool(Box::new(CalculatorTool));

    let response = runtime.run("What is 42 * 17?").await?;
    println!("Response: {}", response);

    Ok(())
}
