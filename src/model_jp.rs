//! Modell der **Steuererklärung juristische Personen** nach **eCH-0276**
//! «E-Bilanz und E-Tax JP» (V1.0.0, eCH + SSK). Offizieller, frei verfügbarer
//! Standard — das erzeugte XML **validiert gegen `schema/eCH-0276-1-0.xsd`**.
//!
//! Aufgebaut direkt nach dem XSD und dem offiziellen Beispiel
//! (`schema/eCH-0276-beispiel.xml`): `eBalanceSheetETaxLegalEntity[@minorVersion]`
//! → `header`(title) + `content` (assets, equityAndLiabilities, incomeStatement,
//! fiscalCorrections, profitAppropriation, taxableEquityAfterProfitAppropriation).
//! `incomeStatement` und `profitAppropriation` sind Pflicht.
//!
//! Namespaces: anders als eCH-0119 trägt hier **jedes** Element den Präfix
//! `eCH-0276:` (so das offizielle Beispiel). Beträge sind `xs:long` (ganze CHF).
//! Modelliert wird der für ywesee benötigte Ausschnitt in `xs:sequence`-Reihenfolge.

use serde::{Deserialize, Serialize};

pub type Chf = i64;

/// `taxAmountType` — Aufteilung Staatssteuer / direkte Bundessteuer.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TaxAmount {
    #[serde(rename = "eCH-0276:cantonalTax", skip_serializing_if = "Option::is_none")]
    pub cantonal: Option<Chf>,
    #[serde(rename = "eCH-0276:federalTax", skip_serializing_if = "Option::is_none")]
    pub federal: Option<Chf>,
}

impl TaxAmount {
    pub fn both(v: Chf) -> Self {
        Self { cantonal: Some(v), federal: Some(v) }
    }
}

// --------------------------------------------------------------------------- //
// JSON-Eingabe + XML-Wurzel
// --------------------------------------------------------------------------- //

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Document {
    pub header: Header,
    pub content: Content,
}

impl Document {
    pub fn into_message(self) -> Message {
        Message::new(self)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename = "eCH-0276:eBalanceSheetETaxLegalEntity")]
pub struct Message {
    #[serde(rename = "@xmlns:eCH-0276")]
    xmlns: &'static str,
    #[serde(rename = "@minorVersion")]
    minor_version: u8,
    #[serde(rename = "eCH-0276:header")]
    pub header: Header,
    #[serde(rename = "eCH-0276:content")]
    pub content: Content,
}

impl Message {
    pub fn new(d: Document) -> Self {
        Self {
            xmlns: "http://www.ech.ch/xmlns/eCH-0276/1",
            minor_version: 0,
            header: d.header,
            content: d.content,
        }
    }
}

// --------------------------------------------------------------------------- //
// header / title
// --------------------------------------------------------------------------- //

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Header {
    #[serde(rename = "eCH-0276:title")]
    pub title: Title,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Title {
    #[serde(rename = "eCH-0276:organisationName")]
    pub organisation_name: String,
    /// Register-Nr. als Zahl (xs:long). Aus "J 000 091 119/9" → 91119.
    #[serde(rename = "eCH-0276:registerNumber")]
    pub register_number: i64,
    #[serde(rename = "eCH-0276:uid", skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(rename = "eCH-0276:assessmentMunicipality")]
    pub assessment_municipality: String,
    #[serde(rename = "eCH-0276:assessmentMunicipalityId", skip_serializing_if = "Option::is_none")]
    pub assessment_municipality_id: Option<u32>,
    #[serde(rename = "eCH-0276:headOffice")]
    pub head_office: HeadOffice,
    #[serde(rename = "eCH-0276:taxPeriodFrom")]
    pub tax_period_from: String,
    #[serde(rename = "eCH-0276:taxPeriodTo")]
    pub tax_period_to: String,
    #[serde(rename = "eCH-0276:currencyShareEquity")]
    pub currency_share_equity: String,
    #[serde(rename = "eCH-0276:currencyFinancialReporting")]
    pub currency_financial_reporting: String,
    #[serde(rename = "eCH-0276:chairmanOfTheBoardOfDirectors", skip_serializing_if = "Option::is_none")]
    pub chairman_of_the_board_of_directors: Option<String>,
    #[serde(rename = "eCH-0276:management", skip_serializing_if = "Option::is_none")]
    pub management: Option<String>,
    #[serde(rename = "eCH-0276:responsibleForAccounting", skip_serializing_if = "Option::is_none")]
    pub responsible_for_accounting: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HeadOffice {
    #[serde(rename = "eCH-0276:streetHeadOffice")]
    pub street: String,
    #[serde(rename = "eCH-0276:houseNumberHeadOffice")]
    pub house_number: String,
    #[serde(rename = "eCH-0276:zipCodeHeadOffice")]
    pub zip_code: u32,
    #[serde(rename = "eCH-0276:townHeadOffice")]
    pub town: String,
    #[serde(rename = "eCH-0276:headOfficeMunicipalityId")]
    pub municipality_id: u32,
    #[serde(rename = "eCH-0276:headOfficeCanton")]
    pub canton: String,
}

// --------------------------------------------------------------------------- //
// content
// --------------------------------------------------------------------------- //

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Content {
    #[serde(rename = "eCH-0276:assets", skip_serializing_if = "Option::is_none")]
    pub assets: Option<Assets>,
    #[serde(rename = "eCH-0276:equityAndLiabilities", skip_serializing_if = "Option::is_none")]
    pub equity_and_liabilities: Option<EquityAndLiabilities>,
    #[serde(rename = "eCH-0276:incomeStatement")]
    pub income_statement: IncomeStatement,
    #[serde(rename = "eCH-0276:fiscalCorrections", skip_serializing_if = "Option::is_none")]
    pub fiscal_corrections: Option<FiscalCorrections>,
    #[serde(rename = "eCH-0276:profitAppropriation")]
    pub profit_appropriation: ProfitAppropriation,
    #[serde(rename = "eCH-0276:taxableEquityAfterProfitAppropriation", skip_serializing_if = "Option::is_none")]
    pub taxable_equity_after_profit_appropriation: Option<TaxableEquityAfterProfitAppropriation>,
}

// ---- Bilanz: Aktiven ----

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Assets {
    #[serde(rename = "eCH-0276:currentAssets", skip_serializing_if = "Option::is_none")]
    pub current_assets: Option<CurrentAssets>,
    #[serde(rename = "eCH-0276:noncurrentAssets", skip_serializing_if = "Option::is_none")]
    pub noncurrent_assets: Option<NoncurrentAssets>,
    #[serde(rename = "eCH-0276:totalAssets", skip_serializing_if = "Option::is_none")]
    pub total_assets: Option<Chf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CurrentAssets {
    #[serde(rename = "eCH-0276:cashAndCashEquivalentsAndCurrentAssetsWithStockMarketPrice", skip_serializing_if = "Option::is_none")]
    pub cash_and_securities: Option<CashAndSecurities>,
    #[serde(rename = "eCH-0276:totalCurrentAssets", skip_serializing_if = "Option::is_none")]
    pub total_current_assets: Option<Chf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CashAndSecurities {
    #[serde(rename = "eCH-0276:cashAndCashEquivalents", skip_serializing_if = "Option::is_none")]
    pub cash_and_cash_equivalents: Option<Chf>,
    #[serde(rename = "eCH-0276:currentAssetsWithStockMarketPrice", skip_serializing_if = "Option::is_none")]
    pub current_assets_with_stock_market_price: Option<Chf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NoncurrentAssets {
    #[serde(rename = "eCH-0276:propertyPlantEquipment", skip_serializing_if = "Option::is_none")]
    pub property_plant_equipment: Option<PropertyPlantEquipment>,
    #[serde(rename = "eCH-0276:totalNoncurrentAssets", skip_serializing_if = "Option::is_none")]
    pub total_noncurrent_assets: Option<Chf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PropertyPlantEquipment {
    /// EDV-Anlage / IT- und Kommunikationstechnik.
    #[serde(rename = "eCH-0276:officeEquipmentItAndCommunicationsTechnology", skip_serializing_if = "Option::is_none")]
    pub office_equipment_it: Option<Chf>,
}

// ---- Bilanz: Passiven + Eigenkapital ----

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EquityAndLiabilities {
    #[serde(rename = "eCH-0276:currentLiabilities", skip_serializing_if = "Option::is_none")]
    pub current_liabilities: Option<CurrentLiabilities>,
    #[serde(rename = "eCH-0276:equity", skip_serializing_if = "Option::is_none")]
    pub equity: Option<Equity>,
    #[serde(rename = "eCH-0276:totalEquityAndLiabilities")]
    pub total_equity_and_liabilities: Chf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CurrentLiabilities {
    #[serde(rename = "eCH-0276:tradeAndOtherCurrentPayables", skip_serializing_if = "Option::is_none")]
    pub trade_and_other_current_payables: Option<TradeAndOtherCurrentPayables>,
    #[serde(rename = "eCH-0276:accruedExpenses", skip_serializing_if = "Option::is_none")]
    pub accrued_expenses: Option<Chf>,
    #[serde(rename = "eCH-0276:totalCurrentLiabilities", skip_serializing_if = "Option::is_none")]
    pub total_current_liabilities: Option<Chf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TradeAndOtherCurrentPayables {
    #[serde(rename = "eCH-0276:currentPayablesThirdParties", skip_serializing_if = "Option::is_none")]
    pub current_payables_third_parties: Option<Chf>,
    #[serde(rename = "eCH-0276:currentPayablesShareholders", skip_serializing_if = "Option::is_none")]
    pub current_payables_shareholders: Option<Chf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Equity {
    #[serde(rename = "eCH-0276:shareCapitalShareholderCapitalAssociationCapitalFoundationCapital")]
    pub share_capital: Chf,
    #[serde(rename = "eCH-0276:statutoryProfitReserveLimitedCompanies", skip_serializing_if = "Option::is_none")]
    pub statutory_profit_reserve: Option<StatutoryProfitReserve>,
    #[serde(rename = "eCH-0276:netProfitOrLoss", skip_serializing_if = "Option::is_none")]
    pub net_profit_or_loss: Option<NetProfitOrLoss>,
    #[serde(rename = "eCH-0276:totalEquity")]
    pub total_equity: Chf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StatutoryProfitReserve {
    #[serde(rename = "eCH-0276:statutoryProfitReserveLimitedCompaniesAmount", skip_serializing_if = "Option::is_none")]
    pub amount: Option<Chf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NetProfitOrLoss {
    #[serde(rename = "eCH-0276:profitOrLossBroughtForward", skip_serializing_if = "Option::is_none")]
    pub profit_or_loss_brought_forward: Option<Chf>,
    #[serde(rename = "eCH-0276:annualProfitOrAnnualLoss", skip_serializing_if = "Option::is_none")]
    pub annual_profit_or_annual_loss: Option<Chf>,
}

// ---- Erfolgsrechnung ----

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IncomeStatement {
    #[serde(rename = "eCH-0276:deliveriesAndServicesRevenue", skip_serializing_if = "Option::is_none")]
    pub deliveries_and_services_revenue: Option<DeliveriesAndServicesRevenue>,
    #[serde(rename = "eCH-0276:totalOperatingRevenueFromDeliveriesAndServices", skip_serializing_if = "Option::is_none")]
    pub total_operating_revenue: Option<Chf>,
    #[serde(rename = "eCH-0276:expenses", skip_serializing_if = "Option::is_none")]
    pub expenses: Option<Expenses>,
    #[serde(rename = "eCH-0276:financialExpensesAndFinancialIncome", skip_serializing_if = "Option::is_none")]
    pub financial_expenses_and_income: Option<FinancialExpensesAndIncome>,
    #[serde(rename = "eCH-0276:directTaxes", skip_serializing_if = "Option::is_none")]
    pub direct_taxes: Option<Chf>,
    #[serde(rename = "eCH-0276:annualProfitOrAnnualLoss")]
    pub annual_profit_or_annual_loss: Chf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DeliveriesAndServicesRevenue {
    #[serde(rename = "eCH-0276:serviceRevenue", skip_serializing_if = "Option::is_none")]
    pub service_revenue: Option<Chf>,
    #[serde(rename = "eCH-0276:totalDeliveriesAndServicesRevenue", skip_serializing_if = "Option::is_none")]
    pub total: Option<Chf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Expenses {
    #[serde(rename = "eCH-0276:expensesForMaterialsGoodsServices", skip_serializing_if = "Option::is_none")]
    pub materials_goods_services: Option<MaterialsGoodsServices>,
    #[serde(rename = "eCH-0276:employeeExpenses", skip_serializing_if = "Option::is_none")]
    pub employee_expenses: Option<EmployeeExpenses>,
    #[serde(rename = "eCH-0276:otherOperatingExpenses", skip_serializing_if = "Option::is_none")]
    pub other_operating_expenses: Option<OtherOperatingExpenses>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MaterialsGoodsServices {
    #[serde(rename = "eCH-0276:expensesForPurchasedServices", skip_serializing_if = "Option::is_none")]
    pub purchased_services: Option<Chf>,
    #[serde(rename = "eCH-0276:totalExpensesForMaterialsGoodsServices", skip_serializing_if = "Option::is_none")]
    pub total: Option<Chf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EmployeeExpenses {
    #[serde(rename = "eCH-0276:salaryExpenses", skip_serializing_if = "Option::is_none")]
    pub salary_expenses: Option<Chf>,
    #[serde(rename = "eCH-0276:socialSecurityExpenses", skip_serializing_if = "Option::is_none")]
    pub social_security_expenses: Option<Chf>,
    #[serde(rename = "eCH-0276:totalEmployeeExpenses", skip_serializing_if = "Option::is_none")]
    pub total: Option<Chf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OtherOperatingExpenses {
    #[serde(rename = "eCH-0276:rentalExpense", skip_serializing_if = "Option::is_none")]
    pub rental_expense: Option<Chf>,
    #[serde(rename = "eCH-0276:maintenanceRepairsReplacementOfMobileAssets", skip_serializing_if = "Option::is_none")]
    pub maintenance_repairs: Option<Chf>,
    #[serde(rename = "eCH-0276:administrativeAndItExpenses", skip_serializing_if = "Option::is_none")]
    pub administrative_and_it: Option<Chf>,
    #[serde(rename = "eCH-0276:travelAndRepresentationExpenses", skip_serializing_if = "Option::is_none")]
    pub travel_and_representation: Option<Chf>,
    #[serde(rename = "eCH-0276:miscellaneousOperatingExpenses", skip_serializing_if = "Option::is_none")]
    pub miscellaneous: Option<Chf>,
    #[serde(rename = "eCH-0276:TotalOtherOperatingExpenses", skip_serializing_if = "Option::is_none")]
    pub total: Option<Chf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FinancialExpensesAndIncome {
    #[serde(rename = "eCH-0276:interestExpenseFromLiabilities", skip_serializing_if = "Option::is_none")]
    pub interest_expense: Option<Chf>,
    #[serde(rename = "eCH-0276:financialIncomeFromCurrentAssets", skip_serializing_if = "Option::is_none")]
    pub financial_income: Option<Chf>,
    #[serde(rename = "eCH-0276:totalFinancialExpensesAndFinancialIncome", skip_serializing_if = "Option::is_none")]
    pub total: Option<Chf>,
}

// ---- Steuerliche Korrekturen (D) ----

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FiscalCorrections {
    #[serde(rename = "eCH-0276:netProfitOrLossActualYear")]
    pub net_profit_or_loss_actual_year: TaxAmount,
    #[serde(rename = "eCH-0276:taxLossCarriedForwardList", skip_serializing_if = "Option::is_none")]
    pub tax_loss_carried_forward_list: Option<TaxLossCarriedForwardList>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TaxLossCarriedForwardList {
    #[serde(rename = "eCH-0276:taxableNetProfitOrLossAfterOffsettingLosses")]
    pub after_offsetting_losses: TaxAmount,
    #[serde(rename = "eCH-0276:taxableNetProfitOrLossSwitzerlandChf")]
    pub switzerland_chf: TaxAmount,
    #[serde(rename = "eCH-0276:taxableNetProfitOrLossCantonXyAfterCantonalReliefsChf")]
    pub canton_after_reliefs_chf: Chf,
}

// ---- Gewinnverwendung (E) ----

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfitAppropriation {
    #[serde(rename = "eCH-0276:profitOrLossCarriedForward", skip_serializing_if = "Option::is_none")]
    pub carried_forward: Option<Chf>,
    #[serde(rename = "eCH-0276:totalProfitToBeDistributed")]
    pub total_to_be_distributed: Chf,
    #[serde(rename = "eCH-0276:totalProfitAppropriation")]
    pub total_appropriation: Chf,
    #[serde(rename = "eCH-0276:profitOrLossBroughtForward")]
    pub brought_forward: Chf,
}

// ---- Steuerbares Eigenkapital nach Gewinnverwendung (F) ----

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TaxableEquityAfterProfitAppropriation {
    #[serde(rename = "eCH-0276:taxableEquity", skip_serializing_if = "Option::is_none")]
    pub taxable_equity: Option<TaxableEquity>,
    #[serde(rename = "eCH-0276:totalTaxableEquity")]
    pub total_taxable_equity: TaxAmount,
    #[serde(rename = "eCH-0276:taxableEquityInSwitzerlandChf")]
    pub in_switzerland_chf: TaxAmount,
    #[serde(rename = "eCH-0276:taxableEquityCantonXyAfterCantonalReliefs")]
    pub canton_after_reliefs: Chf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TaxableEquity {
    #[serde(rename = "eCH-0276:paidInCapitalOrNetAsset", skip_serializing_if = "Option::is_none")]
    pub paid_in_capital: Option<Chf>,
    #[serde(rename = "eCH-0276:statutoryCapitalReserves", skip_serializing_if = "Option::is_none")]
    pub statutory_capital_reserves: Option<Chf>,
}
