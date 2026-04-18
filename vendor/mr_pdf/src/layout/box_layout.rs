use crate::{Align, Color, Pdf, Size};
use std::io::Write;

/// A builder for creating container boxes with styling.
pub struct BoxBuilder<'a, W: Write> {
    pdf: &'a mut Pdf<W>,
    padding_x: f64,
    padding_y: f64,
    width: Option<Size>,
    align: Align,
    bg_color: Option<Color>,
    border_color: Option<Color>,
    border_width: f64,
    border_radius: f64,
}

impl<'a, W: Write> BoxBuilder<'a, W> {
    pub fn new(pdf: &'a mut Pdf<W>) -> Self {
        Self {
            pdf,
            padding_x: 0.0,
            padding_y: 0.0,
            width: None,
            align: Align::Left,
            bg_color: None,
            border_color: None,
            border_width: 0.0,
            border_radius: 0.0,
        }
    }

    /// Sets the width of the box. Can be Points, Percent, or Flex.
    pub fn width(&mut self, w: impl Into<Size>) -> &mut Self {
        self.width = Some(w.into());
        self
    }

    /// Sets the alignment of the box when it is narrower than the available width.
    pub fn align(&mut self, a: Align) -> &mut Self {
        self.align = a;
        self
    }

    /// Sets both horizontal and vertical padding (for convenience).
    pub fn padding(&mut self, p: f64) -> &mut Self {
        self.padding_x = p;
        self.padding_y = p;
        self
    }

    /// Sets only horizontal padding.
    pub fn padding_x(&mut self, p: f64) -> &mut Self {
        self.padding_x = p;
        self
    }

    /// Sets only vertical padding.
    pub fn padding_y(&mut self, p: f64) -> &mut Self {
        self.padding_y = p;
        self
    }

    /// Sets both horizontal and vertical padding.
    pub fn padding_xy(&mut self, px: f64, py: f64) -> &mut Self {
        self.padding_x = px;
        self.padding_y = py;
        self
    }

    /// Sets the background color of the box.
    pub fn bg_color(&mut self, color: Color) -> &mut Self {
        self.bg_color = Some(color);
        self
    }

    /// Sets the border styling.
    pub fn border(&mut self, width: f64, color: Color) -> &mut Self {
        self.border_width = width;
        self.border_color = Some(color);
        self
    }

    /// Sets the border radius for rounded corners.
    pub fn border_radius(&mut self, r: f64) -> &mut Self {
        self.border_radius = r;
        self
    }

    /// Renders the box with provided content closure.
    pub fn content<F>(&mut self, f: F) -> std::io::Result<()>
    where
        F: FnOnce(&mut Pdf<W>) -> std::io::Result<()>,
    {
        let start_y = self.pdf.cursor_pos().1;
        let start_x = self.pdf.cursor_pos().0;
        let available_full_w = self.pdf.content_width();

        // Resolve box width
        let resolved_box_w = match self.width {
            Some(w) => self.pdf.resolve_size(w, available_full_w),
            None => available_full_w,
        }
        .min(available_full_w);

        // Resolve box X offset based on alignment
        let box_x_offset = match self.align {
            Align::Left => 0.0,
            Align::Center => (available_full_w - resolved_box_w) / 2.0,
            Align::Right => available_full_w - resolved_box_w,
        };

        let box_start_x = start_x + box_x_offset;

        // Prepare for content rendering
        let start_stream_pos = self.pdf.get_stream().len();

        // Adjust layout for padding
        let original_width = self.pdf.current_layout_width;
        let original_margin = self.pdf.current_layout_margin;

        let inner_width = resolved_box_w - (2.0 * self.padding_x);
        let inner_margin = self.pdf.margin_pub() + box_x_offset + self.padding_x;

        self.pdf.current_layout_width = Some(inner_width);
        self.pdf.current_layout_margin = Some(inner_margin);

        // Move cursor to inner padded position
        self.pdf.set_cursor(inner_margin, start_y);

        // Move cursor down by top padding (y)
        self.pdf.advance_cursor(self.padding_y);

        // Render content
        f(self.pdf)?;

        // Move cursor down by bottom padding (y)
        self.pdf.advance_cursor(self.padding_y);

        let end_y = self.pdf.cursor_pos().1;
        let box_height = start_y - end_y;

        // Extract content stream to insert background/border behind it
        let content = self.pdf.get_stream().split_off(start_stream_pos);

        // Restore layout
        self.pdf.current_layout_width = original_width;
        self.pdf.current_layout_margin = original_margin;

        // Save state before drawing background/border
        self.pdf.get_stream().push_str("q\n");

        // Draw background and border
        if let Some(bg) = self.bg_color {
            self.pdf.set_fill_color(bg)?;
            if self.border_radius > 0.0 {
                self.pdf.draw_rounded_rect_op(
                    box_start_x,
                    end_y,
                    resolved_box_w,
                    box_height,
                    self.border_radius,
                    "f",
                )?;
            } else {
                self.pdf
                    .fill_rect(box_start_x, end_y, resolved_box_w, box_height)?;
            }
        }

        if let (Some(bc), bw) = (self.border_color, self.border_width) {
            if bw > 0.0 {
                self.pdf.set_stroke_color(bc)?;
                self.pdf.set_line_width(bw)?;
                if self.border_radius > 0.0 {
                    self.pdf.draw_rounded_rect_op(
                        box_start_x,
                        end_y,
                        resolved_box_w,
                        box_height,
                        self.border_radius,
                        "S",
                    )?;
                } else {
                    self.pdf
                        .rect(box_start_x, end_y, resolved_box_w, box_height)?;
                }
            }
        }

        // Restore state
        self.pdf.get_stream().push_str("Q\n");

        // Re-append content stream
        self.pdf.get_stream().push_str(&content);

        // Ensure cursor is at the bottom of the box for next elements and at the original margin
        self.pdf.set_cursor(start_x, end_y);

        Ok(())
    }
}
