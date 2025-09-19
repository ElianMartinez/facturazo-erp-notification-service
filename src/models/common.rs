use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Pdf,
    Excel,
    Csv,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    High,    // < 1 min
    Normal,  // < 5 min
    Low,     // Best effort
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub state: String,
    pub zip: String,
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyInfo {
    pub name: String,
    pub logo_url: Option<String>,
    pub address: Address,
    pub tax_id: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerInfo {
    pub name: String,
    pub tax_id: Option<String>,
    pub address: Address,
    pub phone: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumberFormat {
    English,    // 1,234.56
    European,   // 1.234,56
    Indian,     // 12,34,567
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderOptions {
    pub locale: String,           // "es-DO", "en-US"
    pub currency: String,         // "DOP", "USD"
    pub currency_symbol: String,  // "$", "RD$"
    pub date_format: String,      // "DD/MM/YYYY", "MM/DD/YYYY"
    pub number_format: NumberFormat,
    pub include_qr: Option<bool>,
    pub watermark: Option<String>,
    pub page_size: Option<PageSize>,
    pub orientation: Option<Orientation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageSize {
    A4,
    Letter,
    Legal,
    A3,
    Custom { width: f32, height: f32 }, // in mm
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    Portrait,
    Landscape,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            locale: "es-DO".to_string(),
            currency: "DOP".to_string(),
            currency_symbol: "$".to_string(),
            date_format: "DD/MM/YYYY".to_string(),
            number_format: NumberFormat::English,
            include_qr: Some(false),
            watermark: None,
            page_size: Some(PageSize::Letter),
            orientation: Some(Orientation::Portrait),
        }
    }
}