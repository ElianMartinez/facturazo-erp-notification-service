//! Visual code directive processor for Typst templates.
//!
//! Processes `{{qr ...}}` and `{{barcode ...}}` directives in rendered Typst source,
//! generating real PNG images and replacing the directives with `#image(...)` calls.
//!
//! ## Supported directives
//!
//! ### QR Code
//! ```text
//! {{qr value="https://example.com" size="2.5cm"}}
//! ```
//! Parameters:
//! - `value` (required): The data to encode
//! - `size` (optional, default "2.5cm"): Width/height of the QR code
//!
//! ### Barcode
//! ```text
//! {{barcode value="12345678" type="code128" height="1cm" width="6cm"}}
//! ```
//! Parameters:
//! - `value` (required): The data to encode
//! - `type` (optional, default "code128"): code128, code39, ean13, ean8, codabar
//! - `height` (optional, default "1cm"): Height of the barcode
//! - `width` (optional, default "6cm"): Width of the barcode

use anyhow::Result;
use image::Luma;
use nanoid::nanoid;
use qrcode::QrCode;
use regex::Regex;
use std::path::Path;
use tracing::warn;

/// Process all visual code directives in the rendered Typst source.
/// Generates PNG files in `output_dir` and replaces directives with `#image(...)`.
pub fn process_directives(source: &str, output_dir: &Path) -> Result<String> {
    let mut result = source.to_string();
    result = process_qr_directives(&result, output_dir)?;
    result = process_barcode_directives(&result, output_dir)?;
    Ok(result)
}

/// Process `{{qr value="..." size="..."}}` directives
fn process_qr_directives(source: &str, output_dir: &Path) -> Result<String> {
    let re = Regex::new(r#"\{\{qr\s+([^}]+)\}\}"#)?;
    let mut result = source.to_string();

    for cap in re.captures_iter(source) {
        let full_match = &cap[0];
        let attrs = &cap[1];

        let value = extract_attr(attrs, "value").unwrap_or_default();
        let size = extract_attr(attrs, "size").unwrap_or_else(|| "2.5cm".to_string());

        if value.is_empty() {
            warn!("QR directive with empty value, skipping");
            result = result.replacen(full_match, "", 1);
            continue;
        }

        match generate_qr_png(&value, output_dir) {
            Ok(filename) => {
                let typst_image = format!("#image(\"{}\", width: {})", filename, size);
                result = result.replacen(full_match, &typst_image, 1);
            }
            Err(e) => {
                warn!(error = %e, "Failed to generate QR code, removing directive");
                result = result.replacen(full_match, "", 1);
            }
        }
    }

    Ok(result)
}

/// Process `{{barcode value="..." type="..." height="..." width="..."}}` directives
fn process_barcode_directives(source: &str, output_dir: &Path) -> Result<String> {
    let re = Regex::new(r#"\{\{barcode\s+([^}]+)\}\}"#)?;
    let mut result = source.to_string();

    for cap in re.captures_iter(source) {
        let full_match = &cap[0];
        let attrs = &cap[1];

        let value = extract_attr(attrs, "value").unwrap_or_default();
        let barcode_type = extract_attr(attrs, "type").unwrap_or_else(|| "code128".to_string());
        let height = extract_attr(attrs, "height").unwrap_or_else(|| "1cm".to_string());
        let width = extract_attr(attrs, "width").unwrap_or_else(|| "6cm".to_string());

        if value.is_empty() {
            warn!("Barcode directive with empty value, skipping");
            result = result.replacen(full_match, "", 1);
            continue;
        }

        match generate_barcode_png(&value, &barcode_type, output_dir) {
            Ok(filename) => {
                let typst_image = format!(
                    "#image(\"{}\", width: {}, height: {})",
                    filename, width, height
                );
                result = result.replacen(full_match, &typst_image, 1);
            }
            Err(e) => {
                warn!(error = %e, barcode_type = %barcode_type, "Failed to generate barcode, removing directive");
                result = result.replacen(full_match, "", 1);
            }
        }
    }

    Ok(result)
}

/// Extract attribute value from a directive string like `value="hello" size="2cm"`
fn extract_attr(attrs: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"{}="([^"]*)""#, regex::escape(name))).ok()?;
    re.captures(attrs).map(|c| c[1].to_string())
}

/// Generate a QR code PNG, returns the filename (not full path).
fn generate_qr_png(data: &str, output_dir: &Path) -> Result<String> {
    let code = QrCode::new(data.as_bytes())?;
    let module_size = 8u32;
    let quiet_zone = 2u32; // modules of quiet zone

    let matrix = code.to_colors();
    let width = code.width() as u32;
    let total = width + quiet_zone * 2;
    let img_size = total * module_size;

    let mut img = image::GrayImage::new(img_size, img_size);

    // Fill white
    for pixel in img.pixels_mut() {
        *pixel = Luma([255u8]);
    }

    // Draw modules
    for (i, color) in matrix.iter().enumerate() {
        let mx = (i as u32) % width;
        let my = (i as u32) / width;
        if *color == qrcode::types::Color::Dark {
            let px = (mx + quiet_zone) * module_size;
            let py = (my + quiet_zone) * module_size;
            for dx in 0..module_size {
                for dy in 0..module_size {
                    img.put_pixel(px + dx, py + dy, Luma([0u8]));
                }
            }
        }
    }

    let filename = format!("qr_{}.png", nanoid!(8));
    let path = output_dir.join(&filename);
    img.save(&path)?;

    Ok(filename)
}

/// Generate a barcode PNG, returns the filename.
fn generate_barcode_png(data: &str, barcode_type: &str, output_dir: &Path) -> Result<String> {
    use barcoders::sym;

    let encoded: Vec<u8> = match barcode_type {
        "code128" => sym::code128::Code128::new(data)
            .map_err(|e| anyhow::anyhow!("Code128 error: {}", e))?
            .encode(),
        "code39" => sym::code39::Code39::new(data)
            .map_err(|e| anyhow::anyhow!("Code39 error: {}", e))?
            .encode(),
        "ean13" => sym::ean13::EAN13::new(data)
            .map_err(|e| anyhow::anyhow!("EAN13 error: {}", e))?
            .encode(),
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported barcode type: {}",
                barcode_type
            ))
        }
    };

    // Convert bars to PNG image
    let bar_width = 2u32;
    let img_height = 80u32;
    let img_width = encoded.len() as u32 * bar_width;

    let mut img = image::GrayImage::new(img_width, img_height);
    for pixel in img.pixels_mut() {
        *pixel = Luma([255u8]);
    }

    for (i, &bar) in encoded.iter().enumerate() {
        if bar == 1 {
            let x_start = i as u32 * bar_width;
            for dx in 0..bar_width {
                for y in 0..img_height {
                    img.put_pixel(x_start + dx, y, Luma([0u8]));
                }
            }
        }
    }

    let filename = format!("barcode_{}.png", nanoid!(8));
    let path = output_dir.join(&filename);
    img.save(&path)?;

    Ok(filename)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_extract_attr() {
        let attrs = r#"value="https://example.com" size="3cm""#;
        assert_eq!(
            extract_attr(attrs, "value"),
            Some("https://example.com".to_string())
        );
        assert_eq!(extract_attr(attrs, "size"), Some("3cm".to_string()));
        assert_eq!(extract_attr(attrs, "missing"), None);
    }

    #[test]
    fn test_qr_generation() {
        let dir = TempDir::new().unwrap();
        let filename = generate_qr_png("https://dgii.gov.do/test", dir.path()).unwrap();
        assert!(filename.starts_with("qr_"));
        assert!(filename.ends_with(".png"));
        assert!(dir.path().join(&filename).exists());
    }

    #[test]
    fn test_barcode_generation_code39() {
        let dir = TempDir::new().unwrap();
        let filename = generate_barcode_png("ABC123", "code39", dir.path()).unwrap();
        assert!(filename.starts_with("barcode_"));
        assert!(dir.path().join(&filename).exists());
    }

    #[test]
    fn test_barcode_generation_ean13() {
        let dir = TempDir::new().unwrap();
        let filename = generate_barcode_png("5901234123457", "ean13", dir.path()).unwrap();
        assert!(filename.starts_with("barcode_"));
        assert!(dir.path().join(&filename).exists());
    }

    #[test]
    fn test_process_qr_directive() {
        let dir = TempDir::new().unwrap();
        let source = r#"Some text before
{{qr value="https://test.com" size="2cm"}}
Some text after"#;

        let result = process_directives(source, dir.path()).unwrap();
        assert!(!result.contains("{{qr"));
        assert!(result.contains("#image(\"qr_"));
        assert!(result.contains("width: 2cm"));
    }

    #[test]
    fn test_process_barcode_directive() {
        let dir = TempDir::new().unwrap();
        let source = r#"{{barcode value="ABC123" type="code39" height="1.5cm" width="8cm"}}"#;

        let result = process_directives(source, dir.path()).unwrap();
        assert!(!result.contains("{{barcode"));
        assert!(result.contains("#image(\"barcode_"));
        assert!(result.contains("width: 8cm"));
        assert!(result.contains("height: 1.5cm"));
    }
}
