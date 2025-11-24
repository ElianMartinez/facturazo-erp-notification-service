//! Report PDF generator with table support

use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;
use super::{DocumentGenerator, DocumentType, TypstGenerator};

/// Report PDF generator
pub struct ReportGenerator {
    typst_generator: TypstGenerator,
}

impl ReportGenerator {
    pub fn new(work_dir: PathBuf) -> Self {
        Self {
            typst_generator: TypstGenerator::new(work_dir),
        }
    }

    /// Create default report template
    fn create_report_template() -> &'static str {
        r#"
#set page(
  paper: "us-letter",
  margin: (top: 2cm, bottom: 2cm, left: 1.5cm, right: 1.5cm),
  numbering: "1 / 1",
)

#set text(font: "Arial", size: 10pt)

// Header
#align(center)[
  #text(size: 18pt, weight: "bold")[{{report_title}}]
  #v(0.5em)
  {{#if report_subtitle}}#text(size: 14pt)[{{report_subtitle}}]{{/if}}
  #v(0.5em)
  #text(size: 10pt, fill: gray)[
    Generado: {{generated_date}} | Período: {{period_start}} - {{period_end}}
  ]
]

#line(length: 100%, stroke: 1pt)
#v(1em)

// Executive Summary
{{#if executive_summary}}
#rect(width: 100%, inset: 10pt, fill: rgb("#f5f5f5"))[
  #text(weight: "bold", size: 12pt)[Resumen Ejecutivo]
  #v(0.5em)
  {{executive_summary}}
]
#v(1em)
{{/if}}

// Key Metrics
{{#if metrics}}
#grid(
  columns: (1fr, 1fr, 1fr, 1fr),
  gutter: 1em,

  {{#each metrics}}
  #rect(width: 100%, inset: 8pt, stroke: 0.5pt)[
    #text(size: 8pt, fill: gray)[{{label}}] \
    #text(size: 14pt, weight: "bold")[{{value}}] \
    {{#if change}}
    #text(size: 8pt, fill: {{#if positive}}green{{else}}red{{/if}})[{{change}}]
    {{/if}}
  ],
  {{/each}}
)
#v(1em)
{{/if}}

// Main Table
{{#if table_data}}
#text(weight: "bold", size: 12pt)[{{table_title}}]
#v(0.5em)

#table(
  columns: {{table_columns}},
  inset: 6pt,
  align: {{table_align}},

  // Header
  table.header(
    {{#each table_headers}}
    [*{{this}}*],
    {{/each}}
  ),

  // Data rows
  {{#each table_rows}}
  {{#each this}}
  [{{this}}],
  {{/each}}
  {{/each}}

  // Footer/Totals
  {{#if table_totals}}
  table.hline(stroke: 1.5pt),
  {{#each table_totals}}
  [#text(weight: "bold")[{{this}}]],
  {{/each}}
  {{/if}}
)
{{/if}}

// Charts placeholder
{{#if chart_data}}
#v(1em)
#align(center)[
  #rect(width: 80%, height: 200pt, stroke: 0.5pt)[
    #align(center + horizon)[
      #text(fill: gray)[Gráfico: {{chart_title}}] \
      #text(size: 8pt, fill: gray)[(Los gráficos se generarán en futuras versiones)]
    ]
  ]
]
{{/if}}

// Additional sections
{{#each sections}}
#v(1em)
#text(weight: "bold", size: 12pt)[{{title}}]
#v(0.5em)
{{content}}
{{/each}}

// Footer
#v(1fr)
#line(length: 100%, stroke: 0.5pt)
#grid(
  columns: (1fr, 1fr, 1fr),
  align: (left, center, right),

  [
    #text(size: 8pt, fill: gray)[
      {{#if company_name}}{{company_name}}{{/if}}
    ]
  ],
  [
    #text(size: 8pt, fill: gray)[
      Página #counter(page) de #context counter(page).final()
    ]
  ],
  [
    #text(size: 8pt, fill: gray)[
      {{#if report_id}}Reporte ID: {{report_id}}{{/if}}
    ]
  ]
)
        "#
    }
}

#[async_trait::async_trait]
impl DocumentGenerator for ReportGenerator {
    async fn generate(&self, template: &str, data: &Value) -> Result<Vec<u8>> {
        // Process data for report formatting
        let mut report_data = data.clone();

        // Add default values if missing
        if let Some(obj) = report_data.as_object_mut() {
            // Add generated date if not present
            if !obj.contains_key("generated_date") {
                obj.insert(
                    "generated_date".to_string(),
                    serde_json::json!(chrono::Utc::now().format("%d/%m/%Y %H:%M").to_string())
                );
            }

            // Format table columns if present
            if let Some(headers) = obj.get("table_headers").and_then(|v| v.as_array()) {
                let columns = headers.iter()
                    .map(|_| "1fr")
                    .collect::<Vec<_>>()
                    .join(", ");

                obj.insert(
                    "table_columns".to_string(),
                    serde_json::json!(format!("({})", columns))
                );

                obj.insert(
                    "table_align".to_string(),
                    serde_json::json!("center")
                );
            }

            // Format numeric values in table
            if let Some(rows) = obj.get_mut("table_rows").and_then(|v| v.as_array_mut()) {
                for row in rows {
                    if let Some(cells) = row.as_array_mut() {
                        for cell in cells {
                            // Format numbers with thousands separator
                            if let Some(num) = cell.as_f64() {
                                *cell = serde_json::json!(format!("{:,.2}", num));
                            }
                        }
                    }
                }
            }
        }

        // Use provided template or default
        let final_template = if template.is_empty() {
            Self::create_report_template()
        } else {
            template
        };

        self.typst_generator.process(final_template, &report_data).await
    }

    fn supported_types(&self) -> Vec<DocumentType> {
        vec![DocumentType::Report]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_report_generation() {
        let temp_dir = TempDir::new().unwrap();
        let generator = ReportGenerator::new(temp_dir.path().to_path_buf());

        let report_data = serde_json::json!({
            "report_title": "Reporte de Ventas Mensual",
            "report_subtitle": "Le Croissant Doré - Noviembre 2024",
            "period_start": "01/11/2024",
            "period_end": "30/11/2024",

            "executive_summary": "Las ventas del mes de noviembre mostraron un crecimiento del 15% comparado con el mismo período del año anterior. Se registraron 1,250 transacciones con un ticket promedio de RD$850.",

            "metrics": [
                {
                    "label": "Ventas Totales",
                    "value": "RD$ 1,062,500",
                    "change": "+15%",
                    "positive": true
                },
                {
                    "label": "Transacciones",
                    "value": "1,250",
                    "change": "+8%",
                    "positive": true
                },
                {
                    "label": "Ticket Promedio",
                    "value": "RD$ 850",
                    "change": "+6.25%",
                    "positive": true
                },
                {
                    "label": "Productos Vendidos",
                    "value": "3,750",
                    "change": "+12%",
                    "positive": true
                }
            ],

            "table_title": "Ventas por Sucursal",
            "table_headers": ["Sucursal", "Transacciones", "Ventas", "Promedio", "% Total"],
            "table_rows": [
                ["Piantini", "450", "425000.00", "944.44", "40.0%"],
                ["Naco", "380", "342000.00", "900.00", "32.2%"],
                ["Blue Mall", "250", "187500.00", "750.00", "17.6%"],
                ["Bella Vista", "170", "108000.00", "635.29", "10.2%"]
            ],
            "table_totals": ["TOTAL", "1,250", "RD$ 1,062,500", "RD$ 850", "100%"],

            "sections": [
                {
                    "title": "Productos Más Vendidos",
                    "content": "1. Croissant de Chocolate (850 unidades)\n2. Café Cappuccino (720 unidades)\n3. Pan Baguette (650 unidades)"
                },
                {
                    "title": "Observaciones",
                    "content": "Se observó un incremento significativo en las ventas durante los fines de semana, especialmente en la sucursal de Blue Mall."
                }
            ],

            "company_name": "Le Croissant Doré SRL",
            "report_id": "RPT-2024-11-001"
        });

        // Try to generate (will fail if Typst not installed)
        match generator.generate("", &report_data).await {
            Ok(pdf) => {
                println!("Generated report PDF: {} bytes", pdf.len());
                assert!(!pdf.is_empty());
            }
            Err(e) => {
                println!("Could not generate report (Typst may not be installed): {}", e);
            }
        }
    }
}