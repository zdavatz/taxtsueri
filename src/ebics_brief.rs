//! Rendert den **EBICS-Initialisierungsbrief** (INI-Brief) als PDF: je eine Seite pro
//! Teilnehmerschlüssel (`A006` aus dem `INI`, `X002` und `E002` aus dem `HIA`) mit
//! Exponent, Modulus und SHA-256-Hashwert. Der Brief geht **handschriftlich
//! unterzeichnet** an die Bank; sie vergleicht die Hashwerte mit den elektronisch
//! übermittelten Schlüsseln und schaltet den Zugang frei.
//!
//! Helvetica/Courier (WinAnsi) via `lopdf`, wie [`crate::pdf_report`].

use crate::ebics::{public_key_hash, public_parts, KeyKind, Keys, Params};
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream, StringFormat};
use rsa::traits::PublicKeyParts;

const PAGE_W: f64 = 595.0; // A4 in PDF-Punkten
const PAGE_H: f64 = 842.0;
const LEFT: f64 = 60.0;
const RIGHT: f64 = 535.0;
/// Spalte der Werte neben den Bezeichnungen.
const VALUE_X: f64 = 200.0;
const REGULAR: &str = "F1";
const BOLD: &str = "F2";
const MONO: &str = "F3";

/// UTF-8 → WinAnsi-(Latin-1-)Bytes für die PDF-Textausgabe (deckt Umlaute ab).
fn winansi(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c {
            '\u{2013}' | '\u{2014}' => b'-',
            c if (c as u32) <= 0xFF => c as u8,
            _ => b'?',
        })
        .collect()
}

fn text(ops: &mut Vec<Operation>, font: &str, size: f64, x: f64, y: f64, s: &str) {
    ops.push(Operation::new("BT", vec![]));
    ops.push(Operation::new("Tf", vec![font.into(), size.into()]));
    ops.push(Operation::new("Td", vec![x.into(), y.into()]));
    ops.push(Operation::new("Tj", vec![Object::String(winansi(s), StringFormat::Literal)]));
    ops.push(Operation::new("ET", vec![]));
}

fn rule(ops: &mut Vec<Operation>, x1: f64, x2: f64, y: f64) {
    ops.push(Operation::new("w", vec![0.5.into()]));
    ops.push(Operation::new("m", vec![x1.into(), y.into()]));
    ops.push(Operation::new("l", vec![x2.into(), y.into()]));
    ops.push(Operation::new("S", vec![]));
}

/// Bytes als Grossbuchstaben-Hex in Zeilen zu 16 Bytes (`2F 8F B6 …`).
pub fn hex_rows(bytes: &[u8]) -> Vec<String> {
    bytes
        .chunks(16)
        .map(|row| row.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" "))
        .collect()
}

/// `2026-10-02T08:34:56Z` → (`02.10.2026`, `08:34:56 UTC`).
fn date_time(iso: &str) -> (String, String) {
    match (iso.get(0..4), iso.get(5..7), iso.get(8..10), iso.get(11..19)) {
        (Some(y), Some(m), Some(d), Some(t)) => (format!("{d}.{m}.{y}"), format!("{t} UTC")),
        _ => (iso.to_string(), String::new()),
    }
}

fn title(kind: KeyKind) -> (&'static str, &'static str) {
    match kind {
        KeyKind::Signature => ("INI", "Öffentlicher Schlüssel für die elektronische Unterschrift"),
        KeyKind::Authentication => ("HIA", "Öffentlicher Authentifikationsschlüssel"),
        KeyKind::Encryption => ("HIA", "Öffentlicher Verschlüsselungsschlüssel"),
    }
}

fn page(p: &Params, keys: &Keys, kind: KeyKind) -> Content {
    let mut ops = Vec::new();
    let key = keys.get(kind);
    let (modulus, exponent) = public_parts(key);
    let hash = public_key_hash(&modulus, &exponent);
    let (order, heading) = title(kind);
    let (date, time) = date_time(&keys.state.created);

    let mut y = PAGE_H - 70.0;
    text(&mut ops, BOLD, 15.0, LEFT, y, &format!("EBICS-Initialisierungsbrief ({order})"));
    y -= 20.0;
    text(&mut ops, REGULAR, 11.0, LEFT, y, heading);
    y -= 12.0;
    rule(&mut ops, LEFT, RIGHT, y);
    y -= 24.0;

    let user_name = if p.user_name.is_empty() { "—" } else { &p.user_name };
    for (label, value) in [
        ("Datum", date.as_str()),
        ("Uhrzeit", time.as_str()),
        ("Empfänger (Host-ID)", p.host_id.as_str()),
        ("Kunden-ID", p.partner_id.as_str()),
        ("Teilnehmer-ID", p.user_id.as_str()),
        ("Teilnehmername", user_name),
        ("Version", kind.version()),
    ] {
        text(&mut ops, REGULAR, 10.0, LEFT, y, label);
        text(&mut ops, BOLD, 10.0, VALUE_X, y, value);
        y -= 16.0;
    }

    y -= 10.0;
    text(&mut ops, BOLD, 10.0, LEFT, y, &format!("Exponent ({} Bit)", key.e().bits()));
    y -= 14.0;
    for row in hex_rows(&exponent) {
        text(&mut ops, MONO, 9.5, LEFT, y, &row);
        y -= 12.0;
    }

    y -= 10.0;
    text(&mut ops, BOLD, 10.0, LEFT, y, &format!("Modulus ({} Bit)", key.n().bits()));
    y -= 14.0;
    for row in hex_rows(&modulus) {
        text(&mut ops, MONO, 9.5, LEFT, y, &row);
        y -= 12.0;
    }

    y -= 10.0;
    text(&mut ops, BOLD, 10.0, LEFT, y, "Hashwert (SHA-256)");
    y -= 14.0;
    for row in hex_rows(&hash) {
        text(&mut ops, MONO, 9.5, LEFT, y, &row);
        y -= 12.0;
    }

    y -= 20.0;
    text(&mut ops, REGULAR, 10.0, LEFT, y, "Ich bestätige hiermit den obigen öffentlichen Schlüssel für meinen EBICS-Zugang.");

    y -= 60.0;
    for (x1, x2, label) in [(LEFT, 200.0, "Ort, Datum"), (220.0, 370.0, "Name / Firma"), (390.0, RIGHT, "Unterschrift")] {
        rule(&mut ops, x1, x2, y);
        text(&mut ops, REGULAR, 8.0, x1, y - 11.0, label);
    }
    Content { operations: ops }
}

/// Erzeugt den dreiseitigen INI-Brief (A006, X002, E002).
pub fn ini_brief_pdf(p: &Params, keys: &Keys) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut font = |base: &str| {
        doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => base,
            "Encoding" => "WinAnsiEncoding",
        })
    };
    let (regular, bold, mono) = (font("Helvetica"), font("Helvetica-Bold"), font("Courier"));
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { REGULAR => regular, BOLD => bold, MONO => mono },
    });

    let mut kids: Vec<Object> = Vec::new();
    for kind in [KeyKind::Signature, KeyKind::Authentication, KeyKind::Encryption] {
        let content = page(p, keys, kind);
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

    #[test]
    fn hex_rows_of_16_bytes() {
        let rows = hex_rows(&(0u8..20).collect::<Vec<_>>());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], "00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F");
        assert_eq!(rows[1], "10 11 12 13");
    }

    #[test]
    fn splits_iso_timestamp() {
        assert_eq!(date_time("2026-10-02T08:34:56Z"), ("02.10.2026".into(), "08:34:56 UTC".into()));
    }
}
