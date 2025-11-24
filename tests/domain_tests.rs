//! Integration tests for domain layer

use pdf_services::domain::{
    fiscal::{NCF, RNC, Cedula, ITBIS, NCFSequence, FiscalError},
    invoice::{Invoice, InvoiceItem, Currency, PaymentTerms, PaymentMethod, InvoiceStatus, Seller, Customer, Address, TaxId},
    TenantId,
};
use chrono::{Utc, Duration};

#[test]
fn test_ncf_validation_and_formatting() {
    // Valid NCF - Crédito Fiscal
    let ncf = NCF::new_credito_fiscal(1).unwrap();
    assert_eq!(ncf.as_str(), "E3100000001");
    assert_eq!(ncf.serie(), 'E');
    assert_eq!(ncf.tipo_code(), 31);
    assert_eq!(ncf.sequence(), 1);

    // Parse NCF from string - Consumidor Final
    let parsed = NCF::parse("B0100000123").unwrap();
    assert_eq!(parsed.serie(), 'B');
    assert_eq!(parsed.tipo_code(), 1);
    assert_eq!(parsed.sequence(), 123);

    // Invalid NCF
    assert!(NCF::parse("123456").is_err());
    assert!(NCF::parse("Z12345678901").is_err());
    assert!(NCF::parse("E123").is_err());
}

#[test]
fn test_rnc_validation_and_formatting() {
    // Company RNC (9 digits)
    let rnc = RNC::new("101123456".to_string()).unwrap();
    assert_eq!(rnc.as_str(), "101123456");
    assert_eq!(rnc.formatted(), "1-01-12345-6");

    // Personal RNC (11 digits - same as cedula)
    let rnc_personal = RNC::new("00112345678".to_string()).unwrap();
    assert_eq!(rnc_personal.as_str(), "00112345678");
    assert_eq!(rnc_personal.formatted(), "001-1234567-8");

    // Invalid RNC
    assert!(RNC::new("123".to_string()).is_err());
    assert!(RNC::new("12345678901234".to_string()).is_err());

    // RNC with dashes should be cleaned
    let rnc_with_dashes = RNC::new("1-01-12345-6".to_string()).unwrap();
    assert_eq!(rnc_with_dashes.as_str(), "101123456");
}

#[test]
fn test_cedula_validation_and_formatting() {
    // Valid cedula
    let cedula = Cedula::new("00112345678".to_string()).unwrap();
    assert_eq!(cedula.as_str(), "00112345678");
    assert_eq!(cedula.formatted(), "001-1234567-8");

    // Cedula with dashes should be cleaned
    let cedula_with_dashes = Cedula::new("001-1234567-8".to_string()).unwrap();
    assert_eq!(cedula_with_dashes.as_str(), "00112345678");

    // Invalid cedula
    assert!(Cedula::new("123".to_string()).is_err());
    assert!(Cedula::new("0011234567".to_string()).is_err()); // Too short
}

#[test]
fn test_itbis_calculations() {
    // Standard rate (18%)
    let itbis = ITBIS::standard(1000.0);
    assert_eq!(itbis.rate(), 0.18);
    assert_eq!(itbis.amount(), 180.0);
    assert_eq!(itbis.rate_percentage(), 18);

    // Reduced rate (16%)
    let itbis_reduced = ITBIS::reduced(1000.0);
    assert_eq!(itbis_reduced.rate(), 0.16);
    assert_eq!(itbis_reduced.amount(), 160.0);

    // Custom rate
    let itbis_custom = ITBIS::with_rate(1000.0, 0.10).unwrap();
    assert_eq!(itbis_custom.amount(), 100.0);

    // Reverse calculation from total
    let itbis_reverse = ITBIS::from_total(1180.0, 0.18).unwrap();
    assert!((itbis_reverse.amount() - 180.0).abs() < 0.01);

    // Invalid rate
    assert!(ITBIS::with_rate(1000.0, 1.5).is_err());
    assert!(ITBIS::with_rate(1000.0, -0.1).is_err());
}

#[test]
fn test_ncf_sequence_management() {
    let expiry = Utc::now() + Duration::days(365);
    let mut sequence = NCFSequence::new_credito_fiscal(1, 10, expiry);

    // Get first NCF
    let ncf1 = sequence.next().unwrap();
    assert_eq!(ncf1.as_str(), "E3100000001");

    // Get second NCF
    let ncf2 = sequence.next().unwrap();
    assert_eq!(ncf2.as_str(), "E3100000002");

    // Check remaining
    assert_eq!(sequence.remaining(), 8);
    assert!(sequence.days_until_expiry() > 360);

    // Exhaust sequence
    for _ in 0..8 {
        sequence.next().unwrap();
    }

    // Should fail when exhausted
    assert!(matches!(
        sequence.next().unwrap_err(),
        FiscalError::NCFSequenceExhausted
    ));
}

#[test]
fn test_invoice_creation_with_fiscal_data() {
    let tenant_id = TenantId::new("test-tenant".to_string());
    let ncf = NCF::new_credito_fiscal(1).unwrap();
    let ncf_expiry = Utc::now() + Duration::days(365);

    let seller = create_test_seller();
    let customer = create_test_customer_with_rnc();

    let items = vec![
        InvoiceItem::new(
            "Producto A".to_string(),
            2.0,
            "UND".to_string(),
            100.0,
            0.18,
        ),
        InvoiceItem::new(
            "Servicio B".to_string(),
            1.0,
            "SERV".to_string(),
            500.0,
            0.18,
        ),
    ];

    let invoice = Invoice::new(
        tenant_id,
        ncf,
        ncf_expiry,
        seller,
        customer,
        items,
        Currency::DOP,
        PaymentTerms::Net30,
    ).unwrap();

    assert_eq!(invoice.subtotal, 700.0);
    assert_eq!(invoice.itbis_amount, 126.0);
    assert_eq!(invoice.total_amount, 826.0);
    assert_eq!(invoice.status, InvoiceStatus::Draft);
    assert!(invoice.due_date.is_some());
}

#[test]
fn test_invoice_item_with_discount() {
    let mut item = InvoiceItem::new(
        "Producto con Descuento".to_string(),
        10.0,
        "UND".to_string(),
        100.0,
        0.18,
    );

    // Initial values
    assert_eq!(item.subtotal, 1000.0);
    assert_eq!(item.itbis_amount, 180.0);
    assert_eq!(item.total, 1180.0);

    // Apply 15% discount
    item.apply_discount(0.15).unwrap();

    assert_eq!(item.discount_amount, 150.0);
    assert_eq!(item.itbis_amount, 153.0); // 18% of 850
    assert_eq!(item.total, 1003.0); // 850 + 153

    // Invalid discount
    assert!(item.apply_discount(1.5).is_err());
    assert!(item.apply_discount(-0.1).is_err());
}

#[test]
fn test_invoice_status_workflow() {
    let mut invoice = create_test_invoice();

    // Draft -> Issued
    assert_eq!(invoice.status, InvoiceStatus::Draft);
    invoice.issue().unwrap();
    assert_eq!(invoice.status, InvoiceStatus::Issued);

    // Cannot issue again
    assert!(invoice.issue().is_err());

    // Issued -> Sent
    invoice.mark_sent().unwrap();
    assert_eq!(invoice.status, InvoiceStatus::Sent);

    // Sent -> Paid
    invoice.mark_paid(PaymentMethod::BankTransfer).unwrap();
    assert_eq!(invoice.status, InvoiceStatus::Paid);
    assert!(invoice.paid_date.is_some());

    // Check payment method
    match &invoice.payment_method {
        Some(method) => assert_eq!(method.description(), "Transferencia Bancaria"),
        None => panic!("Payment method should be set"),
    }

    // Cannot cancel paid invoice
    assert!(invoice.cancel().is_err());
}

#[test]
fn test_invoice_cancellation() {
    let mut invoice = create_test_invoice();

    // Can cancel draft invoice
    invoice.cancel().unwrap();
    assert_eq!(invoice.status, InvoiceStatus::Cancelled);

    // Create another invoice
    let mut invoice2 = create_test_invoice();
    invoice2.issue().unwrap();
    invoice2.mark_sent().unwrap();

    // Can cancel sent invoice
    invoice2.cancel().unwrap();
    assert_eq!(invoice2.status, InvoiceStatus::Cancelled);
}

#[test]
fn test_invoice_overdue_check() {
    let mut invoice = create_test_invoice_with_payment_terms(PaymentTerms::Immediate);

    invoice.issue().unwrap();
    invoice.mark_sent().unwrap();

    // Set due date in the past
    invoice.due_date = Some(Utc::now() - Duration::days(1));

    // Check overdue
    assert!(invoice.check_overdue());
    assert_eq!(invoice.status, InvoiceStatus::Overdue);
}

#[test]
fn test_invoice_fiscal_compliance() {
    // Test 1: Credit fiscal requires customer tax ID
    let mut invoice = create_test_invoice();
    invoice.customer.tax_id = None;
    assert!(invoice.validate_fiscal_compliance().is_err());

    // Test 2: Consumer final doesn't require tax ID for small amounts
    let ncf_consumer = NCF::new_consumidor_final(1).unwrap();
    let mut invoice_consumer = create_test_invoice();
    invoice_consumer.ncf = ncf_consumer;
    invoice_consumer.customer.tax_id = None;
    invoice_consumer.total_amount = 50_000.0;
    assert!(invoice_consumer.validate_fiscal_compliance().is_ok());

    // Test 3: Consumer final requires tax ID for large amounts
    invoice_consumer.total_amount = 300_000.0;
    assert!(invoice_consumer.validate_fiscal_compliance().is_err());

    // Test 4: NCF expiry validation
    let mut expired_invoice = create_test_invoice();
    expired_invoice.ncf_expiry_date = Utc::now() - Duration::days(1);
    assert!(expired_invoice.validate_fiscal_compliance().is_err());
}

#[test]
fn test_payment_terms() {
    assert_eq!(PaymentTerms::Immediate.days(), 0);
    assert_eq!(PaymentTerms::Net15.days(), 15);
    assert_eq!(PaymentTerms::Net30.days(), 30);
    assert_eq!(PaymentTerms::Custom(45).days(), 45);

    assert_eq!(PaymentTerms::Immediate.description(), "Contado");
    assert_eq!(PaymentTerms::Net30.description(), "30 días");
    assert_eq!(PaymentTerms::Custom(7).description(), "7 días");
}

#[test]
fn test_currency() {
    assert_eq!(Currency::DOP.symbol(), "RD$");
    assert_eq!(Currency::DOP.code(), "DOP");

    assert_eq!(Currency::USD.symbol(), "US$");
    assert_eq!(Currency::USD.code(), "USD");

    assert_eq!(Currency::EUR.symbol(), "€");
    assert_eq!(Currency::EUR.code(), "EUR");
}

#[test]
fn test_formatted_amounts() {
    let invoice = create_test_invoice();
    let formatted = invoice.formatted_amounts();

    assert!(formatted.subtotal.starts_with("RD$"));
    assert!(formatted.itbis.starts_with("RD$"));
    assert!(formatted.total.starts_with("RD$"));
}

// Helper functions

fn create_test_seller() -> Seller {
    Seller {
        company_name: "Le Croissant Doré SRL".to_string(),
        trade_name: Some("Le Croissant Doré".to_string()),
        rnc: RNC::new("130123456".to_string()).unwrap(),
        address: Address {
            street: "Av. Abraham Lincoln".to_string(),
            number: Some("123".to_string()),
            neighborhood: Some("Piantini".to_string()),
            city: "Santo Domingo".to_string(),
            province: "Distrito Nacional".to_string(),
            postal_code: Some("10147".to_string()),
            country: "República Dominicana".to_string(),
        },
        phone: "809-555-1234".to_string(),
        email: "facturas@lecroissantdore.com".to_string(),
        website: Some("www.lecroissantdore.com".to_string()),
    }
}

fn create_test_customer_with_rnc() -> Customer {
    Customer {
        name: "Empresa Cliente SRL".to_string(),
        tax_id: Some(TaxId::RNC(RNC::new("131654321".to_string()).unwrap())),
        address: Some(Address {
            street: "Calle Principal".to_string(),
            number: Some("456".to_string()),
            neighborhood: None,
            city: "Santiago".to_string(),
            province: "Santiago".to_string(),
            postal_code: None,
            country: "República Dominicana".to_string(),
        }),
        phone: Some("809-555-5678".to_string()),
        email: Some("compras@cliente.com".to_string()),
        contact_person: Some("Juan Pérez".to_string()),
    }
}

fn create_test_customer_with_cedula() -> Customer {
    Customer {
        name: "Juan Pérez".to_string(),
        tax_id: Some(TaxId::Cedula(Cedula::new("00112345678".to_string()).unwrap())),
        address: None,
        phone: Some("809-555-9999".to_string()),
        email: Some("juan@example.com".to_string()),
        contact_person: None,
    }
}

fn create_test_invoice() -> Invoice {
    create_test_invoice_with_payment_terms(PaymentTerms::Net30)
}

fn create_test_invoice_with_payment_terms(payment_terms: PaymentTerms) -> Invoice {
    let tenant_id = TenantId::new("test".to_string());
    let ncf = NCF::new_credito_fiscal(1).unwrap();
    let ncf_expiry = Utc::now() + Duration::days(365);

    let item = InvoiceItem::new(
        "Test Product".to_string(),
        1.0,
        "UND".to_string(),
        100.0,
        0.18,
    );

    Invoice::new(
        tenant_id,
        ncf,
        ncf_expiry,
        create_test_seller(),
        create_test_customer_with_rnc(),
        vec![item],
        Currency::DOP,
        payment_terms,
    ).unwrap()
}