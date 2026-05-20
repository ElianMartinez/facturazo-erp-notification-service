use super::state::ApiState;
use crate::api::middleware::auth::extract_tenant_user_or_default;
use crate::templates::{InvoiceData, TemplateData, TemplateEngine};
use actix_web::{web, HttpRequest, HttpResponse, Result};
use serde_json::json;
use uuid::Uuid;

pub async fn generate_pdf_from_template(
    req: HttpRequest,
    data: web::Json<serde_json::Value>,
    state: web::Data<ApiState>,
) -> Result<HttpResponse> {
    let (tenant_id, _user_id) = extract_tenant_user_or_default(&req);

    // Support both camelCase and snake_case for template_id
    let template_id = data
        .get("template_id")
        .or_else(|| data.get("templateId"))
        .and_then(|v| v.as_str())
        .unwrap_or("fiscal_invoice");

    // Support both camelCase and snake_case for template_type
    let template_type = data
        .get("template_type")
        .or_else(|| data.get("templateType"))
        .and_then(|v| v.as_str());

    // Check if user wants binary response (default: true - returns PDF directly)
    let return_binary = data
        .get("return_binary")
        .or_else(|| data.get("returnBinary"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Check if user wants to upload to S3 (default: false)
    let upload_to_s3 = data
        .get("upload_to_s3")
        .or_else(|| data.get("uploadToS3"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let template_data = match template_type {
        Some("invoice") | Some("fiscal_invoice") => {
            // Get data from either "data" or directly from root
            let invoice_json = data.get("data").cloned().unwrap_or_else(|| {
                // If no "data" field, try to use the root object (minus metadata fields)
                let mut root = (*data).clone();
                if let Some(obj) = root.as_object_mut() {
                    obj.remove("template_id");
                    obj.remove("templateId");
                    obj.remove("template_type");
                    obj.remove("templateType");
                    obj.remove("output_filename");
                    obj.remove("outputFilename");
                }
                root
            });

            let invoice_data: InvoiceData = serde_json::from_value(invoice_json).map_err(|e| {
                actix_web::error::ErrorBadRequest(format!(
                    "Invalid invoice data: {}. Ensure required fields are present: invoice_number/invoiceNumber, issue_date/issueDate, due_date/dueDate, company_info/companyInfo, client_info/clientInfo, items",
                    e
                ))
            })?;
            TemplateData::Invoice(invoice_data)
        }
        Some("report") => {
            let report_data =
                serde_json::from_value(data.get("data").cloned().unwrap_or(json!({}))).map_err(
                    |e| actix_web::error::ErrorBadRequest(format!("Invalid report data: {}", e)),
                )?;
            TemplateData::Report(report_data)
        }
        Some("receipt") => {
            let receipt_data =
                serde_json::from_value(data.get("data").cloned().unwrap_or(json!({}))).map_err(
                    |e| actix_web::error::ErrorBadRequest(format!("Invalid receipt data: {}", e)),
                )?;
            TemplateData::Receipt(receipt_data)
        }
        Some("cash_register") | Some("cuadre") => {
            // Get data from either "data" or directly from root
            let cash_register_json = data.get("data").cloned().unwrap_or_else(|| {
                let mut root = (*data).clone();
                if let Some(obj) = root.as_object_mut() {
                    obj.remove("template_id");
                    obj.remove("templateId");
                    obj.remove("template_type");
                    obj.remove("templateType");
                    obj.remove("output_filename");
                    obj.remove("outputFilename");
                    obj.remove("return_binary");
                    obj.remove("returnBinary");
                    obj.remove("upload_to_s3");
                    obj.remove("uploadToS3");
                }
                root
            });

            let cash_register_data: crate::templates::CashRegisterData =
                serde_json::from_value(cash_register_json).map_err(|e| {
                    actix_web::error::ErrorBadRequest(format!(
                        "Invalid cash register data: {}. Ensure required fields: documentCode, date, companyInfo, cashier, transactionsCount, status, income, expenses, summary, paymentMethods, finalBalance",
                        e
                    ))
                })?;
            TemplateData::CashRegister(cash_register_data)
        }
        _ => {
            let custom_data = data
                .get("data")
                .and_then(|v| v.as_object())
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();
            TemplateData::Custom(custom_data)
        }
    };

    let engine = TemplateEngine::new("templates".to_string(), "facturas".to_string());

    // Support both camelCase and snake_case for output_filename
    let output_filename = data
        .get("output_filename")
        .or_else(|| data.get("outputFilename"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    match engine
        .generate_pdf(template_id, template_data, output_filename)
        .await
    {
        Ok(pdf_path) => {
            let document_id = Uuid::new_v4();

            // Read PDF bytes
            let pdf_bytes = tokio::fs::read(&pdf_path).await.map_err(|e| {
                actix_web::error::ErrorInternalServerError(format!("Failed to read PDF: {}", e))
            })?;

            // If return_binary is true, return the PDF directly
            if return_binary {
                let filename = pdf_path.split('/').last().unwrap_or("document.pdf");
                let _ = tokio::fs::remove_file(&pdf_path).await;

                return Ok(HttpResponse::Ok()
                    .content_type("application/pdf")
                    .insert_header((
                        "Content-Disposition",
                        format!("attachment; filename=\"{}\"", filename),
                    ))
                    .insert_header(("X-Document-Id", document_id.to_string()))
                    .body(pdf_bytes));
            }

            // If upload_to_s3 is true, upload and return URL
            if upload_to_s3 {
                let org_id = format!("tenant_{}", tenant_id);
                let key = format!("documents/{}/{}.pdf", org_id, document_id);

                let url = state
                    .s3_client
                    .put_object(
                        &state.config.s3_bucket_documents,
                        &key,
                        pdf_bytes,
                        "application/pdf",
                    )
                    .await
                    .map_err(|e| {
                        actix_web::error::ErrorInternalServerError(format!(
                            "Failed to upload to S3: {}",
                            e
                        ))
                    })?;

                let _ = tokio::fs::remove_file(&pdf_path).await;

                return Ok(HttpResponse::Ok().json(json!({
                    "status": "success",
                    "document_id": document_id,
                    "url": url,
                    "storage": "s3"
                })));
            }

            // Default: return JSON with local path info (don't delete the file)
            Ok(HttpResponse::Ok().json(json!({
                "status": "success",
                "document_id": document_id,
                "local_path": pdf_path,
                "storage": "local",
                "size_bytes": pdf_bytes.len()
            })))
        }
        Err(e) => {
            tracing::error!("Failed to generate PDF from template: {:?}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "error": "Failed to generate PDF",
                "details": e.to_string()
            })))
        }
    }
}

pub async fn list_templates(
    _req: HttpRequest,
    _state: web::Data<ApiState>,
) -> Result<HttpResponse> {
    use std::fs;
    use std::path::Path;

    let templates_dir = Path::new("templates");
    let mut templates = vec![];

    if let Ok(categories) = fs::read_dir(templates_dir) {
        for category in categories.filter_map(Result::ok) {
            let category_name = category.file_name().to_string_lossy().to_string();

            if let Ok(files) = fs::read_dir(category.path()) {
                for file in files.filter_map(Result::ok) {
                    let file_name = file.file_name().to_string_lossy().to_string();
                    if file_name.ends_with(".typ") {
                        let template_id = file_name.trim_end_matches(".typ");
                        templates.push(json!({
                            "id": template_id,
                            "category": category_name,
                            "path": format!("{}/{}", category_name, template_id)
                        }));
                    }
                }
            }
        }
    }

    templates.push(json!({
        "id": "fiscal_electronic",
        "category": "invoice",
        "path": "invoice/fiscal_electronic",
        "description": "Factura fiscal electrónica dominicana"
    }));

    Ok(HttpResponse::Ok().json(json!({
        "templates": templates
    })))
}

pub async fn preview_template(
    _req: HttpRequest,
    path: web::Path<String>,
    _state: web::Data<ApiState>,
) -> Result<HttpResponse> {
    let template_id = path.into_inner();

    let sample_data = get_sample_data_for_template(&template_id);

    let engine = TemplateEngine::new("templates".to_string(), "temp".to_string());

    match engine
        .generate_pdf(
            &template_id,
            sample_data,
            Some(format!("preview_{}", template_id)),
        )
        .await
    {
        Ok(pdf_path) => {
            let pdf_bytes = tokio::fs::read(&pdf_path).await.map_err(|e| {
                actix_web::error::ErrorInternalServerError(format!("Failed to read PDF: {}", e))
            })?;

            let _ = tokio::fs::remove_file(&pdf_path).await;

            Ok(HttpResponse::Ok()
                .content_type("application/pdf")
                .body(pdf_bytes))
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(json!({
            "error": "Failed to generate preview",
            "details": e.to_string()
        }))),
    }
}

fn get_sample_data_for_template(template_id: &str) -> TemplateData {
    use crate::templates::*;

    match template_id {
        "fiscal_electronic" | "fiscal_invoice" => {
            TemplateData::Invoice(InvoiceData {
                invoice_number: "INV-2024-001".to_string(),
                issue_date: "2024-01-15".to_string(),
                due_date: "2024-02-15".to_string(),
                company_info: CompanyInfo {
                    name: "COMERCIAL ZYL".to_string(),
                    legal_name: Some("ZYL, SRL".to_string()),
                    tax_id: "101000001".to_string(),
                    address: Address {
                        street: "Calle Segunda #01, Gascue".to_string(),
                        line2: None,
                        city: "Santo Domingo".to_string(),
                        state: Some("Distrito Nacional".to_string()),
                        postal_code: Some("10210".to_string()),
                        country: "República Dominicana".to_string(),
                    },
                    phone: Some("809-555-0100".to_string()),
                    email: Some("ventas@zyl.com.do".to_string()),
                    website: Some("www.zyl.com.do".to_string()),
                    logo_path: None,
                },
                client_info: ClientInfo {
                    name: "COMERCIO, SRL".to_string(),
                    legal_name: Some("COMERCIO, SRL".to_string()),
                    tax_id: Some("130000001".to_string()),
                    address: None,
                    phone: Some("809-555-0200".to_string()),
                    email: Some("compras@comercio.com.do".to_string()),
                },
                items: vec![
                    InvoiceItem {
                        code: Some("ZAP-001".to_string()),
                        quantity: 150.0,
                        description: "Zapatos".to_string(),
                        unit_price: 550.00,
                        unit: Some("CAJ".to_string()),
                        tax_rate: Some(0.18),
                        tax_amount: Some(14880.00),
                        discount: None,
                        subtotal: Some(82500.00),
                        total: Some(97380.00),
                        document_date: None,
                        source_document_number: None,
                        notes: None,
                    },
                    InvoiceItem {
                        code: Some("VES-002".to_string()),
                        quantity: 200.0,
                        description: "Vestidos".to_string(),
                        unit_price: 800.00,
                        unit: Some("PZA".to_string()),
                        tax_rate: Some(0.18),
                        tax_amount: Some(28800.00),
                        discount: None,
                        subtotal: Some(160000.00),
                        total: Some(188800.00),
                        document_date: None,
                        source_document_number: None,
                        notes: None,
                    },
                ],
                totals: Some(InvoiceTotals {
                    subtotal: 242500.00,
                    tax_amount: 43650.00,
                    discount_amount: None,
                    total: 286150.00,
                    currency: "RD$".to_string(),
                    tip: None,
                }),
                fiscal_info: Some(FiscalInfo {
                    e_ncf: "E310000000001".to_string(),
                    security_code: "S7DQdu".to_string(),
                    signature_date: "2024-01-15 10:30:00".to_string(),
                    qr_data: "https://fc.dgii.gov.do/eCF/consultatimbrefc?rncemisor=101000001&encf=E310000000001".to_string(),
                    expiration_date: Some("2025-12-31".to_string()),
                }),
                payment_info: Some(PaymentInfo {
                    method: "Crédito".to_string(),
                    terms: Some("30 días".to_string()),
                    bank_info: None,
                    paid: false,
                    paid_date: None,
                    due_date: None,
                }),
                notes: Some("Gracias por su compra.".to_string()),
                currency: "DOP".to_string(),
                custom_fields: None,
            })
        }
        _ => TemplateData::Custom(std::collections::HashMap::new()),
    }
}

// NOTE: extract_tenant_user function moved to middleware/auth.rs for centralization
