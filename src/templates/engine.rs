use minijinja::{Environment, Value, Error as JinjaError};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use chrono::{DateTime, NaiveDate, Utc};
use anyhow::Result;

use crate::models::{InvoiceData, ReportRequest, RenderOptions, NumberFormat};
use super::helpers;

pub struct TemplateEngine {
    env: Arc<RwLock<Environment<'static>>>,
    cache_dir: String,
}

impl TemplateEngine {
    pub fn new(cache_dir: String) -> Result<Self> {
        let mut env = Environment::new();

        // Registrar funciones helper globales
        env.add_function("format_money", helpers::format_money);
        env.add_function("format_date", helpers::format_date);
        env.add_function("format_number", helpers::format_number);
        env.add_function("calculate_total", helpers::calculate_total);
        env.add_function("qr_code", helpers::generate_qr_base64);

        // Registrar filtros
        env.add_filter("money", helpers::money_filter);
        env.add_filter("date", helpers::date_filter);
        env.add_filter("percentage", helpers::percentage_filter);
        env.add_filter("capitalize", helpers::capitalize_filter);
        env.add_filter("escape_typst", helpers::escape_typst_filter);

        Ok(TemplateEngine {
            env: Arc::new(RwLock::new(env)),
            cache_dir,
        })
    }

    pub async fn load_template(&self, template_id: &str, content: &str) -> Result<()> {
        let mut env = self.env.write().await;
        env.add_template(template_id, content)?;
        Ok(())
    }

    pub async fn render_invoice(
        &self,
        template_id: &str,
        data: &InvoiceData,
        options: &RenderOptions,
    ) -> Result<String> {
        let env = self.env.read().await;
        let template = env.get_template(template_id)?;

        // Preparar contexto con datos procesados
        let mut context = HashMap::new();

        // Datos básicos
        context.insert("company", serde_json::to_value(&data.company)?);
        context.insert("customer", serde_json::to_value(&data.customer)?);
        context.insert("invoice", serde_json::to_value(&data.invoice)?);

        // Calcular totales si no están proporcionados
        let totals = data.totals.clone()
            .unwrap_or_else(|| data.calculate_totals());
        context.insert("totals", serde_json::to_value(&totals)?);

        // Procesar items con cálculos
        let processed_items = self.process_invoice_items(&data.items);
        context.insert("items", serde_json::to_value(&processed_items)?);

        // Opciones de renderizado
        context.insert("options", serde_json::to_value(&options)?);

        // Metadata adicional
        context.insert("generated_at", serde_json::to_value(&Utc::now())?);
        context.insert("template_id", serde_json::to_value(&template_id)?);

        // Renderizar template
        let rendered = template.render(&context)?;
        Ok(rendered)
    }

    pub async fn render_report(
        &self,
        template_id: &str,
        request: &ReportRequest,
        data: Vec<serde_json::Value>,
    ) -> Result<String> {
        let env = self.env.read().await;
        let template = env.get_template(template_id)?;

        let mut context = HashMap::new();

        // Metadata del reporte
        context.insert("title", serde_json::to_value(&request.title)?);
        context.insert("schema", serde_json::to_value(&request.schema)?);

        // Procesar datos según schema
        let processed_data = self.process_report_data(data, &request.schema);
        context.insert("data", serde_json::to_value(&processed_data)?);

        // Agregaciones si existen
        if let Some(aggregations) = &request.schema.aggregations {
            let agg_results = self.calculate_aggregations(&processed_data, aggregations);
            context.insert("aggregations", serde_json::to_value(&agg_results)?);
        }

        // Opciones
        if let Some(options) = &request.options {
            context.insert("options", serde_json::to_value(&options)?);
        }

        let rendered = template.render(&context)?;
        Ok(rendered)
    }

    fn process_invoice_items(&self, items: &[crate::models::InvoiceItem]) -> Vec<HashMap<String, Value>> {
        items.iter().map(|item| {
            let mut processed = HashMap::new();

            // Datos originales
            processed.insert("code".to_string(),
                Value::from(item.code.clone().unwrap_or_default()));
            processed.insert("description".to_string(),
                Value::from(item.description.clone()));
            processed.insert("quantity".to_string(),
                Value::from(item.quantity));
            processed.insert("unit_price".to_string(),
                Value::from(item.unit_price));

            // Cálculos
            let subtotal = item.quantity * item.unit_price;
            processed.insert("subtotal".to_string(), Value::from(subtotal));

            let discount = if let Some(percent) = item.discount_percent {
                subtotal * (percent / 100.0)
            } else {
                item.discount_amount.unwrap_or(0.0)
            };
            processed.insert("discount".to_string(), Value::from(discount));

            let taxable = subtotal - discount;
            let tax = if let Some(rate) = item.tax_rate {
                taxable * (rate / 100.0)
            } else {
                item.tax_amount.unwrap_or(0.0)
            };
            processed.insert("tax".to_string(), Value::from(tax));

            let total = taxable + tax;
            processed.insert("total".to_string(), Value::from(total));

            processed
        }).collect()
    }

    fn process_report_data(
        &self,
        data: Vec<serde_json::Value>,
        schema: &crate::models::ReportSchema,
    ) -> Vec<HashMap<String, Value>> {
        use rayon::prelude::*;

        // Procesar en paralelo para mejor rendimiento
        data.par_iter()
            .map(|row| {
                let mut processed = HashMap::new();

                for column in &schema.columns {
                    if !column.visible {
                        continue;
                    }

                    let value = row.get(&column.field)
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);

                    // Aplicar formato según tipo de dato
                    let formatted = self.format_cell_value(
                        value,
                        &column.data_type,
                        column.format.as_deref()
                    );

                    processed.insert(column.field.clone(), formatted);
                }

                processed
            })
            .collect()
    }

    fn format_cell_value(
        &self,
        value: serde_json::Value,
        data_type: &crate::models::DataType,
        format: Option<&str>,
    ) -> Value {
        use crate::models::DataType;

        match data_type {
            DataType::Currency => {
                if let Some(num) = value.as_f64() {
                    Value::from(helpers::format_currency(num, "USD", "$"))
                } else {
                    Value::from("")
                }
            },
            DataType::Percentage => {
                if let Some(num) = value.as_f64() {
                    Value::from(format!("{:.2}%", num * 100.0))
                } else {
                    Value::from("")
                }
            },
            DataType::Date | DataType::DateTime => {
                if let Some(date_str) = value.as_str() {
                    Value::from(helpers::format_date_string(date_str, format.unwrap_or("%d/%m/%Y")))
                } else {
                    Value::from("")
                }
            },
            _ => Value::from(value),
        }
    }

    fn calculate_aggregations(
        &self,
        data: &[HashMap<String, Value>],
        aggregations: &[crate::models::Aggregation],
    ) -> HashMap<String, Value> {
        use crate::models::AggregateOperation;

        let mut results = HashMap::new();

        for agg in aggregations {
            let values: Vec<f64> = data.iter()
                .filter_map(|row| {
                    row.get(&agg.field)
                        .and_then(|v| v.as_f64())
                })
                .collect();

            let result = match agg.operation {
                AggregateOperation::Sum => values.iter().sum::<f64>(),
                AggregateOperation::Average => {
                    if !values.is_empty() {
                        values.iter().sum::<f64>() / values.len() as f64
                    } else {
                        0.0
                    }
                },
                AggregateOperation::Count => values.len() as f64,
                AggregateOperation::Min => values.iter().cloned().fold(f64::INFINITY, f64::min),
                AggregateOperation::Max => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                AggregateOperation::Distinct => {
                    let unique: std::collections::HashSet<_> = values.iter().collect();
                    unique.len() as f64
                },
            };

            let key = agg.alias.clone()
                .unwrap_or_else(|| format!("{}_{}", agg.field, format!("{:?}", agg.operation).to_lowercase()));
            results.insert(key, Value::from(result));
        }

        results
    }
}