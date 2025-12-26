//! Scheduler example with reminders and triggers
//!
//! Run with: cargo run --example scheduler

use agent_sdk::{
    EventBus, OpenAIClient, Result, Runtime, RuntimeOptions, Scheduler, Tool, ToolResult, Trigger,
    ScheduledTask, ScheduledAction,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::time::Duration;

struct SlowTool;

#[async_trait]
impl Tool for SlowTool {
    fn name(&self) -> &str {
        "slow_operation"
    }

    fn description(&self) -> &str {
        "A slow operation that takes time"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "duration_secs": { "type": "integer" }
            },
            "required": ["duration_secs"]
        })
    }

    async fn execute(&self, parameters: &Value) -> Result<ToolResult> {
        let duration = parameters["duration_secs"].as_u64().unwrap_or(1);
        tokio::time::sleep(Duration::from_secs(duration)).await;
        Ok(ToolResult::success(format!("Completed after {} seconds", duration)))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

    // Create event bus and scheduler
    let event_bus = Arc::new(EventBus::new(1024));
    let scheduler = Arc::new(Scheduler::new(event_bus.clone()));

    // Add scheduled tasks
    
    // Reminder after 3 iterations
    scheduler
        .remind_after_iterations("iteration_reminder", 3, "You've completed 3 iterations. Consider wrapping up.")
        .await;

    // Reminder every 30 seconds
    scheduler
        .remind_at_interval("time_reminder", Duration::from_secs(30), "30 seconds have passed.")
        .await;

    // Custom trigger: pause after 5 iterations
    scheduler
        .add_task(ScheduledTask {
            id: "auto_pause".to_string(),
            trigger: Trigger::AfterIterations(5),
            action: ScheduledAction::Pause,
            repeat: false,
            last_triggered: None,
        })
        .await;

    // Subscribe to events
    let mut receiver = event_bus.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = receiver.recv().await {
            println!("📬 Event: {:?}", event);
        }
    });

    // Create runtime
    let llm = OpenAIClient::new(api_key);
    let options = RuntimeOptions {
        model: "gpt-4".to_string(),
        system_prompt: Some("You are an assistant that can perform slow operations.".to_string()),
        max_iterations: 10,
        ..Default::default()
    };

    let mut runtime = Runtime::new(llm)
        .with_options(options)
        .with_event_bus(event_bus);

    runtime.register_tool(Box::new(SlowTool));

    println!("Starting agent with scheduler...");
    println!("Scheduler will:");
    println!("  - Send reminder after 3 iterations");
    println!("  - Send reminder every 30 seconds");
    println!("  - Pause after 5 iterations");

    // Note: In a real scenario, you'd run the agent
    // let response = runtime.run("Perform a slow operation for 2 seconds").await?;
    // println!("Response: {}", response);

    // Demo scheduler context
    for i in 0..6 {
        scheduler.tick_iteration().await;
        scheduler.update_elapsed(Duration::from_secs(i * 10)).await;
        let actions = scheduler.check_triggers("demo_agent").await;
        if !actions.is_empty() {
            println!("Iteration {}: Triggered actions: {:?}", i + 1, actions);
        }
    }

    Ok(())
}
