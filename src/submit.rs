//! Einreichungs-Paket für den Kanton / die Stadt Zürich.
//!
//! Realität des Einreichungskanals (Stand 2025, Kanton ZH):
//! - Die Online-Steuererklärung **ZHprivateTax** (zhp.services.zh.ch) löst die
//!   Download-Software ab. Eingereicht wird interaktiv über das Portal
//!   (AHV-Nr. + Zugangscode + starke Authentifizierung) – es gibt **keine**
//!   offene Upload-API für eCH-0119-XML.
//! - Bei Erstellung mit (PC-)Software muss zwingend das **2D-Barcode-Blatt**
//!   eingereicht werden; die Daten stecken dort als PDF417-Barcode.
//!
//! Dieses Werkzeug kann den Kanal nicht automatisch bedienen. Es erzeugt ein
//! **Einreichungs-Paket**: das validierte eCH-0119-XML plus ein Manifest mit
//! Prüfsumme und den konkreten Kanal-Hinweisen – bereit für ZHprivateTax bzw.
//! die Software-/Barcode-Einreichung.

use crate::model::Message;
use std::fs;
use std::path::{Path, PathBuf};

/// Schreibt das Einreichungs-Paket nach `data/submission/` und gibt den Pfad zurück.
pub fn write_package(xml: &str, message: &Message) -> Result<PathBuf, String> {
    let dir = Path::new("data").join("submission");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let xml_path = dir.join("eCH-0119.xml");
    fs::write(&xml_path, xml).map_err(|e| e.to_string())?;

    let h = &message.header;
    let p = &message.content.main_form.person_data_partner1.identification;
    let muni = message
        .content
        .main_form
        .person_data_partner1
        .tax_municipality
        .as_ref();
    let manifest = format!(
        "taxtsueri – Einreichungs-Paket\n\
         ================================\n\
         Steuerperiode : {}\n\
         Kanton        : {}\n\
         Gemeinde      : {}\n\
         Pflichtige/r  : {} {} (AHVN13 {})\n\
         eCH-0119-XML  : eCH-0119.xml ({} Bytes)\n\
         SHA-256       : {}\n\
         \n\
         Einreichung (Kanton/Stadt Zürich, Stand 2025):\n\
         1. ZHprivateTax (Online): https://zhp.services.zh.ch/app/ZHprivateTax/\n\
            Anmeldung mit AHV-Nr. + Zugangscode + starker Authentifizierung;\n\
            Belege fotografieren/hochladen, ohne Unterschrift einreichbar.\n\
         2. Mit Steuersoftware: 2D-Barcode-Blatt drucken und einreichen\n\
            (das Barcode-Blatt muss immer beigelegt werden).\n\
         Eine offene Upload-API fuer eCH-0119-XML existiert nicht.\n",
        h.tax_period,
        h.canton.as_deref().unwrap_or("-"),
        muni.map(|m| m.municipality_name.as_str()).unwrap_or("-"),
        p.first_name,
        p.official_name,
        p.vn,
        xml.len(),
        sha256_hex(xml.as_bytes()),
    );
    let manifest_path = dir.join("MANIFEST.txt");
    fs::write(&manifest_path, manifest).map_err(|e| e.to_string())?;

    Ok(dir)
}

/// Minimaler, abhängigkeitsfreier SHA-256 (FIPS 180-4) für die Manifest-Prüfsumme.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bitlen = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bitlen.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (hi, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *hi = hi.wrapping_add(v);
        }
    }

    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::sha256_hex;

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
