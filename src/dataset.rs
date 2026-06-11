//! Konkreter Datensatz aus `pdf/Steuererklärung 2025_privat.pdf`
//! (Dr. Tax ZH NP 2025, Zeno Davatz, Stadt Zürich). Die AHVN13 steht nicht im
//! Code, sondern in `settings.json` (gitignored) und wird beim Lauf gesetzt.
//!
//! Die Quell-PDF enthält Personendaten und ist via `.gitignore` vom Commit
//! ausgeschlossen. Dieser Datensatz dient als Beispiel-Eingabe, bis ein Parser
//! (PDF / eCH-0196-Barcode) die Werte automatisch einliest.

use crate::model::*;

/// Eingebauter Beispiel-Datensatz (Steuerperiode 2025, Stadt Zürich), wenn keine
/// JSON-Eingabe vorhanden ist. Wird via `Document::into_message()` zu eCH-0119-XML.
pub fn example() -> Document {
    let header = Header {
        tax_period: 2025,
        period_from: Some("2025-01-01".into()),
        period_to: Some("2025-12-31".into()),
        canton: Some("ZH".into()),
        source: 0, // 0 = Software
    };

    // Hilfsfunktion: Schweizer Postadresse.
    let ch_address = |street: Option<&str>, house: Option<&str>, zip: u32, town: &str| {
        AddressInformation {
            postal_address: MailAddress {
                address_information: PostalAddressInfo {
                    street: street.map(Into::into),
                    house_number: house.map(Into::into),
                    town: town.into(),
                    swiss_zip_code: Some(zip),
                    country: Some(Country {
                        iso2: Some("CH".into()),
                        name_short: Some("Schweiz".into()),
                    }),
                },
            },
        }
    };

    let representative = RepresentativePerson {
        address: Some(ch_address(Some("Postfach"), None, 8034, "Zürich")),
        organisation_name: Some("Aeberli Treuhand AG".into()),
        phone_number: Some("0442656666".into()), // eCH: nur Ziffern, 10–20
        uid: Some(Uid {
            categorie: "CHE".into(),
            id: 101_652_973, // CHE-101.652.973
        }),
    };

    // Stadt Zürich, BFS-Gemeindenummer 261, Kanton ZH.
    let stadt_zuerich = SwissMunicipality {
        municipality_id: 261,
        municipality_name: "Zürich".into(),
        canton: "ZH".into(),
    };

    let partner1 = PersonDataPartner1 {
        identification: PartnerPersonIdentification {
            official_name: "Davatz".into(),
            first_name: "Zeno Ramun Raphael".into(),
            sex: Some("1".into()), // männlich
            date_of_birth: Some(DateYearMonthDay {
                year_month_day: "1975-05-20".into(),
            }),
            // Platzhalter (gültiger AHVN13-Bereich); echte Nr. kommt aus settings.json.
            vn: 7_560_000_000_001,
        },
        address: Some(ch_address(Some("Winterthurerstrasse"), Some("52"), 8006, "Zürich")),
        // Zivilstand «getrennt» = verheiratet (2) mit tatsächlicher Trennung (2).
        marital_status_tax: Some(MaritalStatusTax {
            marital_status: Some("2".into()),
            separation_data: Some(SeparationData {
                separation: Some("2".into()),
            }),
        }),
        // Konfession «andere»: eCH-0011-Religionscode unbekannt → bewusst leer.
        religion: None,
        job: Some("Unternehmer".into()),
        payment_pension: Some(true),
        tax_municipality: Some(stadt_zuerich),
    };

    let children = vec![
        ChildData {
            identification: Ech0044PersonId {
                official_name: "Davatz".into(),
                first_name: "Ayano Julius".into(),
                date_of_birth: Some(Ech0044Date {
                    year_month_day: "2012-04-15".into(),
                }),
            },
            home_or_external: Some(true), // ausserhalb des Haushalts
            correct_to: Some("2030-01-01".into()),
        },
        ChildData {
            identification: Ech0044PersonId {
                official_name: "Davatz".into(),
                first_name: "Lionel Masato".into(),
                date_of_birth: Some(Ech0044Date {
                    year_month_day: "2010-06-01".into(),
                }),
            },
            home_or_external: Some(true),
            correct_to: Some("2030-01-01".into()),
        },
    ];

    // Einkünfte (Seite 2).
    let revenue = Revenue {
        employed_main: Some(PartnerAmount::p1(107_935)),      // Ziffer 100
        employed_sideline: Some(PartnerAmount::p1(2_244)),    // Ziffer 102
        selfemployed_sideline: Some(PartnerAmount::p1(9_894)),// Ziffer 122
        securities_revenue: Some(TaxAmount::new(2_292, 2_292)),// Ziffer 150
        property_notional_rental_value: Some(1_350),          // Eigenmietwert
        property_deduction_flatrate: Some(270),               // Pauschale
        property_remaining_revenue: Some(1_080),              // Ziffer 188
        total_amount_revenue: Some(TaxAmount::new(123_445, 123_445)), // Ziffer 199
    };

    // Abzüge (Seite 3). TaxAmount = (Staat, Bund).
    let deduction = Deduction {
        job_expenses_p1: Some(TaxAmount::new(4_039, 4_039)),          // Ziffer 220
        payment_alimony_child: Some(TaxAmount::new(12_000, 12_000)),  // Ziffer 255
        insurance_and_interest: Some(TaxAmount::new(2_900, 1_800)),   // Ziffer 270
        financial_management: Some(TaxAmount::new(724, 724)),         // Ziffer 283
        nonparental_supervision: Some(TaxAmount::new(648, 648)),      // Ziffer 376
        total_amount_deduction: Some(TaxAmount::new(20_311, 19_211)), // Ziffer 299
        payment_pension_total: Some(2_561),                           // Ziffer 256
    };

    let revenue_calculation = RevenueCalculation {
        total_amount_revenue: Some(TaxAmount::new(123_445, 123_445)),
        total_amount_deduction: Some(TaxAmount::new(20_311, 19_211)),
        net_income: Some(TaxAmount::new(103_134, 104_234)),           // Ziffer 310
        deduction_charity: Some(TaxAmount::new(300, 300)),            // Ziffer 324
        adjusted_net_income: Some(TaxAmount::new(102_834, 103_934)),  // Ziffer 350
        total_amount_fiscal_revenue: Some(TaxAmount::new(102_834, 103_934)), // Ziffer 390
        fiscal_revenue_abroad: Some(TaxAmount::new(936, 946)),        // Ziffer 396
        resulting_fiscal_revenue: Some(TaxAmount::new(101_898, 102_988)), // Ziffer 398
    };

    // Vermögen (Seite 4).
    let asset = Asset {
        securities_and_assets: Some(FiscalValue::new(251_394)),  // Ziffer 400
        property_market_value: Some(FiscalValue::new(36_000)),   // Ziffer 421
        total_amount_assets: Some(FiscalValue::new(287_394)),    // Ziffer 460
        total_amount_liabilities: Some(FiscalValue::new(10_000)),// Ziffer 470
        total_amount_fiscal_assets: Some(FiscalValue::new(277_394)), // Ziffer 490
        fiscal_assets_abroad: Some(FiscalValue::new(34_747)),    // Ziffer 496
        resulting_fiscal_assets: Some(FiscalValue::new(242_647)),// Ziffer 498
    };

    // Wertschriften- und Guthabenverzeichnis (Seite 18).
    let mut security = Vec::new();
    // Inländische Position: Bruttoertrag in Kolonne A (verrechnungssteuerbelastet).
    let mut add_ch = |currency: Option<&str>, qty: Option<&str>, valor: Option<&str>,
                      desc: &str, tax: i64, gross_a: i64| {
        security.push(SecurityEntry {
            currency: currency.map(Into::into),
            quantity: qty.map(Into::into),
            securities_number: valor.map(Into::into),
            description: desc.into(),
            country: None,
            tax_value: Some(TaxAmount::new(tax, tax)),
            gross_revenue_a: Some(TaxAmount::new(gross_a, gross_a)),
            gross_revenue_b: None,
        });
    };
    add_ch(None, None, None, "PK UBS - CH86 0022 5225 P560 1230 0", 42_898, 0);
    add_ch(None, None, None, "JPK UBS - CH93 0022 5225 1276 3640 L (Lionel Davatz)", 593, 0);
    add_ch(None, None, None, "JSK UBS - CH10 0022 5225 1276 36M1 T (Lionel Davatz)", 2_851, 1);
    add_ch(None, None, None, "JSK UBS - CH24 0023 0230 8767 9340 U (Ayano Davatz)", 126, 0);
    add_ch(None, None, None, "Kontokorrent ywesee GmbH, Zürich", 49_504, 0);
    add_ch(None, Some("12000"), None, "Stammanteile ywesee GmbH, Zürich", 30_200, 0);
    add_ch(Some("USD"), Some("100"), Some("24476758"), "UBS Group AG, Zuerich, CH", 3_696, 37);
    add_ch(Some("CHF"), Some("65"), Some("12688156"), "Swiss Re AG, Zürich, CH", 8_635, 390);
    add_ch(Some("CHF"), Some("24"), Some("874251"), "Swisscom AG, Ittigen, CH", 13_812, 528);

    // DA-1: US-Titel mit Bruttoertrag in Kolonne B und Domizil "US".
    // (valor, Stückzahl, Bezeichnung, Steuerwert, Bruttoertrag B)
    let mut add_us = |valor: &str, qty: &str, desc: &str, tax: i64, gross_b: i64| {
        security.push(SecurityEntry {
            currency: Some("USD".into()),
            quantity: Some(qty.into()),
            securities_number: Some(valor.into()),
            description: desc.into(),
            country: Some("US".into()),
            tax_value: Some(TaxAmount::new(tax, tax)),
            gross_revenue_a: None,
            gross_revenue_b: Some(TaxAmount::new(gross_b, gross_b)),
        });
    };
    add_us("966021", "303", "Wells Fargo & Company, San Francisco, US", 22_373, 426);
    add_us("906153", "70", "American Express Company, New York, US", 20_517, 185);
    add_us("748628", "100", "Bank of America Corporation, Charlotte, US", 4_357, 88);
    add_us("10926529", "50", "Berkshire Hathaway Inc., Omaha, US", 19_911, 0);
    add_us("943981", "17", "Johnson & Johnson, New Brunswick, US", 2_787, 72);
    add_us("12915350", "21", "Citigroup Inc, New York, NY, US", 1_941, 41);
    add_us("1112258433", "8", "GE Aerospace, Fairfield, CT, US", 1_952, 9);
    add_us("919390", "308", "Coca-Cola Company (The), Atlanta, US", 17_059, 513);
    add_us("941595", "240", "Intel Corporation, Santa Clara, US", 7_016, 0);
    add_us("123736674", "2", "GE HealthCare Technologies Inc, US", 130, 0);
    add_us("1332624491", "2", "GE Vernova Inc, Delaware, US", 1_036, 2);

    let list_of_securities = ListOfSecurities {
        bank_account: Some(BankAccount {
            iban: Some("CH86 0022 5225 P560 1230 0".into()),
            bank_name: Some("UBS".into()),
            account_owner: Some("Davatz Zeno".into()),
        }),
        security_entry: security,
        attached_form_da1: Some(1), // ein DA-1-Formular (ausländische Quellensteuern)
        total_tax_value: Some(TaxAmount::new(251_394, 251_394)), // Ziffer 400
        subtotal_gross_revenue_a1: Some(TaxAmount::new(955, 955)), // → 35 % = 334.25
        subtotal_gross_revenue_b: Some(TaxAmount::new(1_336, 1_336)), // DA-1-Total B
        total_gross_revenue: Some(TaxAmount::new(2_292, 2_292)), // Ziffer 150
        withholding_tax: Some("334.25".into()), // Verrechnungssteueranspruch
    };

    let main_form = MainForm {
        representative_person: Some(representative),
        person_data_partner1: partner1,
        child_data: children,
        revenue: Some(revenue),
        deduction: Some(deduction),
        revenue_calculation: Some(revenue_calculation),
        asset: Some(asset),
    };

    // Schuldenverzeichnis (Seite 15): «Diverse Schulden / Steuerschulden» 10'000.
    let list_of_liabilities = ListOfLiabilities {
        private_liabilities: vec![Liability {
            identification: Some("Diverse Schulden / Steuerschulden".into()),
            liability: Some(10_000), // Ziffer 470
            liability_interest: None,
        }],
        business_liabilities: vec![],
        total_private_liabilities: Some(10_000),
        total_private_liabilities_interest: None,
        total_amount_liabilities: Some(10_000),
        total_amount_liabilities_interest: None,
    };

    let content = Content {
        main_form,
        list_of_securities: Some(list_of_securities),
        list_of_liabilities: Some(list_of_liabilities),
    };

    Document { header, content }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_xml() {
        let xml = quick_xml::se::to_string(&example().into_message()).expect("serialize");
        assert!(xml.contains("<vn>7560000000001</vn>")); // Platzhalter; echt via settings.json
        assert!(xml.contains("<eCH-0007f:municipalityId>261</eCH-0007f:municipalityId>"));
        assert!(xml.contains("<eCH-0011f:separation>2</eCH-0011f:separation>"));
    }

    #[test]
    fn json_roundtrips_through_document() {
        let doc = example();
        let json = serde_json::to_string(&doc).expect("to json");
        let back: Document = serde_json::from_str(&json).expect("from json");
        // Nach dem Roundtrip muss das XML identisch validieren-fähig sein.
        let a = quick_xml::se::to_string(&doc.into_message()).unwrap();
        let b = quick_xml::se::to_string(&back.into_message()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn declared_income_total_is_consistent() {
        let msg = example().into_message();
        let r = msg.content.main_form.revenue.unwrap();
        let sum = r.employed_main.unwrap().partner1
            + r.employed_sideline.unwrap().partner1
            + r.selfemployed_sideline.unwrap().partner1
            + r.securities_revenue.unwrap().cantonal
            + r.property_remaining_revenue.unwrap();
        // Ziffer 199 = Summe der Einzelpositionen (Seite 2 der Steuererklärung).
        assert_eq!(sum, r.total_amount_revenue.unwrap().cantonal);
    }

    #[test]
    fn securities_tax_value_matches_ziffer_400() {
        let msg = example().into_message();
        // Alle Positionen (inländisch + DA-1/US) ergeben den Steuerwert Ziffer 400.
        let listed: i64 = msg
            .content
            .list_of_securities
            .as_ref()
            .unwrap()
            .security_entry
            .iter()
            .map(|s| s.tax_value.unwrap().cantonal)
            .sum();
        let z400 = msg
            .content
            .main_form
            .asset
            .unwrap()
            .securities_and_assets
            .unwrap()
            .fiscal_value;
        assert_eq!(listed, z400);
    }

    #[test]
    fn da1_foreign_gross_revenue_b_totals_match() {
        let msg = example().into_message();
        let los = msg.content.list_of_securities.unwrap();
        let us_tax: i64 = los
            .security_entry
            .iter()
            .filter(|s| s.country.as_deref() == Some("US"))
            .map(|s| s.tax_value.unwrap().cantonal)
            .sum();
        let us_gross_b: i64 = los
            .security_entry
            .iter()
            .filter(|s| s.country.as_deref() == Some("US"))
            .filter_map(|s| s.gross_revenue_b.map(|g| g.cantonal))
            .sum();
        // DA-1-Übertrag: Steuerwert 99'079, Bruttoertrag B 1'336.
        assert_eq!(us_tax, 99_079);
        assert_eq!(us_gross_b, los.subtotal_gross_revenue_b.unwrap().cantonal);
        assert_eq!(us_gross_b, 1_336);
    }
}
