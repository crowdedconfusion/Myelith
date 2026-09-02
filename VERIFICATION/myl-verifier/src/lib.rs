//! `myl-verifier` — Verifikations-Subsystem (Whitepaper Kap. 6.4–6.9, Anhang A.4).
//!
//! Implementiert die drei Verifikationsstufen:
//! - **Stufe 1 (Redundanz):** Commitment-Hash-Vergleich zweier Pods
//! - **Stufe 2 (Stichproben):** Bisektions-Spiel bei Abweichung
//! - **Stufe 3 (zkML-Anker):** Zukunftspfad (noch nicht implementiert)
//!
//! sowie das Kontrollsegment-Verfahren (Kap. 6.7).
//!
//! **Abhängigkeiten:**
//! - INTEGER_LLM: Determinismus-Eigenschaft (Kap. 6.2) — Entscheidungspunkt 12.21 ✅
//! - CONSENSUS: BFT-Blockproduktion (Phase 3) für On-Chain-Schiedsrunde
//! - NETWORKING: Verschlüsselte Aktivierungs-Streams (Phase 3) für DA-Fragmente
//!
//! **Konsens-Regeln:** Keine Gleitkomma im Pfad, alle Vergleiche sind binär
//! (gleich/ungleich), keine Schwellenwerte.

#![deny(unsafe_code)]

pub mod redundancy;
pub mod delivery;
pub mod checker;
pub mod challenge;
pub mod bisection;
// ⚑ Hier standen bis zum 2026-09-02 drei Module: `kontrollsegmente`
// (Mechanik und Vorrat), `unterscheider` (die Spur der Mechanik, Fund
// 58) und `unterscheidbarkeit` (das Messgeraet fuer Kap. 6.7
// Anforderung 1). Zusammen rund 1 700 Zeilen.
//
// **Sie sind mit ihrem Gegenstand entfallen** (Entscheidung A1 des
// Projektinhabers), und der Grund ist kein Mangel an ihnen, sondern
// einer an der Rolle, die sie voraussetzten:
//
// Kap. 6.7 legt die Einschleusung den Gateways zu. **Ein Gateway hat
// nichts zu verlieren, und ein selbstbetriebenes speist gar nichts
// ein.** Truebit schliesst den Auftraggeber aus demselben Grund
// ausdruecklich vom Wissen aus, Proof of Sampling ersetzt ihn durch ein
// BFT-Zufallsverfahren. Myelith hat dieses Zufallsverfahren seit Punkt
// 44 im Block, selbstbeschreibend und geprueft.
//
// ⚑ **Und beim Nebeneinanderlegen fiel auf, dass der Mechanismus
// zweimal dastand:** Kontrollsegmente und Stufe 2 der Verifikation
// haben dieselbe Form, denselben Zweck und decken denselben Angriff.
// Nur einer von beiden braucht einen Einspeiser, einen endlichen Vorrat
// und eine Ununterscheidbarkeit, die er nachweislich nicht herstellen
// kann. `gamma` ist in die Stichprobenrate aufgegangen.
//
// **Was dabei nicht verlorengeht, gehoert hierher, weil es sonst
// nirgends steht:**
//
//  - **Fund 58 als Erkenntnis:** Ein endlicher Vorrat in einem
//    unbegrenzten Auftragsstrom wiederholt Ids, und echte Arbeit tut
//    das nie. Bei gamma = 2 % ueber 100 000 Auftraege erkannte ein
//    Miner mit Gedaechtnis bei einem Vorrat von 64 rund 96,8 % der
//    Kontrollen, **ohne einen einzigen Fehlalarm**. Wer je wieder einen
//    Vorrat einfuehrt, braucht diese Schranke zurueck.
//  - **Der Grund, warum Anforderung 1 unerfuellbar war:** Unauffaellig
//    zu sein ist eine Eigenschaft der Daten und keine des Codes. Eine
//    gezogene **echte** Arbeitseinheit ist von echter Arbeit nicht zu
//    unterscheiden, weil sie es ist; damit entfaellt die Anforderung,
//    statt erfuellt zu werden.
//  - ⚑ **Fund 138, und er hat die Zusammenlegung korrigiert:** Ein
//    Kontrollsegment wird gegen eine hinterlegte Antwort geprueft, eine
//    Stichprobe von einem **Checker**, und der kann dem Angreifer
//    gehoeren. Die beiden Stufen sind deshalb **nicht** austauschbar.
//    Gleichwertig ist `gamma/(1-c) + p*(1-gamma)`; bei c = 1/3 sind das
//    4,96 %, aufgerundet auf 5 %. Der naive Ansatz `p + gamma - p*gamma`
//    haette 20 % zu wenig geprueft. Gerechnet in `security_sim.py`.
pub mod slash;
pub mod adjudicate;

pub use redundancy::{
    compare_commitments, CompareResult, RedundancyError, VerificationMode,
};
pub use delivery::{
    decide_delivery, DeliveryDecision, DeliveryError, first_divergence, should_deliver_confirmed,
};
pub mod anzeige;
pub mod nachrechner;
pub use anzeige::{anzeige_erheben, beschuldigter, Anzeigefehler, Zustaendigkeit};
pub use nachrechner::ModellAuditor;
pub use checker::{
    check_segment, pruefe_spurantwort, CheckError, CheckResult, SegmentAuditor, Spurfehler,
};
pub use challenge::{
    create_challenge, find_first_divergence, challenge_hash, Challenge, ChallengeError,
};
pub use bisection::{ANTWORTFRIST_EPOCHEN, 
    BisectionSession, BisectionRequest, BisectionResponse, BisectionResult, BisectionError,
};
pub use slash::{
    create_slash_decision, Anfechtungsbeleg, Nachweis, Schuldbeleg, SlashDecision, SlashError,
    SlashReason, VerdictOutcome,
};
pub use adjudicate::{
    AdjudicationRequest, AdjudicationResult, AdjudicationError,
    ShardExecutor, adjudicate, zusicherung_ist_belegt,
};
