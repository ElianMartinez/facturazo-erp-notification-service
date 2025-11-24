//! Excel (XLSX) generator for complex reports
//!
//! Uses rust_xlsxwriter for professional Excel document generation

use anyhow::Result;
use rust_xlsxwriter::{Workbook, Worksheet, Format, FormatAlign, Color, FormatBorder};
use serde_json::Value;
use std::path::PathBuf;
use super::{DocumentGenerator, DocumentType};

/// Excel document generator
pub struct ExcelGenerator {
    work_dir: PathBuf,
}

impl ExcelGenerator {
    pub fn new(work_dir: PathBuf) -> Self {
        Self { work_dir }
    }

    /// Generate Excel report from data
    pub async fn generate_excel(&self, data: &Value) -> Result<Vec<u8>> {
        // Create workbook
        let mut workbook = Workbook::new();

        // Add worksheets based on data structure
        if let Some(sheets) = data.get("sheets").and_then(|s| s.as_array()) {
            for sheet_data in sheets {
                self.create_worksheet(&mut workbook, sheet_data)?;
            }
        } else {
            // Single sheet mode
            self.create_worksheet(&mut workbook, data)?;
        }

        // Save to bytes
        let bytes = workbook.save_to_buffer()?;
        Ok(bytes)
    }

    /// Create a worksheet from data
    fn create_worksheet(&self, workbook: &mut Workbook, data: &Value) -> Result<()> {
        let sheet_name = data.get("sheet_name")
            .and_then(|n| n.as_str())
            .unwrap_or("Sheet1");

        let worksheet = workbook.add_worksheet();
        worksheet.set_name(sheet_name)?;

        // Add title if present
        if let Some(title) = data.get("title").and_then(|t| t.as_str()) {
            self.add_title(worksheet, title)?;
        }

        // Add headers and data
        if let Some(table) = data.get("table") {
            self.add_table(worksheet, table)?;
        }

        // Add summary if present
        if let Some(summary) = data.get("summary") {
            self.add_summary(worksheet, summary)?;
        }

        Ok(())
    }

    /// Add title to worksheet
    fn add_title(&self, worksheet: &mut Worksheet, title: &str) -> Result<()> {
        let title_format = Format::new()
            .set_font_size(16)
            .set_bold()
            .set_align(FormatAlign::Center);

        worksheet.merge_range(0, 0, 0, 10, title, &title_format)?;
        Ok(())
    }

    /// Add table with headers and data
    fn add_table(&self, worksheet: &mut Worksheet, table_data: &Value) -> Result<()> {
        let mut row = 2; // Start after title

        // Format for headers
        let header_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0x004080))
            .set_font_color(Color::White)
            .set_align(FormatAlign::Center)
            .set_border(FormatBorder::Thin);

        // Format for data
        let data_format = Format::new()
            .set_border(FormatBorder::Thin);

        let currency_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_num_format("RD$ #,##0.00");

        // Add headers
        if let Some(headers) = table_data.get("headers").and_then(|h| h.as_array()) {
            for (col, header) in headers.iter().enumerate() {
                if let Some(header_text) = header.as_str() {
                    worksheet.write_string_with_format(row, col as u16, header_text, &header_format)?;
                }
            }
            row += 1;
        }

        // Add data rows
        if let Some(rows) = table_data.get("rows").and_then(|r| r.as_array()) {
            for data_row in rows {
                if let Some(cells) = data_row.as_array() {
                    for (col, cell) in cells.iter().enumerate() {
                        match cell {
                            Value::String(s) => {
                                worksheet.write_string_with_format(row, col as u16, s, &data_format)?;
                            }
                            Value::Number(n) => {
                                // Check if it's a currency column
                                let is_currency = table_data.get("currency_columns")
                                    .and_then(|c| c.as_array())
                                    .map(|cols| cols.iter().any(|c| c.as_u64() == Some(col as u64)))
                                    .unwrap_or(false);

                                if is_currency {
                                    worksheet.write_number_with_format(row, col as u16, n.as_f64().unwrap_or(0.0), &currency_format)?;
                                } else {
                                    worksheet.write_number_with_format(row, col as u16, n.as_f64().unwrap_or(0.0), &data_format)?;
                                }
                            }
                            Value::Bool(b) => {
                                worksheet.write_string_with_format(row, col as u16, if *b { "Sí" } else { "No" }, &data_format)?;
                            }
                            _ => {
                                worksheet.write_string(row, col as u16, "")?;
                            }
                        }
                    }
                    row += 1;
                }
            }
        }

        // Add totals row if present
        if let Some(totals) = table_data.get("totals").and_then(|t| t.as_array()) {
            let total_format = Format::new()
                .set_bold()
                .set_background_color(Color::RGB(0xF0F0F0))
                .set_border(FormatBorder::Thin);

            for (col, total) in totals.iter().enumerate() {
                match total {
                    Value::String(s) => {
                        worksheet.write_string_with_format(row, col as u16, s, &total_format)?;
                    }
                    Value::Number(n) => {
                        worksheet.write_number_with_format(row, col as u16, n.as_f64().unwrap_or(0.0), &total_format)?;
                    }
                    _ => {}
                }
            }
        }

        // Auto-fit columns
        for col in 0..20 {
            worksheet.set_column_width(col, 15)?;
        }

        Ok(())
    }

    /// Add summary section
    fn add_summary(&self, worksheet: &mut Worksheet, summary: &Value) -> Result<()> {
        let start_row = 30; // Start summary after table

        let summary_format = Format::new()
            .set_background_color(Color::RGB(0xFFFDD0))
            .set_border(FormatBorder::Thin);

        worksheet.write_string_with_format(start_row, 0, "RESUMEN", &Format::new().set_bold())?;

        if let Some(items) = summary.as_array() {
            for (i, item) in items.iter().enumerate() {
                if let Some(obj) = item.as_object() {
                    let label = obj.get("label").and_then(|l| l.as_str()).unwrap_or("");
                    let value = obj.get("value").and_then(|v| v.as_str()).unwrap_or("");

                    worksheet.write_string_with_format(start_row + i as u32 + 1, 0, label, &summary_format)?;
                    worksheet.write_string_with_format(start_row + i as u32 + 1, 1, value, &summary_format)?;
                }
            }
        }

        Ok(())
    }

    /// Generate invoice Excel
    pub async fn generate_invoice_excel(&self, invoice_data: &Value) -> Result<Vec<u8>> {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet.set_name("Factura")?;

        // Company header
        self.add_invoice_header(worksheet, invoice_data)?;

        // Customer info
        self.add_customer_info(worksheet, invoice_data)?;

        // Items table
        self.add_invoice_items(worksheet, invoice_data)?;

        // Totals
        self.add_invoice_totals(worksheet, invoice_data)?;

        Ok(workbook.save_to_buffer()?)
    }

    fn add_invoice_header(&self, worksheet: &mut Worksheet, data: &Value) -> Result<()> {
        let header_format = Format::new()
            .set_font_size(14)
            .set_bold();
        let bold_format = Format::new().set_bold();

        worksheet.write_string_with_format(0, 0, "FACTURA FISCAL", &header_format)?;

        if let Some(seller) = data.get("seller") {
            worksheet.write_string_with_format(2, 0, "Vendedor:", &bold_format)?;
            worksheet.write_string(2, 1, seller.get("company_name").and_then(|n| n.as_str()).unwrap_or(""))?;

            if let Some(rnc) = seller.get("rnc").and_then(|r| r.get("formatted")).and_then(|f| f.as_str()) {
                worksheet.write_string_with_format(3, 0, "RNC:", &bold_format)?;
                worksheet.write_string(3, 1, rnc)?;
            }
        }

        if let Some(ncf) = data.get("ncf").and_then(|n| n.get("formatted")).and_then(|f| f.as_str()) {
            worksheet.write_string_with_format(5, 0, "NCF:", &bold_format)?;
            worksheet.write_string(5, 1, ncf)?;
        }

        Ok(())
    }

    fn add_customer_info(&self, worksheet: &mut Worksheet, data: &Value) -> Result<()> {
        let bold_format = Format::new().set_bold();

        if let Some(customer) = data.get("customer") {
            worksheet.write_string_with_format(7, 0, "Cliente:", &bold_format)?;
            worksheet.write_string(7, 1, customer.get("name").and_then(|n| n.as_str()).unwrap_or(""))?;

            if let Some(tax_id) = customer.get("tax_id").and_then(|t| t.get("formatted")).and_then(|f| f.as_str()) {
                worksheet.write_string_with_format(8, 0, "RNC/Cédula:", &bold_format)?;
                worksheet.write_string(8, 1, tax_id)?;
            }
        }

        Ok(())
    }

    fn add_invoice_items(&self, worksheet: &mut Worksheet, data: &Value) -> Result<()> {
        let header_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0x004080))
            .set_font_color(Color::White);

        let start_row = 11;

        // Headers
        worksheet.write_string_with_format(start_row, 0, "Cant.", &header_format)?;
        worksheet.write_string_with_format(start_row, 1, "Descripción", &header_format)?;
        worksheet.write_string_with_format(start_row, 2, "Precio", &header_format)?;
        worksheet.write_string_with_format(start_row, 3, "ITBIS", &header_format)?;
        worksheet.write_string_with_format(start_row, 4, "Total", &header_format)?;

        // Items
        if let Some(items) = data.get("items").and_then(|i| i.as_array()) {
            for (i, item) in items.iter().enumerate() {
                let row = start_row + 1 + i as u32;

                worksheet.write_number(row, 0, item.get("quantity").and_then(|q| q.as_f64()).unwrap_or(0.0))?;
                worksheet.write_string(row, 1, item.get("description").and_then(|d| d.as_str()).unwrap_or(""))?;
                worksheet.write_number(row, 2, item.get("unit_price").and_then(|p| p.as_f64()).unwrap_or(0.0))?;
                worksheet.write_number(row, 3, item.get("itbis_amount").and_then(|i| i.as_f64()).unwrap_or(0.0))?;
                worksheet.write_number(row, 4, item.get("total").and_then(|t| t.as_f64()).unwrap_or(0.0))?;
            }
        }

        Ok(())
    }

    fn add_invoice_totals(&self, worksheet: &mut Worksheet, data: &Value) -> Result<()> {
        let start_row = 25;
        let bold = Format::new().set_bold();

        worksheet.write_string_with_format(start_row, 3, "Subtotal:", &bold)?;
        worksheet.write_number(start_row, 4, data.get("subtotal").and_then(|s| s.as_f64()).unwrap_or(0.0))?;

        worksheet.write_string_with_format(start_row + 1, 3, "ITBIS:", &bold)?;
        worksheet.write_number(start_row + 1, 4, data.get("itbis_amount").and_then(|i| i.as_f64()).unwrap_or(0.0))?;

        worksheet.write_string_with_format(start_row + 2, 3, "TOTAL:", &bold)?;
        worksheet.write_number_with_format(start_row + 2, 4, data.get("total_amount").and_then(|t| t.as_f64()).unwrap_or(0.0), &bold)?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl DocumentGenerator for ExcelGenerator {
    async fn generate(&self, _template: &str, data: &Value) -> Result<Vec<u8>> {
        // Determine document type and generate accordingly
        if data.get("document_type").and_then(|t| t.as_str()) == Some("invoice") {
            self.generate_invoice_excel(data).await
        } else {
            self.generate_excel(data).await
        }
    }

    fn supported_types(&self) -> Vec<DocumentType> {
        vec![DocumentType::Report, DocumentType::Invoice]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_excel_generation() {
        let temp_dir = TempDir::new().unwrap();
        let generator = ExcelGenerator::new(temp_dir.path().to_path_buf());

        let data = serde_json::json!({
            "title": "Sales Report",
            "table": {
                "headers": ["Product", "Quantity", "Price", "Total"],
                "rows": [
                    ["Product A", 10, 100.0, 1000.0],
                    ["Product B", 5, 200.0, 1000.0]
                ],
                "totals": ["TOTAL", 15, "", 2000.0],
                "currency_columns": [2, 3]
            }
        });

        let result = generator.generate_excel(&data).await;
        assert!(result.is_ok());

        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
    }
}