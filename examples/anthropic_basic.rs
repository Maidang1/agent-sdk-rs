use agent_sdk::{Agent, AgentOptions, AnthropicProvider, ToolChoice};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let auth_token = env::var("ANTHROPIC_AUTH_TOKEN")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let api_key = env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_default();

    if auth_token.is_none() && api_key.is_empty() {
        panic!("Please set ANTHROPIC_AUTH_TOKEN or ANTHROPIC_API_KEY");
    }

    // Stable Claude Sonnet default. Override with ANTHROPIC_MODEL if needed.
    let model = env::var("ANTHROPIC_MODEL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string());

    let mut provider = AnthropicProvider::new(api_key, model)?;
    if let Some(token) = auth_token {
        provider = provider.with_auth_token(token);
    }
    if let Ok(base_url) = env::var("ANTHROPIC_BASE_URL") {
        if !base_url.trim().is_empty() {
            provider = provider.with_base_url(base_url);
        }
    }

    let mut agent = Agent::new(provider).with_options(AgentOptions {
        system_prompt: Some("You are a concise and practical coding assistant.".into()),
        max_iterations: 3,
        tool_choice: ToolChoice::None,
        generate_options: Default::default(),
    });

    match agent
        .run("Explain the difference between Vec and LinkedList in Rust in 3 bullet points.")
        .await
    {
        Ok(response) => println!("{}", response),
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok(())
}
