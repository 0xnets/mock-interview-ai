//! Server-side PDF renderer for interview reports. Replaces the client
//! `jsPDF` path. Uses `printpdf` directly with a flow-style layout so we
//! don't need a Typst/WeasyPrint sidecar — the report is plain text with
//! coloured section heads, which `printpdf` can do in pure Rust.

use std::io::BufWriter;

use anyhow::{Context, Result};
use printpdf::{BuiltinFont, IndirectFontRef, Mm, PdfDocument, PdfDocumentReference, PdfLayerReference};
use serde_json::Value as JsonValue;

use persistence::repo_reports::ReportForRender;

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN: f32 = 18.0;
const TOP: f32 = 280.0;
const BOTTOM: f32 = 18.0;

struct Cursor {
    y: f32,
    layer: PdfLayerReference,
    regular: IndirectFontRef,
    bold: IndirectFontRef,
}

impl Cursor {
    fn new(layer: PdfLayerReference, regular: IndirectFontRef, bold: IndirectFontRef) -> Self {
        Self {
            y: TOP,
            layer,
            regular,
            bold,
        }
    }

    fn ensure_room(&mut self, doc: &PdfDocumentReference, needed_mm: f32) {
        if self.y - needed_mm < BOTTOM {
            let (page, layer) = doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "page");
            self.layer = doc.get_page(page).get_layer(layer);
            self.y = TOP;
        }
    }

    fn write_line(
        &mut self,
        doc: &PdfDocumentReference,
        text: &str,
        size: f32,
        bold: bool,
        color: Option<(f32, f32, f32)>,
    ) {
        let line_h = size * 0.42;
        for chunk in wrap(text, max_chars_for(size)) {
            self.ensure_room(doc, line_h + 1.0);
            if let Some((r, g, b)) = color {
                self.layer.set_fill_color(printpdf::Color::Rgb(printpdf::Rgb::new(r, g, b, None)));
            } else {
                self.layer
                    .set_fill_color(printpdf::Color::Rgb(printpdf::Rgb::new(0.16, 0.16, 0.16, None)));
            }
            self.layer.use_text(
                chunk,
                size,
                Mm(MARGIN),
                Mm(self.y),
                if bold { &self.bold } else { &self.regular },
            );
            self.y -= line_h;
        }
        self.y -= 0.8;
    }

    fn spacer(&mut self, mm: f32) {
        self.y -= mm;
    }
}

/// Render a PDF and return the bytes. Pure function — no I/O beyond
/// `printpdf`'s in-memory buffer.
pub fn render_report_pdf(r: &ReportForRender) -> Result<Vec<u8>> {
    let (doc, page, layer) = PdfDocument::new(
        format!("Interview Report — {}", r.candidate_name),
        Mm(PAGE_W),
        Mm(PAGE_H),
        "page",
    );
    let regular = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .context("load Helvetica")?;
    let bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .context("load Helvetica-Bold")?;

    let layer = doc.get_page(page).get_layer(layer);
    let mut c = Cursor::new(layer, regular, bold);

    // Header band
    c.write_line(
        &doc,
        "Mock Interview Report",
        20.0,
        true,
        Some((0.31, 0.27, 0.90)),
    );
    c.write_line(
        &doc,
        &format!("{} — {}", r.candidate_name, r.role_title),
        12.0,
        false,
        Some((0.30, 0.30, 0.30)),
    );
    c.write_line(
        &doc,
        &format!("Generated: {}", r.generated_at.format("%Y-%m-%d %H:%M UTC")),
        9.0,
        false,
        Some((0.45, 0.45, 0.45)),
    );
    c.spacer(4.0);

    // Headline score
    let score_color = if r.passed {
        (0.06, 0.72, 0.51)
    } else {
        (0.94, 0.27, 0.27)
    };
    c.write_line(
        &doc,
        &format!("Overall Score: {}%", r.overall),
        24.0,
        true,
        Some(score_color),
    );
    let verdict = if r.passed {
        format!(
            "PASSED — at or above the {}% threshold. Advance to a human round.",
            r.pass_threshold
        )
    } else {
        format!(
            "BELOW THRESHOLD — under {}%. Practice and retry.",
            r.pass_threshold
        )
    };
    c.write_line(&doc, &verdict, 11.0, true, Some(score_color));
    c.write_line(
        &doc,
        &format!(
            "Technical: {}%   |   Behavioral: {}%",
            r.technical, r.behavioral
        ),
        11.0,
        false,
        None,
    );
    c.spacer(3.0);

    section(&doc, &mut c, "Summary");
    c.write_line(&doc, r.summary.trim(), 10.0, false, None);
    c.spacer(2.0);

    bullet_section(&doc, &mut c, "Strengths", &r.strengths, (0.06, 0.72, 0.51));
    bullet_section(
        &doc,
        &mut c,
        "Weaknesses",
        &r.weaknesses,
        (0.85, 0.47, 0.02),
    );
    bullet_section(
        &doc,
        &mut c,
        "Action Items for the Next Round",
        &r.action_items,
        (0.31, 0.27, 0.90),
    );

    // Per-question breakdown (from raw_ai_output.per_question if present)
    if let Some(items) = r
        .raw_ai_output
        .get("per_question")
        .and_then(|v| v.as_array())
    {
        c.ensure_room(&doc, 30.0);
        section(&doc, &mut c, "Per-Question Breakdown");
        for it in items {
            let q = it.get("q").and_then(|v| v.as_i64()).unwrap_or(0);
            let section_lbl = it
                .get("section")
                .and_then(|v| v.as_str())
                .unwrap_or("Question");
            let score = it.get("score").and_then(|v| v.as_i64());
            let question = it.get("question").and_then(|v| v.as_str()).unwrap_or("");
            let feedback = it.get("feedback").and_then(|v| v.as_str()).unwrap_or("");
            let header = match score {
                Some(s) => format!("Q{} ({}) — {}%", q, section_lbl, s),
                None => format!("Q{} ({}) — not graded", q, section_lbl),
            };
            c.write_line(&doc, &header, 11.0, true, Some((0.20, 0.20, 0.20)));
            if !question.is_empty() {
                c.write_line(&doc, &format!("Question: {question}"), 9.0, false, None);
            }
            if !feedback.is_empty() {
                c.write_line(&doc, &format!("Feedback: {feedback}"), 9.0, false, None);
            }
            c.spacer(1.5);
        }
    }

    pdf_to_writer(doc)
}

fn section(doc: &PdfDocumentReference, c: &mut Cursor, title: &str) {
    c.spacer(2.0);
    c.write_line(doc, title, 14.0, true, Some((0.31, 0.27, 0.90)));
}

fn bullet_section(
    doc: &PdfDocumentReference,
    c: &mut Cursor,
    title: &str,
    items: &JsonValue,
    color: (f32, f32, f32),
) {
    c.spacer(1.0);
    c.write_line(doc, title, 13.0, true, Some(color));
    if let Some(arr) = items.as_array() {
        for it in arr {
            if let Some(s) = it.as_str() {
                c.write_line(doc, &format!("• {s}"), 10.0, false, None);
            }
        }
    }
    c.spacer(2.0);
}

/// Approximate max characters per line for the given point size at our
/// margin. Conservative so we rarely overflow; rendered text uses Helvetica
/// which is mostly monospaceable for layout estimation.
fn max_chars_for(size: f32) -> usize {
    let width = PAGE_W - 2.0 * MARGIN;
    // ~1.9 chars/mm at 10pt for Helvetica; scales inversely with size.
    let chars_per_mm = 1.9 * (10.0 / size);
    (width * chars_per_mm) as usize
}

fn wrap(text: &str, max_chars: usize) -> Vec<String> {
    if max_chars == 0 {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    for line in text.split('\n') {
        if line.len() <= max_chars {
            out.push(line.to_string());
            continue;
        }
        let mut current = String::new();
        for word in line.split_whitespace() {
            if current.is_empty() {
                if word.len() > max_chars {
                    // hard-split very long token
                    for chunk in word.as_bytes().chunks(max_chars) {
                        out.push(String::from_utf8_lossy(chunk).into_owned());
                    }
                } else {
                    current.push_str(word);
                }
            } else if current.len() + 1 + word.len() > max_chars {
                out.push(std::mem::take(&mut current));
                current.push_str(word);
            } else {
                current.push(' ');
                current.push_str(word);
            }
        }
        if !current.is_empty() {
            out.push(current);
        }
    }
    out
}

fn pdf_to_writer(doc: PdfDocumentReference) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    {
        let mut w = BufWriter::new(&mut buf);
        doc.save(&mut w).context("printpdf save")?;
    }
    Ok(buf)
}
