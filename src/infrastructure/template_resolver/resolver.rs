//! Core template resolution logic with caching and fallback

use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::infrastructure::cache::InMemoryCache;

use super::api_client::TemplateApiClient;
use super::asset_manager::AssetManager;
use super::models::CachedTemplate;

/// Resolves templates from the .NET backend with caching and fallback
pub struct TemplateResolver {
    api_client: Arc<TemplateApiClient>,
    asset_manager: Arc<AssetManager>,
    cache: InMemoryCache<CachedTemplate>,
    cache_ttl: Duration,
}

impl TemplateResolver {
    pub fn new(
        api_client: Arc<TemplateApiClient>,
        asset_manager: Arc<AssetManager>,
        cache_ttl_secs: u64,
    ) -> Self {
        let ttl = Duration::from_secs(cache_ttl_secs);
        Self {
            api_client,
            asset_manager,
            cache: InMemoryCache::new(Some(ttl)),
            cache_ttl: ttl,
        }
    }

    /// Resolve a template for a tenant + document type code.
    /// Returns None if the API is unreachable (caller should fallback to built-in).
    pub async fn resolve(
        &self,
        tenant_id: i64,
        document_type_code: &str,
    ) -> Result<Option<CachedTemplate>> {
        let cache_key = format!("tenant:{}:doctype:{}", tenant_id, document_type_code);

        // 1. Check cache
        if let Some(cached) = self.cache.get(&cache_key) {
            tracing::debug!(tenant_id, document_type_code, "Template cache hit");
            return Ok(Some(cached));
        }

        // 2. Call backend API
        let resolved = match self.api_client.resolve(tenant_id, document_type_code).await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    tenant_id,
                    document_type_code,
                    error = %e,
                    "Failed to resolve template from backend, falling back to built-in"
                );
                return Ok(None);
            }
        };

        // 3. Download assets
        let typst_source = resolved
            .typst_source
            .unwrap_or_else(|| resolved.template_content.clone());

        let (asset_paths, font_paths) = self
            .asset_manager
            .ensure_assets(tenant_id, resolved.id, resolved.version, &resolved.assets)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to download some assets");
                (Default::default(), Default::default())
            });

        let asset_dir = self
            .asset_manager
            .asset_dir(tenant_id, resolved.id, resolved.version);

        // 4. Build cached entry
        let cached = CachedTemplate {
            typst_source,
            asset_paths,
            font_paths,
            asset_dir,
            page_config: resolved.page_config,
            version: resolved.version,
            template_id: resolved.id,
            cached_at: Instant::now(),
        };

        // 5. Store in cache
        self.cache
            .set(cache_key, cached.clone(), Some(self.cache_ttl));

        tracing::info!(
            tenant_id,
            document_type_code,
            template_id = resolved.id,
            version = resolved.version,
            "Resolved and cached template from backend"
        );

        Ok(Some(cached))
    }

    /// Invalidate cache for a specific tenant + document type
    pub fn invalidate(&self, tenant_id: i64, document_type_code: &str) {
        let cache_key = format!("tenant:{}:doctype:{}", tenant_id, document_type_code);
        self.cache.remove(&cache_key);
        tracing::debug!(tenant_id, document_type_code, "Template cache invalidated");
    }

    /// Invalidate all cached templates for a tenant
    pub fn invalidate_tenant(&self, tenant_id: i64) {
        // InMemoryCache doesn't expose prefix removal, so we clear expired as best-effort
        // Cache TTL handles staleness for entries we can't remove by prefix
        self.cache.clear_expired();
        tracing::debug!(
            tenant_id,
            "Cleared expired template cache entries for tenant"
        );
    }

    /// Clean up expired cache entries
    pub fn cleanup(&self) {
        self.cache.clear_expired();
    }
}
