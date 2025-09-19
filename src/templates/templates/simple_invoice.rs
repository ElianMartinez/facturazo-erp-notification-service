use anyhow::{Result, Context};
use serde_json::Value;
use crate::templates::template_trait::{TypstTemplate, utils};
use crate::templates::template_models::{InvoiceData, InvoiceItem};

pub struct SimpleInvoiceTemplate;

impl SimpleInvoiceTemplate {
    pub fn new() -> Self {
        Self
    }

    fn format_items(&self, items: &[InvoiceItem]) -> String {
        items
            .iter()
            .map(|item| {
                let total = item.quantity * item.unit_price;
                format!(
                    "  [{}], [{}], [{:.2}], [{:.2}]",
                    utils::escape_typst(&item.description),
                    item.quantity,
                    item.unit_price,
                    total
                )
            })
            .collect::<Vec<_>>()
            .join(",\n")
    }
}

impl TypstTemplate for SimpleInvoiceTemplate {
    fn generate(&self, data: &Value) -> Result<String> {
        let invoice: InvoiceData = serde_json::from_value(data.clone())
            .context("Error deserializando datos de factura simple")?;

        let company = &invoice.company_info;
        let client = &invoice.client_info;
        let totals = &invoice.totals;

        let content = format!(r#"#set document(title: "Factura - {}", author: "{}")
#set page(paper: "us-letter", margin: 2cm)
#set text(font: "Arial", size: 11pt)

// Encabezado
#align(center)[
  #text(size: 18pt, weight: "bold")[{}]

  #text(size: 10pt)[
    {} \
    Tel: {} | Email: {}
  ]
]

#v(10pt)
#align(center)[
  #text(size: 14pt, weight: "bold")[FACTURA]
]

#v(15pt)

// Información de factura
#grid(
  columns: (1fr, 1fr),
  [
    #text(weight: "bold")[Factura No:] {} \
    #text(weight: "bold")[Fecha:] {}
  ],
  [
    #align(right)[
      #text(weight: "bold")[Vencimiento:] {}
    ]
  ]
)

#v(15pt)

// Información del cliente
#rect(width: 100%, fill: rgb(245, 245, 245), stroke: 0.5pt + gray, radius: 3pt, inset: 10pt)[
  #text(weight: "bold")[Cliente:] {} \
  #text(weight: "bold")[RNC/ID:] {} \
  {}
]

#v(15pt)

// Tabla de productos
#table(
  columns: (1fr, 60pt, 80pt, 80pt),
  stroke: 0.5pt + gray,
  fill: (x, y) => if y == 0 {{ rgb(230, 230, 230) }} else {{ white }},
  align: (col, row) => if col == 0 {{ left }} else {{ right }},
  inset: 8pt,

  [*Descripción*], [*Cantidad*], [*Precio*], [*Total*],
{}
)

#v(15pt)

// Totales
#align(right)[
  #grid(
    columns: (100pt, 80pt),
    row-gutter: 3pt,
    align: (right, right),
    [Subtotal:], [{} {:.2}],
    [Impuestos:], [{} {:.2}],
    [#text(weight: "bold")[Total:]], [#text(weight: "bold")[{} {:.2}]]
  )
]

{}

#v(30pt)
#align(center)[
  #text(size: 9pt, fill: gray)[¡Gracias por su compra!]
]"#,
            // Metadata
            invoice.invoice_number,
            company.name,
            // Header
            utils::escape_typst(&company.name),
            utils::escape_typst(&format!("{}, {}", company.address.city, company.address.country)),
            company.phone.as_deref().unwrap_or(""),
            utils::escape_typst(company.email.as_deref().unwrap_or("")),
            // Invoice info
            invoice.invoice_number,
            invoice.issue_date,
            invoice.due_date,
            // Client info
            utils::escape_typst(&client.name),
            client.tax_id,
            if let Some(address) = &client.address {
                format!("Dirección: {}", utils::escape_typst(&format!("{}, {}",
                    address.street, address.city)))
            } else {
                String::new()
            },
            // Items
            self.format_items(&invoice.items),
            // Totals
            totals.currency, totals.subtotal,
            totals.currency, totals.tax_amount,
            totals.currency, totals.total,
            // Notes
            if let Some(notes) = &invoice.notes {
                format!("\n#v(15pt)\n#text(size: 10pt)[*Notas:* {}]", utils::escape_typst(notes))
            } else {
                String::new()
            }
        );

        Ok(content)
    }

    fn template_id(&self) -> &str {
        "simple_invoice"
    }

    fn validate(&self, data: &Value) -> Result<()> {
        if !data.is_object() {
            anyhow::bail!("Los datos deben ser un objeto JSON");
        }

        let obj = data.as_object().unwrap();
        let required = vec!["invoice_number", "company_info", "client_info", "items", "totals"];

        for field in required {
            if !obj.contains_key(field) {
                anyhow::bail!("Campo requerido faltante: {}", field);
            }
        }

        Ok(())
    }

    fn description(&self) -> &str {
        "Factura Simple (sin información fiscal)"
    }
}