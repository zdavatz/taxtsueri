# taxtsueri

Steuererklärung für die Stadt Tsüri einreichen.

Rust-Werkzeug, das aus den Daten einer Zürcher Steuererklärung eine
validierungsfähige XML-Datei für die elektronische Einreichung erzeugt — für
**beide** Personenkategorien:

- **Natürliche Personen (NP)** → **eCH-0119** «E-Tax Filing»
- **Juristische Personen (JP)** → **eCH-0276** «E-Bilanz und E-Tax JP»

Dazu die **MWST-Abrechnung** für die ESTV → **eCH-0217** «E-MWST» V2.0.0.

## Standards

- **eCH-0119** «E-Tax Filing» — Austauschformat der Steuermeldung natürlicher
  Personen; kantonale Ergänzungen über `cantonExtensionType`.
- **eCH-0196** «eSteuerauszug» — maschinenlesbarer Bankauszug (PDF mit
  eingebettetem XML/Barcode).
- **eCH-0276** «E-Bilanz und E-Tax JP» — Austauschformat für **juristische
  Personen** (Bilanz, Erfolgsrechnung, Gewinn-/Kapitalsteuer), von eCH + SSK.
- **eCH-0217** «Spezifikation E-MWST» — elektronische **MWST-Abrechnung** für das
  ESTV-Portal SuisseTax. Der Import in «MWST abrechnen» akzeptiert **ausschliesslich
  Version 2.0.0**; ältere Versionen und abweichende Strukturen werden nicht mehr
  verarbeitet.
- **eCH-0044 / 0010 / 0011 / 0007** — Basisstandards für Personen-, Adress-
  und Gemeindedaten, auf denen eCH-0119/0276 aufbauen.
- **eCH-0058 / 0108** — Rahmenstandards (Sendungs-Header, Unternehmensidentifikation),
  die eCH-0217 importiert.

Spezifikationen und XSD-Schemas sind frei (ohne Mitgliedschaft) von
[www.ech.ch](https://www.ech.ch) beziehbar.

## XSD-Schemas

Die vollständigen Import-Hüllen von **eCH-0119** v4.0.0 (NP), **eCH-0276** v1.0.0
(JP) und **eCH-0217** v2.0.0 (MWST) liegen vendoriert in `schema/` und sind offline
validierbar; das eCH-0196-XSD (eSteuerauszug) liegt als Referenz daneben, die vier
offiziellen eCH-0217-Beispiel-XML ebenfalls (das Fetch-Skript validiert sie erneut). Neu beziehen / reproduzieren:

```bash
./scripts/fetch-schemas.sh
xmllint --nonet --noout --schema schema/eCH-0119-4-0-0.xsd <datei>.xml
```

Das Skript lädt von www.ech.ch und verdrahtet anschliessend die `schemaLocation`
lokal (`scripts/patch_schema_locations.py`), weil die eCH-Schemas per Namespace
ohne `schemaLocation` importieren. eCH-0119 importiert die **Framework-Varianten**
`eCH-0007-f`, `eCH-0011-f`, `eCH-0044-f`, `eCH-0046-f` sowie `eCH-0097`.

## Desktop-GUI (eframe, Win/Mac/Linux)

Ein natives Fenster: **UBS-Vermögensausweis (PDF) wählen → validierungsfähiges
eCH-0119-XML** erstellen und speichern (importierbar in die Steuersoftware).
Mit GitHub-Releases-Update-Check (Banner, wenn eine neuere Version vorliegt).

```bash
cargo run --features gui --bin taxtsueri-gui      # GUI starten (braucht Display + pdftotext)
cargo build --release --features gui --bin taxtsueri-gui
```

Das GUI ist **feature-gated** (`gui`), damit die CLI schlank bleibt (kein
eframe/reqwest im Standardbuild). Releases (`.github/workflows/release.yml`)
bauen das GUI bei einem `vX.Y.Z`-Tag für alle drei Plattformen; der In-App-
Updater (`src/update.rs`) findet die passenden Assets.

Datei-Dialoge: ein **rein in egui gezeichneter** Picker (`egui-file-dialog`) —
kein GTK, kein XDG-Portal, kein Hilfs-Thread. Dadurch braucht der Linux-Build
kein `libgtk-3-dev`, der Dialog sieht auf allen Plattformen gleich aus, und die
nativen Dialog-Crashes (GTK-Main-Loop-Reentrancy bzw. GTK-Thread-Unsicherheit
gegenüber der winit/X11-Eventloop) fallen ganz weg.

## Build & Run (CLI)

```bash
cargo run                          # liest data/input.json (oder schreibt Beispiel) → data/steuererklaerung-2025.xml
cargo run -- meine-daten.json      # eigene Eingabe verwenden
cargo run -- examples/input.sample.json                    # synthetische Vorlage
cargo run -- --from-ech0196 examples/ech0196.sample.xml     # Wertschriften aus eSteuerauszug
cargo run -- --from-pdf auszug.pdf                          # eCH-0196-XML aus PDF-Anhang
cargo run -- --from-vermoegensausweis depot.pdf            # Wertschriften aus UBS-Vermögensausweis-PDF
cargo run -- --barcode statement.xml                       # eCH-0196 → Barcode-Nutzlast (ZLIB) vorbereiten
cargo run -- --zh-barcode                                  # ZH-Steuererklärungs-Barcode (eCH-0119 v3 + zh:cantonExtension) → PDF417-Blatt
cargo run -- --package                                      # zusätzlich Einreichungs-Paket
cargo run -- --jp                                           # juristische Person (ywesee GmbH) → eCH-0276
cargo run -- --jp --package                                 # JP + Einreichungs-Paket
cargo run -- --from-mt940 auszug.mt940                      # MT940-Kontoauszug einlesen (Salden/Buchungen)
cargo run -- --from-camt kontoauszug.xml                    # camt.053-Kontoauszug (ISO 20022) einlesen — Nachfolger von MT940
cargo run -- --from-camt camt/                              # Verzeichnis mit camt.053-Dateien (täglich/monatlich) → aggregiert
cargo run -- --from-mt940 konto.mt940 --from-vermoegensausweis depot.pdf  # kombiniert → eCH-0119 (MT940-Konto = Basis + Wertschriften)
cargo run -- --from-mt940 konto.mt940 --wertschriften 38628 # → data/Cash-Flow-Rechnung.pdf (Bilanz + ER, Entwurf z. Hd. Vermögensverwalter)
cargo run -- --mwst --periode S1/2026 --umsatz 123456.78 --activity-id 12345   # MWST-Abrechnung → eCH-0217 V2.0.0
cargo run -- --mwst --periode S1/2026 --umsatz 123456.78 --from-mt940 konto.mt940  # + Gegenprobe gegen den Kontoauszug
```

### eCH-0196-Barcode (`--barcode`) — Fundament

Bereitet die Nutzlast für das **2D-Barcode-Blatt** nach «eCH-0196 Beilage 2 –
Barcode Generierung» vor: der eSteuerauszug-XML wird **ZLIB-komprimiert** (Deflate,
beste Stufe), die **Barcode-ID** (Kap. 2.1) ermittelt und die dokumentierten
PDF417-Parameter angewandt (13 Spalten × 35 Zeilen, EC-Level 4, 6 Blöcke/Blatt).
Die Nutzlast wird als **PDF417 Structured Append** über ein oder mehrere Segmente
gerendert (→ `data/barcode-1.pbm`, `…-2.pbm`, …; Geometrie je 290×35 Module = exakt
die eCH-Spec). Encoder/Renderer in `src/pdf417/` (Byte-Compaction, Symbol-
Assemblierung, Macro-PDF417-Kontrollblock und Rendering eigener Code; `HL_TO_LL`-
Tabellen + Reed-Solomon aus der MIT-Crate `pdf417` vendoriert).

Das Ganze wird zu einem **A4-Barcode-Blatt-PDF** zusammengesetzt (`data/barcode-blatt.pdf`,
Querformat): die PDF417-Segmente (max. 6/Blatt) + der **CODE128C-Seitenbarcode**
(16-stellig, `src/code128.rs`), via `lopdf` als 1-Bit-ImageMask-XObjects (`src/sheet.rs`).

**Verifiziert per Round-Trip:** PDF417 (Einzel- **und** Mehrsegment) und CODE128C
werden im Test mit `rxing` (zxing-Port) dekodiert und stimmen 1:1 mit der Eingabe
überein. (`zbar` decodiert PDF417 nicht vollständig — daher rxing.)

Hinweis: Dies ist der **eCH-0196**-eSteuerauszug-Barcode (öffentlich spezifiziert).
Das ZH-*Steuererklärungs*-Barcode-Format ist separat und nur auf Anfrage beim
Steueramt erhältlich (siehe oben).

### Wertschriften aus dem Vermögensausweis-PDF (`--from-vermoegensausweis`)

Erstellt **direkt aus einem UBS-Vermögensausweis** (Portfolio-Auszug, Text-PDF)
das eCH-0119-Wertschriftenverzeichnis und damit ein **validiertes XML**: per
`pdftotext -layout` extrahiert, dann werden Anzahl, Bezeichnung, Valor/ISIN,
Währung, Domizilland und **Marktwert (= Steuerwert)** je Position gelesen. Für
CHF-Titel wird der Bruttoertrag (Kolonne A) aus Anzahl × Ausschüttung berechnet;
bei Fremdwährungstiteln bleibt der Ertrag offen (präzise via eCH-0196). Braucht
`pdftotext` (poppler-utils).

### MT940-Kontoauszug (`--from-mt940`)

Liest einen SWIFT-Kontoauszug (`:60F`/`:62F`-Salden, `:61:`/`:86:`-Buchungen),
**gruppiert die Buchungen heuristisch nach Kategorien** (Erlös, Daueraufträge,
Lastschriften, Steuern, Sozialversicherungen, Debitkarten-Spesen, Dividenden,
Bankspesen …) und gibt daraus eine **näherungsweise Erfolgsrechnung** (Cash-Basis:
Total Ertrag − Aufwand = Geldfluss-Saldo) sowie die **Bilanz-Position** «Flüssige
Mittel» (Schlusssaldo) aus. Voller Report inkl. Kategorien + Buchungen →
`data/mt940-summary.json`. Beträge in Rappen (kein Float).

Wichtig: Das ist **cash-basiert** — keine buchhalterisch exakte Erfolgsrechnung
(Abgrenzungen/RAG, Abschreibungen fehlen; MwSt./Timing in den Cash-Flüssen). Als
Gegenprobe: Eröffnung + Gutschriften − Belastungen = Schluss. Für steuerlich
kategorisierte Wertschriftendaten dient eCH-0196, nicht MT940.

### camt.053-Kontoauszug (`--from-camt`)

Liest einen **camt.053**-Kontoauszug (ISO 20022, XML) — den **modernen Nachfolger
von MT940** und das XML-Export-Format der UBS für Kontoauszüge. Unterstützt wird
**`camt.053.001.08`** nach den **Swiss Payment Standards** (Namespace
`urn:iso:std:iso:20022:tech:xsd:camt.053.001.08`). Gegenüber dem MT940-`:86:`-Freitext
liefert camt.053 **strukturierte Felder** (Gegenpartei `RltdPties`, Zahlungszweck
`RmtInf`, Buchungscode `BkTxCd`), was die automatische Kategorisierung (Dividenden,
Lohn, MWST/ESTV, Reisespesen) deutlich zuverlässiger macht.

`--from-camt <datei.xml>` liest eine Datei, `--from-camt <verzeichnis>` aggregiert
**beliebig viele** camt.053-Dateien (tägliche oder monatliche Auszüge) zu einem
Statement (Eröffnung aus der frühesten, Schluss aus der spätesten Datei). Ausgabe wie
bei `--from-mt940`: Kategorien, Cash-Basis-Erfolgsrechnung und Bilanz-Position.

### Juristische Personen (eCH-0276)

`--jp` erzeugt die Steuererklärung einer **juristischen Person** nach **eCH-0276**
(«E-Bilanz und E-Tax JP», offizieller eCH/SSK-Standard) und **validiert gegen
`schema/eCH-0276-1-0.xsd`**. Eingabe: `data/input-jp.json` (sonst eingebautes
ywesee-Beispiel). Abgebildet: Kopf/Sitz, Bilanz (Aktiven/Passiven/Eigenkapital),
Erfolgsrechnung, steuerliche Korrekturen, Gewinnverwendung und steuerbares
Eigenkapital — gespeist aus Steuererklärung (StA 500) **und** Jahresrechnung.

### MWST-Abrechnung (eCH-0217, `--mwst`)

`--mwst` erzeugt die **MWST-Abrechnung** nach **eCH-0217 V2.0.0** und validiert sie
gegen `schema/eCH-0217-2-0-0.xsd`. Hochladen im ESTV-Portal: *SuisseTax → MWST
abrechnen → Abrechnungsdaten importieren*. Das deklarierende Unternehmen und die
Steuerperiode erkennt das Portal automatisch aus der Datei.

```bash
cargo run -- --mwst --periode S1/2026 --umsatz 123456.78 \
             --activity-id 12345 --from-mt940 konto.mt940
```

Ausgabe im Aufbau des Papierformulars (Ziff. 200 / 289 / 299 / 3xx / 399 / 500),
danach die Gegenprobe gegen den Kontoauszug und `data/mwst-abrechnung-<von>-bis-<bis>.xml`.

**Periode** `--periode S1/2026` (Semester), `Q2/2026` (Quartal) oder
`2026-01-01:2026-06-30`. Ohne Angabe wird sie aus den `:60F:`/`:62F:`-Salden des
MT940 übernommen.

**Methode.** Die Abrechnungsmethode steckt im XML nicht in einem Flag, sondern im
Namen des `xs:choice`-Elements:

| Methode | Element | gültig |
|---|---|---|
| Saldo-/Pauschalsteuersatz | `simpleTaxRateMethod` | Abrechnungsperioden **ab 01.01.2025** |
| effektiv (`--effektiv`) | `effectiveReportingMethod` | immer |

Bei Saldosteuersatz verlangt der Standard seit 2025 eine **5-stellige `activityID`**
(Tätigkeitscode). Es dürfen nur **von der ESTV bewilligte** Codes übermittelt werden;
sie stehen in «MWST abrechnen» unter *Abrechnungsmodalitäten* bzw. auf den
Subformularen. taxtsueri rät sie **nicht** — ohne `--activity-id` (oder
`mwst.activityId` in `settings.json`) bricht der Lauf mit einem Hinweis ab. Ein nicht
bewilligter Code wird vom Portal mit *«Die übermittelten Tätigkeiten entsprechen nicht
der Bewilligung.»* zurückgewiesen. Für Umsätze aus Leistungen der Jahre 2023/2024, die
erst ab 2025 deklariert werden, gibt es die technischen Codes `T0001`–`T0020`
(in `model_mwst::TECHNICAL_ACTIVITY_IDS`).

**Mehrere Tätigkeiten oder Steuersätze.** Umfasst die Bewilligung mehr als eine
Tätigkeit, muss der Umsatz aufgeteilt werden — je eine Zeile `suppliesPerTaxRate`
pro Tätigkeit. Dafür `--position` mehrfach angeben (statt `--activity-id`):

```bash
cargo run -- --mwst --periode S1/2026 --umsatz 123456.78 \
  --position 12345:6.2:100000.00 \
  --position 54321:1.2:23456.78
```

Format `CODE:SATZ:UMSATZ` (Saldosteuersatz) bzw. `SATZ:UMSATZ` (effektive Methode).
Die Summe der Positionen muss Ziff. 299 ergeben, sonst bricht taxtsueri mit MWST-0005 ab.

**Vereinbart oder vereinnahmt.** `formOfReporting = 1` (vereinbart, Art. 39 Abs. 1
MWSTG) ist der gesetzliche Regelfall: massgebend ist das **Rechnungsdatum**, also der
Erlös aus der Buchhaltung — den gibt `--umsatz` vor. Mit `--vereinnahmt`
(`formOfReporting = 2`, nur mit Bewilligung nach Art. 39 Abs. 2) zählt der
**Zahlungseingang**; dann leitet taxtsueri Ziff. 200 direkt aus den Kundengutschriften
des MT940 ab.

**Gegenprobe.** Ein mitgegebenes `--from-mt940` wird immer ausgewertet:
Gutschriften total → abzüglich Dividenden/Zinsen (die als Ziff. 910
`otherFlowsOfFunds/donations` deklariert werden) → abzüglich
Wertschriftenabrechnungen → Kundenzahlungen (Ist). Die Differenz zum deklarierten
Soll ist die Debitorenverschiebung über den Stichtag. Jede nicht als Entgelt gezählte
Gutschrift wird einzeln aufgelistet, damit die Klassierung prüfbar bleibt.

**Steuerberechnung.** `payableTax` (Ziff. 500 bzw. 510 bei Guthaben) wird nach
eCH-0217 Kap. 6.2 berechnet — intern in Rappen bzw. Mikro-Rappen, damit wie in
Kap. 7.5 gefordert **kein Zwischenschritt gerundet** wird. Gerundet wird genau einmal
am Schluss, standardmässig **kaufmännisch auf Rappen** — so rechnet das ESTV-Portal
(am Abrechnungsbeleg S1/2026 verifiziert); die ebenfalls zulässige 5-Rappen-Variante
*zu Gunsten der steuerpflichtigen Unternehmung* (so die ESTV-Belege bis 2016)
`--fuenf-rappen` schaltet auf die 5-Rappen-Rundung um. Vor dem Schreiben
prüft taxtsueri zusätzlich die Plausibilisierung aus Kap. 7.5 (Ziff. 299 = Summe der
Leistungen, Fehlercode MWST-0005).

Identifizierende Angaben gehören in das gitignorierte `settings.json` — Vorlage:
`settings.example.json`:

| Schlüssel | Bedeutung |
|---|---|
| `uid` | MWST-Nummer, z. B. `"CHE-123.456.789 MWST"` (wird normalisiert) |
| `organisationName` | Firmenname wie bei der ESTV registriert |
| `activityId` | 5-stelliger bewilligter Tätigkeitscode |
| `taxRate` | Saldosteuersatz, z. B. `"6.2"` |
| `methode` | `"saldosteuersatz"` (Default) oder `"effektiv"` |
| `abrechnungsart` | `"vereinbart"` (Default) oder `"vereinnahmt"` |
| `manufacturer` | Hersteller in `sendingApplication`; ohne Angabe neutral `taxtsueri` |

Alle sind auch als CLI-Option überschreibbar (`--uid`, `--firma`, `--activity-id`,
`--satz`, `--effektiv`, `--vereinnahmt`).

### Eingabe (datengetrieben)

Die Steuererklärung wird aus einer **JSON-Eingabe** (`Document`) aufgebaut.
Reihenfolge: CLI-Argument → `data/input.json` → eingebauter Beispiel-Datensatz
(der beim ersten Lauf nach `data/input.json` geschrieben wird, danach editierbar).
`cargo run` serialisiert nach eCH-0119-XML und validiert direkt mit `xmllint`.

- `data/` enthält Personendaten (Eingabe, XML, Einreichungs-Paket) → gitignored.
- `examples/input.sample.json` und `examples/ech0196.sample.xml` sind
  **synthetische** Vorlagen (committet).

### eSteuerauszug (eCH-0196) aus PDF / XML

`--from-ech0196 <xml>` liest einen Bank-eSteuerauszug (eCH-0196 `taxStatement`)
und ersetzt damit das Wertschriftenverzeichnis. `--from-pdf <pdf>` extrahiert
zuvor das **eingebettete XML** aus dem PDF (`/EmbeddedFiles`). Reine *Scan*-PDFs
(wie die Barcode-Blätter im `pdf/`-Ordner) enthalten kein eingebettetes XML –
ihre Daten stecken in PDF417-Bildern und bräuchten einen Barcode-Bilddecoder
(nicht enthalten); das wird klar gemeldet.

### Einreichung (`--package`)

Schreibt `data/submission/` mit dem validierten `eCH-0119.xml`, SHA-256 und
einem Manifest. Der reale ZH-Kanal (Stand 2025): **ZHprivateTax** (Online-Portal,
AHV-Nr. + Zugangscode + starke Authentifizierung) bzw. das **2D-Barcode-Blatt**
der Steuersoftware – es gibt keine offene Upload-API für eCH-0119-XML.

### IDG-Zugangsgesuch als PDF (`examples/idg_brief.rs`)

Der **API-Beschrieb der Steuerbehörde** für NP/JP ist nicht öffentlich publiziert,
und der Zugang zur Schnittstellendokumentation wird an ein Onboarding gekoppelt
(Business-Plan, Timeline, B2B/B2C-Konzept, Firmenstruktur, Entscheidgremium). Für
diese Anforderungen ist **keine gesetzliche Grundlage** ersichtlich: Die elektronische
Einreichung stützt sich auf **§§ 109c/109d/133 StG** i. V. m. der **Verordnung
LS 631.121**, die weder ein Zulassungsverfahren für Drittsoftware noch die verlangten
Angaben kennt — für gewerbsmässige Vertreter hält **§ 12 Abs. 2** sogar ausdrücklich
fest, dass «keine weiteren Abklärungen» vorgenommen werden. Zugang zu amtlichen
Dokumenten steht ohnehin nach dem **Öffentlichkeitsprinzip** zu (Art. 17 KV ZH; §§ 20 ff. IDG).

Dieses eigenständige Beispiel erzeugt daraus ein **formelles Gesuch um Zugang zu
amtlichen Dokumenten** als zweiseitiges **PDF mit klickbaren Gesetzeslinks**:

```bash
cargo run --example idg_brief                 # → ~/idg-zugangsgesuch.pdf
cargo run --example idg_brief -- /pfad/x.pdf  # eigener Ausgabepfad
```

Rein mit `lopdf` gebaut (wie `src/pdf_report.rs`), zusätzlich mit **`/Link`-URI-
Annotationen**, eigenem Zeilenumbruch, Helvetica/Helvetica-Bold-Breitentabellen und
WinAnsi-Encoding (Umlaute, §, •, –). Der Brieftext enthält **nur Platzhalter** (keine
Personen-/Firmennamen).

## Stand

Das erzeugte XML **validiert gegen das offizielle eCH-0119-v4.0.0-XSD**
(`cargo run` ruft am Ende selbst `xmllint` auf; zusätzlich `cargo test`).
Der Beispieldatensatz (Steuererklärung 2025, Stadt Zürich / Gemeinde 261) deckt ab:
Kopf, Vertreter, Person 1 inkl. Postadresse + Zivilstand «getrennt», Kinder,
Einkünfte (Ziffern 100–199), Abzüge (220–299), Einkommensberechnung (310–398),
Vermögen (400–498), Wertschriftenverzeichnis mit Verrechnungssteueranspruch und
**DA-1-Positionen** (US-Titel, Kolonne B, Domizil «US») sowie das
Schuldenverzeichnis (`listOfLiabilities`).

### Konfession (`religion`) — recherchiert

Die publizierte **eCH-0011-Religionscodeliste** kennt nur `111`
(evangelisch-reformiert), `121` (römisch-katholisch), `122` (christkatholisch),
`211` (jüdisch) und `000` (Unbekannt) — **keinen** Code für «andere/
konfessionslos». Diese verifizierten Codes stehen als Konstanten bereit
(`model::religion`). Für Zeno («andere», kirchensteuerlich irrelevant) bleibt
`religion` daher bewusst leer.

### Offene Schritte

- Die DA-1-Quellensteueranrechnung selbst (beantragter Anrechnungsbetrag,
  zusätzlicher US-Rückbehalt) kennt eCH-0119 v4 nicht als strukturiertes Feld –
  sie liegt dem als `attachedFormDA1` gezählten PDF-Formular bei.
- PDF417-Barcode-**Bild**decoder für Scan-PDFs (heute: eingebettetes XML +
  JSON `Document`).
- Tatsächliche Übermittlung an ZHprivateTax (interaktives Portal, keine API).

## Lizenz

GPL-3.0-or-later.
