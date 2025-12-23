use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================
// ERP Generic Report Models
// ============================================

/// Complete payload for generating ERP reports
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErpReportPayload {
    /// Correlation ID for distributed tracing across services
    #[serde(alias = "correlation_id")]
    pub correlation_id: Option<String>,

    /// Document type identifier
    #[serde(default = "default_document_type")]
    pub document_type: String,

    /// Tenant ID for multi-tenant support
    #[serde(alias = "tenant_id")]
    pub tenant_id: i64,

    /// User ID who requested the report
    #[serde(alias = "user_id")]
    pub user_id: i64,

    /// Report metadata
    pub report: ReportInfo,

    /// Column and grouping definitions
    pub metadata: ReportMetadata,

    /// The actual report data
    pub data: ReportDataSet,

    /// Output options (PDF settings)
    pub output: OutputOptions,

    /// Delivery options (email, whatsapp)
    pub delivery: Option<DeliveryOptions>,

    /// Company information (for header/logo)
    #[serde(alias = "company_info")]
    pub company_info: Option<ReportCompanyInfo>,
}

/// Company information for report header
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportCompanyInfo {
    /// Company name
    pub name: String,

    /// Display name
    #[serde(alias = "display_name")]
    pub display_name: Option<String>,

    /// Tax ID / RNC
    #[serde(alias = "tax_id")]
    pub tax_id: Option<String>,

    /// Logo URL
    #[serde(alias = "logo_url")]
    pub logo_url: Option<String>,

    /// Company address
    pub address: Option<String>,

    /// Phone number
    pub phone: Option<String>,

    /// Email address
    pub email: Option<String>,

    /// Website URL
    pub website: Option<String>,
}

fn default_document_type() -> String {
    "REPORT".to_string()
}

/// Threshold for synchronous processing (number of records)
const SYNC_RECORD_THRESHOLD: i32 = 1000;

/// Threshold for synchronous processing (estimated seconds)
const SYNC_TIME_THRESHOLD_SECS: u64 = 5;

impl ErpReportPayload {
    /// Determines if the report should be processed synchronously
    /// Returns true if:
    /// - Total records < 1000
    /// - Estimated processing time < 5 seconds
    pub fn should_process_sync(&self) -> bool {
        // Check record count threshold
        if self.data.total_records >= SYNC_RECORD_THRESHOLD {
            return false;
        }

        // Check estimated processing time
        let estimated_time = self.estimate_processing_time_secs();
        if estimated_time >= SYNC_TIME_THRESHOLD_SECS {
            return false;
        }

        true
    }

    /// Estimates processing time in seconds based on report characteristics
    pub fn estimate_processing_time_secs(&self) -> u64 {
        let base_time: u64 = 1; // Base time for any report
        let records = self.data.total_records as u64;
        let columns = self.metadata.columns.len() as u64;

        // Estimate: ~0.001s per record + 0.0001s per column per record
        let record_time = records / 1000; // 1ms per record
        let column_factor = (columns * records) / 10000; // 0.1ms per column-record

        // Add time for complex structures
        let structure_factor = match self.data.structure_type {
            DataStructureType::Flat => 0,
            DataStructureType::Grouped => 1,
            DataStructureType::HierarchicalGrouped => 2,
            DataStructureType::Statement => 1,
        };

        // Add time for output format
        let format_factor = match self.output.format {
            OutputFormat::Pdf => 2,  // PDF generation is slower
            OutputFormat::Xlsx => 1, // Excel is medium
            OutputFormat::Csv => 0,  // CSV is fastest
            OutputFormat::Html => 0, // HTML is fast
        };

        base_time + record_time + column_factor + structure_factor + format_factor
    }

    /// Returns the processing mode recommendation
    pub fn get_processing_mode(&self) -> ProcessingMode {
        if self.should_process_sync() {
            ProcessingMode::Synchronous
        } else {
            ProcessingMode::Asynchronous
        }
    }
}

/// Processing mode for report generation
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessingMode {
    /// Process immediately and return result
    Synchronous,
    /// Queue for background processing and return job ID
    Asynchronous,
}

/// Report header information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReportInfo {
    /// Report code (e.g., "NCF_SUMMARY")
    #[serde(default)]
    pub code: String,

    /// Report variant (e.g., "DETAILED")
    pub variant: Option<String>,

    /// Display title
    #[serde(default)]
    pub title: String,

    /// Report subtitle (e.g., "Ventas", "Compras")
    #[serde(default)]
    pub subtitle: Option<String>,

    /// Breadcrumb path
    pub breadcrumb: Option<String>,

    /// Generation timestamp
    #[serde(alias = "generated_at", default)]
    pub generated_at: String,

    /// User display name who generated the report
    #[serde(alias = "user_name", default)]
    pub user_name: Option<String>,

    /// Date range filter (if applicable)
    #[serde(alias = "date_range")]
    pub date_range: Option<DateRange>,

    /// As-of date filter (if applicable)
    #[serde(alias = "as_of_date")]
    pub as_of_date: Option<String>,
}

/// Date range for report filters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub from: String,
    pub to: String,
}

/// Report metadata with column and grouping definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportMetadata {
    /// Column definitions
    pub columns: Vec<ColumnDefinition>,

    /// Grouping configuration (optional)
    pub grouping: Option<GroupingConfig>,

    /// Whether to show grand total
    #[serde(default)]
    pub show_grand_total: bool,

    /// Fields to aggregate for grand total
    #[serde(default)]
    pub grand_total_fields: Vec<String>,

    /// Whether report has running balance (for statements)
    #[serde(default)]
    pub has_running_balance: bool,

    /// Whether to show opening balance
    #[serde(default)]
    pub show_opening_balance: bool,

    /// Aging buckets configuration (for AR reports)
    #[serde(alias = "aging_buckets")]
    pub aging_buckets: Option<Vec<AgingBucket>>,
}

/// Column definition for report tables
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnDefinition {
    /// Field key (maps to data property)
    pub key: String,

    /// Display label
    pub label: String,

    /// Column data type
    #[serde(alias = "type", rename = "type")]
    pub column_type: ColumnType,

    /// Display format (e.g., "dd/MM/yyyy", "#,##0.00")
    pub format: Option<String>,

    /// Suggested width in pixels
    pub width: Option<i32>,

    /// Text alignment
    pub align: Option<ColumnAlign>,

    /// Whether column is sortable
    #[serde(default)]
    pub sortable: bool,

    /// Highlight style (e.g., "primary", "warning", "danger")
    pub highlight: Option<String>,

    /// Whether column is hidden
    #[serde(default)]
    pub hidden: bool,

    /// Whether column should be hidden when printing (PDF export)
    /// When true, column appears in preview but not in PDF
    #[serde(alias = "hide_in_print", default)]
    pub hide_in_print: bool,
}

/// Column data types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ColumnType {
    String,
    Integer,
    Decimal,
    Currency,
    Date,
    DateTime,
    Time,
    Boolean,
    Percentage,
}

impl Default for ColumnType {
    fn default() -> Self {
        ColumnType::String
    }
}

/// Column text alignment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ColumnAlign {
    Left,
    Center,
    Right,
}

impl Default for ColumnAlign {
    fn default() -> Self {
        ColumnAlign::Left
    }
}

/// Grouping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupingConfig {
    /// Whether grouping is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Field to group by
    pub field: Option<String>,

    /// Field containing display label
    #[serde(alias = "label_field")]
    pub label_field: Option<String>,

    /// Prefix for group labels
    #[serde(alias = "label_prefix")]
    pub label_prefix: Option<String>,

    /// Whether to show subtotals
    #[serde(alias = "show_subtotals", default)]
    pub show_subtotals: bool,

    /// Label for subtotal rows
    #[serde(alias = "subtotal_label")]
    pub subtotal_label: Option<String>,

    /// Fields to aggregate for subtotals
    #[serde(alias = "subtotal_fields", default)]
    pub subtotal_fields: Vec<String>,

    /// Multi-level grouping configuration
    pub levels: Option<Vec<GroupingLevel>>,
}

/// Grouping level for hierarchical reports
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupingLevel {
    /// Hierarchy depth (0 = top level)
    pub order: i32,

    /// Field to group by
    pub field: String,

    /// Field containing display label
    #[serde(alias = "label_field")]
    pub label_field: String,

    /// Template for group label
    #[serde(alias = "label_template")]
    pub label_template: Option<String>,

    /// Whether to show subtotals
    #[serde(alias = "show_subtotals", default)]
    pub show_subtotals: bool,

    /// Fields to aggregate for subtotals
    #[serde(alias = "subtotal_fields", default)]
    pub subtotal_fields: Vec<String>,
}

/// Aging bucket configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgingBucket {
    pub name: String,
    #[serde(alias = "days_from")]
    pub days_from: i32,
    #[serde(alias = "days_to")]
    pub days_to: Option<i32>,
}

// ============================================
// Report Data Structures
// ============================================

/// Report data set - supports multiple structure types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportDataSet {
    /// Structure type determines how data is organized
    #[serde(alias = "structure_type")]
    pub structure_type: DataStructureType,

    /// Flat rows (for Flat structure)
    pub rows: Option<Vec<HashMap<String, serde_json::Value>>>,

    /// Grouped data (for Grouped/HierarchicalGrouped)
    pub groups: Option<Vec<ReportGroup>>,

    /// Opening balance (for Statement structure)
    #[serde(alias = "opening_balance")]
    pub opening_balance: Option<OpeningBalance>,

    /// Grand total row
    #[serde(alias = "grand_total")]
    pub grand_total: Option<HashMap<String, serde_json::Value>>,

    /// Summary (for Statement structure)
    pub summary: Option<StatementSummary>,

    /// Total record count
    #[serde(alias = "total_records")]
    pub total_records: i32,

    /// Totals object (alternative to grand_total)
    pub totals: Option<HashMap<String, serde_json::Value>>,
}

/// Data structure types
/// Supports both string ("Flat", "Grouped", etc.) and number (0, 1, 2, 3) deserialization
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum DataStructureType {
    Flat,
    Grouped,
    HierarchicalGrouped,
    Statement,
}

// Custom deserializer to handle both string and integer formats from C#
impl<'de> serde::Deserialize<'de> for DataStructureType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct DataStructureTypeVisitor;

        impl<'de> Visitor<'de> for DataStructureTypeVisitor {
            type Value = DataStructureType;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or integer representing DataStructureType")
            }

            fn visit_str<E>(self, value: &str) -> Result<DataStructureType, E>
            where
                E: de::Error,
            {
                match value {
                    "Flat" | "flat" => Ok(DataStructureType::Flat),
                    "Grouped" | "grouped" => Ok(DataStructureType::Grouped),
                    "HierarchicalGrouped" | "hierarchicalGrouped" | "hierarchical_grouped" => {
                        Ok(DataStructureType::HierarchicalGrouped)
                    }
                    "Statement" | "statement" => Ok(DataStructureType::Statement),
                    _ => Err(de::Error::unknown_variant(
                        value,
                        &["Flat", "Grouped", "HierarchicalGrouped", "Statement"],
                    )),
                }
            }

            fn visit_u64<E>(self, value: u64) -> Result<DataStructureType, E>
            where
                E: de::Error,
            {
                match value {
                    0 => Ok(DataStructureType::Flat),
                    1 => Ok(DataStructureType::Grouped),
                    2 => Ok(DataStructureType::HierarchicalGrouped),
                    3 => Ok(DataStructureType::Statement),
                    _ => Err(de::Error::invalid_value(
                        de::Unexpected::Unsigned(value),
                        &"0, 1, 2, or 3",
                    )),
                }
            }

            fn visit_i64<E>(self, value: i64) -> Result<DataStructureType, E>
            where
                E: de::Error,
            {
                self.visit_u64(value as u64)
            }
        }

        deserializer.deserialize_any(DataStructureTypeVisitor)
    }
}

impl Default for DataStructureType {
    fn default() -> Self {
        DataStructureType::Flat
    }
}

/// Report group (for grouped data)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportGroup {
    /// Grouping level (0 = top)
    #[serde(default)]
    pub level: i32,

    /// Group key
    pub key: String,

    /// Display label
    pub label: String,

    /// Record count in this group
    #[serde(alias = "record_count", default)]
    pub record_count: i32,

    /// Subtotal values
    pub subtotal: Option<HashMap<String, serde_json::Value>>,

    /// Data rows in this group
    #[serde(default)]
    pub rows: Vec<HashMap<String, serde_json::Value>>,

    /// Nested groups (for hierarchical)
    #[serde(alias = "sub_groups")]
    pub sub_groups: Option<Vec<ReportGroup>>,

    /// Whether group is expanded by default (UI hint)
    #[serde(alias = "default_expanded", default = "default_true")]
    pub default_expanded: bool,
}

fn default_true() -> bool {
    true
}

/// Opening balance for statement reports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpeningBalance {
    pub label: String,
    pub value: f64,
}

/// Statement summary
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatementSummary {
    #[serde(alias = "document_count")]
    pub document_count: Option<i32>,
    pub totals: Option<HashMap<String, f64>>,
    #[serde(alias = "final_balance")]
    pub final_balance: Option<f64>,
}

// ============================================
// Output Options
// ============================================

/// PDF/Document output options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputOptions {
    /// Output format (Pdf, Xlsx, Csv)
    pub format: OutputFormat,

    /// Page size
    #[serde(alias = "page_size")]
    pub page_size: Option<PageSize>,

    /// Page orientation
    pub orientation: Option<PageOrientation>,

    /// Scale percentage (50-150)
    #[serde(default = "default_scale")]
    pub scale: i32,

    /// Page margins
    pub margins: Option<PageMargins>,

    /// Whether to include header
    #[serde(alias = "include_header", default = "default_true")]
    pub include_header: bool,

    /// Whether to include footer
    #[serde(alias = "include_footer", default = "default_true")]
    pub include_footer: bool,

    /// Whether to show company logo
    #[serde(alias = "show_logo", default)]
    pub show_logo: bool,

    /// Custom file name
    #[serde(alias = "file_name")]
    pub file_name: Option<String>,
}

fn default_scale() -> i32 {
    100
}

/// Output formats
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OutputFormat {
    Pdf,
    Xlsx,
    Csv,
    Html,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Pdf
    }
}

/// Page sizes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PageSize {
    Letter,
    Legal,
    A4,
    A3,
    Custom { width: f64, height: f64 },
}

impl Default for PageSize {
    fn default() -> Self {
        PageSize::Letter
    }
}

impl PageSize {
    /// Get Typst paper string
    pub fn to_typst_paper(&self) -> String {
        match self {
            PageSize::Letter => "us-letter".to_string(),
            PageSize::Legal => "us-legal".to_string(),
            PageSize::A4 => "a4".to_string(),
            PageSize::A3 => "a3".to_string(),
            PageSize::Custom { width, height } => {
                format!("(width: {}mm, height: {}mm)", width, height)
            }
        }
    }
}

/// Page orientation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PageOrientation {
    Portrait,
    Landscape,
}

impl Default for PageOrientation {
    fn default() -> Self {
        PageOrientation::Portrait
    }
}

/// Page margins in mm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageMargins {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

impl Default for PageMargins {
    fn default() -> Self {
        Self {
            top: 15.0,
            bottom: 15.0,
            left: 15.0,
            right: 15.0,
        }
    }
}

// ============================================
// Delivery Options
// ============================================

/// Delivery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryOptions {
    /// Delivery method
    pub method: DeliveryMethod,

    /// Email delivery options
    pub email: Option<EmailDelivery>,

    /// WhatsApp delivery options
    pub whatsapp: Option<WhatsAppDelivery>,
}

/// Delivery methods
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryMethod {
    Download,
    View,
    Email,
    WhatsApp,
}

impl Default for DeliveryMethod {
    fn default() -> Self {
        DeliveryMethod::Download
    }
}

/// Email delivery options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailDelivery {
    pub recipients: Vec<String>,
    pub subject: Option<String>,
    pub message: Option<String>,
}

/// WhatsApp delivery options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppDelivery {
    pub numbers: Vec<String>,
    pub message: Option<String>,
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_report_payload() {
        let json = r#"{
            "documentType": "REPORT",
            "tenantId": 1,
            "userId": 42,
            "report": {
                "code": "NCF_SUMMARY",
                "variant": "DETAILED",
                "title": "Facturas Emitidas por NCF",
                "generatedAt": "2025-12-15T04:30:00Z"
            },
            "metadata": {
                "columns": [
                    { "key": "invoiceDate", "label": "Fecha", "type": "Date" },
                    { "key": "total", "label": "Total", "type": "Currency", "highlight": "primary" }
                ],
                "showGrandTotal": true,
                "grandTotalFields": ["total"]
            },
            "data": {
                "structureType": "Grouped",
                "groups": [
                    {
                        "key": "VENTAS",
                        "label": "VENTAS",
                        "recordCount": 10,
                        "rows": []
                    }
                ],
                "totalRecords": 10
            },
            "output": {
                "format": "Pdf",
                "pageSize": "Letter",
                "orientation": "Landscape"
            }
        }"#;

        let payload: ErpReportPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.report.code, "NCF_SUMMARY");
        assert_eq!(payload.metadata.columns.len(), 2);
        assert!(payload.metadata.show_grand_total);
    }

    #[test]
    fn test_page_size_to_typst() {
        assert_eq!(PageSize::Letter.to_typst_paper(), "us-letter");
        assert_eq!(PageSize::A4.to_typst_paper(), "a4");
        assert_eq!(
            PageSize::Custom {
                width: 200.0,
                height: 300.0
            }
            .to_typst_paper(),
            "(width: 200mm, height: 300mm)"
        );
    }

    #[test]
    fn test_sync_processing_small_report() {
        let json = r#"{
            "tenantId": 1,
            "userId": 1,
            "report": {
                "code": "TEST",
                "title": "Test",
                "generatedAt": "2025-12-15T04:30:00Z"
            },
            "metadata": {
                "columns": [
                    { "key": "name", "label": "Name", "type": "String" }
                ]
            },
            "data": {
                "structureType": "Flat",
                "totalRecords": 100
            },
            "output": {
                "format": "Pdf"
            }
        }"#;

        let payload: ErpReportPayload = serde_json::from_str(json).unwrap();
        assert!(payload.should_process_sync());
        assert_eq!(payload.get_processing_mode(), ProcessingMode::Synchronous);
    }

    #[test]
    fn test_async_processing_large_report() {
        let json = r#"{
            "tenantId": 1,
            "userId": 1,
            "report": {
                "code": "TEST",
                "title": "Test",
                "generatedAt": "2025-12-15T04:30:00Z"
            },
            "metadata": {
                "columns": [
                    { "key": "name", "label": "Name", "type": "String" }
                ]
            },
            "data": {
                "structureType": "Flat",
                "totalRecords": 5000
            },
            "output": {
                "format": "Pdf"
            }
        }"#;

        let payload: ErpReportPayload = serde_json::from_str(json).unwrap();
        assert!(!payload.should_process_sync());
        assert_eq!(payload.get_processing_mode(), ProcessingMode::Asynchronous);
    }

    #[test]
    fn test_estimate_processing_time() {
        let json = r#"{
            "tenantId": 1,
            "userId": 1,
            "report": {
                "code": "TEST",
                "title": "Test",
                "generatedAt": "2025-12-15T04:30:00Z"
            },
            "metadata": {
                "columns": [
                    { "key": "a", "label": "A", "type": "String" },
                    { "key": "b", "label": "B", "type": "Currency" }
                ]
            },
            "data": {
                "structureType": "Grouped",
                "totalRecords": 500
            },
            "output": {
                "format": "Pdf"
            }
        }"#;

        let payload: ErpReportPayload = serde_json::from_str(json).unwrap();
        let time = payload.estimate_processing_time_secs();
        // Should be: 1 (base) + 0 (500/1000) + 0 (2*500/10000) + 1 (grouped) + 2 (pdf) = 4
        assert!(time < 5, "Small report should estimate < 5 seconds");
    }

    #[test]
    fn test_output_options_snake_case_deserialization() {
        // This JSON simulates what C# sends with snake_case naming policy
        let json = r#"{
            "tenantId": 1,
            "userId": 1,
            "report": {
                "code": "TEST",
                "title": "Test",
                "generatedAt": "2025-12-15T04:30:00Z"
            },
            "metadata": {
                "columns": [
                    { "key": "name", "label": "Name", "type": "String" }
                ]
            },
            "data": {
                "structureType": "Flat",
                "totalRecords": 10
            },
            "output": {
                "format": "Pdf",
                "page_size": "Letter",
                "orientation": "Landscape",
                "scale": 85,
                "margins": {"top": 10, "bottom": 10, "left": 5, "right": 5},
                "include_header": true,
                "include_footer": false,
                "show_logo": true
            }
        }"#;

        let payload: ErpReportPayload = serde_json::from_str(json).unwrap();

        // Verify output options are deserialized correctly
        assert_eq!(payload.output.format, OutputFormat::Pdf);
        assert_eq!(
            payload.output.page_size,
            Some(PageSize::Letter),
            "page_size should be Letter"
        );
        assert_eq!(
            payload.output.orientation,
            Some(PageOrientation::Landscape),
            "orientation should be Landscape"
        );
        assert_eq!(payload.output.scale, 85, "scale should be 85");
        assert!(
            payload.output.margins.is_some(),
            "margins should be present"
        );
        let margins = payload.output.margins.as_ref().unwrap();
        assert_eq!(margins.top, 10.0);
        assert_eq!(margins.left, 5.0);
        assert!(
            payload.output.include_header,
            "include_header should be true"
        );
        assert!(
            !payload.output.include_footer,
            "include_footer should be false"
        );
        assert!(payload.output.show_logo, "show_logo should be true");
    }
}
