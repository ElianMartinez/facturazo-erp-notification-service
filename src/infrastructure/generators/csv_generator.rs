//! CSV generator for bulk data exports
//!
//! Efficient CSV generation for large datasets

use super::{DocumentGenerator, DocumentType};
use anyhow::Result;
use csv::Writer;
use serde_json::Value;

/// CSV document generator
pub struct CsvGenerator;

impl CsvGenerator {
    pub fn new() -> Self {
        Self
    }

    /// Generate CSV from JSON data
    pub async fn generate_csv(&self, data: &Value) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        {
            let mut writer = Writer::from_writer(&mut buffer);

            // Write headers
            if let Some(headers) = data.get("headers").and_then(|h| h.as_array()) {
                let header_row: Vec<String> = headers
                    .iter()
                    .map(|h| h.as_str().unwrap_or("").to_string())
                    .collect();
                writer.write_record(&header_row)?;
            }

            // Write data rows
            if let Some(rows) = data.get("rows").and_then(|r| r.as_array()) {
                for row in rows {
                    let record = self.format_row(row)?;
                    writer.write_record(&record)?;
                }
            }

            writer.flush()?;
        }

        Ok(buffer)
    }

    /// Format a row for CSV output
    fn format_row(&self, row: &Value) -> Result<Vec<String>> {
        let mut record = Vec::new();

        match row {
            Value::Array(cells) => {
                for cell in cells {
                    record.push(self.format_cell(cell));
                }
            }
            Value::Object(obj) => {
                // If row is an object, extract values in order
                for (_key, value) in obj {
                    record.push(self.format_cell(value));
                }
            }
            _ => {
                record.push(self.format_cell(row));
            }
        }

        Ok(record)
    }

    /// Format a single cell value
    fn format_cell(&self, value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    i.to_string()
                } else if let Some(f) = n.as_f64() {
                    format!("{:.2}", f)
                } else {
                    n.to_string()
                }
            }
            Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Value::Null => String::new(),
            Value::Array(arr) => {
                // Join array elements with semicolon
                arr.iter()
                    .map(|v| self.format_cell(v))
                    .collect::<Vec<_>>()
                    .join(";")
            }
            Value::Object(_) => {
                // Convert object to JSON string
                serde_json::to_string(value).unwrap_or_default()
            }
        }
    }

    /// Generate invoice CSV
    pub async fn generate_invoice_csv(&self, invoice_data: &Value) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        {
            let mut writer = Writer::from_writer(&mut buffer);

            // Write metadata (5 columns to match items table)
            writer.write_record(&["FACTURA FISCAL", "", "", "", ""])?;
            writer.write_record(&["", "", "", "", ""])?;

            // Seller info
            if let Some(seller) = invoice_data.get("seller") {
                writer.write_record(&[
                    "Vendedor:",
                    seller
                        .get("company_name")
                        .and_then(|n| n.as_str())
                        .unwrap_or(""),
                    "",
                    "",
                    "",
                ])?;

                if let Some(rnc) = seller
                    .get("rnc")
                    .and_then(|r| r.get("value"))
                    .and_then(|v| v.as_str())
                {
                    writer.write_record(&["RNC:", rnc, "", "", ""])?;
                }
            }

            // NCF
            if let Some(ncf) = invoice_data
                .get("ncf")
                .and_then(|n| n.get("formatted"))
                .and_then(|f| f.as_str())
            {
                writer.write_record(&["NCF:", ncf, "", "", ""])?;
            }

            writer.write_record(&["", "", "", "", ""])?;

            // Customer info
            if let Some(customer) = invoice_data.get("customer") {
                writer.write_record(&[
                    "Cliente:",
                    customer.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                    "",
                    "",
                    "",
                ])?;
            }

            writer.write_record(&["", "", "", "", ""])?;

            // Items header
            writer.write_record(&["Cant.", "Descripción", "Precio Unit.", "ITBIS", "Total"])?;

            // Items
            if let Some(items) = invoice_data.get("items").and_then(|i| i.as_array()) {
                for item in items {
                    writer.write_record(&[
                        &self.format_cell(item.get("quantity").unwrap_or(&Value::Null)),
                        &self.format_cell(item.get("description").unwrap_or(&Value::Null)),
                        &self.format_cell(item.get("unit_price").unwrap_or(&Value::Null)),
                        &self.format_cell(item.get("itbis_amount").unwrap_or(&Value::Null)),
                        &self.format_cell(item.get("total").unwrap_or(&Value::Null)),
                    ])?;
                }
            }

            writer.write_record(&["", "", "", "", ""])?;

            // Totals
            writer.write_record(&[
                "",
                "",
                "Subtotal:",
                "",
                &self.format_cell(invoice_data.get("subtotal").unwrap_or(&Value::Null)),
            ])?;
            writer.write_record(&[
                "",
                "",
                "ITBIS:",
                "",
                &self.format_cell(invoice_data.get("itbis_amount").unwrap_or(&Value::Null)),
            ])?;
            writer.write_record(&[
                "",
                "",
                "TOTAL:",
                "",
                &self.format_cell(invoice_data.get("total_amount").unwrap_or(&Value::Null)),
            ])?;

            writer.flush()?;
        }

        Ok(buffer)
    }

    /// Generate report CSV with multiple sections
    pub async fn generate_report_csv(&self, report_data: &Value) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        {
            let mut writer = Writer::from_writer(&mut buffer);

            // Title
            if let Some(title) = report_data.get("title").and_then(|t| t.as_str()) {
                writer.write_record(&[title])?;
                writer.write_record(&[""])?;
            }

            // Executive summary
            if let Some(summary) = report_data
                .get("executive_summary")
                .and_then(|s| s.as_str())
            {
                writer.write_record(&["Resumen Ejecutivo:"])?;
                writer.write_record(&[summary])?;
                writer.write_record(&[""])?;
            }

            // Metrics
            if let Some(metrics) = report_data.get("metrics").and_then(|m| m.as_array()) {
                writer.write_record(&["MÉTRICAS CLAVE"])?;
                for metric in metrics {
                    let label = metric.get("label").and_then(|l| l.as_str()).unwrap_or("");
                    let value = metric.get("value").and_then(|v| v.as_str()).unwrap_or("");
                    writer.write_record(&[label, value])?;
                }
                writer.write_record(&[""])?;
            }

            // Main table data
            if let Some(table) = report_data.get("table") {
                // Headers
                if let Some(headers) = table.get("headers").and_then(|h| h.as_array()) {
                    let header_row: Vec<String> =
                        headers.iter().map(|h| self.format_cell(h)).collect();
                    writer.write_record(&header_row)?;
                }

                // Rows
                if let Some(rows) = table.get("rows").and_then(|r| r.as_array()) {
                    for row in rows {
                        let record = self.format_row(row)?;
                        writer.write_record(&record)?;
                    }
                }

                // Totals
                if let Some(totals) = table.get("totals").and_then(|t| t.as_array()) {
                    let total_row: Vec<String> =
                        totals.iter().map(|t| self.format_cell(t)).collect();
                    writer.write_record(&total_row)?;
                }
            }

            writer.flush()?;
        }

        Ok(buffer)
    }

    /// Generate bulk export CSV
    pub async fn generate_bulk_export(&self, data: &Value) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        {
            let mut writer = Writer::from_writer(&mut buffer);

            // Check if we have streaming data
            if let Some(stream_config) = data.get("stream_config") {
                // Handle streaming configuration for large datasets
                let batch_size = stream_config
                    .get("batch_size")
                    .and_then(|b| b.as_u64())
                    .unwrap_or(1000) as usize;

                // Process in batches for memory efficiency
                if let Some(headers) = data.get("headers").and_then(|h| h.as_array()) {
                    let header_row: Vec<String> =
                        headers.iter().map(|h| self.format_cell(h)).collect();
                    writer.write_record(&header_row)?;
                }

                // Process rows in batches
                if let Some(rows) = data.get("rows").and_then(|r| r.as_array()) {
                    for chunk in rows.chunks(batch_size) {
                        for row in chunk {
                            let record = self.format_row(row)?;
                            writer.write_record(&record)?;
                        }
                        writer.flush()?; // Flush after each batch
                    }
                }
            } else {
                // Standard CSV generation
                return self.generate_csv(data).await;
            }
        }

        Ok(buffer)
    }
}

#[async_trait::async_trait]
impl DocumentGenerator for CsvGenerator {
    async fn generate(&self, _template: &str, data: &Value) -> Result<Vec<u8>> {
        // Determine document type from data
        if let Some(doc_type) = data.get("document_type").and_then(|t| t.as_str()) {
            match doc_type {
                "invoice" => self.generate_invoice_csv(data).await,
                "report" => self.generate_report_csv(data).await,
                "bulk_export" => self.generate_bulk_export(data).await,
                _ => self.generate_csv(data).await,
            }
        } else {
            self.generate_csv(data).await
        }
    }

    fn supported_types(&self) -> Vec<DocumentType> {
        vec![DocumentType::Report, DocumentType::Invoice]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_csv_generation() {
        let generator = CsvGenerator::new();

        let data = serde_json::json!({
            "headers": ["Name", "Age", "City"],
            "rows": [
                ["John Doe", 30, "New York"],
                ["Jane Smith", 25, "Los Angeles"],
                ["Bob Johnson", 35, "Chicago"]
            ]
        });

        let result = generator.generate_csv(&data).await.unwrap();
        let csv_string = String::from_utf8(result).unwrap();

        assert!(csv_string.contains("Name,Age,City"));
        assert!(csv_string.contains("John Doe"));
        assert!(csv_string.contains("30"));
    }

    #[tokio::test]
    async fn test_invoice_csv() {
        let generator = CsvGenerator::new();

        let invoice_data = serde_json::json!({
            "seller": {
                "company_name": "Test Company",
                "rnc": {"value": "130123456"}
            },
            "customer": {
                "name": "Test Customer"
            },
            "items": [
                {
                    "quantity": 2,
                    "description": "Product A",
                    "unit_price": 100.00,
                    "itbis_amount": 18.00,
                    "total": 236.00
                }
            ],
            "subtotal": 200.00,
            "itbis_amount": 36.00,
            "total_amount": 236.00
        });

        let result = generator.generate_invoice_csv(&invoice_data).await.unwrap();
        let csv_string = String::from_utf8(result).unwrap();

        assert!(csv_string.contains("FACTURA FISCAL"));
        assert!(csv_string.contains("Test Company"));
        assert!(csv_string.contains("Product A"));
    }
}
