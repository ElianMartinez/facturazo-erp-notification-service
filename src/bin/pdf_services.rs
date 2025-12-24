use chrono::Local;
use std::fs::{self, create_dir_all};
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Generador de Facturas Fiscales Electrónicas");

    create_dir_all("facturas")?;

    let invoice_number = format!("E31000000{:03}", 1);
    let date = Local::now().format("%d/%m/%Y").to_string();

    let _qr_url = format!(
        "https://dgii.gov.do/verificarNCF?RNC=133478341&NCF={}&Monto=3540.00",
        invoice_number
    );

    let typst_content = format!(
        r#"
#set page(
  paper: "us-letter",
  margin: (top: 1.5cm, bottom: 1.5cm, left: 1.5cm, right: 1.5cm),
)

#set text(font: "Arial", size: 10pt)

#align(center)[
  #text(size: 16pt, weight: "bold")[FACTURA FISCAL ELECTRÓNICA]
]

#grid(
  columns: (1fr, 1fr),
  align: (left, right),
  [
    *Fecha:* {}

    *Cliente:* Cliente Ejemplo

    *RNC:* 123456789
  ],
  [
    *NCF:* {}

    *Válido hasta:* 31/12/2025

    *Código QR:* Verificación DGII
  ]
)

#table(
  columns: (1fr, 3fr, 1fr, 1fr, 1fr),
  stroke: 0.5pt + gray,
  inset: 8pt,

  table.header(
    [*Cant.*], [*Descripción*], [*Precio*], [*ITBIS*], [*Total*],
  ),

  [1], [Servicio de Desarrollo Web], [RD$ 3,000.00], [RD$ 540.00], [RD$ 3,540.00],
)

#align(right)[
  #table(
    columns: (150pt, 100pt),
    stroke: none,
    align: (right, right),
    [*Subtotal:*], [RD$ 3,000.00],
    [*ITBIS (18%):*], [RD$ 540.00],
    table.hline(stroke: 1.5pt),
    [*Total:*], [*RD$ 3,540.00*],
  )
]

#place(
  center + horizon,
  float: true,
  clearance: 3em,
  text(size: 72pt, fill: rgb(255, 0, 0, 30), weight: "bold")[PAGADA]
)
"#,
        date, invoice_number
    );

    let typst_filename = format!("temp_factura_{}.typ", invoice_number);
    fs::write(&typst_filename, typst_content)?;

    let pdf_filename = format!("facturas/factura_fiscal_{}.pdf", 1);

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
        return Err("Error al generar PDF".into());
    }

    fs::remove_file(&typst_filename)?;

    println!("Factura fiscal generada exitosamente: {}", pdf_filename);

    Ok(())
}
