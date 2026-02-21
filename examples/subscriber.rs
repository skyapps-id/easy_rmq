use easy_amqp::{AmqpClient, AmqpSubscriber, Result};
use tokio::signal;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let client = AmqpClient::new("amqp://admin:password@localhost:5672".to_string(), 10)?
    .with_exchange("order");

    println!("📝 Setting up AMQP infrastructure...\n");

    let subscriber = client.subscriber();

    // Spawn subscriber task with graceful shutdown
    let handle = tokio::spawn(async move {
        println!("📥 Subscribing (auto-create queue + binding to default exchange)...\n");
        println!("(Press Ctrl+C to exit)\n");
        
        // Simple: queue, routing_key, handler
        if let Err(e) = subscriber.subscribe(
            "order_queue",
            "order.created",
            |data| {
                let msg = String::from_utf8_lossy(&data);
                println!("📨 Received: {}", msg);
                Ok(())
            }
        ).await {
            eprintln!("❌ Subscribe error: {:?}", e);
        }
    });

    // Wait for Ctrl+C
    signal::ctrl_c().await?;
    println!("\n🛑 Shutting down gracefully...");

    handle.abort();
    
    tokio::time::sleep(Duration::from_millis(500)).await;
    println!("✓ Shutdown complete");

    Ok(())
}
