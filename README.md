# lib-amqp

Rust AMQP library dengan connection pool, publisher, subscriber, dan support untuk dependency injection.

## Fitur

- **Connection Pool**: Mengelola koneksi AMQP dengan efisien menggunakan deadpool
- **Publisher**: Mengirim pesan ke exchange dengan routing key
- **Subscriber**: Menerima pesan dari queue dengan handler
- **Auto Register**: Otomatis membuat exchange dan queue
- **Dependency Injection**: Support untuk trait-based DI pattern
- **Type Safe**: Error handling yang kuat dengan thiserror
- **Async**: Full async support menggunakan tokio

## Installation

```toml
[dependencies]
easy_amqp = { path = "./lib-amqp" }
```

## Quick Start

### 1. Jalankan RabbitMQ

```bash
docker run -d --name rabbitmq \
  -p 5672:5672 \
  -p 15672:15672 \
  rabbitmq:3-management
```

### 2. Subscriber Example (Run First!)

Buka terminal 1 - Subscriber setup queue & binding:
```bash
cargo run --example subscriber
```

### 3. Publisher Example (Run Second)

Buka terminal 2:
```bash
cargo run --example publisher
```

Press `Ctrl+C` pada subscriber untuk graceful shutdown.

## Architecture & Best Practices

🎯 **Simple & Clean:**
- **Default Exchange**: `amq.direct` (RabbitMQ built-in)
- **Publisher**: Auto-create exchange + kirim pesan
- **Subscriber**: Auto-create exchange + queue + binding
- **Full Auto-Setup**: Tidak perlu manual infrastructure

Ini mengikuti AMQP best practice:
- Producer → Kirim ke exchange (auto-created jika belum ada)
- Consumer → Auto-create semua + consume

## Penggunaan Dasar

### Membuat Client

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

Publisher **simple** - kirim ke default exchange:

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

✅ **Auto kirim ke default exchange** (`amq.direct`)  
✅ **Auto-create exchange** jika belum ada (durable)  
✅ **Tidak perlu manual setup**

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

✅ **Explicit** - tipe exchange jelas dari parameter  
✅ **Fleksibel** - Direct, Topic, Fanout, Headers  
✅ **Auto-create** exchange dengan tipe yang sesuai

### Subscriber

Subscriber **auto-create queue** dengan nama terformat:

```rust
use easy_amqp::{AmqpClient, AmqpSubscriber};
use lapin::ExchangeKind;

let client = AmqpClient::new("amqp://guest:guest@localhost:5672".to_string(), 10)?;

// Direct exchange - format: routing_key + "worker"
let sub = client.subscriber().with_exchange("sales", ExchangeKind::Direct);
sub.subscribe("job", "order.process", handler).await?;
// Queue: order.processworker

// Topic exchange - format: routing_key.queue
let sub = client.subscriber().with_exchange("logs", ExchangeKind::Topic);
sub.subscribe("api_logs", "order.*", handler).await?;
// Queue: order.*.api_logs

// Fanout exchange - format: queue
let sub = client.subscriber().with_exchange("broadcast", ExchangeKind::Fanout);
sub.subscribe("queue1", "", handler).await?;
// Queue: queue1
```

**Format Queue:**
- **Direct**: `routing_key + "worker"` → `order.processworker`
- **Topic**: `routing_key.queue` → `order.*.api_queue`
- **Fanout**: `queue` → `queue1`

✅ **Auto-created** berdasarkan tipe exchange  
✅ **Simpel untuk Direct** - tidak perlu tulis nama queue  
✅ **Jelas untuk Topic/Fanout** - grouping jelas

### Multiple Exchanges

```rust
use lapin::ExchangeKind;

let client = AmqpClient::new("...", 10)?;

// Subscriber 1 - Direct exchange (queue: routing_key + "worker")
let sub1 = client.subscriber().with_exchange("sales", ExchangeKind::Direct);
sub1.subscribe("job", "order.process", handler1).await?;
// Queue: order.processworker

// Subscriber 2 - Topic exchange (queue: routing_key.queue)
let sub2 = client.subscriber().with_exchange("logs", ExchangeKind::Topic);
sub2.subscribe("api_queue", "order.*", handler2).await?;
// Queue: order.*.api_queue

// Subscriber 3 - Fanout exchange (queue: queue saja)
let sub3 = client.subscriber().with_exchange("broadcast", ExchangeKind::Fanout);
sub3.subscribe("queue1", "", handler3).await?;
// Queue: queue1
```

**Format Queue per Tipe:**
- **Direct**: `routing_key + "worker"` → `order.processworker`
- **Topic**: `routing_key.queue` → `order.*.api_queue`
- **Fanout**: `queue` → `queue1`

✅ **Simple untuk Direct** - parameter queue diabaikan  
✅ **Explicit untuk Topic/Fanout** - grouping jelas  
✅ **Auto-create** exchange + queue + binding

## Dependency Injection

Library ini support dependency injection menggunakan traits:

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

## Queue Management

Manual setup exchange dan queue:

```rust
let client = AmqpClient::new("amqp://guest:guest@localhost:5672".to_string(), 10)?;

// Membuat queue
client.create_queue("my_queue", true).await?;

// Membuat exchange
client.create_exchange("my_exchange", lapin::ExchangeKind::Direct).await?;

// Binding queue ke exchange
client.bind_queue("my_queue", "my_exchange", "routing.key").await?;
```

## Examples

Lihat folder `examples/` untuk contoh penggunaan:
- `publisher.rs` - Publisher murni (hanya kirim pesan)
- `subscriber.rs` - Subscriber dengan full setup & graceful shutdown
- `di_example.rs` - Contoh dependency injection pattern

Jalankan example:
```bash
# Terminal 1 - Start subscriber dulu
cargo run --example subscriber

# Terminal 2 - Lalu publisher
cargo run --example publisher
```

## Testing

```bash
cargo test
```

## Requirements

- Rust 1.70+
- RabbitMQ server (atau Docker)

## License

ISC
