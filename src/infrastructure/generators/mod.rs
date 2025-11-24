//! Document generation infrastructure
//!
//! Typst-based PDF generation with template management

use anyhow::Result;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use uuid::Uuid;

pub mod typst_generator;
pub mod invoice_generator;
pub mod quotation_generator;
pub mod report_generator;
pub mod qr_generator;
pub mod template_manager;

pub use typst_generator::TypstGenerator;
pub use invoice_generator::InvoiceGenerator;
pub use quotation_generator::QuotationGenerator;
pub use report_generator::ReportGenerator;
pub use qr_generator::QRGenerator;
pub use template_manager::TemplateManager;

/// Document generator trait
#[async_trait::async_trait]
pub trait DocumentGenerator: Send + Sync {
    /// Generate a document from template and data
    async fn generate(&self, template: &str, data: &serde_json::Value) -> Result<Vec<u8>>;

    /// Get supported document types
    fn supported_types(&self) -> Vec<DocumentType>;
}

/// Document types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DocumentType {
    Invoice,
    Quotation,
    Report,
    Receipt,
    CreditNote,
}

impl DocumentType {
    pub fn as_str(&self) -> &str {
        match self {
            DocumentType::Invoice => "invoice",
            DocumentType::Quotation => "quotation",
            DocumentType::Report => "report",
            DocumentType::Receipt => "receipt",
            DocumentType::CreditNote => "credit_note",
        }
    }

    pub fn template_name(&self) -> &str {
        match self {
            DocumentType::Invoice => "invoice_fiscal",
            DocumentType::Quotation => "quotation",
            DocumentType::Report => "report",
            DocumentType::Receipt => "receipt",
            DocumentType::CreditNote => "credit_note",
        }
    }
}

/// Document generation result
#[derive(Debug, Clone)]
pub struct GenerationResult {
    pub document_type: DocumentType,
    pub pdf_bytes: Vec<u8>,
    pub metadata: GenerationMetadata,
}

/// Generation metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationMetadata {
    pub document_id: String,
    pub template_version: String,
    pub generation_time_ms: u64,
    pub page_count: usize,
    pub file_size_bytes: usize,
}

/// Generation options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationOptions {
    pub watermark: Option<String>,
    pub password_protect: bool,
    pub compress: bool,
    pub include_attachments: bool,
}

impl Default for GenerationOptions {
    fn default() -> Self {
        Self {
            watermark: None,
            password_protect: false,
            compress: true,
            include_attachments: false,
        }
    }
}

/// Generator factory
pub struct GeneratorFactory {
    generators: HashMap<DocumentType, Box<dyn DocumentGenerator>>,
    template_manager: TemplateManager,
}

impl GeneratorFactory {
    pub fn new(template_dir: PathBuf) -> Self {
        let mut generators = HashMap::new();
        let template_manager = TemplateManager::new(template_dir.clone());

        // Register generators
        generators.insert(
            DocumentType::Invoice,
            Box::new(InvoiceGenerator::new(template_dir.clone())) as Box<dyn DocumentGenerator>
        );

        generators.insert(
            DocumentType::Quotation,
            Box::new(QuotationGenerator::new(template_dir.clone())) as Box<dyn DocumentGenerator>
        );

        generators.insert(
            DocumentType::Report,
            Box::new(ReportGenerator::new(template_dir.clone())) as Box<dyn DocumentGenerator>
        );

        Self {
            generators,
            template_manager,
        }
    }

    /// Get generator for document type
    pub fn get_generator(&self, doc_type: DocumentType) -> Option<&Box<dyn DocumentGenerator>> {
        self.generators.get(&doc_type)
    }

    /// Generate document
    pub async fn generate(
        &self,
        doc_type: DocumentType,
        data: serde_json::Value,
        options: GenerationOptions,
    ) -> Result<GenerationResult> {
        let start_time = std::time::Instant::now();

        // Get generator
        let generator = self.get_generator(doc_type)
            .ok_or_else(|| anyhow::anyhow!("No generator for document type: {:?}", doc_type))?;

        // Get template
        let template = self.template_manager
            .get_template(doc_type.template_name())
            .await?;

        // Generate document
        let mut pdf_bytes = generator.generate(&template, &data).await?;

        // Apply options
        if options.compress {
            // PDF compression would go here
        }

        if let Some(watermark) = options.watermark {
            // Watermark application would go here
        }

        if options.password_protect {
            // Password protection would go here
        }

        let generation_time_ms = start_time.elapsed().as_millis() as u64;

        Ok(GenerationResult {
            document_type: doc_type,
            pdf_bytes: pdf_bytes.clone(),
            metadata: GenerationMetadata {
                document_id: Uuid::new_v4().to_string(),
                template_version: "1.0.0".to_string(),
                generation_time_ms,
                page_count: 1, // Would need to parse PDF to get actual count
                file_size_bytes: pdf_bytes.len(),
            },
        })
    }
}