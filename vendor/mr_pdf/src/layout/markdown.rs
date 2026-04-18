use crate::Color;
use crate::Pdf;
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::io::Write;

pub struct MarkdownRenderer<'a, W: Write> {
    pdf: &'a mut Pdf<W>,
    markdown: String,
    base_font_size: f64,
    font_name: Option<String>,
}

impl<'a, W: Write> MarkdownRenderer<'a, W> {
    pub fn new(pdf: &'a mut Pdf<W>, markdown: &str) -> Self {
        Self {
            pdf,
            markdown: markdown.to_string(),
            base_font_size: 12.0,
            font_name: None,
        }
    }

    /// Sets the base font size for the markdown text
    pub fn size(mut self, size: f64) -> Self {
        self.base_font_size = size;
        self
    }

    /// Sets the font to be used for the entire markdown block
    pub fn font(mut self, name: &str) -> Self {
        self.font_name = Some(name.to_string());
        self
    }

    pub fn render(self) -> std::io::Result<()> {
        let parser = Parser::new(&self.markdown);
        let mut current_text = String::new();
        let mut in_list = false;

        let mut current_heading_level = None;

        for event in parser {
            match event {
                Event::Start(tag) => match tag {
                    Tag::Paragraph => current_text.clear(),
                    Tag::Heading { level, .. } => {
                        current_text.clear();
                        current_heading_level = Some(level as usize);
                    }
                    Tag::List(_) => {
                        in_list = true;
                    }
                    Tag::Item => {
                        current_text.clear();
                    }
                    Tag::CodeBlock(_) => {
                        current_text.clear();
                    }
                    _ => {}
                },
                Event::Text(t) | Event::Code(t) => {
                    current_text.push_str(&t);
                }
                Event::SoftBreak | Event::HardBreak => {
                    current_text.push(' ');
                }
                Event::End(tag) => match tag {
                    TagEnd::Paragraph => {
                        if !current_text.is_empty() && !in_list {
                            let mut txt = self
                                .pdf
                                .text(&current_text)
                                .size(self.base_font_size)
                                .margin_bottom(self.base_font_size * 0.8);
                            if let Some(f) = &self.font_name {
                                txt = txt.font(f);
                            }
                            let _ = txt;
                        }
                    }
                    TagEnd::Heading(_level) => {
                        let level = current_heading_level.unwrap_or(1);
                        let size = self.base_font_size + ((6 - level.min(6)) as f64 * 3.0);
                        let mut txt = self
                            .pdf
                            .text(&current_text)
                            .size(size)
                            .margin_bottom(self.base_font_size * 0.8);
                        if let Some(f) = &self.font_name {
                            txt = txt.font(f);
                        }
                        let _ = txt;
                        current_heading_level = None;
                    }
                    TagEnd::Item => {
                        let bullet = format!("- {}", current_text.trim());
                        let margin = self.base_font_size * 1.5;

                        // Setup inner properties we can use in the closure
                        let fs = self.base_font_size;
                        let fnm = self.font_name.clone();

                        self.pdf.row(move |row| {
                            let fnm1 = fnm.clone();
                            row.col(margin, move |p| {
                                let mut txt = p.text("").size(fs);
                                if let Some(f) = &fnm1 {
                                    txt = txt.font(f);
                                }
                                let _ = txt;
                                Ok(())
                            });

                            let fnm2 = fnm.clone();
                            row.col(crate::pct(95.0), move |p| {
                                let mut txt = p.text(&bullet).size(fs).margin_bottom(fs * 0.5);
                                if let Some(f) = &fnm2 {
                                    txt = txt.font(f);
                                }
                                let _ = txt;
                                Ok(())
                            });
                        })?;
                    }
                    TagEnd::List(_) => {
                        in_list = false;
                    }
                    TagEnd::CodeBlock => {
                        let mut txt = self
                            .pdf
                            .text(&current_text)
                            .size(self.base_font_size * 0.9)
                            .color(Color::Rgb(100, 100, 100))
                            .margin_top(self.base_font_size * 0.5)
                            .margin_bottom(self.base_font_size);
                        if let Some(f) = &self.font_name {
                            txt = txt.font(f);
                        }
                        let _ = txt;
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        Ok(())
    }
}
