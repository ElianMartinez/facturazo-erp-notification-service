use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Invoice data model - compatible with both camelCase and snake_case input
/// This is the unified model used by all invoice templates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceData {
    /// Invoice number - accepts: invoice_number, invoiceNumber, number
    #[serde(alias = "invoice_number", alias = "invoiceNumber", alias = "number")]
    pub invoice_number: String,

    /// Issue date - accepts: issue_date, issueDate, date
    #[serde(alias = "issue_date", alias = "issueDate", alias = "date")]
    pub issue_date: String,

    /// Due date - accepts: due_date, dueDate
    #[serde(alias = "due_date", alias = "dueDate")]
    pub due_date: String,

    /// Company info - accepts: company_info, companyInfo, company
    #[serde(alias = "company_info", alias = "companyInfo", alias = "company")]
    pub company_info: CompanyInfo,

    /// Client info - accepts: client_info, clientInfo, client, customer
    #[serde(
        alias = "client_info",
        alias = "clientInfo",
        alias = "client",
        alias = "customer"
    )]
    pub client_info: ClientInfo,

    /// Invoice items
    pub items: Vec<InvoiceItem>,

    /// Totals - calculated if not provided
    #[serde(default)]
    pub totals: Option<InvoiceTotals>,

    /// Fiscal info for electronic invoices (NCF, etc.)
    #[serde(alias = "fiscal_info", alias = "fiscalInfo")]
    pub fiscal_info: Option<FiscalInfo>,

    /// Payment information
    #[serde(alias = "payment_info", alias = "paymentInfo", alias = "payment")]
    pub payment_info: Option<PaymentInfo>,

    /// Additional notes
    pub notes: Option<String>,

    /// Currency code (DOP, USD, etc.)
    #[serde(default = "default_currency")]
    pub currency: String,

    /// Custom fields for extensibility
    #[serde(alias = "custom_fields", alias = "customFields")]
    pub custom_fields: Option<HashMap<String, serde_json::Value>>,
}

fn default_currency() -> String {
    "DOP".to_string()
}

impl InvoiceData {
    /// Calculate totals from items if not provided
    pub fn calculate_totals(&self) -> InvoiceTotals {
        let mut subtotal = 0.0;
        let mut tax_amount = 0.0;
        let mut discount_amount = 0.0;

        for item in &self.items {
            let item_subtotal = item.quantity * item.unit_price;

            // Calculate item discount
            let item_discount = item.discount.unwrap_or(0.0);
            let discounted_subtotal = item_subtotal - item_discount;

            // Calculate item tax
            let item_tax = if let Some(rate) = item.tax_rate {
                discounted_subtotal * (rate / 100.0)
            } else {
                item.tax_amount.unwrap_or(0.0)
            };

            subtotal += item_subtotal;
            discount_amount += item_discount;
            tax_amount += item_tax;
        }

        let total = subtotal - discount_amount + tax_amount;

        InvoiceTotals {
            subtotal,
            tax_amount,
            discount_amount: Some(discount_amount),
            total,
            currency: self.currency.clone(),
            tip: None,
        }
    }

    /// Get totals - returns calculated if not set
    pub fn get_totals(&self) -> InvoiceTotals {
        self.totals
            .clone()
            .unwrap_or_else(|| self.calculate_totals())
    }
}

/// Company information - the invoice issuer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyInfo {
    pub name: String,

    #[serde(alias = "legal_name", alias = "legalName")]
    pub legal_name: Option<String>,

    /// Tax ID (RNC in DR) - accepts: tax_id, taxId, rnc
    #[serde(alias = "tax_id", alias = "taxId", alias = "rnc")]
    pub tax_id: String,

    /// Address - can be object or string
    #[serde(deserialize_with = "deserialize_address")]
    pub address: Address,

    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,

    #[serde(
        alias = "logo_path",
        alias = "logoPath",
        alias = "logo_url",
        alias = "logoUrl"
    )]
    pub logo_path: Option<String>,
}

/// Client/Customer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,

    #[serde(alias = "legal_name", alias = "legalName")]
    pub legal_name: Option<String>,

    /// Tax ID - accepts: tax_id, taxId, rnc, document_number, documentNumber
    #[serde(
        alias = "tax_id",
        alias = "taxId",
        alias = "rnc",
        alias = "document_number",
        alias = "documentNumber"
    )]
    pub tax_id: Option<String>,

    /// Address - optional, can be object or string
    #[serde(default, deserialize_with = "deserialize_optional_address")]
    pub address: Option<Address>,

    pub phone: Option<String>,
    pub email: Option<String>,
}

/// Address structure - flexible to accept multiple formats
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Address {
    /// Street - accepts: street, line1
    #[serde(alias = "line1", default)]
    pub street: String,

    /// Additional line - accepts: line2
    #[serde(alias = "line2")]
    pub line2: Option<String>,

    #[serde(default)]
    pub city: String,

    pub state: Option<String>,

    /// Postal code - accepts: postal_code, postalCode, zip
    #[serde(alias = "postal_code", alias = "postalCode", alias = "zip")]
    pub postal_code: Option<String>,

    #[serde(default = "default_country")]
    pub country: String,
}

fn default_country() -> String {
    "DO".to_string()
}

impl Address {
    /// Format address as a single string
    pub fn to_string(&self) -> String {
        let mut parts = vec![self.street.clone()];
        if let Some(line2) = &self.line2 {
            if !line2.is_empty() {
                parts.push(line2.clone());
            }
        }
        parts.push(self.city.clone());
        if let Some(state) = &self.state {
            if !state.is_empty() {
                parts.push(state.clone());
            }
        }
        if let Some(zip) = &self.postal_code {
            if !zip.is_empty() {
                parts.push(zip.clone());
            }
        }
        parts.push(self.country.clone());
        parts.join(", ")
    }
}

/// Custom deserializer for Address that handles both string and object formats
fn deserialize_address<'de, D>(deserializer: D) -> Result<Address, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    let value = serde_json::Value::deserialize(deserializer)?;

    match value {
        serde_json::Value::String(s) => {
            // Parse string address (simple format: "street, city, country")
            let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
            Ok(Address {
                street: parts.get(0).unwrap_or(&"").to_string(),
                line2: None,
                city: parts.get(1).unwrap_or(&"").to_string(),
                state: None,
                postal_code: None,
                country: parts.get(2).unwrap_or(&"DO").to_string(),
            })
        }
        serde_json::Value::Object(_) => {
            serde_json::from_value(value).map_err(|e| D::Error::custom(e.to_string()))
        }
        _ => Err(D::Error::custom("Address must be an object or string")),
    }
}

/// Custom deserializer for optional Address
fn deserialize_optional_address<'de, D>(deserializer: D) -> Result<Option<Address>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    let value = Option::<serde_json::Value>::deserialize(deserializer)?;

    match value {
        None => Ok(None),
        Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) if s.is_empty() => Ok(None),
        Some(serde_json::Value::String(s)) => {
            let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
            Ok(Some(Address {
                street: parts.get(0).unwrap_or(&"").to_string(),
                line2: None,
                city: parts.get(1).unwrap_or(&"").to_string(),
                state: None,
                postal_code: None,
                country: parts.get(2).unwrap_or(&"DO").to_string(),
            }))
        }
        Some(serde_json::Value::Object(_)) => {
            let addr: Address = serde_json::from_value(value.unwrap())
                .map_err(|e| D::Error::custom(e.to_string()))?;
            Ok(Some(addr))
        }
        _ => Ok(None),
    }
}

/// Invoice line item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceItem {
    /// Product/service code
    pub code: Option<String>,

    /// Item description
    pub description: String,

    /// Quantity
    pub quantity: f64,

    /// Unit of measure
    pub unit: Option<String>,

    /// Unit price - accepts: unit_price, unitPrice
    #[serde(alias = "unit_price", alias = "unitPrice")]
    pub unit_price: f64,

    /// Tax rate percentage - accepts: tax_rate, taxRate
    #[serde(alias = "tax_rate", alias = "taxRate")]
    pub tax_rate: Option<f64>,

    /// Tax amount - accepts: tax_amount, taxAmount
    #[serde(alias = "tax_amount", alias = "taxAmount")]
    pub tax_amount: Option<f64>,

    /// Discount amount - accepts: discount, discount_amount, discountAmount
    #[serde(alias = "discount_amount", alias = "discountAmount")]
    pub discount: Option<f64>,

    /// Subtotal (quantity * unit_price) - calculated if not provided
    #[serde(default)]
    pub subtotal: Option<f64>,

    /// Line total - calculated if not provided
    #[serde(default)]
    pub total: Option<f64>,

    /// Document date for the line item (used for conduce/consumption invoices)
    /// accepts: document_date, documentDate
    #[serde(alias = "document_date", alias = "documentDate", default)]
    pub document_date: Option<String>,

    /// Notes for the line item (consumer info, plate number, etc.)
    #[serde(default)]
    pub notes: Option<String>,
}

impl InvoiceItem {
    /// Get the subtotal for this item
    pub fn get_subtotal(&self) -> f64 {
        self.subtotal
            .unwrap_or_else(|| self.quantity * self.unit_price)
    }

    /// Get the total for this item (including tax, minus discount)
    pub fn get_total(&self) -> f64 {
        if let Some(total) = self.total {
            return total;
        }

        let subtotal = self.get_subtotal();
        let discount = self.discount.unwrap_or(0.0);
        let discounted = subtotal - discount;

        let tax = if let Some(rate) = self.tax_rate {
            discounted * (rate / 100.0)
        } else {
            self.tax_amount.unwrap_or(0.0)
        };

        discounted + tax
    }
}

/// Invoice totals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceTotals {
    pub subtotal: f64,

    /// Tax amount - accepts: tax_amount, taxAmount, itbis, tax_total, taxTotal
    #[serde(
        alias = "tax_amount",
        alias = "taxAmount",
        alias = "itbis",
        alias = "tax_total",
        alias = "taxTotal"
    )]
    pub tax_amount: f64,

    /// Discount - accepts: discount_amount, discountAmount, discount_total, discountTotal
    #[serde(
        alias = "discount_amount",
        alias = "discountAmount",
        alias = "discount_total",
        alias = "discountTotal"
    )]
    pub discount_amount: Option<f64>,

    /// Grand total - accepts: total, grand_total, grandTotal
    #[serde(alias = "grand_total", alias = "grandTotal")]
    pub total: f64,

    #[serde(default = "default_currency")]
    pub currency: String,

    /// Tip amount (for restaurant invoices)
    #[serde(alias = "tip_amount", alias = "tipAmount")]
    pub tip: Option<f64>,
}

/// Fiscal information for electronic invoices (DGII)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiscalInfo {
    /// Electronic NCF - accepts: e_ncf, eNcf, ncf, ncf_number, ncfNumber
    #[serde(
        alias = "e_ncf",
        alias = "eNcf",
        alias = "ncf",
        alias = "ncf_number",
        alias = "ncfNumber"
    )]
    pub e_ncf: String,

    /// Security code - accepts: security_code, securityCode
    #[serde(alias = "security_code", alias = "securityCode")]
    pub security_code: String,

    /// Signature date - accepts: signature_date, signatureDate
    #[serde(alias = "signature_date", alias = "signatureDate")]
    pub signature_date: String,

    /// QR code data - accepts: qr_data, qrData, qr_url, qrUrl
    #[serde(
        alias = "qr_data",
        alias = "qrData",
        alias = "qr_url",
        alias = "qrUrl",
        default
    )]
    pub qr_data: String,

    /// Expiration date - accepts: expiration_date, expirationDate
    #[serde(alias = "expiration_date", alias = "expirationDate")]
    pub expiration_date: Option<String>,
}

/// Payment information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentInfo {
    /// Payment method - accepts: method, payment_method, paymentMethod
    #[serde(alias = "payment_method", alias = "paymentMethod")]
    pub method: String,

    /// Payment terms - accepts: terms, payment_terms, paymentTerms
    #[serde(alias = "payment_terms", alias = "paymentTerms")]
    pub terms: Option<String>,

    /// Bank information
    #[serde(alias = "bank_info", alias = "bankInfo")]
    pub bank_info: Option<BankInfo>,

    /// Whether the invoice is paid
    #[serde(default)]
    pub paid: bool,

    /// Payment date - accepts: paid_date, paidDate
    #[serde(alias = "paid_date", alias = "paidDate")]
    pub paid_date: Option<String>,

    /// Due date (alternative location) - accepts: due_date, dueDate
    #[serde(alias = "due_date", alias = "dueDate")]
    pub due_date: Option<String>,
}

/// Bank information for wire transfers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankInfo {
    #[serde(alias = "bank_name", alias = "bankName")]
    pub bank_name: String,

    #[serde(alias = "account_number", alias = "accountNumber")]
    pub account_number: String,

    #[serde(alias = "routing_number", alias = "routingNumber")]
    pub routing_number: Option<String>,

    #[serde(alias = "swift_code", alias = "swiftCode")]
    pub swift_code: Option<String>,
}

// ============================================
// Report Models
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportData {
    pub title: String,

    #[serde(alias = "generated_date", alias = "generatedDate")]
    pub generated_date: String,

    pub period: ReportPeriod,

    pub data: Vec<HashMap<String, serde_json::Value>>,

    pub summary: Option<ReportSummary>,

    pub charts: Option<Vec<ChartData>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportPeriod {
    #[serde(alias = "start_date", alias = "startDate", alias = "from")]
    pub start_date: String,

    #[serde(alias = "end_date", alias = "endDate", alias = "to")]
    pub end_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub metrics: HashMap<String, f64>,
    pub highlights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    #[serde(alias = "chart_type", alias = "chartType")]
    pub chart_type: String,

    #[serde(alias = "data_points", alias = "dataPoints")]
    pub data_points: Vec<DataPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub label: String,
    pub value: f64,
}

// ============================================
// Receipt Models
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptData {
    #[serde(alias = "receipt_number", alias = "receiptNumber")]
    pub receipt_number: String,

    pub date: String,

    pub vendor: CompanyInfo,

    pub items: Vec<ReceiptItem>,

    pub total: f64,

    #[serde(alias = "payment_method", alias = "paymentMethod")]
    pub payment_method: String,

    #[serde(default = "default_currency")]
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptItem {
    pub description: String,
    pub quantity: f64,

    #[serde(alias = "unit_price", alias = "unitPrice")]
    pub unit_price: f64,

    pub total: f64,
}

// ============================================
// Cash Register / Cuadre de Caja
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashRegisterData {
    #[serde(alias = "document_code", alias = "documentCode")]
    pub document_code: String,

    pub date: String,

    #[serde(alias = "company_info", alias = "companyInfo", alias = "company")]
    pub company_info: CompanyInfo,

    pub cashier: CashierInfo,

    #[serde(alias = "transactions_count", alias = "transactionsCount")]
    pub transactions_count: u32,

    pub status: CashRegisterStatus,

    pub income: Vec<CashRegisterItem>,

    pub expenses: Vec<CashRegisterItem>,

    pub summary: CashRegisterSummary,

    pub denominations: Option<Vec<Denomination>>,

    #[serde(alias = "payment_methods", alias = "paymentMethods")]
    pub payment_methods: Vec<PaymentMethodBalance>,

    #[serde(alias = "final_balance", alias = "finalBalance")]
    pub final_balance: FinalBalance,

    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashierInfo {
    pub name: String,
    pub id: Option<String>,
    pub shift: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CashRegisterStatus {
    Balanced,
    Surplus,
    Shortage,
}

impl CashRegisterStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CashRegisterStatus::Balanced => "Cuadrado",
            CashRegisterStatus::Surplus => "Sobrante",
            CashRegisterStatus::Shortage => "Faltante",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            CashRegisterStatus::Balanced => "#64748B",
            CashRegisterStatus::Surplus => "#059669",
            CashRegisterStatus::Shortage => "#DC2626",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashRegisterItem {
    pub concept: String,
    pub quantity: Option<u32>,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashRegisterSummary {
    #[serde(alias = "total_income", alias = "totalIncome")]
    pub total_income: f64,

    #[serde(alias = "total_expenses", alias = "totalExpenses")]
    pub total_expenses: f64,

    #[serde(alias = "total_to_balance", alias = "totalToBalance")]
    pub total_to_balance: f64,

    #[serde(alias = "total_in_register", alias = "totalInRegister")]
    pub total_in_register: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Denomination {
    pub value: f64,
    pub quantity: u32,
    pub total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethodBalance {
    pub method: String,

    #[serde(alias = "system_amount", alias = "systemAmount")]
    pub system_amount: f64,

    #[serde(alias = "actual_amount", alias = "actualAmount")]
    pub actual_amount: f64,

    pub difference: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalBalance {
    #[serde(alias = "system_total", alias = "systemTotal")]
    pub system_total: f64,

    #[serde(alias = "actual_total", alias = "actualTotal")]
    pub actual_total: f64,

    pub difference: f64,
}

// ============================================
// Template Data Enum - for type-safe template routing
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TemplateData {
    Invoice(InvoiceData),
    Report(ReportData),
    Receipt(ReceiptData),
    CashRegister(CashRegisterData),
    Custom(HashMap<String, serde_json::Value>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_snake_case_invoice() {
        let json = r#"{
            "invoice_number": "INV-001",
            "issue_date": "2024-12-11",
            "due_date": "2024-12-25",
            "company_info": {
                "name": "Test Company",
                "tax_id": "123456789",
                "address": {
                    "street": "123 Main St",
                    "city": "Santo Domingo",
                    "country": "DO"
                }
            },
            "client_info": {
                "name": "Client Name",
                "tax_id": "987654321"
            },
            "items": [
                {
                    "description": "Service A",
                    "quantity": 2.0,
                    "unit_price": 100.0
                }
            ],
            "currency": "DOP"
        }"#;

        let invoice: InvoiceData = serde_json::from_str(json).unwrap();
        assert_eq!(invoice.invoice_number, "INV-001");
        assert_eq!(invoice.company_info.tax_id, "123456789");
    }

    #[test]
    fn test_deserialize_camel_case_invoice() {
        let json = r#"{
            "invoiceNumber": "INV-002",
            "issueDate": "2024-12-11",
            "dueDate": "2024-12-25",
            "companyInfo": {
                "name": "Test Company",
                "taxId": "123456789",
                "address": {
                    "street": "123 Main St",
                    "city": "Santo Domingo",
                    "country": "DO"
                }
            },
            "clientInfo": {
                "name": "Client Name",
                "taxId": "987654321"
            },
            "items": [
                {
                    "description": "Service A",
                    "quantity": 2.0,
                    "unitPrice": 100.0
                }
            ],
            "currency": "DOP"
        }"#;

        let invoice: InvoiceData = serde_json::from_str(json).unwrap();
        assert_eq!(invoice.invoice_number, "INV-002");
        assert_eq!(invoice.company_info.tax_id, "123456789");
    }

    #[test]
    fn test_deserialize_string_address() {
        let json = r#"{
            "invoice_number": "INV-003",
            "issue_date": "2024-12-11",
            "due_date": "2024-12-25",
            "company_info": {
                "name": "Test Company",
                "tax_id": "123456789",
                "address": "123 Main St, Santo Domingo, DO"
            },
            "client_info": {
                "name": "Client Name"
            },
            "items": [],
            "currency": "DOP"
        }"#;

        let invoice: InvoiceData = serde_json::from_str(json).unwrap();
        assert_eq!(invoice.company_info.address.street, "123 Main St");
        assert_eq!(invoice.company_info.address.city, "Santo Domingo");
    }

    #[test]
    fn test_calculate_totals() {
        let invoice = InvoiceData {
            invoice_number: "INV-001".to_string(),
            issue_date: "2024-12-11".to_string(),
            due_date: "2024-12-25".to_string(),
            company_info: CompanyInfo {
                name: "Test".to_string(),
                legal_name: None,
                tax_id: "123".to_string(),
                address: Address::default(),
                phone: None,
                email: None,
                website: None,
                logo_path: None,
            },
            client_info: ClientInfo {
                name: "Client".to_string(),
                legal_name: None,
                tax_id: None,
                address: None,
                phone: None,
                email: None,
            },
            items: vec![InvoiceItem {
                code: None,
                description: "Item 1".to_string(),
                quantity: 2.0,
                unit: None,
                unit_price: 100.0,
                tax_rate: Some(18.0),
                tax_amount: None,
                discount: None,
                subtotal: None,
                total: None,
                document_date: None,
                notes: None,
            }],
            totals: None,
            fiscal_info: None,
            payment_info: None,
            notes: None,
            currency: "DOP".to_string(),
            custom_fields: None,
        };

        let totals = invoice.calculate_totals();
        assert_eq!(totals.subtotal, 200.0);
        assert_eq!(totals.tax_amount, 36.0); // 200 * 18%
        assert_eq!(totals.total, 236.0);
    }
}
