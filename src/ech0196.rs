//! Minimaler Reader für **eCH-0196** «eSteuerauszug» (elektronischer Bank-Steuerauszug).
//!
//! eCH-0196-PDFs der Banken enthalten ein eingebettetes XML (`taxStatement`) mit
//! `listOfSecurities` → `depot` → `security`. Dieser Reader liest den für die
//! Steuererklärung relevanten Ausschnitt und baut daraus unsere
//! [`ListOfSecurities`](crate::model::ListOfSecurities) (Wertschriftenverzeichnis).
//!
//! Bewusst nur Teilmenge: Wertpapier-Stammdaten (Name, Valor, ISIN, Währung,
//! Land), Steuerwert (`taxValue/@value`) sowie je Wertpapier die Bruttoerträge
//! aus `payment` (Kolonne A/B). Die Totalwerte stammen aus den Attributen von
//! `listOfSecurities`. Beträge werden auf ganze Franken gerundet (moneyType1),
//! die Verrechnungssteuer behält zwei Nachkommastellen (moneyType2).

use crate::model::{ListOfSecurities, SecurityEntry, TaxAmount};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct TaxStatement {
    #[serde(rename = "listOfSecurities")]
    list_of_securities: Option<RawList>,
}

#[derive(Debug, Deserialize)]
struct RawList {
    #[serde(rename = "@totalTaxValue")]
    total_tax_value: Option<f64>,
    #[serde(rename = "@totalGrossRevenueA")]
    total_gross_revenue_a: Option<f64>,
    #[serde(rename = "@totalGrossRevenueB")]
    total_gross_revenue_b: Option<f64>,
    #[serde(rename = "@totalWithHoldingTaxClaim")]
    total_withholding_tax_claim: Option<f64>,
    #[serde(rename = "depot", default)]
    depot: Vec<RawDepot>,
}

#[derive(Debug, Deserialize)]
struct RawDepot {
    #[serde(rename = "security", default)]
    security: Vec<RawSecurity>,
}

#[derive(Debug, Deserialize)]
struct RawSecurity {
    #[serde(rename = "@valorNumber")]
    valor_number: Option<String>,
    #[serde(rename = "@isin")]
    isin: Option<String>,
    #[serde(rename = "@country")]
    country: Option<String>,
    #[serde(rename = "@currency")]
    currency: Option<String>,
    #[serde(rename = "@securityName")]
    security_name: Option<String>,
    #[serde(rename = "@nominalValue")]
    nominal_value: Option<f64>,
    #[serde(rename = "taxValue")]
    tax_value: Option<RawTaxValue>,
    #[serde(rename = "payment", default)]
    payment: Vec<RawPayment>,
}

#[derive(Debug, Deserialize)]
struct RawTaxValue {
    #[serde(rename = "@value")]
    value: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawPayment {
    #[serde(rename = "@grossRevenueA")]
    gross_revenue_a: Option<f64>,
    #[serde(rename = "@grossRevenueB")]
    gross_revenue_b: Option<f64>,
}

fn chf(v: Option<f64>) -> Option<i64> {
    v.map(|x| x.round() as i64)
}

/// Liest ein eCH-0196-`taxStatement`-XML und baut das Wertschriftenverzeichnis.
pub fn list_of_securities_from_xml(xml: &str) -> Result<ListOfSecurities, String> {
    let stmt: TaxStatement =
        quick_xml::de::from_str(xml).map_err(|e| format!("eCH-0196 XML nicht lesbar: {e}"))?;
    let raw = stmt
        .list_of_securities
        .ok_or_else(|| "eCH-0196: kein <listOfSecurities> gefunden".to_string())?;

    let mut entries = Vec::new();
    for depot in &raw.depot {
        for s in &depot.security {
            let gross_a: f64 = s.payment.iter().filter_map(|p| p.gross_revenue_a).sum();
            let gross_b: f64 = s.payment.iter().filter_map(|p| p.gross_revenue_b).sum();
            entries.push(SecurityEntry {
                currency: s.currency.clone(),
                quantity: s.nominal_value.map(|n| n.to_string()),
                securities_number: s.valor_number.clone().or_else(|| s.isin.clone()),
                description: s.security_name.clone().unwrap_or_default(),
                country: s.country.clone(),
                tax_value: s
                    .tax_value
                    .as_ref()
                    .and_then(|t| chf(t.value))
                    .map(|v| TaxAmount::new(v, v)),
                gross_revenue_a: chf(Some(gross_a)).filter(|&v| v != 0).map(|v| TaxAmount::new(v, v)),
                gross_revenue_b: chf(Some(gross_b)).filter(|&v| v != 0).map(|v| TaxAmount::new(v, v)),
            });
        }
    }

    let total = |v: Option<f64>| chf(v).map(|x| TaxAmount::new(x, x));
    Ok(ListOfSecurities {
        bank_account: None,
        security_entry: entries,
        attached_form_da1: None,
        total_tax_value: total(raw.total_tax_value),
        subtotal_gross_revenue_a1: total(raw.total_gross_revenue_a),
        subtotal_gross_revenue_b: total(raw.total_gross_revenue_b),
        total_gross_revenue: None,
        withholding_tax: raw.total_withholding_tax_claim.map(|v| format!("{v:.2}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimaler, dem eCH-0196-Aufbau folgender Auszug (Attribute, die wir lesen).
    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<taxStatement xmlns="http://www.ech.ch/xmlns/eCH-0196/2" id="x" minorVersion="2">
  <listOfSecurities totalTaxValue="13812" totalGrossRevenueA="528" totalGrossRevenueB="0" totalWithHoldingTaxClaim="184.80">
    <depot depotNumber="1">
      <security positionId="1" country="CH" currency="CHF" quotationType="PIECE"
                securityCategory="SHARE" securityName="Swisscom AG, Ittigen, CH"
                valorNumber="874251" nominalValue="24">
        <taxValue referenceDate="2025-12-31" quotationType="PIECE" quantity="24" balanceCurrency="CHF" value="13812"/>
        <payment paymentDate="2025-04-01" quotationType="PIECE" quantity="24" amountCurrency="CHF" grossRevenueA="528" withHoldingTaxClaim="184.80"/>
      </security>
    </depot>
  </listOfSecurities>
</taxStatement>"#;

    #[test]
    fn parses_minimal_ech0196() {
        let los = list_of_securities_from_xml(SAMPLE).expect("parse");
        assert_eq!(los.security_entry.len(), 1);
        let s = &los.security_entry[0];
        assert_eq!(s.description, "Swisscom AG, Ittigen, CH");
        assert_eq!(s.securities_number.as_deref(), Some("874251"));
        assert_eq!(s.tax_value.unwrap().cantonal, 13812);
        assert_eq!(s.gross_revenue_a.unwrap().cantonal, 528);
        assert!(s.gross_revenue_b.is_none()); // 0 → weggelassen
        assert_eq!(los.total_tax_value.unwrap().cantonal, 13812);
        assert_eq!(los.withholding_tax.as_deref(), Some("184.80"));
    }
}
