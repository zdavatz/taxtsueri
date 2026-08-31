//! Modell der **MWST-Abrechnung** nach **eCH-0217 «Spezifikation E-MWST»
//! Version 2.0.0** (Genehmigt 2025-09-08).
//!
//! Das ESTV-Portal «MWST abrechnen» akzeptiert für den Datenimport
//! ausschliesslich dieses Format — ältere Versionen werden nicht mehr
//! verarbeitet. Das erzeugte XML **validiert gegen `schema/eCH-0217-2-0-0.xsd`**.
//!
//! Aufbau nach dem XSD und den vier offiziellen Beispieldateien
//! (`schema/eCH-0217_V2.0.0_example_*.xml`):
//! `VATDeclaration` → `generalInformation` + `turnoverComputation` +
//! **choice** (Abrechnungsmethode) + `payableTax` + optional `otherFlowsOfFunds`.
//!
//! # Namespaces
//!
//! Anders als eCH-0119 (Default-Namespace) trägt hier **jedes** Element den
//! Präfix `eCH-0217:` — mit einer Ausnahme: `sendingApplication` ist vom Typ
//! `eCH-0058:sendingApplicationType`, die lokalen Kinder dieses Typs leben
//! deshalb im Namespace **eCH-0058** (`eCH-0058:manufacturer` usw.). Das ist
//! dieselbe Regel wie bei den Cross-Schema-Teilbäumen in `model.rs`.
//!
//! # Abrechnungsmethode
//!
//! Es gibt kein Flag für die Methode — sie steckt im **Namen des choice-
//! Elements**. Modelliert sind die beiden heute relevanten Zweige:
//!
//! * `effectiveReportingMethod` — effektive Methode,
//! * `simpleTaxRateMethod` — Saldo-/Pauschalsteuersatz, **Abrechnungsperioden
//!   ab 01.01.2025** (verlangt die 5-stellige `activityID`).
//!
//! `netTaxRateMethod`/`flatTaxRateMethod` gelten nur für Perioden bis
//! 31.12.2024 und werden hier nicht emittiert (wir modellieren nur, was wir
//! schreiben — vgl. CLAUDE.md).
//!
//! # Beträge
//!
//! `amountType` ist `xs:decimal` mit `fractionDigits="2"`. Intern rechnen wir
//! wie in `mt940.rs` in **Rappen** (`i64`), serialisiert wird rappengenau mit
//! zwei Nachkommastellen. `percentType` (0..100, 2 Nachkommastellen) liegt als
//! Hundertstel-Prozent vor (620 = 6.20 %).

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

// --------------------------------------------------------------------------- //
// Skalare: amountType (Rappen) und percentType (Hundertstel-Prozent)
// --------------------------------------------------------------------------- //

/// `eCH-0217:amountType` — `xs:decimal`, 2 Nachkommastellen. Intern **Rappen**.
///
/// Vorzeichen ist erlaubt: `payableTax` ist negativ, wenn ein Guthaben der
/// steuerpflichtigen Person besteht (Ziff. 510 statt 500).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Amount(pub i64);

impl Amount {
    /// Betrag in Rappen.
    pub fn rappen(self) -> i64 {
        self.0
    }

    /// `"123456.78"`, `"123'456.78"`, `"123456,78"` → `Amount`.
    pub fn parse_chf(s: &str) -> Result<Self, String> {
        let cleaned: String = s
            .chars()
            .filter(|c| !matches!(c, '\'' | '’' | ' ' | '_'))
            .map(|c| if c == ',' { '.' } else { c })
            .collect();
        let neg = cleaned.starts_with('-');
        let body = cleaned.trim_start_matches(['-', '+']);
        let (whole, frac) = match body.split_once('.') {
            Some((w, f)) => (w, f),
            None => (body, ""),
        };
        if whole.is_empty() && frac.is_empty() {
            return Err(format!("kein Betrag: {s:?}"));
        }
        if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!("ungültiger Betrag: {s:?}"));
        }
        if frac.len() > 2 {
            return Err(format!("mehr als 2 Nachkommastellen: {s:?}"));
        }
        let whole: i64 = if whole.is_empty() { 0 } else { whole.parse().map_err(|_| format!("Betrag zu gross: {s:?}"))? };
        let frac: i64 = match frac.len() {
            0 => 0,
            1 => frac.parse::<i64>().unwrap() * 10,
            _ => frac.parse::<i64>().unwrap(),
        };
        let v = whole * 100 + frac;
        Ok(Amount(if neg { -v } else { v }))
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let a = self.0.abs();
        write!(f, "{sign}{}.{:02}", a / 100, a % 100)
    }
}

impl Serialize for Amount {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Amount {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Amount;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("CHF-Betrag als Zahl oder Zeichenkette")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Amount, E> {
                Amount::parse_chf(v).map_err(E::custom)
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Amount, E> {
                Ok(Amount((v * 100.0).round() as i64))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Amount, E> {
                Ok(Amount(v * 100))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Amount, E> {
                Ok(Amount(v as i64 * 100))
            }
        }
        d.deserialize_any(V)
    }
}

/// `eCH-0217:percentType` — 0..100, 2 Nachkommastellen. Intern **Hundertstel-Prozent**
/// (620 = 6.20 %), damit die Steuerberechnung ohne Fliesskomma auskommt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Percent(pub i64);

impl Percent {
    /// `"6.2"`, `"6,2"`, `"8.1"` → `Percent`.
    pub fn parse(s: &str) -> Result<Self, String> {
        let a = Amount::parse_chf(s.trim_end_matches('%').trim())?;
        if a.0 < 0 || a.0 > 10_000 {
            return Err(format!("Steuersatz ausserhalb 0..100: {s:?}"));
        }
        Ok(Percent(a.0))
    }
}

impl fmt::Display for Percent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Nachlaufende Nullen weglassen: 620 → "6.2", 800 → "8", 815 → "8.15".
        let (w, c) = (self.0 / 100, self.0 % 100);
        if c == 0 {
            write!(f, "{w}")
        } else if c % 10 == 0 {
            write!(f, "{w}.{}", c / 10)
        } else {
            write!(f, "{w}.{c:02}")
        }
    }
}

impl Serialize for Percent {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Percent {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Percent;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("Steuersatz in Prozent")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Percent, E> {
                Percent::parse(v).map_err(E::custom)
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Percent, E> {
                Ok(Percent((v * 100.0).round() as i64))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Percent, E> {
                Ok(Percent(v * 100))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Percent, E> {
                Ok(Percent(v as i64 * 100))
            }
        }
        d.deserialize_any(V)
    }
}

// --------------------------------------------------------------------------- //
// JSON-Eingabe + XML-Wurzel
// --------------------------------------------------------------------------- //

/// JSON-I/O-Hülle: dieselben Daten wie das XML, aber ohne die `xmlns:`-Plumbing.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Document {
    pub general_information: GeneralInformation,
    pub turnover_computation: TurnoverComputation,
    /// Effektive Methode — schliesst `simple_tax_rate_method` aus (XSD-`xs:choice`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_reporting_method: Option<EffectiveReportingMethod>,
    /// Saldo-/Pauschalsteuersatz ab Abrechnungsperiode 01.01.2025.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub simple_tax_rate_method: Option<SimpleTaxRateMethod>,
    pub payable_tax: Amount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_flows_of_funds: Option<OtherFlowsOfFunds>,
}

impl Document {
    pub fn into_message(self) -> Message {
        Message::new(self)
    }
}

/// XML-Wurzel `eCH-0217:VATDeclaration` inklusive der `xmlns:`-Deklarationen.
/// Nur `Serialize` — gelesen wird über [`Document`].
#[derive(Debug, Serialize)]
#[serde(rename = "eCH-0217:VATDeclaration")]
pub struct Message {
    #[serde(rename = "@xmlns:eCH-0217")]
    xmlns_0217: &'static str,
    #[serde(rename = "@xmlns:eCH-0058")]
    xmlns_0058: &'static str,
    #[serde(rename = "@xmlns:eCH-0108")]
    xmlns_0108: &'static str,
    #[serde(rename = "eCH-0217:generalInformation")]
    pub general_information: GeneralInformation,
    #[serde(rename = "eCH-0217:turnoverComputation")]
    pub turnover_computation: TurnoverComputation,
    #[serde(rename = "eCH-0217:effectiveReportingMethod", skip_serializing_if = "Option::is_none")]
    pub effective_reporting_method: Option<EffectiveReportingMethod>,
    #[serde(rename = "eCH-0217:simpleTaxRateMethod", skip_serializing_if = "Option::is_none")]
    pub simple_tax_rate_method: Option<SimpleTaxRateMethod>,
    #[serde(rename = "eCH-0217:payableTax")]
    pub payable_tax: Amount,
    #[serde(rename = "eCH-0217:otherFlowsOfFunds", skip_serializing_if = "Option::is_none")]
    pub other_flows_of_funds: Option<OtherFlowsOfFunds>,
}

impl Message {
    pub fn new(d: Document) -> Self {
        Self {
            xmlns_0217: "http://www.ech.ch/xmlns/eCH-0217/2",
            xmlns_0058: "http://www.ech.ch/xmlns/eCH-0058/5",
            xmlns_0108: "http://www.ech.ch/xmlns/eCH-0108/7",
            general_information: d.general_information,
            turnover_computation: d.turnover_computation,
            effective_reporting_method: d.effective_reporting_method,
            simple_tax_rate_method: d.simple_tax_rate_method,
            payable_tax: d.payable_tax,
            other_flows_of_funds: d.other_flows_of_funds,
        }
    }
}

// --------------------------------------------------------------------------- //
// generalInformationType
// --------------------------------------------------------------------------- //

/// Angaben zur steuerpflichtigen Person und zur Abrechnungsperiode.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralInformation {
    /// `eCH-0108:uidType` — Muster `CHE[1-9][0-9]{8}`, also **ohne** Punkte und
    /// ohne den Zusatz «MWST» (aus «CHE-123.456.789 MWST» wird `CHE123456789`).
    #[serde(rename = "eCH-0217:uid")]
    pub uid: String,
    /// Firmenname, 1..255 Zeichen.
    #[serde(rename = "eCH-0217:organisationName")]
    pub organisation_name: String,
    /// Erstellungszeitpunkt der Datei (`xs:dateTime`, z. B. `2026-08-31T10:04:00Z`).
    #[serde(rename = "eCH-0217:generationTime")]
    pub generation_time: String,
    #[serde(rename = "eCH-0217:reportingPeriodFrom")]
    pub reporting_period_from: String,
    #[serde(rename = "eCH-0217:reportingPeriodTill")]
    pub reporting_period_till: String,
    /// 1 = Ersteinreichung, 2 = Korrekturabrechnung, 3 = Jahresabstimmung.
    #[serde(rename = "eCH-0217:typeOfSubmission")]
    pub type_of_submission: u8,
    /// 1 = vereinbart (Art. 39 Abs. 1 MWSTG), 2 = vereinnahmt (bewilligungspflichtig).
    #[serde(rename = "eCH-0217:formOfReporting")]
    pub form_of_reporting: u8,
    /// Freie Geschäftsreferenz, 1..50 Zeichen.
    #[serde(rename = "eCH-0217:businessReferenceId")]
    pub business_reference_id: String,
    #[serde(rename = "eCH-0217:sendingApplication")]
    pub sending_application: SendingApplication,
}

/// `eCH-0058:sendingApplicationType` — die Kinder liegen im Namespace des Typs
/// (eCH-0058), nicht in eCH-0217.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SendingApplication {
    /// max. 30 Zeichen.
    #[serde(rename = "eCH-0058:manufacturer")]
    pub manufacturer: String,
    /// max. 30 Zeichen.
    #[serde(rename = "eCH-0058:product")]
    pub product: String,
    /// max. 10 Zeichen.
    #[serde(rename = "eCH-0058:productVersion")]
    pub product_version: String,
}

impl Default for SendingApplication {
    fn default() -> Self {
        Self {
            // Neutraler Default; wer als Hersteller die eigene Firma ausweisen
            // will, setzt `mwst.manufacturer` in `settings.json` (gitignored).
            manufacturer: "taxtsueri".into(),
            product: "taxtsueri".into(),
            product_version: env!("CARGO_PKG_VERSION").into(),
        }
    }
}

// --------------------------------------------------------------------------- //
// turnoverComputationType — Teil I des Formulars (Ziff. 2xx)
// --------------------------------------------------------------------------- //

/// Umsatzberechnung (Teil I). Reihenfolge = `xs:sequence` des XSD.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TurnoverComputation {
    /// Ziff. 200 — Total der vereinbarten bzw. vereinnahmten Entgelte, inkl.
    /// optierter Leistungen, Meldeverfahren und Leistungen im Ausland (weltweit).
    #[serde(rename = "eCH-0217:totalConsideration")]
    pub total_consideration: Amount,
    /// Ziff. 220 — von der Steuer befreite Leistungen (u. a. Exporte).
    #[serde(rename = "eCH-0217:suppliesToForeignCountries", skip_serializing_if = "Option::is_none")]
    pub supplies_to_foreign_countries: Option<Amount>,
    /// Ziff. 221 — Leistungen im Ausland (Ort der Leistung im Ausland).
    #[serde(rename = "eCH-0217:suppliesAbroad", skip_serializing_if = "Option::is_none")]
    pub supplies_abroad: Option<Amount>,
    /// Ziff. 225 — Übertragung im Meldeverfahren.
    #[serde(rename = "eCH-0217:transferNotificationProcedure", skip_serializing_if = "Option::is_none")]
    pub transfer_notification_procedure: Option<Amount>,
    /// Ziff. 230 — von der Steuer ausgenommene Inlandleistungen ohne Option.
    #[serde(rename = "eCH-0217:suppliesExemptFromTax", skip_serializing_if = "Option::is_none")]
    pub supplies_exempt_from_tax: Option<Amount>,
    /// Ziff. 235 — Entgeltsminderungen (Skonti, Rabatte).
    #[serde(rename = "eCH-0217:reductionOfConsideration", skip_serializing_if = "Option::is_none")]
    pub reduction_of_consideration: Option<Amount>,
    /// Ziff. 280 — Diverses (z. B. Wert des Bodens).
    #[serde(rename = "eCH-0217:variousDeduction", skip_serializing_if = "Option::is_none")]
    pub various_deduction: Option<VariousDeduction>,
}

impl TurnoverComputation {
    /// Ziff. 289 — Total der Abzüge (Ziff. 220 bis 280).
    pub fn total_deductions(&self) -> Amount {
        let o = |a: &Option<Amount>| a.map(|x| x.0).unwrap_or(0);
        Amount(
            o(&self.supplies_to_foreign_countries)
                + o(&self.supplies_abroad)
                + o(&self.transfer_notification_procedure)
                + o(&self.supplies_exempt_from_tax)
                + o(&self.reduction_of_consideration)
                + self.various_deduction.as_ref().map(|v| v.amount.0).unwrap_or(0),
        )
    }

    /// Ziff. 299 — steuerbarer Gesamtumsatz (Ziff. 200 abzüglich Ziff. 289).
    pub fn taxable_turnover(&self) -> Amount {
        Amount(self.total_consideration.0 - self.total_deductions().0)
    }
}

/// Ziff. 280 — «Diverses», Betrag plus Beschreibung (max. 50 Zeichen).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VariousDeduction {
    #[serde(rename = "eCH-0217:amountVariousDeduction")]
    pub amount: Amount,
    #[serde(rename = "eCH-0217:descriptionVariousDeduction")]
    pub description: String,
}

// --------------------------------------------------------------------------- //
// Steuerberechnung — Teil II des Formulars (Ziff. 3xx/4xx)
// --------------------------------------------------------------------------- //

/// `turnoverTaxRateType` — Umsatz pro **gesetzlichem** Steuersatz.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TurnoverTaxRate {
    #[serde(rename = "eCH-0217:taxRate")]
    pub tax_rate: Percent,
    #[serde(rename = "eCH-0217:turnover")]
    pub turnover: Amount,
}

/// `activityIDTurnoverTaxRateType` — Umsatz pro **Tätigkeit** (ab 01.01.2025).
///
/// `activityID` ist ein genau 5-stelliger Code. Es dürfen nur **bewilligte**
/// Codes übermittelt werden; sie stehen in der ESTV-Applikation «MWST
/// abrechnen» unter «Abrechnungsmodalitäten» bzw. in den Subformularen.
/// Für Umsätze aus Leistungen vor 2025 gibt es die technischen Codes
/// `T0001`..`T0020` (siehe [`TECHNICAL_ACTIVITY_IDS`]).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ActivityTurnoverTaxRate {
    #[serde(rename = "eCH-0217:activityID")]
    pub activity_id: String,
    #[serde(rename = "eCH-0217:taxRate")]
    pub tax_rate: Percent,
    #[serde(rename = "eCH-0217:turnover")]
    pub turnover: Amount,
}

/// Abschliessende Liste der **technischen** `activityID` (eCH-0217 Tabelle 15) —
/// nur für Umsätze aus Leistungen der Jahre 2023/2024, die erst in einer
/// Abrechnungsperiode ab 01.01.2025 deklariert werden.
pub const TECHNICAL_ACTIVITY_IDS: &[(&str, &str, i64)] = &[
    ("T0001", "Leistungen 2024 (0.1%)", 10),
    ("T0002", "Leistungen 2024 (0.6%)", 60),
    ("T0003", "Leistungen 2024 (1.3%)", 130),
    ("T0004", "Leistungen 2024 (2.1%)", 210),
    ("T0005", "Leistungen 2024 (3.0%)", 300),
    ("T0006", "Leistungen 2024 (3.7%)", 370),
    ("T0007", "Leistungen 2024 (4.5%)", 450),
    ("T0008", "Leistungen 2024 (5.3%)", 530),
    ("T0009", "Leistungen 2024 (6.2%)", 620),
    ("T0010", "Leistungen 2024 (6.8%)", 680),
    ("T0011", "Leistungen 2023 (0.1%)", 10),
    ("T0012", "Leistungen 2023 (0.6%)", 60),
    ("T0013", "Leistungen 2023 (1.2%)", 120),
    ("T0014", "Leistungen 2023 (2.0%)", 200),
    ("T0015", "Leistungen 2023 (2.8%)", 280),
    ("T0016", "Leistungen 2023 (3.5%)", 350),
    ("T0017", "Leistungen 2023 (4.3%)", 430),
    ("T0018", "Leistungen 2023 (5.1%)", 510),
    ("T0019", "Leistungen 2023 (5.9%)", 590),
    ("T0020", "Leistungen 2023 (6.5%)", 650),
];

/// Effektive Methode (Teil II, Ziff. 205 + 3xx/4xx).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EffectiveReportingMethod {
    /// 1 = Netto (ESTV-Empfehlung), 2 = Brutto (inkl. MWST).
    #[serde(rename = "eCH-0217:grossOrNet")]
    pub gross_or_net: u8,
    /// Ziff. 205 — in Ziff. 200 enthaltene optierte Entgelte.
    #[serde(rename = "eCH-0217:opted", skip_serializing_if = "Option::is_none")]
    pub opted: Option<Amount>,
    /// Ziff. 300–379 — Leistungen pro gesetzlichem Steuersatz.
    #[serde(rename = "eCH-0217:suppliesPerTaxRate", skip_serializing_if = "Vec::is_empty")]
    pub supplies_per_tax_rate: Vec<TurnoverTaxRate>,
    /// Ziff. 38x — Bezugsteuer (Art. 45), immer **netto** und zum gesetzlichen Satz.
    #[serde(rename = "eCH-0217:acquisitionTax", skip_serializing_if = "Vec::is_empty")]
    pub acquisition_tax: Vec<TurnoverTaxRate>,
    /// Ziff. 400 — Vorsteuer auf Material- und Dienstleistungsaufwand.
    #[serde(rename = "eCH-0217:inputTaxMaterialAndServices", skip_serializing_if = "Option::is_none")]
    pub input_tax_material_and_services: Option<Amount>,
    /// Ziff. 405 — Vorsteuer auf Investitionen und übrigem Betriebsaufwand.
    #[serde(rename = "eCH-0217:inputTaxInvestments", skip_serializing_if = "Option::is_none")]
    pub input_tax_investments: Option<Amount>,
    /// Ziff. 410 — Einlageentsteuerung (Art. 32).
    #[serde(rename = "eCH-0217:subsequentInputTaxDeduction", skip_serializing_if = "Option::is_none")]
    pub subsequent_input_tax_deduction: Option<Amount>,
    /// Ziff. 415 — Vorsteuerkorrekturen (Art. 30/31).
    #[serde(rename = "eCH-0217:inputTaxCorrections", skip_serializing_if = "Option::is_none")]
    pub input_tax_corrections: Option<Amount>,
    /// Ziff. 420 — Vorsteuerkürzungen (Art. 33 Abs. 2).
    #[serde(rename = "eCH-0217:inputTaxReductions", skip_serializing_if = "Option::is_none")]
    pub input_tax_reductions: Option<Amount>,
}

/// Saldo-/Pauschalsteuersatzmethode für Abrechnungsperioden **ab 01.01.2025**.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SimpleTaxRateMethod {
    /// Ziff. 300–379 — Umsatz und Saldo-/Pauschalsteuersatz pro Tätigkeit,
    /// **immer brutto** (inkl. MWST).
    #[serde(rename = "eCH-0217:suppliesPerTaxRate", skip_serializing_if = "Vec::is_empty")]
    pub supplies_per_tax_rate: Vec<ActivityTurnoverTaxRate>,
    /// Ziff. 38x — Bezugsteuer, netto und zum **gesetzlichen** Satz (nicht SSS).
    #[serde(rename = "eCH-0217:acquisitionTax", skip_serializing_if = "Vec::is_empty")]
    pub acquisition_tax: Vec<TurnoverTaxRate>,
    /// Ziff. 415 — Korrekturen bei unbeweglichen Gegenständen (Art. 82 Abs. 2 /
    /// Art. 93 MWSTV).
    #[serde(rename = "eCH-0217:inputTaxCorrections", skip_serializing_if = "Option::is_none")]
    pub input_tax_corrections: Option<Amount>,
}

// --------------------------------------------------------------------------- //
// otherFlowsOfFundsType — Teil III (Ziff. 9xx)
// --------------------------------------------------------------------------- //

/// Andere Mittelflüsse (Art. 18 Abs. 2), die nicht in Teil I deklariert wurden.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OtherFlowsOfFunds {
    /// Ziff. 900 — Subventionen, Tourismusabgaben, Entsorgungsbeiträge (Bst. a–c).
    #[serde(rename = "eCH-0217:subsidies", skip_serializing_if = "Option::is_none")]
    pub subsidies: Option<Amount>,
    /// Ziff. 910 — Spenden, **Dividenden**, Schadenersatz usw. (Bst. d–l).
    #[serde(rename = "eCH-0217:donations", skip_serializing_if = "Option::is_none")]
    pub donations: Option<Amount>,
}

// --------------------------------------------------------------------------- //
// Steuerberechnung (eCH-0217 Kap. 6.2) + Rundung
// --------------------------------------------------------------------------- //

/// Rundungsart für `payableTax` (eCH-0217 Kap. 6.2.1 / 7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rounding {
    /// Kaufmännisch auf 2 Nachkommastellen — hier ein No-op, weil intern bereits
    /// rappengenau gerundet wird.
    Rappen,
    /// Auf die nächsten 5 Rappen **zu Gunsten der steuerpflichtigen Unternehmung**,
    /// also stets abwärts: eine positive Schuld wird kleiner, ein Guthaben
    /// (negativer Betrag) grösser. So rechnet auch die ESTV in ihren eigenen
    /// Abrechnungsbelegen.
    FiveRappen,
}

/// Rundet einen Rappen-Betrag gemäss [`Rounding`].
pub fn round_payable(rappen: i64, mode: Rounding) -> i64 {
    match mode {
        Rounding::Rappen => rappen,
        // Abrunden Richtung -unendlich (Euclidean division), nicht Richtung null.
        Rounding::FiveRappen => rappen.div_euclid(5) * 5,
    }
}

/// eCH-0217 Kap. 7.5: «Das Element payableTax muss **ohne Runden der Zwischen-
/// schritte** berechnet werden.» Deshalb summieren wir in **Mikro-Rappen**
/// (Rappen × 10^6) und runden erst ganz am Schluss ([`micro_to_rappen`]).
const MICRO: i128 = 1_000_000;

/// Rappen → Mikro-Rappen.
fn micro(rappen: i64) -> i128 {
    rappen as i128 * MICRO
}

/// Mikro-Rappen → Rappen, kaufmännisch gerundet. Nur am Ende einer Berechnung.
fn micro_to_rappen(m: i128) -> i64 {
    round_div(m, MICRO)
}

/// Steuer aus `suppliesPerTaxRate` der **SSS/PSS**-Methode, in Mikro-Rappen.
fn tax_on_activities(rows: &[ActivityTurnoverTaxRate]) -> i128 {
    rows.iter().map(|r| mul_rate(r.turnover.0, r.tax_rate.0)).sum()
}

/// Summe `taxRate/100 × turnover` über Umsätze zum gesetzlichen Satz (netto),
/// in Mikro-Rappen. Ganzzahlig exakt, ohne Zwischenrundung.
fn tax_on_net(rows: &[TurnoverTaxRate]) -> i128 {
    rows.iter().map(|r| mul_rate(r.turnover.0, r.tax_rate.0)).sum()
}

/// Summe `(taxRate/100)/(1 + taxRate/100) × turnover` — Steuer, die in **Brutto**-
/// Umsätzen bereits steckt; in Mikro-Rappen.
fn tax_on_gross(rows: &[TurnoverTaxRate]) -> i128 {
    rows.iter()
        .map(|r| {
            // rappen × rate / (10000 + rate), rate in Hundertstel-Prozent.
            let num = r.turnover.0 as i128 * r.tax_rate.0 as i128 * MICRO;
            let den = 10_000i128 + r.tax_rate.0 as i128;
            round_div_i128(num, den)
        })
        .sum()
}

/// `rappen × rate/10000` in Mikro-Rappen — exakt, weil `MICRO/10_000 = 100`.
fn mul_rate(rappen: i64, rate_hundredths_percent: i64) -> i128 {
    rappen as i128 * rate_hundredths_percent as i128 * (MICRO / 10_000)
}

/// Kaufmännische Division auf i128 (halbe Beträge von der Null weg).
fn round_div_i128(num: i128, den: i128) -> i128 {
    debug_assert!(den > 0);
    if num >= 0 {
        (num * 2 + den) / (den * 2)
    } else {
        -((-num * 2 + den) / (den * 2))
    }
}

/// Wie [`round_div_i128`], aber mit Rückgabe als `i64`.
fn round_div(num: i128, den: i128) -> i64 {
    round_div_i128(num, den) as i64
}

/// Steuerbetrag einer einzelnen Zeile `suppliesPerTaxRate` — nur für die **Anzeige**
/// (Spalte «Steuer CHF» des Formulars). In `payableTax` fliesst stets der
/// ungerundete Wert ein, deshalb kann die Summe der angezeigten Zeilenbeträge
/// um Rappen von Ziff. 399 abweichen.
pub fn line_tax(turnover: Amount, rate: Percent, gross: bool) -> Amount {
    let rows = [TurnoverTaxRate { tax_rate: rate, turnover }];
    Amount(micro_to_rappen(if gross { tax_on_gross(&rows) } else { tax_on_net(&rows) }))
}

impl Document {
    /// Berechnet `payableTax` (Ziff. 500 bzw. 510) exakt nach eCH-0217 Kap. 6.2
    /// und rundet erst am Schluss.
    ///
    /// Positiver Wert = an die ESTV zu bezahlen, negativer = Guthaben.
    pub fn compute_payable_tax(&self, mode: Rounding) -> Amount {
        let mut m6 = self.total_tax_due_micro();
        if let Some(m) = &self.simple_tax_rate_method {
            // Tabelle 25: geschuldete Steuer + Ziff. 415.
            m6 += micro(m.input_tax_corrections.map(|a| a.0).unwrap_or(0));
        }
        if let Some(m) = &self.effective_reporting_method {
            // Tabelle 22: geschuldete Steuer abzüglich Vorsteuern, zuzüglich
            // Korrekturen und Kürzungen.
            m6 -= micro(m.input_tax_material_and_services.map(|a| a.0).unwrap_or(0));
            m6 -= micro(m.input_tax_investments.map(|a| a.0).unwrap_or(0));
            m6 -= micro(m.subsequent_input_tax_deduction.map(|a| a.0).unwrap_or(0));
            m6 += micro(m.input_tax_corrections.map(|a| a.0).unwrap_or(0));
            m6 += micro(m.input_tax_reductions.map(|a| a.0).unwrap_or(0));
        }
        Amount(round_payable(micro_to_rappen(m6), mode))
    }

    /// Total geschuldete Steuer in Mikro-Rappen, ungerundet.
    fn total_tax_due_micro(&self) -> i128 {
        let mut m6 = 0i128;
        if let Some(m) = &self.simple_tax_rate_method {
            m6 += tax_on_activities(&m.supplies_per_tax_rate) + tax_on_net(&m.acquisition_tax);
        }
        if let Some(m) = &self.effective_reporting_method {
            m6 += if m.gross_or_net == 2 {
                tax_on_gross(&m.supplies_per_tax_rate)
            } else {
                tax_on_net(&m.supplies_per_tax_rate)
            };
            m6 += tax_on_net(&m.acquisition_tax);
        }
        m6
    }

    /// Ziff. 399 — total geschuldete Steuer **vor** Anrechnungen und Vorsteuer
    /// (im XSD nicht abgebildet, aber auf dem Papierformular ausgewiesen).
    pub fn total_tax_due(&self) -> Amount {
        Amount(micro_to_rappen(self.total_tax_due_micro()))
    }

    /// Summe der Leistungen aus Teil II (eCH-0217 Kap. 6.4). Muss dem steuerbaren
    /// Gesamtumsatz (Ziff. 299) entsprechen, sonst weist die ESTV die Datei mit
    /// «MWST-0005» zurück.
    pub fn supplies_total(&self) -> Amount {
        let mut cents = 0i64;
        if let Some(m) = &self.simple_tax_rate_method {
            cents += m.supplies_per_tax_rate.iter().map(|r| r.turnover.0).sum::<i64>();
            cents += m.acquisition_tax.iter().map(|r| r.turnover.0).sum::<i64>();
        }
        if let Some(m) = &self.effective_reporting_method {
            cents += m.supplies_per_tax_rate.iter().map(|r| r.turnover.0).sum::<i64>();
            cents += m.acquisition_tax.iter().map(|r| r.turnover.0).sum::<i64>();
        }
        Amount(cents)
    }

    /// Prüft die Plausibilisierungen aus eCH-0217 Kap. 7.5 und die XSD-Facetten,
    /// die sich nicht durch `xmllint` allein absichern lassen.
    pub fn validate(&self) -> Vec<String> {
        let mut errs = Vec::new();
        let g = &self.general_information;

        let uid_ok = g.uid.len() == 12
            && g.uid.starts_with("CHE")
            && g.uid.as_bytes()[3].is_ascii_digit()
            && g.uid.as_bytes()[3] != b'0'
            && g.uid[3..].chars().all(|c| c.is_ascii_digit());
        if !uid_ok {
            errs.push(format!("uid {:?} entspricht nicht CHE[1-9][0-9]{{8}}", g.uid));
        }
        if g.organisation_name.is_empty() || g.organisation_name.chars().count() > 255 {
            errs.push("organisationName muss 1..255 Zeichen haben".into());
        }
        if !(1..=3).contains(&g.type_of_submission) {
            errs.push("typeOfSubmission muss 1, 2 oder 3 sein".into());
        }
        if !(1..=2).contains(&g.form_of_reporting) {
            errs.push("formOfReporting muss 1 (vereinbart) oder 2 (vereinnahmt) sein".into());
        }
        if g.business_reference_id.is_empty() || g.business_reference_id.chars().count() > 50 {
            errs.push("businessReferenceId muss 1..50 Zeichen haben".into());
        }
        if g.reporting_period_from > g.reporting_period_till {
            errs.push("reportingPeriodFrom liegt nach reportingPeriodTill".into());
        }
        let s = &g.sending_application;
        if s.manufacturer.chars().count() > 30 || s.product.chars().count() > 30 {
            errs.push("sendingApplication: manufacturer/product max. 30 Zeichen".into());
        }
        if s.product_version.chars().count() > 10 {
            errs.push("sendingApplication: productVersion max. 10 Zeichen".into());
        }

        match (&self.effective_reporting_method, &self.simple_tax_rate_method) {
            (None, None) => errs.push("keine Abrechnungsmethode gesetzt (xs:choice)".into()),
            (Some(_), Some(_)) => {
                errs.push("effectiveReportingMethod und simpleTaxRateMethod schliessen sich aus".into())
            }
            _ => {}
        }
        if let Some(m) = &self.simple_tax_rate_method {
            for r in &m.supplies_per_tax_rate {
                if r.activity_id.chars().count() != 5 {
                    errs.push(format!("activityID {:?} muss genau 5 Zeichen haben", r.activity_id));
                }
            }
            // Ab 01.01.2025 sind SSS-Umsätze immer brutto anzugeben.
            if g.reporting_period_from.as_str() < "2025-01-01" && !m.supplies_per_tax_rate.is_empty() {
                errs.push(
                    "simpleTaxRateMethod gilt erst ab Abrechnungsperiode 01.01.2025 \
                     (davor netTaxRateMethod/flatTaxRateMethod)"
                        .into(),
                );
            }
        }
        if let Some(m) = &self.effective_reporting_method {
            if !(1..=2).contains(&m.gross_or_net) {
                errs.push("grossOrNet muss 1 (netto) oder 2 (brutto) sein".into());
            }
        }

        // Kap. 7.5: Ziff. 299 muss der Summe der Leistungen entsprechen (MWST-0005).
        let taxable = self.turnover_computation.taxable_turnover();
        let supplies = self.supplies_total();
        if taxable != supplies {
            errs.push(format!(
                "MWST-0005: steuerbarer Gesamtumsatz (Ziff. 299) {taxable} != Summe der Leistungen {supplies}"
            ));
        }
        errs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_roundtrip() {
        assert_eq!(Amount::parse_chf("123'456.78").unwrap(), Amount(12_345_678));
        assert_eq!(Amount::parse_chf("123456,78").unwrap(), Amount(12_345_678));
        assert_eq!(Amount::parse_chf("-12.5").unwrap(), Amount(-1250));
        assert_eq!(Amount::parse_chf("7").unwrap(), Amount(700));
        assert_eq!(Amount(12_345_678).to_string(), "123456.78");
        assert_eq!(Amount(-5).to_string(), "-0.05");
        assert!(Amount::parse_chf("1.234").is_err());
    }

    #[test]
    fn percent_formats_without_trailing_zeros() {
        assert_eq!(Percent::parse("6.2").unwrap(), Percent(620));
        assert_eq!(Percent(620).to_string(), "6.2");
        assert_eq!(Percent(810).to_string(), "8.1");
        assert_eq!(Percent(800).to_string(), "8");
        assert_eq!(Percent(815).to_string(), "8.15");
        assert!(Percent::parse("120").is_err());
    }

    /// Der Weg Ziff. 200 → 299 → 399 → 500 einer Saldosteuersatz-Abrechnung, mit
    /// dem für die ESTV typischen Rundungsschritt: die geschuldete Steuer endet auf
    /// einen Rappen, der kein Vielfaches von 5 ist, und wird für Ziff. 500 **abwärts**
    /// auf die nächsten 5 Rappen gerundet (zu Gunsten der Unternehmung).
    #[test]
    fn saldosteuersatz_path_from_ziff_200_to_500() {
        let mut doc = Document::default();
        doc.turnover_computation.total_consideration = Amount(12_345_678);
        doc.simple_tax_rate_method = Some(SimpleTaxRateMethod {
            supplies_per_tax_rate: vec![ActivityTurnoverTaxRate {
                activity_id: "00001".into(),
                tax_rate: Percent(610),
                turnover: Amount(12_345_678),
            }],
            ..Default::default()
        });
        // 123'456.78 × 6.1 % = 7'530.85358 → Ziff. 399 = 7'530.86.
        assert_eq!(doc.total_tax_due(), Amount(753_086));
        assert_eq!(doc.compute_payable_tax(Rounding::FiveRappen), Amount(753_085));
        assert_eq!(doc.compute_payable_tax(Rounding::Rappen), Amount(753_086));
    }

    /// Kap. 7.5: «payableTax muss **ohne Runden der Zwischenschritte** berechnet
    /// werden.» Drei Zeilen mit je 0.5 Rappen Bruchteil ergeben zusammen 1.5
    /// Rappen — wer pro Zeile rundet, landet bei 3 statt 2 Rappen.
    #[test]
    fn payable_tax_does_not_round_intermediate_steps() {
        let row = |cents| ActivityTurnoverTaxRate {
            activity_id: "00001".into(),
            tax_rate: Percent(1000), // 10 %
            turnover: Amount(cents),
        };
        let mut doc = Document::default();
        // 10 % von 0.05 = 0.005 CHF = 0.5 Rappen, dreimal = 1.5 Rappen → 2 Rappen.
        doc.turnover_computation.total_consideration = Amount(15);
        doc.simple_tax_rate_method = Some(SimpleTaxRateMethod {
            supplies_per_tax_rate: vec![row(5), row(5), row(5)],
            ..Default::default()
        });
        assert_eq!(doc.compute_payable_tax(Rounding::Rappen), Amount(2));
        // Zeilenweise gerundet wäre jede Zeile 1 Rappen, also 3 — das wäre falsch.
        assert_eq!(line_tax(Amount(5), Percent(1000), false), Amount(1));
    }

    #[test]
    fn five_rappen_rounding_favours_the_taxpayer() {
        // Schuld wird kleiner, Guthaben (negativ) wird grösser.
        assert_eq!(round_payable(753_086, Rounding::FiveRappen), 753_085);
        assert_eq!(round_payable(-753_086, Rounding::FiveRappen), -753_090);
        assert_eq!(round_payable(753_085, Rounding::FiveRappen), 753_085);
    }

    #[test]
    fn effective_method_gross_extracts_tax_from_turnover() {
        let mut doc = Document::default();
        doc.turnover_computation.total_consideration = Amount(108_100);
        doc.effective_reporting_method = Some(EffectiveReportingMethod {
            gross_or_net: 2,
            supplies_per_tax_rate: vec![TurnoverTaxRate {
                tax_rate: Percent(810),
                turnover: Amount(108_100), // 1000.00 netto + 8.1 %
            }],
            input_tax_material_and_services: Some(Amount(2_000)),
            ..Default::default()
        });
        // 1081.00 brutto → 81.00 Steuer, abzüglich 20.00 Vorsteuer = 61.00.
        assert_eq!(doc.total_tax_due(), Amount(8_100));
        assert_eq!(doc.compute_payable_tax(Rounding::Rappen), Amount(6_100));
    }

    #[test]
    fn validate_flags_turnover_mismatch() {
        let mut doc = Document::default();
        doc.general_information.uid = "CHE123456789".into();
        doc.general_information.organisation_name = "Beispiel GmbH".into();
        doc.general_information.type_of_submission = 1;
        doc.general_information.form_of_reporting = 1;
        doc.general_information.business_reference_id = "x".into();
        doc.general_information.reporting_period_from = "2026-01-01".into();
        doc.general_information.reporting_period_till = "2026-06-30".into();
        doc.turnover_computation.total_consideration = Amount(10_000_000);
        doc.simple_tax_rate_method = Some(SimpleTaxRateMethod {
            supplies_per_tax_rate: vec![ActivityTurnoverTaxRate {
                activity_id: "00001".into(),
                tax_rate: Percent(620),
                turnover: Amount(9_000_000),
            }],
            ..Default::default()
        });
        let errs = doc.validate();
        assert!(errs.iter().any(|e| e.contains("MWST-0005")), "{errs:?}");
    }
}
