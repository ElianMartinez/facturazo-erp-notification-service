//! Cash Register / Cuadre de Caja template

use crate::templates::template_models::{
    CashRegisterData, CashRegisterItem, CashRegisterStatus, Denomination, PaymentMethodBalance,
};
use crate::templates::template_trait::{utils, TypstTemplate};
use anyhow::{Context, Result};
use serde_json::Value;

pub struct CashRegisterTemplate;

impl CashRegisterTemplate {
    pub fn new() -> Self {
        Self
    }

    fn format_income_rows(&self, items: &[CashRegisterItem]) -> String {
        items
            .iter()
            .map(|item| {
                let qty = item
                    .quantity
                    .map(|q| q.to_string())
                    .unwrap_or_else(|| "—".to_string());
                format!(
                    "    [{}], [#align(center)[{}]], [#align(right)[{}]],",
                    utils::escape_typst(&item.concept),
                    qty,
                    Self::format_currency(item.amount)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn format_denomination_items(&self, denoms: &[Denomination]) -> String {
        denoms
            .iter()
            .map(|d| {
                format!(
                    r#"      box(
        width: 100%,
        stroke: 1pt + rgb("E2E8F0"),
        radius: 4pt,
        inset: 8pt,
        [
          #grid(
            columns: (1fr, auto),
            [#text(size: 9pt, fill: rgb("64748B"))[${}× {}]],
            [#text(size: 10pt, weight: "semibold")[{}]]
          )
        ]
      ),"#,
                    d.value as i64,
                    d.quantity,
                    Self::format_currency(d.total)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn format_payment_method_rows(&self, methods: &[PaymentMethodBalance]) -> String {
        methods
            .iter()
            .map(|m| {
                let diff_color = if m.difference > 0.0 {
                    "059669"
                } else if m.difference < 0.0 {
                    "DC2626"
                } else {
                    "64748B"
                };
                let diff_prefix = if m.difference > 0.0 { "+" } else { "" };
                format!(
                    r#"    [{}],
    [#align(center)[{}]],
    [#align(center)[{}]],
    [#align(right)[#text(fill: rgb("{}"))[{}{}]]],"#,
                    utils::escape_typst(&m.method),
                    Self::format_currency(m.system_amount),
                    Self::format_currency(m.actual_amount),
                    diff_color,
                    diff_prefix,
                    Self::format_currency(m.difference)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn format_currency(amount: f64) -> String {
        let formatted = format!("{:.2}", amount.abs());
        let parts: Vec<&str> = formatted.split('.').collect();
        let integer_part = parts[0];
        let decimal_part = parts.get(1).unwrap_or(&"00");

        let mut result = String::new();
        let chars: Vec<char> = integer_part.chars().rev().collect();
        for (i, ch) in chars.iter().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.push(',');
            }
            result.push(*ch);
        }
        let integer_formatted: String = result.chars().rev().collect();

        if amount < 0.0 {
            format!("-\\${}.{}", integer_formatted, decimal_part)
        } else {
            format!("\\${}.{}", integer_formatted, decimal_part)
        }
    }

    fn get_status_badge(&self, status: &CashRegisterStatus) -> String {
        let (bg_color, text_color, label) = match status {
            CashRegisterStatus::Balanced => ("F1F5F9", "64748B", "Cuadrado"),
            CashRegisterStatus::Surplus => ("ECFDF5", "059669", "Sobrante"),
            CashRegisterStatus::Shortage => ("FEF2F2", "DC2626", "Faltante"),
        };

        format!(
            r#"#box(
      fill: rgb("{}"),
      radius: 12pt,
      inset: (x: 10pt, y: 4pt),
      [
        #text(size: 9pt, weight: "semibold", fill: rgb("{}"))[\u{{25CF}} {}]
      ]
    )"#,
            bg_color, text_color, label
        )
    }

    fn generate_typst_content(&self, data: &CashRegisterData) -> Result<String> {
        let company = &data.company_info;
        let address = format!("{}, {}", company.address.street, company.address.city);

        // Status badge
        let status_badge = self.get_status_badge(&data.status);

        // Income rows
        let income_rows = self.format_income_rows(&data.income);
        let total_income = Self::format_currency(data.summary.total_income);

        // Expense rows
        let expense_rows = self.format_income_rows(&data.expenses);
        let total_expenses = Self::format_currency(data.summary.total_expenses);

        // Denominations section
        let denominations_section = if let Some(ref denoms) = data.denominations {
            let denom_items = self.format_denomination_items(denoms);
            let total_cash: f64 = denoms.iter().map(|d| d.total).sum();
            format!(
                r#"
    // Desglose de Efectivo
    #box(
      width: 100%,
      stroke: 1pt + rgb("E2E8F0"),
      radius: 6pt,
      [
        #box(
          width: 100%,
          fill: white,
          inset: 10pt,
          [#text(size: 10pt, weight: "bold")[DESGLOSE DE EFECTIVO]]
        )
        #line(length: 100%, stroke: 1pt + rgb("E2E8F0"))
        #box(
          width: 100%,
          fill: rgb("F8FAFC"),
          inset: 10pt,
          [
            #grid(
              columns: (1fr, 1fr),
              gutter: 6pt,
{}
            )
            #v(8pt)
            #box(
              width: 100%,
              fill: rgb("EFF6FF"),
              stroke: 1pt + rgb("2563EB"),
              radius: 4pt,
              inset: 10pt,
              [
                #grid(
                  columns: (1fr, auto),
                  [#text(size: 9pt, weight: "semibold", fill: rgb("2563EB"))[TOTAL EFECTIVO]],
                  [#text(size: 13pt, weight: "bold", fill: rgb("2563EB"))[{}]]
                )
              ]
            )
          ]
        )
      ]
    )"#,
                denom_items,
                Self::format_currency(total_cash)
            )
        } else {
            String::new()
        };

        // Payment methods rows
        let payment_rows = self.format_payment_method_rows(&data.payment_methods);

        // Final balance colors
        let (balance_color, balance_label) = if data.final_balance.difference > 0.0 {
            ("4ADE80", format!("SOBRANTE DE {}", Self::format_currency(data.final_balance.difference)))
        } else if data.final_balance.difference < 0.0 {
            ("F87171", format!("FALTANTE DE {}", Self::format_currency(data.final_balance.difference.abs())))
        } else {
            ("94A3B8", "CUADRADO".to_string())
        };

        let diff_prefix = if data.final_balance.difference > 0.0 {
            "+"
        } else {
            ""
        };

        let content = format!(
            r#"#set document(title: "Cuadre de Caja - {}", author: "{}")
#set page(
  paper: "a4",
  margin: (top: 12mm, bottom: 12mm, left: 15mm, right: 15mm),
)
#set text(font: "Inter", size: 10pt, fill: rgb("1E293B"))

// ============================================
// HEADER
// ============================================
#grid(
  columns: (1fr, auto),
  [
    #text(size: 18pt, weight: "bold")[Cuadre de Caja]
    #v(2pt)
    #text(size: 12pt, weight: "semibold", fill: rgb("2563EB"))[{}]
    #v(2pt)
    #text(size: 9pt, fill: rgb("64748B"))[RNC: {} · {}]
  ],
  [
    #align(right)[
      #text(size: 16pt, weight: "bold")[{}]
      #v(4pt)
      #text(size: 9pt, fill: rgb("64748B"))[{}]
    ]
  ]
)

#v(8pt)
#line(length: 100%, stroke: 2pt + rgb("2563EB"))
#v(12pt)

// ============================================
// INFO BAR
// ============================================
#grid(
  columns: (1fr, 1fr, 1fr),
  gutter: 12pt,
  // Cajero
  box(
    width: 100%,
    fill: rgb("F8FAFC"),
    stroke: 1pt + rgb("E2E8F0"),
    radius: 6pt,
    inset: 10pt,
    [
      #text(size: 8pt, weight: "semibold", fill: rgb("64748B"))[CAJERO]
      #v(3pt)
      #text(size: 12pt, weight: "semibold")[{}]
    ]
  ),
  // Transacciones
  box(
    width: 100%,
    fill: rgb("F8FAFC"),
    stroke: 1pt + rgb("E2E8F0"),
    radius: 6pt,
    inset: 10pt,
    [
      #text(size: 8pt, weight: "semibold", fill: rgb("64748B"))[TRANSACCIONES]
      #v(3pt)
      #text(size: 12pt, weight: "semibold")[{}]
    ]
  ),
  // Estado
  box(
    width: 100%,
    fill: rgb("F8FAFC"),
    stroke: 1pt + rgb("E2E8F0"),
    radius: 6pt,
    inset: 10pt,
    [
      #text(size: 8pt, weight: "semibold", fill: rgb("64748B"))[ESTADO]
      #v(3pt)
      {}
    ]
  ),
)

#v(15pt)

// ============================================
// MAIN CONTENT - TWO COLUMNS
// ============================================
#grid(
  columns: (1fr, 1fr),
  gutter: 18pt,

  // ========== LEFT COLUMN ==========
  [
    // INGRESOS
    #text(size: 10pt, weight: "bold")[INGRESOS]
    #h(8pt)
    #line(length: 1fr, stroke: 1pt + rgb("E2E8F0"))
    #v(8pt)

    #table(
      columns: (1fr, auto, auto),
      stroke: 1pt + rgb("E2E8F0"),
      inset: 9pt,
      fill: (col, row) => if row == 0 {{ rgb("F8FAFC") }} else {{ white }},
      [#text(size: 8pt, weight: "semibold", fill: rgb("64748B"))[CONCEPTO]],
      [#text(size: 8pt, weight: "semibold", fill: rgb("64748B"))[CANT.]],
      [#align(right)[#text(size: 8pt, weight: "semibold", fill: rgb("64748B"))[MONTO]]],
{}
    )

    #v(8pt)
    #box(
      width: 100%,
      fill: rgb("2563EB"),
      radius: 4pt,
      inset: 10pt,
      [
        #grid(
          columns: (1fr, auto),
          [#text(size: 10pt, weight: "semibold", fill: white)[TOTAL INGRESOS]],
          [#text(size: 14pt, weight: "bold", fill: white)[{}]]
        )
      ]
    )

    #v(15pt)

    // EGRESOS
    #text(size: 10pt, weight: "bold")[EGRESOS]
    #h(8pt)
    #line(length: 1fr, stroke: 1pt + rgb("E2E8F0"))
    #v(8pt)

    #table(
      columns: (1fr, auto, auto),
      stroke: 1pt + rgb("E2E8F0"),
      inset: 9pt,
      fill: (col, row) => if row == 0 {{ rgb("F8FAFC") }} else {{ white }},
      [#text(size: 8pt, weight: "semibold", fill: rgb("64748B"))[CONCEPTO]],
      [#text(size: 8pt, weight: "semibold", fill: rgb("64748B"))[CANT.]],
      [#align(right)[#text(size: 8pt, weight: "semibold", fill: rgb("64748B"))[MONTO]]],
{}
    )

    #v(8pt)
    #box(
      width: 100%,
      fill: rgb("475569"),
      radius: 4pt,
      inset: 10pt,
      [
        #grid(
          columns: (1fr, auto),
          [#text(size: 10pt, weight: "semibold", fill: white)[TOTAL EGRESOS]],
          [#text(size: 14pt, weight: "bold", fill: white)[{}]]
        )
      ]
    )
  ],

  // ========== RIGHT COLUMN ==========
  [
    // RESUMEN GENERAL
    #box(
      width: 100%,
      stroke: 1pt + rgb("E2E8F0"),
      radius: 6pt,
      [
        #box(
          width: 100%,
          fill: white,
          inset: 10pt,
          [#text(size: 10pt, weight: "bold")[RESUMEN GENERAL]]
        )
        #line(length: 100%, stroke: 1pt + rgb("E2E8F0"))
        #box(
          width: 100%,
          fill: rgb("F8FAFC"),
          inset: 10pt,
          [
            #grid(
              columns: (1fr, auto),
              row-gutter: 7pt,
              [#text(size: 10pt, fill: rgb("64748B"))[Total Ingresos]],
              [#text(size: 10pt, weight: "semibold")[{}]],
              [#text(size: 10pt, fill: rgb("64748B"))[Total Egresos]],
              [#text(size: 10pt, weight: "semibold")[{}]],
            )
            #v(4pt)
            #box(
              width: 100%,
              fill: rgb("EFF6FF"),
              radius: 4pt,
              inset: 10pt,
              [
                #grid(
                  columns: (1fr, auto),
                  [#text(size: 10pt, weight: "semibold")[Total a Cuadrar]],
                  [#text(size: 12pt, weight: "bold", fill: rgb("2563EB"))[{}]]
                )
              ]
            )
            #v(4pt)
            #grid(
              columns: (1fr, auto),
              [#text(size: 10pt, fill: rgb("64748B"))[Total en Caja]],
              [#text(size: 10pt, weight: "semibold")[{}]],
            )
          ]
        )
      ]
    )

    #v(15pt)
{}
  ]
)

#v(18pt)

// ============================================
// CUADRE POR MÉTODO DE PAGO
// ============================================
#text(size: 10pt, weight: "bold")[CUADRE POR MÉTODO DE PAGO]
#h(8pt)
#line(length: 1fr, stroke: 1pt + rgb("E2E8F0"))
#v(8pt)

#table(
  columns: (1fr, 1fr, 1fr, auto),
  stroke: 1pt + rgb("E2E8F0"),
  inset: 9pt,
  fill: (col, row) => if row == 0 {{ rgb("F8FAFC") }} else {{ white }},
  [#text(size: 8pt, weight: "semibold", fill: rgb("64748B"))[MÉTODO]],
  [#align(center)[#text(size: 8pt, weight: "semibold", fill: rgb("64748B"))[SISTEMA]]],
  [#align(center)[#text(size: 8pt, weight: "semibold", fill: rgb("64748B"))[EN CAJA]]],
  [#align(right)[#text(size: 8pt, weight: "semibold", fill: rgb("64748B"))[DIFERENCIA]]],
{}
)

// Total row
#box(
  width: 100%,
  fill: rgb("2563EB"),
  inset: 9pt,
  [
    #grid(
      columns: (1fr, 1fr, 1fr, auto),
      [#text(weight: "bold", fill: white)[Total]],
      [#align(center)[#text(weight: "bold", fill: white)[{}]]],
      [#align(center)[#text(weight: "bold", fill: white)[{}]]],
      [#align(right)[#text(weight: "bold", fill: white)[{}{}]]]
    )
  ]
)

#v(16pt)

// ============================================
// BALANCE FINAL
// ============================================
#box(
  width: 100%,
  radius: 8pt,
  clip: true,
  [
    // Header oscuro
    #box(
      width: 100%,
      fill: rgb("1E293B"),
      inset: 12pt,
      [
        #grid(
          columns: (1fr, auto),
          [#text(size: 12pt, weight: "semibold", fill: white)[BALANCE FINAL]],
          [#text(size: 26pt, weight: "bold", fill: rgb("{}"))[{}{}]]
        )
      ]
    )
    // Detalle
    #box(
      width: 100%,
      fill: rgb("F8FAFC"),
      stroke: (left: 1pt + rgb("E2E8F0"), right: 1pt + rgb("E2E8F0"), bottom: 1pt + rgb("E2E8F0")),
      inset: 10pt,
      [
        #grid(
          columns: (1fr, 1fr, 1fr),
          [
            #box[
              #box(width: 8pt, height: 8pt, fill: rgb("2563EB"), radius: 50%)
              #h(6pt)
              #text(size: 9pt, fill: rgb("64748B"))[Sistema:]
              #h(4pt)
              #text(size: 9pt, weight: "semibold")[{}]
            ]
          ],
          [
            #box[
              #box(width: 8pt, height: 8pt, fill: rgb("475569"), radius: 50%)
              #h(6pt)
              #text(size: 9pt, fill: rgb("64748B"))[En Caja:]
              #h(4pt)
              #text(size: 9pt, weight: "semibold")[{}]
            ]
          ],
          [
            #align(right)[
              #box(
                fill: rgb("{}"),
                radius: 12pt,
                inset: (x: 10pt, y: 4pt),
                [#text(size: 9pt, weight: "semibold", fill: rgb("{}"))[{}]]
              )
            ]
          ]
        )
      ]
    )
  ]
)

#v(20pt)

// ============================================
// FOOTER
// ============================================
#line(length: 100%, stroke: 1pt + rgb("E2E8F0"))
#v(8pt)
#grid(
  columns: (1fr, auto),
  [#text(size: 8pt, fill: rgb("64748B"))[Documento generado por {} · Sistema de Gestión]],
  [#text(size: 8pt, fill: rgb("64748B"))[Generado: {}]]
)
"#,
            // Document metadata
            data.document_code,
            company.name,
            // Header
            utils::escape_typst(&company.name),
            company.tax_id,
            utils::escape_typst(&address),
            data.document_code,
            data.date,
            // Info bar
            utils::escape_typst(&data.cashier.name),
            data.transactions_count,
            status_badge,
            // Income table
            income_rows,
            total_income,
            // Expenses table
            expense_rows,
            total_expenses,
            // Summary
            Self::format_currency(data.summary.total_income),
            Self::format_currency(data.summary.total_expenses),
            Self::format_currency(data.summary.total_to_balance),
            Self::format_currency(data.summary.total_in_register),
            // Denominations
            denominations_section,
            // Payment methods
            payment_rows,
            // Payment totals
            Self::format_currency(data.final_balance.system_total),
            Self::format_currency(data.final_balance.actual_total),
            diff_prefix,
            Self::format_currency(data.final_balance.difference),
            // Balance final
            balance_color,
            diff_prefix,
            Self::format_currency(data.final_balance.difference),
            Self::format_currency(data.final_balance.system_total),
            Self::format_currency(data.final_balance.actual_total),
            if data.final_balance.difference > 0.0 { "ECFDF5" } else if data.final_balance.difference < 0.0 { "FEF2F2" } else { "F1F5F9" },
            if data.final_balance.difference > 0.0 { "059669" } else if data.final_balance.difference < 0.0 { "DC2626" } else { "64748B" },
            balance_label,
            // Footer
            utils::escape_typst(&company.name),
            data.date,
        );

        Ok(content)
    }
}

impl TypstTemplate for CashRegisterTemplate {
    fn generate(&self, data: &Value) -> Result<String> {
        let cash_register: CashRegisterData = serde_json::from_value(data.clone())
            .context("Error deserializando datos de cuadre de caja")?;

        self.generate_typst_content(&cash_register)
    }

    fn template_id(&self) -> &str {
        "cash_register"
    }

    fn validate(&self, data: &Value) -> Result<()> {
        // Validate required fields - accept both camelCase and snake_case
        let obj = data.as_object()
            .context("Los datos deben ser un objeto JSON")?;

        // Pairs of (camelCase, snake_case) for required fields
        let required_fields = [
            ("documentCode", "document_code"),
            ("date", "date"),
            ("companyInfo", "company_info"),
            ("cashier", "cashier"),
            ("transactionsCount", "transactions_count"),
            ("status", "status"),
            ("income", "income"),
            ("expenses", "expenses"),
            ("summary", "summary"),
            ("paymentMethods", "payment_methods"),
            ("finalBalance", "final_balance"),
        ];

        for (camel, snake) in required_fields {
            if !obj.contains_key(camel) && !obj.contains_key(snake) {
                anyhow::bail!("Campo requerido faltante: {} / {}", camel, snake);
            }
        }

        Ok(())
    }

    fn description(&self) -> &str {
        "Cuadre de Caja - Reporte de cierre de caja con desglose de ingresos, egresos y métodos de pago"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_cash_register_template() {
        let template = CashRegisterTemplate::new();

        let data = json!({
            "documentCode": "SQ-138",
            "date": "12/02/2025, 4:00 a.m.",
            "companyInfo": {
                "name": "LE CROISSANT DORE",
                "taxId": "101707399",
                "address": {
                    "street": "Enriquillo, No. 25",
                    "city": "Santo Domingo",
                    "country": "DO"
                }
            },
            "cashier": {
                "name": "Caja 4"
            },
            "transactionsCount": 21,
            "status": "surplus",
            "income": [
                { "concept": "POS", "quantity": 21, "amount": 28444.00 },
                { "concept": "Recibos", "quantity": 0, "amount": 0.00 },
                { "concept": "Efectivo Inicial", "amount": 0.00 }
            ],
            "expenses": [
                { "concept": "Comprobantes", "quantity": 0, "amount": 0.00 },
                { "concept": "Pagos con NC", "quantity": 0, "amount": 0.00 }
            ],
            "summary": {
                "totalIncome": 28444.00,
                "totalExpenses": 0.00,
                "totalToBalance": 28444.00,
                "totalInRegister": 29120.27
            },
            "denominations": [
                { "value": 1000, "quantity": 2, "total": 2000.00 },
                { "value": 100, "quantity": 3, "total": 300.00 },
                { "value": 50, "quantity": 2, "total": 100.00 },
                { "value": 5, "quantity": 1, "total": 5.00 }
            ],
            "paymentMethods": [
                { "method": "Efectivo", "systemAmount": 2392.70, "actualAmount": 2405.00, "difference": 12.30 },
                { "method": "Cheque", "systemAmount": 0.00, "actualAmount": 0.00, "difference": 0.00 },
                { "method": "Tarjeta", "systemAmount": 26715.24, "actualAmount": 26715.27, "difference": 0.03 },
                { "method": "Transferencia", "systemAmount": 0.00, "actualAmount": 0.00, "difference": 0.00 }
            ],
            "finalBalance": {
                "systemTotal": 29107.94,
                "actualTotal": 29120.27,
                "difference": 12.33
            }
        });

        let result = template.generate(&data);
        assert!(result.is_ok(), "Template generation failed: {:?}", result.err());

        let typst_content = result.unwrap();
        assert!(typst_content.contains("Cuadre de Caja"));
        assert!(typst_content.contains("LE CROISSANT DORE"));
        assert!(typst_content.contains("SQ-138"));
    }

    #[test]
    fn test_validate_cash_register() {
        let template = CashRegisterTemplate::new();

        let valid_data = json!({
            "documentCode": "SQ-001",
            "date": "01/01/2025",
            "companyInfo": {},
            "cashier": {},
            "transactionsCount": 0,
            "status": "balanced",
            "income": [],
            "expenses": [],
            "summary": {},
            "paymentMethods": [],
            "finalBalance": {}
        });

        assert!(template.validate(&valid_data).is_ok());

        let invalid_data = json!({
            "documentCode": "SQ-001"
        });

        assert!(template.validate(&invalid_data).is_err());
    }
}
