use easy_amqp::{AmqpClient, Result};
use tokio::signal;
use std::time::Duration;

fn handle_order_event(data: Vec<u8>) -> Result<()> {
    let msg = String::from_utf8_lossy(&data);
    let event: serde_json::Value = serde_json::from_str(&msg)?;
    
    println!("📦 [Order] Event: {}", event);
    
    // Simulasi processing order
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    let order_id = event["id"].as_str().unwrap_or("unknown");
    let total = event["total"].as_f64().unwrap_or(0.0);
    println!("✅ [Order] Processed: {} | Total: ${}", order_id, total);
    Ok(())
}

fn handle_log_event(data: Vec<u8>) -> Result<()> {
    let msg = String::from_utf8_lossy(&data);
    let log: serde_json::Value = serde_json::from_str(&msg)?;
    
    let level = log["level"].as_str().unwrap_or("INFO");
    let service = log["service"].as_str().unwrap_or("unknown");
    let message = log["message"].as_str().unwrap_or("-");
    
    println!("📊 [Log] [{}] {} | {}", level, service, message);
    
    // Simulasi write ke log storage
    std::thread::sleep(std::time::Duration::from_millis(50));
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let client = AmqpClient::new("amqp://admin:password@localhost:5672".to_string(), 10)?;

    println!("📝 Setting up AMQP subscribers...\n");

    // Subscriber 1 - orders exchange (Direct)
    let sub1 = client.subscriber(lapin::ExchangeKind::Direct).with_exchange("order.events.v1");
    
    let handle1 = tokio::spawn(async move {
        println!("📥 [Direct] Queue: sales.order.processworker\n");
        if let Err(e) = sub1.direct("order.process").build(handle_order_event).await {
            eprintln!("❌ Subscribe error: {:?}", e);
        }
    });

    // Subscriber 2 - logs exchange (Topic)
    let sub2 = client.subscriber(lapin::ExchangeKind::Topic).with_exchange("logs.v1");
    
    let handle2 = tokio::spawn(async move {
        println!("📥 [Topic] Queue: order.*.api_logs\n");
        
        if let Err(e) = sub2.topic("order.*", "api_logs").build(handle_log_event).await {
            eprintln!("❌ Subscribe error: {:?}", e);
        }
    });

    println!("\n(Press Ctrl+C to exit)\n");

    // Wait for Ctrl+C
    signal::ctrl_c().await?;
    println!("\n🛑 Shutting down gracefully...");

    handle1.abort();
    handle2.abort();
    
    tokio::time::sleep(Duration::from_millis(500)).await;
    println!("✓ Shutdown complete");

    Ok(())
}
