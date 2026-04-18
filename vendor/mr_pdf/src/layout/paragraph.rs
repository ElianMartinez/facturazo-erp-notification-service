use crate::font::FontId;
use crate::layout::text::escape_pdf_str;
use crate::{Align, Pdf};
use std::io::Write;

/// A builder for multi-line, word-wrapped text blocks.
pub struct Paragraph<'a, W: Write> {
    pdf: &'a mut Pdf<W>,
    text: String,
    font: Option<FontId>,
    size: f64,
    align: Align,
    max_width: Option<f64>,
    line_spacing: f64,
}

impl<'a, W: Write> Paragraph<'a, W> {
    pub fn new(pdf: &'a mut Pdf<W>, text: &str) -> Self {
        let font = pdf.current_font;
        Self {
            pdf,
            text: text.to_string(),
            font,
            size: 12.0,
            align: Align::Left,
            max_width: None,
            line_spacing: 1.2,
        }
    }

    /// Sets the font size for the paragraph.
    pub fn size(mut self, size: f64) -> Self {
        self.size = size;
        self
    }

    /// Sets the horizontal alignment of the text.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Sets the maximum width of the paragraph before wrapping.
    pub fn max_width(mut self, w: f64) -> Self {
        self.max_width = Some(w);
        self
    }

    /// Sets the vertical spacing multiplier between lines (default is 1.2).
    pub fn line_spacing(mut self, s: f64) -> Self {
        self.line_spacing = s;
        self
    }

    /// Centers the text within the paragraph.
    pub fn align_center(self) -> Self {
        self.align(Align::Center)
    }
}

impl<'a, W: Write> Drop for Paragraph<'a, W> {
    fn drop(&mut self) {
        let _ = self.pdf.ensure_page_pub();

        let page_w = self.pdf.page_width;
        let margin = self.pdf.margin_pub();
        let available_width = self.max_width.unwrap_or(page_w - margin * 2.0);

        match self.font {
            Some(font_id) => {
                let words: Vec<&str> = self.text.split_whitespace().collect();
                let mut lines: Vec<String> = Vec::new();
                let mut current = String::new();

                for word in &words {
                    let test = if current.is_empty() {
                        word.to_string()
                    } else {
                        format!("{} {}", current, word)
                    };
                    if self
                        .pdf
                        .font_manager
                        .string_width(font_id, &test, self.size)
                        > available_width
                        && !current.is_empty()
                    {
                        lines.push(current);
                        current = word.to_string();
                    } else {
                        current = test;
                    }
                }
                if !current.is_empty() {
                    lines.push(current);
                }

                let h = self.pdf.font_manager.line_height(font_id, self.size);
                for line in lines {
                    let _ = self.pdf.check_page_break(h * self.line_spacing);
                    let (x, y) = self.pdf.cursor_pos();
                    let lw = self
                        .pdf
                        .font_manager
                        .string_width(font_id, &line, self.size);
                    let x_off = match self.align {
                        Align::Left => x,
                        Align::Center => margin + (available_width - lw) / 2.0,
                        Align::Right => margin + available_width - lw,
                    };
                    let encoded = self.pdf.font_manager.encode_text(font_id, &line);
                    let stream = self.pdf.get_stream();
                    stream.push_str("BT\n");
                    stream.push_str(&format!("/F{} {:.1} Tf\n", font_id.0, self.size));
                    stream.push_str(&format!("{:.2} {:.2} Td\n", x_off, y - h));
                    stream.push_str(&format!("{} Tj\n", encoded));
                    stream.push_str("ET\n");
                    self.pdf.advance_cursor(h * self.line_spacing);
                }
            }

            None => {
                let char_w = self.size * 0.5;
                let h = self.size * 1.2;
                let max_chars = (available_width / char_w).floor() as usize;

                let words: Vec<&str> = self.text.split_whitespace().collect();
                let mut lines: Vec<String> = Vec::new();
                let mut current = String::new();

                for word in &words {
                    let test = if current.is_empty() {
                        word.to_string()
                    } else {
                        format!("{} {}", current, word)
                    };
                    if test.len() > max_chars && !current.is_empty() {
                        lines.push(current);
                        current = word.to_string();
                    } else {
                        current = test;
                    }
                }
                if !current.is_empty() {
                    lines.push(current);
                }

                for line in lines {
                    let _ = self.pdf.check_page_break(h * self.line_spacing);
                    let (x, y) = self.pdf.cursor_pos();
                    let lw = line.len() as f64 * char_w;
                    let x_off = match self.align {
                        Align::Left => x,
                        Align::Center => margin + (available_width - lw) / 2.0,
                        Align::Right => (margin + available_width - lw).max(margin),
                    };
                    let escaped = escape_pdf_str(&line);
                    let stream = self.pdf.get_stream();
                    stream.push_str("BT\n");
                    stream.push_str(&format!("/FBuiltin {:.1} Tf\n", self.size));
                    stream.push_str(&format!("{:.2} {:.2} Td\n", x_off, y - h));
                    stream.push_str(&format!("({}) Tj\n", escaped));
                    stream.push_str("ET\n");
                    self.pdf.advance_cursor(h * self.line_spacing);
                }
            }
        }
    }
}
