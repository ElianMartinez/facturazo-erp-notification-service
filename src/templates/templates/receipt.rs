use anyhow::{Result, Context};
use serde_json::Value;
use crate::templates::template_trait::{TypstTemplate, utils};
use crate::templates::template_models::{ReceiptData, ReceiptItem};

pub struct ReceiptTemplate;

impl ReceiptTemplate {
    pub fn new() -> Self {
        Self
    }

    fn format_items(&self, items: &[ReceiptItem]) -> String {
        items
            .iter()
            .map(|item| {
                format!(
                    "  [{}], [{}], [{:.2}], [{:.2}]",
                    utils::escape_typst(&item.description),
                    item.quantity,
                    item.unit_price,
                    item.total
                )
            })
            .collect::<Vec<_>>()
            .join(",\n")
    }
}

impl TypstTemplate for ReceiptTemplate {
    fn generate(&self, data: &Value) -> Result<String> {
        let receipt: ReceiptData = serde_json::from_value(data.clone())
            .context("Error deserializando datos de recibo")?;

        let vendor = &receipt.vendor;

        let content = format!(r#"#set document(title: "Recibo #{}", author: "{}")
#set page(paper: "a5", margin: 1.5cm)
#set text(font: "Arial", size: 10pt)

// Encabezado
#align(center)[
  #text(size: 16pt, weight: "bold")[{}]

  #text(size: 9pt)[
    {} \
    Tel: {}
  ]
]

#v(10pt)
#align(center)[
  #rect(stroke: 2pt + black, radius: 3pt, inset: 5pt)[
    #text(size: 12pt, weight: "bold")[RECIBO DE PAGO]
  ]
]

#v(10pt)

// Información del recibo
#grid(
  columns: (1fr, 1fr),
  [
    #text(weight: "bold")[No. Recibo:] {}
  ],
  [
    #align(right)[
      #text(weight: "bold")[Fecha:] {}
    ]
  ]
)

#v(10pt)
#line(length: 100%, stroke: 0.5pt)
#v(10pt)

// Items del recibo
#text(weight: "bold")[Detalle:]
#table(
  columns: (1fr, 60pt, 80pt, 80pt),
  stroke: 0.5pt + gray,
  inset: 8pt,
  [*Descripción*], [*Cantidad*], [*Precio*], [*Total*],
{}
)

#v(10pt)

// Total
#align(right)[
  #rect(fill: rgb(240, 240, 240), stroke: 1pt + gray, radius: 3pt, inset: 10pt)[
    #text(size: 12pt, weight: "bold")[Total: {} {:.2}]
  ]
]

#v(10pt)

// Método de pago
#text(weight: "bold")[Forma de pago:] {}

#v(20pt)

// Firma
#grid(
  columns: (1fr, 1fr),
  gutter: 20pt,
  [
    #line(length: 100%, stroke: 0.5pt)
    #align(center)[
      #text(size: 9pt)[Firma del Vendedor]
    ]
  ],
  [
    #line(length: 100%, stroke: 0.5pt)
    #align(center)[
      #text(size: 9pt)[Firma del Cliente]
    ]
  ]
)

#v(15pt)
#align(center)[
  #text(size: 8pt, fill: gray, style: "italic")[
    Este recibo es válido salvo buen cobro. \
    Conserve este documento como comprobante de pago.
  ]
]"#,
            // Metadata
            receipt.receipt_number,
            vendor.name,
            // Header
            utils::escape_typst(&vendor.name),
            utils::escape_typst(&format!("{}, {}", vendor.address.city, vendor.address.country)),
            vendor.phone.as_deref().unwrap_or(""),
            // Receipt info
            receipt.receipt_number,
            receipt.date,
            // Items
            self.format_items(&receipt.items),
            // Total
            receipt.currency,
            receipt.total,
            // Payment method
            utils::escape_typst(&receipt.payment_method)
        );

        Ok(content)
    }

    fn template_id(&self) -> &str {
        "receipt"
    }

    fn validate(&self, data: &Value) -> Result<()> {
        if !data.is_object() {
            anyhow::bail!("Los datos deben ser un objeto JSON");
        }

        let obj = data.as_object().unwrap();
        let required = vec![
            "receipt_number",
            "date",
            "vendor",
            "items",
            "total",
            "payment_method",
            "currency"
        ];

        for field in required {
            if !obj.contains_key(field) {
                anyhow::bail!("Campo requerido faltante: {}", field);
            }
        }

        Ok(())
    }

    fn description(&self) -> &str {
        "Recibo de Pago"
    }
}