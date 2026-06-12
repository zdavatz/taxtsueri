#!/usr/bin/env bash
#
# Lädt die für eCH-0119 v4.0.0 benötigten XSD-Schemas (vollständige
# Import-Hülle) von www.ech.ch nach schema/ und verdrahtet sie für die
# Offline-Validierung (scripts/patch_schema_locations.py).
#
# Quelle: Verein eCH, https://www.ech.ch — die Standards und Schemas sind
# frei und ohne Mitgliedschaft beziehbar. Nach dem Lauf validiert:
#   xmllint --nonet --noout --schema schema/eCH-0119-4-0-0.xsd <datei>.xml
#
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p schema
cd schema

XMLNS="http://www.ech.ch/xmlns"
IMCE="https://www.ech.ch/sites/default/files/imce/eCH-Dossier"

dl() {  # $1 = URL, $2 = Zieldatei
  echo "  -> $2"
  curl -fsSL --max-time 60 -o "$2" "$1"
  head -c20 "$2" | grep -q '<?xml' || { echo "FEHLER: $2 ist kein XML"; exit 1; }
}

echo "eCH-0119 (Wurzelschema) + Testdatei:"
B0119="$IMCE/0091-0120/eCH-0119/4.0.0/Beilagen"
dl "$B0119/eCH-0119-4-0-0.xsd"                 eCH-0119-4-0-0.xsd
dl "$B0119/test.eCH-0119.v4.0.0.major.xml"     test.eCH-0119.v4.0.0.major.xml

echo "eCH-0119 v3.0 (Basis des ZH-Steuererklärungs-Barcodes, ssk-prefixed):"
dl "$IMCE/0091-0120/eCH-0119/3.0/Beilagen/eCH-0119-2015-3-0.xsd"  eCH-0119-2015-3-0.xsd
dl "$IMCE/0031-0060/eCH-0046/3.0/Beilagen/eCH-0046-3-0f.xsd"      eCH-0046-3-0f.xsd

echo "eCH-0196 (eSteuerauszug) – Referenz für den Reader (src/ech0196.rs):"
dl "$IMCE/0181-0210/eCH-0196/2.2.0/Beilagen/eCH-0196-2-2.xsd"  eCH-0196-2-2.xsd

echo "Direkt & transitiv importierte Schemas (xmlns-Pfad):"
dl "$XMLNS/eCH-0097/5/eCH-0097-5-0.xsd"        eCH-0097-5-0.xsd
dl "$XMLNS/eCH-0006/2/eCH-0006-2-0.xsd"        eCH-0006-2-0.xsd
dl "$XMLNS/eCH-0135/1/eCH-0135-1-0.xsd"        eCH-0135-1-0.xsd
dl "$XMLNS/eCH-0007/5/eCH-0007-5-0.xsd"        eCH-0007-5-0.xsd
dl "$XMLNS/eCH-0007/5/eCH-0007-5-0f.xsd"       eCH-0007-5-0f.xsd
dl "$XMLNS/eCH-0008/3/eCH-0008-3-0f.xsd"       eCH-0008-3-0f.xsd
dl "$XMLNS/eCH-0010/5/eCH-0010-5-0f.xsd"       eCH-0010-5-0f.xsd

echo "Framework-Schemas (-f), nur im Beilagen-Pfad verfügbar:"
dl "$IMCE/0031-0060/eCH-0044/4.1/Beilagen/eCH-0044-4-1f.xsd"  eCH-0044-4-1f.xsd
dl "$IMCE/0001-0030/eCH-0011/8.1/Beilagen/eCH-0011-8-1f.xsd"  eCH-0011-8-1f.xsd
dl "$IMCE/0001-0030/eCH-0007/6.00/Beilagen/eCH-0007-6-0f.xsd" eCH-0007-6-0f.xsd
dl "$IMCE/0031-0060/eCH-0046/5.0/Beilagen/eCH-0046-5-0f.xsd"  eCH-0046-5-0f.xsd
dl "$IMCE/0001-0030/eCH-0010/7.0/Beilagen/eCH-0010-7-0f.xsd"  eCH-0010-7-0f.xsd

echo "eCH-0276 (E-Bilanz und E-Tax JP) + Beispiel:"
B0276="$IMCE/0271-0310/eCH-0276/1.0.0/Beilagen/D_F"
dl "$B0276/eCH-0276-1-0.xsd"                                eCH-0276-1-0.xsd
dl "$B0276/Beispiel_XML_eBalanceSheetETaxLegalEntity.xml"  eCH-0276-beispiel.xml

echo "eCH-0276 Import-Hülle (xmlns-Pfad):"
dl "$XMLNS/eCH-0007/6/eCH-0007-6-0.xsd"        eCH-0007-6-0.xsd
dl "$XMLNS/eCH-0008/3/eCH-0008-3-0.xsd"        eCH-0008-3-0.xsd
dl "$XMLNS/eCH-0010/6/eCH-0010-6-0.xsd"        eCH-0010-6-0.xsd
dl "$XMLNS/eCH-0010/7/eCH-0010-7-0.xsd"        eCH-0010-7-0.xsd
dl "$XMLNS/eCH-0010/8/eCH-0010-8-0.xsd"        eCH-0010-8-0.xsd
dl "$XMLNS/eCH-0044/4/eCH-0044-4-0.xsd"        eCH-0044-4-0.xsd
dl "$XMLNS/eCH-0046/6/eCH-0046-6-0.xsd"        eCH-0046-6-0.xsd
dl "$XMLNS/eCH-0097/2/eCH-0097-2-0.xsd"        eCH-0097-2-0.xsd
dl "$XMLNS/eCH-0097/4/eCH-0097-4-0.xsd"        eCH-0097-4-0.xsd
dl "$XMLNS/eCH-0108/7/eCH-0108-7-0.xsd"        eCH-0108-7-0.xsd
dl "$XMLNS/eCH-0129/6/eCH-0129-6-0.xsd"        eCH-0129-6-0.xsd

cd ..
echo "schemaLocation für Offline-Validierung verdrahten:"
python3 scripts/patch_schema_locations.py schema

echo "Verifikation (offizielle eCH-Testdateien):"
xmllint --nonet --noout --schema schema/eCH-0119-4-0-0.xsd \
  schema/test.eCH-0119.v4.0.0.major.xml && echo "eCH-0119 OK"
xmllint --nonet --noout --schema schema/eCH-0276-1-0.xsd \
  schema/eCH-0276-beispiel.xml && echo "eCH-0276 OK"
