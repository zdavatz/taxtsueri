//! Parser für einen **UBS-Vermögensausweis** (Portfolio-Auszug, Text-PDF).
//!
//! Liest die Detailpositionen (Aktien + Konten) aus dem via `pdftotext -layout`
//! extrahierten Text: Anzahl, Bezeichnung, **Valor/ISIN**, Währung, Domizilland
//! und **Marktwert in CHF** (= Steuerwert/Verkehrswert per Stichtag) sowie – wo
//! direkt vorhanden – die Ausschüttung pro Titel.
//!
//! Mapping ins [`SecurityEntry`](crate::model::SecurityEntry): `taxValueEndOfYear`
//! = Marktwert (für die Vermögenssteuer ist der Verkehrswert massgebend). Der
//! **CHF-Bruttoertrag** lässt sich für CHF-Titel aus Anzahl × Ausschüttung direkt
//! rechnen; bei Fremdwährungstiteln braucht es den Umrechnungskurs (präzise aus
//! dem eCH-0196-eSteuerauszug) – dort bleibt der Ertrag offen.

use crate::model::{ListOfSecurities, SecurityEntry, TaxAmount};

#[derive(Debug, Default)]
pub struct Position {
    pub quantity: String,
    pub name: String,
    pub valor: Option<String>,
    pub isin: Option<String>,
    pub currency: Option<String>,
    pub country: Option<String>,
    pub tax_value_chf: i64,
    pub dividend_per_share: Option<f64>,
    pub dividend_currency: Option<String>,
}

fn country_code(section: &str) -> Option<&'static str> {
    match section.trim() {
        "Schweiz" => Some("CH"),
        "Vereinigte Staaten" => Some("US"),
        "Deutschland" => Some("DE"),
        "Japan" => Some("JP"),
        "Frankreich" => Some("FR"),
        "Vereinigtes Königreich" => Some("GB"),
        "Niederlande" => Some("NL"),
        _ => None,
    }
}

const CURRENCIES: &[&str] = &["CHF", "USD", "EUR", "JPY", "GBP", "CAD", "AUD"];

/// Marktwert + %NV stehen am Zeilenende nach dem letzten `%`-Token.
fn parse_market_value(line: &str) -> Option<i64> {
    let after = line.rsplit_once('%')?.1;
    let toks: Vec<&str> = after.split_whitespace().collect();
    if toks.len() < 2 {
        return None;
    }
    // letztes Token = %NV (enthält '.'), davor die ganzzahligen Marktwert-Teile.
    let digits: String = toks[..toks.len() - 1]
        .iter()
        .take_while(|t| t.chars().all(|c| c.is_ascii_digit()))
        .flat_map(|t| t.chars())
        .collect();
    digits.parse().ok()
}

/// Kopfzeile einer Position: "Anzahl  Name … WÄHRUNG Preis … % Marktwert %NV".
fn parse_header(line: &str) -> Option<(String, String, Option<String>, i64)> {
    let toks: Vec<&str> = line.split_whitespace().collect();
    let cur_pos = toks.iter().position(|t| CURRENCIES.contains(t))?;
    if cur_pos == 0 {
        return None;
    }
    // Anzahl = führende reine Ziffern-Tokens, Name = Rest bis zur Währung.
    let mut quantity = String::new();
    let mut name_start = 0;
    for (i, t) in toks[..cur_pos].iter().enumerate() {
        if t.chars().all(|c| c.is_ascii_digit()) {
            quantity.push_str(t);
        } else {
            name_start = i;
            break;
        }
    }
    if quantity.is_empty() || name_start == 0 {
        return None;
    }
    let name = toks[name_start..cur_pos].join(" ");
    let currency = toks[cur_pos].to_string();
    let market = parse_market_value(line)?;
    Some((quantity, name, Some(currency), market))
}

/// Parst den `pdftotext -layout`-Text eines Vermögensausweises.
pub fn parse_text(text: &str) -> Vec<Position> {
    let mut out = Vec::new();
    let mut section = String::new();
    let mut pending: Option<Position> = None;

    for raw in text.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim();

        if let Some(cc) = country_code(trimmed) {
            section = cc.to_string();
            continue;
        }
        if trimmed.starts_with("Subtotal") || trimmed.starts_with("Total") {
            continue;
        }

        // Kopfzeile einer Wertschriftenposition?
        if line.contains('%') {
            if let Some((quantity, name, currency, market)) = parse_header(line) {
                if let Some(p) = pending.take() {
                    out.push(p);
                }
                pending = Some(Position {
                    quantity,
                    name,
                    currency,
                    tax_value_chf: market,
                    country: Some(section.clone()).filter(|s| !s.is_empty()),
                    ..Default::default()
                });
                continue;
            }
        }

        if let Some(p) = pending.as_mut() {
            if let Some(rest) = trimmed.strip_prefix("Ausschüttungsbetrag:") {
                let toks: Vec<&str> = rest.split_whitespace().collect();
                if toks.len() >= 2 {
                    p.dividend_currency = Some(toks[0].to_string());
                    p.dividend_per_share = toks[1].parse().ok();
                }
            } else if let Some(rest) = trimmed.strip_prefix("Valor ") {
                // "12345 - ISIN CH0001234567"
                let mut it = rest.split_whitespace();
                p.valor = it.next().map(|s| s.to_string());
                if let Some(isin) = rest.split("ISIN").nth(1) {
                    p.isin = isin.split_whitespace().next().map(|s| s.to_string());
                }
            }
        }
    }
    if let Some(p) = pending.take() {
        out.push(p);
    }
    out
}

/// Wandelt Positionen in eCH-0119-Wertschriftenpositionen.
/// CHF-Titel: Bruttoertrag (Kolonne A) = Anzahl × Ausschüttung; sonst offen.
pub fn to_securities(positions: &[Position]) -> Vec<SecurityEntry> {
    positions
        .iter()
        .map(|p| {
            let is_ch = p.country.as_deref() == Some("CH");
            let gross_chf = match (p.dividend_per_share, p.dividend_currency.as_deref()) {
                (Some(d), Some("CHF")) => {
                    let qty: f64 = p.quantity.parse().unwrap_or(0.0);
                    Some((d * qty).round() as i64)
                }
                _ => None,
            };
            SecurityEntry {
                currency: p.currency.clone(),
                quantity: Some(p.quantity.clone()),
                securities_number: p.valor.clone().or_else(|| p.isin.clone()),
                description: p.name.clone(),
                country: p.country.clone().filter(|_| !is_ch),
                tax_value: Some(TaxAmount::new(p.tax_value_chf, p.tax_value_chf)),
                gross_revenue_a: gross_chf.filter(|_| is_ch).map(|g| TaxAmount::new(g, g)),
                gross_revenue_b: None,
            }
        })
        .collect()
}

/// Baut aus beliebigen [`SecurityEntry`]s ein eCH-0119-Wertschriftenverzeichnis und
/// berechnet die Summen (Steuerwert, Bruttoertrag A, 35 % Verrechnungssteuer).
/// Gemeinsame Basis für Vermögensausweis und MT940-Konto.
pub fn build_list_of_securities(security: Vec<SecurityEntry>) -> ListOfSecurities {
    let total_tax: i64 = security.iter().filter_map(|s| s.tax_value.map(|t| t.cantonal)).sum();
    let gross_a: i64 = security.iter().filter_map(|s| s.gross_revenue_a.map(|t| t.cantonal)).sum();
    ListOfSecurities {
        bank_account: None,
        security_entry: security,
        attached_form_da1: None,
        total_tax_value: Some(TaxAmount::new(total_tax, total_tax)),
        subtotal_gross_revenue_a1: (gross_a != 0).then(|| TaxAmount::new(gross_a, gross_a)),
        subtotal_gross_revenue_b: None,
        total_gross_revenue: None,
        withholding_tax: (gross_a != 0).then(|| format!("{:.2}", gross_a as f64 * 0.35)),
    }
}

/// Baut aus dem Vermögensausweis-Text direkt ein eCH-0119-Wertschriftenverzeichnis.
pub fn list_of_securities_from_text(text: &str) -> ListOfSecurities {
    build_list_of_securities(to_securities(&parse_text(text)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Auszug im pdftotext -layout-Stil (zwei Positionen).
    const SAMPLE: &str = "Aktien - Aktienanlagen
Schweiz
                            24 N-Akt Swisscom AG                          CHF 430.8         575.5      33.59%        13 812     8.22
                               (SCMN)                       Kommunikation
                               Ausschüttung: 01.04.2025
                               Ausschüttungsbetrag: CHF 22                3.83% DR
                               Valor 874251 - ISIN CH0008742519
                               Lagerland Schweiz
Vereinigte Staaten
                           308 N-Akt Coca-Cola Co                         USD 41.2         70.74      71.69%        17 059    10.15
                               (KO)                         Basiskonsumgüter
                               Ausschüttungsbetrag: USD 2.04              2.88% DR
                               Valor 919390 - ISIN US1912161007
                               Lagerland Vereinigte Staaten
";

    #[test]
    fn parses_positions() {
        let p = parse_text(SAMPLE);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].name, "N-Akt Swisscom AG");
        assert_eq!(p[0].valor.as_deref(), Some("874251"));
        assert_eq!(p[0].isin.as_deref(), Some("CH0008742519"));
        assert_eq!(p[0].tax_value_chf, 13_812);
        assert_eq!(p[0].country.as_deref(), Some("CH"));
        assert_eq!(p[0].dividend_per_share, Some(22.0));
        assert_eq!(p[1].name, "N-Akt Coca-Cola Co");
        assert_eq!(p[1].tax_value_chf, 17_059);
        assert_eq!(p[1].country.as_deref(), Some("US"));
    }

    #[test]
    fn maps_to_securities() {
        let secs = to_securities(&parse_text(SAMPLE));
        // CHF-Titel: Bruttoertrag A = 24 × 22 = 528.
        assert_eq!(secs[0].gross_revenue_a.unwrap().cantonal, 528);
        assert_eq!(secs[0].country, None); // CH → kein countryOfDepositaryBank
        assert_eq!(secs[0].tax_value.unwrap().cantonal, 13_812);
        // US-Titel: Domizil US, Bruttoertrag offen (Fremdwährung).
        assert_eq!(secs[1].country.as_deref(), Some("US"));
        assert!(secs[1].gross_revenue_a.is_none());
    }
}
