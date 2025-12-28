pub mod registry;
pub mod executor;
pub mod parser;

pub use registry::*;
pub use executor::*;
pub use parser::*;

use async_trait::async_trait;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub success: bool,
    pub content: String,
    pub error: Option<String>,
}

impl ToolResult {
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
            error: None,
        }
    }

    pub fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            content: String::new(),
            error: Some(error.into()),
        }
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    
    /// Validate parameters against schema. Default implementation does basic validation.
    fn validate_parameters(&self, params: &Value) -> Result<(), String> {
        let schema = self.parameters_schema();
        validate_against_schema(params, &schema)
    }
    
    async fn execute(&self, params: &Value) -> ToolResult;
}

/// Basic JSON schema validation
fn validate_against_schema(params: &Value, schema: &Value) -> Result<(), String> {
    let schema_obj = schema.as_object().ok_or("Schema must be an object")?;
    let params_obj = params.as_object().ok_or("Parameters must be an object")?;
    
    // Check required fields
    if let Some(required) = schema_obj.get("required").and_then(|r| r.as_array()) {
        for req_field in required {
            let field_name = req_field.as_str().ok_or("Required field name must be string")?;
            if !params_obj.contains_key(field_name) {
                return Err(format!("Missing required parameter: {}", field_name));
            }
        }
    }
    
    // Check properties
    if let Some(properties) = schema_obj.get("properties").and_then(|p| p.as_object()) {
        for (param_name, param_value) in params_obj {
            if let Some(prop_schema) = properties.get(param_name).and_then(|p| p.as_object()) {
                validate_property(param_value, prop_schema, param_name)?;
            }
        }
    }
    
    Ok(())
}

fn validate_property(value: &Value, schema: &serde_json::Map<String, Value>, param_name: &str) -> Result<(), String> {
    // Check type
    if let Some(expected_type) = schema.get("type").and_then(|t| t.as_str()) {
        let actual_type = match value {
            Value::String(_) => "string",
            Value::Number(_) => "number",
            Value::Bool(_) => "boolean",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
            Value::Null => "null",
        };
        
        if actual_type != expected_type {
            return Err(format!("Parameter '{}' must be of type '{}', got '{}'", param_name, expected_type, actual_type));
        }
    }
    
    // Check enum values
    if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
        if !enum_values.contains(value) {
            let valid_values: Vec<String> = enum_values.iter()
                .map(|v| v.to_string())
                .collect();
            return Err(format!("Parameter '{}' must be one of: [{}]", param_name, valid_values.join(", ")));
        }
    }
    
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub parameters_schema: Value,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub parameters: Value,
}
