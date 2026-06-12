//! eCH-0119 **v3** Modell (ssk-prefixed) — Basis des **ZH-Steuererklärungs-Barcodes**.
//!
//! Der ZH-Barcode (`.ptax20`, von der «Private Tax»-Software erzeugt) ist eCH-0119
//! **Version 3** mit `ssk:`-Prefix (nicht Default-Namespace wie unser v4-Modell in
//! [`crate::model`]) plus eine ZH-`cantonExtension`
//! (`http://www.zh.ch/xmlns/zh-taxdeclaration-it/ech3-0/6`), deflate-komprimiert in
//! PDF417. Belegt durch das dekodierte Real-Sample in
//! `tests/fixtures/zh-barcode-sample.xml`.
//!
//! Dieses Modul modelliert den **Kern** (Header + mainForm-Personendaten), der gegen
//! `schema/eCH-0119-2015-3-0.xsd` validiert. Die `zh:`-Extension (Phase 2) folgt; sie
//! ist `processContents="strict"` und damit nur gegen die — nicht öffentliche —
//! ZH-XSD bzw. strukturell gegen das Sample prüfbar, nicht gegen die Kern-XSD.

use crate::model::{Document, SwissMunicipality};
use serde::Serialize;

const NS_0119_V3: &str = "http://www.ech.ch/xmlns/eCH-0119/3";
const NS_0044F: &str = "http://www.ech.ch/xmlns/eCH-0044-f/4";
const NS_0007F: &str = "http://www.ech.ch/xmlns/eCH-0007-f/6";
const NS_0011F: &str = "http://www.ech.ch/xmlns/eCH-0011-f/8";
const NS_ZH: &str = "http://www.zh.ch/xmlns/zh-taxdeclaration-it/ech3-0/6";

/// Wurzel `ssk:message` (Serialize-only — hält die `xmlns:`-Deklarationen).
#[derive(Debug, Serialize)]
#[serde(rename = "ssk:message")]
pub struct ZhMessage {
    #[serde(rename = "@xmlns:ssk")]
    xmlns_ssk: &'static str,
    #[serde(rename = "@xmlns:eCH-0044f")]
    xmlns_0044f: &'static str,
    #[serde(rename = "@xmlns:eCH-0007f")]
    xmlns_0007f: &'static str,
    #[serde(rename = "@xmlns:eCH-0011f")]
    xmlns_0011f: &'static str,
    #[serde(rename = "@xmlns:zh")]
    xmlns_zh: &'static str,
    #[serde(rename = "@minorVersion")]
    minor_version: u8,

    #[serde(rename = "ssk:header")]
    pub header: ZhHeader,
    #[serde(rename = "ssk:content")]
    pub content: ZhContent,
}

impl ZhMessage {
    pub fn new(header: ZhHeader, content: ZhContent) -> Self {
        Self {
            xmlns_ssk: NS_0119_V3,
            xmlns_0044f: NS_0044F,
            xmlns_0007f: NS_0007F,
            xmlns_0011f: NS_0011F,
            xmlns_zh: NS_ZH,
            minor_version: 3,
            header,
            content,
        }
    }

    /// Baut eine v3-Nachricht aus den Kern-Personendaten unseres v4-[`Document`].
    pub fn from_document(doc: &Document, tax_period: u16) -> Self {
        let p1 = &doc.content.main_form.person_data_partner1;
        let header = ZhHeader {
            tax_period: tax_period.to_string(),
            source: 1, // 1 = 2D-Barcode
            source_description: Some("taxtsueri".into()),
        };
        let ident = ZhPartnerIdentification {
            official_name: p1.identification.official_name.clone(),
            first_name: p1.identification.first_name.clone(),
            vn: p1.identification.vn,
        };
        let person1 = ZhPersonDataPartner1 {
            partner_person_identification: ident,
            tax_municipality: p1.tax_municipality.clone(),
        };
        let content = ZhContent {
            main_form: ZhMainForm {
                person_data_partner1: person1,
            },
        };
        Self::new(header, content)
    }
}

/// `headerType` (Ausschnitt). Reihenfolge laut XSD: …, `taxPeriod`, …, `source`,
/// `sourceDescription`. `source`: 0 = Software, 1 = 2D-Barcode, 2 = OCR.
#[derive(Debug, Serialize)]
pub struct ZhHeader {
    #[serde(rename = "ssk:taxPeriod")]
    pub tax_period: String,
    #[serde(rename = "ssk:source")]
    pub source: u8,
    #[serde(rename = "ssk:sourceDescription", skip_serializing_if = "Option::is_none")]
    pub source_description: Option<String>,
}

/// `contentType` (Ausschnitt) — vorerst nur `mainForm`.
#[derive(Debug, Serialize)]
pub struct ZhContent {
    #[serde(rename = "ssk:mainForm")]
    pub main_form: ZhMainForm,
}

/// `mainFormType` (Ausschnitt). `personDataPartner1` ist Pflicht.
#[derive(Debug, Serialize)]
pub struct ZhMainForm {
    #[serde(rename = "ssk:personDataPartner1")]
    pub person_data_partner1: ZhPersonDataPartner1,
}

/// `personDataPartner1Type` (Ausschnitt). Reihenfolge: `partnerPersonIdentification`,
/// …, `taxMunicipality`.
#[derive(Debug, Serialize)]
pub struct ZhPersonDataPartner1 {
    #[serde(rename = "ssk:partnerPersonIdentification")]
    pub partner_person_identification: ZhPartnerIdentification,
    #[serde(rename = "ssk:taxMunicipality", skip_serializing_if = "Option::is_none")]
    pub tax_municipality: Option<SwissMunicipality>,
}

/// `partnerPersonIdentificationType` (Ausschnitt): `officialName`, `firstName`, `vn`.
#[derive(Debug, Serialize)]
pub struct ZhPartnerIdentification {
    #[serde(rename = "ssk:officialName")]
    pub official_name: String,
    #[serde(rename = "ssk:firstName")]
    pub first_name: String,
    #[serde(rename = "ssk:vn")]
    pub vn: u64,
}

/// Serialisiert eine [`ZhMessage`] zu eingerücktem v3-XML (mit Deklaration).
pub fn zh_message_to_xml(message: &ZhMessage) -> Result<String, String> {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let mut ser = quick_xml::se::Serializer::new(&mut xml);
    ser.indent(' ', 2);
    serde::Serialize::serialize(message, ser).map_err(|e| e.to_string())?;
    xml.push('\n');
    Ok(xml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_minimal_valid_v3() {
        let doc = crate::dataset::example();
        let msg = ZhMessage::from_document(&doc, 2025);
        let xml = zh_message_to_xml(&msg).unwrap();
        assert!(xml.contains("<ssk:message"));
        assert!(xml.contains("minorVersion=\"3\""));
        assert!(xml.contains("xmlns:ssk=\"http://www.ech.ch/xmlns/eCH-0119/3\""));
        assert!(xml.contains("<ssk:taxPeriod>2025</ssk:taxPeriod>"));
        assert!(xml.contains("<ssk:partnerPersonIdentification>"));
    }
}
