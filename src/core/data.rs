use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TableData {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub column_widths: Option<Vec<f32>>,
    pub alignment: Option<Vec<ColumnAlign>>,
}

#[derive(Debug, Clone)]
pub enum ColumnAlign {
    Left,
    Center,
    Right,
}

impl TableData {
    pub fn new(headers: Vec<String>) -> Self {
        TableData {
            headers,
            rows: Vec::new(),
            column_widths: None,
            alignment: None,
        }
    }

    pub fn add_row(&mut self, row: Vec<String>) {
        self.rows.push(row);
    }

    pub fn with_column_widths(mut self, widths: Vec<f32>) -> Self {
        self.column_widths = Some(widths);
        self
    }

    pub fn with_alignment(mut self, alignment: Vec<ColumnAlign>) -> Self {
        self.alignment = Some(alignment);
        self
    }
}

#[derive(Debug, Clone)]
pub struct DocumentMetadata {
    pub title: String,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub custom_fields: HashMap<String, String>,
}

impl Default for DocumentMetadata {
    fn default() -> Self {
        DocumentMetadata {
            title: "Documento".to_string(),
            author: None,
            subject: None,
            keywords: Vec::new(),
            created_at: Utc::now(),
            custom_fields: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Money {
    pub amount: f64,
    pub currency: String,
    pub symbol: String,
}

impl Money {
    pub fn new(amount: f64) -> Self {
        Money {
            amount,
            currency: "DOP".to_string(),
            symbol: "$".to_string(),
        }
    }

    pub fn with_currency(mut self, currency: &str, symbol: &str) -> Self {
        self.currency = currency.to_string();
        self.symbol = symbol.to_string();
        self
    }

    pub fn format(&self) -> String {
        let formatted = format!("{:.2}", self.amount);
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

        // Escapar el símbolo $ para Typst
        let symbol = if self.symbol == "$" { "\\$" } else { &self.symbol };
        format!("{}{}.{}", symbol, result.chars().rev().collect::<String>(), decimal)
    }
}

#[derive(Debug, Clone)]
pub struct Address {
    pub street: String,
    pub city: String,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: String,
}

impl Address {
    pub fn format_multiline(&self) -> String {
        let mut lines = vec![self.street.clone()];

        let mut city_line = self.city.clone();
        if let Some(state) = &self.state {
            city_line.push_str(&format!(", {}", state));
        }
        if let Some(postal) = &self.postal_code {
            city_line.push_str(&format!(" {}", postal));
        }
        lines.push(city_line);
        lines.push(self.country.clone());

        lines.join("\n")
    }
}

#[derive(Debug, Clone)]
pub struct Company {
    pub name: String,
    pub tax_id: Option<String>,
    pub address: Option<Address>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
}

impl Company {
    pub fn new(name: String) -> Self {
        Company {
            name,
            tax_id: None,
            address: None,
            phone: None,
            email: None,
            website: None,
        }
    }
}