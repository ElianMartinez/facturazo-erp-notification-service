//! Fiscal DTOs - Data Transfer Objects for Dominican Republic fiscal documents
//!
//! This module only contains data structures for receiving fiscal information
//! from the core service. NO business logic or validations.

use serde::{Deserialize, Serialize};

/// NCF - Fiscal receipt number (comes from core service)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NCF {
    pub value: String,                    // Full NCF like "E3100000001"
    pub formatted: Option<String>,        // Formatted like "E31-0000-0001"
    pub serie: Option<String>,            // E, B, etc
    pub tipo_description: Option<String>, // "Factura de Crédito Fiscal", etc
}

/// RNC - Tax ID (comes from core service)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RNC {
    pub value: String,             // Raw RNC
    pub formatted: Option<String>, // Formatted with dashes
}

/// Cédula - Personal ID (comes from core service)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Cedula {
    pub value: String,
    pub formatted: Option<String>,
}

/// ITBIS - Sales tax information (calculated by core service)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ITBIS {
    pub rate: f64,   // Rate as decimal (0.18 for 18%)
    pub amount: f64, // Calculated amount
}

/// Tax ID types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaxIdType {
    RNC,
    Cedula,
    Passport,
}

/// Generic Tax ID
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxId {
    pub value: String,
    pub tax_type: TaxIdType,
    pub formatted: Option<String>,
}
