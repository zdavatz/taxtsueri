//! Lokale Einstellungen aus `settings.json` (gitignored).
//!
//! Hier liegen identifizierende/konfigurierbare Werte, die **nicht** in den
//! Code gehören (z. B. UID, Register-Nr.). Fehlt die Datei, gelten Defaults.
//! Eine Vorlage ist als `settings.example.json` committet.

use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub jp: JpSettings,
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

/// Lädt `settings.json`; gibt bei fehlender/ungültiger Datei Defaults zurück.
pub fn load() -> Settings {
    std::fs::read_to_string("settings.json")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
