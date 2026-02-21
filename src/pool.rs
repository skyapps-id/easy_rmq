use deadpool::managed::{Manager, Pool};
use lapin::{Connection, ConnectionProperties, Channel};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::error::{AmqpError, Result};

pub struct AmqpConnectionManager {
    uri: String,
}

impl AmqpConnectionManager {
    pub fn new(uri: String) -> Self {
        Self { uri }
    }
}

#[async_trait::async_trait]
impl Manager for AmqpConnectionManager {
    type Type = Connection;
    type Error = lapin::Error;

    async fn create(&self) -> std::result::Result<Self::Type, Self::Error> {
        let opts = ConnectionProperties::default()
            .with_executor(tokio_executor_trait::Tokio::current())
            .with_reactor(tokio_reactor_trait::Tokio);
        
        Connection::connect(&self.uri, opts).await
    }

    async fn recycle(&self, conn: &mut Self::Type, _metrics: &deadpool::managed::Metrics) -> deadpool::managed::RecycleResult<Self::Error> {
        if conn.status().connected() {
            Ok(())
        } else {
            Err(deadpool::managed::RecycleError::Backend(
                lapin::Error::IOError(std::sync::Arc::new(std::io::Error::new(
                    std::io::ErrorKind::ConnectionReset,
                    "Connection not connected",
                )))
            ))
        }
    }
}

pub type AmqpPool = Pool<AmqpConnectionManager>;

pub fn create_pool(uri: String, max_size: usize) -> Result<AmqpPool> {
    let manager = AmqpConnectionManager::new(uri);
    let pool = Pool::builder(manager)
        .max_size(max_size)
        .build()
        .map_err(|e| AmqpError::PoolError(e.to_string()))?;
    
    Ok(pool)
}

#[derive(Clone)]
pub struct ChannelPool {
    pool: Arc<AmqpPool>,
    channel: Arc<Mutex<Option<Channel>>>,
}

impl ChannelPool {
    pub fn new(pool: Arc<AmqpPool>) -> Self {
        Self {
            pool,
            channel: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn get_channel(&self) -> Result<Channel> {
        let mut cached = self.channel.lock().await;
        
        if let Some(channel) = cached.as_ref() {
            if channel.status().connected() {
                return Ok(channel.clone());
            }
        }

        let conn = self.pool.get().await
            .map_err(|e| AmqpError::PoolError(e.to_string()))?;
        
        let channel = conn.create_channel().await.map_err(AmqpError::ConnectionError)?;
        *cached = Some(channel.clone());
        
        Ok(channel)
    }
}
