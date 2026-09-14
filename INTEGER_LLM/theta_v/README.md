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
2. Jede Änderung erzeugt einen neuen `theta_v_hash` und verlangt neu
   erzeugte Golden Vectors; `tests/regression/test_theta_v_changes.py`
   lehnt Vektoren ab, die eine andere Fassung nennen.
3. Durchgesetzt wird beim Laden die **Fassungsnummer**: Der Lader lehnt
   ein Artefakt ab, dessen `theta_v.json` eine andere Version trägt als
   die eingebettete `spec.json`, und prüft die Hashes von Gewichten,
   Skalen und Tabellen. Eine Pipeline-Stufe lehnt zusätzlich ab, wenn
   die kanonische Kennung des Artefakts (SHA-256 über
   `version|weights_hash|scales_hash|luts_hash`) nicht der ihres
   Manifests entspricht.

   *Bis zum 2026-09-14 stand hier „Knoten lehnen Inferenz ab, wenn der
   Hash nicht übereinstimmt". Der Hash von `spec.json` selbst wird von
   keinem Knoten verglichen; durchgesetzt werden die beiden Prüfungen
   darüber.*
4. Kein Gleitkomma im Inferenzpfad — ausschließlich die hier spezifizierten
   Ganzzahloperationen; Division nur als arithmetischer Rechtsshift.
