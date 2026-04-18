use crate::font::font::Font;
use crate::pdf::writer::PdfWriter;
use std::io::Write;
use ttf_parser::Face;

/// Low-level function to embed a TTF font into the PDF document.
/// It creates several PDF objects: FontFile2, FontDescriptor, CIDFont, and Type0.
pub fn embed_ttf<W: Write>(writer: &mut PdfWriter<W>, font: &Font) -> std::io::Result<u32> {
    let raw_bytes = font.raw_data.clone();

    // Subsetting is disabled for now to maintain Glyph ID stability in streaming mode.
    // This fixes the "garbled text" issue where GIDs were shifted during subsetting.

    let face = Face::parse(&raw_bytes, 0)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid TTF"))?;

    let font_file2_id = writer.alloc_id();
    let cid_font_id = writer.alloc_id();
    let font_descriptor_id = writer.alloc_id();
    let type0_id = writer.alloc_id();

    writer.start_obj(font_file2_id)?;
    let stream_header = format!(
        "<< /Length {} /Length1 {} >>\nstream\n",
        raw_bytes.len(),
        raw_bytes.len()
    );
    writer.write_raw(stream_header.as_bytes())?;
    writer.write_raw(&raw_bytes)?;
    writer.write_raw(b"\nendstream\n")?;
    writer.end_obj()?;

    let ascent = face.ascender();
    let descent = face.descender();
    let bbox = face.global_bounding_box();
    let cap_height = face.capital_height().unwrap_or(ascent);
    let italic_angle = face.italic_angle();
    let flags = 32;

    let upe = face.units_per_em() as f64;
    let scale = 1000.0 / upe;

    writer.start_obj(font_descriptor_id)?;
    let desc = format!(
        "<< /Type /FontDescriptor\n   /FontName /{}\n   /Flags {}\n   /FontBBox [{:.0} {:.0} {:.0} {:.0}]\n   /ItalicAngle {:.0}\n   /Ascent {:.0}\n   /Descent {:.0}\n   /CapHeight {:.0}\n   /StemV 80\n   /FontFile2 {} 0 R\n>>\n",
        font.name,
        flags,
        bbox.x_min as f64 * scale,
        bbox.y_min as f64 * scale,
        bbox.x_max as f64 * scale,
        bbox.y_max as f64 * scale,
        italic_angle as f64,
        ascent as f64 * scale,
        descent as f64 * scale,
        cap_height as f64 * scale,
        font_file2_id
    );
    writer.write_raw(desc.as_bytes())?;
    writer.end_obj()?;

    let num_glyphs = face.number_of_glyphs();
    let mut widths_str = String::new();
    for i in 0..num_glyphs {
        let w = face.glyph_hor_advance(ttf_parser::GlyphId(i)).unwrap_or(0) as f64 * scale;
        widths_str.push_str(&format!("{:.0} ", w));
    }

    writer.start_obj(cid_font_id)?;
    let cid_dict = format!(
        "<< /Type /Font\n   /Subtype /CIDFontType2\n   /BaseFont /{}\n   /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >>\n   /FontDescriptor {} 0 R\n   /W [ 0 [ {} ] ]\n   /CIDToGIDMap /Identity\n>>\n",
        font.name, font_descriptor_id, widths_str
    );
    writer.write_raw(cid_dict.as_bytes())?;
    writer.end_obj()?;

    writer.start_obj(type0_id)?;
    let type0_dict = format!(
        "<< /Type /Font\n   /Subtype /Type0\n   /BaseFont /{}\n   /Encoding /Identity-H\n   /DescendantFonts [ {} 0 R ]\n>>\n",
        font.name, cid_font_id
    );
    writer.write_raw(type0_dict.as_bytes())?;
    writer.end_obj()?;

    Ok(type0_id)
}
