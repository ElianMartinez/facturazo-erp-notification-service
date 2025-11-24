use anyhow::{Result, Context};
use serde_json::Value;
use serde::{Deserialize, Serialize};
use crate::templates::template_trait::{TypstTemplate, utils};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotationItem {
    pub quantity: u32,
    pub description: String,
    pub price: f64,
    pub tax_rate: f64,
}

impl QuotationItem {
    pub fn tax_amount(&self) -> f64 {
        self.price * self.quantity as f64 * self.tax_rate
    }

    pub fn total(&self) -> f64 {
        self.price * self.quantity as f64 * (1.0 + self.tax_rate)
    }

    pub fn subtotal(&self) -> f64 {
        self.price * self.quantity as f64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    pub name: String,
    pub rnc_cedula: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    pub name: String,
    pub description: String,
    pub logo_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MonthlyRent {
    pub description: String,
    pub subtotal: f64,
    pub tax_rate: f64,
}

impl MonthlyRent {
    pub fn tax_amount(&self) -> f64 {
        self.subtotal * self.tax_rate
    }

    pub fn total(&self) -> f64 {
        self.subtotal * (1.0 + self.tax_rate)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QuotationData {
    pub company: Company,
    pub customer: Customer,
    pub items: Vec<QuotationItem>,
    pub discount: f64,
    pub date: String,
    pub quotation_number: String,
    pub monthly_rent: Option<MonthlyRent>,
}

impl QuotationData {
    pub fn subtotal(&self) -> f64 {
        self.items.iter().map(|item| item.subtotal()).sum()
    }

    pub fn total_tax(&self) -> f64 {
        self.items.iter().map(|item| item.tax_amount()).sum()
    }

    pub fn total_after_discount(&self) -> f64 {
        let subtotal_with_tax = self.subtotal() + self.total_tax();
        subtotal_with_tax - self.discount
    }
}

pub struct QuotationTemplate;

impl QuotationTemplate {
    pub fn new() -> Self {
        Self
    }

    fn format_currency(&self, value: f64) -> String {
        let int_part = value as i64;
        let decimal_part = ((value - int_part as f64) * 100.0).round() as i64;

        // Formatear parte entera con comas
        let int_str = int_part.to_string();
        let mut result = String::new();
        let mut count = 0;

        for ch in int_str.chars().rev() {
            if count == 3 {
                result.push(',');
                count = 0;
            }
            result.push(ch);
            count += 1;
        }

        format!("{}.{:02}", result.chars().rev().collect::<String>(), decimal_part)
    }

    fn format_items(&self, items: &[QuotationItem]) -> String {
        let mut result = String::new();
        for item in items {
            result.push_str(&format!(
                "  [{}], [{}], [USD {}], [USD {}], [USD {}],\n",
                item.quantity,
                utils::escape_typst(&item.description),
                self.format_currency(item.price),
                self.format_currency(item.tax_amount()),
                self.format_currency(item.total())
            ));
        }
        result
    }

    fn format_customer_info(&self, customer: &Customer) -> String {
        let mut info = format!("  #grid(\n    columns: (auto, 1fr),\n    column-gutter: 10pt,\n    row-gutter: 5pt,\n\n    text(size: 10pt, weight: \"bold\")[Nombre:],\n    text(size: 10pt)[{}],\n", customer.name);

        if let Some(rnc) = &customer.rnc_cedula {
            info.push_str(&format!("    text(size: 10pt, weight: \"bold\")[RNC/Cédula:],\n    text(size: 10pt)[{}],\n", rnc));
        }

        if let Some(phone) = &customer.phone {
            info.push_str(&format!("    text(size: 10pt, weight: \"bold\")[Teléfono:],\n    text(size: 10pt)[{}],\n", phone));
        }

        info.push_str("  )");
        info
    }

    fn format_monthly_rent(&self, monthly_rent: &Option<MonthlyRent>) -> String {
        if let Some(rent) = monthly_rent {
            format!(r#"#rect(
      width: 100%,
      fill: rgb("fff9e6"),
      stroke: 1pt + rgb("f39c12"),
      radius: 3pt,
      inset: 10pt,
    )[
      #text(size: 12pt, weight: "bold", fill: rgb("10162B"))[Renta Mensual (Recurrente)]

      #v(6pt)

      #table(
        columns: (1fr, auto),
        stroke: none,
        align: (left, right),
        inset: 6pt,
        row-gutter: 6pt,

        [{}], [USD {}],
        [ITBIS ({}%)], [USD {}],

        table.hline(stroke: 0.5pt + gray),

        [#text(weight: "bold")[Total mensual]], [#text(weight: "bold", fill: rgb("f39c12"))[USD {}]],
      )
    ]"#,
                utils::escape_typst(&rent.description),
                self.format_currency(rent.subtotal),
                (rent.tax_rate * 100.0) as i32,
                self.format_currency(rent.tax_amount()),
                self.format_currency(rent.total())
            )
        } else {
            // Si no hay renta mensual, devolver un espacio vacío
            String::new()
        }
    }
}

impl TypstTemplate for QuotationTemplate {
    fn template_id(&self) -> &str {
        "quotation"
    }

    fn validate(&self, data: &Value) -> Result<()> {
        if !data.is_object() {
            anyhow::bail!("Los datos deben ser un objeto JSON");
        }

        let required_fields = ["company", "customer", "items", "discount", "date", "quotation_number"];
        for field in required_fields.iter() {
            if data.get(field).is_none() {
                anyhow::bail!("Campo requerido '{}' no encontrado", field);
            }
        }

        Ok(())
    }

    fn generate(&self, data: &Value) -> Result<String> {
        let quotation: QuotationData = serde_json::from_value(data.clone())
            .context("Error deserializando datos de cotización")?;

        let content = format!(r#"#set page(
  paper: "us-letter",
  margin: (top: 1.5cm, bottom: 1.5cm, left: 1.5cm, right: 1.5cm),
)

#set text(
  font: "Arial",
  size: 10pt,
)

#grid(
  columns: (1fr, 1fr),
  align: (left, right),

  // Columna izquierda - Logo y empresa
  [
    #grid(
      columns: (auto, auto),
      column-gutter: 10pt,
      align: (left, left + horizon),

      image("{}", width: 45pt),

      [
        #text(size: 18pt, weight: "bold")[{}]
        #text(size: 8pt, style: "italic")[{}]
        #text(size: 8pt, fill: gray)[
          RNC: 131-64038-9 #h(5pt) Tel: 829-663-0497
          #linebreak()
          Nicola Silfa Canario #7, Alma Rosa 1, Santo Domingo Este
        ]
      ]
    )
  ],

  // Columna derecha - Fecha y número
  [
    #align(right + horizon)[
      #text(size: 9pt)[
        *Fecha:* {}
        #linebreak()
        *Cotización No.:* {}
      ]
    ]
  ]
)

#v(8pt)

#rect(
  width: 100%,
  fill: rgb("10162B"),
  radius: 3pt,
  inset: 8pt,
)[
  #align(center)[
    #text(fill: white, size: 16pt, weight: "bold")[COTIZACIÓN]
  ]
]

#v(12pt)

#rect(
  width: 100%,
  fill: rgb("f8f9fa"),
  stroke: 0.5pt + rgb("10162B"),
  radius: 3pt,
  inset: 12pt,
)[
  #text(size: 11pt, weight: "bold", fill: rgb("10162B"))[Información del Cliente]

  #v(8pt)

  {}
]

#v(15pt)

#text(size: 12pt, weight: "bold")[Detalle de la Cotización]

#v(8pt)

#table(
  columns: (1fr, 4fr, 1.5fr, 1.5fr, 1.5fr),
  stroke: 0.5pt + gray,
  inset: 8pt,

  table.header(
    [*Cantidad*],
    [*Descripción*],
    [*Precio*],
    [*ITBIS*],
    [*Valor*],
  ),

{}
)

#v(15pt)

#grid(
  columns: (1fr, auto),
  column-gutter: 15pt,
  align: (left, right),

  // Columna izquierda - Cuadro amarillo de Renta Mensual
  [
    {}
  ],

  // Columna derecha - Totales
  [
    #table(
      columns: (150pt, 100pt),
      stroke: none,
      align: (right, right),
      inset: 5pt,

      [*Subtotal:*], [USD {}],
      [*Total ITBIS:*], [USD {}],

      table.hline(stroke: 1.5pt),

      [#text(size: 14pt, weight: "bold")[Total:]],
      [#text(size: 14pt, weight: "bold")[USD {}]],
    )
  ]
)

#v(20pt)

#align(center)[
  #rect(
    width: 100%,
    fill: rgb("f0f4f8"),
    radius: 5pt,
    inset: 20pt,
  )[
    #text(size: 11pt, style: "italic")[
      *¡Gracias por su confianza!*

      Agradecemos la oportunidad de presentarle nuestra cotización. Estamos comprometidos
      con brindarle el mejor servicio y calidad. Para cualquier consulta o aclaración,
      no dude en contactarnos.

      Esta cotización tiene una validez de 30 días a partir de la fecha de emisión.
    ]
  ]
]


"#,
            quotation.company.logo_path,
            quotation.company.name,
            quotation.company.description,
            quotation.date,
            quotation.quotation_number,
            self.format_customer_info(&quotation.customer),
            self.format_items(&quotation.items),
            self.format_monthly_rent(&quotation.monthly_rent),
            self.format_currency(quotation.subtotal()),
            self.format_currency(quotation.total_tax()),
            self.format_currency(quotation.total_after_discount())
        );

        Ok(content)
    }
}