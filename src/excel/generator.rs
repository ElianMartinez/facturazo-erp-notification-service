use crate::core::{DocumentError, DocumentResult};
use rust_xlsxwriter::{Workbook, Worksheet, Format, FormatBorder};
use std::path::Path;

pub struct ExcelGenerator {
    workbook: Workbook,
    current_sheet: Option<Worksheet>,
}

impl ExcelGenerator {
    pub fn new() -> Self {
        ExcelGenerator {
            workbook: Workbook::new(),
            current_sheet: None,
        }
    }

    pub fn add_worksheet(&mut self, name: &str) -> DocumentResult<()> {
        let mut worksheet = Worksheet::new();
        worksheet.set_name(name)
            .map_err(|e| DocumentError::GenerationError(e.to_string()))?;

        self.current_sheet = Some(worksheet);
        Ok(())
    }

    pub fn get_current_sheet_mut(&mut self) -> DocumentResult<&mut Worksheet> {
        self.current_sheet.as_mut()
            .ok_or_else(|| DocumentError::ValidationError(
                "No hay hoja de trabajo activa".to_string()
            ))
    }

    pub fn write_string(&mut self, row: u32, col: u16, text: &str) -> DocumentResult<()> {
        let sheet = self.get_current_sheet_mut()?;
        sheet.write_string(row, col, text)
            .map_err(|e| DocumentError::GenerationError(e.to_string()))?;
        Ok(())
    }

    pub fn write_number(&mut self, row: u32, col: u16, number: f64) -> DocumentResult<()> {
        let sheet = self.get_current_sheet_mut()?;
        sheet.write_number(row, col, number)
            .map_err(|e| DocumentError::GenerationError(e.to_string()))?;
        Ok(())
    }

    pub fn write_formula(&mut self, row: u32, col: u16, formula: &str) -> DocumentResult<()> {
        let sheet = self.get_current_sheet_mut()?;
        sheet.write_formula(row, col, formula)
            .map_err(|e| DocumentError::GenerationError(e.to_string()))?;
        Ok(())
    }

    pub fn set_column_width(&mut self, first_col: u16, width: f64) -> DocumentResult<()> {
        let sheet = self.get_current_sheet_mut()?;
        sheet.set_column_width(first_col, width)
            .map_err(|e| DocumentError::GenerationError(e.to_string()))?;
        Ok(())
    }

    pub fn create_header_format(&mut self) -> Format {
        Format::new()
            .set_bold()
            .set_background_color(rust_xlsxwriter::Color::RGB(0xE0E0E0))
            .set_border(FormatBorder::Thin)
    }

    pub fn create_money_format(&mut self) -> Format {
        Format::new()
            .set_num_format("$#,##0.00")
            .set_border(FormatBorder::Thin)
    }

    pub fn create_percentage_format(&mut self) -> Format {
        Format::new()
            .set_num_format("0.00%")
            .set_border(FormatBorder::Thin)
    }

    pub fn create_date_format(&mut self) -> Format {
        Format::new()
            .set_num_format("dd/mm/yyyy")
            .set_border(FormatBorder::Thin)
    }

    pub fn write_with_format(&mut self, row: u32, col: u16, text: &str, format: &Format) -> DocumentResult<()> {
        let sheet = self.get_current_sheet_mut()?;
        sheet.write_string_with_format(row, col, text, format)
            .map_err(|e| DocumentError::GenerationError(e.to_string()))?;
        Ok(())
    }

    pub fn write_number_with_format(&mut self, row: u32, col: u16, number: f64, format: &Format) -> DocumentResult<()> {
        let sheet = self.get_current_sheet_mut()?;
        sheet.write_number_with_format(row, col, number, format)
            .map_err(|e| DocumentError::GenerationError(e.to_string()))?;
        Ok(())
    }

    pub fn autofilter(&mut self, first_row: u32, first_col: u16, last_row: u32, last_col: u16) -> DocumentResult<()> {
        let sheet = self.get_current_sheet_mut()?;
        sheet.autofilter(first_row, first_col, last_row, last_col)
            .map_err(|e| DocumentError::GenerationError(e.to_string()))?;
        Ok(())
    }

    pub fn freeze_panes(&mut self, row: u32, col: u16) -> DocumentResult<()> {
        let sheet = self.get_current_sheet_mut()?;
        sheet.set_freeze_panes(row, col)
            .map_err(|e| DocumentError::GenerationError(e.to_string()))?;
        Ok(())
    }

    pub fn save<P: AsRef<Path>>(&mut self, path: P) -> DocumentResult<()> {
        if let Some(sheet) = self.current_sheet.take() {
            self.workbook.push_worksheet(sheet);
        }

        self.workbook.save(path)
            .map_err(|e| DocumentError::GenerationError(
                format!("Error guardando el archivo Excel: {}", e)
            ))?;
        Ok(())
    }
}