//! Domain DTOs tests - Only serialization and structure tests
//! NO business logic tests - that belongs to core service

use pdf_services::domain::{
    fiscal::{NCF, RNC, Cedula, ITBIS, TaxId, TaxIdType},
    invoice::{InvoiceData, SellerData, CustomerData, InvoiceItemData, InvoiceStatus},
    document::{Document, DocumentId, DocumentType, DocumentStatus},
};
use serde_json;

#[test]
fn test_ncf_dto_serialization() {
    let ncf = NCF {
        value: "E3100000001".to_string(),
        formatted: Some("E31-0000-0001".to_string()),
        serie: Some("E".to_string()),
        tipo_description: Some("Factura de Crédito Fiscal".to_string()),
    };

    // Should serialize and deserialize correctly
    let json = serde_json::to_string(&ncf).unwrap();
    let deserialized: NCF = serde_json::from_str(&json).unwrap();

    assert_eq!(ncf.value, deserialized.value);
    assert_eq!(ncf.formatted, deserialized.formatted);
}

#[test]
fn test_rnc_dto_serialization() {
    let rnc = RNC {
        value: "130123456".to_string(),
        formatted: Some("1-30-12345-6".to_string()),
    };

    let json = serde_json::to_string(&rnc).unwrap();
    let deserialized: RNC = serde_json::from_str(&json).unwrap();

    assert_eq!(rnc.value, deserialized.value);
    assert_eq!(rnc.formatted, deserialized.formatted);
}

#[test]
fn test_invoice_data_dto() {
    let invoice_json = r#"{
        "id": "INV-001",
        "invoice_number": "2024-001",
        "tenant_id": "tenant-123",
        "ncf": {
            "value": "E3100000001",
            "formatted": "E31-0000-0001"
        },
        "seller": {
            "company_name": "Test Company",
            "rnc": {
                "value": "130123456"
            }
        },
        "customer": {
            "name": "Test Customer",
            "tax_id": {
                "value": "131654321",
                "tax_type": "RNC"
            }
        },
        "items": [
            {
                "description": "Product A",
                "quantity": 2,
                "unit_of_measure": "UND",
                "unit_price": 100.0,
                "subtotal": 200.0,
                "discount_amount": 0.0,
                "itbis_amount": 36.0,
                "total": 236.0
            }
        ],
        "subtotal": 200.0,
        "discount_amount": 0.0,
        "itbis_amount": 36.0,
        "total_amount": 236.0,
        "currency": "DOP",
        "issue_date": "2024-11-24",
        "status": "issued"
    }"#;

    let invoice: InvoiceData = serde_json::from_str(invoice_json).unwrap();

    assert_eq!(invoice.id, "INV-001");
    assert_eq!(invoice.invoice_number, "2024-001");
    assert_eq!(invoice.tenant_id, "tenant-123");
    assert_eq!(invoice.items.len(), 1);
    assert_eq!(invoice.total_amount, 236.0);
}

#[test]
fn test_document_dto() {
    let doc = Document {
        id: DocumentId::new(),
        tenant_id: "tenant-123".to_string(),
        document_type: DocumentType::Invoice,
        status: DocumentStatus::Pending,
        template_id: "invoice_fiscal".to_string(),
        template_version: "1.0.0".to_string(),
        data: serde_json::json!({"test": "data"}),
        format: pdf_services::domain::document::DocumentFormat::Pdf,
        storage_path: None,
        storage_url: None,
        size_bytes: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    // Should serialize without errors
    let json = serde_json::to_string(&doc).unwrap();
    assert!(json.contains("tenant-123"));
}

#[test]
fn test_invoice_status_serialization() {
    let status = InvoiceStatus::Paid;
    assert_eq!(status.as_str(), "paid");

    let json = serde_json::to_string(&status).unwrap();
    assert!(json.contains("Paid"));
}

#[test]
fn test_document_type_serialization() {
    let doc_type = DocumentType::Invoice;
    assert_eq!(doc_type.as_str(), "invoice");

    let json = serde_json::to_string(&doc_type).unwrap();
    let deserialized: DocumentType = serde_json::from_str(&json).unwrap();
    assert_eq!(doc_type, deserialized);
}

#[test]
fn test_itbis_dto() {
    let itbis = ITBIS {
        rate: 0.18,
        amount: 180.0,
    };

    let json = serde_json::to_string(&itbis).unwrap();
    let deserialized: ITBIS = serde_json::from_str(&json).unwrap();

    assert_eq!(itbis.rate, deserialized.rate);
    assert_eq!(itbis.amount, deserialized.amount);
}

#[test]
fn test_tax_id_types() {
    let tax_id = TaxId {
        value: "130123456".to_string(),
        tax_type: TaxIdType::RNC,
        formatted: Some("1-30-12345-6".to_string()),
    };

    let json = serde_json::to_string(&tax_id).unwrap();
    assert!(json.contains("RNC"));
    assert!(json.contains("130123456"));
}