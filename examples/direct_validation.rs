use agent_sdk::tool::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

struct TestTool;

#[async_trait]
impl Tool for TestTool {
    fn name(&self) -> &str {
        "test_tool"
    }

    fn description(&self) -> &str {
        "Test tool for parameter validation"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "number"},
                "status": {"type": "string", "enum": ["active", "inactive"]}
            },
            "required": ["name", "age"]
        })
    }

    async fn execute(&self, params: &Value) -> ToolResult {
        ToolResult::success(format!("Executed with params: {}", params))
    }
}

#[tokio::main]
async fn main() {
    let tool = TestTool;
    
    println!("🧪 Testing Parameter Validation");
    println!("================================\n");
    
    // Test 1: Valid parameters
    println!("✅ Test 1: Valid parameters");
    let valid_params = json!({"name": "John", "age": 30, "status": "active"});
    match tool.validate_parameters(&valid_params) {
        Ok(_) => println!("✅ Validation passed"),
        Err(e) => println!("❌ Validation failed: {}", e),
    }
    
    // Test 2: Missing required parameter
    println!("\n❌ Test 2: Missing required parameter");
    let missing_params = json!({"name": "John"});
    match tool.validate_parameters(&missing_params) {
        Ok(_) => println!("✅ Validation passed"),
        Err(e) => println!("❌ Validation failed: {}", e),
    }
    
    // Test 3: Wrong type
    println!("\n❌ Test 3: Wrong parameter type");
    let wrong_type = json!({"name": "John", "age": "thirty"});
    match tool.validate_parameters(&wrong_type) {
        Ok(_) => println!("✅ Validation passed"),
        Err(e) => println!("❌ Validation failed: {}", e),
    }
    
    // Test 4: Invalid enum value
    println!("\n❌ Test 4: Invalid enum value");
    let invalid_enum = json!({"name": "John", "age": 30, "status": "unknown"});
    match tool.validate_parameters(&invalid_enum) {
        Ok(_) => println!("✅ Validation passed"),
        Err(e) => println!("❌ Validation failed: {}", e),
    }
    
    // Test 5: Execute with validation in registry
    println!("\n🔧 Test 5: Full execution with validation");
    use agent_sdk::tool::ToolRegistry;
    
    let registry = ToolRegistry::new();
    registry.register(Box::new(TestTool)).await;
    
    // Valid execution
    let result = registry.execute_tool("test_tool", &valid_params).await;
    println!("Valid params result: success={}, content={}", result.success, result.content);
    
    // Invalid execution
    let result = registry.execute_tool("test_tool", &missing_params).await;
    println!("Invalid params result: success={}, error={:?}", result.success, result.error);
}
