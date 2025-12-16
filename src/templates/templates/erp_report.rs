use crate::templates::erp_report_models::*;
use crate::templates::template_trait::{utils, TypstTemplate};
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;

/// Generic ERP Report Template
/// Supports all report types from the ERP system with professional formatting
pub struct ErpReportTemplate;

impl ErpReportTemplate {
    pub fn new() -> Self {
        Self
    }

    /// Get Typst page configuration
    fn get_page_config(&self, output: &OutputOptions) -> String {
        let paper = output
            .page_size
            .as_ref()
            .unwrap_or(&PageSize::Letter)
            .to_typst_paper();

        let orientation = match output
            .orientation
            .as_ref()
            .unwrap_or(&PageOrientation::Portrait)
        {
            PageOrientation::Portrait => "",
            PageOrientation::Landscape => ", flipped: true",
        };

        let default_margins = PageMargins::default();
        let margins = output.margins.as_ref().unwrap_or(&default_margins);

        format!(
            "paper: \"{}\"{}, margin: (top: {}mm, bottom: {}mm, left: {}mm, right: {}mm)",
            paper, orientation, margins.top, margins.bottom, margins.left, margins.right
        )
    }

    /// Get font size based on scale percentage
    fn get_font_size(&self, base_size: f64, scale: i32) -> f64 {
        base_size * (scale as f64 / 100.0)
    }

    /// Format a value based on column type
    fn format_value(&self, value: &serde_json::Value, column: &ColumnDefinition) -> String {
        match value {
            serde_json::Value::Null => "-".to_string(),
            serde_json::Value::Bool(b) => if *b { "Si" } else { "No" }.to_string(),
            serde_json::Value::Number(n) => {
                let num = n.as_f64().unwrap_or(0.0);
                match column.column_type {
                    ColumnType::Currency => self.format_currency(num),
                    ColumnType::Decimal => self.format_decimal(num, 2),
                    ColumnType::Percentage => format!("{:.2}%", num),
                    ColumnType::Integer => format!("{}", num as i64),
                    _ => n.to_string(),
                }
            }
            serde_json::Value::String(s) => match column.column_type {
                ColumnType::Date => self.format_date(s),
                ColumnType::DateTime => self.format_datetime(s),
                ColumnType::Time => s.clone(),
                _ => utils::escape_typst(s),
            },
            _ => utils::escape_typst(&value.to_string()),
        }
    }

    /// Format currency value
    fn format_currency(&self, value: f64) -> String {
        if value == 0.0 {
            return "-".to_string();
        }
        let formatted = self.format_number_with_thousands(value.abs(), 2);
        if value < 0.0 {
            format!("({})", formatted)
        } else {
            formatted
        }
    }

    /// Format decimal value
    fn format_decimal(&self, value: f64, decimals: usize) -> String {
        self.format_number_with_thousands(value, decimals)
    }

    /// Format number with thousands separators (fixes the issue with utils::format_number)
    fn format_number_with_thousands(&self, value: f64, decimals: usize) -> String {
        let formatted = format!("{:.prec$}", value, prec = decimals);
        let parts: Vec<&str> = formatted.split('.').collect();

        let integer_part = parts[0];
        let decimal_part = parts.get(1).unwrap_or(&"");

        // Add thousands separators to integer part
        let integer_with_commas: String = integer_part
            .chars()
            .rev()
            .enumerate()
            .map(|(i, c)| {
                if i > 0 && i % 3 == 0 && c != '-' {
                    format!(",{}", c)
                } else {
                    c.to_string()
                }
            })
            .collect::<String>()
            .chars()
            .rev()
            .collect();

        if decimal_part.is_empty() {
            integer_with_commas
        } else {
            format!("{}.{}", integer_with_commas, decimal_part)
        }
    }

    /// Format date from ISO string
    fn format_date(&self, iso_date: &str) -> String {
        // Parse ISO date and format as dd/MM/yyyy
        if let Some(date_part) = iso_date.split('T').next() {
            let parts: Vec<&str> = date_part.split('-').collect();
            if parts.len() >= 3 {
                return format!("{}/{}/{}", parts[2], parts[1], parts[0]);
            }
        }
        iso_date.to_string()
    }

    /// Format datetime from ISO string
    fn format_datetime(&self, iso_datetime: &str) -> String {
        let date = self.format_date(iso_datetime);
        if let Some(time_part) = iso_datetime.split('T').nth(1) {
            let time = time_part.split('.').next().unwrap_or(time_part);
            let time = time.trim_end_matches('Z');
            return format!("{} {}", date, &time[..5.min(time.len())]);
        }
        date
    }

    /// Get visible columns (non-hidden)
    fn get_visible_columns<'a>(
        &self,
        columns: &'a [ColumnDefinition],
    ) -> Vec<&'a ColumnDefinition> {
        columns.iter().filter(|c| !c.hidden).collect()
    }

    /// Generate column widths using fractional units (fr) for proportional distribution
    /// This ensures the table always fits within the page width
    fn generate_column_widths(&self, columns: &[&ColumnDefinition]) -> String {
        columns
            .iter()
            .map(|col| {
                // Use width as weight for proportional distribution
                // Default weight of 100 if not specified
                let weight = col.width.unwrap_or(100);
                format!("{}fr", weight)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Generate table header row
    /// Uses em units so sizes scale with base font size
    fn generate_header_row(&self, columns: &[&ColumnDefinition]) -> String {
        columns
            .iter()
            .map(|col| {
                let align = match col.align.as_ref().unwrap_or(&ColumnAlign::Left) {
                    ColumnAlign::Left => "left",
                    ColumnAlign::Center => "center",
                    ColumnAlign::Right => "right",
                };
                format!(
                    "table.cell(align: {})[#text(weight: \"bold\", size: 0.9em)[{}]]",
                    align,
                    utils::escape_typst(&col.label)
                )
            })
            .collect::<Vec<_>>()
            .join(",\n    ")
    }

    /// Generate a data row
    /// Uses em units so sizes scale with base font size
    fn generate_data_row(
        &self,
        row: &HashMap<String, serde_json::Value>,
        columns: &[&ColumnDefinition],
        is_subtotal: bool,
    ) -> String {
        columns
            .iter()
            .map(|col| {
                let value = row.get(&col.key).unwrap_or(&serde_json::Value::Null);
                let formatted = self.format_value(value, col);

                let align = match col.align.as_ref().unwrap_or(&ColumnAlign::Left) {
                    ColumnAlign::Left => "left",
                    ColumnAlign::Center => "center",
                    ColumnAlign::Right => "right",
                };

                let weight = if is_subtotal || col.highlight.is_some() {
                    "\"bold\""
                } else {
                    "\"regular\""
                };

                let color = if col
                    .highlight
                    .as_ref()
                    .map(|h| h == "primary")
                    .unwrap_or(false)
                {
                    ", fill: rgb(24, 144, 255)"
                } else {
                    ""
                };

                format!(
                    "table.cell(align: {})[#text(weight: {}, size: 0.8em{})[{}]]",
                    align, weight, color, formatted
                )
            })
            .collect::<Vec<_>>()
            .join(",\n    ")
    }

    /// Generate subtotal row
    fn generate_subtotal_row(
        &self,
        subtotal: &HashMap<String, serde_json::Value>,
        columns: &[&ColumnDefinition],
        label: &str,
    ) -> String {
        let mut cells = Vec::new();
        let mut first_cell = true;

        for col in columns {
            if first_cell {
                cells.push(format!(
                    "table.cell(align: left)[#text(weight: \"bold\", size: 0.8em)[{}]]",
                    label
                ));
                first_cell = false;
            } else {
                let value = subtotal.get(&col.key).unwrap_or(&serde_json::Value::Null);
                let formatted = self.format_value(value, col);

                let align = match col.align.as_ref().unwrap_or(&ColumnAlign::Left) {
                    ColumnAlign::Left => "left",
                    ColumnAlign::Center => "center",
                    ColumnAlign::Right => "right",
                };

                cells.push(format!(
                    "table.cell(align: {})[#text(weight: \"bold\", size: 0.8em)[{}]]",
                    align, formatted
                ));
            }
        }

        cells.join(",\n    ")
    }

    /// Generate flat data table
    fn generate_flat_table(
        &self,
        rows: &[HashMap<String, serde_json::Value>],
        columns: &[&ColumnDefinition],
    ) -> String {
        if rows.is_empty() {
            return self.generate_empty_message();
        }

        let widths = self.generate_column_widths(columns);
        let header = self.generate_header_row(columns);

        let data_rows: Vec<String> = rows
            .iter()
            .map(|row| self.generate_data_row(row, columns, false))
            .collect();

        format!(
            r#"#table(
  columns: ({}),
  stroke: 0.5pt + rgb(220, 220, 220),
  fill: (x, y) => if y == 0 {{ rgb(245, 245, 245) }} else if calc.odd(y) {{ rgb(252, 252, 252) }} else {{ white }},
  inset: 5pt,
  {},
  {}
)"#,
            widths,
            header,
            data_rows.join(",\n  ")
        )
    }

    /// Generate grouped data table
    fn generate_grouped_table(
        &self,
        groups: &[ReportGroup],
        columns: &[&ColumnDefinition],
        grouping: &Option<GroupingConfig>,
    ) -> String {
        if groups.is_empty() {
            return self.generate_empty_message();
        }

        let widths = self.generate_column_widths(columns);
        let header = self.generate_header_row(columns);
        let show_subtotals = grouping.as_ref().map(|g| g.show_subtotals).unwrap_or(false);
        let subtotal_label = grouping
            .as_ref()
            .and_then(|g| g.subtotal_label.clone())
            .unwrap_or_else(|| "Subtotal".to_string());

        let mut all_rows = Vec::new();

        for group in groups {
            // Group header
            all_rows.push(format!(
                "table.cell(colspan: {}, fill: rgb(230, 240, 250))[#text(weight: \"bold\", size: 0.9em)[{} ({} registros)]]",
                columns.len(),
                utils::escape_typst(&group.label),
                group.record_count
            ));

            // Data rows
            for row in &group.rows {
                all_rows.push(self.generate_data_row(row, columns, false));
            }

            // Subtotal row
            if show_subtotals {
                if let Some(ref subtotal) = group.subtotal {
                    all_rows.push(format!(
                        "table.hline(stroke: 1pt + rgb(180, 180, 180)),\n    {}",
                        self.generate_subtotal_row(subtotal, columns, &subtotal_label)
                    ));
                }
            }
        }

        format!(
            r#"#table(
  columns: ({}),
  stroke: 0.5pt + rgb(220, 220, 220),
  fill: (x, y) => if y == 0 {{ rgb(245, 245, 245) }} else {{ white }},
  inset: 5pt,
  {},
  {}
)"#,
            widths,
            header,
            all_rows.join(",\n  ")
        )
    }

    /// Generate hierarchical grouped data table
    fn generate_hierarchical_table(
        &self,
        groups: &[ReportGroup],
        columns: &[&ColumnDefinition],
        grouping: &Option<GroupingConfig>,
    ) -> String {
        if groups.is_empty() {
            return self.generate_empty_message();
        }

        let widths = self.generate_column_widths(columns);
        let header = self.generate_header_row(columns);
        let show_subtotals = grouping.as_ref().map(|g| g.show_subtotals).unwrap_or(false);

        let mut all_rows = Vec::new();

        for group in groups {
            // Level 0 group header
            all_rows.push(format!(
                "table.cell(colspan: {}, fill: rgb(200, 220, 240))[#text(weight: \"bold\", size: 1em)[{}]]",
                columns.len(),
                utils::escape_typst(&group.label)
            ));

            // Process subgroups if they exist
            if let Some(ref sub_groups) = group.sub_groups {
                for sub_group in sub_groups {
                    // Level 1 group header
                    all_rows.push(format!(
                        "table.cell(colspan: {}, fill: rgb(235, 245, 255))[#h(1em)#text(weight: \"semibold\", size: 0.9em)[{} ({})]]",
                        columns.len(),
                        utils::escape_typst(&sub_group.label),
                        sub_group.record_count
                    ));

                    // Data rows
                    for row in &sub_group.rows {
                        all_rows.push(self.generate_data_row(row, columns, false));
                    }

                    // Subgroup subtotal
                    if show_subtotals {
                        if let Some(ref subtotal) = sub_group.subtotal {
                            all_rows.push(format!(
                                "table.hline(stroke: 0.5pt + rgb(200, 200, 200)),\n    {}",
                                self.generate_subtotal_row(
                                    subtotal,
                                    columns,
                                    &format!("Subtotal {}", &sub_group.label)
                                )
                            ));
                        }
                    }
                }
            } else {
                // Direct rows if no subgroups
                for row in &group.rows {
                    all_rows.push(self.generate_data_row(row, columns, false));
                }
            }

            // Group subtotal
            if show_subtotals {
                if let Some(ref subtotal) = group.subtotal {
                    all_rows.push(format!(
                        "table.hline(stroke: 1pt + rgb(180, 180, 180)),\n    {}",
                        self.generate_subtotal_row(
                            subtotal,
                            columns,
                            &format!("Total {}", &group.label)
                        )
                    ));
                }
            }
        }

        format!(
            r#"#table(
  columns: ({}),
  stroke: 0.5pt + rgb(220, 220, 220),
  fill: (x, y) => if y == 0 {{ rgb(240, 240, 240) }} else {{ white }},
  inset: 5pt,
  {},
  {}
)"#,
            widths,
            header,
            all_rows.join(",\n  ")
        )
    }

    /// Generate statement table with running balance
    fn generate_statement_table(
        &self,
        data: &ReportDataSet,
        columns: &[&ColumnDefinition],
    ) -> String {
        let mut content = String::new();

        // Opening balance
        if let Some(ref opening) = data.opening_balance {
            content.push_str(&format!(
                r#"#rect(width: 100%, fill: rgb(255, 250, 240), stroke: 1pt + rgb(255, 200, 100), radius: 3pt, inset: 8pt)[
  #grid(columns: (1fr, auto), [#text(weight: "bold")[{}]], [#text(weight: "bold")[{}]])
]
#v(8pt)
"#,
                utils::escape_typst(&opening.label),
                self.format_currency(opening.value)
            ));
        }

        // Main table
        if let Some(ref rows) = data.rows {
            content.push_str(&self.generate_flat_table(rows, columns));
        } else if let Some(ref groups) = data.groups {
            content.push_str(&self.generate_grouped_table(groups, columns, &None));
        }

        // Summary
        if let Some(ref summary) = data.summary {
            content.push_str("\n#v(10pt)\n");
            content.push_str(&format!(
                r#"#rect(width: 100%, fill: rgb(240, 248, 255), stroke: 1pt + rgb(100, 150, 200), radius: 3pt, inset: 8pt)[
  #text(weight: "bold", size: 10pt)[Resumen]
  #v(5pt)
  #grid(columns: (1fr, auto), gutter: 3pt,"#
            ));

            if let Some(count) = summary.document_count {
                content.push_str(&format!("\n    [Documentos:], [{}],", count));
            }

            if let Some(ref totals) = summary.totals {
                for (key, value) in totals {
                    content.push_str(&format!(
                        "\n    [{}:], [{}],",
                        key,
                        self.format_currency(*value)
                    ));
                }
            }

            if let Some(balance) = summary.final_balance {
                content.push_str(&format!(
                    "\n    [#text(weight: \"bold\")[Saldo Final]:], [#text(weight: \"bold\")[{}]],",
                    self.format_currency(balance)
                ));
            }

            content.push_str("\n  )\n]");
        }

        content
    }

    /// Generate grand total section
    fn generate_grand_total(
        &self,
        grand_total: &HashMap<String, serde_json::Value>,
        columns: &[&ColumnDefinition],
        total_records: i32,
    ) -> String {
        let visible_totals: Vec<String> = columns
            .iter()
            .filter_map(|col| {
                grand_total.get(&col.key).map(|value| {
                    let formatted = self.format_value(value, col);
                    format!("[{}:], [#text(weight: \"bold\")[{}]]", col.label, formatted)
                })
            })
            .collect();

        if visible_totals.is_empty() {
            return String::new();
        }

        format!(
            r#"
#v(1.2em)
#rect(width: 100%, fill: rgb(240, 245, 250), stroke: 1.5pt + rgb(24, 144, 255), radius: 4pt, inset: 1em)[
  #grid(columns: (auto, 1fr), gutter: 5pt,
    [#text(weight: "bold", size: 1.2em, fill: rgb(24, 144, 255))[TOTAL GENERAL]],
    [#text(size: 1em, fill: gray)[({} registros)]],
  )
  #v(0.8em)
  #grid(columns: (15em, auto), gutter: 4pt,
    {}
  )
]"#,
            total_records,
            visible_totals.join(",\n    ")
        )
    }

    /// Generate empty data message
    fn generate_empty_message(&self) -> String {
        r#"#align(center)[
  #v(3em)
  #text(size: 1.3em, fill: gray)[No hay datos para mostrar]
  #v(3em)
]"#
        .to_string()
    }

    /// Generate header section with company info
    fn generate_header_section(
        &self,
        report: &ReportInfo,
        company_info: &Option<ReportCompanyInfo>,
        show_logo: bool,
    ) -> String {
        let mut header = String::new();

        // Company info section (if available)
        if let Some(ref company) = company_info {
            let company_name = company.display_name.as_ref().unwrap_or(&company.name);

            // Check if logo_url is a local file (not a URL)
            // Typst's #image() only supports local files, not URLs
            let has_local_logo = company
                .logo_url
                .as_ref()
                .map(|url| !url.starts_with("http://") && !url.starts_with("https://"))
                .unwrap_or(false);

            // Logo and company name layout
            if show_logo && has_local_logo {
                // With local logo: grid layout (logo left, info right)
                header.push_str(&format!(
                    r#"#grid(columns: (auto, 1fr), gutter: 15pt,
  [#image("{}", width: 80pt, height: auto)],
  [
    #align(right)[
      #text(size: 12pt, weight: "bold")[{}]
      {}
      {}
    ]
  ]
)
#v(8pt)"#,
                    company.logo_url.as_ref().unwrap(),
                    utils::escape_typst(company_name),
                    company
                        .tax_id
                        .as_ref()
                        .map(|t| format!(
                            "\n      #text(size: 9pt, fill: gray)[RNC: {}]",
                            utils::escape_typst(t)
                        ))
                        .unwrap_or_default(),
                    company
                        .address
                        .as_ref()
                        .map(|a| format!(
                            "\n      #text(size: 8pt, fill: gray)[{}]",
                            utils::escape_typst(a)
                        ))
                        .unwrap_or_default()
                ));
            } else {
                // Without logo (or URL logo): centered company name with info
                header.push_str(&format!(
                    r#"#align(center)[
  #text(size: 14pt, weight: "bold")[{}]
  {}
  {}
]
#v(6pt)"#,
                    utils::escape_typst(company_name),
                    company
                        .tax_id
                        .as_ref()
                        .map(|t| format!(
                            "\n  #text(size: 10pt, fill: gray)[RNC: {}]",
                            utils::escape_typst(t)
                        ))
                        .unwrap_or_default(),
                    company
                        .address
                        .as_ref()
                        .map(|a| format!(
                            "\n  #text(size: 9pt, fill: gray)[{}]",
                            utils::escape_typst(a)
                        ))
                        .unwrap_or_default()
                ));
            }
        }

        // Report title and breadcrumb
        header.push_str(&format!(
            r#"#align(center)[
  #text(size: 14pt, weight: "bold")[{}]
  {}
]
#v(8pt)
#line(length: 100%, stroke: 1pt + rgb(24, 144, 255))
#v(8pt)"#,
            utils::escape_typst(&report.title),
            report
                .breadcrumb
                .as_ref()
                .map(|b| format!(
                    "\n  #v(2pt)\n  #text(size: 9pt, fill: gray)[{}]",
                    utils::escape_typst(b)
                ))
                .unwrap_or_default()
        ));

        header
    }

    /// Generate filters display
    fn generate_filters_display(&self, report: &ReportInfo) -> String {
        let mut filters = Vec::new();

        if let Some(ref date_range) = report.date_range {
            filters.push(format!(
                "Periodo: {} al {}",
                self.format_date(&date_range.from),
                self.format_date(&date_range.to)
            ));
        }

        if let Some(ref as_of) = report.as_of_date {
            filters.push(format!("Al: {}", self.format_date(as_of)));
        }

        if filters.is_empty() {
            return String::new();
        }

        format!(
            r#"#rect(width: 100%, fill: rgb(250, 250, 250), stroke: 0.5pt + rgb(200, 200, 200), radius: 2pt, inset: 6pt)[
  #text(size: 8pt, fill: gray)[{}]
]
#v(8pt)"#,
            filters.join(" | ")
        )
    }
}

impl Default for ErpReportTemplate {
    fn default() -> Self {
        Self::new()
    }
}

impl TypstTemplate for ErpReportTemplate {
    fn generate(&self, data: &Value) -> Result<String> {
        let payload: ErpReportPayload = serde_json::from_value(data.clone())
            .context("Error deserializando payload de reporte ERP")?;

        // Debug: Log company info
        if let Some(ref company) = payload.company_info {
            println!(
                "DEBUG: Company info received: name={}, tax_id={:?}, logo_url={:?}",
                company.name, company.tax_id, company.logo_url
            );
        } else {
            println!("DEBUG: No company info in payload");
        }

        let page_config = self.get_page_config(&payload.output);
        let font_size = self.get_font_size(9.0, payload.output.scale);
        let visible_columns = self.get_visible_columns(&payload.metadata.columns);

        // Generate main data content based on structure type
        let data_content = match payload.data.structure_type {
            DataStructureType::Flat => {
                if let Some(ref rows) = payload.data.rows {
                    self.generate_flat_table(rows, &visible_columns)
                } else {
                    self.generate_empty_message()
                }
            }
            DataStructureType::Grouped => {
                if let Some(ref groups) = payload.data.groups {
                    self.generate_grouped_table(
                        groups,
                        &visible_columns,
                        &payload.metadata.grouping,
                    )
                } else {
                    self.generate_empty_message()
                }
            }
            DataStructureType::HierarchicalGrouped => {
                if let Some(ref groups) = payload.data.groups {
                    self.generate_hierarchical_table(
                        groups,
                        &visible_columns,
                        &payload.metadata.grouping,
                    )
                } else {
                    self.generate_empty_message()
                }
            }
            DataStructureType::Statement => {
                self.generate_statement_table(&payload.data, &visible_columns)
            }
        };

        // Generate grand total if enabled
        let grand_total_content = if payload.metadata.show_grand_total {
            let totals = payload
                .data
                .grand_total
                .as_ref()
                .or(payload.data.totals.as_ref());

            if let Some(gt) = totals {
                self.generate_grand_total(gt, &visible_columns, payload.data.total_records)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Build the complete document
        let content = format!(
            r##"#set document(title: "{title}", author: "Facturazo ERP")
#set page({page_config}, numbering: "1 / 1")
#set text(font: "Inter", size: {font_size}pt)
#set par(justify: false)

// Header
{header_section}

// Filters
{filters_section}

// Data
{data_content}

// Grand Total
{grand_total}

// Footer
#v(1fr)
#line(length: 100%, stroke: 0.5pt + rgb(200, 200, 200))
#v(0.3em)
#grid(columns: (1fr, 1fr), gutter: 0pt,
  [#text(size: 0.8em, fill: gray)[Generado: {generated_at}]],
  [#align(right)[#text(size: 0.8em, fill: gray)[Pagina #context counter(page).display() de #context counter(page).final().at(0)]]]
)"##,
            title = utils::escape_typst(&payload.report.title),
            page_config = page_config,
            font_size = font_size,
            header_section = if payload.output.include_header {
                self.generate_header_section(
                    &payload.report,
                    &payload.company_info,
                    payload.output.show_logo,
                )
            } else {
                String::new()
            },
            filters_section = self.generate_filters_display(&payload.report),
            data_content = data_content,
            grand_total = grand_total_content,
            generated_at = payload.report.generated_at,
        );

        Ok(content)
    }

    fn template_id(&self) -> &str {
        "erp_report"
    }

    fn validate(&self, data: &Value) -> Result<()> {
        if !data.is_object() {
            anyhow::bail!("Los datos deben ser un objeto JSON");
        }

        let obj = data.as_object().unwrap();

        // Required fields
        if !obj.contains_key("report") {
            anyhow::bail!("Campo requerido faltante: report");
        }

        if !obj.contains_key("metadata") {
            anyhow::bail!("Campo requerido faltante: metadata");
        }

        if !obj.contains_key("data") {
            anyhow::bail!("Campo requerido faltante: data");
        }

        if !obj.contains_key("output") {
            anyhow::bail!("Campo requerido faltante: output");
        }

        Ok(())
    }

    fn description(&self) -> &str {
        "Reporte Generico ERP - Soporta todos los tipos de reportes del sistema"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_currency() {
        let template = ErpReportTemplate::new();
        assert_eq!(template.format_currency(1234.56), "1,234.56");
        assert_eq!(template.format_currency(-500.00), "(500.00)");
        assert_eq!(template.format_currency(0.0), "-");
    }

    #[test]
    fn test_format_date() {
        let template = ErpReportTemplate::new();
        assert_eq!(template.format_date("2025-12-15"), "15/12/2025");
        assert_eq!(template.format_date("2025-12-15T04:30:00Z"), "15/12/2025");
    }

    #[test]
    fn test_generate_simple_report() {
        let template = ErpReportTemplate::new();

        let json = serde_json::json!({
            "documentType": "REPORT",
            "tenantId": 1,
            "userId": 1,
            "report": {
                "code": "TEST",
                "title": "Test Report",
                "generatedAt": "2025-12-15T04:30:00Z"
            },
            "metadata": {
                "columns": [
                    { "key": "name", "label": "Name", "type": "String" },
                    { "key": "amount", "label": "Amount", "type": "Currency", "align": "Right" }
                ],
                "showGrandTotal": true
            },
            "data": {
                "structureType": "Flat",
                "rows": [
                    { "name": "Item 1", "amount": 100.50 },
                    { "name": "Item 2", "amount": 200.75 }
                ],
                "grandTotal": { "amount": 301.25 },
                "totalRecords": 2
            },
            "output": {
                "format": "Pdf",
                "pageSize": "Letter",
                "orientation": "Portrait"
            }
        });

        let result = template.generate(&json);
        assert!(result.is_ok());

        let content = result.unwrap();
        assert!(content.contains("Test Report"));
        assert!(content.contains("table"));
    }

    #[test]
    fn test_validate() {
        let template = ErpReportTemplate::new();

        // Missing report field
        let invalid = serde_json::json!({
            "metadata": {},
            "data": {},
            "output": {}
        });
        assert!(template.validate(&invalid).is_err());

        // Valid structure
        let valid = serde_json::json!({
            "report": {},
            "metadata": {},
            "data": {},
            "output": {}
        });
        assert!(template.validate(&valid).is_ok());
    }
}
