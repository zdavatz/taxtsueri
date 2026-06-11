//! Erzeugung des **eCH-0196-2D-Barcodes** (Barcode-Blatt) — Fundament.
//!
//! Nach «eCH-0196 Beilage 2 – Barcode Generierung – Technische Wegleitung»:
//! der eSteuerauszug-XML wird **ZLIB-komprimiert** (Deflate, beste Stufe) und in
//! **PDF417 Structured Append** kodiert. Vorgaben pro Blatt: **6 Blöcke,
//! 13 Spalten, 35 Zeilen, EC-Level 4** (auch das letzte Segment auf 35 Zeilen).
//!
//! Dieses Modul liefert die fertig vorbereitete Nutzlast (komprimiert) + die
//! Barcode-ID + die Layout-Parameter. Die PDF417-Symbolerzeugung (Codewörter,
//! Reed-Solomon, Macro/Structured-Append, Bild) ist der nächste Schritt.

use flate2::{write::ZlibEncoder, Compression};
use std::io::Write;

/// PDF417-Layout laut eCH-0196 Beilage 2.
pub const COLUMNS: u8 = 13;
pub const ROWS: u8 = 35;
pub const EC_LEVEL: u8 = 4;
pub const BLOCKS_PER_SHEET: usize = 6;

/// ZLIB-Komprimierung (Deflate, beste Stufe) — wie in der Wegleitung gefordert.
pub fn compress(data: &[u8]) -> Vec<u8> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
    enc.write_all(data).expect("zlib write");
    enc.finish().expect("zlib finish")
}

/// eCH-0196-Barcode-ID (Kap. 2.1), ohne Trennzeichen:
/// Land(2) · Organisation · Seite(2) · AHVN13(13) · Stichtag JJJJMMTT(8) · lfdNr(2).
/// Muss mit alphanumerischem Zeichen beginnen (xs:ID) — das Länderkürzel erfüllt das.
#[allow(dead_code)] // Referenz-API zum ID-Aufbau, wenn das XML keine ID trägt.
pub fn build_id(country: &str, organisation: &str, page: u8, ahvn13: &str, stichtag: &str, seq: u8) -> String {
    format!("{country}{organisation}{page:02}{ahvn13}{stichtag}{seq:02}")
}

/// Vorbereitete Barcode-Nutzlast.
#[derive(Debug)]
pub struct Payload {
    pub id: String,
    pub compressed: Vec<u8>,
}

impl Payload {
    /// Grobe Schätzung der benötigten PDF417-Segmente (13×35, EC-Level 4).
    /// Datencodewörter/Segment ≈ Matrix − EC − Overhead; Byte-Compaction ≈ 6 Bytes/5 CW.
    pub fn estimated_segments(&self) -> usize {
        let ec = 1usize << (EC_LEVEL as usize + 1); // 2^(level+1) = 32 EC-Codewörter
        let data_cw = (COLUMNS as usize * ROWS as usize).saturating_sub(ec + 3); // -Length -SA-Overhead
        let bytes_per_segment = data_cw * 6 / 5; // Byte-Compaction-Verhältnis
        (self.compressed.len() + bytes_per_segment - 1) / bytes_per_segment.max(1)
    }
    pub fn sheets(&self) -> usize {
        (self.estimated_segments() + BLOCKS_PER_SHEET - 1) / BLOCKS_PER_SHEET
    }
}

/// Bereitet die Barcode-Nutzlast aus einem eCH-0196-XML vor (komprimieren + ID).
/// Die ID wird – falls vorhanden – dem `id`-Attribut des `taxStatement` entnommen
/// (es folgt bereits dem Format aus Kap. 2.1), sonst ein Platzhalter.
pub fn prepare(xml: &str) -> Payload {
    let id = extract_id(xml).unwrap_or_else(|| "CH0000000".to_string());
    Payload { id, compressed: compress(xml.as_bytes()) }
}

fn extract_id(xml: &str) -> Option<String> {
    let after = xml.split("taxStatement").nth(1)?;
    let start = after.find("id=\"")? + 4;
    let rest = &after[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    #[test]
    fn compress_roundtrips() {
        let original = b"<taxStatement>eCH-0196 Demo Inhalt</taxStatement>";
        let c = compress(original);
        assert!(c.len() < original.len() + 16); // komprimiert (oder ~gleich bei Kleinstdaten)
        let mut dec = ZlibDecoder::new(&c[..]);
        let mut back = Vec::new();
        dec.read_to_end(&mut back).unwrap();
        assert_eq!(back, original);
    }

    #[test]
    fn id_format() {
        // Land CH, Clearing 00250, Seite 01, AHVN13, Stichtag 20251231, lfdNr 01.
        let id = build_id("CH", "00250", 1, "7569087687779", "20251231", 1);
        assert_eq!(id, "CH002500175690876877792025123101");
        assert!(id.chars().next().unwrap().is_ascii_alphabetic()); // beginnt alphanumerisch
    }

    #[test]
    fn segment_estimate_reasonable() {
        let p = Payload { id: "X".into(), compressed: vec![0u8; 2000] };
        assert!(p.estimated_segments() >= 4 && p.estimated_segments() <= 8);
        assert!(p.sheets() >= 1);
    }
}
