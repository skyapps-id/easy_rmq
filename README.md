# easy_rmq

Rust AMQP library with connection pool, publisher, subscriber, and dependency injection support.

## Features

- **Connection Pool**: Efficiently manages AMQP connections using deadpool
- **Publisher**: Send messages to exchanges with routing keys
- **Subscriber**: Receive messages from queues with handlers
- **Worker Registry**: Register and manage multiple workers with a clean pattern
- **Auto Setup**: Automatically creates exchanges and queues
- **Retry Mechanism**: Automatic retry with delay for failed messages
- **Prefetch Control**: AMQP prefetch (QoS) configuration
- **Parallel Processing**: Configurable worker concurrency with async/blocking spawn
- **Dependency Injection**: Support for trait-based DI pattern
- **Type Safe**: Strong error handling with thiserror
- **Async**: Full async support using tokio

## Installation

```toml
[dependencies]
easy_rmq = { path = "./easy_rmq" }
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
- **Retry**: Automatic retry with delay for failed messages
- **Prefetch**: AMQP QoS control for message buffering
- **Concurrency**: Parallel worker processing
- **Full Auto-Setup**: No manual infrastructure needed

This follows AMQP best practices:
- Producer → Send to exchange (auto-created if not exists)
- Consumer → Auto-create everything + consume
- Registry → Manage multiple workers with consistent pattern

## Basic Usage

### Creating a Client

```rust
use easy_rmq::AmqpClient;

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
use easy_rmq::AmqpClient;

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
use easy_rmq::{AmqpClient, SubscriberRegistry, WorkerBuilder};
use lapin::ExchangeKind;

#[tokio::main]
async fn main() -> easy_rmq::Result<()> {
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

fn handle_order_event(data: Vec<u8>) -> easy_rmq::Result<()> {
    let msg = String::from_utf8_lossy(&data);
    println!("📦 Order: {}", msg);
    Ok(())
}

fn handle_log_event(data: Vec<u8>) -> easy_rmq::Result<()> {
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

### Advanced Worker Configuration

#### Retry Mechanism

Automatically retry failed messages with delay:

```rust
WorkerBuilder::new(ExchangeKind::Direct)
    .pool(pool)
    .with_exchange("order.events.v1")
    .queue("order.process")
    .retry(3, 5000)  // max 3 retries, 5 second delay
    .build(handler)
```

**How it works:**
- Failed messages sent to `{queue}.retry` with TTL
- After delay, message returns to original queue
- After max retries exceeded, sent to `{queue}.dlq` (Dead Letter Queue)
- Retry count tracked in message headers: `x-retry-count`

#### Prefetch (QoS) Control

Control how many messages pre-fetched from broker:

```rust
WorkerBuilder::new(ExchangeKind::Direct)
    .pool(pool)
    .queue("order.process")
    .prefetch(10)  // Buffer 10 messages
    .build(handler)
```

**Prefetch behavior:**
- Without `.concurrency()`: Messages buffered, processed sequentially 1-by-1
- With `.concurrency()`: Buffer size for parallel workers

#### Parallel Processing

Run multiple workers concurrently with controlled parallelism:

```rust
WorkerBuilder::new(ExchangeKind::Direct)
    .pool(pool)
    .queue("order.process")
    .prefetch(50)              // Buffer 50 messages
    .concurrency(10)           // Spawn 10 parallel workers
    .parallelize(tokio::task::spawn)  // Async tasks
    .build(handler)
```

**Configuration breakdown:**
- `.prefetch(N)` - AMQP prefetch count (buffer size from broker)
- `.concurrency(N)` - Number of parallel worker tasks
- `.parallelize(spawn_fn)` - Spawn function for task creation

**Spawn function options:**

```rust
// Async I/O tasks (default, good for database/HTTP calls)
.parallelize(tokio::task::spawn)

// CPU-intensive or blocking operations
.parallelize(tokio::task::spawn_blocking)
```

**Worker model:**
- Each worker runs its own consumer loop with unique consumer tag
- Workers compete for messages from the same queue
- Prefetch divides evenly among workers (e.g., prefetch=50, 10 workers → 5 per worker)

**Configuration Comparison:**

| Scenario | `.prefetch()` | `.concurrency()` | `.parallelize()` | Behavior |
|----------|---------------|------------------|------------------|----------|
| Sequential | Not set / 1 | Not set | Not set | 1 message at a time |
| Buffered | 10 | Not set | Not set | Buffer 10, process 1-by-1 |
| Parallel Async | 50 | 10 | `tokio::task::spawn` | 10 workers, async execution |
| Parallel Blocking | 50 | 10 | `tokio::task::spawn_blocking` | 10 workers, blocking threads |

#### Complete Example with All Features

```rust
WorkerBuilder::new(ExchangeKind::Topic)
    .pool(pool)
    .with_exchange("logs.v1")
    .routing_key("order.*")
    .queue("api_logs")
    .retry(2, 10000)              // 2 retries, 10s delay
    .prefetch(100)                // Buffer 100 messages
    .concurrency(20)              // 20 parallel workers
    .parallelize(tokio::task::spawn)  // Async execution
    .build(handle_log_event)
```

⚠️ **Important:** `.concurrency()` requires `.parallelize()` to be set

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
use easy_rmq::{AmqpPublisher, Result};
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
- `subscriber.rs` - Multi-worker with retry, prefetch, concurrency, and parallelize

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
