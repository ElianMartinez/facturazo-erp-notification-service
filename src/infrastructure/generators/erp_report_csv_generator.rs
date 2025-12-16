//! CSV generator for ERP Reports
//!
//! Generates CSV files from ErpReportPayload with proper encoding and escaping

use anyhow::Result;
use std::collections::HashMap;
use std::io::Write;

use crate::templates::{
    ColumnDefinition, ColumnType, DataStructureType, ErpReportPayload, ReportGroup,
};

/// CSV generator for ERP reports
pub struct ErpReportCsvGenerator {
    /// Field delimiter (default: comma)
    delimiter: char,
    /// Include BOM for Excel compatibility
    include_bom: bool,
    /// Include subtotals in output
    include_subtotals: bool,
}

impl ErpReportCsvGenerator {
    pub fn new() -> Self {
        Self {
            delimiter: ',',
            include_bom: true,
            include_subtotals: true,
        }
    }

    /// Create with custom delimiter
    pub fn with_delimiter(mut self, delimiter: char) -> Self {
        self.delimiter = delimiter;
        self
    }

    /// Set whether to include BOM
    pub fn with_bom(mut self, include_bom: bool) -> Self {
        self.include_bom = include_bom;
        self
    }

    /// Set whether to include subtotals
    pub fn with_subtotals(mut self, include: bool) -> Self {
        self.include_subtotals = include;
        self
    }

    /// Generate CSV from ErpReportPayload
    pub async fn generate(&self, payload: &ErpReportPayload) -> Result<Vec<u8>> {
        let payload = payload.clone();
        let delimiter = self.delimiter;
        let include_bom = self.include_bom;
        let include_subtotals = self.include_subtotals;

        tokio::task::spawn_blocking(move || {
            Self::generate_csv(&payload, delimiter, include_bom, include_subtotals)
        })
        .await?
    }

    fn generate_csv(
        payload: &ErpReportPayload,
        delimiter: char,
        include_bom: bool,
        include_subtotals: bool,
    ) -> Result<Vec<u8>> {
        let mut output = Vec::new();

        // Add UTF-8 BOM for Excel compatibility
        if include_bom {
            output.write_all(&[0xEF, 0xBB, 0xBF])?;
        }

        // Get visible columns
        let columns: Vec<&ColumnDefinition> = payload
            .metadata
            .columns
            .iter()
            .filter(|c| !c.hidden)
            .collect();

        // Write header row
        Self::write_header_row(&mut output, &columns, delimiter)?;

        // Write data based on structure type
        match payload.data.structure_type {
            DataStructureType::Flat => {
                Self::write_flat_data(&mut output, &payload, &columns, delimiter)?;
            }
            DataStructureType::Grouped => {
                Self::write_grouped_data(
                    &mut output,
                    &payload,
                    &columns,
                    delimiter,
                    include_subtotals,
                )?;
            }
            DataStructureType::HierarchicalGrouped => {
                Self::write_hierarchical_data(
                    &mut output,
                    &payload,
                    &columns,
                    delimiter,
                    include_subtotals,
                )?;
            }
            DataStructureType::Statement => {
                Self::write_statement_data(&mut output, &payload, &columns, delimiter)?;
            }
        }

        // Write grand total if present and enabled
        if include_subtotals {
            if let Some(ref grand_total) = payload.data.grand_total {
                Self::write_grand_total(&mut output, &columns, grand_total, delimiter)?;
            }
        }

        Ok(output)
    }

    fn write_header_row(
        output: &mut Vec<u8>,
        columns: &[&ColumnDefinition],
        delimiter: char,
    ) -> Result<()> {
        let headers: Vec<String> = columns.iter().map(|c| Self::escape_csv(&c.label)).collect();
        writeln!(output, "{}", headers.join(&delimiter.to_string()))?;
        Ok(())
    }

    fn write_flat_data(
        output: &mut Vec<u8>,
        payload: &ErpReportPayload,
        columns: &[&ColumnDefinition],
        delimiter: char,
    ) -> Result<()> {
        if let Some(ref rows) = payload.data.rows {
            for row in rows {
                Self::write_data_row(output, columns, row, delimiter)?;
            }
        }
        Ok(())
    }

    fn write_grouped_data(
        output: &mut Vec<u8>,
        payload: &ErpReportPayload,
        columns: &[&ColumnDefinition],
        delimiter: char,
        include_subtotals: bool,
    ) -> Result<()> {
        if let Some(ref groups) = payload.data.groups {
            for group in groups {
                Self::write_group(output, columns, group, delimiter, include_subtotals)?;
            }
        }
        Ok(())
    }

    fn write_group(
        output: &mut Vec<u8>,
        columns: &[&ColumnDefinition],
        group: &ReportGroup,
        delimiter: char,
        include_subtotals: bool,
    ) -> Result<()> {
        // Write group header as a row
        let empty_cols = delimiter
            .to_string()
            .repeat(columns.len().saturating_sub(1));
        writeln!(output, "\"--- {} ---\"{}", group.label, empty_cols)?;

        // Write group rows
        for row in &group.rows {
            Self::write_data_row(output, columns, row, delimiter)?;
        }

        // Write subtotal if present
        if include_subtotals {
            if let Some(ref subtotal) = group.subtotal {
                Self::write_subtotal_row(
                    output,
                    columns,
                    subtotal,
                    &format!("Subtotal {}", group.label),
                    delimiter,
                )?;
            }
        }

        Ok(())
    }

    fn write_hierarchical_data(
        output: &mut Vec<u8>,
        payload: &ErpReportPayload,
        columns: &[&ColumnDefinition],
        delimiter: char,
        include_subtotals: bool,
    ) -> Result<()> {
        if let Some(ref groups) = payload.data.groups {
            for group in groups {
                Self::write_hierarchical_group(
                    output,
                    columns,
                    group,
                    delimiter,
                    include_subtotals,
                    0,
                )?;
            }
        }
        Ok(())
    }

    fn write_hierarchical_group(
        output: &mut Vec<u8>,
        columns: &[&ColumnDefinition],
        group: &ReportGroup,
        delimiter: char,
        include_subtotals: bool,
        level: usize,
    ) -> Result<()> {
        // Write group header with indentation
        let indent = "  ".repeat(level);
        let empty_cols = delimiter
            .to_string()
            .repeat(columns.len().saturating_sub(1));
        writeln!(
            output,
            "\"{}--- {} ---\"{}",
            indent, group.label, empty_cols
        )?;

        // Write subgroups
        if let Some(ref sub_groups) = group.sub_groups {
            for sub_group in sub_groups {
                Self::write_hierarchical_group(
                    output,
                    columns,
                    sub_group,
                    delimiter,
                    include_subtotals,
                    level + 1,
                )?;
            }
        }

        // Write rows
        for row in &group.rows {
            Self::write_data_row(output, columns, row, delimiter)?;
        }

        // Write subtotal
        if include_subtotals {
            if let Some(ref subtotal) = group.subtotal {
                Self::write_subtotal_row(
                    output,
                    columns,
                    subtotal,
                    &format!("{}Subtotal {}", indent, group.label),
                    delimiter,
                )?;
            }
        }

        Ok(())
    }

    fn write_statement_data(
        output: &mut Vec<u8>,
        payload: &ErpReportPayload,
        columns: &[&ColumnDefinition],
        delimiter: char,
    ) -> Result<()> {
        // Write opening balance if present
        if let Some(ref opening) = payload.data.opening_balance {
            let empty_cols = delimiter
                .to_string()
                .repeat(columns.len().saturating_sub(2));
            writeln!(
                output,
                "\"{}\"{}{}\"{}\"",
                opening.label, empty_cols, delimiter, opening.value
            )?;
        }

        // Write data rows
        if let Some(ref rows) = payload.data.rows {
            for row in rows {
                Self::write_data_row(output, columns, row, delimiter)?;
            }
        }

        Ok(())
    }

    fn write_data_row(
        output: &mut Vec<u8>,
        columns: &[&ColumnDefinition],
        data: &HashMap<String, serde_json::Value>,
        delimiter: char,
    ) -> Result<()> {
        let values: Vec<String> = columns
            .iter()
            .map(|col| Self::format_value(data.get(&col.key), &col.column_type))
            .collect();

        writeln!(output, "{}", values.join(&delimiter.to_string()))?;
        Ok(())
    }

    fn write_subtotal_row(
        output: &mut Vec<u8>,
        columns: &[&ColumnDefinition],
        subtotal: &HashMap<String, serde_json::Value>,
        label: &str,
        delimiter: char,
    ) -> Result<()> {
        let mut values = Vec::new();
        values.push(Self::escape_csv(label));

        for col in columns.iter().skip(1) {
            if let Some(value) = subtotal.get(&col.key) {
                values.push(Self::format_value(Some(value), &col.column_type));
            } else {
                values.push(String::new());
            }
        }

        writeln!(output, "{}", values.join(&delimiter.to_string()))?;
        Ok(())
    }

    fn write_grand_total(
        output: &mut Vec<u8>,
        columns: &[&ColumnDefinition],
        grand_total: &HashMap<String, serde_json::Value>,
        delimiter: char,
    ) -> Result<()> {
        let mut values = Vec::new();
        values.push("\"TOTAL GENERAL\"".to_string());

        for col in columns.iter().skip(1) {
            if let Some(value) = grand_total.get(&col.key) {
                values.push(Self::format_value(Some(value), &col.column_type));
            } else {
                values.push(String::new());
            }
        }

        writeln!(output, "{}", values.join(&delimiter.to_string()))?;

        // Add record count if present
        if let Some(count) = grand_total.get("recordCount").and_then(|v| v.as_i64()) {
            let empty_cols = delimiter
                .to_string()
                .repeat(columns.len().saturating_sub(1));
            writeln!(output, "\"Total registros: {}\"{}", count, empty_cols)?;
        }

        Ok(())
    }

    fn format_value(value: Option<&serde_json::Value>, column_type: &ColumnType) -> String {
        match value {
            None => String::new(),
            Some(v) => match column_type {
                ColumnType::Currency | ColumnType::Decimal => {
                    if let Some(num) = v.as_f64() {
                        if num == 0.0 {
                            String::new()
                        } else {
                            format!("{:.2}", num)
                        }
                    } else {
                        String::new()
                    }
                }
                ColumnType::Integer => {
                    if let Some(num) = v.as_i64() {
                        num.to_string()
                    } else if let Some(num) = v.as_f64() {
                        format!("{:.0}", num)
                    } else {
                        String::new()
                    }
                }
                ColumnType::Percentage => {
                    if let Some(num) = v.as_f64() {
                        format!("{:.2}%", num)
                    } else {
                        String::new()
                    }
                }
                ColumnType::Boolean => {
                    if let Some(b) = v.as_bool() {
                        if b { "Sí" } else { "No" }.to_string()
                    } else {
                        String::new()
                    }
                }
                ColumnType::Date | ColumnType::DateTime => {
                    if let Some(s) = v.as_str() {
                        Self::format_date(s)
                    } else {
                        String::new()
                    }
                }
                _ => match v {
                    serde_json::Value::String(s) => Self::escape_csv(s),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Null => String::new(),
                    _ => Self::escape_csv(&v.to_string()),
                },
            },
        }
    }

    fn escape_csv(value: &str) -> String {
        if value.contains(',')
            || value.contains('"')
            || value.contains('\n')
            || value.contains('\r')
        {
            format!("\"{}\"", value.replace('"', "\"\""))
        } else {
            value.to_string()
        }
    }

    fn format_date(iso_date: &str) -> String {
        if let Some(date_part) = iso_date.split('T').next() {
            let parts: Vec<&str> = date_part.split('-').collect();
            if parts.len() >= 3 {
                return format!("{}/{}/{}", parts[2], parts[1], parts[0]);
            }
        }
        iso_date.to_string()
    }
}

impl Default for ErpReportCsvGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::{
        ColumnAlign, DataStructureType, OutputFormat, OutputOptions, PageOrientation, PageSize,
        ReportDataSet, ReportInfo, ReportMetadata,
    };

    fn create_test_payload() -> ErpReportPayload {
        let mut rows = Vec::new();
        for i in 0..3 {
            let mut row = HashMap::new();
            row.insert("name".to_string(), serde_json::json!(format!("Item {}", i)));
            row.insert(
                "amount".to_string(),
                serde_json::json!(100.0 * (i + 1) as f64),
            );
            rows.push(row);
        }

        ErpReportPayload {
            document_type: "REPORT".to_string(),
            tenant_id: 1,
            user_id: 1,
            report: ReportInfo {
                code: "TEST_REPORT".to_string(),
                variant: None,
                title: "Test Report".to_string(),
                breadcrumb: None,
                generated_at: "2025-12-15T10:00:00Z".to_string(),
                date_range: None,
                as_of_date: None,
            },
            metadata: ReportMetadata {
                columns: vec![
                    ColumnDefinition {
                        key: "name".to_string(),
                        label: "Name".to_string(),
                        column_type: ColumnType::String,
                        format: None,
                        width: Some(150),
                        align: Some(ColumnAlign::Left),
                        sortable: false,
                        highlight: None,
                        hidden: false,
                    },
                    ColumnDefinition {
                        key: "amount".to_string(),
                        label: "Amount".to_string(),
                        column_type: ColumnType::Currency,
                        format: None,
                        width: Some(100),
                        align: Some(ColumnAlign::Right),
                        sortable: false,
                        highlight: None,
                        hidden: false,
                    },
                ],
                grouping: None,
                show_grand_total: true,
                grand_total_fields: vec!["amount".to_string()],
                has_running_balance: false,
                show_opening_balance: false,
                aging_buckets: None,
            },
            data: ReportDataSet {
                structure_type: DataStructureType::Flat,
                rows: Some(rows),
                groups: None,
                opening_balance: None,
                grand_total: Some({
                    let mut gt = HashMap::new();
                    gt.insert("amount".to_string(), serde_json::json!(600.0));
                    gt.insert("recordCount".to_string(), serde_json::json!(3));
                    gt
                }),
                summary: None,
                total_records: 3,
                totals: None,
            },
            output: OutputOptions {
                format: OutputFormat::Csv,
                page_size: Some(PageSize::Letter),
                orientation: Some(PageOrientation::Portrait),
                scale: 100,
                margins: None,
                include_header: true,
                include_footer: true,
                show_logo: false,
                file_name: Some("test.csv".to_string()),
            },
            delivery: None,
            company_info: None,
        }
    }

    #[tokio::test]
    async fn test_csv_generation() {
        let generator = ErpReportCsvGenerator::new();
        let payload = create_test_payload();

        let result = generator.generate(&payload).await;
        assert!(result.is_ok());

        let bytes = result.unwrap();
        let content = String::from_utf8_lossy(&bytes);

        // Check BOM
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));

        // Check headers
        assert!(content.contains("Name,Amount"));

        // Check data
        assert!(content.contains("Item 0"));
        assert!(content.contains("100.00"));

        // Check grand total
        assert!(content.contains("TOTAL GENERAL"));
        assert!(content.contains("600.00"));
    }

    #[tokio::test]
    async fn test_csv_escaping() {
        let generator = ErpReportCsvGenerator::new();
        let mut payload = create_test_payload();

        // Add row with special characters
        if let Some(ref mut rows) = payload.data.rows {
            let mut row = HashMap::new();
            row.insert(
                "name".to_string(),
                serde_json::json!("Item with, comma and \"quotes\""),
            );
            row.insert("amount".to_string(), serde_json::json!(500.0));
            rows.push(row);
        }

        let result = generator.generate(&payload).await;
        assert!(result.is_ok());

        let binding = result.unwrap();
        let content = String::from_utf8_lossy(&binding);
        assert!(content.contains("\"Item with, comma and \"\"quotes\"\"\""));
    }
}
