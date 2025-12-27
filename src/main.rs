use reqwest::Client;

mod provider;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {}", e);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    let rest = client
        .get("https://www.rust-lang.org")
        .send()
        .await?
        .text()
        .await?;
    println!("Response: {:?}", rest);
    Ok(())
}
