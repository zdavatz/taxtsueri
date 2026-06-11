//! Setzt das **2D-Barcode-Blatt** als A4-PDF zusammen (eCH-0196):
//! PDF417-Segmente (max. 6 pro Blatt) + CODE128C-Seitenbarcode, via `lopdf`.
//!
//! Module werden als **1-Bit-ImageMask**-XObjects eingebettet (schwarzes Modul =
//! gemalt). Masse nach Wegleitung: Modulbreite ≈ 0.42 mm, Zeilenhöhe ≈ 0.8 mm.

use lopdf::{dictionary, Document, Object, Stream};

const MM: f64 = 2.834_645_7; // pt pro mm
const PAGE_W: f64 = 297.0 * MM; // A4 quer
const PAGE_H: f64 = 210.0 * MM;
const MODULE_W: f64 = 0.42 * MM; // PDF417-Modulbreite
const ROW_H: f64 = 0.8 * MM; // PDF417-Zeilenhöhe
const SEG_GAP: f64 = 4.0 * MM;
const MARGIN: f64 = 12.0 * MM;
const BLOCKS_PER_SHEET: usize = 6;

/// Modulraster → 1-Bit-ImageMask-Daten (0 = malen/schwarz), Breite, Höhe.
fn pack_mask(grid: &[Vec<bool>]) -> (Vec<u8>, usize, usize) {
    let h = grid.len();
    let w = grid.first().map(|r| r.len()).unwrap_or(0);
    let row_bytes = (w + 7) / 8;
    let mut data = vec![0u8; row_bytes * h]; // 0 = malen
    for (y, row) in grid.iter().enumerate() {
        for (x, &m) in row.iter().enumerate() {
            if !m {
                data[y * row_bytes + x / 8] |= 0x80 >> (x % 8); // weiss = 1 = nicht malen
            }
        }
    }
    (data, w, h)
}

fn add_mask(doc: &mut Document, grid: &[Vec<bool>]) -> lopdf::ObjectId {
    let (data, w, h) = pack_mask(grid);
    let dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Image",
        "Width" => w as i64,
        "Height" => h as i64,
        "ImageMask" => true,
        "BitsPerComponent" => 1i64,
    };
    doc.add_object(Stream::new(dict, data))
}

/// Baut das Barcode-Blatt-PDF aus PDF417-Segment-Rastern + dem 1D-Seitenbarcode.
pub fn build_sheet_pdf(segments: &[Vec<Vec<bool>>], page_bits: &[bool]) -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();

    // 1D-Seitenbarcode als 1-zeiliges Raster.
    let page_grid = vec![page_bits.to_vec()];

    let mut page_ids = Vec::new();
    let chunks: Vec<&[Vec<Vec<bool>>]> = if segments.is_empty() {
        vec![&[][..]]
    } else {
        segments.chunks(BLOCKS_PER_SHEET).collect()
    };

    for chunk in chunks {
        let mut xobjects = lopdf::Dictionary::new();
        let mut content = String::from("0 0 0 rg\n"); // Füllfarbe schwarz

        // PDF417-Segmente von oben nach unten stapeln.
        let mut y = PAGE_H - MARGIN;
        for (i, grid) in chunk.iter().enumerate() {
            let w = grid.first().map(|r| r.len()).unwrap_or(0) as f64 * MODULE_W;
            let h = grid.len() as f64 * ROW_H;
            y -= h;
            let id = add_mask(&mut doc, grid);
            let name = format!("Im{i}");
            xobjects.set(name.clone(), Object::Reference(id));
            content.push_str(&format!("q {w:.2} 0 0 {h:.2} {MARGIN:.2} {y:.2} cm /{name} Do Q\n"));
            y -= SEG_GAP;
        }

        // 1D-Seitenbarcode unten links (ca. 60 mm breit, 10 mm hoch).
        let bw = 60.0 * MM;
        let bh = 10.0 * MM;
        let id = add_mask(&mut doc, &page_grid);
        xobjects.set("Im1d", Object::Reference(id));
        content.push_str(&format!(
            "q {bw:.2} 0 0 {bh:.2} {MARGIN:.2} {MARGIN:.2} cm /Im1d Do Q\n"
        ));

        let content_id = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), PAGE_W.into(), PAGE_H.into()],
            "Contents" => content_id,
            "Resources" => dictionary! { "XObject" => xobjects },
        });
        page_ids.push(Object::Reference(page_id));
    }

    let count = page_ids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids,
            "Count" => count,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("save pdf");
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{code128, pdf417};

    #[test]
    fn builds_valid_a4_pdf_with_images() {
        // Zwei PDF417-Segmente + ein CODE128-Seitenbarcode.
        let payload: Vec<u8> = (0..600u32).map(|i| b'A' + (i % 26) as u8).collect();
        let segments: Vec<Vec<Vec<bool>>> = pdf417::build_symbols(&payload, 13, 35, 4)
            .unwrap()
            .iter()
            .map(|s| pdf417::render(s, 13, 35, 4))
            .collect();
        let bits = code128::encode("1962200250001101").unwrap();
        let pdf = build_sheet_pdf(&segments, &bits);
        assert!(pdf.starts_with(b"%PDF-"));

        // Wieder laden: 1 Seite, ImageMask-XObjects vorhanden.
        let doc = Document::load_mem(&pdf).expect("reload");
        assert_eq!(doc.page_iter().count(), 1);
        let images = doc
            .objects
            .values()
            .filter(|o| matches!(o, Object::Stream(s) if s.dict.get(b"Subtype").ok().and_then(|v| v.as_name().ok()) == Some(b"Image")))
            .count();
        assert!(images >= 3, "≥3 Bild-XObjects (2 PDF417 + 1 CODE128), waren {images}");
    }
}
