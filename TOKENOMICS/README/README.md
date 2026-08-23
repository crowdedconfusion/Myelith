# tokenomics (`myl-tokenomics`)

> **Version:** 0.3.0
> **Datum:** 2026-08-23
> **Status:** Design-Entscheidungen getroffen (Fixed-Point bestätigt,
> vTFE-Skalierung 10⁻⁶, MYL-Kleinstbeträge 10⁶, EMA-Fenster 30 Epochen
> α=2/31 — Details im Fahrplan); 🎉 **Phase 2 abgeschlossen**
> (`myl-tokenomics` v0.1.1–v0.2.4, Akzeptanzkriterien erfüllt).

Prägefunktion, Burn-and-Mint-Kreislauf, Credit-Preisbildung,
Staking/Slashing-Matrix, Ausgabestruktur und Genesis. Referenzimplementierung
von Whitepaper Kap. 5 und Anhang B.1–B.4, B.7–B.8.

## Aufgabe

Der geschlossene Wertkreislauf (Kap. 5.1): Nutzer verbrennen MYL gegen
Inferenz-Credits, Miner erhalten neu geprägte MYL proportional zur
verifizierten Arbeit. Diese Komponente implementiert die konkreten Formeln
(Prägefunktion, Credit-Preisbildung, Sicherheitsbedingung S_min) auf Basis
der Ledger-Zustandsübergänge aus CONSENSUS. Wo das Protokoll `exp()`
verwendet (Credit-Preisbildung), muss die Approximation ganzzahlig erfolgen
(LUT-basiert), um dieselbe Determinismus-Anforderung wie die Inferenzseite zu
erfüllen.

## Abhängigkeiten

CONSENSUS (Ledger-Zustandsübergänge `burn`/`mint_credits`/`apply_verdict`,
Anhang A.5). Benötigt wird nur die Zustandsübergangs-Schnittstelle — die
fertige BFT-Blockproduktion ist dafür noch nicht vorausgesetzt.

## Struktur

```
TOKENOMICS/
├── README/                   diese Kurzübersicht + Fahrplan
└── myl-tokenomics/           Tokenomik-Berechnungen (Kap. 5)
    └── src/
        ├── lib.rs             Fixed-Point-Grundregeln, Einheiten-Skalierungen
        │                      (1 MYL = 10⁶ Kleinstbeträge, vTFE 10⁻⁶)
        ├── ema.rs             Ganzzahlige EMA für B̄_e (α = 2/31, i128,
        │                      dokumentierte Totzone)
        ├── mint.rs            Prägefunktion M_e = min(B̄_e·(1+s), M_max)
        ├── distribute.rs      Kap.-5.3-Verteilung (Basispunkte, exakte
        │                      Summe, Redundanz-Normierung, proportionale
        │                      Aufteilung)
        └── training.rs        Trainingsvergütungs-Obergrenze (≤ 70 %)
```

## Changelog

### v0.3.0 – 2026-08-23 (Punkt 1.5: die vTFE-Gutschrift bekommt eine Regel)

**Bis hierher war vTFE eine Eingabe.** `redundancy_normalized_weight`
halbierte sie, `distribute.rs` verteilte danach, und wie ein Shard zu
seinem Anteil kommt, stand nirgends. Solange jeder Pod dieselben vier
oder acht gleich großen Shards hatte, fiel das nicht auf. Der Entwurf für
variable Knotenzahl je Pipeline bricht die Annahme: Ein Knoten mit sieben
Layern darf nicht dasselbe bekommen wie einer mit zweien.

**Die Regel** steht jetzt in `src/vtfe.rs`. Ein Token-Forward-Äquivalent
ist der vollständige Vorwärtspass eines Tokens durch das ganze Modell; ein
Shard bekommt davon den Anteil, den er gerechnet hat, gemessen an den
**Multiplikations-Additionen der Gewichtsmatrizen**, die ihm gehören. Alle
Eingaben stehen in `model_config.json` und sind über `theta_v_hash`
gebunden: Jeder Prüfer rechnet dieselbe Zahl nach, ohne den Zustand einer
Anfrage zu kennen.

**Warum nicht Layer, wie der Punkt ursprünglich hieß:** Der LM-Kopf ist
keine Layer, rechnet aber wie viele.

| Modell | eine Layer | LM-Kopf | Kopf in Layern | Anteil am Vorwärtspass |
|---|---|---|---|---|
| Qwen2.5-0,5B | 14,9 M MAC | 136,1 M MAC | **9,13** | 27,6 % |
| Qwen2.5-7B | 233,0 M MAC | 545,0 M MAC | **2,34** | 7,7 % |

Eine reine Layer-Regel gäbe dem letzten Shard bei 0,5B und acht Shards
12,5 %, während er 36,6 % leistet.

**Bewusst draußen:** die Attention-Scores, weil sie an der Kontextlänge
der einzelnen Anfrage hängen und die Gutschrift damit zu einer Größe je
Anfrage machten (benannte Näherung, lange Kontexte sind unterbezahlt);
das Embedding, weil ein Tabellennachschlag nicht rechnet; RMSNorm, RoPE,
SiLU und Residual-Additionen, weil sie drei Größenordnungen unter den
Matrixprodukten derselben Layer liegen.

**Die Eigenschaft, auf die es ankommt**, ist als Test festgehalten:
Zuschnitte von 1 bis 28 Shards verteilen dieselbe Summe, bis auf die
Abrundung. Ohne sie wäre die gemischte Paarung aus dem
COMPUTE_PIPELINE-Entwurf ökonomisch nicht neutral.

### v0.2.6 – 2026-08-18 (Audit-Block 5, Nachtrag)
- `exp_one` und `exp_negative` prüften gegen handgetippte Näherungen
  (`2.71828`, `0.36788`) mit 1 % Toleranz. Seit dem Einfrieren der
  Tabelle ist der erwartete Wert bit-genau bekannt — die Tests
  vergleichen jetzt **exakt** gegen die Golden Vectors
  (e·2³² = 11 675 001 401, (1/e)·2³² = 1 580 039 711). Eine
  Toleranzprüfung hätte einen Drift der Tabelle verschluckt.
- Neuer Test `exakte_erwartungswerte_stimmen_mit_der_konstante_ueberein`:
  bindet die exakten Werte an `std::f64::consts::E` zurück, damit ein
  Zahlendreher in den Golden Vectors auffällt — ein reiner
  Selbstvergleich würde ihn nicht sehen.
- 55 → 56 Tests.


### Audit-Block 5 – 2026-08-18 (Warnungsfreiheit, Tests, Float-Audit)

Repository-weiter Block; die Einzelheiten stehen im jeweiligen Fahrplan.

- **Fund A17 behoben:** 111 Compiler-Warnungen → **0** über alle elf
  Crates. Dabei kamen drei echte Lücken zum Vorschein, die sich hinter
  „harmlosen" Warnungen versteckten (siehe unten).
- **clippy sauber** über alle Crates; `RUSTFLAGS: -D warnings` und ein
  eigener `lint`-Job in der CI verankern den Zustand. Bewusste Ausnahmen
  stehen als `#![allow(...)]` **mit Begründung** im Modulkopf (die
  Kernel-Signaturen tragen den vollständigen Fixed-Point-Vertrag; die
  Matrix-Namen `W`, `W_gate` folgen Whitepaper-Anhang B).
- **Fund A18 behoben:** Das Gleitkomma-Audit prüfte nur INTEGER_LLM
  (20 Dateien). Es deckt jetzt auch den **Konsenspfad** ab (37 weitere
  Dateien aus myl-types, -ledger, -scheduler, -consensus, -tokenomics,
  -verifier). Beide Pfade: null Treffer.

### v0.2.4 – 2026-08-18 (Audit-Block 2: exp-LUT eingefroren)

**Fund A5 — die exp()-LUT wurde zur Laufzeit mit `f64::exp()` gebaut.**
`exp_approx.rs` erzeugte die 2048 Stützstellen beim ersten Aufruf per
`OnceLock` mit `x_float.exp()` und `.round()`. `f64::exp()` ist **nicht**
korrekt gerundet und unterscheidet sich zwischen glibc-Versionen, musl,
macOS-libm und Windows-CRT. Da jeder Node die Tabelle lokal erzeugt,
hätten zwei Nodes auf verschiedenen Betriebssystemen unterschiedliche
Credit-Preise berechnet — ein Konsens-Fork, und zwar genau die Klasse
Nichtdeterminismus, gegen die Whitepaper Kap. 6.2 auf der Inferenzseite
argumentiert. Der Modul-Header behauptete dabei wörtlich „Determinismus:
Bitgleich auf allen Plattformen".

**Zusatzfund im selben Modul:** `step = (EXP_MAX - EXP_MIN) / (LUT_SIZE - 1)`
war eine Ganzzahldivision → 640 statt 640,3126. Die Tabelle endete damit
bei x = 9,990, während der Interpolator bis x = 10,0 indizierte. Ergebnis:
ein systematischer Drift von bis zu **0,97 %** am oberen Rand — die
dokumentierte „<1 % Fehler"-Zusage wurde nur knapp gehalten, und nicht
wegen der Auflösung.

Behoben (Muster von INTEGER_LLM: einfrieren statt zur Laufzeit erzeugen):
- Neues `src/exp_lut_table.rs` mit der eingefrorenen Tabelle, erzeugt von
  `tools/generate_exp_lut.py` (60 Stellen Dezimalgenauigkeit,
  ROUND_HALF_EVEN, exakte Bruch-Stützstellen).
- `exp_approx()` liest nur noch aus der Konstanten — kein Gleitkomma mehr
  zur Laufzeit. Zwischenprodukt der Interpolation auf `i128` gezogen.
- SHA-256 über die Tabelle als Konstante, im Test geprüft: eine
  versehentliche Änderung des Konsens-Felds fällt sofort auf.
- Golden Vectors (12 Stützpunkte, unabhängig mit Dezimalarithmetik
  gerechnet), Genauigkeitsschranke, Monotonie, Klemmverhalten,
  Regressionstest gegen den Step-Bug.
- **Genauigkeit jetzt 0,00125 % statt 0,97 %** (Faktor ~780).
- 45 → 52 Tests.
- **Konsensrelevant:** Die Preisformel liefert andere Werte als zuvor.

### v0.2.3 – 2026-08-17 (Phase 2: Credit-Preisbildung)
- Ganzzahlige exp()-Approximation (LUT-basiert, 2048 Stützstellen,
  lineare Interpolation) für Preisformel P_{e+1} = P_e · exp(κ(u_e − u*)).
  10 Tests, <1% Fehler im Bereich [-10, +10].
- Auslastungsmessung u_e = demanded_vtfe / available_capacity mit
  Fixed-Point-Arithmetik (16 Bit Nachkommastellen). 9 Tests.
- Preis-Update-Funktion update_price() mit Überlaufsicherung (i128).
- Neue Module: `exp_approx.rs`, `utilization.rs`. 19 neue Tests grün.

### v0.1.1–v0.1.4 – 2026-08-13 (Phase 1)
- Durchgehend Fixed-Point-Ganzzahl-Arithmetik: Brüche als
  Zähler/Nenner-Paare, floor-Divisionen dokumentiert,
  u128/i128-Zwischenrechnungen gegen Überlauf — jede Formel ist ein
  Ledger-Zustandsübergang und muss auf jedem Node bitgleich
  nachrechenbar sein.
- Ganzzahlige EMA (α = 2/31, 30-Epochen-Fenster) mit dokumentierter
  Totzone; Prägefunktion mit M_max-Kappung; Verteilung 78/5/10/4/3 %
  mit Summe-exakt-M_e-Invariante (Rundungsrest ans Treasury);
  Trainingsvergütungs-Obergrenze 70 %.
- Akzeptanzkriterium erfüllt: 10.000-Epochen-Tests (Determinismus und
  Verteilungsexaktheit). 26 Tests grün, keine Warnungen.
