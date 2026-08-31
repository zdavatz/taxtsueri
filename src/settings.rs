//! Lokale Einstellungen aus `settings.json` (gitignored).
//!
//! Hier liegen identifizierende/konfigurierbare Werte, die **nicht** in den
//! Code gehören (z. B. UID, Register-Nr.). Fehlt die Datei, gelten Defaults.
//! Eine Vorlage ist als `settings.example.json` committet.

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub np: NpSettings,
    pub jp: JpSettings,
    pub mwst: MwstSettings,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct NpSettings {
    /// AHVN13 / Versichertennummer (Bereich 7560000000001..7569999999999) – nicht im Code.
    pub vn: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct JpSettings {
    /// Unternehmens-Identifikationsnummer, Format CHE + 9 Ziffern (ohne Punkte).
    pub uid: Option<String>,
    /// Register-Nr. der juristischen Person (xs:long).
    #[serde(rename = "registerNumber")]
    pub register_number: Option<i64>,
}

/// MWST-Abrechnung (eCH-0217). Die UID und der bewilligte Tätigkeitscode sind
/// identifizierend und gehören deshalb nicht in den Code.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct MwstSettings {
    /// MWST-Nummer, z. B. "CHE-123.456.789 MWST" — wird auf `CHE123456789` normalisiert.
    pub uid: Option<String>,
    /// Firmenname, wie er bei der ESTV registriert ist.
    #[serde(rename = "organisationName")]
    pub organisation_name: Option<String>,
    /// 5-stelliger, von der ESTV **bewilligter** Tätigkeitscode (Saldosteuersatz-
    /// methode, Abrechnungsperioden ab 01.01.2025). Steht in der Applikation
    /// «MWST abrechnen» unter «Abrechnungsmodalitäten».
    #[serde(rename = "activityId")]
    pub activity_id: Option<String>,
    /// Saldosteuersatz in Prozent, z. B. "6.2".
    #[serde(rename = "taxRate")]
    pub tax_rate: Option<String>,
    /// Abrechnungsmethode: "saldosteuersatz" (Default) oder "effektiv".
    pub methode: Option<String>,
    /// Abrechnungsart: "vereinbart" (Default) oder "vereinnahmt".
    pub abrechnungsart: Option<String>,
    /// Hersteller für `sendingApplication` (max. 30 Zeichen). Ohne Angabe
    /// meldet sich das Programm neutral als "taxtsueri".
    pub manufacturer: Option<String>,
}

/// Lädt `settings.json`; gibt bei fehlender/ungültiger Datei Defaults zurück.
pub fn load() -> Settings {
    std::fs::read_to_string("settings.json")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
