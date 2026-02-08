use super::{Tool, ToolInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ToolRegistry {
    tools: Arc<RwLock<HashMap<String, Box<dyn Tool>>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        let mut tools = self.tools.write().await;
        tools.insert(name, tool);
    }

    pub async fn execute_tool(
        &self,
        name: &str,
        params: &serde_json::Value,
    ) -> crate::tool::ToolResult {
        let tools = self.tools.read().await;
        if let Some(tool) = tools.get(name) {
            // Validate parameters first
            if let Err(validation_error) = tool.validate_parameters(params) {
                return crate::tool::ToolResult::error(format!(
                    "Parameter validation failed: {}",
                    validation_error
                ));
            }

            // Execute tool if validation passes
            tool.execute(params).await
        } else {
            crate::tool::ToolResult::error("Tool not found")
        }
    }

    pub async fn list_tools(&self) -> Vec<ToolInfo> {
        let tools = self.tools.read().await;
        tools
            .values()
            .map(|tool| ToolInfo {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters_schema: tool.parameters_schema(),
            })
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ToolRegistry {
    fn clone(&self) -> Self {
        Self {
            tools: self.tools.clone(),
        }
    }
}
