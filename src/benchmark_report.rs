use std::fs;
use std::process::Command;
use std::time::Instant;
use chrono::Utc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Benchmark de Generación de Reportes de Facturación");
    println!("=====================================================");

    // Crear directorio para los reportes
    fs::create_dir_all("reportes")?;

    // Probar diferentes tamaños
    let sizes = vec![5000, 10000, 20000];

    for size in sizes {
        println!("\n📊 Generando reporte con {} filas...", size);
        let start = Instant::now();

        generate_large_report(size)?;

        let duration = start.elapsed();
        println!("✅ Reporte de {} filas generado en: {:.2} segundos", size, duration.as_secs_f64());

        // Obtener tamaño del archivo
        let file_path = format!("reportes/reporte_{}_filas.pdf", size);
        let metadata = fs::metadata(&file_path)?;
        let file_size = metadata.len() as f64 / (1024.0 * 1024.0);
        println!("📁 Tamaño del archivo: {:.2} MB", file_size);
    }

    Ok(())
}

fn generate_large_report(rows: usize) -> Result<(), Box<dyn std::error::Error>> {
    let report_typst = generate_report_typst(rows);
    let typ_filename = format!("reportes/reporte_{}_filas.typ", rows);
    let pdf_filename = format!("reportes/reporte_{}_filas.pdf", rows);

    // Guardar el archivo Typst
    fs::write(&typ_filename, &report_typst)?;

    // Compilar usando typst
    let output = Command::new("typst")
        .args(&["compile", &typ_filename, &pdf_filename])
        .output()?;

    // Limpiar archivo temporal
    let _ = fs::remove_file(&typ_filename);

    if !output.status.success() {
        return Err(format!("Error compilando Typst: {}",
                          String::from_utf8_lossy(&output.stderr)).into());
    }

    Ok(())
}

fn generate_report_typst(rows: usize) -> String {
    let mut typst_content = String::from(r#"#set page(
  paper: "us-letter",
  margin: (top: 1cm, bottom: 1cm, left: 0.5cm, right: 0.5cm),
  header: [
    #rect(width: 100%, fill: rgb(240, 246, 252), stroke: none, inset: 5pt)[
      #grid(
        columns: (1fr, 1fr),
        [
          #text(size: 8pt, weight: "bold", fill: rgb(30, 60, 90))[Reporte de Facturación]
        ],
        [
          #align(right)[
            #text(size: 7pt, fill: rgb(60, 60, 60))[Página #context counter(page).display()]
          ]
        ]
      )
    ]
    #v(3pt)
  ]
)
#set text(font: "Helvetica", size: 5.5pt, fill: rgb(40, 40, 40))

// Título del reporte con estilo mejorado
#align(center)[
  #text(size: 12pt, weight: "bold", fill: rgb(25, 50, 80))[REPORTE DE FACTURACIÓN MENSUAL]
  #v(3pt)
  #rect(width: 250pt, fill: none, stroke: (bottom: 1.5pt + rgb(25, 50, 80)))[
    #text(size: 7pt, fill: rgb(60, 60, 60))[Período: Enero 2024 | Total de registros: "#);

    typst_content.push_str(&format!("{}", rows));
    typst_content.push_str(r#"]
  ]
]

#v(5pt)

// Resumen ejecutivo con gradiente
#rect(width: 100%, fill: gradient.linear(rgb(245, 250, 255), rgb(235, 245, 255)), stroke: 0.5pt + rgb(180, 200, 220), radius: 3pt)[
  #pad(6pt)[
    #grid(
      columns: (1fr, 1fr, 1fr),
      column-gutter: 10pt,
      [
        #rect(width: 100%, fill: white, stroke: none, radius: 2pt, inset: 4pt)[
          #text(size: 6pt, weight: "bold", fill: rgb(40, 80, 120))[Total Facturado]
          #v(2pt)
          #text(size: 8pt, weight: "bold", fill: rgb(20, 40, 60))[\"#);

    let total = calculate_total(rows);
    typst_content.push_str(&format_money(total));

    typst_content.push_str(r#"]
        ]
      ],
      [
        #rect(width: 100%, fill: white, stroke: none, radius: 2pt, inset: 4pt)[
          #text(size: 6pt, weight: "bold", fill: rgb(40, 80, 120))[Promedio por Factura]
          #v(2pt)
          #text(size: 8pt, weight: "bold", fill: rgb(20, 40, 60))[\"#);

    typst_content.push_str(&format_money(total / rows as f64));

    typst_content.push_str(r#"]
        ]
      ],
      [
        #rect(width: 100%, fill: white, stroke: none, radius: 2pt, inset: 4pt)[
          #text(size: 6pt, weight: "bold", fill: rgb(40, 80, 120))[Facturas Procesadas]
          #v(2pt)
          #text(size: 8pt, weight: "bold", fill: rgb(20, 40, 60))["#);

    typst_content.push_str(&format!("{}", rows));

    typst_content.push_str(r#"]
        ]
      ]
    )
  ]
]

#v(5pt)

// Tabla de datos con ancho completo automático
#table(
  columns: (3%, 14%, 7%, 15%, 11%, 4%, 12%, 11%, 11%, 12%),
  stroke: (x, y) => {
    if y == 0 { (bottom: 1.5pt + rgb(25, 50, 80)) }
    else { 0.3pt + rgb(220, 220, 220) }
  },
  fill: (x, y) => {
    if y == 0 { gradient.linear(rgb(35, 70, 110), rgb(25, 50, 85)) }
    else if calc.odd(y) { rgb(248, 251, 254) }
    else { white }
  },
  align: (col, row) => {
    if row == 0 { center }
    else if col == 0 or col == 5 { center }
    else if col == 2 { center }
    else if col >= 6 { right }
    else { left }
  },
  inset: (x, y) => {
    if y == 0 { 3pt }
    else { 2pt }
  },

  // Encabezados con estilo mejorado
  [#text(size: 5pt, fill: white, weight: "bold")[No.]],
  [#text(size: 5pt, fill: white, weight: "bold")[NCF]],
  [#text(size: 5pt, fill: white, weight: "bold")[Fecha]],
  [#text(size: 5pt, fill: white, weight: "bold")[Cliente]],
  [#text(size: 5pt, fill: white, weight: "bold")[RNC]],
  [#text(size: 5pt, fill: white, weight: "bold")[Items]],
  [#text(size: 5pt, fill: white, weight: "bold")[Subtotal]],
  [#text(size: 5pt, fill: white, weight: "bold")[ITBIS]],
  [#text(size: 5pt, fill: white, weight: "bold")[Desc.]],
  [#text(size: 5pt, fill: white, weight: "bold")[Total]],
"#);

    // Generar las filas de datos
    for i in 1..=rows {
        let (ncf, fecha, cliente, rnc, items, subtotal, itbis, descuento, total) = generate_row_data(i);

        // Formatear cada celda individualmente para evitar problemas
        typst_content.push_str("\n  ");
        typst_content.push_str(&format!("[{}], ", i));
        typst_content.push_str(&format!("[{}], ", ncf));
        typst_content.push_str(&format!("[{}], ", fecha));
        typst_content.push_str(&format!("[{}], ", cliente));
        typst_content.push_str(&format!("[{}], ", rnc));
        typst_content.push_str(&format!("[{}], ", items));
        typst_content.push_str(&format!("[{}], ", format_money_simple(subtotal)));
        typst_content.push_str(&format!("[{}], ", format_money_simple(itbis)));
        typst_content.push_str(&format!("[{}], ", format_money_simple(descuento)));
        typst_content.push_str(&format!("[{}],", format_money_simple(total)));
    }

    typst_content.push_str(r#"
)

#v(8pt)

// Pie de página con totales estilizado
#align(right)[
  #rect(width: 240pt, fill: gradient.linear(rgb(250, 252, 255), rgb(245, 248, 252)), stroke: 0.5pt + rgb(180, 200, 220), radius: 4pt)[
    #pad(6pt)[
      #grid(
        columns: (140pt, 80pt),
        row-gutter: 3pt,
        [#text(size: 6pt, weight: "bold", fill: rgb(60, 60, 60))[Subtotal General:]], [#align(right)[#text(size: 6pt, fill: rgb(40, 40, 40))[\"#);

    let subtotal_general = calculate_subtotal(rows);
    typst_content.push_str(&format_money(subtotal_general));

    typst_content.push_str(r#"]]],
        [#text(size: 6pt, weight: "bold", fill: rgb(60, 60, 60))[ITBIS Total (18%):]], [#align(right)[#text(size: 6pt, fill: rgb(40, 40, 40))[\"#);

    let itbis_total = subtotal_general * 0.18;
    typst_content.push_str(&format_money(itbis_total));

    typst_content.push_str(r#"]]],
        [#text(size: 6pt, weight: "bold", fill: rgb(60, 60, 60))[Descuentos Totales:]], [#align(right)[#text(size: 6pt, fill: rgb(200, 50, 50))[\"#);

    let descuentos = subtotal_general * 0.05;
    typst_content.push_str(&format_money(descuentos));

    typst_content.push_str(r#"]]],
        [#line(length: 100%, stroke: 0.8pt + rgb(150, 150, 150))], [#line(length: 100%, stroke: 0.8pt + rgb(150, 150, 150))],
        [#text(size: 8pt, weight: "bold", fill: rgb(25, 50, 80))[TOTAL GENERAL:]], [#align(right)[#text(size: 8pt, weight: "bold", fill: rgb(25, 50, 80))[\"#);

    typst_content.push_str(&format_money(total));

    typst_content.push_str(r#"]]]
      )
    ]
  ]
]

#v(5pt)
#align(center)[
  #text(size: 5pt, fill: rgb(120, 120, 120), style: "italic")[
    Reporte generado automáticamente el "#);

    typst_content.push_str(&Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string());

    typst_content.push_str(r#"
  ]
]"#);

    typst_content
}

fn generate_row_data(index: usize) -> (String, String, String, String, usize, f64, f64, f64, f64) {
    // Generar datos pseudo-aleatorios basados en el índice
    let ncf = format!("E31000{:07}", index);
    let day = ((index - 1) % 28) + 1;
    let fecha = format!("{:02}/01/24", day);

    let clientes = vec!["ABC Corp", "XYZ Ltd", "Serv 123", "Dist LMN", "QRS Shop"];
    let cliente = clientes[index % clientes.len()].to_string();

    let rnc = format!("1{:08}", 30000000 + index);

    let items = (index % 10) + 1;

    // Generar montos variados
    let base_amount = 1000.0 + (index as f64 * 123.45) % 50000.0;
    let subtotal = base_amount;
    let itbis = subtotal * 0.18;
    let descuento = if index % 3 == 0 { subtotal * 0.05 } else { 0.0 };
    let total = subtotal + itbis - descuento;

    (ncf, fecha, cliente, rnc, items, subtotal, itbis, descuento, total)
}

fn calculate_subtotal(rows: usize) -> f64 {
    let mut total = 0.0;
    for i in 1..=rows {
        let base_amount = 1000.0 + (i as f64 * 123.45) % 50000.0;
        total += base_amount;
    }
    total
}

fn calculate_total(rows: usize) -> f64 {
    let subtotal = calculate_subtotal(rows);
    let itbis = subtotal * 0.18;
    let descuentos = subtotal * 0.05;
    subtotal + itbis - descuentos
}

fn format_money(amount: f64) -> String {
    // Formato con comas para el resumen
    let formatted = format!("{:.2}", amount);
    let parts: Vec<&str> = formatted.split('.').collect();
    let integer = parts[0];
    let decimal = parts.get(1).unwrap_or(&"00");

    let mut result = String::new();
    for (i, c) in integer.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }

    format!("${}.{}", result.chars().rev().collect::<String>(), decimal)
}

fn format_money_simple(amount: f64) -> String {
    // Formato con comas pero sin símbolo $ para la tabla
    let formatted = format!("{:.2}", amount);
    let parts: Vec<&str> = formatted.split('.').collect();
    let integer = parts[0];
    let decimal = parts.get(1).unwrap_or(&"00");

    let mut result = String::new();
    for (i, c) in integer.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }

    format!("{}.{}", result.chars().rev().collect::<String>(), decimal)
}