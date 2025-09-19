use std::sync::Arc;
use anyhow::Result;
use std::process::Command;
use uuid::Uuid;
use std::fs;

use crate::templates::TemplateManager;

/// Generador genérico de PDFs usando Typst
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

    /// Genera un PDF desde cualquier template y datos JSON
    pub async fn generate(&self, template_id: &str, data: serde_json::Value) -> Result<Vec<u8>> {
        // El template engine se encarga de toda la lógica específica
        let pdf_path = self.template_manager
            .generate_pdf_from_json(template_id, data, None)
            .await?;

        // Leer el PDF generado
        let pdf_bytes = tokio::fs::read(&pdf_path).await?;

        // Limpiar el archivo temporal
        let _ = tokio::fs::remove_file(&pdf_path).await;

        Ok(pdf_bytes)
    }

    /// Genera un PDF con un template personalizado (no registrado)
    pub async fn generate_with_custom_template(&self, typst_content: &str) -> Result<Vec<u8>> {
        self.compile_typst_to_pdf(typst_content).await
    }

    /// Compila contenido Typst a PDF
    async fn compile_typst_to_pdf(&self, typst_content: &str) -> Result<Vec<u8>> {
        // Crear archivos temporales
        let temp_id = Uuid::new_v4();
        let typ_path = format!("{}/temp_{}.typ", self.temp_dir, temp_id);
        let pdf_path = format!("{}/temp_{}.pdf", self.temp_dir, temp_id);

        // Escribir contenido Typst
        tokio::fs::write(&typ_path, typst_content).await?;

        // Compilar con Typst
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
            // Limpiar archivos temporales
            let _ = fs::remove_file(&typ_path);
            return Err(anyhow::anyhow!(
                "Typst compilation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Leer bytes del PDF
        let pdf_bytes = tokio::fs::read(&pdf_path).await?;

        // Limpiar archivos temporales
        let _ = tokio::fs::remove_file(&typ_path).await;
        let _ = tokio::fs::remove_file(&pdf_path).await;

        Ok(pdf_bytes)
    }

    /// Lista todos los templates disponibles
    pub fn list_templates(&self) -> Vec<(String, String)> {
        self.template_manager.list_templates()
    }

    /// Verifica si existe un template
    pub fn template_exists(&self, template_id: &str) -> bool {
        self.template_manager.template_exists(template_id)
    }
}