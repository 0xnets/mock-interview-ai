//! Server-side PDF renderer for interview reports. Replaces the client
//! `jsPDF` path. Uses `printpdf` directly with a flow-style layout so we
//! don't need a Typst/WeasyPrint sidecar — the report is plain text with
//! coloured section heads, which `printpdf` can do in pure Rust.

use std::io::BufWriter;

use anyhow::{Context, Result};
use printpdf::{
    BuiltinFont, IndirectFontRef, Mm, PdfDocument, PdfDocumentReference, PdfLayerReference,
};
use serde_json::Value as JsonValue;

use persistence::repo_reports::ReportForRender;

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN: f32 = 18.0;
const CONTENT_W: f32 = PAGE_W - 2.0 * MARGIN;
const TOP: f32 = 280.0;
const BOTTOM: f32 = 18.0;
const PT_TO_MM: f32 = 0.352_778;

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
        self.write_wrapped(doc, text, size, bold, color, MARGIN, CONTENT_W, MARGIN);
    }

    fn write_wrapped(
        &mut self,
        doc: &PdfDocumentReference,
        text: &str,
        size: f32,
        bold: bool,
        color: Option<(f32, f32, f32)>,
        x: f32,
        width: f32,
        continuation_x: f32,
    ) {
        let line_h = size * 0.42;
        let continuation_w = PAGE_W - MARGIN - continuation_x;
        for (idx, chunk) in wrap_to_width(text, width, continuation_w, size)
            .into_iter()
            .enumerate()
        {
            self.ensure_room(doc, line_h + 1.0);
            if let Some((r, g, b)) = color {
                self.layer
                    .set_fill_color(printpdf::Color::Rgb(printpdf::Rgb::new(r, g, b, None)));
            } else {
                self.layer
                    .set_fill_color(printpdf::Color::Rgb(printpdf::Rgb::new(
                        0.16, 0.16, 0.16, None,
                    )));
            }
            self.layer.use_text(
                chunk,
                size,
                Mm(if idx == 0 { x } else { continuation_x }),
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
    if let Some(timing) = answer_time_summary(&r.raw_ai_output) {
        c.write_line(&doc, &timing, 10.0, false, Some((0.30, 0.30, 0.30)));
    }
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
            let answer = it.get("answer").and_then(|v| v.as_str()).unwrap_or("");
            let feedback = it.get("feedback").and_then(|v| v.as_str()).unwrap_or("");
            let header = match score {
                Some(s) => format!("Q{} ({}) — {}%", q, section_lbl, s),
                None => format!("Q{} ({}) — not graded", q, section_lbl),
            };
            c.write_line(&doc, &header, 11.0, true, Some((0.20, 0.20, 0.20)));
            if !question.is_empty() {
                c.write_wrapped(
                    &doc,
                    &format!("Question: {question}"),
                    9.0,
                    false,
                    None,
                    MARGIN,
                    CONTENT_W,
                    MARGIN + 17.0,
                );
            }
            if !answer.trim().is_empty() {
                c.write_wrapped(
                    &doc,
                    &format!("Answer: {}", answer.trim()),
                    9.0,
                    false,
                    None,
                    MARGIN,
                    CONTENT_W,
                    MARGIN + 17.0,
                );
            }
            if !feedback.is_empty() {
                c.write_wrapped(
                    &doc,
                    &format!("Feedback: {feedback}"),
                    9.0,
                    false,
                    None,
                    MARGIN,
                    CONTENT_W,
                    MARGIN + 17.0,
                );
            }
            if let Some(secs) = it.get("duration_seconds").and_then(|v| v.as_i64()) {
                let bucket = it
                    .get("time_bucket")
                    .and_then(|v| v.as_str())
                    .map(|b| format!(" ({})", b.replace('_', " ")))
                    .unwrap_or_default();
                c.write_line(
                    &doc,
                    &format!("Time: {}{}", format_duration(secs), bucket),
                    9.0,
                    false,
                    Some((0.45, 0.45, 0.45)),
                );
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
                c.write_wrapped(
                    doc,
                    &format!("- {s}"),
                    10.0,
                    false,
                    None,
                    MARGIN,
                    CONTENT_W,
                    MARGIN + 5.0,
                );
            }
        }
    }
    c.spacer(2.0);
}

fn wrap_to_width(
    text: &str,
    first_width_mm: f32,
    continuation_width_mm: f32,
    size: f32,
) -> Vec<String> {
    let mut out = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.trim().is_empty() {
            out.push(String::new());
            continue;
        }
        let mut current = String::new();
        let mut current_width = first_width_mm;
        for word in paragraph.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };

            if text_width_mm(&candidate, size) <= current_width {
                current = candidate;
                continue;
            }

            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
                current_width = continuation_width_mm;
            }

            if text_width_mm(word, size) <= current_width {
                current.push_str(word);
            } else {
                for chunk in split_long_word(word, current_width, size) {
                    if current.is_empty() {
                        current = chunk;
                    } else {
                        out.push(std::mem::take(&mut current));
                        current = chunk;
                    }
                    if text_width_mm(&current, size) >= current_width {
                        out.push(std::mem::take(&mut current));
                        current_width = continuation_width_mm;
                    }
                }
            }
        }
        if !current.is_empty() {
            out.push(current);
        }
    }
    out
}

fn split_long_word(word: &str, width_mm: f32, size: f32) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for ch in word.chars() {
        let candidate = format!("{current}{ch}");
        if !current.is_empty() && text_width_mm(&candidate, size) > width_mm {
            chunks.push(std::mem::take(&mut current));
        }
        current.push(ch);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn text_width_mm(text: &str, size: f32) -> f32 {
    text.chars().map(helvetica_width_em).sum::<f32>() * size * PT_TO_MM
}

fn helvetica_width_em(ch: char) -> f32 {
    match ch {
        ' ' => 0.28,
        'i' | 'j' | 'l' | '!' | '|' => 0.22,
        'f' | 'r' | 't' | '.' | ',' | ':' | ';' | '\'' | '`' => 0.28,
        '-' | '(' | ')' | '[' | ']' | '{' | '}' => 0.33,
        'm' | 'w' | 'M' | 'W' => 0.83,
        'A'..='Z' => 0.67,
        '0'..='9' => 0.56,
        _ if ch.is_ascii() => 0.50,
        _ => 0.56,
    }
}

fn pdf_to_writer(doc: PdfDocumentReference) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    {
        let mut w = BufWriter::new(&mut buf);
        doc.save(&mut w).context("printpdf save")?;
    }
    Ok(buf)
}

/// Human-readable answer duration: `45s`, `1m 24s`, `1h 03m`. Seconds are
/// zero-padded once minutes are shown so values line up; negative input (a
/// broken client clock) is clamped to zero.
fn format_duration(total_seconds: i64) -> String {
    let s = total_seconds.max(0);
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m {sec:02}s")
    } else {
        format!("{sec}s")
    }
}

/// Builds the "Total answer time … | Average …" header line from the timing
/// fields `finalize` stores in `raw_ai_output`. Returns `None` for older
/// reports that predate those fields or when nothing was answered.
fn answer_time_summary(raw: &JsonValue) -> Option<String> {
    let answered = raw
        .get("answered_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if answered <= 0 {
        return None;
    }
    let total = raw
        .get("total_answer_time_seconds")
        .and_then(|v| v.as_i64())?;
    let avg = raw
        .get("average_answer_time_seconds")
        .and_then(|v| v.as_i64())?;
    Some(format!(
        "Total answer time: {}   |   Average: {}",
        format_duration(total),
        format_duration(avg),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_duration_sub_minute() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(9), "9s");
        assert_eq!(format_duration(59), "59s");
    }

    #[test]
    fn format_duration_minutes_zero_pads_seconds() {
        assert_eq!(format_duration(84), "1m 24s");
        assert_eq!(format_duration(64), "1m 04s");
        assert_eq!(format_duration(600), "10m 00s");
    }

    #[test]
    fn format_duration_hours() {
        assert_eq!(format_duration(3600), "1h 00m");
        assert_eq!(format_duration(3720), "1h 02m");
        assert_eq!(format_duration(7380), "2h 03m");
    }

    #[test]
    fn format_duration_negative_clamps_to_zero() {
        assert_eq!(format_duration(-5), "0s");
    }

    #[test]
    fn answer_time_summary_present() {
        let raw = serde_json::json!({
            "answered_count": 3,
            "total_answer_time_seconds": 252,
            "average_answer_time_seconds": 84,
        });
        assert_eq!(
            answer_time_summary(&raw).as_deref(),
            Some("Total answer time: 4m 12s   |   Average: 1m 24s"),
        );
    }

    #[test]
    fn answer_time_summary_absent_for_old_or_empty_reports() {
        assert_eq!(answer_time_summary(&serde_json::json!({})), None);
        assert_eq!(
            answer_time_summary(&serde_json::json!({ "answered_count": 0 })),
            None,
        );
    }

    #[test]
    fn wrap_to_width_uses_continuation_width() {
        let wrapped = wrap_to_width(
            "Feedback: The candidate gave a detailed answer with clear tradeoffs",
            45.0,
            22.0,
            10.0,
        );

        assert!(wrapped.len() > 2);
        assert!(text_width_mm(&wrapped[0], 10.0) <= 45.0);
        for line in wrapped.iter().skip(1) {
            assert!(text_width_mm(line, 10.0) <= 22.0);
        }
    }

    #[test]
    fn wrap_to_width_splits_long_tokens_on_character_boundaries() {
        let wrapped = wrap_to_width("supercalifragilisticexpialidocious", 12.0, 12.0, 10.0);

        assert!(wrapped.len() > 1);
        assert_eq!(wrapped.concat(), "supercalifragilisticexpialidocious");
    }
}
