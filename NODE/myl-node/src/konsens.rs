//! Eine BFT-Runde, wie ein Knoten sie fährt.
//!
//! # Was hier hinzukommt und was schon da war
//!
//! `myl_consensus::bft::BftState` ist ein **Zustandsautomat**: Man
//! reicht ihm Nachrichten, er prüft und zählt. Er erzeugt nichts, weil
//! er nichts erzeugen kann: Ihm fehlt der geheime Schlüssel.
//!
//! Was fehlte, war die andere Hälfte: **wann ein Knoten selbst etwas
//! sagen muss**, und mit welcher Signatur. Genau das ist [`Konsensrunde`].
//! Sie hält den Automaten, den Schlüssel und die eine Regel dazwischen:
//!
//! | Beobachtung | Antwort |
//! |---|---|
//! | Ich bin Leader dieser Runde | Propose |
//! | Ein gültiger Propose liegt vor | Vote, genau einmal |
//! | Das Vote-Quorum steht | Commit, genau einmal |
//! | Das Commit-Quorum steht | fertig |
//!
//! # ⚑ Die eigene Nachricht muss in den eigenen Automaten
//!
//! Gossipsub liefert einem Knoten seine **eigenen** Veröffentlichungen
//! nicht zurück. Wer nur veröffentlicht und auf das Echo wartet, zählt
//! seine eigene Stimme nie mit und hängt bei `n-1` Stimmen fest, wenn
//! genau seine zum Quorum fehlt. Bei fünf Validatoren mit ungleichem
//! Gewicht ist das kein Randfall: Wessen Gewicht fehlt, entscheidet.
//!
//! Deshalb legt [`Konsensrunde`] jede erzeugte Nachricht **zuerst dem
//! eigenen Automaten vor** und gibt sie erst dann nach draußen. Der Test
//! `die_eigene_stimme_zaehlt_im_eigenen_automaten` hält das fest.
//!
//! # Was diese Fassung noch nicht kann
//!
//! **Kein Rundenwechsel.** Fällt der Leader aus, hängt die Runde.
//! `myl_consensus::round_change` hat die Sperrregel und das
//! Polka-Zertifikat fertig; sie hier anzuschließen ist der nächste
//! Schritt und braucht eine Uhr, also einen Timeout, also eine
//! Entscheidung über GST. Diese Fassung fährt den Pfad, auf dem alle
//! ehrlich sind und niemand ausfällt.
//!
//! **Kein Blockinhalt.** Der Propose trägt einen Block-*Hash*, nicht den
//! Block. Was dieser Hash bezeichnet, entscheidet die Kette, und deren
//! Persistenz ist ein eigener offener Punkt. Bis dahin ist der Hash ein
//! Platzhalter, den alle Knoten übereinstimmend erzeugen können, und die
//! Runde prüft, was sie prüfen kann: dass alle **denselben** commiten.

use myl_consensus::bft::{
    BftError, BftState, Commit, Konsensnachricht, Propose, Round, RoundStatus, Vote,
};
use myl_consensus::select_leader;
use myl_consensus::signing::{commit_message, propose_message, vote_message};
use myl_types::hash::Hash;
use myl_types::ids::MinerId;

use crate::genesis::Genesis;
use crate::schluessel::{Konsensschluessel, SchluesselFehler};

/// Was beim Fahren einer Runde schiefgehen kann.
#[derive(Debug)]
pub enum KonsensFehler {
    /// Der eigene Schlüssel steht nicht in der Genesis-Datei.
    ///
    /// Ein Knoten, der nicht im Validator-Satz steht, kann zuhören, aber
    /// nicht mitstimmen. Das ist kein Fehler des Netzes, sondern eine
    /// Frage an die Konfiguration.
    NichtStimmberechtigt { kennung: MinerId },
    /// Der Automat ließ sich nicht aufsetzen.
    Bft(BftError),
    /// Das Signieren schlug fehl.
    Schluessel(SchluesselFehler),
}

impl std::fmt::Display for KonsensFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NichtStimmberechtigt { kennung } => write!(
                f,
                "{kennung:?} steht nicht im Validator-Satz der Genesis-Datei \
                 und kann deshalb nicht mitstimmen"
            ),
            Self::Bft(e) => write!(f, "BFT: {e}"),
            Self::Schluessel(e) => write!(f, "Schlüssel: {e}"),
        }
    }
}

impl std::error::Error for KonsensFehler {}

impl From<BftError> for KonsensFehler {
    fn from(e: BftError) -> Self {
        Self::Bft(e)
    }
}

impl From<SchluesselFehler> for KonsensFehler {
    fn from(e: SchluesselFehler) -> Self {
        Self::Schluessel(e)
    }
}

/// Das Urteil über eine eingegangene Nachricht.
///
/// Getrennt vom Rückgabewert der Folgenachrichten, weil das
/// Betriebsprotokoll den **Grund** braucht: „falsche Runde" ist im
/// Normalbetrieb häufig und harmlos, „ungültige Signatur" nie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Urteil {
    /// Angenommen und gezählt.
    Angenommen,
    /// Vom Automaten abgelehnt, mit Grund.
    Abgelehnt(BftError),
    /// Die Bytes ließen sich nicht als Konsensnachricht lesen.
    Unlesbar,
}

impl Urteil {
    pub fn als_text(&self) -> &'static str {
        match self {
            Self::Angenommen => "angenommen",
            Self::Abgelehnt(BftError::WrongRound { .. }) => "falsche-runde",
            Self::Abgelehnt(BftError::DuplicateMessage) => "doppelt",
            Self::Abgelehnt(BftError::WrongLeader) => "falscher-leader",
            Self::Abgelehnt(BftError::UnknownBlock) => "unbekannter-block",
            Self::Abgelehnt(BftError::InvalidSignature) => "signatur-falsch",
            Self::Abgelehnt(BftError::NotInCommittee) => "nicht-stimmberechtigt",
            Self::Abgelehnt(BftError::EmptyCommittee) => "leeres-komitee",
            Self::Unlesbar => "unlesbar",
        }
    }

    pub fn ist_angenommen(&self) -> bool {
        matches!(self, Self::Angenommen)
    }

    /// Ist das ein Urteil, das im Normalbetrieb vorkommt?
    ///
    /// Doppelte Nachrichten und falsche Runden sind bei Gossip die
    /// Regel, nicht die Ausnahme: Dieselbe Stimme kommt über mehrere
    /// Wege an. **Ein Protokoll, das jede davon als Auffälligkeit
    /// meldet, verdeckt die echten.**
    pub fn ist_harmlos(&self) -> bool {
        matches!(
            self,
            Self::Angenommen
                | Self::Abgelehnt(BftError::DuplicateMessage)
                | Self::Abgelehnt(BftError::WrongRound { .. })
        )
    }
}

/// Eine laufende BFT-Runde aus Sicht eines Knotens.
#[derive(Debug)]
pub struct Konsensrunde {
    schluessel: Konsensschluessel,
    zustand: BftState,
    ich: MinerId,
    gestimmt: bool,
    commitet: bool,
}

impl Konsensrunde {
    /// Beginnt eine Runde.
    ///
    /// `vorschlag` ist der Block-Hash, den dieser Knoten vorschlagen
    /// würde, **falls** er Leader dieser Runde ist. Ist er es nicht,
    /// bleibt der Wert ungenutzt.
    ///
    /// Gibt die Runde zurück und die Nachrichten, die sofort hinaus
    /// müssen: bei einem Leader Propose und die eigene Vote, sonst
    /// nichts.
    pub fn beginnen(
        genesis: &Genesis,
        schluessel: Konsensschluessel,
        runde: Round,
        vorschlag: Hash,
    ) -> Result<(Self, Vec<Konsensnachricht>), KonsensFehler> {
        let ich = schluessel.kennung();
        let menge = genesis.stimmberechtigte();
        if !menge.contains(&ich) {
            return Err(KonsensFehler::NichtStimmberechtigt { kennung: ich });
        }
        // Die Producer-Liste ist die kanonische Kennungsfolge der
        // Genesis-Datei. Sie hängt an den Schlüsseln, also rechnet jeder
        // Knoten dieselbe, und damit denselben Leader.
        let producer = genesis.kennungen();
        let leader = select_leader(runde, &producer).ok_or(BftError::EmptyCommittee)?;
        let zustand = BftState::new(runde, leader, menge)?;

        let mut runde = Self {
            schluessel,
            zustand,
            ich,
            gestimmt: false,
            commitet: false,
        };

        let mut raus = Vec::new();
        if leader == ich {
            let propose = Propose {
                round: runde.zustand.round,
                block_hash: vorschlag,
                leader: ich,
                signature: runde
                    .schluessel
                    .signiere(&propose_message(runde.zustand.round, &vorschlag))?,
            };
            // Erst dem eigenen Automaten vorlegen: Gossipsub schickt uns
            // die eigene Nachricht nicht zurück (siehe Modulkopf).
            runde.zustand.receive_propose(&propose)?;
            raus.push(Konsensnachricht::Propose(propose));
        }
        raus.extend(runde.folgenachrichten()?);
        Ok((runde, raus))
    }

    /// Verarbeitet rohe Bytes von der Leitung.
    pub fn empfange_bytes(
        &mut self,
        daten: &[u8],
    ) -> (Urteil, Vec<Konsensnachricht>) {
        match borsh::from_slice::<Konsensnachricht>(daten) {
            Ok(n) => self.empfange(&n),
            Err(_) => (Urteil::Unlesbar, Vec::new()),
        }
    }

    /// Verarbeitet eine Nachricht und gibt zurück, was daraufhin hinaus
    /// muss.
    pub fn empfange(&mut self, n: &Konsensnachricht) -> (Urteil, Vec<Konsensnachricht>) {
        let ergebnis = match n {
            Konsensnachricht::Propose(p) => self.zustand.receive_propose(p),
            Konsensnachricht::Vote(v) => self.zustand.receive_vote(v),
            Konsensnachricht::Commit(c) => self.zustand.receive_commit(c),
        };
        match ergebnis {
            Ok(()) => {
                let raus = self.folgenachrichten().unwrap_or_default();
                (Urteil::Angenommen, raus)
            }
            Err(e) => (Urteil::Abgelehnt(e), Vec::new()),
        }
    }

    /// Was aus dem jetzigen Zustand folgt.
    ///
    /// Zwei Schritte in fester Reihenfolge, und mehr braucht es nicht:
    /// Stimmen kann eine Commit-Möglichkeit eröffnen, Commiten eröffnet
    /// nichts weiter.
    fn folgenachrichten(&mut self) -> Result<Vec<Konsensnachricht>, KonsensFehler> {
        let mut raus = Vec::new();
        let runde = self.zustand.round;

        if !self.gestimmt {
            if let Some(hash) = self.zustand.proposed_block {
                let vote = Vote {
                    round: runde,
                    block_hash: hash,
                    voter: self.ich,
                    signature: self.schluessel.signiere(&vote_message(runde, &hash))?,
                };
                self.zustand.receive_vote(&vote)?;
                self.gestimmt = true;
                raus.push(Konsensnachricht::Vote(vote));
            }
        }

        let quorum_steht = matches!(
            self.zustand.status,
            RoundStatus::CollectingCommits | RoundStatus::Committed
        );
        if quorum_steht && !self.commitet {
            if let Some(hash) = self.zustand.proposed_block {
                let commit = Commit {
                    round: runde,
                    block_hash: hash,
                    committer: self.ich,
                    signature: self.schluessel.signiere(&commit_message(runde, &hash))?,
                };
                self.zustand.receive_commit(&commit)?;
                self.commitet = true;
                raus.push(Konsensnachricht::Commit(commit));
            }
        }

        Ok(raus)
    }

    /// Ist die Runde abgeschlossen?
    pub fn ist_commitet(&self) -> bool {
        self.zustand.is_committed()
    }

    /// Der commitete Block, falls die Runde durch ist.
    pub fn commiteter_block(&self) -> Option<Hash> {
        self.zustand.committed_block()
    }

    /// Die eigene Kennung.
    pub fn ich(&self) -> MinerId {
        self.ich
    }

    /// Der Leader dieser Runde.
    pub fn leader(&self) -> MinerId {
        self.zustand.leader
    }

    /// Die Rundennummer.
    pub fn runde(&self) -> Round {
        self.zustand.round
    }

    /// Der Zustand, für das Betriebsprotokoll.
    pub fn status(&self) -> RoundStatus {
        self.zustand.status
    }

    /// Eingegangenes Stimmgewicht, Commit-Gewicht und Schwelle.
    ///
    /// Für das Betriebsprotokoll. **Gewicht, nicht Köpfe:** Ein
    /// Protokoll, das „3 von 5 Stimmen" meldet, verdeckt genau den
    /// Unterschied, für den die Genesis-Verteilung gebaut wurde.
    pub fn gewichte(&self) -> (u64, u64, u64) {
        (
            self.zustand.vote_weight(),
            self.zustand.commit_weight(),
            self.zustand.threshold(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genesis::Genesis;
    use std::collections::VecDeque;

    /// Die Verteilung des Probenetzes: 250/230/200/120/100 MYL.
    fn probenetz() -> (Genesis, Vec<&'static str>) {
        let namen = vec!["alpha", "beta", "gamma", "delta", "epsilon"];
        let stakes = [
            250_000_000u64,
            230_000_000,
            200_000_000,
            120_000_000,
            100_000_000,
        ];
        let mut text = String::from("netz konsens-test\n");
        for (name, stake) in namen.iter().zip(stakes) {
            let k = Konsensschluessel::probe(name).expect("Schlüssel");
            text.push_str(&k.genesiszeile(stake).expect("Zeile"));
            text.push('\n');
        }
        (Genesis::aus_text(&text).expect("lesbar"), namen)
    }

    fn vorschlag() -> Hash {
        Hash::sha256(b"block der ersten runde")
    }

    /// Fährt eine Runde über alle Knoten, mit einer Nachrichtenschlange
    /// statt eines Netzes. Gibt zurück, wie viele Knoten commitet haben
    /// und welchen Block.
    fn runde_fahren(
        genesis: &Genesis,
        namen: &[&str],
        runde: Round,
    ) -> (Vec<Konsensrunde>, usize) {
        let mut knoten = Vec::new();
        // (Absenderindex, Nachricht) — der Absender bekommt sie nicht
        // zurück, genau wie bei Gossipsub.
        let mut schlange: VecDeque<(usize, Konsensnachricht)> = VecDeque::new();

        for (i, name) in namen.iter().enumerate() {
            let k = Konsensschluessel::probe(name).expect("Schlüssel");
            let (r, raus) =
                Konsensrunde::beginnen(genesis, k, runde, vorschlag()).expect("Runde");
            for n in raus {
                schlange.push_back((i, n));
            }
            knoten.push(r);
        }

        let mut schritte = 0;
        while let Some((von, n)) = schlange.pop_front() {
            schritte += 1;
            assert!(schritte < 10_000, "die Runde kam nicht zur Ruhe");
            for (i, k) in knoten.iter_mut().enumerate() {
                // Der Absender bekommt seine eigene Nachricht nicht
                // zurück, genau wie bei Gossipsub.
                if i == von {
                    continue;
                }
                let (_urteil, raus) = k.empfange(&n);
                for m in raus {
                    schlange.push_back((i, m));
                }
            }
        }

        let fertig = knoten.iter().filter(|k| k.ist_commitet()).count();
        (knoten, fertig)
    }

    #[test]
    fn eine_runde_kommt_bei_allen_fuenf_zum_abschluss() {
        let (g, namen) = probenetz();
        let (knoten, fertig) = runde_fahren(&g, &namen, 0);
        assert_eq!(fertig, 5, "nicht alle Knoten haben commitet");
        for k in &knoten {
            assert_eq!(
                k.commiteter_block(),
                Some(vorschlag()),
                "{:?} commitete einen anderen Block",
                k.ich()
            );
        }
    }

    #[test]
    fn alle_knoten_rechnen_denselben_leader() {
        // Die Producer-Liste hängt an den Schlüsseln, nicht an der
        // Reihenfolge in der Datei. Rechneten zwei Knoten verschiedene
        // Leader, verwürfe jeder den Propose des anderen.
        let (g, namen) = probenetz();
        for runde in 0..12u64 {
            let (knoten, _) = runde_fahren(&g, &namen, runde);
            let leader: Vec<MinerId> = knoten.iter().map(|k| k.leader()).collect();
            assert!(
                leader.windows(2).all(|w| w[0] == w[1]),
                "Runde {runde}: verschiedene Leader"
            );
        }
    }

    #[test]
    fn die_leaderrolle_wandert_ueber_die_runden() {
        // Bliebe sie stehen, wäre ein einzelner Ausfall das Ende.
        let (g, namen) = probenetz();
        let mut gesehen = std::collections::BTreeSet::new();
        for runde in 0..5u64 {
            let (knoten, _) = runde_fahren(&g, &namen, runde);
            gesehen.insert(knoten[0].leader());
        }
        assert_eq!(gesehen.len(), 5, "die Leaderrolle blieb stehen");
    }

    /// ⚑ **Die eigene Stimme muss im eigenen Automaten landen.**
    ///
    /// Gossipsub liefert eigene Veröffentlichungen nicht zurück. Wer nur
    /// veröffentlicht, zählt seine eigene Stimme nie mit.
    #[test]
    fn die_eigene_stimme_zaehlt_im_eigenen_automaten() {
        let (g, namen) = probenetz();
        let k = Konsensschluessel::probe(namen[0]).expect("Schlüssel");
        let (runde, raus) = Konsensrunde::beginnen(&g, k, 0, vorschlag()).expect("Runde");
        let (stimmgewicht, _, _) = runde.gewichte();
        if runde.leader() == runde.ich() {
            // Leader: Propose plus eigene Stimme gehen hinaus, und das
            // eigene Gewicht steht schon im Automaten.
            assert_eq!(raus.len(), 2);
            assert!(stimmgewicht > 0, "die eigene Stimme fehlt im Automaten");
        } else {
            assert!(raus.is_empty());
            assert_eq!(stimmgewicht, 0);
        }
    }

    /// Lässt **genau** die genannten Knoten stimmen und gibt zurück,
    /// welches Gewicht dabei zusammenkommt.
    ///
    /// Der Beobachter ist einer von ihnen; seine eigene Stimme entsteht,
    /// sobald ihm der Propose zugestellt wird. Die übrigen Stimmen
    /// werden direkt gebaut. **Unabhängig davon, wer Leader ist**, denn
    /// das hängt an den Schlüsseln und nicht an der Namensreihenfolge.
    fn stimmgewicht_von(g: &Genesis, stimmende: &[&str]) -> (u64, u64, bool) {
        let leader_name = leader_name_der_runde(g, stimmende, 0);
        let beobachter_name = stimmende
            .iter()
            .copied()
            .find(|n| *n != leader_name)
            .unwrap_or(stimmende[0]);

        let k = Konsensschluessel::probe(beobachter_name).expect("Schlüssel");
        let (mut beobachter, _) =
            Konsensrunde::beginnen(g, k, 0, vorschlag()).expect("Runde");

        // Den Propose des Leaders zustellen. Der Beobachter stimmt
        // daraufhin selbst, und seine Stimme zählt in seinem Automaten.
        if beobachter_name != leader_name {
            let lk = Konsensschluessel::probe(leader_name).expect("Leaderschlüssel");
            let h = vorschlag();
            let p = Propose {
                round: 0,
                block_hash: h,
                leader: lk.kennung(),
                signature: lk.signiere(&propose_message(0, &h)).expect("Signatur"),
            };
            assert!(beobachter.empfange(&Konsensnachricht::Propose(p)).0.ist_angenommen());
        }

        // Die Stimmen der übrigen genannten Knoten.
        for name in stimmende.iter().filter(|n| **n != beobachter_name) {
            let k = Konsensschluessel::probe(name).expect("Schlüssel");
            let h = vorschlag();
            let v = Vote {
                round: 0,
                block_hash: h,
                voter: k.kennung(),
                signature: k.signiere(&vote_message(0, &h)).expect("Signatur"),
            };
            let (urteil, _) = beobachter.empfange(&Konsensnachricht::Vote(v));
            assert!(urteil.ist_angenommen(), "{name}: {urteil:?}");
        }

        let (gewicht, _, schwelle) = beobachter.gewichte();
        (gewicht, schwelle, beobachter.ist_commitet())
    }

    /// ⚑ **Der Test, für den die Verteilung gebaut wurde.**
    ///
    /// Drei von fünf Köpfen erreichen das Quorum **nicht**, wenn es die
    /// leichtesten drei sind. Ein Automat, der Nachrichten statt Gewicht
    /// zählt (Fund A3), commitete hier trotzdem.
    #[test]
    fn drei_von_fuenf_koepfen_sind_kein_quorum() {
        let (g, _) = probenetz();
        // gamma 200 + delta 120 + epsilon 100 = 420 von 900.
        let (gewicht, schwelle, commitet) =
            stimmgewicht_von(&g, &["gamma", "delta", "epsilon"]);
        assert_eq!(gewicht, 420_000_000, "die drei leichtesten halten 420 MYL");
        assert!(
            gewicht < schwelle,
            "drei von fünf Köpfen erreichten das Quorum: {gewicht} >= {schwelle}. \
             Dann zählt hier jemand Köpfe statt Gewicht (Fund A3)"
        );
        assert!(!commitet);
    }

    /// Die Gegenprobe: **dieselbe Kopfzahl**, anderes Gewicht, anderes
    /// Urteil. Ohne sie wäre „immer ablehnen" grün.
    #[test]
    fn drei_schwere_koepfe_sind_ein_quorum() {
        let (g, _) = probenetz();
        // alpha 250 + beta 230 + gamma 200 = 680 von 900.
        let (gewicht, schwelle, _) = stimmgewicht_von(&g, &["alpha", "beta", "gamma"]);
        assert_eq!(gewicht, 680_000_000);
        assert!(
            gewicht >= schwelle,
            "680 von 900 erreichten die Schwelle nicht: {gewicht} < {schwelle}"
        );
    }

    /// ⚑ **Exakt zwei Drittel reichen nicht.**
    ///
    /// alpha 250 + beta 230 + delta 120 = 600 von 900, also genau 2/3.
    /// Die Schwelle ist `⌊2·900/3⌋ + 1`. Daran hängt BFT-Safety: Bei
    /// exakt zwei Dritteln überschneiden sich zwei Quoren nicht mehr
    /// zwingend in einem ehrlichen Gewicht. Ein verrutschtes `+ 1`
    /// fällt **nur** an dieser Verteilung auf.
    #[test]
    fn exakt_zwei_drittel_sind_kein_quorum() {
        let (g, _) = probenetz();
        let (gewicht, schwelle, commitet) =
            stimmgewicht_von(&g, &["alpha", "beta", "delta"]);
        assert_eq!(gewicht, 600_000_000, "exakt zwei Drittel von 900");
        assert_eq!(schwelle, 600_000_001);
        assert!(
            gewicht < schwelle,
            "exakt zwei Drittel erreichten das Quorum. Dann steht das `+ 1` in \
             quorum_threshold falsch, und zwei Quoren können sich verfehlen"
        );
        assert!(!commitet);
        // Und ein einziges weiteres Gewicht kippt es.
        let (mehr, schwelle2, _) =
            stimmgewicht_von(&g, &["alpha", "beta", "delta", "epsilon"]);
        assert_eq!(mehr, 700_000_000);
        assert!(mehr >= schwelle2);
    }

    fn leader_name_der_runde(g: &Genesis, namen: &[&str], runde: Round) -> &'static str {
        let leader = select_leader(runde, &g.kennungen()).expect("Leader");
        const ALLE: [&str; 5] = ["alpha", "beta", "gamma", "delta", "epsilon"];
        for name in ALLE {
            if Konsensschluessel::probe(name).unwrap().kennung() == leader {
                return name;
            }
        }
        panic!("Leader gehört zu keinem bekannten Namen: {namen:?}");
    }

    #[test]
    fn ein_aussenstehender_kann_keine_runde_beginnen() {
        // Wer nicht im Validator-Satz steht, hört zu und stimmt nicht
        // mit. Das ist eine Frage an die Konfiguration, kein Netzfehler.
        let (g, _) = probenetz();
        let fremd = Konsensschluessel::probe("zeta").expect("Schlüssel");
        assert!(matches!(
            Konsensrunde::beginnen(&g, fremd, 0, vorschlag()),
            Err(KonsensFehler::NichtStimmberechtigt { .. })
        ));
    }

    #[test]
    fn eine_stimme_eines_aussenstehenden_wird_abgewiesen() {
        let (g, namen) = probenetz();
        let leader_name = leader_name_der_runde(&g, &namen, 0);
        let k = Konsensschluessel::probe(leader_name).expect("Schlüssel");
        let (mut r, _) = Konsensrunde::beginnen(&g, k, 0, vorschlag()).expect("Runde");

        let fremd = Konsensschluessel::probe("zeta").expect("Schlüssel");
        let h = vorschlag();
        let vote = Vote {
            round: 0,
            block_hash: h,
            voter: fremd.kennung(),
            signature: fremd.signiere(&vote_message(0, &h)).expect("Signatur"),
        };
        let (urteil, raus) = r.empfange(&Konsensnachricht::Vote(vote));
        assert_eq!(urteil, Urteil::Abgelehnt(BftError::NotInCommittee));
        assert_eq!(urteil.als_text(), "nicht-stimmberechtigt");
        assert!(raus.is_empty());
        assert!(!urteil.ist_harmlos(), "eine fremde Stimme ist nicht harmlos");
    }

    #[test]
    fn eine_gefaelschte_signatur_wird_abgewiesen() {
        let (g, namen) = probenetz();
        let leader_name = leader_name_der_runde(&g, &namen, 0);
        let k = Konsensschluessel::probe(leader_name).expect("Schlüssel");
        let (mut r, _) = Konsensrunde::beginnen(&g, k, 0, vorschlag()).expect("Runde");

        // Der Beobachter ist Leader und hat schon für sich gestimmt. Die
        // Fälschung muss deshalb im Namen eines **anderen** kommen,
        // sonst greift die Duplikatprüfung vor der Signaturprüfung, und
        // der Test bewiese nur die Reihenfolge der Prüfungen.
        let mut fremde = namen.iter().filter(|n| **n != leader_name);
        let opfer = Konsensschluessel::probe(fremde.next().expect("ein anderer")).unwrap();
        let taeter = Konsensschluessel::probe(fremde.next().expect("noch einer")).unwrap();

        let h = vorschlag();
        let vote = Vote {
            round: 0,
            block_hash: h,
            voter: opfer.kennung(),
            signature: taeter.signiere(&vote_message(0, &h)).expect("Signatur"),
        };
        let (urteil, folge) = r.empfange(&Konsensnachricht::Vote(vote));
        assert_eq!(urteil, Urteil::Abgelehnt(BftError::InvalidSignature));
        assert_eq!(urteil.als_text(), "signatur-falsch");
        assert!(!urteil.ist_harmlos());
        assert!(folge.is_empty());
    }

    /// Gegenprobe zum vorigen: Dieselbe Stimme mit **eigener** Signatur
    /// kommt durch. Ohne diese Gegenprobe wäre auch ein Automat grün,
    /// der jede Stimme ablehnt.
    #[test]
    fn dieselbe_stimme_mit_eigener_signatur_kommt_durch() {
        let (g, namen) = probenetz();
        let leader_name = leader_name_der_runde(&g, &namen, 0);
        let k = Konsensschluessel::probe(leader_name).expect("Schlüssel");
        let (mut r, _) = Konsensrunde::beginnen(&g, k, 0, vorschlag()).expect("Runde");

        let opfer_name = namen.iter().find(|n| **n != leader_name).expect("anderer");
        let opfer = Konsensschluessel::probe(opfer_name).unwrap();
        let h = vorschlag();
        let vote = Vote {
            round: 0,
            block_hash: h,
            voter: opfer.kennung(),
            signature: opfer.signiere(&vote_message(0, &h)).expect("Signatur"),
        };
        assert!(r.empfange(&Konsensnachricht::Vote(vote)).0.ist_angenommen());
    }

    #[test]
    fn ein_propose_von_jemandem_der_nicht_leader_ist_wird_abgewiesen() {
        let (g, namen) = probenetz();
        let leader = select_leader(0, &g.kennungen()).expect("Leader");
        let nicht_leader = namen
            .iter()
            .find(|n| Konsensschluessel::probe(n).unwrap().kennung() != leader)
            .expect("jemand ist nicht Leader");

        let k = Konsensschluessel::probe(namen[0]).expect("Schlüssel");
        let (mut r, _) = Konsensrunde::beginnen(&g, k, 0, vorschlag()).expect("Runde");

        let falscher = Konsensschluessel::probe(nicht_leader).expect("Schlüssel");
        let h = Hash::sha256(b"eigenmaechtiger vorschlag");
        let p = Propose {
            round: 0,
            block_hash: h,
            leader: falscher.kennung(),
            signature: falscher.signiere(&propose_message(0, &h)).expect("Signatur"),
        };
        let (urteil, _) = r.empfange(&Konsensnachricht::Propose(p));
        assert_eq!(urteil, Urteil::Abgelehnt(BftError::WrongLeader));
    }

    #[test]
    fn doppelte_nachrichten_gelten_als_harmlos() {
        // Bei Gossip ist dieselbe Stimme über mehrere Wege die Regel.
        // Ein Protokoll, das jede davon meldet, verdeckt die echten.
        let (g, namen) = probenetz();
        let leader_name = leader_name_der_runde(&g, &namen, 0);
        let k = Konsensschluessel::probe(leader_name).expect("Schlüssel");
        let (_, raus) = Konsensrunde::beginnen(&g, k, 0, vorschlag()).expect("Runde");

        let anderer = namen.iter().find(|n| **n != leader_name).expect("anderer");
        let k2 = Konsensschluessel::probe(anderer).expect("Schlüssel");
        let (mut r2, _) = Konsensrunde::beginnen(&g, k2, 0, vorschlag()).expect("Runde");

        let propose = raus.first().expect("ein Propose").clone();
        assert!(r2.empfange(&propose).0.ist_angenommen());
        let (zweites, folge) = r2.empfange(&propose);
        assert_eq!(zweites, Urteil::Abgelehnt(BftError::DuplicateMessage));
        assert_eq!(zweites.als_text(), "doppelt");
        assert!(zweites.ist_harmlos());
        assert!(folge.is_empty(), "ein Duplikat löste eine zweite Stimme aus");
    }

    #[test]
    fn eine_nachricht_der_falschen_runde_gilt_als_harmlos() {
        let (g, namen) = probenetz();
        let k = Konsensschluessel::probe(namen[0]).expect("Schlüssel");
        let (mut r, _) = Konsensrunde::beginnen(&g, k, 5, vorschlag()).expect("Runde");
        let alpha = Konsensschluessel::probe("alpha").expect("a");
        let h = vorschlag();
        let vote = Vote {
            round: 4,
            block_hash: h,
            voter: alpha.kennung(),
            signature: alpha.signiere(&vote_message(4, &h)).expect("Signatur"),
        };
        let (urteil, _) = r.empfange(&Konsensnachricht::Vote(vote));
        assert!(matches!(
            urteil,
            Urteil::Abgelehnt(BftError::WrongRound { expected: 5, got: 4 })
        ));
        assert!(urteil.ist_harmlos());
    }

    #[test]
    fn zufallsbytes_gelten_als_unlesbar_und_nicht_als_angriff() {
        let (g, namen) = probenetz();
        let k = Konsensschluessel::probe(namen[0]).expect("Schlüssel");
        let (mut r, _) = Konsensrunde::beginnen(&g, k, 0, vorschlag()).expect("Runde");
        let (urteil, raus) = r.empfange_bytes(&[0xAB; 7]);
        assert_eq!(urteil, Urteil::Unlesbar);
        assert!(raus.is_empty());
    }

    #[test]
    fn die_bytes_von_der_leitung_ergeben_dieselbe_runde() {
        // Der Weg über borsh muss zum selben Ergebnis führen wie der
        // direkte Aufruf, sonst prüft der Test etwas anderes als der
        // Betrieb.
        let (g, namen) = probenetz();
        let leader_name = leader_name_der_runde(&g, &namen, 0);
        let k = Konsensschluessel::probe(leader_name).expect("Schlüssel");
        let (_, raus) = Konsensrunde::beginnen(&g, k, 0, vorschlag()).expect("Runde");

        let anderer = namen.iter().find(|n| **n != leader_name).expect("anderer");
        let k2 = Konsensschluessel::probe(anderer).expect("Schlüssel");
        let (mut r2, _) = Konsensrunde::beginnen(&g, k2, 0, vorschlag()).expect("Runde");

        let bytes = borsh::to_vec(&raus[0]).expect("serialisieren");
        let (urteil, folge) = r2.empfange_bytes(&bytes);
        assert!(urteil.ist_angenommen());
        assert_eq!(folge.len(), 1, "auf einen Propose folgt genau eine Stimme");
        assert!(matches!(folge[0], Konsensnachricht::Vote(_)));
    }
}
