use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Trait base para todas las plantillas de documentos
pub trait TypstTemplate: Send + Sync {
    /// Genera el contenido Typst a partir de los datos JSON
    fn generate(&self, data: &Value) -> Result<String>;

    /// Retorna el ID único de la plantilla
    fn template_id(&self) -> &str;

    /// Valida que los datos contengan los campos requeridos
    fn validate(&self, data: &Value) -> Result<()>;

    /// Retorna una descripción de la plantilla
    fn description(&self) -> &str {
        "Template de documento"
    }
}

/// Registry central de todas las plantillas disponibles
pub struct TemplateRegistry {
    templates: HashMap<String, Arc<dyn TypstTemplate>>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        let mut templates: HashMap<String, Arc<dyn TypstTemplate>> = HashMap::new();

        // Registrar todas las plantillas disponibles
        use crate::templates::templates::*;

        // Factura fiscal electrónica
        let fiscal = Arc::new(FiscalInvoiceTemplate::new());
        templates.insert(fiscal.template_id().to_string(), fiscal);

        // Factura simple
        let simple = Arc::new(SimpleInvoiceTemplate::new());
        templates.insert(simple.template_id().to_string(), simple);

        // Recibo
        let receipt = Arc::new(ReceiptTemplate::new());
        templates.insert(receipt.template_id().to_string(), receipt);

        // Reporte
        let report = Arc::new(ReportTemplate::new());
        templates.insert(report.template_id().to_string(), report);

        Self { templates }
    }

    /// Obtiene una plantilla por su ID
    pub fn get(&self, template_id: &str) -> Option<Arc<dyn TypstTemplate>> {
        self.templates.get(template_id).cloned()
    }

    /// Lista todas las plantillas disponibles
    pub fn list(&self) -> Vec<(String, String)> {
        self.templates
            .iter()
            .map(|(id, template)| (id.clone(), template.description().to_string()))
            .collect()
    }

    /// Valida si existe una plantilla con el ID dado
    pub fn exists(&self, template_id: &str) -> bool {
        self.templates.contains_key(template_id)
    }
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Utilidades compartidas para generar elementos Typst
pub mod utils {
    use super::*;

    /// Escapa caracteres especiales para Typst
    pub fn escape_typst(text: &str) -> String {
        text.replace('@', "\\@")
            .replace('#', "\\#")
            .replace('$', "\\$")
    }

    /// Formatea un número con separadores de miles
    pub fn format_number(value: f64, decimals: usize) -> String {
        format!("{:.*}", decimals, value)
            .chars()
            .rev()
            .enumerate()
            .map(|(i, c)| {
                if i > 0 && i % 3 == 0 && c.is_ascii_digit() {
                    format!(",{}", c)
                } else {
                    c.to_string()
                }
            })
            .collect::<String>()
            .chars()
            .rev()
            .collect()
    }

    /// Genera código QR y retorna la ruta del archivo
    pub fn generate_qr_code(data: &str, output_path: &str) -> Result<String> {
        use qrcode::{QrCode, Color};
        use image::{ImageBuffer, Rgb};

        let code = QrCode::new(data)?;
        let width = code.width();
        let scale = 5;
        let img_size = width * scale;

        let mut image = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(img_size as u32, img_size as u32);

        for y in 0..width {
            for x in 0..width {
                let color = match code[(x, y)] {
                    Color::Dark => Rgb([0, 0, 0]),
                    Color::Light => Rgb([255, 255, 255]),
                };

                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = (x * scale + dx) as u32;
                        let py = (y * scale + dy) as u32;
                        image.put_pixel(px, py, color);
                    }
                }
            }
        }

        image.save(output_path)?;
        Ok(output_path.to_string())
    }
}