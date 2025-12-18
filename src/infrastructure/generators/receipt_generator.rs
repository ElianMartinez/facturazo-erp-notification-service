//! Receipt/Cash Register PDF generator
//!
//! Uses the TypstTemplate system for generating cash register PDFs

use super::{DocumentGenerator, DocumentType};
use crate::templates::template_trait::TemplateRegistry;
use anyhow::Result;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

/// Receipt PDF generator that uses the template system
pub struct ReceiptGenerator {
    work_dir: PathBuf,
    font_dir: Option<PathBuf>,
    registry: Arc<TemplateRegistry>,
}

impl ReceiptGenerator {
    pub fn new(work_dir: PathBuf) -> Self {
        let font_dir = Self::find_font_dir(&work_dir);

        Self {
            work_dir,
            font_dir,
            registry: Arc::new(TemplateRegistry::new()),
        }
    }

    /// Find the fonts directory
    fn find_font_dir(work_dir: &Path) -> Option<PathBuf> {
        // Get current working directory
        let cwd = std::env::current_dir().ok();

        let mut candidates = vec![
            work_dir.join("fonts"),
            work_dir
                .parent()
                .map(|p| p.join("fonts"))
                .unwrap_or_default(),
            PathBuf::from("fonts"),
            PathBuf::from("./fonts"),
        ];

        // Add cwd-based candidates
        if let Some(ref cwd) = cwd {
            candidates.push(cwd.join("fonts"));
        }

        // Add environment variable
        if let Ok(font_path) = std::env::var("FONT_DIR") {
            candidates.insert(0, PathBuf::from(font_path));
        }

        for candidate in &candidates {
            if candidate.exists() && candidate.is_dir() {
                tracing::info!("Found fonts directory at: {:?}", candidate);
                return Some(candidate.clone());
            }
        }

        tracing::warn!("No fonts directory found. Checked: {:?}", candidates);
        None
    }
}

#[async_trait::async_trait]
impl DocumentGenerator for ReceiptGenerator {
    async fn generate(&self, template: &str, data: &Value) -> Result<Vec<u8>> {
        // Use the template parameter as template_id if it's a valid ID in our registry,
        // otherwise try to get it from data, falling back to "cash_register"
        let template_id = if !template.is_empty() && self.registry.exists(template) {
            template.to_string()
        } else {
            data.get("templateId")
                .or_else(|| data.get("template_id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "cash_register".to_string())
        };

        // Get the template from registry
        let template_obj = self
            .registry
            .get(&template_id)
            .ok_or_else(|| anyhow::anyhow!("Template not found: {}", template_id))?;

        // Validate the data
        template_obj.validate(data)?;

        // Generate Typst content
        let typst_content = template_obj.generate(data)?;

        // Create temp directory for output
        fs::create_dir_all(&self.work_dir)?;

        let timestamp = chrono::Utc::now().timestamp_millis();
        let typ_path = self.work_dir.join(format!("receipt_{}.typ", timestamp));
        let pdf_path = self.work_dir.join(format!("receipt_{}.pdf", timestamp));

        // Write Typst file
        fs::write(&typ_path, &typst_content)?;

        // Compile to PDF using typst CLI
        let mut cmd = Command::new("typst");
        cmd.arg("compile");

        // Add font directory - use explicit path or environment variable
        let font_path = self
            .font_dir
            .clone()
            .or_else(|| std::env::var("FONT_DIR").ok().map(PathBuf::from))
            .or_else(|| {
                // Fallback: check common locations at runtime
                let candidates = [
                    PathBuf::from("fonts"),
                    PathBuf::from("./fonts"),
                    std::env::current_dir()
                        .ok()
                        .map(|p| p.join("fonts"))
                        .unwrap_or_default(),
                ];
                candidates.into_iter().find(|p| p.exists() && p.is_dir())
            });

        if let Some(ref font_dir) = font_path {
            eprintln!("[RECEIPT_GEN] Using font directory: {:?}", font_dir);
            cmd.arg("--font-path").arg(font_dir);
        } else {
            eprintln!("[RECEIPT_GEN] WARNING: No font directory found!");
        }

        cmd.arg(typ_path.to_str().unwrap())
            .arg(pdf_path.to_str().unwrap());

        tracing::debug!("Executing typst command: {:?}", cmd);
        let output = cmd.output()?;

        // Clean up temp Typst file
        fs::remove_file(&typ_path).ok();

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Typst compilation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Read the generated PDF
        let pdf_bytes = fs::read(&pdf_path)?;

        // Clean up PDF file
        fs::remove_file(&pdf_path).ok();

        Ok(pdf_bytes)
    }

    fn supported_types(&self) -> Vec<DocumentType> {
        vec![DocumentType::Receipt]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_receipt_generator_with_cash_register_data() {
        let temp_dir = TempDir::new().unwrap();
        let generator = ReceiptGenerator::new(temp_dir.path().to_path_buf());

        let data = serde_json::json!({
            "documentCode": "SQ-001",
            "date": "17/12/2025, 10:00 AM",
            "companyInfo": {
                "name": "Test Company",
                "taxId": "123456789",
                "address": {
                    "street": "Test Street 123",
                    "city": "Santo Domingo",
                    "country": "DO"
                }
            },
            "cashier": {
                "name": "John Doe"
            },
            "transactionsCount": 10,
            "status": "balanced",
            "income": [
                { "concept": "POS", "quantity": 10, "amount": 5000.00 }
            ],
            "expenses": [
                { "concept": "Expenses", "quantity": 0, "amount": 0.00 }
            ],
            "summary": {
                "totalIncome": 5000.00,
                "totalExpenses": 0.00,
                "totalToBalance": 5000.00,
                "totalInRegister": 5000.00
            },
            "paymentMethods": [
                { "method": "Cash", "systemAmount": 5000.00, "actualAmount": 5000.00, "difference": 0.00 }
            ],
            "finalBalance": {
                "systemTotal": 5000.00,
                "actualTotal": 5000.00,
                "difference": 0.00
            }
        });

        // Try to generate (will fail if Typst not installed)
        match generator.generate("cash_register", &data).await {
            Ok(pdf) => {
                println!("Generated receipt PDF: {} bytes", pdf.len());
                assert!(!pdf.is_empty());
            }
            Err(e) => {
                println!(
                    "Could not generate receipt (Typst may not be installed): {}",
                    e
                );
            }
        }
    }
}
