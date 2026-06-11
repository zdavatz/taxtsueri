# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**taxtsueri** — generates an **eCH-0119**-conformant XML for electronically filing a
Zürich tax declaration (natural persons). Steuererklärung für die Stadt Zürich/"Tsüri".

License: GPL-3.0-or-later. Remote: https://github.com/zdavatz/taxtsueri.git

## Commands

```bash
cargo build                 # compile
cargo run                   # read data/input.json (or write the built-in example) → data/steuererklaerung-2025.xml, then validate
cargo run -- input.json     # use a specific JSON input
cargo run -- --from-ech0196 statement.xml   # replace securities list from an eCH-0196 eSteuerauszug
cargo run -- --from-pdf bank.pdf            # extract embedded eCH-0196 XML from a PDF, then as above
cargo run -- --package                      # also write data/submission/ (XML + SHA-256 manifest)
cargo run -- --jp                           # legal entity (juristische Person) → eCH-0276 XML, validated
cargo test                  # run tests (incl. xmllint validation of NP eCH-0119 + JP eCH-0276, eCH-0196 parse, PDF roundtrip, SHA-256)

./scripts/fetch-schemas.sh  # (re)download the eCH-0119 XSD set into schema/ + wire it for offline use
xmllint --nonet --noout --schema schema/eCH-0119-4-0-0.xsd <file>.xml   # validate against eCH-0119
```

Rust toolchain: edition 2021, rustc/cargo 1.93.

## Architecture

A small serde + `quick-xml` pipeline that turns a JSON `Document` into eCH-0119 XML
that **validates against the official XSD**. Three modules:

- **`src/model.rs`** — the eCH-0119 v4.0.0 model, built directly from
  `schema/eCH-0119-4-0-0.xsd`: `Message[@minorVersion]` → `Header` + `Content` →
  `MainForm` (`representativePerson`, `personDataPartner1`, `childData*`, `revenue`,
  `deduction`, `revenueCalculation`, `asset`) + `ListOfSecurities` + `ListOfLiabilities`.
  Every struct derives `Serialize + Deserialize + Default` with `#[serde(default)]` so
  it round-trips to JSON; **only `Message` is Serialize-only** (it holds the XML
  `xmlns:` plumbing). `Document { header, content }` is the JSON I/O wrapper;
  `Document::into_message()` wraps it for XML. `TaxAmount` = cantonal/federal split
  (`taxAmountType`); `PartnerAmount`/`FiscalValue` are the other recurring shapes.
- **`src/dataset.rs`** — `example() -> Document`, the concrete data (taxpayer Zeno
  Davatz, 2025, Stadt Zürich / municipality 261) from the PDFs.
- **`src/ech0196.rs`** — minimal serde reader for an **eCH-0196** «eSteuerauszug»
  (`taxStatement` → `listOfSecurities` → `depot` → `security`); `list_of_securities_from_xml`
  maps it into our `ListOfSecurities`. CHF rounded to integers; withholding keeps 2 decimals.
- **`src/pdf.rs`** — extracts embedded files (PDF `/Names/EmbeddedFiles` name tree) via
  `lopdf`; `extract_embedded_xml` returns XML attachments. Scans (our `pdf/` samples) have
  none — that's reported, not faked.
- **`src/vermoegensausweis.rs`** — parser for a UBS **Vermögensausweis** (portfolio statement,
  text PDF via `pdftotext -layout`): extracts positions (quantity, name, Valor/ISIN, currency,
  country, market value = tax value, dividend) → `SecurityEntry`. `--from-vermoegensausweis`
  builds the eCH-0119 securities list directly from the PDF (tax values exact; CHF gross income
  computed, foreign gross left to eCH-0196).
- **`src/mt940.rs`** — SWIFT MT940 bank-statement reader (`:60F`/`:62F` balances, `:61:`/`:86:`
  transactions, `booking_type` from the `:61:` narrative); amounts in Rappen (i64). `category()`
  groups transactions heuristically; `--from-mt940` prints categories + a cash-basis
  Erfolgsrechnung + the Bilanz cash position, and writes `data/mt940-summary.json`. Payment data
  only / cash-basis — not tax-categorised securities data (that's eCH-0196).
- **`src/model_jp.rs` / `src/dataset_jp.rs`** — **juristische Personen** per **eCH-0276**
  «E-Bilanz und E-Tax JP» (built from `schema/eCH-0276-1-0.xsd` + `eCH-0276-beispiel.xml`):
  root `eBalanceSheetETaxLegalEntity` → `header`(title) + `content` (assets, equityAndLiabilities,
  incomeStatement, fiscalCorrections, profitAppropriation, taxableEquityAfterProfitAppropriation).
  Every element is prefixed `eCH-0276:`; amounts are `xs:long` (whole CHF). `dataset_jp::example()`
  is ywesee GmbH from the StA-500 + Jahresrechnung PDFs. `--jp` validates against the eCH-0276 XSD.
- **`src/submit.rs`** — `write_package`/`write_package_jp` emit `data/submission[-jp]/` (XML + MANIFEST
  with a dependency-free SHA-256 and the real ZH channel guidance: ZHprivateTax / ZHCorporateTax / barcode).
- **`src/main.rs`** — CLI: resolves input (arg → `data/input.json` → built-in example),
  optionally replaces securities from eCH-0196 (`--from-ech0196` / `--from-pdf`), serializes
  to `data/steuererklaerung-2025.xml`, runs `xmllint`, and with `--package` writes the bundle.

`model::religion` holds the verified eCH-0011 confession codes (111/121/122/211/000); the
published catalog has **no** code for «andere/konfessionslos», so that case stays empty.

`src/settings.rs` loads `settings.json` (gitignored) for identifiers that must **not** live in
code — currently the JP `uid` and `registerNumber`, applied as overrides in `run_jp`. A
committed `settings.example.json` documents the format.

`examples/input.sample.json` (synthetic JSON input) and `examples/ech0196.sample.xml`
(synthetic eSteuerauszug) are committed; `tests/validation.rs` validates the example dataset
and the sample input against the XSD via xmllint.

### Namespaces — the rule that makes it validate

eCH-0119 is the instance's **default namespace** (unprefixed). A complexType's local
elements live in the namespace of the schema that *defines* it, so only cross-schema
subtrees carry a prefix: `childData/personIdentification` → `eCH-0044f`,
`taxMunicipality` → `eCH-0007f`, `uid` → `eCH-0097`. `dateOfBirth` is
`datePartiallyKnownType` whose child `yearMonthDay` is always `eCH-0044f`. The address
chain crosses three namespaces: the `addressInformation` element is eCH-0119, then
`eCH-0046f:postalAddress` → `eCH-0010f:addressInformation` → street/town/swissZipCode/
`eCH-0010f:country`. `maritalStatusTax` → `eCH-0011f` (status "2" + `separationData`
encodes «getrennt»). Prefixes are emitted via `#[serde(rename = "prefix:local")]`; all
`xmlns:` decls (eCH-0044f/0007f/0097/0046f/0010f/0011f) sit on `Message`.

DA-1 (foreign withholding) has no dedicated form in eCH-0119 v4 — foreign holdings are
`securityEntry`s with `countryOfDepositaryBank` + `grossRevenueB` (column B), plus
`attachedFormDA1` and the `subtotalGrossRevenueB`/`totalGrossRevenue` totals on
`listOfSecurities`.

### How to extend the model — CRITICAL: order + format

- **Field order MUST match the `xs:sequence`** of the type in the XSD, or validation
  fails. When adding a field, open `schema/eCH-0119-4-0-0.xsd`, find the type, and
  insert the field at the right position. We model only the elements we emit, but
  they must stay in schema order.
- Watch simple-type **facets**: e.g. `phoneNumber` is `\d{10,20}` (digits only, no
  spaces); `bankName`/`accountOwner` ≤ 24 chars; `vn` ∈ 7560000000001..7569999999999.
- `cantonExtension` is omitted everywhere (it uses `xs:any processContents="strict"`).
- After any change: `cargo run` (self-validates) and `cargo test` (the
  `tests/validation.rs` integration test re-runs xmllint).

## Schemas (`schema/`)

The full eCH-0119 v4.0.0 import closure (13 XSDs) is vendored here and validates
offline. `scripts/fetch-schemas.sh` re-downloads them from www.ech.ch;
`scripts/patch_schema_locations.py` then rewrites every `<xs:import>` to point at the
sibling local file (the eCH schemas import by namespace with no `schemaLocation`, so
libxml2 can't resolve them otherwise). The patch is idempotent and part of the fetch
script — re-run the fetch script rather than editing XSDs by hand.

Root element is `message` (attr `minorVersion`) with `header` + `content`; types
come from the `-f` framework standards (`eCH-0044-f`, `eCH-0046-f`, `eCH-0007-f`,
`eCH-0011-f`) plus `eCH-0097`. `schema/test.eCH-0119.v4.0.0.major.xml` is the
canonical minimal example. `src/model.rs` is built to this schema and validates.

## Important constraints

- **Never commit personal/tax data.** `pdf/` (source PDFs) and `data/` (generated XML)
  are git-ignored and contain real personal data. `*.pdf` is ignored globally. Verify
  with `git status` before committing. The `schema/` XSDs are public eCH standards and
  are safe to commit.
- `Cargo.lock` is git-ignored (this is currently a bin-only PoC).
