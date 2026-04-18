//! Document generation infrastructure
//!
//! Multi-format document generation: PDF, Excel, CSV

use crate::domain::document::DocumentFormat;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

// PDF generators
pub mod invoice_generator;
pub mod qr_generator;
pub mod quotation_generator;
pub mod receipt_generator;
pub mod report_generator;
pub mod typst_generator;
pub mod visual_codes;

// New streaming PDF engine for ERP reports (replaces Typst path for reports only)
pub mod streaming_report_engine;
pub mod streaming_report_generator;

// Excel & CSV generators
pub mod csv_generator;
pub mod erp_report_csv_generator;
pub mod erp_report_excel_generator;
pub mod excel_generator;

// Template management
pub mod template_manager;

// Re-exports
pub use csv_generator::CsvGenerator;
pub use erp_report_csv_generator::ErpReportCsvGenerator;
pub use erp_report_excel_generator::ErpReportExcelGenerator;
pub use excel_generator::ExcelGenerator;
pub use invoice_generator::InvoiceGenerator;
pub use qr_generator::QRGenerator;
pub use quotation_generator::QuotationGenerator;
pub use receipt_generator::ReceiptGenerator;
pub use report_generator::ReportGenerator;
pub use template_manager::TemplateManager;
pub use typst_generator::TypstGenerator;

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
    pub format: DocumentFormat,
    pub document_bytes: Vec<u8>,
    pub mime_type: String,
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

/// Generator factory with multi-format support
pub struct GeneratorFactory {
    pdf_generators: HashMap<DocumentType, Box<dyn DocumentGenerator>>,
    excel_generator: ExcelGenerator,
    csv_generator: CsvGenerator,
    template_manager: TemplateManager,
}

impl GeneratorFactory {
    pub fn new(template_dir: PathBuf) -> Self {
        let mut pdf_generators = HashMap::new();
        let template_manager = TemplateManager::new(template_dir.clone());

        // Register PDF generators
        pdf_generators.insert(
            DocumentType::Invoice,
            Box::new(InvoiceGenerator::new(template_dir.clone())) as Box<dyn DocumentGenerator>,
        );

        pdf_generators.insert(
            DocumentType::Quotation,
            Box::new(QuotationGenerator::new(template_dir.clone())) as Box<dyn DocumentGenerator>,
        );

        pdf_generators.insert(
            DocumentType::Report,
            Box::new(ReportGenerator::new(template_dir.clone())) as Box<dyn DocumentGenerator>,
        );

        pdf_generators.insert(
            DocumentType::Receipt,
            Box::new(ReceiptGenerator::new(template_dir.clone())) as Box<dyn DocumentGenerator>,
        );

        // Create Excel and CSV generators
        let excel_generator = ExcelGenerator::new(template_dir.clone());
        let csv_generator = CsvGenerator::new();

        Self {
            pdf_generators,
            excel_generator,
            csv_generator,
            template_manager,
        }
    }

    /// Get PDF generator for document type
    pub fn get_pdf_generator(&self, doc_type: DocumentType) -> Option<&Box<dyn DocumentGenerator>> {
        self.pdf_generators.get(&doc_type)
    }

    /// Get Excel generator
    pub fn get_excel_generator(&self) -> &ExcelGenerator {
        &self.excel_generator
    }

    /// Get CSV generator
    pub fn get_csv_generator(&self) -> &CsvGenerator {
        &self.csv_generator
    }

    /// Generate document with format selection
    pub async fn generate(
        &self,
        doc_type: DocumentType,
        format: DocumentFormat,
        data: serde_json::Value,
        options: GenerationOptions,
    ) -> Result<GenerationResult> {
        let start_time = std::time::Instant::now();

        // Generate based on format
        let (document_bytes, mime_type) = match format {
            DocumentFormat::Pdf => {
                // Get PDF generator
                let generator = self.get_pdf_generator(doc_type).ok_or_else(|| {
                    anyhow::anyhow!("No PDF generator for document type: {:?}", doc_type)
                })?;

                // Get template
                let template = self
                    .template_manager
                    .get_template(doc_type.template_name())
                    .await?;

                // Generate PDF
                let pdf_bytes = generator.generate(&template, &data).await?;

                // Apply PDF-specific options
                if options.compress {
                    // PDF compression would go here
                }

                if let Some(_watermark) = &options.watermark {
                    // Watermark application would go here
                }

                if options.password_protect {
                    // Password protection would go here
                }

                (pdf_bytes, "application/pdf")
            }
            DocumentFormat::Excel => {
                // Generate Excel
                let excel_bytes = self.excel_generator.generate("", &data).await?;
                (
                    excel_bytes,
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                )
            }
            DocumentFormat::Csv => {
                // Generate CSV
                let csv_bytes = self.csv_generator.generate("", &data).await?;
                (csv_bytes, "text/csv")
            }
            DocumentFormat::Html => {
                return Err(anyhow::anyhow!("HTML format not yet implemented"));
            }
        };

        let generation_time_ms = start_time.elapsed().as_millis() as u64;

        Ok(GenerationResult {
            document_type: doc_type,
            format,
            document_bytes: document_bytes.clone(),
            mime_type: mime_type.to_string(),
            metadata: GenerationMetadata {
                document_id: Uuid::new_v4().to_string(),
                template_version: "1.0.0".to_string(),
                generation_time_ms,
                page_count: 1, // Would need to parse document to get actual count
                file_size_bytes: document_bytes.len(),
            },
        })
    }
}
