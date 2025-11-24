//! Kafka Admin client for topic management
//!
//! Create, delete, and verify Kafka topics

use anyhow::{Result, Context};
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::metadata::Metadata;
use std::time::Duration;
use tracing::{info, warn, error};

/// Kafka topic configuration
#[derive(Debug, Clone)]
pub struct TopicConfig {
    pub name: String,
    pub partitions: i32,
    pub replication_factor: i32,
}

impl TopicConfig {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            partitions: 3,
            replication_factor: 1,
        }
    }

    pub fn with_partitions(mut self, partitions: i32) -> Self {
        self.partitions = partitions;
        self
    }
}

/// Kafka Admin client for managing topics
pub struct KafkaAdmin {
    client: AdminClient<DefaultClientContext>,
    brokers: String,
}

impl KafkaAdmin {
    /// Create new Kafka admin client
    pub fn new(brokers: &str) -> Result<Self> {
        let client: AdminClient<DefaultClientContext> = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .create()
            .context("Failed to create Kafka admin client")?;

        Ok(Self {
            client,
            brokers: brokers.to_string(),
        })
    }

    /// Check if Kafka is reachable
    pub async fn health_check(&self) -> Result<bool> {
        match self.get_metadata(Duration::from_secs(5)).await {
            Ok(_) => Ok(true),
            Err(e) => {
                warn!("Kafka health check failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Get cluster metadata
    pub async fn get_metadata(&self, timeout: Duration) -> Result<Metadata> {
        let metadata = self.client
            .inner()
            .fetch_metadata(None, timeout)
            .context("Failed to fetch Kafka metadata")?;

        Ok(metadata)
    }

    /// List all topics
    pub async fn list_topics(&self) -> Result<Vec<String>> {
        let metadata = self.get_metadata(Duration::from_secs(10)).await?;

        let topics: Vec<String> = metadata
            .topics()
            .iter()
            .map(|t| t.name().to_string())
            .filter(|name| !name.starts_with("__")) // Filter internal topics
            .collect();

        Ok(topics)
    }

    /// Check if a topic exists
    pub async fn topic_exists(&self, topic: &str) -> Result<bool> {
        let topics = self.list_topics().await?;
        Ok(topics.contains(&topic.to_string()))
    }

    /// Create a topic if it doesn't exist
    pub async fn create_topic(&self, config: &TopicConfig) -> Result<bool> {
        if self.topic_exists(&config.name).await? {
            info!("Topic '{}' already exists", config.name);
            return Ok(false);
        }

        let new_topic = NewTopic::new(
            &config.name,
            config.partitions,
            TopicReplication::Fixed(config.replication_factor),
        );

        let opts = AdminOptions::new().operation_timeout(Some(Duration::from_secs(30)));

        let results = self.client.create_topics(&[new_topic], &opts).await?;

        for result in results {
            match result {
                Ok(name) => {
                    info!("Created topic: {}", name);
                }
                Err((name, err)) => {
                    error!("Failed to create topic {}: {}", name, err);
                    anyhow::bail!("Failed to create topic {}: {}", name, err);
                }
            }
        }

        Ok(true)
    }

    /// Create multiple topics
    pub async fn create_topics(&self, configs: &[TopicConfig]) -> Result<Vec<String>> {
        let mut created = Vec::new();

        for config in configs {
            if self.create_topic(config).await? {
                created.push(config.name.clone());
            }
        }

        Ok(created)
    }

    /// Ensure all required topics exist
    pub async fn ensure_topics(&self, topics: &crate::config::KafkaTopics) -> Result<()> {
        let required_topics = vec![
            TopicConfig::new(&topics.document_request).with_partitions(3),
            TopicConfig::new(&topics.document_batch).with_partitions(3),
            TopicConfig::new(&topics.notification_dispatch).with_partitions(3),
            TopicConfig::new(&topics.events).with_partitions(6),
            TopicConfig::new(&topics.dlq).with_partitions(1),
        ];

        info!("Ensuring {} Kafka topics exist...", required_topics.len());

        for config in &required_topics {
            match self.create_topic(config).await {
                Ok(created) => {
                    if created {
                        info!("✅ Created topic: {}", config.name);
                    } else {
                        info!("✓ Topic exists: {}", config.name);
                    }
                }
                Err(e) => {
                    error!("❌ Failed to create topic {}: {}", config.name, e);
                    return Err(e);
                }
            }
        }

        info!("All required topics are ready");
        Ok(())
    }

    /// Delete a topic
    pub async fn delete_topic(&self, topic: &str) -> Result<()> {
        let opts = AdminOptions::new().operation_timeout(Some(Duration::from_secs(30)));

        let results = self.client.delete_topics(&[topic], &opts).await?;

        for result in results {
            match result {
                Ok(name) => {
                    info!("Deleted topic: {}", name);
                }
                Err((name, err)) => {
                    warn!("Failed to delete topic {}: {}", name, err);
                }
            }
        }

        Ok(())
    }

    /// Get topic info
    pub async fn get_topic_info(&self, topic: &str) -> Result<TopicInfo> {
        let metadata = self.get_metadata(Duration::from_secs(10)).await?;

        for t in metadata.topics() {
            if t.name() == topic {
                return Ok(TopicInfo {
                    name: t.name().to_string(),
                    partitions: t.partitions().len() as i32,
                    error: t.error().map(|e| format!("{:?}", e)),
                });
            }
        }

        anyhow::bail!("Topic '{}' not found", topic)
    }

    /// Get cluster info
    pub async fn get_cluster_info(&self) -> Result<ClusterInfo> {
        let metadata = self.get_metadata(Duration::from_secs(10)).await?;

        Ok(ClusterInfo {
            brokers: metadata.brokers().iter().map(|b| {
                BrokerInfo {
                    id: b.id(),
                    host: b.host().to_string(),
                    port: b.port() as u16,
                }
            }).collect(),
            topics_count: metadata.topics().iter().filter(|t| !t.name().starts_with("__")).count(),
        })
    }
}

/// Topic information
#[derive(Debug, Clone)]
pub struct TopicInfo {
    pub name: String,
    pub partitions: i32,
    pub error: Option<String>,
}

/// Cluster information
#[derive(Debug, Clone)]
pub struct ClusterInfo {
    pub brokers: Vec<BrokerInfo>,
    pub topics_count: usize,
}

/// Broker information
#[derive(Debug, Clone)]
pub struct BrokerInfo {
    pub id: i32,
    pub host: String,
    pub port: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_config() {
        let config = TopicConfig::new("test-topic")
            .with_partitions(6);

        assert_eq!(config.name, "test-topic");
        assert_eq!(config.partitions, 6);
        assert_eq!(config.replication_factor, 1);
    }
}
