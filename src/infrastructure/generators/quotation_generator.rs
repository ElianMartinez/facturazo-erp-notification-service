//! Quotation PDF generator

use super::{DocumentGenerator, DocumentType, TypstGenerator};
use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

const DEFAULT_QUOTATION_TEMPLATE: &str = r#"
#set page(paper: "us-letter", margin: 1.5cm)
#set text(font: "Inter", size: 10pt)

= Quotation / Cotización

Company: {{company_name}}
Date: {{date}}

== Items
{{#each items}}
- {{description}}: {{price}}
{{/each}}

Total: {{total}}
"#;

/// Quotation PDF generator
pub struct QuotationGenerator {
    typst_generator: TypstGenerator,
}

impl QuotationGenerator {
    pub fn new(work_dir: PathBuf) -> Self {
        Self {
            typst_generator: TypstGenerator::new(work_dir),
        }
    }
}

#[async_trait::async_trait]
impl DocumentGenerator for QuotationGenerator {
    async fn generate(&self, template: &str, data: &Value) -> Result<Vec<u8>> {
        // Use provided template or default quotation template
        let final_template = if template.is_empty() {
            DEFAULT_QUOTATION_TEMPLATE
        } else {
            template
        };

        self.typst_generator.process(final_template, data).await
    }

    fn supported_types(&self) -> Vec<DocumentType> {
        vec![DocumentType::Quotation]
    }
}
