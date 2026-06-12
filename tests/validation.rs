//! Integrationstest: das erzeugte XML muss gegen das echte eCH-0119-XSD validieren.
//! Benötigt `xmllint` und die Schemas in `schema/` (via `scripts/fetch-schemas.sh`).
//! Fehlt eines von beidem, wird der Test übersprungen statt zu scheitern.

use std::process::Command;
use taxtsueri::{dataset, dataset_jp, model, model_zh};

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
