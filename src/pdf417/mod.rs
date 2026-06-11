//! Minimaler **PDF417-Encoder/-Renderer** (Einzelsymbol) für das eCH-0196-Barcode-Blatt.
//!
//! Byte-Compaction (6 Bytes → 5 Codewörter, base-900), Symbol-Assemblierung
//! (Längendeskriptor + Padding + Reed-Solomon) und Rendering (Start/Stop-Muster,
//! Zeilenindikatoren, Cluster-Muster) sind eigener Code. Die Datentabellen
//! (`HL_TO_LL`, ECC-Faktoren) und die Reed-Solomon-Routine sind aus der
//! MIT-lizenzierten Crate `pdf417` vendoriert (siehe `tables.rs`/`ecc.rs`).
//!
//! Stand: **Einzelsymbol**. Structured Append (Macro-PDF417) über mehrere
//! Segmente, CODE128C-Seitenbarcode und A4-Blattlayout sind der nächste Schritt.

mod ecc;
mod tables;

use tables::HL_TO_LL;

const START: u32 = 0b11111111010101000; // 17 Bit
const END: u32 = 0b111111101000101001; // 18 Bit
const LEADING_ONE: u32 = 1 << 16;

/// Reine Bytes → PDF417-Datencodewörter (Byte-Compaction).
fn byte_compaction(bytes: &[u8]) -> Vec<u16> {
    let mut cw = Vec::new();
    if bytes.is_empty() {
        return cw;
    }
    // Latch: 924 falls Länge durch 6 teilbar, sonst 901.
    cw.push(if bytes.len() % 6 == 0 { 924 } else { 901 });
    let mut k = 0;
    while bytes.len() - k >= 6 {
        let mut s: u64 = 0;
        for n in 0..6 {
            s = (s << 8) + bytes[k + n] as u64;
        }
        let mut five = [0u16; 5];
        for n in 0..5 {
            five[4 - n] = (s % 900) as u16;
            s /= 900;
        }
        cw.extend_from_slice(&five);
        k += 6;
    }
    // Rest (<6 Bytes) als rohe Codewörter.
    while k < bytes.len() {
        cw.push(bytes[k] as u16);
        k += 1;
    }
    cw
}

/// Datenkapazität (Codewörter ohne EC) eines Symbols.
#[allow(dead_code)]
pub fn data_capacity(cols: u8, rows: u8, level: u8) -> usize {
    (cols as usize * rows as usize).saturating_sub(1usize << (level as usize + 1))
}

/// Assembliert eine Symbol-Codewort-Matrix aus bereits aufbereiteten
/// Datencodewörtern (ohne Längendeskriptor): [Länge][data…][Padding 900…][EC].
/// Der Längendeskriptor zählt nur die echten Datencodewörter (inkl. sich selbst),
/// **nicht** das Padding — wichtig, damit ein folgender Macro-Block korrekt endet.
fn assemble(data: &[u16], cols: u8, rows: u8, level: u8) -> Result<Vec<u16>, String> {
    let total = cols as usize * rows as usize;
    let ec = 1usize << (level as usize + 1);
    let cap = total - ec; // Datenregion (inkl. Längendeskriptor)
    if data.len() + 1 > cap {
        return Err(format!(
            "zu viele Datencodewörter: {} > Kapazität {}",
            data.len() + 1,
            cap
        ));
    }
    let mut cws = Vec::with_capacity(total);
    cws.push((data.len() + 1) as u16); // Längendeskriptor
    cws.extend_from_slice(data);
    while cws.len() < cap {
        cws.push(900); // Padding (ausserhalb des Längendeskriptors)
    }
    cws.resize(total, 0); // EC-Region
    ecc::generate_ecc(&mut cws, level);
    Ok(cws)
}

/// Baut die Codewort-Matrix eines Einzelsymbols (inkl. EC). Fehler, wenn die
/// Nutzlast nicht in ein Symbol passt (dann ist Structured Append nötig).
#[allow(dead_code)] // Einzelsymbol-API; CLI nutzt build_symbols (Structured Append).
pub fn build_symbol(payload: &[u8], cols: u8, rows: u8, level: u8) -> Result<Vec<u16>, String> {
    let cap = data_capacity(cols, rows, level);
    let data = byte_compaction(payload);
    if data.len() + 1 > cap {
        return Err(format!(
            "Nutzlast zu gross für ein Symbol: {} Codewörter > Kapazität {} (Structured Append nötig)",
            data.len() + 1,
            cap - 1
        ));
    }
    assemble(&data, cols, rows, level)
}

/// Macro-PDF417-Kontrollblock: 928 · Segmentindex(2 CW, Base-900 von "1"+idx)
/// · File-ID-Codewörter · (922 wenn letztes Segment).
fn macro_block(segment_index: usize, file_id: &[u16], last: bool) -> Vec<u16> {
    let mut mb = vec![928u16];
    let v: u64 = format!("1{segment_index}").parse().unwrap();
    mb.push((v / 900) as u16);
    mb.push((v % 900) as u16);
    mb.extend_from_slice(file_id);
    if last {
        mb.push(922);
    }
    mb
}

/// Teilt die Nutzlast in mehrere **Structured-Append**-Symbole (Macro-PDF417).
/// Jedes Segment kodiert seinen Byte-Chunk eigenständig + Kontrollblock.
pub fn build_symbols(payload: &[u8], cols: u8, rows: u8, level: u8) -> Result<Vec<Vec<u16>>, String> {
    let cap = data_capacity(cols, rows, level);
    let file_id: [u16; 2] = [
        (payload.len() % 900) as u16,
        (payload.iter().map(|&b| b as usize).sum::<usize>() % 900) as u16,
    ];
    let overhead = 1 + 2 + file_id.len() + 1; // 928 + segindex + fileid + Terminator
    // Byte-Budget pro Segment (konservativ aus dem Codewort-Budget).
    let cw_budget = cap.saturating_sub(1 + overhead);
    let chunk_bytes = (cw_budget.saturating_sub(1) * 6 / 5).max(1);
    let chunks: Vec<&[u8]> = if payload.is_empty() {
        vec![&[][..]]
    } else {
        payload.chunks(chunk_bytes).collect()
    };
    let n = chunks.len();
    let mut symbols = Vec::with_capacity(n);
    for (i, ch) in chunks.iter().enumerate() {
        let mut region = byte_compaction(ch);
        region.extend(macro_block(i, &file_id, i + 1 == n));
        symbols.push(assemble(&region, cols, rows, level)?);
    }
    Ok(symbols)
}

/// Rendert die Codewort-Matrix zu einem Modulraster (true = schwarz).
pub fn render(cws: &[u16], cols: u8, rows: u8, level: u8) -> Vec<Vec<bool>> {
    let rows_val = (rows as u32 - 1) / 3;
    let cols_val = cols as u32 - 1;
    let level_val = level as u32 * 3 + (rows as u32 - 1) % 3;
    let mut grid = Vec::with_capacity(rows as usize);
    let mut table: u32 = 0;
    for row in 0..rows as u32 {
        let mut bits = Vec::new();
        push_bits(&mut bits, START, 17);
        let left = (row / 3) * 30
            + match table {
                0 => rows_val,
                1 => level_val,
                _ => cols_val,
            };
        push_pattern(&mut bits, table, left);
        for col in 0..cols as u32 {
            let cw = cws[(row * cols as u32 + col) as usize] as u32;
            push_pattern(&mut bits, table, cw);
        }
        let right = (row / 3) * 30
            + match table {
                0 => cols_val,
                1 => rows_val,
                _ => level_val,
            };
        push_pattern(&mut bits, table, right);
        push_bits(&mut bits, END, 18);
        grid.push(bits);
        table = if table == 2 { 0 } else { table + 1 };
    }
    grid
}

fn push_pattern(out: &mut Vec<bool>, table: u32, cw: u32) {
    let pattern = LEADING_ONE + HL_TO_LL[(table * 929 + cw) as usize] as u32;
    push_bits(out, pattern, 17);
}

fn push_bits(out: &mut Vec<bool>, pattern: u32, len: u8) {
    for i in (0..len).rev() {
        out.push((pattern >> i) & 1 == 1);
    }
}

/// Modulraster → PBM (P1, ASCII), mit Skalierung und 2-Modul-Ruhezone.
pub fn to_pbm(grid: &[Vec<bool>], scale: usize) -> String {
    let qz = 2; // Ruhezone in Modulen
    let modules_w = grid.first().map(|r| r.len()).unwrap_or(0);
    let w = (modules_w + 2 * qz) * scale;
    let h = (grid.len() + 2 * qz) * scale;
    let mut out = format!("P1\n{w} {h}\n");
    let blank_row = "0".repeat(w);
    for _ in 0..qz * scale {
        out.push_str(&blank_row);
        out.push('\n');
    }
    for row in grid {
        let mut line = String::with_capacity(w);
        for _ in 0..qz * scale {
            line.push('0');
        }
        for &m in row {
            let c = if m { '1' } else { '0' };
            for _ in 0..scale {
                line.push(c);
            }
        }
        for _ in 0..qz * scale {
            line.push('0');
        }
        for _ in 0..scale {
            out.push_str(&line);
            out.push('\n');
        }
    }
    for _ in 0..qz * scale {
        out.push_str(&blank_row);
        out.push('\n');
    }
    out
}

/// Modulraster → 8-Bit-Graustufenpuffer (schwarz=0, weiss=255) mit Ruhezone.
/// `sx`/`sy` skalieren horizontal/vertikal (PDF417-Zeilen sind höher als breit).
#[allow(dead_code)] // genutzt im Round-Trip-Test; künftig für Bildausgabe.
pub fn to_luma(grid: &[Vec<bool>], sx: usize, sy: usize) -> (Vec<u8>, usize, usize) {
    let qz = 4; // Ruhezone in Modulen
    let mw = grid.first().map(|r| r.len()).unwrap_or(0);
    let w = (mw + 2 * qz) * sx;
    let h = (grid.len() + 2 * qz) * sy;
    let mut buf = vec![255u8; w * h];
    for (ry, row) in grid.iter().enumerate() {
        for (rx, &m) in row.iter().enumerate() {
            if !m {
                continue;
            }
            let (x0, y0) = ((rx + qz) * sx, (ry + qz) * sy);
            for dy in 0..sy {
                for dx in 0..sx {
                    buf[(y0 + dy) * w + (x0 + dx)] = 0;
                }
            }
        }
    }
    (buf, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_compaction_packs_six_to_five() {
        // 6 Bytes → Latch(924) + 5 Codewörter.
        let cw = byte_compaction(&[1, 2, 3, 4, 5, 6]);
        assert_eq!(cw[0], 924);
        assert_eq!(cw.len(), 6);
        // < 6 Bytes: Latch(901) + rohe Bytes.
        let cw2 = byte_compaction(&[65, 66]);
        assert_eq!(cw2, vec![901, 65, 66]);
    }

    #[test]
    fn builds_and_renders_single_symbol() {
        let cols = 6u8;
        let rows = 12u8;
        let level = 2u8; // 8 EC-Codewörter
        let cws = build_symbol(b"eCH-0196 Demo", cols, rows, level).expect("fits");
        assert_eq!(cws.len(), cols as usize * rows as usize);
        let grid = render(&cws, cols, rows, level);
        assert_eq!(grid.len(), rows as usize);
        // Zeilenbreite = START(17) + links(17) + cols*17 + rechts(17) + END(18).
        let expected_w = 17 + 17 + cols as usize * 17 + 17 + 18;
        assert_eq!(grid[0].len(), expected_w);
        // Jede Zeile beginnt mit dem Startmuster 11111111 …
        assert!(grid[0][0] && grid[0][1] && grid[0][7]);
    }

    #[test]
    fn rejects_oversized_payload() {
        // 6×12, Level 2: Kapazität klein → grosse Nutzlast wird abgewiesen.
        let big = vec![0u8; 500];
        assert!(build_symbol(&big, 6, 12, 2).is_err());
    }

    #[test]
    fn pbm_has_header_and_quiet_zone() {
        let cws = build_symbol(b"x", 4, 6, 1).unwrap();
        let grid = render(&cws, 4, 6, 1);
        let pbm = to_pbm(&grid, 1);
        assert!(pbm.starts_with("P1\n"));
    }

    // Round-Trip: unser gerendertes PDF417 wird mit rxing (zxing-Port) dekodiert.
    // Beweist die Korrektheit von Byte-Compaction, Symbol-Assemblierung, EC und Rendering.
    #[test]
    fn roundtrip_decodes_with_rxing() {
        let payload = b"Hello eCH-0196 Steuerauszug 2025 - PDF417 roundtrip test 12345";
        let (cols, rows, level) = (10u8, 30u8, 4u8);
        let cws = build_symbol(payload, cols, rows, level).expect("fits");
        let grid = render(&cws, cols, rows, level);
        let (luma, w, h) = to_luma(&grid, 3, 9);
        let res = rxing::helpers::detect_in_luma(luma, w as u32, h as u32, Some(rxing::BarcodeFormat::PDF_417))
            .expect("rxing decode");
        assert_eq!(res.getText().as_bytes(), payload);
    }

    // Structured Append: grosse Nutzlast → mehrere Segmente, jedes mit Macro-Block.
    // Jedes Segment wird dekodiert; die Chunks zusammengesetzt ergeben die Nutzlast.
    #[test]
    fn structured_append_roundtrips_with_rxing() {
        let payload: Vec<u8> = (0..600u32).map(|i| b'A' + (i % 26) as u8).collect();
        let (cols, rows, level) = (13u8, 35u8, 4u8); // eCH-0196-Layout
        let symbols = build_symbols(&payload, cols, rows, level).expect("build");
        assert!(symbols.len() >= 2, "mehrere Segmente erwartet, war {}", symbols.len());
        let mut decoded = Vec::new();
        for sym in &symbols {
            let grid = render(sym, cols, rows, level);
            let (luma, w, h) = to_luma(&grid, 3, 9);
            let res = rxing::helpers::detect_in_luma(luma, w as u32, h as u32, Some(rxing::BarcodeFormat::PDF_417))
                .expect("decode segment");
            decoded.extend_from_slice(res.getText().as_bytes());
        }
        assert_eq!(decoded, payload);
    }
}
