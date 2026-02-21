use async_trait::async_trait;
use crate::error::Result;

#[async_trait]
pub trait AmqpPublisher: Send + Sync {
    async fn publish(&self, exchange: &str, routing_key: &str, payload: &[u8]) -> Result<()>;
}

#[async_trait]
pub trait AmqpSubscriber: Send + Sync {
    async fn subscribe<F>(&self, queue: &str, routing_key: &str, handler: F) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static;
}
