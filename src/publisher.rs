use lapin::{
    options::*, BasicProperties, ExchangeKind,
};
use crate::{error::{Result, AmqpError}, traits::AmqpPublisher, pool::ChannelPool};

pub struct Publisher {
    channel_pool: ChannelPool,
    default_exchange: String,
}

impl Publisher {
    pub fn new(channel_pool: ChannelPool, default_exchange: String) -> Self {
        Self {
            channel_pool,
            default_exchange,
        }
    }

    async fn ensure_exchange(&self, exchange: &str) -> Result<()> {
        if !exchange.is_empty() {
            let channel = self.channel_pool.get_channel().await?;
            
            tracing::info!("Creating exchange: {}", exchange);
            
            // Try to create, ignore if already exists
            let result = channel
                .exchange_declare(
                    exchange, 
                    ExchangeKind::Direct, 
                    ExchangeDeclareOptions {
                        durable: true,
                        ..Default::default()
                    }, 
                    lapin::types::FieldTable::default()
                )
                .await;
            
            match &result {
                Ok(_) => tracing::info!("✓ Exchange '{}' created", exchange),
                Err(e) => {
                    if e.to_string().contains("406") || e.to_string().contains("PRECONDITION_FAILED") {
                        tracing::info!("✓ Exchange '{}' already exists", exchange);
                    } else {
                        tracing::error!("Failed to create exchange '{}': {:?}", exchange, e);
                        return Err(AmqpError::ConnectionError(e.clone()));
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn publish_json<T: serde::Serialize>(
        &self,
        routing_key: &str,
        payload: &T,
    ) -> Result<()> {
        let json = serde_json::to_vec(payload).map_err(AmqpError::SerializationError)?;
        self.publish(&self.default_exchange, routing_key, &json).await
    }

    pub async fn publish_text(
        &self,
        routing_key: &str,
        payload: &str,
    ) -> Result<()> {
        self.publish(&self.default_exchange, routing_key, payload.as_bytes()).await
    }
}

#[async_trait::async_trait]
impl AmqpPublisher for Publisher {
    async fn publish(&self, exchange: &str, routing_key: &str, payload: &[u8]) -> Result<()> {
        self.ensure_exchange(exchange).await?;
        
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
