//! Template resolver module
//!
//! Fetches templates from the .NET Facturazo ERP Core backend,
//! downloads assets from R2, and caches everything for fast Typst compilation.

pub mod api_client;
pub mod asset_manager;
pub mod models;
pub mod resolver;

pub use api_client::TemplateApiClient;
pub use asset_manager::AssetManager;
pub use models::{CachedTemplate, ResolvedTemplate};
pub use resolver::TemplateResolver;
