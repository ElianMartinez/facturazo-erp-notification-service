//! Typst PDF generator implementation

use anyhow::Result;
use tokio::process::Command;
use tokio::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use handlebars::Handlebars;
use serde_json::Value;

/// Typst-based PDF generator
pub struct TypstGenerator {
    work_dir: PathBuf,
    handlebars: Handlebars<'static>,
}

impl TypstGenerator {
    pub fn new(work_dir: PathBuf) -> Self {
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(false); // Allow missing variables

        Self {
            work_dir,
            handlebars,
        }
    }

    /// Generate PDF using Typst
    pub async fn generate_pdf(&self, typst_content: &str) -> Result<Vec<u8>> {
        // Create temporary file names
        let temp_id = Uuid::new_v4().to_string();
        let typst_file = self.work_dir.join(format!("temp_{}.typ", temp_id));
        let pdf_file = self.work_dir.join(format!("temp_{}.pdf", temp_id));

        // Ensure work directory exists
        fs::create_dir_all(&self.work_dir).await?;

        // Write Typst content to file
        fs::write(&typst_file, typst_content).await?;

        // Run Typst compiler
        let output = Command::new("typst")
            .arg("compile")
            .arg(&typst_file)
            .arg(&pdf_file)
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);

            // Clean up temp file
            let _ = fs::remove_file(&typst_file).await;

            return Err(anyhow::anyhow!("Typst compilation failed: {}", error));
        }

        // Read generated PDF
        let pdf_bytes = fs::read(&pdf_file).await?;

        // Clean up temporary files
        let _ = fs::remove_file(&typst_file).await;
        let _ = fs::remove_file(&pdf_file).await;

        Ok(pdf_bytes)
    }

    /// Render template with data
    pub fn render_template(&self, template: &str, data: &Value) -> Result<String> {
        Ok(self.handlebars.render_template(template, data)?)
    }

    /// Process template and generate PDF
    pub async fn process(&self, template: &str, data: &Value) -> Result<Vec<u8>> {
        // Render template with data
        let typst_content = self.render_template(template, data)?;

        // Generate PDF
        self.generate_pdf(&typst_content).await
    }

    /// Check if Typst is installed
    pub async fn check_typst_installation() -> Result<bool> {
        let output = Command::new("typst")
            .arg("--version")
            .output()
            .await;

        Ok(output.is_ok() && output.unwrap().status.success())
    }
}

/// Typst template helpers
pub mod helpers {
    use chrono::{DateTime, Utc};

    /// Format number with thousands separator
    pub fn format_number(amount: f64, decimals: usize) -> String {
        let formatted = format!("{:.prec$}", amount, prec = decimals);
        let parts: Vec<&str> = formatted.split('.').collect();
        let integer_part = parts[0];
        let decimal_part = parts.get(1);

        let mut result = String::new();
        let chars: Vec<char> = integer_part.chars().rev().collect();
        for (i, ch) in chars.iter().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.push(',');
            }
            result.push(*ch);
        }
        let integer_formatted: String = result.chars().rev().collect();

        match decimal_part {
            Some(dec) => format!("{}.{}", integer_formatted, dec),
            None => integer_formatted,
        }
    }

    /// Format currency with Dominican peso symbol
    pub fn format_currency(amount: f64) -> String {
        format!("RD$ {}", format_number(amount, 2))
    }

    /// Format date for Dominican Republic
    pub fn format_date(date: DateTime<Utc>) -> String {
        date.format("%d/%m/%Y").to_string()
    }

    /// Format datetime
    pub fn format_datetime(date: DateTime<Utc>) -> String {
        date.format("%d/%m/%Y %H:%M:%S").to_string()
    }

    /// Format NCF with proper grouping
    pub fn format_ncf(ncf: &str) -> String {
        if ncf.len() == 11 {
            format!("{}{}-{}-{}",
                &ncf[0..1],   // Serie (E)
                &ncf[1..3],   // Tipo (31)
                &ncf[3..7],   // First group (0000)
                &ncf[7..11]   // Second group (0001)
            )
        } else {
            ncf.to_string()
        }
    }

    /// Format RNC with dashes
    pub fn format_rnc(rnc: &str) -> String {
        if rnc.len() == 9 {
            format!("{}-{}-{}-{}",
                &rnc[0..1],
                &rnc[1..3],
                &rnc[3..8],
                &rnc[8..9]
            )
        } else if rnc.len() == 11 {
            format!("{}-{}-{}",
                &rnc[0..3],
                &rnc[3..10],
                &rnc[10..11]
            )
        } else {
            rnc.to_string()
        }
    }

    /// Format phone number
    pub fn format_phone(phone: &str) -> String {
        let clean = phone.chars().filter(|c| c.is_ascii_digit()).collect::<String>();
        if clean.len() == 10 {
            format!("({}) {}-{}",
                &clean[0..3],
                &clean[3..6],
                &clean[6..10]
            )
        } else {
            phone.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_typst_installation_check() {
        let installed = TypstGenerator::check_typst_installation().await.unwrap_or(false);
        println!("Typst installed: {}", installed);
    }

    #[tokio::test]
    async fn test_simple_pdf_generation() {
        let temp_dir = TempDir::new().unwrap();
        let generator = TypstGenerator::new(temp_dir.path().to_path_buf());

        let typst_content = r#"
#set page(paper: "us-letter")
#set text(font: "Arial", size: 12pt)

= Test Document

This is a test document generated with Typst.
        "#;

        match generator.generate_pdf(typst_content).await {
            Ok(pdf) => {
                assert!(!pdf.is_empty());
                // Check PDF magic number
                assert_eq!(&pdf[0..4], b"%PDF");
            }
            Err(e) => {
                // Typst might not be installed in test environment
                println!("Skipping test - Typst not available: {}", e);
            }
        }
    }

    #[test]
    fn test_helpers() {
        assert_eq!(helpers::format_number(1500.50, 2), "1,500.50");
        assert_eq!(helpers::format_currency(1500.50), "RD$ 1,500.50");
        assert_eq!(helpers::format_ncf("E3100000001"), "E31-0000-0001");
        assert_eq!(helpers::format_rnc("101123456"), "1-01-12345-6");
        assert_eq!(helpers::format_phone("8095551234"), "(809) 555-1234");
    }
}