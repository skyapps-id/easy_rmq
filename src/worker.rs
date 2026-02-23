use crate::{Result, Subscriber, ChannelPool, HandlerFn};
use std::sync::Arc;

pub struct WorkerBuilder;

impl WorkerBuilder {
    pub fn direct(channel_pool: Arc<ChannelPool>) -> DirectWorkerBuilder {
        DirectWorkerBuilder::direct(channel_pool)
    }

    pub fn topic(channel_pool: Arc<ChannelPool>) -> TopicWorkerBuilder {
        TopicWorkerBuilder::topic(channel_pool)
    }

    pub fn fanout(channel_pool: Arc<ChannelPool>) -> FanoutWorkerBuilder {
        FanoutWorkerBuilder::fanout(channel_pool)
    }
}

pub struct BuiltWorker {
    subscriber: Subscriber,
    config: WorkerConfig,
}

pub enum WorkerConfig {
    Direct {
        routing_key: String,
        handler: HandlerFn,
    },
    Topic {
        routing_key: String,
        queue: String,
        handler: HandlerFn,
    },
    Fanout {
        queue: String,
        handler: HandlerFn,
    },
}

pub struct DirectWorkerBuilder {
    exchange: String,
    channel_pool: Arc<ChannelPool>,
    routing_key: String,
}

impl DirectWorkerBuilder {
    pub fn direct(channel_pool: Arc<ChannelPool>) -> Self {
        Self {
            exchange: "amq.direct".to_string(),
            channel_pool,
            routing_key: String::new(),
        }
    }
    pub fn with_exchange(mut self, exchange: impl Into<String>) -> Self {
        self.exchange = exchange.into();
        self
    }

    pub fn queue(mut self, routing_key: impl Into<String>) -> Self {
        self.routing_key = routing_key.into();
        self
    }

    pub fn build<F>(self, handler: F) -> BuiltWorker
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        let subscriber = Subscriber::new(self.channel_pool, lapin::ExchangeKind::Direct)
            .with_exchange(&self.exchange);

        BuiltWorker {
            subscriber,
            config: WorkerConfig::Direct {
                routing_key: self.routing_key,
                handler: Box::new(handler),
            },
        }
    }
}

pub struct TopicWorkerBuilder {
    exchange: String,
    channel_pool: Arc<ChannelPool>,
    routing_key: String,
    queue: String,
}

impl TopicWorkerBuilder {
    pub fn topic(channel_pool: Arc<ChannelPool>) -> Self {
        Self {
            exchange: "amq.topic".to_string(),
            channel_pool,
            routing_key: String::new(),
            queue: String::new(),
        }
    }
    pub fn with_exchange(mut self, exchange: impl Into<String>) -> Self {
        self.exchange = exchange.into();
        self
    }

    pub fn queue(mut self, routing_key: impl Into<String>, queue: impl Into<String>) -> Self {
        self.routing_key = routing_key.into();
        self.queue = queue.into();
        self
    }

    pub fn build<F>(self, handler: F) -> BuiltWorker
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        let subscriber = Subscriber::new(self.channel_pool, lapin::ExchangeKind::Topic)
            .with_exchange(&self.exchange);

        BuiltWorker {
            subscriber,
            config: WorkerConfig::Topic {
                routing_key: self.routing_key,
                queue: self.queue,
                handler: Box::new(handler),
            },
        }
    }
}

pub struct FanoutWorkerBuilder {
    exchange: String,
    channel_pool: Arc<ChannelPool>,
    queue: String,
}

impl FanoutWorkerBuilder {
    pub fn fanout(channel_pool: Arc<ChannelPool>) -> Self {
        Self {
            exchange: "amq.fanout".to_string(),
            channel_pool,
            queue: String::new(),
        }
    }
    pub fn with_exchange(mut self, exchange: impl Into<String>) -> Self {
        self.exchange = exchange.into();
        self
    }

    pub fn queue(mut self, queue: impl Into<String>) -> Self {
        self.queue = queue.into();
        self
    }

    pub fn build<F>(self, handler: F) -> BuiltWorker
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        let subscriber = Subscriber::new(self.channel_pool, lapin::ExchangeKind::Fanout)
            .with_exchange(&self.exchange);

        BuiltWorker {
            subscriber,
            config: WorkerConfig::Fanout {
                queue: self.queue,
                handler: Box::new(handler),
            },
        }
    }
}

impl BuiltWorker {
    pub async fn run(self) -> Result<()> {
        match self.config {
            WorkerConfig::Direct { routing_key, handler } => {
                self.subscriber.direct(&routing_key).build(handler).await
            }
            WorkerConfig::Topic { routing_key, queue, handler } => {
                self.subscriber.topic(&routing_key, &queue).build(handler).await
            }
            WorkerConfig::Fanout { queue, handler } => {
                self.subscriber.fanout(&queue).build(handler).await
            }
        }
    }
}
