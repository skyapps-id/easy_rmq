use lapin::{
    options::*, Channel, Consumer, types::FieldTable, ExchangeKind,
};
use futures::StreamExt;
use crate::{error::{Result, AmqpError}, traits::AmqpSubscriber, pool::ChannelPool};

pub struct Subscriber {
    channel_pool: ChannelPool,
    default_exchange: String,
    auto_declare: bool,
}

impl Subscriber {
    pub fn new(channel_pool: ChannelPool, default_exchange: String) -> Self {
        Self {
            channel_pool,
            default_exchange,
            auto_declare: true,
        }
    }

    pub fn with_auto_declare(mut self, auto_declare: bool) -> Self {
        self.auto_declare = auto_declare;
        self
    }

    async fn ensure_exchange(&self, exchange: &str) -> Result<()> {
        if self.auto_declare && !exchange.is_empty() {
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
                    FieldTable::default()
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

    async fn ensure_queue(&self, queue: &str) -> Result<()> {
        if self.auto_declare {
            let channel = self.channel_pool.get_channel().await?;
            let options = QueueDeclareOptions {
                durable: true,
                ..Default::default()
            };
            
            channel
                .queue_declare(queue, options, FieldTable::default())
                .await
                .map_err(AmqpError::ConnectionError)?;
        }
        Ok(())
    }

    async fn ensure_binding(&self, queue: &str, exchange: &str, routing_key: &str) -> Result<()> {
        if self.auto_declare && !exchange.is_empty() {
            let channel = self.channel_pool.get_channel().await?;
            
            channel
                .queue_bind(
                    queue,
                    exchange,
                    routing_key,
                    QueueBindOptions::default(),
                    FieldTable::default(),
                )
                .await
                .map_err(AmqpError::ConnectionError)?;
        }
        Ok(())
    }

    pub async fn consume<F>(
        &self,
        queue: &str,
        handler: F,
    ) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        self.ensure_queue(queue).await?;
        
        let channel = self.channel_pool.get_channel().await?;

        let consumer = channel
            .basic_consume(
                queue,
                "",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await.map_err(AmqpError::ConnectionError)?;

        self.process_messages(consumer, handler).await
    }

    pub async fn consume_with_channel<F>(
        channel: Channel,
        queue: &str,
        handler: F,
    ) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        let consumer = channel
            .basic_consume(
                queue,
                "",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await.map_err(AmqpError::ConnectionError)?;

        Self::process_messages_static(consumer, handler).await
    }

    async fn process_messages<F>(
        &self,
        consumer: Consumer,
        handler: F,
    ) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        Self::process_messages_static(consumer, handler).await
    }

    async fn process_messages_static<F>(
        mut consumer: Consumer,
        handler: F,
    ) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        while let Some(delivery_result) = consumer.next().await {
            match delivery_result {
                Ok(delivery) => {
                    let data = delivery.data;
                    
                    let acker = delivery.acker;

                    match handler(data) {
                        Ok(_) => {
                            acker
                                .ack(BasicAckOptions::default())
                                .await.map_err(AmqpError::ConnectionError)?;
                        }
                        Err(e) => {
                            tracing::error!("Handler error: {:?}", e);
                            acker
                                .nack(BasicNackOptions::default())
                                .await.map_err(AmqpError::ConnectionError)?;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Consumer error: {:?}", e);
                }
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl AmqpSubscriber for Subscriber {
    async fn subscribe<F>(&self, queue: &str, routing_key: &str, handler: F) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        self.ensure_exchange(&self.default_exchange).await?;
        self.ensure_queue(queue).await?;
        self.ensure_binding(queue, &self.default_exchange, routing_key).await?;
        self.consume(queue, handler).await
    }
}
