//! Kafka integration module
//!
//! Handles all Kafka consumer and producer operations

use anyhow::Result;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::sync::Arc;
use tracing::{error, info, warn};

pub mod consumer;
pub mod producer;
pub mod handlers;

/// Kafka client wrapper
pub struct KafkaClient {
    consumer: Arc<StreamConsumer>,
    producer: Arc<FutureProducer>,
    config: crate::config::KafkaConfig,
}

impl KafkaClient {
    /// Create a new Kafka client
    pub fn new(config: crate::config::KafkaConfig) -> Result<Self> {
        let mut client_config = ClientConfig::new();

        client_config
            .set("bootstrap.servers", &config.brokers)
            .set("group.id", &config.group_id)
            .set("client.id", &config.client_id)
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest");

        // Add security configuration if provided
        if let Some(security) = &config.security {
            client_config.set("security.protocol", &security.protocol);

            if let Some(mechanism) = &security.sasl_mechanism {
                client_config.set("sasl.mechanism", mechanism);
            }

            if let Some(username) = &security.sasl_username {
                client_config.set("sasl.username", username);
            }

            if let Some(password) = &security.sasl_password {
                client_config.set("sasl.password", password);
            }
        }

        let consumer: StreamConsumer = client_config.clone().create()?;
        let producer: FutureProducer = client_config.create()?;

        Ok(Self {
            consumer: Arc::new(consumer),
            producer: Arc::new(producer),
            config,
        })
    }

    /// Get consumer reference
    pub fn consumer(&self) -> Arc<StreamConsumer> {
        self.consumer.clone()
    }

    /// Get producer reference
    pub fn producer(&self) -> Arc<FutureProducer> {
        self.producer.clone()
    }

    /// Subscribe to configured topics
    pub fn subscribe(&self) -> Result<()> {
        let topics = vec![
            self.config.topics.document_request.as_str(),
            self.config.topics.document_batch.as_str(),
            self.config.topics.notification_dispatch.as_str(),
        ];

        self.consumer.subscribe(&topics)?;
        info!("Subscribed to Kafka topics: {:?}", topics);

        Ok(())
    }
}