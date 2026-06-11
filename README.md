# taxtsueri

Steuererklärung für die Stadt Tsüri einreichen.

Rust-Werkzeug, das aus den Daten einer Zürcher Steuererklärung (natürliche
Personen) eine **eCH-0119**-konforme XML-Datei für die elektronische
Einreichung erzeugt.

## Standards

- **eCH-0119** «E-Tax Filing» — Austauschformat der Steuermeldung natürlicher
  Personen; kantonale Ergänzungen über `cantonExtensionType`.
- **eCH-0196** «eSteuerauszug» — maschinenlesbarer Bankauszug (PDF mit
  eingebettetem XML/Barcode).
- **eCH-0276** «E-Bilanz und E-Tax JP» — Austauschformat für **juristische
  Personen** (Bilanz, Erfolgsrechnung, Gewinn-/Kapitalsteuer), von eCH + SSK.
- **eCH-0044 / 0010 / 0011 / 0007** — Basisstandards für Personen-, Adress-
  und Gemeindedaten, auf denen eCH-0119/0276 aufbauen.

Spezifikationen und XSD-Schemas sind frei (ohne Mitgliedschaft) von
[www.ech.ch](https://www.ech.ch) beziehbar.

## XSD-Schemas

Die vollständigen Import-Hüllen von **eCH-0119** v4.0.0 (NP) und **eCH-0276** v1.0.0
(JP) liegen vendoriert in `schema/` (26 XSD) und sind offline validierbar; das
eCH-0196-XSD (eSteuerauszug) liegt als Referenz daneben. Neu beziehen / reproduzieren:

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

## Build & Run (CLI)

```bash
cargo run                          # liest data/input.json (oder schreibt Beispiel) → data/steuererklaerung-2025.xml
cargo run -- meine-daten.json      # eigene Eingabe verwenden
cargo run -- examples/input.sample.json                    # synthetische Vorlage
cargo run -- --from-ech0196 examples/ech0196.sample.xml     # Wertschriften aus eSteuerauszug
cargo run -- --from-pdf auszug.pdf                          # eCH-0196-XML aus PDF-Anhang
cargo run -- --from-vermoegensausweis depot.pdf            # Wertschriften aus UBS-Vermögensausweis-PDF
cargo run -- --barcode statement.xml                       # eCH-0196 → Barcode-Nutzlast (ZLIB) vorbereiten
cargo run -- --package                                      # zusätzlich Einreichungs-Paket
cargo run -- --jp                                           # juristische Person (ywesee GmbH) → eCH-0276
cargo run -- --jp --package                                 # JP + Einreichungs-Paket
cargo run -- --from-mt940 auszug.mt940                      # MT940-Kontoauszug einlesen (Salden/Buchungen)
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

### Juristische Personen (eCH-0276)

`--jp` erzeugt die Steuererklärung einer **juristischen Person** nach **eCH-0276**
(«E-Bilanz und E-Tax JP», offizieller eCH/SSK-Standard) und **validiert gegen
`schema/eCH-0276-1-0.xsd`**. Eingabe: `data/input-jp.json` (sonst eingebautes
ywesee-Beispiel). Abgebildet: Kopf/Sitz, Bilanz (Aktiven/Passiven/Eigenkapital),
Erfolgsrechnung, steuerliche Korrekturen, Gewinnverwendung und steuerbares
Eigenkapital — gespeist aus Steuererklärung (StA 500) **und** Jahresrechnung.

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
