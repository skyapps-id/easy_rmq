use easy_rmq::{AmqpClient, AmqpPublisher};
use std::sync::Arc;

#[test]
fn test_create_client() {
    let client = AmqpClient::new("amqp://guest:guest@localhost:5672".to_string(), 10);
    assert!(client.is_ok());
}

#[test]
fn test_client_has_publisher() {
    let client = AmqpClient::new("amqp://guest:guest@localhost:5672".to_string(), 10).unwrap();
    let publisher = client.publisher();
    drop(publisher);
}

#[test]
fn test_client_has_channel_pool() {
    let client = AmqpClient::new("amqp://guest:guest@localhost:5672".to_string(), 10).unwrap();
    let pool = client.channel_pool();
    drop(pool);
}

#[test]
fn test_publisher_trait() {
    let client = AmqpClient::new("amqp://guest:guest@localhost:5672".to_string(), 10).unwrap();
    let publisher: Arc<dyn AmqpPublisher> = Arc::new(client.publisher());
    drop(publisher);
}
