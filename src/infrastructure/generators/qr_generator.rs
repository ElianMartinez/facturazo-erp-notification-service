//! QR Code generator for fiscal documents

use anyhow::Result;
use qrcode::{QrCode, EcLevel};
use image::{ImageBuffer, Luma};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Serialize, Deserialize};

/// QR code generator for Dominican Republic fiscal documents
pub struct QRGenerator;

impl QRGenerator {
    /// Generate QR code for fiscal invoice
    pub fn generate_fiscal_qr(data: &FiscalQRData) -> Result<Vec<u8>> {
        // Create QR data string according to DGII format
        let qr_content = Self::format_fiscal_data(data);

        // Generate QR code
        let code = QrCode::with_error_correction_level(&qr_content, EcLevel::M)?;

        // Render to image
        let image = code.render::<Luma<u8>>()
            .min_dimensions(200, 200)
            .max_dimensions(400, 400)
            .build();

        // Convert to PNG bytes
        let mut png_bytes = Vec::new();
        image.write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageOutputFormat::Png
        )?;

        Ok(png_bytes)
    }

    /// Generate QR code as base64 string
    pub fn generate_fiscal_qr_base64(data: &FiscalQRData) -> Result<String> {
        let png_bytes = Self::generate_fiscal_qr(data)?;
        Ok(BASE64.encode(&png_bytes))
    }

    /// Generate QR code and save to file
    pub async fn generate_fiscal_qr_file(data: &FiscalQRData, path: &str) -> Result<()> {
        let png_bytes = Self::generate_fiscal_qr(data)?;
        tokio::fs::write(path, png_bytes).await?;
        Ok(())
    }

    /// Format fiscal data for QR code - uses pre-formatted URL from core service
    fn format_fiscal_data(data: &FiscalQRData) -> String {
        // If core service provides a pre-formatted URL, use it
        if let Some(url) = &data.qr_url {
            return url.clone();
        }

        // Otherwise, build basic URL (core service should provide this)
        format!(
            "https://dgii.gov.do/factura/{}/{}/{}",
            data.seller_rnc,
            data.ncf,
            data.invoice_number
        )
    }

    /// Generate generic QR code
    pub fn generate_qr(content: &str) -> Result<Vec<u8>> {
        let code = QrCode::new(content)?;

        let image = code.render::<Luma<u8>>()
            .min_dimensions(200, 200)
            .max_dimensions(400, 400)
            .build();

        let mut png_bytes = Vec::new();
        image.write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageOutputFormat::Png
        )?;

        Ok(png_bytes)
    }

    /// Generate payment QR code
    pub fn generate_payment_qr(data: &PaymentQRData) -> Result<Vec<u8>> {
        let qr_content = serde_json::to_string(data)?;
        Self::generate_qr(&qr_content)
    }
}

/// Fiscal QR data structure (data comes from core service)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiscalQRData {
    pub seller_rnc: String,
    pub ncf: String,
    pub invoice_number: String,
    pub qr_url: Option<String>,  // Pre-formatted URL from core service
    // Optional fields if not using pre-formatted URL
    pub buyer_rnc: Option<String>,
}

/// Payment QR data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentQRData {
    pub invoice_id: String,
    pub amount: f64,
    pub currency: String,
    pub bank_account: Option<String>,
    pub reference: String,
}

/// QR code options
#[derive(Debug, Clone)]
pub struct QROptions {
    pub size: u32,
    pub margin: u32,
    pub error_correction: EcLevel,
    pub dark_color: [u8; 3],
    pub light_color: [u8; 3],
}

impl Default for QROptions {
    fn default() -> Self {
        Self {
            size: 300,
            margin: 10,
            error_correction: EcLevel::M,
            dark_color: [0, 0, 0],
            light_color: [255, 255, 255],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_fiscal_qr_generation() {
        let data = FiscalQRData {
            seller_rnc: "130123456".to_string(),
            ncf: "E3100000001".to_string(),
            invoice_number: "INV-2024-0001".to_string(),
            qr_url: Some("https://dgii.gov.do/factura/130123456/E3100000001/example".to_string()),
            buyer_rnc: Some("131654321".to_string()),
        };

        let qr_bytes = QRGenerator::generate_fiscal_qr(&data).unwrap();
        assert!(!qr_bytes.is_empty());

        // Check PNG header
        assert_eq!(&qr_bytes[1..4], b"PNG");
    }

    #[test]
    fn test_fiscal_qr_base64() {
        let data = FiscalQRData {
            seller_rnc: "130123456".to_string(),
            ncf: "E3100000001".to_string(),
            invoice_number: "INV-2024-0001".to_string(),
            qr_url: None,
            buyer_rnc: None,
        };

        let base64_qr = QRGenerator::generate_fiscal_qr_base64(&data).unwrap();
        assert!(base64_qr.starts_with("iVBOR")); // PNG base64 signature
    }

    #[test]
    fn test_payment_qr() {
        let data = PaymentQRData {
            invoice_id: "INV-2024-0001".to_string(),
            amount: 1500.00,
            currency: "DOP".to_string(),
            bank_account: Some("1234567890".to_string()),
            reference: "PAGO-001".to_string(),
        };

        let qr_bytes = QRGenerator::generate_payment_qr(&data).unwrap();
        assert!(!qr_bytes.is_empty());
    }

    #[test]
    fn test_fiscal_data_formatting() {
        // Test with pre-formatted URL
        let data_with_url = FiscalQRData {
            seller_rnc: "130123456".to_string(),
            ncf: "E3100000001".to_string(),
            invoice_number: "INV-2024-0001".to_string(),
            qr_url: Some("https://dgii.gov.do/custom/url".to_string()),
            buyer_rnc: Some("131654321".to_string()),
        };

        let formatted = QRGenerator::format_fiscal_data(&data_with_url);
        assert_eq!(formatted, "https://dgii.gov.do/custom/url");

        // Test without pre-formatted URL
        let data_no_url = FiscalQRData {
            seller_rnc: "130123456".to_string(),
            ncf: "E3100000001".to_string(),
            invoice_number: "INV-2024-0001".to_string(),
            qr_url: None,
            buyer_rnc: None,
        };

        let formatted = QRGenerator::format_fiscal_data(&data_no_url);
        assert!(formatted.contains("dgii.gov.do"));
        assert!(formatted.contains("130123456"));
        assert!(formatted.contains("E3100000001"));
    }
}