use crate::{Color, Pdf};
use roxmltree::Document;
use std::fs;
use std::io::Write;

pub struct SvgRenderer;

impl SvgRenderer {
    pub fn render<W: Write>(
        pdf: &mut Pdf<W>,
        path: &str,
        w: Option<f64>,
        h: Option<f64>,
    ) -> std::io::Result<()> {
        let content = fs::read_to_string(path).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("SVG file not found: {}", path),
            )
        })?;
        let doc = Document::parse(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let svg_root = doc.root_element();
        let view_box = svg_root.attribute("viewBox").unwrap_or("0 0 100 100");
        let coords: Vec<f64> = view_box
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();

        let (min_x, min_y, vb_w, vb_h) = if coords.len() >= 4 {
            (coords[0], coords[1], coords[2], coords[3])
        } else {
            (0.0, 0.0, 100.0, 100.0)
        };

        let (display_w, display_h) = match (w, h) {
            (Some(ww), Some(hh)) => (ww, hh),
            (Some(ww), None) => (ww, vb_h * (ww / vb_w)),
            (None, Some(hh)) => (vb_w * (hh / vb_h), hh),
            (None, None) => (100.0, vb_h * (100.0 / vb_w)),
        };

        let scale = display_w / vb_w;
        let (start_x, start_y) = pdf.cursor_pos();
        pdf.advance_cursor(display_h + 10.0);
        let base_y = start_y;

        for node in doc.descendants() {
            if node.is_element() {
                let tag = node.tag_name().name();
                if tag == "svg" || tag == "g" || tag == "defs" {
                    continue;
                }

                let fill = find_attribute(node, "fill").unwrap_or("#000000");
                let stroke = find_attribute(node, "stroke").unwrap_or("none");
                let stroke_width = find_attribute(node, "stroke-width")
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(1.0);

                if fill == "none" && stroke == "none" {
                    continue;
                }

                match tag {
                    "rect" => {
                        let rx = (node.attribute("x").unwrap_or("0").parse().unwrap_or(0.0)
                            - min_x)
                            * scale;
                        let ry = (node.attribute("y").unwrap_or("0").parse().unwrap_or(0.0)
                            - min_y)
                            * scale;
                        let rw = node
                            .attribute("width")
                            .unwrap_or("0")
                            .parse()
                            .unwrap_or(0.0)
                            * scale;
                        let rh = node
                            .attribute("height")
                            .unwrap_or("0")
                            .parse()
                            .unwrap_or(0.0)
                            * scale;

                        pdf.ensure_page()?;
                        if fill != "none" {
                            pdf.set_fill_color(parse_color(fill))?;
                        }
                        if stroke != "none" {
                            pdf.set_stroke_color(parse_color(stroke))?;
                            pdf.set_line_width(stroke_width * scale)?;
                        }

                        let stream = pdf.get_stream();
                        stream.push_str("q\n");
                        stream.push_str(&format!(
                            "{:.2} {:.2} {:.2} {:.2} re ",
                            start_x + rx,
                            base_y - ry - rh,
                            rw,
                            rh
                        ));
                        let op = match (fill != "none", stroke != "none") {
                            (true, true) => "B\n",
                            (true, false) => "f\n",
                            (false, true) => "S\n",
                            _ => "n\n",
                        };
                        stream.push_str(op);
                        stream.push_str("Q\n");
                    }
                    "circle" => {
                        let cx = (node.attribute("cx").unwrap_or("0").parse().unwrap_or(0.0)
                            - min_x)
                            * scale;
                        let cy = (node.attribute("cy").unwrap_or("0").parse().unwrap_or(0.0)
                            - min_y)
                            * scale;
                        let r = node.attribute("r").unwrap_or("0").parse().unwrap_or(0.0) * scale;

                        pdf.ensure_page()?;
                        if fill != "none" {
                            pdf.set_fill_color(parse_color(fill))?;
                        }
                        if stroke != "none" {
                            pdf.set_stroke_color(parse_color(stroke))?;
                            pdf.set_line_width(stroke_width * scale)?;
                        }

                        pdf.get_stream().push_str("q\n");
                        let op = match (fill != "none", stroke != "none") {
                            (true, true) => "B",
                            (true, false) => "f",
                            (false, true) => "S",
                            _ => "n",
                        };
                        pdf.draw_circle_op(start_x + cx, base_y - cy, r, op)?;
                        pdf.get_stream().push_str("Q\n");
                    }
                    "path" => {
                        if let Some(d) = node.attribute("d") {
                            pdf.ensure_page()?;
                            if fill != "none" {
                                pdf.set_fill_color(parse_color(fill))?;
                            }
                            if stroke != "none" {
                                pdf.set_stroke_color(parse_color(stroke))?;
                                pdf.set_line_width(stroke_width * scale)?;
                            }

                            pdf.get_stream().push_str("q\n");
                            let op = match (fill != "none", stroke != "none") {
                                (true, true) => "B",
                                (true, false) => "f",
                                (false, true) => "S",
                                _ => "n",
                            };
                            render_path(pdf, d, start_x, base_y, scale, min_x, min_y, op)?;
                            pdf.get_stream().push_str("Q\n");
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

fn find_attribute<'a>(node: roxmltree::Node<'a, 'a>, name: &str) -> Option<&'a str> {
    if let Some(val) = node.attribute(name) {
        return Some(val);
    }
    // Check style
    if let Some(style) = node.attribute("style") {
        for part in style.split(';') {
            let mut kv = part.split(':');
            if let Some(k) = kv.next() {
                if k.trim() == name {
                    return kv.next().map(|v| v.trim());
                }
            }
        }
    }
    // Inheritance
    node.parent().and_then(|p| {
        if p.is_element() {
            find_attribute(p, name)
        } else {
            None
        }
    })
}

fn parse_color(hex: &str) -> Color {
    let hex = hex.trim();
    if hex.starts_with('#') {
        let h = hex.trim_start_matches('#');
        if h.len() == 6 {
            let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0);
            return Color::Rgb(r, g, b);
        } else if h.len() == 3 {
            let r = u8::from_str_radix(&h[0..1], 16).unwrap_or(0) * 17;
            let g = u8::from_str_radix(&h[1..2], 16).unwrap_or(0) * 17;
            let b = u8::from_str_radix(&h[2..3], 16).unwrap_or(0) * 17;
            return Color::Rgb(r, g, b);
        }
    } else if hex == "red" {
        return Color::Rgb(255, 0, 0);
    } else if hex == "green" {
        return Color::Rgb(0, 255, 0);
    } else if hex == "blue" {
        return Color::Rgb(0, 0, 255);
    } else if hex == "white" {
        return Color::Rgb(255, 255, 255);
    } else if hex == "black" {
        return Color::Rgb(0, 0, 0);
    }

    Color::Rgb(0, 0, 0)
}

fn render_path<W: Write>(
    pdf: &mut Pdf<W>,
    d: &str,
    offset_x: f64,
    offset_y: f64,
    scale: f64,
    min_x: f64,
    min_y: f64,
    op: &str,
) -> std::io::Result<()> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    // Robust tokenization
    let chars: Vec<char> = d.chars().collect();
    let mut j = 0;
    while j < chars.len() {
        let c = chars[j];
        if c.is_alphabetic() && c != 'e' && c != 'E' {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            tokens.push(c.to_string());
        } else if c.is_whitespace() || c == ',' {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else if c == '-' {
            let prev_is_e =
                !current.is_empty() && (current.ends_with('e') || current.ends_with('E'));
            if !current.is_empty() && !prev_is_e {
                tokens.push(current.clone());
                current = "-".to_string();
            } else {
                current.push('-');
            }
        } else {
            current.push(c);
        }
        j += 1;
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    let stream = pdf.get_stream();
    let mut i = 0;
    let mut last_x = 0.0;
    let mut last_y = 0.0;
    let mut last_cp_x = 0.0;
    let mut last_cp_y = 0.0;
    let mut subpath_start_x = 0.0;
    let mut subpath_start_y = 0.0;
    let mut current_cmd = ' ';
    let mut has_path = false;

    while i < tokens.len() {
        let token = &tokens[i];
        if token.len() == 1 && token.chars().next().unwrap().is_alphabetic() {
            current_cmd = token.chars().next().unwrap();
            i += 1;
        }

        match current_cmd {
            'M' | 'm' => {
                let is_rel = current_cmd == 'm';
                let x = tokens[i].parse::<f64>().unwrap_or(0.0);
                let y = tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                let (nx, ny) = if is_rel {
                    (last_x + x, last_y + y)
                } else {
                    (x, y)
                };
                let px = (nx - min_x) * scale;
                let py = (ny - min_y) * scale;
                stream.push_str(&format!("{:.2} {:.2} m\n", offset_x + px, offset_y - py));
                last_x = nx;
                last_y = ny;
                subpath_start_x = nx;
                subpath_start_y = ny;
                last_cp_x = nx;
                last_cp_y = ny;
                i += 2;
                has_path = true;
                current_cmd = if is_rel { 'l' } else { 'L' };
            }
            'L' | 'l' => {
                let is_rel = current_cmd == 'l';
                let x = tokens[i].parse::<f64>().unwrap_or(0.0);
                let y = tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                let (nx, ny) = if is_rel {
                    (last_x + x, last_y + y)
                } else {
                    (x, y)
                };
                let px = (nx - min_x) * scale;
                let py = (ny - min_y) * scale;
                stream.push_str(&format!("{:.2} {:.2} l\n", offset_x + px, offset_y - py));
                last_x = nx;
                last_y = ny;
                last_cp_x = nx;
                last_cp_y = ny;
                i += 2;
            }
            'H' | 'h' => {
                let x = tokens[i].parse::<f64>().unwrap_or(0.0);
                let nx = if current_cmd == 'h' { last_x + x } else { x };
                let px = (nx - min_x) * scale;
                let py = (last_y - min_y) * scale;
                stream.push_str(&format!("{:.2} {:.2} l\n", offset_x + px, offset_y - py));
                last_x = nx;
                last_cp_x = nx;
                last_cp_y = last_y;
                i += 1;
            }
            'V' | 'v' => {
                let y = tokens[i].parse::<f64>().unwrap_or(0.0);
                let ny = if current_cmd == 'v' { last_y + y } else { y };
                let px = (last_x - min_x) * scale;
                let py = (ny - min_y) * scale;
                stream.push_str(&format!("{:.2} {:.2} l\n", offset_x + px, offset_y - py));
                last_y = ny;
                last_cp_x = last_x;
                last_cp_y = ny;
                i += 1;
            }
            'C' | 'c' => {
                let is_rel = current_cmd == 'c';
                let c1x = tokens[i].parse::<f64>().unwrap_or(0.0);
                let c1y = tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                let c2x = tokens[i + 2].parse::<f64>().unwrap_or(0.0);
                let c2y = tokens[i + 3].parse::<f64>().unwrap_or(0.0);
                let ex = tokens[i + 4].parse::<f64>().unwrap_or(0.0);
                let ey = tokens[i + 5].parse::<f64>().unwrap_or(0.0);
                let (p1x, p1y, p2x, p2y, nx, ny) = if is_rel {
                    (
                        last_x + c1x,
                        last_y + c1y,
                        last_x + c2x,
                        last_y + c2y,
                        last_x + ex,
                        last_y + ey,
                    )
                } else {
                    (c1x, c1y, c2x, c2y, ex, ey)
                };
                let sx = (p1x - min_x) * scale;
                let sy = (p1y - min_y) * scale;
                let tx = (p2x - min_x) * scale;
                let ty = (p2y - min_y) * scale;
                let ux = (nx - min_x) * scale;
                let uy = (ny - min_y) * scale;
                stream.push_str(&format!(
                    "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
                    offset_x + sx,
                    offset_y - sy,
                    offset_x + tx,
                    offset_y - ty,
                    offset_x + ux,
                    offset_y - uy
                ));
                last_x = nx;
                last_y = ny;
                last_cp_x = p2x;
                last_cp_y = p2y;
                i += 6;
            }
            'S' | 's' => {
                let is_rel = current_cmd == 's';
                let c2x = tokens[i].parse::<f64>().unwrap_or(0.0);
                let c2y = tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                let ex = tokens[i + 2].parse::<f64>().unwrap_or(0.0);
                let ey = tokens[i + 3].parse::<f64>().unwrap_or(0.0);
                let (p2x, p2y, nx, ny) = if is_rel {
                    (last_x + c2x, last_y + c2y, last_x + ex, last_y + ey)
                } else {
                    (c2x, c2y, ex, ey)
                };
                let p1x = 2.0 * last_x - last_cp_x;
                let p1y = 2.0 * last_y - last_cp_y;
                let sx = (p1x - min_x) * scale;
                let sy = (p1y - min_y) * scale;
                let tx = (p2x - min_x) * scale;
                let ty = (p2y - min_y) * scale;
                let ux = (nx - min_x) * scale;
                let uy = (ny - min_y) * scale;
                stream.push_str(&format!(
                    "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
                    offset_x + sx,
                    offset_y - sy,
                    offset_x + tx,
                    offset_y - ty,
                    offset_x + ux,
                    offset_y - uy
                ));
                last_x = nx;
                last_y = ny;
                last_cp_x = p2x;
                last_cp_y = p2y;
                i += 4;
            }
            'Q' | 'q' => {
                let is_rel = current_cmd == 'q';
                let c1x = tokens[i].parse::<f64>().unwrap_or(0.0);
                let c1y = tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                let ex = tokens[i + 2].parse::<f64>().unwrap_or(0.0);
                let ey = tokens[i + 3].parse::<f64>().unwrap_or(0.0);
                let (cx, cy, nx, ny) = if is_rel {
                    (last_x + c1x, last_y + c1y, last_x + ex, last_y + ey)
                } else {
                    (c1x, c1y, ex, ey)
                };
                let qp1x = last_x + 2.0 / 3.0 * (cx - last_x);
                let qp1y = last_y + 2.0 / 3.0 * (cy - last_y);
                let qp2x = nx + 2.0 / 3.0 * (cx - nx);
                let qp2y = ny + 2.0 / 3.0 * (cy - ny);
                let p1x = (qp1x - min_x) * scale;
                let p1y = (qp1y - min_y) * scale;
                let p2x = (qp2x - min_x) * scale;
                let p2y = (qp2y - min_y) * scale;
                let ux = (nx - min_x) * scale;
                let uy = (ny - min_y) * scale;
                stream.push_str(&format!(
                    "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
                    offset_x + p1x,
                    offset_y - p1y,
                    offset_x + p2x,
                    offset_y - p2y,
                    offset_x + ux,
                    offset_y - uy
                ));
                last_x = nx;
                last_y = ny;
                last_cp_x = cx;
                last_cp_y = cy;
                i += 4;
            }
            'Z' | 'z' => {
                stream.push_str("h\n");
                last_x = subpath_start_x;
                last_y = subpath_start_y;
                last_cp_x = last_x;
                last_cp_y = last_y;
                current_cmd = ' ';
            }
            'A' | 'a' => {
                let is_rel = current_cmd == 'a';
                let rx = tokens[i].parse::<f64>().unwrap_or(0.0).abs();
                let ry = tokens[i + 1].parse::<f64>().unwrap_or(0.0).abs();
                let ex = tokens[i + 5].parse::<f64>().unwrap_or(0.0);
                let ey = tokens[i + 6].parse::<f64>().unwrap_or(0.0);
                let (nx, ny) = if is_rel {
                    (last_x + ex, last_y + ey)
                } else {
                    (ex, ey)
                };

                // Smart approximation: midpoint of arc if it's a large change
                let mid_x = (last_x + nx) / 2.0;
                let mut mid_y = (last_y + ny) / 2.0;

                if rx > 0.0 && ry > 0.0 {
                    let dx = (nx - last_x).abs();
                    if dx < rx * 2.0 {
                        let offset = (rx * rx - (dx / 2.0) * (dx / 2.0)).max(0.0).sqrt();
                        mid_y = if ey > 0.0 {
                            mid_y + offset
                        } else {
                            mid_y - offset
                        };
                    }
                }

                let px1 = (mid_x - min_x) * scale;
                let py1 = (mid_y - min_y) * scale;
                let px2 = (nx - min_x) * scale;
                let py2 = (ny - min_y) * scale;
                stream.push_str(&format!(
                    "{:.2} {:.2} l\n{:.2} {:.2} l\n",
                    offset_x + px1,
                    offset_y - py1,
                    offset_x + px2,
                    offset_y - py2
                ));

                last_x = nx;
                last_y = ny;
                last_cp_x = nx;
                last_cp_y = ny;
                i += 7;
            }
            _ => {
                i += 1;
            }
        }
    }
    if has_path {
        stream.push_str(&format!(" {}\n", op));
    }
    Ok(())
}
