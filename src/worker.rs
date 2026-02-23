use crate::{ChannelPool, HandlerFn, Result, Subscriber};
use lapin::ExchangeKind;
use std::sync::Arc;

pub struct WorkerBuilder;

impl WorkerBuilder {
    pub fn new(exchange_kind: ExchangeKind) -> WorkerBuilderWithKind {
        WorkerBuilderWithKind::new(exchange_kind)
    }

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

pub struct WorkerBuilderWithKind {
    exchange_kind: ExchangeKind,
    exchange: String,
    channel_pool: Option<Arc<ChannelPool>>,
    routing_key: Option<String>,
    queue: Option<String>,
}

impl WorkerBuilderWithKind {
    pub fn new(exchange_kind: ExchangeKind) -> Self {
        let exchange = match exchange_kind {
            ExchangeKind::Direct => "amq.direct",
            ExchangeKind::Topic => "amq.topic",
            ExchangeKind::Fanout => "amq.fanout",
            _ => "amq.direct",
        }
        .to_string();

        Self {
            exchange_kind,
            exchange,
            channel_pool: None,
            routing_key: None,
            queue: None,
        }
    }

    pub fn pool(mut self, pool: Arc<ChannelPool>) -> Self {
        self.channel_pool = Some(pool);
        self
    }

    pub fn with_exchange(mut self, exchange: impl Into<String>) -> Self {
        self.exchange = exchange.into();
        self
    }

    pub fn routing_key(mut self, routing_key: impl Into<String>) -> Self {
        self.routing_key = Some(routing_key.into());
        self
    }

    pub fn queue(mut self, queue: impl Into<String>) -> Self {
        self.queue = Some(queue.into());
        self
    }

    pub fn build<F>(self, handler: F) -> BuiltWorker
    where
        F: Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static,
    {
        let pool = self.channel_pool.expect("Pool must be set with .pool()");

        let subscriber =
            Subscriber::new(pool, self.exchange_kind.clone()).with_exchange(&self.exchange);

        match self.exchange_kind {
            ExchangeKind::Direct => {
                let queue = self.queue.expect("queue required for Direct");
                BuiltWorker {
                    subscriber,
                    config: WorkerConfig::Direct {
                        queue,
                        handler: Box::new(handler),
                    },
                }
            }
            ExchangeKind::Topic => {
                let routing_key = self.routing_key.expect("routing_key required for Topic");
                let queue = self.queue.expect("queue required for Topic");
                BuiltWorker {
                    subscriber,
                    config: WorkerConfig::Topic {
                        routing_key,
                        queue,
                        handler: Box::new(handler),
                    },
                }
            }
            ExchangeKind::Fanout => {
                let queue = self.queue.expect("queue required for Fanout");
                BuiltWorker {
                    subscriber,
                    config: WorkerConfig::Fanout {
                        queue,
                        handler: Box::new(handler),
                    },
                }
            }
            _ => panic!("Unsupported exchange kind"),
        }
    }
}

pub struct BuiltWorker {
    subscriber: Subscriber,
    config: WorkerConfig,
}

pub enum WorkerConfig {
    Direct {
        queue: String,
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
    queue: String,
}

impl DirectWorkerBuilder {
    pub fn direct(channel_pool: Arc<ChannelPool>) -> Self {
        Self {
            exchange: "amq.direct".to_string(),
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
        let subscriber = Subscriber::new(self.channel_pool, lapin::ExchangeKind::Direct)
            .with_exchange(&self.exchange);

        BuiltWorker {
            subscriber,
            config: WorkerConfig::Direct {
                queue: self.queue,
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
            WorkerConfig::Direct { queue, handler } => {
                self.subscriber.direct(&queue).build(handler).await
            }
            WorkerConfig::Topic {
                routing_key,
                queue,
                handler,
            } => {
                self.subscriber
                    .topic(&routing_key, &queue)
                    .build(handler)
                    .await
            }
            WorkerConfig::Fanout { queue, handler } => {
                self.subscriber.fanout(&queue).build(handler).await
            }
        }
    }
}
