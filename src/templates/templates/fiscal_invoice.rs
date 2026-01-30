use crate::templates::template_models::{InvoiceData, InvoiceItem, InvoiceTotals};
use crate::templates::template_trait::{utils, TypstTemplate};
use anyhow::{Context, Result};
use serde_json::Value;

/// DGII-compliant Fiscal Invoice Template for Dominican Republic
/// Based on "Representación Impresa (Modelos ilustrativos)" from DGII
///
/// This template follows the official DGII format exactly:
/// - Header: Logo, company name (colored), legal name, branch, RNC, address, issue date
/// - Right side: Document type (colored), e-NCF, expiration date
/// - Client section: Razón Social Cliente, RNC Cliente
/// - Items table: Cantidad | Descripción | Unidad de Medida | Precio | ITBIS | Valor
/// - Footer: QR code (left), security code, signature date, totals (right)
/// - Pagination: "Página No. X de Y" with page subtotals for multi-page invoices
pub struct FiscalInvoiceTemplate;

/// Default color matching DGII examples (green tones)
const DEFAULT_PRIMARY_COLOR: &str = "rgb(0, 128, 102)"; // Verde DGII
const DEFAULT_FONT: &str = "Helvetica Neue";

impl FiscalInvoiceTemplate {
    pub fn new() -> Self {
        Self
    }

    /// Extract brand color from custom_fields, supports multiple formats:
    /// - "brand_color": "#FF5500" (hex)
    /// - "primary_color": "rgb(255, 85, 0)" (rgb)
    /// - "brand_color": "rgb(255, 85, 0)" (rgb)
    fn get_brand_color(
        custom_fields: &Option<std::collections::HashMap<String, serde_json::Value>>,
    ) -> String {
        if let Some(fields) = custom_fields {
            // Try brand_color first, then primary_color
            let color_value = fields
                .get("brand_color")
                .or_else(|| fields.get("primary_color"))
                .and_then(|v| v.as_str());

            if let Some(color) = color_value {
                // If it's a hex color, convert to rgb format for Typst
                if color.starts_with('#') {
                    return Self::hex_to_typst_rgb(color);
                }
                // If it's already rgb format, use as-is
                if color.starts_with("rgb") {
                    return color.to_string();
                }
                // Otherwise treat as raw color value
                return format!("rgb({})", color);
            }
        }
        DEFAULT_PRIMARY_COLOR.to_string()
    }

    /// Convert hex color (#RRGGBB or #RGB) to Typst rgb() format
    fn hex_to_typst_rgb(hex: &str) -> String {
        let hex = hex.trim_start_matches('#');

        let (r, g, b) = if hex.len() == 6 {
            (
                u8::from_str_radix(&hex[0..2], 16).unwrap_or(0),
                u8::from_str_radix(&hex[2..4], 16).unwrap_or(0),
                u8::from_str_radix(&hex[4..6], 16).unwrap_or(0),
            )
        } else if hex.len() == 3 {
            (
                u8::from_str_radix(&hex[0..1].repeat(2), 16).unwrap_or(0),
                u8::from_str_radix(&hex[1..2].repeat(2), 16).unwrap_or(0),
                u8::from_str_radix(&hex[2..3].repeat(2), 16).unwrap_or(0),
            )
        } else {
            return DEFAULT_PRIMARY_COLOR.to_string();
        };

        format!("rgb({}, {}, {})", r, g, b)
    }

    /// Check if items have document_date or notes (for conduce invoices)
    fn has_extended_columns(items: &[InvoiceItem]) -> bool {
        items
            .iter()
            .any(|item| item.document_date.is_some() || item.notes.is_some())
    }

    /// Format items table rows - standard 6 columns:
    /// Cantidad | Descripción | Unidad de Medida | Precio | ITBIS | Valor
    fn format_items_standard(&self, items: &[InvoiceItem]) -> String {
        items
            .iter()
            .map(|item| {
                let itbis_amount = if let Some(rate) = item.tax_rate {
                    let subtotal = item.get_subtotal() - item.discount.unwrap_or(0.0);
                    subtotal * (rate / 100.0)
                } else {
                    item.tax_amount.unwrap_or(0.0)
                };

                format!(
                    "  [{:.2}], [{}], [{}], [{:.2}], [{:.2}], [{:.2}]",
                    item.quantity,
                    utils::escape_typst(&item.description),
                    item.unit.as_deref().unwrap_or("UND"),
                    item.unit_price,
                    itbis_amount,
                    item.get_total()
                )
            })
            .collect::<Vec<_>>()
            .join(",\n")
    }

    /// Format items table rows - extended 8 columns for conduce invoices:
    /// Fecha | Cantidad | Descripción | Nota | Unidad | Precio | ITBIS | Valor
    fn format_items_extended(&self, items: &[InvoiceItem]) -> String {
        items
            .iter()
            .map(|item| {
                let itbis_amount = if let Some(rate) = item.tax_rate {
                    let subtotal = item.get_subtotal() - item.discount.unwrap_or(0.0);
                    subtotal * (rate / 100.0)
                } else {
                    item.tax_amount.unwrap_or(0.0)
                };

                let date_str = item.document_date.as_deref().unwrap_or("-");
                let notes_str = item.notes.as_deref().unwrap_or("");

                format!(
                    "  [{}], [{:.2}], [{}], [{}], [{}], [{:.2}], [{:.2}], [{:.2}]",
                    date_str,
                    item.quantity,
                    utils::escape_typst(&item.description),
                    utils::escape_typst(notes_str),
                    item.unit.as_deref().unwrap_or("UND"),
                    item.unit_price,
                    itbis_amount,
                    item.get_total()
                )
            })
            .collect::<Vec<_>>()
            .join(",\n")
    }

    /// Generate the complete items table section with dynamic columns
    fn generate_items_table(&self, items: &[InvoiceItem]) -> String {
        let use_extended = Self::has_extended_columns(items);

        if use_extended {
            // Extended table with 8 columns for conduce invoices
            let items_content = self.format_items_extended(items);
            format!(
                r#"#table(
  columns: (60pt, 45pt, 1fr, 1fr, 50pt, 55pt, 55pt, 65pt),
  stroke: 0.5pt + rgb(150, 150, 150),
  fill: (x, y) => if y == 0 {{ rgb(240, 240, 240) }} else {{ white }},
  align: (col, row) => {{
    if col == 2 || col == 3 {{ left }}
    else if col >= 5 {{ right }}
    else {{ center }}
  }},
  inset: 5pt,

  // Header row - Extended columns for conduce invoices
  [#text(size: 7pt, weight: "bold")[Fecha]],
  [#text(size: 7pt, weight: "bold")[Cant.]],
  [#text(size: 7pt, weight: "bold")[Descripción]],
  [#text(size: 7pt, weight: "bold")[Nota]],
  [#text(size: 7pt, weight: "bold")[Unidad]],
  [#text(size: 7pt, weight: "bold")[Precio]],
  [#text(size: 7pt, weight: "bold")[ITBIS]],
  [#text(size: 7pt, weight: "bold")[Valor]],

  // Items
{items}
)"#,
                items = items_content
            )
        } else {
            // Standard table with 6 columns
            let items_content = self.format_items_standard(items);
            format!(
                r#"#table(
  columns: (55pt, 1fr, 75pt, 70pt, 70pt, 80pt),
  stroke: 0.5pt + rgb(150, 150, 150),
  fill: (x, y) => if y == 0 {{ rgb(240, 240, 240) }} else {{ white }},
  align: (col, row) => {{
    if col == 1 {{ left }}
    else if col >= 3 {{ right }}
    else {{ center }}
  }},
  inset: 6pt,

  // Header row - DGII standard column names
  [#text(size: 8pt, weight: "bold")[Cantidad]],
  [#text(size: 8pt, weight: "bold")[Descripción]],
  [#text(size: 8pt, weight: "bold")[Unidad de Medida]],
  [#text(size: 8pt, weight: "bold")[Precio]],
  [#text(size: 8pt, weight: "bold")[ITBIS]],
  [#text(size: 8pt, weight: "bold")[Valor]],

  // Items
{items}
)"#,
                items = items_content
            )
        }
    }

    /// Format totals section following DGII standard layout:
    /// Right-aligned box with Subtotal Gravado, Total ITBIS, Total
    fn format_totals(
        &self,
        totals: &InvoiceTotals,
        custom_fields: &Option<std::collections::HashMap<String, serde_json::Value>>,
    ) -> String {
        // Extract custom fields for detailed DGII breakdown
        let (subtotal_gravado_18, subtotal_gravado_16, subtotal_exento, itbis_18, itbis_16) =
            if let Some(fields) = custom_fields {
                (
                    fields
                        .get("subtotal_gravado_18")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0),
                    fields
                        .get("subtotal_gravado_16")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0),
                    fields
                        .get("subtotal_exento")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0),
                    fields
                        .get("itbis_18")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0),
                    fields
                        .get("itbis_16")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0),
                )
            } else {
                (totals.subtotal, 0.0, 0.0, totals.tax_amount, 0.0)
            };

        let total_subtotal_gravado = subtotal_gravado_18 + subtotal_gravado_16;
        let total_itbis = itbis_18 + itbis_16;

        // Build exempt line if applicable
        let exempt_line = if subtotal_exento > 0.0 {
            format!(
                r#"[#text(size: 9pt)[Subtotal Exento:]], [#text(size: 9pt)[{:.2}]],"#,
                subtotal_exento
            )
        } else {
            String::new()
        };

        // DGII-style totals box (right-aligned, simple border)
        format!(
            r#"#align(right)[
  #rect(stroke: 0.5pt + rgb(100, 100, 100), inset: 0pt)[
    #table(
      columns: (120pt, 100pt),
      stroke: 0.5pt + rgb(100, 100, 100),
      inset: 6pt,
      align: (left, right),
      [#text(size: 9pt, weight: "bold")[Subtotal Gravado:]], [#text(size: 9pt)[{:.2}]],
      {}
      [#text(size: 9pt, weight: "bold")[Total ITBIS:]], [#text(size: 9pt)[{:.2}]],
      [#text(size: 9pt, weight: "bold")[Total:]], [#text(size: 9pt, weight: "bold")[{:.2}]]
    )
  ]
]"#,
            total_subtotal_gravado, exempt_line, total_itbis, totals.total
        )
    }

    /// Get document type name based on e-NCF prefix
    fn get_document_type_name(e_ncf: &str) -> &'static str {
        if e_ncf.len() >= 3 {
            match &e_ncf[1..3] {
                "31" => "Factura de Crédito Fiscal Electrónica",
                "32" => "Factura de Consumo Electrónica",
                "33" => "Nota de Débito Electrónica",
                "34" => "Nota de Crédito Electrónica",
                "41" => "Compras Electrónico",
                "43" => "Gastos Menores Electrónico",
                "44" => "Regímenes Especiales Electrónico",
                "45" => "Gubernamental Electrónico",
                "46" => "Comprobante para Exportaciones",
                "47" => "Pagos al Exterior",
                _ => "Factura Electrónica",
            }
        } else {
            "Factura Electrónica"
        }
    }

    fn generate_typst_content(&self, invoice: &InvoiceData) -> Result<String> {
        let company = &invoice.company_info;
        let client = &invoice.client_info;
        let totals = invoice.get_totals();

        // Get client tax_id with fallback
        let client_tax_id = client.tax_id.as_deref().unwrap_or("N/A");

        // Get brand color from custom_fields (allows customization from core-service)
        let primary_color = Self::get_brand_color(&invoice.custom_fields);

        // Get branch info from custom_fields
        let branch_name = invoice
            .custom_fields
            .as_ref()
            .and_then(|cf| cf.get("branch_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Determine document type from e-NCF
        let document_type = if let Some(fiscal) = &invoice.fiscal_info {
            Self::get_document_type_name(&fiscal.e_ncf)
        } else {
            "Factura de Crédito Fiscal Electrónica"
        };

        // Get work_dir from custom_fields (for QR generation)
        let work_dir = invoice
            .custom_fields
            .as_ref()
            .and_then(|cf| cf.get("work_dir"))
            .and_then(|v| v.as_str())
            .unwrap_or("/tmp");

        // Generate QR section if fiscal info exists (DGII requirement)
        // Uses PNG image for maximum compatibility across Typst versions
        let qr_section = if let Some(fiscal) = &invoice.fiscal_info {
            let qr_data = if fiscal.qr_data.is_empty() {
                format!(
                    "https://dgii.gov.do/ecf?rnc={}&encf={}&monto={:.2}&codigo={}",
                    company.tax_id, fiscal.e_ncf, totals.total, fiscal.security_code
                )
            } else {
                fiscal.qr_data.clone()
            };

            // Generate QR code as PNG in work_dir (relative path for Typst)
            let qr_filename = format!(
                "qr_{}.png",
                fiscal.e_ncf.replace("/", "_").replace("-", "_")
            );
            let qr_full_path = format!("{}/{}", work_dir, qr_filename);
            utils::generate_qr_code(&qr_data, &qr_full_path)?;

            // DGII layout: QR on left, totals on right
            format!(
                r#"#grid(
  columns: (1fr, 1fr),
  gutter: 20pt,
  [
    // QR Code section (DGII requirement)
    #box(width: 90pt, height: 90pt)[
      #image("{qr_filename}", width: 90pt, height: 90pt)
    ]
    #v(4pt)
    #text(size: 8pt, weight: "bold")[Código de Seguridad: {security_code}]
    #linebreak()
    #text(size: 8pt)[Fecha Firma: {signature_date}]
  ],
  [
    {totals}
  ]
)"#,
                qr_filename = qr_filename,
                security_code = fiscal.security_code,
                signature_date = fiscal.signature_date,
                totals = self.format_totals(&totals, &invoice.custom_fields)
            )
        } else {
            // No fiscal info - just show totals aligned right
            self.format_totals(&totals, &invoice.custom_fields)
        };

        // Build the complete Typst document following exact DGII layout
        let content = format!(
            r##"#set document(title: "Factura Fiscal - {invoice_number}", author: "{company_name}")
#set page(
  paper: "us-letter",
  margin: (left: 15mm, right: 15mm, top: 15mm, bottom: 20mm),
  footer: context [
    #align(right)[
      #text(size: 8pt)[Página No. #counter(page).display() de #counter(page).final().first()]
    ]
  ]
)
#set text(font: "{font}", size: 9pt, lang: "es", fill: rgb(30, 30, 30))

// ============================================================
// HEADER SECTION - DGII Official Layout
// Left: Logo + Company Info | Right: Document Type + e-NCF
// ============================================================
#grid(
  columns: (1fr, auto),
  gutter: 20pt,
  [
    // Left side: Company logo and info
    #grid(
      columns: (55pt, 1fr),
      gutter: 10pt,
      [
        // Logo placeholder (company initials in colored box)
        #rect(width: 50pt, height: 50pt, fill: {primary_color}, radius: 3pt)[
          #place(center + horizon)[
            #text(size: 18pt, weight: "bold", fill: white)[{company_initials}]
          ]
        ]
      ],
      [
        #text(size: 14pt, weight: "bold", fill: {primary_color})[{company_name}]
        #linebreak()
        #text(size: 9pt)[{legal_name}]
        #linebreak()
        {branch_section}
        #text(size: 9pt, weight: "bold")[RNC {tax_id}]
        #linebreak()
        #text(size: 8pt)[Dirección: {address}]
        #linebreak()
        #text(size: 8pt, weight: "bold")[Fecha Emisión:] #text(size: 8pt)[{issue_date}]
      ]
    )
  ],
  [
    // Right side: Document type and fiscal info (DGII format)
    #align(right)[
      #text(size: 12pt, weight: "bold", fill: {primary_color})[{document_type}]
      #v(6pt)
      {encf_section}
    ]
  ]
)

#v(10pt)
#line(length: 100%, stroke: 1pt + rgb(180, 180, 180))
#v(8pt)

// ============================================================
// CLIENT SECTION - DGII Standard
// Razón Social Cliente + RNC Cliente
// ============================================================
#text(size: 9pt, weight: "bold")[Razón Social Cliente:] #text(size: 9pt)[ {client_name}]
#linebreak()
#text(size: 9pt, weight: "bold")[RNC Cliente:] #text(size: 9pt)[ {client_tax_id}]
{client_address_section}

#v(8pt)
#line(length: 100%, stroke: 0.5pt + rgb(180, 180, 180))
#v(10pt)

// ============================================================
// ITEMS TABLE - Dynamic columns (6 standard or 8 for conduce)
// Standard: Cantidad | Descripción | Unidad de Medida | Precio | ITBIS | Valor
// Extended: Fecha | Cant. | Descripción | Nota | Unidad | Precio | ITBIS | Valor
// ============================================================
{items_table}

#v(15pt)

// ============================================================
// FOOTER SECTION - QR Code + Totals (DGII Layout)
// QR on left with security code, Totals on right
// ============================================================
{qr_section}

// Notes section (if any)
{notes_section}

// Payment info (if any)
{payment_section}
"##,
            invoice_number = invoice.invoice_number,
            company_name = utils::escape_typst(&company.name),
            font = DEFAULT_FONT,
            primary_color = primary_color,
            company_initials = get_initials(&company.name),
            legal_name = utils::escape_typst(
                &company
                    .legal_name
                    .clone()
                    .unwrap_or_else(|| company.name.clone())
            ),
            branch_section = if !branch_name.is_empty() {
                format!(
                    "#text(size: 8pt)[Sucursal {}]\n        #linebreak()\n        ",
                    branch_name
                )
            } else {
                String::new()
            },
            tax_id = company.tax_id,
            address = utils::escape_typst(&company.address.to_string()),
            issue_date = invoice.issue_date,
            document_type = document_type,
            encf_section = if let Some(fiscal) = &invoice.fiscal_info {
                format!(
                    r#"#text(size: 10pt, weight: "bold")[e-NCF:] #text(size: 10pt)[{}]
      #linebreak()
      #text(size: 9pt, weight: "bold")[Fecha Vencimiento:] #text(size: 9pt)[{}]"#,
                    fiscal.e_ncf,
                    fiscal
                        .expiration_date
                        .as_deref()
                        .unwrap_or(&invoice.due_date)
                )
            } else {
                format!(
                    r#"#text(size: 10pt, weight: "bold")[No. {}]
      #linebreak()
      #text(size: 9pt)[Vence: {}]"#,
                    invoice.invoice_number, invoice.due_date
                )
            },
            client_name = utils::escape_typst(&client.name),
            client_tax_id = client_tax_id,
            client_address_section = if let Some(addr) = &client.address {
                format!(
                    "\n#linebreak()\n#text(size: 8pt)[Dirección: {}]",
                    utils::escape_typst(&addr.to_string())
                )
            } else {
                String::new()
            },
            items_table = self.generate_items_table(&invoice.items),
            qr_section = qr_section,
            notes_section = if let Some(notes) = &invoice.notes {
                format!(
                    r#"
#v(10pt)
#text(size: 8pt, weight: "bold")[Notas:] #text(size: 8pt)[{}]"#,
                    utils::escape_typst(notes)
                )
            } else {
                String::new()
            },
            payment_section = if let Some(payment) = &invoice.payment_info {
                format!(
                    r#"
#v(8pt)
#text(size: 8pt, fill: rgb(100, 100, 100))[Condición de Pago: {} | Términos: {}]"#,
                    payment.method,
                    payment.terms.as_deref().unwrap_or("Contado")
                )
            } else {
                String::new()
            }
        );

        Ok(content)
    }
}

/// Get initials from a company name (first letter of first two words)
fn get_initials(name: &str) -> String {
    name.split_whitespace()
        .take(2)
        .filter_map(|word| word.chars().next())
        .map(|c| c.to_uppercase().next().unwrap_or(c))
        .collect()
}

impl TypstTemplate for FiscalInvoiceTemplate {
    fn generate(&self, data: &Value) -> Result<String> {
        // Deserialize data to InvoiceData
        let invoice: InvoiceData = serde_json::from_value(data.clone())
            .context("Error deserializando datos de factura. Asegúrese de enviar los campos requeridos: invoice_number/invoiceNumber, issue_date/issueDate, due_date/dueDate, company_info/companyInfo, client_info/clientInfo, items")?;

        // Generate Typst content
        self.generate_typst_content(&invoice)
    }

    fn template_id(&self) -> &str {
        "fiscal_invoice"
    }

    fn validate(&self, data: &Value) -> Result<()> {
        // Validate required fields
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

        Ok(())
    }

    fn description(&self) -> &str {
        "Factura de Crédito Fiscal Electrónica (República Dominicana) - Formato DGII"
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
        assert_eq!(get_initials("COMERCIAL ZYL"), "CZ");
    }

    #[test]
    fn test_document_type_from_ncf() {
        assert_eq!(
            FiscalInvoiceTemplate::get_document_type_name("E310000000001"),
            "Factura de Crédito Fiscal Electrónica"
        );
        assert_eq!(
            FiscalInvoiceTemplate::get_document_type_name("E320000000001"),
            "Factura de Consumo Electrónica"
        );
        assert_eq!(
            FiscalInvoiceTemplate::get_document_type_name("E330000000001"),
            "Nota de Débito Electrónica"
        );
        assert_eq!(
            FiscalInvoiceTemplate::get_document_type_name("E340000000001"),
            "Nota de Crédito Electrónica"
        );
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

    #[test]
    fn test_hex_to_rgb_conversion() {
        // Test 6-digit hex
        assert_eq!(
            FiscalInvoiceTemplate::hex_to_typst_rgb("#FF5500"),
            "rgb(255, 85, 0)"
        );
        assert_eq!(
            FiscalInvoiceTemplate::hex_to_typst_rgb("#008066"),
            "rgb(0, 128, 102)"
        );

        // Test 3-digit hex
        assert_eq!(
            FiscalInvoiceTemplate::hex_to_typst_rgb("#F50"),
            "rgb(255, 85, 0)"
        );

        // Test lowercase
        assert_eq!(
            FiscalInvoiceTemplate::hex_to_typst_rgb("#ff5500"),
            "rgb(255, 85, 0)"
        );
    }

    #[test]
    fn test_get_brand_color_from_custom_fields() {
        // Test with brand_color hex
        let mut fields = std::collections::HashMap::new();
        fields.insert("brand_color".to_string(), serde_json::json!("#FF5500"));
        let custom_fields = Some(fields);
        assert_eq!(
            FiscalInvoiceTemplate::get_brand_color(&custom_fields),
            "rgb(255, 85, 0)"
        );

        // Test with primary_color rgb
        let mut fields2 = std::collections::HashMap::new();
        fields2.insert(
            "primary_color".to_string(),
            serde_json::json!("rgb(100, 200, 50)"),
        );
        let custom_fields2 = Some(fields2);
        assert_eq!(
            FiscalInvoiceTemplate::get_brand_color(&custom_fields2),
            "rgb(100, 200, 50)"
        );

        // Test default (no custom_fields)
        assert_eq!(
            FiscalInvoiceTemplate::get_brand_color(&None),
            "rgb(0, 128, 102)"
        );
    }

    #[test]
    fn test_validate_with_fiscal_info() {
        let template = FiscalInvoiceTemplate::new();
        let data = serde_json::json!({
            "invoice_number": "FCF-00001",
            "issue_date": "2024-12-11",
            "due_date": "2024-12-25",
            "company_info": {
                "name": "COMERCIAL ZYL",
                "legal_name": "ZYL SRL",
                "tax_id": "123456789",
                "address": {"street": "Calle Principal #123", "city": "Santo Domingo", "country": "DO"}
            },
            "client_info": {
                "name": "Cliente Test SRL",
                "tax_id": "987654321"
            },
            "items": [
                {
                    "description": "Producto A",
                    "quantity": 10.0,
                    "unit": "UND",
                    "unit_price": 100.0,
                    "tax_rate": 18.0
                }
            ],
            "fiscal_info": {
                "e_ncf": "E310000000001",
                "security_code": "ABC123",
                "signature_date": "2024-12-11",
                "qr_data": "https://dgii.gov.do/ecf?test"
            }
        });
        assert!(template.validate(&data).is_ok());
    }
}
