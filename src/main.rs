//! taxtsueri — erzeugt aus den Steuererklärungsdaten eine eCH-0119-XML-Datei
//! für die elektronische Einreichung beim Kanton / der Stadt Zürich.
//!
//! Aufrufe:
//!   taxtsueri [EINGABE.json]            Eingabe aus JSON (sonst data/input.json
//!                                       bzw. eingebautes Beispiel)
//!   taxtsueri --from-ech0196 AUSZUG.xml eCH-0196-eSteuerauszug einlesen und das
//!                                       Wertschriftenverzeichnis ersetzen
//!   taxtsueri --from-pdf AUSZUG.pdf     eingebettetes eCH-0196-XML aus einem PDF
//!                                       extrahieren und einlesen
//!   taxtsueri --package                 zusätzlich ein Einreichungs-Paket schreiben
//!
//! `data/` (Eingabe + XML + Paket) enthält Personendaten und ist gitignored.

mod dataset;
mod ech0196;
mod model;
mod pdf;
mod submit;

use model::Document;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Default)]
struct Args {
    input_json: Option<String>,
    from_ech0196: Option<String>,
    from_pdf: Option<String>,
    package: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--from-ech0196" => {
                a.from_ech0196 = Some(it.next().ok_or("--from-ech0196 erwartet einen Pfad")?)
            }
            "--from-pdf" => a.from_pdf = Some(it.next().ok_or("--from-pdf erwartet einen Pfad")?),
            "--package" => a.package = true,
            s if s.starts_with("--") => return Err(format!("unbekannte Option: {s}")),
            s => a.input_json = Some(s.to_string()),
        }
    }
    Ok(a)
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    // 1) Basis-Document beschaffen.
    let default_input = PathBuf::from("data/input.json");
    let mut doc: Document = if let Some(path) = &args.input_json {
        match load_document(Path::new(path)) {
            Ok(d) => {
                println!("Eingabe gelesen aus: {path}");
                d
            }
            Err(e) => {
                eprintln!("Konnte {path} nicht lesen: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else if default_input.exists() && args.from_ech0196.is_none() && args.from_pdf.is_none() {
        match load_document(&default_input) {
            Ok(d) => {
                println!("Eingabe gelesen aus: {}", default_input.display());
                d
            }
            Err(e) => {
                eprintln!("Konnte {} nicht lesen: {e}", default_input.display());
                return ExitCode::FAILURE;
            }
        }
    } else {
        let d = dataset::example();
        if args.from_ech0196.is_none() && args.from_pdf.is_none() {
            if let Err(e) = write_json(&default_input, &d) {
                eprintln!("Hinweis: konnte Vorlage nicht schreiben: {e}");
            } else {
                println!(
                    "Keine Eingabe gefunden – Beispiel nach {} geschrieben (editierbar).",
                    default_input.display()
                );
            }
        }
        d
    };

    // 2) Optional: Wertschriftenverzeichnis aus eCH-0196 (direkt oder aus PDF) ersetzen.
    let ech0196_xml: Option<String> = if let Some(p) = &args.from_pdf {
        match extract_ech0196_from_pdf(Path::new(p)) {
            Ok(Some(xml)) => Some(xml),
            Ok(None) => {
                eprintln!(
                    "In {p} wurde kein eingebettetes eCH-0196-XML gefunden \
                     (vermutlich ein Scan; PDF417-Barcodes brauchen einen Bilddecoder)."
                );
                return ExitCode::FAILURE;
            }
            Err(e) => {
                eprintln!("PDF-Extraktion fehlgeschlagen: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else if let Some(p) = &args.from_ech0196 {
        match std::fs::read_to_string(p) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("Konnte {p} nicht lesen: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        None
    };

    if let Some(xml) = ech0196_xml {
        match ech0196::list_of_securities_from_xml(&xml) {
            Ok(los) => {
                let n = los.security_entry.len();
                doc.content.list_of_securities = Some(los);
                println!("eCH-0196 eingelesen: {n} Wertschriftenpositionen übernommen.");
            }
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        }
    }

    // 3) eCH-0119-XML serialisieren.
    let message = doc.into_message();
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    let mut ser = quick_xml::se::Serializer::new(&mut xml);
    ser.indent(' ', 2);
    if let Err(e) = serde::Serialize::serialize(&message, ser) {
        eprintln!("Fehler beim Serialisieren der eCH-0119-Nachricht: {e}");
        return ExitCode::FAILURE;
    }
    xml.push('\n');

    let out_path = Path::new("data").join("steuererklaerung-2025.xml");
    if let Err(e) = std::fs::create_dir_all("data").and_then(|_| std::fs::write(&out_path, &xml)) {
        eprintln!("Konnte {} nicht schreiben: {e}", out_path.display());
        return ExitCode::FAILURE;
    }
    println!("eCH-0119-XML geschrieben nach: {}", out_path.display());
    println!("{} Bytes, {} Zeilen", xml.len(), xml.lines().count());

    // 4) Validierung gegen eCH-0119 (falls Schema + xmllint vorhanden).
    let schema = Path::new("schema/eCH-0119-4-0-0.xsd");
    if schema.exists() {
        match std::process::Command::new("xmllint")
            .args(["--nonet", "--noout", "--schema"])
            .arg(schema)
            .arg(&out_path)
            .status()
        {
            Ok(s) if s.success() => println!("eCH-0119-Validierung: OK"),
            Ok(_) => {
                eprintln!("eCH-0119-Validierung fehlgeschlagen (siehe xmllint-Ausgabe oben)");
                return ExitCode::FAILURE;
            }
            Err(_) => println!("Hinweis: xmllint nicht gefunden – Validierung übersprungen"),
        }
    } else {
        println!("Hinweis: schema/ fehlt – ./scripts/fetch-schemas.sh ausführen zum Validieren");
    }

    // 5) Optional: Einreichungs-Paket schreiben.
    if args.package {
        match submit::write_package(&xml, &message) {
            Ok(dir) => println!("Einreichungs-Paket geschrieben nach: {}", dir.display()),
            Err(e) => {
                eprintln!("Konnte Einreichungs-Paket nicht schreiben: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    ExitCode::SUCCESS
}

fn extract_ech0196_from_pdf(path: &Path) -> Result<Option<String>, String> {
    let xmls = pdf::extract_embedded_xml(path)?;
    Ok(xmls.into_iter().map(|(_, content)| content).next())
}

fn load_document(path: &Path) -> Result<Document, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn write_json(path: &Path, doc: &Document) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(doc).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}
