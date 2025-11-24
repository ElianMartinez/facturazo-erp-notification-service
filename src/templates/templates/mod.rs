// Exportar todos los templates disponibles

mod fiscal_invoice;
mod simple_invoice;
mod receipt;
mod report;
mod quotation;

pub use fiscal_invoice::FiscalInvoiceTemplate;
pub use simple_invoice::SimpleInvoiceTemplate;
pub use receipt::ReceiptTemplate;
pub use report::ReportTemplate;
pub use quotation::{QuotationTemplate, QuotationData, QuotationItem, Customer, Company, MonthlyRent};