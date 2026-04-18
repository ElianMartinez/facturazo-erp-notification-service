use crate::layout::text::escape_pdf_str;
use crate::{Align, Color, FontId, Pdf, Size, VAlign};
use std::io::Write;

/// Configuration for table visual appearance.
#[derive(Clone, Debug)]
pub struct TableStyle {
    pub bg_color: Option<Color>,
    pub text_color: Option<Color>,
    pub font_size: f64,
    pub font: Option<FontId>,
}

/// Border style options for tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TableBorderStyle {
    #[default]
    Full,
    None,
    Ghost,
    HeaderOnly,
}

impl TableStyle {
    pub fn new(font_size: f64) -> Self {
        Self {
            bg_color: None,
            text_color: None,
            font_size,
            font: None,
        }
    }

    /// Sets the background color for cells.
    pub fn bg_color(&mut self, color: Color) -> &mut Self {
        self.bg_color = Some(color);
        self
    }

    /// Sets the text color for cells.
    pub fn text_color(&mut self, color: Color) -> &mut Self {
        self.text_color = Some(color);
        self
    }

    /// Sets the font size for text in cells.
    pub fn font_size(&mut self, size: f64) -> &mut Self {
        self.font_size = size;
        self
    }

    /// Sets the font for cells.
    pub fn font(&mut self, font: FontId) -> &mut Self {
        self.font = Some(font);
        self
    }
}

/// Default alignment configurations for a column.
#[derive(Clone, Debug)]
pub struct ColumnConfig {
    pub align: Align,
    pub valign: VAlign,
}

impl Default for ColumnConfig {
    fn default() -> Self {
        Self {
            align: Align::Left,
            valign: VAlign::Top,
        }
    }
}

#[derive(Clone)]
pub enum Cell {
    Text(String),
    ImagePath(String),
    ImageBase64(String),
    #[cfg(feature = "qrcode")]
    QrCode(String),
}

/// Represents a single cell in a table row.
#[derive(Clone)]
pub struct TableCell {
    pub content: Cell,
    pub colspan: usize,
    pub rowspan: usize,
    pub align: Option<Align>,
    pub valign: Option<VAlign>,
    pub link: Option<String>,
    pub bg_color: Option<Color>,
    pub text_color: Option<Color>,
    pub font_size: Option<f64>,
    pub font: Option<FontId>,
    pub fill_char: Option<char>,
}

impl TableCell {
    /// Creates a text cell.
    pub fn text(s: &str) -> Self {
        Self {
            content: Cell::Text(s.to_string()),
            colspan: 1,
            rowspan: 1,
            align: None,
            valign: None,
            link: None,
            bg_color: None,
            text_color: None,
            font_size: None,
            font: None,
            fill_char: None,
        }
    }

    /// Creates an image cell from a file path.
    pub fn image(path: &str) -> Self {
        Self {
            content: Cell::ImagePath(path.to_string()),
            colspan: 1,
            rowspan: 1,
            align: None,
            valign: None,
            link: None,
            bg_color: None,
            text_color: None,
            font_size: None,
            font: None,
            fill_char: None,
        }
    }

    /// Creates an image cell from a Base64 string.
    pub fn image_base64(b64: &str) -> Self {
        Self {
            content: Cell::ImageBase64(b64.to_string()),
            colspan: 1,
            rowspan: 1,
            align: None,
            valign: None,
            link: None,
            bg_color: None,
            text_color: None,
            font_size: None,
            font: None,
            fill_char: None,
        }
    }

    /// Creates a QR code cell.
    #[cfg(feature = "qrcode")]
    pub fn qr(data: &str) -> Self {
        Self {
            content: Cell::QrCode(data.to_string()),
            colspan: 1,
            rowspan: 1,
            align: None,
            valign: None,
            link: None,
            bg_color: None,
            text_color: None,
            font_size: None,
            fill_char: None,
        }
    }

    /// Sets how many columns this cell should span.
    pub fn with_span(mut self, n: usize) -> Self {
        self.colspan = n.max(1);
        self
    }

    /// Sets how many rows this cell should span.
    pub fn with_rowspan(mut self, n: usize) -> Self {
        self.rowspan = n.max(1);
        self
    }

    /// Overrides the horizontal alignment for this cell.
    pub fn align(mut self, a: Align) -> Self {
        self.align = Some(a);
        self
    }

    /// Overrides the vertical alignment for this cell.
    pub fn valign(mut self, v: VAlign) -> Self {
        self.valign = Some(v);
        self
    }

    /// Adds a clickable hyperlink to the cell.
    pub fn link(mut self, url: &str) -> Self {
        self.link = Some(url.to_string());
        self
    }

    /// Sets an individual background color for this cell.
    pub fn bg_color(mut self, color: Color) -> Self {
        self.bg_color = Some(color);
        self
    }

    /// Sets an individual text color for this cell.
    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = Some(color);
        self
    }

    /// Sets an individual font size for this cell.
    pub fn font_size(mut self, size: f64) -> Self {
        self.font_size = Some(size);
        self
    }

    /// Sets an individual font for this cell.
    pub fn font(mut self, font: FontId) -> Self {
        self.font = Some(font);
        self
    }

    /// Pads the text content at the end to reaching the specified character length.
    pub fn pad_end(mut self, len: usize, filler: char) -> Self {
        if let Cell::Text(ref mut s) = self.content {
            let current_len = s.chars().count();
            if current_len < len {
                for _ in 0..(len - current_len) {
                    s.push(filler);
                }
            }
        }
        self
    }

    /// Pads the text content at the start to reaching the specified character length.
    pub fn pad_start(mut self, len: usize, filler: char) -> Self {
        if let Cell::Text(ref mut s) = self.content {
            let current_len = s.chars().count();
            if current_len < len {
                let mut prefix = String::new();
                for _ in 0..(len - current_len) {
                    prefix.push(filler);
                }
                s.insert_str(0, &prefix);
            }
        }
        self
    }

    /// Automatically expands and fills the remaining space in the cell with the filler character.
    pub fn fill(mut self, filler: char) -> Self {
        self.fill_char = Some(filler);
        self
    }
}

impl From<&str> for TableCell {
    fn from(s: &str) -> Self {
        Self::text(s)
    }
}

impl From<String> for TableCell {
    fn from(s: String) -> Self {
        Self {
            content: Cell::Text(s),
            colspan: 1,
            rowspan: 1,
            align: None,
            valign: None,
            link: None,
            bg_color: None,
            text_color: None,
            font_size: None,
            font: None,
            fill_char: None,
        }
    }
}

/// Fluent builder for creating table rows.
pub struct RowBuilder {
    pub cells: Vec<TableCell>,
}

impl Default for RowBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RowBuilder {
    pub fn new() -> Self {
        Self { cells: Vec::new() }
    }

    /// Adds a text cell.
    pub fn cell(&mut self, text: &str) -> &mut Self {
        self.cells.push(TableCell::text(text));
        self
    }

    /// Adds an image cell.
    pub fn cell_image(&mut self, path: &str) -> &mut Self {
        self.cells.push(TableCell::image(path));
        self
    }

    /// Adds an image cell from Base64.
    pub fn cell_image_base64(&mut self, b64: &str) -> &mut Self {
        self.cells.push(TableCell::image_base64(b64));
        self
    }

    /// Adds a QR code cell.
    #[cfg(feature = "qrcode")]
    pub fn cell_qr(&mut self, data: &str) -> &mut Self {
        self.cells.push(TableCell::qr(data));
        self
    }

    /// Sets the colspan for the most recently added cell.
    pub fn span(&mut self, n: usize) -> &mut Self {
        if let Some(last) = self.cells.last_mut() {
            last.colspan = n.max(1);
        }
        self
    }

    /// Sets the rowspan for the most recently added cell.
    pub fn rowspan(&mut self, n: usize) -> &mut Self {
        if let Some(last) = self.cells.last_mut() {
            last.rowspan = n.max(1);
        }
        self
    }

    /// Sets the horizontal alignment for the most recently added cell.
    pub fn align(&mut self, a: Align) -> &mut Self {
        if let Some(last) = self.cells.last_mut() {
            last.align = Some(a);
        }
        self
    }

    /// Sets the vertical alignment for the most recently added cell.
    pub fn valign(&mut self, v: VAlign) -> &mut Self {
        if let Some(last) = self.cells.last_mut() {
            last.valign = Some(v);
        }
        self
    }

    /// Adds a hyperlink to the most recently added cell.
    pub fn link(&mut self, url: &str) -> &mut Self {
        if let Some(last) = self.cells.last_mut() {
            last.link = Some(url.to_string());
        }
        self
    }

    /// Sets the background color for the most recently added cell.
    pub fn bg_color(&mut self, color: Color) -> &mut Self {
        if let Some(last) = self.cells.last_mut() {
            last.bg_color = Some(color);
        }
        self
    }

    /// Sets the text color for the most recently added cell.
    pub fn text_color(&mut self, color: Color) -> &mut Self {
        if let Some(last) = self.cells.last_mut() {
            last.text_color = Some(color);
        }
        self
    }

    /// Sets the font size for the most recently added cell.
    pub fn font_size(&mut self, size: f64) -> &mut Self {
        if let Some(last) = self.cells.last_mut() {
            last.font_size = Some(size);
        }
        self
    }

    /// Sets the font for the most recently added cell.
    pub fn font(&mut self, font: FontId) -> &mut Self {
        if let Some(last) = self.cells.last_mut() {
            last.font = Some(font);
        }
        self
    }
}

/// Builder for complex PDF tables.
#[derive(Clone)]
pub struct TableBuilder {
    widths: Vec<Size>,
    header: Vec<Vec<TableCell>>,
    rows: Vec<Vec<TableCell>>,
    repeat_header: bool,
    column_configs: Vec<ColumnConfig>,
    header_style: TableStyle,
    row_style: TableStyle,
    border_style: TableBorderStyle,
    zebra_color: Option<Color>,
}

impl Default for TableBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TableBuilder {
    pub fn new() -> Self {
        Self {
            widths: Vec::new(),
            header: Vec::new(),
            rows: Vec::new(),
            repeat_header: false,
            column_configs: Vec::new(),
            header_style: TableStyle::new(11.0),
            row_style: TableStyle::new(11.0),
            border_style: TableBorderStyle::Full,
            zebra_color: None,
        }
    }

    /// Defines column widths. Can use points, percentages, or flex.
    pub fn widths<I, T>(&mut self, widths: I) -> &mut Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Size>,
    {
        self.widths = widths.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the table header cells (replaces existing header rows).
    pub fn header<I, T>(&mut self, header: I) -> &mut Self
    where
        I: IntoIterator<Item = T>,
        T: Into<TableCell>,
    {
        self.header = vec![header.into_iter().map(Into::into).collect()];
        self
    }

    /// Adds a row to the table header.
    pub fn header_row<I, T>(&mut self, header: I) -> &mut Self
    where
        I: IntoIterator<Item = T>,
        T: Into<TableCell>,
    {
        self.header
            .push(header.into_iter().map(Into::into).collect());
        self
    }

    /// Adds a header row using the RowBuilder closure.
    pub fn header_row_builder<F>(&mut self, f: F) -> &mut Self
    where
        F: FnOnce(&mut RowBuilder),
    {
        let mut builder = RowBuilder::new();
        f(&mut builder);
        self.header.push(builder.cells);
        self
    }

    /// Adds a simple row of text cells.
    pub fn row<I, T>(&mut self, row: I) -> &mut Self
    where
        I: IntoIterator<Item = T>,
        T: Into<TableCell>,
    {
        self.rows.push(row.into_iter().map(Into::into).collect());
        self
    }

    /// Adds a row using the RowBuilder closure for complex cell config.
    pub fn row_builder<F>(&mut self, f: F) -> &mut Self
    where
        F: FnOnce(&mut RowBuilder),
    {
        let mut builder = RowBuilder::new();
        f(&mut builder);
        self.rows.push(builder.cells);
        self
    }

    /// Configures default horizontal alignment for a specific column.
    pub fn column_align(&mut self, col: usize, align: Align) -> &mut Self {
        while self.column_configs.len() <= col {
            self.column_configs.push(ColumnConfig::default());
        }
        self.column_configs[col].align = align;
        self
    }

    /// Configures default vertical alignment for a specific column.
    pub fn column_valign(&mut self, col: usize, valign: VAlign) -> &mut Self {
        while self.column_configs.len() <= col {
            self.column_configs.push(ColumnConfig::default());
        }
        self.column_configs[col].valign = valign;
        self
    }

    /// Returns a mutable reference to the header style configuration.
    pub fn header_style(&mut self) -> &mut TableStyle {
        &mut self.header_style
    }

    /// Returns a mutable reference to the body row style configuration.
    pub fn row_style(&mut self) -> &mut TableStyle {
        &mut self.row_style
    }

    /// Enables header repetition on new pages.
    pub fn repeat_header(&mut self, repeat: bool) -> &mut Self {
        self.repeat_header = repeat;
        self
    }

    /// Sets the table border style.
    pub fn border(&mut self, style: TableBorderStyle) -> &mut Self {
        self.border_style = style;
        self
    }

    /// Enables zebra striping with the specified background color for even rows.
    pub fn zebra(&mut self, color: Color) -> &mut Self {
        self.zebra_color = Some(color);
        self
    }

    pub fn build(self) -> Table {
        Table {
            widths: self.widths,
            header: self.header,
            rows: self.rows,
            repeat_header: self.repeat_header,
            column_configs: self.column_configs,
            header_style: self.header_style,
            row_style: self.row_style,
            border_style: self.border_style,
            zebra_color: self.zebra_color,
        }
    }

    /// Initializes a StreamingTable that writes directly to the PDF without buffering all rows.
    pub fn start<'a, W: Write>(
        self,
        pdf: &'a mut Pdf<W>,
    ) -> std::io::Result<StreamingTable<'a, W>> {
        let available_full_w = pdf.content_width();

        let resolved_widths: Vec<f64> = self
            .widths
            .iter()
            .map(|w| match w {
                Size::Points(p) => *p,
                Size::Percent(pct) => (pct / 100.0) * available_full_w,
                Size::Flex(_) => (1.0 / self.widths.len() as f64) * available_full_w,
            })
            .collect();

        let num_cols = resolved_widths.len();
        let default_w = resolved_widths.first().copied().unwrap_or(100.0);
        let total_table_w: f64 = resolved_widths.iter().copied().sum();

        let mut st = StreamingTable {
            _widths: self.widths, // Kept if needed later
            resolved_widths,
            num_cols,
            default_w,
            total_table_w,
            header: self.header,
            repeat_header: self.repeat_header,
            column_configs: self.column_configs,
            header_style: self.header_style,
            row_style: self.row_style,
            border_style: self.border_style,
            zebra_color: self.zebra_color,
            pdf,
            row_count: 0,
            top_y: 0.0,
        };

        // Initialize and draw the header on the very first page
        st.draw_header()?;
        st.top_y = st.pdf.cursor_pos().1;

        Ok(st)
    }
}

pub struct StreamingTable<'a, W: Write> {
    _widths: Vec<Size>,
    resolved_widths: Vec<f64>,
    num_cols: usize,
    default_w: f64,
    total_table_w: f64,
    header: Vec<Vec<TableCell>>,
    repeat_header: bool,
    column_configs: Vec<ColumnConfig>,
    header_style: TableStyle,
    row_style: TableStyle,
    border_style: TableBorderStyle,
    zebra_color: Option<Color>,

    pdf: &'a mut Pdf<W>,
    row_count: usize,
    top_y: f64,
}

impl<'a, W: Write> StreamingTable<'a, W> {
    /// Adds a single row using a builder closure and immediately renders it.
    pub fn row<F>(&mut self, f: F) -> std::io::Result<()>
    where
        F: FnOnce(&mut RowBuilder),
    {
        let mut row_builder = RowBuilder::new();
        f(&mut row_builder);
        self.add_row(row_builder.cells)
    }

    /// Replaces the header rows that repeat on page break.
    /// Useful to change a "group label" header dynamically as the report progresses.
    /// The new header takes effect on the NEXT page break (not the current page).
    pub fn set_repeating_header_rows<F>(&mut self, build: F)
    where
        F: FnOnce(&mut Vec<Vec<TableCell>>),
    {
        let mut new_header: Vec<Vec<TableCell>> = Vec::new();
        build(&mut new_header);
        self.header = new_header;
    }

    /// Adds a single row manually configured.
    pub fn add_row(&mut self, row: Vec<TableCell>) -> std::io::Result<()> {
        // Evaluate the height needed for this specific row.
        // NOTE: StreamingTable processes one row at a time. Multi-row body rowspans are not supported within StreamingTable.
        let rows = [row];
        let (placements, heights) = process_rows(
            self.pdf,
            &rows,
            self.num_cols,
            &self.resolved_widths,
            self.default_w,
            self.row_style.font_size,
            self.row_style.font,
        );
        let h = heights.first().copied().unwrap_or(0.0);

        self.pdf.check_page_break(h)?;
        let pos_y = self.pdf.cursor_pos().1;

        // Page break occurred (or cursor moved higher)
        if pos_y > self.top_y {
            if self.repeat_header && !self.header.is_empty() {
                self.draw_header()?;
            }
            // Always update top_y to the actual drawing start point on the new page
            self.top_y = self.pdf.cursor_pos().1;
        }

        let start_x = self.pdf.cursor_pos().0;

        for p in &placements {
            let x = start_x
                + (0..p.start_col)
                    .map(|c| {
                        self.resolved_widths
                            .get(c)
                            .copied()
                            .unwrap_or(self.default_w)
                    })
                    .sum::<f64>();
            let bottom = self.top_y - h;
            let w = spanned_width(&self.resolved_widths, p.start_col, p.span_w, self.default_w);

            let config = self
                .column_configs
                .get(p.start_col)
                .cloned()
                .unwrap_or_default();

            // Background / Zebra coloring for EACH cell
            let mut cell_bg = p.cell.bg_color.or(self.row_style.bg_color);
            if cell_bg.is_none()
                && let Some(zc) = &self.zebra_color
                && self.row_count % 2 == 1
            {
                cell_bg = Some(*zc);
            }
            if let Some(bg) = cell_bg {
                self.pdf.set_fill_color(bg)?;
                self.pdf.fill_rect(x, bottom, w, h)?;
            }

            draw_cell_content(
                self.pdf,
                p.cell,
                x,
                bottom,
                w,
                h,
                &self.row_style,
                &config,
                false,
                self.border_style,
            )?;
        }

        self.pdf.advance_cursor(h);
        self.top_y = self.pdf.cursor_pos().1;
        self.row_count += 1;

        Ok(())
    }

    fn draw_header(&mut self) -> std::io::Result<()> {
        if self.header.is_empty() {
            return Ok(());
        }

        let (header_placements, header_heights) = process_rows(
            self.pdf,
            &self.header,
            self.num_cols,
            &self.resolved_widths,
            self.default_w,
            self.header_style.font_size,
            self.header_style.font,
        );

        if header_heights.is_empty() {
            return Ok(());
        }

        let total_h: f64 = header_heights.iter().sum();
        self.pdf.check_page_break(total_h)?;

        let (start_x, top_y) = self.pdf.cursor_pos();
        let current_y = top_y;

        if let Some(bg) = &self.header_style.bg_color {
            self.pdf.set_fill_color(*bg)?;
            self.pdf
                .fill_rect(start_x, current_y - total_h, self.total_table_w, total_h)?;
        }

        for p in &header_placements {
            let x = start_x
                + (0..p.start_col)
                    .map(|c| {
                        self.resolved_widths
                            .get(c)
                            .copied()
                            .unwrap_or(self.default_w)
                    })
                    .sum::<f64>();
            let cell_top_y = top_y - (0..p.start_row).map(|r| header_heights[r]).sum::<f64>();
            let cell_h = (0..p.span_h)
                .map(|r| header_heights[p.start_row + r])
                .sum::<f64>();
            let bottom = cell_top_y - cell_h;
            let w = spanned_width(&self.resolved_widths, p.start_col, p.span_w, self.default_w);

            let config = self
                .column_configs
                .get(p.start_col)
                .cloned()
                .unwrap_or_default();

            draw_cell_content(
                self.pdf,
                p.cell,
                x,
                bottom,
                w,
                cell_h,
                &self.header_style,
                &config,
                true,
                self.border_style,
            )?;
        }

        self.pdf.advance_cursor(total_h);
        Ok(())
    }
}

pub struct Table {
    widths: Vec<Size>,
    header: Vec<Vec<TableCell>>,
    rows: Vec<Vec<TableCell>>,
    repeat_header: bool,
    column_configs: Vec<ColumnConfig>,
    header_style: TableStyle,
    row_style: TableStyle,
    border_style: TableBorderStyle,
    zebra_color: Option<Color>,
}

fn measure<W: Write>(
    pdf: &Pdf<W>,
    text: &str,
    font_size: f64,
    bold: bool,
    font_override: Option<FontId>,
) -> f64 {
    let mut font_name = String::new();

    let base_font = font_override.or(pdf.current_font);

    if let Some(fid) = base_font {
        font_name = pdf.font_manager.get_font(fid).name.clone();
    }

    let target_font = if bold && !font_name.is_empty() {
        let bold_name = format!("{}-Bold", font_name);
        pdf.font_manager.get_font_id(&bold_name).or(base_font)
    } else {
        base_font
    };

    match target_font {
        Some(fid) => pdf.font_manager.string_width(fid, text, font_size),
        None => crate::layout::text::helvetica_string_width(text) * font_size,
    }
}

#[derive(Debug, Clone)]
struct TextSegment {
    text: String,
    bold: bool,
    color: Option<Color>,
}

fn parse_rich_text(text: &str) -> Vec<TextSegment> {
    let mut segments = Vec::new();
    let mut current_pos = 0;

    while current_pos < text.len() {
        if text[current_pos..].starts_with("**") {
            let start = current_pos + 2;
            if let Some(end_offset) = text[start..].find("**") {
                let bold_text = &text[start..start + end_offset];
                segments.push(TextSegment {
                    text: bold_text.to_string(),
                    bold: true,
                    color: None,
                });
                current_pos = start + end_offset + 2;
                continue;
            }
        }

        // Handle [color:#hex] or [#hex] color syntax (optional but premium)
        if text[current_pos..].starts_with("[#") {
            if let Some(end_bracket) = text[current_pos..].find(']') {
                let hex = &text[current_pos + 2..current_pos + end_bracket];
                if let Some(end_tag) = text[current_pos + end_bracket + 1..].find("[]") {
                    let colored_text = &text
                        [current_pos + end_bracket + 1..current_pos + end_bracket + 1 + end_tag];

                    // Simple hex parser (e.g. FF0000)
                    if hex.len() == 6 {
                        if let (Ok(r), Ok(g), Ok(b)) = (
                            u8::from_str_radix(&hex[0..2], 16),
                            u8::from_str_radix(&hex[2..4], 16),
                            u8::from_str_radix(&hex[4..6], 16),
                        ) {
                            segments.push(TextSegment {
                                text: colored_text.to_string(),
                                bold: false,
                                color: Some(Color::Rgb(r, g, b)),
                            });
                            current_pos += end_bracket + 1 + end_tag + 2;
                            continue;
                        }
                    }
                }
            }
        }

        let next_bold = text[current_pos..]
            .find("**")
            .unwrap_or(text.len() - current_pos);
        let next_color = text[current_pos..]
            .find("[#")
            .unwrap_or(text.len() - current_pos);
        let next_tag = next_bold.min(next_color);

        if next_tag > 0 {
            segments.push(TextSegment {
                text: text[current_pos..current_pos + next_tag].to_string(),
                bold: false,
                color: None,
            });
            current_pos += next_tag;
        } else {
            // Should not happen with logic above, but for safety:
            current_pos += 1;
        }
    }

    if segments.is_empty() && !text.is_empty() {
        segments.push(TextSegment {
            text: text.to_string(),
            bold: false,
            color: None,
        });
    }
    segments
}

fn wrap<W: Write>(
    pdf: &Pdf<W>,
    text: &str,
    col_width: f64,
    font_size: f64,
    font_override: Option<FontId>,
) -> Vec<Vec<TextSegment>> {
    let available = (col_width - pdf.cell_padding * 2.0).max(1.0);
    let mut lines: Vec<Vec<TextSegment>> = Vec::new();

    for explicit_line in text.split('\n') {
        let mut current_line: Vec<TextSegment> = Vec::new();
        let mut current_line_width = 0.0;

        let segments = parse_rich_text(explicit_line);

        for seg in segments {
            let words: Vec<&str> = seg.text.split_inclusive(char::is_whitespace).collect();

            for word in words {
                let mut word_w = measure(pdf, word, font_size, seg.bold, font_override);

                if current_line_width + word_w > available {
                    // If current line has content, move to next line and try again
                    if !current_line.is_empty() {
                        lines.push(current_line);
                        current_line = Vec::new();
                        current_line_width = 0.0;
                    }

                    // Re-measure after clearing current line
                    word_w = measure(pdf, word, font_size, seg.bold, font_override);

                    // If word still exceeds available width, break it down character by character
                    if word_w > available {
                        let mut current_word_fragment = String::new();
                        let mut current_fragment_w = 0.0;

                        for c in word.chars() {
                            let char_w =
                                measure(pdf, &c.to_string(), font_size, seg.bold, font_override);

                            if current_fragment_w + char_w > available {
                                if !current_word_fragment.is_empty() {
                                    lines.push(vec![TextSegment {
                                        text: current_word_fragment,
                                        bold: seg.bold,
                                        color: seg.color,
                                    }]);
                                }
                                current_word_fragment = c.to_string();
                                current_fragment_w = char_w;
                            } else {
                                current_word_fragment.push(c);
                                current_fragment_w += char_w;
                            }
                        }

                        if !current_word_fragment.is_empty() {
                            current_line.push(TextSegment {
                                text: current_word_fragment,
                                bold: seg.bold,
                                color: seg.color,
                            });
                            current_line_width = current_fragment_w;
                        }
                        continue;
                    }
                }

                // Normal case: word fits in current (possibly new) line
                if let Some(last) = current_line.last_mut()
                    && last.bold == seg.bold
                    && last.color == seg.color
                {
                    last.text.push_str(word);
                } else {
                    current_line.push(TextSegment {
                        text: word.to_string(),
                        bold: seg.bold,
                        color: seg.color,
                    });
                }
                current_line_width += word_w;
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        } else if lines.is_empty() || explicit_line.is_empty() {
            lines.push(vec![TextSegment {
                text: String::new(),
                bold: false,
                color: None,
            }]);
        }
    }

    lines
}

fn spanned_width(widths: &[f64], start_col: usize, colspan: usize, default_w: f64) -> f64 {
    (0..colspan)
        .map(|c| widths.get(start_col + c).copied().unwrap_or(default_w))
        .sum()
}

struct GridCell<'a> {
    cell: &'a TableCell,
    start_row: usize,
    start_col: usize,
    span_w: usize,
    span_h: usize,
}

fn process_rows<'a, W: Write>(
    pdf: &Pdf<W>,
    rows: &'a [Vec<TableCell>],
    num_cols: usize,
    resolved_widths: &[f64],
    default_w: f64,
    font_size: f64,
    font_override: Option<FontId>,
) -> (Vec<GridCell<'a>>, Vec<f64>) {
    let mut occupied = vec![vec![false; num_cols]; rows.len()];
    let mut placements = Vec::new();

    for (r, row) in rows.iter().enumerate() {
        let mut c = 0;
        for tc in row {
            while c < num_cols && occupied[r][c] {
                c += 1;
            }
            if c >= num_cols {
                break;
            }

            let span_w = tc.colspan.min(num_cols - c);
            let span_h = tc.rowspan;

            if r + span_h > occupied.len() {
                occupied.resize(r + span_h, vec![false; num_cols]);
            }

            placements.push(GridCell {
                cell: tc,
                start_row: r,
                start_col: c,
                span_w,
                span_h,
            });

            for rr in 0..span_h {
                for cc in 0..span_w {
                    occupied[r + rr][c + cc] = true;
                }
            }
        }
    }

    let min_row_h = font_size * pdf.line_spacing + pdf.cell_padding * 2.0;
    let mut row_heights = vec![min_row_h; occupied.len()];

    for p in &placements {
        if p.span_h == 1 {
            let w = spanned_width(resolved_widths, p.start_col, p.span_w, default_w);
            let cell_font_size = p.cell.font_size.unwrap_or(font_size);
            let cell_font = p.cell.font.or(font_override);
            let needed_h = match &p.cell.content {
                Cell::Text(t) => {
                    let lines = wrap(pdf, t, w, cell_font_size, cell_font);
                    lines.len() as f64 * (cell_font_size * pdf.line_spacing)
                        + pdf.cell_padding * 2.0
                }
                _ => cell_font_size * pdf.line_spacing + pdf.cell_padding * 2.0,
            };
            if needed_h > row_heights[p.start_row] {
                row_heights[p.start_row] = needed_h;
            }
        }
    }

    for p in &placements {
        if p.span_h > 1 {
            let w = spanned_width(resolved_widths, p.start_col, p.span_w, default_w);
            let cell_font_size = p.cell.font_size.unwrap_or(font_size);
            let cell_font = p.cell.font.or(font_override);
            let needed_h = match &p.cell.content {
                Cell::Text(t) => {
                    let lines = wrap(pdf, t, w, cell_font_size, cell_font);
                    lines.len() as f64 * (cell_font_size * pdf.line_spacing)
                        + pdf.cell_padding * 2.0
                }
                _ => cell_font_size * pdf.line_spacing + pdf.cell_padding * 2.0,
            };
            let current_h: f64 = (0..p.span_h).map(|i| row_heights[p.start_row + i]).sum();

            if needed_h > current_h {
                let extra_per_row = (needed_h - current_h) / (p.span_h as f64);
                for i in 0..p.span_h {
                    row_heights[p.start_row + i] += extra_per_row;
                }
            }
        }
    }

    (placements, row_heights)
}

fn draw_cell_content<W: Write>(
    pdf: &mut Pdf<W>,
    tc: &TableCell,
    x: f64,
    bottom_y: f64,
    w: f64,
    h: f64,
    style: &TableStyle,
    config: &ColumnConfig,
    is_header: bool,
    border_style: TableBorderStyle,
) -> std::io::Result<()> {
    let cell_font_size = tc.font_size.unwrap_or(style.font_size);
    let top_y = bottom_y + h;
    let line_height = cell_font_size * pdf.line_spacing;

    let align = tc.align.unwrap_or(config.align);
    let valign = tc.valign.unwrap_or(config.valign);

    // Individual background color can override common styles/zebra
    if let Some(bg) = tc.bg_color {
        pdf.set_fill_color(bg)?;
        pdf.fill_rect(x, bottom_y, w, h)?;
    }

    pdf.set_stroke_color(Color::Rgb(0, 0, 0))?;
    match border_style {
        TableBorderStyle::Full => {
            pdf.rect(x, bottom_y, w, h)?;
        }
        TableBorderStyle::Ghost => {
            pdf.line(x, bottom_y, x + w, bottom_y)?;
        }
        TableBorderStyle::HeaderOnly => {
            if is_header {
                pdf.line(x, bottom_y, x + w, bottom_y)?;
            }
        }
        TableBorderStyle::None => {}
    }

    match &tc.content {
        Cell::Text(text) => {
            let cell_font = tc.font.or(style.font);
            let lines = wrap(pdf, text, w, cell_font_size, cell_font);
            let total_text_h = lines.len() as f64 * line_height;
            let v_shift = match valign {
                VAlign::Top => 0.0,
                VAlign::Center => (h - 2.0 * pdf.cell_padding - total_text_h).max(0.0) / 2.0,
                VAlign::Bottom => (h - 2.0 * pdf.cell_padding - total_text_h).max(0.0),
            };

            let baseline_adj = cell_font_size * 0.2;
            let mut current_y = top_y - pdf.cell_padding - v_shift - line_height + baseline_adj;

            let final_text_color = tc
                .text_color
                .or(style.text_color)
                .unwrap_or(Color::Rgb(0, 0, 0));
            pdf.set_fill_color(final_text_color)?;

            for line in lines {
                let mut line_w = 0.0;
                for seg in &line {
                    line_w += measure(pdf, &seg.text, cell_font_size, seg.bold, cell_font);
                }

                let mut current_line_segs = line.clone();
                if let Some(fc) = tc.fill_char {
                    let available_for_fill = (w - 2.0 * pdf.cell_padding - line_w - 1.0).max(0.0); // 1.0pt safety margin
                    if available_for_fill > 0.0 {
                        let fc_w = match cell_font {
                            Some(fid) => {
                                pdf.font_manager
                                    .string_width(fid, &fc.to_string(), cell_font_size)
                            }
                            None => crate::layout::text::helvetica_char_width(fc) * cell_font_size,
                        };
                        if fc_w > 0.0 {
                            let count = (available_for_fill / fc_w).floor() as usize;
                            if count > 0 {
                                let _ = std::iter::repeat(fc).take(count).collect::<String>();
                                line_w += count as f64 * fc_w;

                                match align {
                                    Align::Left => {
                                        if count > 2 {
                                            if let Some(last) = current_line_segs.last_mut() {
                                                last.text.push(' ');
                                            } else {
                                                current_line_segs.push(TextSegment {
                                                    text: " ".to_string(),
                                                    bold: false,
                                                    color: None,
                                                });
                                            }
                                            let sub_filler = std::iter::repeat(fc)
                                                .take(count - 1)
                                                .collect::<String>();
                                            current_line_segs.push(TextSegment {
                                                text: sub_filler,
                                                bold: false,
                                                color: None,
                                            });
                                        } else {
                                            let filler = std::iter::repeat(fc)
                                                .take(count)
                                                .collect::<String>();
                                            current_line_segs.push(TextSegment {
                                                text: filler,
                                                bold: false,
                                                color: None,
                                            });
                                        }
                                    }
                                    Align::Right => {
                                        if count > 2 {
                                            current_line_segs.insert(
                                                0,
                                                TextSegment {
                                                    text: " ".to_string(),
                                                    bold: false,
                                                    color: None,
                                                },
                                            );
                                            let sub_filler = std::iter::repeat(fc)
                                                .take(count - 1)
                                                .collect::<String>();
                                            current_line_segs.insert(
                                                0,
                                                TextSegment {
                                                    text: sub_filler,
                                                    bold: false,
                                                    color: None,
                                                },
                                            );
                                        } else {
                                            let filler = std::iter::repeat(fc)
                                                .take(count)
                                                .collect::<String>();
                                            current_line_segs.insert(
                                                0,
                                                TextSegment {
                                                    text: filler,
                                                    bold: false,
                                                    color: None,
                                                },
                                            );
                                        }
                                    }
                                    Align::Center => {
                                        if count > 4 {
                                            let sub_count = count - 2;
                                            let half = sub_count / 2;
                                            let f1 = std::iter::repeat(fc)
                                                .take(half)
                                                .collect::<String>();
                                            let f2 = std::iter::repeat(fc)
                                                .take(sub_count - half)
                                                .collect::<String>();
                                            current_line_segs.insert(
                                                0,
                                                TextSegment {
                                                    text: f1,
                                                    bold: false,
                                                    color: None,
                                                },
                                            );
                                            current_line_segs.insert(
                                                1,
                                                TextSegment {
                                                    text: " ".to_string(),
                                                    bold: false,
                                                    color: None,
                                                },
                                            );
                                            current_line_segs.push(TextSegment {
                                                text: " ".to_string(),
                                                bold: false,
                                                color: None,
                                            });
                                            current_line_segs.push(TextSegment {
                                                text: f2,
                                                bold: false,
                                                color: None,
                                            });
                                        } else {
                                            let half = count / 2;
                                            let f1 = std::iter::repeat(fc)
                                                .take(half)
                                                .collect::<String>();
                                            let f2 = std::iter::repeat(fc)
                                                .take(count - half)
                                                .collect::<String>();
                                            current_line_segs.insert(
                                                0,
                                                TextSegment {
                                                    text: f1,
                                                    bold: false,
                                                    color: None,
                                                },
                                            );
                                            current_line_segs.push(TextSegment {
                                                text: f2,
                                                bold: false,
                                                color: None,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                let h_shift = match align {
                    Align::Left => 0.0,
                    Align::Center => (w - 2.0 * pdf.cell_padding - line_w).max(0.0) / 2.0,
                    Align::Right => (w - 2.0 * pdf.cell_padding - line_w).max(0.0),
                };

                let mut current_x = x + pdf.cell_padding + h_shift;

                for seg in &current_line_segs {
                    let seg_w = measure(pdf, &seg.text, cell_font_size, seg.bold, cell_font);
                    let seg_color = seg
                        .color
                        .or(tc.text_color)
                        .or(style.text_color)
                        .unwrap_or(Color::Rgb(0, 0, 0));
                    pdf.set_fill_color(seg_color)?;

                    let mut font_name = String::new();
                    let base_font = cell_font.or(pdf.current_font);

                    if let Some(fid) = base_font {
                        font_name = pdf.font_manager.get_font(fid).name.clone();
                    }

                    let target_font = if seg.bold && !font_name.is_empty() {
                        let bold_name = format!("{}-Bold", font_name);
                        pdf.font_manager.get_font_id(&bold_name).or(base_font)
                    } else {
                        base_font
                    };

                    match target_font {
                        Some(fid) => {
                            let encoded = pdf.font_manager.encode_text(fid, &seg.text);
                            let s = pdf.get_stream();
                            s.push_str("BT\n");
                            s.push_str(&format!("/F{} {:.1} Tf\n", fid.0, cell_font_size));
                            s.push_str(&format!("{:.2} {:.2} Td\n", current_x, current_y));
                            s.push_str(&format!("{} Tj\n", encoded));
                            s.push_str("ET\n");
                        }
                        None => {
                            let escaped = escape_pdf_str(&seg.text);
                            let s = pdf.get_stream();
                            s.push_str("BT\n");
                            s.push_str(&format!("/FBuiltin {:.1} Tf\n", cell_font_size));
                            s.push_str(&format!("{:.2} {:.2} Td\n", current_x, current_y));
                            s.push_str(&format!("({}) Tj\n", escaped));
                            s.push_str("ET\n");
                        }
                    }
                    current_x += seg_w;
                }

                if let Some(url) = &tc.link {
                    pdf.add_link(
                        (
                            x + pdf.cell_padding + h_shift,
                            current_y,
                            x + pdf.cell_padding + h_shift + line_w,
                            current_y + cell_font_size,
                        ),
                        url,
                    );
                }

                current_y -= line_height;
            }
        }
        Cell::ImagePath(path) => {
            let pad = 3.0;
            let iw = w - pad * 2.0;
            let ih = h - pad * 2.0;
            if iw > 0.0 && ih > 0.0 {
                pdf.image(path)
                    .position(x + pad, bottom_y + pad)
                    .size(iw, ih)
                    .render()?;
            }
        }
        Cell::ImageBase64(b64) => {
            let pad = 3.0;
            let iw = w - pad * 2.0;
            let ih = h - pad * 2.0;
            if iw > 0.0 && ih > 0.0 {
                pdf.image_base64(b64)
                    .position(x + pad, bottom_y + pad)
                    .size(iw, ih)
                    .render()?;
            }
        }
        #[cfg(feature = "qrcode")]
        Cell::QrCode(data) => {
            let pad = 3.0;
            let size = w.min(h) - pad * 2.0;
            if size > 0.0 {
                let qx = x + (w - size) / 2.0;
                let qy = bottom_y + (h - size) / 2.0;
                pdf.render_qr(data, qx, qy, size)?;
            }
        }
    }

    if let Some(url) = &tc.link {
        pdf.add_link((x, bottom_y, x + w, bottom_y + h), url);
    }

    Ok(())
}

impl Table {
    pub fn render<W: Write>(&self, pdf: &mut Pdf<W>) -> std::io::Result<()> {
        let available_full_w = pdf.content_width();

        let resolved_widths: Vec<f64> = self
            .widths
            .iter()
            .map(|w| match w {
                Size::Points(p) => *p,
                Size::Percent(pct) => (pct / 100.0) * available_full_w,
                Size::Flex(_) => (1.0 / self.widths.len() as f64) * available_full_w,
            })
            .collect();

        let num_cols = resolved_widths.len();
        let default_w = resolved_widths.first().copied().unwrap_or(100.0);
        let total_table_w: f64 = resolved_widths.iter().copied().sum();

        let (header_placements, header_heights) = process_rows(
            pdf,
            &self.header,
            num_cols,
            &resolved_widths,
            default_w,
            self.header_style.font_size,
            self.header_style.font,
        );

        let (body_placements, body_heights) = process_rows(
            pdf,
            &self.rows,
            num_cols,
            &resolved_widths,
            default_w,
            self.row_style.font_size,
            self.row_style.font,
        );

        let draw_header = |pdf: &mut Pdf<W>| -> std::io::Result<()> {
            if header_heights.is_empty() {
                return Ok(());
            }

            let total_h: f64 = header_heights.iter().sum();
            pdf.check_page_break(total_h)?;

            let (start_x, top_y) = pdf.cursor_pos();
            let current_y = top_y;

            if let Some(bg) = &self.header_style.bg_color {
                pdf.set_fill_color(*bg)?;
                pdf.fill_rect(start_x, current_y - total_h, total_table_w, total_h)?;
            }

            for p in &header_placements {
                let x = start_x
                    + (0..p.start_col)
                        .map(|c| resolved_widths.get(c).copied().unwrap_or(default_w))
                        .sum::<f64>();
                let cell_top_y = top_y - (0..p.start_row).map(|r| header_heights[r]).sum::<f64>();
                let cell_h = (0..p.span_h)
                    .map(|r| header_heights[p.start_row + r])
                    .sum::<f64>();
                let bottom = cell_top_y - cell_h;
                let w = spanned_width(&resolved_widths, p.start_col, p.span_w, default_w);

                let config = self
                    .column_configs
                    .get(p.start_col)
                    .cloned()
                    .unwrap_or_default();

                draw_cell_content(
                    pdf,
                    p.cell,
                    x,
                    bottom,
                    w,
                    cell_h,
                    &self.header_style,
                    &config,
                    true,
                    self.border_style,
                )?;
            }

            pdf.advance_cursor(total_h);
            Ok(())
        };

        if !self.header.is_empty() {
            draw_header(pdf)?;
        }

        let mut top_y = pdf.cursor_pos().1;

        for r in 0..body_heights.len() {
            let h = body_heights[r];
            pdf.check_page_break(h)?;

            let pos_y = pdf.cursor_pos().1;
            if pos_y > top_y {
                if self.repeat_header && !self.header.is_empty() {
                    draw_header(pdf)?;
                }
                // Always sync top_y to the current page cursor
                top_y = pdf.cursor_pos().1;
            }

            let start_x = pdf.cursor_pos().0;

            for p in &body_placements {
                if p.start_row == r {
                    let x = start_x
                        + (0..p.start_col)
                            .map(|c| resolved_widths.get(c).copied().unwrap_or(default_w))
                            .sum::<f64>();
                    let cell_h = (0..p.span_h)
                        .map(|ri| body_heights[p.start_row + ri])
                        .sum::<f64>();
                    let bottom = top_y - cell_h;
                    let w = spanned_width(&resolved_widths, p.start_col, p.span_w, default_w);

                    let config = self
                        .column_configs
                        .get(p.start_col)
                        .cloned()
                        .unwrap_or_default();

                    // Background coloring for EACH cell
                    let mut cell_bg = p.cell.bg_color.or(self.row_style.bg_color);
                    if cell_bg.is_none()
                        && let Some(zc) = &self.zebra_color
                        && r % 2 == 1
                    {
                        cell_bg = Some(*zc);
                    }

                    if let Some(bg) = cell_bg {
                        pdf.set_fill_color(bg)?;
                        pdf.fill_rect(x, bottom, w, cell_h)?;
                    }

                    draw_cell_content(
                        pdf,
                        p.cell,
                        x,
                        bottom,
                        w,
                        cell_h,
                        &self.row_style,
                        &config,
                        false,
                        self.border_style,
                    )?;
                }
            }

            top_y -= h;
            pdf.advance_cursor(h);
        }

        Ok(())
    }
}
