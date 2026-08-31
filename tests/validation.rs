//! Integrationstest: das erzeugte XML muss gegen das echte eCH-0119-XSD validieren.
//! Benötigt `xmllint` und die Schemas in `schema/` (via `scripts/fetch-schemas.sh`).
//! Fehlt eines von beidem, wird der Test übersprungen statt zu scheitern.

use std::process::Command;
use taxtsueri::{dataset, dataset_jp, model, model_mwst, model_zh, mwst};

fn validate(schema: &str, xml: &str, tmp_name: &str) -> bool {
    if !std::path::Path::new(schema).exists()
        || Command::new("xmllint").arg("--version").output().is_err()
    {
        eprintln!("übersprungen: {schema} oder xmllint fehlt");
        return true;
    }
    let tmp = std::env::temp_dir().join(tmp_name);
    std::fs::write(&tmp, xml).expect("write tmp");
    let out = Command::new("xmllint")
        .args(["--nonet", "--noout", "--schema", schema])
        .arg(&tmp)
        .output()
        .expect("run xmllint");
    let _ = std::fs::remove_file(&tmp);
    if !out.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&out.stderr));
    }
    out.status.success()
}

#[test]
fn jp_example_validates_against_ech0276() {
    let xml = quick_xml::se::to_string(&dataset_jp::example().into_message()).expect("serialize");
    assert!(validate(
        "schema/eCH-0276-1-0.xsd",
        &xml,
        "taxtsueri-jp-validation.xml"
    ));
}

/// Die MWST-Abrechnung (eCH-0217 V2.0.0) muss gegen das offizielle XSD validieren —
/// beide modellierten Zweige des `xs:choice`, weil nur einer davon je Datei auftritt.
#[test]
fn mwst_declaration_validates_against_ech0217() {
    let base = mwst::Params {
        uid: "CHE-123.456.789 MWST".into(),
        organisation_name: "Beispiel GmbH".into(),
        period_from: "2026-01-01".into(),
        period_till: "2026-06-30".into(),
        generation_time: mwst::now_utc_iso(),
        donations: Some(model_mwst::Amount(89_975)),
        ..Default::default()
    };

    // Saldosteuersatz (simpleTaxRateMethod, Perioden ab 01.01.2025).
    let sss = mwst::build(&mwst::Params {
        method: mwst::Method::Saldosteuersatz,
        total_consideration: model_mwst::Amount(12_345_678),
        activity_id: Some("00001".into()),
        tax_rate: model_mwst::Percent(620),
        ..base.clone()
    })
    .expect("SSS bauen");
    assert!(sss.validate().is_empty(), "{:?}", sss.validate());
    let xml = taxtsueri::mwst_to_xml(sss).expect("serialize SSS");
    assert!(xml.contains("<eCH-0217:simpleTaxRateMethod>"));
    assert!(validate(
        "schema/eCH-0217-2-0-0.xsd",
        &xml,
        "taxtsueri-mwst-sss-validation.xml"
    ));

    // Effektive Methode mit Vorsteuerabzug und Exportabzug (Ziff. 220).
    let eff = mwst::build(&mwst::Params {
        method: mwst::Method::Effektiv,
        total_consideration: model_mwst::Amount(10_000_000),
        supplies_to_foreign_countries: Some(model_mwst::Amount(1_000_000)),
        tax_rate: model_mwst::Percent(810),
        input_tax_material_and_services: Some(model_mwst::Amount(120_000)),
        input_tax_investments: Some(model_mwst::Amount(30_000)),
        ..base
    })
    .expect("effektiv bauen");
    assert!(eff.validate().is_empty(), "{:?}", eff.validate());
    // 90'000.00 netto zu 8.1 % = 7'290.00, abzüglich 1'500.00 Vorsteuer = 5'790.00.
    assert_eq!(eff.payable_tax, model_mwst::Amount(579_000));
    let xml = taxtsueri::mwst_to_xml(eff).expect("serialize effektiv");
    assert!(xml.contains("<eCH-0217:effectiveReportingMethod>"));
    assert!(validate(
        "schema/eCH-0217-2-0-0.xsd",
        &xml,
        "taxtsueri-mwst-eff-validation.xml"
    ));
}

#[test]
fn zh_v3_core_validates_against_ech0119_v3() {
    // Kern des ZH-Steuererklärungs-Barcodes: eCH-0119 v3 (ssk-prefixed). Die
    // zh:-cantonExtension (strict wildcard) kommt in Phase 2 hinzu und ist nur
    // strukturell gegen das Sample prüfbar, nicht gegen diese Kern-XSD.
    let doc = dataset::example();
    let msg = model_zh::ZhMessage::from_document(&doc, 2025);
    let xml = model_zh::zh_message_to_xml(&msg).expect("serialize v3");
    assert!(validate(
        "schema/eCH-0119-2015-3-0.xsd",
        &xml,
        "taxtsueri-zh-v3-validation.xml"
    ));
}

#[test]
fn generated_xml_validates_against_ech0119() {
    let schema = "schema/eCH-0119-4-0-0.xsd";
    if !std::path::Path::new(schema).exists() {
        eprintln!("übersprungen: {schema} fehlt (scripts/fetch-schemas.sh ausführen)");
        return;
    }
    if Command::new("xmllint").arg("--version").output().is_err() {
        eprintln!("übersprungen: xmllint nicht verfügbar");
        return;
    }

    let xml = quick_xml::se::to_string(&dataset::example().into_message()).expect("serialize");
    let tmp = std::env::temp_dir().join("taxtsueri-validation.xml");
    std::fs::write(&tmp, &xml).expect("write tmp");

    let out = Command::new("xmllint")
        .args(["--nonet", "--noout", "--schema", schema])
        .arg(&tmp)
        .output()
        .expect("run xmllint");

    let _ = std::fs::remove_file(&tmp);
    assert!(
        out.status.success(),
        "eCH-0119-Validierung fehlgeschlagen:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn sample_input_json_validates() {
    let schema = "schema/eCH-0119-4-0-0.xsd";
    let sample = "examples/input.sample.json";
    if !std::path::Path::new(schema).exists() || !std::path::Path::new(sample).exists() {
        eprintln!("übersprungen: Schema oder Beispiel fehlt");
        return;
    }
    if Command::new("xmllint").arg("--version").output().is_err() {
        eprintln!("übersprungen: xmllint nicht verfügbar");
        return;
    }

    let json = std::fs::read_to_string(sample).expect("read sample");
    let doc: model::Document = serde_json::from_str(&json).expect("parse sample json");
    let xml = quick_xml::se::to_string(&doc.into_message()).expect("serialize");
    let tmp = std::env::temp_dir().join("taxtsueri-sample-validation.xml");
    std::fs::write(&tmp, &xml).expect("write tmp");

    let out = Command::new("xmllint")
        .args(["--nonet", "--noout", "--schema", schema])
        .arg(&tmp)
        .output()
        .expect("run xmllint");

    let _ = std::fs::remove_file(&tmp);
    assert!(
        out.status.success(),
        "examples/input.sample.json validiert nicht gegen eCH-0119:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
