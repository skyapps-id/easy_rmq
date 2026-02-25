use crate::{
    error::{AmqpError, Result},
    pool::ChannelPool,
    worker::SpawnFn,
};
use futures::StreamExt;
use lapin::{Consumer, ExchangeKind, options::*, types::AMQPValue, types::FieldTable};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct Subscriber {
    channel_pool: Arc<ChannelPool>,
    exchange: String,
    exchange_type: ExchangeKind,
    auto_declare: bool,
    retry_max_retries: Option<u32>,
    retry_delay: Option<Duration>,
    prefetch: Option<u16>,
    concurrency: Option<u16>,
    spawn_fn: Option<SpawnFn>,
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
            retry_max_retries: None,
            retry_delay: None,
            prefetch: None,
            concurrency: None,
            spawn_fn: None,
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

    pub fn with_retry(mut self, max_retries: u32, delay: Duration) -> Self {
        self.retry_max_retries = Some(max_retries);
        self.retry_delay = Some(delay);
        self
    }

    pub fn with_prefetch(mut self, prefetch: u16) -> Self {
        self.prefetch = Some(prefetch);
        self
    }

    pub fn with_concurrency(mut self, concurrency: Option<u16>) -> Self {
        self.concurrency = concurrency;
        self
    }

    pub fn with_spawn_fn(mut self, spawn_fn: Option<SpawnFn>) -> Self {
        self.spawn_fn = spawn_fn;
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

    async fn setup_retry_infrastructure(&self, main_queue: &str) -> Result<Option<String>> {
        if let (Some(_max_retries), Some(delay)) = (self.retry_max_retries, self.retry_delay) {
            let retry_queue = format!("{}.retry", main_queue);
            let dlq_queue = format!("{}.dlq", main_queue);

            let delay_ms = delay.as_millis() as u32;

            let mut retry_args = FieldTable::default();
            retry_args.insert("x-dead-letter-exchange".into(), AMQPValue::LongString("".into()));
            retry_args.insert("x-dead-letter-routing-key".into(), AMQPValue::LongString(main_queue.into()));
            retry_args.insert("x-message-ttl".into(), AMQPValue::LongLongInt(delay_ms as i64));

            let retry_queue_ok = match self.try_declare_queue(&retry_queue, retry_args).await {
                Ok(_) => true,
                Err(e) if e.to_string().contains("406") || e.to_string().contains("PRECONDITION_FAILED") => {
                    tracing::warn!("⚠️  Retry queue '{}' already exists with different arguments", retry_queue);
                    false
                }
                Err(e) => return Err(e),
            };

            match self.try_declare_queue(&dlq_queue, FieldTable::default()).await {
                Ok(_) => {}
                Err(e) if e.to_string().contains("406") || e.to_string().contains("PRECONDITION_FAILED") => {
                    tracing::warn!("⚠️  DLQ '{}' already exists with different arguments", dlq_queue);
                }
                Err(e) => return Err(e),
            }

            if retry_queue_ok {
                tracing::info!("✓ Retry infrastructure created for '{}'", main_queue);
            } else {
                tracing::warn!("⚠️  Using existing retry infrastructure for '{}' (may not work as expected)", main_queue);
            }

            Ok(Some(retry_queue))
        } else {
            Ok(None)
        }
    }

    async fn try_declare_queue(&self, queue: &str, args: FieldTable) -> Result<()> {
        let channel = self.channel_pool.get_channel().await?;

        channel
            .queue_declare(
                queue,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                args,
            )
            .await
            .map_err(AmqpError::ConnectionError)?;

        Ok(())
    }

    pub async fn consume<F>(&self, queue: &str, handler: F) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        self.ensure_queue(queue).await?;

        let retry_queue = self.setup_retry_infrastructure(queue).await?;

        if let Some(num_workers) = self.concurrency {
            if let Some(spawner) = &self.spawn_fn {
                self.consume_parallel_workers(queue, handler, retry_queue, num_workers, spawner).await
            } else {
                return Err(AmqpError::ChannelError("concurrency requires parallelize() to be set".to_string()));
            }
        } else {
            self.consume_single(queue, handler, retry_queue).await
        }
    }

    async fn consume_single<F>(&self, queue: &str, handler: F, retry_queue: Option<String>) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        let channel = self.channel_pool.get_channel().await?;

        if let Some(prefetch) = self.prefetch {
            channel
                .basic_qos(prefetch, BasicQosOptions::default())
                .await
                .map_err(AmqpError::ConnectionError)?;
        }

        let consumer = channel
            .basic_consume(
                queue,
                "",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(AmqpError::ConnectionError)?;

        self.process_messages(consumer, handler, retry_queue, queue).await
    }

    async fn consume_parallel_workers<F>(&self, queue: &str, handler: F, retry_queue: Option<String>, num_workers: u16, spawner: &SpawnFn) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        let mut handles = Vec::new();
        let handler = Arc::new(handler);

        for i in 0..num_workers {
            let subscriber = self.clone();
            let queue = queue.to_string();
            let handler = Arc::clone(&handler);
            let retry_queue = retry_queue.clone();
            let spawner = spawner.clone();

            let future = async move {
                tracing::info!("Worker {} starting", i);
                let result = Self::worker_loop(subscriber, &queue, handler, retry_queue, i).await;
                if let Err(e) = &result {
                    tracing::error!("Worker {} failed: {:?}", i, e);
                }
                result
            };

            handles.push(spawner(Box::pin(future)));
        }

        for handle in handles {
            match handle.await {
                Ok(Ok(())) => {},
                Ok(Err(e)) => return Err(e),
                Err(e) => {
                    tracing::error!("Task join error: {:?}", e);
                    return Err(AmqpError::ChannelError(format!("Task join error: {}", e)));
                },
            }
        }

        Ok(())
    }

    async fn worker_loop<F>(subscriber: Subscriber, queue: &str, handler: Arc<F>, retry_queue: Option<String>, worker_id: u16) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        let channel = subscriber.channel_pool.get_channel().await?;

        if let Some(prefetch) = subscriber.prefetch {
            channel
                .basic_qos(prefetch, BasicQosOptions::default())
                .await
                .map_err(AmqpError::ConnectionError)?;
        }

        let consumer_tag = format!("{}-worker-{}-{}", queue, worker_id, std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos());

        let consumer = channel
            .basic_consume(
                queue,
                &consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(AmqpError::ConnectionError)?;

        let max_retries = subscriber.retry_max_retries.unwrap_or(0);
        Self::process_messages_static(consumer, handler, retry_queue, subscriber.channel_pool.clone(), max_retries, queue, subscriber.spawn_fn.clone()).await
    }
}

impl Subscriber {
    async fn process_messages<F>(&self, consumer: Consumer, handler: F, retry_queue: Option<String>, main_queue: &str) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        let max_retries = self.retry_max_retries.unwrap_or(0);
        let handler = Arc::new(handler);
        Self::process_messages_static(consumer, handler, retry_queue, self.channel_pool.clone(), max_retries, main_queue, self.spawn_fn.clone()).await
    }

    async fn process_messages_static<F>(mut consumer: Consumer, handler: Arc<F>, retry_queue: Option<String>, channel_pool: Arc<ChannelPool>, max_retries: u32, main_queue: &str, spawn_fn: Option<SpawnFn>) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        while let Some(delivery_result) = consumer.next().await {
            match delivery_result {
                Ok(delivery) => {
                    let data = delivery.data.clone();
                    let acker = delivery.acker;
                    let headers = delivery.properties.headers().clone();
                    let delivery_data = delivery.data.clone();

                    let retry_queue_clone = retry_queue.clone();
                    let channel_pool_clone = channel_pool.clone();
                    let main_queue_clone = main_queue.to_string();
                    let handler_clone = Arc::clone(&handler);

                    let process_future = async move {
                        match handler_clone(data) {
                            Ok(_) => {
                                acker
                                    .ack(BasicAckOptions::default())
                                    .await
                                    .map_err(AmqpError::ConnectionError)?;
                            }
                            Err(e) => {
                                tracing::error!("Handler error: {:?}", e);

                                if let Some(ref retry_q) = retry_queue_clone {
                                    let retry_count = headers
                                        .and_then(|h| h.inner().get("x-retry-count").cloned())
                                        .and_then(|v| {
                                            match v {
                                                AMQPValue::LongLongInt(n) => Some(n as u32),
                                                AMQPValue::ShortShortInt(n) => Some(n as u32),
                                                AMQPValue::ShortInt(n) => Some(n as u32),
                                                AMQPValue::LongInt(n) => Some(n as u32),
                                                _ => None,
                                            }
                                        })
                                        .unwrap_or(0);

                                    if retry_count < max_retries {
                                        let channel = channel_pool_clone.get_channel().await?;
                                        let mut new_headers = FieldTable::default();
                                        new_headers.insert("x-retry-count".into(), AMQPValue::LongLongInt((retry_count + 1) as i64));

                                        let publish_props = lapin::BasicProperties::default()
                                            .with_headers(new_headers)
                                            .with_delivery_mode(2);

                                        channel
                                            .basic_publish(
                                                "",
                                                retry_q,
                                                BasicPublishOptions::default(),
                                                &delivery_data,
                                                publish_props,
                                            )
                                            .await
                                            .map_err(AmqpError::ConnectionError)?;

                                        tracing::warn!("🔄 Message sent to retry queue (attempt {}/{})", retry_count + 1, max_retries);

                                        acker
                                            .ack(BasicAckOptions::default())
                                            .await
                                            .map_err(AmqpError::ConnectionError)?;
                                    } else {
                                        let dlq_queue = format!("{}.dlq", main_queue_clone);
                                        let channel = channel_pool_clone.get_channel().await?;

                                        channel
                                            .basic_publish(
                                                "",
                                                &dlq_queue,
                                                BasicPublishOptions::default(),
                                                &delivery_data,
                                                lapin::BasicProperties::default()
                                                    .with_delivery_mode(2),
                                            )
                                            .await
                                            .map_err(AmqpError::ConnectionError)?;

                                        tracing::error!("❌ Max retries exceeded ({}/{}), sent to DLQ: {}", retry_count, max_retries, dlq_queue);

                                        acker
                                            .ack(BasicAckOptions::default())
                                            .await
                                            .map_err(AmqpError::ConnectionError)?;
                                    }
                                } else {
                                    acker
                                        .nack(BasicNackOptions::default())
                                        .await
                                        .map_err(AmqpError::ConnectionError)?;
                                }
                            }
                        }
                        Ok(())
                    };

                    if let Some(spawner) = spawn_fn.clone() {
                        spawner(Box::pin(process_future));
                    } else {
                        process_future.await?;
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
