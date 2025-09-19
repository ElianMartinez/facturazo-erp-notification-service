use std::fmt;

#[derive(Debug, Clone)]
pub enum PageSize {
    A4,
    Letter,
    Legal,
    A3,
    Custom(f32, f32), // width, height in mm
}

impl PageSize {
    pub fn to_typst(&self) -> String {
        match self {
            PageSize::A4 => "\"a4\"".to_string(),
            PageSize::Letter => "\"us-letter\"".to_string(),
            PageSize::Legal => "\"us-legal\"".to_string(),
            PageSize::A3 => "\"a3\"".to_string(),
            PageSize::Custom(w, h) => format!("(width: {}mm, height: {}mm)", w, h),
        }
    }

    pub fn dimensions(&self) -> (f32, f32) {
        match self {
            PageSize::A4 => (210.0, 297.0),
            PageSize::Letter => (215.9, 279.4),
            PageSize::Legal => (215.9, 355.6),
            PageSize::A3 => (297.0, 420.0),
            PageSize::Custom(w, h) => (*w, *h),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Orientation {
    Portrait,
    Landscape,
}

impl fmt::Display for Orientation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Orientation::Portrait => write!(f, "portrait"),
            Orientation::Landscape => write!(f, "landscape"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Margin {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

impl Default for Margin {
    fn default() -> Self {
        Margin {
            top: 20.0,
            bottom: 20.0,
            left: 20.0,
            right: 20.0,
        }
    }
}

impl Margin {
    pub fn new(top: f32, bottom: f32, left: f32, right: f32) -> Self {
        Margin { top, bottom, left, right }
    }

    pub fn uniform(size: f32) -> Self {
        Margin {
            top: size,
            bottom: size,
            left: size,
            right: size,
        }
    }

    pub fn to_typst(&self) -> String {
        format!(
            "(top: {}mm, bottom: {}mm, left: {}mm, right: {}mm)",
            self.top, self.bottom, self.left, self.right
        )
    }
}

#[derive(Debug, Clone)]
pub struct PdfConfig {
    pub page_size: PageSize,
    pub orientation: Orientation,
    pub margin: Margin,
    pub scale: f32,
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
}

impl Default for PdfConfig {
    fn default() -> Self {
        PdfConfig {
            page_size: PageSize::A4,
            orientation: Orientation::Portrait,
            margin: Margin::default(),
            scale: 1.0,
            font_family: "Helvetica".to_string(),
            font_size: 11.0,
            line_height: 1.5,
        }
    }
}

impl PdfConfig {
    pub fn builder() -> PdfConfigBuilder {
        PdfConfigBuilder::default()
    }

    pub fn to_typst_header(&self) -> String {
        format!(
            r#"#set page(
  paper: {},
  margin: {},
  flipped: {}
)
#set text(
  font: "{}",
  size: {}pt,
  spacing: {}em
)"#,
            self.page_size.to_typst(),
            self.margin.to_typst(),
            matches!(self.orientation, Orientation::Landscape),
            self.font_family,
            self.font_size * self.scale,
            self.line_height
        )
    }
}

#[derive(Default)]
pub struct PdfConfigBuilder {
    page_size: Option<PageSize>,
    orientation: Option<Orientation>,
    margin: Option<Margin>,
    scale: Option<f32>,
    font_family: Option<String>,
    font_size: Option<f32>,
    line_height: Option<f32>,
}

impl PdfConfigBuilder {
    pub fn page_size(mut self, size: PageSize) -> Self {
        self.page_size = Some(size);
        self
    }

    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    pub fn margin(mut self, margin: Margin) -> Self {
        self.margin = Some(margin);
        self
    }

    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = Some(scale);
        self
    }

    pub fn font_family(mut self, font: String) -> Self {
        self.font_family = Some(font);
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size);
        self
    }

    pub fn build(self) -> PdfConfig {
        let default = PdfConfig::default();
        PdfConfig {
            page_size: self.page_size.unwrap_or(default.page_size),
            orientation: self.orientation.unwrap_or(default.orientation),
            margin: self.margin.unwrap_or(default.margin),
            scale: self.scale.unwrap_or(default.scale),
            font_family: self.font_family.unwrap_or(default.font_family),
            font_size: self.font_size.unwrap_or(default.font_size),
            line_height: self.line_height.unwrap_or(default.line_height),
        }
    }
}