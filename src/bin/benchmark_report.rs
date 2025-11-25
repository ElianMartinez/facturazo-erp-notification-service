use chrono::Local;
use std::fs::{self, create_dir_all};
use std::process::Command;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Generador de Reportes de Benchmark");

    create_dir_all("reportes")?;

    let test_sizes = vec![5000, 10000, 20000];

    for size in test_sizes {
        println!("\nGenerando reporte con {} filas...", size);

        let start = Instant::now();

        let date = Local::now().format("%d/%m/%Y %H:%M:%S").to_string();

        let mut table_rows = String::new();
        for i in 1..=size {
            table_rows.push_str(&format!(
                "  [{}], [Cliente {}], [Factura-{:05}], [RD$ {:.2}], [RD$ {:.2}],\n",
                i,
                i,
                i,
                1000.0 + (i as f64 * 10.0),
                (1000.0 + (i as f64 * 10.0)) * 1.18
            ));
        }

        let typst_content = format!(
            r#"
#set page(
  paper: "us-letter",
  margin: (top: 1cm, bottom: 1cm, left: 1cm, right: 1cm),
)

#set text(font: "Arial", size: 9pt)

#align(center)[
  #text(size: 14pt, weight: "bold")[REPORTE DE FACTURACIÓN - BENCHMARK]

  #text(size: 10pt)[Generado: {}]
]

#text(size: 11pt, weight: "bold")[Resumen Ejecutivo]

Este reporte contiene {} registros de facturación para pruebas de rendimiento.

#table(
  columns: (0.5fr, 2fr, 2fr, 1.5fr, 1.5fr),
  stroke: 0.5pt + gray,
  inset: 5pt,

  table.header(
    [*#*], [*Cliente*], [*Factura*], [*Subtotal*], [*Total*],
  ),

{}
)

#align(right)[
  #text(size: 10pt)[
    *Total de registros:* {}

    *Tiempo de procesamiento:* Medido en aplicación
  ]
]
"#,
            date, size, table_rows, size
        );

        let typst_filename = format!("temp_benchmark_{}.typ", size);
        fs::write(&typst_filename, typst_content)?;

        let pdf_filename = format!("reportes/benchmark_{}rows.pdf", size);

        let output = Command::new("typst")
            .arg("compile")
            .arg(&typst_filename)
            .arg(&pdf_filename)
            .output()?;

        if !output.status.success() {
            eprintln!(
                "Error al compilar Typst: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            continue;
        }

        fs::remove_file(&typst_filename)?;

        let duration = start.elapsed();
        let file_size = fs::metadata(&pdf_filename)?.len();

        println!("✓ Reporte generado: {}", pdf_filename);
        println!("  Tiempo: {:.2?}", duration);
        println!("  Tamaño: {:.2} MB", file_size as f64 / 1_048_576.0);
    }

    println!("\n¡Benchmark completado exitosamente!");

    Ok(())
}
