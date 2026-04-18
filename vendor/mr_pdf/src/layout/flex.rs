use crate::Pdf;
use crate::Size;
use std::io::Write;

pub struct RowItem<'a, W: Write> {
    pub width: Size,
    pub builder: Box<dyn FnOnce(&mut Pdf<W>) -> std::io::Result<()> + 'a>,
}

/// A builder to define columns within a flexible row.
pub struct RowBuilder<'a, W: Write> {
    pub items: Vec<RowItem<'a, W>>,
}

impl<'a, W: Write> Default for RowBuilder<'a, W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, W: Write> RowBuilder<'a, W> {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Adds a column with a specific width (Points, Percent, or Flex).
    pub fn col<F>(&mut self, width: impl Into<Size>, f: F)
    where
        F: FnOnce(&mut Pdf<W>) -> std::io::Result<()> + 'a,
    {
        self.items.push(RowItem {
            width: width.into(),
            builder: Box::new(f),
        });
    }

    /// Adds a flex-expanded column with a specific weight.
    pub fn expanded<F>(&mut self, flex_val: u32, f: F)
    where
        F: FnOnce(&mut Pdf<W>) -> std::io::Result<()> + 'a,
    {
        self.items.push(RowItem {
            width: Size::Flex(flex_val),
            builder: Box::new(f),
        });
    }
}

pub fn render_row<W: Write>(
    pdf: &mut Pdf<W>,
    f: impl FnOnce(&mut RowBuilder<W>),
) -> std::io::Result<()> {
    let mut builder = RowBuilder::new();
    f(&mut builder);

    let items = builder.items;
    if items.is_empty() {
        return Ok(());
    }

    let total_width = pdf.content_width();
    let start_x = pdf.margin_pub();
    let (_, start_y) = pdf.cursor_pos();

    let mut fixed_sum = 0.0;
    let mut flex_sum = 0;
    for it in &items {
        match it.width {
            Size::Points(p) => fixed_sum += p,
            Size::Percent(p) => fixed_sum += (p / 100.0) * total_width,
            Size::Flex(f) => flex_sum += f,
        }
    }

    let remaining = (total_width - fixed_sum).max(0.0);
    let flex_unit = if flex_sum > 0 {
        remaining / flex_sum as f64
    } else {
        0.0
    };

    let mut current_column_x = start_x;
    let mut max_row_height = 0.0;

    for it in items {
        let col_w = match it.width {
            Size::Points(p) => p,
            Size::Percent(p) => (p / 100.0) * total_width,
            Size::Flex(f) => f as f64 * flex_unit,
        };

        let original_l_w = pdf.current_layout_width;
        let original_l_m = pdf.current_layout_margin;

        pdf.current_layout_width = Some(col_w);
        pdf.current_layout_margin = Some(current_column_x);

        pdf.set_cursor(current_column_x, start_y);

        (it.builder)(pdf)?;

        let h = start_y - pdf.cursor_pos().1;
        if h > max_row_height {
            max_row_height = h;
        }

        pdf.current_layout_width = original_l_w;
        pdf.current_layout_margin = original_l_m;

        current_column_x += col_w;
    }

    pdf.set_cursor(start_x, start_y - max_row_height);

    Ok(())
}
