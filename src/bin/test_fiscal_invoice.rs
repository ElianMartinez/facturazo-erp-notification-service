//! Test fiscal invoice generation with all item fields
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
    println!("Generando factura fiscal con FiscalInvoiceTemplate (500 items)...\n");

    let work_dir = PathBuf::from("./temp");
    std::fs::create_dir_all(&work_dir)?;

    let factory = GeneratorFactory::new(work_dir);

    // Generate 500 items with ALL fields populated
    let items: Vec<serde_json::Value> = (1..=500)
        .map(|i| {
            let unit_price = 100.0 + (i as f64 * 10.0);
            let quantity = 1.0 + (i % 10) as f64;
            let subtotal = unit_price * quantity;
            let discount = if i % 5 == 0 { subtotal * 0.05 } else { 0.0 }; // 5% discount every 5th item
            let taxable = subtotal - discount;
            let tax_rate = 18.0;
            let tax_amount = taxable * (tax_rate / 100.0);
            let total = taxable + tax_amount;

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

            json!({
                "code": format!("PROD-{:04}", i),
                "description": format!("{} {} - Item de prueba numero {} con descripcion extendida para verificar el manejo de texto largo en la factura",
                    categories[(i - 1) as usize % categories.len()],
                    i,
                    i
                ),
                "quantity": quantity,
                "unit": units[(i - 1) as usize % units.len()],
                "unit_price": unit_price,
                "tax_rate": tax_rate,
                "tax_amount": tax_amount,
                "discount": if discount > 0.0 { Some(discount) } else { None::<f64> },
                "subtotal": subtotal,
                "total": total,
                "document_date": format!("2026-01-{:02}", ((i - 1) % 28) + 1),
                "notes": format!("Nota del item {}: Referencia interna REF-{:06}, Lote: LOT-{:04}", i, i * 100, i)
            })
        })
        .collect();

    // Calculate totals
    let total_subtotal: f64 = items
        .iter()
        .filter_map(|item| item["subtotal"].as_f64())
        .sum();
    let total_discount: f64 = items
        .iter()
        .filter_map(|item| item["discount"].as_f64())
        .sum();
    let total_tax: f64 = items
        .iter()
        .filter_map(|item| item["tax_amount"].as_f64())
        .sum();
    let grand_total: f64 = items.iter().filter_map(|item| item["total"].as_f64()).sum();

    // Datos de factura completos para FiscalInvoiceTemplate
    let invoice_data = json!({
        "invoice_number": "FAC-2026-0500",
        "issue_date": "2026-01-30",
        "due_date": "2026-02-28",
        "company_info": {
            "name": "Facturazo ERP S.R.L.",
            "legal_name": "Facturazo ERP Soluciones Tecnologicas S.R.L.",
            "tax_id": "133478341",
            "address": {
                "street": "Calle Principal #123, Torre Empresarial, Piso 5",
                "city": "Santo Domingo",
                "state": "Distrito Nacional",
                "country": "Republica Dominicana",
                "postal_code": "10101"
            },
            "phone": "809-555-1234",
            "email": "facturacion@facturazo.com",
            "website": "https://facturazo.com"
        },
        "client_info": {
            "name": "Mega Distribuidora Nacional S.R.L.",
            "legal_name": "Mega Distribuidora Nacional Importaciones y Exportaciones S.R.L.",
            "tax_id": "131999888",
            "address": {
                "street": "Av. John F. Kennedy Km 6.5, Plaza Comercial Kennedy, Local 201-B",
                "city": "Santo Domingo",
                "state": "Distrito Nacional",
                "country": "Republica Dominicana",
                "postal_code": "10501"
            },
            "phone": "809-567-8900",
            "email": "compras@megadistribuidora.com.do"
        },
        "items": items,
        "totals": {
            "subtotal": total_subtotal,
            "discount_total": total_discount,
            "tax_total": total_tax,
            "grand_total": grand_total
        },
        "fiscal_info": {
            "e_ncf": "E310000000500",
            "security_code": "MEGA-TEST-500",
            "signature_date": "2026-01-30",
            "authorization_number": "DGII-2026-0005000",
            "qr_data": ""
        },
        "payment_info": {
            "method": "Credito 30 dias",
            "bank_name": "Banco Popular Dominicano",
            "account_number": "801-123456-7",
            "terms": "Neto 30 dias"
        },
        "notes": "Factura de prueba con 500 items para verificar rendimiento y paginacion del template FiscalInvoice. Todos los campos de items estan poblados incluyendo: code, description, quantity, unit, unit_price, tax_rate, tax_amount, discount, subtotal, total, document_date y notes.",
        "currency": "DOP"
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
    println!("\nTotales:");
    println!("  Subtotal:  RD$ {:.2}", total_subtotal);
    println!("  Descuento: RD$ {:.2}", total_discount);
    println!("  ITBIS:     RD$ {:.2}", total_tax);
    println!("  Total:     RD$ {:.2}", grand_total);

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
            println!("   MIME Type: {}", gen_result.mime_type);
            println!("   Tiempo: {:.2?}", elapsed);
        }
        Err(e) => {
            eprintln!("\n Error al generar factura: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
