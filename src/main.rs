mod provider;

use crate::provider::LlmProvider;
use provider::OpenRouterProvider;
use std::env;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {}", e);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = match env::var("OPEN_ROUTER_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            panic!("Please set the OPEN_ROUTER_API_KEY environment variable");
        }
    };
    let open_router =
        OpenRouterProvider::new(api_key, "google/gemini-2.5-flash-lite-preview-09-2025");
    let messages = vec![provider::Message {
        role: provider::Role::User,
        content: "写一个 react to-do app".into(),
    }];

    let mut response = open_router.generate_stream(messages, None).await?;
    while let Some(msg) = response.receiver.recv().await {
        match msg {
            Ok(content) => print!("{}", content),
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    println!();
    Ok(())
}
