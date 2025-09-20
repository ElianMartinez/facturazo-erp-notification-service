use anyhow::Result;
use rust_xlsxwriter::{Workbook, Format, Color, FormatBorder};
use serde_json::Value;

/// Generador genérico de Excel
pub struct ExcelGenerator;

impl ExcelGenerator {
    pub fn new() -> Self {
        ExcelGenerator
    }

    /// Genera un archivo Excel desde datos JSON genéricos
    pub async fn generate(&self, data: Value) -> Result<Vec<u8>> {
        // Procesar en tarea bloqueante para trabajo intensivo de CPU
        tokio::task::spawn_blocking(move || {
            Self::generate_excel_from_json(data)
        })
        .await?
    }

    fn generate_excel_from_json(data: Value) -> Result<Vec<u8>> {
        let mut workbook = Workbook::new();

        // Extraer configuración básica del JSON
        let title = data["title"].as_str().unwrap_or("Sheet1");
        let headers = data["headers"].as_array();
        let rows = data["rows"].as_array();
        let use_memory_optimization = data["memory_optimization"].as_bool().unwrap_or(false);

        // Optimización de memoria para archivos grandes - comentado temporalmente
        // if use_memory_optimization {
        //     workbook.use_constant_memory(true)?;
        // }

        // Crear hoja de trabajo
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(title)?;

        // Crear formato para encabezados
        let header_format = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0x4472C4))
            .set_font_color(Color::White)
            .set_border(FormatBorder::Thin);

        // Crear formato para celdas normales
        let cell_format = Format::new()
            .set_border(FormatBorder::Thin);

        // Escribir encabezados si existen
        if let Some(headers) = headers {
            for (col, header) in headers.iter().enumerate() {
                let header_text = header.as_str().unwrap_or("");
                worksheet.write_string_with_format(0, col as u16, header_text, &header_format)?;
            }
        }

        // Escribir filas de datos si existen
        if let Some(rows) = rows {
            for (row_idx, row) in rows.iter().enumerate() {
                let row_num = (row_idx + 1) as u32; // +1 para el header

                if let Some(row_array) = row.as_array() {
                    for (col_idx, value) in row_array.iter().enumerate() {
                        let col_num = col_idx as u16;

                        // Escribir valor según su tipo
                        match value {
                            Value::Number(n) => {
                                worksheet.write_number_with_format(
                                    row_num,
                                    col_num,
                                    n.as_f64().unwrap_or(0.0),
                                    &cell_format
                                )?;
                            },
                            Value::String(s) => {
                                worksheet.write_string_with_format(
                                    row_num,
                                    col_num,
                                    s,
                                    &cell_format
                                )?;
                            },
                            Value::Bool(b) => {
                                worksheet.write_string_with_format(
                                    row_num,
                                    col_num,
                                    &b.to_string(),
                                    &cell_format
                                )?;
                            },
                            _ => {
                                worksheet.write_string_with_format(
                                    row_num,
                                    col_num,
                                    &value.to_string(),
                                    &cell_format
                                )?;
                            }
                        }
                    }
                }
            }
        }

        // Aplicar opciones adicionales si existen
        if let Some(options) = data["options"].as_object() {
            if options.get("freeze_headers").and_then(|v| v.as_bool()).unwrap_or(false) {
                worksheet.set_freeze_panes(1, 0)?;
            }

            if options.get("auto_filter").and_then(|v| v.as_bool()).unwrap_or(false) {
                if let (Some(headers), Some(rows)) = (headers, rows) {
                    let last_col = headers.len() as u16 - 1;
                    let last_row = rows.len() as u32;
                    worksheet.autofilter(0, 0, last_row, last_col)?;
                }
            }

            // Ajustar anchos de columna si se especifican
            if let Some(widths) = options.get("column_widths").and_then(|v| v.as_array()) {
                for (idx, width) in widths.iter().enumerate() {
                    if let Some(w) = width.as_f64() {
                        worksheet.set_column_width(idx as u16, w)?;
                    }
                }
            }
        }

        // Guardar en buffer
        let buffer = workbook.save_to_buffer()?;

        Ok(buffer)
    }


    /// Genera un Excel simple desde arrays de headers y rows
    pub async fn generate_simple(
        &self,
        title: &str,
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    ) -> Result<Vec<u8>> {
        let data = serde_json::json!({
            "title": title,
            "headers": headers,
            "rows": rows.into_iter().map(|row| Value::Array(
                row.into_iter().map(Value::String).collect()
            )).collect::<Vec<_>>(),
            "options": {
                "freeze_headers": true,
                "auto_filter": true
            }
        });

        self.generate(data).await
    }
}
