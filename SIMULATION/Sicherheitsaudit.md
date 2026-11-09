# Sicherheitsaudit

**Stand:** 2026-08-28, Tür und Wallet (A16 bis A24) und Quorumsschnitt (A25) am 2026-10-26, Stichprobe im Netz (A27) und Zeitprobe (A28 bis A31) am 2026-10-27, Saat und Unterschrift der Zeitprobe (A32) am 2026-10-28, der rotierende Erzeuger und die Prüfpunkte (A33 bis A40) am 2026-11-05, die Wahl des Tokens (A41) am selben Tag, die Kollusion beider Pods im Netz (A12) am 2026-11-06, die Lücke der Wechselbelege nach einer Ruhe der Finalität (A42) am 2026-11-07, der Durchgang vor dem Testnetz (A10, A12, A23 neu geprüft, A43 bis A45 neu) und seine Behebungen (A23, A43, A46) sowie der grosse Durchgang über die Eingänge des Knotens (A47 bis A50) am 2026-11-08
**Grundlage:** die Angreiferklassen aus Whitepaper Kap. 5.6 und 9.2, die
Sicherheitsargumente aus Kap. 6.8 und Anhang B, sowie die Funde 41 bis 53
dieses Projekts.

**Was dieses Dokument ist:** eine Bestandsaufnahme, welche Angriffe heute
abgewehrt werden, welche gemessen offen sind und welche niemand geprüft
hat. Es ist **kein externes Review** (K5, K9) und ersetzt keines.

**Der wichtigste Satz vorweg:** Von den fünfzig Angriffsklassen
unten (Stand 2026-11-08) sind **46 abgewehrt und belegt**, **drei offen mit
gemessener oder benannter Lücke** (A9, A12, A44) und **eine
ungeprüft** (A13, die Krypto-Primitiven ohne externes Review). Keine der
Lücken ist neu entdeckt worden, ohne dass sie hier steht.

⚑ **Nachtrag vom 2026-11-05: der rotierende Erzeuger, die Prüfpunkte und
die Wege der Prüfung (A33 bis A40).** Seit die Kette einen Erzeuger je Slot
zieht und Prüfpunkte mit Quelle und Ziel endgültig macht, gibt es Gabeln, und
mit ihnen neue Klassen. Gefunden in Netzläufen mit Prüfrollen
(Zurückhalter, Doppelerzeuger, falsche Uhr, Verstummen) und beim Lesen,
alle am selben oder am nächsten Tag geschlossen: eine Gabel, die nie
erkannt wurde (A33), ein Doppelsigner, der zurückkam (A34), eine erfundene
Höhe (A35), Spurablagen, die jede Gegenstelle verdrängen konnte (A36),
und lange Sitzungen, deren Beleg nicht in eine Antwort passte (A37). Dazu
belegt ohne Schaden für die Ehrlichen: zurückgehaltene Blöcke (A38), eine
falsche Uhr (A39) und der Ausfall von mehr als einem Drittel (A40).
Nachgezogen: A4 gilt jetzt für Prüfpunkte, A27, A29 bis A31 sind
geschlossen.

⚑ **Nachtrag vom 2026-10-26: neun Klassen für Tür, Wallet und die lokalen
Leitungen (A16 bis A24).** Diese Teile sind seit Oktober gebaut und standen
hier noch nicht. Acht sind abgewehrt und belegt (A24 seit dem Abend), eine offen (A23).
Beim Durchgehen fiel eine Lücke auf und ist geschlossen (A17, Fund 651).
Dazu A25: Der erste Eigenschaftstest über den Quorumsschnitt fand eine
latente Lücke an den Rändern von `u64` (Fund 652), am selben Tag behoben.

⚑ **Neu und am selben Tag geschlossen: A15 (2026-08-28).** Die erste
Klasse dieses Dokuments, die nicht aus einem Angriffsmodell kam, sondern
aus einer Codelektüre: Der Merkle-Baum war nicht injektiv, zwei
verschiedene Blattfolgen konnten dieselbe Wurzel tragen. Sie stand einen
halben Tag als latent und ist behoben.

⚑ **Der Grund für die Eile steht in der Klasse selbst und gilt über sie
hinaus:** Es war der einzige Punkt dieses Dokuments, dessen
Behebungskosten mit jedem Betriebstag gestiegen wären, ohne dass jemand
etwas falsch macht. Eine Änderung an einer Commitment-Konstruktion
kostet vor dem Genesis-Block fünf Prüfvektoren und danach eine
Kettenmigration.

---

## 1. Zusammenfassung nach Schwere

| # | Angriff | Stand | Beleg / Lücke |
|---|---|---|---|
| A1 | Falsches Rechenergebnis | ✅ | Stufe 1 (Redundanz), Stufe 2 (Stichprobe aus echter Arbeit, Nachrechnen ab den Tokens, Schiedsrunde) |
| A2 | Rogue-Key auf Aggregatsignaturen | ✅ | Proof-of-Possession, Fund 27, `rogue_key.rs` |
| A3 | Koordinator fälscht PoI-Bündel | ✅ | 7 Angriffe in `koordinator_byzantinisch.rs`, Fund 52 geschlossen |
| A4 | Double-Signing | ✅ | Prüfpunkte mit Quelle und Ziel: doppelte und umschliessende Stimmen schlachtbar, jede Stimme nennt ihre Quelle; Erzeuger im selben Slot zweimal schlachtbar; Beweis mit BLS |
| A5 | Manipulierte Aktivierungen im Pod | ✅ | Eingangs-Hash-Prüfung, Fund 41 geschlossen |
| A6 | Self-Dealing in der Subventionsphase | ✅ | `s < c/(1−c)` gegen das untere Bandende, Fund 49 |
| A7 | Verbrauchs-Stoß mit Ausstieg | ✅ | Burn-Cap je Adresse, gemessen |
| A8 | Parametervorschlag, der Invarianten bricht | ✅ | Registry prüft **vor** der Abstimmung |
| A9 | **Eclipse: Umzingelung eines Knotens** | ⚠️ | **Fund 53 geschlossen** (2026-08-24); Restbedingung: ehrlicher Bootstrap-Knoten |
| A10 | **Latenzwerte fälschen** | ✅ | Signatur wird **geprüft** (2026-08-25). ⚑ Neu eingeordnet (2026-11-08): Seit dem 2026-09-01 bilden Zonen aus der Registrierung die Pods, nicht gemessene Latenzen; ein Attest geht nur noch als Probe durchs Gossip und wirkt auf nichts im Konsens. Die ableitbaren Schlüssel des Probelaufs betreffen nur diesen Probeverkehr. Was bleibt, ist die selbst erklärte Zone, und die steht unter A12 |
| A11 | **Kontrollsegmente erkennen** | ✅ | **Gegenstandslos** (2026-11-08 festgehalten): Die Kontrollsegmente sind seit dem 2026-09-02 entfallen, die Stichprobe zieht echte Arbeit, und die ist von echter Arbeit nicht zu unterscheiden. Die Messung unten bleibt als Geschichte der Klasse stehen |
| A12 | Kollusion beider Pods | ⚠️ | Schranke gemessen (β^2k trifft); Gegenmaßnahme im Netz belegt (2026-11-06: Paar einig, nur der Checker fand es). ⚑ Clusterbild gemessen (2026-11-08, Test `der_zonenhebel_ueber_die_zwillinge`): Mit gut einem Viertel der Kennungen, die Hälfte in einer leeren Zone erklärt, gehört ein ganzes Zwillingspaar in 3,7 % der Epochen dem Angreifer, gleichmässig verteilt nie; beide Zwillinge an der ersten Position (genug für eine Kollusion dort) in 44 % ohne und 58 % mit dem Hebel. Gegen Kollusion an einer Position schützt Stufe 1 also nie, die Abwehr ist die Stichprobe mit Nachrechnen. Paaren nach der Saat wäre gemessen schlechter (bis 50 %) |
| A13 | Angriff auf die Krypto-Primitiven | ❓ | nie extern geprüft (K5). ⚠️ Seit 2026-11-03 dazu selbst geschriebene Schwellenkryptographie für die Saat (Feldarithmetik mit konstanter Zeit, Polynome mit Feldman-Bindungen und Kenntnisnachweis, ElGamal über ristretto255, rund 1 350 Zeilen), gegen `blst` über den Homomorphismus und mit Angriffstests geprüft; das macht das externe Review vordringlich |
| A14 | **Ein Gateway liest den Inhalt mit** | ✅ | Sitzungs-E2E seit `myl-net` v0.9.0, über echte Verbindungen gemessen; **Verkehrsdaten bleiben sichtbar**, und das Schema selbst fällt unter A13 |
| A15 | **Zwei Blattfolgen, eine Merkle-Wurzel** | ✅ | **Fund 77** (2026-08-28), gemessen und am selben Tag behoben: die Wurzel bindet die Blattzahl (`myl-types` v0.6.0) |
| A16 | Tür ohne gültige Vollmacht | ✅ | Unterschrift, Vorbehalte und Kontrakt vor jedem Zerlegen; jede Ablehnung sieht gleich aus |
| A17 | **Tür rechnet über die Grenzen des Kontrakts** | ✅ | **Fund 651** (2026-10-26) geschlossen: Einzellimit und Restbudget vor dem Rechnen |
| A18 | Veralteter Stand an der Tür | ✅ | **Funde 647, 648** (2026-10-26) geschlossen: Epoche nach dem Annehmen, Abschrift je Block |
| A19 | Doppelte oder fremde Abrechnung | ✅ | Riegel der Abrechnungsnummer in der Kette, Vollmacht autorisiert |
| A20 | Lokale Leitungen ohne Ausweis | ✅ | Relais- und Ortsleitung nur mit Ausweis (`0600`), Rahmen gedeckelt |
| A21 | Transaktion aus dem Wallet verfälscht | ✅ | Unterschrift bindet Netz, Nonce und Anweisung; der Knoten verbreitet nur Geprüftes |
| A22 | Auszahlungskonto umlenken | ✅ | **Fund 643**: ein Wechsel braucht den Schlüssel des alten Kontos |
| A23 | **Die Tür belegen** | ✅ | **Geschlossen** (NODE 0.117.0, GATEWAY 0.11.0): bis zu 32 Anfragen zugleich, der Deckel steht vor dem Lesen (503), je Sitzung rechnet eine (429, zugleich keine zwei Anfragen auf demselben Budget), Annahme und Zugang nur für die Buchung gesperrt. Tests mit Gegenproben. Nach einem Fehler beim Annehmen eine Pause statt einer leeren Schleife |
| A24 | **Auftrag über die Epochengrenze** | ✅ | **Fund 649** (2026-10-26) geschlossen: das Podwerk rechnet einmal über den Pod der neuen Epoche |
| A26 | **Ein ehrlicher Miner wird gesperrt** | ✅ | **Fund 655** geschlossen: Anlaufzeit und zwei Fenster; im langen Lauf über 100 Epochen 210 Verdachtsfälle, kein ehrlicher Miner gemeldet |
| A27 | **Die Stichprobe erreicht niemanden** | ✅ | **Geschlossen** (2026-10-28 und 2026-10-29): Die Mitglieder halten die Spuren, der Prüfer rechnet ab den Tokens nach, zwei Checker legen fest, bevor sie aufdecken, eine Schiedsrunde mit Komitee entscheidet, im Netz belegt. Stand vorher: | **Fund 656** zur Hälfte: Seit NODE 0.66.0 gehen Spuranfragen als Nachforderung hinaus, seit NODE 0.67.0 finden sie ihren Pod auch nach dem Epochenwechsel (**Fund 658**: vorher endete jede Ziehung ohne Adresse). Im kurzen Podlauf kamen 18 von 18 an (`spuranfrage_eingegangen`). **Beantworten kann sie noch niemand**, denn die Segmentzeugnisse hält nur der Koordinator, und die Eingabe eines Segments hebt seit E10 niemand auf (Fund 118). Stufe 2 von A1 findet im Netz also weiter nicht statt |
| A28 | **Ein Lügner lässt jede Zeitprobe verwerfen** | ✅ | **Fund 646** geschlossen (NODE 0.68.0, Festlegung des Projektinhabers 2026-10-27): Es wird kein Ping mehr abgezogen; bestanden ist die kürzeste von fünf Proben ≤ k · Frist + Zugabe (2 s), und k richtet sich nach der Zusage (64 bis 2 048 Positionen). Eine Probe über ihrer Grenze gilt als Überschreitung, auch ohne Antwort. Probe mit Gegenproben in `frist.rs`; im Netzlauf gemeldet |
| A29 | **Ein Mitglied verweigert jede Zeitprobe** | ✅ | **Fund 660 geschlossen** (2026-10-28): Eine lange Folge von Abweisungen zählt wie eine Überschreitung. Stand vorher: Wer jede Probe sofort mit `Nichts` beantwortet (angeblich besetzt), wird nie gemessen. Abhilfe braucht eine Festlegung (verweigerte Proben zählen wie eine Überschreitung, ab einer Zahl je Epoche) |
| A30 | **Ein Mitglied antwortet mit erfundenem Abdruck** | ✅ | **Fund 661 geschlossen** (2026-10-28, mit A32): frische Saat je Probe, Antwort unterschrieben. Stand vorher: Der Abdruck einer Zeitprobe wird nicht nachgerechnet; wer sofort mit einem beliebigen Abdruck antwortet, besteht jede Probe. Abhilfe braucht eine Festlegung (etwa Stichproben der Abdrücke durch Checker, die das Modell haben) |
| A31 | **Ein Lügner, der nur kurz auf Position steht** | ✅ | **Fund 664 erklärt** (2026-10-31): Der Lügner der Probe sagte über seine Selbstprobe ehrlich zu; ohne sie im Nachtlauf bei jeder Position gesperrt. Stand vorher: Gemeldet wird erst mit Zeugnissen einer Mehrheit des Pods, und ein Zeugnis verlangt zwei Überschreitungen im Abstand eines Achtels der Epoche, ab einem Viertel. Mit 108 s je Epoche (Podlauf mit `--real`, kurze Wege) schafften das zwei von vier Messenden; der Lügner stand in vier Epochen auf Position und wurde nie gemeldet. Bei einer Stunde je Epoche trüge die Regel; die Erkennung hängt aber an der Wanduhrlänge der Epoche, und die steht nirgends als Bedingung. Zur Entscheidung |
| A32 | **Ein Lügner, der den Abdruck der Zeitprobe vorrechnet** | ✅ | **Fund 669** (2026-10-28) geschlossen: Die Anfrage trug nur Positionen und Grenze, der Abdruck war für dieselbe Anfrage immer derselbe; einmal gerechnet, hätte ein Lügner ihn danach sofort geschickt. Jetzt trägt jede Probe eine frische Saat des Messenden (32 Byte), aus der der Shard die Eingabe bildet, und die Antwort ist mit dem Minerschlüssel über Saat, Positionen und Abdruck unterschrieben; gezählt wird nur eine tragende Unterschrift. Proben in `myl-types`, `myl-pod` (`grenzen_der_kette`) und `myl-node` (`frist`) mit Gegenproben |
| A33 | **Eine Gabel wird nie erkannt** | ✅ | Gefunden im Nachtlauf mit Laststufen (2026-11-04): Gruppen von Minern bauten auf eigenen Zweigen weiter, bis zu 2 500 Blöcke hinter der Mehrheit, weil ein fremder Block an der nächsten Höhe als unbelegter Erzeuger galt statt als Gabel. Geschlossen in NODE 0.107.1; Gabeln über 700 Blöcke (tiefer als die Rückwege, auch nach einem Neustart) über Abzug und Nachrechnen seit 0.108.0, im Netz belegt. Sicherheit blieb: gerechtfertigt nur auf dem Zweig der Mehrheit. 📌 Nachtlauf vom 2026-11-06: Trug der Zweig der Mehrheit ein Zertifikat für einen Prüfpunkt über der Gabel, wies die Vorprüfung ihn ab (sie verglich mit dem eigenen Block an dieser Höhe), und vier Knoten blieben nach 900 s Trennung auf ihrem Zweig; geschlossen in NODE 0.113.2 (zertifizierter Block aus dem Zweig, Unterschrift gegen einen Satz, der vor der Gabel feststand; den Rest prüft das Nachrechnen), im Netz belegt mit 900 s Trennung: alle 13 der unterlegenen Seite wechselten um 829 bis 848 Blöcke. ⛔️ Wieder gefunden im Podlauf (2026-11-06): Ein Validator mit einer Gabel an Block 1 baute den ganzen Lauf allein, weil die Blöcke der Mehrheit im Vorausspeicher lagen und dort still wegfielen; geschlossen in NODE 0.114.0 (ein Kandidat, der an der nächsten Höhe nicht anschliesst, startet die Zweigsuche, auch nach dem eigenen Block; Genesis als Vorgänger von Block 1). Belegt im Test, im Netz nur das Ausbleiben (die Lage ist nicht nachzustellen) |
| A34 | **Ein Doppelsigner kommt zurück** | ✅ | Gefunden mit einem Doppelerzeuger im Netzlauf (2026-11-05): Er baute nach dem Beleg drei Epochen weiter (der Satz steht zwei voraus fest), und eine Neuanmeldung hätte ihn ganz zurückgebracht. Geschlossen in NODE 0.108.1: Ein Ruhender wird nicht mehr gezogen, ein Doppelsigner ist unter seinem Schlüssel für immer getilgt (wie Ethereum und Cosmos) |
| A35 | **Eine erfundene Höhe** (Fund 634) | ✅ | Mit Slots entscheidbar: Jeder Block verbraucht einen Slot, jeder Zweig zweigt über dem endgültigen Prüfpunkt ab; eine Höhe über `h_e + (Slot jetzt − s_e)` wird nicht gehört, auch mit gültiger Unterschrift (NODE 0.108.1) |
| A36 | **Spurablagen verdrängen** | ✅ | Beim Lesen gefunden (2026-11-05): Jede Gegenstelle konnte stimmige, erfundene Ablagen schicken und die echte verdrängen; dann lieferte niemand den Verlauf. Geschlossen in NODE 0.110.0: nur von Mitgliedern des Pods (beglaubigte Gegenstelle), je Absender eine |
| A37 | **Eine lange Sitzung entgeht der Prüfung** | ✅ | Beim Lesen gefunden (2026-11-05): Verlauf und Schiedsbeleg mussten in eine Antwort passen (rund tausend Positionen); sonst bekam der Prüfer nichts. Geschlossen in NODE 0.110.0: in Stücken mit Abdruck, je Fall und Gegenstelle, gedeckelt |
| A38 | **Zurückgehaltene Blöcke** | ✅ | Belegt im Netzlauf (2026-11-05): ein Erzeuger hielt jeden Block 3 s zurück; er verlor seine eigenen, kein Ehrlicher einen endgültigen |
| A39 | **Eine falsche Uhr** | ✅ | Belegt im Netzlauf: Uhr +3 s, seine Blöcke wurden als künftig abgewiesen, bis ihr Slot begann; die Kette blieb einig. Die Uhr wirkt nur in der Annahmeregel, der Zustand rechnet nie mit ihr |
| A40 | **Mehr als ein Drittel fällt aus** | ✅ | Belegt im Netzlauf: 12 von 28 still, die Finalität ruhte und kehrte nach fünf Epochen von selbst zurück, ohne Abzug am Einsatz (die Stillen ruhen über ihre Punkte) |
| A41 | **Ein Pod wählt das Token nicht** | ✅ | Beim Lesen gefunden (2026-11-05): Die Spur band Aktivierungen und Tokens, nicht die Wahl; ein Pod, der an einer erzeugten Position nicht den Argmax nahm und danach ehrlich weiterrechnete, entging dem Nachrechnen, und nur ein ehrlicher Zwilling fand ihn. Geschlossen in NODE 0.113.0 (VERIFICATION 0.28.0): Die Kennung trägt die Prompt-Länge, gesetzt vom Koordinator der Tür; ab ihr prüfen Checker und Komitee die Wahl, belegt mit dem echten Modell. ⚠️ Tür und Pod zusammen können die Länge zu gross angeben |
| A42 | **Die Finalität ruht, und ein Knoten ohne Zustand kommt nie wieder hinein** | ✅ | Im Nachtlauf gefunden (2026-11-07): Ruht die Finalität zwei Epochen oder länger (ein Ausfall, oder mehr als ein Drittel des Einsatzes schweigt absichtlich), entsteht für sie kein Wechselbeleg, und der Satz danach ist nirgends gebunden. Ein zustandsloser Knoten und ein voller Einsteiger über ein Zertifikat nahmen danach keinen Anker mehr an, für immer. Geschlossen in NODE 0.115.0 durch eine Brücke: ein Zertifikat, das mehr als zwei Drittel des letzten bekannten Satzes tragen, alle Unterzeichner aus ihm, innerhalb der Vertrauensfrist; dieselbe Sicherheit wie ein Zertifikat dieses Satzes. Tests mit Gegenproben. ⛔️ Im Netz widerlegt für den Hauptfall (`kurz-verstummen`, 2026-11-07): Die Kette schliesst die Stillen während der Ruhe aus, der Satz schrumpft (28 auf 16 Mitglieder), und das erste Zertifikat danach tragen nur 57 % des Satzes vor der Lücke; die Brücke verlangt mehr als zwei Drittel. ⚑ Entschieden (2026-11-07): Eine niedrigere Schwelle wäre ein neuer Angriffsweg (eine Koalition mit mehr als einem Drittel unterschreibt auf einem eigenen Zweig, ohne doppelt zu stimmen); nach einer Ruhe mit Ausschluss ein Anker von außen (NODE 0.116.1, `--anker`; Tests mit Gegenproben; im Netz belegt in `kurz-anker`: Abzug am Anker 7 s nach dem Neustart, an der Spitze nach 18 s). ⚠️ Schwache Subjektivität: Der Anker muss aus einer vertrauenswürdigen Quelle kommen; ohne ihn bleibt ein solcher Knoten draußen |
| A43 | **Langstreckenangriff auf den Einstieg über Wechselbelege** | ✅ | Beim Durchgang gefunden (2026-11-08): Der Einstieg über ein Zertifikat folgte den Belegen ab Genesis ohne Altersgrenze; mit den Schlüsseln von mehr als zwei Dritteln eines alten Satzes, deren Einsatz längst frei ist, ließe sich eine zweite Folge bauen. **Geschlossen** (NODE 0.117.0): Genesis und ein Anker tragen nur, solange sie nach der Uhr jünger als die Vertrauensfrist sind (Startzeit in der Stimmsatzdatei, Slot im Anker); danach hält ein Knoten ohne eigenen Stand mit einer klaren Meldung an. Mit einem Anker gibt es keinen Weg mehr ab Genesis, auch nicht, solange sein Beleg fehlt oder alle schweigen. Volle Knoten nennen einen frischen Anker unter `/anker`. ⚠️ Schwache Subjektivität bleibt: Der Anker muss aus vertrauenswürdiger Quelle kommen, am besten aus mehreren unabhängigen; eine falsch gehende Uhr beim Einsteiger verschiebt die Grenze. Das Alter ist in Proben belegt, ein Lauf über 112 Epochen wäre zu lang |
| A44 | **Wechselbelege zu gross für eine Antwort** | ⚠️ | Beim Durchgang gefunden (2026-11-08): Ein Beleg trägt zwei ganze Sätze und zwei Zertifikate mit Unterzeichnerliste, rund 240 Byte je Stimmberechtigtem. **Das Wachstum ist geschlossen** (NODE 0.117.0): Belege fallen nach der Sperrfrist, mit A43 fragt niemand ältere nach; die eigene Folge beginnt beim ältesten eigenen, der Beleg eines Ankers bleibt. **Offen die Grösse des einzelnen Belegs:** ab rund 8 700 passt er nicht mehr in die halbe Antwort, ab rund 17 000 nicht mehr in eine ganze; dann kommt niemand mehr über Belege hinein. Für das Testnetz ohne Folge. Vor dem Hauptnetz zu entscheiden: Unterschied zum Vorgänger oder Belege in Stücken |
| A45 | **Ein gefälschter Beleg am Anker von außen** | ✅ | Beim Durchgang im eigenen Code des Vortags gefunden (2026-11-08, nie ausgeliefert): Der Anker legte nur Höhe und Block fest. Seine Unterschriften prüft niemand, und gerade sie binden die Sätze der nächsten Epochen an den Block; eine Gegenstelle konnte also einen Beleg mit richtigem Block und eigenen Sätzen liefern. Geschlossen in NODE 0.116.3: Der Anker nennt auch die Bindung. Gegenprobe im Test: mit dem alten Vergleich liegt die Fälschung, jetzt nicht |
| A46 | **Eine erfundene Epoche hält einen Knoten an** | ✅ | Bei der Prüfung von A43 gefunden (2026-11-08): Die Vertrauensfrist las die Epoche aus dem Kopf eines noch ungeprüften Blocks aus dem Gossip, und ihr Anhalten ist endgültig. Eine einzige Nachricht beendete jeden vollen Knoten ausserhalb der Probekette; die Netzläufe fahren auf der Probekette, die von dieser Regel ausgenommen ist, und konnten es nicht zeigen. **Geschlossen** (NODE 0.117.0): Die Epoche des Netzes kommt aus der eigenen Höhenschranke und der Uhr, ohne fremde Angabe; dieselbe Regel gilt jetzt auch für den Knoten ohne Zustand. Tests mit Gegenproben |
| A47 | **Falsches wird netzweit weitergereicht** | ✅ | Beim grossen Durchgang gefunden (2026-11-08): Die Netzschicht prüfte nur die Lesbarkeit und reichte jeden Block und jede Transaktion sofort weiter, auch ohne gültige Unterschrift; verworfene Nachrichten wurden nicht bewertet. Eine Flut traf jeden Knoten des Netzes, und der Puffer der Transaktionen wuchs ohne Grenze, je Stapel eine Rechenaufgabe bis zur Grenze der Laufzeit. **Geschlossen** (NETWORKING 0.25.0, NODE 0.118.0): Blöcke nach dem Beleg des Erzeugers, Transaktionen nach ihrer Unterschrift, Falsches zählt gegen den Absender, Puffer und Prüfungen gedeckelt. Tests mit Gegenproben |
| A48 | **Anfragen ohne Drossel** | ✅ | Beim grossen Durchgang gefunden (2026-11-08): Nur Nachholen und Archiv gingen durch die Drossel. Eine Anfrage von wenigen Bytes konnte unbegrenzt oft zwei MiB Belege serialisieren, einen Verlauf von 64 MiB aufbauen oder Unterschriften prüfen lassen, in der Ereignisschleife. **Geschlossen** (NODE 0.118.0): eine eigene Art für alles, was Arbeit kostet; der Deckel einer Prüfpunktstimme steht vor ihrer Unterschrift, was je Gegenstelle gemerkt wird, geht mit ihr. Gemessen: die Zuteilung einer Epoche 2 ms bei 5 000 Minern |
| A49 | **Eine Reserve hin und her schicken** | ✅ | Beim grossen Durchgang gefunden (2026-11-08): Eine Übernahme ist gegen die Kette geprüft, der Absender war nicht gebunden. Jede Gegenstelle konnte eine Reserve immer neu auf eine andere Position schicken und so das Einspringen verhindern. **Geschlossen** (NODE 0.119.0): höchstens ein Wechsel je Minute von eingetragenen Minern, alle zehn Minuten von anderen, und ein Miner verdrängt den Auftrag eines Fremden sofort (Probe `eine_uebernahme_haelt_ihre_pause`). Rest: Ein eingetragener Miner kann weiter stören; er zahlt den Mindesteinsatz je Kennung |
| A50 | **Ein Beleg, an dem der Prüfer stirbt** | ✅ | Beim grossen Durchgang gefunden (2026-11-08): Der Beleg der Schiedsrunde kommt vom Angeklagten. Ein Pod, der sich auf einen Vektor falscher Länge festlegte, liess die Aufgabe jedes Prüfers an einer Panik sterben, niemand stimmte, und er entging dem Urteil. **Geschlossen** (VERIFICATION 0.29.0): bezeugt und in falscher Länge ist überführt. Probe gegen das echte Modell |
| A25 | **Zwei Quoren ohne gemeinsamen Teilnehmer** | ✅ | **Fund 652** (2026-10-26) geschlossen: ein Gesamtgewicht über `u64` sättigte; Eigenschaftstest `quorumschnitt.rs` |

---

## 2. Abgewehrt und belegt

### A1 Falsches Rechenergebnis

Drei Stufen, und sie decken verschiedene Fälle ab:

- **Stufe 1 (Redundanz)** greift, wenn nur *ein* Pod lügt. Binärer
  Vergleich ohne Toleranzfenster; es gibt keinen Bereich, in dem sich
  eine Manipulation verstecken ließe.
- **Stufe 2 (Bisektion)** grenzt im Streitfall die abweichende Layer ein.
  ⚑ Sie tat das bis zum 2026-08-23 **falsch** (Fund 42: sie nannte `d−1`
  statt `d`, hätte den Betrüger freigesprochen und den ehrlichen Checker
  geschlachtet, in 15 von 16 Fällen). Behoben und über jede Position
  jeder Spurlänge geprüft.
- **Kontrollsegmente** greifen gegen den **einmaligen** Eingriff, auch
  wenn der Angreifer beide Pods hält. Siehe aber A11.

### A2 Rogue-Key

`fast_aggregate_verify` ist ohne Besitznachweis angreifbar, und die
Konstruktion `pk_rogue = g₁^x · pk_opfer⁻¹` besteht `key_validate()`.
Nachgewiesen, nicht vermutet; behoben nach
draft-irtf-cfrg-bls-signature §3.3. Die Regression hält **beide**
Tatsachen fest: dass der Rogue Key die Validierung besteht **und** dass
der Besitznachweis ihn ausschließt.

### A3 Koordinator fälscht

Der Koordinator ist die einzige Stelle im Pod, die für alle spricht.
Abgewehrt: Anspruch nachträglich erhöhen, Segmente hinzudichten,
weglassen, umsortieren, fremdes Bündel einreichen, allein oder zu zweit
unterschreiben.

⚑ **Fund 52 dabei:** Bis zum 2026-08-24 verifizierte **kein** Bündel aus
dem Pod, weil Pod und Konsens über verschiedene Botschaften redeten. Die
Richtung war die gute (abgelehnt statt angenommen), aber der
Vergütungspfad war unbenutzbar. Geschlossen durch eine Signaturrunde über
das fertige Bündel, in der **jedes Mitglied den Anspruch prüft, bevor es
unterschreibt**.

### A6 Self-Dealing

⚑ **Fund 49:** Die Grenze `s < c/(1−c)` ließ sich in zwei je zulässigen
Schritten verschieben, weil `c` abstimmbar war. Geschlossen: geprüft wird
gegen das **untere** Ende des Bandes aus Anhang B.4 (c = 0,6 ⇒ s < 1,5),
und die Prüfung nimmt kein `c` mehr entgegen.

### A7 Verbrauchs-Stoß

Kap. 5.6 nannte den Burn-Cap je Adresse seit v0.1 als Gegenmittel; **er
war nicht implementiert.** Jetzt ist er es. Gemessen bei einer EMA von
20 000 MYL: Ein Stoß von 200 000 MYL hebt die EMA ohne Deckel um
11 612 MYL, mit Deckel gar nicht.

**Was er nicht leistet, gehört dazu:** Zwanzig Adressen mit je eigener
Deckung erreichen denselben Stoß. Der Deckel macht daraus eine
**Kapitalfrage statt einer Sybil-Frage** — die MYL müssen wirklich da
sein.

---

### A14 Ein Gateway liest den Inhalt mit

**Die Klasse stand bis zum 2026-08-27 nicht in diesem Dokument**, obwohl
seine Grundlage Kap. 9.2 ausdrücklich einschließt und Kap. 9.2 die
kompromittierten Gateways ausdrücklich nennt. Nachgetragen mit dem
Schritt, der sie schließt: Ein Audit, das seine eigene Grundlage nur zur
Hälfte abbildet, zählt richtig und deckt zu wenig ab.

**Der Angriff:** Nutzer und erster Shard sprechen über ein Gateway. Mit
Transportverschlüsselung allein sind das zwei Noise-Verbindungen mit
Klartext dazwischen. Das Gateway rechnet nicht, es leitet weiter, und
genau deshalb liefe dort der Verkehr vieler Nutzer zusammen: der
Sammelpunkt, den Kap. 9.2 ausschließen will.

**Abgewehrt seit `myl-net` v0.9.0.** Sitzungsschlüssel je Epoche und
Pod, Ende zu Ende zwischen den beiden wirklichen Gesprächspartnern. Der
Klartextkopf trägt, was zum Weiterleiten nötig ist, und geht vollständig
in die Authentisierung ein: Das Gateway darf lesen, was es zum Leiten
braucht, und nichts davon ändern.

**Gemessen (`tests/sitzung.rs`):** Drei Knoten, ein wirklich
weiterleitendes Gateway, das annimmt, weitergibt, zurückgibt und
unterwegs zu öffnen versucht. Es scheitert, der Shard öffnet, und der
Klartext steht nachweislich nicht in den Bytes, die über das Gateway
gehen. Die Gegenprobe steht im selben Test: Der erlaubte Fall gelingt,
sonst hieße „niemand konnte lesen" nur „es kam nichts an".

⚑ **Die Abwehr hing zwischenzeitlich an der falschen Identität.** Der
Punkt, gegen den verschlüsselt wird, wird angekündigt und beglaubigt;
die erste Fassung beglaubigte ihn mit der Netzidentität. Der Pod-Pfad
nennt aber `MinerId`s, und die Zuordnung dorthin gibt es nicht. Seitdem
unterschreibt der Konsensschlüssel, und weil die `MinerId` der Hash
eben dieses Schlüssels ist, braucht die Prüfung nichts weiter. **Ohne
diese Berichtigung wäre die ganze Abwehr dieser Klasse nur scheinbar
gewesen**, und zwar mit grünen Tests daneben.

⚑ **Was dabei sichtbar bleibt, und es ist nicht nichts:** Der
Klartextkopf nennt Epoche, Pod, Absender, Empfänger und Zähler, und die
Länge einer Nachricht steht ohnehin auf dem Draht. Ein Gateway lernt
daraus die Pod-Zusammensetzung, wer mit wem spricht und wie viel. Es
lernt **nicht**, was gesprochen wird.

Das Whitepaper verspricht an dieser Stelle auch nichts anderes: Kap. 9.2
spricht von Prompt-Inhalt und Aktivierungswerten. Verkehrsdatenschutz
wäre eine eigene Aufgabe mit eigenen Kosten (Füllverkehr, feste
Nachrichtengrößen, Umwege), und er stünde in Spannung zu einer
Pod-Bildung, die auf gemessener Latenz beruht. **Hier steht er als
benannte Grenze, nicht als offene Lücke:** Niemand hat ihn zugesagt.

⚑ **Das Schema selbst ist ungeprüft, und das gehört hierher und nicht
in eine Fußnote.** A13 sagt „nie extern geprüft" über die
Krypto-Primitiven; seit dem 2026-08-27 gibt es zusätzlich ein eigenes
Schema aus X25519, HKDF und ChaCha20-Poly1305, und niemand von außen hat
es angesehen. Die Bausteine sind Standard, die **Zusammensetzung** ist
es nicht, und Fehler in Verschlüsselungsschemata sitzen erfahrungsgemäß
in der Zusammensetzung. Die neunzehn Gegenproben zeigen, dass die
gebauten Prüfungen greifen; sie zeigen nicht, dass die richtigen
Prüfungen gebaut wurden.

**Und was ausdrücklich nicht abgewehrt ist:** die beteiligten
Shard-Miner selbst. Ihre Aufgabe ist die Verarbeitung des Inhalts. Kap.
9.3 zieht daraus die Risikoklasse C („ungeeignet"), und daran ändert
diese Fassung nichts.

### A15 Zwei Blattfolgen, eine Merkle-Wurzel (Fund 77)

**War der Fall bis zum 2026-08-28.** Der Merkle-Baum füllte eine Ebene
mit ungerader Knotenzahl auf, indem er den letzten Knoten mit sich
selbst paarte, also in der klassischen Bauart, und erbte damit deren Fehler
(CVE-2012-2459). Die Abbildung von Blattfolgen auf Wurzeln war **nicht
injektiv**, und die Kollisionsfamilie war größer als der bekannte
Einzelfall:

| Blattzahl | kollidierte mit |
|---|---|
| 3, 5, 7, 9, … (ungerade ab 3) | derselben Folge plus wiederholtem letzten Blatt |
| 6, 14, 22, … (`n ≡ 2 mod 4`, ab 6) | derselben Folge plus den letzten **zwei** Blättern |
| 1, 2, 4, 8, … | nichts davon |

**Wo es scharf gewesen wäre.** Das PoI-Bündel führt kein Feld für die
Segmentzahl; die Signierbotschaft ist
`epoch ‖ pod ‖ segments_root ‖ vtfe_claimed`. Eine Aggregatsignatur über
`n` Segmente galt damit zugleich für `n+1` Segmente mit wiederholtem
letzten. Ein Angreifer gewann daraus **nichts**, weil `vtfe_claimed`
eigenständig signiert ist und nichts eine Mitgliedschaft gegen
`segments_root` prüfte. Es wäre in dem Augenblick scharf geworden, in
dem ein Streitverfahren oder eine Kontrollsegment-Prüfung einen
Merkle-Beweis dagegen laufen lässt.

**Behoben am Tag des Fundes** (`myl-types` v0.6.0): Die Wurzel ist
seither `SHA-256(0x02 ‖ u64_le(n) ‖ innere Wurzel)`. Der innere Aufbau
samt Duplikationsregel ist unverändert; gebunden wird nur die Blattzahl.

**Der Beweis in einem Satz:** Aus gleicher Wurzel folgt gleiches Urbild,
also gleiche Blattzahl und gleiche innere Wurzel; bei fester Blattzahl
liegt die Baumform fest, und über die Domain-Separation bestimmt jeder
Knoten seine beiden Kinder eindeutig.

**Belegt durch:** einen Test, der die Nachbarschaft jeder Blattzahl von
1 bis 12 abfährt (Verlängerung um ein und um zwei Blätter, keine trägt
dieselbe Wurzel); eine Gegenprobe, die festhält, dass die **inneren**
Wurzeln weiterhin zusammenfallen und allein das Präfix sie trennt; einen
Test über sechs erfundene Blattzahlen im Beweis, die alle scheitern; und
einen fünften Konformitätsvektor `four_leaves_last_repeated`, den eine
Umsetzung ohne Blattzahlbindung als einzigen nicht trifft.

⚑ **Die Gegenprobe ist hier wichtiger als der Haupttest.** Ein grüner
Injektivitätstest allein bewiese nicht, dass die Bindung die Ursache
ist; er wäre auch grün, wenn sich nebenbei die Auffüllregel geändert
hätte. Deshalb steht die alte Kollision ausdrücklich im Testcode.

**Gemessene Folgen:** fünf neu erzeugte Konformitätsvektoren und ein
geänderter Gesamtwert des Protokoll-Durchlaufs, von `8c74519a11dceae5`
auf `d02dcacb6aa37026`. Die Ursache ist belegt statt angenommen: Mit der
alten Konstruktion liefert derselbe Lauf weiterhin den alten Wert, und
geändert hat sich allein die Krypto-Stufe. **Kein Crate musste angepasst
werden**, weil alle Wurzeln zur Laufzeit neu entstehen.

⚑ **Was daran über den Fund hinaus lehrreich ist.** Die
Duplikationsregel **war getestet**, und die Domain-Separation ist sauber
und mit dem richtigen Argument begründet. Geprüft wurde, dass die Regel
tut, was sie soll. Nicht geprüft wurde, was daraus **folgt**. Das ist
Fehlerklasse 3 dieses Projekts in einer Sicherheitsprimitive: ein Test,
dessen Form den Defekt verdeckt, weil er die richtige Frage nicht
stellt. Und der Baum in `da.rs` band die Blattzahl über `k` und `m` von
Anfang an mit, also hatte eine Stelle das Richtige getan, ohne dass
jemand die Regel daraus verallgemeinert hätte.

---

## 2b. Tür, Wallet und lokale Leitungen (Nachtrag 2026-10-26)

Was hier zugesagt wird, ist dasselbe wie in Z3: Ein Agent kommt über
seinen Sitzungskontrakt nicht hinaus, und **dazu**, dass ein Betreiber für
Arbeit, die er an der Tür annimmt, auch bezahlt wird. Das Zweite ist neu;
ohne es wäre die Tür ein Ort, an dem Rechenzeit verschenkt wird.

### A16 Tür ohne gültige Vollmacht

Die Tür prüft die Vollmacht vor jedem Zerlegen des Rumpfes: Unterschrift
des Agenten, Kette der Abschwächungen, Vorbehalte, dann den Kontrakt aus
der Kette (Agent, Widerruf, Zeitfenster). Jede Ablehnung ist dieselbe 401.
Belege: `GATEWAY/myl-gateway/tests/tuer.rs` (`ein_fremdes_token_wird_wie_alles_andere_abgewiesen`,
`jede_ablehnung_sieht_gleich_aus`, `zwei_authorization_koepfe_werden_abgewiesen`),
`zugang.rs` (`ein_fremder_schluessel_kommt_nicht_durch`, `widerruf_ablauf_und_vorlauf_schliessen`),
`vollmacht.rs` (`ein_geaenderter_vorbehalt_bricht_die_kette`, `ein_untergeschobener_nachfolger_bricht_die_kette`).

### A17 Tür rechnet über die Grenzen des Kontrakts (Fund 651)

Gefunden beim Aufstellen dieses Nachtrags. Die Tür prüfte den Kontrakt mit
dem Betrag eins; Einzellimit und Restbudget prüfte erst die Kette beim
Abbuchen, also nach dem Rechnen. Eine Anfrage darüber bekam ihre Antwort,
die Abbuchung scheiterte, und die Arbeit war verschenkt, beliebig oft.
**Geschlossen:** Der Höchstbetrag der Anfrage (`max_tokens`) wird vor dem
Rechnen mit derselben Prüfung gegen den Kontrakt gehalten, die die Kette
anlegt (`Zugangsstelle::deckt_der_kontrakt`, 402). Beleg:
`TESTCLIENT/myl-testclient/tests/tuer_bis_rechenwerk.rs`
(`einzellimit_und_restbudget_greifen_vor_dem_rechnen`, das Rechenwerk läuft
nicht); ohne die Prüfung rot.

### A18 Veralteter Stand an der Tür (Funde 647, 648)

Zwei Wege, auf denen die Tür gegen einen alten Stand prüfte: die Epoche,
gelesen vor dem Warten auf eine Verbindung (647), und die Abschrift der
Kontrakte, aufgefrischt nur alle dreissig Sekunden (648). Beides wirkte in
beide Richtungen: Eine neue Sitzung wurde abgewiesen, **und ein Widerruf
griff verspätet**. Belege: `NODE/myl-node/src/tuer.rs`
(`ein_kontrakt_der_erst_spaeter_gilt_kommt_erst_spaeter_durch`, die Kette
wandert, während die Tür wartet), `TESTCLIENT/myl-testclient/tests/bezahlen.rs`
(die Abschrift des Knotens gleich nach dem Block); beide mit Gegenprobe.

### A19 Doppelte oder fremde Abrechnung

Eine Abrechnung trägt eine Nummer, die je Sitzung steigen muss, und die
Vollmacht des Agenten. Belege: `tuer_bis_rechenwerk.rs`
(`eine_gerechnete_anfrage_bucht_credits_ab`, dieselbe Abrechnung ein
zweites Mal scheitert), `CONSENSUS/myl-ledger/src/transitions.rs`
(`eine_vollmacht_autorisiert_die_abbuchung_durch_einen_fremden`,
`ein_agent_kommt_ueber_die_grenzen_nicht_hinaus`).

### A20 Lokale Leitungen ohne Ausweis

Relaisleitung (Knoten zu Koordinator und Wallet) und Ortsleitung (Knoten zu
Shard) nehmen nur Rahmen mit dem Ausweis aus einer Datei mit Rechten
`0600`; ohne ihn kommt keine Antwort. Belege: `NODE/myl-node/src/relais.rs`
(`nur_mit_ausweis_kommt_eine_frage_durch`), `myl-types/src/ortsleitung.rs`
(`ein_falscher_ausweis_kommt_nicht_durch`, `die_laenge_wird_erst_nach_dem_ausweis_gelesen`),
`myl-pod/src/ortsdienst.rs` (`ohne_ausweis_bleibt_die_tuer_stumm`). ⚠️
**Grenze des Modells:** Wer als derselbe Benutzer auf der Maschine läuft,
liest den Ausweis. Das ist gewollt; die Leitungen trennen Prozesse eines
Betreibers, nicht Betreiber voneinander.

### A21 Transaktion aus dem Wallet verfälscht

Die Unterschrift bindet Netzwert, Nonce und Anweisung; der Knoten prüft sie,
bevor er verbreitet. Belege: `CONSENSUS/myl-consensus/src/block.rs`
(`eine_unterschrift_bindet_kette_nummer_und_anweisung`),
`myl-ledger` (`die_nonce_geht_streng_der_reihe_nach`,
`nur_der_inhaber_widerruft_und_zweimal_schadet_nicht`),
`TESTCLIENT/myl-testclient/tests/bezahlen.rs` (eine verfälschte Transaktion
kommt nicht ins Netz, ein fremdes Wallet widerruft nicht).

### A22 Auszahlungskonto umlenken

Die erste Eintragung unterschreibt der Miner, jede weitere das bisher
eingetragene Konto (Fund 643). Belege: `myl-ledger`
(`nach_der_ersten_eintragung_darf_nur_noch_das_konto`, `ein_fremder_traegt_nicht_ein`),
`NODE/myl-node/tests/auszahlung.rs`.

### A23 Die Tür belegen (geschlossen)

Die Tür bediente eine Anfrage nach der anderen; ein **gültiger** Kontrakt
belegte sie für die Dauer eines Auftrags, bis zu 540 s (Fund 650). Seit NODE
0.117.0 und GATEWAY 0.11.0 laufen bis zu 32 Anfragen zugleich, so viele, wie
die Ortsleitung des Shards annimmt. Der Deckel steht vor dem Lesen: Ohne
freien Platz gibt es sofort 503, für alle gleich. Je Sitzung rechnet eine
Anfrage, die zweite bekommt 429 und keine Sitzungsnummer; das verhindert auch,
dass zwei Anfragen dieselbe Budgetprüfung bestehen und die Kette danach eine
abweist. Annahme und Zugang sind nur für die Buchung gesperrt, nie über die
Inferenz. Die Lesefrist (Fund 610) und der Takt je Kontrakt gelten weiter.

### A24 Auftrag über die Epochengrenze (Fund 649, geschlossen)

Am Epochenwechsel beenden die Knoten ihre Shards sofort, auch mitten in
einem Auftrag. Der Auftrag scheitert, und **niemand wird belastet**; es ist
eine Frage der Verfügbarkeit, nicht des Werts. Verwandt mit dem bekannten
Punkt „Nachrichten an der Epochengrenze“ (A14). **Geschlossen am selben
Tag:** Kommt kein Token und steht die Kette in einer neuen Epoche, rechnet
deren Pod den Auftrag einmal neu (COMPUTE_PIPELINE 0.51.0,
`tests/podwerk_epochenwechsel.rs` mit Gegenprobe).

### A26 Ein ehrlicher Miner wird gesperrt (Fund 655, geschlossen)

Gemessen im langen Podlauf über Docker (2026-10-26, 100 Epochen, Störer,
`tc netem`): Ein ehrlicher Miner wurde zweimal für eine Epoche gesperrt.
Beide Male lagen drei Zeitproben über seiner Frist (130 bis 394 ms bei
100 ms), alle in den ersten 25 s einer Epoche, 14 bis 25 s nach dem Start
seines Shards, und dieser rechnete zur selben Zeit schon für einen Auftrag
von der Tür. In ruhigen Phasen lagen dieselben Messungen bei 2 bis 14 ms.
**Die Zeitprobe misst damit nicht nur die Zusage, sondern auch Anlauf und
Last**, und drei ehrliche Zeugen reichen für eine Sperre. Das ist die Klasse
„eine Abwehr trifft den Falschen“, und sie wiegt schwerer als ein
entkommener Lügner. Verwandt: Fund 646 (das Messverfahren unter Latenz).
**Behoben (NODE 0.64.0, Festlegung des Projektinhabers):** Gemessen wird
erst ab einem Viertel der Epoche, und ein Zeugnis gibt es erst nach einer
zweiten Überschreitung mindestens ein Achtel Epoche später. **Belegt im
langen Podlauf vom 2026-10-26/27** (100 Epochen, Tür, Störer, `tc netem`,
35 Abstürze): 1 748 Messungen bestanden, 210 Verdachtsfälle, 121 Zeugnisse,
alle 88 Meldungen gegen den Lügner, der 28-mal gesperrt wurde; kein
ehrlicher Miner gemeldet oder gesperrt. Ein einzelner Lauf ist ein Beleg,
kein Beweis (im ersten Lauf zwei falsche Sperren in 100 Epochen); der Kern
steht im Test mit Gegenprobe.

### A27 Die Stichprobe erreicht niemanden (Fund 656, geschlossen)

A1 stützt sich auf drei Stufen; die zweite ist die Stichprobe: Die Kette
zieht Segmente, der Knoten fragt die Mitglieder des Pods nach Spur und
Beweis, und ein Prüfer rechnet nach. **Im Netz kommt die Frage nicht an.**
`Knoten::stichprobe_verschicken` schickt die nackte `Spuranfrage`, der
Empfänger liest jede Anfrage als `Nachforderung` und bekommt `None`; dazu
hält kein Mitglied die Spuren, die es liefern müsste, und die fragende Seite
prüft keine Antwort. Die Bausteine in `myl-verifier` sind geprüft, die Nähte
fehlen: antworten (die Shards halten, was sie unterschrieben haben),
prüfen, nachrechnen, folgen. ⚠️ **Bis
dahin gilt A1 im Netz nur über Stufe 1 (Redundanz) und die
Kontrollsegmente**, nicht über die Stichprobe.

### A25 Zwei Quoren ohne gemeinsamen Teilnehmer (Fund 652)

Die Sicherheit des BFT hängt daran, dass sich zwei Quoren in mehr als
einem Drittel des Gewichts schneiden. `CONSENSUS/myl-consensus/tests/quorumschnitt.rs`
prüft das über den echten Code: zufällige Gewichte, für jede Reihenfolge
die beiden Quoren mit dem kleinsten Schnitt. Bei gewöhnlichen Gewichten
hält es immer. **Ging die Summe der Gewichte über `u64`, sättigten
Gesamtgewicht, Schwelle und Zählung**, und zwei Quoren ohne einen
gemeinsamen Teilnehmer waren möglich. Im Betrieb ist das Gewicht der
Stake, und die Geldmenge hält die Summe weit darunter; geprüft war diese
Schranke aber nirgends. **Geschlossen:** Eine solche Summe ergibt eine
Menge ohne Gewicht, und der Konsens steht, statt sich zu widersprechen.
Gegenprobe mit dem alten Verhalten: rot.

⚠️ **Was der Test nicht zeigt:** Sperre, Rundenwechsel und den Wechsel
der Menge an der Epochengrenze. Die Frage aus dem Auditzuschnitt dazu
bleibt offen; der Test belegt nur die Schwelle, auf der sie aufbaut.

---

## 3. Offen, mit gemessener Lücke

### A9 Eclipse (Fund 53) — geschlossen, mit benannter Restbedingung

**War gemessen (2026-08-24, früh):** Zwanzig Sybil-Identitäten verbinden
sich mit demselben Opfer, **alle zwanzig werden angenommen**.
`build_swarm` hatte kein `connection_limits`, kein Peer-Scoring, keine
Diversitätsregel.

**Warum das teuer war:** Wer beliebig viele Verbindungen aufbauen darf,
füllt die Peer-Menge des Opfers und entscheidet danach, **welche
Nachrichten es sieht**, nicht durch Fälschung, sondern durch Auswahl.
Die Sicherheit dieses Protokolls hängt daran, dass Checker fremde
Segmente **beobachten**; wer die Beobachtung steuert, steuert die
Verifikation.

**Behoben (2026-08-24, myl-net v0.4.0):** `src/limits.rs` und
`src/scoring.rs`. Der Kern ist eine Trennung: Eingehende Verbindungen,
die der Angreifer wählt, sind bei 48 gedeckelt; ausgehende, die der
Knoten selbst wählt, haben ein eigenes Budget von 16, und die
Gesamtgrenze ist die Summe. **Eine Flut kann die ausgehenden Plätze
deshalb nicht aufzehren.** Dazu vier eingehende je Adressbereich
(IPv4 /24, IPv6 /64) und Gossipsub-Peer-Scoring mit Graylist.

Belegt in `tests/eclipse_sybil.rs` als Kette: eingehend gedeckelt,
ausgehendes Budget unter Flut frei, und über die selbst gewählte
Verbindung kommt auch etwas an.

**⚠️ Die Restbedingung, ausdrücklich:** Garantiert ist, dass der Knoten
**wählen darf**, nicht dass er **richtig wählt**. Adressen kommen aus der
Bootstrap-Liste und aus Kademlia. **Der Eclipse-Angriff reduziert sich
damit auf die Bedingung: Die Bootstrap-Liste enthält mindestens einen
ehrlichen Knoten.** Wer sie stellt, umgeht die Verteidigung. Deshalb
steht A9 hier auf ⚠️ und nicht auf ✅.

**Zwei Funde dabei:**

- **Fund 54:** Die erste Fassung setzte die IP-Kolokationsschwelle auf 4
  und schaltete damit den ehrlichen Knoten mit stumm (−245 bei Graylist
  −80). Die Zahl war zusätzlich wirkungslos, weil die
  Adressbereichsgrenze bereits schärfer bindet. Eine Härtung, die
  niemand durchgerechnet hat, ist eine Vermutung mit Vorzeichen.
- **Fund 55:** Der dokumentierte Weg für die Nutzlastprüfung
  (`report_with`) war über `run_node` nicht erreichbar. Behoben mit
  `run_node_mit()`.

**Weiterhin offen:** Diversität je **ASN** statt je Adressbereich. Ein
Angreifer mit einem großen Provider bekommt viele /24. Die Metadaten
liegen in `myl-types/node_metadata.rs`; es fehlt eine vertrauenswürdige
Quelle für die Zuordnung.

### A10 Latenzwerte fälschen (eingeordnet, siehe Tabelle)

**War:** `myl_types::LatencyAttest` trug ein `signature`-Feld, das **im
ganzen Projekt niemand verifizierte**, und niemand erzeugte ein Attest.

**Warum das schlimmer war als ein fehlendes Feld:** Ein ungeprüftes
Signaturfeld ist gefährlicher als gar keines, weil ein Leser es für
einen Schutz hält. Die Latenzwerte gehen ins Geo-Clustering der Pods;
wer sie frei setzen kann, sucht sich seine Pod-Nachbarn aus, und das ist
die Vorstufe zur Kollusion beider Pods (A12).

**Behoben (2026-08-25):**

- `LatencyAttest::sign` und `::verify` in `myl-types`, 7 Tests. Sie
  fehlten schlicht: Es gab `signable_bytes()`, aber nichts, was damit
  signierte oder prüfte.
- `myl_node::Validatorsatz` ordnet Kennung zu Schlüssel und liefert den
  **Grund** einer Ablehnung, nicht nur ein Nein.
- `ProtokollValidator` prüft Atteste. Dort stand vorher `_ => true`.
- Der Knoten **erzeugt** Atteste aus seinen tatsächlich gemessenen
  Latenzen, nicht aus erfundenen Zahlen.

**Live belegt, drei Knoten:** Alpha und Beta kennen einander und nehmen
gegenseitig an; ein dritter, der in keiner Liste steht, bekommt alle
seine Atteste verworfen und verwirft selbst alle fremden.

**⚠️ Warum nicht ✅:** Im Probelauf werden die Schlüssel aus den
Teilnehmernamen abgeleitet, die der Koordinator ohnehin verteilt. **Wer
die Namen kennt, kann die Schlüssel ableiten** und damit in fremdem
Namen signieren. Für eine Trockenübung ist das hinnehmbar, für ein
echtes Netz nicht.

Die Trennlinie liegt **nicht im Prüfcode**, sondern in der Herkunft der
Schlüssel: Dieselbe Prüffunktion arbeitet unverändert gegen echte
Schlüssel, sobald die Validator-Registrierung zu Genesis steht, mit
Besitznachweis (Fund 27). Das ist dieselbe Voraussetzung, die auch die
BFT-Runden brauchen.

### ⚑ A11 Kontrollsegmente erkennen (gegenstandslos, siehe Tabelle)

Die Kontrollsegmente sind seit dem 2026-09-02 entfallen. Was folgt, hält fest, was damals gemessen wurde.

Kap. 6.7 verlangt Ununterscheidbarkeit als **erste**
Konstruktionsanforderung. Die Mechanik steht seit myl-verifier v0.5.0;
die Ununterscheidbarkeit ist eine **Eigenschaft der Daten, nicht des
Codes**, und das Whitepaper führt sie als offene Messfrage (Kap. 11,
Punkt 5).

**Was sich trotzdem messen ließ**, und das war der Ertrag vom
2026-08-25: ob die **Einschleusung selbst** eine Spur hinterlässt,
unabhängig vom Inhalt der Prompts. `myl-verifier/src/unterscheider.rs`
baut den Angriff, statt ihn zu beschreiben.

⚑ **Fund 58: Ein zu kleiner Vorrat verrät sich vollständig.**

Der Vorrat ist endlich, der Auftragsstrom nicht. Wird öfter
eingeschleust, als der Vorrat verschiedene Segmente hält, wiederholen
sich Ids. **Echte Arbeit wiederholt sich nie**, also ist jedes zweite
Auftreten einer Id ein Beweis, kein Verdacht. Ein Miner braucht dafür
keine Statistik, keine Schwellenwerte und keine Annahme über Prompts,
nur ein Gedächtnis.

Gemessen bei γ = 2 % über 100 000 Aufträge:

| Vorrat | erkannte Kontrollen | Fehlalarme |
|---|---|---|
| 64 | **96,8 %** | 0 |
| 256 | 87,2 % | 0 |
| 1024 | 48,8 % | 0 |
| 2048 | 0 % | 0 |

**Null Fehlalarme ist der schlimmste Teil.** Der Angreifer geht kein
Risiko ein: Er rechnet die erkannten Kontrollen ehrlich und manipuliert
den Rest. Der Mechanismus, der als einziger gegen den **einmaligen**
Eingriff wirkt, wäre damit wirkungslos, und mit ihm A1 für den Fall,
dass der Angreifer beide Pods hält.

**Was den Fund schärfte: γ war ein Governance-Parameter, die
Vorratsgröße keiner.** Die einzigen je geschriebenen Werte standen in
Tests: 10 und 100. Bei γ = 2 % trägt ein Vorrat von 100 genau 5 000
Aufträge. **Die Zahl, an der die Wirksamkeit hängt, war nirgends
festgelegt.**

⚑ **Fund 72 (2026-08-27): Die zweite vermutete Spur gibt es nicht.**

Der Einschleusungsplan zieht **ohne Zurücklegen**: Ein Strom von `n`
Aufträgen enthält genau `⌈n·γ⌉` Kontrollen statt einer schwankenden
Zahl. γ ist ein Governance-Parameter und damit öffentlich, `n` sieht ein
Miner am Strom, also kennt er das Kontingent. Hätte er so viele
Kontrollen erkannt, wie hineinpassen, wäre jeder weitere Auftrag mit
Sicherheit echt, und auf einem sicher echten Auftrag kostet Betrug
nichts. Das sah nach einem Verstärker für Fund 58 aus.

**Gemessen: null.** Über jeden Vorrat von 1 bis 4 096, bei γ = 2 % und
100 000 Aufträgen, wurde das Kontingent nie erschöpft.

**Und die Null hat einen Grund, kein Glück.** Die Wiederholung erkennt
ein Segment frühestens beim **zweiten** Auftreten; von `v`
Vorratssegmenten bleiben die ersten `min(v, k)` Auftreten unerkannt.
Damit gilt `verdächtig ≤ k − min(v, k) < k` für jeden Vorrat `v ≥ 1`.
Die gemessenen Zahlen sind exakt `2 000 − Vorrat`, also der Satz mit
Gleichheit. Erschöpfen ließe sich das Kontingent nur mit einem
Unterscheider, der **jede** Kontrolle erkennt, und wer den hat, braucht
das Kontingent nicht.

**Folgerung: kein Umbau.** Unabhängiges Ziehen je Position würde ein
Loch schließen, das es nicht gibt, und dafür die exakte Einhaltung von γ
je Strom aufgeben, an der die Kostenbuchhaltung hängt.

**Was seit dem 2026-08-27 zusätzlich existiert: das Messgerät** für die
eigentliche, inhaltliche Ununterscheidbarkeit
(`myl-verifier/src/unterscheidbarkeit.rs`, Punkt 3.6). Ein
Vertauschungstest auf ganzzahligen Merkmalen, der kein Nein ohne seine
Trennschärfe herausgibt. **Das ist kein Beleg, sondern die
Voraussetzung dafür, später einen zu bekommen**: Länge, Timing und
Kontextprofil brauchen echten Verkehr, und A11 bleibt deshalb auf ⚠.

**Geschlossen am 2026-08-27.** Vorratsgröße und Beobachtungsfenster sind
Governance-Parameter, und eine Invariante bindet beide an γ:
`Vorrat ≥ ⌈Fenster · γ⌉`, geprüft am **Vorschlag** statt nach der
Abstimmung. Der Wert ist abstimmbar, das Unterschreiten der Schranke
nicht — dieselbe Bauart wie beim Mindest-Stake.

⚑ **Der Zug, gegen den die Invariante gebaut ist, sieht wie eine
Verschärfung aus:** γ von 2 auf 4 Prozent heben, ohne den Vorrat
mitzuziehen. Das verdoppelt die Einschleusungen bei gleichem Vorrat und
halbiert die Reichweite. Vorher wäre er zulässig gewesen. **Das ist die
übertragbare Lehre:** Ein Parameter, der eine Schutzwirkung verstärkt,
kann eine andere aufzehren, und wer nur den geänderten ansieht, sieht es
nicht.

**Das Beobachtungsfenster musste ein eigener Parameter werden.** „Der
Vorrat muss größer sein als die Zahl der Einschleusungen" ist für einen
unbegrenzten Auftragsstrom von **keinem** endlichen Vorrat erfüllbar;
erst ein Fenster macht die Bedingung entscheidbar, und wer es senkt,
senkt die Schranke.

**⚠️ Warum weiterhin nicht ✅:** Geschlossen ist genau **eine** Spur, die
der Mechanik. Länge, Timing und Kontextprofil der Prompts bleiben
ungemessen; dafür braucht es echte Prompt-Verteilungen, und die
entstehen erst im Betrieb. Das bleibt die offene Messfrage aus Kap. 11
Punkt 5. **Und der Anfangswert ist vorläufig:** 2 048 ist der gemessene
Wert, 100 000 die Stromlänge der Messung; die Auftragsrate des Netzes
ist ungemessen.

---

---

## 4. Teilweise belegt

### A12 Kollusion beider Pods

Anhang B.2 gibt `P_koll ≈ β^{2k}` an. **Gemessen an der echten
Zuteilung** (β = 50 %, k = 4, 10 000 Zuteilungen): 3,900 · 10⁻³ gegen
3,906 · 10⁻³ der Formel, Übereinstimmung auf drei Stellen.

**Was das nicht belegt:** Die Formel unterstellt gleichverteilte
Ziehungen. Anhang B.2 vermerkt selbst, dass die geografische Clusterung β
**lokal erhöht**, und verschiebt die Analyse auf Meilenstein M1. Die
Messung oben läuft über künstlich gleichverteilte Regionen; ein
realistisches Clusterbild ist nicht geprüft.

**Zudem:** A10 ist der Hebel, mit dem ein Angreifer β lokal selbst
erhöht.

⚑ **Die Gegenmaßnahme im Netz (2026-11-06, Podlauf `--kollusion`):** 17 von
28 Minern fälschen gleich, aber nur an der ersten Position ihres Pods.
Stehen an beiden Zwillingen eines Paares solche Fälscher vorn, rechnen beide
gleich falsch, und das Paar ist einig: Stufe 1 sieht nichts. In zwei kurzen
Läufen kam das je einmal vor, und jedes Mal fand das Nachrechnen ab den
Tokens (T6) den Fälscher der ersten Position; die Schiedsrunde urteilte nur
gegen Fälscher, kein Ehrlicher wurde beschuldigt. Die Ziehung hängt nicht an
der Einigkeit der Zwillinge (`stichprobe_der_epoche` zieht aus allen
Bündeln). 📌 Beim ersten Lauf gesehen: Der Fälscher des Prüfstands
verschob einen Wert um eins, und das verschwand meist schon in der ersten
Layer; der Pod rechnete dann ehrlich, „gleich“ war richtig. Seitdem um 64.
**Offen bleibt** das realistische Clusterbild oben; darum ⚠️.

---

## 5. Ungeprüft

### A13 Die Krypto-Primitiven

⚑ **Nachtrag 2026-08-28, Quantenrechner:** Der Sitzungsaustausch ist
seit `myl-net` v0.10.0 hybrid (X25519 plus ML-KEM-768); aufgezeichneter
Verkehr ist damit auch gegen späteres Brechen geschützt. **Die
Signaturen sind es nicht**, und der Grund ist die Aggregation: Ein
Polka-Zertifikat ist 96 Byte unabhängig von der Validatorenzahl, mit
ML-DSA wären es bei 21 Validatoren rund 51 KB je Rundenwechsel, und ein
aggregierbares Post-Quantum-Verfahren ist nicht standardisiert.

**Die Reihenfolge ist begründet:** Eine 2040 gebrochene Signatur ist
wertlos, der Block ist längst final. Ein 2026 aufgezeichneter Prompt,
2040 entschlüsselt, ist es nicht.

Dass `myl-types` gegen die RFC-9381-Testvektoren stimmt, ist geprüft.
Dass die **Verwendung** trägt, war bis zum 2026-08-23
Eigenbau-Beurteilung; seither gibt es ein schriftliches Bedrohungsmodell
je Signaturverwendung
(`SHARED_TYPES/README/Signatur-Bedrohungsmodell.md`).

**Es ersetzt kein externes Review** (K5). Fund 27 ist der Beleg, warum
nicht: Der Schutz stand in der Dokumentation, bevor er im Code stand, und
niemand hat den Satz gegen die Literatur geprüft.

---

## 6. Was die Simulation zusätzlich zeigt

`SIMULATION/myl-simulation/tests/durchlauf.rs` fährt den Weg eines
Segments durch alle Schichten. Ergebnisse:

| Prüfung | Ergebnis |
|---|---|
| ehrlicher Durchlauf | keine schweren Befunde |
| falscher Pod → Urteil → Ledger | gefunden, 100 % Slash gebucht, Kopfgeld gezahlt |
| Prägekurve über 200 Epochen | Verteilung stets exakt gleich der Prägung |
| zu kleines Netz | meldet die fehlende Redundanz, statt still weiterzulaufen |
| Burn-Cap gegen den Stoß | greift, gemessen |

**Ein Befund über die Simulation selbst:** Der 200-Epochen-Durchlauf
erreicht den Burn-Cap **nie**, weil die EMA sich der Schwelle von unten
nähert. Ein Test, der den interessanten Zweig nicht betritt, prüft ihn
nicht — dieselbe Falle wie beim ersten Pod-Fuzzer. Deshalb steht der
Stoß-Fall als eigener Test daneben.

---

## 7. Woran zuerst zu arbeiten wäre

Nach Schadenshebel, nicht nach Aufwand:

0. ✅ ~~**A15 entscheiden, bevor der Spur-Merkle gebaut wird**~~
   (Fund 77) — entschieden und behoben am 2026-08-28, am Tag des Fundes.
   Der Punkt stand hier vor allen anderen, nicht weil er schadete,
   sondern weil er **billig zu beheben war, solange keine Wurzel zählt,
   und teuer geworden wäre, sobald eine zählt**. ⚑ **Das Muster gilt
   über diesen Fall hinaus und gehört auf diese Liste:** Fehler in
   Commitment-Konstruktionen sind die einzige Klasse, deren
   Behebungskosten mit der Zeit steigen, ohne dass jemand etwas falsch
   macht. Wer eine findet, behebt sie sofort oder begründet, warum
   nicht.

1. ~~**A9 Verbindungsgrenze** (Fund 53)~~ — erledigt am 2026-08-24.
   Nachtrag zur damaligen Aufwandsschätzung „der Aufwand ist klein,
   `connection_limits` ist eine Behaviour-Zeile": Das stimmte für die
   Zeile und nicht für die Aufgabe. Die Arbeit steckte im Herleiten der
   Zahlen und im Ausrechnen, wen sie treffen, und genau dort lag Fund 54.
2. ~~**A10 Attest-Signatur prüfen**~~ — erledigt am 2026-08-25. Die
   Vermutung „entsteht ohnehin mit dem Knoten-Binary" stimmte für die
   **Stelle**, nicht für die Sache: Der `PayloadValidator` war da, aber
   `LatencyAttest` hatte weder `sign` noch `verify`. Die Prüfung, für
   die alles vorbereitet schien, brauchte erst ihre Primitive.
   **Offen bleibt die Schlüsselherkunft**, und die hängt an derselben
   Validator-Registrierung wie die BFT-Runden.
3. ~~**A11 statistische Analyse**~~ — teilweise erledigt am 2026-08-25,
   die Behebung am 2026-08-27. Die Spur der **Mechanik** ist gemessen
   und geschlossen (Fund 58: Vorratsgröße und Beobachtungsfenster als
   Governance-Parameter mit einer Invariante, die beide an γ bindet);
   die Spur der **Daten** (Länge, Timing, Kontextprofil) braucht echte
   Prompt-Verteilungen aus dem Betrieb.

   Nachtrag zur damaligen Einordnung „offene Messfrage": Das stimmte
   für den Teil, an den alle gedacht hatten, und verdeckte den Teil, der
   schon heute messbar war. **Eine als offen abgelegte Frage wird nicht
   mehr gestellt**, und genau deshalb lag der Fund über Monate offen.
4. **A13 externes Review.** Vor dem Mainnet, nicht danach.

**Was ausdrücklich nicht auf dieser Liste steht:** ein weiterer
Testdurchlauf derselben Art. Die neun abgewehrten Klassen sind belegt;
mehr Tests derselben Machart erhöhen die Zahl, nicht die Sicherheit.
