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
//! # ⚑ Höhe und Epoche sind zwei Dinge (seit 2026-08-27)
//!
//! [`BlockHeader`] trägt beide: `height` ist die Stellung in der Kette
//! und wächst um genau eins je Block, `epoch` ist die Epoche und folgt
//! aus der Höhe ([`epoche_fuer_hoehe`]). Bei 1 800 Blöcken je Epoche
//! sind das eine Stunde Blockzeit und 1 800 Höhen.
//!
//! **Bis dahin war es ein Feld für beides.** Die Probekette schrieb ihre
//! Höhe in `epoch`, und das trug, solange eine Epoche ein Block war.
//! Jede Frist „je Epoche" bedeutete damit in Wahrheit „je Block": Der
//! Verfall von Credits nach einer Epoche war der Verfall nach einem
//! Block, und die Streitfrist von 168 Epochen wären 168 Blöcke gewesen,
//! also gut fünf Minuten statt sieben Tagen.
//!
//! Beide Felder werden beim Übernehmen geprüft: die Höhe gegen die
//! eigene, die Epoche gegen die Umrechnung. Ein mitgeführter Wert, den
//! niemand nachrechnet, ist ein Feld, das jeder setzen darf.

use borsh::BorshDeserialize;
use myl_consensus::block::{epoche_fuer_hoehe, Anweisung, Block, BlockHeader, Transaktion, BLOECKE_JE_EPOCHE};
use myl_ledger::state::LedgerState;
use myl_ledger::transitions::{
    burn_to_credits, nonce_verbrauchen, sitzung_ausgeben, sitzung_eroeffnen, sitzung_widerrufen,
    transfer, miner_abmelden, miner_anmelden, buendel_einreichen, buendel_leeren, angemeldete_miner, buendel_der_epoche,
    einsatz_hinterlegen, einsatz_kuendigen, einsatz_abholen};
use myl_types::hash::Hash;
use myl_types::ids::{Address, EpochId};

/// Credit-Preis zu Genesis der Testkette.
///
/// Nicht null: Ein Preis von null wäre eine Division durch null im
/// Ledger und ist dort als Protokollfehler ausgewiesen.
pub const PROBE_CREDIT_PREIS: u64 = 100;

/// Zahl der Probekonten, die zu Beginn Guthaben bekommen.
///
/// # ⚑ Zwölf seit dem 2026-09-05, und die Zahl ist begründet
///
/// Vorher acht. Ein Pod hat `PROBE_SHARDS + 2` Mitglieder, also sechs;
/// acht Konten ergeben damit **einen** Pod, und ein einzelner Pod kann
/// nicht trainieren: Trainingssegmente werden paarweise vergeben, weil
/// ein Ergebnis, das nur einer gerechnet hat, sich mit nichts
/// vergleichen lässt.
///
/// **Zwölf ist das Minimum eines Netzes, das trainieren kann:** zwei
/// volle Pods, ein Redundanzpaar. Mehr wäre Ballast, weniger machte den
/// Trainingsweg in der Probekette unerreichbar, und ein Weg, den kein
/// Test geht, ist keiner.
pub const PROBEKONTEN: u8 = 12;

/// Guthaben je Probekonto in MYL-Kleinstbeträgen. **Spielgeld.**
pub const PROBEGUTHABEN: u64 = 10_000_000;

/// Wie viele Blöcke der Knoten im Arbeitsspeicher hält.
///
/// ⚑ **Hergeleitet, nicht gegriffen.** Der Zwischenspeicher hat genau
/// eine Aufgabe: eine Nachforderung beantworten, ohne die Platte
/// anzufassen. Eine Nachforderung umfasste bis zum 2026-10-22 höchstens
/// 64 Blöcke, also deckte ein Fenster von vier Lieferungen den Fall ab,
/// dass mehrere Nachzügler gleichzeitig und an verschiedenen Stellen
/// aufholen.
///
/// ⚑ **Seit die Lieferung 512 Blöcke fasst, steht die Zahl fest hier**:
/// Ein Fenster von vier Lieferungen wären 2048 Blöcke im Speicher, und
/// was älter ist als das Fenster, liest der Knoten ohnehin von der Platte.
///
/// **Vier Lieferungen sind 256, und dieselbe Zahl steht in gereiften
/// Kettenknoten** als Deckel ihrer Blockzwischenspeicher. Zwei
/// verschiedene Herleitungen, dieselbe Größenordnung: Das ist ein
/// Hinweis, kein Beweis, aber ein beruhigender.
///
/// **Die Obergrenze im Speicher** ist damit benannt und nicht mehr
/// offen: 256 mal [`crate::speicher::MAX_SATZ_BYTES`] im schlimmsten
/// Fall, im Betrieb um Größenordnungen weniger.
pub const VERLAUFSFENSTER: usize = 256;

/// ⚑ **Trägt das Vertrauen noch?** (2026-10-27, CONSENSUS 7.5): ja, solange
/// die Epoche des Netzes höchstens
/// [`myl_ledger::einsatz::VERTRAUENSFRIST_EPOCHEN`] über der des zuletzt
/// bestätigten eigenen Prüfpunkts liegt. Darüber haftet womöglich niemand
/// mehr, der damals unterschrieben hat.
pub fn vertrauen_traegt(bestaetigt_epoche: u64, netz_epoche: u64) -> bool {
    netz_epoche.saturating_sub(bestaetigt_epoche) <= myl_ledger::einsatz::VERTRAUENSFRIST_EPOCHEN
}

/// Höchstens so viele Zuteilungen abgelaufener Epochen hebt die Kette für
/// ihre Ziehung auf (2026-10-27); länger hängt keine Abstimmung, ohne dass
/// die Ziehung ohnehin verloren wäre.
const OFFENE_ZUTEILUNGEN_HOECHSTENS: usize = 4;

/// ⚑ **Wie viele Blöcke sich höchstens zurücknehmen lassen** (Zurückrollen
/// bis zum endgültigen Prüfpunkt, 2026-11-04): vier Prüfpunkte. Gewöhnlich
/// liegt der endgültige zwei Prüfpunkte zurück; ruht die Endgültigkeit
/// länger, fallen die ältesten Rückwege weg, und tiefer wird nicht
/// gewechselt, statt dass der Speicher wächst.
pub const RUECKWEG_HOECHSTENS: usize = 4 * myl_consensus::pruefpunkt::BLOECKE_JE_PRUEFPUNKT as usize;

/// Wie viele Slots [`Kette::baue_block`] nach dem eigenen sucht.
pub const SLOTS_GESUCHT: u64 = 4_096;

/// ⚑ Was ein angewandter Block braucht, um zurückgenommen zu werden.
struct Rueckspur {
    hoehe: u64,
    /// Der Hash des Vorgängers.
    davor: Hash,
    /// Der Block selbst: seine Transaktionen gehen zurück in den Mempool.
    block: Block,
    weg: myl_ledger::state::Rueckweg,
    /// Was die Kette neben dem Zustand aus ihm ableitete (Zuteilungen,
    /// Ziehung), vor dem Block; nur, wenn er daran etwas änderte.
    neben: Option<Nebenstand>,
}

/// Was die Kette an einer Epochengrenze oder Ziehung neben dem Zustand
/// fortschreibt.
struct Nebenstand {
    offene_zuteilungen: Vec<(u64, myl_scheduler::shard_assignment::Zuteilung)>,
    letzte_stichprobe: Option<(u64, Vec<crate::stichprobe::Segmentstichprobe>)>,
    pods_der_stichprobe: Vec<myl_scheduler::shard_assignment::Pod>,
    pruefer_der_stichprobe: Vec<(myl_types::ids::PodId, u32, myl_types::ids::MinerId)>,
}

/// **Wie lange eine Transaktion im Mempool wartet**, in eigenen Höhen
/// (Fund 627): lange genug, dass eine überholte Vorgängerin nachkommt,
/// kurz genug, dass Unsinn mit erfundener Nummer nicht liegen bleibt.
///
/// 📌 **256 seit dem 2026-10-29** (Skalierung): Mit 64 verfielen unter
/// Rückstau angenommene Transaktionen (bei 60 ms Takt nach knapp 4 s, im
/// Durchsatzlauf 97 bei 3000/s), bevor der volle Mempool Gegendruck gab.
/// Unsinn begrenzt die Platzzahl ([`MEMPOOL_HOECHSTENS`], [`Mempool`]).
pub const WARTEN_HOECHSTENS_HOEHEN: u64 = 256;

/// **Höchstens so viele Transaktionen warten** (Fund 627); was darüber
/// kommt, wird nicht aufgenommen.
///
/// ⚑ **16 384 seit dem 2026-10-29** (Skalierung): bei Blöcken mit rund 1000
/// Transaktionen ein Puffer von gut einer Sekunde Spitzenlast. Ist er voll,
/// verdrängt eine neue Transaktion die billigsten Enden ([`Mempool`]).
pub const MEMPOOL_HOECHSTENS: usize = 16_384;

/// ⚑ **Wie viel ein Erzeuger an Zeugen bereithält** (zustandslose
/// Validierung, 2026-10-29), unterschrieben und in Stücken: Bei einer
/// Million Konten und vollen Blöcken rund 1,7 MB je Block, also etwa 40
/// Blöcke; ein zustandsloser Knoten, der weiter zurückfällt, setzt neu auf.
pub const ZEUGEN_HOECHSTENS_BYTES: usize = 64 << 20;

/// So viele Zeugen hält ein Erzeuger roh, bis er sie zerlegt.
const ZEUGEN_ROH_GEHALTEN: usize = 8;


/// ⚑ **So viele Bytes an Transaktionen trägt ein Block höchstens** (Fund 633):
/// die Hälfte dessen, was Gossip an einem Block annimmt
/// (`myl_net::validation::MAX_BLOCKS_BYTES`), damit Kopf, Rahmen und
/// Saatquelle Platz haben. Bis zum 2026-10-24 ging jede gültige Transaktion
/// in den nächsten Block; rund achtzig Archivbelege zu je 27 KB von einem
/// beliebigen Konto ergaben einen Block, den kein Folger je bekam, und die
/// Kette stand für alle. Was nicht passt, wartet auf den nächsten Block.
pub const BLOCK_TXS_HOECHSTENS_BYTES: usize = myl_net::validation::MAX_BLOCKS_BYTES / 2;

/// So viele Transaktionen eines Absenders hält der Mempool höchstens
/// (Fund 633): Sonst füllt ein einziges Konto alle [`MEMPOOL_HOECHSTENS`]
/// Plätze, und jede weitere Transaktion jedes anderen wird abgewiesen.
pub const MEMPOOL_JE_ABSENDER: usize = 64;

/// ⚑ **Neulinge** (Fund 635): Absender ohne Guthaben, ohne Einsatz und ohne
/// Anmeldung als Miner. Bis zu den Gebühren (2026-10-27, G1) kostete eine
/// Transaktion nichts, und jede legte im Zustand ein Konto an; mit frischen
/// Schlüsseln liess sich so der Mempool füllen und der Zustand ohne Grenze
/// aufblähen. Seither kommt ein Neuling in keinen Block, weil er die Gebühr
/// nicht zahlen kann; die Drossel bleibt für den Mempool, in dem er bis zum
/// Verfall wartet: höchstens so viele zugleich in den Mempool ...
pub const MEMPOOL_NEULINGE_HOECHSTENS: usize = 32;
/// ... und höchstens so viele in einen Block. Ein neuer Knoten braucht eine
/// einzige Transaktion, die Anmeldung; danach ist er kein Neuling mehr.
pub const BLOCK_NEULINGE_HOECHSTENS: usize = 16;

/// ⚑ **Das Ziel des Grundpreises in Gebühreneinheiten** (2026-10-28,
/// Festlegung des Projektinhabers): rund 500 Transaktionen je Block (je
/// Prüfeinheit plus rund 200 Bytes). Ist ein Block voller, steigt der Preis,
/// ist er leerer, sinkt er, je Block um höchstens ein Achtel. Bis zum
/// 2026-10-28 ein halber Block in Bytes, rund 3 000 Transaktionen, weit über
/// dem, was ein Knoten prüft. ⚠️ Vorläufig bis zur Messung auf der
/// kleinsten zugelassenen Maschine.
pub const GRUNDPREIS_ZIEL_EINHEITEN: u64 = 500 * (myl_consensus::block::PRUEFEINHEIT + 200);

/// Höchstens so viele Gebühreneinheiten nimmt der Erzeuger in einen Block:
/// das Doppelte des Ziels.
pub const BLOCK_EINHEITEN_HOECHSTENS: u64 = 2 * GRUNDPREIS_ZIEL_EINHEITEN;

/// Um höchstens den so vielten Teil wandert der Grundpreis je Block.
pub const GRUNDPREIS_SCHRITT_TEILER: u64 = 8;

/// ⚑ **Die Teilung der Gebühren** (Festlegung des Projektinhabers): von
/// dreizehn Teilen drei an die Treasury, zehn an die Validatoren.
pub const GEBUEHR_TEILE: u64 = 13;
/// Der Anteil der Treasury in [`GEBUEHR_TEILE`].
pub const GEBUEHR_TREASURY: u64 = 3;

/// Die Arten einer Hinterlegung (`LedgerState::hinterlegungen`).
pub const HINTERLEGT_ANMELDUNG: u8 = 0;
/// Siehe [`HINTERLEGT_ANMELDUNG`].
pub const HINTERLEGT_SITZUNG: u8 = 1;

/// Der Grundpreis des nächsten Blocks aus dem jetzigen und den Bytes, die
/// dieser Block an Gebühren gezählt hat.
///
/// `preis · (bytes − ziel) / ziel / 8`. ⚑ **Mindestens eins
/// nach oben**, wenn der Block über dem Ziel lag; sonst bliebe ein Preis
/// von eins für immer bei eins, weil ein Achtel davon null ist. Nie unter
/// dem Boden.
pub fn naechster_grundpreis(preis: u64, bytes: u64, boden: u64) -> u64 {
    let ziel = GRUNDPREIS_ZIEL_EINHEITEN as u128;
    let p = preis as u128;
    let neu = if bytes as u128 > ziel {
        let schritt = p * (bytes as u128 - ziel) / ziel / GRUNDPREIS_SCHRITT_TEILER as u128;
        p + schritt.max(1)
    } else {
        p - p * (ziel - bytes as u128) / ziel / GRUNDPREIS_SCHRITT_TEILER as u128
    };
    u64::try_from(neu).unwrap_or(u64::MAX).max(boden)
}

/// Eine wartende Transaktion im Mempool.
///
/// 📌 **Mit ihrem Absender, einmal berechnet** (Fund 668, 2026-10-28): Die
/// Adresse ist ein SHA-256 über den Schlüssel. `aufnehmen` rechnete sie für
/// jeden wartenden Eintrag neu, um je Absender zu zählen, also über eine
/// Füllung quadratisch: gemessen 27 µs je Aufnahme bei 512 wartenden, 413 µs
/// bei 4 096, so viel wie eine Prüfung der Unterschrift.
struct Wartend {
    /// Die eigene Höhe bei der Aufnahme.
    seit: u64,
    tx: Transaktion,
    absender: Address,
    /// Die kodierte Länge, einmal bei der Aufnahme.
    bytes: usize,
    /// Ob der Absender bei der Aufnahme ein Neuling war.
    neuling: bool,
}

impl Wartend {
    /// Der gebotene Preis je Gebühreneinheit, als Bruch (Zähler, Nenner).
    fn preis(&self) -> (u128, u128) {
        (u128::from(self.tx.max_gebuehr), u128::from((self.bytes as u64).saturating_add(myl_consensus::block::PRUEFEINHEIT)))
    }

    fn billiger_als(&self, andere: &Wartend) -> bool {
        let ((a, an), (b, bn)) = (self.preis(), andere.preis());
        a * bn < b * an
    }
}

/// **Die schon geprüften Unterschriften** (Skalierung, 2026-10-29): je
/// Kennung einer Transaktion, die dieser Knoten bei der Aufnahme geprüft
/// hat; höchstens doppelt so viele wie der Mempool Plätze hat, die ältesten
/// fallen zuerst.
#[derive(Default)]
struct Unterschriften {
    menge: std::collections::HashSet<Hash>,
    folge: std::collections::VecDeque<Hash>,
}

impl Unterschriften {
    fn merken(&mut self, h: Hash) {
        if self.menge.insert(h) {
            self.folge.push_back(h);
        }
        while self.folge.len() > 2 * MEMPOOL_HOECHSTENS {
            if let Some(alt) = self.folge.pop_front() {
                self.menge.remove(&alt);
            }
        }
    }

    fn kennt(&self, h: &Hash) -> bool {
        self.menge.contains(h)
    }
}

/// ⚑ **Der Mempool** (Skalierung, 2026-10-29): die Wartenden in der Folge
/// des Eingangs, dazu die Zähler je Absender und für Neulinge, damit die
/// Aufnahme nicht jedes Mal über alle Wartenden zählt.
///
/// ⚑ **Verdrängt wird nach dem Preis, gewählt nach Eingang.** Ein Trinkgeld
/// gibt es nicht (abgebucht wird Grundpreis mal Einheiten, `max_gebuehr` ist
/// die Obergrenze); eine Reihenfolge nach Gebot belohnte also nur, wer
/// `max_gebuehr` hochsetzt. Bei Andrang steigt der Grundpreis, und was ihn
/// nicht mehr bezahlen kann, bietet am wenigsten: Ist der Mempool voll,
/// fallen die billigsten Enden der Nummernketten (nie aus der Mitte, sonst
/// entstünde eine Lücke), und nur solche, die weniger bieten als die neue.
struct Mempool {
    liste: Vec<Wartend>,
    je_absender: std::collections::HashMap<Address, usize>,
    neulinge: usize,
    /// Die Zahl der Plätze, Vorgabe [`MEMPOOL_HOECHSTENS`].
    plaetze: usize,
}

impl Default for Mempool {
    fn default() -> Self {
        Self { liste: Vec::new(), je_absender: Default::default(), neulinge: 0, plaetze: MEMPOOL_HOECHSTENS }
    }
}

impl Mempool {
    fn voll(&self) -> bool {
        self.liste.len() >= self.plaetze
    }

    fn len(&self) -> usize {
        self.liste.len()
    }

    fn je_absender(&self, a: &Address) -> usize {
        self.je_absender.get(a).copied().unwrap_or(0)
    }

    /// Die Absender der wartenden Transaktionen.
    fn absender(&self) -> impl Iterator<Item = Address> + '_ {
        self.je_absender.keys().copied()
    }

    fn hinein(&mut self, w: Wartend) {
        *self.je_absender.entry(w.absender).or_insert(0) += 1;
        self.neulinge += usize::from(w.neuling);
        self.liste.push(w);
    }

    fn alle_nehmen(&mut self) -> Vec<Wartend> {
        self.je_absender.clear();
        self.neulinge = 0;
        std::mem::take(&mut self.liste)
    }

    fn behalten(&mut self, f: impl FnMut(&Wartend) -> bool) {
        let liste = std::mem::take(&mut self.liste).into_iter().filter(f).collect::<Vec<_>>();
        self.alle_nehmen();
        for w in liste {
            self.hinein(w);
        }
    }

    /// Macht Platz für eine Transaktion von `absender` mit `nonce`, wenn
    /// dessen Kontingent voll ist: Seine höchste Nummer fällt, wenn sie über
    /// `nonce` liegt und `nonce` noch nicht wartet. `true`, wenn Platz ist.
    fn ende_tauschen(&mut self, absender: &Address, nonce: u64) -> bool {
        let mut ende: Option<usize> = None;
        for (i, w) in self.liste.iter().enumerate() {
            if w.absender != *absender {
                continue;
            }
            if w.tx.nonce == nonce {
                return false;
            }
            if ende.is_none_or(|e| self.liste[e].tx.nonce < w.tx.nonce) {
                ende = Some(i);
            }
        }
        let Some(i) = ende.filter(|&i| self.liste[i].tx.nonce > nonce) else {
            return false;
        };
        let w = self.liste.remove(i);
        if let Some(n) = self.je_absender.get_mut(&w.absender) {
            *n = n.saturating_sub(1);
        }
        self.neulinge -= usize::from(w.neuling);
        true
    }

    /// Macht Platz für `neu`: höchstens ein Vierundsechzigstel der Plätze
    /// auf einmal, je die billigsten Enden einer Nummernkette, die weniger
    /// bieten als `neu`. `true`, wenn danach Platz ist.
    fn verdraengen(&mut self, neu: &Wartend) -> bool {
        let mut enden: std::collections::HashMap<Address, usize> = std::collections::HashMap::new();
        for (i, w) in self.liste.iter().enumerate() {
            let e = enden.entry(w.absender).or_insert(i);
            if self.liste[*e].tx.nonce < w.tx.nonce {
                *e = i;
            }
        }
        let mut kandidaten: Vec<usize> = enden.into_values().filter(|i| self.liste[*i].billiger_als(neu)).collect();
        kandidaten.sort_by(|a, b| {
            let (x, y) = (&self.liste[*a], &self.liste[*b]);
            if x.billiger_als(y) {
                std::cmp::Ordering::Less
            } else if y.billiger_als(x) {
                std::cmp::Ordering::Greater
            } else {
                // Bei gleichem Preis fällt die jüngere.
                y.seit.cmp(&x.seit)
            }
        });
        kandidaten.truncate((self.plaetze / 64).max(1));
        if kandidaten.is_empty() {
            return false;
        }
        let raus: std::collections::HashSet<usize> = kandidaten.into_iter().collect();
        let liste = std::mem::take(&mut self.liste);
        self.alle_nehmen();
        for (i, w) in liste.into_iter().enumerate() {
            if !raus.contains(&i) {
                self.hinein(w);
            }
        }
        true
    }
}

/// Ob `a` ein Neuling ist (siehe [`MEMPOOL_NEULINGE_HOECHSTENS`]).
/// ⚑ **Anlaufregel** (Stufe 1, Festlegung des Projektinhabers vom
/// 2026-10-28): Was fällig ist (Freigabe bis `alte_epoche`), kommt in die
/// Zuschreibung dieser Epoche; was ein Pod allein rechnete, wartet bis
/// `alte_epoche` plus Streitfrist (`SPERRFRIST_EPOCHEN`).
fn zurueckhalten_und_freigeben(
    zurueckgehalten: &mut Vec<(u64, myl_types::ids::MinerId, u64)>,
    alte_epoche: u64,
    je_miner: &mut std::collections::BTreeMap<myl_types::ids::MinerId, u64>,
    zurueck: std::collections::BTreeMap<myl_types::ids::MinerId, u64>,
) {
    let (faellig, offen): (Vec<_>, Vec<_>) = std::mem::take(zurueckgehalten).into_iter().partition(|(ab, _, _)| *ab <= alte_epoche);
    *zurueckgehalten = offen;
    for (_, wer, vtfe) in faellig {
        let e = je_miner.entry(wer).or_insert(0);
        *e = e.saturating_add(vtfe);
    }
    let ab = alte_epoche.saturating_add(myl_ledger::einsatz::SPERRFRIST_EPOCHEN);
    zurueckgehalten.extend(zurueck.into_iter().filter(|(_, v)| *v > 0).map(|(wer, v)| (ab, wer, v)));
}

/// ⚑ **Ein Pod, der allein rechnete, wird voll gezogen** (Anlaufregel):
/// jedes Segment seines Bündels, höchstens
/// [`Kette::VOLL_GEZOGEN_HOECHSTENS`], ohne Doppel zur Stichprobe.
fn voll_ziehen(
    stichprobe: &mut Vec<crate::stichprobe::Segmentstichprobe>,
    voll: &[myl_types::ids::PodId],
    anteile: &[(myl_types::ids::PodId, u32)],
) {
    for pod in voll {
        let n = anteile.iter().find(|(p, _)| p == pod).map_or(0, |(_, n)| *n);
        for segment in 0..n.min(Kette::VOLL_GEZOGEN_HOECHSTENS) {
            let s = crate::stichprobe::Segmentstichprobe { pod: *pod, segment };
            if !stichprobe.contains(&s) {
                stichprobe.push(s);
            }
        }
    }
}

/// Was [`Kette::zuschreibung_der_epoche`] liefert.
struct Epochenzuschreibung {
    /// Was in dieser Epoche ausgeschüttet wird.
    zuschreibung: myl_tokenomics::Zuschreibung,
    /// Was erst nach der Streitfrist gutgeschrieben wird (Anlaufregel).
    zurueck: std::collections::BTreeMap<myl_types::ids::MinerId, u64>,
    /// Die Pods, die allein rechneten: Jedes ihrer Segmente wird gezogen.
    allein: Vec<myl_types::ids::PodId>,
    /// Die gültig unterschriebenen Bündel (für die Stichprobe).
    bezeugt: Vec<myl_types::PoIBundle>,
    /// Zwillingsgruppen einig und uneinig.
    einig: u64,
    uneinig: u64,
    /// Uneinige Paare mit beiden Bündeln.
    verschieden: Vec<(myl_types::PoIBundle, myl_types::PoIBundle)>,
    /// Je Pod eines uneinigen Paars: Pod, Zwilling und was seine Miner
    /// verdient hätten (zurückgehalten bis zum Urteil).
    vorbehalten: Vec<(myl_types::ids::PodId, myl_types::ids::PodId, std::collections::BTreeMap<myl_types::ids::MinerId, u64>)>,
    /// Je Sitzung, die nur ein Zwilling bezeugt und einen Pflichtauftrag
    /// bekommt: Pod, Zwilling, die Sitzung und was seine Miner für sie
    /// verdient hätten (zurückgehalten, bis der Auftrag einig schliesst).
    pflicht: Vec<(myl_types::ids::PodId, myl_types::ids::PodId, myl_types::FehlendeSitzung, std::collections::BTreeMap<myl_types::ids::MinerId, u64>)>,
}

/// Was [`nach_zwillingen`] liefert.
#[derive(Debug, Default)]
struct NachZwillingen {
    /// Vergütet in dieser Epoche.
    bezahlt: Vec<myl_tokenomics::Podabrechnung>,
    /// Ein Pod, der allein rechnete (Anlaufregel): erst nach der
    /// Streitfrist.
    allein: Vec<(u32, myl_tokenomics::Podabrechnung)>,
    /// Bündel von Pods in Bereitschaft (in keiner Gruppe): nicht vergütet.
    bereitschaft: u64,
    /// Gruppen einig und uneinig (oder unvollständig).
    einig: u64,
    uneinig: u64,
    /// Gruppen, in denen beide Bündel da sind und verschieden: Für sie darf
    /// ein Differenzbeweis gemeldet werden.
    verschieden: Vec<(myl_types::PoIBundle, myl_types::PoIBundle)>,
    /// Ihre Abrechnungen, je Paar halbiert wie bei einer einigen Gruppe:
    /// zurückgehalten bis zum Urteil (Rückbuchung, Whitepaper 6.4).
    vorbehalten: Vec<((myl_types::ids::PodId, myl_tokenomics::Podabrechnung), (myl_types::ids::PodId, myl_tokenomics::Podabrechnung))>,
    /// ⚑ Sitzungen, die nur ein Zwilling bezeugt (Stufe 1 je Sitzung,
    /// 2026-11-16), soweit sie einen Pflichtauftrag bekommen: Pod, Zwilling,
    /// die Sitzung und die Abrechnung ihres halben Anteils.
    pflicht: Vec<(myl_types::ids::PodId, myl_types::ids::PodId, myl_types::FehlendeSitzung, myl_tokenomics::Podabrechnung)>,
}

/// Die Abrechnung eines Pods für `teil` seiner `segmente` Segmente, je Paar
/// zur Hälfte (Redundanz-Normierung).
fn anteil_je_paar(r: &myl_tokenomics::Podabrechnung, teil: u32, segmente: u32) -> myl_tokenomics::Podabrechnung {
    let mut r = r.clone();
    r.vtfe_pod = if segmente == 0 {
        0
    } else {
        (u128::from(r.vtfe_pod) * u128::from(teil.min(segmente)) / u128::from(segmente) / 2) as u64
    };
    r
}

/// **Welche fehlenden Sitzungen einen Pflichtauftrag bekommen:** je Seite
/// die grössten zuerst (bei Gleichstand die kleinere Nummer), abwechselnd
/// je Seite, zusammen höchstens
/// [`myl_ledger::pruefung::PFLICHTAUFTRAEGE_JE_PAAR`]. Abwechselnd, damit
/// nicht ein Pod mit vielen grossen Sitzungen dem anderen alle Plätze nimmt.
fn pflicht_waehlen(
    nur_a: &[myl_types::FehlendeSitzung],
    nur_b: &[myl_types::FehlendeSitzung],
) -> (Vec<myl_types::FehlendeSitzung>, Vec<myl_types::FehlendeSitzung>) {
    let geordnet = |l: &[myl_types::FehlendeSitzung]| {
        let mut l = l.to_vec();
        l.sort_by(|x, y| y.segmente.cmp(&x.segmente).then(x.sitzung.cmp(&y.sitzung)));
        l
    };
    let (a, b) = (geordnet(nur_a), geordnet(nur_b));
    let (mut i, mut j) = (0usize, 0usize);
    let mut an_a = true;
    while i + j < myl_ledger::pruefung::PFLICHTAUFTRAEGE_JE_PAAR && (i < a.len() || j < b.len()) {
        if (an_a && i < a.len()) || j >= b.len() {
            i += 1;
        } else {
            j += 1;
        }
        an_a = !an_a;
    }
    (a[..i].to_vec(), b[..j].to_vec())
}

/// Die Liste der Sitzungen eines Bündels, wenn es eine gültige trägt.
fn sitzungsliste(b: &myl_types::PoIBundle) -> Option<&[myl_types::Sitzungszeugnis]> {
    (!b.sitzungen.is_empty() && myl_types::sitzungsliste_pruefen(&b.sitzungen, b.segmente).is_ok()).then_some(b.sitzungen.as_slice())
}

/// **Die Abrechnung nach Zwillingsgruppen** (Stufe 1): aus den gültig
/// unterschriebenen Bündeln je Pod-Index. Eine Gruppe (ein Paar) ist einig,
/// wenn jeder ein Bündel hat und alle mit dem ersten einig sind; dann bekommt
/// jeder seinen Anteil (Redundanz-Normierung, je Paar die Hälfte). Uneinig
/// oder ein Bündel fehlt: keiner. Ohne jedes Bündel: nichts zu zählen. ⚑ Ein
/// Pod allein (Gruppe aus einem, Anlaufregel vom 2026-10-28) kommt nach
/// `allein`. Das Bündel eines Pods in keiner Gruppe (Bereitschaft) wird
/// nicht vergütet, nur gezählt.
///
/// ⚑ **Je Sitzung** (Whitepaper 6.4, Festlegung des Projektinhabers vom
/// 2026-11-16): Tragen die Bündel eines Paars die Liste ihrer Sitzungen und
/// sind sie nicht als Ganzes einig, wird je Sitzung abgerechnet. Was beide
/// gleich bezeugen, wird sofort vergütet, je zur Hälfte. Was abweicht, wird
/// vorbehalten und darf über den Differenzbeweis angefochten werden. Was
/// einem fehlt (auch sein ganzes Bündel), wird vorbehalten und bekommt einen
/// Pflichtauftrag, höchstens [`myl_ledger::pruefung::PFLICHTAUFTRAEGE_JE_PAAR`]
/// je Paar; was darüber hinaus fehlt, bleibt unvergütet. Vorher zahlte eine
/// einzige Anfrage, die ein Zwilling nicht mehr in sein Bündel bekam, beiden
/// Pods für die ganze Epoche nichts. Ein Paar zählt als einig, wenn nichts
/// abweicht und etwas gleich ist.
fn nach_zwillingen(
    mut gueltig: std::collections::BTreeMap<u32, (myl_types::PoIBundle, myl_tokenomics::Podabrechnung)>,
    gruppen: &[Vec<u32>],
) -> NachZwillingen {
    let mut aus = NachZwillingen::default();
    for gruppe in gruppen {
        let fehlt: Vec<u32> = gruppe.iter().copied().filter(|i| !gueltig.contains_key(i)).collect();
        let mut da: Vec<(myl_types::PoIBundle, myl_tokenomics::Podabrechnung)> =
            gruppe.iter().filter_map(|i| gueltig.remove(i)).collect();
        if gruppe.len() == 1 {
            if let (Some(&i), Some((_, r))) = (gruppe.first(), da.pop()) {
                aus.allein.push((i, r));
            }
            continue;
        }
        if da.is_empty() {
            continue;
        }
        let einig = da.len() == gruppe.len() && da.iter().all(|(b, _)| zwillinge_einig(&da[0].0, b));
        if !einig && gruppe.len() == 2 && je_sitzung(&mut aus, &da, &fehlt) {
            continue;
        }
        if !einig {
            aus.uneinig += 1;
            if let [(a, ra), (b, rb)] = da.as_slice() {
                aus.verschieden.push((a.clone(), b.clone()));
                let halb = |r: &myl_tokenomics::Podabrechnung| {
                    let mut r = r.clone();
                    r.vtfe_pod /= 2;
                    r
                };
                aus.vorbehalten.push(((a.pod, halb(ra)), (b.pod, halb(rb))));
            }
            continue;
        }
        let n = gruppe.len() as u64;
        aus.bezahlt.extend(da.into_iter().map(|(_, mut r)| {
            r.vtfe_pod /= n;
            r
        }));
        aus.einig += 1;
    }
    aus.bereitschaft = gueltig.len() as u64;
    aus
}

/// **Ein uneiniges Paar je Sitzung abrechnen**, wenn die Listen es
/// erlauben; `false` heisst: als Ganzes behandeln wie zuvor (keine Liste,
/// oder die Listen sind gleich und nur die flachen Wurzeln verschieden).
fn je_sitzung(
    aus: &mut NachZwillingen,
    da: &[(myl_types::PoIBundle, myl_tokenomics::Podabrechnung)],
    fehlt: &[u32],
) -> bool {
    match da {
        [(a, ra), (b, rb)] => {
            let (Some(la), Some(lb)) = (sitzungsliste(a), sitzungsliste(b)) else {
                return false;
            };
            let g = myl_types::sitzungen_abgleichen(la, lb);
            if g.abweichend == (0, 0) && g.nur_a.is_empty() && g.nur_b.is_empty() {
                // Dieselben Listen, verschiedene flache Wurzeln: Eine der
                // beiden Listen passt nicht zu ihrem Bündel. Nichts gilt als
                // gleich.
                return false;
            }
            if g.gleich > 0 {
                aus.bezahlt.push(anteil_je_paar(ra, g.gleich, a.segmente));
                aus.bezahlt.push(anteil_je_paar(rb, g.gleich, b.segmente));
            }
            if g.abweichend != (0, 0) {
                aus.verschieden.push((a.clone(), b.clone()));
                aus.vorbehalten.push((
                    (a.pod, anteil_je_paar(ra, g.abweichend.0, a.segmente)),
                    (b.pod, anteil_je_paar(rb, g.abweichend.1, b.segmente)),
                ));
            }
            let (pa, pb) = pflicht_waehlen(&g.nur_a, &g.nur_b);
            aus.pflicht.extend(pa.into_iter().map(|f| (a.pod, b.pod, f, anteil_je_paar(ra, f.segmente, a.segmente))));
            aus.pflicht.extend(pb.into_iter().map(|f| (b.pod, a.pod, f, anteil_je_paar(rb, f.segmente, b.segmente))));
            if g.abweichend == (0, 0) && g.gleich > 0 {
                aus.einig += 1;
            } else {
                aus.uneinig += 1;
            }
            true
        }
        // Das Bündel des Zwillings fehlt ganz: Jede Sitzung fehlt ihm.
        [(a, ra)] => {
            let (Some(la), Some(&zwilling)) = (sitzungsliste(a), fehlt.first()) else {
                return false;
            };
            let g = myl_types::sitzungen_abgleichen(la, &[]);
            let (pa, _) = pflicht_waehlen(&g.nur_a, &[]);
            let zwilling = myl_types::pod_kennung(a.epoch.0, zwilling);
            aus.pflicht.extend(pa.into_iter().map(|f| (a.pod, zwilling, f, anteil_je_paar(ra, f.segmente, a.segmente))));
            aus.uneinig += 1;
            true
        }
        _ => false,
    }
}

/// **Haben zwei Zwillinge dasselbe gerechnet?** Beide mit Ausgabenwurzel
/// (nicht null), dieselbe Wurzel und dieselbe Zahl der Segmente (Stufe 1).
pub fn zwillinge_einig(a: &myl_types::PoIBundle, b: &myl_types::PoIBundle) -> bool {
    a.ausgabenwurzel.as_bytes() != &[0u8; 32] && a.ausgabenwurzel == b.ausgabenwurzel && a.segmente == b.segmente
}

/// **Die Dämpfung des Credit-Preises** κ, Festkomma mit 16 Nachkommabits:
/// 1/6 (Festlegung des Projektinhabers vom 2026-10-28). Ohne Nachfrage fällt
/// der Preis damit je Epoche um ein Achtel;
/// bei voller Auslastung steigt er um rund 3,4 %, weil das Ziel
/// des Whitepapers bei 0,8 liegt und nicht in der Mitte.
pub const CREDIT_PREIS_KAPPA: i64 = (1 << 16) / 6;

/// **Das Ziel der Auslastung** u* = 0,8 (Whitepaper 5.4), Festkomma mit 16
/// Nachkommabits wie [`myl_types::auslastung::AUSLASTUNG_SKALA`].
pub const CREDIT_PREIS_ZIEL: i64 = (myl_types::auslastung::AUSLASTUNG_SKALA * 4 + 2) / 5;

/// **Der Credit-Preis der nächsten Epoche** aus dem jetzigen in
/// Feinauflösung (32 Nachkommabits) und der Auslastung (16 Nachkommabits):
/// `P · exp(κ (u − u*))`, unten begrenzt auf den Kleinstbetrag der
/// Feinauflösung. Liefert den neuen Wert in Feinauflösung und den
/// abgerechneten, gerundet zur nächsten geraden Zahl und mindestens 1 (ein
/// Preis von null verweigert jeden Burn).
///
pub fn credit_preis_naechster(fein: i64, auslastung: i64) -> (i64, u64) {
    let neu = myl_tokenomics::update_price_mit_untergrenze(
        fein,
        CREDIT_PREIS_KAPPA,
        auslastung,
        CREDIT_PREIS_ZIEL,
        myl_tokenomics::PREIS_UNTERGRENZE_VORGABE,
    );
    (neu, credit_preis_abgerechnet(neu))
}

/// Höchstens diese Auslastung zählt für den Preis: Sie begrenzt den Sprung
/// je Epoche auf rund 22 Prozent, wie der Blockdeckel bei einem Grundpreis.
pub const CREDIT_PREIS_AUSLASTUNG_HOECHSTENS: i64 = 2 * myl_types::auslastung::AUSLASTUNG_SKALA;

/// ⚑ **Die nachgefragten vTFE einer Epoche** (Whitepaper 5.4): die gekauften
/// Credits, umgerechnet in vTFE-Einheiten. Ein Credit ist ein Vorwärtspass,
/// also ein TFE (Fund 711); Kapazität und bestätigte Arbeit stehen in
/// Einheiten von 10⁻⁶ TFE ([`myl_tokenomics::VTFE_UNITS_PER_TFE`]).
///
/// 📌 **Fund 714 (2026-11-17):** Hier stand der Kauf in Credits, verglichen
/// mit einer Kapazität in vTFE-Einheiten. Die Nachfrage aus Käufen erschien
/// eine Million Mal zu klein, und der Preis folgte nur der bestätigten
/// Arbeit. ⚑ Umgerechnet wird hier und nicht am Credit (Entscheidung des
/// Projektinhabers vom 2026-11-17): Ein Credit bleibt ein Pass, damit sein
/// Preis ein ganzer Kleinstbetrag bleibt.
pub fn nachgefragte_vtfe(burn_epoche: u64, credit_preis: u64) -> u64 {
    (burn_epoche / credit_preis.max(1)).saturating_mul(myl_tokenomics::VTFE_UNITS_PER_TFE)
}

/// ⚑ **Die Auslastung, der der Preis folgt** (Whitepaper 5.4: nachgefragte
/// vTFE durch Kapazität). Bestätigte Arbeit allein ist nie über 1 und sieht
/// keine Nachfrage, die keinen Platz fand; die in dieser Epoche gekauften
/// Credits sehen sie, und sie sind verbrannt, also nicht umsonst vorzutäuschen.
pub fn preis_auslastung(bestaetigt: i64, nachgefragt_vtfe: u64, kapazitaet: u64) -> i64 {
    bestaetigt
        .max(myl_types::auslastung::auslastung(nachgefragt_vtfe, kapazitaet))
        .min(CREDIT_PREIS_AUSLASTUNG_HOECHSTENS)
}

/// Höchstens so viele Einträge auf einer Seite des Verlaufs.
pub const VERLAUF_JE_SEITE: usize = 200;
/// So viele Blöcke liest eine Seite auf einmal.
const VERLAUF_BLOECKE_JE_SCHUB: u64 = 64;

/// Der abgerechnete Credit-Preis zu einem in Feinauflösung: gerundet zur
/// nächsten geraden Zahl, mindestens 1.
pub fn credit_preis_abgerechnet(fein: i64) -> u64 {
    let ganz = fein >> 32;
    let rest = fein & 0xFFFF_FFFF;
    let halb = 1i64 << 31;
    let gerundet = if rest > halb || (rest == halb && ganz & 1 == 1) { ganz + 1 } else { ganz };
    gerundet.max(1) as u64
}

fn ist_neuling(z: &LedgerState, a: &Address) -> bool {
    let k = z.account(a);
    k.balance == 0 && k.staked == 0 && !z.miner.contains_key(&myl_types::ids::MinerId::new(*a.as_bytes()))
}

/// So viele codierte Gruppen hält die Kette für Splitter vor (Fund 632).
/// Mit einer einzigen erzwang jede Anfrage nach einer anderen Gruppe eine
/// neue Codierung von 1024 Blöcken, auch unter ehrlichen Splittern, die
/// verschiedene Gruppen zugleich holen.
pub const TEILGRUPPEN_BEHALTEN: usize = 8;

/// Der Startwert, an dem die Probekette hängt.
///
/// **Der Riegel gegen eine Verwechslung mit dem echten Netz.** Ein Block
/// nennt seinen Vorgänger; der erste Probeblock nennt diesen Wert. Ein
/// echtes Netz beginnt bei einem anderen, und damit passt ein Probeblock
/// dort nirgends hinein. Der Text sagt es auch dem, der die Bytes
/// anschaut.
///
/// ⚑ **Mit der Historienwurzel neu** (2026-10-23): Der Zustand trägt seitdem
/// die Historie, und eine Kettendatei von davor rechnete an Block 1 eine
/// andere Wurzel. Mit einem neuen Startwert weist der Knoten sie gleich beim
/// Öffnen als fremde Kette ab, statt an einer Wurzel zu scheitern.
pub const PROBE_STARTWERT: &[u8] =
    b"MYELITH-PROBELAUF-KEIN-TESTNETZ-KEINE-ECHTE-KETTE-H1";

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
/// Der Schlüssel eines Probekontos.
///
/// ⚑ **Seit dem 2026-08-28 hat ein Probekonto einen Schlüssel**, denn
/// eine Transaktion muss unterschrieben sein (Fund 85). Vorher war ein
/// Konto nur eine Zeichenkette, und jeder konnte in seinem Namen
/// anweisen.
///
/// Aus einem festen Startwert abgeleitet, damit alle Knoten dieselben
/// Konten sehen: Ein zufälliger Schlüssel machte zwei Läufe
/// unvergleichbar.
pub fn probeschluessel(nummer: u8) -> myl_types::bls::BlsSecretKey {
    let saat = Hash::sha256(
        format!("myelith-probekonto-{}", nummer % PROBEKONTEN).as_bytes(),
    );
    myl_types::bls::BlsSecretKey::key_gen(saat.as_bytes())
        .expect("32 Byte Startwert sind für key_gen immer gültig")
}

/// Die Anmeldung eines Probekontos als Miner in Europa, mit Nummer 0.
pub fn probeanmeldung(nummer: u8) -> Transaktion {
    Transaktion::signiere(
        &Kette::startwert(),
        &probeschluessel(nummer),
        0,
        Anweisung::MinerAnmelden {
            hardware: myl_types::miner::HardwareClass::MediumGpu,
            zone: myl_types::node_metadata::GeoRegion::Europe,
            netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
        },
    )
    .expect("ein gültiger Schlüssel unterschreibt")
}

/// **Die Zusage eines Probekontos für das Probemodell**, als unterschriebene
/// Transaktion (Stufe 4, 2026-10-22): alle Ebenen samt Kopf, 300 µs je
/// Ebene, gültig ab `ab`.
///
/// ⚑ **Seit „kein Pod ohne Zusage“ braucht jede Probe mit Pods das**, wie
/// die Anmeldung: Ein Miner ohne Zusage kommt in keinen Pod.
pub fn probezusage(nummer: u8, ab: EpochId, nonce: u64) -> Transaktion {
    let sk = probeschluessel(nummer);
    let pk = sk.public_key().expect("aus einem gültigen Schlüssel folgt ein gültiger Punkt");
    let pm = crate::netz::PROBEKETTE.podmodell;
    let mut zusage = myl_types::zusage::Kapazitaetszusage::neu(myl_types::ids::MinerId::aus_schluessel(&pk), ab, 0)
        .mit_modell(myl_types::zusage::Modellzusage {
            abdruck: pm.abdruck,
            ebenen_hoechstens: pm.ebenen_samt_kopf(),
            ebenenzeit_us: 300,
            gehalten: None,
        });
    zusage.signiere(&sk).expect("ein gültiger Schlüssel unterschreibt");
    Transaktion::signiere(&Kette::startwert(), &sk, nonce, Anweisung::ZusageEinreichen { zusage })
        .expect("ein gültiger Schlüssel unterschreibt")
}

/// Die Adresse eines Probekontos: `SHA-256` über seinen Schlüssel.
///
/// **Abgeleitet und nicht gewürfelt**, wie jede Adresse im Protokoll.
/// Nur so passt sie zu der Unterschrift, die ihr Inhaber leisten kann.
pub fn probekonto(nummer: u8) -> Address {
    let pk = probeschluessel(nummer)
        .public_key()
        .expect("aus einem gültigen Schlüssel folgt ein gültiger Punkt");
    Address::aus_schluessel(&pk)
}

/// Das Testkonto, das ein Knoten dieses Namens benutzt.
///
/// Über den Namen gewählt, damit jeder Knoten stabil dasselbe Konto
/// bebucht: Ein wechselndes Konto machte zwei Läufe unvergleichbar.
pub fn konto_fuer(name: &str) -> Address {
    let h = Hash::sha256(name.as_bytes());
    probekonto(h.as_bytes()[0])
}

/// Der Schlüssel zu [`konto_fuer`].
///
/// Getrennte Funktion und kein zweites Ableiten: Wer hier etwas ändert,
/// ändert es für Adresse und Schlüssel zugleich, und die beiden können
/// nicht auseinanderlaufen.
pub fn schluessel_fuer(name: &str) -> myl_types::bls::BlsSecretKey {
    let h = Hash::sha256(name.as_bytes());
    probeschluessel(h.as_bytes()[0])
}

/// Was die Wahl des Zweigs ergab ([`Kette::zweig_folgen`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Zweigwahl {
    /// Die Kette blieb auf ihrem Zweig.
    Geblieben,
    /// Die Kette folgt jetzt dem fremden Zweig: ab dem Gabelpunkt `gabel`,
    /// `zurueck` eigene Blöcke zurückgenommen, `neu` fremde übernommen
    /// (ohne Gabelung, `zurueck` null: verlängert).
    Gewechselt { gabel: u64, zurueck: usize, neu: usize },
}

/// Warum ein fremder Zweig nicht in Frage kam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Zweigfehler {
    /// Nichts Neues: Alle Blöcke sind schon eigene.
    Leer,
    /// Zwischen zwei Blöcken fehlt einer (an dieser Höhe).
    Luecke(u64),
    /// Der erste neue Block schliesst an keinen eigenen an.
    PasstNirgends,
    /// Der Gabelpunkt liegt unter dem Boden der Rückwege (dem endgültigen
    /// Prüfpunkt oder tiefer, als die Kette zurück kann).
    ZuTief(u64),
    /// Ein zustandsloser Knoten wählt nicht, er verankert neu.
    Zustandslos,
    /// Ein Block des fremden Zweigs trug nicht; die Kette ist zurück auf
    /// dem eigenen.
    TraegtNicht(u64, KettenFehler),
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
    /// **Das Zertifikat eines Prüfpunkts im Block trägt nicht**
    /// (2026-10-27, CONSENSUS 7.2): kein Prüfpunkt nach dem zuletzt
    /// bestätigten, nicht der eigene Block an dieser Höhe, oder nicht
    /// zwei Drittel des Stimmsatzes.
    ///
    /// ⚑ **Eine Ablehnung und keine Warnung.** Aus dem Zertifikat kommt die
    /// Saat der Stichprobe und die Endgültigkeit; eines, das niemand belegt,
    /// wäre ein frei gewählter Wert.
    PruefpunktTraegtNicht,
    /// **Der Erzeuger ist nicht belegt**: Die Unterschrift
    /// trägt nicht, oder ein Stimmsatz ist gesetzt und der Block ist
    /// unsigniert oder von jemandem außerhalb des Satzes.
    ErzeugerUnbelegt,
    /// **Die Höhe im Kopf passt nicht zur Kette.**
    ///
    /// Der Vorgängerhash bindet die Kette schon; diese Prüfung hält das
    /// Höhenfeld ehrlich. Ein Block, der anschließt und eine falsche
    /// Höhe nennt, wäre sonst gültig und schickte jeden Nachzügler in
    /// die Irre, denn an der Höhe zählt er seine Lücke ab.
    HoeheWeichtAb { erwartet: u64, bekommen: u64 },
    /// **Die Epoche im Kopf folgt nicht aus der Höhe.**
    ///
    /// Die Epoche ist eine Funktion der Höhe
    /// (`myl_consensus::block::epoche_fuer_hoehe`). Sie steht trotzdem
    /// im Kopf, damit ein Block für sich lesbar bleibt — geprüft wird
    /// sie, weil an ihr der Verfall von Credits und das Fenster der
    /// Verstoßhistorie hängen.
    EpocheWeichtAb { erwartet: u64, bekommen: u64 },
    /// **Der Block trägt einen Schuldspruch, und niemand kann ihn
    /// prüfen** (Fund 192).
    ///
    /// ⚑ **Eine Ablehnung, obwohl die Kette Schuldsprüche heute gar
    /// nicht anwendet, und gerade deshalb.** `Block::verdicts` ist Teil
    /// des gehashten Blocks, `Block::add_verdict` steht offen, und der
    /// Ledger hat mit `apply_verdict` alles, was zum Schlachten nötig
    /// ist. Was fehlt, ist der **Beleg**: Ein `Verdict` nennt Täter und
    /// Kopfgeldempfänger und trägt keinen Nachweis. Wer die beiden
    /// Enden verbände, ohne den Beleg zu verlangen, gäbe dem
    /// Blockerzeuger ein Werkzeug, mit dem er jeden schlachten kann.
    ///
    /// **Die Ablehnung ist die sichere Zwischenlage:** Solange der
    /// Beleg nicht auf dem Draht ist, kommt kein Schuldspruch in einen
    /// Block. Damit ist das Feld nicht länger stiller Ballast, den ein
    /// späterer Bau versehentlich scharf schaltet, sondern eine Tür,
    /// die aufgeschlossen werden **muss**.
    SchuldspruchOhneBeleg { anzahl: usize },
    /// **Die Anfechtungen des Blocks sind unsinnig** (Fund 192).
    ///
    /// `Block::validate_challenges` gab es seit jeher und hatte bis zum
    /// 2026-09-06 **keinen Aufrufer**: eine geprüfte, abgehakte Prüfung,
    /// die nie lief.
    AnfechtungUnsinnig,
    /// ⚑ **Ein zustandsloser Knoten bekam keinen Zeugen**, oder einen, der
    /// nicht trägt (zustandslose Validierung, 2026-10-29).
    ZeugeTraegtNicht(Option<myl_ledger::state::Zeugenfehler>),
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
            Self::PruefpunktTraegtNicht => write!(
                f,
                "das Zertifikat des Prüfpunkts trägt nicht"
            ),
            Self::ErzeugerUnbelegt => write!(f, "der Erzeuger ist nicht belegt: keine tragende Unterschrift eines Berechtigten"),
            Self::ZeugeTraegtNicht(None) => write!(f, "zustandslos und ohne Zeugen"),
            Self::ZeugeTraegtNicht(Some(g)) => write!(f, "der Zeuge trägt nicht: {g:?}"),
            Self::ZustandWeichtAb { erwartet, errechnet } => write!(
                f,
                "Zustandswurzel weicht ab: Block sagt {}, errechnet {}",
                &erwartet.to_hex()[..16],
                &errechnet.to_hex()[..16]
            ),
            Self::HoeheWeichtAb { erwartet, bekommen } => write!(
                f,
                "Höhe weicht ab: erwartet {erwartet}, Block sagt {bekommen}"
            ),
            Self::EpocheWeichtAb { erwartet, bekommen } => write!(
                f,
                "Epoche folgt nicht aus der Höhe: erwartet {erwartet}, Block sagt {bekommen}"
            ),
            Self::SchuldspruchOhneBeleg { anzahl } => write!(
                f,
                "{anzahl} Schuldspruch/Schuldsprueche im Block, aber kein Beleg auf dem Draht: \
                 abgelehnt, bis der Nachweisweg steht"
            ),
            Self::AnfechtungUnsinnig => {
                f.write_str("eine Anfechtung des Blocks besteht die Strukturpruefung nicht")
            }
        }
    }
}

impl std::error::Error for KettenFehler {}

/// Die Kette eines Knotens: Zustand, Höhe, Mempool.
pub struct Kette {
    hoehe: u64,
    letzter_hash: Hash,
    zustand: LedgerState,
    /// Der Stimmsatz, gegen den Saatquellen geprüft werden (Punkt 44).
    ///
    /// ⚑ **`None` heißt: nicht prüfbar, und das wird gemeldet.** Ein
    /// Knoten ohne Stimmsatz kann eine Aggregatsignatur nicht
    /// nachrechnen; er soll deshalb sagen, dass er es nicht tut, statt
    /// stillschweigend alles anzunehmen.
    stimmsatz: Option<myl_consensus::validator::VotingSet>,
    /// Der Schlüssel, mit dem dieser Knoten seine Blöcke unterschreibt
    /// (2026-10-27); `None` heißt unsigniert.
    erzeugerschluessel: Option<crate::schluessel::Konsensschluessel>,
    /// ⚑ Ob dieser Knoten nur nach dem Plan baut, als Miner und nicht als
    /// ausdrücklicher Erzeuger (2026-11-15): Vor dem Lernen des Erzeugers
    /// baut er dann gar nicht ([`Self::eigener_slot`]).
    nur_nach_plan: bool,
    /// Ob diese Kette ihren Stimmsatz aus Block 1 lernt (nur auf einer
    /// Probekette, [`crate::netz::Netz::lernt_erzeuger`]).
    lernt_erzeuger: bool,
    /// Das Zertifikat eines Prüfpunkts, das der nächste erzeugte Block
    /// tragen soll (2026-10-27, CONSENSUS 7.2).
    ///
    /// ⚑ **Es kommt von außen, weil es im Stimmsatz entsteht**: Der Knoten
    /// sammelt die Prüfpunktstimmen und bietet es an
    /// ([`Self::pruefpunkt_anbieten`]); die Kette prüft es beim Bauen wie
    /// beim Übernehmen.
    naechstes_pruefpunktzertifikat: Option<myl_consensus::pruefpunkt::Pruefpunktzertifikat>,
    /// Die Zuteilungen abgelaufener Epochen, deren Ziehung noch aussteht:
    /// Gezogen wird erst mit dem Zertifikat (2026-10-27), und dann gilt
    /// schon die Zuteilung der nächsten Epoche (vgl. Fund 658).
    offene_zuteilungen: Vec<(u64, myl_scheduler::shard_assignment::Zuteilung)>,
    /// Die zuletzt gezogene Stichprobe, mit der Epoche, zu der sie
    /// gehört (Punkt 45).
    ///
    /// 📌 **Hier stand zuerst ein blanker `Vec`, und er wurde von jedem
    /// Block überschrieben.** Eine Ziehung gehört zu einer **Epoche**,
    /// und zwischen zwei Epochenwechseln liegen viele Blöcke; jeder
    /// setzte sie auf leer zurück. Der Test fiel sofort um, und das war
    /// die richtige Antwort.
    ///
    /// ⚑ **Nicht im `LedgerState`, und das ist Absicht.** Die Ziehung ist
    /// eine **reine Funktion** aus den bezeugten Bündeln, der Epoche und
    /// dem Blockhash; sie in den Zustand zu schreiben wäre eine zweite
    /// Quelle für dieselbe Aussage, und `commitment()` serialisiert den
    /// ganzen Zustand je Block (D7). Wer sie braucht, rechnet sie nach.
    ///
    /// Hier steht sie nur, damit der Knoten sie **protokollieren** kann:
    /// Eine Ziehung, die niemand sieht, ist von keiner Ziehung nicht zu
    /// unterscheiden, und das war Fund 114.
    letzte_stichprobe: Option<(u64, Vec<crate::stichprobe::Segmentstichprobe>)>,
    /// Die Pods der zuletzt gezogenen Stichprobe, aus der Zuteilung der
    /// Epoche, zu der sie gehört.
    ///
    /// 📌 **Fund 658 (2026-10-27, im kurzen Podlauf): Keine Stichprobe fand
    /// ihren Pod.** Gezogen wird beim Wechsel, für die **abgelaufene**
    /// Epoche; [`Self::pod_der_kennung`] kannte nur die Zuteilung der
    /// **laufenden** und gab für jede andere `None`. Jede Ziehung endete als
    /// `ohne_adresse`, noch vor Fund 656. Jetzt hält die Kette die Pods der
    /// Ziehung fest, bevor der Wechsel die Zuteilung ersetzt.
    pods_der_stichprobe: Vec<myl_scheduler::shard_assignment::Pod>,
    /// ⚑ **Der Checker jeder Ziehung** (Stufe 2, W6b, 2026-10-29): je
    /// gezogenem Segment Pod, Index und der zugeteilte Miner, siehe
    /// [`pruefer_waehlen`]. Leer, wenn die abgelaufene Epoche niemanden ohne
    /// Pod hatte.
    pruefer_der_stichprobe: Vec<(myl_types::ids::PodId, u32, myl_types::ids::MinerId)>,
    /// Abgeschlossene Prüfaufträge und entschiedene Streitfälle, bis der
    /// Knoten sie abholt ([`Self::pruefausgaben_abholen`]).
    pruefausgaben: Pruefausgaben,
    /// ⚑ **Zustandslos** (zustandslose Validierung, 2026-10-29): Der Knoten
    /// hält die grossen Sammlungen nicht (Konten, Sitzungen, Speicher,
    /// Hinterlegungen) und übernimmt einen Block nur mit seinem Zeugen
    /// ([`Self::uebernimm_mit_zeuge`]). Er erzeugt keine Blöcke und legt
    /// keine Abzüge an.
    zustandslos: bool,
    /// Die Zeugen der zuletzt selbst erzeugten Blöcke, je mit dem
    /// Blockhash, für zustandslose Knoten; zusammen höchstens
    /// [`ZEUGEN_HOECHSTENS_BYTES`].
    eigene_zeugen: std::collections::VecDeque<(Hash, u64, std::sync::Arc<Vec<u8>>)>,
    /// Selbst erzeugte Blöcke ohne Zeugen (ein Zugriff über eine ganze
    /// grosse Sammlung im Blockpfad). Null, sonst ein Fund.
    zeugenlos: u64,
    /// ⚑ **Ein voller Knoten hält Zeugen bereit** (2026-10-30): Er zeichnet
    /// beim Übernehmen auf, wie ein Erzeuger, und unterschreibt die Stücke
    /// mit diesem Schlüssel. 📌 Im Podlauf hielt nur der Erzeuger Zeugen;
    /// als er endete, konnte ein zustandsloser Knoten, der 136 Blöcke
    /// zurücklag, sie nie mehr prüfen.
    zeugenschluessel: Option<crate::schluessel::Konsensschluessel>,
    /// Die unterschriebenen Stücke der zuletzt verbreiteten Zeugen.
    zeugenstuecke: std::collections::VecDeque<(Hash, std::sync::Arc<Vec<myl_consensus::zeuge::Zeugenstueck>>)>,
    /// ⚑ **Dieselben Stücke auf der Platte, über drei Prüfpunktabstände**
    /// ([`crate::zeugenablage`], 2026-10-30); `None` ohne Ordner.
    zeugenablage: Option<crate::zeugenablage::Zeugenablage>,
    /// Ablegen auf der Platte misslang so oft (null, sonst ein Fund).
    zeugenablage_fehler: u64,
    /// ⚑ **Beim zustandslosen Knoten seine eigenen Konten** (`None`: es gibt
    /// es nicht), aus jedem geprüften Block fortgeschrieben. Der Knoten
    /// braucht sie für die Nummern seiner Transaktionen (Bündel, Prüfung,
    /// Schiedsstimmen), für Einsatz und Guthaben; ohne sie reichte er seine
    /// Arbeit mit falschen Nummern ein und würde nicht entlohnt.
    eigene_konten: std::collections::BTreeMap<Address, Option<myl_ledger::AccountState>>,
    /// Eigene Konten, deren Stand der zustandslose Knoten nicht kennt (nach
    /// einem leichten Abzug), bis ein Zeuge oder ein Kontobeweis ihn
    /// bringt; bis dahin gelten sie als neu.
    eigene_unbekannt: std::collections::BTreeSet<Address>,
    /// Die zuletzt angewandten Prüfpunktzertifikate (für den Anker eines
    /// leichten Abzugs).
    zertifikate: std::collections::VecDeque<myl_consensus::pruefpunkt::Pruefpunktzertifikat>,
    /// ⚑ Die Wechselbelege (7.3, 7.4), je Epoche einer: aus eigenen Blöcken
    /// oder von Gegenstellen geprüft ([`crate::wechsel`]).
    wechselbelege: std::collections::BTreeMap<u64, crate::wechsel::Wechselbeleg>,
    /// ⚑ Epochen, deren gemerkter Beleg die eigene Folge nicht traegt (nach
    /// einer Luecke); ein spaeterer derselben Epoche darf ihn ersetzen.
    belege_ohne_folge: std::collections::BTreeSet<u64>,
    /// ⚑ Der Anker von aussen (7.4a): Hoehe, Block und Bindung des
    /// Zertifikats eines Belegs, vertraut wie Genesis
    /// ([`crate::wechsel::Folge::ab_beleg`]).
    anker_von_aussen: Option<(u64, Hash, Hash)>,
    /// Wo sie neben der Kettendatei liegen.
    belegablage: Option<crate::wechsel::Belegablage>,
    /// ⚑ Ab welcher Epoche die eigene Folge beginnt, wenn aeltere Belege
    /// gekuerzt sind (A44). Was hier liegt, hat dieser Knoten selbst geprueft.
    belege_ab: Option<u64>,
    /// Wie viele Belege seit dem letzten Neuschreiben der Ablage fielen.
    belege_gefallen: u64,
    /// Wann das Netz begann, aus der Stimmsatzdatei (A43).
    netzstart_ms: Option<u64>,
    /// Wartende Transaktionen, je mit der eigenen Höhe bei der Aufnahme.
    mempool: Mempool,
    /// Die Kennungen der Transaktionen, deren Unterschrift dieser Knoten
    /// schon geprüft hat ([`Self::aufnehmen_geprueft`]).
    unterschriften: Unterschriften,
    /// Die Hashes der übernommenen Blöcke, damit Dubletten aus dem
    /// Gossip nicht zweimal angewandt werden.
    bekannt: std::collections::HashSet<Hash>,
    /// Die jüngsten Blöcke, nach Höhe. **Höchstens
    /// [`VERLAUFSFENSTER`] Stück.**
    ///
    /// **Wer nachliefern soll, muss aufheben.** Ohne diesen Speicher
    /// könnte ein Knoten einem Neuling nicht helfen, und der Rückstand
    /// wäre endgültig.
    ///
    /// ⚑ **Bis zum 2026-09-02 wuchs er unbegrenzt** (Fund 124). Ein
    /// Knoten, der lange genug lief, hielt die vollständige Kette im
    /// Arbeitsspeicher und starb an ihr, und zwar leise: erst
    /// Auslagern, dann der Abschuss durch das Betriebssystem, ohne
    /// Eintrag im eigenen Protokoll.
    ///
    /// Jetzt ist er ein **Zwischenspeicher**, und die Datei ist die
    /// Quelle. Was aus dem Fenster fällt, holt
    /// [`Kette::bloecke_von_bis`] von der Platte zurück, solange ein
    /// [`crate::speicher::Kettenspeicher`] geführt wird. Wer mit
    /// `--ohne-kette` fährt, hat nur noch das Fenster, und das steht
    /// dort ausdrücklich.
    verlauf: std::collections::BTreeMap<u64, Block>,
    /// Das Blockprotokoll auf der Platte, falls eines geführt wird.
    ///
    /// ⚑ **Der Speicher gehört hierher und nicht zum Aufrufer.** Die
    /// Zusage lautet „jeder Block, der in die Kette kommt, steht auch in
    /// der Datei". Läge das Schreiben beim Aufrufer, wäre sie an drei
    /// Aufrufstellen verteilt, und die vierte, die jemand später
    /// hinzufügt, vergäße es. Hier kann sie es nicht.
    speicher: Option<crate::speicher::Kettenspeicher>,
    /// Wie oft das Schreiben fehlschlug.
    ///
    /// **Ein Schreibfehler macht den Block nicht ungültig**, also wird
    /// er angenommen. Er macht aber die Datei unvollständig, und das
    /// darf nicht still bleiben: Der Zähler geht in jede
    /// Zustandsaufnahme.
    schreibfehler: u64,
    /// Wie oft ein Block aus der Datei nicht zurückkam.
    ///
    /// **Zählt nur echte Lesefehler**, nicht „nicht vorhanden". Ein
    /// Nachzügler, der zu weit zurückfragt, ist kein Fehler; eine
    /// Datei, die einen Satz nicht mehr hergibt, den sie beim Öffnen
    /// noch hergab, ist einer. Auch dieser Zähler geht in jede
    /// Zustandsaufnahme.
    lesefehler: u64,
    /// ⚑ **Wo an jeder Epochengrenze ein Abzug abgelegt wird**
    /// (2026-10-22), neben der Kettendatei. Ohne Kettendatei keiner.
    ablage: Option<crate::abzug::Ablage>,
    /// Die Höhe, ab der diese Kette lebt: 0 von Genesis, sonst die Höhe
    /// des Abzugs, aus dem sie begann. Blöcke davor hat sie nicht.
    basis: u64,
    /// Der zuletzt angelegte Abzug (Höhe, Blockhash), für das Protokoll.
    letzter_abzug: Option<(u64, Hash)>,
    /// ⚑ **Das Platzbudget der Kettendatei in Bytes** (Festlegung des
    /// Projektinhabers vom 2026-10-23); null heißt ohne Grenze. Siehe
    /// [`Kette::verdichten_wenn_noetig`].
    budget: u64,
    /// Die letzte Verdichtung (neue Basis, Bytes vorher, nachher), bis
    /// der Knoten sie ins Protokoll geschrieben hat.
    verdichtet: Option<(u64, u64, u64)>,
    /// Die eigene Kennung als Miner, für die Auslosung der Archivknoten.
    archivkennung: Option<myl_types::ids::MinerId>,
    /// ⚑ **Der Historienbaum** (Archivknoten, Stufe A4): Solange die Kette
    /// ab Genesis lebt, führt sie neben der Historie im Zustand die Knoten
    /// ab Stufe zehn mit (rund 64 Byte je 1024 Blöcke) und kann damit jedem,
    /// der nachlädt, Beweise gegen seine Basis geben. Ab einer Basis fehlt
    /// ihr der Anfang, dann `None`.
    baum: Option<myl_types::historie::Historienbaum>,
    /// Die Gruppenwurzeln des laufenden Abschnitts (Grenze, Wurzeln), einmal
    /// gerechnet, für die Teilwurzel und die Pfade der Teile. Solange es
    /// weniger sind als Gruppen im Abschnitt, ist die Rechnung unterwegs
    /// ([`Kette::gruppenwurzeln_vorarbeiten`]).
    teilcache: Option<(u64, Vec<Hash>)>,
    /// Die zuletzt codierten Gruppen, höchstens [`TEILGRUPPEN_BEHALTEN`]:
    /// Splitter fragen dieselben, aber nicht alle zugleich dieselbe
    /// (Fund 632).
    teilgruppe: std::collections::VecDeque<(u64, Vec<Vec<u8>>)>,
    /// ⚑ **Je Block über dem endgültigen Prüfpunkt sein Rückweg**
    /// (Zurückrollen, 2026-11-04), höchstens [`RUECKWEG_HOECHSTENS`]; nur
    /// bei einem vollen Knoten, und nur im Speicher.
    rueckwege: std::collections::VecDeque<Rueckspur>,
    /// Höhe und Slot eines eigenen Blocks an oder unter dem endgültigen
    /// Prüfpunkt, für die Schranke der Höhe ([`Self::hoehen_schranke`]).
    schranke: Option<(u64, u64)>,
}

/// ⚑ **Dieselbe Bindung wie in `myl_pod::pipelinewerk`** (seit
/// 2026-09-03): Der Zuschnitt, gegen den die Kette Pods bildet, und die
/// Gewichte, nach denen sie auszahlt, müssen dieselbe Zahl von
/// Positionen kennen. Eine Abweichung zahlte nichts aus, ohne einen
/// Fehler zu melden.
///
/// ⚑ **Auf Modulebene und nicht im `impl`-Block**, und das ist der
/// Unterschied zwischen Prüfung und Zierde: Ein assoziiertes `const`
/// wird erst berechnet, wenn es jemand benutzt. Der erste Anlauf stand
/// im `impl`, und die Gegenprobe zeigte, dass er bei einer Abweichung
/// **nicht** ausgelöst hätte.
const _: () = assert!(
    Kette::PROBE_SHARDS as u64 == myl_tokenomics::vtfe::PROBE_SHARDS,
    "Shardzahl der Kette und der Gewichtsableitung sind auseinandergelaufen"
);

/// ⚑ **Der Name des Probeerzeugers** (2026-10-27): Eine Probekette
/// unterschreibt ihre Blöcke mit dem Konsensschlüssel aus diesem Namen.
/// **Wer den Namen kennt, kennt den Schlüssel**, und das ist auf der
/// Probekette gewollt; sie hat keinen Wert und keinen Anker.
pub const PROBEERZEUGER: &str = "probeerzeuger";

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
        let mut k = Self::im_netz(&crate::netz::PROBEKETTE);
        // ⚑ Seit dem 2026-10-27 braucht jeder Block die Unterschrift eines
        //   Mitglieds; eine Probekette unterschreibt mit dem Probeerzeuger,
        //   dessen Schlüssel jeder aus dem Namen ableiten kann.
        if let Ok(e) = crate::schluessel::Konsensschluessel::probe(PROBEERZEUGER) {
            k.erzeugerschluessel = Some(e);
        }
        k
    }

    /// Der Stimmsatz von aussen (Genesis oder Stimmsatzdatei), sobald die
    /// Kette einen hat. ⚑ Seit 7.8 Schritt 2 (2026-11-04) traegt er die
    /// Erzeugung und die Startphase; die Finalitaet traegt
    /// [`Self::finalitaetssatz`].
    pub fn stimmsatz(&self) -> Option<&myl_consensus::validator::VotingSet> {
        self.stimmsatz.as_ref()
    }

    /// Der Satz, der die Finalitaet der Epoche `epoche` traegt, nach dem
    /// eigenen Zustand ([`Self::finalitaetssatz_von`]).
    pub fn finalitaetssatz(&self, epoche: u64) -> Option<myl_consensus::validator::VotingSet> {
        Self::finalitaetssatz_von(&self.zustand, self.stimmsatz.as_ref(), epoche)
    }

    /// ⚑ **Wer die Finalitaet einer Epoche traegt** (7.8 Schritt 2,
    /// 2026-11-04): Ueber die Pruefpunkte, ihre Streitfaelle und den Topf der
    /// Validatoren entscheidet der Satz aus dem Zustand, alle mit genug
    /// Einsatz und Arbeit; die Bloecke unterschreibt weiter ein Mitglied des
    /// Satzes von aussen. 📌 Im Nachtlauf vom 2026-11-03 stimmten nur die
    /// Mitglieder des Satzes von aussen, und der Satz aus dem Zustand
    /// schrumpfte auf sie: Wer nicht stimmen darf, faellt wegen
    /// Inaktivitaet heraus.
    ///
    /// 1. **Startphase:** Liegt jeder gefuehrte Satz nach `epoche` (auch
    ///    wenn noch keiner gefuehrt wird), gilt der Satz von aussen.
    /// 2. **Danach nur der Satz der Epoche**, und nur, wenn er tragfaehig
    ///    ist ([`myl_ledger::stimmsatz::tragfaehig`]); sonst ruht die
    ///    Finalitaet. ⛔️ Nie zurueck auf den Satz von aussen.
    /// 3. **Nicht aelter als die Vorepoche:** Aeltere Saetze sind
    ///    vergessen, und ein Rueckfall auf den Satz von aussen waere
    ///    dort ein Weg, alte Schluessel wieder sprechen zu lassen.
    pub fn finalitaetssatz_von(
        zustand: &LedgerState,
        von_aussen: Option<&myl_consensus::validator::VotingSet>,
        epoche: u64,
    ) -> Option<myl_consensus::validator::VotingSet> {
        if epoche.saturating_add(1) < zustand.epoch.0 {
            return None;
        }
        if let Some(satz) = myl_ledger::stimmsatz::satz_der_epoche(zustand, epoche) {
            if !myl_ledger::stimmsatz::tragfaehig(satz, Self::satzregeln(zustand).mindestgewicht) {
                return None;
            }
            return Some(myl_consensus::validator::VotingSet::from_members(
                satz.iter().map(|(m, k, w)| (*m, myl_consensus::validator::VotingMember { pubkey: *k, weight: *w })).collect(),
            ));
        }
        if zustand.stimmsaetze.keys().all(|e| *e > epoche) {
            return von_aussen.cloned();
        }
        None
    }

    /// **Eine frische Kette im gegebenen Netz** (2026-10-22): wie
    /// [`Kette::probestand`], mit Startwert und Podmodell dieses Netzes.
    ///
    /// ⚑ **Das Podmodell steht ab dem ersten Block im Zustand** und geht
    /// in jede Zustandswurzel ein; zwei Knoten verschiedener Netze trennen
    /// sich damit sofort, auch wenn eine Transaktion durchkäme.
    pub fn im_netz(netz: &'static crate::netz::Netz) -> Self {
        let mut zustand = LedgerState::genesis(PROBE_CREDIT_PREIS);
        zustand.podmodell = Some(netz.podmodell);
        // Die Testkonten ausstatten. Ohne Guthaben scheitert jede
        // Burn-Transaktion, der Zustand bleibt unverändert, und die
        // Übereinstimmung der Wurzeln belegt nichts (siehe
        // [`probekonto`]).
        for n in 0..PROBEKONTEN {
            zustand.account_mut(&probekonto(n)).balance = PROBEGUTHABEN;
        }
        // ⚑ **Ein Probekorpus, damit die Trainingspods etwas zu tun
        // haben** (2026-09-05). Ohne Anker bildet die Kette
        // Trainingspods und weist ihnen nichts zu; der Weg von der
        // Zuteilung bis zum Bündel wäre dann von keinem Test berührt.
        //
        // **Die Wurzel ist ein Platzhalter und keine echte Verankerung.**
        // Diese Kette ist eine Probekette; ein echtes Netz setzt hier
        // die Wurzel eines wirklich verankerten Korpus, und wie er sich
        // ändern lässt, steht in `myl_types::korpusanker`.
        zustand.korpus = Some(myl_types::korpusanker::Korpusanker {
            kennung: "probekorpus".into(),
            wurzel: Hash::sha256(b"myelith-probekorpus"),
            segmente: 65_536,
            buendel: 256,
            tokens_je_segment: 2_048,
        });
        // ⚑ **Die Saat der ersten beiden Epochen ist der Startwert**
        // (Fund 143). Der Ledger kennt ihn nicht, die Kette schon; ohne
        // ihn stünde dort eine Null, die in jedem Netz dieselbe wäre.
        zustand.epochensaat = netz.startwert_hash();
        zustand.epochensaat_naechste = netz.startwert_hash();
        Self {
            hoehe: 0,
            // Nicht das Literal wiederholen: Zwei Stellen mit derselben
            // Zeichenkette laufen irgendwann auseinander, und dann
            // lehnte die Kettendatei ihre eigene Kette ab.
            letzter_hash: netz.startwert_hash(),
            zustand,
            stimmsatz: None,
            erzeugerschluessel: None,
            nur_nach_plan: false,
            lernt_erzeuger: netz.lernt_erzeuger,
            naechstes_pruefpunktzertifikat: None,
            offene_zuteilungen: Vec::new(),
            letzte_stichprobe: None,
            pods_der_stichprobe: Vec::new(),
            pruefer_der_stichprobe: Vec::new(),
            pruefausgaben: Pruefausgaben::default(),
            zustandslos: false,
            eigene_zeugen: std::collections::VecDeque::new(),
            zeugenlos: 0,
            zeugenschluessel: None,
            zeugenstuecke: std::collections::VecDeque::new(),
            zeugenablage: None,
            zeugenablage_fehler: 0,
            eigene_konten: std::collections::BTreeMap::new(),
            eigene_unbekannt: std::collections::BTreeSet::new(),
            zertifikate: std::collections::VecDeque::new(),
            wechselbelege: std::collections::BTreeMap::new(),
            belege_ohne_folge: std::collections::BTreeSet::new(),
            anker_von_aussen: None,
            belegablage: None,
            belege_ab: None,
            belege_gefallen: 0,
            netzstart_ms: None,
            mempool: Mempool::default(),
            unterschriften: Unterschriften::default(),
            bekannt: std::collections::HashSet::new(),
            verlauf: std::collections::BTreeMap::new(),
            speicher: None,
            schreibfehler: 0,
            lesefehler: 0,
            ablage: None,
            basis: 0,
            letzter_abzug: None,
            budget: 0,
            verdichtet: None,
            archivkennung: None,
            baum: Some(Default::default()),
            teilcache: None,
            teilgruppe: std::collections::VecDeque::new(),
            rueckwege: std::collections::VecDeque::new(),
            schranke: None,
        }
    }

    /// ⚑ **Eine Zusage ab Epoche 0, unmittelbar in den Zustand** (Stufe 4,
    /// 2026-10-22): für Proben, die ihre Miner ebenso unmittelbar anmelden
    /// und in Epoche 0 Pods brauchen. Alle Ebenen samt Kopf, 300 µs je
    /// Ebene. Über die Kette gilt eine Zusage erst ab der nächsten Epoche
    /// ([`probezusage`]); **kein Weg für einen Betrieb**, denn er umgeht
    /// Unterschrift und Prüfung.
    pub fn probezusage_setzen(&mut self, kennung: myl_types::ids::MinerId) {
        let pm = self.netz().podmodell;
        let z = myl_types::zusage::Kapazitaetszusage::neu(kennung, EpochId(0), 0).mit_modell(
            myl_types::zusage::Modellzusage {
                abdruck: pm.abdruck,
                ebenen_hoechstens: pm.ebenen_samt_kopf(),
                ebenenzeit_us: 300,
                gehalten: None,
            },
        );
        self.zustand.zusagen.entry(kennung).or_default().insert(0, z);
    }

    /// Das Netz dieser Kette, aus dem Podmodell in ihrem Zustand.
    pub fn netz(&self) -> &'static crate::netz::Netz {
        crate::netz::Netz::des_zustands(&self.zustand)
    }

    /// Der Startwert **dieser** Kette, aus ihrem Netz. Bindet Kettendatei
    /// und Transaktionen an das Netz.
    pub fn netzwert(&self) -> Hash {
        self.netz().startwert_hash()
    }

    /// Der Startwert der Probekette.
    ///
    /// Bindet eine Kettendatei an ihr Netz: Eine Datei mit anderem
    /// Startwert wird abgewiesen, statt eine fremde Historie als eigene
    /// auszugeben. ⚠️ Für eine Kette eines anderen Netzes gilt
    /// [`Kette::netzwert`].
    pub fn startwert() -> Hash {
        // ⚑ **Aus [`PROBE_STARTWERT`], nicht aus einem eigenen Literal**
        // (Fund 163, 2026-09-03). Bis dahin stand hier
        // `sha256(b"myelith-testkette-genesis")`, und `PROBE_STARTWERT`
        // wurde **nur in Kommentaren erwähnt**: Der Doc-Kommentar
        // erklärte ihn zum „Riegel gegen eine Verwechslung mit dem
        // echten Netz" und schloss mit „Der Text sagt es auch dem, der
        // die Bytes anschaut". Nur sagte der wirklich benutzte Text
        // genau das nicht.
        Hash::sha256(PROBE_STARTWERT)
    }

    /// Hängt ein Blockprotokoll an diese Kette.
    ///
    /// Ab jetzt wird jeder aufgenommene Block geschrieben. **Blöcke, die
    /// vorher schon in der Kette waren, werden nicht nachgetragen**: Der
    /// vorgesehene Weg ist, den Speicher vor dem ersten Block zu setzen
    /// und die Datei danach nachzuspielen.
    pub fn speicher_setzen(&mut self, speicher: crate::speicher::Kettenspeicher) {
        self.speicher = Some(speicher);
    }

    /// Das Platzbudget der Kettendatei, in Bytes; null heißt ohne Grenze.
    pub fn budget_setzen(&mut self, bytes: u64) {
        self.budget = bytes;
    }

    /// Die eigene Kennung als Miner, für die Auslosung der Archivknoten.
    pub fn archivkennung_setzen(&mut self, kennung: Option<myl_types::ids::MinerId>) {
        self.archivkennung = kennung;
    }

    /// (behält jetzt alles, ist Kopie der laufenden Periode), aus dem
    /// Archivstand des Zustands.
    pub fn archiv(&self) -> (bool, bool) {
        let ich = self.archivkennung.as_ref();
        (self.zustand.archiv.behaelt(ich), ich.is_some_and(|m| self.zustand.archiv.ist_kopie(m)))
    }

    /// **Wer die Geschichte haben sollte**: die Netzadressen der Halter (sie
    /// behalten, bis die neuen Kopien belegt haben), dann der Kopien, die
    /// schon vollständig belegt haben, dann der übrigen. Ohne die eigene.
    pub fn archivkopien(&self) -> Vec<myl_types::latency_attest::PeerIdBytes> {
        let a = &self.zustand.archiv;
        let vollstaendig = a.kopien.iter().filter(|k| a.vollstaendig.contains(k));
        let uebrige = a.kopien.iter().filter(|k| !a.vollstaendig.contains(k));
        let mut aus = Vec::new();
        for k in a.halter.iter().chain(vollstaendig).chain(uebrige) {
            if Some(*k) == self.archivkennung {
                continue;
            }
            if let Some(m) = self.zustand.miner.get(k) {
                if !aus.contains(&m.netzadresse) {
                    aus.push(m.netzadresse);
                }
            }
        }
        aus
    }

    /// Die eigene Archivrolle.
    pub fn archivrolle(&self) -> myl_types::archivwahl::Archivrolle {
        crate::archiv::rolle(&self.zustand.archiv, self.archivkennung.as_ref())
    }

    /// Eine gezogene Kopie, der die Geschichte vor ihrer Basis fehlt: Sie
    /// muss sie holen. Ohne Ziehung muss keiner nachladen, denn dann hat
    /// auch keiner verdichtet.
    pub fn braucht_geschichte(&self) -> bool {
        self.archivkennung.as_ref().is_some_and(|m| self.zustand.archiv.ist_kopie(m)) && self.basis > 0
    }

    /// **Der eigene Archivbeleg dieser Epoche** (Stufe A5): die Blöcke der
    /// Stichproben als Bytes und je ein Beweis gegen die Historienwurzel der
    /// Grenze. `None`, wenn `wer` nicht Kopie ist, schon belegt hat oder
    /// dieser Kette die Geschichte fehlt.
    pub fn archivbeleg(&mut self, wer: &myl_types::ids::MinerId) -> Option<Anweisung> {
        let a = &self.zustand.archiv;
        if !a.ist_kopie(wer) || a.belegt.contains(wer) {
            return None;
        }
        let grenze = a.grenze;
        let hoehen = myl_ledger::archiv::stichproben(
            a,
            &self.zustand.epochensaat,
            self.zustand.epoch.0,
            wer,
            a.stichprobenzahl(wer),
        );
        let baum = self.baum.take()?;
        let mut bloecke = Vec::with_capacity(hoehen.len());
        let mut beweise = Vec::with_capacity(hoehen.len());
        let mut ok = true;
        for &h in &hoehen {
            let block = self.bloecke_von_bis(h, h).pop().filter(|b| b.header.height == h);
            let beweis = baum.beweis_bei(h, grenze, |g| self.gruppenblaetter(g));
            // Die Bytes, die der Blockhash deckt: Die Kette prüft sie gegen
            // die Historie der Hashes (nicht die volle Kodierung mit der
            // Unterschrift des Erzeugers).
            match (block.map(|b| b.hashbytes()), beweis) {
                (Some(roh), Some(bw)) => {
                    bloecke.push(roh);
                    beweise.push(bw);
                }
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        self.baum = Some(baum);
        ok.then_some(Anweisung::ArchivBelegen { bloecke, beweise })
    }

    /// Die Blöcke der Gruppe `g` als Bytes; `None`, wenn diese Kette sie
    /// nicht ganz hat.
    pub fn gruppenbloecke(&mut self, g: u64) -> Option<Vec<Vec<u8>>> {
        let hoehen = myl_types::archivteile::gruppenhoehen(g);
        let (ab, bis) = (*hoehen.start(), *hoehen.end());
        if bis > self.hoehe {
            return None;
        }
        let mut aus = Vec::with_capacity((bis - ab + 1) as usize);
        let mut h = ab;
        while h <= bis {
            let teil = self.bloecke_von_bis(h, bis);
            if teil.first()?.header.height != h {
                return None;
            }
            for b in &teil {
                aus.push(borsh::to_vec(b).ok()?);
            }
            h += teil.len() as u64;
        }
        Some(aus)
    }

    /// Die 64 Teile der Gruppe `g`, aus dem Zwischenspeicher oder frisch.
    fn gruppenteile(&mut self, g: u64) -> Option<Vec<Vec<u8>>> {
        if let Some((_, t)) = self.teilgruppe.iter().find(|(x, _)| *x == g) {
            return Some(t.clone());
        }
        let teile = myl_types::archivteile::kodieren(&self.gruppenbloecke(g)?);
        if self.teilgruppe.len() >= TEILGRUPPEN_BEHALTEN {
            self.teilgruppe.pop_front();
        }
        self.teilgruppe.push_back((g, teile.clone()));
        Some(teile)
    }

    /// **Die Gruppenwurzeln des laufenden Abschnitts**, einmal je Grenze
    /// gerechnet, am Stück oder fertig aus den Scheiben; `None`, wenn dieser
    /// Kette Blöcke fehlen.
    fn gruppenwurzeln(&mut self) -> Option<Vec<Hash>> {
        self.gruppenwurzeln_bis(None)
    }

    /// ⚑ **Rechnet an den Gruppenwurzeln weiter, in Scheiben** (Archivknoten):
    /// höchstens `frist` lang, mindestens eine Gruppe; `true`, wenn alle
    /// Gruppen des Abschnitts gerechnet sind. Das Ergebnis gleicht der
    /// Rechnung am Stück.
    ///
    /// 📌 **Am Stück stand die Rechnung im Ereignisweg**, einmal je Periode
    /// (112 Gruppen zu 1024 Blöcken): gemessen 5 ms je Gruppe bei 300 Byte
    /// je Block, 16 ms bei 2 KB, 122 ms bei 20 KB, also 0,6 bis 14 s, in
    /// denen der Knoten keinen Block annimmt. Ein eigener Faden bräuchte
    /// einen zweiten Lesegriff auf die Kettendatei; Scheiben je Aufnahme
    /// brauchen nichts davon und kommen in rund einer Minute durch.
    pub fn gruppenwurzeln_vorarbeiten(&mut self, frist: std::time::Duration) -> bool {
        self.gruppenwurzeln_bis(Some(frist)).is_some()
    }

    /// Ob die Gruppenwurzeln des laufenden Abschnitts fertig gerechnet sind.
    pub fn gruppenwurzeln_bereit(&self) -> bool {
        let a = &self.zustand.archiv;
        matches!(&self.teilcache, Some((g, w)) if *g == a.grenze && w.len() as u64 == a.teil_bis.saturating_sub(a.teil_ab))
    }

    /// Die Rechnung hinter beiden: ohne Frist bis zum Ende, mit Frist eine
    /// Scheibe. `Some` nur, wenn alle Gruppen gerechnet sind.
    fn gruppenwurzeln_bis(&mut self, frist: Option<std::time::Duration>) -> Option<Vec<Hash>> {
        let a = &self.zustand.archiv;
        let (grenze, anzahl) = (a.grenze, a.teil_bis.saturating_sub(a.teil_ab));
        let ab = a.teil_ab;
        let mut wurzeln = match self.teilcache.take() {
            Some((g, w)) if g == grenze => w,
            _ => Vec::with_capacity(anzahl as usize),
        };
        let start = std::time::Instant::now();
        while (wurzeln.len() as u64) < anzahl {
            let g = ab + wurzeln.len() as u64;
            let wurzel = self
                .gruppenbloecke(g)
                .and_then(|b| myl_types::archivteile::gruppenwurzel(&myl_types::archivteile::kodieren(&b)));
            let Some(wurzel) = wurzel else {
                self.teilcache = Some((grenze, wurzeln));
                return None;
            };
            wurzeln.push(wurzel);
            if frist.is_some_and(|f| start.elapsed() >= f) {
                break;
            }
        }
        let fertig = wurzeln.len() as u64 == anzahl;
        self.teilcache = Some((grenze, wurzeln));
        if fertig {
            self.teilcache.as_ref().map(|(_, w)| w.clone())
        } else {
            None
        }
    }

    /// **Die Teilwurzel, die eine Kopie nennt** (Stufe A7): `None`, wenn
    /// `wer` nicht Kopie ist, schon genannt hat, die Wurzel schon gilt oder
    /// Blöcke fehlen.
    pub fn teilwurzel_anweisung(&mut self, wer: &myl_types::ids::MinerId) -> Option<Anweisung> {
        let a = &self.zustand.archiv;
        if !a.ist_kopie(wer) || a.teilwurzel.is_some() || a.teil_ab == a.teil_bis || a.teilstimmen.contains_key(wer) {
            return None;
        }
        let w = self.gruppenwurzeln()?;
        Some(Anweisung::ArchivTeilwurzel { wurzel: myl_types::archivteile::teilwurzel(&w)? })
    }

    /// **Teil `teil` der Gruppe an Stelle `stelle`** des laufenden
    /// Abschnitts, mit Beweis: was ein Splitter holt. Nur gegen die
    /// geltende Teilwurzel, damit niemand etwas liefert, das sich nicht
    /// belegen lässt.
    pub fn teilstueck(
        &mut self,
        grenze: u64,
        stelle: u64,
        teil: u64,
    ) -> Option<(Vec<u8>, myl_types::archivteile::Teilbeweis)> {
        let a = &self.zustand.archiv;
        let geltend = a.teilwurzel?;
        if a.grenze != grenze || stelle >= a.teil_bis - a.teil_ab || teil >= myl_types::archivteile::TEILE as u64 {
            return None;
        }
        let g = a.teil_ab + stelle;
        let wurzeln = self.gruppenwurzeln()?;
        if myl_types::archivteile::teilwurzel(&wurzeln)? != geltend {
            return None;
        }
        let teile = self.gruppenteile(g)?;
        let bw = myl_types::archivteile::Teilbeweis::bauen(&teile, teil as usize, &wurzeln, stelle as usize)?;
        Some((teile[teil as usize].clone(), bw))
    }

    /// **Setzt die geprüfte Geschichte vor die Kette** und entfernt die
    /// Basis; die Kette lebt danach wieder ab Genesis. Gibt die Zahl der
    /// eingefügten Blöcke zurück.
    pub fn geschichte_einfuegen(&mut self, archiv: &std::path::Path) -> Result<u64, String> {
        let speicher = self.speicher.as_mut().ok_or("ohne Kettendatei")?;
        let n = speicher.vorne_einfuegen(archiv).map_err(|e| e.to_string())?;
        if let Some(a) = self.ablage.as_ref() {
            a.basis_entfernen().map_err(|e| e.to_string())?;
        }
        self.basis = 0;
        self.baum_aufbauen()?;
        Ok(n)
    }

    /// **Baut den Historienbaum aus der Kettendatei neu**, nach dem
    /// Einfügen der Geschichte. Er muss die Historie des Zustands treffen;
    /// sonst stimmt die Datei nicht mit dem Zustand überein, und der Baum
    /// bleibt weg, statt falsche Beweise zu geben.
    fn baum_aufbauen(&mut self) -> Result<(), String> {
        let hoehe = self.hoehe;
        let mut baum = myl_types::historie::Historienbaum::default();
        baum.anhaengen(&self.netz().startwert_hash());
        let speicher = self.speicher.as_mut().ok_or("ohne Kettendatei")?;
        speicher
            .fuer_jeden_satz(|b| {
                if b.header.height < hoehe {
                    baum.anhaengen(&b.hash());
                }
            })
            .map_err(|e| e.to_string())?;
        if baum.historie() != &self.zustand.historie {
            return Err("die Kettendatei trifft die Historie des Zustands nicht".into());
        }
        self.baum = Some(baum);
        Ok(())
    }

    /// **Eine Lieferung für jemanden, der die Geschichte nachlädt**
    /// (Stufe A4): die Blöcke `ab..=bis`, gekürzt auf eine Antwort, und der
    /// Beweis des letzten gegen die Historie mit `anzahl` Blöcken. `None`,
    /// wenn diese Kette nicht ab Genesis lebt oder der Bereich nicht vor
    /// `anzahl` liegt.
    pub fn archivlieferung(
        &mut self,
        ab: u64,
        bis: u64,
        anzahl: u64,
    ) -> Option<(Vec<Block>, myl_types::historie::Beweis)> {
        let n = self.baum.as_ref()?.historie().anzahl();
        if ab == 0 || ab > bis || bis >= anzahl || anzahl > n {
            return None;
        }
        let bloecke = crate::nachschub::in_die_antwort_mit(
            self.bloecke_von_bis(ab, bis),
            crate::nachschub::BEWEIS_HOECHSTENS_BYTES,
        );
        let letzte = bloecke.last()?.header.height;
        if bloecke.first()?.header.height != ab {
            return None;
        }
        let baum = self.baum.take()?;
        let beweis = baum.beweis_bei(letzte, anzahl, |g| self.gruppenblaetter(g));
        self.baum = Some(baum);
        Some((bloecke, beweis?))
    }

    /// Die Blatt-Hashes der Gruppe `g`: Blatt `i` ist der Hash von Block
    /// `i`, Blatt 0 der Startwert.
    fn gruppenblaetter(&mut self, g: u64) -> Option<Vec<Hash>> {
        use myl_types::historie::{blatt, GRUPPENSTUFE};
        let groesse = 1u64 << GRUPPENSTUFE;
        let erstes = g.checked_mul(groesse)?;
        let mut blaetter = Vec::with_capacity(groesse as usize);
        let mut h = erstes;
        if h == 0 {
            blaetter.push(blatt(&self.netz().startwert_hash()));
            h = 1;
        }
        while h < erstes + groesse {
            let teil = self.bloecke_von_bis(h, erstes + groesse - 1);
            if teil.first()?.header.height != h {
                return None;
            }
            for b in &teil {
                blaetter.push(blatt(&b.hash()));
            }
            h += teil.len() as u64;
        }
        Some(blaetter)
    }

    /// Die Ablage der Teile, wenn dieser Knoten Splitter ist.
    pub fn teilablage(&self) -> Option<crate::teilablage::Teilablage> {
        Some(crate::teilablage::Teilablage::neben(self.speicher.as_ref()?.pfad()))
    }

    /// Wo eine nachgeladene Geschichte liegt, bis sie eingefügt ist.
    pub fn archivpfad(&self) -> Option<std::path::PathBuf> {
        let p = self.speicher.as_ref()?.pfad();
        let mut n = p.as_os_str().to_owned();
        n.push(".archiv");
        Some(std::path::PathBuf::from(n))
    }

    /// Die letzte Verdichtung (neue Basis, Bytes vorher, nachher), einmal.
    pub fn verdichtung_nehmen(&mut self) -> Option<(u64, u64, u64)> {
        self.verdichtet.take()
    }

    /// Die Ablage der Abzüge; ab jetzt entsteht an jeder Epochengrenze einer.
    pub fn ablage_setzen(&mut self, ablage: crate::abzug::Ablage) {
        // ⚑ Die Wechselbelege daneben, mit dem, was schon dort liegt; ein
        //   spaeterer derselben Epoche hat einen ersetzt ([`Self::beleg_merken`]).
        let belege = crate::wechsel::Belegablage::in_ordner(ablage.ordner());
        for b in belege.lesen() {
            self.wechselbelege.insert(b.epoche(), b);
        }
        self.belegablage = Some(belege);
        self.belege_ab = self.wechselbelege.keys().next().copied().filter(|e| *e > 0);
        if let Some(e) = self.wechselbelege.keys().next_back().copied() {
            self.belege_kuerzen(e);
        }
        self.ablage = Some(ablage);
    }

    /// Die Ablage der Abzüge, falls eine geführt wird.
    pub fn ablage(&self) -> Option<&crate::abzug::Ablage> {
        self.ablage.as_ref()
    }

    /// Die Höhe, ab der diese Kette lebt (0 von Genesis).
    pub fn basis(&self) -> u64 {
        self.basis
    }

    /// Der zuletzt angelegte Abzug: Höhe und Blockhash.
    pub fn letzter_abzug(&self) -> Option<(u64, Hash)> {
        self.letzter_abzug
    }

    /// ⚑ **Die Kette beginnt beim Abzug** statt bei Genesis (2026-10-22).
    ///
    /// Nur auf einer leeren Kette, deren Kettendatei leer ist: Sonst
    /// schlösse der erste geschriebene Block an etwas an, das die Datei
    /// nicht kennt. Speicher, Ablage, Stimmsatz und Mempool bleiben; Höhe,
    /// letzter Hash, Zustand, Fenster und Basis kommen aus dem Abzug.
    /// **Geprüft ist der Abzug vorher**, gegen einen Anker oder gegen den
    /// eigenen Block ([`crate::abzug::Abzug::pruefen`]); hier wird nur
    /// eingesetzt.
    pub fn auf_abzug_setzen(&mut self, abzug: &crate::abzug::Abzug, zustand: LedgerState) -> Result<(), String> {
        // ⚑ Ein zustandsloser Knoten darf vorwärts neu aufsetzen
        //   (Selbstheilung, 2026-10-29); er führt keine Kettendatei.
        let neu_aufsetzen = self.zustandslos && abzug.hoehe() > self.hoehe;
        if self.hoehe != 0 && !neu_aufsetzen {
            return Err(format!("die Kette steht schon auf Höhe {}", self.hoehe));
        }
        if self.speicher.as_ref().is_some_and(|s| s.bloecke() > 0) {
            return Err("die Kettendatei ist nicht leer".into());
        }
        let hash = abzug.block.hash();
        self.hoehe = abzug.hoehe();
        self.letzter_hash = hash;
        self.zustand = zustand;
        self.verlauf.clear();
        self.bekannt.clear();
        self.bekannt.insert(hash);
        self.verlauf_aufnehmen(abzug.hoehe(), &abzug.block);
        self.basis = abzug.hoehe();
        self.baum = None;
        self.naechstes_pruefpunktzertifikat = None;
        self.offene_zuteilungen.clear();
        self.letzte_stichprobe = None;
        self.pods_der_stichprobe.clear();
        self.pruefer_der_stichprobe.clear();
        Ok(())
    }

    /// ⚑ **Vorspulen beim Wiederanlauf** (Fund 629): einen Block aus der
    /// eigenen Kettendatei übernehmen, ohne ihn anzuwenden, bis zu einem
    /// eigenen Abzug ([`Kette::abzug_einsetzen`]). Geprüft wird, was ohne
    /// Rechnung geht: Anschluss, Höhe, Epoche; Fenster, bekannte Hashes und
    /// Historienbaum wachsen wie beim Übernehmen. **Der Zustand steht danach
    /// nicht zur Höhe**, bis der Abzug eingesetzt ist; ausserhalb des
    /// Ladens ruft das niemand.
    pub fn vorspulen(&mut self, block: &Block) -> Result<(), KettenFehler> {
        if block.header.prev_block_hash != self.letzter_hash {
            return Err(KettenFehler::PasstNichtAn { erwartet: self.letzter_hash, bekommen: block.header.prev_block_hash });
        }
        if block.header.height != self.hoehe + 1 {
            return Err(KettenFehler::HoeheWeichtAb { erwartet: self.hoehe + 1, bekommen: block.header.height });
        }
        let epoche = epoche_fuer_hoehe(block.header.height);
        if block.header.epoch != epoche {
            return Err(KettenFehler::EpocheWeichtAb { erwartet: epoche, bekommen: block.header.epoch });
        }
        let hash = block.hash();
        if let Some(b) = self.baum.as_mut() {
            b.anhaengen(&self.letzter_hash);
        }
        self.hoehe = block.header.height;
        self.letzter_hash = hash;
        self.bekannt.insert(hash);
        self.verlauf_aufnehmen(block.header.height, block);
        Ok(())
    }

    /// ⚑ **Den eigenen Abzug einsetzen**, nachdem bis zu seinem Block
    /// vorgespult ist (Fund 629).
    ///
    /// Gilt nur, wenn der vorgespulte letzte Block genau der Block des
    /// Abzugs ist; dessen Zustandswurzel bindet die Bytes
    /// ([`crate::abzug::Ablage::lesen`] hat sie geprüft), und die Kette von
    /// Hashes bindet den Block an die Basis. Lebt die Kette ab Genesis, muss
    /// zudem der Historienbaum aus der Datei die Historie im Zustand treffen.
    ///
    /// ⚑ **Was hier nicht nachgerechnet wird**, sind die Übergänge bis zum
    /// Abzug: Diese Blöcke hat der Knoten selbst angewandt, bevor er sie
    /// schrieb, und den Abzug danach selbst abgelegt. Ein vollständiges
    /// Nachspielen bei jedem Start bewies das nur noch einmal, und es kostete
    /// mit der Geschichte wachsend: im Archivlauf vom 2026-10-24 166 s für
    /// 349 646 Blöcke, und eine Kopie verlor darüber ihren Platz.
    pub fn abzug_einsetzen(&mut self, abzug: &crate::abzug::Abzug, zustand: LedgerState) -> Result<(), String> {
        if self.hoehe != abzug.hoehe() || self.letzter_hash != abzug.block.hash() {
            return Err(format!("vorgespult bis {}, der Abzug steht bei {}", self.hoehe, abzug.hoehe()));
        }
        if self.baum.as_ref().is_some_and(|b| b.historie() != &zustand.historie) {
            return Err("die Kettendatei trifft die Historie des Abzugs nicht".into());
        }
        self.zustand = zustand;
        Ok(())
    }

    /// Legt an einer Epochengrenze einen Abzug ab, wenn eine Ablage geführt
    /// wird. Ein Fehler beim Schreiben zählt als Schreibfehler und hält die
    /// Kette nicht an: Der Abzug ist eine Gefälligkeit an Neulinge, kein
    /// Teil des Konsenses.
    fn abzug_anlegen(&mut self, block: &Block) {
        if self.zustandslos {
            return;
        }
        let hoehe = block.header.height;
        // ⚑ **Ein leichter Abzug an jedem Prüfpunkt** (zustandslose
        //   Validierung, 2026-10-29): klein (Register, Kopf, grosse Wurzel),
        //   damit ein zustandsloser Neuling höchstens rund 150 Blöcke mit
        //   Zeugen nachrechnet.
        if myl_consensus::pruefpunkt::ist_pruefpunkt(hoehe) {
            if let Some(leicht) = self.ablage.as_ref().map(|a| a.leicht()) {
                let zustand = self.zustand.leicht_bytes();
                if leicht.ablegen(&crate::abzug::Abzug { block: block.clone(), zustand }).is_err() {
                    self.schreibfehler += 1;
                }
            }
        }
        if !crate::abzug::ist_grenze(hoehe) {
            return;
        }
        let Some(ablage) = self.ablage.as_ref() else {
            return;
        };
        let Ok(zustand) = borsh::to_vec(&self.zustand) else {
            self.schreibfehler += 1;
            return;
        };
        let abzug = crate::abzug::Abzug { block: block.clone(), zustand };
        match ablage.ablegen_und_halten(&abzug, self.zustand.pruefpunkt_bestaetigt) {
            Ok(()) => self.letzter_abzug = Some((hoehe, block.hash())),
            Err(_) => self.schreibfehler += 1,
        }
        if crate::abzug::ist_langzeit(hoehe) {
            self.verdichten_wenn_noetig();
        }
    }

    /// **Die Kettendatei unter ihr Budget bringen** (Funde 624 und 625,
    /// Festlegung des Projektinhabers vom 2026-10-23).
    ///
    /// Jeder Knoten, der nicht Kopie ist ([`crate::archiv`]), behält
    /// alles ab einem Langzeitabzug, und zwar **wenigstens ab dem vorletzten**.
    /// Liegt die Datei über dem Budget, wird der älteste Langzeitabzug zur
    /// Basis, nach dem höchstens noch die Hälfte des Budgets liegt; so
    /// läuft das selten und lässt reichlich Geschichte stehen. Reicht auch
    /// der vorletzte nicht, gilt er trotzdem: Die Mindestmenge geht vor dem
    /// Budget.
    ///
    /// Danach beginnt die Datei hinter der neuen Basis, genau wie bei einem
    /// Neuling aus einem Abzug, und ein Neustart rechnet ab dort nach statt
    /// ab Genesis. Ein Fehler zählt als Schreibfehler und lässt die alte
    /// Datei stehen.
    fn verdichten_wenn_noetig(&mut self) {
        let (Some(speicher), Some(ablage)) = (self.speicher.as_mut(), self.ablage.as_ref()) else {
            return;
        };
        let vorher = speicher.laenge();
        if self.budget == 0 || vorher <= self.budget {
            return;
        }
        // ⚑ Wer laut Zustand hält (Kopie, Halter, jeder vor der ersten
        //   Übergabe), verdichtet nicht.
        if self.zustand.archiv.behaelt(self.archivkennung.as_ref()) {
            return;
        }
        let langzeit: Vec<u64> = ablage
            .hoehen()
            .into_iter()
            .filter(|&h| crate::abzug::ist_langzeit(h) && h > self.basis)
            .collect();
        let Some(&spaetestens) = langzeit.len().checked_sub(2).and_then(|i| langzeit.get(i)) else {
            return;
        };
        let neue_basis = langzeit
            .iter()
            .copied()
            .filter(|&h| h <= spaetestens)
            .find(|&h| speicher.bytes_nach(h) <= self.budget / 2)
            .unwrap_or(spaetestens);
        let ergebnis = speicher
            .verdichten_vorbereiten(neue_basis)
            .map_err(|e| e.to_string())
            .and_then(|_| ablage.basis_setzen(neue_basis).map_err(|e| e.to_string()))
            // ⚑ Selbst gerechnet, also bestätigt: Sonst begänne der nächste
            //   Neustart eine Hintergrundprüfung ab Genesis, die niemand mehr
            //   bedienen kann.
            .and_then(|()| ablage.bestaetigt_setzen(neue_basis).map_err(|e| e.to_string()))
            .and_then(|()| speicher.verdichten_abschliessen().map_err(|e| e.to_string()));
        match ergebnis {
            Ok(()) => {
                self.basis = neue_basis;
                self.baum = None;
                let _ = ablage.raeumen(self.zustand.pruefpunkt_bestaetigt);
                self.verdichtet = Some((neue_basis, vorher, speicher.laenge()));
            }
            Err(_) => self.schreibfehler += 1,
        }
    }

    /// Wie oft das Schreiben fehlschlug.
    pub fn schreibfehler(&self) -> u64 {
        self.schreibfehler
    }

    /// Wie oft ein Block aus der Datei nicht zurückkam. Siehe Feld
    /// `lesefehler`.
    pub fn lesefehler(&self) -> u64 {
        self.lesefehler
    }

    /// Wie viele Blöcke die Datei führt, falls eine geführt wird.
    pub fn gespeicherte_bloecke(&self) -> Option<u64> {
        self.speicher.as_ref().map(|s| s.bloecke())
    }

    /// Schreibt einen Block, falls ein Speicher da ist.
    fn schreibe(&mut self, block: &Block) {
        let Some(s) = self.speicher.as_mut() else {
            return;
        };
        if s.anhaengen(block).is_err() {
            self.schreibfehler += 1;
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
    ///
    /// Diese Fassung rechnet alles neu (für Tests und seltene Prüfungen); im
    /// Betrieb gilt [`Self::wurzel`].
    pub fn zustandswurzel(&self) -> Hash {
        self.zustand.commitment()
    }

    /// Die Zustandswurzel, fortgeschrieben über den Zustandsbaum (je Block
    /// nur das Geänderte; gleich [`Self::zustandswurzel`]).
    pub fn wurzel(&mut self) -> Hash {
        self.zustand.wurzel()
    }

    /// **Hält den Mempool klein** (Fund 627): Was seine Nummer im Zustand
    /// schon verbraucht hat, kann nie mehr gelten, und was länger als
    /// [`WARTEN_HOECHSTENS_HOEHEN`] wartet, kommt nicht mehr.
    ///
    /// ⚑ Das ersetzt den Vergleich über die Bytes, der je Block jede
    /// wartende Transaktion serialisierte. Im Archivlauf vom 2026-10-24
    /// lagen auf den übernehmenden Knoten über 5000 davon, weil jede, die
    /// nach ihrem Block über Gossip ankam, nie gestrichen wurde; die Knoten
    /// fielen dabei hinter den Erzeuger zurück. Was ein Block enthält, hat
    /// seine Nummer verbraucht, fällt hier also ebenso heraus.
    fn mempool_bereinigen(&mut self, hoehe: u64) {
        let boden: std::collections::HashMap<Address, u64> =
            self.mempool.absender().map(|a| (a, self.nonce_boden(&a))).collect();
        self.mempool.behalten(|w| {
            hoehe.saturating_sub(w.seit) < WARTEN_HOECHSTENS_HOEHEN && w.tx.nonce >= boden.get(&w.absender).copied().unwrap_or(0)
        });
    }

    /// Nimmt einen Block in den Zwischenspeicher und wirft heraus,
    /// was aus dem Fenster fällt.
    ///
    /// ⚑ **`bekannt` wird mit gekürzt**, und zwar über den Hash des
    /// Blocks, der geht. Sonst wüchse die Hashmenge weiter, und das
    /// Fenster hätte nur die Hälfte des Speichers gebunden.
    ///
    /// **Was das für die Dublettenerkennung heißt:** Ein Block, der
    /// älter ist als das Fenster und ein zweites Mal ankommt, wird
    /// nicht mehr mit [`KettenFehler::SchonBekannt`] abgewiesen,
    /// sondern mit [`KettenFehler::PasstNichtAn`]. **Abgewiesen wird er
    /// so oder so**, denn seine Höhe liegt hinter der eigenen und sein
    /// Vorgängerhash passt nicht auf den letzten. Nur der Grund im
    /// Protokoll ist ein anderer, und der genauere ist ohnehin der
    /// zweite.
    fn verlauf_aufnehmen(&mut self, hoehe: u64, block: &Block) {
        self.verlauf.insert(hoehe, block.clone());
        while self.verlauf.len() > VERLAUFSFENSTER {
            let Some((_, alt)) = self.verlauf.pop_first() else {
                break;
            };
            self.bekannt.remove(&alt.hash());
        }
    }

    /// Die Blöcke der Höhen `ab` bis einschließlich `bis`, soweit
    /// vorhanden, aufsteigend.
    ///
    /// Lücken werden **übersprungen, nicht aufgefüllt**: Wer nur einen
    /// Teil hat, liefert diesen Teil. Der Fragende merkt es daran, dass
    /// sein Rückstand nicht ganz verschwindet, und fragt weiter.
    ///
    /// ⚑ **Erst das Fenster, dann die Platte** (Fund 124). Der
    /// Zwischenspeicher hält nur die jüngsten
    /// [`VERLAUFSFENSTER`] Blöcke; alles davor kommt aus der
    /// Kettendatei, falls eine geführt wird. **Wer mit `--ohne-kette`
    /// fährt, liefert nur das Fenster**, und ein Nachzügler, der weiter
    /// zurückliegt, muss einen anderen fragen. Das ist der Preis dafür,
    /// nichts zu behalten, und er steht in der Hilfe zu `--ohne-kette`.
    ///
    /// **Der häufige Fall kostet keine Platte:** Ein Nachzügler fragt
    /// die jüngsten Blöcke nach, und die stehen im Fenster.
    pub fn bloecke_von_bis(&mut self, ab: u64, bis: u64) -> Vec<Block> {
        if ab > bis {
            return Vec::new();
        }
        // ⚑ **Die Spanne wird gedeckelt, bevor irgendetwas geholt
        // wird** (Fund 141). `ab` und `bis` kommen von einer
        // Gegenstelle, und eine Gegenstelle muss sich nicht an
        // [`crate::nachschub::Nachforderung::fuer_rueckstand`] halten.
        // Ohne diesen Deckel wäre `Bloecke { ab: 0, bis: u64::MAX }`
        // eine Anfrage, die eine ganze Kette in **eine** Antwort packt,
        // also ein Verstärker: ein paar Bytes hinein, Megabyte hinaus.
        // Mehr als eine Lieferung fordert ohnehin niemand an.
        let bis = bis.min(ab.saturating_add(crate::nachschub::MAX_BLOECKE_JE_LIEFERUNG - 1));
        // Was im Fenster liegt, ist umsonst zu haben.
        let aus_dem_fenster: Vec<Block> =
            self.verlauf.range(ab..=bis).map(|(_, b)| b.clone()).collect();
        // Die Grenze, unterhalb derer das Fenster nichts mehr weiß.
        let untergrenze = self.verlauf.keys().next().copied().unwrap_or(u64::MAX);
        if ab >= untergrenze {
            return aus_dem_fenster;
        }
        let Some(speicher) = self.speicher.as_mut() else {
            return aus_dem_fenster;
        };
        // Bis zur Untergrenze des Fensters, damit kein Block zweimal
        // in die Antwort kommt.
        let bis_datei = bis.min(untergrenze - 1);
        // Ein Lesefehler bricht die Lieferung ab und wird gezählt; was das
        // Fenster hat, geht trotzdem hinaus.
        let mut aus_der_datei = match speicher.bloecke_ab(ab, bis_datei) {
            Ok(b) => b,
            Err(_) => {
                self.lesefehler += 1;
                Vec::new()
            }
        };
        aus_der_datei.extend(aus_dem_fenster);
        aus_der_datei
    }

    /// Der Zustand zum Ändern, **nur für Tests und den Betreiber**.
    ///
    /// ⚑ **Kein Weg für Teilnehmer.** Wer den Zustand von außen ändern
    /// kann, umgeht jede Übergangsprüfung; deshalb steht hier
    /// ausdrücklich, wofür das da ist. Alles, was ein Absender darf,
    /// geht über eine Anweisung und über `anwenden`.
    #[doc(hidden)]
    pub fn zustand_mut(&mut self) -> &mut LedgerState {
        &mut self.zustand
    }

    /// Der Ledger-Zustand, für Diagnose und Tests.
    pub fn zustand(&self) -> &LedgerState {
        &self.zustand
    }

    /// Anzahl wartender Transaktionen.
    pub fn wartend(&self) -> usize {
        self.mempool.len()
    }

    /// Nimmt eine Transaktion in den Mempool; `false`, wenn ihre Nummer
    /// schon verbraucht ist oder der Mempool voll ist.
    ///
    /// Ob sie durchgeht, entscheidet sich beim Anwenden. Eine Vorprüfung
    /// hier bräuchte den Zustand **zum Zeitpunkt der Aufnahme**, und der
    /// ist nicht der beim Anwenden. ⚑ **Bis auf die Nummer** (Fund 627):
    /// Sie wächst nur, also gilt eine Transaktion unter der Nummer im
    /// Zustand auch später nie. Dazu die Grenzen aus Fund 633: insgesamt
    /// [`MEMPOOL_HOECHSTENS`], je Absender [`MEMPOOL_JE_ABSENDER`].
    /// Wie [`Self::aufnehmen`], für eine geprüfte Transaktion: Ihre Kennung
    /// kommt in den Speicher der geprüften Unterschriften, und der Block
    /// prüft sie nicht noch einmal.
    pub fn aufnehmen_geprueft(&mut self, g: myl_consensus::block::GepruefteTransaktion) -> bool {
        let tx = g.inhalt().clone();
        self.unterschriften.merken(tx.kennung());
        self.aufnehmen(tx)
    }

    pub fn aufnehmen(&mut self, tx: Transaktion) -> bool {
        // ⚑ Ein zustandsloser Knoten führt keinen Mempool: Er erzeugt keine
        //   Blöcke und kann fremde Nummern nicht prüfen.
        if self.zustandslos {
            return false;
        }
        let absender = tx.absender_adresse();
        let neuling = ist_neuling(&self.zustand, &absender);
        if tx.nonce < self.nonce_boden(&absender) || (neuling && self.mempool.neulinge >= MEMPOOL_NEULINGE_HOECHSTENS) {
            return false;
        }
        // 📌 **Eine niedrigere Nummer geht vor** (`kurz-gabel`, 2026-11-05):
        //   Nach einem Zweigwechsel kommen die Transaktionen des verlassenen
        //   Zweigs zurück, und unter Last liegen vom selben Absender schon
        //   64 spätere. Wurde die frühere abgewiesen, klaffte eine Lücke, die
        //   späteren verfielen nach `WARTEN_HOECHSTENS_HOEHEN`, und 320
        //   angenommene Transaktionen kamen in keinen Block. Jetzt fällt dafür
        //   das Ende seiner Nummernkette, nie die Mitte.
        if self.mempool.je_absender(&absender) >= MEMPOOL_JE_ABSENDER && !self.mempool.ende_tauschen(&absender, tx.nonce) {
            return false;
        }
        let bytes = borsh::to_vec(&tx).map_or(usize::MAX, |b| b.len());
        let w = Wartend { seit: self.hoehe, tx, absender, bytes, neuling };
        if self.mempool.voll() && !self.mempool.verdraengen(&w) {
            return false;
        }
        self.mempool.hinein(w);
        true
    }

    /// Liest eine Transaktion aus Gossip-Bytes und nimmt sie auf.
    pub fn aufnehmen_roh(&mut self, daten: &[u8]) -> bool {
        let mut rest = daten;
        match Transaktion::deserialize(&mut rest) {
            Ok(tx) if rest.is_empty() => self.aufnehmen(tx),
            _ => false,
        }
    }

    /// Die Prägeparameter einer Epoche.
    ///
    /// ⚑ **Fest und nicht aus Governance**: Ein Parameter, der sich ändern
    /// kann, gehört an eine Stelle, an der beide Seiten dieselbe Änderung
    /// sehen; die Registry ist noch nicht an die Kette gebunden. Bis dahin
    /// ist ein fester Plan ehrlicher als ein beweglicher Wert, den nur einer
    /// kennt.
    ///
    /// Die Subventionsrate folgt dem Anlaufplan
    /// ([`myl_tokenomics::ANLAUFPLAN`], Whitepaper 5.2 und 5.7), auf jeder
    /// Kette gleich: eine Probekette mit anderen Regeln belegte nichts.
    fn praegeparameter(epoche: u64) -> myl_tokenomics::MintParams {
        let (subsidy_num, subsidy_den) = myl_tokenomics::anlaufrate(epoche);
        myl_tokenomics::MintParams { subsidy_num, subsidy_den, m_max: u64::MAX }
    }

    // ⚑ **`arbeitsverteilung_setzen` ist am 2026-09-03 entfernt worden**
    // (Fund 161). Der Kommentar hier sagte es fast richtig: „Eine
    // Anweisung wäre schlimmer als keine, sie stünde jedem Absender
    // offen." **Was er nicht sagte:** Auch als „dem Betreiber
    // vorbehalten" war es keine Blockanwendung, sondern eine direkte
    // Mutation des Zustands ausserhalb jeder Wiederherstellung aus
    // Blöcken. Auf einem echten Knoten gerufen, hätte sie dessen
    // Zustandswurzel sofort von jeder anderen getrennt, unabhängig
    // davon, wer sie rufen durfte.
    //
    // **Die Lösung ist keine Anweisung, sondern gar kein Zustand:**
    // `myl_tokenomics::vtfe::arbeitsverteilung_probe` rechnet dieselben
    // Gewichte aus denselben öffentlichen Grunddaten, auf jedem Knoten
    // gleich, ohne dass sie je gesetzt werden müssten.

    /// Die Mitgliedschaft eines Pods, wie sie die Aggregatprüfung
    /// braucht (Punkt 40, Glied 2).
    ///
    /// # ⚑ Der Weg vom Register zur Signaturprüfung, und er fehlte
    ///
    /// `myl_consensus::poi::verify_bundle_signature` ist seit Langem
    /// gebaut und geprüft und **wurde von der Kette nie gerufen**, weil
    /// ihr die öffentlichen Schlüssel der Pod-Mitglieder fehlten:
    /// `MinerId` ist `SHA-256` über den Schlüssel, und aus einem Hash
    /// folgt kein Urbild. Seit das Register den Schlüssel führt, liegen
    /// sie vor.
    ///
    /// **Diese Funktion steht hier und nicht im Scheduler**, denn der
    /// bildet Pods und soll von Bündeln nichts wissen; und nicht in
    /// `myl-consensus`, der die Zuteilung nicht kennt. Der Knoten ist
    /// die einzige Stelle, die beides sieht.
    ///
    /// **Der Koordinator ist das Mitglied auf Position null**, also der
    /// erste Shard der Pipeline.
    fn mitgliedschaft(
        pod: &myl_scheduler::shard_assignment::Pod,
        epoche: u64,
        vertreter: &[(u32, myl_types::ids::MinerId)],
    ) -> Option<myl_consensus::poi::PodMembership> {
        let erste = pod.shards.first()?;
        let mitglieder = Self::unterzeichner_des_buendels(pod, vertreter)?;
        myl_consensus::poi::PodMembership::ohne_besitznachweis(
            EpochId(epoche),
            myl_types::pod_kennung(epoche, pod.pod_index),
            erste.miner.miner_id,
            mitglieder,
        )
        .ok()
    }

    /// **Prüft ein PoI-Bündel gegen die laufende Epoche**, wie das
    /// Einreichen es täte: Pod aus der Zuteilung, Aggregat gegen die
    /// Positionen. Gibt die Kennung des Koordinators (erste Position) und
    /// ihre Netzadresse zurück. Für den Knoten, bevor er ein Bündel
    /// weitergibt oder einreicht: Ein gefälschtes soll gar nicht erst in die
    /// Kette.
    pub fn buendel_pruefen(
        zustand: &LedgerState,
        buendel: &myl_types::PoIBundle,
    ) -> Result<(myl_types::ids::MinerId, myl_types::latency_attest::PeerIdBytes), String> {
        if buendel.epoch != zustand.epoch {
            return Err(format!("Bündel für Epoche {}, laufend ist {}", buendel.epoch.0, zustand.epoch.0));
        }
        if zustand.buendel.contains_key(&buendel.pod) {
            return Err("für diesen Pod liegt schon ein Bündel vor".into());
        }
        let zuteilung = Self::zuteilung_der_laufenden_epoche(zustand);
        let pod = myl_scheduler::zonenzuteilung::pod_zu_kennung(&zuteilung, zustand.epoch.0, &buendel.pod)
            .ok_or("diesen Pod kennt die Zuteilung nicht")?;
        let mitgliedschaft =
            Self::mitgliedschaft(pod, zustand.epoch.0, &buendel.vertreter).ok_or("Pod ohne Positionen oder falsche Vertreter")?;
        myl_consensus::poi::verify_bundle_signature(buendel, &mitgliedschaft).map_err(|e| format!("Aggregat: {e:?}"))?;
        let erster = &pod.shards.first().ok_or("Pod ohne Positionen")?.miner;
        // Vertritt jemand die erste Position, reicht er ein (6.8).
        let vertreter = buendel.vertreter.iter().find(|(p, _)| *p == 0).and_then(|(_, m)| pod.reserve.iter().find(|r| r.miner_id == *m));
        let einreicher = vertreter.unwrap_or(erster);
        Ok((einreicher.miner_id, einreicher.netzadresse))
    }

    /// ⚑ **Wer ein PoI-Bündel unterschreibt: die Positionen, nicht die
    /// Reserve** (Fund 645, 2026-10-26). Hier stand `pod.mitglieder()`, also
    /// auch die Reserve. Die hat keinen Shard, sieht die Arbeit nie und kann
    /// nicht nachrechnen, was sie unterschriebe; ein Shard prüft vor seiner
    /// Unterschrift den Anspruch gegen die eigenen Segmente. Ein echter Pod
    /// mit Reserve bekam so nie ein Bündel durch. Dieselbe Menge prüft das
    /// Einreichen und die Zuschreibung am Epochenwechsel.
    ///
    /// ⚑ **Mit Vertretern** (Whitepaper 6.8, Festlegung des Projektinhabers
    /// vom 2026-10-28): Nennt das Bündel für eine Position ein Mitglied der
    /// Reserve dieses Pods, unterschreibt es statt des Inhabers. `None`, wenn
    /// ein Vertreter nicht zur Reserve gehört, eine Position nicht gibt oder
    /// eine Position oder ein Vertreter doppelt steht: Ein solches Bündel
    /// trägt nicht.
    pub(crate) fn unterzeichner_des_buendels(
        pod: &myl_scheduler::shard_assignment::Pod,
        vertreter: &[(u32, myl_types::ids::MinerId)],
    ) -> Option<Vec<(myl_types::ids::MinerId, myl_types::bls::BlsPublicKey)>> {
        let mut liste: Vec<(myl_types::ids::MinerId, myl_types::bls::BlsPublicKey)> =
            pod.shards.iter().map(|s| (s.miner.miner_id, s.miner.schluessel)).collect();
        let mut positionen = std::collections::BTreeSet::new();
        let mut wer = std::collections::BTreeSet::new();
        for (position, miner) in vertreter {
            let r = pod.reserve.iter().find(|r| r.miner_id == *miner)?;
            if !positionen.insert(*position) || !wer.insert(*miner) {
                return None;
            }
            *liste.get_mut(*position as usize)? = (r.miner_id, r.schluessel);
        }
        Some(liste)
    }

    /// Wie viele Shard-Positionen ein Pod dieser Probekette hat.
    ///
    /// ⚑ **Fest, solange die Kette eine Probekette ist.** Der Zuschnitt
    /// gehört ins Pipeline-Manifest und damit unter Governance; ein
    /// beweglicher Wert, den nur einer kennt, wäre schlimmer als ein
    /// fester, den beide Seiten sehen.
    const PROBE_SHARDS: u32 = 4;

    /// Was ein Pod je Epoche an Rechenleistung liefert, in vTFE.
    ///
    /// ⚑ **Spiegelt `myl_governance::Parameter::PodKapazitaet`**, und
    /// das ist ein Behelf, kein Entwurf. Der Knoten bindet
    /// `myl-governance` nicht ein und kann die Registry deshalb nicht
    /// lesen; die Zahl steht hier zum zweiten Mal.
    ///
    /// **Ein Test hält beide gegeneinander** (`tests/parameter.rs`).
    /// Laufen sie auseinander, rechnet die Kette gegen eine Auslastung,
    /// die niemand beschlossen hat.
    const POD_KAPAZITAET_VTFE: u64 = 3_600_000_000;

    /// Der Sockel der Trainingsmenge, in Basispunkten der
    /// Gesamtkapazität. Spiegelt `Parameter::TrainingsGrundrate`.
    const TRAININGS_GRUNDRATE_BPS: u32 = 200;

    /// γ_train, in Basispunkten der freien Kapazität. Spiegelt
    /// `Parameter::TrainingsFreianteil`.
    const TRAININGS_FREIANTEIL_BPS: u32 = 8_000;

    /// Zähler und Nenner der Lernrate eines Trainingssegments.
    ///
    /// ⚑ **Vom Protokoll gesetzt und nicht vom Pod gewählt.** Ein Pod,
    /// der seine eigene Lernrate wählte, wählte seinen eigenen
    /// Arbeitsaufwand: Eine winzige Schrittweite bewegt fast nichts und
    /// ist trotzdem eine ausgeführte Rechnung.
    ///
    /// ⚑ **Der Zähler ist eins, der Nenner folgt der Tiefe.** Bis zum
    /// 2026-09-06 stand hier ein fester Nenner von 2¹², und der war
    /// gemessen falsch: Über die vierundzwanzig Ebenen des 0,5B zerstört
    /// er das Modell in einem Lauf, Perplexität 2,5 Milliarden gegen
    /// einen Ausgangsstand von 23. Die Zahl stammte aus einem Lauf über
    /// **eine** Ebene gegen **ein** Zielwort.
    const TRAININGS_LR_ZAEHLER: i64 = 1;

    /// Der Nenner der Lernrate für ein Segment über `tiefe` Ebenen mit
    /// `gradienten` eingehenden Gradienten.
    ///
    /// ⚑ **Keine Konstante mehr, sondern eine Funktion von Tiefe und
    /// Chargengrösse** ([`myl_types::lernrate`]). Der Knoten wählt sie
    /// nicht; er rechnet sie aus, und der Übergang prüft sie gegen
    /// dieselbe Funktion.
    fn trainings_lr_nenner(tiefe: u32, gradienten: u32) -> i64 {
        myl_types::lernrate::lernrate_nenner(tiefe, gradienten)
    }

    /// Die Stimmberechtigten aus dem **Kettenzustand**, nicht aus der
    /// Genesis-Datei.
    ///
    /// # ⚑ Was daran der Unterschied ist
    ///
    /// Bis zum 2026-09-06 kam das Stimmgewicht aus
    /// `Stimmsatzdatei::stimmberechtigte`, also aus einer **Textdatei**.
    /// Damit wog eine Zahl, die niemand auf der Kette hinterlegt hatte,
    /// und der Einsatz, den `EinsatzHinterlegen` seit dem 2026-09-03
    /// bewegt, wog nichts.
    ///
    /// Hier folgt das Gewicht dem **hinterlegten Einsatz**, und der
    /// Schlüssel kommt aus dem Minerregister, wo `MinerAnmelden` ihn
    /// gegen die Kennung prüft. Beides steht im Zustand und ist damit
    /// nachrechenbar.
    ///
    /// ⚑ **Arbeit qualifiziert, Stake wiegt** (Entscheidung A3): Wer
    /// nicht im Register steht, ist nicht stimmberechtigt, gleich wie
    /// viel er hinterlegt hat; wer darin steht, wiegt mit seinem
    /// Einsatz.
    ///
    /// ⚑ **Unter der Mindestschwelle zählt niemand mit.** Ein Konto mit
    /// einem Satoshi Einsatz wäre sonst stimmberechtigt und verdünnte
    /// die Zweidrittelmehrheit.
    pub fn stimmberechtigte_aus_zustand(
        zustand: &LedgerState,
        mindesteinsatz: u64,
    ) -> myl_consensus::validator::VotingSet {
        let mitglieder = zustand
            .miner
            .values()
            .filter_map(|r| {
                let konto = Address::new(*r.miner_id.as_bytes());
                let einsatz = zustand.account(&konto).staked;
                if einsatz < mindesteinsatz {
                    return None;
                }
                Some((
                    r.miner_id,
                    myl_consensus::validator::VotingMember {
                        pubkey: r.schluessel,
                        weight: einsatz,
                    },
                ))
            })
            .collect();
        myl_consensus::validator::VotingSet::from_members(mitglieder)
    }

    /// **Die Regeln des Stimmsatzes** (7.8, 2026-11-03), wie der
    /// Mindesteinsatz Parameter der Kette.
    ///
    /// ⚑ **Arbeitsschwelle ein Viertel** des nach Einsatz gewichteten
    /// Medians: Seit bezeugte Arbeit die Miner erreicht, wirkt sie
    /// (Entscheidung A3 hob sie bis dahin nicht an). ⚑ **Mindestgewicht vier
    /// Mindesteinsätze**, der kleinste tragfähige Satz; für ein Hauptnetz vor
    /// Genesis festzulegen.
    pub fn satzregeln(zustand: &LedgerState) -> myl_ledger::stimmsatz::Regeln {
        // ⚑ Auf der Probekette ein eigener, niedriger Wert (2026-11-03,
        //   [`crate::netz::Netz::stimmsatz_mindesteinsatz`]); sonst `S_min`.
        let netz = crate::netz::Netz::des_zustands(zustand);
        let mindesteinsatz = netz.stimmsatz_mindesteinsatz.unwrap_or_else(Self::mindesteinsatz);
        let inaktiv = netz.stimmsatz_inaktiv_epochen.unwrap_or(myl_ledger::stimmsatz::INAKTIV_EPOCHEN);
        myl_ledger::stimmsatz::Regeln {
            mindesteinsatz,
            arbeit_anteil: (1, 4),
            mindestgewicht: mindesteinsatz.saturating_mul(myl_ledger::stimmsatz::MINDESTMITGLIEDER as u64),
            ausschluss_punkte: myl_ledger::stimmsatz::STRAFPUNKTE.saturating_mul(inaktiv),
        }
    }

    /// Der Mindesteinsatz, ab dem jemand mitstimmt.
    ///
    /// ⚑ Spiegel von `Parameter::MindestStake`; ein Test hält beide
    /// Fassungen gegeneinander.
    pub fn mindesteinsatz() -> u64 {
        // ⚑ **Gerechnet und nicht getippt.** Derselbe Weg wie in der
        // Registry: `S_min = g / p²` aus Anhang B.1, mit der
        // Stichprobenrate als Konstante.
        // Anhang B.1: g = 0,5 MYL, dieselbe Herleitung wie in der
        // Registry.
        myl_tokenomics::s_min(
            myl_tokenomics::UNITS_PER_MYL / 2,
            myl_tokenomics::STICHPROBE_ZAEHLER,
            myl_tokenomics::STICHPROBE_NENNER,
        )
        .unwrap_or(u64::MAX)
    }

    /// Der Slashsatz für ein bewiesenes falsches Ergebnis.
    ///
    /// ⚑ **Spiegel von `myl_tokenomics::slashing::matrix()`**, Kap. 5.5,
    /// Zeile „Shard-Miner, falsches Ergebnis, 100 Prozent Stake", mit
    /// dem Kopfgeld aus Anhang B.3. Der Knoten zieht `myl-governance`
    /// nicht ein und kann die Registry nicht lesen; ein Test hält beide
    /// Fassungen gegeneinander.
    pub fn slashsatz_oeffentlich() -> myl_ledger::transitions::SlashParams {
        Self::slashsatz()
    }

    fn slashsatz() -> myl_ledger::transitions::SlashParams {
        myl_ledger::transitions::SlashParams {
            slash_fraction_num: 1,
            slash_fraction_den: 1,
            bounty_fraction_num: myl_tokenomics::slashing::KOPFGELD_ZAEHLER,
            bounty_fraction_den: myl_tokenomics::slashing::KOPFGELD_NENNER,
        }
    }

    /// Die Tiefe, über die ein Trainingssegment dieser Kette rechnet.
    ///
    /// ⚑ **Das ganze Modell und nicht der Shard.** Ein Pod schreibt alle
    /// Ebenen fort, geshardet; die Verkettung, an der die Lernrate
    /// hängt, läuft über alle. Wer je Shard rechnete, bekäme für
    /// dieselbe Arbeit verschiedene Raten, je nachdem, in wie viele
    /// Stücke sie geschnitten war.
    fn trainingstiefe(zustand: &LedgerState) -> u32 {
        crate::netz::Netz::des_zustands(zustand).profil.num_layers as u32
    }

    /// Die drei Spiegelwerte, damit `tests/parameter.rs` sie gegen die
    /// Registry halten kann.
    pub fn pod_kapazitaet_vtfe() -> u64 {
        Self::POD_KAPAZITAET_VTFE
    }
    /// Siehe [`Kette::pod_kapazitaet_vtfe`].
    pub fn trainings_grundrate_bps() -> u32 {
        Self::TRAININGS_GRUNDRATE_BPS
    }
    /// Siehe [`Kette::pod_kapazitaet_vtfe`].
    pub fn trainings_freianteil_bps() -> u32 {
        Self::TRAININGS_FREIANTEIL_BPS
    }


    /// Leitet die Zuschreibung der abzurechnenden Epoche ab.
    ///
    /// # Der Weg, und er ist jetzt vollständig
    ///
    /// Register → Zuteilung → Bündel → Anteile. Jeder Schritt ist eine
    /// **Ableitung**, keine Angabe: Wer im Register steht, sagt die
    /// Kette; wer in welchem Pod sitzt, folgt aus Register, Zone und
    /// Blockhash; welcher Pod ein Bündel eingereicht hat, folgt aus der
    /// abgeleiteten Pod-Kennung; und wie sich die vTFE eines Pods auf
    /// seine Positionen teilt, sagt die Arbeitsverteilung.
    ///
    /// ⚑ **Seit dem 2026-09-03 wird sie gerechnet, nicht gesetzt**
    /// (Fund 161): [`myl_tokenomics::vtfe::arbeitsverteilung_probe`].
    /// Vorher stand hier ein Zustandsfeld, das keine `Anweisung` je
    /// füllte; ohne einen Wert blieb die Zuschreibung immer leer, und
    /// ein Bündel, das im Zustand stand, wurde beim Epochenabschluss
    /// verworfen, **ohne dass je etwas geprägt wurde**. Jeder Knoten
    /// rechnet jetzt dieselbe Zahl aus denselben öffentlichen
    /// Grunddaten; nichts kann mehr auseinanderlaufen, weil nichts mehr
    /// übertragen wird.
    ///
    /// # ⚑ Ein Bündel ohne passenden Pod fällt still weg
    ///
    /// Es nennt eine Kennung, die zu keiner Platznummer dieser Epoche
    /// gehört. Das ist der Normalfall bei einem gefälschten Bündel; es
    /// zu verwerfen ist richtig, und es **den Block verwerfen zu
    /// lassen wäre falsch**, denn dann hielte eine einzige Fälschung die
    /// Kette an.
    /// ⚑ **Die Probeprägung der abgelaufenen Epoche** (2026-10-26): je
    /// angemeldetem Miner mit Auszahlungskonto der Betrag des Netzes, ausser
    /// er war in dieser Epoche gesperrt. Auf jedem Netz ausser der
    /// Probekette ist der Betrag null, und hier geschieht nichts.
    ///
    /// ⚑ **Neben der Ausschüttung, nicht in ihr**: Diese MYL entstehen ohne
    /// Arbeitsnachweis und ohne Burn, und sie sollen in keiner Rechnung
    /// auftauchen, die einmal echtes Geld abrechnet. Fällt die Probeprägung
    /// weg, ändert sich an der Ausschüttung nichts.
    ///
    /// Wer zehn Miner auf ein Wallet zeigen lässt, bekommt zehnmal; auf der
    /// Probekette ist das gewollt einfach.
    fn probepraegung(zustand: &mut LedgerState, alte_epoche: u64) {
        let betrag = crate::netz::Netz::des_zustands(zustand).probepraegung;
        if betrag == 0 {
            return;
        }
        let empfaenger: Vec<Address> = zustand
            .auszahlung
            .iter()
            .filter(|(m, _)| zustand.miner.contains_key(*m))
            .filter(|(m, _)| zustand.fristsperren.get(*m).map(|e| e.0) != Some(alte_epoche))
            .map(|(_, konto)| *konto)
            .collect();
        for konto in empfaenger {
            let _ = myl_ledger::transitions::praegen(zustand, &konto, betrag);
        }
        // 📌 Fund 675 (2026-10-28, Nachtlauf mit 30 Knoten): Die Probekonten
        //    sind der Vorrat des Probehahns, und nichts fuellte sie nach. Der
        //    Testverkehr zahlt seine Gebuehren aus ihnen, rund einmal je
        //    Sekunde und Knoten; mit 30 Knoten auf zwoelf Konten waren alle
        //    nach etwa 40 Minuten leer, die Lastphase hatte nichts mehr zum
        //    Ausstatten, und der spaete Einsteiger fand 452 mal einen leeren
        //    Hahn. Jetzt fuellt der Epochenwechsel ein Konto unter der
        //    Haelfte wieder auf [`PROBEGUTHABEN`] auf, wie ein Testnetz
        //    seinen Hahn nachfuellt; mehr als zwoelf volle Konten je Epoche
        //    entsteht so nie. Erst unter der Haelfte, damit kleine Abgaenge
        //    (eine Hinterlegung, ein Burn) in der Rechnung bleiben und nicht
        //    beim naechsten Wechsel verschwinden.
        for n in 0..PROBEKONTEN {
            let konto = probekonto(n);
            let stand = zustand.account(&konto).balance;
            if stand < PROBEGUTHABEN / 2 {
                let _ = myl_ledger::transitions::praegen(zustand, &konto, PROBEGUTHABEN - stand);
            }
        }
    }

    /// Die Zuschreibung der Epoche aus den gültig unterschriebenen Bündeln,
    /// dazu diese Bündel (für die Stichprobe) und wie viele Zwillingspaare
    /// einig und uneinig waren.
    ///
    /// ⚑ **Stufe 1** (Whitepaper 3.4 und 5.3, Festlegung des Projektinhabers
    /// vom 2026-10-28): Ein Pod wird nur vergütet, wenn er und sein Zwilling
    /// ([`Self::zwillinge_der_laufenden_epoche`]) ein Bündel mit derselben
    /// Ausgabenwurzel (nicht null) und derselben Segmentzahl eingereicht
    /// haben, und dann jeder zur Hälfte (Redundanz-Normierung). Uneinig oder
    /// ohne Bündel des Zwillings: keiner; das ist ein Fall für die
    /// Stichprobe. ⚑ Ein übriger Pod steht in Bereitschaft und wird nicht
    /// vergütet. ⚑ Gibt es gar kein Paar (ein Netz mit einem Pod), gilt die
    /// Anlaufregel: zurückgehalten bis nach der Streitfrist, jedes Segment
    /// gezogen.
    fn zuschreibung_der_epoche(zustand: &LedgerState) -> Epochenzuschreibung {
        let verteilung = myl_tokenomics::vtfe::arbeitsverteilung_probe();
        let epoche = zustand.epoch.0;
        let zuteilung = Self::zuteilung_der_laufenden_epoche(zustand);
        let _ = &verteilung;

        let mut gueltig: std::collections::BTreeMap<u32, (myl_types::PoIBundle, myl_tokenomics::Podabrechnung)> =
            std::collections::BTreeMap::new();
        let mut bezeugt: Vec<myl_types::PoIBundle> = Vec::new();
        for buendel in buendel_der_epoche(zustand) {
            let Some(pod) =
                myl_scheduler::zonenzuteilung::pod_zu_kennung(&zuteilung, epoche, &buendel.pod)
            else {
                continue;
            };
            // ⚑ **Und jetzt wird die Aggregatsignatur geprüft**
            // (Punkt 40, Glied 2). Die Mitgliedschaft kommt aus der
            // **Zuteilung**, nie aus dem Bündel: Wer sie aus dem Bündel
            // nähme, ließe den Einreicher bestimmen, gegen welche
            // Schlüssel geprüft wird.
            let Some(mitgliedschaft) = Self::mitgliedschaft(pod, epoche, &buendel.vertreter) else {
                continue;
            };
            if myl_consensus::poi::verify_bundle_signature(&buendel, &mitgliedschaft).is_err() {
                continue;
            }
            // ⚑ **Jeder Pod nach seinem Schnitt** (Stufe 4, 2026-10-22): Die
            //   Gewichte folgen aus seinen Grenzen und dem Profil des Netzes.
            //   Die gemeinsame Probeverteilung galt nur, solange jeder Pod
            //   gleich schnitt. Ein Pod ohne Schnitt bekommt nichts.
            let Some(grenzen) = Self::grenzen_des_pods(zustand, pod) else {
                continue;
            };
            let netz = crate::netz::Netz::des_zustands(zustand);
            let grenzen: Vec<(u64, u64)> = grenzen.iter().map(|&(a, b)| (a as u64, b as u64)).collect();
            let Some(eigene) = myl_tokenomics::vtfe::arbeitsverteilung_aus_grenzen(&netz.profil, &grenzen, netz.pipeline())
            else {
                continue;
            };
            // ⚑ Eine übernommene Position wird an den Vertreter vergütet, für
            //   das ganze Bündel (6.8; die Zuschreibung je Segment kommt mit
            //   den Unterschriften je Segment).
            let abrechnung = myl_tokenomics::Podabrechnung {
                positionen: pod
                    .shards
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let wer = buendel.vertreter.iter().find(|(p, _)| *p as usize == i).map(|(_, m)| *m);
                        (wer.unwrap_or(s.miner.miner_id), s.shard_index)
                    })
                    .collect(),
                reserve: pod.reserve.iter().map(|m| m.miner_id).collect(),
                vtfe_pod: buendel.vtfe_claimed,
                verteilung: Some(eigene),
            };
            bezeugt.push(buendel.clone());
            gueltig.insert(pod.pod_index, (buendel, abrechnung));
        }
        // ⚑ Stufe 1: nach Zwillingsgruppen.
        let gruppen = Self::zwillinge_der_laufenden_epoche(zustand);
        let n = nach_zwillingen(gueltig, &gruppen);
        let allein: Vec<myl_tokenomics::Podabrechnung> = n.allein.iter().map(|(_, r)| r.clone()).collect();
        Epochenzuschreibung {
            zuschreibung: myl_tokenomics::zuschreiben_aus_abrechnung(&verteilung, &n.bezahlt).unwrap_or_default(),
            zurueck: myl_tokenomics::zuschreiben_aus_abrechnung(&verteilung, &allein).unwrap_or_default().je_miner,
            allein: n.allein.iter().map(|(i, _)| myl_types::pod_kennung(epoche, *i)).collect(),
            bezeugt,
            einig: n.einig,
            uneinig: n.uneinig,
            verschieden: n.verschieden,
            vorbehalten: n
                .vorbehalten
                .iter()
                .flat_map(|((pa, ra), (pb, rb))| {
                    let je = |r: &myl_tokenomics::Podabrechnung| {
                        myl_tokenomics::zuschreiben_aus_abrechnung(&verteilung, std::slice::from_ref(r)).unwrap_or_default().je_miner
                    };
                    [(*pa, *pb, je(ra)), (*pb, *pa, je(rb))]
                })
                .collect(),
            pflicht: n
                .pflicht
                .iter()
                .map(|(pod, zwilling, f, r)| {
                    let je = myl_tokenomics::zuschreiben_aus_abrechnung(&verteilung, std::slice::from_ref(r)).unwrap_or_default().je_miner;
                    (*pod, *zwilling, *f, je)
                })
                .collect(),
        }
    }

    /// Setzt den Stimmsatz, gegen den Saatquellen und Erzeuger geprüft
    /// werden.
    pub fn stimmsatz_setzen(&mut self, satz: myl_consensus::validator::VotingSet) {
        self.stimmsatz = Some(satz);
    }

    /// Setzt den Schlüssel, mit dem dieser Knoten seine Blöcke
    /// unterschreibt (2026-10-27).
    pub fn erzeugerschluessel_setzen(&mut self, schluessel: crate::schluessel::Konsensschluessel) {
        self.erzeugerschluessel = Some(schluessel);
        self.nur_nach_plan = false;
    }

    /// ⚑ **Der Schlüssel eines Miners**, der in seinen Slots erzeugt (7.7
    /// Schritt 5), aber kein ausdrücklicher Erzeuger ist: wie
    /// [`Self::erzeugerschluessel_setzen`], nur baut er auf einer Probekette
    /// erst, wenn sie ihren Erzeuger gelernt hat.
    pub fn miner_erzeugerschluessel_setzen(&mut self, schluessel: crate::schluessel::Konsensschluessel) {
        self.erzeugerschluessel = Some(schluessel);
        self.nur_nach_plan = true;
    }

    /// **Trägt der Erzeuger dieses Blocks?**
    ///
    /// ⚑ **Seit dem 2026-10-27 braucht jeder Block die Unterschrift eines
    /// Mitglieds des Stimmsatzes**, unter genau dem Schlüssel, den der Satz
    /// für es führt (Festlegung des Projektinhabers: Stimmsatz ab Genesis,
    /// immer Pflicht). Unsignierte Blöcke gibt es nicht mehr; eine
    /// Unterschrift, die nicht trägt, nie (Fund 192).
    ///
    /// Hat die Kette noch keinen Satz, gilt das nur auf einer Probekette und
    /// nur bis Block 1: Wer ihn trägt, wird das einzige Mitglied
    /// ([`Self::erzeuger_lernen`]).
    ///
    /// ⚑ **Seit dem 2026-11-04 (Erzeugerplan, 7.7 Schritt 3) genau der für
    /// den Slot gezogene** ([`Self::erzeuger_fuer`]), aus dem Satz der
    /// Epoche ([`Self::erzeugersatz_von`]), in einem Slot nach dem des
    /// Vorgängers. Für einen Block, der nicht
    /// anschliesst (ein Nachzügler sieht ihn, bevor er die Lücke holt),
    /// fehlt die Saat womöglich noch; er muss nur von einem Mitglied des
    /// Satzes seiner Epoche kommen, oder, ist der nicht bestimmbar, der
    /// laufenden. Geholt und übernommen wird dann Block für Block, jeder mit
    /// dem vollen Plan.
    fn erzeuger_traegt(&self, block: &Block) -> bool {
        let (Ok(Some(kennung)), Some(u)) = (block.erzeuger(), &block.unterschrift) else {
            return false;
        };
        let hoehe = block.header.height;
        let e = epoche_fuer_hoehe(hoehe);
        let satz = self.erzeugersatz(e).or_else(|| self.erzeugersatz(self.zustand.epoch.0));
        let Some(satz) = satz else {
            return self.stimmsatz.is_none() && self.lernt_erzeuger;
        };
        if satz.pubkey(&kennung) != Some(&u.schluessel) {
            return false;
        }
        // 📌 Der Plan gilt nur für einen Block auf der eigenen Spitze: Ein
        //   fremder Zweig ist womöglich früher in der Zeit, sein Slot also
        //   unter dem eigenen letzten; das ist kein falscher Erzeuger, sondern
        //   eine Gabel, und die muss als `PasstNichtAn` zur Zweigsuche
        //   (Nachtlauf `nacht-rotation`, 2026-11-04).
        hoehe != self.hoehe + 1
            || block.header.prev_block_hash != self.letzter_hash
            || (block.header.slot > self.zustand.letzter_slot && self.erzeuger_fuer(hoehe, block.header.slot) == Some(kennung))
    }

    /// ⚑ **Aus wem der Erzeuger einer Epoche gezogen wird** (2026-11-04):
    /// der Satz aus dem Zustand, auch wenn er die Finalität nicht trägt (die
    /// verfügbare Kette lebt weiter, wenn die Finalität ruht); gibt es für
    /// die Epoche keinen oder ist er leer, der Satz von aussen.
    /// 📌 Anders als bei der Finalität ist der Rückfall nach aussen hier
    /// kein Weg zu zwei Geschichten: Was ein Erzeuger baut, wird erst über
    /// die Prüfpunkte endgültig.
    pub fn erzeugersatz_von(
        zustand: &LedgerState,
        von_aussen: Option<&myl_consensus::validator::VotingSet>,
        epoche: u64,
    ) -> Option<myl_consensus::validator::VotingSet> {
        // 📌 **Wer ruht, wird nicht gezogen** (`kurz-konsens`, 2026-11-05):
        //   Der Satz der Epoche steht zwei Epochen voraus fest, und ein
        //   geschlachteter Doppelerzeuger baute bis dahin weiter. Für die
        //   Finalität bleibt der Satz fest (dort hängt die Sicherheit an ihm),
        //   für die Ziehung nicht: Ein Ruhender fällt sofort heraus.
        let wach = |m: &myl_types::MinerId| !zustand.mitwirkung.get(m).is_some_and(|w| w.ruht);
        match myl_ledger::stimmsatz::satz_der_epoche(zustand, epoche) {
            Some(satz) if satz.iter().any(|(m, _, _)| wach(m)) => Some(myl_consensus::validator::VotingSet::from_members(
                satz.iter()
                    .filter(|(m, _, _)| wach(m))
                    .map(|(m, k, w)| (*m, myl_consensus::validator::VotingMember { pubkey: *k, weight: *w }))
                    .collect(),
            )),
            _ => von_aussen.cloned(),
        }
    }

    /// Der Satz, aus dem die Erzeuger der Epoche `epoche` gezogen werden.
    pub fn erzeugersatz(&self, epoche: u64) -> Option<myl_consensus::validator::VotingSet> {
        Self::erzeugersatz_von(&self.zustand, self.stimmsatz.as_ref(), epoche)
    }

    /// ⚑ **Die Saat des Plans für die Epoche `epoche`** aus dem Zustand nach
    /// dem letzten Block: für die laufende die gesetzte, für die nächste die,
    /// die ihr Wechsel setzen wird (`β` der laufenden, sonst die alte
    /// weiter). Weiter voraus keine.
    pub fn erzeugersaat_fuer(zustand: &LedgerState, epoche: u64) -> Option<Hash> {
        let jetzt = zustand.epoch.0;
        if epoche == jetzt {
            return Some(zustand.erzeugersaat);
        }
        if epoche != jetzt.saturating_add(1) {
            return None;
        }
        Some(
            myl_ledger::saat::beta_der_epoche(zustand, jetzt)
                .map(|b| myl_consensus::erzeugerplan::saat_aus_beta(&myl_types::schwelle::saat_aus_signatur(&b)))
                .unwrap_or(zustand.erzeugersaat),
        )
    }

    /// ⚑ **Wer einen Block an `hoehe` im Slot `slot` erzeugen darf**; die
    /// Saat und der Satz kommen aus der Epoche der Höhe. Nur für die Höhe
    /// direkt über dem Kopf und die danach in derselben Epoche verlässlich,
    /// denn die Saat der übernächsten Epoche steht noch nicht.
    pub fn erzeuger_fuer(&self, hoehe: u64, slot: u64) -> Option<myl_types::ids::MinerId> {
        let e = epoche_fuer_hoehe(hoehe);
        let saat = Self::erzeugersaat_fuer(&self.zustand, e)?;
        myl_consensus::erzeugerplan::ziehen(&saat, slot, &self.erzeugersatz(e)?)
    }

    /// Der Slot des letzten Blocks.
    pub fn letzter_slot(&self) -> u64 {
        self.zustand.letzter_slot
    }

    /// ⚑ **Der erste Slot in `ab..=bis` nach dem des Kopfs, in dem dieser
    /// Knoten den nächsten Block erzeugen darf**, mit seinem
    /// Erzeugerschlüssel; `None`, wenn in keinem oder ohne Schlüssel.
    pub fn eigener_slot(&self, ab: u64, bis: u64) -> Option<u64> {
        let k = self.erzeugerschluessel.as_ref()?;
        let e = epoche_fuer_hoehe(self.hoehe + 1);
        // Eine Probekette ohne Satz lernt ihren Erzeuger aus dem ersten Block
        // (`erzeuger_traegt`): Bis dahin ist jeder Slot frei, ⚑ aber nur für
        // einen ausdrücklichen Erzeuger (2026-11-15). 📌 Im Probenetz baute
        // sonst jeder Mitläufer, der seinen ersten Slot vor dem ersten Block
        // des Erzeugers erreichte, selbst Block 1, lernte sich als Erzeuger
        // und wies den echten für immer ab; jeder lief dann allein, und jeder
        // Einsteiger sah je Höhe drei fremde Blöcke (`erzeuger-unbelegt`).
        if self.stimmsatz.is_none() && self.lernt_erzeuger && self.erzeugersatz(e).is_none() {
            if self.nur_nach_plan {
                return None;
            }
            let ab = ab.max(self.zustand.letzter_slot.saturating_add(1));
            return (ab <= bis).then_some(ab);
        }
        let satz = self.erzeugersatz(e)?;
        if satz.pubkey(&k.kennung()) != Some(&k.oeffentlich()) {
            return None;
        }
        let saat = Self::erzeugersaat_fuer(&self.zustand, e)?;
        let ab = ab.max(self.zustand.letzter_slot.saturating_add(1));
        myl_consensus::erzeugerplan::naechster_slot(&saat, ab, bis, &satz, &k.kennung())
    }

    /// **Der Stimmsatz aus dem ersten Block** einer Probekette ohne
    /// Stimmsatzdatei: sein Erzeuger als einziges Mitglied, Gewicht eins.
    /// Nur, wenn die Kette lernen darf und noch keinen Satz hat.
    fn erzeuger_lernen(&mut self, block: &Block) {
        if self.stimmsatz.is_some() {
            return;
        }
        self.stimmsatz = self.satz_aus_block(block);
    }

    /// Der Satz, den eine lernende Kette aus diesem Block lernen würde: sein
    /// Erzeuger allein, Gewicht eins. `None`, wenn sie nicht lernt.
    fn satz_aus_block(&self, block: &Block) -> Option<myl_consensus::validator::VotingSet> {
        if !self.lernt_erzeuger {
            return None;
        }
        let (Ok(Some(kennung)), Some(u)) = (block.erzeuger(), &block.unterschrift) else {
            return None;
        };
        let mut mitglieder = std::collections::BTreeMap::new();
        mitglieder.insert(kennung, myl_consensus::validator::VotingMember { pubkey: u.schluessel, weight: 1 });
        Some(myl_consensus::validator::VotingSet::from_members(mitglieder))
    }

    /// Der Hash des eigenen Blocks an `hoehe`, solange er im Fenster liegt.
    pub fn hash_an(&self, hoehe: u64) -> Option<Hash> {
        self.verlauf.get(&hoehe).map(|b| b.hash())
    }

    /// Der eigene Block an `hoehe`, solange er im Fenster liegt.
    pub fn block_an(&self, hoehe: u64) -> Option<&Block> {
        self.verlauf.get(&hoehe)
    }

    /// **Trägt dieses Zertifikat in einem Block der Höhe `hoehe`?**
    /// (2026-10-27, CONSENSUS 7.2)
    ///
    /// 1. **Ein Prüfpunkt nach dem zuletzt gerechtfertigten** und vor diesem
    ///    Block. Ein älteres Zertifikat sagte nichts Neues, und eine Lücke
    ///    ist erlaubt: Die Endgültigkeit eines späteren Prüfpunkts deckt
    ///    die früheren mit.
    /// 2. **Der eigene Block an dieser Höhe.** Ein Zertifikat für einen
    ///    anderen Zweig ist keines für diese Kette.
    /// 3. ⚑ **Die Quelle, die der Zustand für diesen Prüfpunkt vermerkt
    ///    hat** (2026-11-04). Sie war gerechtfertigt, als der
    ///    Block des Ziels angewandt wurde.
    /// 4. **Mehr als zwei Drittel des Stimmsatzes** über die
    ///    Prüfpunktbotschaft. Ohne Satz trägt keines.
    fn zertifikat_traegt(&self, z: &myl_consensus::pruefpunkt::Pruefpunktzertifikat, hoehe: u64) -> bool {
        self.finalitaetssatz(epoche_fuer_hoehe(z.hoehe)).is_some_and(|satz| self.zertifikat_traegt_gegen(z, hoehe, &satz))
    }

    fn zertifikat_traegt_gegen(
        &self,
        z: &myl_consensus::pruefpunkt::Pruefpunktzertifikat,
        hoehe: u64,
        satz: &myl_consensus::validator::VotingSet,
    ) -> bool {
        z.hoehe > self.zustand.pruefpunkt_gerechtfertigt.0
            && z.hoehe < hoehe
            && self.hash_an(z.hoehe) == Some(z.block_hash)
            && self.quelle_an(z.hoehe) == Some(z.quelle)
            && z.bindung == myl_ledger::stimmsatz::bindung(&self.zustand, epoche_fuer_hoehe(z.hoehe))
            && z.verify(satz).is_ok()
    }

    /// ⚑ **Die Quelle, die jede Stimme für den Prüfpunkt `hoehe` nennt**
    /// (2026-11-04), aus dem Zustand; nur für die laufende und die vorige
    /// Epoche.
    pub fn quelle_an(&self, hoehe: u64) -> Option<myl_consensus::pruefpunkt::Quelle> {
        self.zustand
            .pruefpunkt_quellen
            .iter()
            .find(|(h, _, _)| *h == hoehe)
            .map(|(_, q, x)| myl_consensus::pruefpunkt::Quelle { hoehe: *q, block_hash: *x })
    }

    /// Epoche und Platz eines Prüfpunkts in ihr.
    fn pruefpunkt_platz(hoehe: u64) -> (u64, u64) {
        (epoche_fuer_hoehe(hoehe), hoehe % BLOECKE_JE_EPOCHE / myl_consensus::pruefpunkt::BLOECKE_JE_PRUEFPUNKT)
    }

    /// ⚑ **Eine Stimme als Transaktion** (2026-11-03, 7.8): Ein Mitglied des
    /// Stimmsatzes bezeugt den Prüfpunkt `hoehe` mit dem Hash, den diese
    /// Kette dort hat; die Unterschrift der Transaktion ist die seine. Zählt
    /// für die Teilnahme und den Topf wie eine Unterschrift im Zertifikat,
    /// je Prüfpunkt einmal, und nur in der laufenden und der vorigen Epoche.
    /// 📌 Ohne sie gäbe es beim Ausfall von mehr als einem Drittel keinen
    /// Beleg für irgendwen, und der Erzeuger entschiede allein, wessen
    /// Unterschrift im Zertifikat steht. Gibt zurück, ob sie zählte.
    /// Das Drittel der laufenden Epoche, in dem `hoehe` liegt, fuer die
    /// Saatrunde der naechsten.
    fn saatphase(hoehe: u64) -> myl_ledger::saat::Phase {
        myl_ledger::saat::phase(hoehe % BLOECKE_JE_EPOCHE, BLOECKE_JE_EPOCHE)
    }

    /// **`β_e` ist gesetzt** (2026-11-03): Ist es das der laufenden Epoche,
    /// kommt daraus die Saat der naechsten Zuteilung; und jede offene
    /// Stichprobe einer frueheren Epoche wird jetzt gezogen.
    fn beta_verwenden(
        zustand: &mut LedgerState,
        e: u64,
        beta: &myl_types::bls::BlsSignature,
        jetzt: u64,
        gezogen: &mut Option<(u64, Vec<crate::stichprobe::Segmentstichprobe>)>,
    ) {
        if e == jetzt {
            zustand.epochensaat_naechste = myl_ledger::saat::epochensaat_aus(beta);
        }
        Self::stichproben_mit_beta_ziehen(zustand, e, beta, gezogen);
    }

    fn pruefpunkt_bezeugen(
        zustand: &mut LedgerState,
        absender: &Address,
        hoehe: u64,
        block_hash: &Hash,
        epoch: u64,
        stimmsatz: Option<&myl_consensus::validator::VotingSet>,
    ) -> bool {
        let wer = myl_types::ids::MinerId::new(*absender.as_bytes());
        let (e, platz) = Self::pruefpunkt_platz(hoehe);
        // ⚑ Nur wer die Finalitaet dieser Epoche traegt (7.8 Schritt 2).
        let im_satz = Self::finalitaetssatz_von(zustand, stimmsatz, e).is_some_and(|v| v.contains(&wer));
        if !im_satz || !zustand.pruefpunkt_hashes.iter().any(|(h, x)| *h == hoehe && x == block_hash) {
            return false;
        }
        if !myl_ledger::stimmsatz::stimme_vermerken(zustand, &wer, e, platz, epoch) {
            return false;
        }
        *zustand.pruefpunkt_stimmen.entry(wer).or_default() += 1;
        true
    }

    /// **Bucht einen Doppelbeleg** (2026-10-27, CONSENSUS 7.6).
    ///
    /// 1. Der Beleg trägt ([`myl_consensus::double_signing::Doppelbeleg::schuldiger`]).
    /// 2. **Die Tat liegt in der Sperrfrist**: Ältere Widersprüche sind nicht
    ///    mehr schlachtbar, das ist dieselbe Grenze wie die Haftung des
    ///    Einsatzes (und der Grund der Vertrauensfrist).
    /// 3. **Einmal je (Schuldiger, Art, Höhe)**: Ein zweiter Beleg für
    ///    denselben Widerspruch schlachtet nicht noch einmal.
    /// 4. Geschlachtet wird nach der Matrix (Validator, Doppelsignatur oder
    ///    Zensur: 30, 65, 100 % gestaffelt), das Kopfgeld geht an den
    ///    Melder; wer sich selbst meldet, wird abgewiesen
    ///    (`myl_ledger::apply_verdict`). Ohne Einsatz gibt es nichts zu
    ///    schlachten, und der Beleg bleibt ohne Folge.
    /// 5. ⚑ **Korrelation** (2026-11-03): Danach wird jeder Schuldige der
    ///    Sperrfrist nachgeschlachtet, bis er `min(1, 3 · Anteil)` verloren
    ///    hat, Anteil am Gewicht des Stimmsatzes aus dem Zustand, in Einsatz
    ///    ([`myl_tokenomics::slashing::korrelation_buchen`],
    ///    [`myl_ledger::stimmsatz::gesamtgewicht`]); seine
    ///    Kündigungen bleiben dafür bis zum Ende der Sperrfrist fest, und
    ///    er fällt aus dem Stimmsatz des Zustands, bis er sich neu anmeldet.
    fn doppelbeleg_buchen(
        zustand: &mut LedgerState,
        melder: &Address,
        beleg: &myl_consensus::double_signing::Doppelbeleg,
    ) -> Result<(), String> {
        let pk = beleg.schuldiger().map_err(|e| e.to_string())?;
        let tat_epoche = epoche_fuer_hoehe(beleg.hoehe());
        if tat_epoche.saturating_add(myl_ledger::einsatz::SPERRFRIST_EPOCHEN) < zustand.epoch.0 {
            return Err("die Tat liegt vor der Sperrfrist".into());
        }
        let schuldig = Address::aus_schluessel(&pk);
        let schluessel = (schuldig, beleg.art(), beleg.hoehe());
        if zustand.doppelbelege.contains_key(&schluessel) {
            return Err("dieser Widerspruch ist schon gebucht".into());
        }
        let urteil = myl_ledger::transitions::Verdict {
            segment_id: myl_types::ids::SegmentId::new(Hash::sha256(&borsh::to_vec(beleg).map_err(|e| e.to_string())?).0),
            miner: schuldig,
            checker: *melder,
            outcome: myl_ledger::transitions::VerdictOutcome::SlashMiner,
        };
        let konto = zustand.account(&schuldig);
        let masse = myl_ledger::einsatz::schlachtbar(konto.staked, &konto.gekuendigt);
        let (wirkung, _) = myl_tokenomics::slashing::urteil_buchen_gestaffelt(
            zustand,
            &urteil,
            myl_tokenomics::slashing::Akteur::Validator,
            myl_tokenomics::slashing::Grund::DoubleSigningOderZensur,
        )
        .map_err(|e| format!("{e:?}"))?;
        let epoche = zustand.epoch.0;
        zustand
            .doppelbelege
            .insert(schluessel, myl_ledger::transitions::Doppelbuchung { epoche, masse, genommen: wirkung.slashed });
        myl_ledger::transitions::kuendigungen_festhalten(zustand, &schuldig, myl_ledger::einsatz::freigabe_epoche(epoche));
        let gesamt = myl_ledger::stimmsatz::gesamtgewicht(zustand, epoche);
        myl_tokenomics::slashing::korrelation_buchen(zustand, gesamt);
        myl_ledger::stimmsatz::ausschliessen(zustand, &myl_types::ids::MinerId::new(*schuldig.as_bytes()));
        Ok(())
    }

    /// Prüft das Zertifikat im Kopf eines Blocks; ohne Zertifikat nichts.
    ///
    /// 📌 Eine lernende Kette ohne Satz (etwa nach einem Abzug) prüft gegen
    /// den Satz, den sie aus genau diesem Block lernt; sonst wiese sie den
    /// ersten Block nach dem Abzug ab, sobald er ein Zertifikat trägt.
    fn pruefpunkt_traegt(&self, block: &Block) -> bool {
        let Some(z) = &block.header.pruefpunkt else {
            return true;
        };
        match &self.stimmsatz {
            Some(_) => self.zertifikat_traegt(z, block.header.height),
            None => self.satz_aus_block(block).is_some_and(|satz| self.zertifikat_traegt_gegen(z, block.header.height, &satz)),
        }
    }

    /// ⚑ **Das Zertifikat eines fremden Zweigs vor dem Neuaufbau** (7.7a,
    /// 2026-11-06): was ohne den Zustand des Zweigs geht. Der zertifizierte
    /// Block ist der des Zweigs (über der Gabel) oder der eigene (darunter),
    /// und mehr als zwei Drittel des Satzes haben unterschrieben. Quelle,
    /// Bindung und Reihenfolge hängen am Zustand des Zweigs; sie prüft das
    /// Nachrechnen mit [`Self::pruefpunkt_traegt`].
    ///
    /// 📌 Nachtlauf `nacht-konsens` (2026-11-06): Die Vorprüfung nahm
    /// `pruefpunkt_traegt`, also den **eigenen** Block an der Höhe des
    /// Prüfpunkts. Lag er über der Gabel, trug kein Zertifikat der Mehrheit,
    /// und vier Knoten blieben nach 900 s Trennung auf ihrem Zweig.
    ///
    /// ⚑ Geprüft wird nur gegen einen Satz, der vor der Gabel feststand (die
    /// Epoche der Gabel und die nächste, [`myl_ledger::stimmsatz::VORLAUF_EPOCHEN`]);
    /// einen späteren bestimmt jeder Zweig selbst, er bleibt dem Nachrechnen.
    fn pruefpunkt_traegt_im_zweig(&mut self, block: &Block, gabel: u64, zweig: &[Block]) -> bool {
        let Some(z) = &block.header.pruefpunkt else {
            return true;
        };
        if z.hoehe >= block.header.height {
            return false;
        }
        let zertifiziert = if z.hoehe > gabel {
            zweig.iter().find(|b| b.header.height == z.hoehe).map(|b| b.hash())
        } else {
            self.eigener_hash(z.hoehe)
        };
        if zertifiziert != Some(z.block_hash) {
            return false;
        }
        let epoche = epoche_fuer_hoehe(z.hoehe);
        // Fest vor der Gabel: Satz e steht VORLAUF_EPOCHEN vorher, also bis
        // zur Epoche nach der Gabel.
        if epoche + 1 > epoche_fuer_hoehe(gabel) + myl_ledger::stimmsatz::VORLAUF_EPOCHEN {
            return true;
        }
        self.finalitaetssatz(epoche).is_some_and(|satz| z.verify(&satz).is_ok())
    }

    /// **Bietet ein Zertifikat für den nächsten erzeugten Block an.**
    ///
    /// ⚑ **Nur der Erzeuger legt es in einen Block; alle anderen lesen es
    /// daraus.** Behalten wird das höchste, denn es deckt die früheren mit.
    pub fn pruefpunkt_anbieten(&mut self, z: myl_consensus::pruefpunkt::Pruefpunktzertifikat) {
        if z.hoehe <= self.zustand.pruefpunkt_gerechtfertigt.0 {
            return;
        }
        if self.naechstes_pruefpunktzertifikat.as_ref().is_none_or(|alt| z.hoehe > alt.hoehe) {
            self.naechstes_pruefpunktzertifikat = Some(z);
        }
    }

    /// Bis zu welcher Höhe die Kette bestätigt, also endgültig ist.
    pub fn bestaetigt_bis(&self) -> u64 {
        self.zustand.pruefpunkt_bestaetigt
    }

    /// Der jüngste gerechtfertigte Prüfpunkt (ein Zertifikat, noch nicht
    /// endgültig); über ihm wird gestimmt.
    pub fn gerechtfertigt_bis(&self) -> u64 {
        self.zustand.pruefpunkt_gerechtfertigt.0
    }

    /// Die Höhe des angebotenen, noch nicht eingetragenen Zertifikats.
    pub fn naechstes_pruefpunktzertifikat_hoehe(&self) -> Option<u64> {
        self.naechstes_pruefpunktzertifikat.as_ref().map(|z| z.hoehe)
    }

    /// Der Schlüssel, mit dem diese Kette ihre Blöcke unterschreibt.
    pub fn erzeugerschluessel(&self) -> Option<&crate::schluessel::Konsensschluessel> {
        self.erzeugerschluessel.as_ref()
    }

    /// Die zuletzt gezogene Stichprobe (Punkt 45).
    ///
    /// Leer, solange keine Epoche abgeschlossen wurde. **Sie wird
    /// gerechnet, nicht gespeichert**; dieser Zugang gibt nur weiter,
    /// was der letzte Abschluss ergab, damit der Knoten es
    /// protokollieren kann.
    pub fn letzte_stichprobe(&self) -> &[crate::stichprobe::Segmentstichprobe] {
        self.letzte_stichprobe.as_ref().map_or(&[], |(_, s)| s.as_slice())
    }

    /// Die Checker der letzten Ziehung: Pod, Index im Bündel, Miner.
    pub fn pruefer_der_stichprobe(&self) -> &[(myl_types::ids::PodId, u32, myl_types::ids::MinerId)] {
        &self.pruefer_der_stichprobe
    }

    /// ⚑ **Die Grössen der Kette** (Fund 677, 2026-10-29): Im Nachtlauf
    /// wuchs jeder Knoten um rund 2,3 MB je Epoche, ohne dass klar war,
    /// wodurch. Je Sammlung die Zahl der Einträge, dazu die Bytes des
    /// Zustands, den jeder Block ganz hasht.
    pub fn groessen(&self) -> Vec<(&'static str, u64)> {
        let z = &self.zustand;
        let n = |x: usize| x as u64;
        vec![
            ("zustand_bytes", borsh::to_vec(z).map_or(0, |b| b.len() as u64)),
            ("konten", n(z.accounts.len())),
            ("sitzungen", n(z.sitzungen.len())),
            ("speicher", n(z.speicher.len())),
            ("miner", n(z.miner.len())),
            ("zusagen", n(z.zusagen.values().map(Vec::len).sum())),
            ("buendel", n(z.buendel.len())),
            ("doppelbelege", n(z.doppelbelege.len())),
            ("hinterlegungen", n(z.hinterlegungen.len())),
            ("trainingssegmente", n(z.trainingssegmente.len())),
            ("zurueckgehalten", n(z.zurueckgehalten.len())),
            ("stichproben_offen", n(z.stichproben_offen.len())),
            ("pruefauftraege", n(z.pruefauftraege.len())),
            ("streitfaelle", n(z.streitfaelle.len())),
            ("verlauf", n(self.verlauf.len())),
            ("bekannt", n(self.bekannt.len())),
            ("mempool", n(self.mempool.len())),
            ("offene_zuteilungen", n(self.offene_zuteilungen.len())),
            ("teilgruppe", n(self.teilgruppe.len())),
            ("auszahlungen_offen", n(z.auszahlungen_offen.anzahl())),
            ("baum_im_speicher", z.baum_groessen().0),
            ("baum_seiten_bytes", z.baum_groessen().1),
            // Zustandslose Validierung (2026-10-29).
            ("zustandslos", u64::from(self.zustandslos)),
            ("zeugenlos", self.zeugenlos),
            ("zeugenstuecke_bytes", self.zeugenstuecke.iter().map(|(_, s)| s.iter().map(|t| t.bytes.len() as u64).sum::<u64>()).sum()),
            ("zeugen_platte_bytes", self.zeugenablage.as_ref().map_or(0, |a| a.bytes())),
            ("zeugen_platte_bloecke", self.zeugenablage.as_ref().map_or(0, |a| a.anzahl() as u64)),
            ("zeugen_platte_fehler", self.zeugenablage_fehler),
            ("stimmsatz_kuenftig", z.stimmsaetze.iter().next_back().map_or(0, |(_, s)| s.len() as u64)),
            ("stimmsatz_gewicht", z.stimmsaetze.iter().next_back().map_or(0, |(_, s)| myl_ledger::stimmsatz::gewicht(s))),
            ("eigene_unbekannt", n(self.eigene_unbekannt.len())),
        ]
    }

    /// Die seit dem letzten Abholen abgeschlossenen Prüfaufträge und
    /// entschiedenen Streitfälle.
    pub fn pruefausgaben_abholen(&mut self) -> Pruefausgaben {
        std::mem::take(&mut self.pruefausgaben)
    }

    /// ⚑ **Ein Schiedsurteil buchen** (T6d, Whitepaper 6.6 und Kap. 5.5):
    /// Rechnete der Pod falsch, verliert der Beschuldigte nach der Matrix
    /// (Shard-Miner, falsches Ergebnis: 100 % des Einsatzes), der Anfechter
    /// bekommt das Kopfgeld; war die Anfechtung falsch, verliert der
    /// Anfechter seine Kaution. Verfallen heisst: nichts.
    fn schiedsurteil_buchen(
        zustand: &mut LedgerState,
        urteil: &myl_ledger::pruefung::Schiedsurteil,
    ) -> Result<(), String> {
        use myl_ledger::pruefung::Schiedsausgang;
        use myl_tokenomics::slashing::{Akteur, Grund};
        let f = &urteil.fall;
        // Der überführte Pod verliert seinen Vorbehalt, sein Zwilling bekommt
        // seinen, gleich ob die Schlachtung trägt.
        if urteil.ausgang == Schiedsausgang::PodFalsch {
            myl_ledger::pruefung::vorbehalt_entscheiden(zustand, f.epoche, &f.pod);
        }
        let anfechter = Address::new(*f.anfechter.as_bytes());
        let (outcome, akteur, grund) = match urteil.ausgang {
            Schiedsausgang::Verfallen => return Ok(()),
            Schiedsausgang::PodFalsch => {
                (myl_ledger::transitions::VerdictOutcome::SlashMiner, Akteur::ShardMiner, Grund::FalschesErgebnis)
            }
            Schiedsausgang::AnfechtungFalsch => {
                (myl_ledger::transitions::VerdictOutcome::SlashChecker, Akteur::Checker, Grund::MutwilligeAnfechtung)
            }
        };
        let beschuldigt = match (urteil.ausgang, f.beschuldigt) {
            (Schiedsausgang::PodFalsch, None) => return Err("ohne Beschuldigten".into()),
            (_, b) => b.map(|m| Address::new(*m.as_bytes())).unwrap_or(anfechter),
        };
        let mut kennung = b"myelith-streitfall".to_vec();
        kennung.extend_from_slice(&f.epoche.to_le_bytes());
        kennung.extend_from_slice(f.pod.as_ref());
        kennung.extend_from_slice(&f.segment.to_le_bytes());
        let verdict = myl_ledger::transitions::Verdict {
            segment_id: myl_types::ids::SegmentId::new(Hash::sha256(&kennung).0),
            miner: beschuldigt,
            checker: anfechter,
            outcome,
        };
        myl_tokenomics::slashing::urteil_buchen_gestaffelt(zustand, &verdict, akteur, grund)
            .map(|_| ())
            .map_err(|e| format!("{e:?}"))
    }

    /// 📌 Fund 658: die Zuteilung der ablaufenden Epoche, wenn ein Block der
    /// Epoche `epoch` sie beendet, berechnet vor dem Wechsel, der sie
    /// ersetzt. Gezogen wird erst mit dem Zertifikat, also bleibt sie bis
    /// dahin aufgehoben ([`Self::zuteilung_aufheben`]).
    fn ablaufende_zuteilung(&self, epoch: u64) -> Option<(u64, myl_scheduler::shard_assignment::Zuteilung)> {
        (epoch > self.zustand.epoch.0)
            .then(|| (self.zustand.epoch.0, Self::zuteilung_der_laufenden_epoche(&self.zustand)))
    }

    fn zuteilung_aufheben(&mut self, ablaufend: Option<(u64, myl_scheduler::shard_assignment::Zuteilung)>) {
        let Some(z) = ablaufend else {
            return;
        };
        self.offene_zuteilungen.push(z);
        while self.offene_zuteilungen.len() > OFFENE_ZUTEILUNGEN_HOECHSTENS {
            self.offene_zuteilungen.remove(0);
        }
    }

    /// **Eine Ziehung merken**, bei jedem Knoten, ob er den Block baut oder
    /// empfängt: die gezogenen Segmente, ihre Pods und je Ziehung den
    /// Checker aus den Minern ohne Pod der gezogenen Epoche
    /// ([`pruefer_waehlen`]).
    fn ziehung_merken(&mut self, gezogen: Option<(u64, Vec<crate::stichprobe::Segmentstichprobe>)>) {
        let Some((e, proben)) = &gezogen else {
            return;
        };
        let mut pods: Vec<myl_scheduler::shard_assignment::Pod> = Vec::new();
        let mut pruefer = Vec::new();
        if let Some(stelle) = self.offene_zuteilungen.iter().position(|(alt, _)| alt == e) {
            let (_, z) = self.offene_zuteilungen.remove(stelle);
            for s in proben {
                if let Some(p) = myl_scheduler::zonenzuteilung::pod_zu_kennung(&z, *e, &s.pod) {
                    if !pods.iter().any(|q| q.pod_index == p.pod_index) {
                        pods.push(p.clone());
                    }
                }
            }
        }
        // ⚑ Die Checker stehen im Zustand (2026-10-29): je Auftrag der Ziehung.
        for a in self.zustand.pruefauftraege.iter().filter(|a| a.epoche == *e) {
            pruefer.extend(a.pruefer.iter().map(|m| (a.pod, a.segment, *m)));
        }
        self.pods_der_stichprobe = pods;
        self.pruefer_der_stichprobe = pruefer;
        self.letzte_stichprobe = gezogen;
    }

    /// Zu welcher Epoche die letzte Ziehung gehört, wenn es eine gibt.
    pub fn stichprobenepoche(&self) -> Option<u64> {
        self.letzte_stichprobe.as_ref().map(|(e, _)| *e)
    }

    /// Die Pod-Zuteilung der laufenden Epoche.
    ///
    /// ⚑ **Die einzige Stelle, an der sie entsteht** (Fund 143). Vorher
    /// stand die Ableitung an drei Stellen, jede mit einem eigenen
    /// Blockhash, und zwei davon nahmen den **letzten** statt der
    /// Epochensaat. Zwei Herleitungen derselben Konsensgröße laufen
    /// auseinander, und diese hier taten es bereits: Der Abschluss
    /// rechnete gegen eine Zuteilung, die während der Epoche niemand
    /// kannte.
    ///
    /// **Die Saat kommt aus dem Zustand**, gilt die ganze Epoche und
    /// stammt aus `e−2`; siehe [`myl_ledger::state::LedgerState`].
    ///
    /// ⚑ **Der Rückfall ist strukturell ausgeschlossen, nicht nur
    /// getestet.** Diese Funktion sieht **nur** den Zustand, und im
    /// Zustand steht kein Blockhash: `epochensaat` und
    /// `epochensaat_naechste` wechseln ausschließlich am
    /// Epochenwechsel. Wer den alten Fehler wiederholen wollte, müsste
    /// erst einen Parameter hinzufügen, und das fällt beim Lesen auf.
    pub fn zuteilung_der_laufenden_epoche(
        zustand: &LedgerState,
    ) -> myl_scheduler::shard_assignment::Zuteilung {
        let register = angemeldete_miner(zustand);
        // ⚑ **Nach den Zusagen** (Stufe 4, 6.7, 2026-10-22): kein Pod ohne
        //   Zusage für das Podmodell, nur machbare Pods. Ohne Podmodell gilt
        //   keine Modellzusage, also gibt es keinen Pod.
        let Some(podmodell) = zustand.podmodell else {
            return myl_scheduler::shard_assignment::Zuteilung { ohne_zusage: register, ..Default::default() };
        };
        let epoche = zustand.epoch;
        let mut zuteilung = myl_scheduler::zonenzuteilung::zuteilung_der_epoche_mit_zusagen(
            &register,
            epoche.0,
            &zustand.epochensaat,
            Self::PROBE_SHARDS,
            &podmodell,
            &|m| myl_ledger::transitions::modellzusage_der_epoche(zustand, m, epoche),
        );
        // ⚑ **Ein Pod ohne Zwilling steht in Bereitschaft** (W6, Festlegung
        //   des Projektinhabers vom 2026-10-28): Er kommt gar nicht erst in
        //   die Zuteilung, seine Miner stehen bei `ohne_pod`. So startet er
        //   keine Shards, wird nicht gemessen (sonst sperrte ihn die
        //   Zeitprobe als Verweigerer) und reicht nichts ein, an jeder Stelle
        //   gleich. Die Paare der übrigen bleiben dieselben: Die Paarbildung
        //   wählte ihn nie.
        let mit_schnitt: Vec<myl_scheduler::shard_assignment::Pod> =
            zuteilung.pods.iter().filter(|p| Self::grenzen_des_pods(zustand, p).is_some()).cloned().collect();
        let bereit = myl_scheduler::bereitschaft(&mit_schnitt, &myl_scheduler::zwillingsgruppen(&mit_schnitt));
        if !bereit.is_empty() {
            let (raus, bleiben): (Vec<_>, Vec<_>) =
                std::mem::take(&mut zuteilung.pods).into_iter().partition(|p| bereit.contains(&p.pod_index));
            zuteilung.pods = bleiben;
            for p in raus {
                zuteilung.ohne_pod.extend(p.shards.into_iter().map(|s| s.miner));
                zuteilung.ohne_pod.extend(p.reserve);
            }
        }
        zuteilung
    }

    /// **Die Grenzen eines Pods der laufenden Epoche**, aus den Zusagen
    /// seiner Mitglieder: dieselbe Funktion, nach der die Pod-Bildung
    /// geprüft hat und nach der der Pod rechnet. `None` ohne Podmodell oder
    /// wenn er nicht machbar ist (das schließt die Pod-Bildung aus).
    pub fn grenzen_des_pods(
        zustand: &LedgerState,
        pod: &myl_scheduler::shard_assignment::Pod,
    ) -> Option<Vec<(u32, u32)>> {
        let podmodell = zustand.podmodell?;
        let epoche = zustand.epoch;
        let positionen: Vec<myl_types::ids::MinerId> = pod.shards.iter().map(|s| s.miner.miner_id).collect();
        let reserve: Vec<myl_types::ids::MinerId> = pod.reserve.iter().map(|r| r.miner_id).collect();
        myl_scheduler::ebenenverteilung::bereiche_des_pods(
            &positionen,
            &reserve,
            &|m| myl_ledger::transitions::modellzusage_der_epoche(zustand, m, epoche),
            &podmodell,
        )
        .grenzen()
    }

    /// Die Zuschreibung der **Trainingsarbeit** der abgelaufenen Epoche.
    ///
    /// # ⚑ Gleiche Rate wie Inferenz, andere Quelle
    ///
    /// Ein Trainingssegment wird je Rechenstunde genauso vergütet wie
    /// ein Inferenzsegment. Der Deckel aus Kap. 5.6 begründet sich in
    /// Anhang B.7.3 damit, dass Miner zwischen den Arbeitsklassen
    /// **wählen**; seit die Zuteilung erzwungen ist, wählt niemand, und
    /// derselbe Anhang sagt: „Bei Gleichstand entscheidet allein die
    /// Zuteilung."
    ///
    /// Bezahlt wird aus der **Treasury** und nicht aus der
    /// Shard-Miner-Quote; die Quelle bleibt, was Kap. 5.6 festlegt.
    ///
    /// ⚑ **Nur bestätigte Paare**, denn nur sie sind nachgerechnet
    /// worden. Ein Segment, dem der Partner fehlt, ist Arbeit, die
    /// niemand prüfen kann.
    ///
    /// ⚑ **Die Tokenzahl kommt aus dem verankerten Korpus**, nicht vom
    /// Pod. Dürfte er sie nennen, nennte er seine eigene Vergütung.
    fn trainingszuschreibung(
        zustand: &LedgerState,
        paare: &[(u32, u32)],
        epoche: u64,
    ) -> myl_tokenomics::Zuschreibung {
        let mut zu = myl_tokenomics::Zuschreibung::default();
        let Some(korpus) = zustand.korpus.as_ref() else {
            return zu;
        };
        if !korpus.traegt_ein_buendel() {
            return zu;
        }
        let tokens = korpus.tokens_je_buendel();
        let zuteilung = Self::zuteilung_der_laufenden_epoche(zustand);
        let profil = crate::netz::Netz::des_zustands(zustand).profil;
        let bestaetigt = Self::bestaetigte_deltas(zustand, paare, epoche);
        if bestaetigt.is_empty() {
            return zu;
        }
        for (a, b) in paare {
            let ka = myl_types::miner::pod_kennung(epoche, *a);
            let Some(seg) = zustand.trainingssegmente.get(&ka) else {
                continue;
            };
            if !bestaetigt.contains(&seg.delta_commitment) {
                continue;
            }
            for nr in [a, b] {
                let Some(pod) = zuteilung.pods.iter().find(|p| p.pod_index == *nr) else {
                    continue;
                };
                // ⚑ **Der Schnitt aus einer Stelle** (Stufe 4, 2026-10-21):
                //   dieselbe Funktion, nach der der Pod rechnet. Hier stand
                //   der gleiche Schnitt, selbst ausgerechnet.
                let schnitt = Self::grenzen_des_pods(zustand, pod);
                let Some(grenzen) = schnitt else {
                    continue;
                };
                for (i, sh) in pod.shards.iter().enumerate() {
                    let zuschnitt = myl_tokenomics::vtfe::ShardZuschnitt {
                        layer_start: grenzen[i].0 as u64,
                        layer_end: grenzen[i].1 as u64,
                        hat_embedding: i == 0,
                        hat_lm_kopf: i + 1 == pod.shards.len(),
                    };
                    let Ok(g) = myl_tokenomics::vtfe::vtfe_training_gutschrift(
                        &profil,
                        &zuschnitt,
                        tokens,
                        u64::from(seg.schrittzahl),
                    ) else {
                        continue;
                    };
                    let e = zu.je_miner.entry(sh.miner.miner_id).or_insert(0);
                    *e = e.saturating_add(g);
                }
            }
        }
        zu
    }

    /// Die Δ-Commitments, auf die sich beide Pods eines Paars geeinigt
    /// haben, kanonisch geordnet.
    ///
    /// # ⚑ Warum ein Paar und nicht ein einzelnes Segment zählt
    ///
    /// Ein Ergebnis, das nur einer gerechnet hat, **liesse sich mit
    /// nichts vergleichen**. Die Kette könnte es annehmen oder
    /// verwerfen, aber nie prüfen, und die Modellfassung hinge an einer
    /// Behauptung. Dieselbe Regel wie bei der Inferenz (Kap. 4.4,
    /// `r = 2`); beim Training kann sie nicht schwächer sein, denn ein
    /// falsches Δm verdirbt nicht eine Antwort, sondern das Modell.
    ///
    /// ⚑ **Der Auftrag wird mitgeprüft, nicht nur das Ergebnis.** Zwei
    /// Pods, die Verschiedenes gerechnet haben, sind nicht uneinig,
    /// sondern unvergleichbar, und ihre Übereinstimmung im Ergebnis
    /// wäre ein Zufall, keine Bestätigung.
    ///
    /// ⚑ **Sortiert und nicht in Paarreihenfolge.** Die Ordnung geht in
    /// die neue Modellfassung ein; sie an der Paarbildung aufzuhängen
    /// hiesse, zwei Dinge zu koppeln, die nichts miteinander zu tun
    /// haben.
    fn bestaetigte_deltas(
        zustand: &LedgerState,
        paare: &[(u32, u32)],
        epoche: u64,
    ) -> Vec<Hash> {
        let mut bestaetigt: Vec<Hash> = Vec::new();
        for (a, b) in paare {
            let ka = myl_types::miner::pod_kennung(epoche, *a);
            let kb = myl_types::miner::pod_kennung(epoche, *b);
            let (Some(sa), Some(sb)) =
                (zustand.trainingssegmente.get(&ka), zustand.trainingssegmente.get(&kb))
            else {
                continue;
            };
            if sa.charge == sb.charge
                && sa.modell_version == sb.modell_version
                && sa.lr_zaehler == sb.lr_zaehler
                && sa.lr_nenner == sb.lr_nenner
                && sa.schrittzahl == sb.schrittzahl
                && sa.delta_commitment == sb.delta_commitment
            {
                bestaetigt.push(sa.delta_commitment);
            }
        }
        bestaetigt.sort_unstable_by(|x, y| x.as_bytes().cmp(y.as_bytes()));
        bestaetigt
    }

    /// ⚑ **Die Zwillingsgruppen der laufenden Epoche** (Stufe 1, Whitepaper
    /// 3.4; Festlegung des Projektinhabers vom 2026-10-28): aus den Pods, die
    /// einen Schnitt haben, nach `myl_scheduler::zwillingsgruppen` (Paare;
    /// ein übriger steht in Bereitschaft und ist schon aus der Zuteilung
    /// genommen; allein nur ohne jedes Paar). Die Kette
    /// vergütet nach ihnen, das Podbild der Relaisleitung nennt sie dem
    /// Ortsdienst; eine Quelle für beide.
    pub fn zwillinge_der_laufenden_epoche(zustand: &LedgerState) -> Vec<Vec<u32>> {
        let zuteilung = Self::zuteilung_der_laufenden_epoche(zustand);
        let mit_schnitt: Vec<myl_scheduler::shard_assignment::Pod> =
            zuteilung.pods.iter().filter(|p| Self::grenzen_des_pods(zustand, p).is_some()).cloned().collect();
        myl_scheduler::zwillingsgruppen(&mit_schnitt)
    }

    /// Welche Pods der laufenden Epoche trainieren statt zu rechnen.
    ///
    /// # ⚑ Der Weg, und er war bis zum 2026-09-05 nicht da
    ///
    /// `myl_scheduler::trainingszuteilung` beantwortet die Frage, und
    /// bis heute stellte sie niemand. Dasselbe galt für die ganze Kiste
    /// `myl-train` (Fund 183) und für die Auslastungsrechnung
    /// (Fund 184): gebaut, geprüft, unerreicht.
    ///
    /// ⚑ **Die Nachfrage kommt aus der Vorepoche**
    /// ([`LedgerState::vtfe_vorepoche`]), nicht aus der laufenden. Die
    /// Zuteilung steht fest, bevor die Epoche läuft; wer die laufende
    /// Nachfrage nähme, bräuchte eine Zahl, die es noch nicht gibt.
    ///
    /// ⚑ **Und dieselbe Saat wie die Podbildung**, denn beide gehören
    /// zur selben Epoche. Innerhalb der Auswahl wird sie noch einmal
    /// abgeleitet, damit Trainingslast und Shard-Position nicht an
    /// derselben Permutation hängen.
    pub fn trainingsplan_der_laufenden_epoche(
        zustand: &LedgerState,
    ) -> myl_scheduler::trainingszuteilung::Trainingsplan {
        let zuteilung = Self::zuteilung_der_laufenden_epoche(zustand);
        myl_scheduler::trainingszuteilung::plane(
            &zuteilung,
            zustand.vtfe_vorepoche,
            Self::POD_KAPAZITAET_VTFE,
            &myl_scheduler::trainingszuteilung::Trainingsraten {
                grundrate_bps: Self::TRAININGS_GRUNDRATE_BPS,
                freianteil_bps: Self::TRAININGS_FREIANTEIL_BPS,
            },
            zustand.trainingsstand.als_map(),
            zustand.korpus.as_ref(),
            &zustand.epochensaat.0,
        )
    }

    /// Der Pod zu einer Bündel-Kennung, aus der Zuteilung **der
    /// gefragten Epoche**.
    ///
    /// ⚑ **Nicht aus der laufenden.** Ein Checker fragt zu einer
    /// abgeschlossenen Epoche, und die Zuteilung hängt an ihr: Wer die
    /// heutige nähme, bekäme andere Mitglieder und damit andere
    /// Adressen.
    ///
    /// ⚑ **Die Grenze dieser Auskunft, und sie gehört benannt**
    /// (Fund 143): Die Saat im Zustand ist die der **laufenden**
    /// Epoche. Für eine ältere Epoche liefert diese Funktion deshalb
    /// nur dann die richtige Zuteilung, wenn `epoche` die laufende ist.
    /// **Für ältere fehlt die Saat**, und sie zu erfinden wäre
    /// schlimmer als sie zu vermissen: Der Zustand hebt zwei Saaten
    /// auf, nicht die Historie. Wer weiter zurück fragt, braucht den
    /// Block, in dem die Epoche endete.
    pub fn pod_der_kennung(
        &self,
        epoche: u64,
        kennung: &myl_types::ids::PodId,
    ) -> Option<myl_scheduler::shard_assignment::Pod> {
        if epoche != self.zustand.epoch.0 {
            // 📌 Fund 680, Folge (2026-10-29): Die Spuren kommen seit dem
            //    Einreichen danach, also oft nach dem Wechsel. Die Zuteilung
            //    der abgelaufenen Epoche steht bis zur Ziehung aufgehoben.
            if let Some((_, z)) = self.offene_zuteilungen.iter().find(|(e, _)| *e == epoche) {
                return myl_scheduler::zonenzuteilung::pod_zu_kennung(z, epoche, kennung).cloned();
            }
            // 📌 Fund 658: die Epoche der letzten Ziehung, aus den
            //    festgehaltenen Pods.
            if self.stichprobenepoche() != Some(epoche) {
                return None;
            }
            return self
                .pods_der_stichprobe
                .iter()
                .find(|p| myl_types::pod_kennung(epoche, p.pod_index) == *kennung)
                .cloned();
        }
        let zuteilung = Self::zuteilung_der_laufenden_epoche(&self.zustand);
        myl_scheduler::zonenzuteilung::pod_zu_kennung(&zuteilung, epoche, kennung).cloned()
    }

    /// Die Stichprobenrate, in Basispunkten.
    ///
    /// # ⚑ Abgeleitet und nicht mehr abgeschrieben (Fund 171, 2026-09-04)
    ///
    /// Hier standen **200 bp**, „der Wert aus Kap. 3.4 und dem
    /// Zahlenbeispiel in Anhang B.1". Das war richtig, solange es die
    /// **Kontrollsegmente** gab: Zwei Linien teilten sich die Arbeit,
    /// die Stichprobe `p` und die Einschleusung `gamma`.
    ///
    /// ⚑ **Entscheidung A1 hat `gamma` am 2026-09-02 entfernt.** Seither
    /// muss `p` beides tragen, und
    /// `security_sim.py::zusammengelegte_rate` rechnet die Rate aus, die
    /// beide gleichwertig ersetzt: 4,96 %, aufgerundet **fünf**. Die
    /// Registry hat den neuen Wert übernommen, **diese Konstante
    /// nicht**, und damit hat die Kette seit A1 auf eine zweite Linie
    /// vertraut, die es nicht mehr gibt. Dieselbe Klasse wie Fund 151.
    ///
    /// Sie kommt jetzt aus [`myl_tokenomics::stichprobe_bp`], der einen
    /// maßgeblichen Stelle. Wenn der Konsens die Registry liest (B10),
    /// wird auch diese Konstante überflüssig.
    pub const STICHPROBE_BP: u32 = myl_tokenomics::stichprobe_bp();

    /// ⚑ **Die Fenster eines Prüfauftrags** (2026-10-29), je eine
    /// Viertelepoche: so lange Festlegen ab der Ziehung, danach so lange
    /// Aufdecken. Fest, gleich wer wann einreicht; wer früher fertig ist, deckt
    /// früher auf, sobald alle festgelegt haben.
    pub const PRUEF_FENSTER_BLOECKE: u64 = BLOECKE_JE_EPOCHE / 4;

    /// ⚑ **Das Fenster einer Schiedsrunde** (T6d, 2026-10-29): eine Epoche
    /// ab dem Abschluss des Auftrags, fest, gleich wie viele stimmen. Das Nachrechnen einer Layer dauert Millisekunden;
    /// das Fenster ist für das Netz, nicht für die Rechnung.
    pub const SCHIED_FENSTER_BLOECKE: u64 = BLOECKE_JE_EPOCHE;

    /// **Wer eine Ziehung der laufenden Epoche prüfen darf** (W6b, Frage 4
    /// vom 2026-10-29), berechnet am Wechsel, solange der Zustand noch in
    /// ihr steht: die Miner ohne Pod (darin stehen auch die des Pods in
    /// Bereitschaft, siehe [`Self::zuteilung_der_laufenden_epoche`]) und je
    /// Pod die Mitglieder der übrigen Zwillingsgruppen als Ersatz.
    #[allow(clippy::type_complexity)]
    fn pruefkandidaten(
        zustand: &LedgerState,
        epoche: u64,
    ) -> (
        Vec<myl_types::ids::MinerId>,
        Vec<(myl_types::ids::PodId, Vec<myl_types::ids::MinerId>)>,
        Vec<(myl_types::ids::PodId, Vec<(myl_types::ids::MinerId, u32, u32)>)>,
    ) {
        let z = Self::zuteilung_der_laufenden_epoche(zustand);
        let kandidaten: Vec<myl_types::ids::MinerId> = z.ohne_pod.iter().map(|m| m.miner_id).collect();
        let gruppen = myl_scheduler::zwillingsgruppen(&z.pods);
        let gruppe_von = |i: u32| gruppen.iter().position(|g| g.contains(&i));
        let ersatz = z
            .pods
            .iter()
            .map(|p| {
                let eigene = gruppe_von(p.pod_index);
                let andere: Vec<myl_types::ids::MinerId> = z
                    .pods
                    .iter()
                    .filter(|q| gruppe_von(q.pod_index) != eigene)
                    .flat_map(|q| q.mitglieder().map(|m| m.miner_id))
                    .collect();
                (myl_types::pod_kennung(epoche, p.pod_index), andere)
            })
            .collect();
        // Die Besetzung je Pod mit den Layerbereichen dieser Epoche: Daraus
        // folgt im Streitfall, wen eine Abweichung trifft.
        let besetzung = z
            .pods
            .iter()
            .filter_map(|p| {
                let grenzen = Self::grenzen_des_pods(zustand, p)?;
                let b = p.shards.iter().zip(grenzen).map(|(s, (von, bis))| (s.miner.miner_id, von, bis)).collect();
                Some((myl_types::pod_kennung(epoche, p.pod_index), b))
            })
            .collect();
        (kandidaten, ersatz, besetzung)
    }

    /// ⚑ **Wie viele Segmente eines Pods, der allein rechnete, höchstens
    /// gezogen werden** (Anlaufregel, 2026-10-28): jedes, aber nicht mehr als
    /// diese, damit eine Ziehung die Knoten nicht mit Anfragen flutet. Ein
    /// Pod einer Epoche trägt im Probebetrieb wenige Dutzend.
    pub const VOLL_GEZOGEN_HOECHSTENS: u32 = 64;

    /// **Zieht die Stichproben, deren Epochenwechsel dieses Zertifikat
    /// bestätigt** (Punkt 45).
    ///
    /// # ⚑ Warum diese Zeile der eigentliche Fund ist
    ///
    /// `sample_segments` und `check_segment` waren seit dem 2026-08-17
    /// gebaut, geprüft und abgehakt, und **sie hatten bis zum
    /// 2026-09-01 null Aufrufer** (Fund 114). Damit lief Stufe 2 der
    /// Verifikation in keinem Knoten, und die Sicherheitsbedingung aus
    /// Anhang B.1 hängt an genau der Wahrscheinlichkeit, die dabei null
    /// war.
    ///
    /// **Gezogen wird nur aus Bündeln, deren Aggregatsignatur gilt.**
    /// Wer ein Bündel einreicht, das die Prüfung nicht besteht, bekommt
    /// nichts gutgeschrieben und soll auch keinen Indexraum aufblähen
    /// dürfen: Sonst verdünnte eine Flut ungültiger Bündel die
    /// Stichprobenrate der ehrlichen.
    ///
    /// ⚑ **Seit dem 2026-10-27 wird mit dem Zertifikat gezogen** (CONSENSUS
    /// 7.2, Festlegung des Projektinhabers): Beim Wechsel hebt der Zustand
    /// die Anteile auf ([`myl_ledger::state::OffeneStichprobe`]), und
    /// gezogen wird, sobald ein Block das Zertifikat eines Prüfpunkts an
    /// oder nach dem Wechsel trägt. Es entsteht, nachdem die Bündel
    /// feststehen, also kennt es beim Einreichen niemand. Der Rückfall auf
    /// den Blockhash (Fund 120, unbegrenzter Mahlraum) ist entfallen.
    fn offene_stichproben_ziehen(
        zustand: &mut LedgerState,
        z: &myl_consensus::pruefpunkt::Pruefpunktzertifikat,
        gezogen: &mut Option<(u64, Vec<crate::stichprobe::Segmentstichprobe>)>,
    ) {
        // ⛔️ **Nur in der Startphase** (2026-11-03): Gibt es fuer die Epoche
        //   nach der gezogenen einen Saatschluessel, wartet die Ziehung auf
        //   `β` ([`Self::stichproben_mit_beta_ziehen`]). Das Zertifikat waehlt
        //   sein Zusammensteller; ein Rueckfall darauf machte es lohnend, `β`
        //   zu verhindern.
        let (faellig, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut zustand.stichproben_offen)
            .into_iter()
            .partition(|o| {
                (o.epoche + 1).saturating_mul(BLOECKE_JE_EPOCHE) <= z.hoehe
                    && myl_ledger::saat::schluessel_der_epoche(zustand, o.epoche + 1).is_none()
            });
        zustand.stichproben_offen = rest;
        Self::stichproben_ziehen(zustand, faellig, |e| crate::stichprobe::saat_aus_zertifikat(&z.aggregat, &z.unterzeichner, e), gezogen);
    }

    /// **Mit `β_e` ziehen** (2026-11-03): jede offene Stichprobe einer
    /// Epoche vor `e`. `β_e` entsteht erst in `e`, also nachdem ihre Bündel
    /// feststanden, und jede Menge von Anteilen ergibt dieselbe.
    fn stichproben_mit_beta_ziehen(
        zustand: &mut LedgerState,
        e: u64,
        beta: &myl_types::bls::BlsSignature,
        gezogen: &mut Option<(u64, Vec<crate::stichprobe::Segmentstichprobe>)>,
    ) {
        let (faellig, rest): (Vec<_>, Vec<_>) =
            std::mem::take(&mut zustand.stichproben_offen).into_iter().partition(|o| o.epoche < e);
        zustand.stichproben_offen = rest;
        let quelle = myl_types::schwelle::saat_aus_signatur(beta);
        Self::stichproben_ziehen(zustand, faellig, |o| crate::stichprobe::stichprobensaat(&quelle, o), gezogen);
    }

    /// Zieht die faelligen Stichproben mit der Saat je Epoche.
    fn stichproben_ziehen(
        zustand: &mut LedgerState,
        faellig: Vec<myl_ledger::state::OffeneStichprobe>,
        saat_fuer: impl Fn(u64) -> [u8; 32],
        gezogen: &mut Option<(u64, Vec<crate::stichprobe::Segmentstichprobe>)>,
    ) {
        for o in faellig {
            let saat = saat_fuer(o.epoche);
            let mut stichprobe = crate::stichprobe::stichprobe_aus_anteilen(&o.anteile, &saat, Self::STICHPROBE_BP);
            voll_ziehen(&mut stichprobe, &o.voll, &o.anteile);
            // ⚑ **Die Pflichtaufträge** (Stufe 1 je Sitzung, 2026-11-16):
            //   immer gezogen, nicht per Los, und an ihre Sitzung gebunden
            //   (`auftraege_anlegen` liest die Bindung aus `o.pflicht`).
            for (pod, stelle, _, _) in &o.pflicht {
                let s = crate::stichprobe::Segmentstichprobe { pod: *pod, segment: *stelle };
                if !stichprobe.contains(&s) {
                    stichprobe.push(s);
                }
            }
            // Die Wurzeln dazu, gegen die gelieferte Spuren geprüft werden.
            zustand.stichprobe_wurzeln = o.wurzeln.clone();
            // ⚑ **Die Prüfaufträge** (2026-10-29): je Ziehung zwei Checker
            //   mit festen Fristen, aus den Kandidaten der gezogenen Epoche.
            let hoehe = zustand.historie.anzahl();
            let paare: Vec<(myl_types::ids::PodId, u32)> = stichprobe.iter().map(|s| (s.pod, s.segment)).collect();
            myl_ledger::pruefung::auftraege_anlegen(
                zustand,
                &o,
                &paare,
                hoehe + Self::PRUEF_FENSTER_BLOECKE,
                hoehe + 2 * Self::PRUEF_FENSTER_BLOECKE,
            );
            *gezogen = Some((o.epoche, stichprobe));
        }
    }

    /// Schließt die vorige Epoche ab, wenn eine neue beginnt (Punkt 38).
    ///
    /// # Was hier geschieht und was nicht
    ///
    /// Der Aufruf geht an [`myl_tokenomics::epochenausschuettung`]: Der
    /// Knoten rechnet die Prägung **nicht selbst**, er ruft sie. Die
    /// Formeln gehören in die Wirtschaft, der Aufruf in die Kette.
    ///
    /// ⚑ **Die Zuschreibung ist heute leer, und das ist kein
    /// Versehen.** Sie leitet sich aus bestätigten PoI-Bündeln ab, und
    /// **diese Kette trägt keine**: `Anweisung` kennt Burn, Überweisung
    /// und die drei Sitzungsanweisungen, kein Bündel. Ohne
    /// bezeugte Arbeit gibt es nichts zuzuschreiben.
    ///
    /// **Die Folge ist die sichere:** Der Shard-Miner-Anteil wird
    /// **nicht geprägt**, weil ihm kein Empfänger gegenübersteht, und
    /// das Ergebnis benennt es. Geprägt wird allein der Treasury-Anteil.
    /// Die Geldmenge wächst also um das, was ankommt, und um sonst
    /// nichts.
    ///
    /// **Was damit noch fehlt**, ist eine Anweisung, die ein Bündel in
    /// die Kette trägt, samt ihrer Prüfung. Das ist der nächste Draht
    /// und keine Rechnung mehr.
    ///
    /// # Warum ein Fehlschlag den Block nicht verwirft
    ///
    /// Scheitert der Abschluss, wird er **übersprungen**, wie eine
    /// gescheiterte Transaktion. Beide Seiten überspringen dasselbe,
    /// weil beide dieselbe Funktion durchlaufen; ein Abbruch hier
    /// hielte die Kette an, und zwar bei allen gleichzeitig.
    fn epochenwechsel_abschliessen(
        zustand: &mut LedgerState,
        neue_epoche: u64,
        letzter_hash: &Hash,
        stimmsatz: Option<&myl_consensus::validator::VotingSet>,
    ) {
        if neue_epoche <= zustand.epoch.0 {
            return;
        }
        // ⚑ **Die Saat des Erzeugerplans** (2026-11-04), vor allem anderen:
        //   aus demselben Zustand, gegen den der Erzeuger dieses Blocks
        //   schon geprueft ist.
        if let Some(saat) = Self::erzeugersaat_fuer(zustand, neue_epoche) {
            zustand.erzeugersaat = saat;
        }
        let alte_epoche = zustand.epoch.0;
        // Vor der Ausschüttung, die den Zähler leert: gekauft zum Preis
        // dieser Epoche.
        let nachgefragt = nachgefragte_vtfe(zustand.burn_epoche, zustand.credit_price);
        let Epochenzuschreibung { mut zuschreibung, zurueck, allein, bezeugt, einig, uneinig, verschieden, vorbehalten, pflicht } =
            Self::zuschreibung_der_epoche(zustand);
        zustand.zwillinge_einig = zustand.zwillinge_einig.saturating_add(einig);
        zustand.zwillinge_uneinig = zustand.zwillinge_uneinig.saturating_add(uneinig);
        // ⚑ **Anlaufregel** (Stufe 1, Festlegung des Projektinhabers vom
        //   2026-10-28): Was ein Pod allein rechnete, wird erst nach der
        //   Streitfrist gutgeschrieben, dann in der Ausschüttung jener Epoche;
        //   was fällig ist, kommt jetzt dazu.
        zurueckhalten_und_freigeben(&mut zustand.zurueckgehalten, alte_epoche, &mut zuschreibung.je_miner, zurueck);
        // ⚑ **Die Rückbuchung** (Whitepaper 6.4, 2026-10-29): Was frei wurde
        //   (der Zwilling ist überführt), kommt in diese Ausschüttung; was
        //   uneinig ist, wird zurückgehalten bis zum Urteil, höchstens die
        //   Streitfrist.
        for (m, v) in myl_ledger::pruefung::vorbehalte_faellig(zustand, alte_epoche) {
            let e = zuschreibung.je_miner.entry(m).or_insert(0);
            *e = e.saturating_add(v);
        }
        for (pod, zwilling, je) in vorbehalten {
            zustand.vorbehalte.push(myl_ledger::pruefung::Vorbehalt {
                epoche: alte_epoche,
                pod,
                zwilling,
                je_miner: je.into_iter().collect(),
                frei: false,
                bis_epoche: alte_epoche.saturating_add(myl_ledger::einsatz::SPERRFRIST_EPOCHEN),
                pflicht: None,
            });
        }
        // ⚑ **Was dem Zwilling fehlt** (Stufe 1 je Sitzung, 2026-11-16):
        //   je Sitzung ein eigener Vorbehalt, frei erst, wenn ihr
        //   Pflichtauftrag einig und ohne Abweichung schliesst.
        for (pod, zwilling, f, je) in &pflicht {
            zustand.vorbehalte.push(myl_ledger::pruefung::Vorbehalt {
                epoche: alte_epoche,
                pod: *pod,
                zwilling: *zwilling,
                je_miner: je.iter().map(|(m, v)| (*m, *v)).collect(),
                frei: false,
                bis_epoche: alte_epoche.saturating_add(myl_ledger::einsatz::SPERRFRIST_EPOCHEN),
                pflicht: Some(f.stelle),
            });
        }
        // ⚑ **Die Trainingszuschreibung entsteht VOR der Ausschüttung**
        // und aus dem Zustand, den die abgelaufene Epoche hinterlassen
        // hat. Danach sind die Segmente weg.
        let trainingsplan = Self::trainingsplan_der_laufenden_epoche(zustand);
        let trainingszuschreibung =
            Self::trainingszuschreibung(zustand, &trainingsplan.paare, alte_epoche);
        // ⚑ **Der Checker-Pool** (Anhang B.3, 2026-10-29): nach dem geprüften
        //   Volumen der Epoche; danach beginnt es von vorn.
        let pruefvolumen = zustand.pruefvolumen.nehmen();
        // ⚑ **Vorgemerkt** (verteilte Ausschüttung, 2026-10-30): gebucht wird
        //   in diesem und den folgenden Blöcken, höchstens
        //   `AUSZAHLUNGEN_JE_BLOCK` je Block (`anwenden`).
        let ausschuettung = myl_tokenomics::ausschuettung::epochenausschuettung_vorgemerkt(
            zustand,
            &zuschreibung,
            &trainingszuschreibung,
            &pruefvolumen,
            // ⚑ **Die Auslastung steuert die Trainingsabgabe**: bei
            // hoher Last führt das Netz ab, bei niedriger zahlt es aus.
            // Sie kommt aus demselben Plan, aus dem die Trainingspods
            // kommen; eine zweite Rechnung wäre eine zweite Wahrheit.
            trainingsplan.auslastung,
            &Self::praegeparameter(alte_epoche),
        );
        // ⚑ **Die Archivpflicht der abgelaufenen Epoche** (Stufe A5):
        //   Gutschrift aus dem, was die Treasury eben bekam, verfehlte
        //   Epochen, Verstoß und Nachrücken.
        let treasury_eingang = ausschuettung
            .ok()
            .and_then(|a| a.je_konto.get(&myl_types::treasury::treasury_adresse()).copied())
            .unwrap_or(0);
        let _ = myl_ledger::archiv::epoche_abschliessen(
            zustand,
            alte_epoche,
            treasury_eingang,
            crate::archiv::WARTE_EPOCHEN,
        );
        // ⚑ **Stufe 2 wird gezogen, und zwar hier** (Punkt 45, Fund
        // 114). Vorher lief sie in keinem Knoten, und `p` aus Anhang
        // B.1 war null. Gezogen wird aus den **bezeugten** Bündeln,
        // also denen, deren Aggregatsignatur galt.
        //
        // ⚑ **Seit dem 2026-10-27 wird hier nur aufgehoben** (CONSENSUS
        // 7.2): die Anteile der bezeugten Bündel, gezogen wird mit dem
        // Zertifikat des Prüfpunkts an diesem Wechsel
        // (`offene_stichproben_ziehen`).
        let (pruefkandidaten, ersatz, besetzung) = Self::pruefkandidaten(zustand, alte_epoche);
        zustand.stichproben_offen.push(myl_ledger::state::OffeneStichprobe {
            epoche: alte_epoche,
            anteile: bezeugt.iter().map(|b| (b.pod, b.segmente)).collect(),
            wurzeln: bezeugt.iter().map(|b| (b.pod, b.segments_root, b.segmente)).collect(),
            voll: allein,
            pruefkandidaten,
            ersatz: ersatz.into_iter().filter(|(p, _)| bezeugt.iter().any(|b| b.pod == *p)).collect(),
            besetzung: besetzung.iter().filter(|(p, _)| bezeugt.iter().any(|b| b.pod == *p)).cloned().collect(),
            pflicht: pflicht.iter().map(|(pod, _, f, _)| (*pod, f.stelle, f.sitzung, f.segmente)).collect(),
        });
        // ⚑ **Uneinige Paare** (Whitepaper 6.4, Festlegung des
        //   Projektinhabers vom 2026-10-29): bis eine Epoche nach dem Wechsel
        //   darf ein Differenzbeweis gemeldet werden.
        let hoehe = zustand.historie.anzahl();
        for (a, b) in verschieden {
            let nur = |p: &myl_types::ids::PodId| *p == a.pod || *p == b.pod;
            let offen = zustand.stichproben_offen.last().cloned();
            let Some(offen) = offen else {
                continue;
            };
            zustand.uneinigkeiten.push(myl_ledger::pruefung::Uneinigkeit {
                stichprobe: myl_ledger::state::OffeneStichprobe {
                    epoche: alte_epoche,
                    anteile: Vec::new(),
                    wurzeln: offen.wurzeln.iter().filter(|(p, _, _)| nur(p)).cloned().collect(),
                    voll: Vec::new(),
                    pruefkandidaten: offen.pruefkandidaten.clone(),
                    ersatz: offen.ersatz.iter().filter(|(p, _)| nur(p)).cloned().collect(),
                    besetzung: offen.besetzung.iter().filter(|(p, _)| nur(p)).cloned().collect(),
                    pflicht: Vec::new(),
                },
                ausgabenwurzeln: vec![(a.pod, a.ausgabenwurzel), (b.pod, b.ausgabenwurzel)],
                gemeldet: Vec::new(),
                bis_hoehe: hoehe + BLOECKE_JE_EPOCHE,
            });
        }
        // Gebuchte Doppelbelege fallen nach der Sperrfrist weg (CONSENSUS
        // 7.6); ältere Taten sind ohnehin nicht mehr schlachtbar.
        zustand
            .doppelbelege
            .retain(|_, b| b.epoche.saturating_add(myl_ledger::einsatz::SPERRFRIST_EPOCHEN) >= neue_epoche);
        Self::gebuehren_teilen(zustand);
        // ⚑ Der Topf an die, die die Finalitaet der abgelaufenen Epoche
        //   trugen (7.8 Schritt 2, 2026-11-04).
        let traeger = Self::finalitaetssatz_von(zustand, stimmsatz, alte_epoche);
        Self::topf_auszahlen(zustand, traeger.as_ref());
        // ⚑ **Der Credit-Preis folgt der Auslastung** (Whitepaper 5.4),
        //   aus derselben Auslastung wie die Trainingsabgabe; er gilt ab der
        //   neuen Epoche.
        let auslastung = preis_auslastung(trainingsplan.auslastung, nachgefragt, trainingsplan.kapazitaet);
        let (fein, ganz) = credit_preis_naechster(zustand.credit_preis_fein, auslastung);
        zustand.credit_preis_fein = fein;
        zustand.credit_price = ganz;
        // ⚑ **Abgelaufene Sitzungen fallen weg, ihre Hinterlegung geht
        //   zurück** (2026-10-27, Fund 662). Vorher rief die Kette
        //   `sitzung_aufraeumen` nie, und jede Sitzung blieb für immer im
        //   Zustand.
        //   ⚑ Nur die weggefallenen (Skalierung, 2026-10-29); bis hierher
        //   lief die Suche über alle Hinterlegungen.
        for id in myl_ledger::transitions::sitzung_aufraeumen(zustand) {
            Self::zurueckgeben(zustand, HINTERLEGT_SITZUNG, *id.as_bytes());
        }
        // ⛔️ Gebuchte Doppelbelege nach der Sperrfrist (2026-10-30).
        myl_ledger::transitions::doppelbelege_aufraeumen(zustand);
        // ⚑ Der Stimmsatz der übernächsten Epoche aus dem Zustand (7.8,
        //   2026-11-02); bestimmt und geführt, wirkt noch nicht (Schritt 2
        //   mit einer Regel für den Erzeuger).
        myl_ledger::stimmsatz::fortschreiben(zustand, neue_epoche, &Self::satzregeln(zustand), &zuschreibung.je_miner);
        // ⚑ **Die Saat** (2026-11-03): die Runde dieser Epoche abschliessen
        //   (ohne neuen Schluessel gilt der alte weiter), die der naechsten
        //   anlegen (ihr Satz steht seit dem vorigen Wechsel), Altes raeumen.
        //   Schluessel von Minern, die abgemeldet sind, fallen weg.
        myl_ledger::saat::abschliessen(zustand, neue_epoche);
        myl_ledger::saat::runde_anlegen(zustand, neue_epoche + 1);
        myl_ledger::saat::aufraeumen(zustand, neue_epoche);
        zustand.verschluesselung.retain(|m, _| zustand.miner.contains_key(m));
        // ⚑ **Und die Bündel der abgerechneten Epoche fallen weg.**
        // Ohne dies wüchse der Zustand unbegrenzt, und Entscheidung D7
        // wäre gebrochen; die Historie steht in den Blöcken.
        //
        // ⚑ **Heute werden sie verworfen, ohne zugeschrieben zu
        // werden**, weil dafür die Pod-Besetzung im Zustand fehlt
        // (Glied 3c). Verloren geht dabei nichts, denn es wurde für sie
        // auch nichts geprägt; **gewonnen ist nur, dass der Weg in die
        // Kette jetzt steht**.
        let _ = buendel_leeren(zustand);
        // ⚑ **Und die Trainingssegmente ebenso** (2026-09-05). Aus
        // demselben Grund: Eine je Epoche wachsende Menge im Zustand
        // bräche Entscheidung D7, weil `commitment` den ganzen Zustand
        // serialisiert.
        //
        // ⚑ **Auch sie werden heute verworfen, ohne zugeschrieben zu
        // werden.** Die Vergütung eines Trainingssegments hängt an der
        // Prüfung, und die steht noch nicht: Redundanz und Bisektion auf
        // Trainingssegmente sind offen. Gewonnen ist, dass der Weg in
        // die Kette steht.
        // ⚑ **Was zwei Pods übereinstimmend gerechnet haben, bewegt das
        // Modell** (2026-09-05). Alles andere nicht: Ein Ergebnis, das
        // nur einer geliefert hat, liesse sich mit nichts vergleichen,
        // und die Modellfassung hinge an einer Behauptung.
        //
        // ⚑ **Die Kette summiert die Δm nicht.** Sie hält das Rezept aus
        // alter Fassung und geordneter Liste der bestätigten
        // Commitments; wer die Gewichte hat, wendet es an und rechnet
        // die Wurzel nach.
        {
            let plan = Self::trainingsplan_der_laufenden_epoche(zustand);
            let bestaetigt = Self::bestaetigte_deltas(zustand, &plan.paare, alte_epoche);
            zustand.modell_version =
                myl_types::modellversion::naechste_version(&zustand.modell_version, &bestaetigt);
        }
        let _ = myl_ledger::transitions::trainingssegmente_leeren(zustand);
        Self::probepraegung(zustand, alte_epoche);
        // ⚑ **Und die Saat rückt weiter** (Fund 143).
        //
        // **Hinter dem Abschluss, nicht davor.** Die abgerechnete
        // Epoche musste mit der Saat abgerechnet werden, die
        // **während** ihr galt; drehte man zuerst, rechnete der
        // Abschluss gegen eine Zuteilung, die es in dieser Epoche nie
        // gab.
        //
        // Danach gilt: Die Saat der Epoche `e` ist der letzte
        // Blockhash von `e−2`, also zwei Epochen Vorlauf. Genauso weit
        // reicht der Registrierungsschluss aus Anhang A.2, und das ist
        // kein Zufall: Wäre die Saat näher als der Schluss, könnte
        // sich jemand anmelden, nachdem er sie kennt.
        // ⚑ **Wer in der abgelaufenen Epoche zum Training herangezogen
        // war, wird vermerkt** (2026-09-05). Ohne diesen Eintrag bliebe
        // `trainingsstand` leer, die Reihum-Ordnung hätte nichts zu
        // ordnen, und die Auswahl fiele auf das blosse Los zurück.
        //
        // ⚑ **Vor dem Drehen der Saat**, aus demselben Grund, aus dem
        // der Abschluss davor steht: Der Plan der abgelaufenen Epoche
        // hängt an der Saat, die **während** ihr galt. Danach wäre es
        // der Plan einer Epoche, die es nie gab.
        {
            let plan = Self::trainingsplan_der_laufenden_epoche(zustand);
            let zuteilung = Self::zuteilung_der_laufenden_epoche(zustand);
            zustand.trainingsstand.veraendern(|stand| {
                myl_scheduler::trainingszuteilung::stand_fortschreiben(stand, &zuteilung.pods, &plan.pods, alte_epoche)
            });
        }
        // ⚑ **Und die Saat rückt weiter** (Fund 143), siehe oben.
        zustand.epochensaat = zustand.epochensaat_naechste;
        // ⛔️ **Mit einem Saatschluessel nicht mehr aus dem Blockhash**
        //   (2026-11-03): Die naechste kommt aus `β` dieser Epoche, sobald es
        //   steht ([`Self::beta_verwenden`]). Kommt es nicht, bleibt sie die
        //   jetzige, und die Zuteilung wiederholt sich; ein Rueckfall auf den
        //   Hash machte es lohnend, `β` zu verhindern.
        if myl_ledger::saat::schluessel_der_epoche(zustand, neue_epoche).is_none() {
            zustand.epochensaat_naechste = *letzter_hash;
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
    ///
    /// # ⚑ Und hier wechselt die Epoche (Punkt 38)
    ///
    /// Wächst `epoch` gegenüber dem Zustand, wird die **vorige** Epoche
    /// abgeschlossen, bevor die neue gilt: geglätteten Burn
    /// fortschreiben, prägen, verteilen, gutschreiben. Das geschieht
    /// **hier und nirgends sonst**, aus demselben Grund, aus dem das
    /// Anwenden hier steht: Erzeuger und Übernehmer müssen dieselbe
    /// Folge von Änderungen ausführen. Ein Abschluss, den nur der
    /// Erzeuger rechnet, wäre eine abweichende Zustandswurzel.
    // Zwei Ausgaben neben dem Zustand (Ziehung, Abschlüsse) und fünf
    // Eingaben; ein Bündel daraus hätte nur einen Namen mehr.
    #[allow(clippy::too_many_arguments)]
    fn anwenden(
        zustand: &mut LedgerState,
        txs: &[Transaktion],
        epoch: u64,
        letzter_hash: &Hash,
        pruefpunkt: Option<&myl_consensus::pruefpunkt::Pruefpunktzertifikat>,
        gezogen: &mut Option<(u64, Vec<crate::stichprobe::Segmentstichprobe>)>,
        stimmsatz: Option<&myl_consensus::validator::VotingSet>,
        ausgaben: &mut Pruefausgaben,
        schon_geprueft: &dyn Fn(&Transaktion) -> bool,
    ) -> Vec<bool> {
        // ⚑ **Die Historie zuerst** (Archivknoten, Stufe A2): der Hash des
        //   Vorgängers, also des Blocks, an den dieser anschliesst. Erzeuger
        //   und Übernehmer gehen beide hier durch und hängen dasselbe an.
        zustand.historie.anhaengen(letzter_hash);
        // ⚑ **Der Hash eines Prüfpunkts** (2026-11-03), wenn der Vorgänger
        //   einer ist: gegen ihn prüft die Kette Stimmen als Transaktion.
        //   Die laufende und die vorige Epoche.
        let vorgaenger = zustand.historie.anzahl() - 1;
        if myl_consensus::pruefpunkt::ist_pruefpunkt(vorgaenger) {
            zustand.pruefpunkt_hashes.push((vorgaenger, *letzter_hash));
        }
        zustand.pruefpunkt_hashes.retain(|(h, _)| epoche_fuer_hoehe(*h).saturating_add(1) >= epoch);
        zustand.pruefpunkt_quellen.retain(|(h, _, _)| epoche_fuer_hoehe(*h).saturating_add(1) >= epoch);
        // ⚑ **Fällige Prüfaufträge abschliessen** (2026-10-29), vor dem
        //   Epochenwechsel: Was in diesem Block einig endet, zählt noch in
        //   die Auszahlung des Wechsels.
        let hoehe = zustand.historie.anzahl();
        myl_ledger::pruefung::uneinigkeiten_aufraeumen(zustand, hoehe);
        ausgaben.abschluesse.extend(myl_ledger::pruefung::faellige_abschliessen(zustand, hoehe, Self::SCHIED_FENSTER_BLOECKE));
        // ⚑ **Streitfälle entscheiden** (T6d, Whitepaper 6.6): mehr als zwei
        //   Drittel des Stimmsatzes, sonst Verfall nach der Frist. Die
        //   Schlachtung nach der Matrix: der Beschuldigte mit 100 % seines
        //   Einsatzes, Kopfgeld an den Anfechter; oder die Kaution des
        //   Anfechters.
        // ⚑ Seit 7.8 Schritt 2 (2026-11-04) entscheidet der Satz, der die
        //   Finalitaet der laufenden Epoche traegt.
        if let Some(v) = Self::finalitaetssatz_von(zustand, stimmsatz, zustand.epoch.0) {
            for urteil in myl_ledger::pruefung::streitfaelle_entscheiden(zustand, hoehe, |m| v.weight(m), v.quorum_threshold()) {
                let gebucht = Self::schiedsurteil_buchen(zustand, &urteil);
                ausgaben.urteile.push((urteil, gebucht.err()));
            }
        }
        Self::epochenwechsel_abschliessen(zustand, epoch, letzter_hash, stimmsatz);
        // ⚑ Die vorgemerkten Auszahlungen, begrenzt je Block (nach dem
        //   Wechsel, damit der Wechselblock selbst schon bucht; bei wenigen
        //   Minern ist damit alles im selben Block bezahlt).
        myl_ledger::transitions::auszahlungen_buchen(zustand, myl_ledger::transitions::AUSZAHLUNGEN_JE_BLOCK);
        // ⚑ **Das Zertifikat eines Prüfpunkts** (2026-10-27, CONSENSUS
        //   7.2): bestätigt bis zu seiner Höhe, und was auf einen
        //   bestätigten Wechsel wartete, wird jetzt gezogen. Geprüft ist es,
        //   bevor `anwenden` gerufen wird.
        //   ⚑ Seit 2026-11-04 (7.7 Schritt 1) rechtfertigt das
        //   Zertifikat sein Ziel; endgueltig wird seine Quelle, wenn das
        //   Ziel ihr direkter Nachfolger ist. Dass die Quelle die vermerkte
        //   ist, hat `zertifikat_traegt_gegen` geprueft.
        if let Some(z) = pruefpunkt {
            zustand.pruefpunkt_gerechtfertigt = (z.hoehe, z.block_hash);
            if z.schliesst_ab() && z.quelle.hoehe > zustand.pruefpunkt_bestaetigt {
                zustand.pruefpunkt_bestaetigt = z.quelle.hoehe;
            }
            // Für den Topf der Validatoren (2026-10-28): wer unterschrieb.
            //   ⚑ Und je Unterzeichner die Stimme für die Teilnahme (7.8,
            //   2026-11-03); eine als Transaktion nachgetragene zählt nicht
            //   noch einmal.
            let (e, platz) = Self::pruefpunkt_platz(z.hoehe);
            for u in &z.unterzeichner {
                *zustand.pruefpunkt_stimmen.entry(*u).or_default() += 1;
                myl_ledger::stimmsatz::stimme_vermerken(zustand, u, e, platz, epoch);
            }
            Self::offene_stichproben_ziehen(zustand, z, gezogen);
        }
        // ⚑ **Die Quelle dieses Blocks, wenn er ein Pruefpunkt ist**
        //   (2026-11-04): der gerechtfertigte nach dem Zertifikat, das er
        //   selbst traegt. Jede Stimme fuer ihn nennt sie.
        let diese_hoehe = zustand.historie.anzahl();
        if myl_consensus::pruefpunkt::ist_pruefpunkt(diese_hoehe) {
            let (q, x) = zustand.pruefpunkt_gerechtfertigt;
            zustand.pruefpunkt_quellen.push((diese_hoehe, q, x));
        }
        // ⚑ **Die Ziehung der Archivkopien** (Stufe A5) am Grenzblock, nach
        //   dem Abschluss der letzten Epoche der alten Periode und vor den
        //   Transaktionen: Die Historie zählt jetzt genau die Blöcke vor der
        //   Grenze.
        let hoehe = zustand.historie.anzahl();
        if hoehe > 0 && hoehe.is_multiple_of(crate::archiv::PERIODE_BLOECKE) {
            myl_ledger::archiv::grenze_ziehen(zustand, hoehe, epoch, crate::archiv::WARTE_EPOCHEN);
        }
        // ⚑ **Die Epoche des Zustands wird mitgeführt** (seit
        // 2026-08-27). Vorher stand sie auf 0 und blieb dort: `anwenden`
        // benutzte die Epoche nur, um den Verfall auszurechnen, und
        // niemand setzte `zustand.epoch`. Damit lief jede Prüfung, die
        // an der laufenden Epoche hängt, gegen 0 — der Verfall von
        // Credits ebenso wie das Fenster der Verstoßhistorie.
        //
        // Sie steht **vor** dem Anwenden, weil die Übergänge sie lesen.
        zustand.epoch = EpochId(epoch);
        let netz = crate::netz::Netz::des_zustands(zustand).startwert_hash();
        // ⚑ **Träge, nicht immer.** Die meisten Blöcke tragen kein
        // Bündel, und die Ableitung mischt das ganze Register.
        let mut zuteilung: Option<myl_scheduler::shard_assignment::Zuteilung> = None;
        // Je Transaktion, ob sie ihre Nummer verbraucht hat (Fund 627).
        let mut verbraucht = vec![false; txs.len()];
        // Die Gebühreneinheiten der verbrauchten Transaktionen, für den
        // Grundpreis (G1, seit 2026-10-28 Bytes plus Prüfeinheit).
        let mut bytes_im_block: u64 = 0;
        let gebuehren = crate::netz::Netz::des_zustands(zustand).gebuehren;
        // Nie unter dem Boden, auch nicht im ersten Block eines Zustands.
        zustand.grundpreis_je_byte = zustand.grundpreis_je_byte.max(gebuehren.boden_je_byte);
        // ⚑ **Alle Unterschriften des Blocks auf einmal** (2026-10-28): als
        //   Stapel, und nur wenn der nicht trägt, einzeln; das Ergebnis je
        //   Transaktion ist dasselbe wie mit `pruefe` (gemessen: rund 90 statt
        //   470 µs je Unterschrift).
        let mut vorgeprueft = myl_consensus::block::pruefe_alle_ausser(txs, &netz, schon_geprueft).into_iter();
        for (i, _) in txs.iter().enumerate() {
            // ⚑ **Die Unterschrift wird hier geprüft und nicht erst bei
            // der Aufnahme in den Mempool** (2026-08-28, Fund 85). Ein
            // Block kommt über Gossip und sieht den Mempool nie; läge
            // die Prüfung dort, könnte ein Leader eine unsignierte
            // Anweisung in einen Block schreiben, und die ehrlichen
            // Knoten wendeten sie an.
            //
            // **Erzeuger und Übernehmer überspringen dasselbe**, weil
            // beide dieselbe Funktion durchlaufen.
            let Some(geprueft) = vorgeprueft.next().flatten() else {
                continue;
            };
            let absender = geprueft.inhalt().absender_adresse();

            // ⚑ **Die Gebühr** (2026-10-27, G1): Grundpreis mal Bytes,
            //   höchstens `max_gebuehr`. Wer sie nicht decken kann oder
            //   weniger zugesagt hat, wartet, ohne seine Nummer zu verbrauchen.
            let bytes = geprueft.inhalt().gebuehreneinheiten();
            let gebuehr = zustand.grundpreis_je_byte.saturating_mul(bytes);
            if gebuehr > geprueft.inhalt().max_gebuehr || zustand.account(&absender).balance < gebuehr {
                continue;
            }

            // ⚑ **Die Nummer wird auch dann verbraucht, wenn die
            // Anweisung danach scheitert.** Sonst wäre eine ungedeckte
            // Überweisung unverändert gültig und beliebig oft
            // einreichbar.
            if nonce_verbrauchen(zustand, &absender, geprueft.inhalt().nonce).is_err() {
                continue;
            }
            verbraucht[i] = true;
            // Gezahlt wird auch, wenn die Anweisung danach scheitert: Die
            // Bytes stehen im Block.
            zustand.account_mut(&absender).balance -= gebuehr;
            zustand.gebuehren_epoche = zustand.gebuehren_epoche.saturating_add(gebuehr);
            bytes_im_block = bytes_im_block.saturating_add(bytes);

            // Gescheiterte Anweisungen werden übersprungen, nicht
            // abgebrochen: Eine Überweisung ohne Deckung ist kein Grund,
            // den Block zu verwerfen.
            match &geprueft.inhalt().anweisung {
                Anweisung::Burn { betrag } => {
                    // Verfall eine Epoche später: eine Festlegung dieser
                    // Testkette, damit beide Seiten dieselbe rechnen.
                    // ⚑ W4: In der Subventionsphase mit Deckel je Adresse
                    //   (Whitepaper 5.6); darüber scheitert die Anweisung.
                    let subvention = myl_tokenomics::anlaufrate(zustand.epoch.0).0 > 0;
                    if myl_tokenomics::burn_im_deckel(zustand, &absender, *betrag, subvention)
                        && burn_to_credits(zustand, &absender, *betrag, EpochId(epoch + 1)).is_ok()
                        && subvention
                    {
                        myl_ledger::transitions::burn_vermerken(zustand, &absender, *betrag);
                    }
                }
                // ⚑ G1: An ein neues Konto zahlt der Absender einmal den
                //   Aufschlag an die Treasury; ein Konto bleibt für immer.
                Anweisung::Ueberweisung { nach, betrag } => {
                    let aufschlag = if zustand.accounts.contains_key(nach) { 0 } else { gebuehren.aufschlag_konto };
                    if zustand.account(&absender).balance >= betrag.saturating_add(aufschlag)
                        && transfer(zustand, &absender, nach, *betrag).is_ok()
                        && aufschlag > 0
                    {
                        Self::an_die_treasury(zustand, &absender, aufschlag);
                    }
                }
                // ⚑ G1: Eine Sitzung hinterlegt, zurück beim Aufräumen.
                Anweisung::SitzungEroeffnen { kontrakt } => {
                    let h = gebuehren.hinterlegung_sitzung;
                    if zustand.account(&absender).balance >= h
                        && sitzung_eroeffnen(zustand, &absender, kontrakt.clone()).is_ok()
                    {
                        Self::hinterlegen(zustand, &absender, HINTERLEGT_SITZUNG, *kontrakt.adresse().as_bytes(), h);
                    }
                }
                // ⚑ Die Kennung folgt aus dem Absender und steht nicht
                // in der Anweisung: Beide sind derselbe Schlüssel, und
                // ein zweites Feld ließe sich abweichend füllen.
                // ⚑ Der Schlüssel kommt aus der **geprüften**
                // Transaktion, nicht aus einem Feld der Anweisung.
                // Damit ist der Besitz bewiesen: Wer unterschreiben
                // kann, hält den geheimen Teil.
                Anweisung::MinerAnmelden {
                    hardware,
                    zone,
                    netzadresse,
                } => {
                    let kennung = myl_types::ids::MinerId::new(*absender.as_bytes());
                    // ⚑ G1: Nur eine neue Anmeldung hinterlegt; wer seine
                    //   Angaben ändert, hat schon hinterlegt.
                    let neu = !zustand.miner.contains_key(&kennung);
                    let h = if neu { gebuehren.hinterlegung_anmeldung } else { 0 };
                    if zustand.account(&absender).balance >= h
                        && miner_anmelden(
                            zustand,
                            &absender,
                            &kennung,
                            *hardware,
                            *zone,
                            geprueft.inhalt().absender,
                            *netzadresse,
                        )
                        .is_ok()
                        && h > 0
                    {
                        Self::hinterlegen(zustand, &absender, HINTERLEGT_ANMELDUNG, *kennung.as_bytes(), h);
                    }
                }
                Anweisung::MinerAbmelden => {
                    let kennung = myl_types::ids::MinerId::new(*absender.as_bytes());
                    if miner_abmelden(zustand, &absender, &kennung).is_ok() {
                        Self::zurueckgeben(zustand, HINTERLEGT_ANMELDUNG, *kennung.as_bytes());
                    }
                }
                // ⚑ **Die Kapazitätszusage** (Stufe 4 des Dispatchers,
                //   2026-10-21): je Netzmodell Ebenen und Ebenenzeit, ab
                //   einer künftigen Epoche.
                Anweisung::ZusageEinreichen { zusage } => {
                    let _ = myl_ledger::transitions::zusage_einreichen(zustand, &absender, zusage);
                }
                // ⚑ **Die Fristmeldung** (6.8, 2026-10-22): Pod, Mitglieder
                //   und Bereich des Gemeldeten leitet diese Stelle aus
                //   Zuteilung und Schnitt der laufenden Epoche ab, wie beim
                //   Bündel; der Ledger prüft Zeugnisse und Frist.
                Anweisung::FristverstossEinreichen { meldung } => {
                    let zuteilung = zuteilung.get_or_insert_with(|| {
                        Self::zuteilung_der_laufenden_epoche(zustand)
                    });
                    let Some(pod) = zuteilung.pods.iter().find(|p| p.pod_index == meldung.pod) else {
                        continue;
                    };
                    let Some(stelle) = pod.shards.iter().position(|s| s.miner.miner_id == meldung.gemeldeter) else {
                        continue;
                    };
                    let Some(grenzen) = Self::grenzen_des_pods(zustand, pod) else {
                        continue;
                    };
                    let kontext = myl_ledger::transitions::Fristkontext {
                        mitglieder: pod.mitglieder().map(|m| m.miner_id).collect(),
                        ebenen: grenzen[stelle].1 - grenzen[stelle].0,
                        mit_kopf: stelle + 1 == pod.shards.len(),
                    };
                    let _ = myl_ledger::transitions::fristverstoss_einreichen(zustand, &absender, meldung, &kontext);
                }
                // ⚑ **Der Einsatz** (Punkt B11, Fund 145). Bis zum
                // 2026-09-03 gab es keinen Weg, MYL zu hinterlegen;
                // `staked` war im Betrieb immer null, und die ganze
                // wirtschaftliche Sicherheitsschicht hing an einer
                // Zahl, die niemand setzte.
                Anweisung::EinsatzHinterlegen { betrag } => {
                    let _ = einsatz_hinterlegen(zustand, &absender, *betrag);
                }
                // ⚑ **Der Archivbeleg** (Stufe A5): Stichproben, Blöcke und
                //   Beweise prüft der Ledger gegen die Wurzel der Grenze.
                Anweisung::ArchivBelegen { bloecke, beweise } => {
                    let _ = myl_ledger::archiv::archiv_belegen(zustand, &absender, bloecke, beweise);
                }
                // ⚑ **Splitter** (Stufe A7): Kopien nennen die Teilwurzel,
                //   Splitter belegen ihre Teile dagegen.
                Anweisung::ArchivTeilwurzel { wurzel } => {
                    let _ = myl_ledger::archiv::teilwurzel_nennen(zustand, &absender, *wurzel);
                }
                Anweisung::ArchivTeilBelegen { stueck, beweis } => {
                    let _ = myl_ledger::archiv::teil_belegen(zustand, &absender, stueck, beweis);
                }
                Anweisung::EinsatzKuendigen { betrag } => {
                    let _ = einsatz_kuendigen(zustand, &absender, *betrag);
                }
                // ⚑ **Ein Doppelbeleg** (2026-10-27, CONSENSUS 7.6).
                // ⚑ **Festlegen und Aufdecken** (Stufe 2, 2026-10-29): Der
                //   Absender ist die Kennung des Checkers; Auftrag, Fristen
                //   und Festlegung prüft der Ledger.
                Anweisung::PruefungFestlegen { festlegungen } if festlegungen.len() <= myl_consensus::block::PRUEFUNGEN_JE_TX => {
                    let hoehe = zustand.historie.anzahl();
                    for (epoche, pod, segment, festlegung) in festlegungen {
                        let _ = myl_ledger::pruefung::festlegen(zustand, &absender, *epoche, pod, *segment, *festlegung, hoehe);
                    }
                }
                Anweisung::PruefungAufdecken { aufdeckungen } if aufdeckungen.len() <= myl_consensus::block::PRUEFUNGEN_JE_TX => {
                    let hoehe = zustand.historie.anzahl();
                    for (epoche, pod, segment, befund, salz) in aufdeckungen {
                        let _ = myl_ledger::pruefung::aufdecken(
                            zustand,
                            &absender,
                            *epoche,
                            pod,
                            *segment,
                            befund.clone(),
                            salz,
                            hoehe,
                        );
                    }
                }
                // Zu viele auf einmal: als Ganzes abgewiesen.
                Anweisung::PruefungFestlegen { .. } | Anweisung::PruefungAufdecken { .. } => {}
                Anweisung::DifferenzMelden { epoche, pod_a, pod_b, beweis } => {
                    let hoehe = zustand.historie.anzahl();
                    let _ = myl_ledger::pruefung::differenz_melden(
                        zustand,
                        *epoche,
                        pod_a,
                        pod_b,
                        beweis,
                        hoehe,
                        hoehe + Self::PRUEF_FENSTER_BLOECKE,
                        hoehe + 2 * Self::PRUEF_FENSTER_BLOECKE,
                    );
                }
                Anweisung::SchiedsstimmeAbgeben { epoche, pod, segment, pod_falsch } => {
                    let hoehe = zustand.historie.anzahl();
                    let berechtigt = Self::finalitaetssatz_von(zustand, stimmsatz, zustand.epoch.0)
                        .is_some_and(|v| v.contains(&myl_types::ids::MinerId::new(*absender.as_bytes())));
                    let _ = myl_ledger::pruefung::schiedsstimme(
                        zustand,
                        &absender,
                        *epoche,
                        pod,
                        *segment,
                        *pod_falsch,
                        hoehe,
                        berechtigt,
                    );
                }
                Anweisung::DoppelbelegEinreichen { beleg } => {
                    let _ = Self::doppelbeleg_buchen(zustand, &absender, beleg);
                }
                Anweisung::EinsatzAbholen => {
                    let _ = einsatz_abholen(zustand, &absender);
                }
                Anweisung::PruefpunktBezeugen { hoehe, block_hash } => {
                    Self::pruefpunkt_bezeugen(zustand, &absender, *hoehe, block_hash, epoch, stimmsatz);
                }
                // ⚑ **Die Saat** (2026-11-03): Austeilen, Beschweren und
                //   Offenlegen nur fuer die Runde der naechsten Epoche, im
                //   Drittel der laufenden; Anteile fuer `β` der laufenden oder
                //   der vorigen ([`myl_ledger::saat`]).
                Anweisung::SaatschluesselEintragen { schluessel } => {
                    let wer = myl_types::ids::MinerId::new(*absender.as_bytes());
                    if zustand.miner.contains_key(&wer) && schluessel.pruefen() {
                        zustand.verschluesselung.insert(wer, *schluessel);
                    }
                }
                Anweisung::SaatAusteilen { epoche: e, austeilung } => {
                    let wer = myl_types::ids::MinerId::new(*absender.as_bytes());
                    if *e == epoch + 1 {
                        let p = Self::saatphase(hoehe);
                        let _ = myl_ledger::saat::austeilung_aufnehmen(zustand, netz.as_bytes(), *e, &wer, (**austeilung).clone(), p);
                    }
                }
                Anweisung::SaatBeschweren { epoche: e, austeiler, nummer } => {
                    let wer = myl_types::ids::MinerId::new(*absender.as_bytes());
                    if *e == epoch + 1 {
                        let _ = myl_ledger::saat::beschwerde_aufnehmen(zustand, *e, &wer, austeiler, *nummer, Self::saatphase(hoehe));
                    }
                }
                Anweisung::SaatOffenlegen { epoche: e, nummer, wert } => {
                    let wer = myl_types::ids::MinerId::new(*absender.as_bytes());
                    if *e == epoch + 1 {
                        let _ = myl_ledger::saat::offenlegung_aufnehmen(zustand, *e, &wer, *nummer, *wert, Self::saatphase(hoehe));
                    }
                }
                Anweisung::SaatAnteile { epoche: e, teile } => {
                    let wer = myl_types::ids::MinerId::new(*absender.as_bytes());
                    if let Ok(Some(beta)) = myl_ledger::saat::anteile_aufnehmen(zustand, *e, &wer, teile, epoch) {
                        Self::beta_verwenden(zustand, *e, &beta, epoch, gezogen);
                    }
                }
                Anweisung::BuendelEinreichen { buendel } => {
                    // ⚑ **Die Besetzung wird hier nachgeschlagen und
                    // nicht geglaubt** (Fund 144). Der Übergang prüft
                    // Koordinator und Aggregatsignatur, aber er kann
                    // die Zuteilung nicht ableiten: Der Ledger kennt
                    // den Scheduler nicht, und er soll ihn auch nicht
                    // kennen. Diese Stelle sieht beides.
                    //
                    // **Einmal je Block, nicht je Bündel.** Die
                    // Ableitung mischt das ganze Register; sie für
                    // jedes Bündel zu wiederholen wäre eine Einladung,
                    // einen Block mit Bündeln zu füllen.
                    let zuteilung = zuteilung.get_or_insert_with(|| {
                        Self::zuteilung_der_laufenden_epoche(zustand)
                    });
                    let epoche_jetzt = zustand.epoch.0;
                    // Kennt die Zuteilung diesen Pod nicht, ist das
                    // Bündel erfunden. **Übersprungen, nicht
                    // abgebrochen**, wie jede gescheiterte Anweisung.
                    let Some(pod) = myl_scheduler::zonenzuteilung::pod_zu_kennung(
                        zuteilung,
                        epoche_jetzt,
                        &buendel.pod,
                    ) else {
                        continue;
                    };
                    let Some(mitglieder) = Self::unterzeichner_des_buendels(pod, &buendel.vertreter) else {
                        continue;
                    };
                    // Einreichen darf die erste Position, oder wer sie vertritt.
                    let Some((koordinator, _)) = mitglieder.first().copied() else {
                        continue;
                    };
                    let _ = buendel_einreichen(zustand, &absender, buendel.clone(), &koordinator, &mitglieder);
                }
                Anweisung::TrainingssegmentEinreichen { pod: kennung, segment } => {
                    // ⚑ **Dieselbe Arbeitsteilung wie beim Bündel**
                    // (Fund 144): Der Ledger kennt den Scheduler nicht
                    // und soll ihn nicht kennen; diese Stelle sieht
                    // beides und legt vor, was der Übergang prüft.
                    let zuteilung = zuteilung.get_or_insert_with(|| {
                        Self::zuteilung_der_laufenden_epoche(zustand)
                    });
                    let epoche_jetzt = zustand.epoch.0;
                    let Some(pod) = myl_scheduler::zonenzuteilung::pod_zu_kennung(
                        zuteilung,
                        epoche_jetzt,
                        kennung,
                    ) else {
                        continue;
                    };
                    let Some(erster) = pod.shards.first() else {
                        continue;
                    };
                    // ⚑ **Der Plan sagt, ob dieser Pod bestellt war und
                    // woran.** Ohne diese beiden Auskünfte könnte jeder
                    // Pod Arbeit einreichen, die er sich selbst
                    // ausgesucht hat, und die ganze Begründung der
                    // VRF-Zuweisung fiele.
                    //
                    // ⚑ **Der Plan wird hier neu gerechnet und nicht
                    // gespeichert.** Er ist eine reine Funktion des
                    // Zustands; ihn abzulegen führte eine zweite
                    // Wahrheit ein, die mit dem Zustand auseinanderlaufen
                    // könnte.
                    let plan = Self::trainingsplan_der_laufenden_epoche(zustand);
                    let bestellt = plan.pods.contains(&pod.pod_index);
                    let charge = match (zustand.korpus.as_ref(), plan.zuweisungen.get(&pod.pod_index))
                    {
                        (Some(k), Some(z)) => k.charge(z.start, z.laenge),
                        // Ohne Korpus oder ohne Zuweisung gibt es keine
                        // Charge, gegen die zu prüfen wäre. Der Übergang
                        // lehnt dann über `bestellt` oder über die
                        // Charge ab; **eine erfundene Charge wäre
                        // schlimmer als keine.**
                        _ => Hash([0u8; 32]),
                    };
                    let mitglieder: Vec<(myl_types::ids::MinerId, myl_types::bls::BlsPublicKey)> =
                        pod.mitglieder().map(|m| (m.miner_id, m.schluessel)).collect();
                    let vorgabe = myl_ledger::transitions::Segmentvorgabe {
                        charge,
                        modell_version: segment.modell_version,
                        lr_zaehler: Self::TRAININGS_LR_ZAEHLER,
                        // ⚑ **Die Rate folgt dem Budget DIESES Segments.**
                        // Schrittzahl und Folgen stehen darin und sind
                        // signiert; wer sie erhöht, ohne die Rate zu
                        // senken, fällt hier durch.
                        lr_nenner: Self::trainings_lr_nenner(
                            Self::trainingstiefe(zustand),
                            segment.gradienten(),
                        ),
                    };
                    let _ = myl_ledger::transitions::trainingssegment_einreichen(
                        zustand,
                        &absender,
                        *kennung,
                        segment.clone(),
                        bestellt,
                        &erster.miner.miner_id,
                        &mitglieder,
                        &vorgabe,
                    );
                }
                // ⚑ **Der Urteilsweg, und er hat seit heute einen
                // Aufrufer** (Fund 192). `apply_verdict` lag seit jeher
                // fertig da und wurde ausserhalb seiner Tests von nichts
                // erreicht; das Feld `Block::verdicts` daneben trug
                // einen Schuldspruch **ohne Beleg**.
                //
                // Hier kommt der Beleg mit der Anweisung, und der
                // Übergang leitet aus ihm ab, **wen** es trifft. Der
                // Blockerzeuger kann niemanden benennen, dessen
                // Unterschrift er nicht hat.
                // ⛔️ **Fund 679 (2026-10-29): Dieser Weg schlachtete ohne Beweis
                //    der falschen Rechnung.** Sein Beleg ist die Unterschrift
                //    des Beschuldigten, belegt also nur, wen es trifft; dass
                //    falsch gerechnet wurde, setzte er voraus. Wer eine
                //    solche Unterschrift hatte (jeder Mitrechner hat sie),
                //    konnte einen Ehrlichen schlachten lassen. Geschlachtet
                //    wird jetzt nur über eine entschiedene Schiedsrunde
                //    (`SchiedsstimmeAbgeben`, `schiedsurteil_buchen`); die
                //    Anweisung bleibt in der Kodierung und wirkt nicht.
                Anweisung::SchuldspruchEinreichen { .. } => {}
                Anweisung::SitzungWiderrufen { sitzung } => {
                    let _ = sitzung_widerrufen(zustand, sitzung, &absender);
                }
                Anweisung::SitzungAusgeben { vorhaben, vollmacht } => {
                    let _ = sitzung_ausgeben(zustand, &absender, vorhaben, vollmacht.as_ref());
                }
                // ⚑ **Der Weg zum kalten Konto** (Fund 167). Bis zum
                // 2026-09-03 gab es ihn nicht: Die Berechtigungsregel
                // stand seit dem 2026-09-01 fertig im Ledger, und kein
                // Block trug sie. **Ohne Eintrag kein Anteil**, also
                // hätte ein echtes Netz niemanden bezahlt, ohne dass
                // irgendwo ein Fehler entstanden wäre.
                //
                // ⚑ **Die Kennung kommt aus der Anweisung, der
                // Unterzeichner aus der geprüften Transaktion.** Wer
                // eine fremde Kennung nennt, kommt an der Regel im
                // Übergang nicht vorbei: erste Eintragung nur durch den
                // Miner selbst, jede weitere nur durch das eingetragene
                // Konto.
                Anweisung::AuszahlungskontoEintragen { kennung, konto } => {
                    let _ = myl_ledger::transitions::auszahlungskonto_eintragen(
                        zustand, &absender, kennung, *konto,
                    );
                }
            }
        }
        zustand.grundpreis_je_byte = naechster_grundpreis(zustand.grundpreis_je_byte, bytes_im_block, gebuehren.boden_je_byte);
        // ⚑ **Leere Konten fallen weg** (Skalierung, 2026-10-29), nur unter
        //   den in diesem Block geänderten; bei Erzeuger und Folger gleich.
        zustand.leere_konten_entfernen();
        verbraucht
    }

    /// ⚑ **Die Gebühren der Epoche werden geteilt** (2026-10-27, Festlegung
    /// des Projektinhabers): drei Dreizehntel an die Treasury, zehn in den
    /// Topf der Validatoren. Der Topf bleibt stehen, bis es eine
    /// Gewichtsquelle für Validatoren gibt; der Rest einer Teilung fällt
    /// dem Topf zu, damit keine Einheit verschwindet.
    fn gebuehren_teilen(zustand: &mut LedgerState) {
        let summe = std::mem::take(&mut zustand.gebuehren_epoche);
        let treasury = (summe as u128 * GEBUEHR_TREASURY as u128 / GEBUEHR_TEILE as u128) as u64;
        let t = zustand.account_mut(&myl_types::treasury::treasury_adresse());
        t.balance = t.balance.saturating_add(treasury);
        zustand.gebuehren_validatoren = zustand.gebuehren_validatoren.saturating_add(summe - treasury);
    }

    /// ⚑ **Der Topf der Validatoren wird ausgezahlt** (2026-10-28,
    /// Festlegung des Projektinhabers): an die Mitglieder des Stimmsatzes,
    /// gewichtet mit Stake mal Zahl ihrer Unterschriften in den
    /// Prüfpunktzertifikaten der Epoche, an die Adresse ihres Schlüssels. Wer
    /// nicht stimmt, bekommt nichts; der Rest der Teilung bleibt im Topf, und
    /// ohne Stimmen bleibt der ganze Topf stehen. Bis das Stimmgewicht aus
    /// Stake und Arbeit (Whitepaper 3.5.2) entschieden ist.
    fn topf_auszahlen(zustand: &mut LedgerState, stimmsatz: Option<&myl_consensus::validator::VotingSet>) {
        let stimmen = zustand.pruefpunkt_stimmen.nehmen();
        let Some(satz) = stimmsatz else {
            return;
        };
        let gewichte: Vec<(Address, u128)> = stimmen
            .iter()
            .filter_map(|(id, n)| {
                let pk = satz.pubkey(id)?;
                Some((Address::aus_schluessel(pk), satz.weight(id) as u128 * *n as u128))
            })
            .filter(|(_, g)| *g > 0)
            .collect();
        let summe: u128 = gewichte.iter().map(|(_, g)| g).sum();
        if summe == 0 {
            return;
        }
        let topf = zustand.gebuehren_validatoren as u128;
        let mut gezahlt: u128 = 0;
        for (an, g) in gewichte {
            let anteil = topf * g / summe;
            gezahlt += anteil;
            let k = zustand.account_mut(&an);
            k.balance = k.balance.saturating_add(anteil as u64);
        }
        zustand.gebuehren_validatoren -= gezahlt as u64;
    }

    /// Bucht `betrag` vom Konto `von` an die Treasury (G1: Aufschlag für
    /// ein neues Konto). Gedeckt ist er, das prüft der Aufrufer.
    fn an_die_treasury(zustand: &mut LedgerState, von: &Address, betrag: u64) {
        zustand.account_mut(von).balance -= betrag;
        let t = zustand.account_mut(&myl_types::treasury::treasury_adresse());
        t.balance = t.balance.saturating_add(betrag);
    }

    /// Hinterlegt `betrag` vom Konto `von` unter (Art, Kennung) (G1).
    /// Gedeckt ist er, das prüft der Aufrufer.
    fn hinterlegen(zustand: &mut LedgerState, von: &Address, art: u8, kennung: [u8; 32], betrag: u64) {
        zustand.account_mut(von).balance -= betrag;
        let eintrag = zustand.hinterlegungen.entry((art, kennung)).or_insert((*von, 0));
        eintrag.1 = eintrag.1.saturating_add(betrag);
    }

    /// Gibt die Hinterlegung unter (Art, Kennung) an den zurück, der sie
    /// hinterlegt hat (G1). Ohne Eintrag geschieht nichts: Was vor den
    /// Gebühren angemeldet wurde, hat nichts hinterlegt.
    fn zurueckgeben(zustand: &mut LedgerState, art: u8, kennung: [u8; 32]) {
        if let Some((wer, betrag)) = zustand.hinterlegungen.remove(&(art, kennung)) {
            let konto = zustand.account_mut(&wer);
            konto.balance = konto.balance.saturating_add(betrag);
        }
    }

    /// ⚑ **Macht den Knoten zustandslos** (zustandslose Validierung,
    /// 2026-10-29): Die grossen Sammlungen fallen weg, vom grossen Baum
    /// bleibt die Wurzel. Ab hier übernimmt er Blöcke nur mit Zeugen.
    /// `eigene`: die Konten, die der Knoten weiter kennen muss (Netz-,
    /// Miner- und Auszahlungskonto).
    pub fn zustandslos_machen(&mut self, eigene: &[Address]) {
        self.eigene_konten = self.zustand.konten_still(eigene).into_iter().collect();
        self.zustand = self.zustand.zustandslos();
        self.zustandslos = true;
    }

    /// ⚑ **Ein Konto, wie der Knoten es kennt**: beim vollen aus dem
    /// Zustand; beim zustandslosen die eigenen aus `eigene_konten`, jedes
    /// andere als unbekannt (das leere Konto der laufenden Epoche).
    pub fn konto(&self, adresse: &Address) -> myl_ledger::AccountState {
        match self.eigene_konten.get(adresse) {
            Some(Some(k)) if self.zustandslos && !self.eigene_unbekannt.contains(adresse) => k.clone(),
            _ => self.zustand.account(adresse),
        }
    }

    /// Die Nummer, unter der eine Transaktion von `adresse` nicht mehr
    /// gilt; beim zustandslosen Knoten für fremde Konten unbekannt (null):
    /// Das beurteilen die vollen Knoten.
    fn nonce_boden(&self, adresse: &Address) -> u64 {
        if self.zustandslos && !self.eigene_konten.contains_key(adresse) {
            return 0;
        }
        self.konto(adresse).nonce
    }

    /// Ob der Knoten zustandslos ist.
    pub fn ist_zustandslos(&self) -> bool {
        self.zustandslos
    }

    /// Der bereitgehaltene Zeuge eines selbst erzeugten Blocks (Borsh).
    pub fn eigener_zeuge(&self, hash: &Hash) -> Option<std::sync::Arc<Vec<u8>>> {
        self.eigene_zeugen.iter().rev().find(|(h, _, _)| h == hash).map(|(_, _, z)| z.clone())
    }

    /// Selbst erzeugte Blöcke ohne Zeugen (siehe Feld).
    pub fn zeugenlos(&self) -> u64 {
        self.zeugenlos
    }

    /// ⚑ **Die unterschriebenen Stücke des Zeugen** eines selbst erzeugten
    /// Blocks, für das Gossip und für Nachforderungen; einmal
    /// unterschrieben und gehalten. `None` ohne Erzeugerschlüssel oder
    /// ohne Zeugen.
    pub fn zeugenstuecke(&mut self, hash: &Hash) -> Option<std::sync::Arc<Vec<myl_consensus::zeuge::Zeugenstueck>>> {
        if let Some((_, s)) = self.zeugenstuecke.iter().find(|(h, _)| h == hash) {
            return Some(s.clone());
        }
        let (_, hoehe, roh) = self.eigene_zeugen.iter().rev().find(|(h, _, _)| h == hash)?.clone();
        let k = self.erzeugerschluessel.as_ref().or(self.zeugenschluessel.as_ref())?;
        let stuecke = myl_consensus::zeuge::zerlegen(hoehe, *hash, &roh, k.oeffentlich(), |m| k.signiere(m).ok())?;
        let stuecke = std::sync::Arc::new(stuecke);
        self.zeugenstuecke.push_back((*hash, stuecke.clone()));
        if let Some(a) = self.zeugenablage.as_mut() {
            if a.ablegen(hoehe, *hash, &stuecke).is_err() {
                self.zeugenablage_fehler += 1;
            }
        }
        let mut summe: usize = self.zeugenstuecke.iter().map(|(_, s)| s.iter().map(|t| t.bytes.len()).sum::<usize>()).sum();
        while summe > ZEUGEN_HOECHSTENS_BYTES && self.zeugenstuecke.len() > 1 {
            if let Some((_, s)) = self.zeugenstuecke.pop_front() {
                summe -= s.iter().map(|t| t.bytes.len()).sum::<usize>();
            }
        }
        Some(stuecke)
    }

    /// Die schon unterschriebenen Stücke zu `hash`, ohne zu unterschreiben
    /// (für Nachforderungen).
    pub fn zeugenstuecke_gehalten(&self, hash: &Hash) -> Option<std::sync::Arc<Vec<myl_consensus::zeuge::Zeugenstueck>>> {
        if let Some((_, s)) = self.zeugenstuecke.iter().find(|(h, _)| h == hash) {
            return Some(s.clone());
        }
        // Älteres von der Platte, im Fenster ab dem Prüfpunkt.
        self.zeugenablage.as_ref()?.lesen(hash).map(std::sync::Arc::new)
    }

    /// ⚑ **Die Zeugen auch auf der Platte halten** (2026-10-30), über
    /// [`crate::zeugenablage::ZEUGEN_VORHALT_BLOECKE`]: für Erzeuger und
    /// Zeugenhalter, damit ein zustandsloser Knoten nach einem leichten
    /// Abzug am Prüfpunkt die Zeugen ab dort bekommt. Leert `ordner`.
    pub fn zeugen_auf_platte_setzen(&mut self, ordner: std::path::PathBuf) -> std::io::Result<()> {
        self.zeugenablage = Some(crate::zeugenablage::Zeugenablage::neu(ordner)?);
        Ok(())
    }

    fn zertifikat_merken(&mut self, block: &Block) {
        if let Some(z) = &block.header.pruefpunkt {
            self.zertifikate.push_back(z.clone());
            // ⚑ Einer mehr als die leichten Abzuege: Der Anker des aeltesten
            //   ist das Zertifikat danach.
            while self.zertifikate.len() > crate::abzug::LEICHTE_BEHALTEN + 1 {
                self.zertifikate.pop_front();
            }
            if z.schliesst_ab() {
                let quelle = z.quelle;
                let vorher = self.zertifikate.iter().find(|v| v.hoehe == quelle.hoehe && v.block_hash == quelle.block_hash).cloned();
                if let Some(v) = vorher {
                    self.beleg_merken(v, z.clone());
                }
            }
            self.schranke_nachfuehren();
        }
    }

    /// Rückt die Schranke an den endgültigen Prüfpunkt nach, wenn sein Block
    /// in den Rückwegen oder im Fenster liegt; sonst bleibt die ältere, und
    /// die gilt weiter (nur weniger eng).
    fn schranke_nachfuehren(&mut self) {
        let e = self.zustand.pruefpunkt_bestaetigt;
        if e == 0 || self.schranke.is_some_and(|(h, _)| h >= e) {
            return;
        }
        let slot = self.rueckwege.iter().find(|r| r.hoehe == e).map(|r| r.block.header.slot).or_else(|| self.verlauf.get(&e).map(|b| b.header.slot));
        if let Some(s) = slot {
            self.schranke = Some((e, s));
        }
    }

    /// ⚑ **Die höchste Höhe, die es im Slot `slot_jetzt` geben kann**
    /// (Fund 634, 2026-11-05): Jeder Block verbraucht mindestens einen
    /// Slot, und jeder Zweig, dem die Kette folgen darf, zweigt über dem
    /// endgültigen Prüfpunkt ab. Mit Höhe `h` und Slot `s` eines eigenen
    /// Blocks an oder unter ihm ist also jede ehrliche Höhe höchstens
    /// `h + (slot_jetzt − s)`. Eine Behauptung darüber ist nachweislich
    /// erfunden, auch mit einer gültigen Unterschrift. Ohne endgültigen
    /// Prüfpunkt keine Schranke.
    pub fn hoehen_schranke(&self, slot_jetzt: u64) -> u64 {
        self.schranke.map_or(u64::MAX, |(h, s)| h.saturating_add(slot_jetzt.saturating_sub(s)))
    }

    /// ⚑ **Traegt der eigene endgueltige Pruefpunkt noch?** (Vertrauensfrist,
    /// 2026-10-27, CONSENSUS 7.5.) Die Epoche des Netzes kommt aus der eigenen
    /// Schranke und der Uhr, ⛔️ **nie aus einem empfangenen Block** (A46): Der
    /// waere an dieser Stelle noch ungeprueft, und eine erfundene Epoche hielte
    /// den Knoten an. Ohne endgueltigen Pruefpunkt gibt es nichts zu pruefen.
    pub fn vertrauen_traegt_jetzt(&self, slot_jetzt: u64) -> bool {
        match self.schranke {
            None => true,
            Some(_) => vertrauen_traegt(epoche_fuer_hoehe(self.bestaetigt_bis()), epoche_fuer_hoehe(self.hoehen_schranke(slot_jetzt))),
        }
    }

    /// ⚑ **Der Wechselbeleg einer Epoche** (7.3, 2026-11-04): das erste
    /// endgueltige Zertifikat der Epoche, die beiden Saetze, die es bindet,
    /// aus dem eigenen Zustand, und das Zertifikat, das es abschliesst. Die
    /// Saetze stehen dort noch: Der Abschluss kommt spaetestens in der
    /// Epoche danach an, und der Zustand haelt die Saetze ab der Vorepoche.
    ///
    /// ⚑ **Nach einer Luecke** (2026-11-07): Traegt der Beleg die eigene
    /// Folge nicht (die Bruecke verlangt alle Unterzeichner aus dem Satz vor
    /// der Luecke, [`crate::wechsel`]), ersetzt ihn ein spaeterer derselben
    /// Epoche, der sie traegt. Sonst bekaeme ein zustandsloser Knoten keinen,
    /// obwohl es einen gaebe. Ohne Satz von aussen oder mit einer Folge, die
    /// schon vorher abbricht, bleibt es beim ersten.
    fn beleg_merken(
        &mut self,
        z: myl_consensus::pruefpunkt::Pruefpunktzertifikat,
        abschluss: myl_consensus::pruefpunkt::Pruefpunktzertifikat,
    ) {
        let e = epoche_fuer_hoehe(z.hoehe);
        let gemerkt = self.wechselbelege.contains_key(&e);
        if gemerkt && !self.belege_ohne_folge.contains(&e) {
            return;
        }
        let naechster = myl_ledger::stimmsatz::satz_der_epoche(&self.zustand, e + 1).cloned();
        let uebernaechster = myl_ledger::stimmsatz::satz_der_epoche(&self.zustand, e + 2).cloned();
        if myl_ledger::stimmsatz::bindung_aus(naechster.as_ref(), uebernaechster.as_ref()) != z.bindung {
            return;
        }
        let beleg = crate::wechsel::Wechselbeleg { zertifikat: z, naechster, uebernaechster, abschluss };
        // ⚑ Nur nach einer Luecke (Belege fuer beide Vorepochen fehlen) kann
        //   ein Beleg die eigene Folge nicht tragen; nur dann wird sie neu
        //   aufgebaut. 📌 Bis 0.116.1 geschah das bei jedem Beleg auf jedem
        //   vollen Knoten (Sicherheitsaudit 2026-11-08): Der Aufbau prueft
        //   alle Belege ab Genesis, zwei Aggregate je Epoche, nach einem Jahr
        //   stuendlicher Epochen rund 17 500 und Sekunden in der Schleife.
        let nach_luecke = !self.wechselbelege.contains_key(&e.saturating_sub(1)) && !self.wechselbelege.contains_key(&e.saturating_sub(2));
        let traegt = !nach_luecke || self.folge().is_none_or(|mut f| f.aufnehmen(&beleg).is_ok());
        if gemerkt && !traegt {
            return;
        }
        // Nur die laufende Epoche und die davor koennen noch einen bekommen.
        self.belege_ohne_folge.retain(|x| x + 1 >= e);
        if traegt {
            self.belege_ohne_folge.remove(&e);
        } else {
            self.belege_ohne_folge.insert(e);
        }
        if let Some(a) = &self.belegablage {
            let _ = a.anhaengen(&beleg);
        }
        self.wechselbelege.insert(e, beleg);
        self.belege_kuerzen(e);
    }

    /// ⚑ **Laesst Belege fallen, die niemand mehr braucht** (A44): aelter als
    /// [`crate::wechsel::WECHSELBELEGE_BEHALTEN`] Epochen. Der Beleg des
    /// Ankers von aussen bleibt, die eigene Folge beginnt bei ihm. Die Ablage
    /// wird nur alle 64 gefallenen neu geschrieben.
    fn belege_kuerzen(&mut self, jetzt: u64) {
        let grenze = jetzt.saturating_sub(crate::wechsel::WECHSELBELEGE_BEHALTEN);
        let anker = self.wechselbelege.values().find(|b| self.ist_anker_von_aussen(b)).map(|b| b.epoche());
        let alt: Vec<u64> = self.wechselbelege.range(..grenze).map(|(e, _)| *e).filter(|e| Some(*e) != anker).collect();
        if alt.is_empty() {
            return;
        }
        for e in &alt {
            self.wechselbelege.remove(e);
        }
        self.belege_ab = self.wechselbelege.keys().find(|e| Some(**e) != anker).copied();
        self.belege_gefallen += alt.len() as u64;
        if self.belege_gefallen >= 64 {
            if let Some(a) = &self.belegablage {
                if a.neu_schreiben(self.wechselbelege.values()).is_ok() {
                    self.belege_gefallen = 0;
                }
            }
        }
    }

    /// Die Wechselbelege ab Epoche `ab`, aufsteigend, zusammen hoechstens
    /// `bytes` gross (mindestens einer, wenn es einen gibt).
    pub fn wechselbelege_ab(&self, ab: u64, bytes: usize) -> Vec<crate::wechsel::Wechselbeleg> {
        let mut aus = Vec::new();
        let mut summe = 0usize;
        for b in self.wechselbelege.range(ab..).map(|(_, b)| b) {
            let n = borsh::to_vec(b).map_or(usize::MAX, |v| v.len());
            if !aus.is_empty() && summe.saturating_add(n) > bytes {
                break;
            }
            summe = summe.saturating_add(n);
            aus.push(b.clone());
        }
        aus
    }

    /// ⚑ **Die Wechselbelege ab Epoche `ab`, gefaltet** (A44,
    /// [`crate::belegfolge`]): hoechstens `bytes` gefaltet und nie mehr,
    /// als ein Empfaenger entfalten darf.
    pub fn wechselbelege_kompakt_ab(&self, ab: u64, bytes: usize) -> crate::belegfolge::Belegfolge {
        crate::belegfolge::falten_bis(self.wechselbelege.range(ab..).map(|(_, b)| b), bytes)
    }

    /// Die erste Epoche ohne Wechselbeleg nach dem letzten vorhandenen.
    pub fn wechselbelege_bis(&self) -> u64 {
        // Mit einem Anker von aussen, dessen Beleg noch fehlt, zaehlt nichts
        // davor; nur ab ihm hat eine Gegenstelle die Belege noch sicher.
        if let Some((h, _, _)) = self.anker_von_aussen {
            if !self.wechselbelege.values().any(|b| self.ist_anker_von_aussen(b)) {
                return epoche_fuer_hoehe(h);
            }
        }
        self.wechselbelege.keys().next_back().map_or(0, |e| e + 1)
    }

    /// ⚑ **Die Folge der Saetze ab Genesis** (7.4) aus dem Satz von aussen
    /// und den eigenen Belegen; ohne Satz von aussen keine. ⚑ Liegt der Beleg
    /// des Ankers von aussen vor (7.4a), beginnt sie bei ihm. ⛔️ **Fehlt er
    /// noch, gibt es keine Folge** (A43): Der Rueckfall auf Genesis liesse
    /// eine Gegenstelle am Anker vorbei eine zweite Geschichte vorlegen.
    /// Sind aeltere Belege gekuerzt, beginnt sie beim aeltesten eigenen.
    fn folge(&self) -> Option<crate::wechsel::Folge> {
        let von_aussen = self.stimmsatz.clone()?;
        let mindest = Self::satzregeln(&self.zustand).mindestgewicht;
        let anker = self.wechselbelege.values().find(|b| self.ist_anker_von_aussen(b));
        if self.anker_von_aussen.is_some() && anker.is_none() {
            return None;
        }
        let beginn = anker.or_else(|| self.belege_ab.and_then(|e| self.wechselbelege.get(&e)));
        let (mut f, ab) = match beginn.and_then(|b| crate::wechsel::Folge::ab_beleg(von_aussen.clone(), mindest, b).ok().map(|f| (f, b.epoche() + 1))) {
            Some(x) => x,
            None => (crate::wechsel::Folge::ab_genesis(von_aussen, mindest), 0),
        };
        for b in self.wechselbelege.range(ab..).map(|(_, b)| b) {
            if f.aufnehmen(b).is_err() {
                break;
            }
        }
        Some(f)
    }

    /// ⚑ **Setzt den Anker von aussen** (CONSENSUS 7.4a, `--anker`): Hoehe,
    /// Block und Bindung des Zertifikats eines Wechselbelegs.
    ///
    /// ⛔️ **Die Bindung gehoert dazu** (Sicherheitsaudit 2026-11-08): Die
    /// Unterschriften des Ankers prueft niemand, und gerade sie verbinden die
    /// Bindung (die Saetze der beiden naechsten Epochen) mit dem Block. Mit
    /// Hoehe und Block allein konnte eine Gegenstelle einen Beleg mit
    /// richtigem Block und eigenen Saetzen liefern, und der Knoten haette von
    /// da an einem Satz des Angreifers geglaubt.
    pub fn anker_von_aussen_setzen(&mut self, hoehe: u64, block: Hash, bindung: Hash) {
        self.anker_von_aussen = Some((hoehe, block, bindung));
    }

    fn ist_anker_von_aussen(&self, b: &crate::wechsel::Wechselbeleg) -> bool {
        self.anker_von_aussen == Some((b.zertifikat.hoehe, b.zertifikat.block_hash, b.zertifikat.bindung))
    }

    /// Der juengste eigene Wechselbeleg: Epoche und das, was ein Betreiber
    /// als Anker von aussen nennt (Hoehe, Block und Bindung seines
    /// Zertifikats).
    pub fn juengster_wechselbeleg(&self) -> Option<(u64, u64, Hash, Hash)> {
        self.wechselbelege.iter().next_back().map(|(e, b)| (*e, b.zertifikat.hoehe, b.zertifikat.block_hash, b.zertifikat.bindung))
    }

    /// ⚑ **Der Anker, den dieser Knoten anderen nennen kann** (7.4a, A43):
    /// sein juengster Wechselbeleg samt Slot des Blocks. Ohne den Block im
    /// eigenen Fenster keiner; der Slot gehoert dazu, sonst liesse sich das
    /// Alter des Ankers nicht pruefen.
    pub fn anker_zum_weitergeben(&self) -> Option<crate::wechsel::AnkerVonAussen> {
        let (_, hoehe, block, bindung) = self.juengster_wechselbeleg()?;
        let b = self.block_an(hoehe).filter(|b| b.hash() == block)?;
        Some(crate::wechsel::AnkerVonAussen { hoehe, block, bindung, slot: b.header.slot })
    }

    /// Wann das Netz begann (A43); aus der Stimmsatzdatei.
    pub fn netzstart_setzen(&mut self, ms: Option<u64>) {
        self.netzstart_ms = ms;
    }

    /// Wann das Netz begann, falls die Stimmsatzdatei es nennt.
    pub fn netzstart_ms(&self) -> Option<u64> {
        self.netzstart_ms
    }

    /// **Wechselbelege einer Gegenstelle aufnehmen** (7.4): nur, was die
    /// Folge ab Genesis traegt. Gibt zurueck, wie viele neu sind.
    pub fn wechselbelege_aufnehmen(&mut self, belege: &[crate::wechsel::Wechselbeleg]) -> Result<usize, crate::wechsel::Folgefehler> {
        if self.stimmsatz.is_none() {
            return Err(crate::wechsel::Folgefehler::Unbekannt(0));
        }
        let mut folge = self.folge();
        let mut sortiert: Vec<&crate::wechsel::Wechselbeleg> = belege.iter().collect();
        sortiert.sort_by_key(|b| b.zertifikat.hoehe);
        let mut neu = 0usize;
        // ⚑ Mit einem Anker von aussen (7.4a) zaehlt nichts vor seinem Beleg;
        //   liegt er bei, beginnt die Folge bei ihm.
        let gemerkt = self.wechselbelege.values().find(|b| self.ist_anker_von_aussen(b)).map(|b| b.epoche());
        if let Some(i) = sortiert.iter().position(|b| self.ist_anker_von_aussen(b)) {
            let b = sortiert[i];
            if gemerkt != Some(b.epoche()) {
                let von_aussen = self.stimmsatz.clone().ok_or(crate::wechsel::Folgefehler::Unbekannt(0))?;
                folge = Some(crate::wechsel::Folge::ab_beleg(von_aussen, Self::satzregeln(&self.zustand).mindestgewicht, b)?);
                if let Some(a) = &self.belegablage {
                    let _ = a.anhaengen(b);
                }
                self.wechselbelege.insert(b.epoche(), b.clone());
                neu += 1;
            }
            sortiert.drain(..=i);
        } else if let Some(e) = gemerkt {
            sortiert.retain(|b| b.epoche() > e);
        }
        // ⛔️ Ein gesetzter Anker ohne seinen Beleg: nichts aufnehmen (A43).
        let Some(mut f) = folge else {
            return Err(crate::wechsel::Folgefehler::Unbekannt(self.anker_von_aussen.map_or(0, |(h, _, _)| epoche_fuer_hoehe(h))));
        };
        for b in sortiert {
            f.aufnehmen(b)?;
            if !self.wechselbelege.contains_key(&b.epoche()) {
                if let Some(a) = &self.belegablage {
                    let _ = a.anhaengen(b);
                }
                self.wechselbelege.insert(b.epoche(), b.clone());
                neu += 1;
            }
        }
        Ok(neu)
    }

    /// ⚑ **Der Anker eines leichten Abzugs, den dieser Knoten liefern kann**:
    /// das Zertifikat, das den jüngsten Prüfpunkt mit leichtem Abzug
    /// endgültig macht (seit 2026-11-04 das des nächsten Prüfpunkts, das ihn
    /// als Quelle nennt). Der Fragende prüft es gegen seinen Stimmsatz.
    pub fn leichter_anker(&self) -> Option<myl_consensus::pruefpunkt::Pruefpunktzertifikat> {
        let hoehen = self.ablage.as_ref()?.leicht().hoehen();
        self.zertifikate.iter().rev().find(|z| z.schliesst_ab() && hoehen.contains(&z.quelle.hoehe)).cloned()
    }

    /// ⚑ **Der Anker für einen vollen Einsteiger** (Z7, 2026-11-05): ein
    /// Zertifikat, das eine Epochengrenze endgültig macht, an der ein eigener
    /// voller Abzug liegt; aus den jüngsten Zertifikaten, sonst aus den
    /// Abschlüssen der Wechselbelege (je Epoche einer, so lange gehalten wie
    /// die Belege). Der Einsteiger prüft es über die Folge der Wechselbelege
    /// ab Genesis wie der zustandslose; ein Anker von aussen ist dann nicht
    /// mehr nötig.
    pub fn voller_anker(&self) -> Option<myl_consensus::pruefpunkt::Pruefpunktzertifikat> {
        let hoehen = self.ablage.as_ref()?.hoehen();
        let traegt = |z: &&myl_consensus::pruefpunkt::Pruefpunktzertifikat| {
            z.schliesst_ab() && crate::abzug::ist_grenze(z.quelle.hoehe) && hoehen.contains(&z.quelle.hoehe)
        };
        self.zertifikate
            .iter()
            .rev()
            .find(traegt)
            .or_else(|| self.wechselbelege.values().rev().map(|b| &b.abschluss).find(traegt))
            .cloned()
    }

    /// ⚑ **Ein Anker aus einem Zertifikat**, wenn es gegen den Satz seiner
    /// Epoche trägt (mehr als zwei Drittel, Prüfpunkthöhe) und seine Quelle
    /// endgültig macht; der Anker ist die Quelle. Den Satz liefert
    /// die Folge der Wechselbelege ab Genesis (7.4, 2026-11-04); fehlen
    /// Belege, sagt der Fehler, ab welcher Epoche ([`crate::wechsel::Folgefehler::Unbekannt`]).
    /// Ohne Satz von aussen keiner: Eine Gegenstelle kann sonst jeden Anker
    /// nennen.
    pub fn anker_aus_zertifikat(
        &self,
        z: &myl_consensus::pruefpunkt::Pruefpunktzertifikat,
    ) -> Result<crate::abzug::Anker, crate::wechsel::Folgefehler> {
        let f = self.folge().ok_or(crate::wechsel::Folgefehler::Unbekannt(0))?;
        let q = f.endgueltig_durch(z)?;
        Ok(crate::abzug::Anker { hoehe: q.hoehe, blockhash: q.block_hash })
    }

    /// ⚑ **Eine Seite des Verlaufs einer Adresse** (CLIENT 7.4): aus den
    /// gehaltenen Blöcken, neueste zuerst, höchstens
    /// [`VERLAUF_JE_SEITE`] Einträge und höchstens `frist` Lesezeit. Nur was
    /// ein Block trägt; was vor der Verdichtung lag, kennt dieser Knoten
    /// nicht mehr.
    pub fn verlauf_seite(&mut self, adresse: &Address, bis: Option<u64>, frist: std::time::Duration) -> crate::beobachtung::Verlaufsseite {
        use crate::beobachtung::{Verlaufseintrag, Verlaufsseite};
        let start = std::time::Instant::now();
        let mut hoehe = bis.map_or(self.hoehe, |b| b.min(self.hoehe));
        let mut seite = Verlaufsseite::default();
        while hoehe > 0 {
            let ab = hoehe.saturating_sub(VERLAUF_BLOECKE_JE_SCHUB - 1).max(1);
            let bloecke = self.bloecke_von_bis(ab, hoehe);
            if bloecke.is_empty() {
                return seite;
            }
            for b in bloecke.iter().rev() {
                for tx in b.txs.iter().rev() {
                    let ausgehend = tx.absender_adresse() == *adresse;
                    let (art, gegen, betrag) = match &tx.anweisung {
                        Anweisung::Burn { betrag } => ("burn", None, *betrag),
                        Anweisung::Ueberweisung { nach, betrag } => ("ueberweisung", Some(*nach), *betrag),
                        Anweisung::SitzungEroeffnen { .. } | Anweisung::SitzungWiderrufen { .. } | Anweisung::SitzungAusgeben { .. } => ("sitzung", None, 0),
                        _ => ("anderes", None, 0),
                    };
                    let eingehend = !ausgehend && gegen == Some(*adresse);
                    if ausgehend || eingehend {
                        let gegen = if ausgehend { gegen } else { Some(tx.absender_adresse()) };
                        seite.eintraege.push(Verlaufseintrag { hoehe: b.header.height, art, ausgehend, gegen, betrag });
                    }
                }
            }
            hoehe = ab - 1;
            if hoehe > 0 && (seite.eintraege.len() >= VERLAUF_JE_SEITE || start.elapsed() >= frist) {
                seite.weiter_bei = Some(hoehe);
                return seite;
            }
        }
        seite
    }

    /// Kontobeweise auf dem jetzigen Stand (Höhe dazu); nur ein voller Knoten.
    pub fn kontobeweise(&mut self, adressen: &[Address]) -> Option<(u64, Vec<myl_ledger::state::Kontobeweis>)> {
        if self.zustandslos {
            return None;
        }
        Some((self.hoehe, self.zustand.kontobeweise(adressen)?))
    }

    /// Ob der Knoten den Stand dieses Kontos kennt (beim vollen immer).
    pub fn konto_bekannt(&self, adresse: &Address) -> bool {
        !self.eigene_unbekannt.contains(adresse)
    }

    /// Die eigenen Konten, die der zustandslose Knoten nicht kennt.
    pub fn eigene_unbekannt(&self) -> Vec<Address> {
        self.eigene_unbekannt.iter().copied().collect()
    }

    /// Nach einem leichten Abzug: Der Stand der eigenen Konten ist
    /// unbekannt, bis ein Zeuge oder ein Kontobeweis ihn bringt.
    pub fn eigene_vergessen(&mut self) {
        self.eigene_unbekannt = self.eigene_konten.keys().copied().collect();
    }

    /// ⚑ **Setzt Kontobeweise ein**, wenn sie zur eigenen Höhe gehören und
    /// gegen die eigene Wurzel des grossen Baums tragen; nur für eigene,
    /// unbekannte Konten. Die Zahl der eingesetzten; `Err`, wenn die Höhe
    /// nicht passt (`Ok(0)` heisst: getragen hat keiner).
    pub fn kontobeweise_einsetzen(&mut self, hoehe: u64, beweise: &[myl_ledger::state::Kontobeweis]) -> Result<usize, u64> {
        if hoehe != self.hoehe {
            return Err(self.hoehe);
        }
        let mut n = 0;
        for b in beweise {
            if self.eigene_unbekannt.contains(&b.adresse) && self.zustand.kontobeweis_traegt(b) {
                self.eigene_konten.insert(b.adresse, b.konto.clone());
                self.eigene_unbekannt.remove(&b.adresse);
                n += 1;
            }
        }
        Ok(n)
    }

    /// ⚑ **Zeugen bereithalten** als voller Knoten: aufzeichnen beim
    /// Übernehmen, unterschreiben mit `schluessel`.
    pub fn zeugen_bereithalten_setzen(&mut self, schluessel: crate::schluessel::Konsensschluessel) {
        self.zeugenschluessel = Some(schluessel);
    }

    /// ⚑ **Ob der Erzeuger eines Blocks belegt ist** (Unterschrift eines
    /// Berechtigten), ohne ihn anzuwenden: für den zustandslosen Knoten,
    /// bevor ein Block auf seinen Zeugen warten darf.
    pub fn erzeuger_belegt(&self, block: &Block) -> bool {
        self.erzeuger_traegt(block)
    }

    fn zeuge_bereithalten(&mut self, hash: Hash, hoehe: u64, zeuge: Vec<u8>) {
        // Roh nur, bis der Knoten ihn zerlegt und unterschrieben hat
        // (`zeugenstuecke`); bereitgehalten werden die Stücke.
        while self.eigene_zeugen.len() >= ZEUGEN_ROH_GEHALTEN {
            self.eigene_zeugen.pop_front();
        }
        self.eigene_zeugen.push_back((hash, hoehe, std::sync::Arc::new(zeuge)));
        let mut summe: usize = self.eigene_zeugen.iter().map(|(_, _, z)| z.len()).sum();
        while summe > ZEUGEN_HOECHSTENS_BYTES && self.eigene_zeugen.len() > 1 {
            if let Some((_, _, z)) = self.eigene_zeugen.pop_front() {
                summe -= z.len();
            }
        }
    }

    /// Baut den nächsten Block aus dem Mempool und übernimmt ihn selbst.
    ///
    /// Der Erzeuger wendet **zuerst** an und schreibt die daraus
    /// entstandene Zustandswurzel in den Block. Die Empfänger rechnen
    /// dasselbe und vergleichen.
    ///
    /// Im ersten Slot nach dem des Kopfs, in dem der eigene Schlüssel
    /// gezogen ist (bis [`SLOTS_GESUCHT`] weiter), sonst im nächsten; für
    /// Proben ohne Uhr. Der Knoten baut im Slot seiner Uhr
    /// ([`Self::baue_block_im_slot`]).
    pub fn baue_block(&mut self) -> Block {
        let naechster = self.zustand.letzter_slot.saturating_add(1);
        let slot = self.eigener_slot(naechster, naechster.saturating_add(SLOTS_GESUCHT)).unwrap_or(naechster);
        self.baue_block_im_slot(slot)
    }

    /// Baut den nächsten Block im Slot `slot` (2026-11-04, Erzeugerplan);
    /// trägt nur, wenn der eigene Schlüssel dafür gezogen ist und der Slot
    /// nach dem des Kopfs liegt.
    pub fn baue_block_im_slot(&mut self, slot: u64) -> Block {
        // ⚑ **Nur so viel, wie in einen Block passt** (Fund 633), in der
        //   Folge des Mempools. Was gerade nicht mehr passt, bleibt
        //   unangewandt liegen; was allein schon grösser ist als ein Block,
        //   kann nie hinein und fällt heraus, sonst stünde es für immer vorn.
        let mut wartend = Vec::new();
        let mut spaeter = Vec::new();
        let mut bytes = 0usize;
        let mut neulinge = 0usize;
        // ⚑ Höchstens das Doppelte des Ziels in Gebühreneinheiten (2026-10-28).
        let mut einheiten = 0u64;
        for w in self.mempool.alle_nehmen() {
            let n = w.bytes;
            if n > BLOCK_TXS_HOECHSTENS_BYTES {
                continue;
            }
            let e = (n as u64).saturating_add(myl_consensus::block::PRUEFEINHEIT);
            // Neulinge nur bis zu ihrer Grenze je Block (Fund 635).
            let neuling = w.neuling && ist_neuling(&self.zustand, &w.absender);
            if bytes + n <= BLOCK_TXS_HOECHSTENS_BYTES
                && einheiten + e <= BLOCK_EINHEITEN_HOECHSTENS
                && !(neuling && neulinge >= BLOCK_NEULINGE_HOECHSTENS)
            {
                bytes += n;
                einheiten += e;
                neulinge += usize::from(neuling);
                wartend.push(w);
            } else {
                spaeter.push(w);
            }
        }
        let txs: Vec<Transaktion> = wartend.iter().map(|w| w.tx.clone()).collect();
        let hoehe = self.hoehe + 1;
        let epoch = epoche_fuer_hoehe(hoehe);

        // ⚑ **Das angebotene Zertifikat eines Prüfpunkts**, wenn es für
        //   diesen Block trägt (2026-10-27, CONSENSUS 7.2); geprüft vor dem
        //   Anwenden, das den bestätigten Stand fortschreibt.
        let zert = self.naechstes_pruefpunktzertifikat.take();
        let zert = zert.filter(|z| self.zertifikat_traegt(z, hoehe));
        let mut gezogen = None;
        let ablaufend = self.ablaufende_zuteilung(epoch);
        let mut neben = ablaufend.is_some().then(|| self.nebenstand());
        self.zuteilung_aufheben(ablaufend);
        let mut ausgaben = Pruefausgaben::default();
        // ⚑ **Der Zeuge des Blocks** (zustandslose Validierung, 2026-10-29):
        //   Aufgezeichnet wird, was das Anwenden an grossen Sammlungen
        //   berührt; daraus der Zeuge, bevor die Wurzel ihn fortschreibt.
        debug_assert!(!self.zustandslos, "ein zustandsloser Knoten erzeugt keine Blöcke");
        self.zustand.aufzeichnen_beginnen();
        // ⚑ Auch ein eigener Block lässt sich zurücknehmen (2026-11-04).
        let sicherung = self.zustand.sichern();
        let davor = self.letzter_hash;
        let unterschriften = &self.unterschriften;
        let verbraucht = Self::anwenden(
            &mut self.zustand,
            &txs,
            epoch,
            &self.letzter_hash,
            zert.as_ref(),
            &mut gezogen,
            self.stimmsatz.as_ref(),
            &mut ausgaben,
            &|t: &Transaktion| unterschriften.kennt(&t.kennung()),
        );
        self.zustand.letzter_slot = slot;
        let zeuge = self.zustand.zeuge_nehmen().ok().and_then(|z| borsh::to_vec(&z).ok());
        if neben.is_none() && gezogen.is_some() {
            neben = Some(self.nebenstand());
        }
        self.ziehung_merken(gezogen);
        self.pruefausgaben.anhaengen(ausgaben);
        // ⚑ **In den Block kommt nur, was seine Nummer verbraucht hat**
        //   (Fund 627). Was an Unterschrift oder Nummer scheitert, hat den
        //   Zustand nicht angefasst; die Wurzel ist mit und ohne es dieselbe.
        //   Bis zum 2026-10-24 ging der ganze Mempool in den Block, und eine
        //   Transaktion mit der Nummer n+1, die im Gossip vor n ankam,
        //   scheiterte dort und war verloren: Der Absender zählte weiter und
        //   hing für immer an der Lücke. Jetzt wartet sie, bis n da ist.
        let mut im_block = Vec::new();
        for (w, ok) in wartend.into_iter().zip(verbraucht) {
            if ok {
                im_block.push(w.tx);
            } else {
                self.mempool.hinein(w);
            }
        }
        for w in spaeter {
            self.mempool.hinein(w);
        }
        self.mempool_bereinigen(hoehe);

        let mut block = Block::new(BlockHeader {
            height: hoehe,
            epoch,
            prev_block_hash: self.letzter_hash,
            timestamp_ms: crate::protokoll::jetzt_ms().max(0) as u64,
            state_root: self.zustand.wurzel(),
            pruefpunkt: zert,
            slot,
        });
        for tx in im_block {
            block.add_transaction(tx);
        }
        // Unterschreiben, bevor der Block geschrieben und
        // verbreitet wird; die Unterschrift ändert den Blockhash nicht.
        if let Some(k) = &self.erzeugerschluessel {
            if let Ok(signatur) = k.signiere(&block.erzeugernachricht()) {
                block.unterschrift =
                    Some(myl_consensus::block::Erzeugerunterschrift { schluessel: k.oeffentlich(), signatur });
            }
        }
        self.erzeuger_lernen(&block);
        let weg = self.zustand.sicherung_behalten(sicherung);
        self.rueckweg_merken(Rueckspur { hoehe, davor, block: block.clone(), weg, neben });

        if let Some(b) = self.baum.as_mut() {
            b.anhaengen(&self.letzter_hash);
        }
        self.hoehe = hoehe;
        self.letzter_hash = block.hash();
        self.bekannt.insert(self.letzter_hash);
        match zeuge {
            Some(z) => self.zeuge_bereithalten(self.letzter_hash, hoehe, z),
            None => self.zeugenlos += 1,
        }
        self.verlauf_aufnehmen(hoehe, &block);
        self.schreibe(&block);
        self.zertifikat_merken(&block);
        self.abzug_anlegen(&block);
        block
    }

    /// Was die Kette neben dem Zustand fortschreibt, als Kopie.
    fn nebenstand(&self) -> Nebenstand {
        Nebenstand {
            offene_zuteilungen: self.offene_zuteilungen.clone(),
            letzte_stichprobe: self.letzte_stichprobe.clone(),
            pods_der_stichprobe: self.pods_der_stichprobe.clone(),
            pruefer_der_stichprobe: self.pruefer_der_stichprobe.clone(),
        }
    }

    /// Hebt den Rückweg eines Blocks auf und wirft weg, was unter dem
    /// endgültigen Prüfpunkt liegt oder über [`RUECKWEG_HOECHSTENS`] geht.
    fn rueckweg_merken(&mut self, spur: Rueckspur) {
        self.rueckwege.push_back(spur);
        let endgueltig = self.zustand.pruefpunkt_bestaetigt;
        while self.rueckwege.front().is_some_and(|r| r.hoehe <= endgueltig) || self.rueckwege.len() > RUECKWEG_HOECHSTENS {
            self.rueckwege.pop_front();
        }
    }

    /// ⚑ **Bis wohin diese Kette zurück kann**: nie unter den endgültigen
    /// Prüfpunkt, nie unter ihre Basis, und nur so weit, wie sie Rückwege
    /// hat (nach einem Neustart keine).
    pub fn rueckweg_boden(&self) -> u64 {
        let aeltester = self.rueckwege.front().map_or(self.hoehe, |r| r.hoehe - 1);
        aeltester.max(self.zustand.pruefpunkt_bestaetigt).max(self.basis)
    }

    /// ⚑ **Bis wohin die Wahl des Zweigs zurück darf** (CONSENSUS 7.7a,
    /// 2026-11-05): nie unter den endgültigen Prüfpunkt und nie unter die
    /// Basis. Reichen die Rückwege nicht so weit (eine Trennung über ihre
    /// 600 Blöcke, ein Neustart), baut die Kette den Stand am Gabelpunkt aus
    /// dem eigenen Abzug neu auf ([`Self::neu_aufsetzen_bei`]); ohne
    /// Kettendatei und Ablage, und zustandslos, bleibt es beim Boden der
    /// Rückwege.
    pub fn zweig_boden(&self) -> u64 {
        if self.zustandslos || self.speicher.is_none() || self.ablage.is_none() {
            return self.rueckweg_boden();
        }
        self.zustand.pruefpunkt_bestaetigt.max(self.basis).min(self.rueckweg_boden())
    }

    /// Die eigenen Blöcke `ab` bis `bis` aus dem Fenster oder der
    /// Kettendatei, lückenlos, sonst keine.
    fn eigene_bloecke(&mut self, ab: u64, bis: u64) -> Option<Vec<Block>> {
        let mut bloecke = Vec::new();
        let mut naechste = ab;
        while naechste <= bis {
            let stueck = match self.verlauf.get(&naechste) {
                Some(b) => vec![b.clone()],
                None => self.speicher.as_mut()?.bloecke_ab(naechste, bis.min(naechste + 511)).ok()?,
            };
            if stueck.is_empty() {
                return None;
            }
            for b in stueck {
                if b.header.height != naechste {
                    return None;
                }
                naechste += 1;
                bloecke.push(b);
            }
        }
        Some(bloecke)
    }

    /// ⚑ **Baut den Stand an `gabel` aus dem eigenen Abzug neu auf**
    /// (CONSENSUS 7.7a, 2026-11-05), wenn die Rückwege nicht so tief reichen:
    /// der jüngste eigene volle Abzug an oder unter `gabel`, dann die eigenen
    /// Blöcke bis `gabel` aus der Kettendatei nachgerechnet, wie beim
    /// Wiederanlauf (Zustände an Wiederherstellungspunkten, dazwischen
    /// nachspielen). Was über `gabel`
    /// lag, fällt aus Datei und Ablagen, seine Transaktionen gehen in den
    /// Mempool zurück. Gibt die Zahl der verlassenen Blöcke zurück.
    ///
    /// ⛔️ Nie unter [`Self::zweig_boden`] (`Err` mit ihm, nichts geändert).
    /// Kein neues Dateiformat im Konsenspfad: Abzug und Kettendatei sind
    /// dieselben, aus denen jeder Neustart lebt.
    fn neu_aufsetzen_bei(&mut self, gabel: u64) -> Result<usize, u64> {
        let boden = self.zweig_boden();
        if gabel < boden || gabel >= self.hoehe {
            return Err(boden);
        }
        let ablage = self.ablage.as_ref().ok_or(boden)?;
        let a = ablage.hoehen().into_iter().rev().find(|&h| h <= gabel && h >= self.basis).ok_or(boden)?;
        let (abzug, zustand) = ablage.lesen(a).map_err(|_| boden)?;
        let hoehe = self.hoehe;
        // ⛔️ Der Abzug muss der eigene Block an seiner Höhe sein, geprüft
        //   bevor etwas geändert wird; seine Zustandswurzel hat
        //   `Ablage::lesen` schon gegen den Block gehalten.
        let eigener = if a == self.basis { None } else { self.eigene_bloecke(a, a).and_then(|mut v| v.pop()) };
        if a != self.basis && eigener.map(|b| b.hash()) != Some(abzug.block.hash()) {
            return Err(boden);
        }
        let nachrechnen = self.eigene_bloecke(a + 1, gabel).ok_or(boden)?;
        let verlassen = self.eigene_bloecke(gabel + 1, hoehe).ok_or(boden)?;
        // Ab hier wird geändert. Datei und Ablagen zuerst: Was über `gabel`
        // lag, gehört zum verlassenen Zweig.
        if let Some(s) = self.speicher.as_mut() {
            if s.kuerzen_nach(gabel).is_err() {
                self.schreibfehler += 1;
            }
        }
        if let Some(a) = &self.ablage {
            if a.entfernen_ueber(gabel).is_err() || a.leicht().entfernen_ueber(gabel).is_err() {
                self.schreibfehler += 1;
            }
        }
        if self.baum.as_mut().is_some_and(|b| !b.kuerzen(gabel)) {
            self.baum = None;
        }
        // Nachgerechnet wird ohne Datei, Ablage und Baum, wie beim
        // Wiederanlauf: Die Blöcke stehen schon dort.
        let speicher = self.speicher.take();
        let ablage = self.ablage.take();
        let baum = self.baum.take();
        let hash = abzug.block.hash();
        self.hoehe = a;
        self.letzter_hash = hash;
        self.zustand = zustand;
        // Was über dem Abzug gerechtfertigt wurde, bringt das Nachrechnen
        // wieder, einmal.
        let gerechtfertigt = self.zustand.pruefpunkt_gerechtfertigt.0;
        self.zertifikate.retain(|z| z.hoehe <= gerechtfertigt);
        self.verlauf.clear();
        self.bekannt.clear();
        self.bekannt.insert(hash);
        self.verlauf_aufnehmen(a, &abzug.block);
        self.rueckwege.clear();
        self.offene_zuteilungen.clear();
        self.letzte_stichprobe = None;
        self.pods_der_stichprobe.clear();
        self.pruefer_der_stichprobe.clear();
        self.teilcache = None;
        self.teilgruppe.clear();
        let mut fehler = None;
        for b in &nachrechnen {
            if let Err(f) = self.uebernimm(b) {
                fehler = Some(f);
                break;
            }
        }
        self.speicher = speicher;
        self.ablage = ablage;
        self.baum = baum;
        let gerechtfertigt = self.zustand.pruefpunkt_gerechtfertigt.0;
        self.zertifikate.retain(|z| z.hoehe <= gerechtfertigt);
        self.eigene_zeugen.retain(|(_, h, _)| *h <= gabel);
        if self.letzter_abzug.is_some_and(|(h, _)| h > gabel) {
            self.letzter_abzug = None;
        }
        // 📌 Die eigenen Blöcke trugen schon einmal; tragen sie jetzt nicht,
        //   stimmt der Abzug nicht zur Datei, und das ist ein Befund.
        if fehler.is_some() || self.hoehe != gabel {
            self.schreibfehler += 1;
        }
        let n = verlassen.len();
        for b in verlassen {
            for tx in b.txs {
                self.aufnehmen(tx);
            }
        }
        Ok(n)
    }

    /// ⚑ **Rollt die Kette auf die Höhe `ziel` zurück** (CONSENSUS 7.7
    /// Schritt 2, 2026-11-04), damit sie einem anderen Zweig folgen kann.
    /// ⛔️ Nie unter [`Self::rueckweg_boden`], also nie unter den
    /// endgültigen Prüfpunkt (`Err` mit dem Boden, nichts geändert); ein
    /// zustandsloser Knoten gar nicht (er verankert neu).
    ///
    /// Zurückgenommen werden, jüngster Block zuerst: der Zustand über die
    /// Rückwege, Höhe und letzter Hash, `bekannt` und `verlauf`, was die
    /// Kette an Epochengrenzen ableitete; dann die Kettendatei, Abzüge,
    /// Historienbaum, Zertifikate und Zeugen über dem Ziel. Die
    /// Transaktionen der zurückgenommenen Blöcke gehen in den Mempool
    /// zurück, ältester Block zuerst. Gibt die Zahl der Blöcke zurück.
    pub fn zurueckrollen_bis(&mut self, ziel: u64) -> Result<usize, u64> {
        if self.zustandslos {
            return Err(self.hoehe);
        }
        let boden = self.rueckweg_boden();
        if ziel < boden {
            return Err(boden);
        }
        if ziel >= self.hoehe {
            return Ok(0);
        }
        let mut zurueck: Vec<Block> = Vec::new();
        while self.rueckwege.back().is_some_and(|r| r.hoehe > ziel) {
            let Some(r) = self.rueckwege.pop_back() else {
                break;
            };
            self.zustand.zuruecknehmen(r.weg);
            if let Some(n) = r.neben {
                self.offene_zuteilungen = n.offene_zuteilungen;
                self.letzte_stichprobe = n.letzte_stichprobe;
                self.pods_der_stichprobe = n.pods_der_stichprobe;
                self.pruefer_der_stichprobe = n.pruefer_der_stichprobe;
            }
            self.bekannt.remove(&r.block.hash());
            self.verlauf.remove(&r.hoehe);
            self.hoehe = r.hoehe - 1;
            self.letzter_hash = r.davor;
            zurueck.push(r.block);
        }
        // 📌 Der Historienbaum kann nur bis in die zuletzt vollendete Gruppe
        //   zurück; tiefer verliert die Kette ihn, wie nach einem Abzug.
        if self.baum.as_mut().is_some_and(|b| !b.kuerzen(self.hoehe)) {
            self.baum = None;
        }
        if let Some(s) = self.speicher.as_mut() {
            if s.kuerzen_nach(self.hoehe).is_err() {
                self.schreibfehler += 1;
            }
        }
        if let Some(a) = &self.ablage {
            if a.entfernen_ueber(self.hoehe).is_err() || a.leicht().entfernen_ueber(self.hoehe).is_err() {
                self.schreibfehler += 1;
            }
        }
        if self.letzter_abzug.is_some_and(|(h, _)| h > self.hoehe) {
            self.letzter_abzug = None;
        }
        // Angewandt wurden die Zertifikate in steigender Höhe; bis zum Ziel
        // also genau die bis zum jetzt gerechtfertigten.
        let gerechtfertigt = self.zustand.pruefpunkt_gerechtfertigt.0;
        self.zertifikate.retain(|z| z.hoehe <= gerechtfertigt);
        let hoehe = self.hoehe;
        self.eigene_zeugen.retain(|(_, h, _)| *h <= hoehe);
        self.teilcache = None;
        self.teilgruppe.clear();
        let n = zurueck.len();
        for b in zurueck.into_iter().rev() {
            for tx in b.txs {
                self.aufnehmen(tx);
            }
        }
        Ok(n)
    }

    /// Der Hash des eigenen Blocks an `hoehe`: aus den Rückwegen, die tiefer
    /// reichen als der Verlauf, dann aus dem Verlauf, und 📌 seit 7.7a
    /// (2026-11-05) aus der Kettendatei, bis zum Boden der Zweigwahl. Ohne
    /// sie fand eine Gabel unter den Rückwegen ihren Gabelpunkt nie
    /// (`PasstNirgends`).
    ///
    /// ⚑ An Hoehe 0 einer Kette ab Genesis der Startwert (2026-11-06): Er ist
    /// der Vorgaenger von Block 1, steht aber in keinem Rueckweg und in keiner
    /// Datei, und eine Gabel an Block 1 hiess sonst `PasstNirgends`.
    fn eigener_hash(&mut self, hoehe: u64) -> Option<Hash> {
        if hoehe == self.hoehe {
            return Some(self.letzter_hash);
        }
        if hoehe == 0 && self.basis == 0 {
            return Some(self.netzwert());
        }
        if hoehe > self.hoehe {
            return None;
        }
        if let Some(h) = self.rueckwege.iter().find(|r| r.hoehe == hoehe + 1).map(|r| r.davor).or_else(|| self.hash_an(hoehe)) {
            return Some(h);
        }
        if hoehe < self.zweig_boden() {
            return None;
        }
        self.speicher.as_mut()?.block_bei(hoehe).ok().flatten().map(|b| b.hash())
    }

    /// Der höchste Prüfpunkt, für den ein Block über `gabel` auf dem
    /// eigenen Zweig ein Zertifikat trägt; null, wenn keiner.
    ///
    /// Liegt der eigene gerechtfertigte selbst über `gabel`, steht sein
    /// Zertifikat auf dem eigenen Zweig, auch wenn die Rückwege nicht so
    /// tief reichen (7.7a).
    fn gerechtfertigt_ueber(&self, gabel: u64) -> u64 {
        let rueckwege = self.rueckwege.iter().filter(|r| r.hoehe > gabel).filter_map(|r| r.block.header.pruefpunkt.as_ref()).map(|z| z.hoehe).max().unwrap_or(0);
        let gerechtfertigt = self.zustand.pruefpunkt_gerechtfertigt.0;
        if gerechtfertigt > gabel {
            rueckwege.max(gerechtfertigt)
        } else {
            rueckwege
        }
    }

    /// ⚑ **Die Wahl des Zweigs** (CONSENSUS 7.7 Schritt 4, 2026-11-04,
    /// Ebb-and-Flow): Folgt die Kette den Blöcken `bloecke` (aufsteigend,
    /// lückenlos, der erste schliesst an einen eigenen an, schon eigene am
    /// Anfang werden übersprungen)?
    ///
    /// 1. **Der höhere gerechtfertigte Prüfpunkt gewinnt**, dann **die
    ///    grössere Höhe**; bei Gleichstand bleibt die Kette, wo sie ist (ein
    ///    Wechsel nur für einen echten Vorsprung).
    ///    Der gerechtfertigte Prüfpunkt geht vor, weil über ihn mehr als
    ///    zwei Drittel abgestimmt haben; ein längerer Zweig ohne ihn kann
    ///    nie endgültig werden.
    /// 2. ⛔️ **Nie unter [`Self::zweig_boden`]**, also nie unter den
    ///    endgültigen Prüfpunkt. Reichen die Rückwege nicht bis zum
    ///    Gabelpunkt, baut die Kette den Stand dort aus dem eigenen Abzug
    ///    neu auf ([`Self::neu_aufsetzen_bei`], 7.7a), nachdem Erzeuger und
    ///    Zertifikate des fremden Zweigs geprüft sind.
    /// 3. **Erst aus den Köpfen, dann gerechnet:** Verspricht der fremde Zweig
    ///    nach seinen Zertifikaten und seiner Höhe nichts Besseres, bleibt
    ///    alles unberührt. Sonst rollt die Kette zum Gabelpunkt zurück und
    ///    übernimmt Block für Block mit allen Prüfungen. Trägt einer nicht,
    ///    oder ist das Ergebnis doch nicht besser, kehrt sie auf den eigenen
    ///    Zweig zurück.
    pub fn zweig_folgen(&mut self, bloecke: &[Block]) -> Result<Zweigwahl, Zweigfehler> {
        if self.zustandslos {
            return Err(Zweigfehler::Zustandslos);
        }
        let erster = bloecke.iter().position(|b| self.eigener_hash(b.header.height) != Some(b.hash())).ok_or(Zweigfehler::Leer)?;
        let bloecke = &bloecke[erster..];
        for paar in bloecke.windows(2) {
            if paar[1].header.height != paar[0].header.height + 1 || paar[1].header.prev_block_hash != paar[0].hash() {
                return Err(Zweigfehler::Luecke(paar[1].header.height));
            }
        }
        let anfang = &bloecke[0];
        let gabel = anfang.header.height.checked_sub(1).ok_or(Zweigfehler::PasstNirgends)?;
        if gabel > self.hoehe || self.eigener_hash(gabel) != Some(anfang.header.prev_block_hash) {
            return Err(Zweigfehler::PasstNirgends);
        }
        // ⚑ Tiefer als die Rückwege (7.7a): neu aufsetzen aus dem Abzug,
        //   nie unter den endgültigen Prüfpunkt.
        let tief = gabel < self.rueckweg_boden();
        if tief && gabel < self.zweig_boden() {
            return Err(Zweigfehler::ZuTief(self.zweig_boden()));
        }
        let spitze = bloecke[bloecke.len() - 1].header.height;
        if gabel < self.hoehe {
            let fremd = bloecke.iter().filter_map(|b| b.header.pruefpunkt.as_ref()).map(|z| z.hoehe).max().unwrap_or(0);
            if (fremd, spitze) <= (self.gerechtfertigt_ueber(gabel), self.hoehe) {
                return Ok(Zweigwahl::Geblieben);
            }
        }
        // ⛔️ **Vor einem Neuaufbau, was ohne Zustand geht** (7.7a): Er
        //   kostet ein Nachrechnen bis zu einer Epoche; also nur für einen
        //   Zweig, dessen Erzeuger und Zertifikate tragen. Einen solchen zu
        //   fälschen braucht Einsatz.
        if tief {
            for b in bloecke {
                if !self.erzeuger_traegt(b) {
                    return Err(Zweigfehler::TraegtNicht(b.header.height, KettenFehler::ErzeugerUnbelegt));
                }
                if !self.pruefpunkt_traegt_im_zweig(b, gabel, bloecke) {
                    return Err(Zweigfehler::TraegtNicht(b.header.height, KettenFehler::PruefpunktTraegtNicht));
                }
            }
        }
        let vorher = (self.zustand.pruefpunkt_gerechtfertigt.0, self.hoehe);
        let eigene: Vec<Block> = if tief {
            let hoehe = self.hoehe;
            self.eigene_bloecke(gabel + 1, hoehe).ok_or(Zweigfehler::ZuTief(self.rueckweg_boden()))?
        } else {
            self.rueckwege.iter().filter(|r| r.hoehe > gabel).map(|r| r.block.clone()).collect()
        };
        let zurueck = if tief { self.neu_aufsetzen_bei(gabel) } else { self.zurueckrollen_bis(gabel) }.map_err(Zweigfehler::ZuTief)?;
        let mut traegt_nicht = None;
        for b in bloecke {
            if let Err(f) = self.uebernimm(b) {
                traegt_nicht = Some((b.header.height, f));
                break;
            }
        }
        let nachher = (self.zustand.pruefpunkt_gerechtfertigt.0, self.hoehe);
        if traegt_nicht.is_none() && nachher > vorher {
            return Ok(Zweigwahl::Gewechselt { gabel, zurueck, neu: bloecke.len() });
        }
        // Zurück auf den eigenen Zweig; seine Blöcke trugen schon einmal.
        // Reichen die Rückwege nach einem langen fremden Stück nicht mehr bis
        // `gabel`, noch einmal aus dem Abzug.
        if self.zurueckrollen_bis(gabel).is_err() {
            let _ = self.neu_aufsetzen_bei(gabel);
        }
        for b in &eigene {
            let _ = self.uebernimm(b);
        }
        match traegt_nicht {
            Some((hoehe, f)) => Err(Zweigfehler::TraegtNicht(hoehe, f)),
            None => Ok(Zweigwahl::Geblieben),
        }
    }

    /// Übernimmt einen fremden Block und rechnet nach.
    ///
    /// Die Reihenfolge der Prüfungen ist bedeutsam: **Erst anschließen,
    /// dann rechnen.** Ein Block, der nicht anschließt, darf den Zustand
    /// nicht anfassen, sonst hinterlässt ein verirrter Block eine Spur.
    pub fn uebernimm(&mut self, block: &Block) -> Result<(), KettenFehler> {
        if self.zustandslos {
            return Err(KettenFehler::ZeugeTraegtNicht(None));
        }
        self.uebernimm_innen(block, None)
    }

    /// ⚑ **Übernimmt einen Block mit seinem Zeugen** (zustandslose
    /// Validierung, 2026-10-29): für einen zustandslosen Knoten. Der Zeuge
    /// muss zur Wurzel vor dem Block passen, und das Anwenden darf nichts
    /// ausserhalb von ihm berühren; sonst bleibt der Zustand, wie er war.
    /// Ein voller Knoten braucht den Zeugen nicht und übergeht ihn.
    pub fn uebernimm_mit_zeuge(&mut self, block: &Block, zeuge: &myl_ledger::state::Zeuge) -> Result<(), KettenFehler> {
        if !self.zustandslos {
            return self.uebernimm_innen(block, None);
        }
        self.uebernimm_innen(block, Some(zeuge))
    }

    fn uebernimm_innen(&mut self, block: &Block, zeuge: Option<&myl_ledger::state::Zeuge>) -> Result<(), KettenFehler> {
        let hash = block.hash();
        if self.bekannt.contains(&hash) {
            return Err(KettenFehler::SchonBekannt);
        }
        // ⚑ **Wer ihn erzeugt hat**, vor jeder Rechnung: Ein Block, den
        // niemand Berechtigtes unterschrieben hat, kostet keinen
        // Zustandsübergang.
        if !self.erzeuger_traegt(block) {
            return Err(KettenFehler::ErzeugerUnbelegt);
        }
        if block.header.prev_block_hash != self.letzter_hash {
            return Err(KettenFehler::PasstNichtAn {
                erwartet: self.letzter_hash,
                bekommen: block.header.prev_block_hash,
            });
        }
        // **Die Höhe wächst um genau eins.** Der Vorgängerhash bindet
        // die Kette schon; die Höhe zusätzlich zu prüfen kostet nichts
        // und hält ein Feld ehrlich, an dem die Nachlieferung ihre
        // Lücke abzählt. Ein Kopf mit falscher Höhe wäre sonst ein
        // gültiger Block, der einen Nachzügler in die Irre schickt.
        let erwartete_hoehe = self.hoehe + 1;
        if block.header.height != erwartete_hoehe {
            return Err(KettenFehler::HoeheWeichtAb {
                erwartet: erwartete_hoehe,
                bekommen: block.header.height,
            });
        }
        // **Und die Epoche folgt aus der Höhe.** Sie steht mit im Kopf,
        // damit ein Block für sich lesbar bleibt; geprüft wird sie
        // trotzdem, sonst wäre sie ein Feld, das jeder setzen darf und
        // niemand nachrechnet — und daran hängen Verfall und Fristen.
        let erwartete_epoche = epoche_fuer_hoehe(block.header.height);
        if block.header.epoch != erwartete_epoche {
            return Err(KettenFehler::EpocheWeichtAb {
                erwartet: erwartete_epoche,
                bekommen: block.header.epoch,
            });
        }
        // ⚑ **Das Zertifikat eines Prüfpunkts** (2026-10-27, CONSENSUS
        // 7.2), bevor gerechnet wird: Aus ihm kommen Endgültigkeit und Saat.
        // Erst nach Erzeuger und Anschluss, weil es die teurere Prüfung ist.
        if !self.pruefpunkt_traegt(block) {
            return Err(KettenFehler::PruefpunktTraegtNicht);
        }

        // ⚑ **Was im Block steht, muss geprüft werden oder darf nicht
        // drin stehen** (Fund 192). Beide Felder sind Teil des
        // gehashten Blocks und wurden bis zum 2026-09-06 von der Kette
        // weder geprüft noch angewandt.
        if !block.verdicts.is_empty() {
            return Err(KettenFehler::SchuldspruchOhneBeleg {
                anzahl: block.verdicts.len(),
            });
        }
        if block.validate_challenges().is_err() {
            return Err(KettenFehler::AnfechtungUnsinnig);
        }

        // Weicht das Ergebnis ab, bleibt der eigene Zustand unberührt: Ein
        // Knoten, der einem abweichenden Block folgt, hätte den Befund
        // verschluckt. ⚑ **Gesichert statt kopiert** (Skalierung,
        // 2026-10-29): Bis hierher wurde auf einer Kopie des ganzen Zustands
        // gerechnet (bei einer Million Konten 30 ms je Block); jetzt auf dem
        // Zustand selbst, und die Sammlungen spielen ihr Journal zurück, wenn
        // die Wurzel nicht stimmt.
        if let Some(z) = zeuge {
            self.zustand.zeuge_einsetzen(z).map_err(|g| KettenFehler::ZeugeTraegtNicht(Some(g)))?;
        }
        // Ein voller Knoten, der Zeugen bereithält, zeichnet auf wie ein
        // Erzeuger.
        let bereithalten = zeuge.is_none() && !self.zustandslos && self.zeugenschluessel.is_some();
        if bereithalten {
            self.zustand.aufzeichnen_beginnen();
        }
        let sicherung = self.zustand.sichern();
        let davor = self.letzter_hash;
        let unterschriften = &self.unterschriften;
        let mut gezogen = None;
        let ablaufend = self.ablaufende_zuteilung(block.header.epoch);
        let mut ausgaben = Pruefausgaben::default();
        // ⚑ **Das Zertifikat kommt aus dem Block, nicht aus eigenem
        // Zustand.** Sonst zöge jeder Knoten eine andere Stichprobe.
        Self::anwenden(
            &mut self.zustand,
            &block.txs,
            block.header.epoch,
            &self.letzter_hash,
            block.header.pruefpunkt.as_ref(),
            &mut gezogen,
            self.stimmsatz.as_ref(),
            &mut ausgaben,
            &|t: &Transaktion| unterschriften.kennt(&t.kennung()),
        );
        self.zustand.letzter_slot = block.header.slot;
        let eigener_zeuge = if bereithalten { self.zustand.zeuge_nehmen().ok().and_then(|z| borsh::to_vec(&z).ok()) } else { None };
        let errechnet = self.zustand.wurzel();
        // Beim Zustandslosen zählt auch, dass nichts ausserhalb des Zeugen
        // berührt wurde; sonst hiesse die Wurzel nichts.
        let eingehalten = zeuge.is_none() || self.zustand.zeuge_eingehalten();
        if errechnet != block.header.state_root || !eingehalten {
            self.zustand.zuruecksetzen(sicherung);
            if zeuge.is_some() {
                self.zustand.zeuge_abschliessen(false);
            }
            if !eingehalten {
                return Err(KettenFehler::ZeugeTraegtNicht(Some(myl_ledger::state::Zeugenfehler::Unvollstaendig)));
            }
            return Err(KettenFehler::ZustandWeichtAb {
                erwartet: block.header.state_root,
                errechnet,
            });
        }
        // ⚑ Ein voller Knoten hebt den Rückweg auf (Zurückrollen,
        //   2026-11-04); ein zustandsloser hält die Sammlungen nur teilweise
        //   und verankert stattdessen neu.
        let weg = if self.zustandslos {
            self.zustand.sicherung_verwerfen();
            None
        } else {
            Some(self.zustand.sicherung_behalten(sicherung))
        };
        if let Some(z) = eigener_zeuge {
            self.zeuge_bereithalten(hash, block.header.height, z);
            // Gleich unterschrieben: Auf Nachfrage wird nur Gehaltenes
            // geliefert.
            let _ = self.zeugenstuecke(&hash);
        }
        if let Some(z) = zeuge {
            // Die eigenen Konten, die der Block berührte, vor dem Leeren.
            let beruehrt: Vec<Address> =
                self.eigene_konten.keys().filter(|a| z.accounts.iter().any(|(k, _)| k == *a)).copied().collect();
            self.eigene_konten.extend(self.zustand.konten_still(&beruehrt));
            for a in &beruehrt {
                self.eigene_unbekannt.remove(a);
            }
            self.zustand.zeuge_abschliessen(true);
        }
        // 📌 Bis 0.79.0 kannte nur der Erzeuger die Ziehung; der Empfang
        //    warf sie weg, und kein zugeteilter Checker erfuhr von seiner
        //    Ziehung (erster gezielter Lauf mit Nachrechnen, 2026-10-29:
        //    vier Ziehungen, null zugeteilt).
        let neben = (ablaufend.is_some() || gezogen.is_some()).then(|| self.nebenstand());
        self.zuteilung_aufheben(ablaufend);
        self.ziehung_merken(gezogen);
        self.pruefausgaben.anhaengen(ausgaben);
        if let Some(weg) = weg {
            self.rueckweg_merken(Rueckspur { hoehe: block.header.height, davor, block: block.clone(), weg, neben });
        }
        if let Some(b) = self.baum.as_mut() {
            b.anhaengen(&self.letzter_hash);
        }
        self.hoehe = block.header.height;
        self.letzter_hash = hash;
        self.bekannt.insert(hash);
        self.verlauf_aufnehmen(block.header.height, block);
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
        //
        // Seit Fund 627 über die Nummern statt über die Bytes.
        self.mempool_bereinigen(block.header.height);
        self.schreibe(block);
        self.zertifikat_merken(block);
        self.abzug_anlegen(block);
        self.erzeuger_lernen(block);
        Ok(())
    }
}

impl Default for Kette {
    fn default() -> Self {
        Self::probestand()
    }
}


/// Was das Anwenden eines Blocks neben dem Zustand an Prüfung ergab.
#[derive(Debug, Default)]
pub struct Pruefausgaben {
    /// Die abgeschlossenen Prüfaufträge.
    pub abschluesse: Vec<myl_ledger::pruefung::Pruefabschluss>,
    /// Die entschiedenen Streitfälle, mit dem Fehler der Buchung, falls
    /// sie scheiterte.
    pub urteile: Vec<(myl_ledger::pruefung::Schiedsurteil, Option<String>)>,
}

impl Pruefausgaben {
    fn anhaengen(&mut self, andere: Pruefausgaben) {
        self.abschluesse.extend(andere.abschluesse);
        self.urteile.extend(andere.urteile);
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod speicherbindung {
    use super::*;

    #[test]
    fn der_startwert_ist_der_erste_letzte_hash() {
        // Die Kettendatei bindet sich an diesen Wert. Wichen die beiden
        // Stellen auseinander, lehnte eine frische Datei ihre eigene
        // Kette ab, und der Knoten begänne nach jedem Neustart bei null,
        // ohne dass jemand es merkte.
        assert_eq!(Kette::probestand().letzter_hash(), Kette::startwert());
    }

    #[test]
    fn ohne_speicher_zaehlt_nichts_und_es_faellt_nichts_aus() {
        let mut k = Kette::probestand();
        assert_eq!(k.gespeicherte_bloecke(), None);
        let _ = k.baue_block();
        assert_eq!(k.schreibfehler(), 0);
        assert_eq!(k.gespeicherte_bloecke(), None);
    }
}

#[cfg(test)]
mod mempool_tests {
    use super::*;

    fn wartend(wer: u8, nonce: u64, max_gebuehr: u64, seit: u64) -> Wartend {
        let sk = probeschluessel(wer);
        let mut tx = Transaktion::signiere(&Kette::startwert(), &sk, nonce, Anweisung::Burn { betrag: 1 }).expect("signieren");
        tx.max_gebuehr = max_gebuehr;
        let bytes = borsh::to_vec(&tx).map_or(usize::MAX, |b| b.len());
        Wartend { seit, absender: tx.absender_adresse(), tx, bytes, neuling: false }
    }

    /// ⚑ **Verdrängt werden die billigsten Enden** (Skalierung, 2026-10-29):
    /// Ein voller Mempool nimmt eine teurere Transaktion an und wirft dafür
    /// das billigste Ende einer Nummernkette, nie eine aus der Mitte; eine
    /// gleich billige oder billigere kommt nicht hinein. Die Zähler stimmen
    /// danach.
    #[test]
    fn ein_voller_mempool_verdraengt_das_billigste_ende() {
        let mut m = Mempool { plaetze: 4, ..Mempool::default() };
        m.hinein(wartend(1, 0, 5_000, 1));
        m.hinein(wartend(1, 1, 9_000, 1)); // Ende von 1, teuer
        m.hinein(wartend(2, 0, 3_000, 1)); // billig, aber nicht das Ende von 2
        m.hinein(wartend(2, 1, 4_000, 2)); // Ende von 2
        assert!(m.voll());
        assert!(!m.verdraengen(&wartend(3, 0, 4_000, 3)), "gleich billig verdrängt nicht");
        assert!(m.verdraengen(&wartend(3, 0, 6_000, 3)));
        assert_eq!(m.len(), 3);
        assert!(m.liste.iter().any(|w| w.absender == probekonto(2) && w.tx.nonce == 0), "nie aus der Mitte");
        assert!(!m.liste.iter().any(|w| w.absender == probekonto(2) && w.tx.nonce == 1), "das billigste Ende fiel");
        assert_eq!((m.je_absender(&probekonto(1)), m.je_absender(&probekonto(2))), (2, 1));
    }
}

#[cfg(test)]
mod pruefer_tests {
    use myl_types::ids::{MinerId, PodId};

    /// ⚑ **W6b: Der Checker einer Ziehung** ist einer der Kandidaten, für
    /// alle Knoten derselbe (gleich in welcher Folge sie die Kandidaten
    /// kennen), und über viele Ziehungen bekommt jeder etwas. Ohne
    /// Kandidaten gibt es keinen.
    #[test]
    fn der_pruefer_ist_ein_kandidat_und_fuer_alle_derselbe() {
        let kandidaten: Vec<MinerId> = (1..=5u8).map(|i| MinerId::new([i; 32])).collect();
        let mut verdreht = kandidaten.clone();
        verdreht.reverse();
        verdreht.push(kandidaten[0]);
        let pod = PodId::new([7; 32]);
        // Der erste der beiden Checker (`myl_ledger::pruefung`).
        let waehle = |k: &[MinerId], e: u64, s: u32| myl_ledger::pruefung::pruefer_waehlen(k, e, &pod, s, 1).first().copied();
        let mut gezaehlt = std::collections::BTreeMap::new();
        for segment in 0..200u32 {
            let m = waehle(&kandidaten, 3, segment).expect("ein Checker");
            assert!(kandidaten.contains(&m));
            assert_eq!(waehle(&verdreht, 3, segment), Some(m), "Folge und Dubletten ändern nichts");
            *gezaehlt.entry(m).or_insert(0u32) += 1;
        }
        assert_eq!(gezaehlt.len(), 5, "jeder Kandidat kommt dran: {gezaehlt:?}");
        assert!(gezaehlt.values().all(|n| *n >= 20), "grob gleich verteilt: {gezaehlt:?}");
        // Eine andere Epoche wählt anders, und ohne Kandidaten niemand.
        let anders = (0..50u32).filter(|s| waehle(&kandidaten, 4, *s) != waehle(&kandidaten, 3, *s)).count();
        assert!(anders > 0);
        assert_eq!(waehle(&[], 3, 0), None);
    }
}
