use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceData {
    pub invoice_number: String,
    pub issue_date: String,
    pub due_date: String,
    pub company_info: CompanyInfo,
    pub client_info: ClientInfo,
    pub items: Vec<InvoiceItem>,
    pub totals: InvoiceTotals,
    pub fiscal_info: Option<FiscalInfo>,
    pub payment_info: Option<PaymentInfo>,
    pub notes: Option<String>,
    pub custom_fields: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyInfo {
    pub name: String,
    pub legal_name: Option<String>,
    pub tax_id: String,
    pub address: Address,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub logo_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    pub legal_name: Option<String>,
    pub tax_id: String,
    pub address: Option<Address>,
    pub phone: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    pub street: String,
    pub city: String,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceItem {
    pub quantity: f64,
    pub description: String,
    pub unit_price: f64,
    pub unit: Option<String>,
    pub tax_rate: Option<f64>,
    pub tax_amount: Option<f64>,
    pub discount: Option<f64>,
    pub subtotal: f64,
    pub total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceTotals {
    pub subtotal: f64,
    pub tax_amount: f64,
    pub discount_amount: Option<f64>,
    pub total: f64,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiscalInfo {
    pub e_ncf: String,
    pub security_code: String,
    pub signature_date: String,
    pub qr_data: String,
    pub expiration_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentInfo {
    pub method: String,
    pub terms: Option<String>,
    pub bank_info: Option<BankInfo>,
    pub paid: bool,
    pub paid_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BankInfo {
    pub bank_name: String,
    pub account_number: String,
    pub routing_number: Option<String>,
    pub swift_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportData {
    pub title: String,
    pub generated_date: String,
    pub period: ReportPeriod,
    pub data: Vec<HashMap<String, String>>,
    pub summary: Option<ReportSummary>,
    pub charts: Option<Vec<ChartData>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportPeriod {
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportSummary {
    pub metrics: HashMap<String, f64>,
    pub highlights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartData {
    pub chart_type: String,
    pub data_points: Vec<DataPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataPoint {
    pub label: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptData {
    pub receipt_number: String,
    pub date: String,
    pub vendor: CompanyInfo,
    pub items: Vec<ReceiptItem>,
    pub total: f64,
    pub payment_method: String,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptItem {
    pub description: String,
    pub quantity: f64,
    pub unit_price: f64,
    pub total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TemplateData {
    Invoice(InvoiceData),
    Report(ReportData),
    Receipt(ReceiptData),
    Custom(HashMap<String, serde_json::Value>),
}