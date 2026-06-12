//! taxtsueri — eCH-0119/eCH-0276-Steuererklärungs-Engine (Bibliothek).
//!
//! Die CLI (`src/main.rs`) und das Desktop-GUI (`src/bin/gui.rs`, Feature `gui`)
//! nutzen dieselben Module. Datenmodelle, Parser (eCH-0196, MT940,
//! Vermögensausweis), Barcode-Erzeugung und das Einreichungs-Paket leben hier.

pub mod barcode;
pub mod code128;
pub mod dataset;
pub mod dataset_jp;
pub mod ech0196;
pub mod model;
pub mod model_jp;
pub mod model_zh;
pub mod mt940;
pub mod pdf;
pub mod pdf417;
pub mod pdf_report;
pub mod settings;
pub mod sheet;
pub mod submit;
pub mod vermoegensausweis;

#[cfg(feature = "gui")]
pub mod update;

use model::Document;

/// Serialisiert ein [`Document`] zu eingerücktem eCH-0119-XML (mit Deklaration).
pub fn document_to_xml(doc: Document) -> Result<String, String> {
    let message = doc.into_message();
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let mut ser = quick_xml::se::Serializer::new(&mut xml);
    ser.indent(' ', 2);
    serde::Serialize::serialize(&message, ser).map_err(|e| e.to_string())?;
    xml.push('\n');
    Ok(xml)
}
