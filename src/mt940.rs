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

use crate::model::{SecurityEntry, TaxAmount};
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
    /// Transaktionstyp-Kennung aus :61: (z. B. "NTRF").
    pub kind: String,
    /// Klartext-Buchungsart aus der :61:-Fortsetzungszeile (z. B. "Dauerauftrag").
    pub booking_type: String,
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

/// Heuristische Kategorie einer Buchung (anhand Buchungsart + Verwendungszweck).
///
/// Cash-basiert und regelbasiert – eine **Annäherung** an Erfolgsrechnungs-Posten,
/// keine buchhalterisch exakte Zuordnung. `is_income` markiert Ertrags-Kategorien.
pub fn category(tx: &Transaction) -> &'static str {
    let hay = format!("{} {}", tx.booking_type, tx.description).to_lowercase();
    let has = |k: &str| hay.contains(k);
    if tx.credit {
        // Dividenden sind IMMER steuerbarer Ertrag (wie Zinsen, mit 35 % VST) —
        // breit erkennen, da der Beleg sie unterschiedlich benennt.
        if has("dividend") || has("zins") || has("ausschüttung") || has("ausschuettung")
            || has("coupon") || has("kupon") || has("ertragsaussch")
        {
            return "Finanzertrag (Dividenden/Zinsen)";
        }
        // Lohn → über den Lohnausweis deklariert, nicht im Wertschriftenverzeichnis.
        if has("lohn") || has("salär") || has("salaer") || has("gehalt") || has("salary") {
            return "Lohn/Erwerbseinkommen (Lohnausweis)";
        }
        // Steuer-/Spesenrückerstattung → kein steuerbarer Ertrag.
        if has("rückerstattung") || has("rueckerstattung") || has("steuerrück")
            || has("rückzahlung") || has("rueckzahlung") || has("refund")
        {
            return "Rückerstattung (kein Ertrag)";
        }
        // Eigenübertrag zwischen eigenen Konten → kein Einkommen (Doppelzählung vermeiden).
        if has("übertrag") || has("uebertrag") || has("umbuchung") || has("eigenkonto")
            || has("e-banking eigen")
        {
            return "Eigenübertrag (kein Ertrag)";
        }
        if has("e-banking-gutschrift") || has("gutschrift") || has("verg") {
            return "Erlös (Gutschriften)";
        }
        return "Übrige Erträge";
    }
    if has("steuerverwaltung") || has("steueramt") || has("steuern") {
        return "Steuern";
    }
    if has("ausgleichskasse") || has("sva") || has("ahv") || has("pensionskasse")
        || has("bvg") || has("vorsorge") || has("suva") || has("sozialvers")
    {
        return "Sozialversicherungen";
    }
    if has("depot") || has("dl-preisabschluss") || has("geb") {
        return "Bankspesen/Depotgebühren";
    }
    if has("bancomat") || has("barbezug") {
        return "Bargeldbezug";
    }
    if has("debitkarte") {
        return "Spesen Debitkarte (Reise/Verpflegung)";
    }
    if has("dauerauftrag") {
        return "Daueraufträge (Miete/Versicherung)";
    }
    if has("lastschrift") || has("paynet") {
        return "Lastschrift/PayNet (Betriebsaufwand)";
    }
    if has("e-banking-auftrag") {
        return "Überweisungen (e-banking)";
    }
    "Übrige Aufwände"
}

/// Ertrag (Gutschrift) vs. Aufwand (Belastung) — die buchhalterische Grundunter-
/// scheidung. Gutschriften sind Erträge, Belastungen Aufwände.
pub fn is_income(tx: &Transaction) -> bool {
    tx.credit
}

/// Rappen → ganze CHF (kaufmännisch gerundet).
fn round_chf(cents: i64) -> i64 {
    (cents as f64 / 100.0).round() as i64
}

/// Wandelt den Kontoauszug in eine **Konto-Position** fürs eCH-0119-Wertschriften-
/// verzeichnis und bildet damit die **Basis** der Erklärung: Schlusssaldo (`:62F:`) =
/// `taxValueEndOfYear`, der als **Finanzertrag (Zinsen/Dividenden)** erkannte
/// Gutschriftsteil = steuerbarer `grossRevenueA` (Dividenden sind immer Ertrag).
/// Übrige Erträge/Aufwände fliessen NICHT ins Wertschriftenverzeichnis (sie sind im
/// Saldo bereits enthalten bzw. privat).
pub fn account_security_entry(stmt: &Statement) -> SecurityEntry {
    let bal = stmt.closing.as_ref().or(stmt.opening.as_ref());
    let tax_chf = bal.map(|b| round_chf(b.amount_cents)).unwrap_or(0);
    let revenue_cents: i64 = stmt
        .transactions
        .iter()
        .filter(|t| is_income(t) && category(t) == "Finanzertrag (Dividenden/Zinsen)")
        .map(|t| t.amount_cents)
        .sum();
    let interest_chf = round_chf(revenue_cents);
    SecurityEntry {
        currency: bal.map(|b| b.currency.clone()),
        quantity: None,
        securities_number: None,
        description: format!("Kontokorrent {}", stmt.account),
        country: None, // CH
        tax_value: Some(TaxAmount::new(tax_chf, tax_chf)),
        gross_revenue_a: (interest_chf != 0).then(|| TaxAmount::new(interest_chf, interest_chf)),
        gross_revenue_b: None,
    }
}

/// Eine vom Steueramt typischerweise hinterfragte Buchung (verdächtige Gutschrift).
#[derive(Debug, Clone)]
pub struct Flag {
    pub date: String,
    pub amount_cents: i64,
    pub description: String,
    pub category: String,
    pub reason: String,
}

/// Kategorien, die **automatisch** korrekt behandelt werden und daher NICHT
/// nachgefragt werden müssen: Finanzertrag (→ als Ertrag verbucht), Lohn (Lohnausweis),
/// Rückerstattung/Eigenübertrag (kein Ertrag).
fn is_auto_classified(cat: &str) -> bool {
    matches!(
        cat,
        "Finanzertrag (Dividenden/Zinsen)"
            | "Lohn/Erwerbseinkommen (Lohnausweis)"
            | "Rückerstattung (kein Ertrag)"
            | "Eigenübertrag (kein Ertrag)"
    )
}

/// Nur die **wirklich unklaren** Gutschriften (≥ CHF 1'000), die sich nicht automatisch
/// einordnen liessen — typische Aufgriffe des Steueramts (möglicherweise unversteuertes
/// Einkommen). Alles automatisch Erkannte (Zins/Dividende, Lohn, Rückerstattung,
/// Eigenübertrag) erscheint hier NICHT mehr.
pub fn flagged_credits(stmt: &Statement) -> Vec<Flag> {
    stmt.transactions
        .iter()
        .filter(|t| is_income(t))
        .filter(|t| !is_auto_classified(category(t)))
        .filter(|t| t.amount_cents >= 100_000)
        .map(|t| Flag {
            date: t.value_date.clone(),
            amount_cents: t.amount_cents,
            description: t.description.trim().to_string(),
            category: category(t).to_string(),
            reason: "Grössere Gutschrift, nicht automatisch einordenbar — steuerbares Einkommen oder Eigenübertrag?"
                .to_string(),
        })
        .collect()
}

/// Summen je Kategorie.
#[derive(Debug, Serialize)]
pub struct CategoryTotal {
    pub category: String,
    pub count: usize,
    pub credit_cents: i64,
    pub debit_cents: i64,
}

/// Gruppiert die Buchungen nach [`category`], sortiert nach Betrags-Bedeutung.
pub fn categorize(stmt: &Statement) -> Vec<CategoryTotal> {
    let mut map: std::collections::BTreeMap<&'static str, CategoryTotal> = Default::default();
    for tx in &stmt.transactions {
        let cat = category(tx);
        let e = map.entry(cat).or_insert_with(|| CategoryTotal {
            category: cat.to_string(),
            count: 0,
            credit_cents: 0,
            debit_cents: 0,
        });
        e.count += 1;
        if tx.credit {
            e.credit_cents += tx.amount_cents;
        } else {
            e.debit_cents += tx.amount_cents;
        }
    }
    let mut v: Vec<_> = map.into_values().collect();
    v.sort_by_key(|c| -(c.credit_cents + c.debit_cents));
    v
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
    Some(Transaction {
        value_date,
        credit,
        amount_cents,
        kind,
        booking_type: String::new(),
        description: String::new(),
    })
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
                let mut lines = buf.lines();
                let first = lines.next().unwrap_or("");
                if let Some(mut tx) = parse_61(first) {
                    tx.booking_type = lines.collect::<Vec<_>>().join(" ").trim().to_string();
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
    fn categorizes() {
        let s = parse(SAMPLE).expect("parse");
        assert_eq!(s.transactions[0].booking_type, "Zahlung Debitkarte");
        assert_eq!(category(&s.transactions[0]), "Spesen Debitkarte (Reise/Verpflegung)");
        assert_eq!(category(&s.transactions[1]), "Erlös (Gutschriften)");
        let cats = categorize(&s);
        assert!(cats
            .iter()
            .any(|c| c.category.contains("Debitkarte") && c.debit_cents == 14_690));
    }

    #[test]
    fn dividends_are_always_revenue_and_not_flagged() {
        let mt = ":25:CH9300762011623852957\n\
            :60F:C250101CHF100000,00\n\
            :61:2504010401C300,00NTRFNONREF//Div\n\
            :86:Ausschüttung Fonds Anteil\n\
            :61:2505010501C2500,00NTRFNONREF//Lohn\n\
            :86:Lohn April Musterfirma\n\
            :61:2506010601C5000,00NTRFNONREF//X\n\
            :86:Unbekannte Gutschrift Drittperson\n\
            :62F:C251231CHF107800,00\n";
        let s = parse(mt).expect("parse");
        // Dividende/Ausschüttung → Finanzertrag → als Ertrag verbucht.
        let e = account_security_entry(&s);
        assert_eq!(e.gross_revenue_a.unwrap().cantonal, 300);
        // Nur die unbekannte Gutschrift wird markiert — Dividende und Lohn nicht.
        let flags = flagged_credits(&s);
        assert_eq!(flags.len(), 1);
        assert!(flags[0].description.contains("Unbekannte"));
    }

    #[test]
    fn account_becomes_security_entry_with_interest() {
        let mt = ":25:CH9300762011623852957\n\
            :60F:C250101CHF100000,00\n\
            :61:2503150315C500,00NTRFNONREF//Zins\n\
            :86:Zinsgutschrift Sparkonto\n\
            :61:2509100910C8000,00NTRFNONREF//Lohn\n\
            :86:E-Banking-Gutschrift Lohn\n\
            :62F:C251231CHF108500,00\n";
        let s = parse(mt).expect("parse");
        let e = account_security_entry(&s);
        assert!(e.description.contains("CH9300762011623852957"));
        // Schlusssaldo → Vermögen.
        assert_eq!(e.tax_value.unwrap().cantonal, 108_500);
        // Nur der Zinsertrag ist steuerbarer Ertrag — NICHT der Lohn (kein Finanzertrag).
        assert_eq!(e.gross_revenue_a.unwrap().cantonal, 500);
    }

    #[test]
    fn amount_edge_cases() {
        assert_eq!(parse_amount_cents("21,"), Some(2100));
        assert_eq!(parse_amount_cents("34,1"), Some(3410));
        assert_eq!(parse_amount_cents("0,05"), Some(5));
        assert_eq!(format_cents(11_343_985), "113439.85");
    }
}
