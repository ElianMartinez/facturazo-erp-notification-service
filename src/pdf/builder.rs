use crate::core::{TableData, ColumnAlign, PdfConfig};

pub struct PdfBuilder {
    sections: Vec<String>,
    config: PdfConfig,
}

impl PdfBuilder {
    pub fn new() -> Self {
        PdfBuilder {
            sections: Vec::new(),
            config: PdfConfig::default(),
        }
    }

    pub fn with_config(mut self, config: PdfConfig) -> Self {
        self.config = config;
        self
    }

    pub fn add_title(&mut self, title: &str, level: u8) -> &mut Self {
        let marker = "=".repeat(level as usize);
        self.sections.push(format!("{} {}", marker, title));
        self
    }

    pub fn add_paragraph(&mut self, text: &str) -> &mut Self {
        self.sections.push(text.to_string());
        self.sections.push(String::new());
        self
    }

    pub fn add_line_break(&mut self) -> &mut Self {
        self.sections.push("#v(10pt)".to_string());
        self
    }

    pub fn add_horizontal_line(&mut self) -> &mut Self {
        self.sections.push("#line(length: 100%, stroke: 0.5pt)".to_string());
        self
    }

    pub fn add_table(&mut self, table: &TableData) -> &mut Self {
        let mut typst = String::from("#table(\n");

        if let Some(widths) = &table.column_widths {
            let width_str: Vec<String> = widths.iter()
                .map(|w| format!("{}pt", w))
                .collect();
            typst.push_str(&format!("  columns: ({}),\n", width_str.join(", ")));
        } else {
            typst.push_str(&format!("  columns: {},\n", table.headers.len()));
        }

        if let Some(alignment) = &table.alignment {
            let align_str: Vec<&str> = alignment.iter()
                .map(|a| match a {
                    ColumnAlign::Left => "left",
                    ColumnAlign::Center => "center",
                    ColumnAlign::Right => "right",
                })
                .collect();

            typst.push_str("  align: (col, row) => {\n");
            for (i, align) in align_str.iter().enumerate() {
                if i == 0 {
                    typst.push_str(&format!("    if col == {} {{ {} }}\n", i, align));
                } else {
                    typst.push_str(&format!("    else if col == {} {{ {} }}\n", i, align));
                }
            }
            typst.push_str("    else { left }\n  },\n");
        }

        typst.push_str("  stroke: 0.5pt,\n");
        typst.push_str("  fill: (x, y) => if y == 0 { rgb(240, 240, 240) } else { white },\n");
        typst.push_str("  inset: 8pt,\n\n");

        for header in &table.headers {
            typst.push_str(&format!("  [#text(weight: \"bold\")[{}]],\n", header));
        }

        for row in &table.rows {
            for cell in row {
                typst.push_str(&format!("  [{}],\n", cell));
            }
        }

        typst.push_str(")");
        self.sections.push(typst);
        self
    }

    pub fn add_list(&mut self, items: Vec<String>, ordered: bool) -> &mut Self {
        let bullet = if ordered { "+" } else { "-" };
        for (i, item) in items.iter().enumerate() {
            if ordered {
                self.sections.push(format!("{}. {}", i + 1, item));
            } else {
                self.sections.push(format!("{} {}", bullet, item));
            }
        }
        self.sections.push(String::new());
        self
    }

    pub fn add_image(&mut self, path: &str, width: Option<f32>, caption: Option<&str>) -> &mut Self {
        let mut img_str = format!("#image(\"{}\"", path);

        if let Some(w) = width {
            img_str.push_str(&format!(", width: {}pt", w));
        }

        img_str.push(')');

        if let Some(cap) = caption {
            img_str = format!("#figure(\n  {},\n  caption: [{}]\n)", img_str, cap);
        }

        self.sections.push(img_str);
        self
    }

    pub fn add_page_break(&mut self) -> &mut Self {
        self.sections.push("#pagebreak()".to_string());
        self
    }

    pub fn add_raw_typst(&mut self, typst_code: &str) -> &mut Self {
        self.sections.push(typst_code.to_string());
        self
    }

    pub fn add_grid(&mut self, columns: usize, cells: Vec<String>) -> &mut Self {
        let mut grid = format!("#grid(\n  columns: {},\n", columns);
        grid.push_str("  gutter: 10pt,\n");

        for cell in cells {
            grid.push_str(&format!("  [{}],\n", cell));
        }

        grid.push_str(")");
        self.sections.push(grid);
        self
    }

    pub fn add_box(&mut self, content: &str, fill_color: Option<&str>, border: bool) -> &mut Self {
        let mut box_str = String::from("#rect(");

        if let Some(color) = fill_color {
            box_str.push_str(&format!("fill: rgb({}), ", color));
        }

        if border {
            box_str.push_str("stroke: 0.5pt, ");
        }

        box_str.push_str("radius: 3pt, inset: 10pt)[");
        box_str.push_str(content);
        box_str.push_str("]");

        self.sections.push(box_str);
        self
    }

    pub fn build(&self) -> String {
        self.sections.join("\n\n")
    }
}