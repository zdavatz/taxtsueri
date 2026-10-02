//! **EBICS**-Client (Electronic Banking Internet Communication Standard), Protokoll
//! **H004** (EBICS 2.5) — um Kontoauszüge (camt.053, Auftragsart `Z53`) direkt bei der
//! Bank abzuholen statt sie von Hand aus dem E-Banking zu exportieren.
//!
//! Dieser Teil deckt die **Initialisierung** des Teilnehmers ab:
//!
//! 1. drei RSA-Schlüsselpaare erzeugen — `A006` (elektronische Unterschrift), `X002`
//!    (Authentifikation), `E002` (Verschlüsselung);
//! 2. `INI` übermittelt den öffentlichen Unterschriftsschlüssel, `HIA` die beiden
//!    anderen — beides als `ebicsUnsecuredRequest` (unsigniert, unverschlüsselt; die
//!    Auftragsdaten sind zlib-komprimiert und base64-kodiert);
//! 3. der **INI-Brief** ([`crate::ebics_brief`]) trägt die SHA-256-Hashwerte der drei
//!    Schlüssel auf Papier zur Bank, die damit den Zugang freischaltet.
//!
//! Die privaten Schlüssel liegen **ausserhalb des Repos** (Default
//! `~/.config/taxtsueri/ebics/<Host>-<Kunde>-<Teilnehmer>/`, Modus 0600) und werden nie
//! überschrieben: nach einem angenommenen `INI` lässt sich der Schlüssel nur noch über
//! die Bank zurücksetzen. Host-, Kunden- und Teilnehmer-ID kommen aus `settings.json`.

use crate::settings::EbicsSettings;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, LineEnding};
use rsa::traits::PublicKeyParts;
use rsa::RsaPrivateKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Schlüssellänge in Bit (EBICS verlangt für A006/X002/E002 mindestens 2048).
pub const KEY_BITS: usize = 2048;
/// EBICS-Returncode «alles in Ordnung».
pub const EBICS_OK: &str = "000000";

pub(crate) const NS_H004: &str = "urn:org:ebics:H004";
const NS_S001: &str = "http://www.ebics.org/S001";
pub(crate) const NS_DS: &str = "http://www.w3.org/2000/09/xmldsig#";

/// Die Bankparameter eines Teilnehmers (aus dem Bankparameterdaten-Blatt).
#[derive(Debug, Clone)]
pub struct Params {
    pub host_id: String,
    pub url: String,
    /// Kunden-ID.
    pub partner_id: String,
    /// Teilnehmer-ID.
    pub user_id: String,
    /// Teilnehmername, nur für den INI-Brief.
    pub user_name: String,
}

impl Params {
    /// Liest die Parameter aus `settings.json`; fehlende Pflichtangaben werden benannt.
    pub fn from_settings(s: &EbicsSettings) -> Result<Self, String> {
        let need = |v: &Option<String>, key: &str| {
            v.clone()
                .filter(|x| !x.trim().is_empty())
                .ok_or_else(|| format!("settings.json: ebics.{key} fehlt (siehe Bankparameterdaten-Blatt)"))
        };
        Ok(Self {
            host_id: need(&s.host_id, "hostId")?,
            url: need(&s.url, "url")?,
            partner_id: need(&s.partner_id, "partnerId")?,
            user_id: need(&s.user_id, "userId")?,
            user_name: s.user_name.clone().unwrap_or_default(),
        })
    }
}

/// Die drei Schlüsselarten eines EBICS-Teilnehmers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    /// Elektronische Unterschrift (bankfachlich).
    Signature,
    /// Authentifikation (Signatur der EBICS-Nachrichten).
    Authentication,
    /// Verschlüsselung.
    Encryption,
}

impl KeyKind {
    /// Versionskennung des Verfahrens, wie sie in INI/HIA und im Brief steht.
    pub fn version(self) -> &'static str {
        match self {
            KeyKind::Signature => "A006",
            KeyKind::Authentication => "X002",
            KeyKind::Encryption => "E002",
        }
    }

    fn file_name(self) -> &'static str {
        match self {
            KeyKind::Signature => "a006.pem",
            KeyKind::Authentication => "x002.pem",
            KeyKind::Encryption => "e002.pem",
        }
    }
}

/// Fortschritt der Initialisierung, neben den Schlüsseln als `state.json` abgelegt.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Erzeugungszeitpunkt der Schlüssel (UTC, ISO 8601) — steht auch im INI-Brief.
    pub created: String,
    /// Zeitpunkt, zu dem die Bank `INI` bzw. `HIA` angenommen hat.
    pub ini_sent: Option<String>,
    pub hia_sent: Option<String>,
}

/// Die Teilnehmerschlüssel samt Ablageort und Initialisierungs-Fortschritt.
pub struct Keys {
    pub signature: RsaPrivateKey,
    pub authentication: RsaPrivateKey,
    pub encryption: RsaPrivateKey,
    pub state: State,
    pub dir: PathBuf,
}

impl Keys {
    pub fn get(&self, kind: KeyKind) -> &RsaPrivateKey {
        match kind {
            KeyKind::Signature => &self.signature,
            KeyKind::Authentication => &self.authentication,
            KeyKind::Encryption => &self.encryption,
        }
    }

    /// Lädt die Schlüssel aus `dir` oder erzeugt sie, wenn dort noch **keine** liegen.
    /// Ein unvollständiger Satz ist ein Fehler — es wird nie etwas überschrieben.
    pub fn load_or_generate(dir: &Path) -> Result<(Self, bool), String> {
        let kinds = [KeyKind::Signature, KeyKind::Authentication, KeyKind::Encryption];
        let present = kinds.iter().filter(|k| dir.join(k.file_name()).exists()).count();
        match present {
            3 => Ok((Self::load(dir)?, false)),
            0 => Ok((Self::generate(dir)?, true)),
            n => Err(format!(
                "{}: nur {n} von 3 Schlüsseldateien vorhanden — bitte von Hand klären, es wird nichts überschrieben",
                dir.display()
            )),
        }
    }

    fn load(dir: &Path) -> Result<Self, String> {
        let read = |kind: KeyKind| -> Result<RsaPrivateKey, String> {
            let path = dir.join(kind.file_name());
            let pem = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            RsaPrivateKey::from_pkcs8_pem(&pem).map_err(|e| format!("{}: {e}", path.display()))
        };
        let state_path = dir.join("state.json");
        let state = std::fs::read_to_string(&state_path)
            .map_err(|e| format!("{}: {e}", state_path.display()))
            .and_then(|s| serde_json::from_str(&s).map_err(|e| format!("{}: {e}", state_path.display())))?;
        Ok(Self {
            signature: read(KeyKind::Signature)?,
            authentication: read(KeyKind::Authentication)?,
            encryption: read(KeyKind::Encryption)?,
            state,
            dir: dir.to_path_buf(),
        })
    }

    fn generate(dir: &Path) -> Result<Self, String> {
        create_private_dir(dir)?;
        let mut rng = rand::rngs::OsRng;
        let mut new_key = || RsaPrivateKey::new(&mut rng, KEY_BITS).map_err(|e| format!("RSA-Schlüssel erzeugen: {e}"));
        let keys = Self {
            signature: new_key()?,
            authentication: new_key()?,
            encryption: new_key()?,
            state: State { created: utc_now(), ..State::default() },
            dir: dir.to_path_buf(),
        };
        for kind in [KeyKind::Signature, KeyKind::Authentication, KeyKind::Encryption] {
            let pem = keys
                .get(kind)
                .to_pkcs8_pem(LineEnding::LF)
                .map_err(|e| format!("Schlüssel kodieren: {e}"))?;
            write_private(&dir.join(kind.file_name()), pem.as_bytes())?;
        }
        keys.save_state()?;
        Ok(keys)
    }

    pub fn save_state(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.state).map_err(|e| e.to_string())?;
        write_private(&self.dir.join("state.json"), json.as_bytes())
    }
}

/// Schlüsselverzeichnis eines Teilnehmers: `<basis>/<Host>-<Kunde>-<Teilnehmer>`.
/// Ohne `keyDir` in den Einstellungen gilt `~/.config/taxtsueri/ebics`.
pub fn key_dir(base: Option<&str>, p: &Params) -> Result<PathBuf, String> {
    let base = match base {
        Some(b) => PathBuf::from(b),
        None => {
            let home = std::env::var("HOME").map_err(|_| "HOME ist nicht gesetzt — ebics.keyDir in settings.json angeben")?;
            PathBuf::from(home).join(".config/taxtsueri/ebics")
        }
    };
    Ok(base.join(format!("{}-{}-{}", p.host_id, p.partner_id, p.user_id)))
}

fn create_private_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    Ok(())
}

/// Schreibt eine Datei, die nur der Besitzer lesen darf (0600).
pub fn write_private(path: &Path, data: &[u8]) -> Result<(), String> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path)
        .and_then(|mut f| f.write_all(data))
        .map_err(|e| format!("{}: {e}", path.display()))
}

// ---- Öffentlicher Schlüssel: Darstellung + Hash -------------------------------------

/// Modulus und Exponent als Big-Endian-Bytes ohne führende Nullen.
pub fn public_parts(key: &RsaPrivateKey) -> (Vec<u8>, Vec<u8>) {
    (key.n().to_bytes_be(), key.e().to_bytes_be())
}

/// Hexdarstellung ohne führende Nullen (Kleinbuchstaben), wie sie in den Hash eingeht.
fn hex_no_leading_zero(bytes: &[u8]) -> String {
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let trimmed = hex.trim_start_matches('0');
    if trimmed.is_empty() { "0".into() } else { trimmed.into() }
}

/// SHA-256-Hashwert eines öffentlichen Schlüssels nach EBICS (Kap. 5.5.1.1.1):
/// Exponent, **ein Leerzeichen**, Modulus — beide hexadezimal in Kleinbuchstaben ohne
/// führende Nullen, als US-ASCII gehasht. Derselbe Wert steht im INI-Brief und, für die
/// Bankschlüssel, auf dem Bankparameterdaten-Blatt.
pub fn public_key_hash(modulus: &[u8], exponent: &[u8]) -> [u8; 32] {
    let text = format!("{} {}", hex_no_leading_zero(exponent), hex_no_leading_zero(modulus));
    Sha256::digest(text.as_bytes()).into()
}

// ---- Auftragsdaten und Requests ------------------------------------------------------

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// `<PubKeyValue>` mit `ds:RSAKeyValue` und Erzeugungszeitpunkt.
fn pub_key_value(key: &RsaPrivateKey, created: &str) -> String {
    let (n, e) = public_parts(key);
    format!(
        "<PubKeyValue><ds:RSAKeyValue><ds:Modulus>{}</ds:Modulus><ds:Exponent>{}</ds:Exponent></ds:RSAKeyValue><TimeStamp>{}</TimeStamp></PubKeyValue>",
        B64.encode(n),
        B64.encode(e),
        esc(created)
    )
}

/// Auftragsdaten des `INI`: `SignaturePubKeyOrderData` (Namespace S001).
pub fn ini_order_data(p: &Params, keys: &Keys) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<SignaturePubKeyOrderData xmlns=\"{NS_S001}\" xmlns:ds=\"{NS_DS}\"><SignaturePubKeyInfo>{}<SignatureVersion>{}</SignatureVersion></SignaturePubKeyInfo><PartnerID>{}</PartnerID><UserID>{}</UserID></SignaturePubKeyOrderData>",
        pub_key_value(&keys.signature, &keys.state.created),
        KeyKind::Signature.version(),
        esc(&p.partner_id),
        esc(&p.user_id)
    )
}

/// Auftragsdaten des `HIA`: `HIARequestOrderData` (Namespace H004).
pub fn hia_order_data(p: &Params, keys: &Keys) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<HIARequestOrderData xmlns=\"{NS_H004}\" xmlns:ds=\"{NS_DS}\"><AuthenticationPubKeyInfo>{}<AuthenticationVersion>{}</AuthenticationVersion></AuthenticationPubKeyInfo><EncryptionPubKeyInfo>{}<EncryptionVersion>{}</EncryptionVersion></EncryptionPubKeyInfo><PartnerID>{}</PartnerID><UserID>{}</UserID></HIARequestOrderData>",
        pub_key_value(&keys.authentication, &keys.state.created),
        KeyKind::Authentication.version(),
        pub_key_value(&keys.encryption, &keys.state.created),
        KeyKind::Encryption.version(),
        esc(&p.partner_id),
        esc(&p.user_id)
    )
}

/// Auftragsdaten für den Transport: zlib-komprimiert, dann base64.
fn pack_order_data(xml: &str) -> String {
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(xml.as_bytes()).expect("in den Speicher schreiben");
    B64.encode(enc.finish().expect("zlib abschliessen"))
}

/// `ebicsUnsecuredRequest` für `INI`/`HIA` — die einzigen Aufträge ohne
/// Authentifikationssignatur (die Bank kennt die Schlüssel ja noch nicht).
pub fn unsecured_request(p: &Params, order_type: &str, order_data_xml: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ebicsUnsecuredRequest xmlns="{NS_H004}" Version="H004" Revision="1">
  <header authenticate="true">
    <static>
      <HostID>{}</HostID>
      <PartnerID>{}</PartnerID>
      <UserID>{}</UserID>
      <Product Language="de">taxtsueri</Product>
      <OrderDetails>
        <OrderType>{}</OrderType>
        <OrderAttribute>DZNNN</OrderAttribute>
      </OrderDetails>
      <SecurityMedium>0000</SecurityMedium>
    </static>
    <mutable/>
  </header>
  <body>
    <DataTransfer>
      <OrderData>{}</OrderData>
    </DataTransfer>
  </body>
</ebicsUnsecuredRequest>
"#,
        esc(&p.host_id),
        esc(&p.partner_id),
        esc(&p.user_id),
        esc(order_type),
        pack_order_data(order_data_xml)
    )
}

pub fn ini_request(p: &Params, keys: &Keys) -> String {
    unsecured_request(p, "INI", &ini_order_data(p, keys))
}

pub fn hia_request(p: &Params, keys: &Keys) -> String {
    unsecured_request(p, "HIA", &hia_order_data(p, keys))
}

// ---- Antworten -----------------------------------------------------------------------

/// Textinhalte aller Elemente mit lokalem Namen `local` (Präfix und Attribute egal).
pub(crate) fn element_texts(xml: &str, local: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(lt) = rest.find('<') {
        rest = &rest[lt + 1..];
        let Some(gt) = rest.find('>') else { break };
        let tag = &rest[..gt];
        let name = tag.split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("");
        if name.rsplit(':').next() == Some(local) && !tag.ends_with('/') {
            let body = &rest[gt + 1..];
            if let Some(end) = body.find('<') {
                out.push(body[..end].trim().to_string());
            }
        }
        rest = &rest[gt + 1..];
    }
    out
}

/// Die Returncodes einer EBICS-Antwort: technisch (Header) und fachlich (Body).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub return_codes: Vec<String>,
    pub report_text: String,
}

impl Response {
    pub fn parse(xml: &str) -> Result<Self, String> {
        let return_codes = element_texts(xml, "ReturnCode");
        if return_codes.is_empty() {
            return Err("Antwort enthält keinen ReturnCode — keine EBICS-Antwort?".into());
        }
        let report_text = element_texts(xml, "ReportText").into_iter().next().unwrap_or_default();
        Ok(Self { return_codes, report_text })
    }

    /// `true`, wenn technischer **und** fachlicher Returncode `000000` sind.
    pub fn is_ok(&self) -> bool {
        self.return_codes.iter().all(|c| c == EBICS_OK)
    }
}

/// Schickt einen EBICS-Request per HTTPS-POST an die Bank und gibt die Antwort zurück.
pub fn post(url: &str, request_xml: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .user_agent(concat!("taxtsueri/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(url)
        .header("Content-Type", "text/xml; charset=UTF-8")
        .body(request_xml.to_string())
        .send()
        .map_err(|e| format!("{url}: {e}"))?;
    let status = resp.status();
    let body = resp.text().map_err(|e| format!("{url}: {e}"))?;
    if !status.is_success() {
        return Err(format!("{url}: HTTP {status}\n{body}"));
    }
    Ok(body)
}

// ---- Zeit ----------------------------------------------------------------------------

/// Aktuelle Zeit in UTC als ISO 8601 (`2026-01-31T12:34:56Z`), ohne Zusatz-Crate.
pub fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_utc(secs)
}

fn format_utc(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Tage seit 1970-01-01 → bürgerliches Datum (Algorithmus von Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_formatting() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(format_utc(1_790_930_096), "2026-10-02T08:34:56Z");
    }

    #[test]
    fn hash_input_has_no_leading_zeros() {
        assert_eq!(hex_no_leading_zero(&[0x01, 0x00, 0x01]), "10001");
        assert_eq!(hex_no_leading_zero(&[0x00, 0x0a, 0xbc]), "abc");
        // sha256("10001 abc")
        let expected = Sha256::digest(b"10001 abc");
        assert_eq!(public_key_hash(&[0x0a, 0xbc], &[0x01, 0x00, 0x01])[..], expected[..]);
    }

    #[test]
    fn response_return_codes() {
        let ok = r#"<ebicsKeyManagementResponse xmlns="urn:org:ebics:H004"><header authenticate="true"><static/><mutable><ReturnCode>000000</ReturnCode><ReportText>[EBICS_OK] OK</ReportText></mutable></header><body><ReturnCode authenticate="true">000000</ReturnCode></body></ebicsKeyManagementResponse>"#;
        let r = Response::parse(ok).unwrap();
        assert_eq!(r.return_codes, ["000000", "000000"]);
        assert_eq!(r.report_text, "[EBICS_OK] OK");
        assert!(r.is_ok());

        let bad = ok.replace(r#"<ReturnCode authenticate="true">000000"#, r#"<ReturnCode authenticate="true">091002"#);
        assert!(!Response::parse(&bad).unwrap().is_ok());
        assert!(Response::parse("<html/>").is_err());
    }
}
