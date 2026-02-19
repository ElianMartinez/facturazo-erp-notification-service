//! Integration tests for document generation infrastructure
//!
//! Tests the GeneratorFactory, Orchestrators, and related services

use pdf_services::application::commands::GenerateDocumentCommand;
use pdf_services::application::orchestrators::DocumentOrchestrator;
use pdf_services::domain::document::{DocumentFormat, DocumentType as DomainDocType};
use pdf_services::infrastructure::cache::CacheService;
use pdf_services::infrastructure::generators::{
    DocumentType as GenDocType, GenerationOptions, GeneratorFactory,
};
use pdf_services::infrastructure::storage::StorageService;
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;

/// Test basic PDF generation with invoice data
/// Uses the DGII-compliant FiscalInvoiceTemplate format
#[tokio::test]
async fn test_generate_invoice_pdf() {
    let work_dir = tempdir().unwrap();
    let factory = GeneratorFactory::new(work_dir.path().to_path_buf());

    // Data format expected by FiscalInvoiceTemplate (DGII-compliant)
    let invoice_data = json!({
        "invoice_number": "INV-2024-001",
        "issue_date": "2024-11-24",
        "due_date": "2024-12-24",
        "company_info": {
            "name": "Test Company S.R.L.",
            "legal_name": "Test Company S.R.L.",
            "tax_id": "130123456",
            "address": {
                "street": "Calle Principal #123",
                "city": "Santo Domingo",
                "state": "DN",
                "country": "DO"
            },
            "phone": "809-555-1234",
            "email": "test@company.com"
        },
        "client_info": {
            "name": "Cliente de Prueba",
            "tax_id": "131654321",
            "address": {
                "street": "Av. Duarte #456",
                "city": "Santiago",
                "country": "DO"
            }
        },
        "items": [
            {
                "description": "Producto de prueba",
                "quantity": 2.0,
                "unit": "UND",
                "unit_price": 100.0,
                "tax_rate": 18.0
            }
        ],
        "fiscal_info": {
            "e_ncf": "E310000000001",
            "security_code": "ABC123",
            "signature_date": "2024-11-24",
            "qr_data": ""
        }
    });

    let options = GenerationOptions {
        watermark: None,
        password_protect: false,
        compress: true,
        include_attachments: false,
    };

    let result = factory
        .generate(
            GenDocType::Invoice,
            DocumentFormat::Pdf,
            invoice_data,
            options,
        )
        .await;

    assert!(
        result.is_ok(),
        "Invoice PDF generation failed: {:?}",
        result.err()
    );

    let result = result.unwrap();
    assert!(
        !result.document_bytes.is_empty(),
        "Generated PDF should not be empty"
    );
    assert_eq!(result.mime_type, "application/pdf");
}

/// Test CSV generation for reports
#[tokio::test]
async fn test_generate_report_csv() {
    let work_dir = tempdir().unwrap();
    let factory = GeneratorFactory::new(work_dir.path().to_path_buf());

    let report_data = json!({
        "title": "Reporte de Ventas",
        "period": "Noviembre 2024",
        "columns": ["Fecha", "Producto", "Cantidad", "Total"],
        "rows": [
            ["2024-11-01", "Producto A", "10", "1000.00"],
            ["2024-11-02", "Producto B", "5", "500.00"],
            ["2024-11-03", "Producto C", "8", "800.00"]
        ],
        "summary": {
            "total_rows": 3,
            "total_amount": 2300.00
        }
    });

    let options = GenerationOptions {
        watermark: None,
        password_protect: false,
        compress: false,
        include_attachments: false,
    };

    let result = factory
        .generate(
            GenDocType::Report,
            DocumentFormat::Csv,
            report_data,
            options,
        )
        .await;

    assert!(result.is_ok(), "CSV generation failed: {:?}", result.err());

    let result = result.unwrap();
    assert!(
        !result.document_bytes.is_empty(),
        "Generated CSV should not be empty"
    );
    assert_eq!(result.mime_type, "text/csv");

    // Verify CSV content
    let csv_content = String::from_utf8(result.document_bytes).unwrap();
    assert!(
        csv_content.contains("Producto A"),
        "CSV should contain test data"
    );
}

/// Test Excel generation for reports
#[tokio::test]
async fn test_generate_report_excel() {
    let work_dir = tempdir().unwrap();
    let factory = GeneratorFactory::new(work_dir.path().to_path_buf());

    let report_data = json!({
        "title": "Reporte Mensual",
        "generated_at": "2024-11-24",
        "columns": ["ID", "Descripcion", "Monto"],
        "rows": [
            ["001", "Item 1", 100.0],
            ["002", "Item 2", 200.0],
            ["003", "Item 3", 300.0]
        ]
    });

    let options = GenerationOptions::default();

    let result = factory
        .generate(
            GenDocType::Report,
            DocumentFormat::Excel,
            report_data,
            options,
        )
        .await;

    assert!(
        result.is_ok(),
        "Excel generation failed: {:?}",
        result.err()
    );

    let result = result.unwrap();
    assert!(
        !result.document_bytes.is_empty(),
        "Generated Excel should not be empty"
    );
    assert_eq!(
        result.mime_type,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    );

    // Verify it starts with XLSX magic bytes (PK header for ZIP)
    assert!(
        result.document_bytes.starts_with(&[0x50, 0x4B]),
        "Excel file should be a valid ZIP/XLSX"
    );
}

/// Test cache service operations
#[tokio::test]
async fn test_cache_service() {
    let cache = CacheService::new();

    // Test set and get
    let test_data = vec![1u8, 2, 3, 4, 5];
    cache.set("test_key", &test_data, 60).await;

    let retrieved: Option<Vec<u8>> = cache.get("test_key").await;
    assert!(retrieved.is_some(), "Should retrieve cached data");
    assert_eq!(retrieved.unwrap(), test_data);

    // Test non-existent key
    let missing: Option<Vec<u8>> = cache.get("non_existent").await;
    assert!(missing.is_none(), "Non-existent key should return None");
}

/// Test storage service operations
#[tokio::test]
async fn test_storage_service() {
    let temp_dir = tempdir().unwrap();
    let storage = StorageService::new(
        temp_dir.path().to_path_buf(),
        "http://localhost".to_string(),
    );

    let test_data = b"Test PDF content";

    // Upload
    let url = storage
        .upload("test/doc.pdf", test_data, "application/pdf")
        .await;
    assert!(url.is_ok(), "Upload should succeed: {:?}", url.err());

    // Download
    let downloaded = storage.download("test/doc.pdf").await;
    assert!(downloaded.is_ok(), "Download should succeed");
    assert_eq!(downloaded.unwrap(), test_data.to_vec());

    // Exists
    let exists = storage.exists("test/doc.pdf").await;
    assert!(exists.is_ok());
    assert!(exists.unwrap(), "File should exist");

    // Delete
    let deleted = storage.delete("test/doc.pdf").await;
    assert!(deleted.is_ok(), "Delete should succeed");

    // Verify deleted
    let exists_after = storage.exists("test/doc.pdf").await;
    assert!(exists_after.is_ok());
    assert!(
        !exists_after.unwrap(),
        "File should not exist after deletion"
    );
}

/// Test document orchestrator workflow
#[tokio::test]
async fn test_document_orchestrator() {
    let temp_dir = tempdir().unwrap();
    let work_dir = temp_dir.path().to_path_buf();

    let generator_factory = Arc::new(GeneratorFactory::new(work_dir.clone()));
    let cache_service = Arc::new(CacheService::new());
    let storage_service = Arc::new(StorageService::new(
        temp_dir.path().join("storage"),
        "".to_string(),
    ));

    let typst_generator = Arc::new(
        pdf_services::infrastructure::generators::TypstGenerator::new(temp_dir.path().join("work")),
    );

    let orchestrator = DocumentOrchestrator::new(
        generator_factory,
        storage_service,
        cache_service.clone(),
        None,
        None,
        None, // template_resolver
        typst_generator,
    );

    // Data format expected by FiscalInvoiceTemplate (DGII-compliant)
    let command = GenerateDocumentCommand {
        tenant_id: "test-tenant".to_string(),
        user_id: Some("test-user".to_string()),
        document_type: DomainDocType::Invoice,
        format: DocumentFormat::Pdf,
        data: json!({
            "invoice_number": "TEST-001",
            "issue_date": "2024-11-24",
            "due_date": "2024-12-24",
            "company_info": {
                "name": "Test Company",
                "tax_id": "130123456",
                "address": {
                    "street": "Calle Test #1",
                    "city": "Santo Domingo",
                    "country": "DO"
                }
            },
            "client_info": {
                "name": "Customer Test",
                "tax_id": "131654321"
            },
            "items": [
                {
                    "description": "Test Item",
                    "quantity": 1.0,
                    "unit": "UND",
                    "unit_price": 100.0,
                    "tax_rate": 18.0
                }
            ],
            "fiscal_info": {
                "e_ncf": "E310000000001",
                "security_code": "TEST123",
                "signature_date": "2024-11-24",
                "qr_data": ""
            }
        }),
        template_id: "invoice_fiscal".to_string(),
        template_version: "1.0.0".to_string(),
        storage_enabled: true,
        notification_enabled: false,
        priority: pdf_services::application::commands::Priority::Normal,
    };

    let result = orchestrator.execute(command).await;

    assert!(
        result.is_ok(),
        "Orchestrator should succeed: {:?}",
        result.err()
    );

    let doc_result = result.unwrap();
    assert!(
        !doc_result.document_id.is_empty(),
        "Should have document ID"
    );
    assert!(
        !doc_result.document_bytes.is_empty(),
        "Should have document bytes"
    );
    assert!(
        doc_result.storage_url.is_some(),
        "Should have storage URL when enabled"
    );
    assert!(
        doc_result.generation_time_ms > 0,
        "Should have generation time"
    );
}

/// Test QR code generation
#[tokio::test]
async fn test_qr_code_generation() {
    use pdf_services::infrastructure::generators::qr_generator::QRGenerator;

    let result = QRGenerator::generate_qr("https://dgii.gov.do/ecf/verificacion?ncf=E310000001");

    assert!(result.is_ok(), "QR generation failed: {:?}", result.err());

    let svg_data = String::from_utf8(result.unwrap()).unwrap();
    // SVG might have XML declaration or start with <?xml or <svg
    assert!(
        svg_data.contains("<svg") || svg_data.contains("<?xml"),
        "Should be valid SVG, got: {}",
        &svg_data[..100.min(svg_data.len())]
    );
    assert!(svg_data.contains("</svg>"), "Should have closing SVG tag");
}

/// Test generation options default
#[test]
fn test_generation_options_default() {
    let options = GenerationOptions::default();

    assert!(options.watermark.is_none());
    assert!(!options.password_protect);
    assert!(options.compress);
    assert!(!options.include_attachments);
}

/// Test document format file extensions
#[test]
fn test_document_format_extensions() {
    assert_eq!(DocumentFormat::Pdf.file_extension(), "pdf");
    assert_eq!(DocumentFormat::Excel.file_extension(), "xlsx");
    assert_eq!(DocumentFormat::Csv.file_extension(), "csv");
}

#[tokio::test]
async fn test_visual_codes_qr_generation() {
    use pdf_services::infrastructure::generators::visual_codes;
    use std::path::Path;

    let output_dir = Path::new("/tmp/typst-test");
    std::fs::create_dir_all(output_dir).unwrap();

    let source = std::fs::read_to_string("/tmp/typst-test/factura_real_v3_rendered.typ").unwrap();
    let processed = visual_codes::process_directives(&source, output_dir).unwrap();

    // Verify QR was generated
    assert!(
        processed.contains("#image(\"qr_"),
        "Should contain #image for QR"
    );
    assert!(
        !processed.contains("{{qr"),
        "Should not contain QR directive anymore"
    );

    // Write processed output
    std::fs::write("/tmp/typst-test/factura_final.typ", &processed).unwrap();
    println!("Processed template written to /tmp/typst-test/factura_final.typ");
}
