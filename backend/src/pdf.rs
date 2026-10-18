//! A tiny, dependency-free PDF writer for tabular timetables.
//!
//! Produces a valid single- or multi-page PDF using the built-in Helvetica
//! fonts. It intentionally supports only what a timetable grid needs: a title,
//! a meta line, a header row and evenly-weighted text columns.

const PAGE_W: f64 = 595.0; // A4 portrait, points
const PAGE_H: f64 = 842.0;
const MARGIN: f64 = 36.0;
const ROW_H: f64 = 20.0;
const HEAD_H: f64 = 22.0;
const TITLE_SIZE: f64 = 16.0;
const META_SIZE: f64 = 9.0;
const CELL_SIZE: f64 = 8.5;
/// Rough average glyph width as a fraction of font size (Helvetica).
const AVG_GLYPH: f64 = 0.55;

pub struct Table {
    pub title: String,
    pub meta: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Relative column widths (same length as `headers`).
    pub weights: Vec<f64>,
}

fn esc(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\n' | '\r' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Maps a character to its WinAnsi (CP1252) code point. Text drawn with a
/// Type1 base font must be written as single WinAnsi bytes, not UTF-8.
fn winansi_byte(c: char) -> u8 {
    match c as u32 {
        // ASCII, including the newlines that separate content-stream operators.
        0x00..=0x7F => c as u8,
        0xA0..=0xFF => c as u8,
        0x20AC => 0x80,
        0x201A => 0x82,
        0x0192 => 0x83,
        0x201E => 0x84,
        0x2026 => 0x85,
        0x2020 => 0x86,
        0x2021 => 0x87,
        0x02C6 => 0x88,
        0x2030 => 0x89,
        0x0160 => 0x8A,
        0x2039 => 0x8B,
        0x0152 => 0x8C,
        0x017D => 0x8E,
        0x2018 => 0x91,
        0x2019 => 0x92,
        0x201C => 0x93,
        0x201D => 0x94,
        0x2022 => 0x95,
        0x2013 => 0x96,
        0x2014 => 0x97,
        0x02DC => 0x98,
        0x2122 => 0x99,
        0x0161 => 0x9A,
        0x203A => 0x9B,
        0x0153 => 0x9C,
        0x017E => 0x9E,
        0x0178 => 0x9F,
        _ => b'?',
    }
}

fn fit(s: &str, size: f64, max_w: f64) -> String {
    let max_chars = ((max_w / (size * AVG_GLYPH)).floor() as usize).max(1);
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let keep = max_chars.saturating_sub(2);
    let truncated: String = s.chars().take(keep).collect();
    format!("{truncated}..")
}

struct Content {
    ops: String,
}

impl Content {
    fn new() -> Self {
        Content { ops: String::new() }
    }
    fn text(&mut self, x: f64, y: f64, size: f64, bold: bool, s: &str) {
        let font = if bold { "F2" } else { "F1" };
        self.ops.push_str(&format!(
            "BT /{font} {size:.2} Tf 1 0 0 1 {x:.2} {y:.2} Tm ({}) Tj ET\n",
            esc(s)
        ));
    }
    fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        self.ops
            .push_str(&format!("0.6 w {x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S\n"));
    }
}

fn col_xs(weights: &[f64]) -> Vec<f64> {
    let usable = PAGE_W - 2.0 * MARGIN;
    let total: f64 = weights.iter().sum();
    let mut xs = Vec::with_capacity(weights.len());
    let mut acc = 0.0;
    for w in weights {
        xs.push(MARGIN + usable * (acc / total));
        acc += w;
    }
    xs
}

pub fn render(table: &Table) -> Vec<u8> {
    let ncols = table.headers.len().max(1);
    let weights: Vec<f64> = if table.weights.len() == ncols {
        table.weights.clone()
    } else {
        vec![1.0; ncols]
    };
    let xs = col_xs(&weights);
    let usable = PAGE_W - 2.0 * MARGIN;
    let right = MARGIN + usable;

    // Split rows across pages.
    let first_top = PAGE_H - MARGIN - 56.0;
    let cont_top = PAGE_H - MARGIN - 30.0;
    let rows_first = (((first_top - HEAD_H - MARGIN) / ROW_H).floor() as usize).max(1);
    let rows_cont = (((cont_top - HEAD_H - MARGIN) / ROW_H).floor() as usize).max(1);

    let mut pages: Vec<Content> = Vec::new();
    let mut idx = 0usize;
    let mut page_no = 0usize;
    if table.rows.is_empty() {
        pages.push(render_page(table, &xs, right, first_top, &[], 0));
    }
    while idx < table.rows.len() {
        let (top, cap) = if page_no == 0 {
            (first_top, rows_first)
        } else {
            (cont_top, rows_cont)
        };
        let end = (idx + cap).min(table.rows.len());
        pages.push(render_page(
            table,
            &xs,
            right,
            top,
            &table.rows[idx..end],
            page_no,
        ));
        idx = end;
        page_no += 1;
    }

    assemble(&pages)
}

fn render_page(
    table: &Table,
    xs: &[f64],
    right: f64,
    top: f64,
    rows: &[Vec<String>],
    page_no: usize,
) -> Content {
    let mut c = Content::new();

    // Title / meta
    if page_no == 0 {
        c.text(MARGIN, PAGE_H - MARGIN - 16.0, TITLE_SIZE, true, &table.title);
        if !table.meta.is_empty() {
            c.text(MARGIN, PAGE_H - MARGIN - 32.0, META_SIZE, false, &table.meta);
        }
    } else {
        c.text(
            MARGIN,
            PAGE_H - MARGIN - 14.0,
            META_SIZE,
            true,
            &format!("{} (continued)", table.title),
        );
    }

    // Header row
    let head_bottom = top - HEAD_H;
    c.line(MARGIN, top, right, top);
    for (i, h) in table.headers.iter().enumerate() {
        let max_w = (if i + 1 < xs.len() { xs[i + 1] } else { right }) - xs[i] - 6.0;
        c.text(xs[i] + 3.0, head_bottom + 7.0, 9.0, true, &fit(h, 9.0, max_w));
    }
    c.line(MARGIN, head_bottom, right, head_bottom);

    // Rows
    let mut y = head_bottom;
    for row in rows {
        let bottom = y - ROW_H;
        for (i, cell) in row.iter().enumerate() {
            if i >= xs.len() {
                break;
            }
            let max_w = (if i + 1 < xs.len() { xs[i + 1] } else { right }) - xs[i] - 6.0;
            c.text(xs[i] + 3.0, bottom + 6.5, CELL_SIZE, false, &fit(cell, CELL_SIZE, max_w));
        }
        c.line(MARGIN, bottom, right, bottom);
        y = bottom;
    }

    // Vertical separators spanning the drawn band.
    let band_bottom = y;
    for &x in xs.iter() {
        c.line(x, top, x, band_bottom);
    }
    c.line(right, top, right, band_bottom);

    c
}

fn assemble(pages: &[Content]) -> Vec<u8> {
    // Object layout: 1 catalog, 2 pages, 3 F1, 4 F2, then page/content pairs.
    let mut objects: Vec<Vec<u8>> = Vec::new();
    objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());

    let kids: Vec<String> = (0..pages.len())
        .map(|i| format!("{} 0 R", 5 + i * 2))
        .collect();
    objects.push(
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        )
        .into_bytes(),
    );
    objects.push(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    objects.push(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );

    for (i, page) in pages.iter().enumerate() {
        let content_id = 6 + i * 2;
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W:.0} {PAGE_H:.0}] \
                 /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {content_id} 0 R >>"
            )
            .into_bytes(),
        );
        // Draw ops are WinAnsi-encoded single bytes, not UTF-8.
        let stream: Vec<u8> = page.ops.chars().map(winansi_byte).collect();
        let mut obj = format!("<< /Length {} >>\nstream\n", stream.len()).into_bytes();
        obj.extend_from_slice(&stream);
        obj.extend_from_slice(b"\nendstream");
        objects.push(obj);
    }

    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(obj);
        out.extend_from_slice(b"\nendobj\n");
    }

    let xref_off = out.len();
    let count = objects.len() + 1;
    out.extend_from_slice(format!("xref\n0 {count}\n").as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in &offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {count} /Root 1 0 R >>\nstartxref\n{xref_off}\n%%EOF\n")
            .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Table {
        Table {
            title: "Timetable — Grade 7A".into(),
            meta: "Demo High School · class timetable".into(),
            headers: vec!["Period".into(), "Time".into(), "Monday".into(), "Tuesday".into()],
            rows: vec![
                vec!["P1".into(), "08:30 - 09:15".into(), "MAT - SM (Room 101)".into(), "ENG - EH".into()],
                vec!["P2".into(), "09:15 - 10:00".into(), "PHY - PS (Lab)".into(), String::new()],
            ],
            weights: vec![0.4, 0.7, 1.0, 1.0],
        }
    }

    /// Finds the last occurrence of `needle` in a byte slice.
    fn rfind(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).rposition(|w| w == needle)
    }

    #[test]
    fn produces_a_structurally_valid_pdf() {
        let bytes = render(&sample());
        assert!(bytes.starts_with(b"%PDF-1.4"), "missing header");
        // The header carries a binary marker, so compare raw bytes (not UTF-8).
        assert!(rfind(&bytes, b"%%EOF").is_some(), "missing EOF marker");
        assert!(rfind(&bytes, b"/Type /Catalog").is_some(), "missing catalog");
        assert!(rfind(&bytes, b"/BaseFont /Helvetica").is_some(), "missing font");

        let sx = rfind(&bytes, b"startxref").expect("startxref");
        let tail = &bytes[sx + b"startxref".len()..];
        let off_str = tail
            .split(|b| b.is_ascii_whitespace())
            .find(|s| !s.is_empty())
            .expect("offset token");
        let offset: usize = std::str::from_utf8(off_str).unwrap().parse().expect("numeric");
        assert!(
            bytes[offset..].starts_with(b"xref"),
            "startxref must point at the xref table"
        );

        // Parse the xref table and confirm each entry points at its object.
        let xref = &bytes[offset..];
        let text = String::from_utf8_lossy(&xref[..xref.len().min(2000)]);
        let mut lines = text.lines();
        lines.next(); // "xref"
        let count: usize = lines
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        lines.next(); // free entry
        for i in 1..count {
            let line = lines.next().expect("xref entry");
            let off: usize = line[..10].parse().expect("offset");
            assert!(
                bytes[off..].starts_with(format!("{i} 0 obj").as_bytes()),
                "xref entry {i} points at the wrong object"
            );
        }
    }

    #[test]
    fn encodes_text_as_winansi_not_utf8() {
        let mut t = sample();
        t.title = "Timetable — café · Grade 7A".into();
        let out = render(&t);
        assert!(out.contains(&0x97u8), "em dash must map to WinAnsi 0x97");
        assert!(out.contains(&0xB7u8), "middle dot must map to WinAnsi 0xB7");
        assert!(out.contains(&0xE9u8), "é must map to WinAnsi 0xE9");
        assert!(
            !out.windows(2).any(|w| w == [0xC3, 0xA9]),
            "text must not be written as UTF-8 bytes"
        );
        // The content stream must still contain its operator newlines.
        assert!(out.contains(&b'\n'));
    }

    #[test]
    fn escapes_pdf_special_characters() {
        let mut t = sample();
        t.rows[0][2] = "Math (A) \\ B".into();
        let out = String::from_utf8_lossy(&render(&t)).to_string();
        assert!(out.contains("\\(A\\)"), "parentheses must be escaped");
        assert!(out.contains("\\\\"), "backslash must be escaped");
    }
}
