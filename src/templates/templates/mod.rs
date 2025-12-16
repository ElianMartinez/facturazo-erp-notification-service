// Exportar todos los templates disponibles

mod cash_register;
mod erp_report;
mod fiscal_invoice;
mod quotation;
mod receipt;
mod report;
mod simple_invoice;

pub use cash_register::CashRegisterTemplate;
pub use erp_report::ErpReportTemplate;
pub use fiscal_invoice::FiscalInvoiceTemplate;
pub use quotation::{
    Company, Customer, MonthlyRent, QuotationData, QuotationItem, QuotationTemplate,
};
pub use receipt::ReceiptTemplate;
pub use report::ReportTemplate;
pub use simple_invoice::SimpleInvoiceTemplate;
