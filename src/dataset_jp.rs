//! Beispiel-Datensatz juristische Person (eCH-0276) aus
//! `pdf/Steuererklärung 2025_ywesee.pdf` + `pdf/Jahresrechnung 2025.pdf`
//! (ywesee GmbH, Reg.-Nr. J 000 091 119/9, Stadt Zürich).
//!
//! Beträge auf ganze Franken gerundet (eCH-0276 nutzt `xs:long`). Quell-PDFs/xls
//! sind via `.gitignore` ausgeschlossen.

use crate::model_jp::*;

/// Eingebauter JP-Beispiel-Datensatz (Steuerperiode 2025, ywesee GmbH) nach eCH-0276.
pub fn example() -> Document {
    Document {
        header: Header {
            title: Title {
                organisation_name: "ywesee GmbH".into(),
                register_number: 91119, // aus "J 000 091 119/9"
                uid: None,              // auf den Unterlagen nicht ausgewiesen
                assessment_municipality: "Zürich".into(),
                assessment_municipality_id: Some(261),
                head_office: HeadOffice {
                    street: "Winterthurerstrasse".into(),
                    house_number: "52".into(),
                    zip_code: 8006,
                    town: "Zürich".into(),
                    municipality_id: 261,
                    canton: "ZH".into(),
                },
                tax_period_from: "2025-01-01".into(),
                tax_period_to: "2025-12-31".into(),
                currency_share_equity: "CHF".into(),
                currency_financial_reporting: "CHF".into(),
                chairman_of_the_board_of_directors: None,
                management: Some("Zeno R.R. Davatz".into()),
                responsible_for_accounting: Some("Zeno R.R. Davatz".into()),
            },
        },
        content: Content {
            // Bilanz – Aktiven
            assets: Some(Assets {
                current_assets: Some(CurrentAssets {
                    cash_and_securities: Some(CashAndSecurities {
                        cash_and_cash_equivalents: Some(115_440), // Kasse 2'000 + KK UBS 113'440
                        current_assets_with_stock_market_price: Some(38_628), // Wertschriften
                    }),
                    total_current_assets: Some(154_067),
                }),
                noncurrent_assets: Some(NoncurrentAssets {
                    property_plant_equipment: Some(PropertyPlantEquipment {
                        office_equipment_it: Some(1), // EDV-Anlage
                    }),
                    total_noncurrent_assets: Some(1),
                }),
                total_assets: Some(154_068), // Bilanzsumme
            }),
            // Bilanz – Passiven + Eigenkapital
            equity_and_liabilities: Some(EquityAndLiabilities {
                current_liabilities: Some(CurrentLiabilities {
                    trade_and_other_current_payables: Some(TradeAndOtherCurrentPayables {
                        current_payables_third_parties: Some(11_959), // Kreditoren
                        current_payables_shareholders: Some(49_504),  // KK Zeno R.R. Davatz
                    }),
                    accrued_expenses: Some(5_650), // Passive Rechnungsabgrenzungen
                    total_current_liabilities: Some(67_113),
                }),
                equity: Some(Equity {
                    share_capital: 20_000, // Stammkapital
                    statutory_profit_reserve: Some(StatutoryProfitReserve {
                        amount: Some(5_000), // Gesetzliche Gewinnreserven
                    }),
                    net_profit_or_loss: Some(NetProfitOrLoss {
                        profit_or_loss_brought_forward: Some(58_197), // Gewinnvortrag
                        annual_profit_or_annual_loss: Some(3_759),    // Jahresgewinn
                    }),
                    total_equity: 86_956,
                }),
                total_equity_and_liabilities: 154_068,
            }),
            // Erfolgsrechnung
            income_statement: IncomeStatement {
                deliveries_and_services_revenue: Some(DeliveriesAndServicesRevenue {
                    service_revenue: Some(231_012), // Nettoerlöse aus Dienstleistungen
                    total: Some(231_012),
                }),
                total_operating_revenue: Some(231_012),
                expenses: Some(Expenses {
                    materials_goods_services: Some(MaterialsGoodsServices {
                        purchased_services: Some(14_327), // Einkauf Dienstleistungen
                        total: Some(14_327),
                    }),
                    employee_expenses: Some(EmployeeExpenses {
                        salary_expenses: Some(104_383),       // Lohnaufwand
                        social_security_expenses: Some(29_048), // Sozialversicherungsaufwand
                        total: Some(133_431),
                    }),
                    other_operating_expenses: Some(OtherOperatingExpenses {
                        rental_expense: Some(14_400),         // Raumaufwand
                        maintenance_repairs: Some(7_980),     // Unterhalt, Reparaturen
                        administrative_and_it: Some(1_927),   // Verwaltungs-/Beratungsaufwand
                        travel_and_representation: Some(53_741), // Reisespesen
                        miscellaneous: Some(870),             // Übriger Betriebsaufwand
                        total: Some(78_918),
                    }),
                }),
                financial_expenses_and_income: Some(FinancialExpensesAndIncome {
                    interest_expense: Some(723), // Finanzaufwand
                    financial_income: Some(893), // Finanzertrag
                    total: Some(169),
                }),
                direct_taxes: Some(747), // Steuern
                annual_profit_or_annual_loss: 3_759, // Jahresgewinn
            },
            // D. Steuerliche Korrekturen
            fiscal_corrections: Some(FiscalCorrections {
                net_profit_or_loss_actual_year: TaxAmount::both(3_759),
                tax_loss_carried_forward_list: Some(TaxLossCarriedForwardList {
                    after_offsetting_losses: TaxAmount::both(3_759),
                    switzerland_chf: TaxAmount::both(3_759),
                    canton_after_reliefs_chf: 3_759,
                }),
            }),
            // E. Gewinnverwendung
            profit_appropriation: ProfitAppropriation {
                carried_forward: Some(58_197), // Gewinnvortrag Vorjahr
                total_to_be_distributed: 61_956,
                total_appropriation: 0,
                brought_forward: 61_956, // Vortrag auf neue Rechnung
            },
            // F. Steuerbares Eigenkapital nach Gewinnverwendung
            taxable_equity_after_profit_appropriation: Some(TaxableEquityAfterProfitAppropriation {
                taxable_equity: Some(TaxableEquity {
                    paid_in_capital: Some(20_000),
                    statutory_capital_reserves: Some(5_000),
                }),
                total_taxable_equity: TaxAmount::both(86_956),
                in_switzerland_chf: TaxAmount::both(86_956),
                canton_after_reliefs: 86_956,
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_xml() {
        let xml = quick_xml::se::to_string(&example().into_message()).expect("serialize");
        assert!(xml.contains("eCH-0276:eBalanceSheetETaxLegalEntity"));
        assert!(xml.contains("<eCH-0276:organisationName>ywesee GmbH</eCH-0276:organisationName>"));
    }

    #[test]
    fn json_roundtrips() {
        let doc = example();
        let json = serde_json::to_string(&doc).unwrap();
        let back: Document = serde_json::from_str(&json).unwrap();
        let a = quick_xml::se::to_string(&doc.into_message()).unwrap();
        let b = quick_xml::se::to_string(&back.into_message()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn profit_appropriation_adds_up() {
        let pa = example().content.profit_appropriation;
        assert_eq!(pa.carried_forward.unwrap() + 3_759, pa.total_to_be_distributed);
        assert_eq!(pa.total_to_be_distributed, pa.brought_forward);
    }
}
