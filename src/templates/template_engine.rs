use crate::templates::template_models::*;
use crate::templates::template_trait::{TemplateRegistry, TypstTemplate};
use anyhow::{Result, Context};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use serde_json;
use std::collections::HashMap;

pub struct TemplateEngine {
    output_dir: String,
    registry: Arc<TemplateRegistry>,
}

impl TemplateEngine {
    pub fn new(_templates_dir: String, output_dir: String) -> Self {
        Self {
            output_dir,
            registry: Arc::new(TemplateRegistry::new()),
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
        let template = self.registry.get(template_id)
            .ok_or_else(|| anyhow::anyhow!("Template no encontrado: {}", template_id))?;

        // Convertir TemplateData a JSON para la plantilla
        let json_data = serde_json::to_value(&data)?;

        // Validar los datos
        template.validate(&json_data)?;

        // Generar contenido Typst usando la plantilla dinámica
        let typst_content = template.generate(&json_data)?;

        // Assets vacíos por ahora (se pueden manejar dentro de cada plantilla si es necesario)
        let _assets: HashMap<String, String> = HashMap::new();

        let timestamp = chrono::Utc::now().timestamp();
        let base_filename = output_filename.unwrap_or_else(|| format!("{}_{}", template_id, timestamp));

        let typ_path = format!("{}/{}.typ", self.output_dir, base_filename);
        let pdf_path = format!("{}/{}.pdf", self.output_dir, base_filename);

        // Guardar el archivo Typst temporal
        fs::write(&typ_path, &typst_content)?;

        // Compilar Typst a PDF
        let output = Command::new("typst")
            .args(&["compile", &typ_path, &pdf_path])
            .output()?;

        // Limpiar archivo temporal
        fs::remove_file(&typ_path).ok();

        // Limpiar assets temporales (si hubiera alguno)
        // Los assets ahora se manejan dentro de cada plantilla

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
        let template = self.registry.get(template_id)
            .ok_or_else(|| anyhow::anyhow!("Template no encontrado: {}", template_id))?;

        // Validar los datos
        template.validate(&json_data)?;

        // Generar contenido Typst
        let typst_content = template.generate(&json_data)?;

        let timestamp = chrono::Utc::now().timestamp();
        let base_filename = output_filename.unwrap_or_else(|| format!("{}_{}", template_id, timestamp));

        let typ_path = format!("{}/{}.typ", self.output_dir, base_filename);
        let pdf_path = format!("{}/{}.pdf", self.output_dir, base_filename);

        // Guardar el archivo Typst temporal
        fs::write(&typ_path, &typst_content)?;

        // Compilar Typst a PDF
        let output = Command::new("typst")
            .args(&["compile", &typ_path, &pdf_path])
            .output()?;

        // Limpiar archivo temporal
        fs::remove_file(&typ_path).ok();

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