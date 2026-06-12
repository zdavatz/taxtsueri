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

    /// Baut den **extension-freien Kern** aus den Personendaten unseres v4-[`Document`]
    /// — validiert gegen die eCH-0119-v3-XSD.
    pub fn from_document(doc: &Document, tax_period: u16) -> Self {
        Self::build(doc, tax_period, None)
    }

    /// Baut die **barcode-fertige** v3-Nachricht: Kern + ZH-`cantonExtension` mit den
    /// berechneten Steuerwerten ([`ZhBarcodeData`]). Nicht gegen die Kern-XSD
    /// validierbar (strict wildcard ohne ZH-XSD) — strukturell gegen das Sample.
    pub fn from_document_with_data(doc: &Document, tax_period: u16, data: &ZhBarcodeData) -> Self {
        let ext = ZhHeaderCantonExtension {
            header_extension: ZhHeaderExtension {
                hidden_data: ZhHiddenData::default(),
                approval_receipt: ZhApprovalReceipt {
                    rounded_taxable_income: data.taxable_income.clone(),
                    rounded_ratedetermining_income: data.ratedetermining_income.clone(),
                    rounded_taxable_qualified_investments: data.taxable_qualified_investments,
                    rounded_taxable_asset: data.taxable_asset,
                    rounded_ratedetermining_asset: data.ratedetermining_asset,
                },
                source_system: ZhSourceSystem {
                    system: "taxtsueri".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    operating_system: std::env::consts::OS.into(),
                    date: data.date.clone(),
                },
                document_list: data.documents.clone(),
                version_fk: None,
                client_password_protection: false,
            },
            canton: data.canton.clone(),
        };
        Self::build(doc, tax_period, Some(ext))
    }

    fn build(doc: &Document, tax_period: u16, ext: Option<ZhHeaderCantonExtension>) -> Self {
        let p1 = &doc.content.main_form.person_data_partner1;
        let header = ZhHeader {
            canton_extension: ext,
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

/// Eingabe für die ZH-`cantonExtension`: die in der ZHprivateTax-Berechnung
/// ermittelten gerundeten Steuerwerte plus Belegliste. (Weg „b" des Plans —
/// Werte werden übernommen, nicht selbst berechnet.)
#[derive(Debug, Clone)]
pub struct ZhBarcodeData {
    pub canton: String,
    /// `zh:date` / Stichtag der Erzeugung (YYYY-MM-DD) — als Eingabe, da im Skript
    /// keine Systemzeit verfügbar ist.
    pub date: String,
    pub taxable_income: ZhTaxAmount,
    pub ratedetermining_income: ZhTaxAmount,
    pub taxable_qualified_investments: i64,
    pub taxable_asset: i64,
    pub ratedetermining_asset: i64,
    pub documents: Vec<ZhDocument>,
}

/// `headerType` (Ausschnitt). Reihenfolge laut XSD: …, `cantonExtension`, …,
/// `taxPeriod`, …, `source`, `sourceDescription`. `source`: 0 = Software,
/// 1 = 2D-Barcode, 2 = OCR.
#[derive(Debug, Serialize)]
pub struct ZhHeader {
    /// ZH-`cantonExtension` (`zh:headerExtension` + `ssk:canton`). Optional, weil
    /// `cantonExtension` ein `processContents="strict"`-Wildcard ist: ohne die
    /// (nicht-öffentliche) ZH-XSD validiert nur der **extension-freie** Kern gegen
    /// die eCH-0119-v3-XSD. Für den Barcode wird sie gesetzt.
    #[serde(rename = "ssk:cantonExtension", skip_serializing_if = "Option::is_none")]
    pub canton_extension: Option<ZhHeaderCantonExtension>,
    #[serde(rename = "ssk:taxPeriod")]
    pub tax_period: String,
    #[serde(rename = "ssk:source")]
    pub source: u8,
    #[serde(rename = "ssk:sourceDescription", skip_serializing_if = "Option::is_none")]
    pub source_description: Option<String>,
}

// ---------------------------------------------------------------------------
// ZH-cantonExtension (zh-taxdeclaration-it/ech3-0/6) — nur strukturell gegen
// das Real-Sample geprüft (keine öffentliche XSD).
// ---------------------------------------------------------------------------

/// `cantonExtensionType` im Header: `xs:any` (= `zh:headerExtension`) + `ssk:canton`.
#[derive(Debug, Serialize)]
pub struct ZhHeaderCantonExtension {
    #[serde(rename = "zh:headerExtension")]
    pub header_extension: ZhHeaderExtension,
    #[serde(rename = "ssk:canton")]
    pub canton: String,
}

/// `zh:headerExtension` — Reihenfolge laut Real-Sample.
#[derive(Debug, Serialize)]
pub struct ZhHeaderExtension {
    #[serde(rename = "zh:hiddenData")]
    pub hidden_data: ZhHiddenData,
    #[serde(rename = "zh:approvalReceipt")]
    pub approval_receipt: ZhApprovalReceipt,
    #[serde(rename = "zh:sourceSystem")]
    pub source_system: ZhSourceSystem,
    #[serde(rename = "zh:documentList", skip_serializing_if = "Vec::is_empty")]
    pub document_list: Vec<ZhDocument>,
    #[serde(rename = "zh:versionFK", skip_serializing_if = "Option::is_none")]
    pub version_fk: Option<String>,
    #[serde(rename = "zh:clientPasswordProtection")]
    pub client_password_protection: bool,
}

/// `zh:hiddenData` — Selbständigkeits-Flags (Default: keine Selbständigkeit).
#[derive(Debug, Serialize)]
pub struct ZhHiddenData {
    #[serde(rename = "zh:selfEmploymentP1")]
    pub self_employment_p1: bool,
    #[serde(rename = "zh:noSelfEmploymentP1")]
    pub no_self_employment_p1: bool,
    #[serde(rename = "zh:selfEmploymentP2")]
    pub self_employment_p2: bool,
    #[serde(rename = "zh:noSelfEmploymentP2")]
    pub no_self_employment_p2: bool,
    #[serde(rename = "zh:relevantCooperation")]
    pub relevant_cooperation: bool,
}

impl Default for ZhHiddenData {
    fn default() -> Self {
        Self {
            self_employment_p1: false,
            no_self_employment_p1: true,
            self_employment_p2: false,
            no_self_employment_p2: true,
            relevant_cooperation: false,
        }
    }
}

/// `zh:approvalReceipt` — die **berechneten** Steuerwerte (gerundet, kantonal+Bund).
/// Werte kommen als Eingabe (aus der ZHprivateTax-Berechnung), s. [`ZhBarcodeData`].
#[derive(Debug, Serialize)]
pub struct ZhApprovalReceipt {
    #[serde(rename = "zh:roundedTaxableIncome")]
    pub rounded_taxable_income: ZhTaxAmount,
    #[serde(rename = "zh:roundedRatedeterminingIncome")]
    pub rounded_ratedetermining_income: ZhTaxAmount,
    #[serde(rename = "zh:roundedTaxableQualifiedInvestments")]
    pub rounded_taxable_qualified_investments: i64,
    #[serde(rename = "zh:roundedTaxableAsset")]
    pub rounded_taxable_asset: i64,
    #[serde(rename = "zh:roundedRatedeterminingAsset")]
    pub rounded_ratedetermining_asset: i64,
}

/// `ssk:cantonalTax`/`ssk:federalTax` (eCH-0119-Namespace, daher `ssk:`).
#[derive(Debug, Clone, Serialize)]
pub struct ZhTaxAmount {
    #[serde(rename = "ssk:cantonalTax")]
    pub cantonal: i64,
    #[serde(rename = "ssk:federalTax")]
    pub federal: i64,
}

/// `zh:sourceSystem` — erzeugendes System (hier taxtsueri).
#[derive(Debug, Serialize)]
pub struct ZhSourceSystem {
    #[serde(rename = "zh:system")]
    pub system: String,
    #[serde(rename = "zh:version")]
    pub version: String,
    #[serde(rename = "zh:operatingSystem")]
    pub operating_system: String,
    #[serde(rename = "zh:date")]
    pub date: String,
}

/// `zh:documentList` — beigelegter Beleg (Typ/Übermittlungsart/Beschreibung).
#[derive(Debug, Clone, Serialize)]
pub struct ZhDocument {
    #[serde(rename = "zh:documentType")]
    pub document_type: String,
    #[serde(rename = "zh:documentDeliveryMethod")]
    pub document_delivery_method: String,
    #[serde(rename = "zh:documentDescription")]
    pub document_description: String,
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

    fn sample_data() -> ZhBarcodeData {
        ZhBarcodeData {
            canton: "ZH".into(),
            date: "2025-12-31".into(),
            taxable_income: ZhTaxAmount { cantonal: 104300, federal: 100500 },
            ratedetermining_income: ZhTaxAmount { cantonal: 104300, federal: 100500 },
            taxable_qualified_investments: 0,
            taxable_asset: 0,
            ratedetermining_asset: 0,
            documents: vec![ZhDocument {
                document_type: "01".into(),
                document_delivery_method: "01".into(),
                document_description: "Lohnausweis(e) pro Arbeitgeber".into(),
            }],
        }
    }

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
        // Kern trägt KEINE zh:-Extension (sonst nicht XSD-validierbar).
        assert!(!xml.contains("zh:headerExtension"));
    }

    #[test]
    fn builds_barcode_variant_with_zh_extension() {
        let doc = crate::dataset::example();
        let msg = ZhMessage::from_document_with_data(&doc, 2025, &sample_data());
        let xml = zh_message_to_xml(&msg).unwrap();
        // cantonExtension-Struktur wie im Real-Sample.
        assert!(xml.contains("<ssk:cantonExtension>"));
        assert!(xml.contains("<zh:headerExtension>"));
        assert!(xml.contains("<zh:approvalReceipt>"));
        assert!(xml.contains("<zh:roundedTaxableIncome>"));
        assert!(xml.contains("<ssk:cantonalTax>104300</ssk:cantonalTax>"));
        assert!(xml.contains("<zh:system>taxtsueri</zh:system>"));
        assert!(xml.contains("<zh:documentDescription>Lohnausweis(e) pro Arbeitgeber</zh:documentDescription>"));
        // canton steht NACH der Extension (XSD-Sequenz).
        let ext = xml.find("zh:headerExtension").unwrap();
        let canton = xml.find("<ssk:canton>").unwrap();
        assert!(ext < canton);
    }
}
