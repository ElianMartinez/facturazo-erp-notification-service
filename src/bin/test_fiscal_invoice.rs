//! Test fiscal invoice generation with pre-calculated data from core-service
//!
//! This simulates data that comes from InvoiceDetailsDto with ALL calculations done:
//! - TaxableAmount1 (ITBIS 18%)
//! - TaxableAmount2 (ITBIS 16%)
//! - Itbis1Amount, Itbis2Amount
//! - TipAmount (propina legal)
//! - DiscountAmount
//! - All line items pre-calculated
//!
//! Run: cargo run --bin test_fiscal_invoice

use pdf_services::domain::document::DocumentFormat;
use pdf_services::infrastructure::generators::{
    DocumentType as GenDocType, GenerationOptions, GeneratorFactory,
};
use serde_json::json;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Generando factura fiscal con datos de InvoiceDetailsDto (500 items)...\n");

    let work_dir = PathBuf::from("./temp");
    std::fs::create_dir_all(&work_dir)?;

    let factory = GeneratorFactory::new(work_dir);

    // Simulate 500 items matching InvoiceDetailItemDto structure
    // ALL values come pre-calculated from core-service
    let items: Vec<serde_json::Value> = (1..=500)
        .map(|i| {
            // All these values come pre-calculated from core-service
            let unit_price = 100.0 + (i as f64 * 10.0);
            let quantity = 1.0 + (i % 10) as f64;
            let subtotal = unit_price * quantity;
            let discount_percentage = if i % 5 == 0 { 5.0 } else { 0.0 };
            let discount_amount = subtotal * (discount_percentage / 100.0);
            let taxable_amount = subtotal - discount_amount;

            // Tax rate alternates between 18% and 16%
            let itbis_rate = if i % 3 == 0 { 16.0 } else { 18.0 };
            let itbis_amount = taxable_amount * (itbis_rate / 100.0);
            let line_total = taxable_amount + itbis_amount;
            let is_exempt = false;

            let units = ["UND", "HRS", "KG", "LT", "MT", "CJ", "PAQ", "SRV"];
            let categories = [
                "Producto",
                "Servicio",
                "Material",
                "Equipo",
                "Repuesto",
                "Consumible",
                "Licencia",
                "Mantenimiento",
            ];

            // Matching InvoiceDetailItemDto structure
            json!({
                "code": format!("PROD-{:04}", i),
                "description": format!("{} {} - Item de prueba",
                    categories[(i - 1) as usize % categories.len()], i
                ),
                "quantity": quantity,
                "unit": units[(i - 1) as usize % units.len()],
                "unit_price": unit_price,
                // Pre-calculated fields from core-service
                "discount_percentage": discount_percentage,
                "discount": discount_amount,
                "subtotal": subtotal,
                "tax_rate": itbis_rate,
                "tax_amount": itbis_amount,
                "is_exempt": is_exempt,
                "total": line_total,
                // Optional fields
                "document_date": format!("2026-01-{:02}", ((i - 1) % 28) + 1),
                "notes": format!("Ref: REF-{:06}", i * 100)
            })
        })
        .collect();

    // ============================================================
    // TOTALS FROM InvoiceDetailsDto - ALL PRE-CALCULATED
    // These come from the core-service, pdf-service does NOT calculate
    // ============================================================

    // Simulating pre-calculated totals from core-service
    let taxable_amount_1: f64 = 4787500.0; // Monto gravado ITBIS 18%
    let taxable_amount_2: f64 = 2349375.0; // Monto gravado ITBIS 16%
    let taxable_amount_exempt: f64 = 0.0; // Monto exento
    let itbis_1_amount: f64 = 861750.0; // ITBIS 18%
    let itbis_2_amount: f64 = 375900.0; // ITBIS 16%
    let discount_amount: f64 = 45625.0; // Total descuentos
    let tip_amount: f64 = 0.0; // Propina legal (10%)
    let other_taxes_amount: f64 = 0.0; // Otros impuestos
    let total_amount: f64 = 8374525.0; // Total factura

    // ============================================================
    // INVOICE DATA - Matching InvoiceDetailsDto structure
    // ============================================================
    let invoice_data = json!({
        "invoice_number": "FAC-2026-0500",
        "issue_date": "2026-01-30",
        "due_date": "2026-02-28",
        "currency": "DOP",

        // Company info (from BranchOffice + Tenant)
        "company_info": {
            "name": "Facturazo ERP S.R.L.",
            "legal_name": "Facturazo ERP Soluciones Tecnologicas S.R.L.",
            "tax_id": "133478341",
            "address": {
                "street": "Calle Principal #123, Torre Empresarial, Piso 5",
                "city": "Santo Domingo",
                "state": "Distrito Nacional",
                "country": "Republica Dominicana"
            },
            "phone": "809-555-1234",
            "email": "facturacion@facturazo.com"
        },

        // Client info (from InvoiceCustomerInfo)
        "client_info": {
            "name": "Mega Distribuidora Nacional S.R.L.",
            "legal_name": "Mega Distribuidora Nacional S.R.L.",
            "tax_id": "131999888",
            "address": {
                "street": "Av. John F. Kennedy Km 6.5",
                "city": "Santo Domingo",
                "state": "Distrito Nacional",
                "country": "Republica Dominicana"
            },
            "phone": "809-567-8900",
            "email": "compras@megadistribuidora.com.do"
        },

        // Items (from InvoiceDetailItemDto[])
        "items": items,

        // Pre-calculated totals (from InvoiceTotals)
        "totals": {
            "subtotal": taxable_amount_1 + taxable_amount_2 + taxable_amount_exempt,
            "discount_total": discount_amount,
            "tax_total": itbis_1_amount + itbis_2_amount,
            "grand_total": total_amount
        },

        // DGII-specific fields for detailed breakdown
        // Mapped from InvoiceDetailsDto fields in core-service
        "custom_fields": {
            // Operation info from core-service
            "OperationSequence": 12345,              // No. Factura (from operation_sequence)
            "OperationTypeCode": "05302",            // 05301 = Contado, 05302 = Crédito
            // Subtotals by tax rate
            "TaxableAmount1": taxable_amount_1,      // Subtotal Gravado 18%
            "TaxableAmount2": taxable_amount_2,      // Subtotal Gravado 16%
            "TaxableAmountExempt": taxable_amount_exempt, // Subtotal Exento
            // Pre-calculated combined totals
            "SubtotalGravado": taxable_amount_1 + taxable_amount_2,
            // ITBIS by rate
            "Itbis1Amount": itbis_1_amount,          // ITBIS 18%
            "Itbis2Amount": itbis_2_amount,          // ITBIS 16%
            "TotalItbis": itbis_1_amount + itbis_2_amount,
            // Other amounts
            "TipAmount": tip_amount,                 // Propina Legal
            "DiscountAmount": discount_amount,       // Descuento
            "OtherTaxesAmount": other_taxes_amount   // Otros Impuestos
        },

        // Fiscal info (from ElectronicInvoiceDetails)
        "fiscal_info": {
            "e_ncf": "E310000000500",                // Encf
            "security_code": "MEGA-TEST-500",        // SecurityCode
            "signature_date": "2026-01-30",          // SignedDate
            "qr_data": ""                            // QrUrl
        },

        // Payment info (from PaymentMethods)
        "payment_info": {
            "method": "Credito 30 dias",
            "terms": "Neto 30 dias"
        },

        "notes": "Datos simulados de InvoiceDetailsDto con todos los campos pre-calculados."
    });

    let options = GenerationOptions {
        watermark: None,
        password_protect: false,
        compress: true,
        include_attachments: false,
    };

    println!("Empresa: {}", invoice_data["company_info"]["name"]);
    println!("Cliente: {}", invoice_data["client_info"]["name"]);
    println!("NCF: {}", invoice_data["fiscal_info"]["e_ncf"]);
    println!("Items: {}", invoice_data["items"].as_array().unwrap().len());

    println!("\nTotales (de InvoiceDetailsDto):");
    println!("  TaxableAmount1 (18%):   RD$ {:.2}", taxable_amount_1);
    println!("  TaxableAmount2 (16%):   RD$ {:.2}", taxable_amount_2);
    println!("  TaxableAmountExempt:    RD$ {:.2}", taxable_amount_exempt);
    println!("  ----------------------------------");
    println!("  Itbis1Amount (18%):     RD$ {:.2}", itbis_1_amount);
    println!("  Itbis2Amount (16%):     RD$ {:.2}", itbis_2_amount);
    println!("  ----------------------------------");
    println!("  DiscountAmount:         RD$ {:.2}", discount_amount);
    println!("  TipAmount:              RD$ {:.2}", tip_amount);
    println!("  OtherTaxesAmount:       RD$ {:.2}", other_taxes_amount);
    println!("  ==================================");
    println!("  TotalAmount:            RD$ {:.2}", total_amount);

    println!("\nGenerando PDF...");
    let start = std::time::Instant::now();

    let result = factory
        .generate(
            GenDocType::Invoice,
            DocumentFormat::Pdf,
            invoice_data,
            options,
        )
        .await;

    let elapsed = start.elapsed();

    match result {
        Ok(gen_result) => {
            let output_path = "facturas/factura_fiscal_500_items.pdf";
            std::fs::create_dir_all("facturas")?;
            std::fs::write(output_path, &gen_result.document_bytes)?;

            println!("\n Factura fiscal generada exitosamente!");
            println!("   Archivo: {}", output_path);
            println!(
                "   Tamano: {} bytes ({:.2} KB)",
                gen_result.document_bytes.len(),
                gen_result.document_bytes.len() as f64 / 1024.0
            );
            println!("   Tiempo: {:.2?}", elapsed);
        }
        Err(e) => {
            eprintln!("\n Error al generar factura: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
