//! Excel (XLSX) generator for ERP Reports
//!
//! Generates professional Excel documents from ErpReportPayload

use anyhow::Result;
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook, Worksheet};
use std::collections::HashMap;

use crate::templates::{
    ColumnAlign, ColumnDefinition, ColumnType, DataStructureType, ErpReportPayload, ReportGroup,
};

/// Excel generator for ERP reports
pub struct ErpReportExcelGenerator;

impl ErpReportExcelGenerator {
    pub fn new() -> Self {
        Self
    }

    /// Generate Excel from ErpReportPayload
    pub async fn generate(&self, payload: &ErpReportPayload) -> Result<Vec<u8>> {
        let payload = payload.clone();
        tokio::task::spawn_blocking(move || Self::generate_excel(&payload)).await?
    }

    fn generate_excel(payload: &ErpReportPayload) -> Result<Vec<u8>> {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        // Set sheet name from report code
        let sheet_name = payload.report.code.replace("_", " ");
        worksheet.set_name(&sheet_name)?;

        // Create formats
        let formats = Self::create_formats();

        let mut current_row: u32 = 0;

        // Write header section
        current_row = Self::write_header(worksheet, payload, &formats, current_row)?;

        // Write filters info
        current_row = Self::write_filters(worksheet, payload, &formats, current_row)?;

        // Skip a row
        current_row += 1;

        // Remember where the data section starts (for freeze panes)
        // For flat data: freeze after column headers
        // For grouped data: don't freeze (groups have their own headers)
        let freeze_row = match payload.data.structure_type {
            DataStructureType::Flat | DataStructureType::Statement => {
                // Freeze after the column headers row
                Some(current_row + 1)
            }
            // Don't freeze for grouped/hierarchical - each group has its own header
            DataStructureType::Grouped | DataStructureType::HierarchicalGrouped => None,
        };

        // Write data based on structure type
        current_row = match payload.data.structure_type {
            DataStructureType::Flat => {
                Self::write_flat_data(worksheet, payload, &formats, current_row)?
            }
            DataStructureType::Grouped => {
                Self::write_grouped_data(worksheet, payload, &formats, current_row)?
            }
            DataStructureType::HierarchicalGrouped => {
                Self::write_hierarchical_data(worksheet, payload, &formats, current_row)?
            }
            DataStructureType::Statement => {
                Self::write_statement_data(worksheet, payload, &formats, current_row)?
            }
        };

        // Write grand total if present (check both grand_total and totals fields)
        // For Flat data, C# uses GrandTotal; for Grouped data, C# uses Totals
        let grand_total = payload
            .data
            .grand_total
            .as_ref()
            .or(payload.data.totals.as_ref());

        if let Some(gt) = grand_total {
            Self::write_grand_total(
                worksheet,
                &payload.metadata.columns,
                gt,
                &formats,
                current_row,
            )?;
        }

        // Auto-fit columns
        Self::auto_fit_columns(worksheet, &payload.metadata.columns)?;

        // Freeze header rows only for flat/statement data
        // For grouped data, freezing doesn't work well because each group has its own header
        if let Some(row) = freeze_row {
            worksheet.set_freeze_panes(row, 0)?;
        }

        Ok(workbook.save_to_buffer()?)
    }

    fn create_formats() -> ExcelFormats {
        ExcelFormats {
            title: Format::new()
                .set_font_size(14)
                .set_bold()
                .set_font_color(Color::RGB(0x1F4E79)),
            subtitle: Format::new()
                .set_font_size(10)
                .set_font_color(Color::RGB(0x666666)),
            header: Format::new()
                .set_bold()
                .set_background_color(Color::RGB(0x1F4E79))
                .set_font_color(Color::White)
                .set_align(FormatAlign::Center)
                .set_border(FormatBorder::Thin),
            header_right: Format::new()
                .set_bold()
                .set_background_color(Color::RGB(0x1F4E79))
                .set_font_color(Color::White)
                .set_align(FormatAlign::Right)
                .set_border(FormatBorder::Thin),
            cell: Format::new().set_border(FormatBorder::Thin),
            cell_right: Format::new()
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::Right),
            cell_center: Format::new()
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::Center),
            currency: Format::new()
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::Right)
                .set_num_format("#,##0.00"),
            currency_highlight: Format::new()
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::Right)
                .set_num_format("#,##0.00")
                .set_bold(),
            percentage: Format::new()
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::Right)
                .set_num_format("0.00%"),
            date: Format::new()
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::Center),
            group_header: Format::new()
                .set_bold()
                .set_font_size(11)
                .set_background_color(Color::RGB(0xD6DCE4))
                .set_border(FormatBorder::Thin),
            // Subgroup header - lighter gray, slightly smaller
            subgroup_header: Format::new()
                .set_bold()
                .set_font_size(10)
                .set_background_color(Color::RGB(0xE8E8E8))
                .set_border(FormatBorder::Thin),
            // Main group subtotal (level 0) - darker green, bold
            subtotal: Format::new()
                .set_bold()
                .set_font_size(10)
                .set_background_color(Color::RGB(0xC6EFCE))
                .set_font_color(Color::RGB(0x006100))
                .set_border(FormatBorder::Thin),
            subtotal_currency: Format::new()
                .set_bold()
                .set_font_size(10)
                .set_background_color(Color::RGB(0xC6EFCE))
                .set_font_color(Color::RGB(0x006100))
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::Right)
                .set_num_format("#,##0.00"),
            // Subgroup subtotal (level 1+) - lighter green
            subgroup_subtotal: Format::new()
                .set_bold()
                .set_background_color(Color::RGB(0xE2EFDA))
                .set_border(FormatBorder::Thin),
            subgroup_subtotal_currency: Format::new()
                .set_bold()
                .set_background_color(Color::RGB(0xE2EFDA))
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::Right)
                .set_num_format("#,##0.00"),
            grand_total: Format::new()
                .set_bold()
                .set_background_color(Color::RGB(0x1F4E79))
                .set_font_color(Color::White)
                .set_border(FormatBorder::Thin),
            grand_total_currency: Format::new()
                .set_bold()
                .set_background_color(Color::RGB(0x1F4E79))
                .set_font_color(Color::White)
                .set_border(FormatBorder::Thin)
                .set_align(FormatAlign::Right)
                .set_num_format("#,##0.00"),
        }
    }

    /// Write header with three-column layout
    /// Left: Company name + Address
    /// Center: Report title + Subtitle
    /// Right: Breadcrumb + Date + User
    fn write_header(
        worksheet: &mut Worksheet,
        payload: &ErpReportPayload,
        formats: &ExcelFormats,
        start_row: u32,
    ) -> Result<u32> {
        let mut row = start_row;
        let col_count = payload
            .metadata
            .columns
            .iter()
            .filter(|c| !c.hidden)
            .count();

        // Calculate column positions for three sections
        // Left: columns 0-1, Center: middle columns, Right: last 2 columns
        let center_col = (col_count / 2) as u16;
        let right_col = (col_count.saturating_sub(1)) as u16;

        // === Row 1: Company name | Title | Breadcrumb ===
        // Left: Company name
        if let Some(ref company) = payload.company_info {
            let company_name = company.display_name.as_ref().unwrap_or(&company.name);
            worksheet.write_string_with_format(row, 0, company_name, &formats.title)?;
        }

        // Center: Report title
        worksheet.write_string_with_format(
            row,
            center_col,
            &payload.report.title,
            &formats.title,
        )?;

        // Right: Breadcrumb
        if let Some(ref breadcrumb) = payload.report.breadcrumb {
            // Create right-aligned subtitle format
            let right_format = Format::new()
                .set_font_size(10)
                .set_font_color(Color::RGB(0x666666))
                .set_align(FormatAlign::Right);
            worksheet.write_string_with_format(row, right_col, breadcrumb, &right_format)?;
        }
        row += 1;

        // === Row 2: Address | Subtitle + Billing Mode | Date ===
        // Left: Address
        if let Some(ref company) = payload.company_info {
            if let Some(ref address) = company.address {
                worksheet.write_string_with_format(row, 0, address, &formats.subtitle)?;
            }
        }

        // Center: Subtitle + Billing Mode badge (e.g., "Ventas | MODO B")
        let center_text = match (&payload.report.subtitle, &payload.report.billing_mode) {
            (Some(subtitle), Some(mode)) if !mode.is_empty() => {
                format!("{} | MODO {}", subtitle, mode)
            }
            (Some(subtitle), _) => subtitle.clone(),
            (None, Some(mode)) if !mode.is_empty() => format!("MODO {}", mode),
            _ => String::new(),
        };
        if !center_text.is_empty() {
            worksheet.write_string_with_format(row, center_col, &center_text, &formats.subtitle)?;
        }

        // Right: Date
        let date_text = if payload.report.generated_at.len() >= 10 {
            format!(
                "Fecha: {}",
                Self::format_datetime(&payload.report.generated_at)
            )
        } else {
            format!("Fecha: {}", &payload.report.generated_at)
        };
        let right_format = Format::new()
            .set_font_size(10)
            .set_font_color(Color::RGB(0x666666))
            .set_align(FormatAlign::Right);
        worksheet.write_string_with_format(row, right_col, &date_text, &right_format)?;
        row += 1;

        // === Row 3: (empty) | (empty) | User ===
        if let Some(ref user_name) = payload.report.user_name {
            let user_text = format!("Usuario: {}", user_name);
            let right_format = Format::new()
                .set_font_size(10)
                .set_font_color(Color::RGB(0x666666))
                .set_align(FormatAlign::Right);
            worksheet.write_string_with_format(row, right_col, &user_text, &right_format)?;
            row += 1;
        }

        // Add blank row before data
        row += 1;

        Ok(row)
    }

    /// Format datetime from ISO string to dd/MM/yyyy HH:mm format
    fn format_datetime(iso_datetime: &str) -> String {
        // Parse ISO date and format as dd/MM/yyyy HH:mm
        if let Some(date_part) = iso_datetime.split('T').next() {
            let parts: Vec<&str> = date_part.split('-').collect();
            if parts.len() >= 3 {
                let date = format!("{}/{}/{}", parts[2], parts[1], parts[0]);
                // Extract time if available
                if let Some(time_part) = iso_datetime.split('T').nth(1) {
                    let time = time_part.split('.').next().unwrap_or(time_part);
                    let time = time.trim_end_matches('Z');
                    if time.len() >= 5 {
                        return format!("{} {}", date, &time[..5]);
                    }
                }
                return date;
            }
        }
        iso_datetime.to_string()
    }

    fn write_filters(
        worksheet: &mut Worksheet,
        payload: &ErpReportPayload,
        formats: &ExcelFormats,
        start_row: u32,
    ) -> Result<u32> {
        let mut row = start_row;
        let mut filters = Vec::new();

        if let Some(ref date_range) = payload.report.date_range {
            filters.push(format!(
                "Desde: {} Hasta: {}",
                date_range.from, date_range.to
            ));
        }

        if let Some(ref as_of_date) = payload.report.as_of_date {
            filters.push(format!("Al: {}", as_of_date));
        }

        if !filters.is_empty() {
            let filter_text = filters.join(" | ");
            worksheet.write_string_with_format(row, 0, &filter_text, &formats.subtitle)?;
            row += 1;
        }

        Ok(row)
    }

    fn write_column_headers(
        worksheet: &mut Worksheet,
        columns: &[ColumnDefinition],
        formats: &ExcelFormats,
        row: u32,
    ) -> Result<()> {
        for (col_idx, column) in columns.iter().enumerate() {
            if column.hidden {
                continue;
            }
            let format = match column.align.as_ref().unwrap_or(&ColumnAlign::Left) {
                ColumnAlign::Right => &formats.header_right,
                _ => &formats.header,
            };
            worksheet.write_string_with_format(row, col_idx as u16, &column.label, format)?;
        }
        Ok(())
    }

    fn write_flat_data(
        worksheet: &mut Worksheet,
        payload: &ErpReportPayload,
        formats: &ExcelFormats,
        start_row: u32,
    ) -> Result<u32> {
        let mut row = start_row;

        // Write headers
        Self::write_column_headers(worksheet, &payload.metadata.columns, formats, row)?;
        row += 1;

        // Write data rows
        if let Some(ref rows) = payload.data.rows {
            for data_row in rows {
                Self::write_data_row(
                    worksheet,
                    &payload.metadata.columns,
                    data_row,
                    formats,
                    row,
                    false,
                )?;
                row += 1;
            }
        }

        Ok(row)
    }

    fn write_grouped_data(
        worksheet: &mut Worksheet,
        payload: &ErpReportPayload,
        formats: &ExcelFormats,
        start_row: u32,
    ) -> Result<u32> {
        let mut row = start_row;

        // Write headers
        Self::write_column_headers(worksheet, &payload.metadata.columns, formats, row)?;
        row += 1;

        // Write groups
        if let Some(ref groups) = payload.data.groups {
            for group in groups {
                row = Self::write_group(
                    worksheet,
                    &payload.metadata.columns,
                    group,
                    formats,
                    row,
                    &payload.metadata.grouping,
                )?;
            }
        }

        Ok(row)
    }

    fn write_group(
        worksheet: &mut Worksheet,
        columns: &[ColumnDefinition],
        group: &ReportGroup,
        formats: &ExcelFormats,
        start_row: u32,
        grouping: &Option<crate::templates::GroupingConfig>,
    ) -> Result<u32> {
        let mut row = start_row;

        // Write group header
        let col_count = columns.iter().filter(|c| !c.hidden).count();
        worksheet.merge_range(
            row,
            0,
            row,
            (col_count - 1) as u16,
            &group.label,
            &formats.group_header,
        )?;
        row += 1;

        // Write group rows
        for data_row in &group.rows {
            Self::write_data_row(worksheet, columns, data_row, formats, row, false)?;
            row += 1;
        }

        // Write subtotal if present (level 0 for simple grouped data)
        if let Some(ref subtotal) = group.subtotal {
            let subtotal_label = grouping
                .as_ref()
                .and_then(|g| g.subtotal_label.as_ref())
                .map(|s| s.as_str())
                .unwrap_or("Subtotal");

            Self::write_subtotal_row(
                worksheet,
                columns,
                subtotal,
                subtotal_label,
                formats,
                row,
                0,
            )?;
            row += 1;
        }

        // Add spacing between groups
        row += 1;

        Ok(row)
    }

    fn write_hierarchical_data(
        worksheet: &mut Worksheet,
        payload: &ErpReportPayload,
        formats: &ExcelFormats,
        start_row: u32,
    ) -> Result<u32> {
        let mut row = start_row;

        // Write headers
        Self::write_column_headers(worksheet, &payload.metadata.columns, formats, row)?;
        row += 1;

        // Write hierarchical groups
        if let Some(ref groups) = payload.data.groups {
            for group in groups {
                row = Self::write_hierarchical_group(
                    worksheet,
                    &payload.metadata.columns,
                    group,
                    formats,
                    row,
                    0,
                )?;
            }
        }

        Ok(row)
    }

    fn write_hierarchical_group(
        worksheet: &mut Worksheet,
        columns: &[ColumnDefinition],
        group: &ReportGroup,
        formats: &ExcelFormats,
        start_row: u32,
        level: u32,
    ) -> Result<u32> {
        let mut row = start_row;

        // Write group header with indentation based on level
        let col_count = columns.iter().filter(|c| !c.hidden).count();
        let indent = "  ".repeat(level as usize);
        let label = format!("{}{}", indent, group.label);

        // Select header format based on level
        let header_format = if level == 0 {
            &formats.group_header
        } else {
            &formats.subgroup_header
        };

        worksheet.merge_range(row, 0, row, (col_count - 1) as u16, &label, header_format)?;
        row += 1;

        // Write subgroups if present
        if let Some(ref sub_groups) = group.sub_groups {
            for sub_group in sub_groups {
                row = Self::write_hierarchical_group(
                    worksheet,
                    columns,
                    sub_group,
                    formats,
                    row,
                    level + 1,
                )?;
            }
        }

        // Write rows (leaf level)
        for data_row in &group.rows {
            Self::write_data_row(worksheet, columns, data_row, formats, row, false)?;
            row += 1;
        }

        // Write subtotal if present
        if let Some(ref subtotal) = group.subtotal {
            let label = format!("{}Subtotal {}", indent, group.label);
            Self::write_subtotal_row(worksheet, columns, subtotal, &label, formats, row, level)?;
            row += 1;
        }

        Ok(row)
    }

    fn write_statement_data(
        worksheet: &mut Worksheet,
        payload: &ErpReportPayload,
        formats: &ExcelFormats,
        start_row: u32,
    ) -> Result<u32> {
        let mut row = start_row;

        // Write opening balance if present
        if let Some(ref opening) = payload.data.opening_balance {
            worksheet.write_string_with_format(row, 0, &opening.label, &formats.subtotal)?;
            let col_count = payload.metadata.columns.len();
            worksheet.write_number_with_format(
                row,
                (col_count - 1) as u16,
                opening.value,
                &formats.subtotal_currency,
            )?;
            row += 2;
        }

        // Write headers
        Self::write_column_headers(worksheet, &payload.metadata.columns, formats, row)?;
        row += 1;

        // Write data rows
        if let Some(ref rows) = payload.data.rows {
            for data_row in rows {
                Self::write_data_row(
                    worksheet,
                    &payload.metadata.columns,
                    data_row,
                    formats,
                    row,
                    true,
                )?;
                row += 1;
            }
        }

        Ok(row)
    }

    fn write_data_row(
        worksheet: &mut Worksheet,
        columns: &[ColumnDefinition],
        data: &HashMap<String, serde_json::Value>,
        formats: &ExcelFormats,
        row: u32,
        _is_statement: bool,
    ) -> Result<()> {
        for (col_idx, column) in columns.iter().enumerate() {
            if column.hidden {
                continue;
            }

            let col = col_idx as u16;
            let value = data.get(&column.key);

            match &column.column_type {
                ColumnType::Currency | ColumnType::Decimal => {
                    let num = value.and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let format = if column.highlight.is_some() {
                        &formats.currency_highlight
                    } else {
                        &formats.currency
                    };
                    worksheet.write_number_with_format(row, col, num, format)?;
                }
                ColumnType::Integer => {
                    let num = value
                        .and_then(|v| v.as_i64().map(|i| i as f64).or_else(|| v.as_f64()))
                        .unwrap_or(0.0);
                    worksheet.write_number_with_format(row, col, num, &formats.cell_right)?;
                }
                ColumnType::Percentage => {
                    let num = value.and_then(|v| v.as_f64()).unwrap_or(0.0) / 100.0;
                    worksheet.write_number_with_format(row, col, num, &formats.percentage)?;
                }
                ColumnType::Date | ColumnType::DateTime => {
                    let text = value
                        .and_then(|v| v.as_str())
                        .map(|s| Self::format_date(s))
                        .unwrap_or_default();
                    worksheet.write_string_with_format(row, col, &text, &formats.date)?;
                }
                ColumnType::Boolean => {
                    let text = value
                        .and_then(|v| v.as_bool())
                        .map(|b| if b { "Sí" } else { "No" })
                        .unwrap_or("-");
                    worksheet.write_string_with_format(row, col, text, &formats.cell_center)?;
                }
                _ => {
                    let text = value
                        .map(|v| match v {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Number(n) => n.to_string(),
                            serde_json::Value::Bool(b) => b.to_string(),
                            serde_json::Value::Null => String::new(),
                            _ => v.to_string(),
                        })
                        .unwrap_or_default();

                    let format = match column.align.as_ref().unwrap_or(&ColumnAlign::Left) {
                        ColumnAlign::Right => &formats.cell_right,
                        ColumnAlign::Center => &formats.cell_center,
                        ColumnAlign::Left => &formats.cell,
                    };
                    worksheet.write_string_with_format(row, col, &text, format)?;
                }
            }
        }
        Ok(())
    }

    fn write_subtotal_row(
        worksheet: &mut Worksheet,
        columns: &[ColumnDefinition],
        subtotal: &HashMap<String, serde_json::Value>,
        label: &str,
        formats: &ExcelFormats,
        row: u32,
        level: u32,
    ) -> Result<()> {
        // Select format based on level (0 = main group, 1+ = subgroup)
        let (label_format, currency_format) = if level == 0 {
            (&formats.subtotal, &formats.subtotal_currency)
        } else {
            (
                &formats.subgroup_subtotal,
                &formats.subgroup_subtotal_currency,
            )
        };

        // Write label in first column
        worksheet.write_string_with_format(row, 0, label, label_format)?;

        // Write subtotal values
        for (col_idx, column) in columns.iter().enumerate().skip(1) {
            if column.hidden {
                continue;
            }

            let col = col_idx as u16;
            if let Some(value) = subtotal.get(&column.key) {
                if let Some(num) = value.as_f64() {
                    worksheet.write_number_with_format(row, col, num, currency_format)?;
                }
            } else {
                worksheet.write_string_with_format(row, col, "", label_format)?;
            }
        }
        Ok(())
    }

    fn write_grand_total(
        worksheet: &mut Worksheet,
        columns: &[ColumnDefinition],
        grand_total: &HashMap<String, serde_json::Value>,
        formats: &ExcelFormats,
        row: u32,
    ) -> Result<()> {
        // Write label
        worksheet.write_string_with_format(row, 0, "TOTAL GENERAL", &formats.grand_total)?;

        // Write total values
        for (col_idx, column) in columns.iter().enumerate().skip(1) {
            if column.hidden {
                continue;
            }

            let col = col_idx as u16;
            if let Some(value) = grand_total.get(&column.key) {
                if let Some(num) = value.as_f64() {
                    worksheet.write_number_with_format(
                        row,
                        col,
                        num,
                        &formats.grand_total_currency,
                    )?;
                } else {
                    worksheet.write_string_with_format(row, col, "", &formats.grand_total)?;
                }
            } else {
                worksheet.write_string_with_format(row, col, "", &formats.grand_total)?;
            }
        }

        // Add record count if present
        if let Some(count) = grand_total.get("recordCount").and_then(|v| v.as_i64()) {
            let row = row + 1;
            worksheet.write_string_with_format(
                row,
                0,
                &format!("Total registros: {}", count),
                &formats.subtitle,
            )?;
        }

        Ok(())
    }

    fn auto_fit_columns(worksheet: &mut Worksheet, columns: &[ColumnDefinition]) -> Result<()> {
        for (idx, column) in columns.iter().enumerate() {
            if column.hidden {
                continue;
            }
            let width = column.width.unwrap_or(100) as f64 / 7.0; // Approximate conversion
            worksheet.set_column_width(idx as u16, width.max(10.0))?;
        }
        Ok(())
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

impl Default for ErpReportExcelGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Excel format collection
struct ExcelFormats {
    title: Format,
    subtitle: Format,
    header: Format,
    header_right: Format,
    cell: Format,
    cell_right: Format,
    cell_center: Format,
    currency: Format,
    currency_highlight: Format,
    percentage: Format,
    date: Format,
    group_header: Format,
    /// Subgroup header (level 1+) - lighter color
    subgroup_header: Format,
    /// Subtotal for main groups (level 0) - darker green
    subtotal: Format,
    subtotal_currency: Format,
    /// Subtotal for subgroups (level 1+) - lighter green
    subgroup_subtotal: Format,
    subgroup_subtotal_currency: Format,
    grand_total: Format,
    grand_total_currency: Format,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::{
        DataStructureType, OutputFormat, OutputOptions, PageOrientation, PageSize, ReportDataSet,
        ReportInfo, ReportMetadata,
    };

    fn create_test_payload() -> ErpReportPayload {
        let mut rows = Vec::new();
        for i in 0..5 {
            let mut row = HashMap::new();
            row.insert("name".to_string(), serde_json::json!(format!("Item {}", i)));
            row.insert(
                "amount".to_string(),
                serde_json::json!(100.0 * (i + 1) as f64),
            );
            rows.push(row);
        }

        ErpReportPayload {
            correlation_id: None,
            document_type: "REPORT".to_string(),
            tenant_id: 1,
            user_id: 1,
            report: ReportInfo {
                code: "TEST_REPORT".to_string(),
                variant: None,
                title: "Test Report".to_string(),
                subtitle: None,
                breadcrumb: Some("Test / Reports".to_string()),
                generated_at: "2025-12-15T10:00:00Z".to_string(),
                user_name: None,
                date_range: None,
                as_of_date: None,
                billing_mode: None,
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
                        hide_in_print: false,
                    },
                    ColumnDefinition {
                        key: "amount".to_string(),
                        label: "Amount".to_string(),
                        column_type: ColumnType::Currency,
                        format: None,
                        width: Some(100),
                        align: Some(ColumnAlign::Right),
                        sortable: false,
                        highlight: Some("primary".to_string()),
                        hidden: false,
                        hide_in_print: false,
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
                    gt.insert("amount".to_string(), serde_json::json!(1500.0));
                    gt.insert("recordCount".to_string(), serde_json::json!(5));
                    gt
                }),
                summary: None,
                total_records: 5,
                totals: None,
            },
            output: OutputOptions {
                format: OutputFormat::Xlsx,
                page_size: Some(PageSize::Letter),
                orientation: Some(PageOrientation::Portrait),
                scale: 100,
                margins: None,
                include_header: true,
                include_footer: true,
                show_logo: false,
                file_name: Some("test.xlsx".to_string()),
            },
            delivery: None,
            company_info: None,
        }
    }

    #[tokio::test]
    async fn test_excel_generation() {
        let generator = ErpReportExcelGenerator::new();
        let payload = create_test_payload();

        let result = generator.generate(&payload).await;
        assert!(result.is_ok());

        let bytes = result.unwrap();
        assert!(!bytes.is_empty());

        // XLSX files start with PK (zip signature)
        assert_eq!(&bytes[0..2], b"PK");
    }
}
