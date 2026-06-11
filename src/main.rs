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
mod dataset_jp;
mod ech0196;
mod model;
mod model_jp;
mod mt940;
mod pdf;
mod settings;
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
    jp: bool,
    from_mt940: Option<String>,
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
            "--jp" => a.jp = true,
            "--from-mt940" => a.from_mt940 = Some(it.next().ok_or("--from-mt940 erwartet einen Pfad")?),
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

    if let Some(path) = &args.from_mt940 {
        return run_mt940(path);
    }

    if args.jp {
        return run_jp(&args);
    }

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

/// `--from-mt940`: MT940-Kontoauszug einlesen, Zusammenfassung ausgeben +
/// `data/mt940-summary.json` schreiben.
fn run_mt940(path: &str) -> ExitCode {
    // MT940 ist oft Latin-1 (Umlaute) → verlustfrei-tolerant dekodieren.
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Konnte {path} nicht lesen: {e}");
            return ExitCode::FAILURE;
        }
    };
    let text = String::from_utf8_lossy(&bytes);
    let stmt = match mt940::parse(&text) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("MT940 nicht lesbar: {e}");
            return ExitCode::FAILURE;
        }
    };

    println!("MT940-Kontoauszug: {}", stmt.account);
    if let Some(b) = &stmt.opening {
        println!("  Eröffnungssaldo {} : {} {}", b.date, b.currency, mt940::format_cents(b.amount_cents));
    }
    if let Some(b) = &stmt.closing {
        println!("  Schlusssaldo    {} : {} {}", b.date, b.currency, mt940::format_cents(b.amount_cents));
    }
    println!(
        "  Buchungen: {} (Gutschriften CHF {}, Belastungen CHF {})",
        stmt.transactions.len(),
        mt940::format_cents(stmt.total_credit_cents()),
        mt940::format_cents(stmt.total_debit_cents()),
    );

    let out = Path::new("data").join("mt940-summary.json");
    match serde_json::to_string_pretty(&stmt)
        .map_err(|e| e.to_string())
        .and_then(|j| std::fs::create_dir_all("data").and_then(|_| std::fs::write(&out, j)).map_err(|e| e.to_string()))
    {
        Ok(()) => println!("Zusammenfassung geschrieben nach: {}", out.display()),
        Err(e) => eprintln!("Hinweis: konnte {} nicht schreiben: {e}", out.display()),
    }
    ExitCode::SUCCESS
}

fn load_document<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn write_json<T: serde::Serialize>(path: &Path, doc: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(doc).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// Serialisiert eine Nachricht zu eingerücktem XML mit XML-Deklaration.
fn to_xml<T: serde::Serialize>(message: &T) -> Result<String, String> {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    let mut ser = quick_xml::se::Serializer::new(&mut xml);
    ser.indent(' ', 2);
    message.serialize(ser).map_err(|e| e.to_string())?;
    xml.push('\n');
    Ok(xml)
}

/// JP-Modus (`--jp`): Steuererklärung juristische Person (StA 500), inoffiziell.
fn run_jp(args: &Args) -> ExitCode {
    let default_input = PathBuf::from("data/input-jp.json");
    let mut from_example = false;
    let mut doc: model_jp::Document = if let Some(path) = &args.input_json {
        match load_document(Path::new(path)) {
            Ok(d) => {
                println!("JP-Eingabe gelesen aus: {path}");
                d
            }
            Err(e) => {
                eprintln!("Konnte {path} nicht lesen: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else if default_input.exists() {
        match load_document(&default_input) {
            Ok(d) => {
                println!("JP-Eingabe gelesen aus: {}", default_input.display());
                d
            }
            Err(e) => {
                eprintln!("Konnte {} nicht lesen: {e}", default_input.display());
                return ExitCode::FAILURE;
            }
        }
    } else {
        from_example = true;
        dataset_jp::example()
    };

    // settings.json (gitignored) überschreibt Identifikatoren – nicht im Code.
    let settings = settings::load();
    if let Some(uid) = settings.jp.uid.as_deref().filter(|s| !s.is_empty()) {
        doc.header.title.uid = Some(uid.to_string());
    }
    if let Some(rn) = settings.jp.register_number {
        doc.header.title.register_number = rn;
    }

    if from_example {
        if let Err(e) = write_json(&default_input, &doc) {
            eprintln!("Hinweis: konnte Vorlage nicht schreiben: {e}");
        } else {
            println!(
                "Keine JP-Eingabe gefunden – Beispiel nach {} geschrieben (editierbar).",
                default_input.display()
            );
        }
    }

    let message = doc.into_message();
    let xml = match to_xml(&message) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Fehler beim Serialisieren: {e}");
            return ExitCode::FAILURE;
        }
    };

    let out_path = Path::new("data").join("steuererklaerung-jp-2025.xml");
    if let Err(e) = std::fs::create_dir_all("data").and_then(|_| std::fs::write(&out_path, &xml)) {
        eprintln!("Konnte {} nicht schreiben: {e}", out_path.display());
        return ExitCode::FAILURE;
    }
    println!("JP-XML (eCH-0276) geschrieben nach: {}", out_path.display());
    println!("{} Bytes, {} Zeilen", xml.len(), xml.lines().count());

    // Validierung gegen das offizielle eCH-0276-XSD (falls vorhanden).
    let schema = Path::new("schema/eCH-0276-1-0.xsd");
    if schema.exists() {
        match std::process::Command::new("xmllint")
            .args(["--nonet", "--noout", "--schema"])
            .arg(schema)
            .arg(&out_path)
            .status()
        {
            Ok(s) if s.success() => println!("eCH-0276-Validierung: OK"),
            Ok(_) => {
                eprintln!("eCH-0276-Validierung fehlgeschlagen (siehe xmllint-Ausgabe oben)");
                return ExitCode::FAILURE;
            }
            Err(_) => println!("Hinweis: xmllint nicht gefunden – Validierung übersprungen"),
        }
    } else {
        println!("Hinweis: schema/eCH-0276-1-0.xsd fehlt – ./scripts/fetch-schemas.sh ausführen");
    }

    if args.package {
        match submit::write_package_jp(&xml, &message) {
            Ok(dir) => println!("JP-Einreichungs-Paket geschrieben nach: {}", dir.display()),
            Err(e) => {
                eprintln!("Konnte JP-Paket nicht schreiben: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    ExitCode::SUCCESS
}
