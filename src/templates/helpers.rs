use minijinja::Value;
use chrono::{NaiveDate, DateTime, Utc};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use qrcode::QrCode;
use image::Luma;

/// Formatea un número como moneda
pub fn format_money(value: Value, currency: Option<Value>, symbol: Option<Value>) -> Result<Value, minijinja::Error> {
    let amount = value.as_f64()
        .ok_or_else(|| minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            "Value must be a number"
        ))?;

    let symbol = symbol
        .and_then(|s| s.as_str())
        .unwrap_or("$");

    Ok(Value::from(format_currency(amount, "", symbol)))
}

/// Formatea una fecha
pub fn format_date(value: Value, format: Option<Value>) -> Result<Value, minijinja::Error> {
    let format_str = format
        .and_then(|f| f.as_str())
        .unwrap_or("%d/%m/%Y");

    if let Some(date_str) = value.as_str() {
        Ok(Value::from(format_date_string(date_str, format_str)))
    } else {
        Ok(Value::from(""))
    }
}

/// Formatea un número con separadores de miles
pub fn format_number(value: Value, decimals: Option<Value>) -> Result<Value, minijinja::Error> {
    let num = value.as_f64()
        .ok_or_else(|| minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            "Value must be a number"
        ))?;

    let decimals = decimals
        .and_then(|d| d.as_u64())
        .unwrap_or(2) as usize;

    Ok(Value::from(format_number_with_separators(num, decimals)))
}

/// Calcula el total de una lista de valores
pub fn calculate_total(items: Value, field: Value) -> Result<Value, minijinja::Error> {
    let field_name = field.as_str()
        .ok_or_else(|| minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            "Field name must be a string"
        ))?;

    if let Some(items_list) = items.as_seq() {
        let total: f64 = items_list.iter()
            .filter_map(|item| {
                item.get_attr(field_name)
                    .ok()
                    .and_then(|v| v.as_f64())
            })
            .sum();

        Ok(Value::from(total))
    } else {
        Ok(Value::from(0.0))
    }
}

/// Genera un código QR en base64
pub fn generate_qr_base64(data: Value) -> Result<Value, minijinja::Error> {
    let text = data.as_str()
        .ok_or_else(|| minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            "QR data must be a string"
        ))?;

    let code = QrCode::new(text)
        .map_err(|e| minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("Failed to generate QR code: {}", e)
        ))?;

    let image = code.render::<Luma<u8>>()
        .max_dimensions(200, 200)
        .build();

    let mut buffer = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageOutputFormat::Png)
        .map_err(|e| minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("Failed to encode QR image: {}", e)
        ))?;

    let base64_string = BASE64.encode(&buffer);
    Ok(Value::from(format!("data:image/png;base64,{}", base64_string)))
}

// Filtros

pub fn money_filter(value: Value) -> Result<Value, minijinja::Error> {
    format_money(value, None, None)
}

pub fn date_filter(value: Value) -> Result<Value, minijinja::Error> {
    format_date(value, None)
}

pub fn percentage_filter(value: Value) -> Result<Value, minijinja::Error> {
    let num = value.as_f64()
        .ok_or_else(|| minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            "Value must be a number"
        ))?;

    Ok(Value::from(format!("{:.2}%", num * 100.0)))
}

pub fn capitalize_filter(value: Value) -> Result<Value, minijinja::Error> {
    if let Some(s) = value.as_str() {
        let capitalized: String = s.chars()
            .take(1)
            .flat_map(char::to_uppercase)
            .chain(s.chars().skip(1).flat_map(char::to_lowercase))
            .collect();
        Ok(Value::from(capitalized))
    } else {
        Ok(value)
    }
}

pub fn escape_typst_filter(value: Value) -> Result<Value, minijinja::Error> {
    if let Some(s) = value.as_str() {
        let escaped = s
            .replace("$", "\\$")
            .replace("#", "\\#")
            .replace("_", "\\_")
            .replace("*", "\\*");
        Ok(Value::from(escaped))
    } else {
        Ok(value)
    }
}

// Funciones auxiliares

pub fn format_currency(amount: f64, _currency: &str, symbol: &str) -> String {
    let formatted = format_number_with_separators(amount.abs(), 2);
    if amount < 0.0 {
        format!("-{}{}", symbol, formatted)
    } else {
        format!("{}{}", symbol, formatted)
    }
}

pub fn format_number_with_separators(num: f64, decimals: usize) -> String {
    let formatted = format!("{:.decimals$}", num, decimals = decimals);
    let parts: Vec<&str> = formatted.split('.').collect();
    let integer = parts[0];
    let decimal = parts.get(1).unwrap_or(&"00");

    let mut result = String::new();
    let chars: Vec<char> = integer.chars().collect();
    let mut count = 0;

    for c in chars.iter().rev() {
        if count == 3 {
            result.push(',');
            count = 0;
        }
        result.push(*c);
        count += 1;
    }

    let integer_formatted: String = result.chars().rev().collect();

    if decimals > 0 {
        format!("{}.{}", integer_formatted, decimal)
    } else {
        integer_formatted
    }
}

pub fn format_date_string(date_str: &str, format: &str) -> String {
    // Intentar parsear diferentes formatos de fecha
    if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        return date.format(format).to_string();
    }

    if let Ok(datetime) = DateTime::parse_from_rfc3339(date_str) {
        return datetime.format(format).to_string();
    }

    // Si no se puede parsear, devolver como está
    date_str.to_string()
}