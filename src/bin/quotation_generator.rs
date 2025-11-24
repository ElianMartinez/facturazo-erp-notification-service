use pdf_services::templates::templates::{QuotationTemplate, QuotationData, QuotationItem, Customer, Company, MonthlyRent};
use pdf_services::templates::template_trait::TypstTemplate;
use std::fs;
use std::process::Command;
use chrono::Local;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Generador de Cotizaciones - Facturazo");
    println!("=========================================\n");

    // Crear directorio para las cotizaciones si no existe
    fs::create_dir_all("cotizaciones")?;

    // Configurar datos de la empresa
    let company = Company {
        name: "Inverisap".to_string(),
        description: "Soluciones de Facturación Electrónica".to_string(),
        logo_path: "Facturazo-Icon.svg".to_string(),
    };

    // Datos del cliente - CREDIGAS S.A.
    let customer = Customer {
        name: "CREDIGAS S A".to_string(),
        rnc_cedula: Some("101-12243-9".to_string()),
        phone: None,
    };

    // Items de la cotización - Inversión inicial
    let items = vec![
        QuotationItem {
            quantity: 1,
            description: "Implementación de sistema y Proceso de Certificación de la empresa".to_string(),
            price: 5000.00, // USD
            tax_rate: 0.18, // 18% ITBIS
        },
        QuotationItem {
            quantity: 26,
            description: "Teléfonos IP profesionales con configuración completa".to_string(),
            price: 200.00, // USD por unidad
            tax_rate: 0.18, // 18% ITBIS
        },
        QuotationItem {
            quantity: 1,
            description: "Soporte técnico especializado 24/7 (incluido en el servicio)".to_string(),
            price: 0.00, // USD - Incluido sin costo
            tax_rate: 0.00, // Sin impuesto
        },
    ];

    // Información de renta mensual
    let monthly_rent = MonthlyRent {
        description: "Facturas ilimitadas con XML seguro por 10 años, respaldos automáticos en 3 ubicaciones diferentes - $180 por sucursal (26 sucursales)".to_string(),
        subtotal: 4680.00, // USD - $180 x 26 sucursales
        tax_rate: 0.18, // 18% ITBIS
    };

    // Crear la cotización
    let quotation_data = QuotationData {
        company: company.clone(),
        customer: customer.clone(),
        items: items.clone(),
        discount: 0.00, // Sin descuento
        date: Local::now().format("%d/%m/%Y").to_string(),
        quotation_number: "COT-2025-039".to_string(),
        monthly_rent: Some(monthly_rent),
    };

    // Generar PDF
    let output_path = "cotizaciones/cotizacion_credigas.pdf";
    println!("📋 Generando cotización para: {}", quotation_data.customer.name);
    println!("📦 Items en la cotización: {}", quotation_data.items.len());
    println!("💰 Subtotal: ${:.2}", quotation_data.subtotal());
    println!("💸 Descuento: ${:.2}", quotation_data.discount);
    println!("🧾 ITBIS Total: ${:.2}", quotation_data.total_tax());
    println!("💵 Total Final: ${:.2}", quotation_data.total_after_discount());

    let template = QuotationTemplate::new();
    let typst_content = template.generate(&serde_json::to_value(&quotation_data)?)?;

    let temp_file = format!("temp_quotation_{}.typ", quotation_data.quotation_number);
    fs::write(&temp_file, typst_content)?;

    let output = Command::new("typst")
        .arg("compile")
        .arg(&temp_file)
        .arg(output_path)
        .output()?;

    // fs::remove_file(&temp_file)?;

    if !output.status.success() {
        eprintln!("\n❌ Error generando cotización: {}", String::from_utf8_lossy(&output.stderr));
        return Err("Error compilando Typst".into());
    }

    println!("\n✅ Cotización generada exitosamente!");
    println!("📄 Archivo: {}", output_path);
    println!("\n🎉 Proceso completado exitosamente!");
    println!("📁 La cotización está en el directorio 'cotizaciones/'");

    Ok(())
}