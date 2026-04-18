use crate::pdf::writer::PdfWriter;
use std::collections::HashMap;
use std::fs;
use std::io::Write;

/// Unique identifier for a registered font.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontId(pub usize);

pub struct Font {
    pub name: String,
    pub path: String,
    pub raw_data: Vec<u8>,
    pub used_chars: std::cell::RefCell<std::collections::HashSet<char>>,
    pub glyph_to_char: std::cell::RefCell<std::collections::HashMap<u16, char>>,
}

/// Manages font registration, embedding, and text metrics.
pub struct FontManager {
    fonts: Vec<Font>,
    name_to_id: HashMap<String, FontId>,
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FontManager {
    pub fn new() -> Self {
        Self {
            fonts: Vec::new(),
            name_to_id: HashMap::new(),
        }
    }

    /// Reads a TTF file and registers it with the manager.
    pub fn register_font(&mut self, name: &str, path: &str) -> std::io::Result<FontId> {
        let raw_data = fs::read(path)?;
        let id = FontId(self.fonts.len());
        self.fonts.push(Font {
            name: name.to_string(),
            path: path.to_string(),
            raw_data,
            used_chars: std::cell::RefCell::new(std::collections::HashSet::new()),
            glyph_to_char: std::cell::RefCell::new(std::collections::HashMap::new()),
        });
        self.name_to_id.insert(name.to_string(), id);
        Ok(id)
    }

    /// Returns the FontId associated with a registered name.
    pub fn get_font_id(&self, name: &str) -> Option<FontId> {
        self.name_to_id.get(name).copied()
    }

    pub fn get_font(&self, id: FontId) -> &Font {
        &self.fonts[id.0]
    }

    /// Returns the name of the font associated with the given FontId.
    pub fn get_font_name(&self, id: FontId) -> Option<&str> {
        self.fonts.get(id.0).map(|f| f.name.as_str())
    }

    /// Embeds all registered fonts into the PDF document.
    pub fn embed_fonts<W: Write>(&mut self, writer: &mut PdfWriter<W>) -> std::io::Result<String> {
        let mut resources = String::new();
        for (i, font) in self.fonts.iter().enumerate() {
            let res = crate::font::embed::embed_ttf(writer, font)?;
            resources.push_str(&format!("/F{} {} 0 R ", i, res));
        }
        Ok(resources)
    }

    /// Calculates the visual width of a string for a given font and size.
    pub fn string_width(&self, id: FontId, text: &str, size: f64) -> f64 {
        let font = self.get_font(id);
        if let Ok(face) = ttf_parser::Face::parse(&font.raw_data, 0) {
            let units_per_em = face.units_per_em() as f64;
            let mut width = 0.0;
            for c in text.chars() {
                if let Some(glyph_id) = face.glyph_index(c)
                    && let Some(w) = face.glyph_hor_advance(glyph_id)
                {
                    width += w as f64;
                }
            }
            return (width / units_per_em) * size;
        }
        0.0
    }

    /// Returns the full line height (ascent - descent).
    pub fn line_height(&self, id: FontId, size: f64) -> f64 {
        let (a, d) = self.get_ascent_descent(id, size);
        a - d
    }

    /// Retrieves the scaled ascent and descent values.
    pub fn get_ascent_descent(&self, id: FontId, size: f64) -> (f64, f64) {
        let font = self.get_font(id);
        if let Ok(face) = ttf_parser::Face::parse(&font.raw_data, 0) {
            let units_per_em = face.units_per_em() as f64;
            let ascender = face.ascender() as f64;
            let descender = face.descender() as f64;
            return (
                (ascender / units_per_em) * size,
                (descender / units_per_em) * size,
            );
        }
        (size * 0.8, -size * 0.2)
    }

    /// Encodes a UTF-8 string into PDF Hexadecimal format using Glyph IDs.
    pub fn encode_text(&self, id: FontId, text: &str) -> String {
        let font = self.get_font(id);

        {
            let mut chars = font.used_chars.borrow_mut();
            for c in text.chars() {
                chars.insert(c);
            }
        }

        let mut hex = String::with_capacity(text.len() * 4 + 2);
        hex.push('<');
        if let Ok(face) = ttf_parser::Face::parse(&font.raw_data, 0) {
            let mut g2c = font.glyph_to_char.borrow_mut();
            for c in text.chars() {
                let gid = face.glyph_index(c).map(|g| g.0).unwrap_or(0);
                g2c.insert(gid, c);
                hex.push_str(&format!("{:04X}", gid));
            }
        } else {
            for c in text.chars() {
                hex.push_str(&format!("{:04X}", c as u32));
            }
        }
        hex.push('>');
        hex
    }
}
