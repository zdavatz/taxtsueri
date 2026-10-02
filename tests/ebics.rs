//! EBICS-Initialisierung: Schlüsselablage, INI/HIA-Requests gegen die offiziellen
//! EBICS-H004-Schemas (`schema/ebics/`, via xmllint) und der INI-Brief.
#![cfg(feature = "ebics")]

use base64::Engine;
use std::io::Read;
use std::process::Command;
use taxtsueri::ebics::{self, KeyKind, Keys, Params};
use taxtsueri::ebics_brief;

fn params() -> Params {
    Params {
        host_id: "EBXTEST".into(),
        url: "https://ebics.example.invalid/ebicsweb".into(),
        partner_id: "CH000001".into(),
        user_id: "CHT00001".into(),
        user_name: "Beispiel GmbH".into(),
    }
}

fn validate(schema: &str, xml: &str, tmp_name: &str) -> bool {
    if !std::path::Path::new(schema).exists() || Command::new("xmllint").arg("--version").output().is_err() {
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

/// Holt die Auftragsdaten wieder aus dem Request: base64 → zlib → XML.
fn unpack_order_data(request: &str) -> String {
    let start = request.find("<OrderData>").expect("OrderData") + "<OrderData>".len();
    let end = request.find("</OrderData>").expect("OrderData-Ende");
    let zipped = base64::engine::general_purpose::STANDARD.decode(&request[start..end]).expect("base64");
    let mut xml = String::new();
    flate2::read::ZlibDecoder::new(&zipped[..]).read_to_string(&mut xml).expect("zlib");
    xml
}

#[test]
fn init_requests_validate_and_keys_persist() {
    let dir = std::env::temp_dir().join(format!("taxtsueri-ebics-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let p = params();

    let (keys, generated) = Keys::load_or_generate(&dir).expect("Schlüssel erzeugen");
    assert!(generated);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.join("a006.pem")).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "privater Schlüssel muss 0600 sein");
    }

    // Requests gegen das EBICS-H004-Schema.
    let ini = ebics::ini_request(&p, &keys);
    let hia = ebics::hia_request(&p, &keys);
    assert!(validate("schema/ebics/ebics_H004.xsd", &ini, "taxtsueri-ebics-ini.xml"));
    assert!(validate("schema/ebics/ebics_H004.xsd", &hia, "taxtsueri-ebics-hia.xml"));

    // Die Auftragsdaten überstehen zlib + base64 und validieren ihrerseits.
    let ini_data = unpack_order_data(&ini);
    let hia_data = unpack_order_data(&hia);
    assert_eq!(ini_data, ebics::ini_order_data(&p, &keys));
    assert_eq!(hia_data, ebics::hia_order_data(&p, &keys));
    assert!(validate("schema/ebics/ebics_signature.xsd", &ini_data, "taxtsueri-ebics-ini-data.xml"));
    assert!(validate("schema/ebics/ebics_H004.xsd", &hia_data, "taxtsueri-ebics-hia-data.xml"));

    // Zweiter Aufruf lädt dieselben Schlüssel, statt neue zu erzeugen.
    let (again, generated) = Keys::load_or_generate(&dir).expect("Schlüssel laden");
    assert!(!generated);
    assert_eq!(again.state.created, keys.state.created);
    for kind in [KeyKind::Signature, KeyKind::Authentication, KeyKind::Encryption] {
        assert_eq!(ebics::public_parts(again.get(kind)), ebics::public_parts(keys.get(kind)));
    }

    // Ein unvollständiger Schlüsselsatz wird nicht stillschweigend ergänzt.
    std::fs::remove_file(dir.join("e002.pem")).unwrap();
    assert!(Keys::load_or_generate(&dir).is_err());

    // INI-Brief: eine Seite pro Schlüssel.
    let pdf = ebics_brief::ini_brief_pdf(&p, &keys);
    let doc = lopdf::Document::load_mem(&pdf).expect("PDF lesbar");
    assert_eq!(doc.get_pages().len(), 3);

    let _ = std::fs::remove_dir_all(&dir);
}
