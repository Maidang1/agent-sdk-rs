//! Multi-agent collaboration example with AgentPool and Room
//!
//! Run with: cargo run --example multi_agent

use agent_sdk::{
    AgentPool, EventBus, OpenAIClient, Result, Room, RuntimeOptions,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

    // Create shared event bus
    let event_bus = Arc::new(EventBus::new(1024));

    // Create agent pool
    let pool: AgentPool<OpenAIClient> = AgentPool::new(event_bus.clone()).with_default_options(RuntimeOptions {
        model: "gpt-4".to_string(),
        system_prompt: Some("You are a collaborative AI assistant.".to_string()),
        ..Default::default()
    });

    // Create agents
    let llm1 = OpenAIClient::new(&api_key);
    let llm2 = OpenAIClient::new(&api_key);

    pool.create_agent("agent_1", "Researcher", llm1, None).await?;
    pool.create_agent("agent_2", "Writer", llm2, None).await?;

    // Create a collaboration room
    let room = Room::new("collab_room", "Collaboration Room", event_bus.clone());

    // Agents join the room
    room.join("agent_1").await;
    room.join("agent_2").await;

    // Simulate collaboration
    room.send("agent_1", "I found some interesting data about AI trends.").await;
    room.send("agent_2", "Great! I'll incorporate that into the report.").await;

    // Get room messages
    let messages = room.get_all_messages(None).await;
    println!("Room messages:");
    for msg in messages {
        println!("  [{}]: {}", msg.from_agent, msg.content);
    }

    // Fork an agent (create child with same context)
    let llm3 = OpenAIClient::new(&api_key);
    pool.fork_agent("agent_1", "agent_1_fork", llm3).await?;

    // Get lineage
    let lineage = pool.get_lineage("agent_1_fork").await;
    println!("\nAgent lineage: {:?}", lineage);

    // List all agents
    let agents = pool.list_agents().await;
    println!("\nAll agents:");
    for agent in agents {
        println!("  - {} ({}): {:?}", agent.name, agent.id, agent.state);
    }

    Ok(())
}
