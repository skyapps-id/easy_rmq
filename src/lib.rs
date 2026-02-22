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
}

impl AmqpClient {
    pub fn new(uri: String, max_size: usize) -> Result<Self> {
        let pool = Arc::new(create_pool(uri, max_size)?);
        let channel_pool = Arc::new(ChannelPool::new(pool));
        
        Ok(Self {
            channel_pool,
        })
    }

    pub async fn get_channel(&self) -> Result<lapin::Channel> {
        self.channel_pool.get_channel().await
    }

    pub fn publisher(&self) -> Publisher {
        Publisher::new(self.channel_pool.as_ref().clone())
    }

    pub fn subscriber(&self, kind: lapin::ExchangeKind) -> Subscriber {
        Subscriber::new(self.channel_pool.as_ref().clone(), kind)
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
