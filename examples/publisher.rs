use easy_amqp::{AmqpClient, Result};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let client = AmqpClient::new("amqp://admin:password@localhost:5672".to_string(), 10)?
    .with_exchange("order");

    let publisher = client.publisher();

    println!("📤 Starting publisher...\n");
    println!("⚠️  Make sure subscriber is running first to setup queue\n");

    // Publish multiple messages
    for i in 1..=5 {
        let order = serde_json::json!({
            "id": format!("order-{}", i),
            "total": (i as f64) * 100.0,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        println!("📤 Sending: {}", order);
        
        publisher.publish_text("order.created", &order.to_string()).await?;

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    println!("\n✓ All messages published!");

    Ok(())
}
