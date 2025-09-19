use crate::core::{TableData, Money, DocumentResult, DocumentError};
use crate::excel::ExcelGenerator;
use rust_xlsxwriter::Format;

pub struct ExcelBuilder {
    generator: ExcelGenerator,
    current_row: u32,
    current_col: u16,
}

impl ExcelBuilder {
    pub fn new() -> Self {
        let mut builder = ExcelBuilder {
            generator: ExcelGenerator::new(),
            current_row: 0,
            current_col: 0,
        };
        builder.generator.add_worksheet("Hoja1").unwrap();
        builder
    }

    pub fn new_sheet(&mut self, name: &str) -> DocumentResult<&mut Self> {
        self.generator.add_worksheet(name)?;
        self.current_row = 0;
        self.current_col = 0;
        Ok(self)
    }

    pub fn add_title(&mut self, title: &str, merge_cells: Option<u16>) -> DocumentResult<&mut Self> {
        let format = Format::new()
            .set_bold()
            .set_font_size(14)
            .set_align(rust_xlsxwriter::FormatAlign::Center);

        if let Some(cols) = merge_cells {
            let sheet = self.generator.get_current_sheet_mut()?;
            sheet.merge_range(
                self.current_row,
                self.current_col,
                self.current_row,
                self.current_col + cols - 1,
                title,
                &format
            ).map_err(|e| DocumentError::GenerationError(e.to_string()))?;
        } else {
            self.generator.write_with_format(self.current_row, self.current_col, title, &format)?;
        }

        self.current_row += 2;
        Ok(self)
    }

    pub fn add_table(&mut self, table: &TableData) -> DocumentResult<&mut Self> {
        let header_format = self.generator.create_header_format();

        for (col, header) in table.headers.iter().enumerate() {
            self.generator.write_with_format(
                self.current_row,
                col as u16,
                header,
                &header_format
            )?;
        }

        self.current_row += 1;

        let cell_format = Format::new()
            .set_border(rust_xlsxwriter::FormatBorder::Thin);

        for row in &table.rows {
            for (col, cell) in row.iter().enumerate() {
                if let Ok(number) = cell.parse::<f64>() {
                    self.generator.write_number_with_format(
                        self.current_row,
                        col as u16,
                        number,
                        &cell_format
                    )?;
                } else {
                    self.generator.write_with_format(
                        self.current_row,
                        col as u16,
                        cell,
                        &cell_format
                    )?;
                }
            }
            self.current_row += 1;
        }

        self.generator.autofilter(
            self.current_row - table.rows.len() as u32 - 1,
            0,
            self.current_row - 1,
            table.headers.len() as u16 - 1
        )?;

        self.current_row += 1;
        Ok(self)
    }

    pub fn add_summary_row(&mut self, label: &str, values: Vec<f64>) -> DocumentResult<&mut Self> {
        let format = Format::new()
            .set_bold()
            .set_background_color(rust_xlsxwriter::Color::RGB(0xF0F0F0));

        self.generator.write_with_format(self.current_row, 0, label, &format)?;

        for (i, value) in values.iter().enumerate() {
            self.generator.write_number_with_format(
                self.current_row,
                (i + 1) as u16,
                *value,
                &format
            )?;
        }

        self.current_row += 1;
        Ok(self)
    }

    pub fn add_money_column(&mut self, col: u16, start_row: u32, values: Vec<Money>) -> DocumentResult<&mut Self> {
        let money_format = self.generator.create_money_format();

        for (i, money) in values.iter().enumerate() {
            self.generator.write_number_with_format(
                start_row + i as u32,
                col,
                money.amount,
                &money_format
            )?;
        }

        Ok(self)
    }

    pub fn add_formula_row(&mut self, label: &str, col_start: u16, formula: &str) -> DocumentResult<&mut Self> {
        self.generator.write_string(self.current_row, 0, label)?;
        self.generator.write_formula(self.current_row, col_start, formula)?;
        self.current_row += 1;
        Ok(self)
    }

    pub fn set_column_widths(&mut self, widths: Vec<(u16, f64)>) -> DocumentResult<&mut Self> {
        for (col, width) in widths {
            self.generator.set_column_width(col, width)?;
        }
        Ok(self)
    }

    pub fn add_chart_space(&mut self, rows: u32) -> DocumentResult<&mut Self> {
        self.current_row += rows;
        Ok(self)
    }

    pub fn freeze_top_row(&mut self) -> DocumentResult<&mut Self> {
        self.generator.freeze_panes(1, 0)?;
        Ok(self)
    }

    pub fn move_to(&mut self, row: u32, col: u16) -> &mut Self {
        self.current_row = row;
        self.current_col = col;
        self
    }

    pub fn skip_rows(&mut self, rows: u32) -> &mut Self {
        self.current_row += rows;
        self
    }

    pub fn save(mut self, path: &str) -> DocumentResult<()> {
        self.generator.save(path)
    }
}