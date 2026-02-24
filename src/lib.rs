pub mod error;
pub mod traits;
pub mod pool;
pub mod publisher;
pub mod subscriber;
pub mod registry;
pub mod worker;

pub use error::{AmqpError, Result};
pub use traits::AmqpPublisher;
pub use pool::{create_pool, AmqpPool, ChannelPool, AmqpConnectionManager};
pub use publisher::Publisher;
pub use subscriber::Subscriber;
pub use registry::{SubscriberRegistry, HandlerFn};
pub use worker::{WorkerBuilder, BuiltWorker, DirectWorkerBuilder, TopicWorkerBuilder, FanoutWorkerBuilder, RetryConfig};

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
        Publisher::new(self.channel_pool.clone())
    }

    pub fn channel_pool(&self) -> Arc<ChannelPool> {
        self.channel_pool.clone()
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
