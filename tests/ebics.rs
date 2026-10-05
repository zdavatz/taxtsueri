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

// ---- Abholaufträge: HPB und Z53 gegen eine simulierte Bank ----------------------------

use aes::cipher::{block_padding::NoPadding, BlockEncryptMut, KeyIvInit};
use rsa::{Pkcs1v15Encrypt, RsaPublicKey};
use std::io::Write;
use taxtsueri::ebics_download::{self, BankKey, BankKeys, DateRange};

fn b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Verpackt Auftragsdaten wie eine Bank nach E002: zlib → AES-128-CBC (IV 0, Padding
/// nach ANSI X9.23) → base64; der AES-Schlüssel wird mit dem E002-Schlüssel des
/// Teilnehmers RSA-verschlüsselt.
fn bank_encrypt(keys: &Keys, plain: &[u8]) -> (String, String) {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(plain).unwrap();
    let mut data = z.finish().unwrap();
    let pad = 16 - data.len() % 16;
    data.extend(std::iter::repeat(0u8).take(pad - 1));
    data.push(pad as u8);

    let aes_key = [0x42u8; 16];
    let encrypted = cbc::Encryptor::<aes::Aes128>::new_from_slices(&aes_key, &[0u8; 16])
        .unwrap()
        .encrypt_padded_vec_mut::<NoPadding>(&data);
    let transaction_key = RsaPublicKey::from(&keys.encryption)
        .encrypt(&mut rand::rngs::OsRng, Pkcs1v15Encrypt, &aes_key)
        .unwrap();
    (b64(&transaction_key), b64(&encrypted))
}

fn bank_keys() -> BankKeys {
    BankKeys {
        authentication: BankKey { modulus: "Crw=".into(), exponent: "AQAB".into() },
        encryption: BankKey { modulus: "Cr0=".into(), exponent: "AQAB".into() },
        fetched: "2026-01-01T00:00:00Z".into(),
    }
}

fn statement_zip() -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    // A ist ein lesbares camt.053 (→ Unterordner mit der IBAN), B nicht (→ Zielverzeichnis).
    let camt = r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.04"><BkToCstmrStmt><Stmt><Acct><Id><IBAN>CH9300762011623852957</IBAN></Id></Acct></Stmt></BkToCstmrStmt></Document>"#;
    for (name, body) in [("2026-01-05_Z53_TEST_CHF_A.xml", camt), ("sub/2026-01-06_Z53_TEST_CHF_B.xml", "<Document>b</Document>")] {
        zip.start_file(name, options).unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("taxtsueri-ebics-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn signed_requests_validate_against_h004() {
    let dir = temp_dir("signed");
    let (keys, _) = Keys::load_or_generate(&dir).expect("Schlüssel erzeugen");
    let (p, bank) = (params(), bank_keys());
    let range = DateRange::new("2026-01-01", "2026-06-30").unwrap();
    for (name, xml) in [
        ("hpb", ebics_download::hpb_request(&p, &keys).unwrap()),
        ("init", ebics_download::download_init_request(&p, &keys, &bank, "Z53", None).unwrap()),
        ("init-range", ebics_download::download_init_request(&p, &keys, &bank, "Z53", Some(&range)).unwrap()),
        ("transfer", ebics_download::download_transfer_request(&p, &keys, &"A".repeat(32), 2, true).unwrap()),
        ("receipt", ebics_download::download_receipt_request(&p, &keys, &"A".repeat(32), true).unwrap()),
    ] {
        assert!(
            validate("schema/ebics/ebics_H004.xsd", &xml, &format!("taxtsueri-ebics-{name}.xml")),
            "{name} validiert nicht"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hpb_and_segmented_download_against_simulated_bank() {
    let dir = temp_dir("bank");
    let out = temp_dir("camt");
    let (keys, _) = Keys::load_or_generate(&dir).expect("Schlüssel erzeugen");
    let p = params();

    // HPB: die Bank liefert ihre Schlüssel verschlüsselt mit unserem E002-Schlüssel.
    let hpb_data = r#"<?xml version="1.0" encoding="UTF-8"?><HPBResponseOrderData xmlns="urn:org:ebics:H004" xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><AuthenticationPubKeyInfo><PubKeyValue><ds:RSAKeyValue><ds:Modulus>Crw=</ds:Modulus><ds:Exponent>AQAB</ds:Exponent></ds:RSAKeyValue></PubKeyValue><AuthenticationVersion>X002</AuthenticationVersion></AuthenticationPubKeyInfo><EncryptionPubKeyInfo><PubKeyValue><ds:RSAKeyValue><ds:Modulus>Cr0=</ds:Modulus><ds:Exponent>AQAB</ds:Exponent></ds:RSAKeyValue></PubKeyValue><EncryptionVersion>E002</EncryptionVersion></EncryptionPubKeyInfo><HostID>EBXTEST</HostID></HPBResponseOrderData>"#;
    let (key, data) = bank_encrypt(&keys, hpb_data.as_bytes());
    let bank = ebics_download::fetch_bank_keys(&p, &keys, &mut |request| {
        assert!(request.contains("<OrderType>HPB</OrderType>"));
        Ok(format!(
            r#"<ebicsKeyManagementResponse xmlns="urn:org:ebics:H004"><header authenticate="true"><static/><mutable><ReturnCode>000000</ReturnCode><ReportText>[EBICS_OK] OK</ReportText></mutable></header><body><DataTransfer><DataEncryptionInfo authenticate="true"><TransactionKey>{key}</TransactionKey></DataEncryptionInfo><OrderData>{data}</OrderData></DataTransfer><ReturnCode authenticate="true">000000</ReturnCode></body></ebicsKeyManagementResponse>"#
        ))
    })
    .expect("HPB");
    assert_eq!(bank.authentication, bank_keys().authentication);
    assert_eq!(bank.encryption, bank_keys().encryption);

    // Z53 in zwei Segmenten: geteilt wird der base64-Text, bewusst nicht an einer 4er-Grenze.
    let (key, data) = bank_encrypt(&keys, &statement_zip());
    let (first, second) = data.split_at(data.len() / 2 + 1);
    let mut phases = Vec::new();
    let mut bank_sim = |request: &str| -> Result<String, String> {
        let reply = |head: &str, body: &str, code: &str| {
            format!(r#"<ebicsResponse xmlns="urn:org:ebics:H004"><header authenticate="true">{head}<ReturnCode>{code}</ReturnCode><ReportText>ok</ReportText></mutable></header><body>{body}<ReturnCode authenticate="true">000000</ReturnCode></body></ebicsResponse>"#)
        };
        if request.contains("<TransactionPhase>Initialisation</TransactionPhase>") {
            phases.push("init".to_string());
            assert!(request.contains("<OrderType>Z53</OrderType>"));
            Ok(reply(
                r#"<static><TransactionID>TX1</TransactionID><NumSegments>2</NumSegments></static><mutable><TransactionPhase>Initialisation</TransactionPhase><SegmentNumber lastSegment="false">1</SegmentNumber>"#,
                &format!(r#"<DataTransfer><DataEncryptionInfo authenticate="true"><TransactionKey>{key}</TransactionKey></DataEncryptionInfo><OrderData>{first}</OrderData></DataTransfer>"#),
                "000000",
            ))
        } else if request.contains("<TransactionPhase>Transfer</TransactionPhase>") {
            phases.push("transfer".to_string());
            assert!(request.contains(r#"<SegmentNumber lastSegment="true">2</SegmentNumber>"#));
            Ok(reply(
                r#"<static><TransactionID>TX1</TransactionID></static><mutable><TransactionPhase>Transfer</TransactionPhase><SegmentNumber lastSegment="true">2</SegmentNumber>"#,
                &format!("<DataTransfer><OrderData>{second}</OrderData></DataTransfer>"),
                "000000",
            ))
        } else {
            let code = request.split("<ReceiptCode>").nth(1).and_then(|r| r.chars().next()).unwrap();
            phases.push(format!("receipt {code}"));
            Ok(reply("<static><TransactionID>TX1</TransactionID></static><mutable><TransactionPhase>Receipt</TransactionPhase>", "", "011000"))
        }
    };

    let extracted = ebics_download::download(&p, &keys, &bank, "Z53", None, &mut bank_sim, |zip| {
        ebics_download::extract_statements(zip, &out)
    })
    .expect("Download")
    .expect("Daten vorhanden");
    assert_eq!(
        extracted.written,
        ["CH9300762011623852957/2026-01-05_Z53_TEST_CHF_A.xml", "2026-01-06_Z53_TEST_CHF_B.xml"]
    );
    assert!(out.join("CH9300762011623852957/2026-01-05_Z53_TEST_CHF_A.xml").exists());
    assert_eq!(std::fs::read_to_string(out.join("2026-01-06_Z53_TEST_CHF_B.xml")).unwrap(), "<Document>b</Document>");

    // Scheitert die Verarbeitung, wird negativ quittiert — die Daten bleiben abholbar.
    let failed = ebics_download::download(&p, &keys, &bank, "Z53", None, &mut bank_sim, |_| Err::<(), _>("Platte voll".to_string()));
    assert_eq!(failed.unwrap_err(), "Platte voll");
    assert_eq!(phases, ["init", "transfer", "receipt 0", "init", "transfer", "receipt 1"]);

    // Ein zweiter Abruf überschreibt vorhandene Auszüge nicht.
    let again = ebics_download::extract_statements(&statement_zip(), &out).unwrap();
    assert!(again.written.is_empty());
    assert_eq!(again.skipped.len(), 2);

    // «Keine Daten» ist kein Fehler.
    let none = ebics_download::download(&p, &keys, &bank, "Z53", None, &mut |_| {
        Ok(r#"<ebicsResponse xmlns="urn:org:ebics:H004"><header authenticate="true"><static/><mutable><TransactionPhase>Initialisation</TransactionPhase><ReturnCode>000000</ReturnCode><ReportText>[EBICS_OK] OK</ReportText></mutable></header><body><ReturnCode authenticate="true">090005</ReturnCode></body></ebicsResponse>"#.to_string())
    }, |_| Ok(()));
    assert_eq!(none, Ok(None));

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&out);
}
