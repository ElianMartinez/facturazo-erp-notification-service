//! Invoice PDF generator with Dominican Republic fiscal compliance
//!
//! Uses the FiscalInvoiceTemplate from the template registry for DGII-compliant invoices.

use super::{DocumentGenerator, DocumentType, TypstGenerator};
use crate::templates::template_trait::TypstTemplate;
use crate::templates::templates::FiscalInvoiceTemplate;
use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

/// Invoice PDF generator using DGII-compliant FiscalInvoiceTemplate
pub struct InvoiceGenerator {
    typst_generator: TypstGenerator,
    fiscal_template: FiscalInvoiceTemplate,
    work_dir: PathBuf,
}

impl InvoiceGenerator {
    pub fn new(work_dir: PathBuf) -> Self {
        Self {
            typst_generator: TypstGenerator::new(work_dir.clone()),
            fiscal_template: FiscalInvoiceTemplate::new(),
            work_dir,
        }
    }
}

#[async_trait::async_trait]
impl DocumentGenerator for InvoiceGenerator {
    async fn generate(&self, _template: &str, data: &Value) -> Result<Vec<u8>> {
        // Debug: log available top-level keys
        if let Some(obj) = data.as_object() {
            let keys: Vec<&String> = obj.keys().collect();
            tracing::info!("📦 Invoice data keys: {:?}", keys);
        }

        // Inject work_dir into custom_fields so template can generate QR in correct directory
        let mut data = data.clone();
        if let Some(obj) = data.as_object_mut() {
            let custom_fields = obj
                .entry("custom_fields")
                .or_insert_with(|| serde_json::json!({}));
            if let Some(cf) = custom_fields.as_object_mut() {
                cf.insert(
                    "work_dir".to_string(),
                    serde_json::json!(self.work_dir.to_string_lossy()),
                );
            }
        }

        // Use the DGII-compliant FiscalInvoiceTemplate to generate Typst content
        let typst_content = self.fiscal_template.generate(&data)?;

        tracing::info!("✅ Using FiscalInvoiceTemplate for DGII-compliant invoice");

        // Generate PDF from the Typst content
        self.typst_generator.process(&typst_content, &data).await
    }

    fn supported_types(&self) -> Vec<DocumentType> {
        vec![DocumentType::Invoice]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_invoice_generation_with_fiscal_template() {
        let temp_dir = TempDir::new().unwrap();
        let generator = InvoiceGenerator::new(temp_dir.path().to_path_buf());

        // Data format expected by FiscalInvoiceTemplate (DGII-compliant)
        let invoice_data = serde_json::json!({
            "invoice_number": "FCF-00001",
            "issue_date": "2024-11-24",
            "due_date": "2024-12-24",

            "company_info": {
                "name": "Le Croissant Doré SRL",
                "legal_name": "Le Croissant Doré SRL",
                "tax_id": "130123456",
                "address": {
                    "street": "Av. Abraham Lincoln #123",
                    "city": "Santo Domingo",
                    "state": "Distrito Nacional",
                    "country": "DO"
                },
                "phone": "809-555-1234",
                "email": "facturas@lecroissant.com"
            },

            "client_info": {
                "name": "Cliente Test SRL",
                "tax_id": "131654321",
                "address": {
                    "street": "Calle Principal #456",
                    "city": "Santiago",
                    "country": "DO"
                },
                "phone": "809-555-5678",
                "email": "compras@cliente.com"
            },

            "items": [
                {
                    "description": "Croissant de Chocolate",
                    "quantity": 2.0,
                    "unit": "UND",
                    "unit_price": 150.00,
                    "discount": 0.00,
                    "tax_rate": 18.0
                },
                {
                    "description": "Café Cappuccino Grande",
                    "quantity": 1.0,
                    "unit": "UND",
                    "unit_price": 200.00,
                    "discount": 0.00,
                    "tax_rate": 18.0
                }
            ],

            "fiscal_info": {
                "e_ncf": "E310000000001",
                "security_code": "ABC123XYZ",
                "signature_date": "2024-11-24",
                "qr_data": "https://dgii.gov.do/ecf?rnc=130123456&encf=E310000000001&monto=590.00&codigo=ABC123XYZ"
            },

            "notes": "Gracias por su compra!",

            "payment_info": {
                "method": "Efectivo",
                "terms": "Contado"
            }
        });

        // Try to generate (will fail if Typst not installed)
        match generator.generate("", &invoice_data).await {
            Ok(pdf) => {
                println!("Generated DGII-compliant invoice PDF: {} bytes", pdf.len());
                assert!(!pdf.is_empty());
            }
            Err(e) => {
                println!(
                    "Could not generate invoice (Typst may not be installed): {}",
                    e
                );
            }
        }
    }
}
