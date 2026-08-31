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
//!   taxtsueri --mwst --periode S1/2026 --umsatz 123456.78 --activity-id 12345
//!                                       MWST-Abrechnung nach eCH-0217 V2.0.0 für den
//!                                       Import in ESTV SuisseTax «MWST abrechnen»
//!
//! `data/` (Eingabe + XML + Paket) enthält Personendaten und ist gitignored.

use taxtsueri::{
    barcode, camt053, code128, dataset, ech0196, model, model_mwst, model_zh, mt940, mwst, pdf,
    pdf417, pdf_report, settings, sheet, submit, vermoegensausweis,
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
    from_camt: Option<String>,
    from_vermoegensausweis: Option<String>,
    barcode: Option<String>,
    zh_barcode: bool,
    /// Wertschriften-Steuerwert in CHF für die Pseudo-Bilanz (wenn kein Vermögensausweis
    /// vorliegt, z. B. aus der Jahresrechnung).
    wertschriften: Option<i64>,

    // ---- MWST-Abrechnung (eCH-0217) ----
    mwst: bool,
    /// MWST-Nummer, überschreibt settings.json.
    uid: Option<String>,
    /// Firmenname, überschreibt settings.json.
    firma: Option<String>,
    /// Abrechnungsperiode: "S1/2026", "Q2/2026" oder "2026-01-01:2026-06-30".
    periode: Option<String>,
    /// Ziff. 200 — Total der Entgelte (bei Saldosteuersatz brutto).
    umsatz: Option<String>,
    /// Steuersatz in Prozent (Saldosteuersatz bzw. gesetzlicher Satz).
    satz: Option<String>,
    /// 5-stelliger Tätigkeitscode der ESTV (eine einzige Tätigkeit).
    activity_id: Option<String>,
    /// Mehrere Tätigkeiten/Steuersätze: je `CODE:SATZ:UMSATZ` bzw. `SATZ:UMSATZ`.
    positionen: Vec<String>,
    /// formOfReporting = 2 statt 1.
    vereinnahmt: bool,
    /// Effektive Methode statt Saldosteuersatz.
    effektiv: bool,
    /// Effektive Methode: Umsätze brutto (grossOrNet = 2).
    brutto: bool,
    /// typeOfSubmission = 2 bzw. 3.
    korrektur: bool,
    jahresabstimmung: bool,
    /// Ziff. 220 / 230 / 235.
    export: Option<String>,
    ausgenommen: Option<String>,
    entgeltsminderung: Option<String>,
    /// Ziff. 400 / 405 (nur effektive Methode).
    vorsteuer_material: Option<String>,
    vorsteuer_investitionen: Option<String>,
    /// Ziff. 910 / 900.
    dividenden: Option<String>,
    subventionen: Option<String>,
    /// payableTax auf 5 Rappen abwärts runden statt kaufmännisch auf Rappen.
    fuenf_rappen: bool,
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
            "--from-camt" => a.from_camt = Some(it.next().ok_or("--from-camt erwartet einen Pfad (Datei oder Verzeichnis)")?),
            "--from-vermoegensausweis" => {
                a.from_vermoegensausweis = Some(it.next().ok_or("--from-vermoegensausweis erwartet einen Pfad")?)
            }
            "--barcode" => a.barcode = Some(it.next().ok_or("--barcode erwartet einen Pfad (eCH-0196-XML)")?),
            "--zh-barcode" => a.zh_barcode = true,
            "--wertschriften" => {
                a.wertschriften = Some(
                    it.next()
                        .ok_or("--wertschriften erwartet einen CHF-Betrag")?
                        .parse()
                        .map_err(|_| "--wertschriften: ungültige Zahl")?,
                )
            }
            "--mwst" => a.mwst = true,
            "--uid" => a.uid = Some(it.next().ok_or("--uid erwartet eine MWST-Nummer")?),
            "--firma" => a.firma = Some(it.next().ok_or("--firma erwartet den Firmennamen")?),
            "--periode" => a.periode = Some(it.next().ok_or("--periode erwartet z. B. S1/2026")?),
            "--umsatz" => a.umsatz = Some(it.next().ok_or("--umsatz erwartet einen CHF-Betrag")?),
            "--satz" => a.satz = Some(it.next().ok_or("--satz erwartet einen Prozentwert, z. B. 6.2")?),
            "--activity-id" => {
                a.activity_id = Some(it.next().ok_or("--activity-id erwartet 5 Zeichen")?)
            }
            "--position" => a
                .positionen
                .push(it.next().ok_or("--position erwartet CODE:SATZ:UMSATZ")?),
            "--vereinnahmt" => a.vereinnahmt = true,
            "--effektiv" => a.effektiv = true,
            "--brutto" => a.brutto = true,
            "--korrektur" => a.korrektur = true,
            "--jahresabstimmung" => a.jahresabstimmung = true,
            "--export" => a.export = Some(it.next().ok_or("--export erwartet einen CHF-Betrag")?),
            "--ausgenommen" => {
                a.ausgenommen = Some(it.next().ok_or("--ausgenommen erwartet einen CHF-Betrag")?)
            }
            "--entgeltsminderung" => {
                a.entgeltsminderung =
                    Some(it.next().ok_or("--entgeltsminderung erwartet einen CHF-Betrag")?)
            }
            "--vorsteuer-material" => {
                a.vorsteuer_material =
                    Some(it.next().ok_or("--vorsteuer-material erwartet einen CHF-Betrag")?)
            }
            "--vorsteuer-investitionen" => {
                a.vorsteuer_investitionen =
                    Some(it.next().ok_or("--vorsteuer-investitionen erwartet einen CHF-Betrag")?)
            }
            "--dividenden" => {
                a.dividenden = Some(it.next().ok_or("--dividenden erwartet einen CHF-Betrag")?)
            }
            "--subventionen" => {
                a.subventionen = Some(it.next().ok_or("--subventionen erwartet einen CHF-Betrag")?)
            }
            "--fuenf-rappen" => a.fuenf_rappen = true,
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

    // MWST-Abrechnung (eCH-0217) — eigener Modus; ein hier mitgegebenes MT940
    // liefert die Gegenprobe bzw. bei «vereinnahmt» direkt die Entgelte.
    if args.mwst {
        return run_mwst(&args);
    }

    // MT940 **allein** → Kontoauszugs-Report. Zusammen mit einem Erklärungs-Input
    // (Vermögensausweis/eCH-0196) bildet es stattdessen die Basis der Erklärung (unten).
    if args.from_mt940.is_some()
        && args.from_vermoegensausweis.is_none()
        && args.from_ech0196.is_none()
        && args.from_pdf.is_none()
    {
        return run_mt940(args.from_mt940.as_ref().unwrap(), args.wertschriften);
    }

    if let Some(path) = &args.from_camt {
        return run_camt(path, args.wertschriften);
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
        let mut mt940_stmt: Option<mt940::Statement> = None;
        let mut wertschriften_cents: Option<i64> = None;

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
                    mt940_stmt = Some(stmt);
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
                    // Wertschriften-Steuerwert (in Rappen) für die Pseudo-Bilanz.
                    let chf: i64 = secs.iter().filter_map(|s| s.tax_value.map(|t| t.cantonal)).sum();
                    wertschriften_cents = Some(chf * 100);
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

        // Pseudo-Jahresrechnung (Bilanz inkl. Wertschriften + ER) als PDF/MD.
        if let Some(stmt) = &mt940_stmt {
            let ps = mt940::pseudo_statements(stmt, wertschriften_cents);
            write_pseudo_jahresrechnung(&ps);
        }
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
fn run_mt940(path: &str, wertschriften_chf: Option<i64>) -> ExitCode {
    let text = match read_bank_text(Path::new(path)) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let stmt = match mt940::parse(&text) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("MT940 nicht lesbar: {e}");
            return ExitCode::FAILURE;
        }
    };
    report_statement(&stmt, wertschriften_chf)
}

/// Liest einen Kontoauszug als Text. Banken liefern MT940 meist in **Latin-1**;
/// `from_utf8_lossy` würde daraus Ersatzzeichen machen, deshalb bei ungültigem
/// UTF-8 zeichenweise als ISO-8859-1 dekodieren (dort ist jedes Byte gültig).
fn read_bank_text(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Konnte {} nicht lesen: {e}", path.display()))?;
    Ok(match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => e.into_bytes().iter().map(|&b| b as char).collect(),
    })
}

/// `--from-camt`: camt.053 (ISO 20022) einlesen — eine Datei **oder** ein Verzeichnis
/// mit Tagesdateien (`*.xml`), die zu einem Auszug aggregiert werden.
fn run_camt(path: &str, wertschriften_chf: Option<i64>) -> ExitCode {
    let p = Path::new(path);
    let mut xmls: Vec<String> = Vec::new();
    if p.is_dir() {
        let mut files: Vec<PathBuf> = match std::fs::read_dir(p) {
            Ok(rd) => rd
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e == "xml"))
                .collect(),
            Err(e) => {
                eprintln!("Konnte {path} nicht lesen: {e}");
                return ExitCode::FAILURE;
            }
        };
        files.sort();
        for f in files {
            match std::fs::read(&f) {
                Ok(b) => xmls.push(String::from_utf8_lossy(&b).into_owned()),
                Err(e) => eprintln!("Hinweis: {} übersprungen: {e}", f.display()),
            }
        }
    } else {
        match std::fs::read(p) {
            Ok(b) => xmls.push(String::from_utf8_lossy(&b).into_owned()),
            Err(e) => {
                eprintln!("Konnte {path} nicht lesen: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    println!("camt.053: {} Tagesdatei(en) eingelesen.", xmls.len());
    let stmt = match camt053::parse_many(&xmls) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("camt.053 nicht lesbar: {e}");
            return ExitCode::FAILURE;
        }
    };
    report_statement(&stmt, wertschriften_chf)
}

/// Gemeinsamer Report für MT940 und camt.053: Kategorien, Cash-Basis-ER, Bilanz-Position,
/// Cash-Flow-Rechnung (PDF/MD) + JSON-Zusammenfassung.
fn report_statement(stmt: &mt940::Statement, wertschriften_chf: Option<i64>) -> ExitCode {
    let cur = stmt.closing.as_ref().or(stmt.opening.as_ref()).map(|b| b.currency.clone()).unwrap_or_else(|| "CHF".into());
    println!("Kontoauszug: {}", stmt.account);
    if let Some(b) = &stmt.opening {
        println!("  Eröffnungssaldo {} : {} {}", b.date, b.currency, mt940::format_cents(b.amount_cents));
    }
    if let Some(b) = &stmt.closing {
        println!("  Schlusssaldo    {} : {} {}", b.date, b.currency, mt940::format_cents(b.amount_cents));
    }
    let (credit, debit) = (stmt.total_credit_cents(), stmt.total_debit_cents());
    println!("  Buchungen: {}", stmt.transactions.len());

    let categories = mt940::categorize(stmt);
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

    // Pseudo-Jahresrechnung (Entwurf für den Vermögensverwalter). Wertschriften via
    // --wertschriften (z. B. aus der Jahresrechnung) oder im kombinierten Flow aus dem
    // Vermögensausweis.
    let ps = mt940::pseudo_statements(stmt, wertschriften_chf.map(|c| c * 100));
    let report = serde_json::json!({
        "account": stmt.account,
        "opening": stmt.opening,
        "closing": stmt.closing,
        "transactionCount": stmt.transactions.len(),
        "totalCreditCents": credit,
        "totalDebitCents": debit,
        "categories": categories,
        "pseudoStatements": ps,
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
    // Pseudo-Jahresrechnung als Markdown + PDF — der Vermögensverwalter geht darüber.
    write_pseudo_jahresrechnung(&ps);
    ExitCode::SUCCESS
}

/// Schreibt die Pseudo-Jahresrechnung als Markdown und als zweiseitiges PDF (Seite 1
/// Bilanz, Seite 2 Erfolgsrechnung) nach `data/`.
fn write_pseudo_jahresrechnung(ps: &mt940::PseudoStatements) {
    let _ = std::fs::create_dir_all("data");
    let md_out = Path::new("data").join("Cash-Flow-Rechnung.md");
    match std::fs::write(&md_out, mt940::pseudo_statements_markdown(ps)) {
        Ok(()) => println!("Cash-Flow-Rechnung (Markdown): {}", md_out.display()),
        Err(e) => eprintln!("Hinweis: konnte {} nicht schreiben: {e}", md_out.display()),
    }
    let pdf_out = Path::new("data").join("Cash-Flow-Rechnung.pdf");
    match std::fs::write(&pdf_out, pdf_report::pseudo_statements_pdf(ps)) {
        Ok(()) => println!("Cash-Flow-Rechnung (PDF, S.1 Bilanz / S.2 ER): {}", pdf_out.display()),
        Err(e) => eprintln!("Hinweis: konnte {} nicht schreiben: {e}", pdf_out.display()),
    }
}

// --------------------------------------------------------------------------- //
// MWST-Abrechnung (eCH-0217 V2.0.0)
// --------------------------------------------------------------------------- //

/// Rappen → `"123'456.78"`.
fn chf(cents: i64) -> String {
    let neg = cents < 0;
    let a = cents.abs();
    let whole = (a / 100).to_string();
    let mut grouped = String::new();
    for (i, c) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push('\'');
        }
        grouped.push(c);
    }
    format!("{}{grouped}.{:02}", if neg { "-" } else { "" }, a % 100)
}

/// Eine Formularzeile: Ziffer, Text, rechtsbündiger Betrag in der Umsatzspalte.
fn line(nr: &str, text: &str, cents: i64) {
    println!("  {nr:<5}{text:<52}{:>14}", chf(cents));
}

/// Formularzeile mit beiden Spalten des MWST-Formulars: «Leistungen CHF» und
/// «Steuer CHF».
fn line2(nr: &str, text: &str, turnover: i64, tax: i64) {
    println!("  {nr:<5}{text:<52}{:>14}{:>14}", chf(turnover), chf(tax));
}

/// Formularzeile, deren Betrag in der **Steuer**-Spalte steht (Ziff. 399/4xx/5xx).
fn line_tax(nr: &str, text: &str, tax: i64) {
    println!("  {nr:<5}{text:<52}{:>14}{:>14}", "", chf(tax));
}

/// `--mwst`: MWST-Abrechnung nach **eCH-0217 V2.0.0** erzeugen, rechnen, validieren.
fn run_mwst(args: &Args) -> ExitCode {
    match run_mwst_inner(args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn run_mwst_inner(args: &Args) -> Result<ExitCode, String> {
    use model_mwst::{Amount, Percent, Rounding};

    let cfg = settings::load();
    let m = &cfg.mwst;

    let amt = |o: &Option<String>| -> Result<Option<Amount>, String> {
        o.as_deref().map(Amount::parse_chf).transpose()
    };

    // Kontoauszug (optional): Gegenprobe bzw. bei «vereinnahmt» die Ist-Entgelte.
    let credits = match &args.from_mt940 {
        Some(p) => {
            let stmt = mt940::parse(&read_bank_text(Path::new(p))?)
                .map_err(|e| format!("MT940 nicht lesbar: {e}"))?;
            let c = mwst::credits_from_statement(&stmt);
            Some((c, stmt))
        }
        None => None,
    };

    // ---- Periode ----
    let (from, till) = match &args.periode {
        Some(s) => mwst::parse_period(s)?,
        None => credits
            .as_ref()
            .and_then(|(c, _)| c.period.clone())
            .ok_or(
                "Abrechnungsperiode fehlt — mit --periode angeben (z. B. --periode S1/2026, \
                 --periode Q2/2026 oder --periode 2026-01-01:2026-06-30) oder ein MT940 \
                 mitgeben, dessen :60F:/:62F:-Salden die Periode aufspannen.",
            )?,
    };

    // ---- Methode und Abrechnungsart ----
    let method = if args.effektiv {
        mwst::Method::Effektiv
    } else {
        match m.methode.as_deref().map(str::to_lowercase).as_deref() {
            Some("effektiv") => mwst::Method::Effektiv,
            _ => mwst::Method::Saldosteuersatz,
        }
    };
    let vereinnahmt =
        args.vereinnahmt || m.abrechnungsart.as_deref().map(str::to_lowercase).as_deref() == Some("vereinnahmt");

    // ---- Ziff. 200 ----
    let total = match amt(&args.umsatz)? {
        Some(a) => a,
        None => match (&credits, vereinnahmt) {
            // «vereinnahmt»: die Kundenzahlungen des Auszugs SIND die Entgelte.
            (Some((c, _)), true) => Amount(c.turnover_cents),
            (Some(_), false) => {
                return Err(
                    "Bei «vereinbart» (Art. 39 Abs. 1 MWSTG) sind die fakturierten Entgelte \
                     massgebend, nicht die Zahlungseingänge. Ziff. 200 mit --umsatz aus der \
                     Buchhaltung angeben (Erlöskonto der Periode) — das MT940 dient dann als \
                     Gegenprobe. Mit --vereinnahmt (nur mit ESTV-Bewilligung nach Art. 39 \
                     Abs. 2) würde stattdessen der Zahlungseingang deklariert."
                        .into(),
                )
            }
            (None, _) => {
                return Err("Ziff. 200 fehlt — mit --umsatz angeben (z. B. --umsatz 123456.78).".into())
            }
        },
    };

    // ---- Ziff. 910: Dividenden aus dem Auszug, wenn nicht ausdrücklich gesetzt ----
    let donations = match amt(&args.dividenden)? {
        Some(a) => Some(a),
        None => credits
            .as_ref()
            .map(|(c, _)| c.dividends_cents)
            .filter(|&d| d != 0)
            .map(Amount),
    };

    let tax_rate = match args.satz.as_deref().or(m.tax_rate.as_deref()) {
        Some(s) => Percent::parse(s)?,
        None => {
            return Err(
                "Steuersatz fehlt — mit --satz angeben (Saldosteuersatz, z. B. --satz 6.2) \
                 oder in settings.json unter mwst.taxRate."
                    .into(),
            )
        }
    };

    let positions = args
        .positionen
        .iter()
        .map(|s| mwst::Position::parse(s))
        .collect::<Result<Vec<_>, String>>()?;

    let p = mwst::Params {
        positions,
        uid: args
            .uid
            .clone()
            .or_else(|| m.uid.clone())
            .ok_or("MWST-Nummer fehlt — mit --uid angeben oder in settings.json unter mwst.uid eintragen (z. B. \"CHE-123.456.789 MWST\").")?,
        organisation_name: args
            .firma
            .clone()
            .or_else(|| m.organisation_name.clone())
            .ok_or("Firmenname fehlt — mit --firma angeben oder in settings.json unter mwst.organisationName eintragen.")?,
        period_from: from.clone(),
        period_till: till.clone(),
        type_of_submission: if args.jahresabstimmung {
            3
        } else if args.korrektur {
            2
        } else {
            1
        },
        form_of_reporting: if vereinnahmt { 2 } else { 1 },
        method,
        total_consideration: total,
        supplies_to_foreign_countries: amt(&args.export)?,
        supplies_abroad: None,
        supplies_exempt_from_tax: amt(&args.ausgenommen)?,
        reduction_of_consideration: amt(&args.entgeltsminderung)?,
        activity_id: args.activity_id.clone().or_else(|| m.activity_id.clone()),
        tax_rate,
        gross_or_net: if args.brutto { 2 } else { 1 },
        input_tax_material_and_services: amt(&args.vorsteuer_material)?,
        input_tax_investments: amt(&args.vorsteuer_investitionen)?,
        donations,
        subsidies: amt(&args.subventionen)?,
        business_reference_id: String::new(),
        generation_time: mwst::now_utc_iso(),
        manufacturer: m.manufacturer.clone(),
        rounding: if args.fuenf_rappen { Rounding::FiveRappen } else { Rounding::Rappen },
    };

    let doc = mwst::build(&p)?;
    let errs = doc.validate();
    if !errs.is_empty() {
        for e in &errs {
            eprintln!("Fehler: {e}");
        }
        return Err("Deklaration nicht plausibel — siehe oben.".into());
    }

    // ---- Report im Aufbau des MWST-Formulars ----
    let tc = &doc.turnover_computation;
    println!(
        "MWST-Abrechnung (eCH-0217 V2.0.0) — {}",
        mwst::period_label(&from, &till)
    );
    println!(
        "  {} · {} · {}",
        p.organisation_name,
        doc.general_information.uid,
        match method {
            mwst::Method::Saldosteuersatz => "Saldosteuersatz",
            mwst::Method::Effektiv => "effektive Methode",
        }
    );
    println!(
        "  Entgelte {} · {}\n",
        if vereinnahmt { "vereinnahmt (Art. 39 Abs. 2)" } else { "vereinbart (Art. 39 Abs. 1)" },
        match p.type_of_submission {
            2 => "Korrekturabrechnung",
            3 => "Jahresabstimmung",
            _ => "Ersteinreichung",
        }
    );

    println!("I. Umsatz{}", if method == mwst::Method::Saldosteuersatz || args.brutto { " (brutto, inkl. MWST)" } else { " (netto)" });
    line("200", "Total der vereinbarten bzw. vereinnahmten Entgelte", tc.total_consideration.rappen());
    if let Some(a) = tc.supplies_to_foreign_countries {
        line("220", "Von der Steuer befreite Leistungen (Exporte)", a.rappen());
    }
    if let Some(a) = tc.supplies_exempt_from_tax {
        line("230", "Von der Steuer ausgenommene Leistungen", a.rappen());
    }
    if let Some(a) = tc.reduction_of_consideration {
        line("235", "Entgeltsminderungen", a.rappen());
    }
    line("289", "Total Abzüge", tc.total_deductions().rappen());
    line("299", "Steuerbarer Gesamtumsatz", tc.taxable_turnover().rappen());

    println!("\nII. Steuerberechnung");
    println!("  {:<5}{:<52}{:>14}{:>14}", "", "", "Leistungen", "Steuer");
    if let Some(s) = &doc.simple_tax_rate_method {
        for r in &s.supplies_per_tax_rate {
            line2(
                "300",
                &format!("Leistungen Tätigkeit {} zum Saldosteuersatz {} %", r.activity_id, r.tax_rate),
                r.turnover.rappen(),
                model_mwst::line_tax(r.turnover, r.tax_rate, false).rappen(),
            );
        }
    }
    if let Some(e) = &doc.effective_reporting_method {
        let gross = e.gross_or_net == 2;
        for r in &e.supplies_per_tax_rate {
            line2(
                "300",
                &format!("Leistungen zu {} % ({})", r.tax_rate, if gross { "brutto" } else { "netto" }),
                r.turnover.rappen(),
                model_mwst::line_tax(r.turnover, r.tax_rate, gross).rappen(),
            );
        }
        if let Some(a) = e.input_tax_material_and_services {
            line_tax("400", "Vorsteuer auf Material- und Dienstleistungsaufwand", a.rappen());
        }
        if let Some(a) = e.input_tax_investments {
            line_tax("405", "Vorsteuer auf Investitionen / übrigem Betriebsaufwand", a.rappen());
        }
    }
    line_tax("399", "Total geschuldete Steuer", doc.total_tax_due().rappen());
    let pay = doc.payable_tax.rappen();
    if pay >= 0 {
        line_tax("500", "An die Eidg. Steuerverwaltung zu bezahlen", pay);
    } else {
        line_tax("510", "Guthaben der steuerpflichtigen Person", -pay);
    }

    if let Some(o) = &doc.other_flows_of_funds {
        println!("\nIII. Andere Mittelflüsse (Art. 18 Abs. 2)");
        if let Some(a) = o.subsidies {
            line("900", "Subventionen, Tourismusabgaben usw.", a.rappen());
        }
        if let Some(a) = o.donations {
            line("910", "Spenden, Dividenden, Schadenersatz usw.", a.rappen());
        }
        println!("       (Ziff. 9xx sind keine Entgelte — sie erhöhen die Steuer nicht.)");
    }

    // ---- Gegenprobe gegen den Kontoauszug ----
    if let Some((c, stmt)) = &credits {
        println!("\nGegenprobe MT940 ({})", stmt.account);
        line("", "Gutschriften total", stmt.total_credit_cents());
        line("", "− Dividenden/Zinsen (Ziff. 910)", c.dividends_cents);
        line("", "− Wertschriften/Übriges (kein Entgelt)", c.other_cents);
        line("", "= Kundenzahlungen (Ist)", c.turnover_cents);
        line("", "Deklariert (Ziff. 200)", total.rappen());
        let diff = c.turnover_cents - total.rappen();
        line(
            "",
            if vereinnahmt { "Differenz" } else { "Differenz (Debitorenverschiebung)" },
            diff,
        );

        let review: Vec<_> = c
            .lines
            .iter()
            .filter(|l| l.class != mwst::CreditClass::Turnover)
            .collect();
        if !review.is_empty() {
            println!("\n  Nicht als Entgelt gezählt ({} Gutschriften) — bitte durchsehen:", review.len());
            for l in &review {
                println!(
                    "    {} {:>12}  {:<28} {}",
                    l.date,
                    chf(l.amount_cents),
                    l.class.label(),
                    l.description.chars().take(46).collect::<String>()
                );
            }
        }
    }

    // ---- XML schreiben und validieren ----
    let xml = taxtsueri::mwst_to_xml(doc)?;
    let out_path = Path::new("data").join(format!("mwst-abrechnung-{from}-bis-{till}.xml"));
    std::fs::create_dir_all("data")
        .and_then(|_| std::fs::write(&out_path, &xml))
        .map_err(|e| format!("Konnte {} nicht schreiben: {e}", out_path.display()))?;
    println!("\neCH-0217-XML geschrieben nach: {}", out_path.display());

    let schema = Path::new("schema/eCH-0217-2-0-0.xsd");
    if schema.exists() {
        match std::process::Command::new("xmllint")
            .args(["--nonet", "--noout", "--schema"])
            .arg(schema)
            .arg(&out_path)
            .status()
        {
            Ok(s) if s.success() => println!("eCH-0217-Validierung: OK"),
            Ok(_) => return Err("eCH-0217-Validierung fehlgeschlagen (siehe xmllint-Ausgabe oben)".into()),
            Err(_) => println!("Hinweis: xmllint nicht gefunden – Validierung übersprungen"),
        }
    } else {
        println!("Hinweis: schema/eCH-0217-2-0-0.xsd fehlt – ./scripts/fetch-schemas.sh ausführen");
    }
    println!(
        "Upload: ESTV SuisseTax → «MWST abrechnen» → Abrechnungsdaten importieren \
         (nur eCH-0217 V2.0.0)."
    );
    Ok(ExitCode::SUCCESS)
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
