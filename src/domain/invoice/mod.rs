//! Invoice DTOs - Data Transfer Objects for invoice documents
//!
//! This module only contains data structures for receiving invoice information
//! from the core service. NO business logic, validations or calculations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::domain::fiscal::{NCF, RNC, TaxId};

/// Invoice data (comes fully calculated from core service)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceData {
    // Identity
    pub id: String,
    pub invoice_number: String,
    pub tenant_id: String,

    // Fiscal information (from core service)
    pub ncf: Option<NCF>,
    pub ncf_expiry_date: Option<String>,

    // Seller information
    pub seller: SellerData,

    // Customer information
    pub customer: CustomerData,

    // Invoice items (pre-calculated)
    pub items: Vec<InvoiceItemData>,

    // Amounts (pre-calculated by core service)
    pub subtotal: f64,
    pub discount_amount: f64,
    pub itbis_amount: f64,
    pub total_amount: f64,

    // Currency and formatting
    pub currency: String,           // "DOP", "USD", etc
    pub currency_symbol: Option<String>, // "RD$", "US$", etc

    // Dates
    pub issue_date: String,
    pub due_date: Option<String>,
    pub paid_date: Option<String>,

    // Status and metadata
    pub status: String,
    pub payment_terms: Option<String>,
    pub payment_method: Option<String>,
    pub notes: Option<String>,

    // Display options
    pub show_watermark: Option<bool>,
    pub watermark_text: Option<String>,
}

/// Seller data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SellerData {
    pub company_name: String,
    pub trade_name: Option<String>,
    pub rnc: Option<RNC>,
    pub address: Option<AddressData>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub logo_url: Option<String>,
}

/// Customer data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerData {
    pub name: String,
    pub tax_id: Option<TaxId>,
    pub address: Option<AddressData>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub contact_person: Option<String>,
}

/// Address data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressData {
    pub street: Option<String>,
    pub number: Option<String>,
    pub neighborhood: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub formatted: Option<String>, // Pre-formatted address string
}

/// Invoice item data (pre-calculated)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceItemData {
    pub sequence: Option<u32>,
    pub description: String,
    pub quantity: f64,
    pub unit_of_measure: String,
    pub unit_price: f64,

    // Pre-calculated amounts
    pub subtotal: f64,
    pub discount_amount: f64,
    pub discount_percentage: Option<f64>,
    pub itbis_amount: f64,
    pub total: f64,

    // Display formatting
    pub formatted_price: Option<String>,
    pub formatted_total: Option<String>,
}

/// Payment status for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvoiceStatus {
    Draft,
    Issued,
    Sent,
    Paid,
    Overdue,
    Cancelled,
    Voided,
}

impl InvoiceStatus {
    pub fn as_str(&self) -> &str {
        match self {
            InvoiceStatus::Draft => "draft",
            InvoiceStatus::Issued => "issued",
            InvoiceStatus::Sent => "sent",
            InvoiceStatus::Paid => "paid",
            InvoiceStatus::Overdue => "overdue",
            InvoiceStatus::Cancelled => "cancelled",
            InvoiceStatus::Voided => "voided",
        }
    }
}

/// Currency display info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Currency {
    pub code: String,     // "DOP", "USD", "EUR"
    pub symbol: String,   // "RD$", "$", "€"
    pub name: String,     // "Peso Dominicano", etc
}