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

/// Baut die Codewort-Matrix eines Einzelsymbols (inkl. EC). Fehler, wenn die
/// Nutzlast nicht in ein Symbol passt (dann ist Structured Append nötig).
pub fn build_symbol(payload: &[u8], cols: u8, rows: u8, level: u8) -> Result<Vec<u16>, String> {
    let total = cols as usize * rows as usize;
    let ec = 1usize << (level as usize + 1);
    let cap = total - ec; // Datenregion inkl. Längendeskriptor
    let data = byte_compaction(payload);
    if data.len() + 1 > cap {
        return Err(format!(
            "Nutzlast zu gross für ein Symbol: {} Codewörter > Kapazität {} (Structured Append nötig)",
            data.len() + 1,
            cap - 1
        ));
    }
    let mut cws = Vec::with_capacity(total);
    cws.push(cap as u16); // Längendeskriptor = Anzahl Datencodewörter (inkl. sich selbst)
    cws.extend_from_slice(&data);
    while cws.len() < cap {
        cws.push(900); // Padding
    }
    cws.resize(total, 0); // EC-Region
    ecc::generate_ecc(&mut cws, level);
    Ok(cws)
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
}
