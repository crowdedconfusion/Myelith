# Sonden: was ein einzelner Rechenbaustein auf dieser Maschine schafft

Kleine, eigenständige Messprogramme. Sie gehören zu keinem Bau und zu keiner
Prüfung; sie beantworten je eine Frage, bevor ein Kern gebaut wird. Jede
Zahl unten ist vom 2026-09-30 auf einem Apple M5 Pro (5 und 10 Kerne, GPU
mit 16 Kernen) und gilt nur dort.

| Datei | Frage | Ergebnis |
|---|---|---|
| `neon_befehle.rs` | Wie viele `sdot` je Takt schafft ein Kern? | rund 3,5; der ternäre Kern nutzt 3 |
| `sonde.swift`, `sonde.metal` | Wie schnell rechnet die GPU eine ternäre Matrix im Bündel? | siehe Tabelle |
| `sme_durchsatz.c` | Wie viele Produkte je Sekunde schafft die Matrixeinheit (SME)? | `smopa` i16: 1 085 G aus einem Faden, 1 600 G aus 16 |
| `sme_befehle.c` | Was kosten einzelne Befehle im Streaming-Modus? | einfache Vektorbefehle 0,28 ns, `smopa` i8 0,47 ns, Lesen aus ZA 6 ns je Paar |

## Die GPU, 17 408 x 5 120, 231 Eingaben

| Weg | T Gewichte mal Eingaben je Sekunde |
|---|---|
| alle Kerne der CPU (`ternaerprobe`) | 0,6 |
| eigener Shader, ein Faden je Zeile und Eingabe | 1,15 |
| eigener Shader, vier Eingaben je Faden | 1,3 |
| `matmul2d` je 128er-Gruppe, mit dem Betrag gewichtet | 2,3 |
| `matmul2d` über die ganze Breite, nur int8 | 6,0 |

Gebaut wurde der vierte Weg; er steht in
`INTEGER_LLM/kernels/src/metal/`.

## Aufrufe

```bash
cd BENCHMARKS/Inferenz/sonden

# GPU: Zeilen, Spalten, Eingaben, Kern (ternaer | matmul), Fadengruppe x und y
swiftc -O sonde.swift -o /tmp/sonde && cp sonde.metal /tmp/ && (cd /tmp && ./sonde 17408 5120 231 ternaer 32 8 v)
(cd /tmp && ./sonde 17408 5120 231 matmul 1)      # matmul2d je Gruppe und über die ganze Breite

# Matrixeinheit (nur Apple ab M4)
clang -O2 -mcpu=apple-m4 sme_durchsatz.c -o /tmp/sme && /tmp/sme 16
clang -O2 -mcpu=apple-m4 sme_befehle.c -o /tmp/ssve && /tmp/ssve

# NEON
rustc -O -C target-cpu=native neon_befehle.rs -o /tmp/ports && /tmp/ports
```

⚠️ `-mcpu=apple-m4` und nicht `-march=armv9-a`: Mit der zweiten Angabe
nimmt der Übersetzer an, SVE gebe es auch außerhalb des Streaming-Modus,
und das Programm bricht mit einem unzulässigen Befehl ab.

⚠️ Die Sonde der GPU prüft Stichproben gegen eine skalare Rechnung und sagt
es, wenn sie abweichen. Eine Zahl ohne diese Zeile ist keine Messung.
