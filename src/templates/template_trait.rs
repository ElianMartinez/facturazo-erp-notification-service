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

        // Cotización
        let quotation = Arc::new(QuotationTemplate::new());
        templates.insert(quotation.template_id().to_string(), quotation);

        // Cuadre de Caja
        let cash_register = Arc::new(CashRegisterTemplate::new());
        templates.insert(cash_register.template_id().to_string(), cash_register);

        // ERP Report (Generic)
        let erp_report = Arc::new(ErpReportTemplate::new());
        templates.insert(erp_report.template_id().to_string(), erp_report);

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
        text.replace('\\', "\\\\")
            .replace('@', "\\@")
            .replace('#', "\\#")
            .replace('$', "\\$")
            .replace('`', "'")
            .replace('[', "\\[")
            .replace(']', "\\]")
            .replace('<', "\\<")
            .replace('>', "\\>")
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
    /// Incluye quiet zone (margen blanco) requerido para escaneo correcto
    pub fn generate_qr_code(data: &str, output_path: &str) -> Result<String> {
        use image::{ImageBuffer, Rgb};
        use qrcode::{Color, QrCode};

        let code = QrCode::new(data)?;
        let qr_width = code.width();
        let scale = 8; // Mayor escala para mejor calidad de escaneo
        let quiet_zone = 4; // Margen blanco de 4 módulos (estándar QR)
        let total_width = (qr_width + quiet_zone * 2) * scale;

        let mut image =
            ImageBuffer::<Rgb<u8>, Vec<u8>>::new(total_width as u32, total_width as u32);

        // Llenar todo de blanco primero (quiet zone)
        for pixel in image.pixels_mut() {
            *pixel = Rgb([255, 255, 255]);
        }

        // Dibujar el QR con offset para la quiet zone
        let offset = quiet_zone * scale;
        for y in 0..qr_width {
            for x in 0..qr_width {
                let color = match code[(x, y)] {
                    Color::Dark => Rgb([0, 0, 0]),
                    Color::Light => Rgb([255, 255, 255]),
                };

                for dy in 0..scale {
                    for dx in 0..scale {
                        let px = (offset + x * scale + dx) as u32;
                        let py = (offset + y * scale + dy) as u32;
                        image.put_pixel(px, py, color);
                    }
                }
            }
        }

        image.save(output_path)?;
        Ok(output_path.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qr_code_generation() {
        let data = "https://dgii.gov.do/ecf?rnc=101000001&encf=E310000000001";
        let output_path = "/tmp/test_qr_dgii.png";

        let result = utils::generate_qr_code(data, output_path);
        assert!(result.is_ok(), "QR generation failed: {:?}", result.err());

        // Verify file exists
        assert!(
            std::path::Path::new(output_path).exists(),
            "QR file was not created"
        );

        // Verify file is not empty
        let metadata = std::fs::metadata(output_path).unwrap();
        assert!(metadata.len() > 0, "QR file is empty");

        println!("QR code generated successfully at: {}", output_path);
        println!("File size: {} bytes", metadata.len());

        // Don't cleanup - we want to test with Typst
        // let _ = std::fs::remove_file(output_path);
    }
}
