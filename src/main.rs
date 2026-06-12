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

use taxtsueri::{
    barcode, code128, dataset, ech0196, model, model_zh, mt940, pdf, pdf417, settings, sheet,
    submit, vermoegensausweis,
};
// dataset_jp/model_jp werden in run_jp referenziert.
use taxtsueri::{dataset_jp, model_jp};
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
    from_vermoegensausweis: Option<String>,
    barcode: Option<String>,
    zh_barcode: bool,
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
            "--from-vermoegensausweis" => {
                a.from_vermoegensausweis = Some(it.next().ok_or("--from-vermoegensausweis erwartet einen Pfad")?)
            }
            "--barcode" => a.barcode = Some(it.next().ok_or("--barcode erwartet einen Pfad (eCH-0196-XML)")?),
            "--zh-barcode" => a.zh_barcode = true,
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

    // MT940 **allein** → Kontoauszugs-Report. Zusammen mit einem Erklärungs-Input
    // (Vermögensausweis/eCH-0196) bildet es stattdessen die Basis der Erklärung (unten).
    if args.from_mt940.is_some()
        && args.from_vermoegensausweis.is_none()
        && args.from_ech0196.is_none()
        && args.from_pdf.is_none()
    {
        return run_mt940(args.from_mt940.as_ref().unwrap());
    }

    if let Some(path) = &args.barcode {
        return run_barcode(path);
    }

    if args.zh_barcode {
        return run_zh_barcode(&args);
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

    // 2b) Optional: Wertschriftenverzeichnis direkt aus einem Vermögensausweis-PDF.
    // Kombinierter Flow: MT940-Konto (Basis) + Vermögensausweis (Wertschriften) →
    // ein eCH-0119-Wertschriftenverzeichnis. Das Konto steht zuoberst.
    if args.from_mt940.is_some() || args.from_vermoegensausweis.is_some() {
        let mut entries: Vec<model::SecurityEntry> = Vec::new();

        if let Some(path) = &args.from_mt940 {
            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Konnte {path} nicht lesen: {e}");
                    return ExitCode::FAILURE;
                }
            };
            match mt940::parse(&String::from_utf8_lossy(&bytes)) {
                Ok(stmt) => {
                    let acc = mt940::account_security_entry(&stmt);
                    let saldo = acc.tax_value.map(|t| t.cantonal).unwrap_or(0);
                    let zins = acc.gross_revenue_a.map(|t| t.cantonal).unwrap_or(0);
                    let (credit, debit) = (stmt.total_credit_cents(), stmt.total_debit_cents());
                    println!("MT940 (Basis): Konto {}", stmt.account);
                    println!("  Schlusssaldo → Vermögen   : CHF {saldo}");
                    println!("  Zinsertrag   → grossRevenueA: CHF {zins}");
                    println!(
                        "  Ertrag/Aufwand (Geldfluss) : +{} / -{} (Buchungen: {})",
                        mt940::format_cents(credit),
                        mt940::format_cents(debit),
                        stmt.transactions.len()
                    );
                    entries.push(acc);
                }
                Err(e) => {
                    eprintln!("MT940 nicht lesbar: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }

        if let Some(path) = &args.from_vermoegensausweis {
            match run_pdftotext(Path::new(path)) {
                Ok(text) => {
                    let secs = vermoegensausweis::to_securities(&vermoegensausweis::parse_text(&text));
                    println!("Vermögensausweis: {} Wertschriftenpositionen übernommen.", secs.len());
                    entries.extend(secs);
                }
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            }
        }

        let los = vermoegensausweis::build_list_of_securities(entries);
        let total = los.total_tax_value.map(|t| t.cantonal).unwrap_or(0);
        println!(
            "→ Wertschriftenverzeichnis: {} Positionen, Steuerwert total CHF {total}",
            los.security_entry.len()
        );
        doc.content.list_of_securities = Some(los);
    }

    // 2c) settings.json (gitignored) setzt die AHVN13 – nicht im Code.
    if let Some(vn) = settings::load().np.vn {
        doc.content.main_form.person_data_partner1.identification.vn = vn;
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

/// Extrahiert den Text eines (Text-)PDFs via `pdftotext -layout`.
fn run_pdftotext(path: &Path) -> Result<String, String> {
    let out = std::process::Command::new("pdftotext")
        .arg("-layout")
        .arg(path)
        .arg("-")
        .output()
        .map_err(|_| "pdftotext nicht gefunden (poppler-utils installieren)".to_string())?;
    if !out.status.success() {
        return Err(format!("pdftotext fehlgeschlagen für {}", path.display()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
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

    let cur = stmt.closing.as_ref().or(stmt.opening.as_ref()).map(|b| b.currency.clone()).unwrap_or_else(|| "CHF".into());
    println!("MT940-Kontoauszug: {}", stmt.account);
    if let Some(b) = &stmt.opening {
        println!("  Eröffnungssaldo {} : {} {}", b.date, b.currency, mt940::format_cents(b.amount_cents));
    }
    if let Some(b) = &stmt.closing {
        println!("  Schlusssaldo    {} : {} {}", b.date, b.currency, mt940::format_cents(b.amount_cents));
    }
    let (credit, debit) = (stmt.total_credit_cents(), stmt.total_debit_cents());
    println!("  Buchungen: {}", stmt.transactions.len());

    let categories = mt940::categorize(&stmt);
    println!("\nKategorien (Geldfluss, heuristisch) — Gutschrift / Belastung:");
    for c in &categories {
        println!(
            "  {:<38} {:>3}x  +{:>12}  -{:>12}",
            c.category,
            c.count,
            mt940::format_cents(c.credit_cents),
            mt940::format_cents(c.debit_cents),
        );
    }

    println!("\nErfolgsrechnung (Cash-Basis, näherungsweise):");
    println!("  Total Ertrag  (Gutschriften) : {cur} {}", mt940::format_cents(credit));
    println!("  Total Aufwand (Belastungen)  : {cur} {}", mt940::format_cents(debit));
    println!("  Geldfluss-Saldo              : {cur} {}", mt940::format_cents(credit - debit));
    println!("  Hinweis: Cash-Basis ≠ Jahresgewinn (Abgrenzungen/RAG, Abschreibungen nicht enthalten).");

    if let Some(b) = &stmt.closing {
        println!("\nBilanz-Position:");
        println!("  Flüssige Mittel (Bank) per {} : {} {}", b.date, b.currency, mt940::format_cents(b.amount_cents));
    }

    let report = serde_json::json!({
        "account": stmt.account,
        "opening": stmt.opening,
        "closing": stmt.closing,
        "transactionCount": stmt.transactions.len(),
        "totalCreditCents": credit,
        "totalDebitCents": debit,
        "categories": categories,
        "transactions": stmt.transactions,
    });
    let out = Path::new("data").join("mt940-summary.json");
    match serde_json::to_string_pretty(&report)
        .map_err(|e| e.to_string())
        .and_then(|j| std::fs::create_dir_all("data").and_then(|_| std::fs::write(&out, j)).map_err(|e| e.to_string()))
    {
        Ok(()) => println!("\nReport (inkl. Kategorien + Buchungen) geschrieben nach: {}", out.display()),
        Err(e) => eprintln!("Hinweis: konnte {} nicht schreiben: {e}", out.display()),
    }
    ExitCode::SUCCESS
}

/// `--barcode`: eCH-0196-XML → komprimierte Barcode-Nutzlast vorbereiten (Fundament).
fn run_barcode(path: &str) -> ExitCode {
    let xml = match std::fs::read(path) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(e) => {
            eprintln!("Konnte {path} nicht lesen: {e}");
            return ExitCode::FAILURE;
        }
    };
    let p = barcode::prepare(&xml);
    println!("eCH-0196 Barcode-Vorbereitung:");
    println!("  Barcode-ID        : {}", p.id);
    println!("  XML               : {} Bytes", xml.len());
    println!("  ZLIB-komprimiert  : {} Bytes ({:.0}%)", p.compressed.len(), p.compressed.len() as f64 / xml.len() as f64 * 100.0);
    println!(
        "  PDF417            : {} Spalten × {} Zeilen, EC-Level {}, {} Blöcke/Blatt",
        barcode::COLUMNS, barcode::ROWS, barcode::EC_LEVEL, barcode::BLOCKS_PER_SHEET
    );
    println!("  Geschätzt         : ~{} Segmente → ~{} Blatt/Blätter", p.estimated_segments(), p.sheets());

    let out = Path::new("data").join("barcode-payload.bin");
    match std::fs::create_dir_all("data").and_then(|_| std::fs::write(&out, &p.compressed)) {
        Ok(()) => println!("  Nutzlast (zlib)   : {}", out.display()),
        Err(e) => eprintln!("  Hinweis: konnte {} nicht schreiben: {e}", out.display()),
    }
    // PDF417 Structured Append → A4-Barcode-Blatt (PDF) mit CODE128C-Seitenbarcode.
    let (c, r, l) = (barcode::COLUMNS, barcode::ROWS, barcode::EC_LEVEL);
    match pdf417::build_symbols(&p.compressed, c, r, l) {
        Ok(symbols) => {
            let n = symbols.len();
            let grids: Vec<Vec<Vec<bool>>> =
                symbols.iter().map(|s| pdf417::render(s, c, r, l)).collect();
            // 16-stelliger Seitencode (Organisations-Nr. hier Platzhalter 0).
            let page_code = code128::build_page_code(196, 22, 0, 1, true, 0, 1);
            match code128::encode(&page_code) {
                Ok(bits) => {
                    let pdf = sheet::build_sheet_pdf(&grids, &bits);
                    let out = Path::new("data").join("barcode-blatt.pdf");
                    match std::fs::create_dir_all("data").and_then(|_| std::fs::write(&out, &pdf)) {
                        Ok(()) => {
                            let sheets = n.div_ceil(6);
                            println!("  PDF417            : {n} Segment(e), CODE128C {page_code}");
                            println!(
                                "  Barcode-Blatt     : {} ({sheets} Blatt A4, {} Bytes)",
                                out.display(),
                                pdf.len()
                            );
                        }
                        Err(e) => eprintln!("  Hinweis: konnte {} nicht schreiben: {e}", out.display()),
                    }
                }
                Err(e) => eprintln!("  CODE128: {e}"),
            }
        }
        Err(e) => println!("  PDF417            : {e}"),
    }
    ExitCode::SUCCESS
}

/// `--zh-barcode`: ZH-Steuererklärungs-Barcode (eCH-0119 v3 + ZH-cantonExtension)
/// als A4-PDF417-Blatt. Basis-Document aus `input.json`/Beispiel; AHVN13 aus
/// settings.json. Die `approvalReceipt`-Steuerwerte sind hier Platzhalter (0) — sie
/// stammen real aus der ZHprivateTax-Berechnung (s. `ZhBarcodeData`).
fn run_zh_barcode(args: &Args) -> ExitCode {
    let mut doc = match &args.input_json {
        Some(p) => match load_document(Path::new(p)) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Konnte {p} nicht lesen: {e}");
                return ExitCode::FAILURE;
            }
        },
        None => dataset::example(),
    };
    if let Some(vn) = settings::load().np.vn {
        doc.content.main_form.person_data_partner1.identification.vn = vn;
    }

    let period: u16 = 2025;
    let data = model_zh::ZhBarcodeData {
        canton: "ZH".into(),
        date: format!("{period}-12-31"),
        // Platzhalter — reale Werte aus der ZHprivateTax-Berechnung übernehmen.
        taxable_income: model_zh::ZhTaxAmount { cantonal: 0, federal: 0 },
        ratedetermining_income: model_zh::ZhTaxAmount { cantonal: 0, federal: 0 },
        taxable_qualified_investments: 0,
        taxable_asset: 0,
        ratedetermining_asset: 0,
        documents: Vec::new(),
    };
    let msg = model_zh::ZhMessage::from_document_with_data(&doc, period, &data);
    let xml = match model_zh::zh_message_to_xml(&msg) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Serialisierung fehlgeschlagen: {e}");
            return ExitCode::FAILURE;
        }
    };

    println!("ZH-Steuererklärungs-Barcode (eCH-0119 v3 + zh:cantonExtension):");
    let p = barcode::prepare(&xml);
    println!("  XML               : {} Bytes", xml.len());
    println!(
        "  ZLIB-komprimiert  : {} Bytes ({:.0}%)",
        p.compressed.len(),
        p.compressed.len() as f64 / xml.len() as f64 * 100.0
    );

    if let Err(e) = std::fs::create_dir_all("data") {
        eprintln!("Konnte data/ nicht anlegen: {e}");
        return ExitCode::FAILURE;
    }
    let xml_out = Path::new("data").join("steuererklaerung-zh-v3.xml");
    let _ = std::fs::write(&xml_out, &xml);
    println!("  v3-XML            : {}", xml_out.display());

    let (c, r, l) = (barcode::COLUMNS, barcode::ROWS, barcode::EC_LEVEL);
    match pdf417::build_symbols(&p.compressed, c, r, l) {
        Ok(symbols) => {
            let n = symbols.len();
            let grids: Vec<Vec<Vec<bool>>> =
                symbols.iter().map(|s| pdf417::render(s, c, r, l)).collect();
            let page_code = code128::build_page_code(119, 0, 261, 1, true, 0, 1);
            match code128::encode(&page_code) {
                Ok(bits) => {
                    let pdf = sheet::build_sheet_pdf(&grids, &bits);
                    let out = Path::new("data").join("zh-barcode-blatt.pdf");
                    match std::fs::write(&out, &pdf) {
                        Ok(()) => {
                            println!("  PDF417            : {n} Segment(e), CODE128C {page_code}");
                            println!(
                                "  Barcode-Blatt     : {} ({} Blatt A4, {} Bytes)",
                                out.display(),
                                n.div_ceil(6),
                                pdf.len()
                            );
                        }
                        Err(e) => eprintln!("  Hinweis: konnte {} nicht schreiben: {e}", out.display()),
                    }
                }
                Err(e) => eprintln!("  CODE128: {e}"),
            }
        }
        Err(e) => println!("  PDF417            : {e}"),
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
