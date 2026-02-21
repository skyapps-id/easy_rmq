pub mod error;
pub mod traits;
pub mod pool;
pub mod publisher;
pub mod subscriber;

pub use error::{AmqpError, Result};
pub use traits::{AmqpPublisher, AmqpSubscriber};
pub use pool::{create_pool, AmqpPool, ChannelPool, AmqpConnectionManager};
pub use publisher::Publisher;
pub use subscriber::Subscriber;

use std::sync::Arc;

pub struct AmqpClient {
    channel_pool: Arc<ChannelPool>,
    default_exchange: String,
}

impl AmqpClient {
    pub fn new(uri: String, max_size: usize) -> Result<Self> {
        let pool = Arc::new(create_pool(uri, max_size)?);
        let channel_pool = Arc::new(ChannelPool::new(pool));
        
        Ok(Self {
            channel_pool,
            default_exchange: "amq.direct".to_string(),
        })
    }

    pub fn with_exchange(mut self, exchange: impl Into<String>) -> Self {
        self.default_exchange = exchange.into();
        self
    }

    pub async fn get_channel(&self) -> Result<lapin::Channel> {
        self.channel_pool.get_channel().await
    }

    pub fn publisher(&self) -> Publisher {
        Publisher::new(self.channel_pool.as_ref().clone(), self.default_exchange.clone())
    }

    pub fn subscriber(&self) -> Subscriber {
        Subscriber::new(self.channel_pool.as_ref().clone(), self.default_exchange.clone())
    }

    pub async fn create_queue(&self, queue: &str, durable: bool) -> Result<()> {
        let channel = self.channel_pool.get_channel().await?;
        let options = lapin::options::QueueDeclareOptions {
            durable,
            ..Default::default()
        };
        channel.queue_declare(queue, options, lapin::types::FieldTable::default()).await.map_err(AmqpError::ConnectionError)?;
        Ok(())
    }

    pub async fn create_exchange(&self, exchange: &str, exchange_type: lapin::ExchangeKind) -> Result<()> {
        let channel = self.channel_pool.get_channel().await?;
        let options = lapin::options::ExchangeDeclareOptions::default();
        channel.exchange_declare(exchange, exchange_type, options, lapin::types::FieldTable::default()).await.map_err(AmqpError::ConnectionError)?;
        Ok(())
    }

    pub async fn bind_queue(&self, queue: &str, exchange: &str, routing_key: &str) -> Result<()> {
        let channel = self.channel_pool.get_channel().await?;
        channel.queue_bind(queue, exchange, routing_key, lapin::options::QueueBindOptions::default(), lapin::types::FieldTable::default()).await.map_err(AmqpError::ConnectionError)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_client() {
        let client = AmqpClient::new("amqp://guest:guest@localhost:5672".to_string(), 10);
        assert!(client.is_ok());
    }
}
