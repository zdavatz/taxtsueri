//! EBICS-**Abholaufträge** (H004): `HPB` holt die öffentlichen Bankschlüssel, `Z53` die
//! camt.053-Kontoauszüge. Beide sind — anders als `INI`/`HIA` — **signierte** Requests.
//!
//! ## Authentifikationssignatur (X002)
//!
//! Jeder Request trägt eine `AuthSignature` (XML-DSig): SHA-256 über die mit **C14N**
//! kanonisierten Elemente mit `authenticate="true"` (Header, beim Quittieren zusätzlich
//! `TransferReceipt`), darüber RSA-PKCS#1-v1.5 mit dem X002-Schlüssel. Statt einen
//! allgemeinen Kanonisierer mitzubringen, schreibt dieses Modul die signierten Teile
//! **von vornherein kanonisch** (keine Leerzeichen zwischen Elementen, keine
//! `<leer/>`-Kurzform, Attribute alphabetisch) — die kanonische Form unterscheidet sich
//! dann nur durch die am Element wiederholten Namespace-Deklarationen der Wurzel.
//!
//! ## Verschlüsselung (E002)
//!
//! Auftragsdaten kommen so zurück: XML/ZIP → zlib → AES-128-CBC (IV 0) → base64 → in
//! Segmente geteilt. Der AES-Schlüssel («Transaktionsschlüssel») ist mit unserem
//! öffentlichen E002-Schlüssel RSA-verschlüsselt.
//!
//! Die Signatur der **Bank** auf den Antworten wird nicht geprüft; die Echtheit der
//! Gegenseite stützt sich auf TLS und die per `HPB` gegen das Bankparameterdaten-Blatt
//! verglichenen Bankschlüssel.

use crate::ebics::{
    element_texts, public_key_hash, utc_now, write_private, Keys, Params, EBICS_OK, NS_DS, NS_H004,
};
use aes::cipher::{block_padding::NoPadding, BlockDecryptMut, KeyIvInit};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;
use rsa::{Pkcs1v15Encrypt, Pkcs1v15Sign, RsaPrivateKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// Fachlicher Returncode «keine Daten zum Abholen vorhanden».
pub const NO_DOWNLOAD_DATA: &str = "090005";
/// Returncode der Quittungsphase: «Nachbearbeitung des Downloads abgeschlossen».
pub const DOWNLOAD_POSTPROCESS_DONE: &str = "011000";

const C14N: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315";
const SHA256_URI: &str = "http://www.w3.org/2001/04/xmlenc#sha256";

/// Namespace-Deklarationen der Wurzel — C14N wiederholt sie an jedem signierten Element.
fn ns_decl() -> String {
    format!(" xmlns=\"{NS_H004}\" xmlns:ds=\"{NS_DS}\"")
}

/// Text-Escaping wie in C14N (`"` bleibt in Textknoten unverändert).
fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// 16 Zufallsbytes als Hex — macht jeden Request einmalig (Schutz vor Wiedereinspielen).
fn nonce() -> String {
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// Die kanonische Form der signierten Teile eines Requests — also genau die Bytes, über
/// die der Digest der Authentifikationssignatur läuft.
fn canonical_authenticated(header: &str, receipt: Option<&str>) -> String {
    let ns = ns_decl();
    let mut out = format!("<header{ns} authenticate=\"true\">{header}</header>");
    if let Some(r) = receipt {
        out.push_str(&format!("<TransferReceipt{ns} authenticate=\"true\">{r}</TransferReceipt>"));
    }
    out
}

/// Baut einen signierten Request. `header` und `receipt` sind die **Inhalte** der
/// gleichnamigen Elemente und müssen bereits kanonisch geschrieben sein.
fn signed_request(root: &str, auth_key: &RsaPrivateKey, header: &str, receipt: Option<&str>) -> Result<String, String> {
    let ns = ns_decl();
    let digest = Sha256::digest(canonical_authenticated(header, receipt).as_bytes());
    let signed_info = format!(
        "<ds:CanonicalizationMethod Algorithm=\"{C14N}\"></ds:CanonicalizationMethod>\
         <ds:SignatureMethod Algorithm=\"http://www.w3.org/2001/04/xmldsig-more#rsa-sha256\"></ds:SignatureMethod>\
         <ds:Reference URI=\"#xpointer(//*[@authenticate='true'])\">\
         <ds:Transforms><ds:Transform Algorithm=\"{C14N}\"></ds:Transform></ds:Transforms>\
         <ds:DigestMethod Algorithm=\"{SHA256_URI}\"></ds:DigestMethod>\
         <ds:DigestValue>{}</ds:DigestValue>\
         </ds:Reference>",
        B64.encode(digest)
    );
    let signed_info_digest = Sha256::digest(format!("<ds:SignedInfo{ns}>{signed_info}</ds:SignedInfo>").as_bytes());
    let signature = auth_key
        .sign(Pkcs1v15Sign::new::<Sha256>(), &signed_info_digest)
        .map_err(|e| format!("Authentifikationssignatur: {e}"))?;
    let body = match receipt {
        Some(r) => format!("<body><TransferReceipt authenticate=\"true\">{r}</TransferReceipt></body>"),
        None => "<body></body>".to_string(),
    };
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<{root}{ns} Version=\"H004\" Revision=\"1\">\
         <header authenticate=\"true\">{header}</header>\
         <AuthSignature><ds:SignedInfo>{signed_info}</ds:SignedInfo><ds:SignatureValue>{}</ds:SignatureValue></AuthSignature>\
         {body}</{root}>\n",
        B64.encode(signature)
    ))
}

/// Anfang des statischen Headers, der HPB und der Download-Initialisierung gemeinsam ist.
fn static_start(p: &Params) -> String {
    format!(
        "<HostID>{}</HostID><Nonce>{}</Nonce><Timestamp>{}</Timestamp><PartnerID>{}</PartnerID><UserID>{}</UserID><Product Language=\"de\">taxtsueri</Product>",
        esc(&p.host_id),
        nonce(),
        utc_now(),
        esc(&p.partner_id),
        esc(&p.user_id)
    )
}

/// `HPB`: öffentliche Bankschlüssel abholen (`ebicsNoPubKeyDigestsRequest`).
pub fn hpb_request(p: &Params, keys: &Keys) -> Result<String, String> {
    let header = format!(
        "<static>{}<OrderDetails><OrderType>HPB</OrderType><OrderAttribute>DZHNN</OrderAttribute></OrderDetails><SecurityMedium>0000</SecurityMedium></static><mutable></mutable>",
        static_start(p)
    );
    signed_request("ebicsNoPubKeyDigestsRequest", &keys.authentication, &header, None)
}

/// Zeitraum eines Abrufs (`YYYY-MM-DD`, beide Grenzen inklusive).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateRange {
    pub start: String,
    pub end: String,
}

impl DateRange {
    pub fn new(start: &str, end: &str) -> Result<Self, String> {
        let is_date = |s: &str| {
            let b = s.as_bytes();
            b.len() == 10 && b[4] == b'-' && b[7] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
        };
        if !is_date(start) || !is_date(end) {
            return Err(format!("Zeitraum: Datum im Format JJJJ-MM-TT erwartet, erhalten «{start}» bis «{end}»"));
        }
        if start > end {
            return Err(format!("Zeitraum: Beginn {start} liegt nach dem Ende {end}"));
        }
        Ok(Self { start: start.into(), end: end.into() })
    }
}

/// Download, Phase 1 (**Initialisation**): Auftragsart, optionaler Zeitraum und die
/// Hashwerte der Bankschlüssel, mit denen wir arbeiten.
pub fn download_init_request(
    p: &Params,
    keys: &Keys,
    bank: &BankKeys,
    order_type: &str,
    range: Option<&DateRange>,
) -> Result<String, String> {
    let params = match range {
        Some(r) => format!(
            "<StandardOrderParams><DateRange><Start>{}</Start><End>{}</End></DateRange></StandardOrderParams>",
            esc(&r.start),
            esc(&r.end)
        ),
        None => "<StandardOrderParams></StandardOrderParams>".to_string(),
    };
    let header = format!(
        "<static>{}<OrderDetails><OrderType>{}</OrderType><OrderAttribute>DZHNN</OrderAttribute>{params}</OrderDetails>\
         <BankPubKeyDigests>\
         <Authentication Algorithm=\"{SHA256_URI}\" Version=\"X002\">{}</Authentication>\
         <Encryption Algorithm=\"{SHA256_URI}\" Version=\"E002\">{}</Encryption>\
         </BankPubKeyDigests>\
         <SecurityMedium>0000</SecurityMedium></static>\
         <mutable><TransactionPhase>Initialisation</TransactionPhase></mutable>",
        static_start(p),
        esc(order_type),
        B64.encode(bank.authentication.hash()?),
        B64.encode(bank.encryption.hash()?)
    );
    signed_request("ebicsRequest", &keys.authentication, &header, None)
}

/// Download, Phase 2 (**Transfer**): ein weiteres Segment anfordern.
pub fn download_transfer_request(
    p: &Params,
    keys: &Keys,
    transaction_id: &str,
    segment: u32,
    last: bool,
) -> Result<String, String> {
    let header = format!(
        "<static><HostID>{}</HostID><TransactionID>{}</TransactionID></static>\
         <mutable><TransactionPhase>Transfer</TransactionPhase><SegmentNumber lastSegment=\"{last}\">{segment}</SegmentNumber></mutable>",
        esc(&p.host_id),
        esc(transaction_id)
    );
    signed_request("ebicsRequest", &keys.authentication, &header, None)
}

/// Download, Phase 3 (**Receipt**): Empfang quittieren. Erst eine positive Quittung
/// markiert die Daten bei der Bank als abgeholt.
pub fn download_receipt_request(p: &Params, keys: &Keys, transaction_id: &str, ok: bool) -> Result<String, String> {
    let header = format!(
        "<static><HostID>{}</HostID><TransactionID>{}</TransactionID></static>\
         <mutable><TransactionPhase>Receipt</TransactionPhase></mutable>",
        esc(&p.host_id),
        esc(transaction_id)
    );
    let receipt = format!("<ReceiptCode>{}</ReceiptCode>", if ok { 0 } else { 1 });
    signed_request("ebicsRequest", &keys.authentication, &header, Some(&receipt))
}

// ---- Antworten -----------------------------------------------------------------------

/// Die für Abholaufträge relevanten Teile einer EBICS-Antwort.
#[derive(Debug, Clone, Default)]
pub struct Reply {
    /// Technischer Returncode (Header).
    pub technical: String,
    /// Fachlicher Returncode (Body).
    pub business: String,
    pub report_text: String,
    pub transaction_id: Option<String>,
    pub num_segments: Option<u32>,
    /// RSA-verschlüsselter AES-Schlüssel.
    pub transaction_key: Option<Vec<u8>>,
    /// Ein Segment der Auftragsdaten, noch base64-kodiert.
    pub order_data: Option<String>,
}

impl Reply {
    pub fn parse(xml: &str) -> Result<Self, String> {
        let codes = element_texts(xml, "ReturnCode");
        let (Some(technical), Some(business)) = (codes.first(), codes.last()) else {
            return Err("Antwort enthält keinen ReturnCode — keine EBICS-Antwort?".into());
        };
        let first = |name: &str| element_texts(xml, name).into_iter().next();
        let compact = |s: String| s.split_whitespace().collect::<String>();
        let transaction_key = first("TransactionKey")
            .map(|k| B64.decode(compact(k)).map_err(|e| format!("TransactionKey: {e}")))
            .transpose()?;
        Ok(Self {
            technical: technical.clone(),
            business: business.clone(),
            report_text: first("ReportText").unwrap_or_default(),
            transaction_id: first("TransactionID"),
            num_segments: first("NumSegments").and_then(|n| n.parse().ok()),
            transaction_key,
            order_data: first("OrderData").map(compact),
        })
    }

    /// Fehlertext mit beiden Returncodes und, wo bekannt, einem Hinweis zur Ursache.
    pub fn error(&self, order: &str) -> String {
        let hint = match (self.technical.as_str(), self.business.as_str()) {
            ("091002", _) | (_, "091002") => {
                " — Teilnehmer (noch) nicht freigeschaltet: Hat die Bank den unterzeichneten INI-Brief verarbeitet?"
            }
            ("061001", _) => " — Authentifikationssignatur von der Bank abgelehnt",
            ("091008", _) | (_, "091008") => " — die Bankschlüssel haben gewechselt: --ebics-hpb erneut ausführen",
            _ => "",
        };
        format!("{order}: Bank meldet {}/{} {}{hint}", self.technical, self.business, self.report_text)
    }
}

// ---- Entschlüsselung -----------------------------------------------------------------

/// Entschlüsselt Auftragsdaten nach E002: Transaktionsschlüssel per RSA (PKCS#1 v1.5),
/// Daten per AES-128-CBC mit Null-IV, Padding nach ANSI X9.23, danach zlib.
pub fn decrypt_order_data(enc_key: &RsaPrivateKey, transaction_key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    let key = enc_key
        .decrypt(Pkcs1v15Encrypt, transaction_key)
        .map_err(|e| format!("Transaktionsschlüssel entschlüsseln: {e}"))?;
    let cipher = cbc::Decryptor::<aes::Aes128>::new_from_slices(&key, &[0u8; 16])
        .map_err(|e| format!("Transaktionsschlüssel hat {} statt 16 Bytes ({e})", key.len()))?;
    let mut plain = cipher
        .decrypt_padded_vec_mut::<NoPadding>(data)
        .map_err(|e| format!("Auftragsdaten entschlüsseln: {e}"))?;
    let pad = usize::from(*plain.last().ok_or("Auftragsdaten sind leer")?);
    if pad == 0 || pad > 16 || pad > plain.len() {
        return Err(format!("Auftragsdaten: ungültiges Padding ({pad})"));
    }
    plain.truncate(plain.len() - pad);
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(&plain[..])
        .read_to_end(&mut out)
        .map_err(|e| format!("Auftragsdaten entpacken: {e}"))?;
    Ok(out)
}

// ---- Bankschlüssel (HPB) -------------------------------------------------------------

/// Ein öffentlicher RSA-Schlüssel der Bank (Modulus/Exponent base64, wie in der Antwort).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankKey {
    pub modulus: String,
    pub exponent: String,
}

impl BankKey {
    /// SHA-256-Hashwert, vergleichbar mit dem Bankparameterdaten-Blatt.
    pub fn hash(&self) -> Result<[u8; 32], String> {
        let decode = |s: &str| B64.decode(s).map_err(|e| format!("Bankschlüssel: {e}"));
        Ok(public_key_hash(&decode(&self.modulus)?, &decode(&self.exponent)?))
    }
}

/// Die per `HPB` geholten Bankschlüssel, als `bank.json` neben den eigenen abgelegt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankKeys {
    pub authentication: BankKey,
    pub encryption: BankKey,
    /// Abholzeitpunkt (UTC).
    pub fetched: String,
}

impl BankKeys {
    /// Liest die Schlüssel aus den (entschlüsselten) `HPBResponseOrderData`.
    pub fn from_order_data(xml: &str) -> Result<Self, String> {
        let compact = |v: Vec<String>| v.into_iter().map(|s| s.split_whitespace().collect::<String>()).collect::<Vec<_>>();
        let moduli = compact(element_texts(xml, "Modulus"));
        let exponents = compact(element_texts(xml, "Exponent"));
        // Schema-Reihenfolge: AuthenticationPubKeyInfo, dann EncryptionPubKeyInfo.
        let ([auth_n, enc_n], [auth_e, enc_e]) = (moduli.as_slice(), exponents.as_slice()) else {
            return Err("HPB-Antwort enthält nicht genau zwei Bankschlüssel".into());
        };
        if !xml.contains("AuthenticationPubKeyInfo") || !xml.contains("EncryptionPubKeyInfo") {
            return Err("HPB-Antwort ohne Authentifikations- oder Verschlüsselungsschlüssel".into());
        }
        Ok(Self {
            authentication: BankKey { modulus: auth_n.clone(), exponent: auth_e.clone() },
            encryption: BankKey { modulus: enc_n.clone(), exponent: enc_e.clone() },
            fetched: utc_now(),
        })
    }

    /// Vergleicht die Hashwerte mit denen vom Bankparameterdaten-Blatt (Hex, Leerzeichen
    /// und Gross-/Kleinschreibung egal). Nur bei Übereinstimmung sind die Schlüssel echt.
    pub fn verify(&self, expected_auth: &str, expected_enc: &str) -> Result<(), String> {
        for (name, key, expected) in [
            ("X002 (Authentifikation)", &self.authentication, expected_auth),
            ("E002 (Verschlüsselung)", &self.encryption, expected_enc),
        ] {
            let got = hex(&key.hash()?);
            let want: String = expected.split_whitespace().collect::<String>().to_lowercase();
            if got != want {
                return Err(format!(
                    "Bankschlüssel {name} passt nicht zum Bankparameterdaten-Blatt:\n  erhalten: {got}\n  erwartet: {want}"
                ));
            }
        }
        Ok(())
    }

    pub fn load(dir: &Path) -> Result<Option<Self>, String> {
        let path = dir.join("bank.json");
        if !path.exists() {
            return Ok(None);
        }
        let json = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&json).map(Some).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        write_private(&dir.join("bank.json"), json.as_bytes())
    }
}

/// Kleinbuchstaben-Hex ohne Trennzeichen.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Führt `HPB` aus und gibt die (noch **ungeprüften**) Bankschlüssel zurück — der
/// Aufrufer muss sie mit [`BankKeys::verify`] gegen das Bankparameterdaten-Blatt halten.
pub fn fetch_bank_keys(
    p: &Params,
    keys: &Keys,
    send: &mut dyn FnMut(&str) -> Result<String, String>,
) -> Result<BankKeys, String> {
    let reply = Reply::parse(&send(&hpb_request(p, keys)?)?)?;
    if reply.technical != EBICS_OK || reply.business != EBICS_OK {
        return Err(reply.error("HPB"));
    }
    let (Some(key), Some(data)) = (&reply.transaction_key, &reply.order_data) else {
        return Err("HPB: Antwort ohne Auftragsdaten".into());
    };
    let data = B64.decode(data).map_err(|e| format!("HPB: OrderData: {e}"))?;
    let xml = decrypt_order_data(&keys.encryption, key, &data)?;
    BankKeys::from_order_data(&String::from_utf8_lossy(&xml))
}

// ---- Download ------------------------------------------------------------------------

/// Holt die Auftragsdaten einer Abhol-Auftragsart (z. B. `Z53`) in allen drei Phasen.
///
/// `handle` bekommt die entschlüsselten, entpackten Daten. Erst wenn es `Ok` liefert,
/// wird **positiv** quittiert; schlägt die Verarbeitung fehl, geht eine negative
/// Quittung an die Bank und die Daten bleiben dort abholbar. `Ok(None)` heisst: Die Bank
/// hat für die Anfrage keine Daten.
pub fn download<T>(
    p: &Params,
    keys: &Keys,
    bank: &BankKeys,
    order_type: &str,
    range: Option<&DateRange>,
    send: &mut dyn FnMut(&str) -> Result<String, String>,
    handle: impl FnOnce(&[u8]) -> Result<T, String>,
) -> Result<Option<T>, String> {
    let init = Reply::parse(&send(&download_init_request(p, keys, bank, order_type, range)?)?)?;
    if init.technical == NO_DOWNLOAD_DATA || init.business == NO_DOWNLOAD_DATA {
        return Ok(None);
    }
    if init.technical != EBICS_OK || init.business != EBICS_OK {
        return Err(init.error(order_type));
    }
    let transaction_id = init.transaction_id.clone().ok_or_else(|| format!("{order_type}: Antwort ohne TransactionID"))?;
    let transaction_key = init.transaction_key.clone().ok_or_else(|| format!("{order_type}: Antwort ohne TransactionKey"))?;
    let segments = init.num_segments.unwrap_or(1);

    // Segmentiert wird der base64-Text, also erst zusammensetzen, dann dekodieren.
    let mut encoded = init.order_data.clone().ok_or_else(|| format!("{order_type}: Antwort ohne OrderData"))?;
    for segment in 2..=segments {
        let request = download_transfer_request(p, keys, &transaction_id, segment, segment == segments)?;
        let reply = Reply::parse(&send(&request)?)?;
        if reply.technical != EBICS_OK || reply.business != EBICS_OK {
            return Err(reply.error(&format!("{order_type} Segment {segment}/{segments}")));
        }
        encoded.push_str(&reply.order_data.ok_or_else(|| format!("{order_type}: Segment {segment} ohne OrderData"))?);
    }

    let result = B64
        .decode(&encoded)
        .map_err(|e| format!("{order_type}: OrderData: {e}"))
        .and_then(|data| decrypt_order_data(&keys.encryption, &transaction_key, &data))
        .and_then(|data| handle(&data));

    let receipt = Reply::parse(&send(&download_receipt_request(p, keys, &transaction_id, result.is_ok())?)?)?;
    let value = result?;
    if receipt.technical != DOWNLOAD_POSTPROCESS_DONE && receipt.technical != EBICS_OK {
        return Err(receipt.error(&format!("{order_type} Quittung")));
    }
    Ok(Some(value))
}

/// Ergebnis des Entpackens eines Auszugs-ZIPs.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Extracted {
    /// Neu geschriebene Dateien.
    pub written: Vec<String>,
    /// Bereits vorhandene Dateien (gleicher Name) — unverändert gelassen.
    pub skipped: Vec<String>,
}

/// Entpackt das ZIP eines `Z53`-Abrufs nach `dir`. Vorhandene Dateien werden nicht
/// überschrieben; Verzeichnisanteile in den ZIP-Namen werden verworfen.
pub fn extract_statements(zip_bytes: &[u8], dir: &Path) -> Result<Extracted, String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).map_err(|e| format!("ZIP nicht lesbar: {e}"))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out = Extracted::default();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| format!("ZIP-Eintrag {i}: {e}"))?;
        if file.is_dir() {
            continue;
        }
        let Some(name) = Path::new(file.name()).file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        let target = dir.join(&name);
        if target.exists() {
            out.skipped.push(name);
            continue;
        }
        let mut content = Vec::new();
        file.read_to_end(&mut content).map_err(|e| format!("{name}: {e}"))?;
        std::fs::write(&target, content).map_err(|e| format!("{}: {e}", target.display()))?;
        out.written.push(name);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_range_is_checked() {
        assert!(DateRange::new("2026-01-01", "2026-06-30").is_ok());
        assert!(DateRange::new("01.01.2026", "2026-06-30").is_err());
        assert!(DateRange::new("2026-07-01", "2026-06-30").is_err());
    }

    #[test]
    fn canonical_form_repeats_root_namespaces() {
        assert_eq!(
            canonical_authenticated("<static></static>", Some("<ReceiptCode>0</ReceiptCode>")),
            "<header xmlns=\"urn:org:ebics:H004\" xmlns:ds=\"http://www.w3.org/2000/09/xmldsig#\" authenticate=\"true\"><static></static></header>\
             <TransferReceipt xmlns=\"urn:org:ebics:H004\" xmlns:ds=\"http://www.w3.org/2000/09/xmldsig#\" authenticate=\"true\"><ReceiptCode>0</ReceiptCode></TransferReceipt>"
        );
    }

    #[test]
    fn reply_reads_both_return_codes_and_payload() {
        let xml = r#"<ebicsResponse xmlns="urn:org:ebics:H004"><header authenticate="true"><static><TransactionID>ABCDEF</TransactionID><NumSegments>2</NumSegments></static><mutable><TransactionPhase>Initialisation</TransactionPhase><SegmentNumber lastSegment="false">1</SegmentNumber><ReturnCode>000000</ReturnCode><ReportText>[EBICS_OK] OK</ReportText></mutable></header><body><DataTransfer><DataEncryptionInfo authenticate="true"><TransactionKey>AAEC
Aw==</TransactionKey></DataEncryptionInfo><OrderData>QUJD
REVG</OrderData></DataTransfer><ReturnCode authenticate="true">000000</ReturnCode></body></ebicsResponse>"#;
        let r = Reply::parse(xml).unwrap();
        assert_eq!((r.technical.as_str(), r.business.as_str()), ("000000", "000000"));
        assert_eq!(r.transaction_id.as_deref(), Some("ABCDEF"));
        assert_eq!(r.num_segments, Some(2));
        assert_eq!(r.transaction_key.as_deref(), Some(&[0u8, 1, 2, 3][..]));
        assert_eq!(r.order_data.as_deref(), Some("QUJDREVG"));

        let denied = xml.replace(r#"<ReturnCode authenticate="true">000000"#, r#"<ReturnCode authenticate="true">091002"#);
        assert!(Reply::parse(&denied).unwrap().error("HPB").contains("freigeschaltet"));
    }

    #[test]
    fn bank_key_hashes_are_compared_loosely_formatted() {
        let xml = r#"<HPBResponseOrderData xmlns="urn:org:ebics:H004" xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><AuthenticationPubKeyInfo><PubKeyValue><ds:RSAKeyValue><ds:Modulus>Crw=</ds:Modulus><ds:Exponent>AQAB</ds:Exponent></ds:RSAKeyValue></PubKeyValue><AuthenticationVersion>X002</AuthenticationVersion></AuthenticationPubKeyInfo><EncryptionPubKeyInfo><PubKeyValue><ds:RSAKeyValue><ds:Modulus>Cr0=</ds:Modulus><ds:Exponent>AQAB</ds:Exponent></ds:RSAKeyValue></PubKeyValue><EncryptionVersion>E002</EncryptionVersion></EncryptionPubKeyInfo><HostID>EBXTEST</HostID></HPBResponseOrderData>"#;
        let bank = BankKeys::from_order_data(xml).unwrap();
        let auth = hex(&Sha256::digest(b"10001 abc"));
        let enc = hex(&Sha256::digest(b"10001 abd"));
        let spaced = auth.to_uppercase().as_bytes().chunks(2).map(|c| String::from_utf8_lossy(c).into_owned()).collect::<Vec<_>>().join(" ");
        assert!(bank.verify(&spaced, &enc).is_ok());
        assert!(bank.verify(&enc, &auth).unwrap_err().contains("X002"));
    }
}
