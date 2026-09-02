# Sicherheitshinweise

*This document is bilingual. The English version follows below.*

**Stand:** 2026-09-02

## Wo eine Schwachstelle gemeldet wird

**Nicht als öffentliches Issue und nicht als Pull Request.** Beides ist
für jeden lesbar, und bei einer Schwachstelle im Konsens- oder
Kryptografiepfad ist die Veröffentlichung selbst der Schaden.

Der Weg ist die **private Schwachstellenmeldung von GitHub**: Reiter
`Security` dieses Repositoriums, dann `Report a vulnerability`. Der
Bericht ist nur für die Betreuer sichtbar, und die Antwort läuft im
selben Faden.

Wenn dieser Weg für Sie nicht offensteht, legen Sie ein öffentliches
Issue ohne technische Einzelheiten an, das allein um einen privaten
Kanal bittet. Nennen Sie darin **keine** Reproduktion, keine Datei und
keine Zeilennummer.

## Was dieses Projekt heute ist, und was daraus folgt

**Es gibt kein Mainnet, keinen Genesis-Block und keine übertragbaren
Werte.** Was heute läuft, ist ein Probelauf: Der Zustand ist
Wegwerfware, die MYL darin sind Spielgeld, und der Startwert der
Probekette sagt das im Klartext.

Daraus folgt zweierlei, und beides ist für Meldende wichtig:

1. **Es steht heute kein Geld auf dem Spiel.** Eine Schwachstelle in
   diesem Repositorium kostet niemanden etwas, solange kein Netz läuft.
2. **Gerade deshalb wirkt eine Meldung jetzt am meisten.** Eine Änderung
   an einer Commitment-Konstruktion kostet vor dem Genesis-Block ein
   paar Prüfvektoren und danach eine Kettenmigration. Wer heute meldet,
   verhindert eine Migration, nicht einen Verlust.

## Umfang

**Im Umfang** liegt alles, was das Protokoll trägt:

- Konsens und Ledger (`CONSENSUS/`), einschließlich Blockaufbau,
  Stimmgewicht, Quorum, Rundenwechsel und Zustandsübergängen
- Kryptografische Verwendung (`SHARED_TYPES/`): Domänentrennung,
  Aggregationsreihenfolge, Schutz gegen fremde Schlüssel, Bindung der
  Signaturbotschaft an ihren Kontext
- Netzwerkschicht (`NETWORKING/`): Nachrichtenprüfung, Verbindungs- und
  Adressgrenzen, Peer-Bewertung, Sitzungsverschlüsselung
- Knoten (`NODE/`): Schlüsselbehandlung, Kettenspeicher, Nachforderung
- Verifikation (`VERIFICATION/`) und Pod (`COMPUTE_PIPELINE/`):
  Bisektion, Kontrollsegmente, Streitverfahren, Drahtformat
- Ganzzahlpfad (`INTEGER_LLM/`), soweit ein Ergebnis dadurch
  maschinenabhängig würde
- Speicher (`STORAGE/`), Tür (`GATEWAY/`), Agentenschicht
  (`AGENT_LAYER/`) und Governance (`GOVERNANCE/`)

**Ausdrücklich außerhalb des Umfangs:**

- **Aussagen des Whitepapers**, die noch nicht gebaut sind. Das Papier
  beschreibt einen Entwurf; welche Teile Code haben, steht in der
  Komponententabelle des README.
- **Die GPU-Rückenden.** `backends/cuda.rs` und `backends/rocm.rs`
  reichen an die Referenzkerne weiter, statt selbst zu rechnen. Das
  steht in ihren Modulköpfen, und ein Konformitätslauf mit `cuda` wird
  aus genau diesem Grund abgelehnt.
- **Bekannte Grenzen**, siehe den nächsten Abschnitt. Sie stehen schon
  geschrieben und brauchen keine Meldung.
- Ergebnisse automatischer Prüfwerkzeuge ohne einen Weg, auf dem sich
  die Schwäche auslösen lässt.
- Angriffe, die physischen Zugriff auf die Maschine eines Teilnehmers
  voraussetzen, und Social Engineering gegen Beteiligte.

## Bekannte Grenzen

Diese sind aufgeschrieben, gemessen und **keine Meldung wert**. Der
vollständige Stand steht in
[`SIMULATION/Sicherheitsaudit.md`](SIMULATION/Sicherheitsaudit.md), nach
Angriffsklassen geordnet.

⚑ **Und wer prüfen will statt zu melden**, findet in
[`README/Auditzuschnitt.md`](README/Auditzuschnitt.md) den Zuschnitt
eines externen Reviews: was das System behauptet, worauf die Behauptung
ruht, wo nach Schadenshebel anzufangen ist, und was bereits bekannt ist.

Die drei wichtigsten bekannten Grenzen:

- **Die Kryptografie hat nie jemand von außen geprüft.** Dass die
  Primitiven gegen ihre Testvektoren stimmen, ist geprüft; dass ihre
  Verwendung trägt, ist Eigenbeurteilung. Ein externes Review steht vor
  dem Mainnet aus.
- **Bitgleichheit über verschiedene Rechnerarchitekturen ist begründet,
  nicht gemessen.** Sie folgt daraus, dass Ganzzahladdition assoziativ
  ist, und ist bislang auf einer Architektur nachgewiesen.
- **Ein Angreifer, der mehr als ein Drittel des Stimmgewichts hält,
  bricht die Sicherheitsannahme des Verfahrens.** Das ist keine Lücke,
  sondern die Voraussetzung, unter der ein byzantinisch fehlertolerantes
  Verfahren überhaupt etwas zusagt.

## Was Sie erwarten können

Dieses Projekt wird von einer sehr kleinen Zahl von Menschen betreut.
Die Fristen unten sind deshalb bewusst großzügig bemessen: Eine Zusage,
die nicht gehalten wird, ist schlechter als eine ehrliche.

| Schritt | Frist |
|---|---|
| Eingangsbestätigung | 7 Tage |
| Erste Einschätzung, mit Schweregrad und geplantem Vorgehen | 30 Tage |
| Rückmeldung zum Stand, danach wiederkehrend | alle 30 Tage |

**Es gibt kein Kopfgeldprogramm und keine Vergütung.** Sollte sich das
ändern, steht es hier und nicht anderswo.

Wer eine Schwachstelle meldet, wird auf Wunsch im Changelog der
betroffenen Komponente genannt. Wer anonym bleiben möchte, sagt das im
Bericht.

## Offenlegung

Wir bitten um abgestimmte Offenlegung: keine Veröffentlichung, bis eine
Korrektur vorliegt oder **90 Tage** seit der Eingangsbestätigung
vergangen sind, je nachdem, was zuerst eintritt. Wird die Frist ohne
Korrektur erreicht, steht es Ihnen frei zu veröffentlichen; wir werden
dem nicht widersprechen.

Ist eine Schwachstelle bereits öffentlich bekannt oder wird sie
ausgenutzt, entfällt die Frist. Sagen Sie das im Bericht.

## Was in einen guten Bericht gehört

Nicht als Pflicht, sondern weil es die Bearbeitung um Tage verkürzt:

- Welcher Pfad betroffen ist, und ab welcher Version
- Ein Weg, den Zustand herzustellen, gern als Test oder als kurzes
  Programm
- Was ein Angreifer davon hat, und was er dafür können muss
- Ob die Schwäche das Drahtformat oder eine Konsensregel berührt. Das
  entscheidet, ob die Korrektur additiv sein kann

---

# Security Policy

**Status:** 2026-09-02

## Where to report

**Not as a public issue and not as a pull request.** Both are readable
by anyone, and for a flaw in the consensus or cryptography path,
publication is itself the damage.

Use **GitHub private vulnerability reporting**: the `Security` tab of
this repository, then `Report a vulnerability`. The report is visible
only to the maintainers, and the conversation continues in the same
thread.

If that route is not available to you, open a public issue **without
technical detail** that only asks for a private channel. Do not include
a reproduction, a file name, or a line number.

## What this project is today

**There is no mainnet, no genesis block, and nothing transferable.**
What runs today is a dry run: the state is disposable, the MYL in it are
play money, and the seed value of the trial chain says so in plain text.

Two things follow, and both matter to a reporter:

1. **No money is at stake today.** A flaw in this repository costs
   nobody anything while no network is running.
2. **That is exactly why a report is worth most now.** Changing a
   commitment construction costs a handful of test vectors before the
   genesis block and a chain migration afterwards. Reporting today
   prevents a migration, not a loss.

## Scope

**In scope** is everything the protocol rests on: consensus and ledger
(`CONSENSUS/`), cryptographic usage (`SHARED_TYPES/`), the network layer
(`NETWORKING/`), the node (`NODE/`), verification
(`VERIFICATION/`) and the pod (`COMPUTE_PIPELINE/`), the integer path
(`INTEGER_LLM/`) where a result could become machine dependent, and
`STORAGE/`, `GATEWAY/`, `AGENT_LAYER/`, `GOVERNANCE/`.

**Explicitly out of scope:**

- **Claims of the whitepaper that are not built yet.** The paper
  describes a design; the component table in the README says which parts
  have code.
- **The GPU backends.** `backends/cuda.rs` and `backends/rocm.rs`
  delegate to the reference kernels instead of computing themselves.
  Their module headers say so, and a conformance run with `cuda` is
  refused for that reason.
- **Known limitations**, see below. They are already written down.
- Tool output without a path on which the weakness can be triggered.
- Attacks requiring physical access to a participant machine, and social
  engineering against the people involved.

## Known limitations

These are written down, measured, and **not worth a report**. The full
picture is in
[`SIMULATION/Sicherheitsaudit.md`](SIMULATION/Sicherheitsaudit.md)
(German), ordered by attack class. Anyone who would rather review than
report will find the scope of an external review in
[`README/Auditzuschnitt.md`](README/Auditzuschnitt.md) (German): what the
system claims, what the claim rests on, where to start by damage
leverage, and what is already known.

The three most important known limits:

- **The cryptography has never been reviewed externally.** That the
  primitives match their test vectors is verified; that their usage
  holds is self assessment. An external review is due before mainnet.
- **Bit identity across processor architectures is argued, not
  measured.** It follows from integer addition being associative and has
  so far been demonstrated on one architecture.
- **An attacker holding more than one third of the voting weight breaks
  the security assumption.** That is not a gap but the precondition
  under which a byzantine fault tolerant protocol promises anything at
  all.

## What you can expect

This project is maintained by a very small number of people, so the
deadlines below are deliberately generous: a promise that is not kept is
worse than an honest one.

| Step | Deadline |
|---|---|
| Acknowledgement of receipt | 7 days |
| First assessment, with severity and intended course | 30 days |
| Status update, recurring thereafter | every 30 days |

**There is no bounty program and no compensation.** Should that change,
it will say so here and nowhere else.

Reporters are credited in the changelog of the affected component on
request. If you prefer to stay anonymous, say so in the report.

## Disclosure

We ask for coordinated disclosure: no publication until a fix exists or
**90 days** have passed since acknowledgement, whichever comes first. If
the deadline is reached without a fix, you are free to publish, and we
will not object.

If a flaw is already public or being exploited, the deadline does not
apply. Say so in the report.
