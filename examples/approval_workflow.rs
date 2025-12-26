//! Tool approval workflow example
//!
//! Run with: cargo run --example approval_workflow

use agent_sdk::{
    ApprovalDecision, ApprovalManager, ApprovalPolicy, OpenAIClient, Result, Runtime,
    RuntimeOptions, Tool, ToolCall, ToolResult,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file (requires approval)"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, parameters: &Value) -> Result<ToolResult> {
        let path = parameters["path"].as_str().unwrap_or("unknown");
        let content = parameters["content"].as_str().unwrap_or("");
        // In real implementation, write to file
        Ok(ToolResult::success(format!(
            "Wrote {} bytes to {}",
            content.len(),
            path
        )))
    }
}

struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read content from a file"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, parameters: &Value) -> Result<ToolResult> {
        let path = parameters["path"].as_str().unwrap_or("unknown");
        Ok(ToolResult::success(format!("Content of {}: Hello World!", path)))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

    // Create approval manager with policies
    let approval_manager = Arc::new(ApprovalManager::new());

    // Auto-approve read operations
    approval_manager
        .set_tool_policy("read_file", ApprovalPolicy::AutoApprove)
        .await;

    // Require approval for write operations
    approval_manager
        .set_tool_policy("write_file", ApprovalPolicy::RequireApproval)
        .await;

    // Or use custom policy
    approval_manager
        .set_policy(ApprovalPolicy::Custom(Arc::new(|tool_call: &ToolCall| {
            // Auto-approve if path doesn't contain sensitive directories
            if let Some(path) = tool_call.parameters["path"].as_str() {
                if path.contains("/etc") || path.contains("/system") {
                    return ApprovalDecision::Rejected("Cannot access system directories".to_string());
                }
            }
            ApprovalDecision::Approved
        })))
        .await;

    // Create runtime with approval
    let llm = OpenAIClient::new(api_key);
    let options = RuntimeOptions {
        model: "gpt-4".to_string(),
        system_prompt: Some("You are a file management assistant.".to_string()),
        require_tool_approval: true,
        ..Default::default()
    };

    let mut runtime = Runtime::new(llm)
        .with_options(options)
        .with_approval_manager(approval_manager.clone());

    runtime.register_tool(Box::new(FileWriteTool));
    runtime.register_tool(Box::new(ReadFileTool));

    // Spawn approval handler (in real app, this would be UI-driven)
    let approval_manager_clone = approval_manager.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let pending = approval_manager_clone.pending_approvals().await;
            for tool_call in pending {
                println!("Auto-approving tool call: {}", tool_call.name);
                approval_manager_clone.approve(&tool_call.id).await;
            }
        }
    });

    // Run the agent
    let response = runtime.run("Read the file /tmp/test.txt").await?;
    println!("Response: {}", response);

    Ok(())
}
