//! In-memory cache implementation using dashmap
//!
//! Provides fast, thread-safe caching for templates, configurations, and temporary data

use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};
use anyhow::Result;

/// Cache entry with expiration
#[derive(Clone, Debug)]
struct CacheEntry<T> {
    value: T,
    expires_at: Option<Instant>,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Option<Duration>) -> Self {
        let expires_at = ttl.map(|d| Instant::now() + d);
        Self { value, expires_at }
    }

    fn is_expired(&self) -> bool {
        self.expires_at
            .map(|exp| Instant::now() > exp)
            .unwrap_or(false)
    }
}

/// In-memory cache using DashMap
#[derive(Clone)]
pub struct InMemoryCache<T: Clone + Send + Sync + 'static> {
    store: Arc<DashMap<String, CacheEntry<T>>>,
    default_ttl: Option<Duration>,
}

impl<T: Clone + Send + Sync + 'static> InMemoryCache<T> {
    /// Create new cache with optional default TTL
    pub fn new(default_ttl: Option<Duration>) -> Self {
        Self {
            store: Arc::new(DashMap::new()),
            default_ttl,
        }
    }

    /// Get value from cache
    pub fn get(&self, key: &str) -> Option<T> {
        let entry = self.store.get(key)?;

        if entry.is_expired() {
            self.store.remove(key);
            return None;
        }

        Some(entry.value.clone())
    }

    /// Set value in cache with optional TTL
    pub fn set(&self, key: String, value: T, ttl: Option<Duration>) {
        let ttl = ttl.or(self.default_ttl);
        let entry = CacheEntry::new(value, ttl);
        self.store.insert(key, entry);
    }

    /// Remove value from cache
    pub fn remove(&self, key: &str) -> Option<T> {
        self.store.remove(key).map(|(_, entry)| entry.value)
    }

    /// Check if key exists and is not expired
    pub fn contains(&self, key: &str) -> bool {
        match self.store.get(key) {
            Some(entry) => !entry.is_expired(),
            None => false,
        }
    }

    /// Clear all expired entries
    pub fn clear_expired(&self) {
        let keys_to_remove: Vec<String> = self.store
            .iter()
            .filter(|entry| entry.value().is_expired())
            .map(|entry| entry.key().clone())
            .collect();

        for key in keys_to_remove {
            self.store.remove(&key);
        }
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.store.clear();
    }

    /// Get cache size
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}

/// Template cache for storing compiled templates
pub type TemplateCache = InMemoryCache<Arc<String>>;

/// URL cache for storing signed URLs
pub type UrlCache = InMemoryCache<String>;

/// Configuration cache
pub type ConfigCache = InMemoryCache<Arc<serde_json::Value>>;

/// Multi-tenant cache manager
pub struct CacheManager {
    template_cache: TemplateCache,
    url_cache: UrlCache,
    config_cache: ConfigCache,
    ncf_cache: InMemoryCache<NCFCacheEntry>,
}

impl CacheManager {
    pub fn new() -> Self {
        Self {
            template_cache: InMemoryCache::new(Some(Duration::from_secs(3600))), // 1 hour
            url_cache: InMemoryCache::new(Some(Duration::from_secs(900))), // 15 minutes
            config_cache: InMemoryCache::new(Some(Duration::from_secs(300))), // 5 minutes
            ncf_cache: InMemoryCache::new(Some(Duration::from_secs(60))), // 1 minute for NCF sequences
        }
    }

    /// Get template from cache
    pub fn get_template(&self, tenant_id: &str, template_name: &str) -> Option<Arc<String>> {
        let key = format!("{}:{}", tenant_id, template_name);
        self.template_cache.get(&key)
    }

    /// Set template in cache
    pub fn set_template(&self, tenant_id: &str, template_name: &str, content: String) {
        let key = format!("{}:{}", tenant_id, template_name);
        self.template_cache.set(key, Arc::new(content), None);
    }

    /// Get signed URL from cache
    pub fn get_url(&self, tenant_id: &str, document_id: &str) -> Option<String> {
        let key = format!("{}:{}", tenant_id, document_id);
        self.url_cache.get(&key)
    }

    /// Set signed URL in cache
    pub fn set_url(&self, tenant_id: &str, document_id: &str, url: String, ttl: Duration) {
        let key = format!("{}:{}", tenant_id, document_id);
        self.url_cache.set(key, url, Some(ttl));
    }

    /// Get NCF sequence from cache
    pub fn get_ncf_sequence(&self, tenant_id: &str, serie: char, tipo_code: u32) -> Option<NCFCacheEntry> {
        let key = format!("{}:{}:{}", tenant_id, serie, tipo_code);
        self.ncf_cache.get(&key)
    }

    /// Set NCF sequence in cache
    pub fn set_ncf_sequence(&self, tenant_id: &str, serie: char, tipo_code: u32, entry: NCFCacheEntry) {
        let key = format!("{}:{}:{}", tenant_id, serie, tipo_code);
        self.ncf_cache.set(key, entry, None);
    }

    /// Invalidate NCF sequence cache
    pub fn invalidate_ncf_sequence(&self, tenant_id: &str, serie: char, tipo_code: u32) {
        let key = format!("{}:{}:{}", tenant_id, serie, tipo_code);
        self.ncf_cache.remove(&key);
    }

    /// Clear all caches for a tenant
    pub fn clear_tenant(&self, tenant_id: &str) {
        // Clear template cache
        let template_keys: Vec<String> = self.template_cache.store
            .iter()
            .filter(|entry| entry.key().starts_with(&format!("{}:", tenant_id)))
            .map(|entry| entry.key().clone())
            .collect();

        for key in template_keys {
            self.template_cache.remove(&key);
        }

        // Clear URL cache
        let url_keys: Vec<String> = self.url_cache.store
            .iter()
            .filter(|entry| entry.key().starts_with(&format!("{}:", tenant_id)))
            .map(|entry| entry.key().clone())
            .collect();

        for key in url_keys {
            self.url_cache.remove(&key);
        }

        // Clear NCF cache
        let ncf_keys: Vec<String> = self.ncf_cache.store
            .iter()
            .filter(|entry| entry.key().starts_with(&format!("{}:", tenant_id)))
            .map(|entry| entry.key().clone())
            .collect();

        for key in ncf_keys {
            self.ncf_cache.remove(&key);
        }
    }

    /// Clean up all expired entries
    pub fn cleanup(&self) {
        self.template_cache.clear_expired();
        self.url_cache.clear_expired();
        self.config_cache.clear_expired();
        self.ncf_cache.clear_expired();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            template_count: self.template_cache.len(),
            url_count: self.url_cache.len(),
            config_count: self.config_cache.len(),
            ncf_count: self.ncf_cache.len(),
        }
    }
}

/// NCF cache entry for sequence management
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NCFCacheEntry {
    pub current_sequence: u64,
    pub max_sequence: u64,
    pub remaining: u64,
    pub expiry_date: chrono::DateTime<chrono::Utc>,
}

/// Cache statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheStats {
    pub template_count: usize,
    pub url_count: usize,
    pub config_count: usize,
    pub ncf_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic_operations() {
        let cache: InMemoryCache<String> = InMemoryCache::new(None);

        // Set and get
        cache.set("key1".to_string(), "value1".to_string(), None);
        assert_eq!(cache.get("key1"), Some("value1".to_string()));

        // Contains
        assert!(cache.contains("key1"));
        assert!(!cache.contains("key2"));

        // Remove
        assert_eq!(cache.remove("key1"), Some("value1".to_string()));
        assert!(!cache.contains("key1"));
    }

    #[test]
    fn test_cache_expiration() {
        let cache: InMemoryCache<String> = InMemoryCache::new(None);

        // Set with very short TTL
        cache.set("key1".to_string(), "value1".to_string(), Some(Duration::from_millis(10)));

        // Should exist immediately
        assert!(cache.contains("key1"));

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(20));

        // Should be expired
        assert!(!cache.contains("key1"));
        assert_eq!(cache.get("key1"), None);
    }

    #[test]
    fn test_cache_manager() {
        let manager = CacheManager::new();

        // Test template cache
        manager.set_template("tenant1", "template1", "content1".to_string());
        assert_eq!(
            manager.get_template("tenant1", "template1").map(|s| s.to_string()),
            Some("content1".to_string())
        );

        // Test URL cache
        manager.set_url("tenant1", "doc1", "https://example.com".to_string(), Duration::from_secs(60));
        assert_eq!(
            manager.get_url("tenant1", "doc1"),
            Some("https://example.com".to_string())
        );

        // Test stats
        let stats = manager.stats();
        assert_eq!(stats.template_count, 1);
        assert_eq!(stats.url_count, 1);
    }
}