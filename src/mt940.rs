//! Reader für **MT940** (SWIFT-Kontoauszug).
//!
//! Liest Konto, Eröffnungs-/Schlusssaldo (`:60F`/`:62F`) und die Buchungszeilen
//! (`:61:` + `:86:`-Verwendungszweck). Beträge werden in **Rappen** (i64) geführt,
//! um Rundungsfehler zu vermeiden (MT940 nutzt Komma als Dezimaltrenner).
//!
//! Hinweis: MT940 ist ein Zahlungsverkehrs-Auszug — er liefert **Salden und
//! Transaktionen**, aber keine steuerlich kategorisierten Wertschriftendaten
//! (dafür eCH-0196). Nützlich z. B. für den Jahresend-Kontostand (Bilanz/Guthaben)
//! oder als Beleg-Gegenprobe.

use serde::Serialize;

#[derive(Debug, Default, Serialize)]
pub struct Statement {
    pub account: String,
    pub opening: Option<Balance>,
    pub closing: Option<Balance>,
    pub transactions: Vec<Transaction>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Balance {
    pub credit: bool,
    /// ISO-Datum YYYY-MM-DD.
    pub date: String,
    pub currency: String,
    pub amount_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct Transaction {
    pub value_date: String,
    pub credit: bool,
    pub amount_cents: i64,
    /// Transaktionstyp-Kennung (z. B. "NTRF").
    pub kind: String,
    pub description: String,
}

impl Statement {
    pub fn total_credit_cents(&self) -> i64 {
        self.transactions.iter().filter(|t| t.credit).map(|t| t.amount_cents).sum()
    }
    pub fn total_debit_cents(&self) -> i64 {
        self.transactions.iter().filter(|t| !t.credit).map(|t| t.amount_cents).sum()
    }
}

/// Rappen → "CHF 1'234.56"-artige Anzeige (ganze Zahl mit 2 Dezimalen).
pub fn format_cents(c: i64) -> String {
    format!("{}.{:02}", c / 100, (c % 100).abs())
}

/// MT940-Betrag mit Komma ("105232,94", "34,1", "21,") → Rappen.
fn parse_amount_cents(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (int_part, frac_part) = match s.split_once(',') {
        Some((a, b)) => (a, b),
        None => (s, ""),
    };
    let int: i64 = int_part.parse().ok()?;
    // Bruchteil auf genau 2 Stellen bringen.
    let mut frac = frac_part.chars().filter(|c| c.is_ascii_digit()).collect::<String>();
    while frac.len() < 2 {
        frac.push('0');
    }
    let frac: i64 = frac[..2].parse().ok()?;
    Some(int * 100 + frac)
}

/// "YYMMDD" → "20YY-MM-DD".
fn iso_date(yymmdd: &str) -> String {
    if yymmdd.len() == 6 && yymmdd.chars().all(|c| c.is_ascii_digit()) {
        format!("20{}-{}-{}", &yymmdd[0..2], &yymmdd[2..4], &yymmdd[4..6])
    } else {
        yymmdd.to_string()
    }
}

fn parse_balance(v: &str) -> Option<Balance> {
    let v = v.trim();
    let credit = v.starts_with('C');
    if !(credit || v.starts_with('D')) || v.len() < 10 {
        return None;
    }
    let date = iso_date(&v[1..7]);
    let currency = v[7..10].to_string();
    let amount_cents = parse_amount_cents(&v[10..])?;
    Some(Balance { credit, date, currency, amount_cents })
}

fn parse_61(line: &str) -> Option<Transaction> {
    if line.len() < 7 {
        return None;
    }
    let value_date = iso_date(&line[0..6]);
    let mut i = 6;
    // optionales Buchungsdatum (4 Ziffern)
    if line.len() >= 10 && line[6..10].chars().all(|c| c.is_ascii_digit()) {
        i = 10;
    }
    let rest = &line[i..];
    // Soll/Haben-Kennung: C, D, RC, RD (R = Storno).
    let reversal = rest.starts_with('R');
    let base_off = if reversal { 1 } else { 0 };
    let base = rest[base_off..].chars().next()?;
    let credit = match base {
        'C' => !reversal,
        'D' => reversal,
        _ => return None,
    };
    i += base_off + 1;
    // optionaler Währungs-Code (1 Buchstabe) vor dem Betrag
    if let Some(c) = line[i..].chars().next() {
        if c.is_ascii_alphabetic() {
            i += 1;
        }
    }
    let amt: String = line[i..].chars().take_while(|c| c.is_ascii_digit() || *c == ',').collect();
    i += amt.len();
    let amount_cents = parse_amount_cents(&amt)?;
    let kind: String = line[i..].chars().take(4).collect();
    Some(Transaction { value_date, credit, amount_cents, kind, description: String::new() })
}

/// Parst einen MT940-Auszug. Tolerant gegenüber Fortsetzungszeilen.
pub fn parse(input: &str) -> Result<Statement, String> {
    let mut stmt = Statement::default();
    let mut tag = String::new();
    let mut buf = String::new();

    // Schliesst das aktuell gesammelte Feld ab.
    let flush = |stmt: &mut Statement, tag: &str, buf: &str| {
        match tag {
            ":25:" => stmt.account = buf.trim().to_string(),
            ":60F:" | ":60M:" => stmt.opening = parse_balance(buf),
            ":62F:" | ":62M:" => stmt.closing = parse_balance(buf),
            ":61:" => {
                let first = buf.lines().next().unwrap_or("");
                if let Some(tx) = parse_61(first) {
                    stmt.transactions.push(tx);
                }
            }
            ":86:" => {
                if let Some(last) = stmt.transactions.last_mut() {
                    let desc = buf.split('\n').collect::<Vec<_>>().join(" ");
                    last.description = desc.trim().to_string();
                }
            }
            _ => {}
        }
    };

    let is_tag = |line: &str| -> bool {
        let b = line.as_bytes();
        b.first() == Some(&b':')
            && line.len() >= 3
            && b[1].is_ascii_digit()
            && b[2].is_ascii_digit()
    };

    for line in input.lines() {
        if is_tag(line) {
            flush(&mut stmt, &tag, &buf);
            // Tag = ":NN:" oder ":NNA:" (z. B. :60F:)
            let end = line[1..].find(':').map(|p| p + 2).unwrap_or(line.len());
            tag = line[..end].to_string();
            buf = line[end..].to_string();
        } else {
            buf.push('\n');
            buf.push_str(line);
        }
    }
    flush(&mut stmt, &tag, &buf);

    if stmt.account.is_empty() && stmt.opening.is_none() && stmt.closing.is_none() {
        return Err("keine MT940-Felder erkannt (Format?)".into());
    }
    Ok(stmt)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = ":20:0083002401Z162\n\
        :25:CH870025125183002401Z\n\
        :28C:162/1\n\
        :60F:C250101CHF105232,94\n\
        :61:2501020103D146,9NTRFNONREF//9930503BN3667947\n\
        Zahlung Debitkarte\n\
        :86:Avent�ras Sport 7537 Müstair\n\
        :61:2501070107C8273,67NTRFNONREF//9751007TO4147811\n\
        :86:Gutschrift Steuerverwaltung\n\
        :62F:C251231CHF113439,85\n";

    #[test]
    fn parses_balances_and_transactions() {
        let s = parse(SAMPLE).expect("parse");
        assert_eq!(s.account, "CH870025125183002401Z");
        assert_eq!(s.opening.as_ref().unwrap().amount_cents, 10_523_294);
        assert_eq!(s.opening.as_ref().unwrap().date, "2025-01-01");
        assert_eq!(s.closing.as_ref().unwrap().amount_cents, 11_343_985);
        assert_eq!(s.transactions.len(), 2);
        assert_eq!(s.transactions[0].credit, false);
        assert_eq!(s.transactions[0].amount_cents, 14_690); // 146,90
        assert_eq!(s.transactions[0].kind, "NTRF");
        assert!(s.transactions[0].description.contains("Sport"));
        assert_eq!(s.transactions[1].credit, true);
        assert_eq!(s.transactions[1].amount_cents, 827_367); // 8273,67
        assert_eq!(s.total_credit_cents(), 827_367);
        assert_eq!(s.total_debit_cents(), 14_690);
    }

    #[test]
    fn amount_edge_cases() {
        assert_eq!(parse_amount_cents("21,"), Some(2100));
        assert_eq!(parse_amount_cents("34,1"), Some(3410));
        assert_eq!(parse_amount_cents("0,05"), Some(5));
        assert_eq!(format_cents(11_343_985), "113439.85");
    }
}
