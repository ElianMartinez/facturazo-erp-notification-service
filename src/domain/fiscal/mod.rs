//! Dominican Republic fiscal domain module
//!
//! Contains all fiscal-related domain logic specific to Dominican Republic (DGII)

use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::fmt;
use regex::Regex;
use once_cell::sync::Lazy;

// Regex patterns for validation
static NCF_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[A-Z]\d{10}$").unwrap()
});

static RNC_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\d{9}$|^\d{11}$").unwrap()
});

static CEDULA_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\d{11}$").unwrap()
});

/// NCF (Número de Comprobante Fiscal) - Dominican Republic fiscal receipt number
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NCF {
    value: String,
    serie: char,
    tipo_code: u32,
    secuencia: u64,
}

impl NCF {
    /// Create a new NCF with tipo code and sequence
    /// Format: Serie(1 char) + TipoCode(2 digits) + Sequence(8 digits) = 11 chars
    /// Example: E31-00000001 (stored as E3100000001)
    pub fn new_with_tipo(serie: char, tipo_code: u32, secuencia: u64) -> Result<Self, FiscalError> {
        // Validate tipo code is 2 digits
        if tipo_code > 99 {
            return Err(FiscalError::InvalidNCF(format!("Invalid tipo code: {}", tipo_code)));
        }

        // Validate sequence is max 8 digits
        if secuencia > 99_999_999 {
            return Err(FiscalError::InvalidNCF(format!("Sequence too large: {}", secuencia)));
        }

        let value = format!("{}{:02}{:08}", serie, tipo_code, secuencia);

        if !NCF_REGEX.is_match(&value) {
            return Err(FiscalError::InvalidNCF(value));
        }

        Ok(Self {
            value,
            serie,
            tipo_code,
            secuencia,
        })
    }

    /// Create a new NCF for Factura de Crédito Fiscal (E31)
    pub fn new_credito_fiscal(secuencia: u64) -> Result<Self, FiscalError> {
        Self::new_with_tipo('E', 31, secuencia)
    }

    /// Create a new NCF for Factura de Consumidor Final (B01)
    pub fn new_consumidor_final(secuencia: u64) -> Result<Self, FiscalError> {
        Self::new_with_tipo('B', 01, secuencia)
    }

    /// Create a new NCF for Nota de Crédito (B04)
    pub fn new_nota_credito(secuencia: u64) -> Result<Self, FiscalError> {
        Self::new_with_tipo('B', 04, secuencia)
    }

    /// Create a new NCF for Nota de Débito (B03)
    pub fn new_nota_debito(secuencia: u64) -> Result<Self, FiscalError> {
        Self::new_with_tipo('B', 03, secuencia)
    }

    /// Parse NCF from string
    pub fn parse(ncf: &str) -> Result<Self, FiscalError> {
        if !NCF_REGEX.is_match(ncf) {
            return Err(FiscalError::InvalidNCF(ncf.to_string()));
        }

        let serie = ncf.chars().next().unwrap();
        let tipo_code = ncf[1..3].parse::<u32>()
            .map_err(|_| FiscalError::InvalidNCF(ncf.to_string()))?;
        let secuencia = ncf[3..].parse::<u64>()
            .map_err(|_| FiscalError::InvalidNCF(ncf.to_string()))?;

        Self::new_with_tipo(serie, tipo_code, secuencia)
    }

    /// Get NCF as string
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Get serie
    pub fn serie(&self) -> char {
        self.serie
    }

    /// Get tipo code
    pub fn tipo_code(&self) -> u32 {
        self.tipo_code
    }

    /// Get sequence
    pub fn sequence(&self) -> u64 {
        self.secuencia
    }

    /// Check if NCF is valid for current date
    pub fn is_valid_for_date(&self, date: DateTime<Utc>, expiry_date: DateTime<Utc>) -> bool {
        date <= expiry_date
    }
}

impl fmt::Display for NCF {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

/// Tipo de Comprobante Fiscal según DGII
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TipoComprobante {
    FacturaCreditoFiscal,           // E31 - Para contribuyentes con RNC
    FacturaConsumidorFinal,          // E32 - Para consumidores sin RNC
    NotaDebito,                      // E33
    NotaCredito,                     // E34
    ComprasBienes,                   // E41
    GastosGenerales,                 // E43
    RegimenesEspeciales,             // E44
    ComprobanteGubernamental,        // E45
    ComprobanteExportaciones,        // E46
    ComprobantePagosExterior,        // E47
}

impl TipoComprobante {
    /// Get serie letter for NCF
    pub fn serie(&self) -> char {
        match self {
            Self::FacturaCreditoFiscal => 'E',
            Self::FacturaConsumidorFinal => 'B',
            Self::NotaDebito => 'D',
            Self::NotaCredito => 'C',
            _ => 'E',
        }
    }

    /// Create from serie letter
    pub fn from_serie(serie: char) -> Result<Self, FiscalError> {
        match serie {
            'E' => Ok(Self::FacturaCreditoFiscal),
            'B' => Ok(Self::FacturaConsumidorFinal),
            'D' => Ok(Self::NotaDebito),
            'C' => Ok(Self::NotaCredito),
            _ => Err(FiscalError::InvalidTipoComprobante(serie.to_string())),
        }
    }

    /// Get description
    pub fn descripcion(&self) -> &str {
        match self {
            Self::FacturaCreditoFiscal => "Factura de Crédito Fiscal",
            Self::FacturaConsumidorFinal => "Factura de Consumidor Final",
            Self::NotaDebito => "Nota de Débito",
            Self::NotaCredito => "Nota de Crédito",
            Self::ComprasBienes => "Registro de Compras de Bienes",
            Self::GastosGenerales => "Registro de Gastos Generales",
            Self::RegimenesEspeciales => "Regímenes Especiales",
            Self::ComprobanteGubernamental => "Comprobante Gubernamental",
            Self::ComprobanteExportaciones => "Comprobante de Exportaciones",
            Self::ComprobantePagosExterior => "Comprobante de Pagos al Exterior",
        }
    }
}

/// RNC (Registro Nacional del Contribuyente) - Tax ID for companies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RNC {
    value: String,
    tipo: TipoContribuyente,
}

impl RNC {
    /// Create new RNC
    pub fn new(value: String) -> Result<Self, FiscalError> {
        let clean_value = value.chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>();

        if !RNC_REGEX.is_match(&clean_value) {
            return Err(FiscalError::InvalidRNC(value));
        }

        let tipo = if clean_value.len() == 9 {
            TipoContribuyente::PersonaJuridica
        } else {
            TipoContribuyente::PersonaFisica
        };

        Ok(Self {
            value: clean_value,
            tipo,
        })
    }

    /// Format RNC with dashes
    pub fn formatted(&self) -> String {
        if self.value.len() == 9 {
            // Format: 1-01-12345-6
            format!("{}-{}-{}-{}",
                &self.value[0..1],
                &self.value[1..3],
                &self.value[3..8],
                &self.value[8..9]
            )
        } else {
            // Cédula format: 001-1234567-8
            format!("{}-{}-{}",
                &self.value[0..3],
                &self.value[3..10],
                &self.value[10..11]
            )
        }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn tipo(&self) -> &TipoContribuyente {
        &self.tipo
    }
}

/// Cédula - Personal ID for individuals
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Cedula {
    value: String,
}

impl Cedula {
    /// Create new Cédula
    pub fn new(value: String) -> Result<Self, FiscalError> {
        let clean_value = value.chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>();

        if !CEDULA_REGEX.is_match(&clean_value) {
            return Err(FiscalError::InvalidCedula(value));
        }

        Ok(Self { value: clean_value })
    }

    /// Format cédula with dashes
    pub fn formatted(&self) -> String {
        format!("{}-{}-{}",
            &self.value[0..3],
            &self.value[3..10],
            &self.value[10..11]
        )
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// Tipo de Contribuyente
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TipoContribuyente {
    PersonaFisica,
    PersonaJuridica,
}

/// ITBIS (Impuesto a la Transferencia de Bienes y Servicios) - Sales tax
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ITBIS {
    rate: f64,
    amount: f64,
}

impl ITBIS {
    /// Standard ITBIS rate in Dominican Republic (18%)
    pub const STANDARD_RATE: f64 = 0.18;

    /// Reduced ITBIS rate (16%)
    pub const REDUCED_RATE: f64 = 0.16;

    /// Zero rate (exempt)
    pub const ZERO_RATE: f64 = 0.0;

    /// Create ITBIS with standard rate
    pub fn standard(base_amount: f64) -> Self {
        Self {
            rate: Self::STANDARD_RATE,
            amount: base_amount * Self::STANDARD_RATE,
        }
    }

    /// Create ITBIS with reduced rate
    pub fn reduced(base_amount: f64) -> Self {
        Self {
            rate: Self::REDUCED_RATE,
            amount: base_amount * Self::REDUCED_RATE,
        }
    }

    /// Create ITBIS with custom rate
    pub fn with_rate(base_amount: f64, rate: f64) -> Result<Self, FiscalError> {
        if rate < 0.0 || rate > 1.0 {
            return Err(FiscalError::InvalidITBISRate(rate));
        }

        Ok(Self {
            rate,
            amount: base_amount * rate,
        })
    }

    /// Calculate ITBIS from total amount (reverse calculation)
    pub fn from_total(total_amount: f64, rate: f64) -> Result<Self, FiscalError> {
        if rate < 0.0 || rate > 1.0 {
            return Err(FiscalError::InvalidITBISRate(rate));
        }

        let base_amount = total_amount / (1.0 + rate);
        let itbis_amount = total_amount - base_amount;

        Ok(Self {
            rate,
            amount: itbis_amount,
        })
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }

    pub fn amount(&self) -> f64 {
        self.amount
    }

    pub fn rate_percentage(&self) -> u8 {
        (self.rate * 100.0) as u8
    }
}

/// Fiscal errors
#[derive(Debug, thiserror::Error)]
pub enum FiscalError {
    #[error("Invalid NCF: {0}")]
    InvalidNCF(String),

    #[error("Invalid RNC: {0}")]
    InvalidRNC(String),

    #[error("Invalid Cédula: {0}")]
    InvalidCedula(String),

    #[error("Invalid tipo de comprobante: {0}")]
    InvalidTipoComprobante(String),

    #[error("Invalid ITBIS rate: {0}")]
    InvalidITBISRate(f64),

    #[error("NCF sequence exhausted")]
    NCFSequenceExhausted,

    #[error("NCF expired")]
    NCFExpired,
}

/// NCF Sequence Manager for generating sequential NCFs
pub struct NCFSequence {
    serie: char,
    tipo_code: u32,
    current: u64,
    max: u64,
    expiry_date: DateTime<Utc>,
}

impl NCFSequence {
    /// Create new NCF sequence with tipo code
    pub fn new(serie: char, tipo_code: u32, start: u64, max: u64, expiry_date: DateTime<Utc>) -> Self {
        Self {
            serie,
            tipo_code,
            current: start,
            max,
            expiry_date,
        }
    }

    /// Create sequence for Factura de Crédito Fiscal (E31)
    pub fn new_credito_fiscal(start: u64, max: u64, expiry_date: DateTime<Utc>) -> Self {
        Self::new('E', 31, start, max, expiry_date)
    }

    /// Create sequence for Factura de Consumidor Final (B01)
    pub fn new_consumidor_final(start: u64, max: u64, expiry_date: DateTime<Utc>) -> Self {
        Self::new('B', 01, start, max, expiry_date)
    }

    /// Get next NCF in sequence
    pub fn next(&mut self) -> Result<NCF, FiscalError> {
        if self.current > self.max {
            return Err(FiscalError::NCFSequenceExhausted);
        }

        if Utc::now() > self.expiry_date {
            return Err(FiscalError::NCFExpired);
        }

        let ncf = NCF::new_with_tipo(self.serie, self.tipo_code, self.current)?;
        self.current += 1;

        Ok(ncf)
    }

    /// Get remaining NCFs in sequence
    pub fn remaining(&self) -> u64 {
        if self.current > self.max {
            0
        } else {
            self.max - self.current + 1
        }
    }

    /// Days until expiry
    pub fn days_until_expiry(&self) -> i64 {
        (self.expiry_date - Utc::now()).num_days()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ncf_creation() {
        let ncf = NCF::new_credito_fiscal(1).unwrap();
        assert_eq!(ncf.as_str(), "E3100000001");
        assert_eq!(ncf.serie(), 'E');
        assert_eq!(ncf.tipo_code(), 31);
        assert_eq!(ncf.sequence(), 1);
    }

    #[test]
    fn test_ncf_parsing() {
        let ncf = NCF::parse("B0100000123").unwrap();
        assert_eq!(ncf.as_str(), "B0100000123");
        assert_eq!(ncf.serie(), 'B');
        assert_eq!(ncf.tipo_code(), 1);
        assert_eq!(ncf.sequence(), 123);
    }

    #[test]
    fn test_invalid_ncf() {
        assert!(NCF::parse("123456").is_err());
        assert!(NCF::parse("Z12345678901").is_err());
        assert!(NCF::new_with_tipo('E', 31, 100_000_000).is_err()); // Sequence too large
    }

    #[test]
    fn test_rnc_creation() {
        let rnc = RNC::new("101123456".to_string()).unwrap();
        assert_eq!(rnc.as_str(), "101123456");
        assert_eq!(rnc.formatted(), "1-01-12345-6");
        assert_eq!(rnc.tipo(), &TipoContribuyente::PersonaJuridica);
    }

    #[test]
    fn test_cedula_creation() {
        let cedula = Cedula::new("00112345678".to_string()).unwrap();
        assert_eq!(cedula.as_str(), "00112345678");
        assert_eq!(cedula.formatted(), "001-1234567-8");
    }

    #[test]
    fn test_itbis_calculation() {
        let itbis = ITBIS::standard(1000.0);
        assert_eq!(itbis.rate(), 0.18);
        assert_eq!(itbis.amount(), 180.0);
        assert_eq!(itbis.rate_percentage(), 18);

        let itbis_reverse = ITBIS::from_total(1180.0, 0.18).unwrap();
        assert!((itbis_reverse.amount() - 180.0).abs() < 0.01);
    }

    #[test]
    fn test_ncf_sequence() {
        let expiry = Utc::now() + Duration::days(365);
        let mut sequence = NCFSequence::new_credito_fiscal(1, 100, expiry);

        let ncf1 = sequence.next().unwrap();
        assert_eq!(ncf1.as_str(), "E3100000001");

        let ncf2 = sequence.next().unwrap();
        assert_eq!(ncf2.as_str(), "E3100000002");

        assert_eq!(sequence.remaining(), 98);
        assert!(sequence.days_until_expiry() > 360);
    }
}