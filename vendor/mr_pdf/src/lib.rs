#[cfg(feature = "chart")]
pub mod chart;
pub mod draw;
pub mod font;
pub mod image;
pub mod layout;
pub mod pdf;
pub mod svg;

use std::io::Write;

#[cfg(feature = "chart")]
pub use chart::{Chart, ChartType};
pub use font::FontId;
use font::FontManager;
use image::Image;
pub use layout::box_layout::BoxBuilder;
use layout::cursor::Cursor;
pub use layout::flex::RowBuilder;
pub use layout::markdown::MarkdownRenderer;
pub use layout::multiplexed::{MultiplexedTable, TitleStyle};
use layout::paragraph::Paragraph;
pub use layout::table::RowBuilder as TableRowBuilder;
pub use layout::table::TableBorderStyle;
pub use layout::table::TableBuilder;
pub use layout::table::TableCell;
pub use layout::text::TextBlock;
pub use pdf::crypto::PdfPermissions;
use pdf::writer::PdfWriter;

/// Represents the dimension or weight of a layout element.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Size {
    /// Absolute points (1/72 inch).
    Points(f64),
    /// Percentage of the available parent width.
    Percent(f64),
    /// Relative weight in a flex layout.
    Flex(u32),
}

/// Helper to create a point size.
pub fn pt(p: f64) -> Size {
    Size::Points(p)
}

/// Helper to create a percentage size.
pub fn pct(p: f64) -> Size {
    Size::Percent(p)
}

/// Helper to create a flex size.
pub fn flex(f: u32) -> Size {
    Size::Flex(f)
}

impl From<f64> for Size {
    fn from(f: f64) -> Self {
        Size::Points(f)
    }
}

impl From<u32> for Size {
    fn from(u: u32) -> Self {
        Size::Flex(u)
    }
}

/// Extension trait for numbers to provide fluent `Size` creation.
pub trait SizeExt {
    fn pt(self) -> Size;
    fn pct(self) -> Size;
}

impl SizeExt for f64 {
    fn pt(self) -> Size {
        Size::Points(self)
    }
    fn pct(self) -> Size {
        Size::Percent(self)
    }
}

pub trait ToFlex {
    fn flex(self) -> Size;
}

impl ToFlex for u32 {
    fn flex(self) -> Size {
        Size::Flex(self)
    }
}

/// Preset paper sizes for PDF generation.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PaperSize {
    A3,
    A4,
    A5,
    /// Custom dimensions in points (width, height).
    Custom(f64, f64),
}

impl PaperSize {
    /// Returns the dimensions (width, height) in points.
    pub fn dimensions(&self) -> (f64, f64) {
        match self {
            PaperSize::A3 => (841.89, 1190.55),
            PaperSize::A4 => (595.28, 841.89),
            PaperSize::A5 => (419.53, 595.28),
            PaperSize::Custom(w, h) => (*w, *h),
        }
    }
}

/// Page orientation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// PDF document metadata.
pub struct Metadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
}

/// The main entry point for PDF generation.
pub struct Pdf<W: Write> {
    writer: PdfWriter<W>,
    pub font_manager: FontManager,
    cursor: Cursor,
    current_page_id: Option<u32>,
    current_content_id: Option<u32>,
    current_stream: String,
    margin: f64,
    pub paper_size: PaperSize,
    pub orientation: Orientation,
    pub page_width: f64,
    pub page_height: f64,
    pub current_font: Option<FontId>,
    pub metadata: Metadata,
    pub current_layout_width: Option<f64>,
    pub current_layout_margin: Option<f64>,
    pub show_page_numbers: bool,
    pub page_number_position: PageNumberPosition,
    pub page_count: u32,
    pub watermark: Option<Watermark>,
    pub line_spacing: f64,
    /// Padding inside table cells (in points). Default 5.0.
    pub cell_padding: f64,
    /// Optional callback invoked at the start of every new page. Use to draw a recurring page header.
    /// Taken out during invocation to avoid borrow conflicts; put back after.
    pub on_new_page: Option<Box<dyn FnMut(&mut Pdf<W>) -> std::io::Result<()> + Send>>,
}

/// Configuration for page watermarks.
#[derive(Clone, Debug)]
pub struct Watermark {
    pub text: String,
    pub font_size: f64,
    pub color: Color,
    pub opacity: f64, // 0.0 to 1.0 (currently simulated via color if GS is limited)
    pub angle: f64,   // Degrees (counter-clockwise)
}

/// Horizontal text alignment.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Vertical alignment within a container.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VAlign {
    Top,
    Center,
    Bottom,
}

/// A color representation in the RGB space.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    Rgb(u8, u8, u8),
}

/// Position for automatic page numbering.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageNumberPosition {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Pdf<Vec<u8>> {
    /// Generate PDF entirely in memory.
    pub fn memory() -> std::io::Result<Self> {
        Self::new(Vec::new())
    }

    /// Convenience method to generate PDF in memory using a closure and return the bytes.
    pub fn render<F>(f: F) -> std::io::Result<Vec<u8>>
    where
        F: FnOnce(&mut Pdf<Vec<u8>>) -> std::io::Result<()>,
    {
        let mut pdf = Self::memory()?;
        f(&mut pdf)?;
        pdf.finish()
    }
}

impl<W: Write> Pdf<W> {
    /// Write the PDF to any output stream implementing Write.
    pub fn stream(output: W) -> std::io::Result<Self> {
        Self::new(output)
    }

    /// Initializes a new PDF session.
    pub fn new(output: W) -> std::io::Result<Self> {
        let writer = PdfWriter::new(output)?;
        let paper_size = PaperSize::A4;
        let orientation = Orientation::Portrait;
        let (w, h) = paper_size.dimensions();

        Ok(Self {
            writer,
            font_manager: FontManager::new(),
            cursor: Cursor::new(50.0, h - 50.0),
            current_page_id: None,
            current_content_id: None,
            current_stream: String::new(),
            margin: 50.0,
            paper_size,
            orientation,
            page_width: w,
            page_height: h,
            current_font: None,
            metadata: Metadata {
                title: None,
                author: None,
                subject: None,
            },
            current_layout_width: None,
            current_layout_margin: None,
            show_page_numbers: false,
            page_number_position: PageNumberPosition::BottomCenter,
            page_count: 0,
            watermark: None,
            line_spacing: 1.3,
            cell_padding: 5.0,
            on_new_page: None,
        })
    }

    /// Set PDF encryption (password protection and permissions).
    pub fn set_encryption(
        &mut self,
        owner_pwd: &str,
        user_pwd: Option<&str>,
        perms: PdfPermissions,
    ) {
        self.writer
            .enable_encryption(owner_pwd, user_pwd.unwrap_or(""), perms);
    }

    /// Sets the paper size for new pages.
    pub fn set_paper_size(&mut self, size: PaperSize) {
        self.paper_size = size;
        self.resolve_page_dimensions();
    }

    /// Sets the orientation (Portrait/Landscape) for new pages.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.orientation = orientation;
        self.resolve_page_dimensions();
    }

    fn resolve_page_dimensions(&mut self) {
        let (mut w, mut h) = self.paper_size.dimensions();
        if self.orientation == Orientation::Landscape {
            std::mem::swap(&mut w, &mut h);
        }
        self.page_width = w;
        self.page_height = h;

        if self.current_stream.is_empty() {
            self.cursor.x = self.margin;
            self.cursor.y = h - self.margin;
        }
    }

    /// Sets the document title.
    pub fn set_title(&mut self, t: &str) {
        self.metadata.title = Some(t.to_string());
    }

    /// Sets the document author.
    pub fn set_author(&mut self, a: &str) {
        self.metadata.author = Some(a.to_string());
    }

    /// Sets the document subject.
    pub fn set_subject(&mut self, s: &str) {
        self.metadata.subject = Some(s.to_string());
    }

    /// Sets the global page margin.
    pub fn set_margin(&mut self, m: f64) {
        self.margin = m;
        self.cursor.x = m;
    }

    /// Returns the current cursor (x, y) position.
    pub fn cursor_pos(&self) -> (f64, f64) {
        (self.cursor.x, self.cursor.y)
    }

    /// Manually sets the cursor position.
    pub fn set_cursor(&mut self, x: f64, y: f64) {
        self.cursor.x = x;
        self.cursor.y = y;
    }

    /// Adjusts the cursor vertically.
    pub fn advance_cursor(&mut self, dy: f64) {
        self.cursor.y -= dy;
    }

    /// Registers a TTF font for use in the document.
    pub fn register_font(&mut self, name: &str, path: &str) -> std::io::Result<FontId> {
        let id = self.font_manager.register_font(name, path)?;
        if self.current_font.is_none() {
            self.current_font = Some(id);
        }
        Ok(id)
    }

    /// Sets a text watermark for all pages.
    pub fn set_watermark(&mut self, text: &str, size: f64, color: Color, opacity: f64, angle: f64) {
        self.watermark = Some(Watermark {
            text: text.to_string(),
            font_size: size,
            color,
            opacity,
            angle,
        });
    }

    pub fn ensure_page(&mut self) -> std::io::Result<()> {
        if self.current_page_id.is_none() {
            self.new_page()?;
        }
        Ok(())
    }

    /// Explicitly starts a new page.
    pub fn new_page(&mut self) -> std::io::Result<()> {
        self.flush_page()?;
        self.page_count += 1;
        self.cursor.x = self.margin;
        self.cursor.y = self.page_height - self.margin;
        let (p_id, c_id) = self.writer.add_page(self.page_width, self.page_height)?;
        self.current_page_id = Some(p_id);
        self.current_content_id = Some(c_id);
        self.current_stream.clear();

        // Invoke page header callback if set
        if let Some(mut cb) = self.on_new_page.take() {
            let result = cb(self);
            self.on_new_page = Some(cb);
            result?;
        }
        Ok(())
    }

    fn flush_page(&mut self) -> std::io::Result<()> {
        if let Some(c_id) = self.current_content_id {
            if self.show_page_numbers {
                let page_num = self.page_count;
                let text = format!("Page {}", page_num);
                let margin = 30.0;

                let (x, y) = match self.page_number_position {
                    PageNumberPosition::TopLeft => (margin, self.page_height - margin),
                    PageNumberPosition::TopCenter => {
                        (self.page_width / 2.0 - 20.0, self.page_height - margin)
                    }
                    PageNumberPosition::TopRight => {
                        (self.page_width - margin - 40.0, self.page_height - margin)
                    }
                    PageNumberPosition::BottomLeft => (margin, margin),
                    PageNumberPosition::BottomCenter => (self.page_width / 2.0 - 20.0, margin),
                    PageNumberPosition::BottomRight => (self.page_width - margin - 40.0, margin),
                };

                // Temporarily add page number to stream
                self.current_stream.push_str("q\n");
                self.current_stream.push_str("0 0 0 rg\n"); // Black
                self.current_stream.push_str(&format!(
                    "BT /FBuiltin 10 Tf {:.2} {:.2} Td ({}) Tj ET\n",
                    x, y, text
                ));
                self.current_stream.push_str("Q\n");
            }

            if let Some(wm) = &self.watermark {
                self.current_stream.push_str("q\n");
                let rgb = match wm.color {
                    Color::Rgb(r, g, b) => (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0),
                };

                // Set color with some simulated opacity if needed
                self.current_stream
                    .push_str(&format!("{:.2} {:.2} {:.2} rg\n", rgb.0, rgb.1, rgb.2));

                let rad = wm.angle.to_radians();
                let cos = rad.cos();
                let sin = rad.sin();

                // Center of the page
                let cx = self.page_width / 2.0;
                let cy = self.page_height / 2.0;

                // Rotation and translation matrix
                self.current_stream.push_str(&format!(
                    "{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} cm\n",
                    cos, sin, -sin, cos, cx, cy
                ));

                // Draw text (centered relative to rotation point)
                // Roughly center the text string length
                let est_w = wm.text.len() as f64 * wm.font_size * 0.3;
                self.current_stream.push_str(&format!(
                    "BT /FBuiltin {:.1} Tf {:.2} {:.2} Td ({}) Tj ET\n",
                    wm.font_size,
                    -est_w,
                    -wm.font_size / 2.0,
                    wm.text
                ));

                self.current_stream.push_str("Q\n");
            }

            self.writer
                .write_content_stream(c_id, &self.current_stream)?;
            self.current_stream.clear();
            self.current_content_id = None;
            self.current_page_id = None;
        }
        Ok(())
    }

    /// Checks if a page break is needed for the requested height.
    pub fn check_page_break(&mut self, required_height: f64) -> std::io::Result<()> {
        self.ensure_page()?;
        if self.cursor.y - required_height < self.margin {
            self.new_page()?;
        }
        Ok(())
    }

    /// Adds a boxed container with background and border.
    pub fn box_layout<F>(&mut self, f: F) -> std::io::Result<()>
    where
        F: FnOnce(&mut BoxBuilder<W>) -> std::io::Result<()>,
    {
        self.ensure_page()?;
        let mut builder = BoxBuilder::new(self);
        f(&mut builder)
    }

    /// Starts a multiplexed table streaming session that buffers rows to disk.
    /// This allows interleaving rows for different tables while keeping RAM usage constant.
    pub fn multi_table_streaming(
        &mut self,
        builder: TableBuilder,
    ) -> std::io::Result<MultiplexedTable> {
        MultiplexedTable::new(builder)
    }

    /// Creates a new text block.
    pub fn text<'a>(&'a mut self, text: &'a str) -> TextBlock<'a, W> {
        TextBlock::new(self, text)
    }

    /// Creates a word-wrapped paragraph.
    pub fn paragraph<'a>(&'a mut self, text: &'a str) -> Paragraph<'a, W> {
        Paragraph::new(self, text)
    }

    /// Creates a rich text block where individual spans can be styled.
    pub fn rich_text<'a, F>(&'a mut self, build_fn: F) -> crate::layout::text::RichTextBlock<'a, W>
    where
        F: FnOnce(&mut crate::layout::text::RichTextBuilder),
    {
        crate::layout::text::RichTextBlock::new(self, build_fn)
    }

    /// Renders a markdown block.
    pub fn markdown<'a>(&'a mut self, markdown: &'a str) -> MarkdownRenderer<'a, W> {
        MarkdownRenderer::new(self, markdown)
    }

    /// Temporarily changes the line height multiplier for all content generated within the closure.
    pub fn line_height<F>(&mut self, multiplier: f64, build_fn: F) -> &mut Self
    where
        F: FnOnce(&mut Self),
    {
        let old_spacing = self.line_spacing;
        self.line_spacing = multiplier;
        build_fn(self);
        self.line_spacing = old_spacing;
        self
    }

    /// Adds a table using the builder pattern.
    pub fn table<F>(&mut self, f: F) -> std::io::Result<()>
    where
        F: FnOnce(&mut TableBuilder),
    {
        self.ensure_page()?;
        let mut builder = TableBuilder::new();
        f(&mut builder);
        let table = builder.build();
        table.render(self)
    }

    /// Adds a flexible row.
    pub fn row<F>(&mut self, f: F) -> std::io::Result<()>
    where
        F: FnOnce(&mut RowBuilder<W>),
    {
        self.ensure_page()?;
        layout::flex::render_row(self, f)
    }

    /// Adds a content column.
    pub fn column<F>(&mut self, f: F) -> std::io::Result<()>
    where
        F: FnOnce(&mut Pdf<W>),
    {
        self.ensure_page()?;
        f(self);
        Ok(())
    }

    /// Embeds an image from a file path. Returns a builder to set size and position.
    pub fn image<'a>(&'a mut self, path: &str) -> ImageBuilder<'a, W> {
        ImageBuilder {
            pdf: self,
            path: path.to_string(),
            x: None,
            y: None,
            w: None,
            h: None,
        }
    }

    /// Embeds an image from a Base64 string. Returns a builder.
    pub fn image_base64<'a>(&'a mut self, b64: &str) -> ImageBase64Builder<'a, W> {
        ImageBase64Builder {
            pdf: self,
            b64: b64.to_string(),
            x: None,
            y: None,
            w: None,
            h: None,
        }
    }

    fn embed_image(&mut self, img: Image, x: f64, y: f64, w: f64, h: f64) -> std::io::Result<()> {
        let xobj_id = img.embed(&mut self.writer)?;
        self.writer.register_xobject(xobj_id);
        self.current_stream.push_str(&format!(
            "q\n{:.2} 0 0 {:.2} {:.2} {:.2} cm\n/Im{} Do\nQ\n",
            w, h, x, y, xobj_id
        ));
        Ok(())
    }

    /// Renders an SVG file. Returns a builder to set size.
    pub fn svg<'a>(&'a mut self, path: &str) -> SvgBuilder<'a, W> {
        SvgBuilder {
            pdf: self,
            path: path.to_string(),
            w: None,
            h: None,
        }
    }

    /// Renders a QR code at the specified position.
    #[cfg(feature = "qrcode")]
    pub fn render_qr(&mut self, data: &str, x: f64, y: f64, size: f64) -> std::io::Result<()> {
        use fast_qr::qr::QRBuilder;
        let qrcode = QRBuilder::new(data.to_string()).build().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("QR Error: {:?}", e))
        })?;

        let width = qrcode.size;
        let module_size = size / width as f64;

        // Save state and set fill color to black
        self.current_stream.push_str("q\n0 0 0 rg\n");

        for r in 0..width {
            for c in 0..width {
                // We use .value() to check if it's a black module
                if qrcode.data[r * width + c].value() {
                    let rx = x + (c as f64 * module_size);
                    let ry = y + size - ((r + 1) as f64 * module_size);
                    self.current_stream.push_str(&format!(
                        "{:.2} {:.2} {:.2} {:.2} re f\n",
                        rx,
                        ry,
                        module_size + 0.05,
                        module_size + 0.05
                    ));
                }
            }
        }

        self.current_stream.push_str("Q\n");
        Ok(())
    }

    /// Renders a chart. (Requires 'chart' feature)
    #[cfg(feature = "chart")]
    pub fn chart(&mut self, chart: Chart) -> std::io::Result<()> {
        self.ensure_page()?;
        let content_w = self.content_width();

        let chart_w = self.resolve_size(chart.width, content_w);
        let chart_h = self.resolve_size(chart.height, 300.0);

        let (start_x, _start_y) = self.cursor_pos();
        let x_offset = match chart.align {
            Align::Left => 0.0,
            Align::Center => (content_w - chart_w).max(0.0) / 2.0,
            Align::Right => (content_w - chart_w).max(0.0),
        };

        let final_start_x = start_x + x_offset;

        if chart.chart_type == ChartType::Pie {
            self.render_pie_chart(&chart, final_start_x, chart_w, chart_h)?;
        } else {
            self.render_axis_chart(&chart, final_start_x, chart_w, chart_h)?;
        }

        Ok(())
    }

    #[cfg(feature = "chart")]
    fn render_axis_chart(
        &mut self,
        chart: &Chart,
        start_x: f64,
        w: f64,
        h: f64,
    ) -> std::io::Result<()> {
        let left_padding = 30.0;
        let bottom_padding = 20.0;
        let chart_x = start_x + left_padding;

        self.advance_cursor(h + bottom_padding + 20.0);
        let base_y = self.cursor_pos().1 + bottom_padding;

        let max_val = chart
            .data
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max)
            .max(0.0001);
        let usable_w = w - left_padding;
        let w_step = usable_w / (chart.data.len() as f64).max(1.0);

        self.set_stroke_color(Color::Rgb(200, 200, 200))?;
        for i in 0..=5 {
            let ratio = i as f64 / 5.0;
            let gy = base_y + (ratio * h);
            self.line(chart_x, gy, chart_x + usable_w, gy)?;
            let label = format!("{:.0}", ratio * max_val);
            let (old_x, old_y) = self.cursor_pos();
            self.set_cursor(start_x, gy + 5.0);
            self.text(&label).size(8.0).wrap(false);
            self.set_cursor(old_x, old_y);
        }

        self.set_stroke_color(Color::Rgb(0, 0, 0))?;
        self.line(chart_x, base_y, chart_x + usable_w, base_y)?;
        self.line(chart_x, base_y, chart_x, base_y + h)?;

        if chart.chart_type == ChartType::Bar {
            for (i, &val) in chart.data.iter().enumerate() {
                let cx = chart_x + (i as f64 * w_step) + (w_step / 2.0);
                let bw = w_step * 0.7;
                let bh = (val / max_val) * h;
                let bx = cx - (bw / 2.0);

                if let Some(c) = &chart.color {
                    self.set_fill_color(*c)?;
                    self.fill_rect(bx, base_y, bw, bh)?;
                } else {
                    self.rect(bx, base_y, bw, bh)?;
                }

                if chart.show_values {
                    let val_str = format!("{:.0}", val);
                    let (ox, oy) = self.cursor_pos();
                    self.set_cursor(bx + bw / 2.0 - 5.0, base_y + bh + 18.0);
                    self.set_fill_color(Color::Rgb(0, 0, 0))?;
                    self.text(&val_str).size(8.0).wrap(false);
                    self.set_cursor(ox, oy);
                }

                self.draw_chart_label(chart, i, bx, base_y - 15.0)?;
            }
        } else if chart.chart_type == ChartType::Line {
            let mut points = Vec::new();
            for (i, &val) in chart.data.iter().enumerate() {
                let cx = chart_x + (i as f64 * w_step) + (w_step / 2.0);
                let cy = base_y + (val / max_val) * h;
                points.push((cx, cy));

                if chart.show_values {
                    let val_str = format!("{:.0}", val);
                    let (ox, oy) = self.cursor_pos();
                    self.set_cursor(cx - 5.0, cy + 18.0);
                    self.set_fill_color(Color::Rgb(0, 0, 0))?;
                    self.text(&val_str).size(8.0).wrap(false);
                    self.set_cursor(ox, oy);
                }

                self.draw_chart_label(chart, i, cx - 10.0, base_y - 15.0)?;
            }
            if let Some(c) = &chart.color {
                self.set_stroke_color(*c)?;
            }
            for i in 0..points.len().saturating_sub(1) {
                self.line(points[i].0, points[i].1, points[i + 1].0, points[i + 1].1)?;
            }
            for p in points {
                self.fill_circle(p.0, p.1, 3.0)?;
            }
        }
        Ok(())
    }

    #[cfg(feature = "chart")]
    fn render_pie_chart(
        &mut self,
        chart: &Chart,
        start_x: f64,
        w: f64,
        h: f64,
    ) -> std::io::Result<()> {
        let size = w.min(h);
        let center_x = start_x + size / 2.0;
        self.advance_cursor(size + 20.0);
        let center_y = self.cursor_pos().1 + size / 2.0;
        let radius = size / 2.0 * 0.8;

        let total: f64 = chart.data.iter().sum::<f64>().max(0.0001);
        let mut current_angle = 90.0;

        for (i, &val) in chart.data.iter().enumerate() {
            let sweep_angle = (val / total) * 360.0;
            let color = chart.color.unwrap_or_else(|| {
                let r = (100 + (i * 50)) % 255;
                let g = (150 + (i * 30)) % 255;
                let b = (200 - (i * 40)) % 255;
                Color::Rgb(r as u8, g as u8, b as u8)
            });
            self.set_fill_color(color)?;

            self.draw_pie_slice(center_x, center_y, radius, current_angle, sweep_angle)?;

            if chart.show_values {
                let mid_angle = (current_angle - sweep_angle / 2.0).to_radians();
                let lx = center_x + (radius * 0.5) * mid_angle.cos();
                let ly = center_y + (radius * 0.5) * mid_angle.sin();

                let (ox, oy) = self.cursor_pos();
                self.set_cursor(lx - 5.0, ly + 2.0);
                self.set_fill_color(Color::Rgb(255, 255, 255))?;
                let val_str = format!("{:.0}", val);
                self.text(&val_str).size(8.0).wrap(false);
                self.set_cursor(ox, oy);
            }

            let (old_x, old_y) = self.cursor_pos();
            self.set_cursor(start_x + size + 10.0, center_y + radius - (i as f64 * 20.0));
            if let Some(labels) = &chart.labels
                && let Some(l) = labels.get(i)
            {
                self.set_fill_color(color)?;
                self.fill_rect(self.cursor_pos().0, self.cursor_pos().1, 10.0, 10.0)?;
                self.set_cursor(self.cursor_pos().0 + 15.0, self.cursor_pos().1 + 12.0);

                let mut display_text = l.clone();
                if chart.show_values {
                    let pct = (val / total) * 100.0;
                    display_text = format!("{}: {:.0} ({:.1}%)", l, val, pct);
                }

                self.set_fill_color(Color::Rgb(0, 0, 0))?;
                self.text(&display_text).size(8.0).wrap(false);
            }
            self.set_cursor(old_x, old_y);

            current_angle -= sweep_angle;
        }
        Ok(())
    }

    #[cfg(feature = "chart")]
    fn draw_pie_slice(
        &mut self,
        cx: f64,
        cy: f64,
        r: f64,
        start_deg: f64,
        sweep_deg: f64,
    ) -> std::io::Result<()> {
        self.ensure_page()?;

        let start_rad = start_deg.to_radians();
        let sweep_rad = sweep_deg.to_radians();

        let p1x = cx + r * start_rad.cos();
        let p1y = cy + r * start_rad.sin();

        let stream = self.get_stream();
        stream.push_str(&format!("{:.2} {:.2} m\n", cx, cy));
        stream.push_str(&format!("{:.2} {:.2} l\n", p1x, p1y));

        let segments = (sweep_deg.abs() / 90.0).ceil() as usize;
        let seg_sweep_rad = sweep_rad / segments as f64;

        for s in 0..segments {
            let a1 = start_rad - s as f64 * seg_sweep_rad;
            let a2 = a1 - seg_sweep_rad;

            let k = r * (4.0 / 3.0) * (seg_sweep_rad / 4.0).tan();

            let cp1x = cx + r * a1.cos() + k * a1.sin();
            let cp1y = cy + r * a1.sin() - k * a1.cos();

            let cp2x = cx + r * a2.cos() - k * a2.sin();
            let cp2y = cy + r * a2.sin() + k * a2.cos();

            let p2x = cx + r * a2.cos();
            let p2y = cy + r * a2.sin();

            stream.push_str(&format!(
                "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
                cp1x, cp1y, cp2x, cp2y, p2x, p2y
            ));
        }

        stream.push_str("h f\n");
        Ok(())
    }

    #[cfg(feature = "chart")]
    fn draw_chart_label(&mut self, chart: &Chart, i: usize, x: f64, y: f64) -> std::io::Result<()> {
        if let Some(labels) = &chart.labels
            && let Some(label) = labels.get(i)
        {
            let (old_x, old_y) = self.cursor_pos();
            self.set_cursor(x, y);
            self.text(label).size(8.0);
            self.set_cursor(old_x, old_y);
        }
        Ok(())
    }

    fn resolve_size(&self, size: Size, reference: f64) -> f64 {
        match size {
            Size::Points(p) => p,
            Size::Percent(pct) => (pct / 100.0) * reference,
            Size::Flex(_) => reference,
        }
    }

    /// Draws a line between two points.
    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) -> std::io::Result<()> {
        self.ensure_page()?;
        self.current_stream
            .push_str(&format!("{:.2} {:.2} m {:.2} {:.2} l S\n", x1, y1, x2, y2));
        Ok(())
    }

    /// Draws an un-filled rectangle.
    pub fn rect(&mut self, x: f64, y: f64, width: f64, height: f64) -> std::io::Result<()> {
        self.ensure_page()?;
        self.current_stream.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} re S\n",
            x, y, width, height
        ));
        Ok(())
    }

    /// Draws a filled rectangle.
    pub fn fill_rect(&mut self, x: f64, y: f64, width: f64, height: f64) -> std::io::Result<()> {
        self.ensure_page()?;
        self.current_stream.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} re f\n",
            x, y, width, height
        ));
        Ok(())
    }

    /// Draws a rectangle filled with a linear gradient.
    pub fn fill_gradient_rect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        c1: Color,
        c2: Color,
        horizontal: bool,
    ) -> std::io::Result<()> {
        let rgb1 = match c1 {
            Color::Rgb(r, g, b) => [r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0],
        };
        let rgb2 = match c2 {
            Color::Rgb(r, g, b) => [r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0],
        };
        let coords = if horizontal {
            [x, y, x + w, y]
        } else {
            [x, y + h, x, y] // Top to Bottom
        };

        let sh_id = self.writer.register_shading(rgb1, rgb2, coords);

        self.ensure_page()?;
        self.current_stream.push_str("q\n");
        // Clipping path to the rectangle
        self.current_stream
            .push_str(&format!("{:.2} {:.2} {:.2} {:.2} re W n\n", x, y, w, h));
        // Paint the shading
        self.current_stream.push_str(&format!("/Sh{} sh\n", sh_id));
        self.current_stream.push_str("Q\n");
        Ok(())
    }

    /// Draws a circle.
    pub fn circle(&mut self, x: f64, y: f64, radius: f64) -> std::io::Result<()> {
        self.draw_circle_op(x, y, radius, "S")
    }

    /// Draws a filled circle.
    pub fn fill_circle(&mut self, x: f64, y: f64, radius: f64) -> std::io::Result<()> {
        self.draw_circle_op(x, y, radius, "f")
    }

    /// Draws a rectangle with a shadow.
    pub fn shadow_rect(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        blur: f64,
    ) -> std::io::Result<()> {
        self.ensure_page()?;
        self.current_stream.push_str("q\n");
        self.set_fill_color(Color::Rgb(200, 200, 200))?;
        self.fill_rect(x + blur, y - blur, width, height)?;
        self.current_stream.push_str("Q\n");
        Ok(())
    }

    /// Draws a rounded rectangle.
    pub fn rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, r: f64) -> std::io::Result<()> {
        self.draw_rounded_rect_op(x, y, w, h, r, "S")
    }

    /// Draws a filled rounded rectangle.
    pub fn fill_rounded_rect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        r: f64,
    ) -> std::io::Result<()> {
        self.draw_rounded_rect_op(x, y, w, h, r, "f")
    }

    pub fn draw_rounded_rect_op(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        r: f64,
        op: &str,
    ) -> std::io::Result<()> {
        self.ensure_page()?;
        let k = 0.552284749831 * r;
        let stream = &mut self.current_stream;

        // Start from top-left after the corner
        stream.push_str(&format!("{:.2} {:.2} m\n", x + r, y + h));
        // Top edge
        stream.push_str(&format!("{:.2} {:.2} l\n", x + w - r, y + h));
        // Top-right corner
        stream.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
            x + w - r + k,
            y + h,
            x + w,
            y + h - r + k,
            x + w,
            y + h - r
        ));
        // Right edge
        stream.push_str(&format!("{:.2} {:.2} l\n", x + w, y + r));
        // Bottom-right corner
        stream.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
            x + w,
            y + r - k,
            x + w - r + k,
            y,
            x + w - r,
            y
        ));
        // Bottom edge
        stream.push_str(&format!("{:.2} {:.2} l\n", x + r, y));
        // Bottom-left corner
        stream.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
            x + r - k,
            y,
            x,
            y + r - k,
            x,
            y + r
        ));
        // Left edge
        stream.push_str(&format!("{:.2} {:.2} l\n", x, y + h - r));
        // Top-left corner
        stream.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {}\n",
            x,
            y + h - r + k,
            x + r - k,
            y + h,
            x + r,
            y + h,
            op
        ));

        Ok(())
    }

    fn draw_circle_op(&mut self, x: f64, y: f64, radius: f64, op: &str) -> std::io::Result<()> {
        self.ensure_page()?;
        let k = 0.552284749831 * radius;
        let p = &mut self.current_stream;
        p.push_str(&format!("{:.2} {:.2} m\n", x, y + radius));
        p.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
            x + k,
            y + radius,
            x + radius,
            y + k,
            x + radius,
            y
        ));
        p.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
            x + radius,
            y - k,
            x + k,
            y - radius,
            x,
            y - radius
        ));
        p.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
            x - k,
            y - radius,
            x - radius,
            y - k,
            x - radius,
            y
        ));
        p.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c {}\n",
            x - radius,
            y + k,
            x - k,
            y + radius,
            x,
            y + radius,
            op
        ));
        Ok(())
    }

    /// Sets the stroke color.
    pub fn set_stroke_color(&mut self, color: Color) -> std::io::Result<()> {
        self.ensure_page()?;
        match color {
            Color::Rgb(r, g, b) => self.current_stream.push_str(&format!(
                "{:.3} {:.3} {:.3} RG\n",
                r as f64 / 255.0,
                g as f64 / 255.0,
                b as f64 / 255.0
            )),
        }
        Ok(())
    }

    /// Sets the fill color.
    pub fn set_fill_color(&mut self, color: Color) -> std::io::Result<()> {
        self.ensure_page()?;
        match color {
            Color::Rgb(r, g, b) => self.current_stream.push_str(&format!(
                "{:.3} {:.3} {:.3} rg\n",
                r as f64 / 255.0,
                g as f64 / 255.0,
                b as f64 / 255.0
            )),
        }
        Ok(())
    }

    /// Sets the line width for strokes.
    pub fn set_line_width(&mut self, width: f64) -> std::io::Result<()> {
        self.ensure_page()?;
        self.current_stream.push_str(&format!("{:.2} w\n", width));
        Ok(())
    }

    /// Returns the current page content stream.
    pub fn get_stream(&mut self) -> &mut String {
        &mut self.current_stream
    }

    pub fn ensure_page_pub(&mut self) -> std::io::Result<()> {
        self.ensure_page()
    }

    pub fn margin_pub(&self) -> f64 {
        self.current_layout_margin.unwrap_or(self.margin)
    }

    /// Adds a clickable hyperlink.
    pub fn add_link(&mut self, rect: (f64, f64, f64, f64), url: &str) {
        self.writer.add_link(rect, url);
    }

    /// Returns the active content width.
    pub fn content_width(&self) -> f64 {
        self.current_layout_width
            .unwrap_or(self.page_width - 2.0 * self.margin)
    }

    /// Finalizes the PDF document.
    pub fn finish(mut self) -> std::io::Result<W> {
        self.flush_page()?;
        self.writer.finish(&mut self.font_manager, &self.metadata)
    }
}

pub struct SvgBuilder<'a, W: Write> {
    pdf: &'a mut Pdf<W>,
    path: String,
    w: Option<Size>,
    h: Option<Size>,
}

impl<'a, W: Write> SvgBuilder<'a, W> {
    pub fn width(mut self, w: impl Into<Size>) -> Self {
        self.w = Some(w.into());
        self
    }

    pub fn height(mut self, h: impl Into<Size>) -> Self {
        self.h = Some(h.into());
        self
    }

    pub fn size(mut self, w: impl Into<Size>, h: impl Into<Size>) -> Self {
        self.w = Some(w.into());
        self.h = Some(h.into());
        self
    }

    pub fn render(self) -> std::io::Result<()> {
        let content_w = self.pdf.content_width();
        let rw = self.w.map(|s| self.pdf.resolve_size(s, content_w));
        let rh = self.h.map(|s| self.pdf.resolve_size(s, 300.0));

        self.pdf.ensure_page()?;
        crate::svg::SvgRenderer::render(self.pdf, &self.path, rw, rh)
    }
}

pub struct ImageBuilder<'a, W: Write> {
    pdf: &'a mut Pdf<W>,
    path: String,
    x: Option<f64>,
    y: Option<f64>,
    w: Option<Size>,
    h: Option<Size>,
}

impl<'a, W: Write> ImageBuilder<'a, W> {
    pub fn position(mut self, x: f64, y: f64) -> Self {
        self.x = Some(x);
        self.y = Some(y);
        self
    }

    pub fn width(mut self, w: impl Into<Size>) -> Self {
        self.w = Some(w.into());
        self
    }

    pub fn height(mut self, h: impl Into<Size>) -> Self {
        self.h = Some(h.into());
        self
    }

    pub fn size(mut self, w: impl Into<Size>, h: impl Into<Size>) -> Self {
        self.w = Some(w.into());
        self.h = Some(h.into());
        self
    }

    pub fn render(self) -> std::io::Result<()> {
        self.pdf.ensure_page()?;
        let img = Image::load("img", &self.path)?;

        let (cx, cy) = self.pdf.cursor_pos();
        let fx = self.x.unwrap_or(cx);
        let fy = self.y.unwrap_or(cy);

        let content_w = self.pdf.content_width();
        let fw = self
            .pdf
            .resolve_size(self.w.unwrap_or(Size::Points(100.0)), content_w);
        let fh = self
            .pdf
            .resolve_size(self.h.unwrap_or(Size::Points(100.0)), 300.0);

        self.pdf.embed_image(img, fx, fy, fw, fh)
    }
}

pub struct ImageBase64Builder<'a, W: Write> {
    pdf: &'a mut Pdf<W>,
    b64: String,
    x: Option<f64>,
    y: Option<f64>,
    w: Option<Size>,
    h: Option<Size>,
}

impl<'a, W: Write> ImageBase64Builder<'a, W> {
    pub fn position(mut self, x: f64, y: f64) -> Self {
        self.x = Some(x);
        self.y = Some(y);
        self
    }

    pub fn width(mut self, w: impl Into<Size>) -> Self {
        self.w = Some(w.into());
        self
    }

    pub fn height(mut self, h: impl Into<Size>) -> Self {
        self.h = Some(h.into());
        self
    }

    pub fn size(mut self, w: impl Into<Size>, h: impl Into<Size>) -> Self {
        self.w = Some(w.into());
        self.h = Some(h.into());
        self
    }

    pub fn render(self) -> std::io::Result<()> {
        self.pdf.ensure_page()?;
        let img = Image::from_base64("img", &self.b64)?;

        let (cx, cy) = self.pdf.cursor_pos();
        let fx = self.x.unwrap_or(cx);
        let fy = self.y.unwrap_or(cy);

        let content_w = self.pdf.content_width();
        let fw = self
            .pdf
            .resolve_size(self.w.unwrap_or(Size::Points(100.0)), content_w);
        let fh = self
            .pdf
            .resolve_size(self.h.unwrap_or(Size::Points(100.0)), 300.0);

        self.pdf.embed_image(img, fx, fy, fw, fh)
    }
}
