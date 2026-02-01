use crate::templates::template_models::*;
use crate::templates::template_trait::TemplateRegistry;
use anyhow::Result;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::sync::Arc;

pub struct TemplateEngine {
    output_dir: String,
    registry: Arc<TemplateRegistry>,
    font_dir: Option<String>,
}

impl TemplateEngine {
    /// Find the fonts directory from environment or common locations
    fn find_font_dir() -> Option<String> {
        // Priority 1: Check environment variable PDF_FONTS_DIR (used in Docker)
        if let Ok(env_path) = std::env::var("PDF_FONTS_DIR") {
            let path = std::path::Path::new(&env_path);
            if path.exists() && path.is_dir() {
                tracing::info!("Using fonts directory from PDF_FONTS_DIR: {}", env_path);
                return Some(env_path);
            }
        }

        // Priority 2: Check common locations
        let candidates = vec![
            "/app/fonts", // Docker container path
            "fonts",      // Local relative path
            "./fonts",    // Explicit local path
        ];

        for candidate in candidates {
            let path = std::path::Path::new(candidate);
            if path.exists() && path.is_dir() {
                tracing::info!("Found fonts directory at: {}", candidate);
                return Some(candidate.to_string());
            }
        }

        // Also check current working directory
        if let Ok(cwd) = std::env::current_dir() {
            let fonts_path = cwd.join("fonts");
            if fonts_path.exists() && fonts_path.is_dir() {
                if let Some(path_str) = fonts_path.to_str() {
                    tracing::info!("Found fonts directory at: {}", path_str);
                    return Some(path_str.to_string());
                }
            }
        }

        tracing::warn!("No fonts directory found - PDFs may use fallback fonts");
        None
    }
}

impl TemplateEngine {
    pub fn new(_templates_dir: String, output_dir: String) -> Self {
        let font_dir = Self::find_font_dir();
        Self {
            output_dir,
            registry: Arc::new(TemplateRegistry::new()),
            font_dir,
        }
    }

    pub async fn generate_pdf(
        &self,
        template_id: &str,
        data: TemplateData,
        output_filename: Option<String>,
    ) -> Result<String> {
        fs::create_dir_all(&self.output_dir)?;

        // Obtener la plantilla del registro
        let template = self
            .registry
            .get(template_id)
            .ok_or_else(|| anyhow::anyhow!("Template no encontrado: {}", template_id))?;

        // Convertir TemplateData a JSON para la plantilla
        let mut json_data = serde_json::to_value(&data)?;

        // Inject work_dir into custom_fields so templates can generate assets (like QR codes)
        // in the same directory where the .typ file will be saved
        if let Some(obj) = json_data.as_object_mut() {
            let custom_fields = obj
                .entry("custom_fields")
                .or_insert_with(|| serde_json::json!({}));
            if let Some(cf) = custom_fields.as_object_mut() {
                cf.insert("work_dir".to_string(), serde_json::json!(self.output_dir));
            }
        }

        // Validar los datos
        template.validate(&json_data)?;

        // Generar contenido Typst usando la plantilla dinámica
        let typst_content = template.generate(&json_data)?;

        // Assets vacíos por ahora (se pueden manejar dentro de cada plantilla si es necesario)
        let _assets: HashMap<String, String> = HashMap::new();

        let timestamp = chrono::Utc::now().timestamp();
        let base_filename =
            output_filename.unwrap_or_else(|| format!("{}_{}", template_id, timestamp));

        let typ_path = format!("{}/{}.typ", self.output_dir, base_filename);
        let pdf_path = format!("{}/{}.pdf", self.output_dir, base_filename);

        // Guardar el archivo Typst temporal
        fs::write(&typ_path, &typst_content)?;

        // Compilar Typst a PDF
        let mut cmd = Command::new("typst");
        cmd.arg("compile");

        // Add font directory if available
        if let Some(ref font_dir) = self.font_dir {
            tracing::debug!("Adding font-path to typst command: {}", font_dir);
            cmd.arg("--font-path").arg(font_dir);
        }

        cmd.arg(&typ_path).arg(&pdf_path);
        let output = cmd.output()?;

        // Limpiar archivo temporal
        fs::remove_file(&typ_path).ok();

        // Limpiar archivos QR temporales generados por las plantillas
        if let Ok(entries) = fs::read_dir(&self.output_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("qr_") && name.ends_with(".png") {
                        fs::remove_file(&path).ok();
                    }
                }
            }
        }

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Typst compilation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(pdf_path)
    }

    /// Genera un PDF desde datos JSON genéricos
    pub async fn generate_pdf_from_json(
        &self,
        template_id: &str,
        json_data: serde_json::Value,
        output_filename: Option<String>,
    ) -> Result<String> {
        fs::create_dir_all(&self.output_dir)?;

        // Obtener la plantilla del registro
        let template = self
            .registry
            .get(template_id)
            .ok_or_else(|| anyhow::anyhow!("Template no encontrado: {}", template_id))?;

        // Inject work_dir into custom_fields so templates can generate assets (like QR codes)
        let mut json_data = json_data;
        if let Some(obj) = json_data.as_object_mut() {
            let custom_fields = obj
                .entry("custom_fields")
                .or_insert_with(|| serde_json::json!({}));
            if let Some(cf) = custom_fields.as_object_mut() {
                cf.insert("work_dir".to_string(), serde_json::json!(self.output_dir));
            }
        }

        // Validar los datos
        template.validate(&json_data)?;

        // Generar contenido Typst
        let typst_content = template.generate(&json_data)?;

        let timestamp = chrono::Utc::now().timestamp();
        let base_filename =
            output_filename.unwrap_or_else(|| format!("{}_{}", template_id, timestamp));

        let typ_path = format!("{}/{}.typ", self.output_dir, base_filename);
        let pdf_path = format!("{}/{}.pdf", self.output_dir, base_filename);

        // Guardar el archivo Typst temporal
        fs::write(&typ_path, &typst_content)?;

        // Compilar Typst a PDF
        let mut cmd = Command::new("typst");
        cmd.arg("compile");

        // Add font directory if available
        if let Some(ref font_dir) = self.font_dir {
            tracing::debug!("Adding font-path to typst command: {}", font_dir);
            cmd.arg("--font-path").arg(font_dir);
        }

        cmd.arg(&typ_path).arg(&pdf_path);
        let output = cmd.output()?;

        // Limpiar archivo temporal
        fs::remove_file(&typ_path).ok();

        // Limpiar archivos QR temporales generados por las plantillas
        if let Ok(entries) = fs::read_dir(&self.output_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("qr_") && name.ends_with(".png") {
                        fs::remove_file(&path).ok();
                    }
                }
            }
        }

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Typst compilation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(pdf_path)
    }

    /// Lista todas las plantillas disponibles
    pub fn list_templates(&self) -> Vec<(String, String)> {
        self.registry.list()
    }

    /// Verifica si existe una plantilla
    pub fn template_exists(&self, template_id: &str) -> bool {
        self.registry.exists(template_id)
    }

    /// Obtiene el registro de plantillas para operaciones avanzadas
    pub fn get_registry(&self) -> Arc<TemplateRegistry> {
        self.registry.clone()
    }
}

// Implementación para compatibilidad con código existente
impl TemplateEngine {
    /// Método helper para generar desde InvoiceData directamente
    pub async fn generate_invoice(
        &self,
        invoice_data: InvoiceData,
        output_filename: Option<String>,
    ) -> Result<String> {
        let template_id = if invoice_data.fiscal_info.is_some() {
            "fiscal_invoice"
        } else {
            "simple_invoice"
        };

        let data = TemplateData::Invoice(invoice_data);
        self.generate_pdf(template_id, data, output_filename).await
    }

    /// Método helper para generar desde ReportData directamente
    pub async fn generate_report(
        &self,
        report_data: ReportData,
        output_filename: Option<String>,
    ) -> Result<String> {
        let data = TemplateData::Report(report_data);
        self.generate_pdf("report", data, output_filename).await
    }

    /// Método helper para generar desde ReceiptData directamente
    pub async fn generate_receipt(
        &self,
        receipt_data: ReceiptData,
        output_filename: Option<String>,
    ) -> Result<String> {
        let data = TemplateData::Receipt(receipt_data);
        self.generate_pdf("receipt", data, output_filename).await
    }
}
