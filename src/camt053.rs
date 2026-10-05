//! Reader für **camt.053** (ISO 20022 «Bank-to-Customer Statement», Nachfolger von
//! MT940). Mappt eine camt.053-XML in die bestehende [`crate::mt940::Statement`], damit
//! die gesamte Pipeline (Kategorisierung, Cash-Flow-Rechnung, Konto-Position) unverändert
//! weiterläuft.
//!
//! Vorteil gegenüber MT940: **strukturierte Felder** — Gegenpartei (`RltdPties`),
//! Verwendungszweck (`RmtInf/Ustrd`), Bank-Buchungscode (`BkTxCd`) und die Bank-Narrative
//! (`AddtlNtryInf`) liegen getrennt vor und ergeben eine deutlich bessere Beschreibung
//! für `mt940::category()` als der MT940-`:86:`-Freitext.

use crate::mt940::{Balance, Statement, Transaction};
use serde::Deserialize;

// --- camt.053-Teilbaum (nur die genutzten Elemente; Namespace wird ignoriert) ---

#[derive(Debug, Deserialize)]
struct Document {
    #[serde(rename = "BkToCstmrStmt")]
    msg: BkToCstmrStmt,
}

#[derive(Debug, Deserialize)]
struct BkToCstmrStmt {
    #[serde(rename = "Stmt", default)]
    stmt: Vec<Stmt>,
}

#[derive(Debug, Deserialize)]
struct Stmt {
    #[serde(rename = "Acct", default)]
    acct: Option<Acct>,
    #[serde(rename = "Bal", default)]
    bal: Vec<Bal>,
    #[serde(rename = "Ntry", default)]
    ntry: Vec<Ntry>,
}

#[derive(Debug, Deserialize)]
struct Acct {
    #[serde(rename = "Id", default)]
    id: Option<AcctId>,
}

#[derive(Debug, Deserialize)]
struct AcctId {
    #[serde(rename = "IBAN", default)]
    iban: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Amt {
    #[serde(rename = "@Ccy", default)]
    ccy: String,
    #[serde(rename = "$text", default)]
    val: String,
}

/// `<Dt><Dt>2026-06-01</Dt></Dt>` (Datum kann auch `DtTm` sein — hier genügt `Dt`).
#[derive(Debug, Deserialize)]
struct DtWrap {
    #[serde(rename = "Dt", default)]
    dt: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Bal {
    #[serde(rename = "Tp")]
    tp: BalTp,
    #[serde(rename = "Amt")]
    amt: Amt,
    #[serde(rename = "CdtDbtInd")]
    cdt_dbt: String,
    #[serde(rename = "Dt", default)]
    dt: Option<DtWrap>,
}

#[derive(Debug, Deserialize)]
struct BalTp {
    #[serde(rename = "CdOrPrtry")]
    code: CdOrPrtry,
}

#[derive(Debug, Deserialize)]
struct CdOrPrtry {
    #[serde(rename = "Cd", default)]
    cd: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Ntry {
    #[serde(rename = "Amt")]
    amt: Amt,
    #[serde(rename = "CdtDbtInd")]
    cdt_dbt: String,
    #[serde(rename = "BookgDt", default)]
    bookg_dt: Option<DtWrap>,
    #[serde(rename = "ValDt", default)]
    val_dt: Option<DtWrap>,
    #[serde(rename = "AddtlNtryInf", default)]
    addtl_inf: Option<String>,
    #[serde(rename = "BkTxCd", default)]
    bk_tx_cd: Option<BkTxCd>,
    #[serde(rename = "NtryDtls", default)]
    dtls: Vec<NtryDtls>,
}

#[derive(Debug, Deserialize)]
struct BkTxCd {
    #[serde(rename = "Prtry", default)]
    prtry: Option<Prtry>,
}

#[derive(Debug, Deserialize)]
struct Prtry {
    #[serde(rename = "Cd", default)]
    cd: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NtryDtls {
    #[serde(rename = "TxDtls", default)]
    tx: Vec<TxDtls>,
}

#[derive(Debug, Deserialize)]
struct TxDtls {
    #[serde(rename = "RltdPties", default)]
    pties: Option<RltdPties>,
    #[serde(rename = "RmtInf", default)]
    rmt: Option<RmtInf>,
}

#[derive(Debug, Deserialize)]
struct RltdPties {
    #[serde(rename = "Dbtr", default)]
    dbtr: Option<Party>,
    #[serde(rename = "Cdtr", default)]
    cdtr: Option<Party>,
}

#[derive(Debug, Deserialize)]
struct Party {
    #[serde(rename = "Pty", default)]
    pty: Option<PartyDetail>,
}

#[derive(Debug, Deserialize)]
struct PartyDetail {
    #[serde(rename = "Nm", default)]
    nm: Option<String>,
    #[serde(rename = "PstlAdr", default)]
    adr: Option<PstlAdr>,
}

#[derive(Debug, Deserialize)]
struct PstlAdr {
    #[serde(rename = "AdrLine", default)]
    adr_line: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RmtInf {
    #[serde(rename = "Ustrd", default)]
    ustrd: Vec<String>,
}

// --- Mapping camt.053 → mt940::Statement ---

/// Dezimaler CHF-Betrag ("150", "73.47", "1200.5") → Rappen.
fn amount_to_cents(s: &str) -> i64 {
    let s = s.trim();
    let neg = s.starts_with('-');
    let s = s.trim_start_matches('-');
    let (int, frac) = s.split_once('.').unwrap_or((s, ""));
    let int: i64 = int.parse().unwrap_or(0);
    let mut frac: String = frac.chars().take(2).collect();
    while frac.len() < 2 {
        frac.push('0');
    }
    let frac: i64 = frac.parse().unwrap_or(0);
    let cents = int * 100 + frac;
    if neg {
        -cents
    } else {
        cents
    }
}

impl Party {
    fn name(&self) -> Option<String> {
        let d = self.pty.as_ref()?;
        if let Some(n) = &d.nm {
            return Some(n.clone());
        }
        d.adr
            .as_ref()
            .and_then(|a| a.adr_line.first().cloned())
    }
}

/// Parst **eine** camt.053-XML in eine [`Statement`].
pub fn parse(xml: &str) -> Result<Statement, String> {
    let doc: Document = quick_xml::de::from_str(xml).map_err(|e| format!("camt.053: {e}"))?;
    let stmt = doc
        .msg
        .stmt
        .into_iter()
        .next()
        .ok_or("camt.053: kein <Stmt>")?;

    let account = stmt
        .acct
        .and_then(|a| a.id)
        .and_then(|i| i.iban)
        .unwrap_or_default();

    let balance = |codes: &[&str]| -> Option<Balance> {
        stmt.bal.iter().find_map(|b| {
            let cd = b.tp.code.cd.as_deref()?;
            if !codes.contains(&cd) {
                return None;
            }
            Some(Balance {
                credit: b.cdt_dbt == "CRDT",
                date: b.dt.as_ref().and_then(|d| d.dt.clone()).unwrap_or_default(),
                currency: b.amt.ccy.clone(),
                amount_cents: amount_to_cents(&b.amt.val),
            })
        })
    };
    // OPBD = Eröffnungssaldo (PRCD = vorheriger Schlusssaldo als Fallback),
    // CLBD = Schlusssaldo.
    let opening = balance(&["OPBD", "PRCD"]);
    let closing = balance(&["CLBD"]);

    let transactions = stmt.ntry.into_iter().map(ntry_to_tx).collect();

    Ok(Statement {
        account,
        opening,
        closing,
        transactions,
    })
}

fn ntry_to_tx(n: Ntry) -> Transaction {
    let credit = n.cdt_dbt == "CRDT";
    // Gegenpartei: bei Gutschrift der Zahler (Dbtr), bei Belastung der Empfänger (Cdtr).
    let mut counterparty = None;
    let mut remittance: Vec<String> = Vec::new();
    if let Some(dtls) = n.dtls.into_iter().next() {
        if let Some(tx) = dtls.tx.into_iter().next() {
            if let Some(p) = tx.pties {
                counterparty = if credit {
                    p.dbtr.and_then(|d| d.name())
                } else {
                    p.cdtr.and_then(|c| c.name())
                };
            }
            if let Some(r) = tx.rmt {
                remittance = r.ustrd;
            }
        }
    }
    // Beschreibung aus strukturierten Feldern zusammensetzen — Futter für category().
    let mut parts: Vec<String> = Vec::new();
    if let Some(c) = &counterparty {
        parts.push(c.clone());
    }
    if let Some(a) = &n.addtl_inf {
        parts.push(a.clone());
    }
    parts.extend(remittance);
    let description = parts.join(" ");

    Transaction {
        value_date: n
            .val_dt
            .and_then(|d| d.dt)
            .or_else(|| n.bookg_dt.and_then(|d| d.dt))
            .unwrap_or_default(),
        credit,
        amount_cents: amount_to_cents(&n.amt.val),
        kind: n.bk_tx_cd.and_then(|b| b.prtry).and_then(|p| p.cd).unwrap_or_default(),
        booking_type: counterparty.unwrap_or_else(|| n.addtl_inf.clone().unwrap_or_default()),
        description,
    }
}

/// Fasst die Tagesauszüge **eines** Kontos zusammen: Buchungen chronologisch
/// zusammengefügt, Eröffnungssaldo vom frühesten Tag, Schlusssaldo vom spätesten.
fn aggregate(mut stmts: Vec<Statement>) -> Statement {
    // Nach Schlusssaldo-Datum (bzw. erster Buchung) sortieren.
    stmts.sort_by(|a, b| {
        let ka = a.closing.as_ref().map(|x| x.date.clone()).unwrap_or_default();
        let kb = b.closing.as_ref().map(|x| x.date.clone()).unwrap_or_default();
        ka.cmp(&kb)
    });
    let account = stmts.iter().find(|s| !s.account.is_empty()).map(|s| s.account.clone()).unwrap_or_default();
    let opening = stmts.first().and_then(|s| s.opening.clone());
    let closing = stmts.last().and_then(|s| s.closing.clone());
    let transactions = stmts.into_iter().flat_map(|s| s.transactions).collect();
    Statement {
        account,
        opening,
        closing,
        transactions,
    }
}

/// Liest mehrere camt.053-Tagesdateien (XML-Strings, in beliebiger Reihenfolge) und
/// liefert **je Konto** eine aggregierte [`Statement`], nach IBAN sortiert. Auszüge
/// verschiedener Konten werden nie vermischt — ihre Salden hätten nichts miteinander zu tun.
pub fn parse_by_account(xmls: &[String]) -> Result<Vec<Statement>, String> {
    if xmls.is_empty() {
        return Err("keine camt.053-Dateien".into());
    }
    let mut by_account: std::collections::BTreeMap<String, Vec<Statement>> = std::collections::BTreeMap::new();
    for xml in xmls {
        let stmt = parse(xml)?;
        by_account.entry(stmt.account.clone()).or_default().push(stmt);
    }
    Ok(by_account.into_values().map(aggregate).collect())
}

/// Aggregiert die camt.053-Tagesdateien **eines** Kontos zu einer [`Statement`].
/// Enthalten die Dateien mehrere Konten, ist das ein Fehler — dafür gibt es
/// [`parse_by_account`].
pub fn parse_many(xmls: &[String]) -> Result<Statement, String> {
    let mut accounts = parse_by_account(xmls)?;
    if accounts.len() > 1 {
        let ibans: Vec<&str> = accounts.iter().map(|s| s.account.as_str()).collect();
        return Err(format!(
            "die Dateien gehören zu {} verschiedenen Konten ({}) — bitte nach Konto getrennt auswerten",
            accounts.len(),
            ibans.join(", ")
        ));
    }
    Ok(accounts.remove(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:camt.053.001.08">
  <BkToCstmrStmt>
    <Stmt>
      <Acct><Id><IBAN>CH9300762011623852957</IBAN></Id></Acct>
      <Bal><Tp><CdOrPrtry><Cd>OPBD</Cd></CdOrPrtry></Tp><Amt Ccy="CHF">50000.00</Amt><CdtDbtInd>CRDT</CdtDbtInd><Dt><Dt>2026-06-01</Dt></Dt></Bal>
      <Bal><Tp><CdOrPrtry><Cd>CLBD</Cd></CdOrPrtry></Tp><Amt Ccy="CHF">48873.47</Amt><CdtDbtInd>CRDT</CdtDbtInd><Dt><Dt>2026-06-01</Dt></Dt></Bal>
      <Ntry>
        <Amt Ccy="CHF">73.47</Amt><CdtDbtInd>CRDT</CdtDbtInd>
        <ValDt><Dt>2026-05-31</Dt></ValDt>
        <AddtlNtryInf>Dividend // VN 966021 // WELLS FARGO RG</AddtlNtryInf>
        <NtryDtls><TxDtls><RltdPties><Dbtr><Pty><Nm>WELLS FARGO</Nm></Pty></Dbtr></RltdPties></TxDtls></NtryDtls>
      </Ntry>
      <Ntry>
        <Amt Ccy="CHF">1200.00</Amt><CdtDbtInd>DBIT</CdtDbtInd>
        <ValDt><Dt>2026-06-01</Dt></ValDt>
        <AddtlNtryInf>MIETE BEISPIEL</AddtlNtryInf>
        <NtryDtls><TxDtls><RltdPties><Cdtr><Pty><Nm>Immobilien AG</Nm></Pty></Cdtr></RltdPties></TxDtls></NtryDtls>
      </Ntry>
    </Stmt>
  </BkToCstmrStmt>
</Document>"#;

    #[test]
    fn parses_balances_entries_and_parties() {
        let s = parse(SAMPLE).expect("parse");
        assert_eq!(s.account, "CH9300762011623852957");
        assert_eq!(s.opening.as_ref().unwrap().amount_cents, 5_000_000);
        assert_eq!(s.closing.as_ref().unwrap().amount_cents, 4_887_347);
        assert_eq!(s.transactions.len(), 2);
        // Gutschrift: Gegenpartei = Zahler (Dbtr), Dividende erkennbar.
        assert!(s.transactions[0].credit);
        assert_eq!(s.transactions[0].amount_cents, 7_347);
        assert!(s.transactions[0].description.contains("WELLS FARGO"));
        assert!(s.transactions[0].description.to_lowercase().contains("dividend"));
        // Belastung: Gegenpartei = Empfänger (Cdtr).
        assert!(!s.transactions[1].credit);
        assert!(s.transactions[1].description.contains("Immobilien AG"));
        assert!(s.transactions[1].description.contains("MIETE"));
    }

    #[test]
    fn statements_of_different_accounts_are_kept_apart() {
        let other = SAMPLE.replace("CH9300762011623852957", "CH5604835012345678009");
        let next_day = SAMPLE.replace("2026-06-01", "2026-06-02");
        let xmls = [other, SAMPLE.to_string(), next_day];
        let accounts = parse_by_account(&xmls).expect("parse");
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].account, "CH5604835012345678009");
        assert_eq!(accounts[0].transactions.len(), 2);
        // Zwei Tage desselben Kontos werden zusammengefasst.
        assert_eq!(accounts[1].account, "CH9300762011623852957");
        assert_eq!(accounts[1].transactions.len(), 4);
        assert_eq!(accounts[1].closing.as_ref().unwrap().date, "2026-06-02");
        // parse_many vermischt nichts, sondern lehnt gemischte Dateien ab.
        assert!(parse_many(&xmls).unwrap_err().contains("2 verschiedenen Konten"));
        assert_eq!(parse_many(&xmls[1..]).unwrap().transactions.len(), 4);
    }

    #[test]
    fn category_uses_structured_fields() {
        let s = parse(SAMPLE).expect("parse");
        // Dividende → Finanzertrag; Miete → Raumaufwand (dank strukturiertem Text).
        assert_eq!(crate::mt940::category(&s.transactions[0]), "Finanzertrag (Dividenden/Zinsen)");
        assert_eq!(crate::mt940::category(&s.transactions[1]), "Raumaufwand");
    }
}
