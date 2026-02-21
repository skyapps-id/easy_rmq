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

### Custom Default Exchange

```rust
// Set default exchange di client level
let client = AmqpClient::new("...", 10)?
    .with_exchange("my_exchange");

let publisher = client.publisher();
// Semua publish otomatis ke "my_exchange"
```

### Subscriber

Subscriber **auto-create queue** + **bind**:

```rust
use easy_amqp::{AmqpClient, AmqpSubscriber};

let client = AmqpClient::new("amqp://guest:guest@localhost:5672".to_string(), 10)?;

let subscriber = client.subscriber();

// Simple: queue, routing_key, handler
subscriber.subscribe(
    "order_queue",      // Auto-created (durable)
    "order.created",    // Routing key
    |data: Vec<u8>| {
        let message = String::from_utf8_lossy(&data);
        println!("Received: {}", message);
        Ok(())
    }
).await?;
```

✅ **Exchange auto-created** (durable) jika belum ada  
✅ **Queue auto-created** (durable)  
✅ **Auto-bind** queue ke default exchange dengan routing key  
✅ **Full auto-setup** - tidak perlu manual infrastructure

### Disable Auto Declare

```rust
let subscriber = client.subscriber()
    .with_auto_declare(false);
```

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
