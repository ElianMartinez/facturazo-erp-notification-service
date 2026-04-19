// PDF Report Streaming Engine — usa mr_pdf
// Diseño: API funcional con callback - sin state machine ni self-referencias

use mr_pdf::{
    Align, Color as MrColor, PaperSize as MrPaperSize, Pdf, SizeExt, TableBorderStyle, VAlign,
};
use std::fs::File;
use std::io::Write;

// ============================================================================
// Public configuration types
// ============================================================================

#[derive(Clone, Copy, Debug)]
pub enum PageSize {
    A3,
    A4,
    A5,
    Letter,
    Legal,
    Tabloid,
}

impl PageSize {
    pub fn dimensions_pt(&self) -> (f64, f64) {
        match self {
            PageSize::A3 => (841.89, 1190.55),
            PageSize::A4 => (595.28, 841.89),
            PageSize::A5 => (419.53, 595.28),
            PageSize::Letter => (612.0, 792.0),
            PageSize::Legal => (612.0, 1008.0),
            PageSize::Tabloid => (792.0, 1224.0),
        }
    }

    pub fn to_mr_pdf(&self) -> MrPaperSize {
        match self {
            PageSize::A3 => MrPaperSize::A3,
            PageSize::A4 => MrPaperSize::A4,
            PageSize::A5 => MrPaperSize::A5,
            PageSize::Letter => MrPaperSize::Custom(612.0, 792.0),
            PageSize::Legal => MrPaperSize::Custom(612.0, 1008.0),
            PageSize::Tabloid => MrPaperSize::Custom(792.0, 1224.0),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Orientation {
    Portrait,
    Landscape,
}

impl Orientation {
    pub fn to_mr_pdf(&self) -> mr_pdf::Orientation {
        match self {
            Orientation::Portrait => mr_pdf::Orientation::Portrait,
            Orientation::Landscape => mr_pdf::Orientation::Landscape,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Margins {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

impl Margins {
    pub fn all(mm: f32) -> Self {
        Self {
            top: mm,
            bottom: mm,
            left: mm,
            right: mm,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Alignment {
    Left,
    Right,
    Center,
}

impl Alignment {
    pub fn to_mr_pdf(&self) -> Align {
        match self {
            Alignment::Left => Align::Left,
            Alignment::Right => Align::Right,
            Alignment::Center => Align::Center,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ColumnWidth {
    Fixed(f32),
    Auto,
    Flex(f32),
}

#[derive(Clone)]
pub struct Column {
    pub name: String,
    pub width: ColumnWidth,
    pub align: Alignment,
}

#[derive(Clone)]
pub struct Branding {
    pub primary: (u8, u8, u8),
    pub primary_fg: (u8, u8, u8),
    pub accent: (u8, u8, u8),
    pub accent_fg: (u8, u8, u8),
    pub text_dark: (u8, u8, u8),
    pub text_light: (u8, u8, u8),
    pub row_alt: (u8, u8, u8),
    pub border: (u8, u8, u8),
    pub muted: (u8, u8, u8),
}

impl Branding {
    pub fn facturazo() -> Self {
        // Paleta clara estilo Fiori/QuickBooks: contraste por tipografía, no por color
        Self {
            primary: (241, 245, 249),    // slate-100 — column headers (claro)
            primary_fg: (15, 23, 42),    // slate-900 — texto sobre headers claros
            accent: (226, 232, 240),     // slate-200 — group label bar
            accent_fg: (15, 23, 42),     // slate-900 — texto sobre group bar
            text_dark: (15, 23, 42),     // slate-900 — texto base
            text_light: (255, 255, 255), // blanco — solo para grand total (fondo oscuro)
            row_alt: (248, 250, 252),    // slate-50 — zebra apenas perceptible
            border: (226, 232, 240),     // slate-200 — bordes suaves
            muted: (100, 116, 139),      // slate-500
        }
    }
}

#[derive(Clone)]
pub struct ReportConfig {
    pub title: String,
    pub subtitle: String,
    pub company: String,
    pub company_address: String,
    pub company_id: String,
    pub breadcrumb: String,
    pub period: String,
    pub generated_at: String,
    pub user_name: String,
    pub section_label: Option<String>,
    pub page_size: PageSize,
    pub orientation: Orientation,
    pub margins: Margins,
    pub scale: f32,
    pub font_size_base: f32,
    pub repeat_header: bool,
    pub columns: Vec<Column>,
    pub branding: Branding,
}

// ============================================================================
// Public API: trait + render function
// ============================================================================

const MM_TO_PT: f32 = 2.83465;

/// Registra Roboto-Regular como default y Roboto-Bold (opcional) en el PDF.
/// El primer font registrado se vuelve el default automáticamente — por eso Regular va primero.
/// Si los archivos no existen, cae a Helvetica built-in (WinAnsi parcheado).
fn register_roboto_fonts<W: Write>(pdf: &mut Pdf<W>) -> Option<mr_pdf::FontId> {
    let font_dir = std::env::var("PDF_FONTS_DIR").unwrap_or_else(|_| "fonts".to_string());
    let regular_path = format!("{}/Roboto-Regular.ttf", font_dir);
    let bold_path = format!("{}/Roboto-Bold.ttf", font_dir);

    if std::path::Path::new(&regular_path).exists() {
        let _ = pdf.register_font("Regular", &regular_path);
    }
    if std::path::Path::new(&bold_path).exists() {
        pdf.register_font("Bold", &bold_path).ok()
    } else {
        None
    }
}

/// Source of rows. Implement this for any iterable: DB cursor, channel rx, Vec, etc.
pub trait RowSource {
    fn next_row(&mut self) -> Option<Vec<String>>;
}

/// Convenience implementation for any FnMut returning Option<Vec<String>>
pub struct ClosureRowSource<F>(pub F);
impl<F> RowSource for ClosureRowSource<F>
where
    F: FnMut() -> Option<Vec<String>>,
{
    fn next_row(&mut self) -> Option<Vec<String>> {
        (self.0)()
    }
}

/// A group of rows with optional subtotal
pub struct GroupData {
    pub label: String,
    pub rows: Vec<Vec<String>>,
    pub subtotal: Vec<String>, // Same length as columns; empty string for non-aggregated columns
}

/// Hierarchical group — supports nested sub-groups recursively.
/// Leaf groups have rows; parent groups have sub_groups.
pub struct HierarchicalGroup {
    pub label: String,
    pub level: usize,
    pub rows: Vec<Vec<String>>,
    pub sub_groups: Vec<HierarchicalGroup>,
    pub subtotal: Vec<String>, // shown after rows (leaf) or after all sub_groups (parent)
}

pub fn render_report<R: RowSource>(
    config: &ReportConfig,
    output_path: &str,
    rows: &mut R,
    summary: &[(String, String)],
) -> Result<u64, Box<dyn std::error::Error>> {
    let file = File::create(output_path)?;
    let mut pdf = Pdf::stream(file)?;

    pdf.set_paper_size(config.page_size.to_mr_pdf());
    pdf.set_orientation(config.orientation.to_mr_pdf());

    let avg_margin =
        (config.margins.top + config.margins.bottom + config.margins.left + config.margins.right)
            / 4.0;
    pdf.set_margin((avg_margin * MM_TO_PT) as f64);

    pdf.cell_padding = (5.0 * config.scale as f64).max(1.0);
    pdf.line_spacing = 1.15;

    let _bold_font_id = register_roboto_fonts(&mut pdf);

    let scale = config.scale;

    // Header genérico (mismo que render_grouped_report y render_hierarchical_report)
    render_page_header(&mut pdf, config)?;

    // Etiqueta de sección opcional
    if let Some(section) = &config.section_label {
        let small_size = (7.5 * scale) as f64;
        pdf.text(&format!("  {}  ", section.to_uppercase()))
            .size(small_size)
            .color(rgb(config.branding.text_dark))
            .align_left()
            .margin_bottom(2.0);
    }

    let usable_width = compute_usable_width_pt(config);
    let column_widths_pt = compute_column_widths(
        &config.columns,
        usable_width,
        config.font_size_base * scale,
        scale,
    );

    let header_size = ((config.font_size_base + 0.5) * scale) as f64;
    let row_size = (config.font_size_base * scale) as f64;

    let mut total_rows: u64 = 0;

    // ---- StreamingTable scope ----
    {
        let mut tb = mr_pdf::TableBuilder::new();
        tb.widths(
            column_widths_pt
                .iter()
                .map(|w| *w as f64)
                .collect::<Vec<_>>(),
        )
        .repeat_header(config.repeat_header)
        .border(TableBorderStyle::Full)
        .zebra(rgb(config.branding.row_alt))
        .header(
            config
                .columns
                .iter()
                .map(|c| c.name.clone())
                .collect::<Vec<_>>(),
        );

        tb.header_style()
            .bg_color(rgb(config.branding.primary))
            .text_color(rgb(config.branding.primary_fg))
            .font_size(header_size);

        tb.row_style().font_size(row_size);

        for (i, col) in config.columns.iter().enumerate() {
            tb.column_align(i, col.align.to_mr_pdf());
            tb.column_valign(i, VAlign::Center);
        }

        let mut streaming_table = tb.start(&mut pdf)?;

        while let Some(row) = rows.next_row() {
            streaming_table.row(|r| {
                for cell in &row {
                    r.cell(cell);
                }
            })?;
            total_rows += 1;
        }
        // streaming_table dropped here
    }

    // ---- Summary table ----
    if !summary.is_empty() {
        pdf.text("").size(4.0_f64 * scale as f64).margin_bottom(4.0);

        let total_bg = palette_grand_total();
        let text_light = rgb(config.branding.text_light);
        let total_font = ((config.font_size_base + 1.5) * scale) as f64;
        let summary_owned: Vec<(String, String)> = summary.to_vec();

        pdf.table(|t| {
            t.widths(vec![160.0, 110.0]);
            t.column_align(0, Align::Right);
            t.column_align(1, Align::Right);

            for (i, (label, value)) in summary_owned.iter().enumerate() {
                let is_total = i == summary_owned.len() - 1;
                if is_total {
                    t.row_style()
                        .font_size(total_font)
                        .bg_color(total_bg)
                        .text_color(text_light);
                } else {
                    t.row_style().font_size(row_size);
                }
                t.row(vec![label.as_str(), value.as_str()]);
            }
        })?;
    }

    pdf.finish()?;
    Ok(total_rows)
}

// ============================================================================
// GROUPED REPORT — una tabla por grupo, header del grupo se repite en cada página
// ============================================================================

pub fn render_grouped_report(
    config: &ReportConfig,
    output_path: &str,
    groups: Vec<GroupData>,
    grand_total: Vec<String>,
) -> Result<u64, Box<dyn std::error::Error>> {
    let file = File::create(output_path)?;
    let mut pdf = Pdf::stream(file)?;

    pdf.set_paper_size(config.page_size.to_mr_pdf());
    pdf.set_orientation(config.orientation.to_mr_pdf());

    let avg_margin =
        (config.margins.top + config.margins.bottom + config.margins.left + config.margins.right)
            / 4.0;
    pdf.set_margin((avg_margin * MM_TO_PT) as f64);
    pdf.cell_padding = (5.0 * config.scale as f64).max(1.0);
    pdf.line_spacing = 1.15;

    let bold_font_id = register_roboto_fonts(&mut pdf);

    let scale = config.scale;

    // Header de la página (3 zonas)
    render_page_header(&mut pdf, config)?;

    let usable_width = compute_usable_width_pt(config);
    let column_widths_pt = compute_column_widths(
        &config.columns,
        usable_width,
        config.font_size_base * scale,
        scale,
    );

    let header_size = ((config.font_size_base + 0.5) * scale) as f64;
    let row_size = (config.font_size_base * scale) as f64;
    let group_label_size = ((config.font_size_base + 1.5) * scale) as f64;

    let n_cols = config.columns.len();
    let mut total_rows: u64 = 0;

    // ============================================
    // STREAMING per group: uses TableBuilder.start()
    // Multiple header_row_builder calls = both repeat on page break
    // Rows are flushed to PDF immediately, no buffering
    // ============================================
    for group in groups {
        let _ = pdf.text("").size((2.0 * scale) as f64).margin_bottom(2.0);

        let mut tb = mr_pdf::TableBuilder::new();
        tb.widths(
            column_widths_pt
                .iter()
                .map(|w| *w as f64)
                .collect::<Vec<_>>(),
        )
        .repeat_header(true)
        .border(TableBorderStyle::Full)
        .zebra(rgb(config.branding.row_alt));

        for (i, col) in config.columns.iter().enumerate() {
            tb.column_align(i, col.align.to_mr_pdf());
            tb.column_valign(i, VAlign::Center);
        }

        // ROW STYLE for data rows
        tb.row_style().font_size(row_size);

        let text_dark = rgb(config.branding.text_dark);
        // HEADER ROW 1: group label spanning all columns
        let group_label_for_header = group.label.clone();
        let accent = rgb(config.branding.accent);
        let accent_fg = rgb(config.branding.accent_fg);
        let bold_for_header = bold_font_id;
        tb.header_row_builder(move |row| {
            let cell = row
                .cell(&group_label_for_header)
                .span(n_cols)
                .align(Align::Left)
                .bg_color(accent)
                .text_color(accent_fg)
                .font_size(group_label_size);
            if let Some(b) = bold_for_header {
                cell.font(b);
            }
        });

        // HEADER ROW 2: column headers
        let cols_for_header = config.columns.clone();
        let primary = rgb(config.branding.primary);
        let primary_fg = rgb(config.branding.primary_fg);
        let bold_for_cols = bold_font_id;
        tb.header_row_builder(move |row| {
            for col in cols_for_header.iter() {
                let cell = row
                    .cell(&col.name)
                    .align(col.align.to_mr_pdf())
                    .bg_color(primary)
                    .text_color(primary_fg)
                    .font_size(header_size);
                if let Some(b) = bold_for_cols {
                    cell.font(b);
                }
            }
        });

        // Stream rows
        let mut streaming = tb.start(&mut pdf)?;
        let group_rows = group.rows;
        for row in &group_rows {
            streaming.row(|rb| {
                for cell in row {
                    rb.cell(cell);
                }
            })?;
        }

        // Subtotal as last row(s) — also streamed
        if !group.subtotal.is_empty() {
            let subtotal_owned = group.subtotal;
            let subtotal_bg = palette_leaf_subtotal();
            let bold_for_subtotal = bold_font_id;
            streaming.row(|rb| {
                for cell in &subtotal_owned {
                    let c = rb
                        .cell(cell)
                        .bg_color(subtotal_bg)
                        .text_color(text_dark)
                        .font_size(row_size);
                    if let Some(b) = bold_for_subtotal {
                        c.font(b);
                    }
                }
            })?;
        }
        drop(streaming);

        total_rows += group_rows.len() as u64;
    }

    // ---- Grand Total ----
    if !grand_total.is_empty() {
        let _ = pdf.text("").size((4.0 * scale) as f64).margin_bottom(4.0);

        let grand_bg = palette_grand_total();
        let text_light = rgb(config.branding.text_light);
        let total_font = ((config.font_size_base + 1.5) * scale) as f64;
        let widths = column_widths_pt.clone();
        let gt = grand_total.clone();
        let bold_for_total = bold_font_id;

        pdf.table(|t| {
            t.widths(widths.iter().map(|w| *w as f64).collect::<Vec<_>>());
            t.border(TableBorderStyle::None);
            for (i, col) in config.columns.iter().enumerate() {
                t.column_align(i, col.align.to_mr_pdf());
            }
            let style = t
                .row_style()
                .font_size(total_font)
                .bg_color(grand_bg)
                .text_color(text_light);
            if let Some(b) = bold_for_total {
                style.font(b);
            }
            t.row(gt.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        })?;
    }

    pdf.finish()?;
    Ok(total_rows)
}

// ============================================================================
// HIERARCHICAL REPORT — recursivo, streaming por hoja
// Cada hoja es una tabla con TODOS los labels de ancestros como headers que se repiten
// ============================================================================

pub fn render_hierarchical_report(
    config: &ReportConfig,
    output_path: &str,
    groups: Vec<HierarchicalGroup>,
    grand_total: Vec<String>,
) -> Result<u64, Box<dyn std::error::Error>> {
    let file = File::create(output_path)?;
    let mut pdf = Pdf::stream(file)?;

    pdf.set_paper_size(config.page_size.to_mr_pdf());
    pdf.set_orientation(config.orientation.to_mr_pdf());

    let avg_margin =
        (config.margins.top + config.margins.bottom + config.margins.left + config.margins.right)
            / 4.0;
    pdf.set_margin((avg_margin * MM_TO_PT) as f64);
    pdf.cell_padding = (5.0 * config.scale as f64).max(1.0);
    pdf.line_spacing = 1.15;

    let bold_font_id = register_roboto_fonts(&mut pdf);

    let scale = config.scale;
    let header_size = ((config.font_size_base + 0.5) * scale) as f64;
    let row_size = (config.font_size_base * scale) as f64;
    let group_label_size = ((config.font_size_base + 1.0) * scale) as f64;
    let parent_total_size = ((config.font_size_base + 0.5) * scale) as f64;
    let grand_total_size = ((config.font_size_base + 1.5) * scale) as f64;
    let n_cols = config.columns.len();

    let usable_width = compute_usable_width_pt(config);
    let column_widths_pt = compute_column_widths(
        &config.columns,
        usable_width,
        config.font_size_base * scale,
        scale,
    );

    // Page header de empresa SOLO en página 1 (no callback)
    render_page_header(&mut pdf, config)?;

    // ============================================
    // UNA SOLA TABLA continua para todo el reporte
    // ============================================
    let mut tb = mr_pdf::TableBuilder::new();
    tb.widths(
        column_widths_pt
            .iter()
            .map(|w| *w as f64)
            .collect::<Vec<_>>(),
    )
    .repeat_header(true)
    .border(TableBorderStyle::Full)
    .zebra(rgb(config.branding.row_alt));

    for (i, col) in config.columns.iter().enumerate() {
        tb.column_align(i, col.align.to_mr_pdf());
        tb.column_valign(i, VAlign::Center);
    }
    tb.row_style().font_size(row_size);

    // Header inicial: PRIMER group label + column headers
    // Así en página 1 ya aparece "Ventas › Hatillo" arriba de las columnas
    let first_leaf_path = find_first_leaf_path(&groups);
    let first_label = first_leaf_path.join("  ›  ");

    let group_bg_init = palette_group_header(&config.branding);
    let group_fg_init = rgb(config.branding.accent_fg);
    let cols_for_header = config.columns.clone();
    let col_bg = palette_column_header(&config.branding);
    let col_fg = rgb(config.branding.primary_fg);
    let n_cols_init = n_cols;

    let first_label_clone = first_label.clone();
    let bold_for_header = bold_font_id;
    tb.header_row_builder(move |row| {
        let cell = row
            .cell(&first_label_clone)
            .span(n_cols_init)
            .align(Align::Left)
            .bg_color(group_bg_init)
            .text_color(group_fg_init)
            .font_size(group_label_size);
        if let Some(b) = bold_for_header {
            cell.font(b);
        }
    });
    tb.header_row_builder(move |row| {
        for col in cols_for_header.iter() {
            let cell = row
                .cell(&col.name)
                .align(col.align.to_mr_pdf())
                .bg_color(col_bg)
                .text_color(col_fg)
                .font_size(header_size);
            if let Some(b) = bold_for_header {
                cell.font(b);
            }
        }
    });

    let mut streaming = tb.start(&mut pdf)?;

    let mut total_rows: u64 = 0;
    let group_bg = palette_group_header(&config.branding);
    let group_fg = rgb(config.branding.accent_fg);
    let leaf_subtotal_bg = palette_leaf_subtotal();
    let parent_total_bg = palette_parent_total();
    let grand_total_bg = palette_grand_total();
    let text_dark = rgb(config.branding.text_dark);
    let text_light = rgb(config.branding.text_light);

    // Recursive streaming
    fn stream_group<W: Write>(
        streaming: &mut mr_pdf::layout::table::StreamingTable<W>,
        group: &HierarchicalGroup,
        path: &[String],
        n_cols: usize,
        group_bg: MrColor,
        group_fg: MrColor,
        leaf_subtotal_bg: MrColor,
        parent_total_bg: MrColor,
        text_dark: MrColor,
        text_light: MrColor,
        col_bg: MrColor,
        col_fg: MrColor,
        column_names: &[(String, Align)],
        group_label_size: f64,
        parent_total_size: f64,
        row_size: f64,
        header_size: f64,
        total_rows: &mut u64,
        is_first_leaf: &mut bool,
        bold_font: Option<mr_pdf::FontId>,
    ) -> std::io::Result<()> {
        if group.sub_groups.is_empty() {
            // LEAF: actualizar header repetible para incluir el label de este grupo
            let composite_label = path.join("  ›  ");

            let lbl_for_repeat = composite_label.clone();
            let cols_clone: Vec<(String, Align)> = column_names.to_vec();
            streaming.set_repeating_header_rows(|headers| {
                let mut group_row = Vec::new();
                let mut group_cell: mr_pdf::layout::table::TableCell =
                    lbl_for_repeat.as_str().into();
                group_cell.colspan = n_cols;
                group_cell.align = Some(Align::Left);
                group_cell.bg_color = Some(group_bg);
                group_cell.text_color = Some(group_fg);
                group_cell.font_size = Some(group_label_size);
                group_cell.font = bold_font;
                group_row.push(group_cell);
                headers.push(group_row);

                let mut col_row = Vec::new();
                for (name, align) in &cols_clone {
                    let mut c: mr_pdf::layout::table::TableCell = name.as_str().into();
                    c.align = Some(*align);
                    c.bg_color = Some(col_bg);
                    c.text_color = Some(col_fg);
                    c.font_size = Some(header_size);
                    c.font = bold_font;
                    col_row.push(c);
                }
                headers.push(col_row);
            });

            if !*is_first_leaf {
                streaming.row(|rb| {
                    let cell = rb
                        .cell(&composite_label)
                        .span(n_cols)
                        .align(Align::Left)
                        .bg_color(group_bg)
                        .text_color(group_fg)
                        .font_size(group_label_size);
                    if let Some(b) = bold_font {
                        cell.font(b);
                    }
                })?;
            }
            *is_first_leaf = false;

            for row in &group.rows {
                streaming.row(|rb| {
                    for cell in row {
                        rb.cell(cell);
                    }
                })?;
            }
            *total_rows += group.rows.len() as u64;

            // Subtotal con span en celdas vacías iniciales
            if !group.subtotal.is_empty() {
                emit_subtotal_row(
                    streaming,
                    &group.subtotal,
                    leaf_subtotal_bg,
                    text_dark,
                    row_size,
                    bold_font,
                )?;
            }
        } else {
            // PARENT: recurse into children
            for child in &group.sub_groups {
                let mut child_path: Vec<String> = path.to_vec();
                child_path.push(child.label.clone());
                stream_group(
                    streaming,
                    child,
                    &child_path,
                    n_cols,
                    group_bg,
                    group_fg,
                    leaf_subtotal_bg,
                    parent_total_bg,
                    text_dark,
                    text_light,
                    col_bg,
                    col_fg,
                    column_names,
                    group_label_size,
                    parent_total_size,
                    row_size,
                    header_size,
                    total_rows,
                    is_first_leaf,
                    bold_font,
                )?;
            }
            // After all children: emit parent total
            if !group.subtotal.is_empty() {
                emit_subtotal_row(
                    streaming,
                    &group.subtotal,
                    parent_total_bg,
                    text_dark,
                    parent_total_size,
                    bold_font,
                )?;
            }
        }
        Ok(())
    }

    fn emit_subtotal_row<W: Write>(
        streaming: &mut mr_pdf::layout::table::StreamingTable<W>,
        subtotal: &[String],
        bg: MrColor,
        fg: MrColor,
        size: f64,
        bold_font: Option<mr_pdf::FontId>,
    ) -> std::io::Result<()> {
        // Detect span = leading cells until first non-empty after [0]
        let mut span = 1;
        for i in 1..subtotal.len() {
            if subtotal[i].is_empty() {
                span += 1;
            } else {
                break;
            }
        }
        let subtotal_owned = subtotal.to_vec();
        streaming.row(|rb| {
            // Spanned label cell — bold
            let label_cell = rb
                .cell(&subtotal_owned[0])
                .span(span)
                .align(Align::Left)
                .bg_color(bg)
                .text_color(fg)
                .font_size(size);
            if let Some(b) = bold_font {
                label_cell.font(b);
            }
            // Remaining individual cells — bold
            for i in span..subtotal_owned.len() {
                let c = rb
                    .cell(&subtotal_owned[i])
                    .bg_color(bg)
                    .text_color(fg)
                    .font_size(size);
                if let Some(b) = bold_font {
                    c.font(b);
                }
            }
        })?;
        Ok(())
    }

    let column_names: Vec<(String, Align)> = config
        .columns
        .iter()
        .map(|c| (c.name.clone(), c.align.to_mr_pdf()))
        .collect();

    let mut is_first_leaf = true;
    for top in groups {
        let path: Vec<String> = vec![top.label.clone()];
        stream_group(
            &mut streaming,
            &top,
            &path,
            n_cols,
            group_bg,
            group_fg,
            leaf_subtotal_bg,
            parent_total_bg,
            text_dark,
            text_light,
            col_bg,
            col_fg,
            &column_names,
            group_label_size,
            parent_total_size,
            row_size,
            header_size,
            &mut total_rows,
            &mut is_first_leaf,
            bold_font_id,
        )?;
    }

    // GRAND TOTAL como última fila (también con span)
    if !grand_total.is_empty() {
        emit_subtotal_row(
            &mut streaming,
            &grand_total,
            grand_total_bg,
            text_light,
            grand_total_size,
            bold_font_id,
        )?;
    }

    drop(streaming);
    pdf.finish()?;
    Ok(total_rows)
}

fn render_group_recursive<W: Write>(
    pdf: &mut Pdf<W>,
    config: &ReportConfig,
    column_widths_pt: &[f32],
    group: &HierarchicalGroup,
    path: &[String], // labels from root to this group
    total_rows: &mut u64,
) -> Result<(), Box<dyn std::error::Error>> {
    if group.sub_groups.is_empty() {
        // LEAF: stream the rows
        render_leaf_table(pdf, config, column_widths_pt, group, path)?;
        *total_rows += group.rows.len() as u64;
    } else {
        // PARENT: recurse into sub_groups
        for child in &group.sub_groups {
            let mut child_path: Vec<String> = path.to_vec();
            child_path.push(child.label.clone());
            render_group_recursive(
                pdf,
                config,
                column_widths_pt,
                child,
                &child_path,
                total_rows,
            )?;

            // Subtotal of child after its rows (if leaf, already done; if parent, do it here)
            if !child.sub_groups.is_empty() && !child.subtotal.is_empty() {
                render_total_row(
                    pdf,
                    config,
                    column_widths_pt,
                    &child.subtotal,
                    child.level,
                    false,
                    &format!("Total {}", child.label),
                )?;
            }
        }
    }
    Ok(())
}

/// Render a leaf streaming table with ONE composite header row (e.g. "Ventas — Hatillo")
fn render_leaf_table<W: Write>(
    pdf: &mut Pdf<W>,
    config: &ReportConfig,
    column_widths_pt: &[f32],
    leaf: &HierarchicalGroup,
    path: &[String], // includes leaf.label as last element
) -> Result<(), Box<dyn std::error::Error>> {
    let scale = config.scale;
    let header_size = ((config.font_size_base + 0.5) * scale) as f64;
    let row_size = (config.font_size_base * scale) as f64;
    let n_cols = config.columns.len();

    let _ = pdf.text("").size((2.5 * scale) as f64).margin_bottom(3.0);

    let mut tb = mr_pdf::TableBuilder::new();
    tb.widths(
        column_widths_pt
            .iter()
            .map(|w| *w as f64)
            .collect::<Vec<_>>(),
    )
    .repeat_header(true)
    .border(TableBorderStyle::Full)
    .zebra(rgb(config.branding.row_alt));

    for (i, col) in config.columns.iter().enumerate() {
        tb.column_align(i, col.align.to_mr_pdf());
        tb.column_valign(i, VAlign::Center);
    }
    tb.row_style().font_size(row_size);

    // SINGLE composite header row: "Parent — Child — ..."
    let composite_label = path.join("  ›  ");
    let label_size = ((config.font_size_base + 2.0) * scale) as f64;
    let group_bg = palette_group_header(&config.branding);
    let group_fg = rgb(config.branding.accent_fg);
    tb.header_row_builder(move |row| {
        row.cell(&composite_label)
            .span(n_cols)
            .align(Align::Left)
            .bg_color(group_bg)
            .text_color(group_fg)
            .font_size(label_size);
    });

    // Column headers row
    let cols_for_header = config.columns.clone();
    let col_bg = palette_column_header(&config.branding);
    let col_fg = rgb(config.branding.primary_fg);
    tb.header_row_builder(|row| {
        for col in cols_for_header.iter() {
            row.cell(&col.name)
                .align(col.align.to_mr_pdf())
                .bg_color(col_bg)
                .text_color(col_fg)
                .font_size(header_size);
        }
    });

    // Stream rows
    let mut streaming = tb.start(pdf)?;
    for row in &leaf.rows {
        streaming.row(|rb| {
            for cell in row {
                rb.cell(cell);
            }
        })?;
    }

    // Leaf subtotal as last row (light green)
    if !leaf.subtotal.is_empty() {
        let subtotal_owned: Vec<String> = leaf.subtotal.clone();
        let subtotal_bg = palette_leaf_subtotal();
        let text_dark = rgb(config.branding.text_dark);
        let subtotal_size = ((config.font_size_base + 0.5) * scale) as f64;
        streaming.row(|rb| {
            for cell in &subtotal_owned {
                rb.cell(cell)
                    .bg_color(subtotal_bg)
                    .text_color(text_dark)
                    .font_size(subtotal_size);
            }
        })?;
    }
    drop(streaming);
    Ok(())
}

/// Render a standalone total row (for parent group totals)
fn render_total_row<W: Write>(
    pdf: &mut Pdf<W>,
    config: &ReportConfig,
    column_widths_pt: &[f32],
    total: &[String],
    level: usize,
    is_grand: bool,
    label_override: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let scale = config.scale;
    let row_size = ((config.font_size_base + if is_grand { 1.5 } else { 0.5 }) * scale) as f64;
    let widths = column_widths_pt.to_vec();

    let (bg, fg) = if is_grand {
        (palette_grand_total(), rgb(config.branding.text_light))
    } else if level == 0 {
        (palette_parent_total(), rgb(config.branding.text_dark))
    } else {
        (palette_leaf_subtotal(), rgb(config.branding.text_dark))
    };

    let mut row_data: Vec<String> = total.to_vec();
    if !row_data.is_empty() && !label_override.is_empty() {
        row_data[0] = label_override.to_string();
    }
    let cols_aligns: Vec<Align> = config.columns.iter().map(|c| c.align.to_mr_pdf()).collect();

    pdf.table(|t| {
        t.widths(widths.iter().map(|w| *w as f64).collect::<Vec<_>>());
        t.border(TableBorderStyle::None);
        for (i, a) in cols_aligns.iter().enumerate() {
            t.column_align(i, *a);
        }
        t.row_style()
            .font_size(row_size)
            .bg_color(bg)
            .text_color(fg);
        t.row(row_data.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    })?;
    Ok(())
}

/// Style per hierarchy level (legacy, kept for backwards compat)
fn label_style(config: &ReportConfig, depth: usize) -> (MrColor, MrColor, f32) {
    let primary = rgb(config.branding.primary);
    let accent = rgb(config.branding.accent);
    let primary_fg = rgb(config.branding.primary_fg);
    let accent_fg = rgb(config.branding.accent_fg);
    let text_dark = rgb(config.branding.text_dark);
    match depth {
        0 => (primary, primary_fg, 2.5),
        1 => (accent, accent_fg, 1.5),
        _ => (MrColor::Rgb(230, 230, 230), text_dark, 1.0),
    }
}

// ============================================
// Paleta de colores para reportes agrupados
// ============================================

/// Composite group header: slate-200 (claro, contraste por tipografía)
fn palette_group_header(brand: &Branding) -> MrColor {
    rgb(brand.accent)
}

/// Column headers: slate-100 (más claro que group bar para jerarquía visual)
fn palette_column_header(brand: &Branding) -> MrColor {
    rgb(brand.primary)
}

/// Leaf subtotal row: slate-100 neutro
fn palette_leaf_subtotal() -> MrColor {
    MrColor::Rgb(241, 245, 249) // slate-100
}

/// Parent total row: slate-300 (más fuerte que leaf subtotal)
fn palette_parent_total() -> MrColor {
    MrColor::Rgb(203, 213, 225) // slate-300
}

/// Grand total row: slate-700 + texto blanco — fuerte pero no agresivo
fn palette_grand_total() -> MrColor {
    MrColor::Rgb(51, 65, 85) // slate-700
}

// Helper: render page header — 3 zonas, tipografía jerárquica, líneas de acento
fn render_page_header<W: Write>(
    pdf: &mut Pdf<W>,
    config: &ReportConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let scale = config.scale;

    // Tipografía
    let company_size = (13.0 * scale) as f64;
    let title_size = (14.0 * scale) as f64;
    let breadcrumb_size = (8.0 * scale) as f64;
    let info_size = (8.0 * scale) as f64;
    let id_size = (7.5 * scale) as f64;

    let text_dark = rgb(config.branding.text_dark);
    let muted = rgb(config.branding.muted);
    let accent_line_color = rgb(config.branding.muted);

    // Capturar todos los strings ANTES (para closures)
    let company = config.company.clone();
    let company_addr = config.company_address.clone();
    let company_id = config.company_id.clone();
    let title = config.title.clone();
    let breadcrumb = config.breadcrumb.clone();
    let period = config.period.clone();
    let generated = config.generated_at.clone();
    let user = config.user_name.clone();

    // ───── ROW 1: Empresa | Título centrado | Breadcrumb derecha ─────
    pdf.table(|t| {
        t.widths(vec![32.0_f64.pct(), 36.0_f64.pct(), 32.0_f64.pct()]);
        t.column_align(0, Align::Left);
        t.column_align(1, Align::Center);
        t.column_align(2, Align::Right);
        t.column_valign(0, VAlign::Top);
        t.column_valign(1, VAlign::Center);
        t.column_valign(2, VAlign::Top);
        t.border(TableBorderStyle::None);

        // Companía bold + dirección debajo
        let mut col1 = String::new();
        col1.push_str(&company);
        if !company_addr.is_empty() {
            col1.push('\n');
            col1.push_str(&company_addr);
        }
        if !company_id.is_empty() {
            col1.push('\n');
            col1.push_str(&company_id);
        }

        // Title row con tamaños jerárquicos
        t.row_style().font_size(company_size);
        t.row_builder(|row| {
            // Col 1: empresa multi-line
            let mut c1: mr_pdf::layout::table::TableCell = col1.as_str().into();
            c1.text_color = Some(text_dark);
            c1.font_size = Some(company_size);
            row.cells.push(c1);
            // Col 2: título principal
            let mut c2: mr_pdf::layout::table::TableCell = title.as_str().into();
            c2.text_color = Some(text_dark);
            c2.font_size = Some(title_size);
            c2.align = Some(Align::Center);
            row.cells.push(c2);
            // Col 3: breadcrumb + período + generado + usuario
            let mut col3 = String::new();
            col3.push_str(&breadcrumb);
            if !period.is_empty() {
                col3.push('\n');
                col3.push_str("Período: ");
                col3.push_str(&period);
            }
            if !generated.is_empty() {
                col3.push('\n');
                col3.push_str("Generado: ");
                col3.push_str(&generated);
            }
            if !user.is_empty() {
                col3.push('\n');
                col3.push_str("Usuario: ");
                col3.push_str(&user);
            }
            let mut c3: mr_pdf::layout::table::TableCell = col3.as_str().into();
            c3.font_size = Some(breadcrumb_size);
            c3.text_color = Some(muted);
            c3.align = Some(Align::Right);
            row.cells.push(c3);
        });
    })?;

    // ───── Línea de acento horizontal ─────
    pdf.text("").size((1.0 * scale) as f64).margin_bottom(1.0);
    let accent_line = format!("{}", "_".repeat(180));
    pdf.text(&accent_line)
        .size((2.0 * scale) as f64)
        .color(accent_line_color)
        .align_left()
        .margin_bottom(3.0);

    // Suprimir warnings
    let _ = info_size;
    let _ = id_size;
    Ok(())
}

// ============================================================================
// Helpers
// ============================================================================

fn clone_config_for_header(c: &ReportConfig) -> ReportConfig {
    c.clone()
}

fn find_first_leaf_path(groups: &[HierarchicalGroup]) -> Vec<String> {
    let mut path = Vec::new();
    if let Some(first) = groups.first() {
        path.push(first.label.clone());
        find_leaf_recursive(first, &mut path);
    }
    path
}

fn find_leaf_recursive(g: &HierarchicalGroup, path: &mut Vec<String>) {
    if let Some(child) = g.sub_groups.first() {
        path.push(child.label.clone());
        find_leaf_recursive(child, path);
    }
}

fn rgb((r, g, b): (u8, u8, u8)) -> MrColor {
    MrColor::Rgb(r, g, b)
}

fn compute_usable_width_pt(config: &ReportConfig) -> f32 {
    let (w_pt, h_pt) = config.page_size.dimensions_pt();
    let page_w = match config.orientation {
        Orientation::Portrait => w_pt,
        Orientation::Landscape => h_pt,
    };
    page_w as f32 - (config.margins.left + config.margins.right) * MM_TO_PT
}

fn compute_column_widths(
    columns: &[Column],
    available_pt: f32,
    font_size_pt: f32,
    _scale: f32,
) -> Vec<f32> {
    // Las columnas siempre se ajustan al ancho de la página.
    // El "scale" se aplica solo a las fuentes → filas más cortas → más filas por página.
    let mut widths = vec![0.0_f32; columns.len()];
    let mut used = 0.0_f32;
    let mut total_flex = 0.0_f32;

    for (i, col) in columns.iter().enumerate() {
        match col.width {
            ColumnWidth::Fixed(pt) => {
                widths[i] = pt;
                used += pt;
            }
            ColumnWidth::Auto => {
                let approx = (col.name.chars().count() as f32 * font_size_pt * 0.6) + 14.0;
                widths[i] = approx;
                used += approx;
            }
            ColumnWidth::Flex(_) => {}
        }
    }

    for col in columns.iter() {
        if let ColumnWidth::Flex(factor) = col.width {
            total_flex += factor;
        }
    }

    let remaining = (available_pt - used).max(0.0);
    if total_flex > 0.0 {
        for (i, col) in columns.iter().enumerate() {
            if let ColumnWidth::Flex(factor) = col.width {
                widths[i] = remaining * (factor / total_flex);
            }
        }
    } else if used > 0.0 {
        // Scale all columns proportionally to fit
        let scale = available_pt / used;
        for w in widths.iter_mut() {
            *w *= scale;
        }
    }

    widths
}
