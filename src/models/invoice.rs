use serde::{Deserialize, Serialize};
use chrono::{DateTime, NaiveDate, Utc};
use std::collections::HashMap;
use super::{CompanyInfo, CustomerInfo, RenderOptions};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceRequest {
    pub template_id: String,
    pub data: InvoiceData,
    pub options: Option<RenderOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceData {
    pub company: CompanyInfo,
    pub customer: CustomerInfo,
    pub invoice: InvoiceInfo,
    pub items: Vec<InvoiceItem>,
    pub totals: Option<InvoiceTotals>, // Si no se proporciona, se calcula
    pub payment_info: Option<PaymentInfo>,
    pub notes: Option<String>,
    pub custom_fields: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceInfo {
    pub number: String,
    pub date: NaiveDate,
    pub due_date: NaiveDate,
    pub po_number: Option<String>,
    pub payment_terms: String,
    pub currency: String,
    pub exchange_rate: Option<f64>,
    pub tax_rate: f64,
    pub discount_rate: Option<f64>,
    pub status: Option<InvoiceStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    Draft,
    Sent,
    Paid,
    Partial,
    Overdue,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceItem {
    pub code: Option<String>,
    pub description: String,
    pub quantity: f64,
    pub unit: Option<String>, // "hrs", "units", "kg"
    pub unit_price: f64,
    pub discount_percent: Option<f64>,
    pub discount_amount: Option<f64>,
    pub tax_rate: Option<f64>,
    pub tax_amount: Option<f64>,
    pub total: Option<f64>, // Si no se proporciona, se calcula
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceTotals {
    pub subtotal: f64,
    pub discount_total: f64,
    pub tax_total: f64,
    pub shipping: Option<f64>,
    pub grand_total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentInfo {
    pub bank_name: Option<String>,
    pub account_name: Option<String>,
    pub account_number: Option<String>,
    pub routing_number: Option<String>,
    pub swift_code: Option<String>,
    pub payment_methods: Option<Vec<PaymentMethod>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethod {
    pub method_type: String, // "bank_transfer", "credit_card", "paypal"
    pub details: HashMap<String, String>,
}

impl InvoiceData {
    pub fn calculate_totals(&self) -> InvoiceTotals {
        let mut subtotal = 0.0;
        let mut discount_total = 0.0;
        let mut tax_total = 0.0;

        for item in &self.items {
            let item_subtotal = item.quantity * item.unit_price;

            // Calcular descuento
            let discount = if let Some(percent) = item.discount_percent {
                item_subtotal * (percent / 100.0)
            } else {
                item.discount_amount.unwrap_or(0.0)
            };

            let discounted = item_subtotal - discount;

            // Calcular impuesto
            let tax = if let Some(rate) = item.tax_rate {
                discounted * (rate / 100.0)
            } else {
                item.tax_amount.unwrap_or(0.0)
            };

            subtotal += item_subtotal;
            discount_total += discount;
            tax_total += tax;
        }

        // Aplicar descuento global si existe
        if let Some(global_discount) = self.invoice.discount_rate {
            let global_discount_amount = subtotal * (global_discount / 100.0);
            discount_total += global_discount_amount;
        }

        let grand_total = subtotal - discount_total + tax_total;

        InvoiceTotals {
            subtotal,
            discount_total,
            tax_total,
            shipping: None,
            grand_total,
        }
    }
}