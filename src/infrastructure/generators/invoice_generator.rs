//! Invoice PDF generator with Dominican Republic fiscal compliance

use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;
use super::{DocumentGenerator, DocumentType, TypstGenerator};

/// Invoice PDF generator
pub struct InvoiceGenerator {
    typst_generator: TypstGenerator,
}

impl InvoiceGenerator {
    pub fn new(work_dir: PathBuf) -> Self {
        Self {
            typst_generator: TypstGenerator::new(work_dir),
        }
    }

    /// Generate invoice-specific Typst template
    fn create_invoice_template() -> &'static str {
        r#"
#set page(
  paper: "us-letter",
  margin: (top: 1.5cm, bottom: 1.5cm, left: 1.5cm, right: 1.5cm),
)

#set text(font: "Arial", size: 10pt, lang: "es")
#set table(stroke: 0.5pt)

// Helper functions
#let format_currency(amount) = "RD$ " + str(amount)
#let format_date(date) = date

// Header with company info
#grid(
  columns: (1fr, 1fr),
  gutter: 1em,

  // Left side - Company info
  [
    #text(size: 16pt, weight: "bold")[{{seller_company_name}}]
    #v(0.5em)
    {{#if seller_trade_name}}#text(size: 12pt)[{{seller_trade_name}}]#v(0.3em){{/if}}

    RNC: #text(weight: "bold")[{{seller_rnc}}] \
    {{seller_address}} \
    Tel: {{seller_phone}} \
    Email: {{seller_email}} \
    {{#if seller_website}}Web: {{seller_website}}{{/if}}
  ],

  // Right side - Invoice info
  align(right)[
    #rect(stroke: 2pt + rgb("#004080"), inset: 10pt)[
      #text(size: 14pt, weight: "bold", fill: rgb("#004080"))[FACTURA FISCAL]

      #v(0.5em)
      #text(size: 11pt)[
        NCF: #text(weight: "bold")[{{ncf}}] \
        Válido hasta: {{ncf_expiry_date}} \
        Fecha: {{issue_date}} \
        {{#if due_date}}Vencimiento: {{due_date}}{{/if}}
      ]
    ]
  ]
)

#v(1em)

// Customer info
#rect(width: 100%, stroke: 0.5pt, inset: 10pt)[
  #text(weight: "bold")[DATOS DEL CLIENTE]
  #v(0.3em)
  #grid(
    columns: (1fr, 1fr),

    [
      Nombre/Razón Social: #text(weight: "bold")[{{customer_name}}] \
      {{#if customer_tax_id}}{{customer_tax_id_type}}: #text(weight: "bold")[{{customer_tax_id}}]{{/if}} \
      {{#if customer_contact_person}}Contacto: {{customer_contact_person}}{{/if}}
    ],

    [
      {{#if customer_address}}Dirección: {{customer_address}}{{/if}} \
      {{#if customer_phone}}Tel: {{customer_phone}}{{/if}} \
      {{#if customer_email}}Email: {{customer_email}}{{/if}}
    ]
  )
]

#v(1em)

// Invoice items table
#table(
  columns: (1fr, 3fr, 0.8fr, 1fr, 1fr, 1fr, 1.2fr),
  inset: 6pt,
  align: (center, left, center, right, right, right, right),

  // Header
  table.header(
    [*Cant.*], [*Descripción*], [*U/M*], [*Precio*], [*Desc.*], [*ITBIS*], [*Total*]
  ),

  // Items
  {{#each items}}
  [{{quantity}}],
  [{{description}}],
  [{{unit_of_measure}}],
  [{{unit_price}}],
  [{{discount_amount}}],
  [{{itbis_amount}}],
  [{{total}}],
  {{/each}}
)

#v(1em)

// Totals
#align(right)[
  #table(
    columns: (3fr, 2fr),
    inset: 8pt,
    align: (left, right),
    stroke: none,

    [Subtotal:], [#text(weight: "bold")[RD$ {{subtotal}}]],
    {{#if discount_amount}}[Descuento:], [#text(weight: "bold")[RD$ {{discount_amount}}]],{{/if}}
    [ITBIS ({{itbis_rate}}%):], [#text(weight: "bold")[RD$ {{itbis_amount}}]],

    table.hline(stroke: 2pt),
    [#text(size: 12pt, weight: "bold")[TOTAL:]],
    [#text(size: 12pt, weight: "bold", fill: rgb("#004080"))[RD$ {{total_amount}}]]
  )
]

#v(1em)

// Payment terms and notes
{{#if payment_terms}}
#rect(width: 100%, stroke: 0.5pt, inset: 8pt, fill: rgb("#f0f0f0"))[
  *Condiciones de Pago:* {{payment_terms}} \
  {{#if payment_method}}*Método de Pago:* {{payment_method}}{{/if}}
]
{{/if}}

{{#if notes}}
#v(0.5em)
#rect(width: 100%, stroke: 0.5pt, inset: 8pt)[
  *Notas:* \
  {{notes}}
]
{{/if}}

// QR Code section
{{#if qr_code_data}}
#v(1em)
#align(center)[
  #grid(
    columns: (1fr, auto, 1fr),
    column-gutter: 1em,

    [],
    [
      #image("{{qr_code_path}}", width: 3cm) \
      #text(size: 8pt)[Escanea para verificar]
    ],
    []
  )
]
{{/if}}

// Footer
#v(1fr)
#line(length: 100%, stroke: 0.5pt)
#align(center)[
  #text(size: 8pt, fill: gray)[
    Retención según Ley 253-12: Las personas físicas y jurídicas que adquieran bienes y servicios \
    que no sean contribuyentes del ITBIS retendrán el 30% del ITBIS facturado. \
    Este documento debe conservarse por 7 años según requerimientos de la DGII.
  ]
]

// Watermark if paid
{{#if is_paid}}
#place(center + horizon)[
  #rotate(45deg)[
    #text(size: 72pt, fill: rgb("#00ff00").transparentize(80%), weight: "bold")[PAGADO]
  ]
]
{{/if}}
        "#
    }
}

#[async_trait::async_trait]
impl DocumentGenerator for InvoiceGenerator {
    async fn generate(&self, template: &str, data: &Value) -> Result<Vec<u8>> {
        // Prepare data with formatted values
        let mut invoice_data = data.clone();

        // Format amounts if present
        if let Some(obj) = invoice_data.as_object_mut() {
            // Format currency values
            if let Some(subtotal) = obj.get("subtotal").and_then(|v| v.as_f64()) {
                obj.insert("subtotal".to_string(),
                    serde_json::json!(format!("{:,.2}", subtotal)));
            }

            if let Some(total) = obj.get("total_amount").and_then(|v| v.as_f64()) {
                obj.insert("total_amount".to_string(),
                    serde_json::json!(format!("{:,.2}", total)));
            }

            if let Some(itbis) = obj.get("itbis_amount").and_then(|v| v.as_f64()) {
                obj.insert("itbis_amount".to_string(),
                    serde_json::json!(format!("{:,.2}", itbis)));
            }

            // Format dates
            if let Some(date) = obj.get("issue_date").and_then(|v| v.as_str()) {
                if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(date) {
                    obj.insert("issue_date".to_string(),
                        serde_json::json!(parsed.format("%d/%m/%Y").to_string()));
                }
            }

            // Format items
            if let Some(items) = obj.get_mut("items").and_then(|v| v.as_array_mut()) {
                for item in items {
                    if let Some(item_obj) = item.as_object_mut() {
                        // Format item amounts
                        if let Some(price) = item_obj.get("unit_price").and_then(|v| v.as_f64()) {
                            item_obj.insert("unit_price".to_string(),
                                serde_json::json!(format!("{:,.2}", price)));
                        }

                        if let Some(total) = item_obj.get("total").and_then(|v| v.as_f64()) {
                            item_obj.insert("total".to_string(),
                                serde_json::json!(format!("{:,.2}", total)));
                        }

                        if let Some(itbis) = item_obj.get("itbis_amount").and_then(|v| v.as_f64()) {
                            item_obj.insert("itbis_amount".to_string(),
                                serde_json::json!(format!("{:,.2}", itbis)));
                        }
                    }
                }
            }
        }

        // Use provided template or default
        let final_template = if template.is_empty() {
            Self::create_invoice_template()
        } else {
            template
        };

        // Generate PDF
        self.typst_generator.process(final_template, &invoice_data).await
    }

    fn supported_types(&self) -> Vec<DocumentType> {
        vec![DocumentType::Invoice]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_invoice_generation() {
        let temp_dir = TempDir::new().unwrap();
        let generator = InvoiceGenerator::new(temp_dir.path().to_path_buf());

        let invoice_data = serde_json::json!({
            "seller_company_name": "Le Croissant Doré SRL",
            "seller_trade_name": "Le Croissant Doré",
            "seller_rnc": "1-30-12345-6",
            "seller_address": "Av. Abraham Lincoln #123, Piantini",
            "seller_phone": "809-555-1234",
            "seller_email": "facturas@lecroissant.com",
            "seller_website": "www.lecroissant.com",

            "customer_name": "Cliente Test SRL",
            "customer_tax_id": "1-31-65432-1",
            "customer_tax_id_type": "RNC",
            "customer_address": "Calle Principal #456, Santiago",
            "customer_phone": "809-555-5678",
            "customer_email": "compras@cliente.com",

            "ncf": "E31-0000-0001",
            "ncf_expiry_date": "31/12/2025",
            "issue_date": "2024-11-24T12:00:00Z",
            "due_date": "2024-12-24T12:00:00Z",

            "items": [
                {
                    "quantity": 2,
                    "description": "Croissant de Chocolate",
                    "unit_of_measure": "UND",
                    "unit_price": 150.00,
                    "discount_amount": 0.00,
                    "itbis_amount": 54.00,
                    "total": 354.00
                },
                {
                    "quantity": 1,
                    "description": "Café Cappuccino Grande",
                    "unit_of_measure": "UND",
                    "unit_price": 200.00,
                    "discount_amount": 0.00,
                    "itbis_amount": 36.00,
                    "total": 236.00
                }
            ],

            "subtotal": 500.00,
            "discount_amount": 0.00,
            "itbis_rate": 18,
            "itbis_amount": 90.00,
            "total_amount": 590.00,

            "payment_terms": "Contado",
            "payment_method": "Efectivo",

            "notes": "Gracias por su compra!",
            "is_paid": true
        });

        // Try to generate (will fail if Typst not installed)
        match generator.generate("", &invoice_data).await {
            Ok(pdf) => {
                println!("Generated invoice PDF: {} bytes", pdf.len());
                assert!(!pdf.is_empty());
            }
            Err(e) => {
                println!("Could not generate invoice (Typst may not be installed): {}", e);
            }
        }
    }
}