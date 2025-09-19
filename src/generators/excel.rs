use anyhow::Result;
use rust_xlsxwriter::{Workbook, Worksheet, Format, FormatBorder, Color};
use rayon::prelude::*;
use std::collections::HashMap;

use crate::models::{
    ReportRequest, ReportSchema, ColumnDefinition, DataType,
    Alignment, AggregateOperation, ConditionalFormat
};

pub struct ExcelGenerator {
    use_constant_memory: bool,
}

impl ExcelGenerator {
    pub fn new() -> Self {
        ExcelGenerator {
            use_constant_memory: false,
        }
    }

    pub fn with_constant_memory(mut self) -> Self {
        self.use_constant_memory = true;
        self
    }

    pub async fn generate_report(
        &self,
        request: &ReportRequest,
        data: Vec<serde_json::Value>,
    ) -> Result<Vec<u8>> {
        // Process in blocking task for CPU-intensive work
        let use_constant_memory = self.use_constant_memory || data.len() > 10000;

        tokio::task::spawn_blocking(move || {
            Self::generate_excel_sync(request, data, use_constant_memory)
        }).await?
    }

    fn generate_excel_sync(
        request: &ReportRequest,
        data: Vec<serde_json::Value>,
        use_constant_memory: bool,
    ) -> Result<Vec<u8>> {
        let mut workbook = Workbook::new();

        // Configure for large files if needed
        if use_constant_memory {
            workbook.use_constant_memory(true)?;
        }

        // Create main worksheet
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(&request.title)?;

        // Create formats
        let formats = Self::create_formats(&workbook);

        // Write headers
        Self::write_headers(worksheet, &request.schema, &formats)?;

        // Process and write data
        Self::write_data(worksheet, &request.schema, &data, &formats)?;

        // Apply options
        if let Some(options) = &request.options {
            if options.freeze_headers {
                worksheet.set_freeze_panes(1, 0)?;
            }

            if options.auto_filter {
                let last_col = request.schema.columns.len() as u16 - 1;
                let last_row = data.len() as u32;
                worksheet.autofilter(0, 0, last_row, last_col)?;
            }

            // Apply conditional formatting
            if let Some(formats) = &options.conditional_formatting {
                Self::apply_conditional_formatting(worksheet, formats, &data)?;
            }
        }

        // Add aggregations if specified
        if let Some(aggregations) = &request.schema.aggregations {
            Self::write_aggregations(worksheet, &data, aggregations, &formats)?;
        }

        // Set column widths
        Self::set_column_widths(worksheet, &request.schema)?;

        // Save to buffer
        let mut buffer = Vec::new();
        workbook.save_to_buffer(&mut buffer)?;

        Ok(buffer)
    }

    fn create_formats(workbook: &Workbook) -> HashMap<String, Format> {
        let mut formats = HashMap::new();

        // Header format
        let header = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0x4472C4))
            .set_font_color(Color::White)
            .set_border(FormatBorder::Thin);
        formats.insert("header".to_string(), header);

        // Data formats
        let cell = Format::new()
            .set_border(FormatBorder::Thin);
        formats.insert("cell".to_string(), cell);

        // Currency format
        let currency = Format::new()
            .set_num_format("$#,##0.00")
            .set_border(FormatBorder::Thin);
        formats.insert("currency".to_string(), currency);

        // Percentage format
        let percentage = Format::new()
            .set_num_format("0.00%")
            .set_border(FormatBorder::Thin);
        formats.insert("percentage".to_string(), percentage);

        // Date format
        let date = Format::new()
            .set_num_format("dd/mm/yyyy")
            .set_border(FormatBorder::Thin);
        formats.insert("date".to_string(), date);

        // Total row format
        let total = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0xE0E0E0))
            .set_border(FormatBorder::Thin);
        formats.insert("total".to_string(), total);

        formats
    }

    fn write_headers(
        worksheet: &mut Worksheet,
        schema: &ReportSchema,
        formats: &HashMap<String, Format>,
    ) -> Result<()> {
        let header_format = formats.get("header").unwrap();

        for (col, column) in schema.columns.iter().enumerate() {
            if column.visible {
                worksheet.write_string_with_format(
                    0,
                    col as u16,
                    &column.header,
                    header_format,
                )?;
            }
        }

        Ok(())
    }

    fn write_data(
        worksheet: &mut Worksheet,
        schema: &ReportSchema,
        data: &[serde_json::Value],
        formats: &HashMap<String, Format>,
    ) -> Result<()> {
        // Process data in parallel for performance
        let processed_rows: Vec<Vec<(String, DataType, Option<String>)>> = data
            .par_iter()
            .map(|row| {
                schema.columns.iter()
                    .filter(|col| col.visible)
                    .map(|col| {
                        let value = row.get(&col.field)
                            .cloned()
                            .unwrap_or(serde_json::Value::Null);
                        let formatted = Self::format_value(&value, &col.data_type);
                        (formatted, col.data_type.clone(), col.format.clone())
                    })
                    .collect()
            })
            .collect();

        // Write to worksheet sequentially (required for constant_memory mode)
        for (row_idx, row_data) in processed_rows.iter().enumerate() {
            let row_num = (row_idx + 1) as u32; // +1 for header

            for (col_idx, (value, data_type, format_str)) in row_data.iter().enumerate() {
                let col_num = col_idx as u16;

                match data_type {
                    DataType::Currency => {
                        if let Ok(num) = value.parse::<f64>() {
                            worksheet.write_number_with_format(
                                row_num,
                                col_num,
                                num,
                                formats.get("currency").unwrap(),
                            )?;
                        } else {
                            worksheet.write_string(row_num, col_num, value)?;
                        }
                    },
                    DataType::Percentage => {
                        if let Ok(num) = value.parse::<f64>() {
                            worksheet.write_number_with_format(
                                row_num,
                                col_num,
                                num / 100.0,
                                formats.get("percentage").unwrap(),
                            )?;
                        } else {
                            worksheet.write_string(row_num, col_num, value)?;
                        }
                    },
                    DataType::Number => {
                        if let Ok(num) = value.parse::<f64>() {
                            worksheet.write_number_with_format(
                                row_num,
                                col_num,
                                num,
                                formats.get("cell").unwrap(),
                            )?;
                        } else {
                            worksheet.write_string(row_num, col_num, value)?;
                        }
                    },
                    _ => {
                        worksheet.write_string_with_format(
                            row_num,
                            col_num,
                            value,
                            formats.get("cell").unwrap(),
                        )?;
                    }
                }
            }

            // Report progress for large datasets
            if row_idx > 0 && row_idx % 10000 == 0 {
                tracing::debug!("Processed {} rows", row_idx);
            }
        }

        Ok(())
    }

    fn format_value(value: &serde_json::Value, data_type: &DataType) -> String {
        match value {
            serde_json::Value::Null => String::new(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => s.clone(),
            _ => value.to_string(),
        }
    }

    fn write_aggregations(
        worksheet: &mut Worksheet,
        data: &[serde_json::Value],
        aggregations: &[crate::models::Aggregation],
        formats: &HashMap<String, Format>,
    ) -> Result<()> {
        let start_row = (data.len() + 2) as u32;
        let total_format = formats.get("total").unwrap();

        for (idx, agg) in aggregations.iter().enumerate() {
            let row = start_row + idx as u32;

            // Write label
            let label = agg.alias.clone()
                .unwrap_or_else(|| format!("{}_{:?}", agg.field, agg.operation));
            worksheet.write_string_with_format(row, 0, &label, total_format)?;

            // Calculate and write value
            let result = Self::calculate_aggregation(data, agg);
            worksheet.write_number_with_format(row, 1, result, total_format)?;
        }

        Ok(())
    }

    fn calculate_aggregation(
        data: &[serde_json::Value],
        agg: &crate::models::Aggregation,
    ) -> f64 {
        let values: Vec<f64> = data.iter()
            .filter_map(|row| {
                row.get(&agg.field)
                    .and_then(|v| v.as_f64())
            })
            .collect();

        match agg.operation {
            AggregateOperation::Sum => values.iter().sum(),
            AggregateOperation::Average => {
                if values.is_empty() {
                    0.0
                } else {
                    values.iter().sum::<f64>() / values.len() as f64
                }
            },
            AggregateOperation::Count => values.len() as f64,
            AggregateOperation::Min => values.iter().cloned().fold(f64::INFINITY, f64::min),
            AggregateOperation::Max => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            AggregateOperation::Distinct => {
                let unique: std::collections::HashSet<_> = values.iter()
                    .map(|v| (v * 1000.0).round() as i64)
                    .collect();
                unique.len() as f64
            },
        }
    }

    fn set_column_widths(
        worksheet: &mut Worksheet,
        schema: &ReportSchema,
    ) -> Result<()> {
        for (idx, column) in schema.columns.iter().enumerate() {
            if let Some(width) = column.width {
                worksheet.set_column_width(idx as u16, width as f64)?;
            } else {
                // Auto width based on data type
                let width = match column.data_type {
                    DataType::Currency | DataType::Number => 15.0,
                    DataType::Date => 12.0,
                    DataType::DateTime => 20.0,
                    _ => 20.0,
                };
                worksheet.set_column_width(idx as u16, width)?;
            }
        }

        Ok(())
    }

    fn apply_conditional_formatting(
        worksheet: &mut Worksheet,
        formats: &[ConditionalFormat],
        data: &[serde_json::Value],
    ) -> Result<()> {
        // TODO: Implement conditional formatting
        // This would apply color scales, data bars, icon sets, etc.
        Ok(())
    }
}