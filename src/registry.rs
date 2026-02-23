use crate::{Result, BuiltWorker};

pub type HandlerFn = Box<dyn Fn(Vec<u8>) -> Result<()> + Send + Sync + 'static>;

pub struct SubscriberRegistry {
    workers: Vec<BuiltWorker>,
}

impl SubscriberRegistry {
    pub fn new() -> Self {
        Self {
            workers: Vec::new(),
        }
    }

    pub fn add(mut self, worker: BuiltWorker) -> Self {
        self.workers.push(worker);
        self
    }

    pub async fn run(self) -> Result<()> {
        let mut handles = Vec::new();

        for (idx, worker) in self.workers.into_iter().enumerate() {
            let handle = tokio::spawn(async move {
                if let Err(e) = worker.run().await {
                    tracing::error!("Worker {} error: {:?}", idx, e);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.map_err(|e| crate::error::AmqpError::ChannelError(e.to_string()))?;
        }

        Ok(())
    }
}

impl Default for SubscriberRegistry {
    fn default() -> Self {
        Self::new()
    }
}
