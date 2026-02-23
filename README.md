# lib-amqp

Rust AMQP library with connection pool, publisher, subscriber, and dependency injection support.

## Features

- **Connection Pool**: Efficiently manages AMQP connections using deadpool
- **Publisher**: Send messages to exchanges with routing keys
- **Subscriber**: Receive messages from queues with handlers
- **Worker Registry**: Register and manage multiple workers with a clean pattern
- **Auto Setup**: Automatically creates exchanges and queues
- **Dependency Injection**: Support for trait-based DI pattern
- **Type Safe**: Strong error handling with thiserror
- **Async**: Full async support using tokio

## Installation

```toml
[dependencies]
easy_amqp = { path = "./lib-amqp" }
```

## Quick Start

### 1. Start RabbitMQ

```bash
docker run -d --name rabbitmq \
  -p 5672:5672 \
  -p 15672:15672 \
  rabbitmq:3-management
```

### 2. Subscriber Example (Run First!)

Open terminal 1 - Subscriber sets up queue & binding:
```bash
cargo run --example subscriber
```

### 3. Publisher Example (Run Second)

Open terminal 2:
```bash
cargo run --example publisher
```

Press `Ctrl+C` on subscriber for graceful shutdown.

## Architecture & Best Practices

🎯 **Simple & Clean:**
- **Default Exchange**: `amq.direct` (RabbitMQ built-in)
- **Publisher**: Auto-create exchange + send messages
- **Subscriber**: Auto-create exchange + queue + binding
- **Worker Registry**: Register multiple workers with clean pattern
- **Full Auto-Setup**: No manual infrastructure needed

This follows AMQP best practices:
- Producer → Send to exchange (auto-created if not exists)
- Consumer → Auto-create everything + consume
- Registry → Manage multiple workers with consistent pattern

## Basic Usage

### Creating a Client

```rust
use easy_amqp::AmqpClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = AmqpClient::new(
        "amqp://guest:guest@localhost:5672".to_string(),
        10  // max pool size
    )?;

    Ok(())
}
```

### Publisher

Publisher **simple** - send to default exchange:

```rust
use easy_amqp::AmqpClient;

let client = AmqpClient::new("amqp://guest:guest@localhost:5672".to_string(), 10)?;

let publisher = client.publisher();

// Publish text
publisher.publish_text(
    "order.created",    // routing key
    "Hello, AMQP!"
).await?;

// Publish JSON
#[derive(serde::Serialize)]
struct Order {
    id: String,
    total: f64,
}

let order = Order {
    id: "123".to_string(),
    total: 100.0,
};

publisher.publish_json("order.created", &order).await?;
```

✅ **Auto send to default exchange** (`amq.direct`)
✅ **Auto-create exchange** if not exists (durable)
✅ **No manual setup needed**

### Multiple Exchanges

```rust
use lapin::ExchangeKind;

let client = AmqpClient::new("...", 10)?;

// Publisher 1 - Direct exchange
let pub1 = client.publisher().with_exchange("orders", ExchangeKind::Direct);
pub1.publish_text("order.created", "Order data").await?;

// Publisher 2 - Topic exchange
let pub2 = client.publisher().with_exchange("logs", ExchangeKind::Topic);
pub2.publish_text("order.created", "Log data").await?;

// Publisher 3 - Fanout exchange
let pub3 = client.publisher().with_exchange("broadcast", ExchangeKind::Fanout);
pub3.publish_text("any", "Broadcast data").await?;

// Shortcut methods
let pub4 = client.publisher().with_topic("logs");
let pub5 = client.publisher().with_direct("orders");
let pub6 = client.publisher().with_fanout("events");
```

✅ **Explicit** - exchange type clear from parameters
✅ **Flexible** - Direct, Topic, Fanout, Headers
✅ **Auto-create** exchange with appropriate type

### Subscriber with Worker Registry

Use `SubscriberRegistry` to manage multiple workers:

```rust
use easy_amqp::{AmqpClient, SubscriberRegistry, WorkerBuilder};
use lapin::ExchangeKind;

#[tokio::main]
async fn main() -> easy_amqp::Result<()> {
    let client = AmqpClient::new("amqp://admin:password@localhost:5672".to_string(), 10)?;
    let pool = client.channel_pool();

    let worker = SubscriberRegistry::new()
        .register({
            let pool = pool.clone();
            move |_count| {
                println!("📝 Registering worker #{}", _count);
                WorkerBuilder::new(ExchangeKind::Direct)
                    .pool(pool)
                    .with_exchange("order.events.v1")
                    .queue("order.process")
                    .build(handle_order_event)
            }
        })
        .register({
            let pool = pool.clone();
            move |_count| {
                println!("📝 Registering worker #{}", _count);
                WorkerBuilder::new(ExchangeKind::Topic)
                    .pool(pool)
                    .with_exchange("logs.v1")
                    .routing_key("order.*")
                    .queue("api_logs")
                    .build(handle_log_event)
            }
        });

    worker.run().await?;
    Ok(())
}

fn handle_order_event(data: Vec<u8>) -> easy_amqp::Result<()> {
    let msg = String::from_utf8_lossy(&data);
    println!("📦 Order: {}", msg);
    Ok(())
}

fn handle_log_event(data: Vec<u8>) -> easy_amqp::Result<()> {
    let msg = String::from_utf8_lossy(&data);
    println!("📊 Log: {}", msg);
    Ok(())
}
```

**Queue Format per Exchange Type:**

| Exchange | Parameter | Queue Name | Routing Key |
|----------|-----------|------------|-------------|
| **Direct** | `.queue("rk")` | `rk.job` | `rk` |
| **Topic** | `.routing_key("rk")` + `.queue("q")` | `q` | `rk` |
| **Fanout** | `.queue("q")` | `q` | `""` |

✅ **Auto-created** exchange + queue + binding
✅ **Direct**: queue auto-formatted with `.job` suffix
✅ **Topic/Fanout**: full control over queue name

### Exchange Types Detail

**Direct Exchange** - Queue name auto-formatted with `.job` suffix:

```rust
WorkerBuilder::new(ExchangeKind::Direct)
    .pool(pool)
    .with_exchange("order.events")
    .queue("order.created")  // routing_key
    .build(handler)
// Queue: "order.created.job"
// Binding: queue_bind("order.created.job", "order.events", "order.created")
```

**Topic Exchange** - Separate routing key and queue:

```rust
WorkerBuilder::new(ExchangeKind::Topic)
    .pool(pool)
    .with_exchange("logs")
    .routing_key("order.*")  // routing pattern
    .queue("api_logs")       // queue name
    .build(handler)
// Queue: "api_logs"
// Binding: queue_bind("api_logs", "logs", "order.*")
```

**Fanout Exchange** - Broadcast to all queues:

```rust
WorkerBuilder::new(ExchangeKind::Fanout)
    .pool(pool)
    .with_exchange("events")
    .queue("notification_q")
    .build(handler)
// Queue: "notification_q"
// Binding: queue_bind("notification_q", "events", "")
```

## Dependency Injection

This library supports dependency injection using traits:

```rust
use easy_amqp::{AmqpPublisher, Result};
use std::sync::Arc;

struct OrderService {
    publisher: Arc<dyn AmqpPublisher>,
}

impl OrderService {
    fn new(publisher: Arc<dyn AmqpPublisher>) -> Self {
        Self { publisher }
    }

    async fn create_order(&self, order: Order) -> Result<()> {
        let payload = serde_json::to_vec(&order)?;
        self.publisher.publish("orders", "order.created", &payload).await?;
        Ok(())
    }
}

let client = AmqpClient::new("amqp://guest:guest@localhost:5672".to_string(), 10)?;
let publisher: Arc<dyn AmqpPublisher> = Arc::new(client.publisher());
let order_service = OrderService::new(publisher);
```

## Examples

See `examples/` folder for usage examples:
- `publisher.rs` - Publisher with various exchange types
- `subscriber.rs` - Multi-worker with SubscriberRegistry

Run examples:
```bash
# Terminal 1 - Start subscriber first
cargo run --example subscriber

# Terminal 2 - Then publisher
cargo run --example publisher
```

## Testing

```bash
cargo test
```

## Requirements

- Rust 1.70+
- RabbitMQ server (or Docker)

## License

ISC
