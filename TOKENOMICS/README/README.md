# tokenomics (`myl-tokenomics`)

> **Version:** 0.5.0
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

## Tests

**62 Modultests**, dazu **17 adversariale Tests** in `tests/adversarial.rs`
(Kritikpunkt K4).

Die Modultests belegen, dass die Formeln die vorgesehenen Werte liefern,
also den Erfolgsfall. Die adversariale Ebene prüft die **Eigenschaften,
die nach jeder Rechnung gelten müssen**, über Eingaben, die niemand
ausgesucht hat, einschließlich der Ränder des Zahlbereichs:

| Eigenschaft | warum sie zählt |
|---|---|
| die Verteilung gibt genau die Prägung aus | ein verschwundener Rest ist Geld, das niemand bekommt; ein doppelter ist Geld aus dem Nichts |
| kein Empfänger bekommt mehr als seinen Schlüssel | sonst wäre „Rundungsrest" ein Kanal |
| die Prägung übersteigt nie `M_max` | der einzige harte Deckel der Geldmenge |
| extreme Subventionsparameter prägen nicht aus dem Nichts | ⚑ Fund 46 |
| die proportionale Aufteilung zahlt exakt `total` aus | weder mehr noch weniger |
| doppelte Adressen zahlen nicht doppelt | Gewichte werden zusammengeführt |
| ein EMA-Schritt geht nie über die Stichprobe hinaus | sonst folgte die Prägung einem Wert, den niemand verbrannt hat |
| ein α über 1 läuft nicht um | ⚑ Fund 47 |
| der Preis läuft an keiner Eingabe um | ⚑ Fund 46, dritte Stelle |
| `exp_approx` hält jede Eingabe aus | ein Index außerhalb der Tabelle wäre eine Panik im Konsenspfad |
| die Trainingsvergütung bleibt unter 70 % | Kap. 5.6: sonst wäre Training attraktiver als Inferenz |
| die Redundanz-Normierung rundet nach unten | sonst bekämen zwei Pods zusammen mehr als eine volle Gutschrift |
| ein Zuschnitt beansprucht nie mehr als das ganze Modell | die Abrechnungsgrundlage des Netzes |
| ein Zuschnitt außerhalb des Modells wird abgelehnt | sonst ließe sich Arbeit abrechnen, die es nicht gibt |

**Warum die Ränder und nicht nur plausible Werte:** Alle Parameter dieses
Crates sind für Governance vorgesehen (Kap. 10.3). Eine Abstimmung kann
jeden auf jeden Wert setzen, den der Typ hergibt. „So wird das niemand
konfigurieren" ist keine Zusicherung, sondern eine Hoffnung.

**Die Gegenprobe steht dabei:** Ein vollständiger Zuschnitt muss die
volle Gutschrift bekommen. Eine Funktion, die immer null liefert,
verletzt keine Obergrenze.

## Changelog

### v0.5.0 – 2026-08-23 (adversariale Testebene, K4; ⚑ Funde 46 und 47)

#### ⚑ Fund 46: Die Verbreiterung stand eine Rechnung zu spät, an drei Stellen

Drei Funktionen rechnen ausdrücklich in `u128` bzw. `i128`, „um Überlauf
zu vermeiden", und alle drei liefen trotzdem über, jeweils **eine
Operation früher, als der Kommentar hinsah**:

| Stelle | schmale Rechnung | Wirkung |
|---|---|---|
| `mint_amount` | `(den + num) as u128` — die Addition ist `u64` | Prägung entspricht nicht der Formel |
| `update_price` | `utilization_e - utilization_target` in `i64` | Vorzeichen kippt: Überlast **senkt** den Preis |
| `update_price` | der Abschluss `as i64` nach der `i128`-Rechnung | aus einem hohen Preis wird ein **negativer** |

Der letzte ist der teuerste: Ein negativer Credit-Preis heißt, dass das
Protokoll Nutzern Geld dafür gibt, Inferenz zu verbrauchen.

In `mint_amount` reicht auch `u128` am Rand nicht (6,8·10³⁸ gegen
u128::MAX ≈ 3,4·10³⁸). Dort wird gesättigt, und das ist **nicht bloß
sicher, sondern exakt**: Sättigt das Produkt, greift der Deckel `M_max`,
und er hätte auch beim wahren, größeren Wert gegriffen.

Alle drei sind erreichbar, weil Subventionsrate, κ und Auslastungsziel
Governance-Parameter sind. Im Debug-Build eine Panik, im Release-Build
eine stille Falschrechnung, also zwei Bauprofile mit zwei Ergebnissen.

#### ⚑ Fund 47: „total" galt nur im Release-Build

Die Doku von `ema_update_with_alpha` sagte zu, die Funktion bleibe „total
und deterministisch" auch für α > 1. Zwei Dinge hielten das nicht: ein
`debug_assert!` ließ sie im Debug-Build **abstürzen** und im Release-Build
weiterrechnen, und der Abschluss `as u64` lief um. Ein überkorrigierender
Schritt kann unter null gehen; `−200 as u64` ist ein Wert nahe 2⁶⁴, und
der geht als geglättetes Burn-Volumen direkt in `mint_amount`, wo er die
Prägung an die Obergrenze treibt.

Der `debug_assert` ist weg, das Ergebnis wird beschnitten. Die Prüfung von
α gehört in die Governance-Schicht; diese Funktion kann sie nicht
ersetzen, sie kann nur aufhören, den Fehler zu verstärken.

#### Eichung

Alle drei Tests sind gegen die wieder eingebauten Fehler geprüft und
schlagen in allen drei Fällen an. Die K8-Rechnung liefert nach den
Korrekturen unveränderte Werte (1,8× bei 7B, 3,2× bei 0,5B).

### v0.4.0 – 2026-08-23 (K8: die wirtschaftliche Frage, gerechnet)

Kritikpunkt K8 lautete: *„Es gibt keine Rechnung dazu, ob verteilte
Ganzzahl-Inferenz mit Redundanzfaktor gegen zentrale GPU-Inferenz
preislich bestehen kann."* Jetzt gibt es sie, als Programm
(`src/bin/oekonomie.rs`), Protokoll in `TOKENOMICS/results/`.

**Warum als Programm und nicht als Tabelle:** Die Prägekurve benutzt
`mint_amount` und `ema_update` aus diesem Crate, also die Formeln, die
auch im Ledger laufen. Eine Nachbildung wäre eine zweite Quelle für
dieselbe Aussage (Fund 34).

#### Kosten je Token

Durchsatz des Ganzzahlpfads gegen bf16, dieselbe Maschine, beide Seiten
im selben Lauf und beide auf der CPU:

| Modell | ganzzahlig | bf16 | Verhältnis | Kostenverhältnis |
|---|---|---|---|---|
| 0,5B | 49,17 t/s | 77,57 t/s | 0,634 | **3,2×** |
| 7B | 10,74 t/s | 9,86 t/s | **1,089** | **1,9×** |

Kostenverhältnis = `(1 / Durchsatzverhältnis) · (r + Stichprobe)` mit
r = 2 und 1 bis 3 Prozent Kontrollsegmenten.

**Bei 7B ist der Ganzzahlpfad schneller als bf16.** Der Durchsatz taugt
damit nicht mehr als Kostentreiber; übrig bleibt im Wesentlichen die
Redundanz, also der Preis der Verifizierbarkeit. Bei 0,5B bleibt ein
Rückstand, weil die Matrizen zu klein sind, als dass sich das Aufteilen
über Threads voll auszahlt.

> **Diese Rechnung stand zuerst bei 3,6× und 9,2×**, und der Unterschied
> kam nicht aus besserer Numerik. Der Integerpfad lief **einkernig**,
> während die Vergleichsseite fünf Threads benutzte. Die Messung war
> richtig und ihre Deutung falsch: Sie maß Quantisierungskosten **und**
> fehlende Parallelität in einer Zahl. Behoben in kernels v0.21.0,
> bitgleich per Konstruktion, 7B dadurch 5,2-mal schneller.
>
> **Das ist der eigentliche Ertrag dieser Rechnung:** Sie hat nicht nur
> eine Zahl geliefert, sondern einen Fehler gefunden, den vier Jahre
> Kernel-Arbeit nicht gefunden hätten, weil er nicht im Kernel lag.

**Was die Rechnung nicht ist:** kein Marktpreis. Beide Seiten sind
CPU-Messungen. Auf GPU verschiebt sich das Bild, und zwar in beide
Richtungen: Vendor-Kernel für Gleitkomma sind hochoptimiert, und Tensor
Cores sind für uns gesperrt, weil sie in reduzierter Breite akkumulieren.
Eine belastbare Zahl braucht eine GPU-Messung.

#### Prägekurve über 200 Epochen

Simuliert mit einem Verlauf aus flachem Verbrauch, Anstieg, Einbruch und
Erholung, Anlaufphase mit 20 % Subvention, danach Zielbetrieb.

**Zwei Befunde, die so in keinem Kapitel stehen:**

**Wachsende Nachfrage wirkt deflationär.** Zwischen Epoche 75 und 100
steigt der Verbrauch, und der Umlauf **sinkt** von 4733 auf 282 MYL,
obwohl subventioniert wird: Die EMA hinkt nach, es wird weniger geprägt
als verbrannt.

**Die Trägheit schneidet in beide Richtungen, und die zweite ist die
unangenehme.** Beim Einbruch in Epoche 100 fällt der Verbrauch sofort,
die Prägung folgt der EMA und fällt langsam; in 25 Epochen wächst der
Umlauf von 282 auf 30 222 MYL.

Damit ist eine Angriffsfläche benannt: Wer den Verbrauch hochtreibt und
dann aussteigt, lässt eine Prägung zurück, die der EMA folgt. **Ob das
lohnend ist, hängt am Preis und ist mit dieser Rechnung nicht
beantwortet.** Das ist der nächste offene Punkt von K8.

**Die Prägeobergrenze `M_max` hat in diesem Verlauf nie gegriffen.** Sie
ist damit hier nicht geprüft, sondern nur nicht verletzt worden.

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
