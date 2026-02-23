use easy_rmq::{AmqpClient, Result};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let client = AmqpClient::new("amqp://admin:password@localhost:5672".to_string(), 10)?;

    println!("📤 Starting publishers...\n");

    // Publisher 1 - order events exchange (Direct)
    let pub1 = client.publisher().with_exchange("order.events.v1");
    
    for i in 1..=3 {
        let order = serde_json::json!({
            "id": format!("ORD-{:04}", i),
            "total": (i as f64) * 150.0,
            "items": i * 2,
            "status": "created",
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        println!("📦 [Orders] Sending: {}", order);
        pub1.publish_text("order.process", &order.to_string()).await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Publisher 2 - logs exchange (Topic)
    let pub2 = client.publisher().with_exchange("logs.v1");
    
    for i in 1..=2 {
        let log = serde_json::json!({
            "level": "INFO",
            "service": "api-gateway",
            "message": format!("Request processed - request #{}", i),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "request_id": format!("req-{:x}", i)
        });

        println!("📧 [Logs] Sending: {}", log);
        pub2.publish_text("order.api", &log.to_string()).await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    println!("\n✓ All messages published!");
    
    // Wait for messages to be sent
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("✓ Shutdown complete");

    Ok(())
}
