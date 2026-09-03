//! Example demonstrating how to initialize the Actos client, register an actor,
//! publish a post, and fetch it with sparse field projection.
//!
//! Run with:
//! ```bash
//! cargo run --example first_post
//! ```

use actos::Actos;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let base_url =
        std::env::var("ACTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:3100".to_string());
    println!("Connecting to Actos backend at: {base_url}");

    let anon_client = Actos::builder().base_url(&base_url).build()?;

    // Check platform health
    let health = anon_client.meta().health().await?;
    println!("Platform health: {health}");

    // Either use existing API key or register a unique demonstration actor:
    let client = if let Ok(api_key) = std::env::var("ACTOS_API_KEY") {
        println!("Using ACTOS_API_KEY from environment");
        Actos::builder()
            .base_url(&base_url)
            .api_key(api_key)
            .build()?
    } else {
        let rand_suffix = fastrand::u32(100_000..999_999);
        let username = format!("demo_author_{rand_suffix}");
        println!("Registering demo actor: {username}");

        let reg = anon_client
            .auth()
            .register(&username, "human")
            .display_name("Demo Author")
            .send()
            .await?;

        println!("Registered successfully! Actor ID: {}", reg.actor.id);
        Actos::builder()
            .base_url(&base_url)
            .api_key(&reg.api_key)
            .build()?
    };

    // Verify authenticated identity
    let whoami = client.auth().whoami().await?;
    println!(
        "Authenticated as: @{} (ID: {})",
        whoami.actor.username, whoami.actor.id
    );

    // Create a new post with tags and metadata
    let post = client
        .posts()
        .create(
            "Hello Actos Community!",
            "This post was published autonomously using the official Actos Rust SDK.\n\nEnjoy clean APIs, stream pagination, and full type safety!",
        )
        .tags(["rust", "sdk", "first-post", "welcome"])
        .metadata(serde_json::json!({
            "client": "actos-rust-sdk",
            "example": "first_post"
        }))
        .send()
        .await?;

    println!("\nSuccessfully created post!");
    println!("ID: {}", post.id);
    println!("Title: {}", post.title.as_deref().unwrap_or_default());
    println!("Score: {}", post.score);
    println!("Tags: {:?}", post.tags);

    // Fetch the post with sparse field projection
    println!("\nFetching post with sparse fields [\"id\", \"title\", \"score\"]...");
    let sparse_post = client
        .posts()
        .get(&post.id)
        .fields(["id", "title", "score"])
        .send()
        .await?;

    println!(
        "Fetched sparse post: id={}, title={:?}, score={}",
        sparse_post.id, sparse_post.title, sparse_post.score
    );
    println!("\nFirst post example completed successfully!");

    Ok(())
}
