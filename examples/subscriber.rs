use easy_amqp::{AmqpClient, Result, WorkerBuilder};
use lapin::ExchangeKind;
use std::time::Duration;
use tokio::signal;

fn handle_order_event(data: Vec<u8>) -> Result<()> {
    let msg = String::from_utf8_lossy(&data);
    let event: serde_json::Value = serde_json::from_str(&msg)?;

    println!("📦 [Order] Event: {}", event);

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

    std::thread::sleep(std::time::Duration::from_millis(50));

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let client = AmqpClient::new("amqp://admin:password@localhost:5672".to_string(), 10)?;

    println!("📝 Setting up AMQP subscribers...\n");

    let pool = client.channel_pool();
    let pool2 = pool.clone();

    let worker = client
        .registry()
        .register(move |_count| {
            WorkerBuilder::new(ExchangeKind::Direct)
                .pool(pool)
                .with_exchange("order.events.v1")
                .queue("order.process")
                .build(handle_order_event)
        })
        .register(move |_count| {
            WorkerBuilder::new(ExchangeKind::Topic)
                .pool(pool2)
                .with_exchange("logs.v1")
                .routing_key("order.*")
                .queue("api_logs")
                .build(handle_log_event)
        });

    println!("\n(Press Ctrl+C to exit)\n");

    tokio::select! {
        result = worker.run() => {
            if let Err(e) = result {
                eprintln!("❌ Registry error: {:?}", e);
            }
        }
        _ = signal::ctrl_c() => {
            println!("\n🛑 Shutting down gracefully...");
        }
    }

    tokio::time::sleep(Duration::from_millis(500)).await;
    println!("✓ Shutdown complete");

    Ok(())
}
