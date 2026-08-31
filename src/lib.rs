//! taxtsueri — eCH-0119/eCH-0276-Steuererklärungs-Engine (Bibliothek).
//!
//! Die CLI (`src/main.rs`) und das Desktop-GUI (`src/bin/gui.rs`, Feature `gui`)
//! nutzen dieselben Module. Datenmodelle, Parser (eCH-0196, MT940,
//! Vermögensausweis), die MWST-Abrechnung (eCH-0217), Barcode-Erzeugung und das
//! Einreichungs-Paket leben hier.

pub mod barcode;
pub mod camt053;
pub mod code128;
pub mod dataset;
pub mod dataset_jp;
pub mod ech0196;
pub mod model;
pub mod model_jp;
pub mod model_mwst;
pub mod model_zh;
pub mod mt940;
pub mod mwst;
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
    to_indented_xml(&message)
}

/// Serialisiert eine MWST-Abrechnung zu eingerücktem **eCH-0217**-XML.
pub fn mwst_to_xml(doc: model_mwst::Document) -> Result<String, String> {
    let message = doc.into_message();
    to_indented_xml(&message)
}

/// Gemeinsamer Serialisierer: XML-Deklaration + 2 Leerzeichen Einrückung.
fn to_indented_xml<T: serde::Serialize>(message: &T) -> Result<String, String> {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let mut ser = quick_xml::se::Serializer::new(&mut xml);
    ser.indent(' ', 2);
    message.serialize(ser).map_err(|e| e.to_string())?;
    xml.push('\n');
    Ok(xml)
}
