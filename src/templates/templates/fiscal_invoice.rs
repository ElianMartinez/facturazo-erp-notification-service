use crate::templates::template_models::{InvoiceData, InvoiceItem};
use crate::templates::template_trait::{utils, TypstTemplate};
use anyhow::{Context, Result};
use serde_json::Value;

pub struct FiscalInvoiceTemplate;

impl FiscalInvoiceTemplate {
    pub fn new() -> Self {
        Self
    }

    fn format_items(&self, items: &[InvoiceItem]) -> String {
        items
            .iter()
            .map(|item| {
                format!(
                    "  [{}], [{}], [{}], [{:.2}], [{:.2}]",
                    utils::escape_typst(&item.description),
                    item.quantity,
                    item.unit.as_deref().unwrap_or("UND"),
                    item.unit_price,
                    item.get_total() // Use calculated total
                )
            })
            .collect::<Vec<_>>()
            .join(",\n")
    }

    fn generate_typst_content(&self, invoice: &InvoiceData) -> Result<String> {
        let company = &invoice.company_info;
        let client = &invoice.client_info;
        // Use get_totals() which calculates if not provided
        let totals = invoice.get_totals();

        // Generar QR si hay información fiscal
        let qr_section = if let Some(fiscal) = &invoice.fiscal_info {
            let qr_data = if fiscal.qr_data.is_empty() {
                format!(
                    "https://dgii.gov.do/validacion?ncf={}&rnc={}&monto={:.2}&codigo={}",
                    fiscal.e_ncf, company.tax_id, totals.total, fiscal.security_code
                )
            } else {
                fiscal.qr_data.clone()
            };

            // Generar QR (en producción, esto debería manejarse mejor)
            let qr_path = format!("/tmp/qr_{}.png", fiscal.e_ncf.replace("/", "_"));
            utils::generate_qr_code(&qr_data, &qr_path)?;

            format!(
                r#"
// Código QR y datos fiscales
#grid(
  columns: (1fr, 250pt),
  gutter: 20pt,
  [
    #image("{}", width: 100pt, height: 100pt)

    #v(5pt)
    #text(size: 8pt, weight: "bold")[Código de Seguridad: {}] \
    #text(size: 8pt)[Fecha Firma: {}]
  ],
  [
    // Sección de totales se coloca aquí
    TOTALES_PLACEHOLDER
  ]
)"#,
                qr_path, fiscal.security_code, fiscal.signature_date
            )
        } else {
            format!(
                r#"
// Sección de totales
#align(right)[
  TOTALES_PLACEHOLDER
]"#
            )
        };

        // Get client tax_id with fallback
        let client_tax_id = client.tax_id.as_deref().unwrap_or("N/A");

        // Construir el documento completo
        let content = format!(
            r#"#set document(title: "Factura Fiscal Electrónica - {}", author: "{}")
#set page(
  paper: "us-letter",
  margin: (left: 20mm, right: 20mm, top: 20mm, bottom: 20mm)
)
#set text(font: "Inter", size: 10pt, lang: "es", fill: rgb(30, 30, 30))
#set align(left)

// Marca de agua si está pagada
{}

// Header con información de la empresa
#grid(
  columns: (1fr, 1fr),
  [
    // Logo o inicial de la empresa
    #rect(width: 60pt, height: 60pt, fill: rgb(240, 248, 255), stroke: 1pt + rgb(70, 130, 180), radius: 5pt)[
      #place(center + horizon)[
        #text(size: 24pt, weight: "bold", fill: rgb(70, 130, 180))[{}]
      ]
    ]

    #v(5pt)

    #text(size: 14pt, weight: "bold", fill: rgb(70, 130, 180))[{}]

    #text(size: 10pt, weight: "bold")[{}] \
    #text(size: 9pt)[Sucursal {}] \
    #text(size: 9pt, weight: "bold")[RNC {}] \
    #text(size: 8pt)[
      Dirección: {} \
      Tel: {} | Email: {} \
      Fecha Emisión: {}
    ]
  ],
  [
    #align(right)[
      #text(size: 12pt, weight: "bold", fill: rgb(70, 130, 180))[Factura de Crédito Fiscal Electrónica]
      #v(5pt)
      {}
      #text(size: 9pt)[Fecha Vencimiento: {}]
    ]
  ]
)

#v(15pt)
#line(length: 100%, stroke: 1.5pt + rgb(70, 130, 180))
#v(10pt)

// Información del cliente
#text(size: 10pt, weight: "bold")[Razón Social Cliente: {}] \
#text(size: 10pt, weight: "bold")[RNC/Cédula Cliente: {}] \
{}

#v(10pt)
#line(length: 100%, stroke: 1.5pt + rgb(70, 130, 180))
#v(15pt)

// Tabla de productos/servicios
#table(
  columns: (1fr, 60pt, 80pt, 80pt, 100pt),
  stroke: 0.5pt + rgb(150, 150, 150),
  fill: (x, y) => if y == 0 {{ rgb(240, 240, 240) }} else {{ white }},
  align: (col, row) => {{
    if col == 0 {{ left }}
    else {{ right }}
  }},
  inset: 8pt,

  // Encabezados
  [#text(weight: "bold")[Descripción]],
  [#text(weight: "bold")[Cantidad]],
  [#text(weight: "bold")[Unidad]],
  [#text(weight: "bold")[Precio]],
  [#text(weight: "bold")[Total]],

  // Items
{}
)

#v(20pt)

{}

// Notas
{}

// Información de pago
{}

// Pie de página
#v(30pt)
#align(center)[
  #text(size: 8pt, fill: rgb(100, 100, 100), style: "italic")[
    {}
  ]
]"#,
            // Título del documento
            invoice.invoice_number,
            company.name,
            // Marca de agua si está pagado
            if invoice
                .payment_info
                .as_ref()
                .map(|p| p.paid)
                .unwrap_or(false)
            {
                r#"#place(
  center + horizon,
  rotate(45deg)[
    #text(size: 120pt, fill: rgb(200, 200, 200, 40), weight: "bold")[PAGADO]
  ]
)"#
            } else {
                ""
            },
            // Iniciales de la empresa
            get_initials(&company.name),
            // Datos de la empresa
            utils::escape_typst(&company.name),
            utils::escape_typst(
                &company
                    .legal_name
                    .clone()
                    .unwrap_or_else(|| company.name.clone())
            ),
            "Principal", // branch no existe en el modelo actual
            company.tax_id,
            utils::escape_typst(&company.address.to_string()),
            company.phone.as_deref().unwrap_or(""),
            utils::escape_typst(company.email.as_deref().unwrap_or("")),
            invoice.issue_date,
            // Información fiscal si existe
            if let Some(fiscal) = &invoice.fiscal_info {
                format!(
                    "#text(size: 10pt, weight: \"bold\")[e-NCF: {}]",
                    fiscal.e_ncf
                )
            } else {
                format!(
                    "#text(size: 10pt, weight: \"bold\")[Factura No. {}]",
                    invoice.invoice_number
                )
            },
            invoice.due_date,
            // Datos del cliente
            utils::escape_typst(&client.name),
            client_tax_id,
            if let Some(address) = &client.address {
                format!(
                    "#text(size: 9pt)[Dirección: {}] \\",
                    utils::escape_typst(&address.to_string())
                )
            } else {
                String::new()
            },
            // Items de la factura
            self.format_items(&invoice.items),
            // Sección QR y totales
            qr_section.replace("TOTALES_PLACEHOLDER", &self.format_totals(&totals)),
            // Notas
            if let Some(notes) = &invoice.notes {
                format!(
                    r#"
#v(20pt)
#text(size: 9pt, weight: "bold")[Notas:]
#text(size: 9pt)[{}]"#,
                    utils::escape_typst(notes)
                )
            } else {
                String::new()
            },
            // Información de pago
            if let Some(payment) = &invoice.payment_info {
                format!(
                    r#"
#v(10pt)
#text(size: 9pt)[Método de pago: {} | Términos: {}]"#,
                    payment.method,
                    payment.terms.as_deref().unwrap_or("Inmediato")
                )
            } else {
                String::new()
            },
            // Footer
            if let Some(fiscal) = &invoice.fiscal_info {
                format!(
                    "Esta factura fiscal electrónica es válida hasta: {}",
                    fiscal.expiration_date.as_deref().unwrap_or("Indefinido")
                )
            } else {
                "Conserve este documento para futuras referencias.".to_string()
            }
        );

        Ok(content)
    }

    fn format_totals(&self, totals: &crate::templates::template_models::InvoiceTotals) -> String {
        // Escape currency symbol to prevent Typst math mode issues
        let currency = utils::escape_typst(&totals.currency);
        format!(
            r#"#rect(width: 100%, fill: rgb(245, 245, 245), stroke: 0.5pt + rgb(200, 200, 200), radius: 3pt)[
    #pad(10pt)[
      #grid(
        columns: (150pt, 80pt),
        row-gutter: 5pt,
        align: (right, right),
        [#text(size: 10pt, weight: "bold")[Subtotal:]],
        [#text(size: 10pt)[{} {:.2}]],
        [#text(size: 10pt, weight: "bold")[Descuento:]],
        [#text(size: 10pt)[{} {:.2}]],
        [#text(size: 10pt, weight: "bold")[ITBIS (18%):]],
        [#text(size: 10pt)[{} {:.2}]],
        [#line(length: 100%, stroke: 0.5pt + rgb(150, 150, 150))],
        [#line(length: 100%, stroke: 0.5pt + rgb(150, 150, 150))],
        [#text(size: 11pt, weight: "bold")[Total:]],
        [#text(size: 11pt, weight: "bold")[{} {:.2}]]
      )
    ]
  ]"#,
            currency,
            totals.subtotal,
            currency,
            totals.discount_amount.unwrap_or(0.0),
            currency,
            totals.tax_amount,
            currency,
            totals.total
        )
    }
}

/// Get initials from a company name
fn get_initials(name: &str) -> String {
    // First try to get uppercase letters
    let uppercase: String = name.chars().filter(|c| c.is_uppercase()).take(2).collect();

    if uppercase.len() >= 2 {
        uppercase
    } else {
        // Fallback: first letters of first two words
        name.split_whitespace()
            .take(2)
            .filter_map(|word| word.chars().next())
            .map(|c| c.to_uppercase().next().unwrap_or(c))
            .collect()
    }
}

impl TypstTemplate for FiscalInvoiceTemplate {
    fn generate(&self, data: &Value) -> Result<String> {
        // Deserializar los datos a InvoiceData
        let invoice: InvoiceData = serde_json::from_value(data.clone())
            .context("Error deserializando datos de factura. Asegúrese de enviar los campos requeridos: invoice_number/invoiceNumber, issue_date/issueDate, due_date/dueDate, company_info/companyInfo, client_info/clientInfo, items")?;

        // Generar contenido Typst
        self.generate_typst_content(&invoice)
    }

    fn template_id(&self) -> &str {
        "fiscal_invoice"
    }

    fn validate(&self, data: &Value) -> Result<()> {
        // Validar campos requeridos
        if !data.is_object() {
            anyhow::bail!("Los datos deben ser un objeto JSON");
        }

        let obj = data.as_object().unwrap();

        // Check for required fields (both snake_case and camelCase variants)
        let invoice_number_present = obj.contains_key("invoice_number")
            || obj.contains_key("invoiceNumber")
            || obj.contains_key("number");
        let issue_date_present = obj.contains_key("issue_date")
            || obj.contains_key("issueDate")
            || obj.contains_key("date");
        let due_date_present = obj.contains_key("due_date") || obj.contains_key("dueDate");
        let company_present = obj.contains_key("company_info")
            || obj.contains_key("companyInfo")
            || obj.contains_key("company");
        let client_present = obj.contains_key("client_info")
            || obj.contains_key("clientInfo")
            || obj.contains_key("client")
            || obj.contains_key("customer");
        let items_present = obj.contains_key("items");

        if !invoice_number_present {
            anyhow::bail!("Campo requerido faltante: invoice_number (o invoiceNumber, number)");
        }
        if !issue_date_present {
            anyhow::bail!("Campo requerido faltante: issue_date (o issueDate, date)");
        }
        if !due_date_present {
            anyhow::bail!("Campo requerido faltante: due_date (o dueDate)");
        }
        if !company_present {
            anyhow::bail!("Campo requerido faltante: company_info (o companyInfo, company)");
        }
        if !client_present {
            anyhow::bail!("Campo requerido faltante: client_info (o clientInfo, client, customer)");
        }
        if !items_present {
            anyhow::bail!("Campo requerido faltante: items");
        }

        // Validate items is an array
        if let Some(items) = obj.get("items") {
            if !items.is_array() {
                anyhow::bail!("El campo 'items' debe ser un array");
            }
        }

        // Note: 'totals' is now optional - will be calculated if not provided

        Ok(())
    }

    fn description(&self) -> &str {
        "Factura Fiscal Electrónica (República Dominicana)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_initials() {
        assert_eq!(get_initials("Facturazo ERP"), "FE");
        assert_eq!(get_initials("ABC Company"), "AC");
        assert_eq!(get_initials("test company"), "TC");
        assert_eq!(get_initials("X"), "X");
    }

    #[test]
    fn test_validate_snake_case() {
        let template = FiscalInvoiceTemplate::new();
        let data = serde_json::json!({
            "invoice_number": "INV-001",
            "issue_date": "2024-12-11",
            "due_date": "2024-12-25",
            "company_info": {"name": "Test", "tax_id": "123", "address": {"street": "x", "city": "y", "country": "DO"}},
            "client_info": {"name": "Client"},
            "items": []
        });
        assert!(template.validate(&data).is_ok());
    }

    #[test]
    fn test_validate_camel_case() {
        let template = FiscalInvoiceTemplate::new();
        let data = serde_json::json!({
            "invoiceNumber": "INV-001",
            "issueDate": "2024-12-11",
            "dueDate": "2024-12-25",
            "companyInfo": {"name": "Test", "taxId": "123", "address": {"street": "x", "city": "y", "country": "DO"}},
            "clientInfo": {"name": "Client"},
            "items": []
        });
        assert!(template.validate(&data).is_ok());
    }
}
