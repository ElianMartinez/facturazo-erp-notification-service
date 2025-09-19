use pdf_services::{
    PdfBuilder, PdfGenerator, PdfConfig, PageSize, Orientation, Margin,
    ExcelBuilder, TableData, Money, ColumnAlign
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Demo de Generación de Documentos PDF y Excel");
    println!("================================================\n");

    std::fs::create_dir_all("output")?;

    demo_pdf_configuraciones()?;
    demo_excel_reporte()?;
    demo_pdf_tabla_compleja()?;

    println!("\n✅ Todos los documentos han sido generados en la carpeta 'output/'");
    Ok(())
}

fn demo_pdf_configuraciones() -> Result<(), Box<dyn std::error::Error>> {
    println!("📄 Generando PDFs con diferentes configuraciones...");

    let configs = vec![
        ("A4 Vertical", PageSize::A4, Orientation::Portrait),
        ("Letter Horizontal", PageSize::Letter, Orientation::Landscape),
        ("A3 Vertical", PageSize::A3, Orientation::Portrait),
    ];

    for (name, page_size, orientation) in configs {
        let config = PdfConfig::builder()
            .page_size(page_size.clone())
            .orientation(orientation.clone())
            .margin(Margin::uniform(15.0))
            .font_size(11.0)
            .scale(1.0)
            .build();

        let mut builder = PdfBuilder::new().with_config(config.clone());

        builder.add_title(&format!("Documento {}", name), 1);
        builder.add_paragraph(&format!(
            "Este documento está configurado con tamaño {} y orientación {}.",
            match page_size {
                PageSize::A4 => "A4",
                PageSize::Letter => "Letter",
                PageSize::A3 => "A3",
                _ => "Personalizado"
            },
            match orientation {
                Orientation::Portrait => "vertical",
                Orientation::Landscape => "horizontal"
            }
        ));

        builder.add_line_break();
        builder.add_horizontal_line();
        builder.add_line_break();

        builder.add_title("Características del documento", 2);
        builder.add_list(vec![
            format!("Tamaño de página: {:?}", page_size),
            format!("Orientación: {:?}", orientation),
            "Márgenes uniformes de 15mm".to_string(),
            "Fuente Helvetica 11pt".to_string(),
        ], false);

        let content = builder.build();
        let mut generator = PdfGenerator::new(config);
        generator.set_content(content);

        let filename = format!("output/demo_{}.pdf", name.replace(" ", "_").to_lowercase());
        generator.render(&filename)?;
        println!("  ✓ Generado: {}", filename);
    }

    Ok(())
}

fn demo_excel_reporte() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📊 Generando reporte Excel...");

    let mut builder = ExcelBuilder::new();

    builder.add_title("Reporte de Ventas Mensuales", Some(5))?;

    let mut table = TableData::new(vec![
        "Producto".to_string(),
        "Cantidad".to_string(),
        "Precio Unitario".to_string(),
        "Subtotal".to_string(),
        "ITBIS".to_string(),
        "Total".to_string(),
    ]);

    let productos = vec![
        ("Laptop HP", 5, 45000.0),
        ("Mouse Inalámbrico", 15, 1500.0),
        ("Teclado Mecánico", 8, 3500.0),
        ("Monitor 24\"", 3, 12000.0),
        ("Cable HDMI", 20, 800.0),
    ];

    for (producto, cantidad, precio) in productos {
        let subtotal = cantidad as f64 * precio;
        let itbis = subtotal * 0.18;
        let total = subtotal + itbis;

        table.add_row(vec![
            producto.to_string(),
            cantidad.to_string(),
            format!("{:.2}", precio),
            format!("{:.2}", subtotal),
            format!("{:.2}", itbis),
            format!("{:.2}", total),
        ]);
    }

    builder.add_table(&table)?;
    builder.skip_rows(1);

    builder.add_formula_row(
        "TOTALES",
        3,
        "=SUM(D2:D6)"
    )?;

    builder.set_column_widths(vec![
        (0, 20.0),
        (1, 12.0),
        (2, 15.0),
        (3, 15.0),
        (4, 12.0),
        (5, 15.0),
    ])?;

    builder.freeze_top_row()?;

    builder.save("output/reporte_ventas.xlsx")?;
    println!("  ✓ Generado: output/reporte_ventas.xlsx");

    Ok(())
}

fn demo_pdf_tabla_compleja() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📑 Generando PDF con tabla compleja...");

    let config = PdfConfig::builder()
        .page_size(PageSize::Letter)
        .orientation(Orientation::Portrait)
        .margin(Margin::new(25.0, 25.0, 20.0, 20.0))
        .font_size(10.0)
        .build();

    let mut builder = PdfBuilder::new().with_config(config.clone());

    builder.add_title("Reporte de Inventario", 1);
    builder.add_paragraph("Resumen del inventario actual con análisis de rotación");
    builder.add_line_break();

    let mut table = TableData::new(vec![
        "SKU".to_string(),
        "Producto".to_string(),
        "Stock".to_string(),
        "Precio".to_string(),
        "Valor Total".to_string(),
        "Estado".to_string(),
    ]);

    table = table.with_column_widths(vec![60.0, 150.0, 60.0, 80.0, 100.0, 80.0])
        .with_alignment(vec![
            ColumnAlign::Center,
            ColumnAlign::Left,
            ColumnAlign::Center,
            ColumnAlign::Right,
            ColumnAlign::Right,
            ColumnAlign::Center,
        ]);

    let inventario = vec![
        ("A001", "Laptop Dell XPS 13", 12, 55000.0, "Óptimo"),
        ("A002", "iPad Pro 11\"", 8, 42000.0, "Bajo"),
        ("B001", "Samsung Galaxy S24", 25, 38000.0, "Óptimo"),
        ("B002", "iPhone 15 Pro", 5, 65000.0, "Crítico"),
        ("C001", "AirPods Pro", 30, 12000.0, "Óptimo"),
    ];

    for (sku, producto, stock, precio, estado) in inventario {
        let valor_total = stock as f64 * precio;
        table.add_row(vec![
            sku.to_string(),
            producto.to_string(),
            stock.to_string(),
            Money::new(precio).format(),
            Money::new(valor_total).format(),
            estado.to_string(),
        ]);
    }

    builder.add_table(&table);
    builder.add_line_break();

    builder.add_box(
        "Nota: Los productos marcados como 'Crítico' requieren reabastecimiento inmediato.",
        Some("255, 243, 224"),
        true
    );

    builder.add_page_break();
    builder.add_title("Análisis de Inventario", 1);

    builder.add_grid(2, vec![
        "Total de SKUs: 5".to_string(),
        "Valor total del inventario: \\$2,465,000.00".to_string(),
        "Items en estado crítico: 1".to_string(),
        "Items en estado bajo: 1".to_string(),
    ]);

    let content = builder.build();
    let mut generator = PdfGenerator::new(config);
    generator.set_content(content);

    generator.render("output/inventario_complejo.pdf")?;
    println!("  ✓ Generado: output/inventario_complejo.pdf");

    Ok(())
}