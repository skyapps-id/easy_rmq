use lapin::{
    options::*, BasicProperties,
};
use crate::{error::{Result, AmqpError}, traits::AmqpPublisher, pool::ChannelPool};

pub struct Publisher {
    channel_pool: ChannelPool,
    exchange: String,
}

impl Publisher {
    pub fn new(channel_pool: ChannelPool) -> Self {
        Self {
            channel_pool,
            exchange: "amq.direct".to_string(),
        }
    }

    pub fn with_exchange(mut self, exchange: impl Into<String>) -> Self {
        self.exchange = exchange.into();
        self
    }

    pub async fn publish_json<T: serde::Serialize>(
        &self,
        routing_key: &str,
        payload: &T,
    ) -> Result<()> {
        let json = serde_json::to_vec(payload).map_err(AmqpError::SerializationError)?;
        self.publish(&self.exchange, routing_key, &json).await
    }

    pub async fn publish_json_to<T: serde::Serialize>(
        &self,
        exchange: &str,
        routing_key: &str,
        payload: &T,
    ) -> Result<()> {
        let json = serde_json::to_vec(payload).map_err(AmqpError::SerializationError)?;
        self.publish(exchange, routing_key, &json).await
    }

    pub async fn publish_text(
        &self,
        routing_key: &str,
        payload: &str,
    ) -> Result<()> {
        self.publish(&self.exchange, routing_key, payload.as_bytes()).await
    }

    pub async fn publish_text_to(
        &self,
        exchange: &str,
        routing_key: &str,
        payload: &str,
    ) -> Result<()> {
        self.publish(exchange, routing_key, payload.as_bytes()).await
    }
}

#[async_trait::async_trait]
impl AmqpPublisher for Publisher {
    async fn publish(&self, exchange: &str, routing_key: &str, payload: &[u8]) -> Result<()> {
        
        let channel = self.channel_pool.get_channel().await?;

        let props = BasicProperties::default()
            .with_delivery_mode(2);

        channel
            .basic_publish(
                exchange,
                routing_key,
                BasicPublishOptions::default(),
                payload,
                props,
            )
            .await.map_err(AmqpError::ConnectionError)?
            .await.map_err(AmqpError::ConnectionError)?;

        Ok(())
    }
}
