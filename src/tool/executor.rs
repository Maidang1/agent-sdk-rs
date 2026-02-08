use super::{ToolCall, ToolRegistry, ToolResult};

pub struct ToolExecutor {
    registry: ToolRegistry,
}

impl ToolExecutor {
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }

    pub async fn execute_calls(&self, calls: Vec<ToolCall>) -> Vec<ToolResult> {
        let mut results = Vec::new();
        for call in calls {
            results.push(self.execute_single(&call).await);
        }
        results
    }

    pub async fn execute_single(&self, call: &ToolCall) -> ToolResult {
        self.registry
            .execute_tool(&call.name, &call.parameters)
            .await
    }
}
