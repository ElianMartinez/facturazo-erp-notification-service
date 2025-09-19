use std::sync::Arc;
use anyhow::Result;
use std::process::Command;
use uuid::Uuid;
use std::fs;

use crate::models::{InvoiceRequest, ReportRequest, RenderOptions};
use crate::templates::TemplateManager;

pub struct PdfGenerator {
    template_manager: Arc<TemplateManager>,
    temp_dir: String,
}

impl PdfGenerator {
    pub fn new(template_manager: Arc<TemplateManager>) -> Self {
        let temp_dir = std::env::var("TEMP_DIR")
            .unwrap_or_else(|_| "/tmp".to_string());

        PdfGenerator {
            template_manager,
            temp_dir,
        }
    }

    pub async fn generate_invoice(&self, request: &InvoiceRequest) -> Result<Vec<u8>> {
        // Get template
        let template_content = self.template_manager
            .get_template(&request.template_id)
            .await?;

        // Get render options
        let options = request.options.clone()
            .unwrap_or_else(RenderOptions::default);

        // Render with template engine
        let rendered = self.template_manager
            .get_engine()
            .render_invoice(&request.template_id, &request.data, &options)
            .await?;

        // Compile to PDF using Typst
        self.compile_typst_to_pdf(&rendered).await
    }

    pub async fn generate_report(&self, request: &ReportRequest, data: Vec<serde_json::Value>) -> Result<Vec<u8>> {
        // Get template
        let template_content = self.template_manager
            .get_template(&request.template_id)
            .await?;

        // Get render options
        let options = request.options.as_ref()
            .and_then(|o| o.render.clone())
            .unwrap_or_else(RenderOptions::default);

        // Render with template engine
        let rendered = self.template_manager
            .get_engine()
            .render_report(&request.template_id, request, data)
            .await?;

        // Compile to PDF using Typst
        self.compile_typst_to_pdf(&rendered).await
    }

    async fn compile_typst_to_pdf(&self, typst_content: &str) -> Result<Vec<u8>> {
        // Create temporary file
        let temp_id = Uuid::new_v4();
        let typ_path = format!("{}/temp_{}.typ", self.temp_dir, temp_id);
        let pdf_path = format!("{}/temp_{}.pdf", self.temp_dir, temp_id);

        // Write Typst content
        tokio::fs::write(&typ_path, typst_content).await?;

        // Compile with Typst
        let output = tokio::task::spawn_blocking({
            let typ_path = typ_path.clone();
            let pdf_path = pdf_path.clone();
            move || {
                Command::new("typst")
                    .args(&["compile", &typ_path, &pdf_path])
                    .output()
            }
        }).await??;

        if !output.status.success() {
            // Clean up temp files
            let _ = fs::remove_file(&typ_path);
            return Err(anyhow::anyhow!(
                "Typst compilation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Read PDF bytes
        let pdf_bytes = tokio::fs::read(&pdf_path).await?;

        // Clean up temp files
        let _ = tokio::fs::remove_file(&typ_path).await;
        let _ = tokio::fs::remove_file(&pdf_path).await;

        Ok(pdf_bytes)
    }

    pub async fn generate_with_custom_template(&self, template: &str, data: serde_json::Value) -> Result<Vec<u8>> {
        // Render template with data
        let mut env = minijinja::Environment::new();
        env.add_template("custom", template)?;
        let template = env.get_template("custom")?;
        let rendered = template.render(&data)?;

        // Compile to PDF
        self.compile_typst_to_pdf(&rendered).await
    }
}