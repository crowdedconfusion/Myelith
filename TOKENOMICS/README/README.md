# tokenomics (`myl-tokenomics`)

> **Version:** 0.13.0
> **Datum:** 2026-08-31
> **Status:** Design-Entscheidungen getroffen (Fixed-Point bestätigt,
> vTFE-Skalierung 10⁻⁶, MYL-Kleinstbeträge 10⁶, EMA-Fenster 30 Epochen
> α=2/31); 🎉 **Phasen 1 bis 4 abgeschlossen**, Phase 5 offen
> (Auslastungsboden und Subventionsplan).
> **180 Tests grün** (156 Modultests, 17 adversariale, 4 Eigenschafts-, 3 Akzeptanztests).
>
> **Seit dem 27. August greift die Slashing-Staffelung wirklich.** Bis
> dahin war sie eine Tabelle mit drei Stufen, von denen immer die erste
> galt: Die Zahl der Vorverstöße war eine Eingabe, und niemand füllte
> sie. Der Ledger führt sie jetzt, und `urteil_buchen_gestaffelt` setzt
> Lesen, Bestimmen, Buchen und Vermerken in die einzige Reihenfolge, in
> der sie zusammenpassen.
>
> Gebaut sind damit: Prägefunktion und EMA, Verteilung nach Kap. 5.3, die
> vTFE-Zuschreibungsregel, ganzzahliges `exp()` und Credit-Preisbildung,
> Stake-Hinterlegung und Slashing-Matrix, die Sicherheitsbedingung
> S_min als Prüffunktion, Anlaufphase, Genesis-Verteilungsmechanik und
> der Self-Dealing-Schutz.

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
├── README/                   diese Kurzübersicht
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

### v0.13.0 – 2026-08-31 (⚑ Punkt 38 geschlossen: die Prägung erreicht ein Konto)

Die Kette war in der Mitte durchtrennt. Der Burn wurde gezählt, geglättet,
zu einer Prägung gerechnet und auf fünf Klassen aufgeteilt, **und dann
endete der Weg**: Es gab keinen Übergang, der ein Konto erhöht. Zwei neue
Module schließen die Lücke.

**`zuschreibung.rs`: wer welchen Anteil der Pod-Arbeit hatte.** Abgeleitet
aus Pod-Besetzung und Zuschnitt, nicht aus einem erklärten Feld. ⚑ **Ein
erklärtes Feld wäre eine Behauptung**, die ein Pod intern falsch melden
könnte; von außen sieht niemand, wer welchen Zuschnitt gerechnet hat. Was
abgeleitet wird, kann niemand falsch melden.

**Die Reserve bekommt nichts, und sie wird genannt.** Sie hat nicht
gerechnet, und Bereitschaft ist eine andere Größe mit einer eigenen
Quelle, die es noch nicht gibt. Sie stillschweigend wegzulassen sähe aus,
als hätte es keine gegeben; sie steht deshalb in
`Zuschreibung::reserve_ohne_anteil`, dieselbe Lehre wie bei
`Zuteilung::ohne_pod`.

**Die Redundanz-Normierung bleibt draußen**, und das ist kein Vergessen:
Als Gewicht in einer proportionalen Aufteilung ist eine Halbierung aller
Werte wirkungslos und verschluckt bei ungeraden Werten nur eine Einheit.

**`ausschuettung.rs`: der Epochenabschluss.** Fortschreiben, prägen,
aufteilen, gutschreiben, in dieser Reihenfolge und mit dem ganzen Plan
vor der ersten Zustandsänderung. ⚑ **Es wird nichts geprägt, was nicht
gutgeschrieben wird:** Koordinatoren, Validatoren und Prüfer haben noch
keine Gewichtsquelle, und ihre Anteile ins Treasury zu schieben, damit
die Summe aufgeht, hieße die Geldmenge um unverdiente Beträge zu
vermehren. Sie werden **nicht geprägt** und namentlich benannt. Zu wenig
zu prägen lässt sich nachholen, zu viel nicht.

**Ohne Auszahlungskonto kein Anteil**, und das Gewicht des Übergangenen
zählt nicht: Die Übrigen teilen den vollen Anteil ihrer Klasse. Die
Übergangenen stehen namentlich im Ergebnis.

⚑ **Fund 107: Die Datei, die entscheidet, wer wie viel bekommt, stand
nicht in der Gleitkomma-Prüfliste.** `vtfe.rs` zählt die
Multiplikations-Additionen je Zuschnitt und legt damit jeden Anteil fest;
sie lief seit ihrer Entstehung ungeprüft mit. Aufgefallen ist es nur,
weil zwei neue Dateien daneben eingetragen wurden. **Dieselbe Klasse wie
Fund 84, Fund 44 und Fund 103, zum vierten Mal**, und deshalb steht das
ganze Verzeichnis jetzt in der Vollständigkeitsprüfung, mit
`utilization.rs` als benannter Ausnahme. Der Konsenspfad wuchs damit von
84 auf 88 Dateien.

`Abschlussfehler` hat jetzt `Display` und `Error`, wie jeder andere
Fehlertyp hier.

### v0.12.0 – 2026-08-31 (⚑ Punkt 38: der Burn wurde verbrannt und vergessen)

`epochenabschluss_burn` schreibt den geglätteten Burn um eine Epoche
fort, aus der Epochensumme, die der Ledger seit heute mitzählt. Vier
Tests, zwei Gegenproben.

⚑ **Gefunden bei der Suche nach dem Ort einer Auszahlung.** Kap. 5.2
leitet die Prägung `m_e` aus dem geglätteten Burn ab, den geglätteten
aus dem Burn je Epoche. **Der Ledger zerstörte die Münzen und vergaß
sofort, wie viele es waren:** `burn_to_credits` senkte den Kontostand
und schrieb nirgends mit. Die Prägungsformel hatte damit keine Eingabe
im Zustand, und `ema_update` war eine Funktion ohne Aufrufer außerhalb
der Simulation.

**Gezählt wird das Verbrannte, nicht das Gutgeschriebene.** Der
Rundungsrest bei der Umrechnung in Credits ist ebenfalls vernichtet und
gehört dazu.

⚑ **Die Fortschreibung darf je Epoche genau einmal laufen.** Zweimal
gerufen zieht sie den Durchschnitt ein zweites Mal in Richtung derselben
Beobachtung, und das Ergebnis sähe unauffällig aus. Der Zustand merkt
sich deshalb, bis zu welcher Epoche fortgeschrieben ist, und ein zweiter
Aufruf ist ein Fehler und kein Wiederholungsversuch.

**Sie steht hier und nicht im Ledger**, weil die Formel zur Wirtschaft
gehört und `myl-tokenomics` ohnehin an `myl-ledger` hängt; umgekehrt
wäre es ein Ring. Dasselbe Muster wie beim Slashing.

**Damit ist ein Glied der Kette geschlossen, nicht die Kette.** Damit
eine Prägung ein Konto erreicht, fehlen drei weitere Stücke, und jedes
ist eine Festlegung: Ein PoI-Bündel trägt keine Zuschreibung je Miner
und keine Adresse; `MinerId` und `Address` sind absichtlich verschiedene
Typen über denselben Bytes, und ob ein Miner unter seiner Kennung oder
unter einer eingetragenen Adresse bezahlt wird, ist ungeklärt; ein
Treasury-Konto gibt es nicht.

### v0.11.0 – 2026-08-29 (die Summe stimmt jetzt nachweislich immer)

`die_summe_stimmt_immer_exakt` hieß der Test, der **eine** Summe prüfte.
⚑ **Bei Geld ist das die teuerste Stelle für diese Schwäche:** Ein
Verfahren, das in einem von tausend Fällen eine Einheit verliert oder
erfindet, fällt an keinem getippten Beispiel auf und bricht trotzdem die
Invariante „die Prägung wird vollständig verteilt".

Jetzt **erschöpfend** über alle Prägungen bis 100 000, dazu 200 000
gestreute über den ganzen `u64`-Bereich und die Ränder. Für
`split_proportional` ein deterministischer Generator mit Dubletten,
Nullgewichten und Beträgen unterhalb der Empfängerzahl.

⚑ **Und der Test zählt mit, wie oft der interessante Fall vorkam.** Ein
Test, der nur glatte Teilungen sieht, prüft die Restverteilung nie und
meldet trotzdem grün.

⚑ **Erschöpfend, wo der Raum es zulässt, und erst dann ein Generator.**
Die Frage nach `proptest` war im Projekt zweimal verschieden
beantwortet worden: einmal ablehnend wegen der Abhängigkeit, einmal
befürwortend wegen der verkleinerten Gegenbeispiele. **Aufgelöst durch
eine dritte Möglichkeit, die beide übersahen:** Für einen großen Teil
dieser Aussagen ist der Eingaberaum klein genug, um ihn **ganz**
abzugehen. Ein erschöpfender Test ist stärker als jeder Zufallstest,
braucht keine Abhängigkeit, und ein Gegenbeispiel muss man nicht
verkleinern, wenn man ohnehin bei den kleinsten anfängt.

### v0.10.0 – 2026-08-27 (die Staffelung bekommt ihre Vorgeschichte)

**Die Staffelung aus Kap. 5.5 war bis dahin eine Absichtserklärung.**
`satz_gestaffelt` kannte drei Stufen (1/3/5 % bei Nichtverfügbarkeit,
30/65/100 % beim Validator) und nahm die Zahl der Vorverstöße als
Eingabe entgegen. **Gefüllt hat sie niemand:** Die einzigen Aufrufer
gaben eine getippte `0` mit. Es galt immer die erste Stufe, und ein
Miner, der dauerhaft ausfällt, wurde behandelt wie einer mit einer
schlechten Nacht.

Der Ledger führt die Vorgeschichte jetzt (`myl-ledger` v0.3.0). Neu
hier:

- **`satz_aus_ledger`** — der gestaffelte Satz mit der Vorgeschichte aus
  dem Zustand statt aus der Hand des Aufrufers.
- **`urteil_buchen_gestaffelt`** — lesen, bestimmen, buchen, vermerken,
  in dieser Reihenfolge.

⚑ **Warum die zweite Funktion nötig ist, obwohl sie nur zwei andere
aufruft:** Der Satz hängt an der Vorgeschichte **vor** dem Urteil, und
das Buchen verändert genau diese Vorgeschichte. Wer die Aufrufe von Hand
setzt, kann sie vertauschen, und der Fehler ist **still** — es wird
geschlachtet, nur eine Stufe zu hoch. Ein Test stellt beide Reihenfolgen
nebeneinander und hält fest, dass sie verschiedene Sätze ergeben; wären
sie gleich, wäre die Reihenfolge gleichgültig und die Funktion
überflüssig.

**`WIEDERHOLUNGSFENSTER` ist keine eigene Zahl mehr**, sondern
`myl_ledger::VERSTOSS_FENSTER`. Zwei Zahlen dafür wären die gefährlichere
Bauart, weil die Abweichung leise ist: Läge das Staffelungsfenster über
der Aufbewahrung, läse die Staffelung eine Vorgeschichte, die es nicht
mehr gibt, und der Zähler stünde einfach niedriger. Niemand bekäme eine
Fehlermeldung.

**Belegt mit vier Tests, davon zwei Gegenproben:** Drei Urteile
hintereinander ergeben 1/3/5 %; drei Urteile in weit auseinanderliegenden
Epochen ergeben dreimal 1 % (sonst prüfte der erste Test nur, dass
*irgendetwas* den Satz erhöht); der geschlachtete Checker bekommt seine
eigene Vorgeschichte und nicht die des Miners; ein Paar ohne Zeile in
Kap. 5.5 wird abgelehnt statt geraten.

### v0.9.0 – 2026-08-25 (⚑ Fund 60: der MoE-Term in der Gewichtsarbeit)

Nachgetragen am 2026-08-26: Der Eintrag fehlte, obwohl die Version schon
in `Cargo.toml` stand.

⚑ **Fund 60, an derselben Naht wie Fund 51: zwischen COMPUTE_PIPELINE und
TOKENOMICS.** `ModellProfil::macs_je_layer` rechnete die volle
MLP-Breite und **keinen Router**. Bei einem Mixture-of-Experts-Modell rechnet je
Token aber nur `top_k` Experten der Breite `moe_intermediate_size`.

**Der eigentliche Bruch war nicht die Zahl, sondern ihre Herkunft.**
`myl-pod::modell_profil` liest `intermediate_size` aus
`gate_proj.rows()` der ersten Layer. Eine MoE-Layer hat kein `gate_proj`,
sie hat 128 Experten mit je einem. Wer dort den ersten nimmt, bekommt bei
Qwen3-30B-A3B **768 statt 6144** und spricht dem Shard ein Achtel seiner
Arbeit zu.

**Behoben:** `ModellProfil` trägt `num_experts`, `num_experts_per_tok`
und `moe_intermediate_size`; `num_experts == 0` heißt dicht, dann rechnet
die Funktion wie zuvor.

⚑ **Ein Zufall, den ein Test festhält.** Bei Qwen3-30B-A3B gilt
`top_k · moe_intermediate_size = 8 · 768 = 6144 = intermediate_size`. Die
dichte Formel träfe dort bis auf den Router-Term **zufällig** zu.
`test_der_zufall_bei_qwen3_30b_ist_keine_regel` hält beides fest: dass
die Zahlen hier zusammenfallen, und dass sie es bei anderem `top_k`
deutlich nicht tun.

**Was sich nicht ändert:** Die Zuschreibung bleibt ohne Anfragezustand
nachrechenbar. Genau darauf kommt es an, denn eine Vergütungsregel, die
den Zustand der einzelnen Anfrage braucht, kann kein Zweiter nachprüfen.

### v0.8.0 – 2026-08-24 (die entschiedenen Punkte umgesetzt)

- **`self_dealing_sicher_konservativ`** prüft gegen das **untere** Ende
  des Bandes aus Anhang B.4 (c = 0,6 ⇒ s < 1,5) und nimmt kein `c` mehr
  entgegen. Damit ist Fund 49 geschlossen: Die Zwei-Schritte-Lücke hat
  keinen ersten Schritt mehr.
- **`satz_gestaffelt`** staffelt die beiden Spannen aus Kap. 5.5 nach
  Wiederholung innerhalb von zehn Epochen: Nichtverfügbarkeit 1/3/5 %,
  Validator 30/65/100 %. Das Fenster ist so lang wie die Arbeitshistorie
  des Stimmgewichts, weil beide dieselbe Frage beantworten.
- **`burn_spielraum`** ist der Burn-Cap je Adresse, den Kap. 5.6 seit v0.1
  als Gegenmittel gegen Self-Dealing nennt und den niemand implementiert
  hatte: ein Zwanzigstel des geglätteten Burns, wirksam ab 1000 MYL. Er
  beantwortet die offene K8-Frage nicht, er **begrenzt** sie: Eine
  einzelne Adresse kann den geglätteten Burn nicht mehr im Alleingang
  bewegen, und wer den Stoß trotzdem will, braucht zwanzig Adressen mit je
  eigener Deckung.
- **`update_price_mit_untergrenze`** beschneidet den Preis gegen eine
  Untergrenze statt gegen null. Null hieße kostenlose Inferenz für alle.

### v0.7.0 – 2026-08-24 (Phase 3 und 4 vollständig)

Vier neue Module, und mit ihnen ist TOKENOMICS bis auf die
adversariale Ebene durch.

| Modul | Punkt | was es rechnet |
|---|---|---|
| `stake.rs` | 3.1 | erforderlicher Stake je beanspruchter Kapazität, und die Umkehrung: welche Kapazität ein Stake trägt |
| `slashing.rs` | 3.2 | die Tabelle aus Kap. 5.5 als Datensatz, liefert `myl_ledger::SlashParams` |
| `anlauf.rs` | 3.4, 4.1 | Stake-Bedarf je Prüfrate, kleinste ausreichende Rate, Trainingsrate |
| `genesis.rs` | 4.2 | Verteilung der Anfangsmenge auf geprüfte Testnetz-Arbeit und Treasury |
| `sicherheit.rs` | 3.3, 4.3 | `S_min = g/p²` und `s < c/(1−c)` |

**Jede Zahl des Papiers, die diese Module betreffen, steht als Test.**
Kap. 5.5 (S_min = 2500 Segment-Rewards), Kap. 5.7 (der Faktor 625 zwischen
2 und 50 Prozent Prüfrate), Anhang B.1 (1250 MYL je Kapazitätseinheit, 25
Epochen-Einkommen), B.8.1 (62 500 MYL für 50 Startminer) und B.8.2
vollständig (250 000 / 40 000 / 10 000 / 1 600 / 400 MYL). Alle stimmen.

*Eine Anmerkung zur Genauigkeit:* Kap. 5.7 sagt, der Stake-Bedarf falle bei
50 statt 2 Prozent „auf ein Sechshundertstel". Exakt sind es (50/2)² = 625.
Der Test prüft die exakte Zahl und hält fest, dass die Aussage des Papiers
eine Rundung ist und keine Ungenauigkeit der Rechnung.

#### Die Arbeitsteilung beim Slashing

Drei Komponenten, jede mit genau einer Frage: VERIFICATION entscheidet
**wer** verloren hat, TOKENOMICS **wie viel** das ist, CONSENSUS bucht es.
Diese Trennung ist teuer erkauft — bis v0.2.6 hatte `myl-verifier` eine
eigene `SlashConfig` mit festen Beträgen, ein zweites, unvereinbares Modell,
das obendrein gar nicht buchen konnte (Fund A9). Die Matrix liefert deshalb
**Anteile in genau der Form, die der Ledger erwartet**; es tippt niemand
eine Zahl ab.

#### ⚑ Was Kap. 5.5 offenlässt

Zwei Zeilen der Tabelle nennen eine **Spanne** statt eines Wertes: „1–5 %
(gestaffelt)" bei Nichtverfügbarkeit und „30–100 %" beim Validator. **Wonach
gestaffelt wird, steht nirgends.**

Der Entwurf setzt beide auf das **untere Ende** und erzwingt die Spanne als
Schranke. Die Begründung ist einseitig und soll es sein: Ein zu niedriger
Slash schwächt die Abschreckung und ist durch eine Parameteränderung
heilbar; ein zu hoher Slash vernichtet den Einsatz eines ehrlichen
Teilnehmers und ist es nicht. Dasselbe gilt für den Faktor der
Trainings-Stichprobenrate, den Kap. 5.5 ebenfalls nicht beziffert. Beides
ist ein offener Punkt.

#### Wie „kein Vorverkauf" durchgesetzt wird

Nicht durch eine Prüfung, sondern durch die **Form der Funktion**:
`genesis_verteilung` nimmt Arbeitsnachweise und sonst nichts. Kein Parameter
für Sonderzuteilungen, keine Liste von Ausnahmen, kein Rest, über den jemand
verfügen könnte. Wer eine Zuteilung außerhalb der Arbeit unterbringen
wollte, müsste die Signatur ändern, und das fällt in einem Diff auf. Eine
Prüfung wäre die schwächere Lösung: Sie ließe den Weg offen und stellte sich
davor.

Ergänzend lehnt die Funktion eine Menge **ohne jeden Arbeitsnachweis** ab.
Sonst fiele sie vollständig ans Treasury, und das wäre genau die Zuteilung
außerhalb der Arbeit, die Kap. 5.7 ausschließt.

#### ⚑ Ein Widerspruch, der beim Lesen von Kap. 5.7 auffiel

Kap. 5.7 sagt ausdrücklich: „Es liegt nahe, ein Gesamtangebot
festzuschreiben oder die Prägung je Epoche zu deckeln. **Beides ist hier
nicht vorgesehen**", und B.8.3 begründet es: Ein bindender Deckel
stabilisiert den Umlauf nicht, sondern bringt ihn zum Erliegen.
`MintParams` trägt trotzdem ein `m_max`, und die Parameter-Registry führt
es als änderbaren Parameter.

Heute harmlos, weil der Vorgabewert `u64::MAX` ist. Die **Möglichkeit**, ihn
scharf zu stellen, widerspricht dem Kapitel. Hängt mit Fund 48 zusammen
(Kap. 10.3 schützt ein „Gesamtangebot", das Kap. 5.7 ausschließt) und ist
ein offener Punkt.

### v0.6.0 – 2026-08-24 (Punkt 3.3: die Sicherheitsbedingung als Funktion)

`src/sicherheit.rs`: `S_min = g/p²` aus Kap. 5.5 und Anhang B.1,
ganzzahlig, mit `p` als Bruch und **aufgerundet** — Abrunden ließe einen
Stake knapp unter der Schranke durchgehen, und die Schranke ist eine
Untergrenze.

**Gegen alle drei Zahlenbeispiele des Papiers geprüft**, nicht gegen
eines: Kap. 5.5 (p = 2 %, g = 1 → 2500 Segment-Rewards; g = 0,5 MYL →
1250 MYL), Anhang B.8.1 (50 Startminer → 62 500 MYL) und die vollständige
Tabelle aus B.8.2 (250 000 / 40 000 / 10 000 / 1 600 / 400 MYL bei 2, 5,
10, 25 und 50 Prozent). Alle fünf Zeilen stimmen.

**Vorgezogen aus Phase 3**, weil `myl-governance` sie für die
Invarianten-Kopplung (GOVERNANCE 1.3) braucht. Ausdrücklich verlangt
ist, dass GOVERNANCE die Funktion **benutzt** statt sie ein
zweites Mal zu schreiben; das Audit vom 2026-08-18 fand mit A7 den Fall,
in dem dieselbe Formel an zwei Orten stand und nur eine gepflegt wurde.

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

Repository-weiter Block; die Einzelheiten stehen im Changelog der
jeweiligen Komponente.

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
