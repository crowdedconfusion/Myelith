# consensus (`myl-consensus` + `myl-ledger` + `myl-scheduler`)

> **Version:** 0.76.0 (`myl-consensus` 0.47.0, `myl-scheduler` 0.19.0,
> `myl-ledger` 0.45.0)
> **Datum:** 2026-10-29
> **Status:** Design-Entscheidungen getroffen (malachite hinter
> trait-Grenze mit Eigenbau-Fallback, Blockzeit 2 s, Komitee 21/7,
> Streitfrist 7 Tage, Reed-Solomon k=8/m=4);
> Phase 2 ✅ abgeschlossen (`myl-ledger` v0.1.1–v0.1.5,
> `myl-scheduler` v0.2.1–v0.2.9); **Phase 3 ✅ vollständig**
> (`myl-consensus` v0.3.1–v0.5.0): signiertes, stimmgewichtetes BFT mit
> VRF-rotierender Komiteewahl, Double-Signing-Beweis und seit v0.5.0
> Rundenwechsel mit Sperrmechanik — Safety **und** Liveness, die
> Akzeptanz-Testmatrix über 21 simulierte Validatoren läuft;
> **Phase 4 ✅ vollständig** (4.1 PoI-Bündel-Einreichung,
> 4.2 Epochenabschluss, 4.3 DA-Schicht).
> ⚑ **Phase 1 hatte am 2026-08-28 zwei Lücken und hat jetzt keine
> mehr:** Session-Kontrakte stehen im Ledger, **Anweisungen sind
> unterschrieben** (Fund 85), und es gibt eine Überweisung von Konto zu
> Konto.
> **512 Tests grün** (272 `myl-consensus`, 131 `myl-scheduler`,
> 109 `myl-ledger`), über alle Testbinaries gezählt.
>
> ⚑ **Seit dem 27. August trägt der Blockkopf eine Höhe.** Er hieß bis
> dahin `EpochMeta`, führte kein Höhenfeld, und die Probekette schrieb
> ihre Höhe deshalb in `epoch` — eine Doppelbelegung, an der jede Frist
> „je Epoche" in Wahrheit „je Block" bedeutete.
>
> **Seit dem 27. August führt der Ledger eine Verstoßhistorie je Konto**
> (`myl-ledger` v0.3.0). Sie ist Konsensfeld und die Voraussetzung der
> Slashing-Staffelung aus Kap. 5.5: Wer wiederholt auffällt, verliert
> mehr.
>
> **Seit dem 26. August laufen die BFT-Runden über ein echtes Netz.**
> Die Verdrahtung liegt in der Komponente NODE; hier kam die Form auf
> der Leitung dazu (`Konsensnachricht`) und die Bindung der
> Zertifikatsrunde an die Signatur des Vorschlags.

BFT-Blockproduktion, Proof-of-Inference-Aggregation, Staking/Slashing,
Ledger-Zustandsübergänge, deterministischer Epochen-Scheduler.
Referenzimplementierung von Whitepaper Kap. 3.5 und Anhang A.2/A.5.

## Aufgabe

Schicht L1: der Konsens, der unabhängig von der Inferenz-Latenz läuft
(Kap. 3.2). Zwei entkoppelte Prozesse (Kap. 3.5.2): schnelle
BFT-Blockproduktion (Prozess A) und epochenweise PoI-Abrechnung (Prozess B).
Dazu der deterministische Epochen-Scheduler (Anhang A.2), der ohne zentrale
Instanz aus dem Blockhash Miner-Zuteilung, Pod-Bildung und
Stichprobenlotterie ableitet.

## Abhängigkeiten

SHARED_TYPES, NETWORKING (Block-Gossip). Enthält selbst keine
Inferenz-Verifikation (siehe VERIFICATION) und keine Tokenomik-Berechnung im
Detail (siehe TOKENOMICS) — bildet aber deren gemeinsame Grundlage
(Ledger-Zustandsübergänge, Staking/Slashing-Buchhaltung).

## Struktur

```
CONSENSUS/
├── README/                   diese Kurzübersicht
└── myl-ledger/               L1-Ledger (Anhang A.5)
    ├── src/
    │   ├── lib.rs             Konsens-Grundregeln (reine Funktionen,
    │   │                      BTreeMap-Ordnung, Ganzzahligkeit,
    │   │                      Überlaufsicherheit)
    │   ├── state.rs           Kontenmodell (Balance/Stake/Credits),
    │   │                      State-Commitment (SHA-256 über Borsh)
    │   └── transitions.rs     burn→mint_credits, apply_verdict,
    │                          credit_spend (atomare Übergänge)
    └── tests/
        └── determinism.rs     Replay-Akzeptanztest: gleiche Folge ⇒
                               bitgleiches Commitment (zwei unabhängige
                               Läufe, inkl. 1.000-Übergangs-Folge)
```

`myl-consensus` (BFT) und `myl-scheduler` (Epochen-Zuteilung) liegen
daneben. Die BFT-Module im Überblick:

```
myl-consensus/src/
├── validator.rs        Registrierung, VRF-rotierende Komiteewahl,
│                       VotingSet (wer darf, mit welchem Schlüssel,
│                       mit welchem Gewicht)
├── bft.rs              eine Runde: Propose/Vote/Commit, signiert und
│                       stimmgewichtet
├── round_change.rs     mehrere Runden: Timeouts, Leaderwechsel,
│                       Sperre/Entsperrung, PolkaCertificate
├── poi.rs              PoI-Bündel-Einreichung (Prozess B):
│                       Signierbotschaft, Pod-Mitgliedschaft,
│                       Aggregat-Prüfung, Annahme-Registry
├── epoch_close.rs      Epochenabschluss: bestätigte Arbeit,
│                       Rückbuchung widerlegter Segmente,
│                       Streitfrist
├── da.rs               Datenverfügbarkeit: Fragment-Commitment,
│                       Aufbewahrung über die Streitfrist,
│                       definiertes Verhalten nach Ablauf
├── signing.rs          kanonische, domain-getrennte Signierbotschaften
├── voting_weight.rs    Stimmgewicht aus Stake und Arbeitshistorie
├── double_signing.rs   Erkennung + nachprüfbarer BLS-Beweis
└── block.rs            Blockinhalt nach Anhang A.5

myl-consensus/tests/
└── liveness.rs         Akzeptanz-Testmatrix Phase 3 über 21 Validatoren
```

## Changelog

### v0.76.0 (2026-10-30): Durchsicht des Zustands auf unbegrenzte Einträge (`myl-ledger` 0.45.0)

Nach dem Fund bei den Credits (v0.75.0) jedes Feld von `LedgerState` und
seinen Sammlungen durchgesehen: Ist es begrenzt, und wodurch? Begrenzt durch
Fristen, Fenster, Ablauf oder Leeren je Epoche sind die übrigen; zwei waren
es nicht, beide im kleinen Register, das jeder zustandslose Knoten ganz hält.

- ⛔️ **Künftige Zusagen höchstens `ZUSAGE_VORLAUF_EPOCHEN` (8) voraus.**
  Von den geltenden blieb je Miner nur die jüngste, die künftigen waren
  unbegrenzt: Je Gebühr liess sich eine weitere für jetzt+1, +2 … stapeln.
  Der Knoten sagt für die nächste Epoche zu; je Miner bleiben höchstens neun.
- ⛔️ **`doppelbelege_aufraeumen` am Epochenwechsel.** Jeder gebuchte
  Widerspruch blieb für immer; gebraucht wird er nur gegen doppeltes
  Schlachten innerhalb der Sperrfrist. Fällt, wenn `gebucht + SPERRFRIST <
  jetzt` (dann liegt auch die Tat davor, und ein neuer Beleg würde ohnehin
  abgewiesen).
- ⚠️ `speicher_aufnehmen` hat noch keinen Aufrufer ausserhalb der Tests;
  wer es verdrahtet, braucht eine Grenze oder eine Bezahlung.
- Tests `doppelbelege_fallen_nach_der_sperrfrist`, Vorlauf in
  `eine_zusage_gilt_ab_ihrer_epoche_unter_dem_registrierten_schluessel`.

### v0.75.0 (2026-10-30): Credits je Verfall zusammengelegt (`myl-ledger` 0.44.0)

📌 Fund aus dem langen Podlauf `nacht-bereit`, aus der Klasse von Fund 144:
Jeder Kauf von Credits (`burn_to_credits`) hängte an `AccountState.credits`
einen Eintrag an; zusammengelegt wurde nie, Verfallenes nur beim Ausgeben
entsorgt. Der Testverkehr verbrennt laufend, und so wuchs der Zustand von
32 KB auf 2,6 MB in anderthalb Stunden, bei gleichbleibender Zahl von Konten.
Jeder Zeuge, der ein solches Konto berührte, trug die ganze Liste: Median
526 KB je Block statt unter 1 KB, und die zustandslosen Knoten kamen nicht
mehr nach (siehe NODE 0.98.1).

- ⛔️ **Ein Eintrag je Verfalls-Epoche**, und ein Kauf entsorgt Verfallenes
  (in einem Übergang, nie beim Lesen, wie bei `gekuendigt`). Den Verfall
  legt der Knoten fest (Epoche plus eins), nicht der Käufer; je Konto
  bleiben damit höchstens zwei Einträge.
- Ein Überlauf beim Zusammenlegen wird vor jeder Änderung abgelehnt.
- ⚠️ Die Zustandswurzel ändert sich für Konten mit mehreren Käufen
  derselben Epoche: Konsensänderung, nur zusammen mit allen Knoten.
- Test `credits_bleiben_begrenzt` (tausend Käufe ergeben einen Eintrag,
  Verfallenes fällt, Überlauf ändert nichts).

### v0.74.0 (2026-10-30): Der Zeugensammler für mehrere Lieferanten (`myl-consensus` 0.47.0)

Seit auch volle Knoten Zeugen liefern (NODE 0.98.0), kann ein Block mehrere
Köpfe haben.

- ⛔️ **Je Unterzeichner ein Kopf, auch für einen bekannten Block**: Bisher
  galt die Grenze nur, solange der Block unbekannt war; danach hätte einer
  beliebig viele Köpfe zum echten Blockhash unterschreiben und den echten
  aus der Bytegrenze drängen können.
- ⚑ **`fehlende` über alle Köpfe des Blocks**: Nur die Lücken des ersten
  Kopfes zu fragen hiesse, dass ein anderer nie fertig wird, wenn der erste
  Unterzeichner schweigt.
- `aufnehmen` gibt den Unterzeichner mit heraus: Der Knoten sperrt ihn, wenn
  sein Zeuge nicht trägt.
- Test `der_sammler_ist_begrenzt` erweitert (ein Kopf je Unterzeichner beim
  bekannten Block, Lücken zweier Köpfe).

### v0.73.0 (2026-10-30): Der grosse Baum auf der Platte (`myl-ledger` 0.43.0)

- ⚑ **`LedgerState::baum_auslagern(pfad)`** (SHARED_TYPES 0.62.0): Der grosse
  Baum lagert in Seiten aus; ein Neubau (erster Bau, Sicherheitsnetz)
  lagert wieder dorthin aus. `baum_groessen` für Angaben.
- ⛔️ **Eine Kopie vergisst die Ablage**: Öffnete ein Neubau der Kopie
  dieselbe Datei, leerte er die Seiten des Originals.
- 📌 **`grosswurzel_fest` ohne Kopie**: Die Kopie eines ausgelagerten Baums
  lädt ihn ganz, und das liefe an jedem Prüfpunkt (leichter Abzug).
- Test `ausgelagert_ist_derselbe_zustand` (20 Blöcke: Wurzel, Zeuge und
  Kontobeweise ausgelagert wie im Speicher).

### v0.72.0 (2026-10-30): Vorgemerkte Auszahlungen (`myl-ledger` 0.42.0)

- ⚑ **`auszahlungen_offen`** (klein, Nummer 15 im Zustandsbaum): je Konto,
  was aus Ausschüttungen zusteht und noch nicht gebucht ist.
  `auszahlung_vormerken` fasst je Konto zusammen; **`auszahlungen_buchen`**
  bucht höchstens `AUSZAHLUNGEN_JE_BLOCK` (4096, die Partitionsgrösse von
  SIMD-0118), nach Adresse. Im kleinen Baum, weil der Plan mit den Minern
  wächst wie ihr Register: So berührt der Wechselblock keinen Zeugen, und
  jeder Block höchstens 4096 Konten aus Ausschüttungen.
- Test `vorgemerkte_auszahlungen_werden_begrenzt_gebucht`.

### v0.71.0 (2026-10-29): Das Aufräumen der Sitzungen ist begrenzt (`myl-ledger` 0.41.0)

- ⛔️ **`SITZUNGEN_JE_AUFRAEUMEN` = 10 000** je Epochenwechsel; der Rest rückt
  in das Fach der nächsten Epoche. Ohne Grenze könnte jemand viele
  Sitzungen mit derselben Ablaufepoche eröffnen und den Zeugen des
  Wechselblocks über die Grenze der Zeugenstücke (48 MiB) treiben;
  zustandslose Knoten blieben dort hängen. Eine abgelaufene Sitzung gibt
  nichts mehr aus, sie fällt nur später weg. `sitzung_aufraeumen_hoechstens`
  für Tests.
- ⚑ **Die Zeugengrösse in der Gebühr, kritisch geprüft: keine eigene
  Gebühr.** Jede Anweisung berührt höchstens eine kleine feste Zahl grosser
  Einträge (Absender, Empfänger, Sitzung samt Fach und Hinterlegung); die
  Anweisungen mit Listen berühren nur Kopf und Register. Der Zeuge wächst
  also mit der Zahl der Transaktionen, und die ist über Bytes und
  Prüfeinheit bepreist; der Fall aus der Analyse von Dedaub (billige
  Transaktion, grosser Zeuge) besteht nicht. Offen bleibt die Ausschüttung
  am Epochenwechsel bei sehr vielen Minern (siehe NODE).
- Test `das_aufraeumen_ist_begrenzt`; 136 Tests grün, Clippy sauber.

### v0.70.0 (2026-10-29): Die leichte Form und Kontobeweise (`myl-ledger` 0.40.0)

- ⚑ **`LedgerState::leicht_bytes` und `aus_leicht`**: der Zustand ohne die
  grossen Sammlungen, dahinter die Wurzel des grossen Baums; daraus ein
  zustandsloser Stand mit derselben Zustandswurzel. ⛔️ `aus_leicht` weist
  ab, wenn in einer grossen Sammlung etwas steht (die grosse Wurzel deckte
  es nicht, und so kämen ungeprüfte Konten hinein), und was nicht kanonisch
  ist.
- ⚑ **`Kontobeweis`**, `kontobeweise` (nur ein voller Stand) und
  `kontobeweis_traegt` (gegen die gehaltene grosse Wurzel): ein Konto oder
  sein Fehlen, belegt durch den Pfad im grossen Baum.
- Test `die_leichte_form_und_kontobeweise` (dieselbe Wurzel, danach mit
  Zeugen weiter; geschmuggeltes Konto, Anhängsel, gefälschtes Guthaben und
  behauptetes Fehlen werden abgewiesen); 135 Tests grün, Clippy sauber.

### v0.69.0 (2026-10-29): Zeugen auf dem Draht, keine Hintertür in der Sammlung (`myl-consensus` 0.46.0, `myl-ledger` 0.39.0)

- ⚑ **`zeuge`** (neues Modul in `myl-consensus`): Der Zeuge eines Blocks geht
  in **Stücken** von höchstens 1 MiB, höchstens 48. Der Erzeuger
  unterschreibt einen **`Zeugenkopf`** (Höhe, Blockhash, Hashes aller
  Stücke, `DST_ZEUGENKOPF`); jedes **`Zeugenstueck`** trägt ihn und ist für
  sich prüfbar (`passt`, `traegt`). Die Sicherheit hängt nicht daran (ein
  Zeuge belegt sich über die Wurzel vor dem Block), wohl aber die
  Lebendigkeit: Ohne Unterschrift könnte jeder einem Block ein falsches
  Stück voranschicken oder das Netz der Zustandslosen fluten.
- ⚑ **`Zeugensammler`**, begrenzt: Bytes insgesamt (das älteste Unfertige
  fällt), Köpfe je Höhe (`KOEPFE_JE_HOEHE` = 4) und je Höhe und
  Unterzeichner einer. **`erwarten(hoehe, blockhash)`**: Ist der Block
  bekannt, gilt dort nur noch sein Kopf, und der bekommt immer Platz; wer
  vorher Köpfe mit Unsinn unterschrieb, verdrängt den echten nicht.
  `fehlende` für die Nachforderung.
- ⛔️ **`Sammlung` ohne `Deref`**: Darüber war jede `BTreeMap`-Methode
  erreichbar, die die Sammlung nicht selbst hat (`get_key_value`,
  `first_key_value` …), und ein solcher Lesezugriff ginge still an
  Aufzeichnung und Abdeckung vorbei; ein Prüfer rechnete dann mit einem
  Wert, den kein Zeuge belegt. Jetzt muss jeder Zugriff durch die
  Sammlung; **`als_map`** gibt die ganze `BTreeMap` nur ausdrücklich und
  zählt als Zugriff auf das Ganze; `s[&k]` liest über `get`.
- ⛔️ **Stille Zugriffe nur in der Kiste** (`wert_still`, `enthaelt_still`,
  `alle`): Sie zählen nicht als Zugriff und gehören nicht in den
  Blockpfad. Für den zustandslosen Knoten gibt es
  **`LedgerState::konten_still`** (nicht im Blockpfad).
- Tests `zerlegt_geprueft_zusammengesetzt`, `der_sammler_ist_begrenzt`;
  Konsens 288, Ledger 134 Tests grün, Clippy sauber.

### v0.68.0 (2026-10-29): Zustandslos prüfen, leere Konten fallen weg (`myl-ledger` 0.38.0)

- ⚑ **Zwei Bäume, Wurzel v3** (zustandslose Validierung, Festlegung des
  Projektinhabers vom 2026-10-29, nach dem Vorbild von NEAR, eigene
  Umsetzung): `SHA-256("myelith-zustand-v3" ‖ SHA-256(Kopf) ‖ kleine Wurzel
  ‖ grosse Wurzel)`. Der **grosse** Baum trägt, was mit den Teilnehmern
  wächst (`GROSSE_SAMMLUNGEN`: Konten, Sitzungen, Speicher,
  Hinterlegungen, Ablauffächer); der **kleine** die Register, die mit den
  Minern wachsen. Ein prüfender Knoten hält den kleinen ganz, vom grossen
  nur die Wurzel.
- ⚑ **Der Zeuge eines Blocks** (`Zeuge`): je grosser Sammlung die
  berührten Einträge mit ihrem Wert davor und der Teilbaum über ihre Pfade
  (SHARED_TYPES 0.60.0). Der Erzeuger: `aufzeichnen_beginnen`, anwenden,
  `zeuge_nehmen`, dann erst `wurzel` (der grosse Baum steht beim Zeugen
  noch auf dem Stand davor). Der Prüfer: `zeuge_einsetzen` (Wurzel davor
  und jeder Eintrag gegen den Teilbaum), anwenden, `wurzel`,
  `zeuge_eingehalten`, `zeuge_abschliessen`; `zustandslos` macht aus einem
  vollen Stand den eines Prüfers mit derselben Wurzel. `Zeugenfehler`:
  unlesbar, fremde Wurzel, Eintrag falsch, unvollständig, nicht voll.
- ⚑ **`Sammlung` zeichnet auf und hält teilweise**: Eigene Lesemethoden
  (`get`, `contains_key`, `iter`, `len` …) gehen denen der `BTreeMap` vor.
  Beim Erzeuger merkt sich jeder Zugriff den Wert davor, gelesen wie
  geändert, auch fehlende; beim Prüfer ist ein Zugriff ausserhalb des
  Zeugen ein **Fehlgriff**, und was über die ganze Sammlung läuft, ist in
  beiden Lagen einer (es lässt sich nicht bezeugen). `anzahl`, `alle`,
  `enthaelt_still`, `wert_still` zählen nicht als Zugriff (für Baum und
  Angaben über den Knoten).
- ⚑ **Ablauffächer der Sitzungen** (`sitzungsablauf`,
  `sitzungsablauf_bis`): Beim Eröffnen kommt eine Sitzung in das Fach der
  Epoche, in der sie wegfällt; `sitzung_aufraeumen` liest nur die fälligen
  Fächer und liefert die weggefallenen. 📌 Bis hierher ein `retain` über
  alle Sitzungen je Epochenwechsel, und die Kette suchte danach über alle
  Hinterlegungen; bei einer Milliarde Teilnehmern ein Durchlauf über den
  ganzen Bestand, und kein Zeuge könnte ihn belegen.
- ⚑ **Nonce-Boden und leere Konten** (kritisch geprüft statt eines Ablaufs
  ungenutzter Konten): Ein Konto, das in Epoche E entsteht, beginnt mit
  der Nonce E·2³² (`AccountState::neu`); `leere_konten_entfernen` nimmt je
  Block die leeren unter den geänderten heraus, O(geändert). Ein
  Staubkonto bläht den Zustand so nicht auf, und eine alte Transaktion
  greift an derselben Adresse nicht wieder. Ein Ablauf mit Wiederbelebung
  über einen Beweis (Stellar, EIP-7736) ist zurückgestellt: Er braucht
  Archivknoten mit allen abgelaufenen Einträgen, und Konten mit Guthaben
  dürfen ohnehin nicht verfallen.
- Gemessen (Release, eine Million Konten, Block mit 2000 Überweisungen,
  200 an neue Konten): ohne Zeugen 22 ms, mit Zeugen 32 ms, **Prüfer ohne
  Konten 29 ms**; Zeuge 1,7 MB (Teilbaum 1,4 MB, rund 4000 Konten).
- Tests `ein_pruefer_ohne_grosse_sammlungen_rechnet_denselben_block`
  (Überweisungen, neues Konto, geleertes Konto fällt weg, Sitzung auf und
  zu; Gegenproben: Zeuge nach dem Block, fehlendes Konto, gefälschtes
  Guthaben), `aufzeichnung_und_teilweise_haltung`,
  `leere_konten_fallen_weg_und_der_nonce_boden_schuetzt`; Ledger 134 Tests
  grün, Clippy sauber; Konsens, Tokenomics, Governance, Verifier,
  Simulation, Testclient grün.

### v0.67.0 (2026-10-29): Jede Unterschrift einmal (`myl-consensus` 0.45.0)

- **`pruefe_alle_ausser`**: wie `pruefe_alle`, ohne die Transaktionen, die
  dieser Knoten schon geprüft hat; das Ergebnis ist dasselbe, die Prüfung
  hängt nur an den Bytes. **`Transaktion::kennung`**: SHA-256 über die
  Bytes samt Unterschrift. 📌 Seit der Prüfung vor dem Mempool prüfte jeder
  Knoten jede Transaktion zweimal; im Durchsatzlauf mit 14 Knoten auf
  einem Rechner sank die Decke von rund 1750 auf rund 1040 je Sekunde.
- Test `schon_gepruefte_werden_uebersprungen`.

### v0.66.0 (2026-10-29): Die Zustandswurzel aus dem Baum (`myl-ledger` 0.37.0)

- ⚑ **Die Zustandswurzel** (Skalierung, Festlegung des Projektinhabers vom
  2026-10-29): `SHA-256("myelith-zustand-v2" ‖ SHA-256(Kopf) ‖ Baumwurzel)`.
  Der Kopf sind die Einzelwerte (erschöpfend zerlegt: Ein neues Feld baut
  nicht, bis es zugeordnet ist); die 14 Sammlungen, die mit Teilnehmern und
  Nutzung wachsen, stehen im Zustandsbaum (SHARED_TYPES 0.59.0).
  📌 Bis hierher SHA-256 über den ganzen Zustand je Block: bei einer
  Million Konten 127 ms, länger als ein Blocktakt.
- ⚑ **`Sammlung`**: eine `BTreeMap`, die geänderte Schlüssel merkt und auf
  Wunsch ein Journal der alten Werte führt; gelesen wird über `Deref`,
  geändert nur über ihre Methoden (`retain` nur lesend, `veraendern` für
  Code, der eine `BTreeMap` will, `nehmen` statt `std::mem::take`).
- **`LedgerState::wurzel`** schreibt den Baum mit dem Geänderten fort,
  **`commitment`** rechnet alles neu (für Tests und seltene Prüfungen),
  beide gleich. **`sichern` / `zuruecksetzen` / `sicherung_verwerfen`**
  statt einer Kopie des ganzen Zustands; **`als_epoche`** für Ableitungen
  unter anderer Epoche ohne Kopie.
- ⚑ **Das Sicherheitsnetz**: Stimmt die Zahl der Einträge im Baum nicht mit
  den Sammlungen, ging eine Änderung am Merken vorbei; dann wird neu
  gebaut (die Wurzel bleibt richtig), in Tests schlägt es laut an.
  📌 Beim Einbau so gefunden: zwei `std::mem::take` am Epochenwechsel.
- Gemessen (Release, eine Million Konten, 2000 Änderungen je Block): 12,5 ms
  je Block statt 157 ms (Wurzel und Kopie); einmaliger Aufbau 3,8 s.
- ⚠️ Die Zustandswurzel ändert sich; eine Kette einer älteren Fassung lässt
  sich nicht weiterführen.
- Tests `die_fortgeschriebene_wurzel_ist_die_neu_gerechnete`,
  `zuruecksetzen_stellt_inhalt_und_wurzel_wieder_her`,
  `das_journal_spielt_alles_zurueck`, `kodiert_wie_eine_btreemap`,
  `veraendern_und_nehmen_werden_gemerkt`; 131 Tests grün.

### v0.65.0 (2026-10-29): Die Rückbuchung (`myl-ledger` 0.36.0)

- ⚑ **`Vorbehalt`** (Whitepaper 6.4: „Rückbuchung der vTFE-Gutschrift“,
  Festlegung des Projektinhabers vom 2026-10-29): Was die Pods einer
  uneinigen Gruppe verdient hätten (je Paar halbiert wie bei einer
  einigen), bleibt zurückgehalten. `vorbehalt_entscheiden`: Wird ein Pod
  überführt, verfällt sein Vorbehalt und der seines Zwillings wird frei;
  `vorbehalte_faellig` zahlt freie am nächsten Wechsel aus und lässt
  unfreie nach der Streitfrist verfallen. So bekommt ein ehrlicher Pod
  seinen Lohn, sobald der andere überführt ist, und hat Grund, den
  Differenzbeweis zu melden. Neu im Zustand (angehängt): `vorbehalte`,
  `rueckbuchungen`.
- 📌 **Kein zweiter Auftrag für dasselbe Segment**: In `gezielt22` traf ein
  Differenzbeweis ein schon gezogenes Segment; der zweite Auftrag bekam nie
  eine Festlegung (sie ging an den ersten) und schloss unvollständig.
  `auftraege_anlegen` überspringt jetzt ein Segment mit offenem Auftrag;
  die Checker wären dieselben.
- Test `der_ehrliche_zwilling_bekommt_seinen_lohn_nach_dem_urteil`, Probe
  im Test des Differenzbeweises; 117 Tests grün.

### v0.64.0 (2026-10-29): Uneinige Zwillinge werden genau geprüft (`myl-ledger` 0.35.0, `myl-consensus` 0.44.0)

- ⚑ **`Uneinigkeit`** (Whitepaper 6.4: eine Abweichung in Stufe 1 „wirkt
  über Slashing“; wer falsch rechnete, entscheidet Stufe 2): Am Wechsel
  legt die Kette jedes Paar ab, das zwei Bündel mit verschiedener
  Ausgabenwurzel einreichte, mit Wurzeln, Kandidaten, Ersatz und
  Besetzung, eine Epoche lang. **`differenz_melden`**: Ein
  Differenzbeweis, der gegen beide Bündel trägt, legt für genau das
  abweichende Segment in beiden Pods einen Prüfauftrag an; höchstens
  `DIFFERENZEN_JE_GRUPPE` (4) je Paar, jedes Segment einmal.
- ⚑ **Warum nicht das ganze Paar ziehen** (kritisch geprüft, Festlegung
  des Projektinhabers vom 2026-10-29): Mit der Obergrenze für volles
  Ziehen könnte das eine falsche Segment unentdeckt bleiben, und ein Pod
  könnte seinen ehrlichen Zwilling mit einem einzigen falschen Segment um
  die Vergütung bringen, ohne sicher gefunden zu werden. Der
  Differenzbeweis findet die Stelle immer und belastet die Prüfer nur mit
  ihr.
- **`Anweisung::DifferenzMelden`** (angehängt). Neu im Zustand
  (angehängt): `uneinigkeiten`.
- Test `ein_differenzbeweis_legt_je_pod_einen_auftrag_an`.

### v0.63.0 (2026-10-29): Die Schiedsrunde (`myl-ledger` 0.34.0, `myl-consensus` 0.43.0)

- ⚑ **`Streitfall`** (Whitepaper 6.6, Festlegung des Projektinhabers vom
  2026-10-29): `faellige_abschliessen` öffnet je Auftrag mit aufgedeckter
  Abweichung einen Streitfall mit der frühesten behaupteten Stelle, dem
  Anfechter und dem **Beschuldigten aus der Besetzung der gezogenen
  Epoche** (`OffeneStichprobe::besetzung`, `Pruefauftrag::besetzung`, je
  Position Miner und Layerbereich). Wurzel und Segmentzahl des Bündels
  stehen im Auftrag und im Fall, damit der Verlauf auch nach weiteren
  Ziehungen geprüft werden kann.
- **`schiedsstimme`** (einmal je Validator, bis zur Frist; ob er im
  Stimmsatz steht, sagt die Kette) und **`streitfaelle_entscheiden`**:
  mehr als zwei Drittel des Gewichts für eine Seite entscheiden
  (`PodFalsch`, `AnfechtungFalsch`), ohne Mehrheit bis zur Frist verfällt
  der Fall. Das feste Fenster folgt Arbitrum BoLD.
- **`Anweisung::SchiedsstimmeAbgeben`** (angehängt).
- Neu im Zustand (angehängt): `streitfaelle`; `Besetzung` als Typname.
- Tests: `das_komitee_entscheidet_mit_mehr_als_zwei_dritteln_oder_der_fall_verfaellt`,
  der Streitfall im Test der einzelnen Abweichung (Beschuldigter aus dem
  Layerbereich).

### v0.62.0 (2026-10-29): Prüfaufträge mit Festlegen vor Aufdecken (`myl-ledger` 0.33.0, `myl-consensus` 0.42.0)

- ⚑ **`myl_ledger::pruefung`** (Whitepaper 3.3, 6.8 (b), Anhang B.3;
  Festlegung des Projektinhabers vom 2026-10-29):
  - `OffeneStichprobe` hält jetzt, wer prüfen darf: `pruefkandidaten` (die
    Miner ohne Pod, darunter die des Pods in Bereitschaft) und je Pod
    `ersatz` (die Mitglieder der übrigen Zwillingsgruppen, genommen nur
    ohne Kandidaten).
  - `auftraege_anlegen`: je Ziehung ein `Pruefauftrag` mit bis zu zwei
    Checkern (`pruefer_waehlen`, geordnet nach `SHA-256("myelith-pruefer" ‖
    Epoche ‖ Pod ‖ Index ‖ Kennung)`) und zwei festen Fristen in Blöcken.
  - `festlegen` bis zur ersten Frist; `aufdecken` erst, wenn alle
    festgelegt haben oder die erste Frist vorbei ist, bis zur zweiten, und
    nur, was zur eigenen `festlegung` passt (Hash über Befund, Salz und
    Kennung). Der `Pruefbefund` trägt den Hash der Logits an der letzten
    Position, den keine Spur enthält.
  - `faellige_abschliessen`: alle gleich aufgedeckt heisst einig, das
    geprüfte Volumen (Positionen) geht auf `LedgerState::pruefvolumen`;
    verschieden oder unvollständig zahlt niemanden. **Jede aufgedeckte
    Abweichung** steht im Abschluss, auch wenn der zweite Checker
    schweigt: Ein ehrlicher Checker genügt.
  - Neu im Zustand (angehängt): `pruefauftraege`, `pruefvolumen`.
- **`Anweisung::PruefungFestlegen` und `PruefungAufdecken`** (angehängt),
  gebündelt bis `PRUEFUNGEN_JE_TX` (64) je Transaktion: Ein Pod allein wird
  voll gezogen, und eine Transaktion je Ziehung sprengte die Grenze je
  Absender im Mempool (so im ersten Test geschehen).
- ⚠️ Die Zustandswurzel ändert sich; eine Kette einer älteren Fassung lässt
  sich nicht weiterführen.
- Tests: `die_wahl_ist_fest_und_ohne_doppelte`,
  `ohne_kandidaten_kommt_der_ersatz_aus_der_anderen_gruppe`,
  `festlegen_dann_aufdecken_und_einig_gibt_volumen`,
  `eine_abweichung_allein_wird_ein_streitfall_aber_nicht_bezahlt`,
  `verschiedene_befunde_ohne_abweichung_sind_uneinig`.

### v0.61.0 (2026-10-28): Zwillingspaare (`myl-scheduler` 0.19.0, `myl-ledger` 0.32.0, `myl-consensus` 0.41.1)

- ⚑ **`myl-scheduler::zwillingspaare`** (Whitepaper 3.4 und 4.4): je zwei
  Pods, die jedes Segment beide rechnen, disjunkt, in der Folge der
  Indizes, mit Vorzug für eine andere bestimmte Zone; ein übriger Pod hat
  keinen Zwilling. Kette und Ortsdienst bilden die Paare aus derselben
  Zuteilung. Probe mit Gegenprobe (wer ein Mitglied teilt, wird kein
  Zwilling).
- **`LedgerState.zwillinge_einig` und `zwillinge_uneinig`** (angehängt):
  gezählt am Epochenwechsel (NODE 0.77.0). ⚠️ Das Format des Zustands
  ändert sich.
- **`LedgerState.stichprobe_wurzeln` und `OffeneStichprobe.wurzeln`**
  (angehängt; Stufe 2): je bezeugtem Bündel der gezogenen Epoche Pod,
  `segments_root` und Segmentzahl. Dagegen prüft ein Knoten eine gelieferte
  Spur (NODE 0.77.0).
- ⚑ **`myl-scheduler::zwillingsgruppen` und `bereitschaft`** (Festlegung
  des Projektinhabers vom 2026-10-28): die Paare; ein übriger Pod bildet
  keine Gruppe, sondern steht in **Bereitschaft** (rechnet nicht, wird nicht
  vergütet, später Checker), bis genug Miner für ein weiteres Paar da sind.
  So bleibt r = 2 (Whitepaper 3.4) ohne Mehrarbeit; eine Dreiergruppe hätte
  dieselbe Arbeit dreimal gerechnet. Welche Miner die Bereitschaft trifft,
  mischt die Epochensaat mit der Podbildung jede Epoche neu. Nur ohne
  jedes Paar (ein Netz mit einem Pod) ist jeder Pod eine Gruppe für sich
  (Anlaufregel). Probe `ein_uebriger_pod_steht_in_bereitschaft` mit
  Gegenproben (zwei, die ein Mitglied teilen: jeder allein, keiner in
  Bereitschaft; leer).
- **`LedgerState.zurueckgehalten`** (angehängt): je Miner die vTFE eines
  Pods, der allein rechnete, mit der Epoche der Freigabe (nach der
  Streitfrist); **`OffeneStichprobe.voll`** (angehängt): die Pods, deren
  Segmente alle gezogen werden (Anlaufregel, NODE 0.77.0).
- `myl-consensus`: Die Bündel der Proben tragen das neue Feld; kein
  anderes Verhalten.
- `myl-scheduler` 0.19.0 (132 Tests), `myl-ledger` 0.32.0 (118),
  `myl-consensus` 0.41.1 (285), grün, Clippy sauber.

### v0.60.0 (2026-10-28): Der Credit-Preis in Feinauflösung (`myl-ledger` 0.31.0)

- ⚑ **`LedgerState::credit_preis_fein`** (neu, am Ende des Zustands):
  der Credit-Preis als Festkomma mit 32 Nachkommabits, zu Genesis aus
  `credit_price`. Die Kette schreibt ihn seit NODE 0.76.0 je Epoche nach der
  Auslastung fort (Whitepaper 5.4, Festlegung des Projektinhabers vom
  2026-10-28: κ = 1/6, Ziel 0,8, Untergrenze der Kleinstbetrag);
  `credit_price` ist sein gerundeter Wert, mindestens 1. 📌 Ohne die
  Nachkommastellen bliebe ein kleiner ganzzahliger Preis stehen: Ein
  Schritt von 3,4 % rundet unter rund 15 Kleinstbeträgen auf null, einer
  von 12,5 % unter rund 4. ⚠️ Der Zustand ändert damit sein Format; eine
  Kette einer älteren Fassung lässt sich nicht weiterführen.
- `myl-ledger` 0.31.0. 118 Tests grün, Clippy sauber; abhängige Kisten
  gegen die neue Fassung geprüft (siehe NODE 0.76.0).

### v0.59.0 (2026-10-28): Die Gebühr zählt auch die Prüfung; Stimmen der Validatoren (`myl-consensus` 0.41.0, `myl-ledger` 0.30.0)

- ⚑ **Gebühreneinheiten statt Bytes** (Festlegung des Projektinhabers vom
  2026-10-28): `block::PRUEFEINHEIT` (1 000) und
  `Transaktion::gebuehreneinheiten`, die Bytes und eine Prüfeinheit je
  Unterschrift. Gemessen kostet die Prüfung einer Unterschrift so viel wie
  rund tausend Bytes; eine Gebühr nur nach Bytes bepreiste das Teuerste an
  einer Transaktion nicht. Der Grundpreis bewegt sich weiter um ein Achtel.
- `myl-ledger` Zustand `pruefpunkt_stimmen`: die Unterschriften je
  Validator in den Prüfpunktzertifikaten der laufenden Epoche, Grundlage
  der Auszahlung des Validatorentopfs (NODE 0.75.0).
- ⚠️ Der Zustand hat ein Feld mehr; eine Kette einer älteren Fassung lässt
  sich nicht weiterführen.
- 285 Tests in `myl-consensus`, 118 in `myl-ledger` grün, Clippy sauber.

### v0.58.0 (2026-10-28): Alle Unterschriften eines Blocks auf einmal (`myl-consensus` 0.40.0)

- ⚑ `block::pruefe_alle`: prüft die Unterschriften vieler Transaktionen
  erst als Stapel (`myl_types::bls::verify_batch`); trägt er, gelten alle
  als geprüft, sonst jede einzeln mit genau dem Ergebnis von `pruefe`. Das
  Ergebnis ist dasselbe wie einzeln, bis auf eine Fehlerwahrscheinlichkeit
  von 2⁻⁶⁴ je Stapel (das Verfahren der Konsensclients von Ethereum). Ein
  Block mit falschen Unterschriften kostet so viel wie bisher.
- Anlass: Ein Block mit 4 096 Transaktionen brauchte beim Anwenden 1 909 ms,
  fast nur für die Unterschriften; mit `pruefe_alle` 362 ms (NODE 0.74.0).
- Probe `pruefe_alle_gibt_dasselbe_wie_einzeln` (alle gültig, eine
  verfälscht, fremdes Netz, eine einzelne).
- 285 Tests in `myl-consensus`, `myl-ledger` 118 unverändert grün, Clippy sauber.

### v0.57.0 (2026-10-27): Gebühren (`myl-consensus` 0.39.0, `myl-ledger` 0.29.0)

- ⚑ **`Transaktion::max_gebuehr`** (Festlegung des Projektinhabers):
  höchstens so viel Gebühr zahlt der Absender; unterschrieben wie alles
  andere (`transaktionsbytes` nimmt das Feld auf). `signiere` sagt ohne
  Grenze zu (`u64::MAX`), `signiere_mit_grenze` mit.
  `Transaktion::gebuehrenbytes` zählt die kodierte Transaktion.
  ⚠️ Ändert die Kodierung jeder Transaktion; alte Ketten sind nicht lesbar.
- `myl-ledger`: Der Zustand führt `grundpreis_je_byte`, `gebuehren_epoche`
  (die Gebühren der laufenden Epoche), `gebuehren_validatoren` (der Topf
  der Validatoren, bis es eine Gewichtsquelle für sie gibt) und
  `hinterlegungen` (je Art und Kennung wer wie viel hinterlegt hat). Die
  Regeln dazu stehen im Knoten (NODE 0.72.0).
- Fuzz-Korpus: zwei Transaktionen mit dem neuen Feld
  (`korpussaat::transaktionen_fuer_den_fuzzer`, über
  `KORPUS_TRANSAKTION_NACH`); die älteren Saaten lesen sich nicht mehr
  vollständig und bleiben als Regression liegen.
- 284 Tests in `myl-consensus`, 118 in `myl-ledger` grün, Clippy sauber.

### v0.56.0 (2026-10-27): Vertrauensfrist und Doppelbelege (`myl-consensus` 0.38.0, `myl-ledger` 0.28.0)

- ⚑ **Die Vertrauensfrist** (Festlegung des Projektinhabers):
  `einsatz::VERTRAUENSFRIST_EPOCHEN = SPERRFRIST_EPOCHEN * 2 / 3`, also 112
  Epochen; 56 bleiben als Rand, in dem ein Beleg für eine Doppelunterschrift
  noch in einen Block kommt. Abgeleitet, damit sie mit der Sperrfrist wandert.
  Wer sie anwendet, steht im Knoten (NODE 0.71.0).
- ⚑ **Doppelbelege** (`double_signing::Doppelbeleg`): zwei Blöcke derselben
  Höhe vom selben Erzeuger (`Belegblock`: Kopf, Rumpf als rohe Bytes,
  Unterschrift) oder zwei Prüfpunktstimmen eines Wählers für dieselbe Höhe.
  `Doppelbeleg::schuldiger` prüft ohne weiteren Zustand: derselbe Schlüssel,
  dieselbe Höhe, verschiedene Werte, beide Unterschriften tragen; bei
  Prüfpunktstimmen folgt die Kennung aus dem Schlüssel. 📌 **Rohe Bytes statt
  eines Blocks**: Ein Beleg mit Blöcken wäre ein rekursiver Typ (Block,
  Transaktion, Beleg, Block), und beliebig tief verschachtelte Belege liessen
  beim Lesen den Stack wachsen. Dafür `Block::rumpfbytes`,
  `Block::hash_aus_teilen`, `block::erzeugernachricht_zum_hash`.
  Blöcke und Stimmen stehen im Beleg in einer `Box`, ebenso der Beleg in
  der Anweisung (Clippy: grosser Unterschied zwischen den Varianten); an der
  Kodierung ändert das nichts.
- Neu angehängt `Anweisung::DoppelbelegEinreichen`; `DoubleSignError` um
  `VerschiedeneTaeter` und `VerschiedeneHoehen` erweitert.
- `myl-ledger`: Der Zustand führt `doppelbelege` (je Schuldiger, Art, Höhe
  die Epoche der Buchung), damit derselbe Widerspruch nur einmal schlachtet;
  Einträge fallen nach der Sperrfrist weg.
- Proben: `zwei_bloecke_derselben_hoehe_belegen_den_erzeuger`,
  `zwei_pruefpunktstimmen_belegen_den_waehler` (Gegenproben: andere Höhe,
  anderer Täter, derselbe Wert, gefälschte Unterschrift, veränderter Rumpf),
  `der_beleg_ueberlebt_borsh`, `die_vertrauensfrist_liegt_mit_rand_unter_der_sperrfrist`.
- 283 Tests in `myl-consensus`, 118 in `myl-ledger` grün, Clippy sauber.

### v0.55.0 (2026-10-27): Prüfpunkte, Stufe 1 der Endgültigkeit (`myl-consensus` 0.37.0, `myl-ledger` 0.27.0)

- ⚑ **Festlegungen des Projektinhabers vom 2026-10-27:** Ziel ist das
  Whitepaper (Kap. 3.5.2: BFT je Block, HotStuff-Familie, Erzeuger rotiert
  per VRF). **Stufe 1 sind Prüfpunkte** alle 150 Blöcke
  (`pruefpunkt::BLOECKE_JE_PRUEFPUNKT`, teilt die Epoche), und zwar über
  **Prüfpunktstimmen statt Runden** (nach Casper FFG und GRANDPA): Jedes
  Mitglied unterschreibt (Höhe, Blockhash) des Blocks, den es hat
  (`Pruefpunktstimme`, eigenes Trennzeichen `MYELITH_PRUEFPUNKT_v1`); ab mehr
  als zwei Dritteln des Gewichts entsteht ein `Pruefpunktzertifikat`.
  Geprüft mit demselben Kern wie die Zertifikate der Runden
  (`round_change::pruefe_aggregat`, jetzt `pub(crate)`). Zwei Zertifikate für
  dieselbe Höhe brauchen ein Drittel Doppelstimmen; deren Schlachtung folgt.
- ⚑ **Kopffeld `pruefpunkt: Option<Pruefpunktzertifikat>` ersetzt
  `saatquelle`** (rohe Bytes eines Commitzertifikats mit Blockhash als
  Rückfall). Formatwechsel: Blöcke und Kettendateien des alten Formats sind
  nicht mehr lesbar. Neu angehängt `Konsensnachricht::Pruefpunktstimme`.
- `myl-ledger`: Der Zustand führt `pruefpunkt_bestaetigt` (bis wohin die
  Kette bestätigt ist) und `stichproben_offen` (`OffeneStichprobe`): Beim
  Epochenwechsel werden nur noch die Anteile der bezeugten Bündel
  aufgehoben; gezogen wird mit dem Zertifikat des Wechsels (NODE 0.70.0).
  Im Zustand, damit auch ein Knoten aus einem Abzug dasselbe nächste
  Zertifikat annimmt.
- Proben in `pruefpunkt.rs`: drei von vier tragen, zwei nicht; andere Höhe,
  keine Prüfpunkthöhe, ein Fremder, ein Commit der Runden als Stimme, zwei
  Blöcke in einem Zertifikat: alle abgewiesen. Die Fuzz-Saaten des Blocks
  sind im neuen Format neu geschrieben (eine ohne, eine mit Zertifikat);
  373 000 Mutationen kanonisch.
- 280 Tests in `myl-consensus`, 117 in `myl-ledger` grün, Clippy sauber.

### v0.54.1 (2026-10-27): Der Auszahlungseintrag überlebt die Abmeldung, mit Probe (`myl-ledger` 0.26.2)

- ⛔️ **Festgehalten, nicht geändert** (Festlegung des Projektinhabers vom
  2026-10-27): `miner_abmelden` lässt das Auszahlungskonto stehen. Würde es
  mit der Anmeldung gelöscht, könnte ein Dieb mit dem heissen Schlüssel
  abmelden, sich mit derselben Kennung neu anmelden und als „erste
  Eintragung“ sein Konto setzen. Wer bestohlen wurde, meldet sich mit einem
  neuen Konsensschlüssel an. Ein Wechsel des Auszahlungskontos mit Frist
  wurde verworfen: Widersprechen dürfte dasselbe Konto, das gestohlen ist.
- Probe `abmelden_und_neu_anmelden_leitet_den_ertrag_nicht_um`; Gegenprobe
  (Eintrag beim Abmelden gelöscht) rot.
- `myl-ledger` 0.26.2. 117 Tests grün, Clippy sauber.

### v0.54.0 (2026-10-27): Der Block trägt die Unterschrift seines Erzeugers (`myl-consensus` 0.36.0)

- ⚑ **`Block::unterschrift: Option<Erzeugerunterschrift>`**, angehängt:
  öffentlicher Schlüssel und BLS-Unterschrift über
  `DST_BLOCK_ERZEUGER ‖ Blockhash` (neues Präfix
  `MYELITH_BLOCK_ERZEUGER_v1`). Der Schlüssel kommt mit und keine Kennung,
  denn aus einem Hash folgt kein Schlüssel; die Kennung folgt aus ihm.
- **Der Blockhash deckt die Unterschrift nicht** und ist damit bytegleich zu
  vorher; bestehende Hashes und Ketten bleiben gültig, und keine
  Unterschrift verändert, welcher Block gemeint ist.
  `Block::erzeuger()` sagt `Ok(None)` (unsigniert), `Ok(Some(kennung))`
  (trägt) oder `Err` (steht da und trägt nicht).
- Probe `die_unterschrift_des_erzeugers_traegt_und_bindet_den_block`: trägt,
  Hash unverändert, Borsh rundweg; Gegenproben: ein veränderter Block und ein
  untergeschobener Schlüssel tragen nicht. Zwei Saaten im neuen Format für
  das Fuzz-Ziel `block`. Die Prüfung beim Übernehmen steht im Knoten
  (NODE 0.65.0).
- `tests/fuzzkorpus.rs`: die Mutationsprobe auf stable wie in `myl-types`
  (`mutationen_der_saat_bleiben_kanonisch`): 373 000 Mutationen über die
  sechs Ziele, 9 759 lesbar, alle kanonisch, Block und Transaktion im neuen
  Format eingeschlossen. Kein Fund. 276 Tests grün.
- **Im Netz belegt** (kurzer Podlauf mit Tor, 2026-10-27): Der Erzeuger
  unterschreibt jeden Block; die Kettendatei eines Miners wuchs um 144 Byte
  je Block (Schlüssel 48, Unterschrift 96), und kein Knoten lehnte einen
  Block wegen des Erzeugers ab. Eine Wurzel bei Höhe 12 637.
- 📌 **`Block::hashbytes()`**: die Bytes, die der Blockhash deckt (der Block
  ohne die Unterschrift). Wer Blockbytes weitergibt, die gegen den Hash
  geprüft werden, gibt diese. Bis zum neuen Feld waren sie gleich der vollen
  Kodierung, und der Archivbeleg verliess sich still darauf
  (`myl_ledger::archiv::archiv_belegen` hasht rohe Bytes); mit dem Feld trug
  er nicht mehr, und kein Knoten verdichtete. Gefunden an den
  Verdichtungstests des Knotens, behoben dort (NODE 0.65.0).
- ⚠️ Das Blockformat hat ein Byte mehr; Kettendateien im alten Format liest
  ein neuer Knoten nicht mehr (vor dem Genesis-Block gewollt einfach).
- `myl-consensus` 0.36.0.

### v0.53.0 (2026-10-26): Zwei Quoren schneiden sich, auch an den Rändern von `u64` (Fund 652; `myl-consensus` 0.35.0)

- 📌 **Fund 652: Ein Gesamtgewicht über `u64` brach den Quorumsschnitt.**
  `VotingSet` summierte die Gewichte in u128 und sättigte auf `u64::MAX`;
  Quorumsschwelle und Stimmenzählung sättigen ebenfalls. Lag die wahre
  Summe darüber, gab es zwei Quoren ohne einen gemeinsamen Teilnehmer,
  also zwei bestätigte Blöcke derselben Höhe. Im Betrieb begrenzt die
  Geldmenge den Stake (das Gewicht ist heute der Stake), aber geprüft war
  diese Schranke nirgends. Jetzt ergibt eine solche Summe eine Menge ohne
  Gewicht: Kein Quorum ist erreichbar, der Konsens steht, statt sich zu
  widersprechen (Sicherheit vor Lebendigkeit). An beiden Konstruktoren,
  `for_producers` und `from_members`; die Schnittstelle bleibt.
- ⚑ **`tests/quorumschnitt.rs`** (neu): der Quorumsschnitt als
  Eigenschaftstest über den echten Code, die offene Frage aus dem
  Auditzuschnitt („schneiden sich zwei Quoren wirklich in mehr als einem
  Drittel, auch bei ungleichen Gewichten und Rundungen?“). 3 000 zufällige
  Gewichtsvektoren, je 40 Reihenfolgen mit den beiden Quoren kleinsten
  Schnitts: Dreimal der Schnitt liegt immer über der wahren Summe; wer die
  Drittelschranke erreicht, hält mehr als ein Drittel. Dazu 500 Fälle mit
  Gewichten nahe `u64::MAX`: Niemand erreicht ein Quorum. Gegenprobe mit
  dem sättigenden Verhalten: rot (Riesenfall 0, Schnitt 0).
- `myl-consensus` 0.35.0.

### v0.52.1 (2026-10-26): Eine Sitzung hält ihren Kontrakt über jede Folge (`myl-ledger` 0.26.1)

- ⚑ **Invariante 6 in `tests/invarianten.rs`**:
  `eine_sitzung_haelt_ihren_kontrakt_ueber_jede_folge`. 60 Keime zu je 300
  zufälligen Schritten unter einem Sitzungskontrakt: Beträge bis zum
  Doppelten des Einzellimits, wiederholte und übersprungene Nummern, ein
  Fremder als Handelnder, Widerrufe vom Fremden und vom Inhaber. Nach jedem
  Schritt: Verbrauch nie über dem Budget; eine angenommene Abbuchung nie
  über dem Einzellimit, nie nach dem Widerruf, nie mit verbrauchter Nummer,
  mit genau ihrem Abzug; eine abgelehnte lässt den Zustand bitgleich. Der
  Lauf zählt mit, dass gebucht wird und das Budget an seinen Rand kommt.
  Gegenprobe ohne die Budgetprüfung in `pruefe`: rot (Keim 4, Schritt 53,
  Verbrauch 527 bei Budget 500). Bisher standen diese Grenzen nur an
  getippten Beispielen; die Tür verlässt sich auf sie (Angriffsklassen A17
  und A19 in `SIMULATION/Sicherheitsaudit.md`).
- `myl-ledger` 0.26.1.

### v0.52.0 (2026-10-24): Eine knappe halbe Periode zum Nachladen (`myl-ledger` 0.26.0, Fund 628)

⛔️ **Konsensänderung der Regel, nicht des Formats:** Wann eine Kopie
verstößt, hängt an der Frist. Knoten mit 0.25.0 und 0.26.0 liefen beim
ersten Verstoß auseinander; die Probenetze werden gemeinsam gehoben, ein
Produktivnetz bräuchte eine Aktivierungshöhe.

- ⚑ **`FRIST_VOLLSTAENDIG_EPOCHEN` von 8 auf 31**: so lange hat eine neue
  Kopie für die erste Belegung, bevor verfehlte Epochen zählen. 31 ist das
  Größte, bei dem auch die Nachrückerin einer Kopie, die nie belegt, noch in
  derselben Periode von 64 Epochen fertig wird (31 + 2 + 31). Bei 2 s je
  Block sind das 31 Stunden statt acht.
  📌 Im vollen Archivlauf (NODE 0.59.0) wurde eine ehrliche Kopie ersetzt,
  die lud: 115 200 Blöcke in 509 s, die Frist samt zwei verfehlten Epochen
  waren im Testtakt rund 400 s. Was nachzuladen ist, wächst mit der
  Geschichte, die Frist wuchs nicht mit.
  ⚑ **Erwogen und verworfen: die Kopien eine Periode im Voraus ziehen.** Für
  die Nachrückerin gälte dieselbe Bedingung, das Fenster rückte nur eine
  Periode früher; gewonnen wären belegte Kopien schon zum Dienstbeginn, und
  diese Zwischenzeit decken die Halter. Bezahlt würde mit einem zweiten Satz
  Kopien im Zustand, einer neuen Rolle und Kopien, die eine Periode länger
  bekannt und angreifbar sind.
  ⚠️ **Die Grenze ist verschoben, nicht aufgehoben:** Eine volle Kopie holt
  die ganze Geschichte. Die Frist trägt, solange ein Knoten sie in 31
  Epochen laden kann; auf lange Sicht muss die Haltbarkeit der alten
  Perioden bei den Splittern liegen.
- **J1 (SHARED_TYPES 0.49.0):** Die Ledger-Proben bauen ihre Vorhaben mit
  Empfängerbeweis; die Ausgabe selbst prüft über `myl_types::sitzung::pruefe`
  und brauchte keine Änderung.
- **`FRIST_TEILE_EPOCHEN` = 8**, eigens für die Splitter: Sie holen nur ihre
  Teile der Gruppen einer Periode, das wächst nicht mit der Geschichte. Bis
  hierher teilten sie die Frist der Kopien.
- **Belege:** neu `eine_kopie_hat_zum_nachladen_eine_knappe_halbe_periode`
  (Vollständigkeit erst nach dreissig Epochen: kein Verstoß, kein
  Nachrücker; rot mit Frist 8). Der Splittertest lässt die Teilwurzel jetzt
  in Epoche 64 gelten statt in Epoche 0, wo jede Frist schon verstrichen
  war; erst damit wird er rot, wenn Splitter die Frist der Kopien bekommen.
  📌 Die erste Fassung des neuen Tests leitete seinen späten Beleg aus der
  Frist ab und wanderte mit ihr; die Gegenprobe blieb grün, jetzt steht die
  Zahl fest.

### v0.51.0 (2026-10-24): Die Archivpflicht im Konsens (`myl-consensus` 0.34.0, `myl-ledger` 0.25.0)

⛔️ **Konsensänderung**, zweimal: Der Zustand trägt die Historienwurzel und
den Stand der Archivpflicht; die Probenetze haben dafür neue Startwerte
(`-H1`, NODE 0.58.0). Für ein Produktivnetz bräuchte es eine
Aktivierungshöhe.

- ⚑ **`LedgerState.historie`** (Stufe A2): der Merkle Mountain Range über
  alle Blockhashes (SHARED_TYPES 0.48.0), fortgeschrieben beim Anwenden jedes
  Blocks mit dem Hash seines Vorgängers.
- ⚑ **`myl-ledger::archiv`** (Stufen A5 bis A7), `LedgerState.archiv`:
  - **Ziehung an der Langzeitgrenze** (`grenze_ziehen`): Kopien und Splitter
    aus `myl-types::archivwahl`, dazu die Historienwurzel der Grenze, gegen
    die belegt wird, und der Abschnitt der Splitter.
  - **Beleg** (`archiv_belegen`, `Anweisung::ArchivBelegen`): die Blöcke der
    Stichproben **als Bytes** (der Blockhash ist ihr Hash, wer nur Hashes
    aufhebt, kann nicht belegen) und je ein Beweis. Die erste Belegung einer
    Kopie prüft die Vollständigkeit mit 32 Stichproben über alle Höhen,
    danach 4 je Epoche.
  - **Abschluss je Epoche** (`epoche_abschliessen`): Gutschrift **umgebucht**
    aus dem Treasury-Eingang der Epoche (5 % für Kopien, 2 % für Splitter),
    die Geldmenge folgt weiter allein der Prägeformel. Zwei Epochen in Folge
    ohne Beleg nach der Frist (acht Epochen zum Nachladen) sind ein Verstoß:
    ein Zehntel des haftenden Einsatzes an die Treasury, Vermerk in der
    Verstoßhistorie, das nächste Los rückt nach, eine fehlende Zone zuerst;
    gibt es keinen Freien, ein Splitter, der dann beides hält. ⚑ **Der
    Verstoß wird festgestellt, nicht gemeldet:** Eine Meldung, die jemand
    einreichen muss, bliebe aus, wenn niemand es tut.
  - **Übergabe nach Beweis** (Stufe A6): Die Kopien einer Periode werden an
    der nächsten Grenze Halter, sobald mindestens fünf ihre Vollständigkeit
    belegt haben; sonst halten die alten weiter, ohne Frist. Vor der ersten
    Ziehung und nach einer Periode ohne Ziehung hält jeder.
  - **Splitter** (Stufe A7): Kopien nennen die Teilwurzel
    (`ArchivTeilwurzel`), sie gilt bei fünf gleichen Stimmen; Splitter
    belegen je Epoche einen gezogenen Teil (`ArchivTeilBelegen`). Ein
    Splitter wird nicht ersetzt, sein Index ist sein Teil; die Codierung
    verträgt 32 fehlende. Die Teilwurzeln aller Abschnitte bleiben im
    Zustand, für die Wiederherstellung aus Teilen.
  - ⚠️ **Was Stichproben nicht ausschliessen:** Wer die gezogenen Blöcke erst
    nach dem Ziehen bei anderen holt, besteht ebenfalls. Dagegen stehen die
    übrigen Kopien, die Halter und die Splitter.
- **Neue Fehler:** `NichtArchivkopie`, `ArchivBereitsBelegt`,
  `ArchivbelegUngueltig`.
- **Proben:** fünf im Ledger (Belegen und Übergabe, ohne Übergabe halten
  alle, falsche Belege, Verfehlen und Belohnen, Splitter samt Teilwurzel);
  Gegenproben je Prüfung rot. 📌 **Eine davon war zuerst grün:** Der Fall
  „fremder Teil“ trug auch den falschen Index und fiel schon daran; die
  Pfadprüfung des Teils hatte keinen eigenen Fall. Jetzt einer mit richtigem
  Beweis und anderem Inhalt.

### v0.50.1 (2026-10-22): Eine Werkzeugkette für alle Kisten (`myl-consensus` 0.33.1, `myl-scheduler` 0.18.1, `myl-ledger` 0.24.1)

- `rust-version` auf 1.99 in `myl-consensus`, `myl-scheduler`, `myl-ledger`; die Werkzeugkette steht jetzt einmal,
  in `rust-toolchain.toml` (1.99.0), und die CI baut mit genau ihr statt mit
  einem wechselnden `stable` und zwei Mindestfassungen (1.85 und 1.88).
  Nur das Manifest, kein Code.
- **Belege:** `cargo metadata --locked --offline` gegen den Vorrat in jeder
  Kiste; die Wache der CI hält Fassung und Werkzeugkette zusammen.

### v0.50.0 (2026-10-22): Kein Pod ohne Zusage, Nachziehen, Teilablage, Frist

Stufe 4 des Dispatchers, abgeschlossen. ⚑ **Festlegungen des
Projektinhabers vom selben Tag:** kein Pod ohne Zusage; eine Fristmeldung
wirkt erst mit den eigenen Messungen einer Mehrheit der übrigen Mitglieder.

- `myl-scheduler` 0.18.0:
  - `zuteilung_der_epoche_mit_zusagen` (neu): Miner ohne Zusage für das
    Podmodell stehen in `Zuteilung::ohne_zusage`; jeder Pod wird geprüft,
    und ist er nicht machbar, weicht das schwächste Mitglied
    (`schwaechstes`) und er zieht aus derselben Folge nach
    (`assign_pods_geprueft`, ohne Einwand bitgleich wie bisher).
  - `bereiche_des_pods` nimmt das Podmodell und kennt keinen gleichen
    Rückfall mehr: Eine Position ohne Zusage macht den Pod nicht machbar.
  - Teilablage (P10): `Mitglied::gehalten`; mit gehaltenen Bereichen
    verteilt `verteilen_mit_teilablage` (Intervalle von vorn,
    Binärsuche über die Zeit), ohne sie das Wasserfüllen wie bisher.
  - `reserve_traegt` prüft jeden Bereich samt gehaltenem Teil.
- `myl-ledger` 0.24.0: `LedgerState::{podmodell, fristsperren}`;
  `zusage_einreichen` prüft gegen das Podmodell (ohne Podmodell gilt nur
  reiner Speicher); `geltende_zusage` beachtet die Fristsperre;
  `modellzusage_der_epoche`; `fristverstoss_einreichen` mit `Fristkontext`:
  Mehrheit der übrigen über der Frist, dann gilt die Zusage in der nächsten
  Epoche nicht, und das Konto bekommt einen Verstoß.
- `myl-consensus` 0.33.0: `Anweisung::FristverstossEinreichen`, angehängt.
- ⚠️ **Ein bekannter Hebel, benannt:** Eine Zusage wirkt auf die
  Pod-Bildung einer Epoche, deren Saat schon feststeht; wer sie ändert,
  ändert, wer in die Mischung kommt. Das ist dasselbe Ein-Bit-Gewicht je
  Miner, das die Abmeldung heute schon hat.
- **Belege:** Teilablage gezielt und über 2000 Zufallspods, das schwächste
  Mitglied, die Pod-Bildung über 20 Saaten mit Gegenprobe, Zusage gegen
  das Podmodell und Fristmeldung im Ledger; `cargo test` rc=0 in allen
  drei Kisten.

### v0.49.0 (2026-10-22): Die Zusage auf der Kette und der Schnitt eines Pods

Stufe 4 des Dispatchers, Pakete P4 und P5. Die Zusage mit ihren Modellen
(`myl-types` 0.45.0) war bisher eine Datei neben dem Knoten; die Kette
kannte sie nicht und konnte deshalb nicht danach schneiden.

- **`myl-consensus` 0.32.0:** neue Anweisung `ZusageEinreichen { zusage }`,
  hinten angehängt, damit die Kennungen der übrigen bleiben.
- **`myl-ledger` 0.23.0:** `State::zusagen` je Miner (Genesis leer).
  `zusage_einreichen` prüft, dass der Halter der Unterzeichner ist und
  registriert, und prüft die Unterschrift mit dem **registrierten**
  Schlüssel. Eine Zusage für dieselbe Epoche ersetzt die vorige; behalten
  werden die kommenden und die jüngste geltende. `geltende_zusage(state,
  miner, epoche)` liest sie. Neuer Fehler `ZusageUngueltig`, hinten
  angehängt.
- **`myl-scheduler` 0.17.0:**
  - `verteilen_geordnet` hält die Reihenfolge der Positionen und gibt den
    Kopf der letzten. In der Kette hängen Besetzung, Übernahme und
    Bündelprüfung an der Position; die Verteilung darf sie nicht umstellen.
  - `bereiche_des_pods` ergibt einen `Podschnitt`: `Gleich` (der Schnitt
    von heute), `Verteilt` oder `NichtMachbar`, mit `grenzen()` je
    Position. Gleich bleibt er, solange kein Netzmodell feststeht, ein
    Mitglied keine Zusage für das Modell hat oder ein Mitglied nur einen
    Teil hält (Weg a ist noch nicht verteilt). Nicht machbar ist ein Pod,
    wenn keine Reserve den größten Bereich trägt (Festlegung E4).
  - ⚑ **`lohnt_umplanen` ist entfernt, P6 entfällt.** Die Pod-Bildung
    mischt die Miner jede Epoche neu, ein Pod besteht also nie über eine
    Epochengrenze; eine Schwelle gegen das Wandern der Bereiche hätte
    nichts, woran sie sich halten könnte. Der Modulkopf sagt das.
- **Belege:** neue Proben `eine_zusage_gilt_ab_ihrer_epoche_unter_dem_registrierten_schluessel`
  (Ledger) und `der_schnitt_eines_pods_der_kette` (Scheduler); `cargo test`
  rc=0 in allen drei Kisten (272, 107 und 127 Tests).

### v0.48.0 (2026-10-21): `myl-scheduler` 0.16.0, die Ebenenverteilung und ihre Simulation

Stufe 4 des Dispatchers, vorgezogen und geplant auf Bitte des Projektinhabers (2026-10-21): Ein Pod verteilt seine Ebenen nach Kapazität und gemessener Ebenenzeit seiner Mitglieder statt gleich. Hier P3 (Funktion) und P9a (Simulation). Noch nicht angeschlossen:
Zusage, Pod-Bildung und Umplanen (P4 bis P6) folgen mit dem Drahtformat.

- **`ebenenverteilung::verteilen`** (Konsensbaustein, rein und ganzzahlig):
  machbar nur bei genug Kapazität; der Kopf geht an das schnellste
  Mitglied, das ihn trägt; Wasserfüllen über eine Zeitschranke T per
  Binärsuche, der Rest an die Schnellsten, Gleichstand nach Kennung; die
  Saat mischt die übrigen Bereiche, der Kopf zuletzt. Dazu `engpass_us`,
  `lohnt_umplanen` (`4·neu ≤ 3·alt`) und `reserve_traegt` (Festlegung E4).
- ⚑ **Platz und Zeit des Kopfs getrennt** (`kopf_platz`, `kopf_zeit`):
  beim Bau gefunden, gemessen am Raven Platz 4, Zeit 1.
- **Simulation** (`examples/ebenenverteilung_simulation.rs`, 5000 Pods je
  Lage, k = 4, 40 Ebenen, Knoten 8 bis 64 GiB): Der gleiche Schnitt ist bei
  rund der Hälfte der Pods nicht machbar, die Verteilung bei 99 bis
  100 %. Die Zeit verkürzt die Engpasszeit gegen die Verteilung nur nach
  Kapazität um 3,8 % (fast gleiche Knoten), 30,0 % (zwei Klassen) und
  52,1 % (weit gestreut). Umplanen bei ±5 % Rauschen: 0 Wechsel in 25 000
  Epochen.
- **Belege:** neun Proben, darunter Eigenschaften über 2000 Zufallspods
  (lückenlos, in der Kapazität, Summe gleich, schneller nie weniger;
  mindestens 400 machbar) und eine Gegenprobe mit eingebautem Fehler, die
  drei Proben rot macht. `cargo test` (127) und Clippy rc=0;
  `fassungsprobe` grün.

### v0.47.1 – 2026-10-21 (`myl-scheduler` 0.15.1: der Modulkopf nennt die Zonen)

Der Kopf von `myl-scheduler/src/lib.rs` nannte als dritten Schritt noch
„Geo-Clustering unter Latenz-Constraint (konsumiert LatencyGraph aus
NETWORKING)“. Seit dem 2026-09-01 bilden Zonen aus der Registrierung die
Cluster (`zonenzuteilung`), und `myl-types` 0.44.0 hat `LatencyGraph`
entfernt. Nur der Kommentar; kein Code geändert.

**Belege:** `cargo test` rc=0, Clippy mit `-D warnings` still;
`myl-consensus` und `myl-ledger` mit `cargo check --all-targets` rc=0.

### v0.47.0 – 2026-09-06 (der Schuldspruch bringt seinen Beleg mit)

`Anweisung::SchuldspruchEinreichen` und
`transitions::schuldspruch_einreichen`.

⚑ **`apply_verdict` hat damit seinen ersten Aufrufer**, und zwar einen
mit Nachweis. Der Beleg ist die **eigene Unterschrift des
Beschuldigten** über den strittigen Übergang, in der Rolle Shard; wer
sie nicht hat, kann niemanden benennen.

**Zwei Prüfungen, und die zweite ist die unauffälligere:** Der Beleg
muss tragen, **und** die aus seinem Schlüssel abgeleitete Kennung muss
das beschuldigte Konto sein. Ohne die zweite wäre ein echter Beleg eine
Waffe gegen Dritte.

### v0.46.0 – 2026-09-06 (das Trainingspaar sagt, was es an Unabhängigkeit hergibt)

`Trainingsplan` trägt jetzt `zonendivers`, und `paare_bilden` sucht
zuerst einen Partner in einer **anderen** Zone.

⚑ **Der Fund kam aus der verzahnten Simulation, nicht aus dem Code.**
Die Inferenz hat für ihre Redundanzpaare seit jeher eine
Zonenpräferenz und meldet in `Redundanzzuteilung::zonendivers`, wenn es
keine gab. **Das Training hatte beides nicht:** Gepaart wurde nach
Podnummer, und niemand erfuhr, ob die beiden Pods überhaupt getrennt
stehen. Ein falsches Δm verdirbt nicht eine Antwort, sondern das
Modell; die Unabhängigkeit ist dort also nicht weniger wert.

⚑ **Erzwungen wird sie trotzdem nicht, und das ist eine Entscheidung.**
Seit die Zonenreste in einen Sammeltopf gehen, sind Pods in einem dünn
besetzten Netz **zonengemischt**, und ein gemischter Pod ist
unbestimmt. Zwölf Miner auf drei Zonen ergeben zwei Pods, beide
gemischt: Es **gibt** kein diverses Paar. Wer hier auf Diversität
besteht, trainiert nie. Das Feld sagt deshalb, welcher Fall eintrat,
statt den einen zu erzwingen oder den anderen zu verschweigen.

**Zwei neue Tests in `shard_assignment`** halten die Kehrseite des
Sammeltopfs fest: Bei fünf Pods tragen die drei zonenreinen die
Diversität allein; kommen alle Pods aus dem Topf, fällt sie, und
`assign_redundant_pods` meldet es.

### v0.45.0 – 2026-09-06 (der Trainingsplan kennt die Tokenzahl)

Der Probekorpus im Scheduler-Test trägt `tokens_je_segment`, wie der
Anker es seit heute verlangt.

**Ältere Einträge** (v0.44.0 und davor, 36 Stück) stehen in der Git-Geschichte dieser Datei: `git log -p -- CONSENSUS/README/README.md`.
