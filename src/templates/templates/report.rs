use anyhow::{Result, Context};
use serde_json::Value;
use crate::templates::template_trait::{TypstTemplate, utils};
use crate::templates::template_models::{ReportData, ChartData};

pub struct ReportTemplate;

impl ReportTemplate {
    pub fn new() -> Self {
        Self
    }

    fn format_table_data(&self, data: &[HashMap<String, String>]) -> String {
        if data.is_empty() {
            return String::new();
        }

        // Obtener headers de la primera fila
        let headers: Vec<String> = if let Some(first_row) = data.first() {
            first_row.keys().cloned().collect()
        } else {
            return String::new();
        };

        // Generar encabezados
        let header_row = headers
            .iter()
            .map(|h| format!("[*{}*]", utils::escape_typst(h)))
            .collect::<Vec<_>>()
            .join(", ");

        // Generar filas de datos
        let data_rows = data
            .iter()
            .map(|row| {
                headers
                    .iter()
                    .map(|h| {
                        let value = row.get(h).map(|v| v.as_str()).unwrap_or("-");
                        format!("[{}]", utils::escape_typst(value))
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .collect::<Vec<_>>()
            .join(",\n  ");

        format!("{},\n  {}", header_row, data_rows)
    }

    fn format_summary(&self, summary: &crate::templates::template_models::ReportSummary) -> String {
        let mut items = Vec::new();

        // Formatear métricas
        for (key, value) in &summary.metrics {
            items.push(format!("[*{}:*], [{:.2}]", utils::escape_typst(key), value));
        }

        // Agregar highlights
        if !summary.highlights.is_empty() {
            let highlights = summary.highlights.join(", ");
            items.push(format!("[*Destacados:*], [{}]", utils::escape_typst(&highlights)));
        }

        items.join(",\n    ")
    }
}

use std::collections::HashMap;

impl TypstTemplate for ReportTemplate {
    fn generate(&self, data: &Value) -> Result<String> {
        let report: ReportData = serde_json::from_value(data.clone())
            .context("Error deserializando datos de reporte")?;

        let content = format!(r#"#set document(title: "{}", author: "Sistema de Reportes")
#set page(paper: "us-letter", margin: 2cm, numbering: "1 / 1")
#set text(font: "Arial", size: 10pt)
#set par(justify: true)

// Encabezado
#align(center)[
  #text(size: 18pt, weight: "bold")[{}]

  #v(5pt)
  #text(size: 10pt, fill: gray)[
    Generado: {} | Periodo: {} - {}
  ]
]

#v(10pt)
#line(length: 100%, stroke: 1pt + rgb(70, 130, 180))

// Resumen si existe
{}

// Datos del reporte
#v(15pt)
#text(size: 14pt, weight: "bold")[Datos del Reporte]
#v(8pt)

{}

// Charts si existen
{}

// Footer
#v(20pt)
#line(length: 100%, stroke: 0.5pt + gray)
#v(5pt)
#text(size: 8pt, fill: gray)[
  Documento generado automáticamente \
  Página #counter(page).display() de #context counter(page).final().at(0)
]"#,
            // Metadata
            report.title,
            // Header
            utils::escape_typst(&report.title),
            report.generated_date,
            report.period.start_date,
            report.period.end_date,
            // Summary si existe
            if let Some(ref summary) = report.summary {
                format!(r#"
#v(15pt)
#rect(width: 100%, fill: rgb(255, 250, 240), stroke: 1pt + rgb(255, 140, 0), radius: 3pt, inset: 10pt)[
  #text(size: 12pt, weight: "bold")[Resumen Ejecutivo]
  #v(5pt)
  #grid(
    columns: (120pt, 1fr),
    row-gutter: 3pt,
    {}
  )
]"#, self.format_summary(summary))
            } else {
                String::new()
            },
            // Tabla de datos
            if !report.data.is_empty() {
                format!(r#"#table(
  columns: {},
  stroke: 0.5pt + gray,
  fill: (x, y) => if y == 0 {{ rgb(240, 240, 240) }} else {{ white }},
  inset: 8pt,
  {}
)"#,
                    report.data.first().map(|r| r.len()).unwrap_or(2),
                    self.format_table_data(&report.data))
            } else {
                String::new()
            },
            // Charts placeholder
            if report.charts.is_some() {
                r#"
#v(15pt)
#text(size: 14pt, weight: "bold")[Visualizaciones]
#v(8pt)
#rect(width: 100%, height: 150pt, fill: rgb(250, 250, 250), stroke: 0.5pt + gray)[
  #align(center + horizon)[
    #text(fill: gray)[Gráficos disponibles en versión interactiva]
  ]
]"#
            } else {
                ""
            }
        );

        Ok(content)
    }

    fn template_id(&self) -> &str {
        "report"
    }

    fn validate(&self, data: &Value) -> Result<()> {
        if !data.is_object() {
            anyhow::bail!("Los datos deben ser un objeto JSON");
        }

        let obj = data.as_object().unwrap();

        if !obj.contains_key("title") {
            anyhow::bail!("Campo requerido faltante: title");
        }

        if !obj.contains_key("generated_date") {
            anyhow::bail!("Campo requerido faltante: generated_date");
        }

        if !obj.contains_key("period") {
            anyhow::bail!("Campo requerido faltante: period");
        }

        Ok(())
    }

    fn description(&self) -> &str {
        "Reporte General con Datos y Resumen"
    }
}