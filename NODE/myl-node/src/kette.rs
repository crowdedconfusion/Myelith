//! Die Probekette: echte Blöcke, echter Zustand, **Wegwerfware**.
//!
//! # ⚑ Das ist nicht der Beginn der Blockchain
//!
//! Steht zuerst, weil eine Verwechslung teuer wäre. Diese Kette ist eine
//! **Trockenübung des Codes**, keine Inbetriebnahme:
//!
//! - **Jeder Start beginnt bei null.** Keine Fortsetzung, keine
//!   Wiederherstellung, keine Historie. Wird der Knoten beendet, ist der
//!   Zustand weg.
//! - **Die MYL sind Spielgeld.** [`PROBEGUTHABEN`] steht in keinem
//!   Verhältnis zur Genesis-Zuteilung des echten Netzes (TOKENOMICS
//!   Punkt 4.2). Wer hier Guthaben sieht, besitzt nichts.
//! - **Ein Probeblock kann niemals an eine echte Kette anschließen.**
//!   Die Verkettung hängt am Startwert, und [`PROBE_STARTWERT`] ist ein
//!   Wert, der ausdrücklich für Proben gewählt wurde. Das ist keine
//!   Regel, die jemand einhalten muss, sondern eine Eigenschaft der
//!   Verkettung, und ein Test hält sie fest.
//!
//! Wann das Testnetz beginnt, entscheidet das Projekt, nicht dieser
//! Code. Mehr dazu im Kopf von [`crate::probe`].
//!
//! # Was hier geschlossen wird, und was offen bleibt
//!
//! Bis zum 2026-08-24 produzierte kein Knoten Blöcke. Die
//! Zustandsmaschinen in `myl-consensus` waren vollständig, aber niemand
//! trieb sie über die Zeit: Es fehlten Rundentakt, Mempool und
//! Kettenzustand. Dieses Modul liefert alle drei.
//!
//! **Was es ist:** Blockproduktion mit Kettenverkettung, Mempool,
//! Ledger-Anwendung und Zustandswurzel. Jeder Knoten führt seine eigene
//! Kette und rechnet nach, ob er beim selben Zustand landet wie der
//! Erzeuger.
//!
//! **Was es nicht ist: BFT.** Es stimmt niemand ab. Ein Knoten
//! produziert, die übrigen übernehmen und rechnen nach. Die
//! Abstimmungsrunden (Propose, Vote, Commit) liegen fertig in
//! `myl-consensus::bft`, brauchen aber ein eigenes Gossip-Topic, einen
//! Validator-Satz mit Stake und BLS-Schlüssel je Knoten. Ein neues Topic
//! ist eine Protokollentscheidung, kein Detail, und die gehört nicht
//! nebenbei getroffen.
//!
//! # Warum das trotzdem eine echte Messung ist
//!
//! Die Frage, die dieser Aufbau beantwortet, ist nicht „einigen sich die
//! Knoten", sondern die davor: **Kommen zwei Maschinen aus denselben
//! Blöcken zum selben Zustand?**
//!
//! Das ist die Protokollhälfte derselben Frage, die der
//! Determinismus-Test für die Inferenz stellt. Weicht eine
//! Zustandswurzel ab, ist irgendwo im Ledger-Pfad etwas nicht
//! deterministisch, und das bricht den Konsens genauso wie ein
//! abweichendes Inferenzergebnis. **Ohne diesen Lauf wäre das erst im
//! echten Netz aufgefallen.**
//!
//! # ⚑ Ein Block kennt seine eigene Höhe nicht
//!
//! [`Block`] trägt `epoch`, `prev_block_hash`, `timestamp_ms` und
//! `state_root`, aber **kein Höhenfeld**. Die Kette hängt allein am
//! Vorgänger-Hash.
//!
//! Für diesen Aufbau heißt das: Die Höhe führt jeder Knoten selbst, und
//! ein Block, der außer der Reihe eintrifft, lässt sich nur über seinen
//! Vorgänger einordnen, nicht über eine Nummer. Das ist keine
//! Schwäche des Formats, sondern eine Eigenschaft, die man kennen muss,
//! bevor man Synchronisierung baut: **Ein Knoten, der einen Block
//! verpasst, kann die Lücke nicht benennen**, er merkt nur, dass der
//! nächste nicht anschließt.

use borsh::BorshDeserialize;
use myl_consensus::block::{Block, EpochMeta, Transaction};
use myl_ledger::state::LedgerState;
use myl_ledger::transitions::burn_to_credits;
use myl_types::hash::Hash;
use myl_types::ids::{Address, EpochId};

/// Credit-Preis zu Genesis der Testkette.
///
/// Nicht null: Ein Preis von null wäre eine Division durch null im
/// Ledger und ist dort als Protokollfehler ausgewiesen.
pub const PROBE_CREDIT_PREIS: u64 = 100;

/// Zahl der Probekonten, die zu Beginn Guthaben bekommen.
pub const PROBEKONTEN: u8 = 8;

/// Guthaben je Probekonto in MYL-Kleinstbeträgen. **Spielgeld.**
pub const PROBEGUTHABEN: u64 = 10_000_000;

/// Der Startwert, an dem die Probekette hängt.
///
/// **Der Riegel gegen eine Verwechslung mit dem echten Netz.** Ein Block
/// nennt seinen Vorgänger; der erste Probeblock nennt diesen Wert. Ein
/// echtes Netz beginnt bei einem anderen, und damit passt ein Probeblock
/// dort nirgends hinein. Der Text sagt es auch dem, der die Bytes
/// anschaut.
pub const PROBE_STARTWERT: &[u8] =
    b"MYELITH-PROBELAUF-KEIN-TESTNETZ-KEINE-ECHTE-KETTE";

/// ## ⚑ Warum es diese Konten gibt
///
/// Der erste Dreiknotenlauf lief grün und **maß nichts**: Die
/// Zustandswurzel stand bei jeder Höhe auf demselben Wert, obwohl
/// Blöcke mit Transaktionen ankamen.
///
/// Der Grund war einfach und leicht zu übersehen. Eine
/// Burn-Transaktion braucht Deckung; die Absender hatten keine, also
/// scheiterte jede und wurde übersprungen, wie es
/// [`Kette::anwenden`] vorsieht. **Ein unveränderter Zustand ist auf
/// jeder Maschine gleich**, und damit belegte die Übereinstimmung
/// genau nichts.
///
/// Deshalb bekommen zu Genesis acht Konten Guthaben. Sie sind ein
/// **Testaufbau, kein Protokollbestandteil**: Im echten Netz entsteht
/// Guthaben aus der Genesis-Zuteilung (TOKENOMICS Punkt 4.2), nicht aus
/// dieser Konstanten.
///
/// Die Adressen hängen an einer festen Zeichenkette und **nicht** am
/// Knotennamen. Sonst sähe der Genesis-Zustand auf jeder Maschine
/// anders aus, und der Lauf meldete einen Determinismusfehler, bevor
/// irgendetwas passiert ist.
pub fn probekonto(nummer: u8) -> Address {
    let h = Hash::sha256(format!("myelith-probekonto-{}", nummer % PROBEKONTEN).as_bytes());
    let mut roh = [0u8; 32];
    roh.copy_from_slice(h.as_bytes());
    Address::new(roh)
}

/// Das Testkonto, das ein Knoten dieses Namens benutzt.
///
/// Über den Namen gewählt, damit jeder Knoten stabil dasselbe Konto
/// bebucht: Ein wechselndes Konto machte zwei Läufe unvergleichbar.
pub fn konto_fuer(name: &str) -> Address {
    let h = Hash::sha256(name.as_bytes());
    probekonto(h.as_bytes()[0])
}

/// Was beim Übernehmen eines Blocks schiefgehen kann.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KettenFehler {
    /// Der Block schließt nicht an die eigene Kette an.
    ///
    /// **Der häufigste Fall im Betrieb**, und meistens harmlos: Ein
    /// Knoten, der später dazukommt, hat die früheren Blöcke nie
    /// gesehen.
    PasstNichtAn { erwartet: Hash, bekommen: Hash },
    /// Derselbe Block noch einmal. Gossip verbreitet mehrfach.
    SchonBekannt,
    /// **Die Zustandswurzel weicht ab.**
    ///
    /// Der schwerste Fall und der einzige, der ein Befund ist: Zwei
    /// Maschinen haben aus denselben Blöcken verschiedene Zustände
    /// errechnet. Irgendwo im Ledger-Pfad ist etwas nicht
    /// deterministisch.
    ZustandWeichtAb { erwartet: Hash, errechnet: Hash },
}

impl std::fmt::Display for KettenFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PasstNichtAn { erwartet, bekommen } => write!(
                f,
                "schließt nicht an: erwartet {}, bekommen {}",
                &erwartet.to_hex()[..16],
                &bekommen.to_hex()[..16]
            ),
            Self::SchonBekannt => write!(f, "bereits übernommen"),
            Self::ZustandWeichtAb { erwartet, errechnet } => write!(
                f,
                "Zustandswurzel weicht ab: Block sagt {}, errechnet {}",
                &erwartet.to_hex()[..16],
                &errechnet.to_hex()[..16]
            ),
        }
    }
}

impl std::error::Error for KettenFehler {}

/// Die Kette eines Knotens: Zustand, Höhe, Mempool.
pub struct Kette {
    hoehe: u64,
    letzter_hash: Hash,
    zustand: LedgerState,
    mempool: Vec<Transaction>,
    /// Die Hashes der übernommenen Blöcke, damit Dubletten aus dem
    /// Gossip nicht zweimal angewandt werden.
    bekannt: std::collections::HashSet<Hash>,
    /// Die Blöcke selbst, nach Höhe.
    ///
    /// **Wer nachliefern soll, muss aufheben.** Ohne diesen Speicher
    /// könnte ein Knoten einem Neuling nicht helfen, und der Rückstand
    /// wäre endgültig. Für einen Probelauf im Speicher zu halten ist
    /// vertretbar; ein echtes Netz legt sie auf die Platte, und das
    /// steht als offener Punkt im Fahrplan.
    verlauf: std::collections::BTreeMap<u64, Block>,
}

impl Kette {
    /// Eine frische Probekette.
    ///
    /// **Alle Knoten müssen beim selben Wert beginnen**, sonst weichen
    /// ihre Zustandswurzeln von Anfang an ab, und das sähe aus wie ein
    /// Determinismusfehler. Deshalb ist [`PROBE_STARTWERT`] eine
    /// Konstante und kein Zufallswert.
    ///
    /// **Nicht zu verwechseln mit einem Genesis.** Ein Genesis ist ein
    /// einmaliges Ereignis mit Folgen; das hier entsteht bei jedem
    /// Start neu.
    pub fn probestand() -> Self {
        let mut zustand = LedgerState::genesis(PROBE_CREDIT_PREIS);
        // Die Testkonten ausstatten. Ohne Guthaben scheitert jede
        // Burn-Transaktion, der Zustand bleibt unverändert, und die
        // Übereinstimmung der Wurzeln belegt nichts (siehe
        // [`probekonto`]).
        for n in 0..PROBEKONTEN {
            zustand.account_mut(&probekonto(n)).balance = PROBEGUTHABEN;
        }
        Self {
            hoehe: 0,
            letzter_hash: Hash::sha256(b"myelith-testkette-genesis"),
            zustand,
            mempool: Vec::new(),
            bekannt: std::collections::HashSet::new(),
            verlauf: std::collections::BTreeMap::new(),
        }
    }

    /// Die Höhe, die dieser Knoten selbst führt.
    pub fn hoehe(&self) -> u64 {
        self.hoehe
    }

    /// Der Hash des zuletzt übernommenen Blocks.
    pub fn letzter_hash(&self) -> Hash {
        self.letzter_hash
    }

    /// Die Zustandswurzel: der Wert, der zwischen den Knoten
    /// übereinstimmen muss.
    pub fn zustandswurzel(&self) -> Hash {
        self.zustand.commitment()
    }

    /// Entfernt aus dem Mempool, was in einem Block angekommen ist.
    ///
    /// Vergleich über die serialisierten Bytes: Zwei Transaktionen sind
    /// dieselbe, wenn sie sich gleich schreiben. Ein Feldvergleich wäre
    /// dasselbe mit mehr Code, und bei einem neuen Feld würde er still
    /// falsch.
    ///
    /// **Je Vorkommen eines**, nicht alle gleichen auf einmal:
    /// Dieselbe Transaktion zweimal eingereicht ist zweimal eingereicht,
    /// und wenn ein Block sie einmal enthält, wartet die andere weiter.
    fn streiche_verarbeitete(&mut self, verarbeitet: &[Transaction]) {
        if self.mempool.is_empty() || verarbeitet.is_empty() {
            return;
        }
        let mut zu_streichen: Vec<Vec<u8>> = verarbeitet
            .iter()
            .filter_map(|t| borsh::to_vec(t).ok())
            .collect();
        self.mempool.retain(|t| {
            let Ok(bytes) = borsh::to_vec(t) else {
                return true;
            };
            match zu_streichen.iter().position(|z| z == &bytes) {
                Some(i) => {
                    zu_streichen.swap_remove(i);
                    false
                }
                None => true,
            }
        });
    }

    /// Die Blöcke der Höhen `ab` bis einschließlich `bis`, soweit
    /// vorhanden, aufsteigend.
    ///
    /// Lücken werden **übersprungen, nicht aufgefüllt**: Wer nur einen
    /// Teil hat, liefert diesen Teil. Der Fragende merkt es daran, dass
    /// sein Rückstand nicht ganz verschwindet, und fragt weiter.
    pub fn bloecke_von_bis(&self, ab: u64, bis: u64) -> Vec<Block> {
        self.verlauf
            .range(ab..=bis)
            .map(|(_, b)| b.clone())
            .collect()
    }

    /// Der Ledger-Zustand, für Diagnose und Tests.
    pub fn zustand(&self) -> &LedgerState {
        &self.zustand
    }

    /// Anzahl wartender Transaktionen.
    pub fn wartend(&self) -> usize {
        self.mempool.len()
    }

    /// Nimmt eine Transaktion in den Mempool.
    ///
    /// Ohne Prüfung: Ob sie durchgeht, entscheidet sich beim Anwenden.
    /// Eine Vorprüfung hier bräuchte den Zustand **zum Zeitpunkt der
    /// Aufnahme**, und der ist nicht der beim Anwenden.
    pub fn aufnehmen(&mut self, tx: Transaction) {
        self.mempool.push(tx);
    }

    /// Liest eine Transaktion aus Gossip-Bytes und nimmt sie auf.
    pub fn aufnehmen_roh(&mut self, daten: &[u8]) -> bool {
        let mut rest = daten;
        match Transaction::deserialize(&mut rest) {
            Ok(tx) if rest.is_empty() => {
                self.aufnehmen(tx);
                true
            }
            _ => false,
        }
    }

    /// Wendet Transaktionen auf den Zustand an.
    ///
    /// **Die einzige Stelle, an der das geschieht**, und das ist der
    /// Kern: Erzeuger und Übernehmer müssen dieselbe Folge von
    /// Änderungen ausführen, sonst weichen ihre Zustandswurzeln ab,
    /// ohne dass etwas kaputt wäre. Zwei Fassungen dieser Funktion wären
    /// zwei Quellen für dieselbe Aussage (vgl. Fund 34).
    ///
    /// Gescheiterte Transaktionen werden **übersprungen, nicht
    /// abgebrochen**: Eine Burn-Transaktion ohne Deckung ist kein Grund,
    /// den Block zu verwerfen, und beide Seiten überspringen sie gleich.
    fn anwenden(zustand: &mut LedgerState, txs: &[Transaction], epoch: u64) {
        for tx in txs {
            match tx {
                Transaction::Burn(b) => {
                    // Verfall eine Epoche später: eine Festlegung dieser
                    // Testkette, damit beide Seiten dieselbe rechnen.
                    let _ = burn_to_credits(zustand, &b.sender, b.amount, EpochId(epoch + 1));
                }
            }
        }
    }

    /// Baut den nächsten Block aus dem Mempool und übernimmt ihn selbst.
    ///
    /// Der Erzeuger wendet **zuerst** an und schreibt die daraus
    /// entstandene Zustandswurzel in den Block. Die Empfänger rechnen
    /// dasselbe und vergleichen.
    pub fn baue_block(&mut self) -> Block {
        let txs: Vec<Transaction> = std::mem::take(&mut self.mempool);
        let epoch = self.hoehe + 1;

        Self::anwenden(&mut self.zustand, &txs, epoch);

        let mut block = Block::new(EpochMeta {
            epoch,
            prev_block_hash: self.letzter_hash,
            timestamp_ms: crate::protokoll::jetzt_ms().max(0) as u64,
            state_root: self.zustand.commitment(),
        });
        for tx in txs {
            block.add_transaction(tx);
        }

        self.hoehe = epoch;
        self.letzter_hash = block.hash();
        self.bekannt.insert(self.letzter_hash);
        self.verlauf.insert(epoch, block.clone());
        block
    }

    /// Übernimmt einen fremden Block und rechnet nach.
    ///
    /// Die Reihenfolge der Prüfungen ist bedeutsam: **Erst anschließen,
    /// dann rechnen.** Ein Block, der nicht anschließt, darf den Zustand
    /// nicht anfassen, sonst hinterlässt ein verirrter Block eine Spur.
    pub fn uebernimm(&mut self, block: &Block) -> Result<(), KettenFehler> {
        let hash = block.hash();
        if self.bekannt.contains(&hash) {
            return Err(KettenFehler::SchonBekannt);
        }
        if block.epoch_meta.prev_block_hash != self.letzter_hash {
            return Err(KettenFehler::PasstNichtAn {
                erwartet: self.letzter_hash,
                bekommen: block.epoch_meta.prev_block_hash,
            });
        }

        // Auf einer Kopie rechnen: Weicht das Ergebnis ab, bleibt der
        // eigene Zustand unberührt. Ein Knoten, der einem abweichenden
        // Block folgt, hätte den Befund verschluckt.
        let mut versuch = self.zustand.clone();
        Self::anwenden(&mut versuch, &block.txs, block.epoch_meta.epoch);
        let errechnet = versuch.commitment();
        if errechnet != block.epoch_meta.state_root {
            return Err(KettenFehler::ZustandWeichtAb {
                erwartet: block.epoch_meta.state_root,
                errechnet,
            });
        }

        self.zustand = versuch;
        self.hoehe = block.epoch_meta.epoch;
        self.letzter_hash = hash;
        self.bekannt.insert(hash);
        self.verlauf.insert(block.epoch_meta.epoch, block.clone());
        // ⚑ Was der Block enthält, wartet nicht mehr.
        //
        // Ohne diese Zeile wächst der Mempool eines Knotens, der selbst
        // keine Blöcke baut, **für immer**: Er nimmt jede Transaktion
        // aus dem Gossip auf und leert nie. Gemessen im
        // Dreiknoten-Probelauf: 0, 0, 3, 4 wartende Einträge über
        // vierzig Sekunden, und das ohne Ende.
        //
        // Zwei Folgen, und die zweite ist die schlimmere: Die Zahl
        // `wartend` im Protokoll wird bedeutungslos, **und ein solcher
        // Knoten baute, sobald er je Erzeuger würde, einen Block aus
        // tausenden längst verarbeiteter Transaktionen.**
        self.streiche_verarbeitete(&block.txs);
        Ok(())
    }
}

impl Default for Kette {
    fn default() -> Self {
        Self::probestand()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_consensus::block::BurnTx;

    /// Ein **wirksamer** Burn: Absender ist ein ausgestattetes
    /// Testkonto. Ein Burn von einem leeren Konto scheitert still, und
    /// ein Test damit belegte nichts.
    fn burn(wer: u8, betrag: u64) -> Transaction {
        Transaction::Burn(BurnTx {
            sender: probekonto(wer),
            amount: betrag,
        })
    }

    /// **Der Zustand ändert sich, wenn Transaktionen wirken.**
    ///
    /// Der Test, der beim ersten echten Dreiknotenlauf gefehlt hat.
    /// Damals stand die Zustandswurzel bei jeder Höhe auf demselben
    /// Wert: Die Burns scheiterten an fehlender Deckung, wurden
    /// übersprungen, und die Übereinstimmung der Wurzeln belegte nichts.
    /// **Ein Lauf, dessen Zustand sich nie ändert, misst nichts.**
    #[test]
    fn ein_wirksamer_burn_veraendert_die_zustandswurzel() {
        let mut k = Kette::probestand();
        let vorher = k.zustandswurzel();
        k.aufnehmen(Transaction::Burn(BurnTx {
            sender: probekonto(0),
            amount: 5_000,
        }));
        let _ = k.baue_block();
        assert_ne!(
            k.zustandswurzel(),
            vorher,
            "die Zustandswurzel hat sich nicht bewegt: die Transaktion war wirkungslos"
        );
    }

    #[test]
    fn die_testkonten_haben_zu_genesis_guthaben() {
        let k = Kette::probestand();
        for n in 0..PROBEKONTEN {
            assert_eq!(
                k.zustand().account(&probekonto(n)).balance,
                PROBEGUTHABEN,
                "Testkonto {n} ohne Guthaben"
            );
        }
    }

    #[test]
    fn das_konto_eines_knotens_bleibt_stabil() {
        // Ein wechselndes Konto machte zwei Läufe unvergleichbar.
        assert_eq!(konto_fuer("alpha"), konto_fuer("alpha"));
        assert!(
            (0..PROBEKONTEN).any(|n| probekonto(n) == konto_fuer("alpha")),
            "das Konto liegt außerhalb der ausgestatteten"
        );
    }

    /// Zwei Ketten aus demselben Genesis sind gleich.
    #[test]
    fn zwei_frische_ketten_stimmen_ueberein() {
        // Wäre das nicht so, sähe jeder Lauf wie ein
        // Determinismusfehler aus, bevor überhaupt etwas passiert ist.
        let a = Kette::probestand();
        let b = Kette::probestand();
        assert_eq!(a.zustandswurzel(), b.zustandswurzel());
        assert_eq!(a.letzter_hash(), b.letzter_hash());
        assert_eq!(a.hoehe(), 0);
    }

    /// **Der Kern: Erzeuger und Übernehmer landen beim selben Zustand.**
    #[test]
    fn erzeuger_und_uebernehmer_kommen_zum_selben_zustand() {
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();

        erzeuger.aufnehmen(burn(1, 1_000));
        erzeuger.aufnehmen(burn(2, 2_500));
        let block = erzeuger.baue_block();

        folger.uebernimm(&block).expect("Block muss anschließen");
        assert_eq!(
            erzeuger.zustandswurzel(),
            folger.zustandswurzel(),
            "die Zustandswurzeln weichen ab"
        );
        assert_eq!(erzeuger.hoehe(), folger.hoehe());
        assert_eq!(erzeuger.letzter_hash(), folger.letzter_hash());
    }

    /// Mehrere Blöcke hintereinander bleiben gleich.
    #[test]
    fn eine_kette_aus_mehreren_bloecken_bleibt_gleich() {
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        for i in 1..=10u64 {
            erzeuger.aufnehmen(burn((i % 5) as u8 + 1, i * 300));
            let b = erzeuger.baue_block();
            folger.uebernimm(&b).expect("schließt an");
        }
        assert_eq!(erzeuger.hoehe(), 10);
        assert_eq!(erzeuger.zustandswurzel(), folger.zustandswurzel());
    }

    /// Ein Block, der nicht anschließt, lässt den Zustand unberührt.
    #[test]
    fn ein_verirrter_block_hinterlaesst_keine_spur() {
        let mut a = Kette::probestand();
        let mut fremd = Kette::probestand();
        fremd.aufnehmen(burn(9, 5_000));
        let _ = fremd.baue_block();
        fremd.aufnehmen(burn(9, 5_000));
        let zweiter = fremd.baue_block(); // schließt an fremds ersten an

        let vorher = a.zustandswurzel();
        let fehler = a.uebernimm(&zweiter).expect_err("darf nicht anschließen");
        assert!(matches!(fehler, KettenFehler::PasstNichtAn { .. }));
        assert_eq!(a.zustandswurzel(), vorher, "der Zustand wurde angefasst");
        assert_eq!(a.hoehe(), 0);
    }

    /// **Eine abweichende Zustandswurzel wird erkannt und nicht
    /// übernommen.**
    ///
    /// Der Fall, für den es diesen Aufbau gibt. Ein Knoten, der einem
    /// abweichenden Block folgte, hätte den Befund verschluckt.
    #[test]
    fn eine_abweichende_zustandswurzel_wird_zurueckgewiesen() {
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        erzeuger.aufnehmen(burn(1, 1_000));
        let mut block = erzeuger.baue_block();

        // Jemand fälscht die Wurzel.
        block.epoch_meta.state_root = Hash::sha256(b"etwas anderes");

        let vorher = folger.zustandswurzel();
        let fehler = folger.uebernimm(&block).expect_err("muss auffallen");
        assert!(matches!(fehler, KettenFehler::ZustandWeichtAb { .. }));
        assert_eq!(folger.zustandswurzel(), vorher, "Zustand wurde übernommen");
    }

    /// Gossip verbreitet mehrfach; derselbe Block darf nicht zweimal wirken.
    #[test]
    fn ein_doppelt_empfangener_block_wirkt_nur_einmal() {
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        erzeuger.aufnehmen(burn(1, 1_000));
        let block = erzeuger.baue_block();

        folger.uebernimm(&block).expect("erstes Mal");
        let nach_erstem = folger.zustandswurzel();
        assert_eq!(
            folger.uebernimm(&block),
            Err(KettenFehler::SchonBekannt),
            "die Dublette wurde nicht erkannt"
        );
        assert_eq!(folger.zustandswurzel(), nach_erstem);
        assert_eq!(folger.hoehe(), 1);
    }

    /// Eine Transaktion ohne Deckung bricht den Block nicht ab.
    #[test]
    fn eine_ungedeckte_transaktion_wird_uebersprungen_nicht_verworfen() {
        // Beide Seiten überspringen gleich, also stimmen die Wurzeln
        // trotzdem. Bräche der Erzeuger ab und der Folger nicht, wäre
        // genau das die Abweichung.
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        erzeuger.aufnehmen(burn(7, u64::MAX)); // mehr als jedes Konto hält
        let block = erzeuger.baue_block();
        folger.uebernimm(&block).expect("schließt an");
        assert_eq!(erzeuger.zustandswurzel(), folger.zustandswurzel());
    }

    /// **Ein Neuling holt über den Verlauf auf.**
    ///
    /// Der Fall, für den es [`crate::nachschub`] gibt: Beta verpasst
    /// die ersten Blöcke, bekommt sie nachgeliefert und steht danach
    /// beim selben Zustand wie Alpha.
    #[test]
    fn ein_nachzuegler_holt_ueber_den_verlauf_auf() {
        let mut erzeuger = Kette::probestand();
        for i in 1..=5u64 {
            erzeuger.aufnehmen(burn((i % 4) as u8, i * 400));
            let _ = erzeuger.baue_block();
        }
        // Der Neuling war die ganze Zeit nicht da.
        let mut neuling = Kette::probestand();
        assert_eq!(neuling.hoehe(), 0);

        // Er bekommt den Verlauf und wendet ihn ganz normal an.
        for b in erzeuger.bloecke_von_bis(1, 5) {
            neuling.uebernimm(&b).expect("nachgelieferter Block schließt an");
        }
        assert_eq!(neuling.hoehe(), 5);
        assert_eq!(
            neuling.zustandswurzel(),
            erzeuger.zustandswurzel(),
            "der Nachzügler steht bei einem anderen Zustand"
        );
    }

    /// Nachlieferung ist ein Transportweg, **kein Vertrauensweg**.
    #[test]
    fn ein_nachgelieferter_block_mit_falscher_wurzel_wird_genauso_abgewiesen() {
        // Wäre die Nachlieferung ein zweiter, schwächerer Weg in die
        // Kette, wäre sie das Loch: Wer einen Knoten zum Nachfordern
        // bringt, bekäme einen Block hineingelegt, ohne dass er
        // nachrechnet.
        let mut erzeuger = Kette::probestand();
        erzeuger.aufnehmen(burn(1, 900));
        let _ = erzeuger.baue_block();
        let mut geliefert = erzeuger.bloecke_von_bis(1, 1);
        geliefert[0].epoch_meta.state_root = Hash::sha256(b"untergeschoben");

        let mut neuling = Kette::probestand();
        let fehler = neuling.uebernimm(&geliefert[0]).expect_err("muss auffallen");
        assert!(matches!(fehler, KettenFehler::ZustandWeichtAb { .. }));
        assert_eq!(neuling.hoehe(), 0);
    }

    #[test]
    fn der_verlauf_liefert_nur_was_da_ist() {
        // Lücken werden übersprungen, nicht aufgefüllt. Der Fragende
        // merkt es daran, dass sein Rückstand nicht ganz verschwindet.
        let mut k = Kette::probestand();
        for _ in 0..3 {
            let _ = k.baue_block();
        }
        assert_eq!(k.bloecke_von_bis(1, 3).len(), 3);
        assert_eq!(k.bloecke_von_bis(1, 99).len(), 3, "nicht Vorhandenes wird nicht erfunden");
        assert!(k.bloecke_von_bis(7, 9).is_empty());
    }

    /// **Der Mempool eines Nicht-Erzeugers wächst nicht ins Unendliche.**
    ///
    /// Der Fund aus dem Dreiknoten-Probelauf: Beta nahm jede
    /// Transaktion aus dem Gossip auf und leerte nie, weil es selbst
    /// keine Blöcke baut.
    #[test]
    fn was_in_einem_block_ankommt_wartet_nicht_mehr() {
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();

        // Beide sehen dieselben Transaktionen aus dem Gossip.
        for i in 1..=3u64 {
            let t = burn((i % 4) as u8, i * 500);
            erzeuger.aufnehmen(t.clone());
            folger.aufnehmen(t);
        }
        assert_eq!(folger.wartend(), 3);

        let block = erzeuger.baue_block();
        folger.uebernimm(&block).expect("schließt an");
        assert_eq!(
            folger.wartend(),
            0,
            "der Mempool wurde nicht geleert: er wüchse für immer"
        );
    }

    #[test]
    fn was_nicht_im_block_stand_wartet_weiter() {
        // Sonst verschwänden Transaktionen, die noch niemand
        // verarbeitet hat.
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        let drin = burn(1, 700);
        erzeuger.aufnehmen(drin.clone());
        folger.aufnehmen(drin);
        folger.aufnehmen(burn(2, 800)); // kennt der Erzeuger nicht

        let block = erzeuger.baue_block();
        folger.uebernimm(&block).expect("schließt an");
        assert_eq!(folger.wartend(), 1, "die unverarbeitete Transaktion ist weg");
    }

    #[test]
    fn eine_doppelt_eingereichte_transaktion_verschwindet_nur_einmal() {
        // Dieselbe Transaktion zweimal eingereicht ist zweimal
        // eingereicht. Enthält ein Block sie einmal, wartet die andere.
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        let t = burn(3, 600);
        erzeuger.aufnehmen(t.clone());
        folger.aufnehmen(t.clone());
        folger.aufnehmen(t);
        assert_eq!(folger.wartend(), 2);

        let block = erzeuger.baue_block();
        assert_eq!(block.txs.len(), 1);
        folger.uebernimm(&block).expect("schließt an");
        assert_eq!(folger.wartend(), 1);
    }

    /// Ein leerer Block ist gültig und bewegt die Kette weiter.
    #[test]
    fn ein_leerer_block_bewegt_die_kette() {
        let mut erzeuger = Kette::probestand();
        let mut folger = Kette::probestand();
        let block = erzeuger.baue_block();
        assert!(block.txs.is_empty());
        folger.uebernimm(&block).expect("auch leer schließt an");
        assert_eq!(folger.hoehe(), 1);
        assert_eq!(erzeuger.letzter_hash(), folger.letzter_hash());
    }

    /// Der Mempool wird beim Bauen geleert.
    #[test]
    fn der_mempool_wird_beim_bauen_geleert() {
        // Sonst stünde dieselbe Transaktion in jedem folgenden Block.
        let mut k = Kette::probestand();
        k.aufnehmen(burn(1, 500));
        assert_eq!(k.wartend(), 1);
        let b = k.baue_block();
        assert_eq!(b.txs.len(), 1);
        assert_eq!(k.wartend(), 0);
        assert!(k.baue_block().txs.is_empty());
    }

    /// Rohe Gossip-Bytes werden gelesen, Unsinn nicht.
    #[test]
    fn rohe_transaktionen_werden_gelesen_unsinn_nicht() {
        let mut k = Kette::probestand();
        let gut = borsh::to_vec(&burn(3, 900)).unwrap();
        assert!(k.aufnehmen_roh(&gut));
        assert_eq!(k.wartend(), 1);
        assert!(!k.aufnehmen_roh(&[0xFF; 7]));
        // Anhängsel: dieselbe Transaktion mit anderem Rahmen.
        let mut mit_anhang = gut.clone();
        mit_anhang.push(0);
        assert!(!k.aufnehmen_roh(&mit_anhang));
        assert_eq!(k.wartend(), 1);
    }
}
