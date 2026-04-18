use crate::pdf::writer::PdfWriter;
use flate2::Compression;
use flate2::write::ZlibEncoder;

use std::fs;
use std::io::Write;

/// A struct representing an image to be embedded in a PDF.
/// Supports both JPEG and PNG formats.
#[derive(Clone, Debug)]
pub struct Image {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub compressed_data: Vec<u8>,
    pub is_jpeg: bool,
}

impl Image {
    /// Loads an image from a file path.
    pub fn load(name: &str, path: &str) -> std::io::Result<Self> {
        let bytes = fs::read(path)?;
        let ext = path.split('.').next_back().unwrap_or("").to_lowercase();
        let is_jpeg = ext == "jpg" || ext == "jpeg";
        Self::from_bytes(name, &bytes, is_jpeg)
    }

    /// Creates an image from raw bytes.
    pub fn from_bytes(name: &str, bytes: &[u8], is_jpeg: bool) -> std::io::Result<Self> {
        let width;
        let height;
        let compressed_data;

        if is_jpeg {
            let img = image::load_from_memory(bytes)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            width = img.width();
            height = img.height();
            compressed_data = bytes.to_vec();
        } else {
            let img = image::load_from_memory(bytes)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            let rgb = img.to_rgb8();
            width = rgb.width();
            height = rgb.height();

            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(rgb.as_raw())?;
            compressed_data = encoder.finish()?;
        }

        Ok(Self {
            name: name.to_string(),
            width,
            height,
            compressed_data,
            is_jpeg,
        })
    }

    /// Decodes an image from a Base64 string.
    pub fn from_base64(name: &str, base64_str: &str) -> std::io::Result<Self> {
        use base64::{Engine as _, engine::general_purpose};

        let raw = base64_str.trim();
        let clean_b64 = if let Some(pos) = raw.find(";base64,") {
            &raw[pos + 8..]
        } else {
            raw
        };

        let bytes = general_purpose::STANDARD
            .decode(clean_b64)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let is_jpeg = bytes.len() > 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF;

        Self::from_bytes(name, &bytes, is_jpeg)
    }

    /// Low-level function to embed the image as a PDF XObject.
    pub fn embed<W: Write>(&self, writer: &mut PdfWriter<W>) -> std::io::Result<u32> {
        let xobj_id = writer.alloc_id();
        writer.start_obj(xobj_id)?;

        let filter = if self.is_jpeg {
            "/DCTDecode"
        } else {
            "/FlateDecode"
        };
        let dict = format!(
            "<< /Type /XObject\n   /Subtype /Image\n   /Width {}\n   /Height {}\n   /ColorSpace /DeviceRGB\n   /BitsPerComponent 8\n   /Filter {}\n   /Length {} >>\n",
            self.width,
            self.height,
            filter,
            self.compressed_data.len()
        );

        writer.write_raw(dict.as_bytes())?;
        writer.write_raw(b"stream\n")?;
        writer.write_raw(&self.compressed_data)?;
        writer.write_raw(b"\nendstream\n")?;
        writer.end_obj()?;

        Ok(xobj_id)
    }
}
