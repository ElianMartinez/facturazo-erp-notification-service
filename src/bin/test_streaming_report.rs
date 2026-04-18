//! Quick sanity test for the new StreamingReportGenerator.
//!
//! Run:
//!   cargo run --release --bin test-streaming-report -- flat 1000
//!   cargo run --release --bin test-streaming-report -- grouped 600
//!   cargo run --release --bin test-streaming-report -- hierarchical 600
//!
//! Outputs PDF to /tmp/test_streaming_<type>_<n>.pdf and prints metrics.

use anyhow::Result;
use pdf_services::infrastructure::generators::streaming_report_generator::StreamingReportGenerator;
use pdf_services::templates::erp_report_models::*;
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let kind = args.get(1).cloned().unwrap_or_else(|| "flat".to_string());
    let rows: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1000);

    let payload = match kind.as_str() {
        "flat" => build_flat(rows),
        "grouped" => build_grouped(rows),
        "hierarchical" => build_hierarchical(rows),
        _ => {
            eprintln!("Unknown kind: {}. Use flat | grouped | hierarchical", kind);
            std::process::exit(1);
        }
    };

    let out = format!("/tmp/test_streaming_{}_{}.pdf", kind, rows);
    let font_dir = std::env::var("PDF_FONTS_DIR").unwrap_or_else(|_| {
        // default to the bundled fonts in this crate
        let mut p = std::env::current_dir().unwrap_or_default();
        p.push("fonts");
        p.to_string_lossy().to_string()
    });

    println!("Kind: {} | Rows: {} | Font dir: {}", kind, rows, font_dir);

    let t0 = Instant::now();
    let generator = StreamingReportGenerator::new(&font_dir);
    let bytes = generator.generate_pdf(&payload)?;
    let elapsed = t0.elapsed();

    std::fs::write(&out, &bytes)?;

    println!();
    println!("✅ {}", out);
    println!("   Tiempo:   {:.3}s", elapsed.as_secs_f64());
    println!(
        "   Bytes:    {} ({:.1} KB)",
        bytes.len(),
        bytes.len() as f64 / 1024.0
    );
    println!("   Filas/s:  {:.0}", rows as f64 / elapsed.as_secs_f64());
    Ok(())
}

fn build_columns() -> Vec<ColumnDefinition> {
    vec![
        col("fecha", "FECHA", ColumnType::Date, 60, ColumnAlign::Center),
        col("encf", "ENCF", ColumnType::String, 90, ColumnAlign::Left),
        col(
            "rnc",
            "RNC CLIENTE",
            ColumnType::String,
            75,
            ColumnAlign::Left,
        ),
        col(
            "nombre",
            "NOMBRE CLIENTE",
            ColumnType::String,
            175,
            ColumnAlign::Left,
        ),
        col(
            "tipoVenta",
            "TIPO VENTA",
            ColumnType::String,
            60,
            ColumnAlign::Center,
        ),
        col(
            "subtotal",
            "SUBTOTAL",
            ColumnType::Decimal,
            75,
            ColumnAlign::Right,
        ),
        col(
            "itbis",
            "ITBIS",
            ColumnType::Decimal,
            70,
            ColumnAlign::Right,
        ),
        col(
            "total",
            "TOTAL",
            ColumnType::Decimal,
            85,
            ColumnAlign::Right,
        ),
    ]
}

fn col(key: &str, label: &str, t: ColumnType, w: i32, a: ColumnAlign) -> ColumnDefinition {
    ColumnDefinition {
        key: key.into(),
        label: label.into(),
        column_type: t,
        format: None,
        width: Some(w),
        align: Some(a),
        sortable: false,
        highlight: None,
        hidden: false,
        hide_in_print: false,
    }
}

fn synth_row(i: usize) -> HashMap<String, serde_json::Value> {
    let names = [
        "NELMIX SOLUTIONS SRL",
        "PEBERCA SRL",
        "DECM ARQUITECTOS ASOCIADOS SRL",
        "TAPSIA BUSINESS SOLUTIONS SRL",
        "PLAZA VIRGEN SRL",
        "ROALPE SRL",
        "GT SERVICE SRL",
        "FRANK GRUA SRL",
        "DISTRIBUIDORA COMERCIAL FT SRL",
        "CARGAS NACIONALES DEYWAND SRL",
    ];
    let subtotal = 200.0 + ((i as f64 * 7.31) % 3000.0);
    let itbis = subtotal * 0.18;
    let total = subtotal + itbis;
    let mut h = HashMap::new();
    h.insert(
        "fecha".into(),
        json!(format!("{:02}/04/2026", (i % 17) + 1)),
    );
    h.insert("encf".into(), json!(format!("E31{:010}", 6000 + i)));
    h.insert(
        "rnc".into(),
        json!(format!("{:09}", 100000000 + (i * 13) % 900000000)),
    );
    h.insert("nombre".into(), json!(names[i % names.len()]));
    h.insert("tipoVenta".into(), json!("CONTADO"));
    h.insert("subtotal".into(), json!(subtotal));
    h.insert("itbis".into(), json!(itbis));
    h.insert("total".into(), json!(total));
    h
}

fn base_payload() -> ErpReportPayload {
    ErpReportPayload {
        correlation_id: Some("test-001".into()),
        document_type: "REPORT".into(),
        tenant_id: 6,
        user_id: 60,
        report: ReportInfo {
            code: "TEST_REPORT".into(),
            variant: Some("DETAILED".into()),
            title: "Facturas Emitidas por NCF - Detallado".into(),
            subtitle: None,
            breadcrumb: Some("Ventas / Reportes / Por NCF / Detallado".into()),
            generated_at: chrono::Local::now().format("%d/%m/%Y %I:%M %p").to_string(),
            user_name: Some("Elián Martínez".into()),
            date_range: Some(DateRange {
                from: "01/04/2026".into(),
                to: "17/04/2026".into(),
            }),
            as_of_date: None,
            billing_mode: None,
        },
        metadata: ReportMetadata {
            columns: build_columns(),
            grouping: None,
            show_grand_total: true,
            grand_total_fields: vec!["subtotal".into(), "itbis".into(), "total".into()],
            has_running_balance: false,
            show_opening_balance: false,
            aging_buckets: None,
        },
        data: ReportDataSet {
            structure_type: DataStructureType::Flat,
            rows: None,
            groups: None,
            opening_balance: None,
            grand_total: None,
            summary: None,
            total_records: 0,
            totals: None,
        },
        output: OutputOptions {
            format: OutputFormat::Pdf,
            page_size: Some(PageSize::Letter),
            orientation: Some(PageOrientation::Landscape),
            scale: 100,
            margins: Some(PageMargins {
                top: 8.0,
                bottom: 8.0,
                left: 8.0,
                right: 8.0,
            }),
            include_header: true,
            include_footer: true,
            show_logo: false,
            file_name: None,
        },
        delivery: None,
        company_info: Some(ReportCompanyInfo {
            name: "LE CROISSANT DORE".into(),
            display_name: Some("LE CROISSANT DORE".into()),
            tax_id: Some("131800000".into()),
            logo_url: None,
            address: Some("ENRIQUILLO, NO. 25".into()),
            phone: None,
            email: None,
            website: None,
        }),
    }
}

fn build_flat(rows: usize) -> ErpReportPayload {
    let mut p = base_payload();
    let data: Vec<HashMap<String, serde_json::Value>> = (1..=rows).map(synth_row).collect();
    let mut sum_subtotal = 0.0;
    let mut sum_itbis = 0.0;
    let mut sum_total = 0.0;
    for r in &data {
        sum_subtotal += r["subtotal"].as_f64().unwrap_or(0.0);
        sum_itbis += r["itbis"].as_f64().unwrap_or(0.0);
        sum_total += r["total"].as_f64().unwrap_or(0.0);
    }
    let mut grand_total = HashMap::new();
    grand_total.insert("subtotal".into(), json!(sum_subtotal));
    grand_total.insert("itbis".into(), json!(sum_itbis));
    grand_total.insert("total".into(), json!(sum_total));

    p.data.structure_type = DataStructureType::Flat;
    p.data.total_records = rows as i32;
    p.data.rows = Some(data);
    p.data.grand_total = Some(grand_total);
    p
}

fn build_grouped(rows: usize) -> ErpReportPayload {
    let mut p = base_payload();
    let branches = ["SIGMA HATILLO", "SIGMA MADRE VIEJA", "SHELL CAMBITA"];
    let per_group = (rows / branches.len()).max(1);

    let mut groups: Vec<ReportGroup> = Vec::new();
    let mut g_sub = 0.0;
    let mut g_itb = 0.0;
    let mut g_tot = 0.0;
    for (b_idx, b) in branches.iter().enumerate() {
        let mut leaf_rows: Vec<HashMap<String, serde_json::Value>> = Vec::with_capacity(per_group);
        let mut s = 0.0;
        let mut it = 0.0;
        let mut t = 0.0;
        for j in 0..per_group {
            let i = b_idx * per_group + j + 1;
            let mut r = synth_row(i);
            r.insert("nombre".into(), json!(format!("{} - Cliente {}", b, j + 1)));
            s += r["subtotal"].as_f64().unwrap();
            it += r["itbis"].as_f64().unwrap();
            t += r["total"].as_f64().unwrap();
            leaf_rows.push(r);
        }
        let mut subtotal = HashMap::new();
        subtotal.insert("subtotal".into(), json!(s));
        subtotal.insert("itbis".into(), json!(it));
        subtotal.insert("total".into(), json!(t));
        g_sub += s;
        g_itb += it;
        g_tot += t;
        groups.push(ReportGroup {
            level: 0,
            key: b.to_string(),
            label: b.to_string(),
            record_count: per_group as i32,
            subtotal: Some(subtotal),
            rows: leaf_rows,
            sub_groups: None,
            default_expanded: true,
        });
    }
    let mut gt = HashMap::new();
    gt.insert("subtotal".into(), json!(g_sub));
    gt.insert("itbis".into(), json!(g_itb));
    gt.insert("total".into(), json!(g_tot));

    p.metadata.grouping = Some(GroupingConfig {
        enabled: true,
        field: Some("branch".into()),
        label_field: Some("branch".into()),
        label_prefix: Some("Sucursal: ".into()),
        show_subtotals: true,
        subtotal_label: Some("Subtotal".into()),
        subtotal_fields: vec!["subtotal".into(), "itbis".into(), "total".into()],
        levels: None,
    });
    p.data.structure_type = DataStructureType::Grouped;
    p.data.total_records = (per_group * branches.len()) as i32;
    p.data.groups = Some(groups);
    p.data.grand_total = Some(gt);
    p
}

fn build_hierarchical(rows: usize) -> ErpReportPayload {
    let mut p = base_payload();
    let parents = ["VENTAS", "DEVOLUCIONES"];
    let branches = ["SIGMA HATILLO", "SIGMA MADRE VIEJA", "SHELL CAMBITA"];
    let per_leaf = (rows / (parents.len() * branches.len())).max(1);

    let mut top_groups: Vec<ReportGroup> = Vec::new();
    let mut g_sub = 0.0;
    let mut g_itb = 0.0;
    let mut g_tot = 0.0;

    for (p_idx, parent) in parents.iter().enumerate() {
        let mut sub_groups: Vec<ReportGroup> = Vec::new();
        let mut p_sub = 0.0;
        let mut p_itb = 0.0;
        let mut p_tot = 0.0;

        for (b_idx, b) in branches.iter().enumerate() {
            let mut leaf_rows: Vec<HashMap<String, serde_json::Value>> =
                Vec::with_capacity(per_leaf);
            let mut s = 0.0;
            let mut it = 0.0;
            let mut t = 0.0;
            for j in 0..per_leaf {
                let i = (p_idx * branches.len() * per_leaf) + (b_idx * per_leaf) + j + 1;
                let mut r = synth_row(i);
                r.insert("nombre".into(), json!(format!("{} - Cliente {}", b, j + 1)));
                s += r["subtotal"].as_f64().unwrap();
                it += r["itbis"].as_f64().unwrap();
                t += r["total"].as_f64().unwrap();
                leaf_rows.push(r);
            }
            let mut subtotal = HashMap::new();
            subtotal.insert("subtotal".into(), json!(s));
            subtotal.insert("itbis".into(), json!(it));
            subtotal.insert("total".into(), json!(t));
            p_sub += s;
            p_itb += it;
            p_tot += t;
            sub_groups.push(ReportGroup {
                level: 1,
                key: b.to_string(),
                label: b.to_string(),
                record_count: per_leaf as i32,
                subtotal: Some(subtotal),
                rows: leaf_rows,
                sub_groups: None,
                default_expanded: true,
            });
        }
        let mut psubtotal = HashMap::new();
        psubtotal.insert("subtotal".into(), json!(p_sub));
        psubtotal.insert("itbis".into(), json!(p_itb));
        psubtotal.insert("total".into(), json!(p_tot));
        g_sub += p_sub;
        g_itb += p_itb;
        g_tot += p_tot;

        top_groups.push(ReportGroup {
            level: 0,
            key: parent.to_string(),
            label: parent.to_string(),
            record_count: 0,
            subtotal: Some(psubtotal),
            rows: Vec::new(),
            sub_groups: Some(sub_groups),
            default_expanded: true,
        });
    }
    let mut gt = HashMap::new();
    gt.insert("subtotal".into(), json!(g_sub));
    gt.insert("itbis".into(), json!(g_itb));
    gt.insert("total".into(), json!(g_tot));

    p.metadata.grouping = Some(GroupingConfig {
        enabled: true,
        field: None,
        label_field: None,
        label_prefix: None,
        show_subtotals: true,
        subtotal_label: Some("Subtotal".into()),
        subtotal_fields: vec!["subtotal".into(), "itbis".into(), "total".into()],
        levels: None,
    });
    p.data.structure_type = DataStructureType::HierarchicalGrouped;
    p.data.total_records = (per_leaf * branches.len() * parents.len()) as i32;
    p.data.groups = Some(top_groups);
    p.data.grand_total = Some(gt);
    p
}
