//! Data models for the template resolver

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

/// Template resolved from the .NET backend API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTemplate {
    pub id: i64,
    pub name: String,
    pub scope: String,
    pub print_type: String,
    pub document_type_code: String,
    pub template_content: String,
    pub typst_source: Option<String>,
    pub page_config: String,
    pub label_config: Option<String>,
    pub version: i32,
    pub is_system_default: bool,
    pub assets: Vec<TemplateAsset>,
}

/// Asset associated with a template
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateAsset {
    pub asset_type: String,
    pub file_name: String,
    pub file_url: String,
    pub mime_type: Option<String>,
}

/// Cached template with local asset paths ready for Typst compilation
#[derive(Debug, Clone)]
pub struct CachedTemplate {
    /// The Typst source (or template_content if typst_source is None)
    pub typst_source: String,
    /// Mapping of asset filename → local path on disk
    pub asset_paths: HashMap<String, PathBuf>,
    /// Additional font directories (for downloaded font assets)
    pub font_paths: Vec<PathBuf>,
    /// The asset base directory for this template
    pub asset_dir: PathBuf,
    /// Page config JSON
    pub page_config: String,
    /// Template version (used for cache invalidation on version change)
    pub version: i32,
    /// Template ID from backend
    pub template_id: i64,
    /// When this was cached
    pub cached_at: Instant,
}

/// Wrapper for the .NET backend API response
#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}
