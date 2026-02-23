use crate::{
    error::{AmqpError, Result},
    pool::ChannelPool,
};
use futures::StreamExt;
use lapin::{Consumer, ExchangeKind, options::*, types::FieldTable};
use std::sync::Arc;

#[derive(Clone)]
pub struct Subscriber {
    channel_pool: Arc<ChannelPool>,
    exchange: String,
    exchange_type: ExchangeKind,
    auto_declare: bool,
}

pub struct DirectSubscribeBuilder {
    inner: Arc<Subscriber>,
    queue: String,
    routing_key: String,
}

pub struct TopicSubscribeBuilder {
    inner: Arc<Subscriber>,
    routing_key: String,
    queue: String,
}

pub struct FanoutSubscribeBuilder {
    inner: Arc<Subscriber>,
    queue: String,
}

impl Subscriber {
    pub fn new(channel_pool: Arc<ChannelPool>, exchange_type: ExchangeKind) -> Self {
        let exchange = match exchange_type {
            ExchangeKind::Direct => "amq.direct",
            ExchangeKind::Topic => "amq.topic",
            ExchangeKind::Fanout => "amq.fanout",
            _ => "amq.direct",
        }
        .to_string();

        Self {
            channel_pool,
            exchange,
            exchange_type,
            auto_declare: true,
        }
    }

    pub fn with_exchange(mut self, exchange: impl Into<String>) -> Self {
        self.exchange = exchange.into();
        self
    }

    pub fn with_auto_declare(mut self, auto_declare: bool) -> Self {
        self.auto_declare = auto_declare;
        self
    }

    pub fn direct(self, routing_key: impl Into<String>) -> DirectSubscribeBuilder {
        let routing_key = routing_key.into();
        let queue = format!("{}.job", routing_key);
        DirectSubscribeBuilder {
            inner: Arc::new(self),
            queue,
            routing_key,
        }
    }

    pub fn topic(
        self,
        routing_key: impl Into<String>,
        queue: impl Into<String>,
    ) -> TopicSubscribeBuilder {
        TopicSubscribeBuilder {
            inner: Arc::new(self),
            routing_key: routing_key.into(),
            queue: queue.into(),
        }
    }

    pub fn fanout(self, queue: impl Into<String>) -> FanoutSubscribeBuilder {
        FanoutSubscribeBuilder {
            inner: Arc::new(self),
            queue: queue.into(),
        }
    }

    async fn ensure_exchange(&self, exchange: &str) -> Result<()> {
        if self.auto_declare && !exchange.is_empty() {
            let channel = self.channel_pool.get_channel().await?;
            let exchange_type = self.exchange_type.clone();

            tracing::info!("Creating exchange: {} ({:?})", exchange, exchange_type);

            // Try to create, ignore if already exists
            let result = channel
                .exchange_declare(
                    exchange,
                    exchange_type,
                    ExchangeDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await;

            match &result {
                Ok(_) => tracing::info!("✓ Exchange '{}' created", exchange),
                Err(e) => {
                    if e.to_string().contains("406")
                        || e.to_string().contains("PRECONDITION_FAILED")
                    {
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

    pub async fn consume<F>(&self, queue: &str, handler: F) -> Result<()>
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
            .await
            .map_err(AmqpError::ConnectionError)?;

        self.process_messages(consumer, handler).await
    }
}

impl DirectSubscribeBuilder {
    pub async fn build<F>(self, handler: F) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        self.inner.ensure_exchange(&self.inner.exchange).await?;
        self.inner.ensure_queue(&self.queue).await?;
        self.inner
            .ensure_binding(&self.queue, &self.inner.exchange, &self.routing_key)
            .await?;
        self.inner.consume(&self.queue, handler).await
    }
}

impl TopicSubscribeBuilder {
    pub async fn build<F>(self, handler: F) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        self.inner.ensure_exchange(&self.inner.exchange).await?;
        self.inner.ensure_queue(&self.queue).await?;
        self.inner
            .ensure_binding(&self.queue, &self.inner.exchange, &self.routing_key)
            .await?;
        self.inner.consume(&self.queue, handler).await
    }
}

impl FanoutSubscribeBuilder {
    pub async fn build<F>(self, handler: F) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        self.inner.ensure_exchange(&self.inner.exchange).await?;
        self.inner.ensure_queue(&self.queue).await?;
        self.inner
            .ensure_binding(&self.queue, &self.inner.exchange, "")
            .await?;
        self.inner.consume(&self.queue, handler).await
    }
}

impl Subscriber {
    async fn process_messages<F>(&self, consumer: Consumer, handler: F) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        Self::process_messages_static(consumer, handler).await
    }

    async fn process_messages_static<F>(mut consumer: Consumer, handler: F) -> Result<()>
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
                                .await
                                .map_err(AmqpError::ConnectionError)?;
                        }
                        Err(e) => {
                            tracing::error!("Handler error: {:?}", e);
                            acker
                                .nack(BasicNackOptions::default())
                                .await
                                .map_err(AmqpError::ConnectionError)?;
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
