use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::RenderOptions;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportRequest {
    pub template_id: String,
    pub title: String,
    pub data_source: DataSource,
    pub schema: ReportSchema,
    pub options: Option<ReportOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DataSource {
    /// Data inline (< 1MB)
    Inline {
        rows: Vec<serde_json::Value>,
    },

    /// Data comprimida (1-10MB)
    Compressed {
        format: CompressionFormat,
        #[serde(with = "base64")]
        data: Vec<u8>,
    },

    /// Referencia a archivo en R2/S3
    R2Reference {
        bucket: String,
        key: String,
        format: FileFormat,
        row_count: Option<usize>,
        size_bytes: Option<usize>,
    },

    /// Stream desde endpoint
    StreamingEndpoint {
        url: String,
        auth: Option<AuthMethod>,
        pagination: Option<PaginationConfig>,
    },

    /// Query directa a base de datos
    DatabaseQuery {
        connection_id: String,
        query: String,
        parameters: Option<HashMap<String, serde_json::Value>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionFormat {
    Gzip,
    Zstd,
    Deflate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileFormat {
    Csv,
    Json,
    Jsonl,
    Parquet,
    Excel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthMethod {
    #[serde(rename = "type")]
    pub auth_type: String, // "bearer", "basic", "api_key"
    pub credentials: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationConfig {
    pub page_size: usize,
    pub page_param: String,
    pub size_param: String,
    pub total_pages: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSchema {
    pub columns: Vec<ColumnDefinition>,
    pub grouping: Option<GroupingConfig>,
    pub sorting: Option<SortingConfig>,
    pub aggregations: Option<Vec<Aggregation>>,
    pub filters: Option<Vec<FilterConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDefinition {
    pub field: String,
    pub header: String,
    pub data_type: DataType,
    pub format: Option<String>,
    pub width: Option<f32>,
    pub alignment: Alignment,
    pub visible: bool,
    pub formula: Option<String>, // Para columnas calculadas
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    String,
    Number,
    Currency,
    Date,
    DateTime,
    Boolean,
    Percentage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupingConfig {
    pub group_by: Vec<String>,
    pub show_subtotals: bool,
    pub collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortingConfig {
    pub sort_by: Vec<SortColumn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortColumn {
    pub field: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aggregation {
    pub field: String,
    pub operation: AggregateOperation,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateOperation {
    Sum,
    Average,
    Count,
    Min,
    Max,
    Distinct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    pub field: String,
    pub operator: FilterOperator,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOperator {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    GreaterOrEqual,
    LessOrEqual,
    Contains,
    StartsWith,
    EndsWith,
    In,
    NotIn,
    Between,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportOptions {
    pub render: RenderOptions,
    pub include_summary: bool,
    pub include_charts: bool,
    pub page_size: Option<usize>, // Para paginación en PDF
    pub freeze_headers: bool,      // Para Excel
    pub auto_filter: bool,         // Para Excel
    pub conditional_formatting: Option<Vec<ConditionalFormat>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalFormat {
    pub field: String,
    pub condition: String,
    pub value: serde_json::Value,
    pub format: FormatStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatStyle {
    pub background_color: Option<String>,
    pub font_color: Option<String>,
    pub bold: Option<bool>,
    pub icon: Option<String>,
}

// Helper module for base64 encoding/decoding
mod base64 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&base64::encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        base64::decode(s).map_err(serde::de::Error::custom)
    }
}