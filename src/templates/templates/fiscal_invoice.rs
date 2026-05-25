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
const DEFAULT_FONT: &str = "Roboto";

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

    /// Format a number as currency with thousand separators
    /// Example: 8421512.50 -> "8,421,512.50"
    fn format_currency(amount: f64) -> String {
        let formatted = format!("{:.2}", amount);
        let parts: Vec<&str> = formatted.split('.').collect();
        let integer_part = parts[0];
        let decimal_part = parts.get(1).unwrap_or(&"00");

        // Add thousand separators
        let chars: Vec<char> = integer_part.chars().collect();
        let mut result = String::new();
        let len = chars.len();

        for (i, c) in chars.iter().enumerate() {
            if i > 0 && (len - i) % 3 == 0 {
                result.push(',');
            }
            result.push(*c);
        }

        format!("{}.{}", result, decimal_part)
    }

    /// Check if items have source document numbers for credit source-document invoices.
    fn has_source_document_columns(items: &[InvoiceItem]) -> bool {
        items.iter().any(|item| {
            item.source_document_number
                .as_deref()
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
        })
    }

    /// Check if items have document_date for the legacy conduce layout.
    /// Plain notes must not turn normal invoices into per-document-date tables.
    fn has_extended_columns(items: &[InvoiceItem]) -> bool {
        items.iter().any(|item| item.document_date.is_some())
    }

    fn format_document_date(date: Option<&str>) -> String {
        let Some(date) = date else {
            return "-".to_string();
        };

        let parts: Vec<&str> = date.split('-').collect();
        if parts.len() == 3 && parts[0].len() == 4 {
            return format!("{}/{}/{}", parts[2], parts[1], parts[0]);
        }

        date.to_string()
    }

    fn format_source_document_number(number: Option<&str>) -> String {
        let Some(number) = number.map(str::trim).filter(|value| !value.is_empty()) else {
            return "-".to_string();
        };

        let sequence = number
            .rsplit_once('-')
            .map_or(number, |(_, suffix)| suffix.trim());
        let normalized = sequence.trim_start_matches('0');

        if normalized.is_empty() {
            "0".to_string()
        } else {
            normalized.to_string()
        }
    }

    /// Format source-document invoice rows:
    /// Fecha | N# | Descripcion | Cant. | Precio | DES | Impuesto | Total
    fn format_items_source_documents(&self, items: &[InvoiceItem]) -> String {
        items
            .iter()
            .map(|item| {
                let date_str = Self::format_document_date(item.document_date.as_deref());
                let number =
                    Self::format_source_document_number(item.source_document_number.as_deref());
                let tax_amount = item.tax_amount.unwrap_or_else(|| {
                    let subtotal = item.get_subtotal() - item.discount.unwrap_or(0.0);
                    item.tax_rate
                        .map(|rate| subtotal * (rate / 100.0))
                        .unwrap_or(0.0)
                });

                format!(
                    "  [{}], [{}], [{}], [{:.2}], [{}], [{}], [{}], [{}]",
                    utils::escape_typst(&date_str),
                    utils::escape_typst(&number),
                    format!(
                        "#text(weight: \"bold\")[{}]",
                        utils::escape_typst(&item.description)
                    ),
                    item.quantity,
                    Self::format_currency(item.unit_price),
                    Self::format_currency(item.discount.unwrap_or(0.0)),
                    Self::format_currency(tax_amount),
                    Self::format_currency(item.get_total())
                )
            })
            .collect::<Vec<_>>()
            .join(",\n")
    }

    /// Format items table rows - standard 7 columns:
    /// Cantidad | Descripción | Unidad de Medida | Precio | Descuento | ITBIS | Valor
    fn format_items_standard(&self, items: &[InvoiceItem]) -> String {
        items
            .iter()
            .map(|item| {
                let itbis_amount = item.tax_amount.unwrap_or_else(|| {
                    let subtotal = item.get_subtotal() - item.discount.unwrap_or(0.0);
                    item.tax_rate
                        .map(|rate| subtotal * (rate / 100.0))
                        .unwrap_or(0.0)
                });

                format!(
                    "  [{:.2}], [{}], [{}], [{}], [{}], [{}], [{}]",
                    item.quantity,
                    utils::escape_typst(&item.description),
                    item.unit.as_deref().unwrap_or("UND"),
                    Self::format_currency(item.unit_price),
                    Self::format_currency(item.discount.unwrap_or(0.0)),
                    Self::format_currency(itbis_amount),
                    Self::format_currency(item.get_total())
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
                let itbis_amount = item.tax_amount.unwrap_or_else(|| {
                    let subtotal = item.get_subtotal() - item.discount.unwrap_or(0.0);
                    item.tax_rate
                        .map(|rate| subtotal * (rate / 100.0))
                        .unwrap_or(0.0)
                });

                let date_str = item.document_date.as_deref().unwrap_or("-");
                let notes_str = item.notes.as_deref().unwrap_or("");

                format!(
                    "  [{}], [{:.2}], [{}], [{}], [{}], [{}], [{}], [{}]",
                    date_str,
                    item.quantity,
                    utils::escape_typst(&item.description),
                    utils::escape_typst(notes_str),
                    item.unit.as_deref().unwrap_or("UND"),
                    Self::format_currency(item.unit_price),
                    Self::format_currency(itbis_amount),
                    Self::format_currency(item.get_total())
                )
            })
            .collect::<Vec<_>>()
            .join(",\n")
    }

    /// Generate the complete items table section with dynamic columns
    fn generate_items_table(&self, items: &[InvoiceItem]) -> String {
        if Self::has_source_document_columns(items) {
            let items_content = self.format_items_source_documents(items);
            return format!(
                r#"#table(
  columns: (55pt, 55pt, 1fr, 40pt, 60pt, 55pt, 60pt, 65pt),
  stroke: 0.5pt + rgb(210, 216, 226),
  fill: (x, y) => if y == 0 {{ rgb(239, 243, 248) }} else {{ white }},
  align: (col, row) => {{
    if col == 2 {{ left }}
    else if col >= 3 {{ right }}
    else {{ center }}
  }},
  inset: 5pt,

  [#text(size: 7pt, weight: "bold")[FECHA]],
  [#text(size: 7pt, weight: "bold")[N\#]],
  [#text(size: 7pt, weight: "bold")[DESCRIPCIÓN]],
  [#text(size: 7pt, weight: "bold")[CANT.]],
  [#text(size: 7pt, weight: "bold")[PRECIO]],
  [#text(size: 7pt, weight: "bold")[DES]],
  [#text(size: 7pt, weight: "bold")[IMPUESTO]],
  [#text(size: 7pt, weight: "bold")[TOTAL]],

  // Items
{items}
)"#,
                items = items_content
            );
        }

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
    if col == 2 or col == 3 {{ left }}
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
            // Standard table with 7 columns
            let items_content = self.format_items_standard(items);
            format!(
                r#"#table(
  columns: (50pt, 1fr, 62pt, 62pt, 70pt, 65pt, 75pt),
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
  [#text(size: 8pt, weight: "bold")[Descuento]],
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
    /// Right-aligned box with all pre-calculated values from core-service
    /// NO CALCULATIONS - just renders the values as provided
    fn format_totals(
        &self,
        totals: &InvoiceTotals,
        custom_fields: &Option<std::collections::HashMap<String, serde_json::Value>>,
    ) -> String {
        // Helper to get field with multiple possible names (supports both camelCase and snake_case)
        let get_field = |fields: &std::collections::HashMap<String, serde_json::Value>,
                         names: &[&str]|
         -> Option<f64> {
            for name in names {
                if let Some(v) = fields.get(*name).and_then(|v| v.as_f64()) {
                    return Some(v);
                }
            }
            None
        };

        // Extract ALL values from custom_fields - NO CALCULATIONS
        // All these values should come pre-calculated from core-service
        // Supports both InvoiceDetailsDto field names (PascalCase) and snake_case
        let (
            subtotal_gravado_18,
            subtotal_gravado_16,
            subtotal_exento,
            itbis_18,
            itbis_16,
            propina_legal,
            total_descuento,
            // Combined totals (also pre-calculated)
            total_subtotal_gravado,
            total_itbis,
        ) = if let Some(fields) = custom_fields {
            (
                // TaxableAmount1 = Subtotal Gravado 18%
                get_field(
                    fields,
                    &["TaxableAmount1", "taxableAmount1", "subtotal_gravado_18"],
                )
                .unwrap_or(0.0),
                // TaxableAmount2 = Subtotal Gravado 16%
                get_field(
                    fields,
                    &["TaxableAmount2", "taxableAmount2", "subtotal_gravado_16"],
                )
                .unwrap_or(0.0),
                // TaxableAmountExempt = Subtotal Exento
                get_field(
                    fields,
                    &[
                        "TaxableAmountExempt",
                        "taxableAmountExempt",
                        "subtotal_exento",
                    ],
                )
                .unwrap_or(0.0),
                // Itbis1Amount = ITBIS 18%
                get_field(fields, &["Itbis1Amount", "itbis1Amount", "itbis_18"]).unwrap_or(0.0),
                // Itbis2Amount = ITBIS 16%
                get_field(fields, &["Itbis2Amount", "itbis2Amount", "itbis_16"]).unwrap_or(0.0),
                // TipAmount = Propina Legal
                get_field(fields, &["TipAmount", "tipAmount", "propina_legal"]).unwrap_or(0.0),
                // DiscountAmount = Total Descuento
                get_field(
                    fields,
                    &["DiscountAmount", "discountAmount", "total_descuento"],
                )
                .unwrap_or(totals.discount_amount.unwrap_or(0.0)),
                // Subtotal Gravado combined (fallback to totals.subtotal)
                get_field(
                    fields,
                    &["SubtotalGravado", "subtotalGravado", "subtotal_gravado"],
                )
                .unwrap_or(totals.subtotal),
                // Total ITBIS combined (fallback to totals.tax_amount)
                get_field(fields, &["TotalItbis", "totalItbis", "total_itbis"])
                    .unwrap_or(totals.tax_amount),
            )
        } else {
            // Fallback: use InvoiceTotals values directly
            (
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                totals.discount_amount.unwrap_or(0.0),
                totals.subtotal,
                totals.tax_amount,
            )
        };

        // Build dynamic rows based on what values exist
        let mut rows = Vec::new();

        let has_detailed_taxable =
            subtotal_gravado_18 > 0.0 || subtotal_gravado_16 > 0.0 || subtotal_exento > 0.0;

        if has_detailed_taxable {
            if subtotal_gravado_18 > 0.0 {
                rows.push(format!(
                    r#"[#text(size: 9pt)[Monto Gravado 18%:]], [#text(size: 9pt)[{}]]"#,
                    Self::format_currency(subtotal_gravado_18)
                ));
            }

            if subtotal_gravado_16 > 0.0 {
                rows.push(format!(
                    r#"[#text(size: 9pt)[Monto Gravado 16%:]], [#text(size: 9pt)[{}]]"#,
                    Self::format_currency(subtotal_gravado_16)
                ));
            }

            if subtotal_exento > 0.0 {
                rows.push(format!(
                    r#"[#text(size: 9pt)[Monto Exento:]], [#text(size: 9pt)[{}]]"#,
                    Self::format_currency(subtotal_exento)
                ));
            }
        } else {
            rows.push(format!(
                r#"[#text(size: 9pt, weight: "bold")[Monto Gravado:]], [#text(size: 9pt)[{}]]"#,
                Self::format_currency(total_subtotal_gravado)
            ));
        }

        // Discount if applicable (shown in red)
        if total_descuento > 0.0 {
            rows.push(format!(
                r#"[#text(size: 9pt)[Descuento:]], [#text(size: 9pt, fill: rgb(220, 53, 69))[\- {}]]"#,
                Self::format_currency(total_descuento)
            ));
        }

        let has_detailed_itbis = itbis_18 > 0.0 || itbis_16 > 0.0;
        if has_detailed_itbis {
            if itbis_18 > 0.0 {
                rows.push(format!(
                    r#"[#text(size: 9pt)[ITBIS 18%:]], [#text(size: 9pt)[{}]]"#,
                    Self::format_currency(itbis_18)
                ));
            }

            if itbis_16 > 0.0 {
                rows.push(format!(
                    r#"[#text(size: 9pt)[ITBIS 16%:]], [#text(size: 9pt)[{}]]"#,
                    Self::format_currency(itbis_16)
                ));
            }
        } else {
            rows.push(format!(
                r#"[#text(size: 9pt, weight: "bold")[Total ITBIS:]], [#text(size: 9pt)[{}]]"#,
                Self::format_currency(total_itbis)
            ));
        }

        // Propina legal if applicable (10% for restaurants/hotels in DR)
        if propina_legal > 0.0 {
            rows.push(format!(
                r#"[#text(size: 9pt)[Propina Legal (10%):]], [#text(size: 9pt)[{}]]"#,
                Self::format_currency(propina_legal)
            ));
        }

        // Total row
        rows.push(format!(
            r#"[#text(size: 9pt, weight: "bold")[Total:]], [#text(size: 9pt, weight: "bold")[{}]]"#,
            Self::format_currency(totals.total)
        ));

        // DGII-style totals box (right-aligned, simple border)
        format!(
            r#"#align(right)[
  #rect(stroke: 0.5pt + rgb(100, 100, 100), inset: 0pt)[
    #table(
      columns: (130pt, 100pt),
      stroke: 0.5pt + rgb(100, 100, 100),
      inset: 6pt,
      align: (left, right),
      {}
    )
  ]
]"#,
            rows.join(",\n      ")
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

    /// Explicit override for the colored header title (top-right of the document).
    /// When present, takes precedence over e-NCF-based detection and the default
    /// "Factura de Crédito Fiscal Electrónica". Used by non-fiscal documents
    /// (Cotización, Preventa, Conduce) emitted from the Orders module — same
    /// fiscal-invoice look without QR/eNCF, but with the correct document name.
    fn get_document_type_override(
        custom_fields: &Option<std::collections::HashMap<String, serde_json::Value>>,
    ) -> Option<String> {
        custom_fields
            .as_ref()
            .and_then(|fields| {
                fields
                    .get("document_type_label")
                    .or_else(|| fields.get("documentTypeLabel"))
                    .or_else(|| fields.get("DocumentTypeLabel"))
            })
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Get invoice type from custom_fields
    /// Core-service now sends "invoice_type" directly as readable text
    /// Fallback to calculating from operation_type_code if not present
    fn get_invoice_type(
        custom_fields: &Option<std::collections::HashMap<String, serde_json::Value>>,
    ) -> Option<String> {
        if let Some(fields) = custom_fields {
            // Priority 1: Use invoice_type directly (already readable text from core-service)
            if let Some(invoice_type) = fields
                .get("invoice_type")
                .or_else(|| fields.get("invoiceType"))
                .or_else(|| fields.get("InvoiceType"))
                .and_then(|v| v.as_str())
            {
                return Some(invoice_type.to_string());
            }

            // Priority 2: Calculate from operation_type_code (legacy fallback)
            let code = fields
                .get("operation_type_code")
                .or_else(|| fields.get("operationTypeCode"))
                .or_else(|| fields.get("OperationTypeCode"))
                .and_then(|v| v.as_str());

            if let Some(code_str) = code {
                if code_str.ends_with("01") || code_str == "05301" {
                    return Some("Factura de Contado".to_string());
                } else if code_str.ends_with("02") || code_str == "05302" {
                    return Some("Factura a Crédito".to_string());
                }
            }
        }
        None
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

        // Determine document type — priority:
        //   1. custom_fields.document_type_label (explicit override for non-fiscal docs)
        //   2. e-NCF prefix when fiscal_info is present
        //   3. default fiscal-invoice label
        let document_type_override = Self::get_document_type_override(&invoice.custom_fields);
        let document_type: &str = match document_type_override.as_deref() {
            Some(label) => label,
            None => {
                if let Some(fiscal) = &invoice.fiscal_info {
                    Self::get_document_type_name(&fiscal.e_ncf)
                } else {
                    "Factura de Crédito Fiscal Electrónica"
                }
            }
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
#set text(font: "{font}", size: 9pt, lang: "es", fill: rgb(30, 30, 30))
#set page(
  paper: "us-letter",
  margin: (left: 15mm, right: 15mm, top: 40mm, bottom: 20mm),
  header: [
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
    #v(8pt)
    #line(length: 100%, stroke: 1pt + rgb(180, 180, 180))
    #v(4pt)
  ],
  footer: context [
    #align(right)[
      #text(size: 8pt)[Página No. #counter(page).display() de #counter(page).final().first()]
    ]
  ]
)

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
// ITEMS TABLE - Dynamic columns (7 standard or 8 for conduce)
// Standard: Cantidad | Descripción | Unidad de Medida | Precio | Descuento | ITBIS | Valor
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
            encf_section = {
                // Get invoice type from custom_fields (core-service sends it directly)
                let invoice_type = Self::get_invoice_type(&invoice.custom_fields);

                let invoice_type_line = invoice_type
                    .map(|t| {
                        format!(
                            r#"#text(size: 9pt, weight: "bold")[{}]
      #linebreak()"#,
                            t
                        )
                    })
                    .unwrap_or_default();

                // invoice_number now contains OperationSequence directly from core-service
                if let Some(fiscal) = &invoice.fiscal_info {
                    format!(
                        r#"{invoice_type_line}#text(size: 10pt, weight: "bold")[e-NCF:] #text(size: 10pt)[{e_ncf}]
      #linebreak()
      #text(size: 9pt, weight: "bold")[No. Factura:] #text(size: 9pt)[{invoice_number}]
      #linebreak()
      #text(size: 9pt, weight: "bold")[Fecha Vencimiento:] #text(size: 9pt)[{due_date}]"#,
                        invoice_type_line = invoice_type_line,
                        e_ncf = fiscal.e_ncf,
                        invoice_number = invoice.invoice_number,
                        due_date = fiscal
                            .expiration_date
                            .as_deref()
                            .unwrap_or(&invoice.due_date)
                    )
                } else {
                    format!(
                        r#"{invoice_type_line}#text(size: 10pt, weight: "bold")[No. Factura:] #text(size: 10pt)[{invoice_number}]
      #linebreak()
      #text(size: 9pt, weight: "bold")[Vence:] #text(size: 9pt)[{due_date}]"#,
                        invoice_type_line = invoice_type_line,
                        invoice_number = invoice.invoice_number,
                        due_date = invoice.due_date
                    )
                }
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
    fn test_source_document_items_table_uses_requested_columns() {
        let template = FiscalInvoiceTemplate::new();
        let items = vec![InvoiceItem {
            code: Some("P-9001".to_string()),
            description: "Caja de lubricante 10W-30".to_string(),
            quantity: 4.0,
            unit: Some("UND".to_string()),
            unit_price: 1800.0,
            tax_rate: Some(18.0),
            tax_amount: Some(1296.0),
            discount: Some(0.0),
            subtotal: Some(7200.0),
            total: Some(8496.0),
            document_date: Some("2026-04-10".to_string()),
            source_document_number: Some("PV-101".to_string()),
            notes: Some("Preventa".to_string()),
        }];

        let table = template.generate_items_table(&items);

        assert!(table.contains("[FECHA]"));
        assert!(table.contains("[N\\#]"));
        assert!(table.contains("[DESCRIPCIÓN]"));
        assert!(table.contains("[CANT.]"));
        assert!(table.contains("[PRECIO]"));
        assert!(table.contains("[DES]"));
        assert!(table.contains("[IMPUESTO]"));
        assert!(table.contains("[TOTAL]"));
        assert!(table.contains("[10/04/2026]"));
        assert!(table.contains("[101]"));
        assert!(!table.contains("[PV-101]"));
        assert!(!table.contains("[P-9001]"));
        assert!(!table.contains("[Nota]"));
        assert!(!table.contains("[Preventa]"));
    }

    #[test]
    fn test_regular_invoice_notes_do_not_enable_document_date_columns() {
        let template = FiscalInvoiceTemplate::new();
        let items = vec![InvoiceItem {
            code: Some("P-9001".to_string()),
            description: "Producto normal".to_string(),
            quantity: 1.0,
            unit: Some("UND".to_string()),
            unit_price: 100.0,
            tax_rate: Some(18.0),
            tax_amount: Some(18.0),
            discount: Some(0.0),
            subtotal: Some(100.0),
            total: Some(118.0),
            document_date: None,
            source_document_number: None,
            notes: Some("Nota interna".to_string()),
        }];

        let table = template.generate_items_table(&items);

        assert!(table.contains("[Cantidad]"));
        assert!(table.contains("[Descripción]"));
        assert!(!table.contains("[Fecha]"));
        assert!(!table.contains("[Nota]"));
    }

    #[test]
    fn test_totals_render_tax_breakdown_without_mixing_rates() {
        use serde_json::json;
        use std::collections::HashMap;

        let template = FiscalInvoiceTemplate::new();
        let totals = InvoiceTotals {
            subtotal: 1750.0,
            tax_amount: 340.0,
            discount_amount: Some(75.0),
            total: 2090.0,
            currency: "DOP".to_string(),
            tip: None,
        };
        let custom_fields: HashMap<String, serde_json::Value> = HashMap::from([
            ("TaxableAmount1".to_string(), json!(1000.0)),
            ("TaxableAmount2".to_string(), json!(500.0)),
            ("TaxableAmountExempt".to_string(), json!(250.0)),
            ("Itbis1Amount".to_string(), json!(180.0)),
            ("Itbis2Amount".to_string(), json!(80.0)),
            ("DiscountAmount".to_string(), json!(75.0)),
            ("TotalItbis".to_string(), json!(260.0)),
        ]);

        let rendered = template.format_totals(&totals, &Some(custom_fields));

        assert!(rendered.contains("Monto Gravado 18%:"));
        assert!(rendered.contains("1,000.00"));
        assert!(rendered.contains("Monto Gravado 16%:"));
        assert!(rendered.contains("500.00"));
        assert!(rendered.contains("Monto Exento:"));
        assert!(rendered.contains("250.00"));
        assert!(rendered.contains("Descuento:"));
        assert!(rendered.contains("75.00"));
        assert!(rendered.contains("ITBIS 18%:"));
        assert!(rendered.contains("180.00"));
        assert!(rendered.contains("ITBIS 16%:"));
        assert!(rendered.contains("80.00"));
        assert!(!rendered.contains("Monto Gravado:"));
        assert!(!rendered.contains("Total ITBIS:"));
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
    fn test_document_type_override_from_custom_fields() {
        // Explicit override (used by Orders module: Cotización, Preventa, Conduce)
        let mut fields = std::collections::HashMap::new();
        fields.insert(
            "document_type_label".to_string(),
            serde_json::json!("Cotización"),
        );
        let custom_fields = Some(fields);
        assert_eq!(
            FiscalInvoiceTemplate::get_document_type_override(&custom_fields),
            Some("Cotización".to_string())
        );

        // camelCase variant
        let mut fields2 = std::collections::HashMap::new();
        fields2.insert(
            "documentTypeLabel".to_string(),
            serde_json::json!("Preventa"),
        );
        let custom_fields2 = Some(fields2);
        assert_eq!(
            FiscalInvoiceTemplate::get_document_type_override(&custom_fields2),
            Some("Preventa".to_string())
        );

        // Empty string is treated as missing (so it falls back to default)
        let mut fields3 = std::collections::HashMap::new();
        fields3.insert("document_type_label".to_string(), serde_json::json!(""));
        let custom_fields3 = Some(fields3);
        assert_eq!(
            FiscalInvoiceTemplate::get_document_type_override(&custom_fields3),
            None
        );

        // No override at all
        assert_eq!(
            FiscalInvoiceTemplate::get_document_type_override(&None),
            None
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
