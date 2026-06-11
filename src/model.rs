//! eCH-0119 «E-Tax Filing» v4.0.0 — Modell der Steuermeldung natürlicher Personen.
//!
//! Aufgebaut **direkt nach dem XSD** in `schema/eCH-0119-4-0-0.xsd`:
//! `message[@minorVersion]` → `header` + `content`. Modelliert wird genau der
//! Ausschnitt, den wir befüllen; die Feldreihenfolge entspricht der `xs:sequence`
//! des jeweiligen Typs (Pflicht, sonst schlägt die Validierung fehl).
//!
//! Alle Datentypen leiten zusätzlich `Deserialize`/`Default` ab, damit die
//! Eingabe als JSON (`Document`) eingelesen werden kann (siehe `src/input.rs`);
//! `#[serde(default)]` lässt weggelassene Felder zu None/Default werden, sodass
//! `skip_serializing_if` und JSON-Rücklesen zusammenpassen. Das Wurzelelement
//! `Message` trägt nur `Serialize` (es hält die XML-Namespace-Deklarationen).
//!
//! Namespaces: eCH-0119 ist der Default-Namespace (unpräfigiert). Nur Kindelemente
//! von Typen aus anderen Standards tragen einen Präfix (eCH-0044f, eCH-0007f,
//! eCH-0097, eCH-0046f, eCH-0010f, eCH-0011f). `cantonExtension` wird ausgelassen.

use serde::{Deserialize, Serialize};

/// Betrag in ganzen Franken (eCH-0119 `moneyType1`, xs:integer).
pub type Chf = i64;

/// Verifizierte eCH-0011-Religionscodes (`religionType`, `\d{3,6}`).
///
/// Die publizierte eCH-0011-Codeliste kennt nur diese Werte; es gibt **keinen**
/// eigenen Code für «andere» / «konfessionslos» (nur `UNKNOWN` = 000). Für
/// kirchensteuerlich nicht relevante Fälle («andere») bleibt `religion` daher leer.
#[allow(dead_code)] // Referenz-API: für Nutzer mit bekanntem Konfessionscode.
pub mod religion {
    /// evangelisch-reformierte (protestantische) Kirche.
    pub const REFORMIERT: &str = "111";
    /// römisch-katholische Kirche.
    pub const ROEMISCH_KATHOLISCH: &str = "121";
    /// christkatholische / altkatholische Kirche.
    pub const CHRISTKATHOLISCH: &str = "122";
    /// israelitische / jüdische Glaubensgemeinschaft.
    pub const JUEDISCH: &str = "211";
    /// Unbekannt.
    pub const UNKNOWN: &str = "000";
}

/// `taxAmountType`: Aufteilung Staats-/Gemeindesteuer vs. direkte Bundessteuer.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TaxAmount {
    #[serde(rename = "cantonalTax")]
    pub cantonal: Chf,
    #[serde(rename = "federalTax")]
    pub federal: Chf,
}

impl TaxAmount {
    pub fn new(cantonal: Chf, federal: Chf) -> Self {
        Self { cantonal, federal }
    }
}

/// `partnerAmountType` — hier nur Person 1 (partner1Amount).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PartnerAmount {
    #[serde(rename = "partner1Amount")]
    pub partner1: Chf,
}

impl PartnerAmount {
    pub fn p1(amount: Chf) -> Self {
        Self { partner1: amount }
    }
}

/// `privateBusinessType` — hier nur fiscalValue (privater Steuerwert).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FiscalValue {
    #[serde(rename = "fiscalValue")]
    pub fiscal_value: Chf,
}

impl FiscalValue {
    pub fn new(v: Chf) -> Self {
        Self { fiscal_value: v }
    }
}

// ---------------------------------------------------------------------------
// JSON-Eingabe-Wurzel: Document (header + content, ohne XML-Namespace-Ballast)
// ---------------------------------------------------------------------------

/// Serialisierbares Eingabedokument. JSON-Repräsentation der Steuererklärung;
/// `into_message` ergänzt die XML-Namespace-Deklarationen.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Document {
    pub header: Header,
    pub content: Content,
}

impl Document {
    pub fn into_message(self) -> Message {
        Message::new(self.header, self.content)
    }
}

// ---------------------------------------------------------------------------
// Wurzel: message (XML)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename = "message")]
pub struct Message {
    #[serde(rename = "@xmlns")]
    xmlns: &'static str,
    #[serde(rename = "@xmlns:eCH-0044f")]
    xmlns_0044f: &'static str,
    #[serde(rename = "@xmlns:eCH-0007f")]
    xmlns_0007f: &'static str,
    #[serde(rename = "@xmlns:eCH-0097")]
    xmlns_0097: &'static str,
    #[serde(rename = "@xmlns:eCH-0046f")]
    xmlns_0046f: &'static str,
    #[serde(rename = "@xmlns:eCH-0010f")]
    xmlns_0010f: &'static str,
    #[serde(rename = "@xmlns:eCH-0011f")]
    xmlns_0011f: &'static str,
    #[serde(rename = "@minorVersion")]
    minor_version: u8,

    pub header: Header,
    pub content: Content,
}

impl Message {
    pub fn new(header: Header, content: Content) -> Self {
        Self {
            xmlns: "http://www.ech.ch/xmlns/eCH-0119/4",
            xmlns_0044f: "http://www.ech.ch/xmlns/eCH-0044-f/4",
            xmlns_0007f: "http://www.ech.ch/xmlns/eCH-0007-f/6",
            xmlns_0097: "http://www.ech.ch/xmlns/eCH-0097/5",
            xmlns_0046f: "http://www.ech.ch/xmlns/eCH-0046-f/5",
            xmlns_0010f: "http://www.ech.ch/xmlns/eCH-0010-f/7",
            xmlns_0011f: "http://www.ech.ch/xmlns/eCH-0011-f/8",
            minor_version: 0,
            header,
            content,
        }
    }
}

/// `headerType` (Ausschnitt). `source`: 0 = Software, 1 = 2D-Barcode, 2 = OCR.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Header {
    #[serde(rename = "taxPeriod")]
    pub tax_period: u16,
    #[serde(rename = "periodFrom", skip_serializing_if = "Option::is_none")]
    pub period_from: Option<String>,
    #[serde(rename = "periodTo", skip_serializing_if = "Option::is_none")]
    pub period_to: Option<String>,
    #[serde(rename = "canton", skip_serializing_if = "Option::is_none")]
    pub canton: Option<String>,
    pub source: u8,
}

/// `contentType` (Ausschnitt).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Content {
    #[serde(rename = "mainForm")]
    pub main_form: MainForm,
    #[serde(rename = "listOfSecurities", skip_serializing_if = "Option::is_none")]
    pub list_of_securities: Option<ListOfSecurities>,
    #[serde(rename = "listOfLiabilities", skip_serializing_if = "Option::is_none")]
    pub list_of_liabilities: Option<ListOfLiabilities>,
}

// ---------------------------------------------------------------------------
// mainForm
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MainForm {
    #[serde(rename = "representativePerson", skip_serializing_if = "Option::is_none")]
    pub representative_person: Option<RepresentativePerson>,
    #[serde(rename = "personDataPartner1")]
    pub person_data_partner1: PersonDataPartner1,
    #[serde(rename = "childData")]
    pub child_data: Vec<ChildData>,
    #[serde(rename = "revenue", skip_serializing_if = "Option::is_none")]
    pub revenue: Option<Revenue>,
    #[serde(rename = "deduction", skip_serializing_if = "Option::is_none")]
    pub deduction: Option<Deduction>,
    #[serde(rename = "revenueCalculation", skip_serializing_if = "Option::is_none")]
    pub revenue_calculation: Option<RevenueCalculation>,
    #[serde(rename = "asset", skip_serializing_if = "Option::is_none")]
    pub asset: Option<Asset>,
}

/// Postadresse: `addressType`(eCH-0046f) → `mailAddressType`(eCH-0010f)
/// → `addressInformationType`(eCH-0010f). Das äussere Element `addressInformation`
/// ist eCH-0119-lokal (unpräfigiert); ab `postalAddress` greifen die Präfixe.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AddressInformation {
    #[serde(rename = "eCH-0046f:postalAddress")]
    pub postal_address: MailAddress,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MailAddress {
    #[serde(rename = "eCH-0010f:addressInformation")]
    pub address_information: PostalAddressInfo,
}

/// `addressInformationType` — Reihenfolge laut XSD: street, houseNumber, …,
/// town, swissZipCode, country.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PostalAddressInfo {
    #[serde(rename = "eCH-0010f:street", skip_serializing_if = "Option::is_none")]
    pub street: Option<String>,
    #[serde(rename = "eCH-0010f:houseNumber", skip_serializing_if = "Option::is_none")]
    pub house_number: Option<String>,
    #[serde(rename = "eCH-0010f:town")]
    pub town: String,
    #[serde(rename = "eCH-0010f:swissZipCode", skip_serializing_if = "Option::is_none")]
    pub swiss_zip_code: Option<u32>,
    #[serde(rename = "eCH-0010f:country", skip_serializing_if = "Option::is_none")]
    pub country: Option<Country>,
}

/// `countryType` (eCH-0010f).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Country {
    #[serde(rename = "eCH-0010f:countryIdISO2", skip_serializing_if = "Option::is_none")]
    pub iso2: Option<String>,
    #[serde(rename = "eCH-0010f:countryNameShort", skip_serializing_if = "Option::is_none")]
    pub name_short: Option<String>,
}

/// `representativePersonType` (Ausschnitt): addressInformation, organisationName,
/// phoneNumber, uid.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RepresentativePerson {
    #[serde(rename = "addressInformation", skip_serializing_if = "Option::is_none")]
    pub address: Option<AddressInformation>,
    #[serde(rename = "organisationName", skip_serializing_if = "Option::is_none")]
    pub organisation_name: Option<String>,
    #[serde(rename = "phoneNumber", skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<String>,
    #[serde(rename = "uid", skip_serializing_if = "Option::is_none")]
    pub uid: Option<Uid>,
}

/// `eCH-0097:uidStructureType` — Kinder im eCH-0097-Namespace.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Uid {
    /// "CHE" oder "ADM".
    #[serde(rename = "eCH-0097:uidOrganisationIdCategorie")]
    pub categorie: String,
    /// 9-stellige Zahl (z. B. CHE-101.652.973 → 101652973).
    #[serde(rename = "eCH-0097:uidOrganisationId")]
    pub id: u32,
}

/// `personDataPartner1Type` (Ausschnitt). Reihenfolge laut XSD:
/// partnerPersonIdentification, addressInformation, …, maritalStatusTax,
/// religion, job, …, paymentPension, taxMunicipality.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PersonDataPartner1 {
    #[serde(rename = "partnerPersonIdentification")]
    pub identification: PartnerPersonIdentification,
    #[serde(rename = "addressInformation", skip_serializing_if = "Option::is_none")]
    pub address: Option<AddressInformation>,
    #[serde(rename = "maritalStatusTax", skip_serializing_if = "Option::is_none")]
    pub marital_status_tax: Option<MaritalStatusTax>,
    /// eCH-0011 Religionscode (`\d{3,6}`). Die PDF nennt nur das Label «andere»;
    /// ohne den Katalogcode bleibt das Feld leer (siehe README).
    #[serde(rename = "religion", skip_serializing_if = "Option::is_none")]
    pub religion: Option<String>,
    #[serde(rename = "job", skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    #[serde(rename = "paymentPension", skip_serializing_if = "Option::is_none")]
    pub payment_pension: Option<bool>,
    /// Steuergemeinde — Stadt Zürich (BFS 261).
    #[serde(rename = "taxMunicipality", skip_serializing_if = "Option::is_none")]
    pub tax_municipality: Option<SwissMunicipality>,
}

/// `eCH-0011f:maritalDataType` — Kinder im eCH-0011-f-Namespace.
/// `maritalStatus`: 1 ledig, 2 verheiratet, 3 verwitwet, 4 geschieden, …
/// «getrennt» = verheiratet (2) mit `separationData`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MaritalStatusTax {
    #[serde(rename = "eCH-0011f:maritalStatus", skip_serializing_if = "Option::is_none")]
    pub marital_status: Option<String>,
    #[serde(rename = "eCH-0011f:separationData", skip_serializing_if = "Option::is_none")]
    pub separation_data: Option<SeparationData>,
}

/// `eCH-0011f:separationDataType`. `separation`: 1 gerichtlich, 2 tatsächlich getrennt.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SeparationData {
    #[serde(rename = "eCH-0011f:separation", skip_serializing_if = "Option::is_none")]
    pub separation: Option<String>,
}

/// `partnerPersonIdentificationType` — in eCH-0119 definiert, Kinder daher
/// im Default-Namespace. `vn` ist Pflicht.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PartnerPersonIdentification {
    #[serde(rename = "officialName")]
    pub official_name: String,
    #[serde(rename = "firstName")]
    pub first_name: String,
    /// `eCH-0044f:sexType`: "1" = männlich, "2" = weiblich.
    #[serde(rename = "sex", skip_serializing_if = "Option::is_none")]
    pub sex: Option<String>,
    #[serde(rename = "dateOfBirth", skip_serializing_if = "Option::is_none")]
    pub date_of_birth: Option<DateYearMonthDay>,
    /// AHVN13 (7560000000001..7569999999999).
    pub vn: u64,
}

/// `datePartiallyKnownType` — Kind `yearMonthDay` immer im eCH-0044-f-Namespace.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DateYearMonthDay {
    #[serde(rename = "eCH-0044f:yearMonthDay")]
    pub year_month_day: String,
}

/// `eCH-0007f:swissMunicipalityType` — Kinder im eCH-0007-f-Namespace.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SwissMunicipality {
    #[serde(rename = "eCH-0007f:municipalityId")]
    pub municipality_id: u32,
    #[serde(rename = "eCH-0007f:municipalityName")]
    pub municipality_name: String,
    #[serde(rename = "eCH-0007f:cantonAbbreviation")]
    pub canton: String,
}

/// `childDataType` (Ausschnitt). `homeOrExternal`: false = im Haushalt, true = ausserhalb.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ChildData {
    #[serde(rename = "personIdentification")]
    pub identification: Ech0044PersonId,
    #[serde(rename = "homeOrExternal", skip_serializing_if = "Option::is_none")]
    pub home_or_external: Option<bool>,
    #[serde(rename = "correctTo", skip_serializing_if = "Option::is_none")]
    pub correct_to: Option<String>,
}

/// `eCH-0044f:personIdentificationType` — Kinder im eCH-0044-f-Namespace.
/// Reihenfolge laut XSD: …, officialName, firstName, originalName, sex, dateOfBirth.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Ech0044PersonId {
    #[serde(rename = "eCH-0044f:officialName")]
    pub official_name: String,
    #[serde(rename = "eCH-0044f:firstName")]
    pub first_name: String,
    #[serde(rename = "eCH-0044f:dateOfBirth", skip_serializing_if = "Option::is_none")]
    pub date_of_birth: Option<Ech0044Date>,
}

/// `datePartiallyKnownType` im eCH-0044-f-Kontext (Element *und* Kind im selben NS).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Ech0044Date {
    #[serde(rename = "eCH-0044f:yearMonthDay")]
    pub year_month_day: String,
}

// ---------------------------------------------------------------------------
// revenue (Einkünfte) — Reihenfolge laut revenueType
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Revenue {
    /// Ziffer 100.
    #[serde(rename = "employedMainRevenue", skip_serializing_if = "Option::is_none")]
    pub employed_main: Option<PartnerAmount>,
    /// Ziffer 102.
    #[serde(rename = "employedSidelineRevenue", skip_serializing_if = "Option::is_none")]
    pub employed_sideline: Option<PartnerAmount>,
    /// Ziffer 122.
    #[serde(rename = "selfemployedSidelineRevenue", skip_serializing_if = "Option::is_none")]
    pub selfemployed_sideline: Option<PartnerAmount>,
    /// Ziffer 150.
    #[serde(rename = "securitiesRevenue", skip_serializing_if = "Option::is_none")]
    pub securities_revenue: Option<TaxAmount>,
    /// Eigenmietwert (Liegenschaft).
    #[serde(rename = "propertyNotionalRentalValue", skip_serializing_if = "Option::is_none")]
    pub property_notional_rental_value: Option<Chf>,
    /// Pauschale Unterhalts-/Verwaltungskosten.
    #[serde(rename = "propertyDeductionFlatrate", skip_serializing_if = "Option::is_none")]
    pub property_deduction_flatrate: Option<Chf>,
    /// Ziffer 188 — verbleibender Liegenschaftsertrag.
    #[serde(rename = "propertyRemainingRevenue", skip_serializing_if = "Option::is_none")]
    pub property_remaining_revenue: Option<Chf>,
    /// Ziffer 199.
    #[serde(rename = "totalAmountRevenue", skip_serializing_if = "Option::is_none")]
    pub total_amount_revenue: Option<TaxAmount>,
}

// ---------------------------------------------------------------------------
// deduction (Abzüge) — Reihenfolge laut deductionType
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Deduction {
    /// Ziffer 220.
    #[serde(rename = "jobExpensesPartner1", skip_serializing_if = "Option::is_none")]
    pub job_expenses_p1: Option<TaxAmount>,
    /// Ziffer 255.
    #[serde(rename = "paymentAlimonyChild", skip_serializing_if = "Option::is_none")]
    pub payment_alimony_child: Option<TaxAmount>,
    /// Ziffer 270.
    #[serde(rename = "insuranceAndInterest", skip_serializing_if = "Option::is_none")]
    pub insurance_and_interest: Option<TaxAmount>,
    /// Ziffer 283.
    #[serde(rename = "furtherDeductionFinancialManagement", skip_serializing_if = "Option::is_none")]
    pub financial_management: Option<TaxAmount>,
    /// Ziffer 376.
    #[serde(rename = "furtherDeductionNonparentalSuperVision", skip_serializing_if = "Option::is_none")]
    pub nonparental_supervision: Option<TaxAmount>,
    /// Ziffer 299.
    #[serde(rename = "totalAmountDeduction", skip_serializing_if = "Option::is_none")]
    pub total_amount_deduction: Option<TaxAmount>,
    /// Ziffer 256 — Rentenleistungen (Ertragsanteil), Gesamtbetrag.
    #[serde(rename = "paymentPensionTotal", skip_serializing_if = "Option::is_none")]
    pub payment_pension_total: Option<Chf>,
}

// ---------------------------------------------------------------------------
// revenueCalculation — Reihenfolge laut revenueCalculationType
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RevenueCalculation {
    #[serde(rename = "totalAmountRevenue", skip_serializing_if = "Option::is_none")]
    pub total_amount_revenue: Option<TaxAmount>,
    #[serde(rename = "totalAmountDeduction", skip_serializing_if = "Option::is_none")]
    pub total_amount_deduction: Option<TaxAmount>,
    /// Ziffer 310.
    #[serde(rename = "netIncome", skip_serializing_if = "Option::is_none")]
    pub net_income: Option<TaxAmount>,
    /// Ziffer 324.
    #[serde(rename = "deductionCharity", skip_serializing_if = "Option::is_none")]
    pub deduction_charity: Option<TaxAmount>,
    /// Ziffer 350.
    #[serde(rename = "adjustedNetIncome", skip_serializing_if = "Option::is_none")]
    pub adjusted_net_income: Option<TaxAmount>,
    /// Ziffer 390.
    #[serde(rename = "totalAmountFiscalRevenue", skip_serializing_if = "Option::is_none")]
    pub total_amount_fiscal_revenue: Option<TaxAmount>,
    /// Ziffer 396.
    #[serde(rename = "fiscalRevenueAbroad", skip_serializing_if = "Option::is_none")]
    pub fiscal_revenue_abroad: Option<TaxAmount>,
    /// Ziffer 398.
    #[serde(rename = "resultingFiscalRevenue", skip_serializing_if = "Option::is_none")]
    pub resulting_fiscal_revenue: Option<TaxAmount>,
}

// ---------------------------------------------------------------------------
// asset (Vermögen) — Reihenfolge laut assetType
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Asset {
    /// Ziffer 400.
    #[serde(rename = "movablePropertySecuritiesAndAssets", skip_serializing_if = "Option::is_none")]
    pub securities_and_assets: Option<FiscalValue>,
    /// Ziffer 421 — Liegenschaft zum Verkehrswert.
    #[serde(rename = "propertyMarketValue", skip_serializing_if = "Option::is_none")]
    pub property_market_value: Option<FiscalValue>,
    /// Ziffer 460.
    #[serde(rename = "totalAmountAssets", skip_serializing_if = "Option::is_none")]
    pub total_amount_assets: Option<FiscalValue>,
    /// Ziffer 470.
    #[serde(rename = "totalAmountLiabilities", skip_serializing_if = "Option::is_none")]
    pub total_amount_liabilities: Option<FiscalValue>,
    /// Ziffer 490.
    #[serde(rename = "totalAmountFiscalAssets", skip_serializing_if = "Option::is_none")]
    pub total_amount_fiscal_assets: Option<FiscalValue>,
    /// Ziffer 496.
    #[serde(rename = "fiscalAssetsAbroad", skip_serializing_if = "Option::is_none")]
    pub fiscal_assets_abroad: Option<FiscalValue>,
    /// Ziffer 498 — steuerbares Vermögen Kanton Zürich.
    #[serde(rename = "resultingFiscalAssets", skip_serializing_if = "Option::is_none")]
    pub resulting_fiscal_assets: Option<FiscalValue>,
}

// ---------------------------------------------------------------------------
// listOfSecurities (Wertschriftenverzeichnis)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ListOfSecurities {
    #[serde(rename = "bankAccount", skip_serializing_if = "Option::is_none")]
    pub bank_account: Option<BankAccount>,
    #[serde(rename = "securityEntry")]
    pub security_entry: Vec<SecurityEntry>,
    /// Anzahl beigelegter DA-1-Formulare (Anrechnung ausländischer Quellensteuern).
    #[serde(rename = "attachedFormDA1", skip_serializing_if = "Option::is_none")]
    pub attached_form_da1: Option<i64>,
    /// Total Steuerwert (Ziffer 400-Übertrag).
    #[serde(rename = "totalTaxValue", skip_serializing_if = "Option::is_none")]
    pub total_tax_value: Option<TaxAmount>,
    /// Zwischentotal Bruttoertrag Kolonne A (verrechnungssteuerbelastet).
    #[serde(rename = "subtotalGrossRevenueA1", skip_serializing_if = "Option::is_none")]
    pub subtotal_gross_revenue_a1: Option<TaxAmount>,
    /// Zwischentotal Bruttoertrag Kolonne B (inkl. DA-1/Ausland).
    #[serde(rename = "subtotalGrossRevenueB", skip_serializing_if = "Option::is_none")]
    pub subtotal_gross_revenue_b: Option<TaxAmount>,
    /// Total Bruttoertrag A + B (entspricht Ziffer 150).
    #[serde(rename = "totalGrossRevenue", skip_serializing_if = "Option::is_none")]
    pub total_gross_revenue: Option<TaxAmount>,
    /// Verrechnungssteueranspruch (35 % von Bruttoertrag A), `moneyType2`.
    #[serde(rename = "withholdingTax", skip_serializing_if = "Option::is_none")]
    pub withholding_tax: Option<String>,
}

/// `bankAccountType` — bankName/accountOwner sind auf 24 Zeichen begrenzt.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BankAccount {
    #[serde(rename = "ibanNumber", skip_serializing_if = "Option::is_none")]
    pub iban: Option<String>,
    #[serde(rename = "bankName", skip_serializing_if = "Option::is_none")]
    pub bank_name: Option<String>,
    #[serde(rename = "accountOwner", skip_serializing_if = "Option::is_none")]
    pub account_owner: Option<String>,
}

/// `securityEntryType` (Ausschnitt).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityEntry {
    #[serde(rename = "originalCurrency", skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(rename = "faceValueQuantity", skip_serializing_if = "Option::is_none")]
    pub quantity: Option<String>,
    #[serde(rename = "securitiesNumber", skip_serializing_if = "Option::is_none")]
    pub securities_number: Option<String>,
    #[serde(rename = "detailedDescription")]
    pub description: String,
    /// Domizilland der Depotbank, ISO-3166 alpha-2 (z. B. "US" für DA-1-Titel).
    #[serde(rename = "countryOfDepositaryBank", skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// Steuerwert am 31.12.
    #[serde(rename = "taxValueEndOfYear", skip_serializing_if = "Option::is_none")]
    pub tax_value: Option<TaxAmount>,
    /// Bruttoertrag mit Verrechnungssteuerabzug (Kolonne A, inländisch).
    #[serde(rename = "grossRevenueA", skip_serializing_if = "Option::is_none")]
    pub gross_revenue_a: Option<TaxAmount>,
    /// Bruttoertrag ohne Verrechnungssteuerabzug (Kolonne B, u. a. DA-1/Ausland).
    #[serde(rename = "grossRevenueB", skip_serializing_if = "Option::is_none")]
    pub gross_revenue_b: Option<TaxAmount>,
}

// ---------------------------------------------------------------------------
// listOfLiabilities (Schuldenverzeichnis) — Reihenfolge laut listOfLiabilitiesType
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ListOfLiabilities {
    #[serde(rename = "privateLiabilities")]
    pub private_liabilities: Vec<Liability>,
    #[serde(rename = "businessLiabilities")]
    pub business_liabilities: Vec<Liability>,
    /// Total Privatschulden (Ziffer 470-Beitrag).
    #[serde(rename = "totalPrivateLiabilities", skip_serializing_if = "Option::is_none")]
    pub total_private_liabilities: Option<Chf>,
    /// Total private Schuldzinsen.
    #[serde(rename = "totalPrivateLiabilitiesInterest", skip_serializing_if = "Option::is_none")]
    pub total_private_liabilities_interest: Option<Chf>,
    /// Total aller Schulden.
    #[serde(rename = "totalAmountLiabilities", skip_serializing_if = "Option::is_none")]
    pub total_amount_liabilities: Option<Chf>,
    /// Total aller Schuldzinsen.
    #[serde(rename = "totalAmountLiabilitiesInterest", skip_serializing_if = "Option::is_none")]
    pub total_amount_liabilities_interest: Option<Chf>,
}

/// `liabilitiesListingType`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Liability {
    /// Gläubiger / Bezeichnung.
    #[serde(rename = "identification", skip_serializing_if = "Option::is_none")]
    pub identification: Option<String>,
    /// Schuld am 31.12.
    #[serde(rename = "liability", skip_serializing_if = "Option::is_none")]
    pub liability: Option<Chf>,
    /// Schuldzinsen.
    #[serde(rename = "liabilityInterest", skip_serializing_if = "Option::is_none")]
    pub liability_interest: Option<Chf>,
}
