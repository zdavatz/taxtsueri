#!/usr/bin/env python3
"""Verdrahtet die vendorierten eCH-XSDs für die Offline-Validierung.

Setzt bei JEDEM <xs:import> das schemaLocation auf die lokale Datei, die den
betreffenden targetNamespace bereitstellt – egal ob das Original gar kein
schemaLocation hatte oder auf eine absolute www.ech.ch-URL zeigte. Dadurch ist
das Schema-Set self-contained und ohne Netzzugriff validierbar. Idempotent.
"""
import re
import sys
from pathlib import Path

schema_dir = Path(sys.argv[1] if len(sys.argv) > 1 else "schema")

ns_to_file: dict[str, str] = {}
for xsd in sorted(schema_dir.glob("*.xsd")):
    m = re.search(r'targetNamespace="([^"]+)"', xsd.read_text(encoding="utf-8"))
    if m:
        ns_to_file[m.group(1)] = xsd.name

# <xs:import namespace="NS" .../> mit optionalem vorhandenem schemaLocation
import_re = re.compile(r'<xs:import\b([^>]*?)/>', re.DOTALL)
ns_attr_re = re.compile(r'namespace="([^"]+)"')

def fix_import(match: re.Match) -> str:
    attrs = match.group(1)
    m = ns_attr_re.search(attrs)
    if not m:
        return match.group(0)
    ns = m.group(1)
    local = ns_to_file.get(ns)
    if not local:
        return match.group(0)
    return f'<xs:import namespace="{ns}" schemaLocation="{local}"/>'

patched = 0
for xsd in sorted(schema_dir.glob("*.xsd")):
    text = xsd.read_text(encoding="utf-8")
    new = import_re.sub(fix_import, text)
    if new != text:
        xsd.write_text(new, encoding="utf-8")
        patched += 1

print(f"Namespace->Datei: {len(ns_to_file)} Schemas, {patched} Dateien angepasst")
