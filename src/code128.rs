//! CODE128C-Seitenbarcode (1D) für das eCH-0196-Barcode-Blatt.
//!
//! 16-stellig numerisch (Kap. 2.4 der Wegleitung). Die Symbologie (Start C,
//! Prüfziffer mod 103, Stop) liefert die `barcoders`-Crate; hier kommen der
//! eCH-Codeaufbau und das Rendering dazu.

/// Baut den 16-stelligen eCH-0196-Seitencode (E-Steuerauszug-Variante):
/// Formular-Nr(3) · Version(2) · Organisation(5) · Seite(3) · 2D-Blatt(1)
/// · Orientierung(1) · Leserichtung(1).
pub fn build_page_code(
    form: u16,
    version: u8,
    organisation: u32,
    page: u16,
    has_2d: bool,
    orientation: u8,
    reading: u8,
) -> String {
    format!(
        "{form:03}{version:02}{organisation:05}{page:03}{}{orientation}{reading}",
        if has_2d { 1 } else { 0 }
    )
}

/// Kodiert numerische Ziffern als CODE128C-Modulmuster (true = Balken).
pub fn encode(digits: &str) -> Result<Vec<bool>, String> {
    use barcoders::sym::code128::Code128;
    // "Ć" (\u{0106}) = Start Code Set C.
    let code = Code128::new(format!("\u{0106}{digits}"))
        .map_err(|e| format!("CODE128: {e:?}"))?;
    Ok(code.encode().into_iter().map(|b| b == 1).collect())
}

/// 1D-Bitmuster → Graustufenpuffer (schwarz=0) mit Ruhezone, `module_px` breit,
/// `height_px` hoch. Nur für den rxing-Round-Trip-Test.
#[cfg(test)]
pub fn to_luma(bits: &[bool], module_px: usize, height_px: usize) -> (Vec<u8>, usize, usize) {
    let qz = 10; // Ruhezone in Modulen (CODE128: ≥10)
    let w = (bits.len() + 2 * qz) * module_px;
    let h = height_px;
    let mut buf = vec![255u8; w * h];
    for (i, &b) in bits.iter().enumerate() {
        if !b {
            continue;
        }
        let x0 = (i + qz) * module_px;
        for y in 0..h {
            for dx in 0..module_px {
                buf[y * w + x0 + dx] = 0;
            }
        }
    }
    (buf, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_code_is_16_digits() {
        let c = build_page_code(196, 22, 250, 1, true, 0, 1);
        assert_eq!(c.len(), 16);
        assert_eq!(c, "1962200250001101");
        assert!(c.chars().all(|ch| ch.is_ascii_digit()));
    }

    // Round-Trip: unser CODE128C wird mit rxing dekodiert.
    #[test]
    fn roundtrip_decodes_with_rxing() {
        let digits = "1962200250001101";
        let bits = encode(digits).expect("encode");
        let (luma, w, h) = to_luma(&bits, 3, 60);
        let res = rxing::helpers::detect_in_luma(luma, w as u32, h as u32, Some(rxing::BarcodeFormat::CODE_128))
            .expect("rxing decode");
        assert_eq!(res.getText(), digits);
    }
}
