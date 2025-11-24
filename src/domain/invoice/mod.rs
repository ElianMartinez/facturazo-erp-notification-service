//! Invoice domain module with Dominican Republic fiscal requirements
//!
//! This module contains the complete invoice entity with DGII compliance

use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::fmt;

use crate::domain::fiscal::{NCF, RNC, Cedula, ITBIS, FiscalError};
use crate::domain::shared::{TenantId, UserId, DomainError, DomainResult};

/// Invoice entity - Factura fiscal electrónica
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    // Identity
    pub id: InvoiceId,
    pub tenant_id: TenantId,
    pub invoice_number: String,

    // Fiscal information (DGII)
    pub ncf: NCF,
    pub ncf_expiry_date: DateTime<Utc>,

    // Seller information
    pub seller: Seller,

    // Customer information
    pub customer: Customer,

    // Invoice details
    pub items: Vec<InvoiceItem>,
    pub currency: Currency,
    pub exchange_rate: Option<f64>,

    // Amounts
    pub subtotal: f64,
    pub discount_amount: f64,
    pub itbis_amount: f64,
    pub total_amount: f64,

    // Payment information
    pub payment_terms: PaymentTerms,
    pub payment_method: Option<PaymentMethod>,

    // Dates
    pub issue_date: DateTime<Utc>,
    pub due_date: Option<DateTime<Utc>>,
    pub paid_date: Option<DateTime<Utc>>,

    // Status
    pub status: InvoiceStatus,

    // Metadata
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<UserId>,
}

/// Invoice ID value object
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct InvoiceId(String);

impl InvoiceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn from_string(id: String) -> Self {
        Self(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for InvoiceId {
    fn default() -> Self {
        Self::new()
    }
}

/// Seller information - Vendedor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Seller {
    pub company_name: String,
    pub trade_name: Option<String>,
    pub rnc: RNC,
    pub address: Address,
    pub phone: String,
    pub email: String,
    pub website: Option<String>,
}

/// Customer information - Cliente
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    pub name: String,
    pub tax_id: Option<TaxId>,
    pub address: Option<Address>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub contact_person: Option<String>,
}

/// Tax ID - Can be RNC or Cédula
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaxId {
    RNC(RNC),
    Cedula(Cedula),
}

impl TaxId {
    pub fn as_string(&self) -> String {
        match self {
            Self::RNC(rnc) => rnc.formatted(),
            Self::Cedula(cedula) => cedula.formatted(),
        }
    }
}

/// Address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub street: String,
    pub number: Option<String>,
    pub neighborhood: Option<String>,
    pub city: String,
    pub province: String,
    pub postal_code: Option<String>,
    pub country: String,
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts = vec![
            Some(self.street.clone()),
            self.number.clone(),
            self.neighborhood.clone(),
            Some(self.city.clone()),
            Some(self.province.clone()),
            Some(self.country.clone()),
        ];

        let address = parts
            .into_iter()
            .filter_map(|p| p)
            .collect::<Vec<_>>()
            .join(", ");

        write!(f, "{}", address)
    }
}

/// Invoice line item - Línea de factura
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceItem {
    pub id: String,
    pub code: Option<String>,
    pub description: String,
    pub quantity: f64,
    pub unit: String,
    pub unit_price: f64,
    pub discount_rate: f64,
    pub discount_amount: f64,
    pub subtotal: f64,
    pub itbis_rate: f64,
    pub itbis_amount: f64,
    pub total: f64,
}

impl InvoiceItem {
    /// Create new invoice item
    pub fn new(
        description: String,
        quantity: f64,
        unit: String,
        unit_price: f64,
        itbis_rate: f64,
    ) -> Self {
        let subtotal = quantity * unit_price;
        let itbis_amount = subtotal * itbis_rate;
        let total = subtotal + itbis_amount;

        Self {
            id: Uuid::new_v4().to_string(),
            code: None,
            description,
            quantity,
            unit,
            unit_price,
            discount_rate: 0.0,
            discount_amount: 0.0,
            subtotal,
            itbis_rate,
            itbis_amount,
            total,
        }
    }

    /// Apply discount to item
    pub fn apply_discount(&mut self, discount_rate: f64) -> DomainResult<()> {
        if discount_rate < 0.0 || discount_rate > 1.0 {
            return Err(DomainError::ValidationError(
                "Discount rate must be between 0 and 1".to_string()
            ));
        }

        self.discount_rate = discount_rate;
        self.discount_amount = self.subtotal * discount_rate;

        let discounted_subtotal = self.subtotal - self.discount_amount;
        self.itbis_amount = discounted_subtotal * self.itbis_rate;
        self.total = discounted_subtotal + self.itbis_amount;

        Ok(())
    }
}

/// Currency
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Currency {
    DOP, // Dominican Peso
    USD, // US Dollar
    EUR, // Euro
}

impl Currency {
    pub fn symbol(&self) -> &str {
        match self {
            Self::DOP => "RD$",
            Self::USD => "US$",
            Self::EUR => "€",
        }
    }

    pub fn code(&self) -> &str {
        match self {
            Self::DOP => "DOP",
            Self::USD => "USD",
            Self::EUR => "EUR",
        }
    }
}

/// Payment terms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentTerms {
    Immediate,           // Contado
    Net15,              // 15 días
    Net30,              // 30 días
    Net45,              // 45 días
    Net60,              // 60 días
    Net90,              // 90 días
    Custom(u32),        // Custom days
}

impl PaymentTerms {
    pub fn days(&self) -> u32 {
        match self {
            Self::Immediate => 0,
            Self::Net15 => 15,
            Self::Net30 => 30,
            Self::Net45 => 45,
            Self::Net60 => 60,
            Self::Net90 => 90,
            Self::Custom(days) => *days,
        }
    }

    pub fn description(&self) -> String {
        match self {
            Self::Immediate => "Contado".to_string(),
            Self::Net15 => "15 días".to_string(),
            Self::Net30 => "30 días".to_string(),
            Self::Net45 => "45 días".to_string(),
            Self::Net60 => "60 días".to_string(),
            Self::Net90 => "90 días".to_string(),
            Self::Custom(days) => format!("{} días", days),
        }
    }
}

/// Payment method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentMethod {
    Cash,                    // Efectivo
    CreditCard,             // Tarjeta de crédito
    DebitCard,              // Tarjeta de débito
    BankTransfer,           // Transferencia bancaria
    Check,                  // Cheque
    Credit,                 // Crédito
    Other(String),
}

impl PaymentMethod {
    pub fn description(&self) -> &str {
        match self {
            Self::Cash => "Efectivo",
            Self::CreditCard => "Tarjeta de Crédito",
            Self::DebitCard => "Tarjeta de Débito",
            Self::BankTransfer => "Transferencia Bancaria",
            Self::Check => "Cheque",
            Self::Credit => "Crédito",
            Self::Other(desc) => desc,
        }
    }
}

/// Invoice status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InvoiceStatus {
    Draft,          // Borrador
    Issued,         // Emitida
    Sent,           // Enviada
    Paid,           // Pagada
    PartiallyPaid,  // Parcialmente pagada
    Overdue,        // Vencida
    Cancelled,      // Cancelada
    Refunded,       // Reembolsada
}

impl Invoice {
    /// Create a new invoice
    pub fn new(
        tenant_id: TenantId,
        ncf: NCF,
        ncf_expiry_date: DateTime<Utc>,
        seller: Seller,
        customer: Customer,
        items: Vec<InvoiceItem>,
        currency: Currency,
        payment_terms: PaymentTerms,
    ) -> DomainResult<Self> {
        if items.is_empty() {
            return Err(DomainError::ValidationError(
                "Invoice must have at least one item".to_string()
            ));
        }

        let subtotal: f64 = items.iter().map(|i| i.subtotal).sum();
        let itbis_amount: f64 = items.iter().map(|i| i.itbis_amount).sum();
        let discount_amount: f64 = items.iter().map(|i| i.discount_amount).sum();
        let total_amount = subtotal - discount_amount + itbis_amount;

        let issue_date = Utc::now();
        let due_date = if payment_terms.days() > 0 {
            Some(issue_date + Duration::days(payment_terms.days() as i64))
        } else {
            None
        };

        Ok(Self {
            id: InvoiceId::new(),
            tenant_id,
            invoice_number: String::new(), // Will be set by repository
            ncf,
            ncf_expiry_date,
            seller,
            customer,
            items,
            currency,
            exchange_rate: None,
            subtotal,
            discount_amount,
            itbis_amount,
            total_amount,
            payment_terms,
            payment_method: None,
            issue_date,
            due_date,
            paid_date: None,
            status: InvoiceStatus::Draft,
            notes: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: None,
        })
    }

    /// Issue the invoice (change from draft to issued)
    pub fn issue(&mut self) -> DomainResult<()> {
        if self.status != InvoiceStatus::Draft {
            return Err(DomainError::InvalidOperation(
                "Only draft invoices can be issued".to_string()
            ));
        }

        self.status = InvoiceStatus::Issued;
        self.issue_date = Utc::now();
        self.updated_at = Utc::now();

        Ok(())
    }

    /// Mark invoice as sent
    pub fn mark_sent(&mut self) -> DomainResult<()> {
        if self.status != InvoiceStatus::Issued {
            return Err(DomainError::InvalidOperation(
                "Only issued invoices can be marked as sent".to_string()
            ));
        }

        self.status = InvoiceStatus::Sent;
        self.updated_at = Utc::now();

        Ok(())
    }

    /// Mark invoice as paid
    pub fn mark_paid(&mut self, payment_method: PaymentMethod) -> DomainResult<()> {
        if self.status == InvoiceStatus::Cancelled {
            return Err(DomainError::InvalidOperation(
                "Cannot mark cancelled invoice as paid".to_string()
            ));
        }

        self.status = InvoiceStatus::Paid;
        self.payment_method = Some(payment_method);
        self.paid_date = Some(Utc::now());
        self.updated_at = Utc::now();

        Ok(())
    }

    /// Cancel the invoice
    pub fn cancel(&mut self) -> DomainResult<()> {
        if self.status == InvoiceStatus::Paid {
            return Err(DomainError::InvalidOperation(
                "Cannot cancel paid invoice".to_string()
            ));
        }

        self.status = InvoiceStatus::Cancelled;
        self.updated_at = Utc::now();

        Ok(())
    }

    /// Check if invoice is overdue
    pub fn check_overdue(&mut self) -> bool {
        if let Some(due_date) = self.due_date {
            if self.status == InvoiceStatus::Sent && Utc::now() > due_date {
                self.status = InvoiceStatus::Overdue;
                self.updated_at = Utc::now();
                return true;
            }
        }
        false
    }

    /// Apply global discount to invoice
    pub fn apply_global_discount(&mut self, discount_rate: f64) -> DomainResult<()> {
        if discount_rate < 0.0 || discount_rate > 1.0 {
            return Err(DomainError::ValidationError(
                "Discount rate must be between 0 and 1".to_string()
            ));
        }

        for item in &mut self.items {
            item.apply_discount(discount_rate)?;
        }

        self.recalculate_totals();
        Ok(())
    }

    /// Recalculate invoice totals
    pub fn recalculate_totals(&mut self) {
        self.subtotal = self.items.iter().map(|i| i.subtotal).sum();
        self.discount_amount = self.items.iter().map(|i| i.discount_amount).sum();
        self.itbis_amount = self.items.iter().map(|i| i.itbis_amount).sum();
        self.total_amount = self.subtotal - self.discount_amount + self.itbis_amount;
        self.updated_at = Utc::now();
    }

    /// Validate invoice for DGII compliance
    pub fn validate_fiscal_compliance(&self) -> DomainResult<()> {
        // Check NCF validity
        if !self.ncf.is_valid_for_date(self.issue_date, self.ncf_expiry_date) {
            return Err(DomainError::ValidationError(
                "NCF is not valid for invoice date".to_string()
            ));
        }

        // Check if customer tax ID is required based on NCF type
        // E31 = Crédito Fiscal, B01 = Consumidor Final
        if self.ncf.serie() == 'E' && self.ncf.tipo_code() == 31 {
            // Crédito Fiscal requires customer tax ID
            if self.customer.tax_id.is_none() {
                return Err(DomainError::ValidationError(
                    "Customer tax ID is required for credit fiscal invoice".to_string()
                ));
            }
        } else if self.ncf.serie() == 'B' && self.ncf.tipo_code() == 1 {
            // Consumidor Final requires tax ID for large amounts
            if self.total_amount > 250_000.0 && self.customer.tax_id.is_none() {
                return Err(DomainError::ValidationError(
                    "Customer tax ID is required for amounts over RD$250,000".to_string()
                ));
            }
        }

        Ok(())
    }

    /// Generate printable invoice number
    pub fn formatted_invoice_number(&self) -> String {
        format!("{}-{}", self.ncf.serie(), self.invoice_number)
    }

    /// Get formatted amounts for display
    pub fn formatted_amounts(&self) -> FormattedAmounts {
        FormattedAmounts {
            subtotal: format!("{} {:.2}", self.currency.symbol(), self.subtotal),
            discount: format!("{} {:.2}", self.currency.symbol(), self.discount_amount),
            itbis: format!("{} {:.2}", self.currency.symbol(), self.itbis_amount),
            total: format!("{} {:.2}", self.currency.symbol(), self.total_amount),
        }
    }
}

/// Formatted amounts for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormattedAmounts {
    pub subtotal: String,
    pub discount: String,
    pub itbis: String,
    pub total: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fiscal::NCF;

    #[test]
    fn test_invoice_creation() {
        let tenant_id = TenantId::new("tenant-123".to_string());
        let ncf = NCF::new_credito_fiscal(1).unwrap();
        let ncf_expiry = Utc::now() + Duration::days(365);

        let seller = Seller {
            company_name: "Test Company".to_string(),
            trade_name: None,
            rnc: RNC::new("101123456".to_string()).unwrap(),
            address: Address {
                street: "Main St".to_string(),
                number: Some("123".to_string()),
                neighborhood: None,
                city: "Santo Domingo".to_string(),
                province: "DN".to_string(),
                postal_code: None,
                country: "República Dominicana".to_string(),
            },
            phone: "809-555-1234".to_string(),
            email: "test@example.com".to_string(),
            website: None,
        };

        let customer = Customer {
            name: "Customer Test".to_string(),
            tax_id: Some(TaxId::RNC(RNC::new("101654321".to_string()).unwrap())),
            address: None,
            phone: None,
            email: None,
            contact_person: None,
        };

        let item = InvoiceItem::new(
            "Test Product".to_string(),
            2.0,
            "UND".to_string(),
            100.0,
            0.18,
        );

        let invoice = Invoice::new(
            tenant_id,
            ncf,
            ncf_expiry,
            seller,
            customer,
            vec![item],
            Currency::DOP,
            PaymentTerms::Net30,
        ).unwrap();

        assert_eq!(invoice.subtotal, 200.0);
        assert_eq!(invoice.itbis_amount, 36.0);
        assert_eq!(invoice.total_amount, 236.0);
        assert_eq!(invoice.status, InvoiceStatus::Draft);
    }

    #[test]
    fn test_invoice_item_discount() {
        let mut item = InvoiceItem::new(
            "Test Product".to_string(),
            10.0,
            "UND".to_string(),
            100.0,
            0.18,
        );

        assert_eq!(item.subtotal, 1000.0);
        assert_eq!(item.itbis_amount, 180.0);
        assert_eq!(item.total, 1180.0);

        item.apply_discount(0.10).unwrap(); // 10% discount

        assert_eq!(item.discount_amount, 100.0);
        assert_eq!(item.itbis_amount, 162.0); // 18% of 900
        assert_eq!(item.total, 1062.0); // 900 + 162
    }

    #[test]
    fn test_invoice_status_transitions() {
        let mut invoice = create_test_invoice();

        // Draft -> Issued
        assert!(invoice.issue().is_ok());
        assert_eq!(invoice.status, InvoiceStatus::Issued);

        // Issued -> Sent
        assert!(invoice.mark_sent().is_ok());
        assert_eq!(invoice.status, InvoiceStatus::Sent);

        // Sent -> Paid
        assert!(invoice.mark_paid(PaymentMethod::Cash).is_ok());
        assert_eq!(invoice.status, InvoiceStatus::Paid);
        assert!(invoice.paid_date.is_some());

        // Cannot cancel paid invoice
        assert!(invoice.cancel().is_err());
    }

    fn create_test_invoice() -> Invoice {
        let tenant_id = TenantId::new("test".to_string());
        let ncf = NCF::new('E', 31000000001).unwrap();
        let ncf_expiry = Utc::now() + Duration::days(365);

        Invoice::new(
            tenant_id,
            ncf,
            ncf_expiry,
            create_test_seller(),
            create_test_customer(),
            vec![create_test_item()],
            Currency::DOP,
            PaymentTerms::Immediate,
        ).unwrap()
    }

    fn create_test_seller() -> Seller {
        Seller {
            company_name: "Test".to_string(),
            trade_name: None,
            rnc: RNC::new("101123456".to_string()).unwrap(),
            address: create_test_address(),
            phone: "809-555-1234".to_string(),
            email: "test@test.com".to_string(),
            website: None,
        }
    }

    fn create_test_customer() -> Customer {
        Customer {
            name: "Customer".to_string(),
            tax_id: None,
            address: None,
            phone: None,
            email: None,
            contact_person: None,
        }
    }

    fn create_test_address() -> Address {
        Address {
            street: "Street".to_string(),
            number: None,
            neighborhood: None,
            city: "Santo Domingo".to_string(),
            province: "DN".to_string(),
            postal_code: None,
            country: "RD".to_string(),
        }
    }

    fn create_test_item() -> InvoiceItem {
        InvoiceItem::new(
            "Product".to_string(),
            1.0,
            "UND".to_string(),
            100.0,
            0.18,
        )
    }
}