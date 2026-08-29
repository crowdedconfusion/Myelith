//! Eine BFT-Runde, wie ein Knoten sie fährt, samt Rundenwechsel.
//!
//! # Was hier hinzukommt und was schon da war
//!
//! `myl_consensus::round_change::RoundDriver` ist ein **Zustandsautomat
//! mit Uhr**: Man reicht ihm Nachrichten und die Zeit, er prüft, zählt,
//! sperrt und wechselt die Runde. Er erzeugt nichts, weil er nichts
//! erzeugen kann: Ihm fehlt der geheime Schlüssel.
//!
//! Was fehlte, war die andere Hälfte: **wann ein Knoten selbst etwas
//! sagen muss**, und mit welcher Signatur. Genau das ist [`Konsensrunde`]:
//!
//! | Beobachtung | Antwort |
//! |---|---|
//! | Ich bin Leader dieser Runde | Propose, mit Zertifikat falls gesperrt |
//! | Ein gültiger Propose liegt vor | Vote, genau einmal je Runde |
//! | Das Vote-Quorum steht | Commit, genau einmal je Runde |
//! | Die Frist ist abgelaufen | nächste Runde, neuer Leader |
//! | Das Commit-Quorum steht | fertig, und der Beleg liegt bereit |
//! | Jemand sendet aus einer fremden Runde | ihm den Beleg schicken |
//! | Ein gültiger Beleg trifft ein | die Entscheidung übernehmen |
//!
//! # Der Timeout: feste Basis mit Zuwachs, nicht aus Latenzen abgeleitet
//!
//! Entschieden am 2026-08-26. Der wirksame Timeout ist
//! `basis + runde × delta` (`TimeoutConfig`), und der Zuwachs ist der
//! Grund, warum das Verfahren nach GST terminiert: Er überschreitet
//! irgendwann jede reale Nachrichtenlaufzeit.
//!
//! **Die Alternative, ihn aus gemessenen Latenzen abzuleiten, klingt
//! klüger und ist die schlechtere Wahl.** Die Latenzwerte kommen aus
//! Attesten; Audit-Punkt A10 war genau der Fall, dass jemand sie
//! fälscht, A12 die Kollusion, zu der es führt. Ein Timeout, der von
//! dieser Fläche liest, gibt einem Angreifer einen Hebel auf die
//! Liveness, den er heute nicht hat. Dazu käme, dass jeder Knoten eine
//! andere Frist rechnete und eine hängende Runde nicht mehr aus dem
//! Zustand erklärbar wäre.
//!
//! ⚑ **Was an den Vorgabewerten begründet und nicht gemessen ist:**
//! `propose_ms = 1000` stammt aus „Hälfte der 2-Sekunden-Blockzeit",
//! nicht aus beobachteter Verbreitung. Auf Loopback kam der Propose in
//! unter 1 ms an. **Was er über ein echtes Weitverkehrsnetz braucht,
//! ist offen** und lässt sich hier nicht messen.
//!
//! # ⚑ Die eigene Nachricht muss in den eigenen Automaten
//!
//! Gossipsub liefert einem Knoten seine **eigenen** Veröffentlichungen
//! nicht zurück. Wer nur veröffentlicht und auf das Echo wartet, zählt
//! seine eigene Stimme nie mit und hängt bei `n-1` Stimmen fest, wenn
//! genau seine zum Quorum fehlt. Deshalb legt [`Konsensrunde`] jede
//! erzeugte Nachricht **zuerst dem eigenen Automaten vor**.
//!
//! # Warum die Stimmen hier liegen und nicht im Automaten
//!
//! `BftState` speichert Stimmen als `MinerId → Hash`, also **ohne
//! Signatur**: Zum Zählen reicht das. Ein `PolkaCertificate` braucht
//! die Signaturen, denn es ist ihr Aggregat. Der Automat kann es also
//! nicht bauen, und deshalb sammelt dieses Modul die vollständigen
//! Stimmen der laufenden Runde mit.
//!
//! # Was diese Fassung noch nicht kann
//!
//! **Kein Blockinhalt.** Der Propose trägt einen Block-*Hash*, nicht den
//! Block. Was dieser Hash bezeichnet, entscheidet die Kette, und deren
//! Persistenz ist ein eigener offener Punkt.
//!
//! **Kein Wiedereinstieg.** Eine Runde beginnt immer bei 0. Ein Knoten,
//! der später dazukommt, müsste den Stand nachladen, und das hängt an
//! derselben Persistenz.

use myl_consensus::bft::{
    BftError, Commit, Konsensnachricht, Propose, Round, RoundStatus, Vote,
};
use myl_consensus::round_change::{
    Commitzertifikat, Lock, PolkaCertificate, RoundChange, RoundDriver, RoundError, TimeoutConfig,
};
use myl_consensus::signing::{commit_message, propose_message, propose_pol_message, vote_message};
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
    /// Der Automat ließ sich nicht aufsetzen oder wechselte nicht.
    Runde(RoundError),
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
            Self::Runde(e) => write!(f, "BFT: {e}"),
            Self::Schluessel(e) => write!(f, "Schlüssel: {e}"),
        }
    }
}

impl std::error::Error for KonsensFehler {}

impl From<RoundError> for KonsensFehler {
    fn from(e: RoundError) -> Self {
        Self::Runde(e)
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
    Abgelehnt(RoundError),
    /// Die Bytes ließen sich nicht als Konsensnachricht lesen.
    Unlesbar,
}

impl Urteil {
    pub fn als_text(&self) -> &'static str {
        match self {
            Self::Angenommen => "angenommen",
            Self::Abgelehnt(RoundError::Bft(BftError::WrongRound { .. })) => "falsche-runde",
            Self::Abgelehnt(RoundError::Bft(BftError::DuplicateMessage)) => "doppelt",
            Self::Abgelehnt(RoundError::Bft(BftError::WrongLeader)) => "falscher-leader",
            Self::Abgelehnt(RoundError::Bft(BftError::UnknownBlock)) => "unbekannter-block",
            Self::Abgelehnt(RoundError::Bft(BftError::InvalidSignature)) => "signatur-falsch",
            Self::Abgelehnt(RoundError::Bft(BftError::NotInCommittee)) => "nicht-stimmberechtigt",
            Self::Abgelehnt(RoundError::Bft(BftError::EmptyCommittee)) => "leeres-komitee",
            Self::Abgelehnt(RoundError::Locked { .. }) => "gesperrt",
            Self::Abgelehnt(RoundError::CertificateBlockMismatch) => "zertifikat-anderer-block",
            Self::Abgelehnt(RoundError::CertificateRoundNotUsable { .. }) => {
                "zertifikat-runde-unbrauchbar"
            }
            // ⚑ Eigene Marke, kein Sammelposten. Das hier ist der
            // einzige Befund in dieser Liste, der nicht über den
            // Absender spricht, sondern über das Netz: Zwei Quoren für
            // zwei Blöcke. Unter „abgelehnt" gebucht wäre er unauffindbar.
            Self::Abgelehnt(RoundError::ConflictingCommit) => "gabelung",
            Self::Abgelehnt(_) => "abgelehnt",
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
    /// Wege an, und nach einem Rundenwechsel trudeln die Nachrichten der
    /// alten Runde noch ein. **Ein Protokoll, das jede davon als
    /// Auffälligkeit meldet, verdeckt die echten.**
    ///
    /// `Locked` gehört ebenfalls dazu: Ein gesperrter Validator, der
    /// einen anderen Block ablehnt, tut genau das Richtige.
    pub fn ist_harmlos(&self) -> bool {
        matches!(
            self,
            Self::Angenommen
                | Self::Abgelehnt(RoundError::Bft(BftError::DuplicateMessage))
                | Self::Abgelehnt(RoundError::Bft(BftError::WrongRound { .. }))
                | Self::Abgelehnt(RoundError::Locked { .. })
        )
    }
}

/// Eine laufende BFT-Runde aus Sicht eines Knotens.
#[derive(Debug)]
pub struct Konsensrunde {
    schluessel: Konsensschluessel,
    treiber: RoundDriver,
    ich: MinerId,
    /// Was dieser Knoten vorschlägt, wenn er Leader und nicht gesperrt ist.
    eigener_vorschlag: Hash,
    vorgeschlagen_in: Option<Round>,
    gestimmt_in: Option<Round>,
    commitet_in: Option<Round>,
    /// Vollständige Stimmen der laufenden Runde, für den Zertifikatsbau.
    ///
    /// Siehe Modulkopf: Der Automat speichert Stimmen ohne Signatur, ein
    /// Zertifikat ist aber ihr Aggregat.
    stimmen: Vec<Vote>,
    /// Das jüngste Zertifikat, das dieser Knoten gesehen oder gebaut hat.
    zertifikat: Option<PolkaCertificate>,
    /// Vollständige Commits der laufenden Runde, für den Zertifikatsbau.
    ///
    /// Aus demselben Grund wie [`Self::stimmen`]: Der Automat speichert
    /// Commits ohne Signatur, ein Zertifikat ist aber ihr Aggregat.
    commits: Vec<Commit>,
    /// Der Beleg der eigenen Entscheidung, sobald das Commit-Quorum steht.
    commitzertifikat: Option<Commitzertifikat>,
    /// Wem der Beleg schon geschickt wurde.
    ///
    /// ⚑ **Genau einmal je Gegenstelle**, sonst wird aus der Hilfe eine
    /// Schleife: Wer aus einer fremden Runde sendet, tut das mehrfach,
    /// und jede seiner Nachrichten löste sonst einen neuen Beleg aus.
    beantwortet: std::collections::BTreeSet<MinerId>,
    /// Wie oft die Runde gewechselt hat, fürs Protokoll.
    wechsel: u64,
}

impl Konsensrunde {
    /// Beginnt bei Runde 0.
    ///
    /// `vorschlag` ist der Block-Hash, den dieser Knoten vorschlagen
    /// würde, falls er Leader ist und keine Sperre hält.
    ///
    /// Gibt die Runde zurück und die Nachrichten, die sofort hinaus
    /// müssen: bei einem Leader Propose und die eigene Vote, sonst
    /// nichts.
    pub fn beginnen(
        genesis: &Genesis,
        schluessel: Konsensschluessel,
        vorschlag: Hash,
        jetzt_ms: u64,
        timeouts: TimeoutConfig,
    ) -> Result<(Self, Vec<Konsensnachricht>), KonsensFehler> {
        let ich = schluessel.kennung();
        let menge = genesis.stimmberechtigte();
        if !menge.contains(&ich) {
            return Err(KonsensFehler::NichtStimmberechtigt { kennung: ich });
        }
        // Die Producer-Liste ist die kanonische Kennungsfolge der
        // Genesis-Datei. Sie hängt an den Schlüsseln, also rechnet jeder
        // Knoten dieselbe, und damit denselben Leader je Runde.
        let treiber = RoundDriver::new(genesis.kennungen(), menge, timeouts, jetzt_ms)?;

        let mut runde = Self {
            schluessel,
            treiber,
            ich,
            eigener_vorschlag: vorschlag,
            vorgeschlagen_in: None,
            gestimmt_in: None,
            commitet_in: None,
            stimmen: Vec::new(),
            zertifikat: None,
            commits: Vec::new(),
            commitzertifikat: None,
            beantwortet: std::collections::BTreeSet::new(),
            wechsel: 0,
        };
        let raus = runde.folgenachrichten(jetzt_ms)?;
        Ok((runde, raus))
    }

    /// Verarbeitet rohe Bytes von der Leitung.
    pub fn empfange_bytes(
        &mut self,
        daten: &[u8],
        jetzt_ms: u64,
    ) -> (Urteil, Vec<Konsensnachricht>) {
        match borsh::from_slice::<Konsensnachricht>(daten) {
            Ok(n) => self.empfange(&n, jetzt_ms),
            Err(_) => (Urteil::Unlesbar, Vec::new()),
        }
    }

    /// Verarbeitet eine Nachricht und gibt zurück, was daraufhin hinaus
    /// muss.
    pub fn empfange(
        &mut self,
        n: &Konsensnachricht,
        jetzt_ms: u64,
    ) -> (Urteil, Vec<Konsensnachricht>) {
        let ergebnis = match n {
            Konsensnachricht::Propose(p) => self.treiber.receive_propose(p, None, jetzt_ms),
            Konsensnachricht::ProposeMitPolka(p, zert) => {
                let r = self.treiber.receive_propose(p, Some(zert), jetzt_ms);
                if r.is_ok() {
                    self.merke_zertifikat(zert.clone());
                }
                r
            }
            Konsensnachricht::Vote(v) => {
                let r = self.treiber.receive_vote(v, jetzt_ms);
                if r.is_ok() {
                    self.merke_stimme(v.clone());
                }
                r
            }
            Konsensnachricht::Commit(c) => {
                let r = self.treiber.receive_commit(c, jetzt_ms);
                if r.is_ok() {
                    self.merke_commit(c.clone());
                }
                r
            }
            // ⚑ Der Rückweg aus Fund 67. Ein Beleg gilt ohne Rücksicht
            // auf die eigene Runde; deshalb steht er hier neben den
            // rundengebundenen Marken und nicht unter ihnen.
            Konsensnachricht::Commitzertifikat(z) => {
                self.treiber.apply_commitzertifikat(z).map(|_| ())
            }
        };
        match ergebnis {
            Ok(()) => {
                let raus = self.folgenachrichten(jetzt_ms).unwrap_or_default();
                (Urteil::Angenommen, raus)
            }
            Err(e) => {
                let raus = self.hilf_beim_aufholen(n, &e);
                (Urteil::Abgelehnt(e), raus)
            }
        }
    }

    /// Antwortet einem Absender, der erkennbar in einer anderen Runde
    /// steht, mit dem Beleg der eigenen Entscheidung.
    ///
    /// # ⚑ Warum der Beleg nicht einfach mitgesendet wird (Fund 67)
    ///
    /// Der naheliegende Weg wäre, dass jeder Knoten sein
    /// Commit-Zertifikat nach dem Commit ins Netz legt. Das kostet bei
    /// `n` Validatoren `n` Nachrichten je Entscheidung, jede mit der
    /// Unterzeichnerliste darin, und zwar **immer**, auch wenn niemand
    /// sie braucht: Im Normalfall haben alle dieselben Commits ohnehin
    /// gesehen.
    ///
    /// Hier geht der Beleg nur dann hinaus, wenn sich jemand als außer
    /// Takt zu erkennen gibt, und das tut er von selbst: Wer in einer
    /// anderen Runde steht, sendet Nachrichten dieser Runde, und der
    /// Automat weist sie mit
    /// [`BftError::WrongRound`](myl_consensus::bft::BftError::WrongRound)
    /// ab. Diese Abweisung ist das Signal. Im Normalbetrieb kostet der
    /// Rückweg damit nichts.
    ///
    /// **Drei Bedingungen, und jede hat einen Grund:**
    ///
    /// 1. Es muss eine falsche Runde sein. Jede andere Ablehnung sagt
    ///    nichts über den Takt des Absenders.
    /// 2. Dieser Knoten muss selbst einen Beleg haben. Ohne eigene
    ///    Entscheidung ist nicht gesagt, wer von beiden zurückliegt.
    /// 3. Der Absender muss stimmberechtigt sein und wird **einmal**
    ///    bedient. Sonst löst jeder Beliebige mit erfundenen Bytes den
    ///    Versand aus, so oft er will: Die Rundenprüfung steht im
    ///    Automaten vor der Signaturprüfung, ist also billig zu
    ///    erreichen.
    fn hilf_beim_aufholen(
        &mut self,
        n: &Konsensnachricht,
        fehler: &RoundError,
    ) -> Vec<Konsensnachricht> {
        if !matches!(fehler, RoundError::Bft(BftError::WrongRound { .. })) {
            return Vec::new();
        }
        let Some(zert) = self.commitzertifikat.clone() else {
            return Vec::new();
        };
        let Some(wer) = n.absender() else {
            return Vec::new();
        };
        if !self.treiber.state().voting_set().contains(&wer) {
            return Vec::new();
        }
        if !self.beantwortet.insert(wer) {
            return Vec::new();
        }
        vec![Konsensnachricht::Commitzertifikat(zert)]
    }

    /// Der Takt: prüft die Frist und wechselt gegebenenfalls die Runde.
    ///
    /// Gibt den Rundenwechsel zurück, falls einer stattfand, und die
    /// Nachrichten, die daraufhin hinaus müssen. Der neue Leader
    /// schlägt hier vor.
    pub fn takt(&mut self, jetzt_ms: u64) -> (Option<RoundChange>, Vec<Konsensnachricht>) {
        match self.treiber.on_timeout(jetzt_ms) {
            Ok(RoundChange::Advanced { from, to, leader }) => {
                // Die Stimmen gehören zur alten Runde. Das Zertifikat
                // **nicht**: Es ist der Beleg, den der nächste Leader
                // braucht, und überlebt den Wechsel.
                self.stimmen.clear();
                // Wie die Stimmen, und aus demselben Grund. Der fertige
                // Beleg bleibt: Er bezeugt eine Entscheidung, keine Runde.
                self.commits.clear();
                self.wechsel += 1;
                let raus = self.folgenachrichten(jetzt_ms).unwrap_or_default();
                (Some(RoundChange::Advanced { from, to, leader }), raus)
            }
            Ok(_) => (None, Vec::new()),
            Err(_) => (None, Vec::new()),
        }
    }

    /// Wann die laufende Frist abläuft, in Millisekunden derselben Uhr.
    pub fn frist_ms(&self) -> u64 {
        self.treiber.deadline_ms()
    }

    /// Nimmt eine Stimme in die Sammlung auf und baut daraus ein
    /// Zertifikat, sobald das Quorum steht.
    fn merke_stimme(&mut self, v: Vote) {
        if v.round != self.treiber.round() {
            return;
        }
        self.stimmen.push(v);
        // Steht das Quorum, ist die Sammlung ein Beleg. Ihn hier zu
        // bauen und nicht erst beim Vorschlagen ist wichtig: Nach dem
        // Rundenwechsel sind die Stimmen weg, das Zertifikat bleibt.
        if self.treiber.state().vote_weight() >= self.treiber.state().threshold() {
            if let Ok(zert) = PolkaCertificate::from_votes(&self.stimmen) {
                self.merke_zertifikat(zert);
            }
        }
    }

    /// Nimmt einen Commit in die Sammlung auf und baut daraus den Beleg,
    /// sobald das Quorum steht.
    ///
    /// Spiegelbild von [`Self::merke_stimme`], mit einem Unterschied:
    /// Der Beleg wird **nicht** verworfen, wenn die Runde wechselt. Eine
    /// Entscheidung überlebt jeden Rundenwechsel, sonst wäre sie keine.
    fn merke_commit(&mut self, c: Commit) {
        if c.round != self.treiber.round() {
            return;
        }
        self.commits.push(c);
        if self.treiber.state().commit_weight() >= self.treiber.state().threshold() {
            if let Ok(zert) = Commitzertifikat::from_commits(&self.commits) {
                self.commitzertifikat = Some(zert);
            }
        }
    }

    /// Der Beleg der eigenen Entscheidung, falls dieser Knoten einen hat.
    pub fn commitzertifikat(&self) -> Option<&Commitzertifikat> {
        self.commitzertifikat.as_ref()
    }

    /// Hat dieser Knoten die Entscheidung **übernommen**, statt sie
    /// selbst gezählt zu haben?
    ///
    /// ⚑ Für das Betriebsprotokoll, und dort ist es der Unterschied
    /// zwischen „lief mit" und „musste zurückgeholt werden" (Fund 67).
    /// Beides endet in derselben Zeile `konsens_commitet`, und ohne
    /// dieses Feld wäre der zweite Fall im Nachhinein nicht mehr von
    /// dem ersten zu unterscheiden.
    ///
    /// Wer selbst commitet hat, hat es in der laufenden Runde getan: Ein
    /// commiteter Automat wechselt die Runde nicht mehr.
    pub fn durch_beleg_commitet(&self) -> bool {
        self.ist_commitet() && self.commitet_in != Some(self.runde())
    }

    fn merke_zertifikat(&mut self, zert: PolkaCertificate) {
        let besser = match &self.zertifikat {
            None => true,
            Some(alt) => zert.round > alt.round,
        };
        if besser {
            self.zertifikat = Some(zert);
        }
    }

    /// Was aus dem jetzigen Zustand folgt.
    ///
    /// Drei Schritte in fester Reihenfolge, und mehr braucht es nicht:
    /// Vorschlagen kann eine Stimme eröffnen, Stimmen einen Commit,
    /// Commiten eröffnet nichts weiter.
    fn folgenachrichten(&mut self, jetzt_ms: u64) -> Result<Vec<Konsensnachricht>, KonsensFehler> {
        let mut raus = Vec::new();
        let runde = self.treiber.round();

        // 1. Bin ich Leader und habe in dieser Runde noch nicht
        //    vorgeschlagen?
        if self.treiber.leader() == self.ich && self.vorgeschlagen_in != Some(runde) {
            let n = self.baue_vorschlag(runde)?;
            // Erst dem eigenen Automaten vorlegen: Gossipsub schickt uns
            // die eigene Nachricht nicht zurück (siehe Modulkopf).
            let selbst = match &n {
                Konsensnachricht::ProposeMitPolka(p, z) => {
                    self.treiber.receive_propose(p, Some(z), jetzt_ms)
                }
                Konsensnachricht::Propose(p) => self.treiber.receive_propose(p, None, jetzt_ms),
                _ => unreachable!("baue_vorschlag liefert nur Vorschläge"),
            };
            // Ein Leader, der seinen eigenen Vorschlag nicht annehmen
            // kann, ist selbst gesperrt und darf ihn auch nicht senden.
            if selbst.is_ok() {
                self.vorgeschlagen_in = Some(runde);
                raus.push(n);
            }
        }

        // 2. Liegt ein Vorschlag vor und habe ich noch nicht gestimmt?
        if self.gestimmt_in != Some(runde) {
            if let Some(hash) = self.treiber.state().proposed_block {
                let vote = Vote {
                    round: runde,
                    block_hash: hash,
                    voter: self.ich,
                    signature: self.schluessel.signiere(&vote_message(runde, &hash))?,
                };
                if self.treiber.receive_vote(&vote, jetzt_ms).is_ok() {
                    self.merke_stimme(vote.clone());
                    self.gestimmt_in = Some(runde);
                    raus.push(Konsensnachricht::Vote(vote));
                }
            }
        }

        // 3. Steht das Vote-Quorum und habe ich noch nicht commitet?
        let quorum_steht = matches!(
            self.treiber.status(),
            RoundStatus::CollectingCommits | RoundStatus::Committed
        );
        if quorum_steht && self.commitet_in != Some(runde) {
            if let Some(hash) = self.treiber.state().proposed_block {
                let commit = Commit {
                    round: runde,
                    block_hash: hash,
                    committer: self.ich,
                    signature: self.schluessel.signiere(&commit_message(runde, &hash))?,
                };
                if self.treiber.receive_commit(&commit, jetzt_ms).is_ok() {
                    self.merke_commit(commit.clone());
                    self.commitet_in = Some(runde);
                    raus.push(Konsensnachricht::Commit(commit));
                }
            }
        }

        Ok(raus)
    }

    /// Baut den Vorschlag dieser Runde.
    ///
    /// **Gesperrt heißt gebunden:** Wer eine Sperre hält, schlägt den
    /// gesperrten Block vor, nichts anderes. Das ist keine Höflichkeit,
    /// sondern die Safety-Regel: Ein Leader, der trotz Sperre etwas
    /// anderes vorschlüge, bekäme von allen gesperrten Validatoren ein
    /// `Locked` zurück und verlöre die Runde.
    ///
    /// **Das Zertifikat reist mit, wenn es taugt.** Es ist der Beleg für
    /// die anderen, dass sie ihre Sperre gefahrlos lösen dürfen. Taugen
    /// heißt: derselbe Block und eine Runde **vor** der laufenden, denn
    /// ein Zertifikat aus der laufenden Runde belegte ein Quorum, das
    /// der Vorschlag erst herbeiführen soll.
    fn baue_vorschlag(&self, runde: Round) -> Result<Konsensnachricht, KonsensFehler> {
        let block = match self.treiber.lock() {
            Some(Lock { block_hash, .. }) => block_hash,
            None => self.eigener_vorschlag,
        };
        let zertifikat = self
            .zertifikat
            .as_ref()
            .filter(|z| z.block_hash == block && z.round < runde)
            .cloned();

        match zertifikat {
            Some(z) => {
                // ⚑ Fund 66: Die Signatur muss die Runde des Zertifikats
                // mit abdecken, sonst kann ein Abhörer ein anderes
                // gültiges Zertifikat anhängen.
                let sig = self
                    .schluessel
                    .signiere(&propose_pol_message(runde, &block, z.round))?;
                Ok(Konsensnachricht::ProposeMitPolka(
                    Propose {
                        round: runde,
                        block_hash: block,
                        leader: self.ich,
                        signature: sig,
                    },
                    z,
                ))
            }
            None => {
                let sig = self.schluessel.signiere(&propose_message(runde, &block))?;
                Ok(Konsensnachricht::Propose(Propose {
                    round: runde,
                    block_hash: block,
                    leader: self.ich,
                    signature: sig,
                }))
            }
        }
    }

    /// Ist die Runde abgeschlossen?
    pub fn ist_commitet(&self) -> bool {
        self.treiber.is_committed()
    }

    /// Der commitete Block, falls die Runde durch ist.
    pub fn commiteter_block(&self) -> Option<Hash> {
        self.treiber.committed_block()
    }

    /// Die eigene Kennung.
    pub fn ich(&self) -> MinerId {
        self.ich
    }

    /// Der Leader dieser Runde.
    pub fn leader(&self) -> MinerId {
        self.treiber.leader()
    }

    /// Die Rundennummer.
    pub fn runde(&self) -> Round {
        self.treiber.round()
    }

    /// Wie oft die Runde gewechselt hat.
    pub fn wechsel(&self) -> u64 {
        self.wechsel
    }

    /// Die Sperre, falls eine gehalten wird.
    pub fn sperre(&self) -> Option<Lock> {
        self.treiber.lock()
    }

    /// Ob ein Polka-Zertifikat vorliegt, und aus welcher Runde.
    pub fn zertifikatsrunde(&self) -> Option<Round> {
        self.zertifikat.as_ref().map(|z| z.round)
    }

    /// Der Zustand, für das Betriebsprotokoll.
    pub fn status(&self) -> RoundStatus {
        self.treiber.status()
    }

    /// Eingegangenes Stimmgewicht, Commit-Gewicht und Schwelle.
    ///
    /// Für das Betriebsprotokoll. **Gewicht, nicht Köpfe:** Ein
    /// Protokoll, das „3 von 5 Stimmen" meldet, verdeckt genau den
    /// Unterschied, für den die Genesis-Verteilung gebaut wurde.
    pub fn gewichte(&self) -> (u64, u64, u64) {
        let z = self.treiber.state();
        (z.vote_weight(), z.commit_weight(), z.threshold())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_consensus::select_leader;
    use std::collections::VecDeque;

    const NAMEN: [&str; 5] = ["alpha", "beta", "gamma", "delta", "epsilon"];
    const STAKES: [u64; 5] = [250_000_000, 230_000_000, 200_000_000, 120_000_000, 100_000_000];

    /// Die Verteilung des Probenetzes.
    fn probenetz() -> Genesis {
        let mut text = String::from("netz konsens-test\n");
        for (name, stake) in NAMEN.iter().zip(STAKES) {
            let k = Konsensschluessel::probe(name).expect("Schlüssel");
            text.push_str(&k.genesiszeile(stake).expect("Zeile"));
            text.push('\n');
        }
        Genesis::aus_text(&text).expect("lesbar")
    }

    fn vorschlag() -> Hash {
        Hash::sha256(b"block der ersten runde")
    }

    /// Großzügige Fristen: Wo kein Wechsel gemessen wird, soll auch
    /// keiner passieren.
    fn weite_fristen() -> TimeoutConfig {
        TimeoutConfig {
            propose_ms: 1_000_000,
            vote_ms: 1_000_000,
            commit_ms: 1_000_000,
            delta_ms: 1_000,
        }
    }

    fn leader_name_der_runde(g: &Genesis, runde: Round) -> &'static str {
        let leader = select_leader(runde, &g.kennungen()).expect("Leader");
        for name in NAMEN {
            if Konsensschluessel::probe(name).unwrap().kennung() == leader {
                return name;
            }
        }
        panic!("Leader gehört zu keinem bekannten Namen");
    }

    /// Ein Netz aus Konsensrunden mit **einer** virtuellen Uhr.
    ///
    /// Der Absender bekommt seine eigene Nachricht nicht zurück, genau
    /// wie bei Gossipsub.
    struct Netz {
        knoten: Vec<(&'static str, Konsensrunde)>,
        schlange: VecDeque<(usize, Konsensnachricht)>,
        uhr: u64,
    }

    impl Netz {
        /// `teilnehmer` nennt, wer überhaupt startet. Wer fehlt, ist
        /// ausgefallen, bevor es losging.
        fn neu(g: &Genesis, teilnehmer: &[&'static str], t: TimeoutConfig) -> Self {
            let mut netz = Self {
                knoten: Vec::new(),
                schlange: VecDeque::new(),
                uhr: 0,
            };
            for name in teilnehmer {
                let k = Konsensschluessel::probe(name).expect("Schlüssel");
                let (r, raus) =
                    Konsensrunde::beginnen(g, k, vorschlag(), 0, t).expect("Runde");
                let i = netz.knoten.len();
                for n in raus {
                    netz.schlange.push_back((i, n));
                }
                netz.knoten.push((name, r));
            }
            netz
        }

        /// Stellt zu, bis nichts mehr nachkommt.
        fn zustellen(&mut self) {
            let mut schritte = 0;
            while let Some((von, n)) = self.schlange.pop_front() {
                schritte += 1;
                assert!(schritte < 10_000, "das Netz kam nicht zur Ruhe");
                let uhr = self.uhr;
                for i in 0..self.knoten.len() {
                    if i == von {
                        continue;
                    }
                    let (_, raus) = self.knoten[i].1.empfange(&n, uhr);
                    for m in raus {
                        self.schlange.push_back((i, m));
                    }
                }
            }
        }

        /// Stellt die Uhr vor und lässt alle Knoten takten.
        fn vorspulen(&mut self, ms: u64) {
            self.uhr += ms;
            let uhr = self.uhr;
            for i in 0..self.knoten.len() {
                let (_, raus) = self.knoten[i].1.takt(uhr);
                for m in raus {
                    self.schlange.push_back((i, m));
                }
            }
            self.zustellen();
        }

        fn fertig(&self) -> usize {
            self.knoten.iter().filter(|(_, r)| r.ist_commitet()).count()
        }

        fn runde(&self, name: &str) -> &Konsensrunde {
            &self.knoten.iter().find(|(n, _)| *n == name).expect("Knoten").1
        }

        fn index(&self, name: &str) -> usize {
            self.knoten
                .iter()
                .position(|(n, _)| *n == name)
                .expect("Knoten")
        }

        /// Gibt eine Nachricht von außen herein und gibt zurück, was
        /// daraufhin hinaus müsste. Zwei Netze lassen sich damit
        /// verbinden, ohne sie zu einem zu machen.
        fn von_aussen(&mut self, name: &str, n: &Konsensnachricht) -> Vec<Konsensnachricht> {
            let i = self.index(name);
            let uhr = self.uhr;
            self.knoten[i].1.empfange(n, uhr).1
        }

        /// Lässt **einen** Knoten takten und liefert seine Nachrichten
        /// aus, statt sie zuzustellen.
        fn takt_von(&mut self, name: &str, ms: u64) -> Vec<Konsensnachricht> {
            self.uhr += ms;
            let i = self.index(name);
            let uhr = self.uhr;
            self.knoten[i].1.takt(uhr).1
        }
    }

    // ── Der ungestörte Fall ─────────────────────────────────────────

    #[test]
    fn eine_runde_kommt_bei_allen_fuenf_zum_abschluss() {
        let g = probenetz();
        let mut netz = Netz::neu(&g, &NAMEN, weite_fristen());
        netz.zustellen();
        assert_eq!(netz.fertig(), 5, "nicht alle Knoten haben commitet");
        for (name, r) in &netz.knoten {
            assert_eq!(r.commiteter_block(), Some(vorschlag()), "{name}");
            assert_eq!(r.runde(), 0, "{name} wechselte ohne Grund die Runde");
            assert_eq!(r.wechsel(), 0);
        }
    }

    #[test]
    fn alle_knoten_rechnen_denselben_leader() {
        // Die Producer-Liste hängt an den Schlüsseln, nicht an der
        // Reihenfolge in der Datei. Rechneten zwei Knoten verschiedene
        // Leader, verwürfe jeder den Propose des anderen.
        let g = probenetz();
        let netz = Netz::neu(&g, &NAMEN, weite_fristen());
        let leader: Vec<MinerId> = netz.knoten.iter().map(|(_, r)| r.leader()).collect();
        assert!(leader.windows(2).all(|w| w[0] == w[1]));
    }

    #[test]
    fn die_leaderrolle_wandert_ueber_die_runden() {
        // Bliebe sie stehen, wäre ein einzelner Ausfall das Ende.
        let g = probenetz();
        let mut gesehen = std::collections::BTreeSet::new();
        for runde in 0..5u64 {
            gesehen.insert(select_leader(runde, &g.kennungen()).expect("Leader"));
        }
        assert_eq!(gesehen.len(), 5, "die Leaderrolle blieb stehen");
    }

    /// ⚑ **Die eigene Stimme muss im eigenen Automaten landen.**
    #[test]
    fn die_eigene_stimme_zaehlt_im_eigenen_automaten() {
        let g = probenetz();
        let leader = leader_name_der_runde(&g, 0);
        let netz = Netz::neu(&g, &[leader], weite_fristen());
        let (stimmgewicht, _, _) = netz.runde(leader).gewichte();
        assert!(
            stimmgewicht > 0,
            "der Leader zählte seine eigene Stimme nicht mit"
        );
    }

    // ── Gewicht gegen Köpfe ─────────────────────────────────────────

    /// Lässt **genau** die genannten Knoten stimmen.
    fn stimmgewicht_von(g: &Genesis, stimmende: &[&'static str]) -> (u64, u64, bool) {
        let leader_name = leader_name_der_runde(g, 0);
        let beobachter_name = stimmende
            .iter()
            .copied()
            .find(|n| *n != leader_name)
            .unwrap_or(stimmende[0]);

        let k = Konsensschluessel::probe(beobachter_name).expect("Schlüssel");
        let (mut beobachter, _) =
            Konsensrunde::beginnen(g, k, vorschlag(), 0, weite_fristen()).expect("Runde");

        if beobachter_name != leader_name {
            let lk = Konsensschluessel::probe(leader_name).expect("Leaderschlüssel");
            let h = vorschlag();
            let p = Propose {
                round: 0,
                block_hash: h,
                leader: lk.kennung(),
                signature: lk.signiere(&propose_message(0, &h)).expect("Signatur"),
            };
            assert!(beobachter
                .empfange(&Konsensnachricht::Propose(p), 0)
                .0
                .ist_angenommen());
        }

        for name in stimmende.iter().filter(|n| **n != beobachter_name) {
            let k = Konsensschluessel::probe(name).expect("Schlüssel");
            let h = vorschlag();
            let v = Vote {
                round: 0,
                block_hash: h,
                voter: k.kennung(),
                signature: k.signiere(&vote_message(0, &h)).expect("Signatur"),
            };
            let (urteil, _) = beobachter.empfange(&Konsensnachricht::Vote(v), 0);
            assert!(urteil.ist_angenommen(), "{name}: {urteil:?}");
        }

        let (gewicht, _, schwelle) = beobachter.gewichte();
        (gewicht, schwelle, beobachter.ist_commitet())
    }

    /// ⚑ **Der Test, für den die Verteilung gebaut wurde.**
    #[test]
    fn drei_von_fuenf_koepfen_sind_kein_quorum() {
        let g = probenetz();
        let (gewicht, schwelle, commitet) =
            stimmgewicht_von(&g, &["gamma", "delta", "epsilon"]);
        assert_eq!(gewicht, 420_000_000);
        assert!(
            gewicht < schwelle,
            "drei von fünf Köpfen erreichten das Quorum: {gewicht} >= {schwelle}. \
             Dann zählt hier jemand Köpfe statt Gewicht (Fund A3)"
        );
        assert!(!commitet);
    }

    #[test]
    fn drei_schwere_koepfe_sind_ein_quorum() {
        let g = probenetz();
        let (gewicht, schwelle, _) = stimmgewicht_von(&g, &["alpha", "beta", "gamma"]);
        assert_eq!(gewicht, 680_000_000);
        assert!(gewicht >= schwelle);
    }

    /// ⚑ **Exakt zwei Drittel reichen nicht.**
    #[test]
    fn exakt_zwei_drittel_sind_kein_quorum() {
        let g = probenetz();
        let (gewicht, schwelle, commitet) = stimmgewicht_von(&g, &["alpha", "beta", "delta"]);
        assert_eq!(gewicht, 600_000_000, "exakt zwei Drittel von 900");
        assert_eq!(schwelle, 600_000_001);
        assert!(
            gewicht < schwelle,
            "exakt zwei Drittel erreichten das Quorum. Dann steht das `+ 1` in \
             quorum_threshold falsch, und zwei Quoren können sich verfehlen"
        );
        assert!(!commitet);
        let (mehr, schwelle2, _) = stimmgewicht_von(&g, &["alpha", "beta", "delta", "epsilon"]);
        assert_eq!(mehr, 700_000_000);
        assert!(mehr >= schwelle2);
    }

    // ── Rundenwechsel ───────────────────────────────────────────────

    /// ⚑ **Der Punkt, für den der Rundenwechsel gebaut wurde.**
    ///
    /// Der Leader von Runde 0 startet gar nicht erst. Ohne
    /// Rundenwechsel wartet das Netz ewig auf seinen Vorschlag.
    #[test]
    fn ein_ausgefallener_leader_haelt_die_runde_nicht_auf() {
        let g = probenetz();
        let ausgefallen = leader_name_der_runde(&g, 0);
        let uebrige: Vec<&'static str> =
            NAMEN.iter().copied().filter(|n| *n != ausgefallen).collect();

        let t = TimeoutConfig {
            propose_ms: 1_000,
            vote_ms: 1_000,
            commit_ms: 1_000,
            delta_ms: 500,
        };
        let mut netz = Netz::neu(&g, &uebrige, t);
        netz.zustellen();
        assert_eq!(netz.fertig(), 0, "ohne Leader darf nichts commiten");

        // Die Frist verfällt.
        netz.vorspulen(1_001);

        assert_eq!(
            netz.fertig(),
            4,
            "nach dem Rundenwechsel muss der neue Leader vorschlagen und \
             alle vier übrigen commiten"
        );
        for (name, r) in &netz.knoten {
            assert_eq!(r.runde(), 1, "{name} steht in der falschen Runde");
            assert_eq!(r.wechsel(), 1, "{name}");
            assert_eq!(r.commiteter_block(), Some(vorschlag()), "{name}");
        }
    }

    /// **Egal wer ausfällt, die übrigen kommen durch.**
    ///
    /// Folgt aus der Ein-Drittel-Schranke: Wer fehlt, hält weniger als
    /// ein Drittel, also halten die übrigen mehr als zwei Drittel. Das
    /// ist die Zusage, die der Rundenwechsel einlösen muss, und zwar
    /// **für jeden** Ausfall, nicht nur für den bequemen.
    ///
    /// *Hier stand zuerst ein Test, der nur die Stake-Konstanten
    /// addierte. Er hätte auch bestanden, wenn der Rundenwechsel gar
    /// nicht funktionierte.*
    #[test]
    fn egal_wer_ausfaellt_die_uebrigen_kommen_durch() {
        let g = probenetz();
        let t = TimeoutConfig {
            propose_ms: 1_000,
            vote_ms: 1_000,
            commit_ms: 1_000,
            delta_ms: 500,
        };
        for ausgefallen in NAMEN {
            let uebrige: Vec<&'static str> =
                NAMEN.iter().copied().filter(|n| *n != ausgefallen).collect();
            let mut netz = Netz::neu(&g, &uebrige, t);
            netz.zustellen();
            // Bis zu vier Wechsel, falls mehrere Runden hintereinander
            // den Ausgefallenen als Leader ziehen. Mehr kann es nicht
            // geben: Die Leaderrolle wandert über alle fünf.
            for _ in 0..5 {
                if netz.fertig() == uebrige.len() {
                    break;
                }
                let bis = netz
                    .knoten
                    .iter()
                    .map(|(_, r)| r.frist_ms())
                    .min()
                    .expect("Knoten");
                netz.vorspulen(bis.saturating_sub(netz.uhr) + 1);
            }
            assert_eq!(
                netz.fertig(),
                uebrige.len(),
                "ohne {ausgefallen} kamen nur {} von {} durch",
                netz.fertig(),
                uebrige.len()
            );
            // `Hash` ist nicht `Ord`, also über die Bytes vergleichen.
            let bloecke: std::collections::BTreeSet<[u8; 32]> = netz
                .knoten
                .iter()
                .filter_map(|(_, r)| r.commiteter_block())
                .map(|h| {
                    let mut b = [0u8; 32];
                    b.copy_from_slice(h.as_bytes());
                    b
                })
                .collect();
            assert_eq!(
                bloecke.len(),
                1,
                "ohne {ausgefallen} standen {} verschiedene Blöcke",
                bloecke.len()
            );
        }
    }

    /// ⚑ **Fund 67: Wer allein vorauseilt, kommt nicht zurück.**
    ///
    /// Gemessen am 2026-08-26 über fünf echte Prozesse. Die Zeitachse
    /// aus den Betriebsprotokollen:
    ///
    /// | Zeit | Ereignis |
    /// |---|---|
    /// | +1 ms | alpha hat Mesh 4, beginnt Runde 0, schlägt vor |
    /// | +502 ms | alpha wechselt auf Runde 1, **Stimmgewicht 0** |
    /// | +523 ms | die anderen vier beginnen erst jetzt ihre Runde 0 |
    /// | +531 ms | die vier commiten Runde 0, ohne alpha |
    /// | +9519 ms | alpha ist bei Runde 5 und schlägt ins Leere |
    ///
    /// **Zwei Dinge trafen zusammen.** Erstens startete alpha sechs
    /// Sekunden vor den anderen, und sein Mesh war voll, bevor deren
    /// Konsensrunden liefen: Ein volles Gossip-Mesh heißt nicht, dass
    /// die Gegenstellen schon mitstimmen. Zweitens ist die Frist der
    /// Vote-Phase 500 ms, und das ist kürzer als der Abstand zwischen
    /// zwei Prozessstarts.
    ///
    /// **Die Safety hielt**, alle vier commiteten denselben Block.
    /// Verloren ging die **Liveness eines einzelnen Knotens**: alpha
    /// kommt nicht zurück, denn nichts sagt ihm, dass Runde 0 längst
    /// entschieden ist.
    ///
    /// **Warum die naheliegende Regel hier nicht half.** Der übliche
    /// Ausgleich ist, auf Nachrichten aus einer **höheren** Runde zu
    /// springen, sobald mehr als ein Drittel des Gewichts von dort
    /// kommt. Alpha ist aber nicht zurück, sondern **voraus**. Es
    /// braucht den umgekehrten Weg: den Beleg, dass eine frühere Runde
    /// entschieden hat.
    ///
    /// # Der Ausgleich, seit dem 2026-08-29
    ///
    /// Gebaut als [`Commitzertifikat`], und **nicht** über die Kette,
    /// wie hier zuvor vermutet stand. Der Umweg über die Kette scheiterte
    /// an einer Tatsache, die erst beim Nachlesen auffiel: Ein Commit
    /// legt bis heute keinen Block in die Kette und veröffentlicht auch
    /// keinen, er schreibt eine Protokollzeile. Über die Kette wäre also
    /// nichts zurückgekommen.
    ///
    /// Der Beleg dagegen steht für sich. Alpha braucht dafür weder eine
    /// Kette noch die Runde der anderen, nur die stimmberechtigte Menge,
    /// die es aus der Genesis-Datei ohnehin hat.
    ///
    /// **Der Weg im Test ist der Weg im Betrieb**, in beide Richtungen:
    /// Alpha gibt sich durch seinen Vorschlag aus Runde 5 selbst zu
    /// erkennen, die Gegenseite antwortet mit dem Beleg, Alpha übernimmt
    /// die Entscheidung. Niemand reicht ihm etwas an, das er im Betrieb
    /// nicht bekäme.
    ///
    /// **Was der Ausgleich nicht heilt:** Alpha hat in Runde 0 nicht
    /// mitgestimmt und bekommt für sie keine Belohnung. Zurück heißt
    /// hier, wieder mitzulaufen, nicht, das Versäumte gutgeschrieben zu
    /// bekommen.
    #[test]
    fn fund_67_wer_allein_vorauseilt_kommt_mit_dem_beleg_zurueck() {
        let g = probenetz();
        let t = TimeoutConfig {
            propose_ms: 1_000,
            vote_ms: 500,
            commit_ms: 500,
            delta_ms: 500,
        };
        let leader = leader_name_der_runde(&g, 0);
        let uebrige: Vec<&'static str> =
            NAMEN.iter().copied().filter(|n| *n != leader).collect();

        // Der Leader startet allein und schlägt ins Leere.
        let mut vorlaeufer = Netz::neu(&g, &[leader], t);
        vorlaeufer.zustellen();
        assert_eq!(vorlaeufer.runde(leader).runde(), 0);

        // Seine Vote-Frist verfällt, bevor jemand antworten kann.
        vorlaeufer.vorspulen(501);
        assert_eq!(
            vorlaeufer.runde(leader).runde(),
            1,
            "der Leader müsste allein weitergewechselt haben"
        );
        assert_eq!(vorlaeufer.runde(leader).gewichte().0, 0, "Stimmgewicht 0");

        // Die übrigen vier beginnen jetzt erst, in Runde 0, und kommen
        // ohne ihn durch: Sie halten zusammen mehr als zwei Drittel.
        let mut spaete = Netz::neu(&g, &uebrige, t);
        spaete.zustellen();
        // Ohne den Leader von Runde 0 fehlt der Vorschlag; erst nach dem
        // Wechsel geht es weiter.
        spaete.vorspulen(1_001);
        assert_eq!(spaete.fertig(), 4, "die vier kommen ohne den Vorläufer durch");

        // Bis hierher ist es der aufgezeichnete Vorfall: Der Vorläufer
        // steht draußen, und von allein kommt er nicht zurück.
        assert!(!vorlaeufer.runde(leader).ist_commitet());

        // ── Der Rückweg ────────────────────────────────────────────
        //
        // Der Vorläufer dreht weiter, bis er wieder Leader ist. Bei
        // fünf Producern ist das Runde 5, und genau dort stand er im
        // Vorfall. Erst dann sendet er wieder etwas, und erst dadurch
        // gibt er sich als außer Takt zu erkennen.
        let mut alphas_vorschlag = None;
        for _ in 0..20 {
            let raus = vorlaeufer.takt_von(leader, 2_000);
            if let Some(n) = raus.into_iter().find(|n| {
                matches!(
                    n,
                    Konsensnachricht::Propose(_) | Konsensnachricht::ProposeMitPolka(_, _)
                )
            }) {
                alphas_vorschlag = Some(n);
                break;
            }
        }
        let alphas_vorschlag = alphas_vorschlag.expect("der Vorläufer schlägt wieder vor");
        assert_eq!(
            vorlaeufer.runde(leader).runde(),
            5,
            "im Vorfall stand der Vorläufer bei Runde 5"
        );

        // Ein Knoten der Gegenseite hört ihn. Für seinen Automaten ist
        // das eine Nachricht der falschen Runde, und genau daran erkennt
        // er, dass der Absender Hilfe braucht.
        let antwort = spaete.von_aussen(uebrige[0], &alphas_vorschlag);
        let beleg = antwort
            .iter()
            .find(|n| matches!(n, Konsensnachricht::Commitzertifikat(_)))
            .expect("die Gegenseite antwortet mit dem Beleg");

        // ⚑ Und damit kommt er zurück.
        let (urteil, _) = {
            let i = vorlaeufer.index(leader);
            let uhr = vorlaeufer.uhr;
            vorlaeufer.knoten[i].1.empfange(beleg, uhr)
        };
        assert!(urteil.ist_angenommen(), "der Beleg wurde abgewiesen: {urteil:?}");
        assert!(
            vorlaeufer.runde(leader).ist_commitet(),
            "der Vorläufer ist nicht zurückgekommen"
        );
        assert_eq!(
            vorlaeufer.runde(leader).commiteter_block(),
            spaete.runde(uebrige[0]).commiteter_block(),
            "zurückgekommen, aber auf einen anderen Block"
        );

        // Ein zweiter Beleg über dieselbe Entscheidung ändert nichts und
        // kostet auch nichts: Die Gegenseite hilft jedem genau einmal.
        let nochmal = spaete.von_aussen(uebrige[0], &alphas_vorschlag);
        assert!(
            !nochmal
                .iter()
                .any(|n| matches!(n, Konsensnachricht::Commitzertifikat(_))),
            "die Gegenseite schickt den Beleg ein zweites Mal"
        );
    }

    #[test]
    fn die_frist_waechst_mit_der_runde() {
        // Der Zuwachs ist der Grund, warum das Verfahren nach GST
        // terminiert: Er überschreitet irgendwann jede reale Laufzeit.
        let t = TimeoutConfig {
            propose_ms: 1_000,
            vote_ms: 1_000,
            commit_ms: 1_000,
            delta_ms: 500,
        };
        let g = probenetz();
        let ausgefallen = leader_name_der_runde(&g, 0);
        let uebrige: Vec<&'static str> =
            NAMEN.iter().copied().filter(|n| *n != ausgefallen).collect();
        // Nur einen Knoten fahren: Er wechselt allein, ohne dass jemand
        // vorschlägt, und die Fristen sind ablesbar.
        let einzeln: Vec<&'static str> = vec![uebrige[0]];
        let mut netz = Netz::neu(&g, &einzeln, t);
        let f0 = netz.runde(einzeln[0]).frist_ms();
        assert_eq!(f0, 1_000);
        netz.vorspulen(1_001);
        assert_eq!(netz.runde(einzeln[0]).runde(), 1);
        // Runde 1: 1000 + 1 × 500.
        assert_eq!(netz.runde(einzeln[0]).frist_ms(), netz.uhr + 1_500);
    }

    #[test]
    fn ohne_zuwachs_ist_liveness_nicht_zugesichert() {
        // Sicher, aber möglicherweise dauerhaft blockiert.
        assert!(!TimeoutConfig {
            propose_ms: 1_000,
            vote_ms: 500,
            commit_ms: 500,
            delta_ms: 0,
        }
        .is_live());
        assert!(TimeoutConfig::default().is_live());
    }

    /// ⚑ **Gesperrt heißt gebunden.**
    ///
    /// Wer nach einem Quorum an Stimmen gesperrt ist, schlägt in der
    /// nächsten Runde denselben Block vor, nicht seinen eigenen.
    #[test]
    fn wer_gesperrt_ist_schlaegt_den_gesperrten_block_vor() {
        let g = probenetz();
        let t = TimeoutConfig {
            propose_ms: 1_000,
            vote_ms: 1_000,
            commit_ms: 1_000,
            delta_ms: 0,
        };
        let mut netz = Netz::neu(&g, &NAMEN, t);
        netz.zustellen();
        // Alle sind commitet und gesperrt auf denselben Block.
        for (name, r) in &netz.knoten {
            assert_eq!(
                r.sperre().map(|l| l.block_hash),
                Some(vorschlag()),
                "{name} hält keine Sperre"
            );
        }
        // Und ein Zertifikat liegt vor, aus Runde 0.
        for (name, r) in &netz.knoten {
            assert_eq!(r.zertifikatsrunde(), Some(0), "{name} hat kein Zertifikat");
        }
    }

    // ── Ablehnungen ─────────────────────────────────────────────────

    #[test]
    fn ein_aussenstehender_kann_keine_runde_beginnen() {
        let g = probenetz();
        let fremd = Konsensschluessel::probe("zeta").expect("Schlüssel");
        assert!(matches!(
            Konsensrunde::beginnen(&g, fremd, vorschlag(), 0, weite_fristen()),
            Err(KonsensFehler::NichtStimmberechtigt { .. })
        ));
    }

    #[test]
    fn eine_stimme_eines_aussenstehenden_wird_abgewiesen() {
        let g = probenetz();
        let leader_name = leader_name_der_runde(&g, 0);
        let k = Konsensschluessel::probe(leader_name).expect("Schlüssel");
        let (mut r, _) =
            Konsensrunde::beginnen(&g, k, vorschlag(), 0, weite_fristen()).expect("Runde");

        let fremd = Konsensschluessel::probe("zeta").expect("Schlüssel");
        let h = vorschlag();
        let vote = Vote {
            round: 0,
            block_hash: h,
            voter: fremd.kennung(),
            signature: fremd.signiere(&vote_message(0, &h)).expect("Signatur"),
        };
        let (urteil, raus) = r.empfange(&Konsensnachricht::Vote(vote), 0);
        assert_eq!(urteil.als_text(), "nicht-stimmberechtigt");
        assert!(raus.is_empty());
        assert!(!urteil.ist_harmlos());
    }

    #[test]
    fn eine_gefaelschte_signatur_wird_abgewiesen() {
        let g = probenetz();
        let leader_name = leader_name_der_runde(&g, 0);
        let k = Konsensschluessel::probe(leader_name).expect("Schlüssel");
        let (mut r, _) =
            Konsensrunde::beginnen(&g, k, vorschlag(), 0, weite_fristen()).expect("Runde");

        // Der Beobachter ist Leader und hat schon für sich gestimmt. Die
        // Fälschung muss deshalb im Namen eines **anderen** kommen,
        // sonst greift die Duplikatprüfung vor der Signaturprüfung.
        let mut fremde = NAMEN.iter().filter(|n| **n != leader_name);
        let opfer = Konsensschluessel::probe(fremde.next().unwrap()).unwrap();
        let taeter = Konsensschluessel::probe(fremde.next().unwrap()).unwrap();
        let h = vorschlag();
        let vote = Vote {
            round: 0,
            block_hash: h,
            voter: opfer.kennung(),
            signature: taeter.signiere(&vote_message(0, &h)).expect("Signatur"),
        };
        let (urteil, folge) = r.empfange(&Konsensnachricht::Vote(vote), 0);
        assert_eq!(urteil.als_text(), "signatur-falsch");
        assert!(!urteil.ist_harmlos());
        assert!(folge.is_empty());
    }

    #[test]
    fn dieselbe_stimme_mit_eigener_signatur_kommt_durch() {
        // Gegenprobe: Ohne sie wäre auch ein Automat grün, der jede
        // Stimme ablehnt.
        let g = probenetz();
        let leader_name = leader_name_der_runde(&g, 0);
        let k = Konsensschluessel::probe(leader_name).expect("Schlüssel");
        let (mut r, _) =
            Konsensrunde::beginnen(&g, k, vorschlag(), 0, weite_fristen()).expect("Runde");
        let opfer_name = NAMEN.iter().find(|n| **n != leader_name).unwrap();
        let opfer = Konsensschluessel::probe(opfer_name).unwrap();
        let h = vorschlag();
        let vote = Vote {
            round: 0,
            block_hash: h,
            voter: opfer.kennung(),
            signature: opfer.signiere(&vote_message(0, &h)).expect("Signatur"),
        };
        assert!(r
            .empfange(&Konsensnachricht::Vote(vote), 0)
            .0
            .ist_angenommen());
    }

    #[test]
    fn ein_propose_von_jemandem_der_nicht_leader_ist_wird_abgewiesen() {
        let g = probenetz();
        let leader = select_leader(0, &g.kennungen()).expect("Leader");
        let nicht_leader = NAMEN
            .iter()
            .find(|n| Konsensschluessel::probe(n).unwrap().kennung() != leader)
            .unwrap();

        let k = Konsensschluessel::probe(NAMEN[0]).expect("Schlüssel");
        let (mut r, _) =
            Konsensrunde::beginnen(&g, k, vorschlag(), 0, weite_fristen()).expect("Runde");

        let falscher = Konsensschluessel::probe(nicht_leader).expect("Schlüssel");
        let h = Hash::sha256(b"eigenmaechtiger vorschlag");
        let p = Propose {
            round: 0,
            block_hash: h,
            leader: falscher.kennung(),
            signature: falscher.signiere(&propose_message(0, &h)).expect("Signatur"),
        };
        let (urteil, _) = r.empfange(&Konsensnachricht::Propose(p), 0);
        assert_eq!(urteil.als_text(), "falscher-leader");
    }

    #[test]
    fn doppelte_nachrichten_gelten_als_harmlos() {
        // Bei Gossip ist dieselbe Stimme über mehrere Wege die Regel.
        let g = probenetz();
        let leader_name = leader_name_der_runde(&g, 0);
        let k = Konsensschluessel::probe(leader_name).expect("Schlüssel");
        let (_, raus) =
            Konsensrunde::beginnen(&g, k, vorschlag(), 0, weite_fristen()).expect("Runde");

        let anderer = NAMEN.iter().find(|n| **n != leader_name).unwrap();
        let k2 = Konsensschluessel::probe(anderer).expect("Schlüssel");
        let (mut r2, _) =
            Konsensrunde::beginnen(&g, k2, vorschlag(), 0, weite_fristen()).expect("Runde");

        let propose = raus.first().expect("ein Propose").clone();
        assert!(r2.empfange(&propose, 0).0.ist_angenommen());
        let (zweites, folge) = r2.empfange(&propose, 0);
        assert_eq!(zweites.als_text(), "doppelt");
        assert!(zweites.ist_harmlos());
        assert!(folge.is_empty(), "ein Duplikat löste eine zweite Stimme aus");
    }

    #[test]
    fn eine_nachricht_der_falschen_runde_gilt_als_harmlos() {
        // Nach einem Rundenwechsel trudeln die Nachrichten der alten
        // Runde noch ein. Das ist der Normalfall, keine Auffälligkeit.
        let g = probenetz();
        let k = Konsensschluessel::probe(NAMEN[0]).expect("Schlüssel");
        let (mut r, _) =
            Konsensrunde::beginnen(&g, k, vorschlag(), 0, weite_fristen()).expect("Runde");
        let alpha = Konsensschluessel::probe("alpha").expect("a");
        let h = vorschlag();
        let vote = Vote {
            round: 4,
            block_hash: h,
            voter: alpha.kennung(),
            signature: alpha.signiere(&vote_message(4, &h)).expect("Signatur"),
        };
        let (urteil, _) = r.empfange(&Konsensnachricht::Vote(vote), 0);
        assert_eq!(urteil.als_text(), "falsche-runde");
        assert!(urteil.ist_harmlos());
    }

    #[test]
    fn zufallsbytes_gelten_als_unlesbar_und_nicht_als_angriff() {
        let g = probenetz();
        let k = Konsensschluessel::probe(NAMEN[0]).expect("Schlüssel");
        let (mut r, _) =
            Konsensrunde::beginnen(&g, k, vorschlag(), 0, weite_fristen()).expect("Runde");
        let (urteil, raus) = r.empfange_bytes(&[0xAB; 7], 0);
        assert_eq!(urteil, Urteil::Unlesbar);
        assert!(raus.is_empty());
    }

    #[test]
    fn die_bytes_von_der_leitung_ergeben_dieselbe_runde() {
        let g = probenetz();
        let leader_name = leader_name_der_runde(&g, 0);
        let k = Konsensschluessel::probe(leader_name).expect("Schlüssel");
        let (_, raus) =
            Konsensrunde::beginnen(&g, k, vorschlag(), 0, weite_fristen()).expect("Runde");

        let anderer = NAMEN.iter().find(|n| **n != leader_name).unwrap();
        let k2 = Konsensschluessel::probe(anderer).expect("Schlüssel");
        let (mut r2, _) =
            Konsensrunde::beginnen(&g, k2, vorschlag(), 0, weite_fristen()).expect("Runde");

        let bytes = borsh::to_vec(&raus[0]).expect("serialisieren");
        let (urteil, folge) = r2.empfange_bytes(&bytes, 0);
        assert!(urteil.ist_angenommen());
        assert_eq!(folge.len(), 1, "auf einen Propose folgt genau eine Stimme");
        assert!(matches!(folge[0], Konsensnachricht::Vote(_)));
    }
}
