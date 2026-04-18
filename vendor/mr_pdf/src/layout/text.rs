use crate::font::FontId;
use crate::{Align, Pdf};
use std::io::Write;

/// A text block represents a single piece of text with specific styling.
/// It is rendered to the PDF when the block is dropped (at the end of its scope).
pub struct TextBlock<'a, W: Write> {
    pdf: &'a mut Pdf<W>,
    text: String,
    font: Option<FontId>,
    size: f64,
    align: Align,
    max_width: Option<f64>,
    wrap: bool,
    margin_top: f64,
    margin_bottom: f64,
    margin_left: f64,
    margin_right: f64,
    link: Option<String>,
    color: Option<crate::Color>,
    bold: bool,
    fill_char: Option<char>,
}

impl<'a, W: Write> TextBlock<'a, W> {
    pub fn new(pdf: &'a mut Pdf<W>, text: &str) -> Self {
        let font = pdf.current_font;
        Self {
            pdf,
            text: text.to_string(),
            font,
            size: 12.0,
            align: Align::Left,
            max_width: None,
            wrap: true,
            margin_top: 0.0,
            margin_bottom: 0.0,
            margin_left: 0.0,
            margin_right: 0.0,
            link: None,
            color: None,
            bold: false,
            fill_char: None,
        }
    }

    /// Sets the font size.
    pub fn size(mut self, size: f64) -> Self {
        self.size = size;
        self
    }

    /// Sets the text alignment.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Centers the text horizontally.
    pub fn align_center(self) -> Self {
        self.align(Align::Center)
    }

    pub fn align_left(self) -> Self {
        self.align(Align::Left)
    }

    /// Aligns the text to the right.
    pub fn align_right(self) -> Self {
        self.align(Align::Right)
    }

    /// Sets the maximum width for the text block.
    pub fn max_width(mut self, w: f64) -> Self {
        self.max_width = Some(w);
        self
    }

    /// Enables or disables word-wrapping.
    pub fn wrap(mut self, w: bool) -> Self {
        self.wrap = w;
        self
    }

    /// Adds a margin at the top of the text block.
    pub fn margin_top(mut self, m: f64) -> Self {
        self.margin_top = m;
        self
    }

    /// Adds a margin at the bottom of the text block.
    pub fn margin_bottom(mut self, m: f64) -> Self {
        self.margin_bottom = m;
        self
    }

    /// Adds a margin at the left of the text block.
    pub fn margin_left(mut self, m: f64) -> Self {
        self.margin_left = m;
        self
    }

    /// Adds a margin at the right of the text block.
    pub fn margin_right(mut self, m: f64) -> Self {
        self.margin_right = m;
        self
    }

    /// Sets the font to be used from the registered fonts.
    pub fn font(mut self, name: &str) -> Self {
        self.font = self.pdf.font_manager.get_font_id(name);
        self
    }

    /// Adds a clickable hyperlink to the text.
    pub fn link(mut self, url: &str) -> Self {
        self.link = Some(url.to_string());
        self
    }

    /// Sets the text color.
    pub fn color(mut self, color: crate::Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Sets the text to be bold.
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Pads the text at the end to reaching the specified character length using the filler character.
    pub fn pad_end(mut self, len: usize, filler: char) -> Self {
        let current_len = self.text.chars().count();
        if current_len < len {
            for _ in 0..(len - current_len) {
                self.text.push(filler);
            }
        }
        self
    }

    /// Pads the text at the start to reaching the specified character length using the filler character.
    pub fn pad_start(mut self, len: usize, filler: char) -> Self {
        let current_len = self.text.chars().count();
        if current_len < len {
            let mut prefix = String::new();
            for _ in 0..(len - current_len) {
                prefix.push(filler);
            }
            self.text.insert_str(0, &prefix);
        }
        self
    }

    /// Automatically expands and fills the remaining space on the line with the filler character.
    pub fn fill(mut self, filler: char) -> Self {
        self.fill_char = Some(filler);
        self
    }
}

impl<'a, W: Write> Drop for TextBlock<'a, W> {
    fn drop(&mut self) {
        let _ = self.pdf.ensure_page_pub();

        let margin = self.pdf.margin_pub() + self.margin_left;
        let available = (self.max_width.unwrap_or(self.pdf.content_width())
            - self.margin_left
            - self.margin_right)
            .max(1.0);

        if self.margin_top > 0.0 {
            self.pdf.advance_cursor(self.margin_top);
        }

        match self.font {
            Some(mut font_id) => {
                if self.bold {
                    if let Some(name) = self.pdf.font_manager.get_font_name(font_id) {
                        let bold_name = format!("{}-Bold", name);
                        if let Some(bid) = self.pdf.font_manager.get_font_id(&bold_name) {
                            font_id = bid;
                        }
                    }
                }
                let (ascent, descent) =
                    self.pdf.font_manager.get_ascent_descent(font_id, self.size);
                let line_h = ascent - descent;

                let lines = word_wrap_ttf(
                    &self.pdf.font_manager,
                    font_id,
                    &self.text,
                    self.size,
                    available,
                    self.wrap,
                );

                let line_count = lines.len();
                for (i, line) in lines.iter().enumerate() {
                    let mut line_to_draw = line.clone();

                    // Only fill the last line if it's a multi-line block
                    if let Some(fc) = self.fill_char
                        && i == line_count - 1
                    {
                        let current_w =
                            self.pdf
                                .font_manager
                                .string_width(font_id, &line_to_draw, self.size);
                        let _ = self.pdf.font_manager.string_width(font_id, " ", self.size);

                        let remaining_w = (available - current_w - 1.0).max(0.0); // 1.0pt safety margin
                        if remaining_w > 0.0 {
                            let fc_w = self.pdf.font_manager.string_width(
                                font_id,
                                &fc.to_string(),
                                self.size,
                            );
                            if fc_w > 0.0 {
                                let count = (remaining_w / fc_w).floor() as usize;
                                match self.align {
                                    Align::Left => {
                                        if count > 2 {
                                            line_to_draw.push(' ');
                                            for _ in 0..(count - 1) {
                                                line_to_draw.push(fc);
                                            }
                                        } else {
                                            for _ in 0..count {
                                                line_to_draw.push(fc);
                                            }
                                        }
                                    }
                                    Align::Right => {
                                        if count > 2 {
                                            line_to_draw.insert(0, ' ');
                                            for _ in 0..(count - 1) {
                                                line_to_draw.insert(0, fc);
                                            }
                                        } else {
                                            for _ in 0..count {
                                                line_to_draw.insert(0, fc);
                                            }
                                        }
                                    }
                                    Align::Center => {
                                        if count > 4 {
                                            let sub_count = count - 2;
                                            let half = sub_count / 2;
                                            line_to_draw.push(' ');
                                            for _ in 0..(sub_count - half) {
                                                line_to_draw.push(fc);
                                            }
                                            line_to_draw.insert(0, ' ');
                                            for _ in 0..half {
                                                line_to_draw.insert(0, fc);
                                            }
                                        } else {
                                            let half = count / 2;
                                            for _ in 0..half {
                                                line_to_draw.insert(0, fc);
                                            }
                                            for _ in 0..(count - half) {
                                                line_to_draw.push(fc);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    let _ = self.pdf.check_page_break(line_h);
                    let (x, y) = self.pdf.cursor_pos();

                    let text_w =
                        self.pdf
                            .font_manager
                            .string_width(font_id, &line_to_draw, self.size);
                    let x_off =
                        x_offset(self.align, x + self.margin_left, margin, available, text_w);

                    let baseline = y - ascent;

                    if let Some(c) = &self.color {
                        let _ = self.pdf.set_fill_color(*c);
                    }

                    let encoded = self.pdf.font_manager.encode_text(font_id, &line_to_draw);
                    let s = self.pdf.get_stream();
                    s.push_str("BT\n");
                    s.push_str(&format!("/F{} {:.1} Tf\n", font_id.0, self.size));
                    s.push_str(&format!("{:.2} {:.2} Td\n", x_off, baseline));
                    s.push_str(&format!("{} Tj\n", encoded));
                    s.push_str("ET\n");

                    if self.color.is_some() {
                        let _ = self.pdf.set_fill_color(crate::Color::Rgb(0, 0, 0));
                    }

                    if let Some(url) = &self.link {
                        self.pdf.add_link(
                            (x_off, baseline + descent, x_off + text_w, baseline + ascent),
                            url,
                        );
                    }

                    self.pdf.advance_cursor(line_h * self.pdf.line_spacing);

                    if i == lines.len() - 1 && self.margin_bottom > 0.0 {
                        self.pdf.advance_cursor(self.margin_bottom);
                    }
                }
            }

            None => {
                let ascent = self.size * 0.8;
                let descent = -self.size * 0.2;
                let line_h = ascent - descent;
                let char_w = self.size * 0.52;

                let lines = word_wrap_helvetica(&self.text, available, char_w, self.wrap);

                let line_count = lines.len();
                for (i, line) in lines.iter().enumerate() {
                    let mut line_to_draw = line.clone();

                    if let Some(fc) = self.fill_char
                        && i == line_count - 1
                    {
                        let fc_w = helvetica_char_width(fc) * self.size;
                        let _ = helvetica_char_width(' ') * self.size;
                        let line_raw_w = helvetica_string_width(&line_to_draw) * self.size;
                        let remaining_w = (available - line_raw_w - 1.0).max(0.0);
                        if remaining_w > 0.0 {
                            let count = (remaining_w / fc_w).floor() as usize;
                            match self.align {
                                Align::Left => {
                                    if count > 2 {
                                        line_to_draw.push(' ');
                                        for _ in 0..(count - 1) {
                                            line_to_draw.push(fc);
                                        }
                                    } else {
                                        for _ in 0..count {
                                            line_to_draw.push(fc);
                                        }
                                    }
                                }
                                Align::Right => {
                                    if count > 2 {
                                        line_to_draw.insert(0, ' ');
                                        for _ in 0..(count - 1) {
                                            line_to_draw.insert(0, fc);
                                        }
                                    } else {
                                        for _ in 0..count {
                                            line_to_draw.insert(0, fc);
                                        }
                                    }
                                }
                                Align::Center => {
                                    if count > 4 {
                                        let sub_count = count - 2;
                                        let half = sub_count / 2;
                                        line_to_draw.push(' ');
                                        for _ in 0..(sub_count - half) {
                                            line_to_draw.push(fc);
                                        }
                                        line_to_draw.insert(0, ' ');
                                        for _ in 0..half {
                                            line_to_draw.insert(0, fc);
                                        }
                                    } else {
                                        let half = count / 2;
                                        for _ in 0..half {
                                            line_to_draw.insert(0, fc);
                                        }
                                        for _ in 0..(count - half) {
                                            line_to_draw.push(fc);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    let _ = self.pdf.check_page_break(line_h);
                    let (x, y) = self.pdf.cursor_pos();

                    let text_w = helvetica_string_width(&line_to_draw) * self.size;
                    let x_off =
                        x_offset(self.align, x + self.margin_left, margin, available, text_w);
                    let baseline = y - ascent;

                    if let Some(c) = &self.color {
                        let _ = self.pdf.set_fill_color(*c);
                    }

                    let escaped = escape_pdf_str(&line_to_draw);
                    let s = self.pdf.get_stream();
                    s.push_str("BT\n");
                    s.push_str(&format!("/FBuiltin {:.1} Tf\n", self.size));
                    s.push_str(&format!("{:.2} {:.2} Td\n", x_off, baseline));
                    s.push_str(&format!("({}) Tj\n", escaped));
                    s.push_str("ET\n");

                    if self.color.is_some() {
                        let _ = self.pdf.set_fill_color(crate::Color::Rgb(0, 0, 0));
                    }

                    if let Some(url) = &self.link {
                        self.pdf.add_link(
                            (x_off, baseline + descent, x_off + text_w, baseline + ascent),
                            url,
                        );
                    }

                    self.pdf.advance_cursor(line_h * self.pdf.line_spacing);

                    if i == lines.len() - 1 && self.margin_bottom > 0.0 {
                        self.pdf.advance_cursor(self.margin_bottom);
                    }
                }
            }
        }
    }
}

fn x_offset(align: Align, cursor_x: f64, margin: f64, available: f64, text_w: f64) -> f64 {
    match align {
        Align::Left => cursor_x,
        Align::Center => margin + (available - text_w) / 2.0,
        Align::Right => (margin + available - text_w).max(margin),
    }
}

fn word_wrap_ttf(
    fm: &crate::font::FontManager,
    font_id: crate::font::FontId,
    text: &str,
    size: f64,
    available: f64,
    do_wrap: bool,
) -> Vec<String> {
    if !do_wrap {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", current, word)
        };
        if fm.string_width(font_id, &candidate, size) > available && !current.is_empty() {
            lines.push(current);
            current = word.to_string();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn word_wrap_helvetica(text: &str, available: f64, char_w: f64, do_wrap: bool) -> Vec<String> {
    if !do_wrap {
        return vec![text.to_string()];
    }
    let max_chars = (available / char_w).floor() as usize;
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", current, word)
        };
        if candidate.len() > max_chars && !current.is_empty() {
            lines.push(current);
            current = word.to_string();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub fn escape_pdf_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        match c {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) >= 0x80 => {
                // PATCH: convert non-ASCII Unicode chars to WinAnsi single-byte
                // octal escape so built-in Helvetica (WinAnsiEncoding) renders correctly.
                if let Some(byte) = unicode_to_winansi(c) {
                    out.push_str(&format!("\\{:03o}", byte));
                } else {
                    out.push('?');
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Maps Unicode codepoints to WinAnsi (Windows-1252) byte values.
fn unicode_to_winansi(c: char) -> Option<u8> {
    let code = c as u32;
    if code < 0x80 {
        return Some(code as u8);
    }
    if (0xA0..=0xFF).contains(&code) {
        return Some(code as u8);
    }
    match code {
        0x20AC => Some(0x80), // €
        0x201A => Some(0x82),
        0x0192 => Some(0x83),
        0x201E => Some(0x84),
        0x2026 => Some(0x85), // …
        0x2020 => Some(0x86),
        0x2021 => Some(0x87),
        0x02C6 => Some(0x88),
        0x2030 => Some(0x89),
        0x0160 => Some(0x8A),
        0x2039 => Some(0x8B),
        0x0152 => Some(0x8C),
        0x017D => Some(0x8E),
        0x2018 => Some(0x91),
        0x2019 => Some(0x92),
        0x201C => Some(0x93),
        0x201D => Some(0x94),
        0x2022 => Some(0x95), // •
        0x2013 => Some(0x96), // – en-dash
        0x2014 => Some(0x97), // — em-dash
        0x02DC => Some(0x98),
        0x2122 => Some(0x99),
        0x0161 => Some(0x9A),
        0x203A => Some(0x9B),
        0x0153 => Some(0x9C),
        0x017E => Some(0x9E),
        0x0178 => Some(0x9F),
        _ => None,
    }
}

pub fn helvetica_char_width(c: char) -> f64 {
    match c {
        ' ' => 0.278,
        '!' => 0.278,
        '"' => 0.355,
        '#' => 0.556,
        '$' => 0.556,
        '%' => 0.889,
        '&' => 0.667,
        '\'' => 0.191,
        '(' => 0.333,
        ')' => 0.333,
        '*' => 0.389,
        '+' => 0.584,
        ',' => 0.278,
        '-' => 0.333,
        '.' => 0.278,
        '/' => 0.278,
        '0'..='9' => 0.556,
        ':' => 0.278,
        ';' => 0.278,
        '<' => 0.584,
        '=' => 0.584,
        '>' => 0.584,
        '?' => 0.556,
        '@' => 1.015,
        'A' => 0.667,
        'B' => 0.667,
        'C' => 0.722,
        'D' => 0.722,
        'E' => 0.667,
        'F' => 0.611,
        'G' => 0.778,
        'H' => 0.722,
        'I' => 0.278,
        'J' => 0.5,
        'K' => 0.667,
        'L' => 0.556,
        'M' => 0.833,
        'N' => 0.722,
        'O' => 0.778,
        'P' => 0.667,
        'Q' => 0.778,
        'R' => 0.722,
        'S' => 0.667,
        'T' => 0.611,
        'U' => 0.722,
        'V' => 0.667,
        'W' => 0.944,
        'X' => 0.667,
        'Y' => 0.667,
        'Z' => 0.611,
        '[' => 0.278,
        '\\' => 0.278,
        ']' => 0.278,
        '^' => 0.469,
        '_' => 0.5,
        '`' => 0.333,
        'a' => 0.556,
        'b' => 0.556,
        'c' => 0.5,
        'd' => 0.556,
        'e' => 0.556,
        'f' => 0.278,
        'g' => 0.556,
        'h' => 0.556,
        'i' => 0.222,
        'j' => 0.222,
        'k' => 0.5,
        'l' => 0.222,
        'm' => 0.833,
        'n' => 0.556,
        'o' => 0.556,
        'p' => 0.556,
        'q' => 0.556,
        'r' => 0.333,
        's' => 0.5,
        't' => 0.278,
        'u' => 0.556,
        'v' => 0.5,
        'w' => 0.722,
        'x' => 0.5,
        'y' => 0.5,
        'z' => 0.5,
        '{' => 0.334,
        '|' => 0.260,
        '}' => 0.334,
        '~' => 0.584,
        _ => 0.556,
    }
}

pub fn helvetica_string_width(s: &str) -> f64 {
    s.chars().map(helvetica_char_width).sum()
}

#[derive(Clone)]
pub struct RichTextSpan {
    pub text: String,
    pub bold: bool,
    pub color: Option<crate::Color>,
    pub size: Option<f64>,
    pub margin_left: f64,
    pub fill_char: Option<char>,
    pub align: Option<Align>,
}

impl RichTextSpan {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            bold: false,
            color: None,
            size: None,
            margin_left: 0.0,
            fill_char: None,
            align: None,
        }
    }
    pub fn bold(&mut self) -> &mut Self {
        self.bold = true;
        self
    }
    pub fn color(&mut self, color: crate::Color) -> &mut Self {
        self.color = Some(color);
        self
    }
    pub fn size(&mut self, size: f64) -> &mut Self {
        self.size = Some(size);
        self
    }
    pub fn margin_left(&mut self, m: f64) -> &mut Self {
        self.margin_left = m;
        self
    }
    /// Pads the text content at the end to reaching the specified character length using the filler character.
    pub fn pad_end(&mut self, len: usize, filler: char) -> &mut Self {
        let current_len = self.text.chars().count();
        if current_len < len {
            for _ in 0..(len - current_len) {
                self.text.push(filler);
            }
        }
        self
    }
    /// Pads the text content at the start to reaching the specified character length using the filler character.
    pub fn pad_start(&mut self, len: usize, filler: char) -> &mut Self {
        let current_len = self.text.chars().count();
        if current_len < len {
            let mut prefix = String::new();
            for _ in 0..(len - current_len) {
                prefix.push(filler);
            }
            self.text.insert_str(0, &prefix);
        }
        self
    }
    pub fn fill(&mut self, filler: char) -> &mut Self {
        self.fill_char = Some(filler);
        self
    }
    pub fn align(&mut self, align: Align) -> &mut Self {
        self.align = Some(align);
        self
    }
}

pub struct RichTextBuilder {
    pub spans: Vec<RichTextSpan>,
}

impl RichTextBuilder {
    pub fn new() -> Self {
        Self { spans: Vec::new() }
    }
    pub fn span(&mut self, text: &str) -> &mut RichTextSpan {
        self.spans.push(RichTextSpan::new(text));
        self.spans.last_mut().unwrap()
    }
}

pub struct RichTextBlock<'a, W: Write> {
    pdf: &'a mut Pdf<W>,
    builder: RichTextBuilder,
    font: Option<FontId>,
    size: f64,
    align: Align,
    max_width: Option<f64>,
    wrap: bool,
    margin_top: f64,
    margin_bottom: f64,
    margin_left: f64,
    margin_right: f64,
}

impl<'a, W: Write> RichTextBlock<'a, W> {
    pub fn new<F>(pdf: &'a mut Pdf<W>, build_fn: F) -> Self
    where
        F: FnOnce(&mut RichTextBuilder),
    {
        let mut builder = RichTextBuilder::new();
        build_fn(&mut builder);
        let font = pdf.current_font;
        Self {
            pdf,
            builder,
            font,
            size: 12.0,
            align: Align::Left,
            max_width: None,
            wrap: true,
            margin_top: 0.0,
            margin_bottom: 0.0,
            margin_left: 0.0,
            margin_right: 0.0,
        }
    }

    pub fn size(mut self, size: f64) -> Self {
        self.size = size;
        self
    }
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }
    pub fn align_center(self) -> Self {
        self.align(Align::Center)
    }
    pub fn align_left(self) -> Self {
        self.align(Align::Left)
    }
    pub fn align_right(self) -> Self {
        self.align(Align::Right)
    }
    pub fn max_width(mut self, w: f64) -> Self {
        self.max_width = Some(w);
        self
    }
    pub fn wrap(mut self, w: bool) -> Self {
        self.wrap = w;
        self
    }
    pub fn margin_top(mut self, m: f64) -> Self {
        self.margin_top = m;
        self
    }
    pub fn margin_bottom(mut self, m: f64) -> Self {
        self.margin_bottom = m;
        self
    }
    pub fn margin_left(mut self, m: f64) -> Self {
        self.margin_left = m;
        self
    }
    pub fn margin_right(mut self, m: f64) -> Self {
        self.margin_right = m;
        self
    }
    pub fn font(mut self, name: &str) -> Self {
        self.font = self.pdf.font_manager.get_font_id(name);
        self
    }
}

impl<'a, W: Write> Drop for RichTextBlock<'a, W> {
    fn drop(&mut self) {
        let _ = self.pdf.ensure_page_pub();
        let _ = self.pdf.margin_pub() + self.margin_left;
        let available = (self.max_width.unwrap_or(self.pdf.content_width())
            - self.margin_left
            - self.margin_right)
            .max(1.0);

        if self.margin_top > 0.0 {
            self.pdf.advance_cursor(self.margin_top);
        }

        let base_id = self.font;
        let (ascent, line_h, bold_id) = match base_id {
            Some(fid) => {
                let (a, d) = self.pdf.font_manager.get_ascent_descent(fid, self.size);
                let bn = self.pdf.font_manager.get_font_name(fid).unwrap_or("");
                let b_id = self
                    .pdf
                    .font_manager
                    .get_font_id(&format!("{}-Bold", bn))
                    .unwrap_or(fid);
                (a, a - d, Some(b_id))
            }
            None => (self.size * 0.8, self.size, None),
        };

        let lines = word_wrap_rich_text(
            &self.pdf.font_manager,
            base_id,
            bold_id,
            &self.builder.spans,
            self.size,
            available,
            self.wrap,
        );

        let line_count = lines.len();
        for (i, line) in lines.iter().enumerate() {
            let mut current_line_width: f64 = line
                .iter()
                .map(|seg| {
                    let sz = seg.size.unwrap_or(self.size);
                    let w = match base_id {
                        Some(_) => {
                            let fid = if seg.bold {
                                bold_id.unwrap_or(base_id.unwrap())
                            } else {
                                base_id.unwrap()
                            };
                            self.pdf.font_manager.string_width(fid, &seg.text, sz)
                        }
                        None => helvetica_string_width(&seg.text) * sz,
                    };
                    seg.margin_left + w
                })
                .sum();

            let mut current_line_spans = line.clone();

            if let Some(filler_idx) = current_line_spans
                .iter()
                .position(|s| s.fill_char.is_some())
            {
                if i == line_count - 1 || self.wrap {
                    let filler_span = &current_line_spans[filler_idx];
                    let fc = filler_span.fill_char.unwrap();
                    let sz = filler_span.size.unwrap_or(self.size);
                    let fid = if filler_span.bold {
                        bold_id.unwrap_or(base_id.unwrap_or(FontId(0)))
                    } else {
                        base_id.unwrap_or(FontId(0))
                    };

                    let available_for_fill = (available - current_line_width - 2.0).max(0.0);
                    if available_for_fill > 0.0 {
                        let fc_w = match base_id {
                            Some(_) => self.pdf.font_manager.string_width(fid, &fc.to_string(), sz),
                            None => helvetica_char_width(fc) * sz,
                        };

                        if fc_w > 0.0 {
                            let count = (available_for_fill / fc_w).floor() as usize;
                            if count > 2 {
                                let mut filler_str = String::new();
                                filler_str.push(' ');
                                for _ in 0..(count - 1) {
                                    filler_str.push(fc);
                                }
                                filler_str.push(' ');

                                let space_w = match base_id {
                                    Some(_) => self.pdf.font_manager.string_width(fid, " ", sz),
                                    None => helvetica_char_width(' ') * sz,
                                };
                                current_line_width += (count as f64 * fc_w) + (2.0 * space_w);

                                let filler_align = filler_span.align.unwrap_or(self.align);
                                match filler_align {
                                    Align::Left => {
                                        current_line_spans[filler_idx].text.push_str(&filler_str);
                                    }
                                    Align::Right => {
                                        current_line_spans[filler_idx]
                                            .text
                                            .insert_str(0, &filler_str);
                                    }
                                    Align::Center => {
                                        if count > 4 {
                                            let sub_count = count - 2;
                                            let half = sub_count / 2;
                                            let mut f1 = String::new();
                                            f1.push(' ');
                                            for _ in 0..half {
                                                f1.push(fc);
                                            }
                                            f1.push(' ');

                                            let mut f2 = String::new();
                                            f2.push(' ');
                                            for _ in 0..(sub_count - half) {
                                                f2.push(fc);
                                            }
                                            f2.push(' ');

                                            current_line_spans[filler_idx].text.insert_str(0, &f1);
                                            current_line_spans[filler_idx].text.push_str(&f2);
                                        } else {
                                            current_line_spans[filler_idx]
                                                .text
                                                .push_str(&filler_str);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let _ = self.pdf.check_page_break(line_h);
            let (x, y) = self.pdf.cursor_pos();

            let h_shift = match self.align {
                Align::Left => 0.0,
                Align::Center => (available - current_line_width).max(0.0) / 2.0,
                Align::Right => (available - current_line_width).max(0.0),
            };

            let mut x_off = x + self.margin_left + h_shift;
            let baseline = y - ascent;

            for span in &current_line_spans {
                let sz = span.size.unwrap_or(self.size);
                let seg_w = match base_id {
                    Some(_) => {
                        let fid = if span.bold {
                            bold_id.unwrap_or(base_id.unwrap())
                        } else {
                            base_id.unwrap()
                        };
                        self.pdf.font_manager.string_width(fid, &span.text, sz)
                    }
                    None => helvetica_string_width(&span.text) * sz,
                };

                x_off += span.margin_left;

                if let Some(c) = &span.color {
                    let _ = self.pdf.set_fill_color(*c);
                } else {
                    let _ = self.pdf.set_fill_color(crate::Color::Rgb(0, 0, 0));
                }

                match base_id {
                    Some(_) => {
                        let fid = if span.bold {
                            bold_id.unwrap_or(base_id.unwrap())
                        } else {
                            base_id.unwrap()
                        };
                        let encoded = self.pdf.font_manager.encode_text(fid, &span.text);
                        let s = self.pdf.get_stream();
                        s.push_str("BT\n");
                        s.push_str(&format!("/F{} {:.1} Tf\n", fid.0, sz));
                        s.push_str(&format!("{:.2} {:.2} Td\n", x_off, baseline));
                        s.push_str(&format!("{} Tj\n", encoded));
                    }
                    None => {
                        let escaped = escape_pdf_str(&span.text);
                        let s = self.pdf.get_stream();
                        s.push_str("BT\n");
                        s.push_str(&format!("/FBuiltin {:.1} Tf\n", sz));
                        s.push_str(&format!("{:.2} {:.2} Td\n", x_off, baseline));
                        s.push_str(&format!("({}) Tj\n", escaped));
                    }
                }
                let s = self.pdf.get_stream();
                s.push_str("ET\n");
                x_off += seg_w;
            }

            self.pdf.advance_cursor(line_h * self.pdf.line_spacing);
            if i == line_count - 1 && self.margin_bottom > 0.0 {
                self.pdf.advance_cursor(self.margin_bottom);
            }
        }
    }
}

fn word_wrap_rich_text(
    fm: &crate::font::FontManager,
    base_font_id: Option<crate::font::FontId>,
    bold_font_id: Option<crate::font::FontId>,
    spans: &[RichTextSpan],
    size: f64,
    available: f64,
    do_wrap: bool,
) -> Vec<Vec<RichTextSpan>> {
    if !do_wrap {
        return vec![spans.to_vec()];
    }
    let mut lines = Vec::new();
    let mut current_line: Vec<RichTextSpan> = Vec::new();
    let mut current_line_width = 0.0;

    for span in spans {
        let span_lines: Vec<&str> = span.text.split('\n').collect();
        for (i, part) in span_lines.iter().enumerate() {
            if i > 0 {
                // Manual new line present in text
                lines.push(current_line);
                current_line = Vec::new();
                current_line_width = 0.0;
            }

            if part.is_empty() && span.fill_char.is_none() {
                continue;
            }

            let sz = span.size.unwrap_or(size);
            let words: Vec<&str> = if part.is_empty() {
                vec![""]
            } else {
                part.split_inclusive(char::is_whitespace).collect()
            };

            for word in words {
                let word_w = match base_font_id {
                    Some(fid) => {
                        let f_id = if span.bold {
                            bold_font_id.unwrap_or(fid)
                        } else {
                            fid
                        };
                        fm.string_width(f_id, word, sz)
                    }
                    None => helvetica_string_width(word) * sz,
                };

                // Determine if we need to apply margin for this new span start
                let is_new_span_start = current_line.is_empty()
                    || current_line.last().map_or(true, |l| {
                        l.bold != span.bold || l.color != span.color || l.size != span.size
                    });

                let effective_margin = if is_new_span_start {
                    span.margin_left
                } else {
                    0.0
                };

                if current_line_width + word_w + effective_margin > available
                    && !current_line.is_empty()
                {
                    lines.push(current_line);
                    current_line = Vec::new();
                    current_line_width = 0.0;
                }

                // Re-calculate effective margin for the potentially new line
                let is_new_span_start_final = current_line.is_empty()
                    || current_line.last().map_or(true, |l| {
                        l.bold != span.bold || l.color != span.color || l.size != span.size
                    });
                let final_margin = if is_new_span_start_final {
                    span.margin_left
                } else {
                    0.0
                };

                if let Some(last) = current_line.last_mut().filter(|l: &&mut RichTextSpan| {
                    l.bold == span.bold
                        && l.color == span.color
                        && l.size == span.size
                        && l.fill_char == span.fill_char
                }) {
                    last.text.push_str(word);
                } else {
                    current_line.push(RichTextSpan {
                        text: word.to_string(),
                        bold: span.bold,
                        color: span.color,
                        size: span.size,
                        margin_left: final_margin,
                        fill_char: span.fill_char,
                        align: span.align,
                    });
                    current_line_width += final_margin;
                }
                current_line_width += word_w;
            }
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    if lines.is_empty() {
        lines.push(vec![RichTextSpan::new("")]);
    }
    lines
}
