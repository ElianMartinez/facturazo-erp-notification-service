//! Template manager with caching and versioning

use anyhow::Result;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

/// Template manager for document generation
pub struct TemplateManager {
    template_dir: PathBuf,
    cache: RwLock<HashMap<String, CachedTemplate>>,
}

impl TemplateManager {
    pub fn new(template_dir: PathBuf) -> Self {
        Self {
            template_dir,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get template by name (thread-safe)
    pub async fn get_template(&self, name: &str) -> Result<String> {
        // Check cache first
        {
            let cache = self.cache.read();
            if let Some(cached) = cache.get(name) {
                if !cached.is_expired() {
                    return Ok(cached.content.clone());
                }
            }
        }

        // Load from disk
        let template = self.load_template_from_disk(name).await?;

        // Update cache
        {
            let mut cache = self.cache.write();
            cache.insert(name.to_string(), CachedTemplate::new(template.clone()));
        }

        Ok(template)
    }

    /// Load template from disk
    async fn load_template_from_disk(&self, name: &str) -> Result<String> {
        let file_path = self.template_dir.join(format!("{}.typ", name));

        if !file_path.exists() {
            // Try with .typst extension
            let alt_path = self.template_dir.join(format!("{}.typst", name));

            if alt_path.exists() {
                return Ok(fs::read_to_string(alt_path).await?);
            }

            // Return built-in template
            return self.get_builtin_template(name);
        }

        Ok(fs::read_to_string(file_path).await?)
    }

    /// Get built-in template
    fn get_builtin_template(&self, name: &str) -> Result<String> {
        match name {
            "invoice_fiscal" => Ok(templates::INVOICE_FISCAL.to_string()),
            "quotation" => Ok(templates::QUOTATION.to_string()),
            "report" => Ok(templates::REPORT.to_string()),
            "receipt" => Ok(templates::RECEIPT.to_string()),
            "credit_note" => Ok(templates::CREDIT_NOTE.to_string()),
            _ => Err(anyhow::anyhow!("Template not found: {}", name)),
        }
    }

    /// Save template to disk
    pub async fn save_template(&self, name: &str, content: &str) -> Result<()> {
        let file_path = self.template_dir.join(format!("{}.typ", name));

        // Ensure directory exists
        fs::create_dir_all(&self.template_dir).await?;

        // Save to disk
        fs::write(&file_path, content).await?;

        // Update cache
        {
            let mut cache = self.cache.write();
            cache.insert(name.to_string(), CachedTemplate::new(content.to_string()));
        }

        Ok(())
    }

    /// List available templates
    pub async fn list_templates(&self) -> Result<Vec<TemplateInfo>> {
        let mut templates = Vec::new();

        // List disk templates
        if self.template_dir.exists() {
            let mut entries = fs::read_dir(&self.template_dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                if path.extension().and_then(|s| s.to_str()) == Some("typ")
                    || path.extension().and_then(|s| s.to_str()) == Some("typst")
                {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        let metadata = entry.metadata().await?;
                        let modified = metadata
                            .modified()?
                            .duration_since(std::time::UNIX_EPOCH)?
                            .as_secs();

                        templates.push(TemplateInfo {
                            name: name.to_string(),
                            path: path.to_string_lossy().to_string(),
                            size: metadata.len() as usize,
                            modified: DateTime::from_timestamp(modified as i64, 0)
                                .unwrap_or_else(|| Utc::now()),
                            is_builtin: false,
                        });
                    }
                }
            }
        }

        // Add built-in templates
        for name in &[
            "invoice_fiscal",
            "quotation",
            "report",
            "receipt",
            "credit_note",
        ] {
            if !templates.iter().any(|t| t.name == *name) {
                templates.push(TemplateInfo {
                    name: name.to_string(),
                    path: format!("builtin:{}", name),
                    size: 0,
                    modified: Utc::now(),
                    is_builtin: true,
                });
            }
        }

        Ok(templates)
    }

    /// Clear cache
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        let cache = self.cache.read();
        CacheStats {
            templates_cached: cache.len(),
            total_size: cache.values().map(|t| t.content.len()).sum(),
        }
    }
}

/// Cached template
#[derive(Clone)]
struct CachedTemplate {
    content: String,
    cached_at: DateTime<Utc>,
    ttl_seconds: i64,
}

impl CachedTemplate {
    fn new(content: String) -> Self {
        Self {
            content,
            cached_at: Utc::now(),
            ttl_seconds: 3600, // 1 hour TTL
        }
    }

    fn is_expired(&self) -> bool {
        let age = Utc::now() - self.cached_at;
        age.num_seconds() > self.ttl_seconds
    }
}

/// Template information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub name: String,
    pub path: String,
    pub size: usize,
    pub modified: DateTime<Utc>,
    pub is_builtin: bool,
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub templates_cached: usize,
    pub total_size: usize,
}

/// Template versioning support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVersion {
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<String>,
    pub description: Option<String>,
    pub content_hash: String,
}

impl TemplateVersion {
    pub fn new(content: &str, created_by: Option<String>, description: Option<String>) -> Self {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        use ring::digest;

        let hash = digest::digest(&digest::SHA256, content.as_bytes());
        let content_hash = URL_SAFE_NO_PAD.encode(hash.as_ref());

        Self {
            version: Self::generate_version(),
            created_at: Utc::now(),
            created_by,
            description,
            content_hash,
        }
    }

    fn generate_version() -> String {
        format!("v{}", Utc::now().format("%Y%m%d.%H%M%S"))
    }
}

// Built-in templates
mod templates {
    // Full fiscal invoice template with Dominican Republic compliance
    // Uses nested structure matching InvoiceData from Core service:
    // - company_info: {name, legal_name, tax_id, address, phone, email}
    // - client_info: {name, tax_id, address, phone, email}
    // - items: [{description, quantity, unit, unit_price, tax_rate, tax_amount, discount, total}]
    // - totals: {subtotal, tax_amount, discount_amount, total, currency}
    // - fiscal_info: {e_ncf, security_code, signature_date, expiration_date}
    // - payment_info: {method, terms, paid}
    pub const INVOICE_FISCAL: &str = r##"
#set page(
  paper: "us-letter",
  margin: (top: 1.5cm, bottom: 1.5cm, left: 1.5cm, right: 1.5cm),
)

#set text(font: "Inter", size: 10pt, lang: "es")
#set table(stroke: 0.5pt)

// Header with company info
#grid(
  columns: (1fr, 1fr),
  gutter: 1em,

  // Left side - Company info
  [
    #text(size: 16pt, weight: "bold")[{{company_info.name}}]
    #v(0.5em)
    {{#if company_info.legal_name}}#text(size: 12pt)[{{company_info.legal_name}}]#v(0.3em){{/if}}

    RNC: #text(weight: "bold")[{{company_info.tax_id}}] \
    {{company_info.address.street}} {{company_info.address.line2}} \
    Tel: {{company_info.phone}} \
    {{#if company_info.email}}Email: #raw("{{company_info.email}}"){{/if}}
  ],

  // Right side - Invoice info
  align(right)[
    #rect(stroke: 2pt + rgb("#004080"), inset: 10pt)[
      #text(size: 12pt, weight: "bold", fill: rgb("#004080"))[{{#if document_type_name}}{{document_type_name}}{{else}}Factura Electrónica{{/if}}]

      #v(0.5em)
      #text(size: 10pt)[
        {{#if fiscal_info}}e-NCF: #text(weight: "bold")[{{fiscal_info.e_ncf}}]{{/if}}
        {{#unless fiscal_info}}Factura: #text(weight: "bold")[{{invoice_number}}]{{/unless}} \
        {{#if fiscal_info.expiration_date}}Válido hasta: {{fiscal_info.expiration_date}}{{/if}} \
        Fecha: {{issue_date}} \
        {{#if due_date}}Vencimiento: {{due_date}}{{/if}}
      ]
    ]
  ]
)

#v(1em)

// Customer info
#rect(width: 100%, stroke: 0.5pt, inset: 10pt)[
  #text(weight: "bold")[DATOS DEL CLIENTE]
  #v(0.5em)
  #table(
    columns: (auto, 1fr, auto, 1fr),
    stroke: none,
    inset: 3pt,
    align: (left, left, left, left),

    [Nombre/Razón Social:], [#text(weight: "bold")[{{client_info.name}}]],
    {{#if client_info.tax_id}}[RNC/Cédula:], [#text(weight: "bold")[{{client_info.tax_id}}]],{{/if}}
    {{#unless client_info.tax_id}}[], [],{{/unless}}

    {{#if client_info.phone}}[Teléfono:], [{{client_info.phone}}],{{/if}}
    {{#unless client_info.phone}}[], [],{{/unless}}
    {{#if client_info.email}}[Email:], [#raw("{{client_info.email}}")],{{/if}}
    {{#unless client_info.email}}[], [],{{/unless}}

    {{#if client_info.address}}[Dirección:], [#text[{{client_info.address}}]], [], [],{{/if}}
  )
]

#v(1em)

// Invoice items table
#table(
  columns: (0.8fr, 3fr, 0.8fr, 1fr, 1fr, 1.2fr),
  inset: 6pt,
  align: (center, left, center, right, right, right),

  // Header
  table.header(
    [*Cant.*], [*Descripción*], [*U/M*], [*Precio*], [*ITBIS*], [*Total*]
  ),

  // Items
  {{#each items}}
  [{{quantity}}],
  [{{description}}],
  [{{unit}}],
  [{{unit_price}}],
  [{{tax_amount}}],
  [{{total}}],
  {{/each}}
)

#v(1em)

// Totals
#align(right)[
  #table(
    columns: (3fr, 2fr),
    inset: 8pt,
    align: (left, right),
    stroke: none,

    [Subtotal:], [#text(weight: "bold")[{{totals.currency}} {{totals.subtotal}}]],
    {{#if totals.discount_amount}}[Descuento:], [#text(weight: "bold")[-{{totals.currency}} {{totals.discount_amount}}]],{{/if}}
    [ITBIS:], [#text(weight: "bold")[{{totals.currency}} {{totals.tax_amount}}]],
    {{#if totals.tip}}[Propina (10%):], [#text(weight: "bold")[{{totals.currency}} {{totals.tip}}]],{{/if}}

    table.hline(stroke: 2pt),
    [#text(size: 12pt, weight: "bold")[TOTAL:]],
    [#text(size: 12pt, weight: "bold", fill: rgb("#004080"))[{{totals.currency}} {{totals.total}}]]
  )
]

#v(1em)

// Fiscal verification section
{{#if fiscal_info}}
#rect(width: 100%, stroke: 0.5pt, inset: 8pt, fill: rgb("#f8f9fa"))[
  #grid(
    columns: (1fr, auto),
    column-gutter: 1em,
    [
      #text(weight: "bold")[Verificación DGII] \
      #text(size: 9pt)[Código de Seguridad: {{fiscal_info.security_code}}] \
      #text(size: 9pt)[Fecha Firma: {{fiscal_info.signature_date}}]
    ],
    [
      {{#if qr_code_path}}
      #align(center)[
        #image("{{qr_code_path}}", width: 2.5cm)
        #text(size: 7pt)[Escanea para verificar]
      ]
      {{/if}}
    ]
  )
]
{{/if}}

#v(0.5em)

// Payment terms
{{#if payment_info}}
#rect(width: 100%, stroke: 0.5pt, inset: 8pt, fill: rgb("#f0f0f0"))[
  *Condiciones de Pago:* {{payment_info.method}} \
  *Términos:* {{payment_info.terms}}
]
{{/if}}

{{#if notes}}
#v(0.5em)
#rect(width: 100%, stroke: 0.5pt, inset: 8pt)[
  *Notas:* \
  {{notes}}
]
{{/if}}

// Footer
#v(1fr)
#line(length: 100%, stroke: 0.5pt)
#align(center)[
  #text(size: 8pt, fill: gray)[
    Retención según Ley 253-12: Las personas físicas y jurídicas que adquieran bienes y servicios \
    que no sean contribuyentes del ITBIS retendrán el 30% del ITBIS facturado. \
    Este documento debe conservarse por 7 años según requerimientos de la DGII.
  ]
]

// Watermark if paid
{{#if payment_info.paid}}
#place(center + horizon)[
  #rotate(45deg)[
    #text(size: 72pt, fill: rgb("#00ff00").transparentize(80%), weight: "bold")[PAGADO]
  ]
]
{{/if}}
    "##;

    pub const QUOTATION: &str = r#"
#set page(paper: "us-letter")
= Quotation Template
Business quotation template.
    "#;

    pub const REPORT: &str = r#"
#set page(paper: "us-letter")
= Report Template
Report with tables and charts.
    "#;

    pub const RECEIPT: &str = r#"
#set page(paper: "us-letter")
= Receipt Template
Payment receipt template.
    "#;

    pub const CREDIT_NOTE: &str = r#"
#set page(paper: "us-letter")
= Credit Note Template
Credit note for returns and adjustments.
    "#;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_template_manager() {
        let temp_dir = TempDir::new().unwrap();
        let manager = TemplateManager::new(temp_dir.path().to_path_buf());

        // Save a template
        manager
            .save_template("test", "Test template content")
            .await
            .unwrap();

        // Get template (should be cached)
        let template = manager.get_template("test").await.unwrap();
        assert_eq!(template, "Test template content");

        // Check cache stats
        let stats = manager.cache_stats();
        assert_eq!(stats.templates_cached, 1);

        // List templates
        let templates = manager.list_templates().await.unwrap();
        assert!(templates.iter().any(|t| t.name == "test"));

        // Clear cache
        manager.clear_cache();
        assert_eq!(manager.cache_stats().templates_cached, 0);
    }

    #[test]
    fn test_template_versioning() {
        let version = TemplateVersion::new(
            "template content",
            Some("test_user".to_string()),
            Some("Initial version".to_string()),
        );

        assert!(version.version.starts_with("v"));
        assert!(!version.content_hash.is_empty()); // SHA256 base64 encoded
    }
}
