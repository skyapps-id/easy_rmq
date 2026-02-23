use async_trait::async_trait;
use crate::error::Result;

#[async_trait]
pub trait AmqpPublisher: Send + Sync {
    async fn publish(&self, exchange: &str, routing_key: &str, payload: &[u8]) -> Result<()>;
}
