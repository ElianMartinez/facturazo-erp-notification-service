# mr-pdf 📄✨

**mr-pdf** is a high-performance, lightweight PDF generation library for Rust. Designed with a focus on low memory footprint and premium aesthetics, it allows you to create stunning PDF documents with a fluent, developer-friendly API.

## 🚀 Key Features

- **Low Memory Footprint**: Uses a streaming architecture that writes document sections directly to the output. Memory usage scales with the complexity of a single page, not the entire document.
- **Premium Aesthetics**: Built-in support for advanced layouts, charts, and modern typography.
- **Flexible Layout Engine**: Flutter-inspired rows and columns (`Pdf::row`, `Pdf::column`) for complex designs.
- **Interactive Elements**: Support for clickable hyperlinks in text, tables, and images.
- **Rich Text & Inline Style**: Support for **bold**, colors, and mixed styles within table cells and paragraphs.
- **QR Code Generation**: Native vector-based QR code generation (Optional).
- **Watermarks**: Global text watermarks with customizable opacity, angle, and size.
- **Dynamic Charts**: Generate beautiful Bar, Line, and Pie charts directly in your PDF (Optional).

## 📦 Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
mr-pdf = "0.1.0"
```

To enable extra capabilities, use the corresponding features:

```toml
[dependencies]
# Basic installation
mr-pdf = "0.1.4"

# Full installation (Charts + QR Code)
mr-pdf = { version = "0.1.4", features = ["chart", "qrcode"] }
```

## 🛠️ Quick Start

```rust
use mr_pdf::{Pdf, Color, Align, PaperSize};

fn main() -> std::io::Result<()> {
    // Create a new PDF writer (to file or memory)
    let mut pdf = Pdf::memory()?;
    
    // Set Document Metadata
    pdf.set_title("My Awesome Report");
    pdf.set_author("Antigravity");

    // Add Text with Styling
    pdf.text("Hello, mr-pdf!")
        .size(24.0)
        .align_center()
        .color(Color::Rgb(30, 144, 255)) // Dodger Blue
        .margin_bottom(20.0);

    // Dynamic Text Box Margins
    pdf.text("This paragraph is indented from the left edges and wrapped cleanly.")
        .margin_left(50.0)
        .margin_right(50.0)
        .margin_bottom(15.0);

    // Create a Responsive Row
    pdf.row(|r| {
        r.col(50.0.pct(), |pdf| {
            pdf.text("This is the left column.")
               .link("https://github.com");
            Ok(())
        });
        r.col(50.0.pct(), |pdf| {
            pdf.text("This is the right column with a red background.")
               .color(Color::Rgb(255, 0, 0));
            Ok(())
        });
    })?;

    // Finalize
    let _bytes = pdf.finish()?;
    Ok(())
}
```

## 📊 Visual Components

### Beautiful Charts (Opt-in)
Easily visualize data with built-in chart types by enabling the `chart` feature:
- **Bar Charts**: For comparison.
- **Line Charts**: For trends.
- **Pie Charts**: For proportions.

```rust
#[cfg(feature = "chart")]
{
    use mr_pdf::{Chart, ChartType};
    pdf.chart(Chart::new(vec![10.0, 20.0, 30.0])
        .chart_type(ChartType::Pie)
        .labels(vec!["A", "B", "C"])
        .show_values(true));
}
```

### Flexible Image & SVG Sizing
```rust
use mr_pdf::SizeExt;

// SVG with custom positioning and size (supports Points or Percent)
pdf.svg("logo.svg")
    .width(50.0.pct())
    .render()?;

// Images (PNG/JPG) with specific position and size
pdf.image("photo.jpg")
    .position(100.0, 500.0)
    .size(200.0, 150.0)
    .render()?;
```

### Tables with Spanning & Advanced Headers
```rust
use mr_pdf::{TableBorderStyle, Color, SizeExt, Align, VAlign};

pdf.table(|t| {
    t.widths(vec![33.0.pct(), 33.0.pct(), 33.0.pct()]);
    
    // Support for multiple header rows and rowspan via `header_row_builder`
    t.header_row_builder(|r| {
        r.cell("Multi-Row Header").span(2).rowspan(2).align(Align::Center).valign(VAlign::Center);
        r.cell("Col 3 Top");
    });
    t.header_row_builder(|r| {
        r.cell("Col 3 Bottom"); // Only provide remaining cells since span(2) rowspan(2) occupies earlier slots
    });
    
    // Customize Borders
    t.border(TableBorderStyle::Ghost); // Ghost style (no vertical lines)
    
    // Enable Zebra Striping
    t.zebra(Color::Rgb(240, 240, 240)); // Light Gray
    
    // Feature: Rich Text & Individual Cell Styling
    t.row_builder(|r| {
        r.cell("Mix **Bold** and [#FF0000]Color[]").span(2);
        r.cell("Yellow BG")
         .bg_color(Color::Rgb(255, 255, 200))
         .text_color(Color::Rgb(0, 0, 255));
    });

    // Feature: QR Code inside Table
    #[cfg(feature = "qrcode")]
    t.row_builder(|r| {
        r.cell("Scan this:");
        r.cell_qr("https://github.com/supaatwi/mr_pdf").span(2);
    });
})?;
```

### Page Watermarks
Set a global watermark that appears on every page of your document:

```rust
pdf.set_watermark(
    "CONFIDENTIAL", // Text
    60.0,           // Font size
    Color::Rgb(200, 200, 200), // Color
    0.3,            // Opacity
    45.0            // Angle (Degrees)
);
```

### Low-Memory Streaming Tables
If you need to process thousands or even millions of rows from a database or stream, keeping them all in memory is inefficient. The `StreamingTable` API consumes only enough memory for a single row buffer and writes immediately to the PDF!

```rust
let mut builder = mr_pdf::TableBuilder::new();
builder.widths(vec![50.0.pct(), 50.0.pct()])
       .header(vec!["User ID", "Event Type"])
       .repeat_header(true);

let mut stream_table = builder.start(&mut pdf)?; // Renders the header!

while let Some(data) = my_db_rx.recv().await {
    stream_table.row(|r| {
        r.cell(&data.user_id);
        r.cell(&data.event_type);
    })?; // Renders this row and frees memory instantly
}
// Drops map to table end automatically
```

### Automatic Font Subsetting (O(1) Memory Usage)
`mr-pdf` is extremely memory efficient. Behind the scenes, we automatically collect the unique characters you use. When you call `.finish()`, we invoke a subsetting engine (`fontcull`) to slice the raw embedded TTF files perfectly.

This means you can embed large 2MB `ttf` Asian/Arabic files—and if you only type "Hello World", the embedded font size inside the PDF will shrink to just **~15 KB**, reducing your total file size by up to **90%**!

### PDF Security & Password Protection
Keep your documents private with 128-bit encryption. You can set an Owner password to restrict permissions (printing, copying) and a User password to lock the file entirely.

```rust
pdf.set_encryption(
    "admin_pass",   // Owner password
    Some("user123"), // User password
    PdfPermissions {
        can_print: false,
        can_copy: false,
        ..Default::default()
    }
);
```

### Writing Documents in Markdown
You can use standard markdown syntax to rapidly compose large text blocks. MR-PDF automatically maps elements like Headings, Paragraphs, Lists, and Code blocks into beautifully laid-out PDF text blocks!


```rust
let markdown_text = "
# MR-PDF Markdown

This lets you quickly write **bold** and *italic* text concepts!
- Support for Headings (Auto scalable sizes)
- Support for Paragraph wrapping
- Bullet lists

```
fn demo() {}
```
";

pdf.markdown(markdown_text)
   .size(12.0)
   .font("MyRegisteredFont") // Use a specific font for the block
   .render()?;
```


## 🏎️ Performance Objectives

`mr-pdf` is tailored for environments where resource efficiency is paramount:
1. **Direct Writing**: Offsets and object metadata are managed centrally, but content streams are flushed directly to the disk/socket once a page is complete.
2. **Minimal Buffering**: Does not store the entire document DOM in memory.
3. **OptimizedTTF**: Only necessary font metrics are loaded into memory for layout calculation.

## 📄 License

MIT / Apache-2.0
