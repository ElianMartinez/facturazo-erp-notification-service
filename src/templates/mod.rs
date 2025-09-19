pub mod template_engine;
pub mod template_models;
pub mod template_trait;
pub mod templates;

pub use template_engine::*;
pub use template_models::*;
pub use template_trait::{TypstTemplate, TemplateRegistry};

// Re-export TemplateEngine as TemplateManager for backward compatibility
pub type TemplateManager = template_engine::TemplateEngine;