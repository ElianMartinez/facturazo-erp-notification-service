use crate::Metadata;
use crate::font::FontManager;
use crate::layout::text::escape_pdf_str;
use crate::pdf::crypto::{PdfPermissions, SecurityHandler};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use std::io::Write;

pub struct LinkAnnotation {
    pub rect: (f64, f64, f64, f64),
    pub url: String,
}

/// Low-level PDF writer that handles object generation, cross-references, and streaming.
pub struct PdfWriter<W: Write> {
    pub writer: W,
    offsets: Vec<u64>,
    pages: Vec<u32>,
    pub next_object_id: u32,
    pos: u64,
    catalog_id: u32,
    pages_id: u32,
    resources_id: u32,
    info_id: u32,
    builtin_font_id: u32,
    xobjects: Vec<u32>,
    page_annots: Vec<(u32, Vec<LinkAnnotation>)>,
    pub compress: bool,
    shadings: Vec<(u32, [f64; 3], [f64; 3], [f64; 4])>,
    pub security: Option<SecurityHandler>,
    pub doc_id: [u8; 16],
    encrypt_id: u32,
}

impl<W: Write> PdfWriter<W> {
    pub fn new(writer: W) -> std::io::Result<Self> {
        let mut slf = Self {
            writer,
            offsets: vec![0],
            pages: Vec::new(),
            next_object_id: 1,
            pos: 0,
            catalog_id: 0,
            pages_id: 0,
            resources_id: 0,
            info_id: 0,
            builtin_font_id: 0,
            xobjects: Vec::new(),
            page_annots: Vec::new(),
            compress: true,
            shadings: Vec::new(),
            security: None,
            // Simple generic document ID for demo
            doc_id: [
                0x4d, 0x52, 0x2d, 0x50, 0x44, 0x46, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                0x09, 0x0A,
            ],
            encrypt_id: 0,
        };
        slf.write_raw(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n")?;

        slf.catalog_id = slf.alloc_id();
        slf.pages_id = slf.alloc_id();
        slf.resources_id = slf.alloc_id();
        slf.info_id = slf.alloc_id();
        slf.builtin_font_id = slf.alloc_id();
        slf.encrypt_id = slf.alloc_id();

        let bfid = slf.builtin_font_id;
        slf.start_obj(bfid)?;
        slf.write_raw(
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\n",
        )?;
        slf.end_obj()?;

        Ok(slf)
    }

    pub fn enable_encryption(&mut self, owner: &str, user: &str, perms: PdfPermissions) {
        let handler = SecurityHandler::new(owner, user, perms, &self.doc_id);
        self.security = Some(handler);
    }

    pub fn encrypt_string(&self, obj_id: u32, text: &str) -> String {
        let mut data = text.as_bytes().to_vec();
        if let Some(sec) = &self.security {
            sec.encrypt_bytes(obj_id, 0, &mut data);
        }
        let mut hex = String::with_capacity(data.len() * 2 + 2);
        hex.push('<');
        for b in data {
            hex.push_str(&format!("{:02X}", b));
        }
        hex.push('>');
        hex
    }

    pub fn write_raw(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(data)?;
        self.pos += data.len() as u64;
        Ok(())
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_object_id;
        self.next_object_id += 1;
        self.offsets.push(0);
        id
    }

    pub fn start_obj(&mut self, id: u32) -> std::io::Result<()> {
        self.offsets[id as usize] = self.pos;
        let s = format!("{} 0 obj\n", id);
        self.write_raw(s.as_bytes())?;
        Ok(())
    }

    pub fn end_obj(&mut self) -> std::io::Result<()> {
        self.write_raw(b"endobj\n")?;
        Ok(())
    }

    /// Adds a new page dictionary to the document.
    pub fn add_page(&mut self, width: f64, height: f64) -> std::io::Result<(u32, u32)> {
        let page_id = self.alloc_id();
        let content_id = self.alloc_id();
        let annots_id = self.alloc_id();
        self.pages.push(page_id);
        self.page_annots.push((annots_id, Vec::new()));

        self.start_obj(page_id)?;
        let s = format!(
            "<< /Type /Page\n   /Parent {} 0 R\n   /Resources {} 0 R\n   /MediaBox [0 0 {:.2} {:.2}]\n   /Contents {} 0 R\n   /Annots {} 0 R\n>>\n",
            self.pages_id, self.resources_id, width, height, content_id, annots_id
        );
        self.write_raw(s.as_bytes())?;
        self.end_obj()?;

        Ok((page_id, content_id))
    }

    /// Adds a hyperlink to the current page.
    pub fn add_link(&mut self, rect: (f64, f64, f64, f64), url: &str) {
        if let Some((_, annots)) = self.page_annots.last_mut() {
            annots.push(LinkAnnotation {
                rect,
                url: url.to_string(),
            });
        }
    }

    /// Appends a content stream to a specific object.
    pub fn write_content_stream(&mut self, content_id: u32, content: &str) -> std::io::Result<()> {
        self.start_obj(content_id)?;

        let mut final_data: Vec<u8> = if self.compress {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(content.as_bytes())?;
            encoder.finish()?
        } else {
            content.as_bytes().to_vec()
        };

        if let Some(sec) = &self.security {
            sec.encrypt_bytes(content_id, 0, &mut final_data);
        }

        if self.compress {
            let s = format!(
                "<< /Length {} /Filter /FlateDecode >>\nstream\n",
                final_data.len()
            );
            self.write_raw(s.as_bytes())?;
        } else {
            let s = format!("<< /Length {} >>\nstream\n", final_data.len());
            self.write_raw(s.as_bytes())?;
        }
        self.write_raw(&final_data)?;
        self.write_raw(b"\nendstream\n")?;
        self.end_obj()?;
        Ok(())
    }

    pub fn register_xobject(&mut self, xobj_id: u32) {
        self.xobjects.push(xobj_id);
    }

    pub fn register_shading(&mut self, c1: [f64; 3], c2: [f64; 3], coords: [f64; 4]) -> u32 {
        let id = self.alloc_id();
        self.shadings.push((id, c1, c2, coords));
        id
    }

    /// Finalizes the PDF by writing cross-reference tables and trailers.
    pub fn finish(
        mut self,
        font_manager: &mut FontManager,
        metadata: &Metadata,
    ) -> std::io::Result<W> {
        let font_resources = font_manager.embed_fonts(&mut self)?;

        let annots_to_write = std::mem::take(&mut self.page_annots);
        for (annots_id, links) in annots_to_write {
            let mut link_ids = Vec::new();
            for link in links {
                let id = self.alloc_id();
                link_ids.push(id);
                self.start_obj(id)?;
                let esc_url = escape_pdf_str(&link.url);
                let s = format!(
                    "<< /Type /Annot /Subtype /Link /Rect [{:.2} {:.2} {:.2} {:.2}] /Border [0 0 0] /A << /Type /Action /S /URI /URI ({}) >> >>\n",
                    link.rect.0, link.rect.1, link.rect.2, link.rect.3, esc_url
                );
                self.write_raw(s.as_bytes())?;
                self.end_obj()?;
            }

            self.start_obj(annots_id)?;
            let mut s = String::from("[ ");
            for lid in link_ids {
                s.push_str(&format!("{} 0 R ", lid));
            }
            s.push_str("]\n");
            self.write_raw(s.as_bytes())?;
            self.end_obj()?;
        }

        self.start_obj(self.resources_id)?;
        self.write_raw(b"<< ")?;
        let bfid = self.builtin_font_id;
        let s = format!(
            "/Font << /FBuiltin {} 0 R {} >> ",
            bfid,
            if font_resources.is_empty() {
                String::new()
            } else {
                font_resources
            }
        );
        self.write_raw(s.as_bytes())?;
        let xobjects = self.xobjects.clone();
        if !xobjects.is_empty() {
            self.write_raw(b"/XObject << ")?;
            for xobj_id in &xobjects {
                self.write_raw(format!("/Im{} {} 0 R ", xobj_id, xobj_id).as_bytes())?;
            }
            self.write_raw(b">> ")?;
        }
        if !self.shadings.is_empty() {
            self.write_raw(b"/Shading << ")?;
            let sh_ids: Vec<u32> = self.shadings.iter().map(|s| s.0).collect();
            for id in sh_ids {
                self.write_raw(format!("/Sh{} {} 0 R ", id, id).as_bytes())?;
            }
            self.write_raw(b">> ")?;
        }
        self.write_raw(b">>\n")?;
        self.end_obj()?;

        let shadings = self.shadings.clone();
        for (sh_id, c1, c2, coords) in shadings {
            let func_id = self.alloc_id();
            self.start_obj(func_id)?;
            let s = format!(
                "<< /FunctionType 2 /Domain [0 1] /C0 [{:.3} {:.3} {:.3}] /C1 [{:.3} {:.3} {:.3}] /N 1 >>\n",
                c1[0], c1[1], c1[2], c2[0], c2[1], c2[2]
            );
            self.write_raw(s.as_bytes())?;
            self.end_obj()?;

            self.start_obj(sh_id)?;
            let s = format!(
                "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [{:.2} {:.2} {:.2} {:.2}] /Function {} 0 R /Extend [true true] >>\n",
                coords[0], coords[1], coords[2], coords[3], func_id
            );
            self.write_raw(s.as_bytes())?;
            self.end_obj()?;
        }

        self.start_obj(self.pages_id)?;
        let mut kids = String::new();
        for pid in &self.pages {
            kids.push_str(&format!("{} 0 R ", pid));
        }
        let s = format!(
            "<< /Type /Pages /Count {} /Kids [{}] >>\n",
            self.pages.len(),
            kids
        );
        self.write_raw(s.as_bytes())?;
        self.end_obj()?;

        self.start_obj(self.catalog_id)?;
        let s = format!("<< /Type /Catalog /Pages {} 0 R >>\n", self.pages_id);
        self.write_raw(s.as_bytes())?;
        self.end_obj()?;

        if let Some(sec) = &self.security {
            let mut o_hex = String::new();
            for b in &sec.o {
                o_hex.push_str(&format!("{:02X}", b));
            }
            let mut u_hex = String::new();
            for b in &sec.u {
                u_hex.push_str(&format!("{:02X}", b));
            }
            let p_val = sec.p;

            self.start_obj(self.encrypt_id)?;
            let s = format!(
                "<< /Filter /Standard /V 2 /R 3 /Length 128 /O <{}> /U <{}> /P {} >>\n",
                o_hex, u_hex, p_val
            );
            self.write_raw(s.as_bytes())?;
            self.end_obj()?;
        }

        let xref_pos = self.pos;
        let s = format!("xref\n0 {}\n", self.offsets.len());
        self.write_raw(s.as_bytes())?;
        self.write_raw(b"0000000000 65535 f \n")?;
        for i in 1..self.offsets.len() {
            let s = format!("{:010} 00000 n \n", self.offsets[i]);
            self.write_raw(s.as_bytes())?;
        }

        self.start_obj(self.info_id)?;
        self.write_raw(b"<< ")?;
        if let Some(t) = &metadata.title {
            let v = if self.security.is_some() {
                self.encrypt_string(self.info_id, t)
            } else {
                format!("({})", escape_pdf_str(t))
            };
            self.write_raw(format!("/Title {} ", v).as_bytes())?;
        }
        if let Some(a) = &metadata.author {
            let v = if self.security.is_some() {
                self.encrypt_string(self.info_id, a)
            } else {
                format!("({})", escape_pdf_str(a))
            };
            self.write_raw(format!("/Author {} ", v).as_bytes())?;
        }
        if let Some(s) = &metadata.subject {
            let v = if self.security.is_some() {
                self.encrypt_string(self.info_id, s)
            } else {
                format!("({})", escape_pdf_str(s))
            };
            self.write_raw(format!("/Subject {} ", v).as_bytes())?;
        }
        self.write_raw(b">>\n")?;
        self.end_obj()?;

        let mut doc_id_hex = String::new();
        for b in &self.doc_id {
            doc_id_hex.push_str(&format!("{:02X}", b));
        }

        let encrypt_ext = if self.security.is_some() {
            format!(
                "/Encrypt {} 0 R /ID [ <{}> <{}> ] ",
                self.encrypt_id, doc_id_hex, doc_id_hex
            )
        } else {
            String::new()
        };

        let s = format!(
            "trailer\n<< /Size {} /Root {} 0 R /Info {} 0 R {}>>\nstartxref\n{}\n%%EOF\n",
            self.offsets.len(),
            self.catalog_id,
            self.info_id,
            encrypt_ext,
            xref_pos
        );
        self.write_raw(s.as_bytes())?;

        Ok(self.writer)
    }
}
