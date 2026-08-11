# theta_v – Numerischer Vertrag

Dieses Verzeichnis enthält den kanonischen numerischen Vertrag θ_v, der die
vollständig ganzzahlige Inferenzausführung spezifiziert (Whitepaper Kap. 6.2).

## Inhalt

- `spec.json` — die kanonische Spezifikation (JSON, sortierte Schlüssel).
  Der SHA-256-Hash dieser Datei ist der `theta_v_hash`, gegen den Knoten die
  Ausführungsvorschrift prüfen.

Die abgeleiteten Hashes der Artefakte (Gewichte, Skalen, LUTs) liegen in den
Manifest-Dateien der exportierten Artefakte (`artifacts/<modell>/`), nicht
hier.

## Regeln

1. `spec.json` ist die Single Source of Truth für den numerischen Vertrag.
2. Jede Änderung erzeugt einen neuen `theta_v_hash`.
3. Knoten lehnen Inferenz ab, wenn der Hash nicht übereinstimmt.
4. Kein Gleitkomma im Inferenzpfad — ausschließlich die hier spezifizierten
   Ganzzahloperationen; Division nur als arithmetischer Rechtsshift.
