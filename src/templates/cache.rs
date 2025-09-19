use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;
use chrono::{DateTime, Utc, Duration};
use redis::AsyncCommands;
use anyhow::Result;

#[derive(Clone)]
pub struct CachedTemplate {
    pub content: String,
    pub compiled_at: DateTime<Utc>,
    pub ttl_seconds: i64,
    pub version: String,
}

impl CachedTemplate {
    pub fn is_expired(&self) -> bool {
        let now = Utc::now();
        let expiry = self.compiled_at + Duration::seconds(self.ttl_seconds);
        now > expiry
    }
}

pub struct TemplateCache {
    memory_cache: Arc<RwLock<HashMap<String, CachedTemplate>>>,
    redis_client: Option<redis::aio::ConnectionManager>,
    ttl_seconds: i64,
}

impl TemplateCache {
    pub async fn new(redis_url: Option<String>, ttl_seconds: i64) -> Result<Self> {
        let redis_client = if let Some(url) = redis_url {
            let client = redis::Client::open(url)?;
            Some(redis::aio::ConnectionManager::new(client).await?)
        } else {
            None
        };

        Ok(TemplateCache {
            memory_cache: Arc::new(RwLock::new(HashMap::new())),
            redis_client,
            ttl_seconds,
        })
    }

    pub async fn get(&self, template_id: &str) -> Option<CachedTemplate> {
        // 1. Check memory cache
        {
            let cache = self.memory_cache.read().await;
            if let Some(template) = cache.get(template_id) {
                if !template.is_expired() {
                    return Some(template.clone());
                }
            }
        }

        // 2. Check Redis cache
        if let Some(ref mut redis) = self.redis_client.clone() {
            if let Ok(cached_str) = redis.get::<_, String>(format!("template:{}", template_id)).await {
                if let Ok(template) = serde_json::from_str::<CachedTemplate>(&cached_str) {
                    if !template.is_expired() {
                        // Update memory cache
                        self.memory_cache.write().await.insert(template_id.to_string(), template.clone());
                        return Some(template);
                    }
                }
            }
        }

        None
    }

    pub async fn set(&self, template_id: &str, content: String, version: String) -> Result<()> {
        let cached = CachedTemplate {
            content,
            compiled_at: Utc::now(),
            ttl_seconds: self.ttl_seconds,
            version,
        };

        // 1. Update memory cache
        self.memory_cache.write().await.insert(template_id.to_string(), cached.clone());

        // 2. Update Redis cache
        if let Some(ref mut redis) = self.redis_client.clone() {
            let serialized = serde_json::to_string(&cached)?;
            redis.set_ex(
                format!("template:{}", template_id),
                serialized,
                self.ttl_seconds as usize
            ).await?;
        }

        Ok(())
    }

    pub async fn invalidate(&self, template_id: &str) -> Result<()> {
        // Remove from memory cache
        self.memory_cache.write().await.remove(template_id);

        // Remove from Redis
        if let Some(ref mut redis) = self.redis_client.clone() {
            redis.del(format!("template:{}", template_id)).await?;
        }

        Ok(())
    }

    pub async fn invalidate_all(&self) -> Result<()> {
        // Clear memory cache
        self.memory_cache.write().await.clear();

        // Clear Redis templates
        if let Some(ref mut redis) = self.redis_client.clone() {
            let keys: Vec<String> = redis.keys("template:*").await?;
            if !keys.is_empty() {
                redis.del(keys).await?;
            }
        }

        Ok(())
    }

    pub async fn get_stats(&self) -> HashMap<String, usize> {
        let cache = self.memory_cache.read().await;
        let mut stats = HashMap::new();

        stats.insert("memory_cached".to_string(), cache.len());
        stats.insert("memory_expired".to_string(),
            cache.values().filter(|t| t.is_expired()).count());

        stats
    }
}

impl serde::Serialize for CachedTemplate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("CachedTemplate", 4)?;
        state.serialize_field("content", &self.content)?;
        state.serialize_field("compiled_at", &self.compiled_at)?;
        state.serialize_field("ttl_seconds", &self.ttl_seconds)?;
        state.serialize_field("version", &self.version)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for CachedTemplate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct CachedTemplateData {
            content: String,
            compiled_at: DateTime<Utc>,
            ttl_seconds: i64,
            version: String,
        }

        let data = CachedTemplateData::deserialize(deserializer)?;
        Ok(CachedTemplate {
            content: data.content,
            compiled_at: data.compiled_at,
            ttl_seconds: data.ttl_seconds,
            version: data.version,
        })
    }
}