use crate::core::{DocumentError, DocumentResult, PdfConfig};
use std::fs;
use std::path::Path;
use std::process::Command;

pub struct PdfGenerator {
    config: PdfConfig,
    content: String,
    temp_dir: String,
}

impl PdfGenerator {
    pub fn new(config: PdfConfig) -> Self {
        PdfGenerator {
            config,
            content: String::new(),
            temp_dir: "temp".to_string(),
        }
    }

    pub fn with_config(mut self, config: PdfConfig) -> Self {
        self.config = config;
        self
    }

    pub fn set_content(&mut self, content: String) {
        self.content = content;
    }

    pub fn generate_typst_document(&self, body_content: &str) -> String {
        format!(
            "{}\n\n{}",
            self.config.to_typst_header(),
            body_content
        )
    }

    pub fn compile_to_pdf(&self, output_path: &str) -> DocumentResult<()> {
        fs::create_dir_all(&self.temp_dir)?;

        let typst_content = self.generate_typst_document(&self.content);

        let temp_file = format!("{}/temp_{}.typ", self.temp_dir, uuid_simple());
        let pdf_path = Path::new(output_path);

        fs::write(&temp_file, &typst_content)?;

        let output = Command::new("typst")
            .args(&["compile", &temp_file, pdf_path.to_str().unwrap()])
            .output()
            .map_err(|e| DocumentError::GenerationError(
                format!("Error ejecutando typst: {}", e)
            ))?;

        let _ = fs::remove_file(&temp_file);

        if !output.status.success() {
            return Err(DocumentError::GenerationError(
                format!("Typst compilation failed: {}",
                    String::from_utf8_lossy(&output.stderr))
            ));
        }

        Ok(())
    }

    pub fn render(&self, output_path: &str) -> DocumentResult<()> {
        if self.content.is_empty() {
            return Err(DocumentError::ValidationError(
                "El contenido del documento está vacío".to_string()
            ));
        }

        self.compile_to_pdf(output_path)?;
        Ok(())
    }
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("{:x}", timestamp)
}