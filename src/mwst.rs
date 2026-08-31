//! MWST-Abrechnung: Zahlen beschaffen und die eCH-0217-Deklaration bauen.
//!
//! Zwei Wege führen zum Umsatz der Ziff. 200:
//!
//! * **vereinbart** (`formOfReporting = 1`, gesetzlicher Regelfall nach Art. 39
//!   Abs. 1 MWSTG) — massgebend ist das Rechnungsdatum, also der Erlös aus der
//!   Buchhaltung (z. B. GnuCash-Konto «Erlös Kunden»). Der Betrag kommt von
//!   aussen herein.
//! * **vereinnahmt** (`formOfReporting = 2`, nur mit ESTV-Bewilligung nach
//!   Art. 39 Abs. 2) — massgebend ist der Zahlungseingang. Den liest
//!   [`credits_from_statement`] aus dem MT940-Kontoauszug.
//!
//! In beiden Fällen dient der Kontoauszug als **Gegenprobe**: die Differenz
//! zwischen Erlös und Zahlungseingang ist die Debitorenverschiebung über den
//! Stichtag. Nicht-Umsatz-Gutschriften (Dividenden, Wertschriftenabrechnungen,
//! Eigenüberträge) werden dabei ausgeschieden — Dividenden gehören nicht in
//! Ziff. 200, sondern in Ziff. 910 (`otherFlowsOfFunds/donations`).

use crate::model_mwst::*;
use crate::mt940::{category, Statement, Transaction};

// --------------------------------------------------------------------------- //
// Gutschriften klassifizieren
// --------------------------------------------------------------------------- //

/// MWST-Einordnung einer Gutschrift auf dem Kontoauszug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditClass {
    /// Kundenzahlung → Entgelt, gehört in Ziff. 200.
    Turnover,
    /// Dividende/Zins → kein Entgelt, Ziff. 910 (Art. 18 Abs. 2 Bst. d–l).
    Dividend,
    /// Weder das eine noch das andere; der Grund steht dabei.
    NotTurnover(&'static str),
}

impl CreditClass {
    pub fn label(self) -> &'static str {
        match self {
            CreditClass::Turnover => "Entgelt (Ziff. 200)",
            CreditClass::Dividend => "Dividende/Zins (Ziff. 910)",
            CreditClass::NotTurnover(r) => r,
        }
    }
}

/// Ordnet eine **Gutschrift** MWST-technisch ein.
///
/// Grundlage sind der MT940-Transaktionscode (`:61:`, z. B. `NDIV` für
/// Dividenden, `NSEC` für Wertschriftenabrechnungen) und die Heuristik aus
/// [`crate::mt940::category`]. Belastungen sind nie Entgelt.
pub fn classify_credit(tx: &Transaction) -> CreditClass {
    if !tx.credit {
        return CreditClass::NotTurnover("Belastung");
    }
    match tx.kind.as_str() {
        // Dividenden und Zinsen: kein Leistungsverhältnis, also kein Entgelt.
        "NDIV" | "NINT" => return CreditClass::Dividend,
        // Wertschriften-Abrechnungen (Fraktionen, Nennwertänderungen, Verkäufe).
        "NSEC" | "NCOL" => return CreditClass::NotTurnover("Wertschriften (kein Entgelt)"),
        _ => {}
    }
    match category(tx) {
        "Finanzertrag (Dividenden/Zinsen)" => CreditClass::Dividend,
        "Eigenübertrag (kein Ertrag)" => CreditClass::NotTurnover("Eigenübertrag"),
        "Rückerstattung (kein Ertrag)" => CreditClass::NotTurnover("Rückerstattung"),
        "Lohn/Erwerbseinkommen (Lohnausweis)" => CreditClass::NotTurnover("Lohn (kein Entgelt)"),
        _ => CreditClass::Turnover,
    }
}

/// Eine einzelne Gutschrift mit ihrer MWST-Einordnung.
#[derive(Debug, Clone)]
pub struct CreditLine {
    pub date: String,
    pub amount_cents: i64,
    pub kind: String,
    pub description: String,
    pub class: CreditClass,
}

/// Aus dem Kontoauszug abgeleitete Zahlen für die MWST-Abrechnung.
#[derive(Debug, Default, Clone)]
pub struct StatementCredits {
    /// Kundenzahlungen = Entgelte auf Ist-Basis (Ziff. 200 bei «vereinnahmt»).
    pub turnover_cents: i64,
    /// Dividenden/Zinsen (Ziff. 910).
    pub dividends_cents: i64,
    /// Übrige Gutschriften, die weder Entgelt noch Ziff. 910 sind.
    pub other_cents: i64,
    /// Alle Gutschriften einzeln, zur Durchsicht.
    pub lines: Vec<CreditLine>,
    /// Erste und letzte Valuta im Auszug (ISO), als Vorschlag für die Periode.
    pub period: Option<(String, String)>,
}

/// Liest alle **Gutschriften** eines MT940-Auszugs und ordnet sie MWST-technisch ein.
pub fn credits_from_statement(stmt: &Statement) -> StatementCredits {
    let mut c = StatementCredits::default();
    for tx in stmt.transactions.iter().filter(|t| t.credit) {
        let class = classify_credit(tx);
        match class {
            CreditClass::Turnover => c.turnover_cents += tx.amount_cents,
            CreditClass::Dividend => c.dividends_cents += tx.amount_cents,
            CreditClass::NotTurnover(_) => c.other_cents += tx.amount_cents,
        }
        c.lines.push(CreditLine {
            date: tx.value_date.clone(),
            amount_cents: tx.amount_cents,
            kind: tx.kind.clone(),
            description: tx.description.clone(),
            class,
        });
    }
    // Periode bevorzugt aus den :60F:/:62F:-Salden, sonst aus den Valuten.
    c.period = match (&stmt.opening, &stmt.closing) {
        (Some(o), Some(cl)) => Some((o.date.clone(), cl.date.clone())),
        _ => {
            let mut dates: Vec<&str> =
                stmt.transactions.iter().map(|t| t.value_date.as_str()).collect();
            dates.sort_unstable();
            match (dates.first(), dates.last()) {
                (Some(a), Some(b)) => Some((a.to_string(), b.to_string())),
                _ => None,
            }
        }
    };
    c
}

// --------------------------------------------------------------------------- //
// Perioden
// --------------------------------------------------------------------------- //

/// Semester `n` (1 oder 2) des Jahres `year` als ISO-Datumspaar.
pub fn semester(year: i32, n: u8) -> Result<(String, String), String> {
    match n {
        1 => Ok((format!("{year}-01-01"), format!("{year}-06-30"))),
        2 => Ok((format!("{year}-07-01"), format!("{year}-12-31"))),
        _ => Err("Semester muss 1 oder 2 sein".into()),
    }
}

/// Quartal `n` (1..4) des Jahres `year` als ISO-Datumspaar.
pub fn quarter(year: i32, n: u8) -> Result<(String, String), String> {
    match n {
        1 => Ok((format!("{year}-01-01"), format!("{year}-03-31"))),
        2 => Ok((format!("{year}-04-01"), format!("{year}-06-30"))),
        3 => Ok((format!("{year}-07-01"), format!("{year}-09-30"))),
        4 => Ok((format!("{year}-10-01"), format!("{year}-12-31"))),
        _ => Err("Quartal muss 1..4 sein".into()),
    }
}

/// `"2026-01-01:2026-06-30"`, `"S1/2026"`, `"Q2/2026"` → ISO-Datumspaar.
pub fn parse_period(s: &str) -> Result<(String, String), String> {
    if let Some((a, b)) = s.split_once(':') {
        let (a, b) = (a.trim(), b.trim());
        if !is_iso_date(a) || !is_iso_date(b) {
            return Err(format!("Periode {s:?}: Daten müssen als JJJJ-MM-TT vorliegen"));
        }
        if a > b {
            return Err(format!("Periode {s:?}: Beginn liegt nach dem Ende"));
        }
        return Ok((a.to_string(), b.to_string()));
    }
    let up = s.trim().to_uppercase();
    if let Some(rest) = up.strip_prefix('S').or_else(|| up.strip_prefix('H')) {
        let (n, year) = split_n_year(rest)?;
        return semester(year, n);
    }
    if let Some(rest) = up.strip_prefix('Q') {
        let (n, year) = split_n_year(rest)?;
        return quarter(year, n);
    }
    Err(format!(
        "Periode {s:?} nicht verstanden — erwartet «JJJJ-MM-TT:JJJJ-MM-TT», «S1/2026» oder «Q2/2026»"
    ))
}

fn split_n_year(rest: &str) -> Result<(u8, i32), String> {
    let (n, year) = rest
        .split_once(['/', '-'])
        .ok_or_else(|| format!("erwartet «1/2026», bekam {rest:?}"))?;
    let n: u8 = n.trim().parse().map_err(|_| format!("ungültige Nummer: {n:?}"))?;
    let year: i32 = year.trim().parse().map_err(|_| format!("ungültiges Jahr: {year:?}"))?;
    Ok((n, year))
}

fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..].iter().all(u8::is_ascii_digit)
}

/// Bezeichnet die Periode wie die ESTV: «1. Semester 2026», «2. Quartal 2026»
/// oder sonst «01.01.2026 – 30.06.2026».
pub fn period_label(from: &str, till: &str) -> String {
    let de = |iso: &str| {
        if is_iso_date(iso) {
            format!("{}.{}.{}", &iso[8..10], &iso[5..7], &iso[0..4])
        } else {
            iso.to_string()
        }
    };
    if !is_iso_date(from) || !is_iso_date(till) || from[0..4] != till[0..4] {
        return format!("{} – {}", de(from), de(till));
    }
    let year = &from[0..4];
    match (&from[5..10], &till[5..10]) {
        ("01-01", "06-30") => format!("1. Semester {year}"),
        ("07-01", "12-31") => format!("2. Semester {year}"),
        ("01-01", "03-31") => format!("1. Quartal {year}"),
        ("04-01", "06-30") => format!("2. Quartal {year}"),
        ("07-01", "09-30") => format!("3. Quartal {year}"),
        ("10-01", "12-31") => format!("4. Quartal {year}"),
        ("01-01", "12-31") => format!("Jahr {year}"),
        _ => format!("{} – {}", de(from), de(till)),
    }
}

/// Jetzt als `xs:dateTime` in UTC, z. B. `2026-08-31T10:04:00Z` — für
/// `generationTime`. Eigene Zivilkalender-Rechnung (Howard Hinnants
/// `civil_from_days`), damit keine Datums-Abhängigkeit nötig ist.
pub fn now_utc_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = civil_from_days(days);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Tage seit 1970-01-01 → (Jahr, Monat, Tag) im proleptisch gregorianischen Kalender.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// --------------------------------------------------------------------------- //
// Deklaration bauen
// --------------------------------------------------------------------------- //

/// Abrechnungsmethode — bestimmt im XML den Namen des `xs:choice`-Elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Saldo-/Pauschalsteuersatz (`simpleTaxRateMethod`, Perioden ab 01.01.2025).
    Saldosteuersatz,
    /// Effektive Methode (`effectiveReportingMethod`).
    Effektiv,
}

/// Fehlermeldung, wenn der bewilligte Tätigkeitscode fehlt. Geraten wird er nie —
/// die ESTV weist eine Abrechnung mit unbewilligter `activityID` zurück
/// («Die übermittelten Tätigkeiten entsprechen nicht der Bewilligung.»).
const ACTIVITY_ID_MISSING: &str =
    "Saldosteuersatz braucht die 5-stellige activityID (Tätigkeitscode). Sie steht in der \
     ESTV-Applikation «MWST abrechnen» unter «Abrechnungsmodalitäten» bzw. auf den \
     Subformularen; die ESTV stellt die bewilligten Codes auf Verlangen schriftlich zu. \
     Angabe mit --activity-id, bei mehreren Tätigkeiten mit --position CODE:SATZ:UMSATZ \
     (mehrfach), oder in settings.json unter mwst.activityId.";

/// Eine Zeile `suppliesPerTaxRate` — bei Saldosteuersatz eine **bewilligte
/// Tätigkeit** mit ihrem Satz und ihrem Umsatzanteil, bei der effektiven Methode
/// ein gesetzlicher Steuersatz mit dem darauf entfallenden Umsatz.
#[derive(Debug, Clone)]
pub struct Position {
    /// 5-stelliger Tätigkeitscode; bei der effektiven Methode `None`.
    pub activity_id: Option<String>,
    pub tax_rate: Percent,
    pub turnover: Amount,
}

impl Position {
    /// `"12345:6.2:80000.00"` (Saldosteuersatz) oder `"8.1:80000.00"` (effektiv).
    pub fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split(':').map(str::trim).collect();
        match parts.as_slice() {
            [id, rate, turnover] => Ok(Position {
                activity_id: Some((*id).to_string()),
                tax_rate: Percent::parse(rate)?,
                turnover: Amount::parse_chf(turnover)?,
            }),
            [rate, turnover] => Ok(Position {
                activity_id: None,
                tax_rate: Percent::parse(rate)?,
                turnover: Amount::parse_chf(turnover)?,
            }),
            _ => Err(format!(
                "Position {s:?} nicht verstanden — erwartet «CODE:SATZ:UMSATZ» \
                 (Saldosteuersatz) oder «SATZ:UMSATZ» (effektiv)"
            )),
        }
    }
}

/// Alle Eingaben für eine Abrechnung an einem Ort.
#[derive(Debug, Clone)]
pub struct Params {
    /// Zeilen der Steuerberechnung. Leer = der ganze steuerbare Umsatz geht auf
    /// `activity_id`/`tax_rate`; gefüllt = eine Zeile je Tätigkeit bzw. Steuersatz.
    pub positions: Vec<Position>,
    /// UID im Format `CHE123456789` (ohne Punkte, ohne «MWST»).
    pub uid: String,
    pub organisation_name: String,
    pub period_from: String,
    pub period_till: String,
    /// 1 = Ersteinreichung, 2 = Korrekturabrechnung, 3 = Jahresabstimmung.
    pub type_of_submission: u8,
    /// 1 = vereinbart, 2 = vereinnahmt.
    pub form_of_reporting: u8,
    pub method: Method,
    /// Ziff. 200 — Total der Entgelte. Bei Saldosteuersatz **brutto** (inkl. MWST).
    pub total_consideration: Amount,
    /// Ziff. 220 — von der Steuer befreite Leistungen (Exporte).
    pub supplies_to_foreign_countries: Option<Amount>,
    /// Ziff. 221 — Leistungen im Ausland.
    pub supplies_abroad: Option<Amount>,
    /// Ziff. 230 — von der Steuer ausgenommene Inlandleistungen.
    pub supplies_exempt_from_tax: Option<Amount>,
    /// Ziff. 235 — Entgeltsminderungen.
    pub reduction_of_consideration: Option<Amount>,
    /// Saldosteuersatz: 5-stelliger Tätigkeitscode aus «Abrechnungsmodalitäten».
    pub activity_id: Option<String>,
    /// Steuersatz: Saldosteuersatz bzw. gesetzlicher Satz bei effektiver Methode.
    pub tax_rate: Percent,
    /// Effektive Methode: 1 = Umsätze netto, 2 = brutto.
    pub gross_or_net: u8,
    /// Ziff. 400 — Vorsteuer auf Material- und Dienstleistungsaufwand.
    pub input_tax_material_and_services: Option<Amount>,
    /// Ziff. 405 — Vorsteuer auf Investitionen und übrigem Betriebsaufwand.
    pub input_tax_investments: Option<Amount>,
    /// Ziff. 910 — Spenden, Dividenden, Schadenersatz.
    pub donations: Option<Amount>,
    /// Ziff. 900 — Subventionen.
    pub subsidies: Option<Amount>,
    /// Freie Geschäftsreferenz (1..50 Zeichen).
    pub business_reference_id: String,
    /// Erstellungszeitpunkt `xs:dateTime`.
    pub generation_time: String,
    /// Hersteller für `sendingApplication`; leer = neutraler Default "taxtsueri".
    pub manufacturer: Option<String>,
    pub rounding: Rounding,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            positions: Vec::new(),
            uid: String::new(),
            organisation_name: String::new(),
            period_from: String::new(),
            period_till: String::new(),
            type_of_submission: 1,
            // Art. 39 Abs. 1 MWSTG: nach vereinbarten Entgelten ist der Regelfall;
            // «vereinnahmt» braucht eine Bewilligung der ESTV (Abs. 2).
            form_of_reporting: 1,
            method: Method::Saldosteuersatz,
            total_consideration: Amount(0),
            supplies_to_foreign_countries: None,
            supplies_abroad: None,
            supplies_exempt_from_tax: None,
            reduction_of_consideration: None,
            activity_id: None,
            tax_rate: Percent(0),
            gross_or_net: 1,
            input_tax_material_and_services: None,
            input_tax_investments: None,
            donations: None,
            subsidies: None,
            business_reference_id: String::new(),
            generation_time: String::new(),
            manufacturer: None,
            // Die ESTV rundet Ziff. 500 heute kaufmännisch auf Rappen (belegt am
            // Abrechnungsbeleg S1/2026); die 5-Rappen-Variante aus Kap. 7.5 ist zwar
            // weiterhin zulässig, weicht aber vom Portal ab.
            rounding: Rounding::Rappen,
        }
    }
}

/// Baut die eCH-0217-Deklaration und berechnet `payableTax` selbst.
///
/// Der steuerbare Gesamtumsatz (Ziff. 299 = Ziff. 200 − Ziff. 289) wandert
/// vollständig in **eine** Zeile `suppliesPerTaxRate`. Wer mehrere Tätigkeiten
/// oder Steuersätze hat, ergänzt die Zeilen danach von Hand — die ESTV prüft
/// nur, dass ihre Summe wieder Ziff. 299 ergibt (Kap. 7.5, MWST-0005).
pub fn build(p: &Params) -> Result<Document, String> {
    let mut doc = Document::default();

    doc.general_information = GeneralInformation {
        uid: normalise_uid(&p.uid),
        organisation_name: p.organisation_name.clone(),
        generation_time: p.generation_time.clone(),
        reporting_period_from: p.period_from.clone(),
        reporting_period_till: p.period_till.clone(),
        type_of_submission: p.type_of_submission,
        form_of_reporting: p.form_of_reporting,
        business_reference_id: if p.business_reference_id.is_empty() {
            format!("taxtsueri-{}-{}", p.period_from, p.period_till)
        } else {
            p.business_reference_id.clone()
        },
        sending_application: SendingApplication {
            manufacturer: p
                .manufacturer
                .clone()
                .unwrap_or_else(|| SendingApplication::default().manufacturer),
            ..SendingApplication::default()
        },
    };

    doc.turnover_computation = TurnoverComputation {
        total_consideration: p.total_consideration,
        supplies_to_foreign_countries: p.supplies_to_foreign_countries,
        supplies_abroad: p.supplies_abroad,
        transfer_notification_procedure: None,
        supplies_exempt_from_tax: p.supplies_exempt_from_tax,
        reduction_of_consideration: p.reduction_of_consideration,
        various_deduction: None,
    };

    let taxable = doc.turnover_computation.taxable_turnover();

    match p.method {
        Method::Saldosteuersatz => {
            // Mehrere bewilligte Tätigkeiten → je eine Zeile; sonst geht der ganze
            // steuerbare Umsatz auf die eine Tätigkeit.
            let rows: Vec<ActivityTurnoverTaxRate> = if p.positions.is_empty() {
                let id = p.activity_id.clone().ok_or_else(|| ACTIVITY_ID_MISSING.to_string())?;
                vec![ActivityTurnoverTaxRate {
                    activity_id: id,
                    tax_rate: p.tax_rate,
                    turnover: taxable,
                }]
            } else {
                p.positions
                    .iter()
                    .map(|pos| {
                        Ok(ActivityTurnoverTaxRate {
                            activity_id: pos
                                .activity_id
                                .clone()
                                .ok_or_else(|| ACTIVITY_ID_MISSING.to_string())?,
                            tax_rate: pos.tax_rate,
                            turnover: pos.turnover,
                        })
                    })
                    .collect::<Result<_, String>>()?
            };
            for r in &rows {
                if r.activity_id.chars().count() != 5 {
                    return Err(format!(
                        "activityID {:?} muss genau 5 Zeichen haben",
                        r.activity_id
                    ));
                }
            }
            doc.simple_tax_rate_method = Some(SimpleTaxRateMethod {
                supplies_per_tax_rate: rows,
                ..Default::default()
            });
        }
        Method::Effektiv => {
            let rows: Vec<TurnoverTaxRate> = if p.positions.is_empty() {
                vec![TurnoverTaxRate { tax_rate: p.tax_rate, turnover: taxable }]
            } else {
                p.positions
                    .iter()
                    .map(|pos| TurnoverTaxRate { tax_rate: pos.tax_rate, turnover: pos.turnover })
                    .collect()
            };
            doc.effective_reporting_method = Some(EffectiveReportingMethod {
                gross_or_net: p.gross_or_net,
                supplies_per_tax_rate: rows,
                input_tax_material_and_services: p.input_tax_material_and_services,
                input_tax_investments: p.input_tax_investments,
                ..Default::default()
            });
        }
    }

    if p.donations.is_some() || p.subsidies.is_some() {
        doc.other_flows_of_funds = Some(OtherFlowsOfFunds {
            subsidies: p.subsidies,
            donations: p.donations,
        });
    }

    doc.payable_tax = doc.compute_payable_tax(p.rounding);
    Ok(doc)
}

/// «CHE-123.456.789 MWST» → `CHE123456789` (das XSD-Muster `CHE[1-9][0-9]{8}`).
pub fn normalise_uid(s: &str) -> String {
    let up = s.trim().to_uppercase();
    let digits: String = up.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 9 {
        format!("CHE{}", &digits[..9])
    } else {
        up.replace(['-', '.', ' '], "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_calendar_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_601), (2023, 9, 1));
        // Schalttag und Jahrhundert-Regel (2000 ist ein Schaltjahr, 1900 nicht).
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(-25_567), (1900, 1, 1));
        // 2026-08-31: 20'696 Tage nach dem Epoch-Beginn.
        assert_eq!(civil_from_days(20_696), (2026, 8, 31));
        let now = now_utc_iso();
        assert!(is_iso_date(&now[..10]) && now.ends_with('Z') && now.len() == 20, "{now}");
    }

    #[test]
    fn uid_normalisation() {
        assert_eq!(normalise_uid("CHE-123.456.789 MWST"), "CHE123456789");
        assert_eq!(normalise_uid("che 123 456 789"), "CHE123456789");
        assert_eq!(normalise_uid("CHE123456789"), "CHE123456789");
    }

    #[test]
    fn period_parsing_and_labels() {
        assert_eq!(
            parse_period("S1/2026").unwrap(),
            ("2026-01-01".into(), "2026-06-30".into())
        );
        assert_eq!(
            parse_period("Q3/2026").unwrap(),
            ("2026-07-01".into(), "2026-09-30".into())
        );
        assert_eq!(
            parse_period("2026-02-01:2026-02-28").unwrap(),
            ("2026-02-01".into(), "2026-02-28".into())
        );
        assert!(parse_period("2026-06-30:2026-01-01").is_err());
        assert_eq!(period_label("2026-01-01", "2026-06-30"), "1. Semester 2026");
        assert_eq!(period_label("2026-07-01", "2026-12-31"), "2. Semester 2026");
        assert_eq!(period_label("2026-01-01", "2026-12-31"), "Jahr 2026");
        assert_eq!(period_label("2026-02-01", "2026-02-28"), "01.02.2026 – 28.02.2026");
    }

    fn credit(kind: &str, booking: &str, desc: &str, cents: i64) -> Transaction {
        Transaction {
            value_date: "2026-01-06".into(),
            credit: true,
            amount_cents: cents,
            kind: kind.into(),
            booking_type: booking.into(),
            description: desc.into(),
        }
    }

    #[test]
    fn dividends_and_securities_are_not_turnover() {
        let d = credit("NDIV", "Dividende", "W05?VN 3886335 NESTLE N", 25_790);
        assert_eq!(classify_credit(&d), CreditClass::Dividend);

        let s = credit("NSEC", "Fraktions-Abrechnung", "E03?VN 112574 WATERS RG", 17_555);
        assert!(matches!(classify_credit(&s), CreditClass::NotTurnover(_)));

        let k = credit("NTRF", "e-banking-Gutschrift", "Z46?KUNDE AG RECHNUNG 13325", 43_240);
        assert_eq!(classify_credit(&k), CreditClass::Turnover);
    }

    #[test]
    fn statement_credits_split_by_class() {
        let stmt = Statement {
            account: "CH87".into(),
            opening: None,
            closing: None,
            transactions: vec![
                credit("NTRF", "Gutschrift", "Z40?KUNDE AG RG 13321", 1_621_500),
                credit("NDIV", "Dividende", "W05?NESTLE N", 25_790),
                credit("NSEC", "Nennwertaenderung", "E03?ROCHE", 400),
                Transaction {
                    value_date: "2026-01-06".into(),
                    credit: false,
                    amount_cents: 100_000,
                    kind: "NTRF".into(),
                    booking_type: "e-banking-Auftrag".into(),
                    description: "Z44?LIEFERANT".into(),
                },
            ],
        };
        let c = credits_from_statement(&stmt);
        assert_eq!(c.turnover_cents, 1_621_500);
        assert_eq!(c.dividends_cents, 25_790);
        assert_eq!(c.other_cents, 400);
        assert_eq!(c.lines.len(), 3, "Belastungen bleiben draussen");
    }

    #[test]
    fn build_saldosteuersatz_declaration() {
        let p = Params {
            uid: "CHE-123.456.789 MWST".into(),
            organisation_name: "Beispiel GmbH".into(),
            period_from: "2026-01-01".into(),
            period_till: "2026-06-30".into(),
            method: Method::Saldosteuersatz,
            total_consideration: Amount(12_345_678),
            activity_id: Some("00001".into()),
            tax_rate: Percent(620),
            generation_time: "2026-08-31T10:04:00Z".into(),
            ..Default::default()
        };
        let doc = build(&p).unwrap();
        assert_eq!(doc.general_information.uid, "CHE123456789");
        assert_eq!(doc.turnover_computation.taxable_turnover(), Amount(12_345_678));
        // 123'456.78 × 6.2 % = 7'654.320'36 → Ziff. 399 = Ziff. 500 = 7'654.32
        // (kaufmännisch auf Rappen, wie das ESTV-Portal rechnet).
        assert_eq!(doc.total_tax_due(), Amount(765_432));
        assert_eq!(doc.payable_tax, Amount(765_432));
        // Mit der ebenfalls zulässigen 5-Rappen-Rundung abwärts: 7'654.30.
        let alt = build(&Params { rounding: Rounding::FiveRappen, ..p.clone() }).unwrap();
        assert_eq!(alt.payable_tax, Amount(765_430));
        assert!(doc.validate().is_empty(), "{:?}", doc.validate());
    }

    #[test]
    fn saldosteuersatz_without_activity_id_is_refused() {
        let p = Params {
            uid: "CHE123456789".into(),
            organisation_name: "Beispiel GmbH".into(),
            period_from: "2026-01-01".into(),
            period_till: "2026-06-30".into(),
            total_consideration: Amount(12_345_678),
            tax_rate: Percent(620),
            ..Default::default()
        };
        let err = build(&p).unwrap_err();
        assert!(err.contains("activityID"), "{err}");
    }

    #[test]
    fn position_parsing() {
        let p = Position::parse("12345:6.2:80000.00").unwrap();
        assert_eq!(p.activity_id.as_deref(), Some("12345"));
        assert_eq!(p.tax_rate, Percent(620));
        assert_eq!(p.turnover, Amount(8_000_000));
        // Zweiteilig = effektive Methode, ohne Tätigkeitscode.
        let e = Position::parse("8.1:80000").unwrap();
        assert_eq!(e.activity_id, None);
        assert_eq!(e.tax_rate, Percent(810));
        assert!(Position::parse("nur-eins").is_err());
    }

    /// Zwei bewilligte Tätigkeiten mit verschiedenen Saldosteuersätzen — der Fall,
    /// den die ESTV mit «Die übermittelten Tätigkeiten entsprechen nicht der
    /// Bewilligung.» zurückweist, wenn er auf eine Zeile zusammengefasst wird.
    #[test]
    fn two_activities_split_the_turnover() {
        let base = Params {
            uid: "CHE123456789".into(),
            organisation_name: "Beispiel GmbH".into(),
            period_from: "2026-01-01".into(),
            period_till: "2026-06-30".into(),
            total_consideration: Amount(12_345_678),
            ..Default::default()
        };
        let doc = build(&Params {
            positions: vec![
                Position::parse("12345:6.2:100000.00").unwrap(),
                Position::parse("54321:1.2:23456.78").unwrap(),
            ],
            ..base.clone()
        })
        .expect("zwei Tätigkeiten");
        let rows = &doc.simple_tax_rate_method.as_ref().unwrap().supplies_per_tax_rate;
        assert_eq!(rows.len(), 2);
        assert_eq!(doc.supplies_total(), Amount(12_345_678));
        // 100'000.00 × 6.2 % = 6'200.00; 23'456.78 × 1.2 % = 281.481'36 → zusammen
        // 6'481.481'36 → Ziff. 399 = Ziff. 500 = 6'481.48.
        assert_eq!(doc.total_tax_due(), Amount(648_148));
        assert_eq!(doc.payable_tax, Amount(648_148));
        assert!(doc.validate().is_empty(), "{:?}", doc.validate());

        // Stimmt die Aufteilung nicht mit Ziff. 299 überein, greift MWST-0005.
        let bad = build(&Params {
            positions: vec![Position::parse("12345:6.2:100000.00").unwrap()],
            ..base
        })
        .expect("baut trotzdem");
        assert!(bad.validate().iter().any(|e| e.contains("MWST-0005")), "{:?}", bad.validate());
    }

    #[test]
    fn deductions_reduce_the_taxed_turnover() {
        let p = Params {
            uid: "CHE123456789".into(),
            organisation_name: "Beispiel GmbH".into(),
            period_from: "2026-01-01".into(),
            period_till: "2026-06-30".into(),
            total_consideration: Amount(12_345_678),
            supplies_to_foreign_countries: Some(Amount(1_000_000)),
            activity_id: Some("00001".into()),
            tax_rate: Percent(620),
            ..Default::default()
        };
        let doc = build(&p).unwrap();
        assert_eq!(doc.turnover_computation.total_deductions(), Amount(1_000_000));
        assert_eq!(doc.turnover_computation.taxable_turnover(), Amount(11_345_678));
        assert_eq!(doc.supplies_total(), Amount(11_345_678));
        assert!(doc.validate().is_empty(), "{:?}", doc.validate());
    }
}
