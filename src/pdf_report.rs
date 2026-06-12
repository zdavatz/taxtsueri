//! Rendert die **Pseudo-Jahresrechnung** ([`crate::mt940::PseudoStatements`]) als
//! zweiseitiges PDF (Seite 1: Bilanz, Seite 2: Erfolgsrechnung) — Entwurf zur Prüfung
//! durch den Vermögensverwalter. Reiner Text mit der Standard-Schrift Helvetica
//! (WinAnsi), zusammengesetzt via `lopdf`.

use crate::mt940::{format_cents, PseudoStatements};
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream, StringFormat};

const PAGE_W: f64 = 595.0; // A4 in PDF-Punkten
const PAGE_H: f64 = 842.0;
const LEFT: f64 = 60.0;
const RIGHT: f64 = 535.0; // rechter Rand für Beträge
const FONT: &str = "F1";

/// Eine Zeile auf einer Seite: links Label, optional rechtsbündig ein Betrag.
struct Line {
    indent: f64,
    size: f64,
    bold: bool,
    label: String,
    amount: Option<String>,
}

impl Line {
    fn new(label: &str) -> Self {
        Self { indent: 0.0, size: 10.0, bold: false, label: label.into(), amount: None }
    }
    fn amount(mut self, chf_cents: i64) -> Self {
        self.amount = Some(format_cents(chf_cents));
        self
    }
    fn amount_str(mut self, s: &str) -> Self {
        self.amount = Some(s.into());
        self
    }
    fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
    fn size(mut self, s: f64) -> Self {
        self.size = s;
        self
    }
    fn indent(mut self, x: f64) -> Self {
        self.indent = x;
        self
    }
}

/// UTF-8 → WinAnsi-(Latin-1-)Bytes für die PDF-Textausgabe (deckt Umlaute ab).
/// Typografische Striche werden auf den ASCII-Bindestrich abgebildet.
fn winansi(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c {
            '\u{2013}' | '\u{2014}' => b'-', // En-/Em-Dash
            c if (c as u32) <= 0xFF => c as u8,
            _ => b'?',
        })
        .collect()
}

fn pdf_string(s: &str) -> Object {
    Object::String(winansi(s), StringFormat::Literal)
}

/// Näherungsweise Textbreite in Helvetica (für die Rechtsbündigkeit der Beträge).
fn text_width(s: &str, size: f64) -> f64 {
    // Helvetica: Ziffern 556, Punkt/Komma 278, Minus 333, Buchstaben grob 0.52 em.
    let units: f64 = s
        .chars()
        .map(|c| match c {
            '0'..='9' => 556.0,
            '.' | ',' | '\'' | ' ' | ':' => 278.0,
            '-' => 333.0,
            _ => 540.0,
        })
        .sum();
    units / 1000.0 * size
}

/// Baut den Content-Stream einer Seite aus Zeilen, von oben nach unten gesetzt.
fn page_content(title: &str, subtitle: &str, lines: &[Line]) -> Content {
    let mut ops: Vec<Operation> = Vec::new();
    let mut y = PAGE_H - 70.0;

    // Titel
    ops.push(Operation::new("BT", vec![]));
    ops.push(Operation::new("Tf", vec![FONT.into(), 16.into()]));
    ops.push(Operation::new("Td", vec![LEFT.into(), y.into()]));
    ops.push(Operation::new("Tj", vec![pdf_string(title)]));
    ops.push(Operation::new("ET", vec![]));
    y -= 20.0;

    // Untertitel (Hinweis)
    if !subtitle.is_empty() {
        ops.push(Operation::new("BT", vec![]));
        ops.push(Operation::new("Tf", vec![FONT.into(), 9.into()]));
        ops.push(Operation::new("Td", vec![LEFT.into(), y.into()]));
        ops.push(Operation::new("Tj", vec![pdf_string(subtitle)]));
        ops.push(Operation::new("ET", vec![]));
        y -= 22.0;
    }

    for line in lines {
        let x = LEFT + line.indent;
        // Label
        ops.push(Operation::new("BT", vec![]));
        ops.push(Operation::new("Tf", vec![FONT.into(), line.size.into()]));
        ops.push(Operation::new("Td", vec![x.into(), y.into()]));
        ops.push(Operation::new("Tj", vec![pdf_string(&line.label)]));
        ops.push(Operation::new("ET", vec![]));
        // Betrag, rechtsbündig
        if let Some(a) = &line.amount {
            let ax = RIGHT - text_width(a, line.size);
            ops.push(Operation::new("BT", vec![]));
            ops.push(Operation::new("Tf", vec![FONT.into(), line.size.into()]));
            ops.push(Operation::new("Td", vec![ax.into(), y.into()]));
            ops.push(Operation::new("Tj", vec![pdf_string(a)]));
            ops.push(Operation::new("ET", vec![]));
        }
        y -= if line.bold { line.size + 6.0 } else { line.size + 4.0 };
    }
    Content { operations: ops }
}

/// Zeilen der **Bilanz**-Seite.
fn bilanz_lines(ps: &PseudoStatements) -> Vec<Line> {
    let b = &ps.bilanz;
    let mut v = vec![
        Line::new(&format!("Konto: {}", b.konto)).size(9.0),
        Line::new("").size(2.0),
        Line::new("AKTIVEN").bold().amount_str("CHF"),
        Line::new("Flüssige Mittel (Bank)").indent(12.0).amount(b.fluessige_mittel_cents),
    ];
    if let Some(w) = b.wertschriften_cents {
        v.push(Line::new("Wertschriften (Vermögensausweis)").indent(12.0).amount(w));
    }
    v.push(Line::new("Total Aktiven (ableitbar)").bold().amount(b.total_aktiven_cents()));
    if let Some(o) = b.eroeffnung_cents {
        v.push(Line::new("").size(6.0));
        v.push(Line::new(&format!("(Bank-Eröffnungssaldo: {})", format_cents(o))).size(8.0));
    }
    v.push(Line::new("").size(10.0));
    v.push(Line::new("Nicht aus den Daten ableitbar - durch den Vermögensverwalter ergänzen:").size(9.0));
    v.push(Line::new("Kasse, Anlagevermögen, Kreditoren, Darlehen, Rechnungsabgrenzungen, Eigenkapital.").indent(12.0).size(9.0));
    v
}

/// Zeilen der **Erfolgsrechnungs**-Seite.
fn er_lines(ps: &PseudoStatements) -> Vec<Line> {
    let er = &ps.erfolgsrechnung;
    let mut v = vec![Line::new("ERTRAG (Gutschriften)").bold().amount_str("CHF")];
    for l in &er.ertrag {
        let label = match &l.note {
            Some(n) => format!("{}  ({})", l.category, n),
            None => l.category.clone(),
        };
        v.push(Line::new(&label).indent(12.0).size(9.0).amount(l.amount_cents));
    }
    v.push(Line::new("Total Ertrag").bold().amount(er.total_ertrag_cents));
    v.push(Line::new("").size(8.0));
    v.push(Line::new("AUFWAND (Belastungen)").bold().amount_str("CHF"));
    for l in &er.aufwand {
        v.push(Line::new(&l.category).indent(12.0).size(9.0).amount(l.amount_cents));
    }
    v.push(Line::new("Total Aufwand").bold().amount(er.total_aufwand_cents));
    v.push(Line::new("").size(8.0));
    v.push(Line::new("Geldfluss-Saldo (ungleich Jahresgewinn)").bold().amount(er.saldo_cents));
    v
}

/// Erzeugt das zweiseitige Pseudo-Jahresrechnungs-PDF.
pub fn pseudo_statements_pdf(ps: &PseudoStatements) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { FONT => font_id },
    });

    let warn = "Entwurf (Cash-Basis aus MT940 + Vermögensausweis) - vom Vermögensverwalter zu prüfen.";

    let mut kids: Vec<Object> = Vec::new();
    for (title, sub, lines) in [
        ("Cash-Flow-Rechnung — Bilanz (Entwurf)", warn, bilanz_lines(ps)),
        ("Cash-Flow-Rechnung — Erfolgsrechnung (Entwurf)", warn, er_lines(ps)),
    ] {
        let content = page_content(title, sub, &lines);
        let stream_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), PAGE_W.into(), PAGE_H.into()],
            "Resources" => resources_id,
            "Contents" => stream_id,
        });
        kids.push(page_id.into());
    }

    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("PDF serialisieren");
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mt940;

    #[test]
    fn renders_two_page_pdf() {
        let mt = ":25:CH9300762011623852957\n\
            :60F:C250101CHF100000,00\n\
            :61:2503150315C500,00NTRFNONREF//Zins\n\
            :86:Zinsgutschrift Sparkonto\n\
            :61:2506200620D1200,50NTRFNONREF//Miete\n\
            :86:Dauerauftrag Miete\n\
            :62F:C251231CHF99299,50\n";
        let stmt = mt940::parse(mt).unwrap();
        let ps = mt940::pseudo_statements(&stmt, Some(3_862_753));
        let pdf = pseudo_statements_pdf(&ps);
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.len() > 800, "PDF wirkt zu klein: {} Bytes", pdf.len());
    }
}
