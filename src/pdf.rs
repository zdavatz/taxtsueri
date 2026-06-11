//! Extraktion eingebetteter Dateien (PDF `/EmbeddedFiles`) aus einem PDF.
//!
//! eCH-0196-eSteuerauszüge der Banken hängen das maschinenlesbare XML als
//! eingebettete Datei an. Dieser Reader holt diese Anhänge heraus, damit sie
//! mit [`crate::ech0196`] geparst werden können.
//!
//! Hinweis: Reine *Scan*-PDFs (wie die Dr.-Tax-Barcode-Blätter im `pdf/`-Ordner)
//! enthalten keine eingebetteten Dateien – ihre Daten stecken in PDF417-Bildern
//! und benötigen einen Barcode-Bilddecoder (nicht Teil dieses Werkzeugs).

use lopdf::{Document, Object};
use std::path::Path;

/// Gibt alle eingebetteten Dateien als (Dateiname, Inhalt) zurück.
pub fn extract_embedded_files(path: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let doc = Document::load(path).map_err(|e| format!("PDF nicht lesbar: {e}"))?;
    let mut out = Vec::new();

    let root_ref = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(|e| format!("kein /Root: {e}"))?;
    let catalog = doc
        .get_dictionary(root_ref)
        .map_err(|e| format!("kein Katalog: {e}"))?;

    // /Names -> /EmbeddedFiles (Name-Tree)
    let names = match catalog.get(b"Names").ok().and_then(|o| deref_dict(&doc, o)) {
        Some(d) => d,
        None => return Ok(out), // keine Namen → keine Anhänge
    };
    let ef = match names.get(b"EmbeddedFiles").ok() {
        Some(o) => o.clone(),
        None => return Ok(out),
    };
    walk_name_tree(&doc, &ef, &mut out);
    Ok(out)
}

/// Wie [`extract_embedded_files`], aber nur XML-artige Anhänge.
pub fn extract_embedded_xml(path: &Path) -> Result<Vec<(String, String)>, String> {
    let files = extract_embedded_files(path)?;
    Ok(files
        .into_iter()
        .filter_map(|(name, bytes)| {
            let looks_xml = name.to_ascii_lowercase().ends_with(".xml")
                || bytes.starts_with(b"<?xml")
                || starts_with_bom_xml(&bytes);
            let text = String::from_utf8(bytes).ok()?;
            (looks_xml || text.contains("eCH-0196")).then_some((name, text))
        })
        .collect())
}

fn starts_with_bom_xml(b: &[u8]) -> bool {
    b.len() >= 5 && &b[0..3] == [0xEF, 0xBB, 0xBF] && &b[3..5] == b"<?"
}

fn deref_dict<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a lopdf::Dictionary> {
    doc.dereference(obj).ok().and_then(|(_, o)| o.as_dict().ok())
}

fn walk_name_tree(doc: &Document, node: &Object, out: &mut Vec<(String, Vec<u8>)>) {
    let Some(dict) = deref_dict(doc, node) else {
        return;
    };

    // Blattknoten: /Names [ name1 filespec1 name2 filespec2 ... ]
    if let Ok(Object::Array(arr)) = dict.get(b"Names") {
        let mut i = 0;
        while i + 1 < arr.len() {
            let name = object_to_string(doc, &arr[i]).unwrap_or_else(|| format!("anhang-{i}"));
            if let Some(bytes) = extract_filespec(doc, &arr[i + 1]) {
                out.push((name, bytes));
            }
            i += 2;
        }
    }

    // Zwischenknoten: /Kids [ ... ]
    if let Ok(Object::Array(kids)) = dict.get(b"Kids") {
        for kid in kids {
            walk_name_tree(doc, kid, out);
        }
    }
}

/// Filespec-Dict → /EF → /F (oder /UF) → Stream-Inhalt.
fn extract_filespec(doc: &Document, obj: &Object) -> Option<Vec<u8>> {
    let dict = deref_dict(doc, obj)?;
    let ef = dict.get(b"EF").ok().and_then(|o| deref_dict(doc, o))?;
    let stream_obj = ef.get(b"F").or_else(|_| ef.get(b"UF")).ok()?;
    let (_, resolved) = doc.dereference(stream_obj).ok()?;
    let stream = resolved.as_stream().ok()?;
    stream
        .decompressed_content()
        .ok()
        .or_else(|| Some(stream.content.clone()))
}

fn object_to_string(doc: &Document, obj: &Object) -> Option<String> {
    let (_, resolved) = doc.dereference(obj).ok()?;
    resolved
        .as_str()
        .ok()
        .map(|b| String::from_utf8_lossy(b).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Object, Stream};

    #[test]
    fn extracts_embedded_xml_roundtrip() {
        // Minimales PDF mit einer eingebetteten XML-Datei bauen …
        let xml = b"<?xml version=\"1.0\"?><taxStatement>eCH-0196</taxStatement>";
        let mut doc = Document::with_version("1.5");
        let content_id = doc.add_object(Stream::new(dictionary! {}, xml.to_vec()));
        let filespec = doc.add_object(dictionary! {
            "Type" => "Filespec",
            "F" => Object::string_literal("auszug.xml"),
            "EF" => dictionary! { "F" => Object::Reference(content_id) },
        });
        let names_dict = doc.add_object(dictionary! {
            "EmbeddedFiles" => dictionary! {
                "Names" => vec![Object::string_literal("auszug.xml"), Object::Reference(filespec)],
            },
        });
        let pages = doc.add_object(dictionary! { "Type" => "Pages", "Kids" => vec![], "Count" => 0 });
        let catalog = doc.add_object(dictionary! {
            "Type" => "Catalog", "Pages" => Object::Reference(pages),
            "Names" => Object::Reference(names_dict),
        });
        doc.trailer.set("Root", Object::Reference(catalog));

        let path = std::env::temp_dir().join("taxtsueri-embed-test.pdf");
        doc.save(&path).expect("save pdf");

        // … und wieder herauslesen.
        let xmls = extract_embedded_xml(&path).expect("extract");
        let _ = std::fs::remove_file(&path);

        assert_eq!(xmls.len(), 1);
        assert_eq!(xmls[0].0, "auszug.xml");
        assert!(xmls[0].1.contains("eCH-0196"));
    }
}
