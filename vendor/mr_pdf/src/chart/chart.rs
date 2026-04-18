use crate::{Align, Color, Size};

/// Supported chart types for visual data representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartType {
    /// Vertical bar chart.
    Bar,
    /// Line plot connecting data points.
    Line,
    /// Circular pie chart.
    Pie,
}

/// A configuration object for rendering charts in the PDF.
#[derive(Debug, Clone)]
pub struct Chart {
    pub chart_type: ChartType,
    pub data: Vec<f64>,
    pub labels: Option<Vec<String>>,
    pub width: Size,
    pub height: Size,
    pub color: Option<Color>,
    pub align: Align,
    pub show_values: bool,
}

impl Chart {
    /// Creates a new Bar chart with the provided data.
    pub fn new(data: Vec<f64>) -> Self {
        Self {
            chart_type: ChartType::Bar,
            data,
            labels: None,
            width: Size::Points(300.0),
            height: Size::Points(200.0),
            color: None,
            align: Align::Left,
            show_values: false,
        }
    }

    /// Toggles the display of numerical values on the chart.
    pub fn show_values(mut self, show: bool) -> Self {
        self.show_values = show;
        self
    }

    /// Sets the chart type (Bar, Line, or Pie).
    pub fn chart_type(mut self, t: ChartType) -> Self {
        self.chart_type = t;
        self
    }

    /// Sets the display width of the chart.
    pub fn width(mut self, w: impl Into<Size>) -> Self {
        self.width = w.into();
        self
    }

    /// Sets the display height of the chart.
    pub fn height(mut self, h: impl Into<Size>) -> Self {
        self.height = h.into();
        self
    }

    /// Assigns labels to the data categories.
    pub fn labels(mut self, l: Vec<&str>) -> Self {
        self.labels = Some(l.into_iter().map(|s| s.to_string()).collect());
        self
    }

    /// Sets the primary theme color for the chart.
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }

    /// Sets the horizontal alignment of the chart on the page.
    pub fn align(mut self, a: Align) -> Self {
        self.align = a;
        self
    }
}
