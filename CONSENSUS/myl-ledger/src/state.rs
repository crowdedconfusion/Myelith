//! Kontenmodell und Ledger-Zustand (Punkt 1.1).

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::ids::{Address, EpochId, SitzungId};
use myl_types::sitzung::{Sitzungskontrakt, Sitzungszustand};
use myl_types::Hash;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Wie viele Epochen weit die Verstoßhistorie eines Kontos zurückreicht.
///
/// **Eine Aufbewahrungsfrist, keine Politik.** Sie begrenzt, wie viel
/// Zustand ein Konto tragen kann: Nach jedem Vermerk stehen höchstens so
/// viele Einträge in [`AccountState::verstoesse`], wie hier Epochen
/// genannt sind. Ohne diese Grenze wüchse der Konsenszustand mit jedem
/// Urteil weiter, und zwar dauerhaft.
///
/// ⚑ **Wer daran dreht, dreht zugleich an der Slashing-Staffelung.**
/// `myl_tokenomics::WIEDERHOLUNGSFENSTER` **ist** diese Konstante und
/// keine zweite daneben; eine Staffelung über ein längeres Fenster als
/// die Aufbewahrung würde eine Vorgeschichte lesen, die es nicht mehr
/// gibt, und das fiele niemandem auf — der Zähler stünde einfach
/// niedriger. Dieselbe Klasse wie γ und der Kontrollsegment-Vorrat: Ein
/// Wert, der eine Schutzwirkung verstärkt, kann eine andere aufzehren.
pub const VERSTOSS_FENSTER: u64 = 10;

/// Wie viele Epochen nach ihrem Ende eine Session noch im Zustand
/// steht.
///
/// ⚑ **Eine Aufbewahrungsfrist aus demselben Grund wie
/// [`VERSTOSS_FENSTER`], und zwar ein dringenderer.** Session-Kontrakte
/// legt jeder Nutzer selbst an; ohne Frist wüchse der Konsenszustand
/// mit jedem jemals eröffneten Kontrakt weiter, und die Größe hinge an
/// einer Eingabe, die ein Angreifer bestimmt.
///
/// **Aufgeräumt wird in einem Übergang, nicht nebenbei**
/// ([`crate::transitions::sitzung_aufraeumen`]): Ein Aufräumen beim
/// Lesen machte den Zustand davon abhängig, **wer wann gelesen hat**,
/// und zwei Knoten mit verschiedener Lesereihenfolge kämen zu
/// verschiedenen Verpflichtungen. Dieselbe Lehre wie bei
/// [`LedgerState::verstoesse_im_fenster`].
pub const SITZUNG_NACHFRIST: u64 = 100;

/// Eine Agenten-Session im Zustand: der unveränderliche Kontrakt und
/// was unter ihm schon verbraucht ist (Whitepaper Kap. 8.2).
///
/// **Der Kontrakt liegt hier, obwohl seine Adresse der Schlüssel ist**,
/// und nicht etwa nur der Verbrauch. Ein Knoten muss die Grenzen prüfen
/// können, ohne sie von irgendwem gezeigt zu bekommen; genau darin
/// besteht der Unterschied zwischen „vom Konsens durchgesetzt" und „vom
/// Client behauptet".
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Sitzung {
    /// Die Grenzen, wie der Inhaber sie gesetzt hat.
    pub kontrakt: Sitzungskontrakt,
    /// Was davon verbraucht ist.
    pub zustand: Sitzungszustand,
}

/// Ein Eintrag der Verstoßhistorie: wie oft ein Konto in **einer**
/// Epoche geschlachtet wurde.
///
/// **Je Epoche ein Eintrag, nicht je Verstoß.** Ein Konto, das in
/// derselben Epoche zehnmal auffällt, trägt einen Eintrag mit `anzahl =
/// 10` statt zehn Einträgen. Sonst hinge die Größe des Konsenszustands
/// daran, wie oft jemand auffällt, und das ist genau die Größe, die ein
/// Angreifer selbst bestimmt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Verstoss {
    /// Die Epoche, in der die Verstöße gebucht wurden.
    pub epoche: EpochId,
    /// Wie viele es in dieser Epoche waren.
    pub anzahl: u32,
}

/// Zustand eines Kontos (Adresse).
///
/// - `balance`: verfügbare MYL-Kleinstbeträge
/// - `staked`: als Validator-/Miner-Stake gebundene MYL-Kleinstbeträge
/// - `credits`: noch nicht verbrauchte Inferenz-Credits (vTFE),
///   aufsteigend nach Verfalls-Epoche geordnet (Ausgabereihenfolge:
///   zuerst verfallende Credits, siehe `credit_spend`).
/// - `verstoesse`: Verstoßhistorie, aufsteigend nach Epoche, gekürzt auf
///   [`VERSTOSS_FENSTER`] (siehe [`LedgerState::verstoesse_im_fenster`]).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct AccountState {
    pub balance: u64,
    pub staked: u64,
    pub credits: Vec<myl_types::InferenceCredit>,
    /// Wann dieses Konto geschlachtet wurde, je Epoche gezählt.
    ///
    /// **Ein Konsensfeld.** Es geht in [`LedgerState::commitment`] ein,
    /// weil die Slashing-Staffelung daraus folgt: Zwei Knoten, die
    /// verschiedene Vorgeschichten führen, schlachten verschieden hoch
    /// und kommen zu verschiedenen Zuständen.
    pub verstoesse: Vec<Verstoss>,
}

impl AccountState {
    /// Leeres Konto.
    pub fn empty() -> Self {
        Self {
            balance: 0,
            staked: 0,
            credits: Vec::new(),
            verstoesse: Vec::new(),
        }
    }
}

/// Der vollständige Ledger-Zustand.
///
/// `accounts` ist eine `BTreeMap` — die deterministische Ordnung ist
/// Konsens-Eigenschaft (siehe Modul-Dokumentation in `lib.rs`).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct LedgerState {
    /// Aktuelle Epoche.
    pub epoch: EpochId,
    /// Credit-Preis: MYL-Kleinstbeträge je vTFE-Einheit
    /// (später von TOKENOMICS aktualisiert; Startwert zu Genesis).
    pub credit_price: u64,
    /// Konten, deterministisch nach Adresse geordnet.
    pub accounts: BTreeMap<Address, AccountState>,
    /// Offene Agenten-Sessions, deterministisch nach Kontraktadresse
    /// geordnet (Whitepaper Kap. 8.2).
    pub sitzungen: BTreeMap<SitzungId, Sitzung>,
}

impl LedgerState {
    /// Genesis-Leerzustand mit gegebenem Credit-Preis.
    pub fn genesis(credit_price: u64) -> Self {
        Self {
            epoch: EpochId(0),
            credit_price,
            accounts: BTreeMap::new(),
            sitzungen: BTreeMap::new(),
        }
    }

    /// Konto lesen oder das leere Konto liefern (liest nie zustands-
    /// verändernd; `account_mut` für Übergänge).
    pub fn account(&self, addr: &Address) -> AccountState {
        self.accounts.get(addr).cloned().unwrap_or_else(AccountState::empty)
    }

    /// Eine Session lesen, ohne sie anzulegen.
    pub fn sitzung(&self, adresse: &SitzungId) -> Option<&Sitzung> {
        self.sitzungen.get(adresse)
    }

    /// Konto zur Veränderung lesen bzw. anlegen.
    pub fn account_mut(&mut self, addr: &Address) -> &mut AccountState {
        self.accounts.entry(*addr).or_insert_with(AccountState::empty)
    }

    /// Wie oft dieses Konto innerhalb der letzten `fenster` Epochen
    /// geschlachtet wurde, die laufende eingeschlossen.
    ///
    /// **Rein lesend und ohne Kürzung.** Der Aufruf verändert nichts,
    /// auch nicht die Historie: Würde er dabei alte Einträge wegräumen,
    /// hinge der Zustand daran, **wer wann gelesen hat**, und zwei
    /// Knoten mit unterschiedlicher Lesereihenfolge kämen zu
    /// verschiedenen Verpflichtungen. Gekürzt wird ausschließlich beim
    /// Vermerken, also in einem Übergang. Dasselbe Muster wie bei den
    /// verfallenen Credits in `credit_spend`.
    ///
    /// `fenster = 0` ergibt null: kein Fenster, keine Vorgeschichte.
    /// Ein Fenster, das über die Epoche 0 hinausreicht, wird bei 0
    /// abgeschnitten statt umzulaufen.
    pub fn verstoesse_im_fenster(&self, addr: &Address, fenster: u64) -> u64 {
        if fenster == 0 {
            return 0;
        }
        let ab = EpochId(self.epoch.0.saturating_sub(fenster.saturating_sub(1)));
        self.accounts
            .get(addr)
            .map(|k| {
                k.verstoesse
                    .iter()
                    .filter(|v| v.epoche >= ab && v.epoche <= self.epoch)
                    .fold(0u64, |summe, v| summe.saturating_add(v.anzahl as u64))
            })
            .unwrap_or(0)
    }

    /// Vermerkt einen Verstoß in der laufenden Epoche und kürzt die
    /// Historie auf [`VERSTOSS_FENSTER`].
    ///
    /// **Nicht öffentlich, und das ist die Zusage.** Der einzige Weg,
    /// einen Verstoß in den Zustand zu bekommen, führt über ein
    /// gebuchtes Urteil ([`crate::transitions::apply_verdict`]). Wäre
    /// das Vermerken von außen aufrufbar, gäbe es zwei Wege zu
    /// derselben Tatsache — einen, der schlachtet und zählt, und einen,
    /// der nur zählt. Genau daraus entstehen Zähler, die von dem
    /// abweichen, was tatsächlich geschehen ist.
    ///
    /// **Gekürzt wird vor dem Zählen**, damit die Länge der Historie
    /// nach jedem Vermerk höchstens [`VERSTOSS_FENSTER`] Einträge
    /// beträgt, unabhängig davon, wie lange das Konto ruhig war.
    pub(crate) fn verstoss_vermerken(&mut self, addr: &Address) {
        let jetzt = self.epoch;
        let ab = EpochId(jetzt.0.saturating_sub(VERSTOSS_FENSTER.saturating_sub(1)));
        let konto = self.account_mut(addr);
        konto.verstoesse.retain(|v| v.epoche >= ab && v.epoche <= jetzt);
        match konto.verstoesse.iter_mut().find(|v| v.epoche == jetzt) {
            Some(v) => v.anzahl = v.anzahl.saturating_add(1),
            None => {
                konto.verstoesse.push(Verstoss { epoche: jetzt, anzahl: 1 });
                // Aufsteigend nach Epoche: Die Ordnung ist
                // Konsens-Eigenschaft wie die der Konten, denn sie geht
                // in die Serialisierung und damit in das Commitment ein.
                konto.verstoesse.sort_by_key(|v| v.epoche.0);
            }
        }
    }

    /// Kanonische Zustands-Verpflichtung: SHA-256 über die
    /// Borsh-Serialisierung. Borsh ist kanonisch und die Kontenordnung
    /// fest — gleiche Zustände ergeben auf jedem Node dieselben Bytes
    /// und damit denselben Hash (Grundlage für spätere
    /// Cross-Node-Konsistenzprüfung und Block-Commitments).
    pub fn commitment(&self) -> Hash {
        let bytes = borsh::to_vec(self).expect("Ledger-Zustand ist stets serialisierbar");
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let digest = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        Hash::from_bytes(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adresse(byte: u8) -> Address {
        Address::new([byte; 32])
    }

    #[test]
    fn genesis_zustand_ist_leer_und_bekommt_konten() {
        let mut state = LedgerState::genesis(100);
        assert_eq!(state.epoch, EpochId(0));
        assert_eq!(state.accounts.len(), 0);
        assert_eq!(state.account(&adresse(1)), AccountState::empty());
        state.account_mut(&adresse(1)).balance = 500;
        assert_eq!(state.account(&adresse(1)).balance, 500);
        assert_eq!(state.accounts.len(), 1);
    }

    #[test]
    fn commitment_ist_deterministisch_und_unterscheidet_zustaende() {
        let mut a = LedgerState::genesis(100);
        let mut b = LedgerState::genesis(100);
        assert_eq!(a.commitment(), b.commitment());

        a.account_mut(&adresse(1)).balance = 10;
        b.account_mut(&adresse(1)).balance = 10;
        // Gleiche Änderungen, unabhängig voneinander ausgeführt:
        assert_eq!(a.commitment(), b.commitment());

        b.account_mut(&adresse(1)).balance = 11;
        assert_ne!(a.commitment(), b.commitment());
    }

    // --- Verstoßhistorie ---------------------------------------------

    /// Gezählt wird, was im Fenster liegt, und nur das.
    #[test]
    fn nur_verstoesse_im_fenster_zaehlen() {
        let mut state = LedgerState::genesis(100);
        let a = adresse(1);

        state.epoch = EpochId(1);
        state.verstoss_vermerken(&a);
        state.epoch = EpochId(5);
        state.verstoss_vermerken(&a);
        state.verstoss_vermerken(&a);

        // Fenster 10 ab Epoche 5 reicht bis Epoche 0: alle drei zaehlen.
        assert_eq!(state.verstoesse_im_fenster(&a, VERSTOSS_FENSTER), 3);
        // Fenster 5 ab Epoche 5 reicht bis Epoche 1: ebenfalls alle drei.
        assert_eq!(state.verstoesse_im_fenster(&a, 5), 3);
        // Fenster 2 ab Epoche 5 reicht bis Epoche 4: nur die beiden aus 5.
        assert_eq!(state.verstoesse_im_fenster(&a, 2), 2);
        // Kein Fenster, keine Vorgeschichte.
        assert_eq!(state.verstoesse_im_fenster(&a, 0), 0);
        // Ein unbeteiligtes Konto hat keine.
        assert_eq!(state.verstoesse_im_fenster(&adresse(2), VERSTOSS_FENSTER), 0);
    }

    /// **Ein Fenster ueber die Epoche 0 hinaus laeuft nicht um.**
    ///
    /// Ohne die saettigende Subtraktion ergaebe `0 - 9` die Untergrenze
    /// `u64::MAX`, und die Bedingung `epoche >= ab` waere fuer **keinen**
    /// Eintrag erfuellt: Die Vorgeschichte waere in den ersten Epochen des
    /// Netzes leer, und die Staffelung damit abgeschaltet, genau in der
    /// Zeit, in der sie am ehesten gebraucht wird.
    #[test]
    fn ein_fenster_vor_der_ersten_epoche_laeuft_nicht_um() {
        let mut state = LedgerState::genesis(100);
        let a = adresse(1);
        state.epoch = EpochId(0);
        state.verstoss_vermerken(&a);
        assert_eq!(state.verstoesse_im_fenster(&a, VERSTOSS_FENSTER), 1);
        assert_eq!(state.verstoesse_im_fenster(&a, u64::MAX), 1);
    }

    /// **Die Historie waechst nicht ueber das Fenster hinaus.**
    ///
    /// Sonst haenge die Groesse des Konsenszustands daran, wie oft jemand
    /// auffaellt, und das ist eine Groesse, die ein Angreifer selbst
    /// bestimmt.
    #[test]
    fn die_historie_bleibt_auf_das_fenster_begrenzt() {
        let mut state = LedgerState::genesis(100);
        let a = adresse(1);
        for e in 0..200u64 {
            state.epoch = EpochId(e);
            // Mehrfach je Epoche: Der zweite Vermerk darf keinen zweiten
            // Eintrag erzeugen.
            state.verstoss_vermerken(&a);
            state.verstoss_vermerken(&a);
            assert!(
                state.account(&a).verstoesse.len() as u64 <= VERSTOSS_FENSTER,
                "nach Epoche {e} stehen {} Eintraege",
                state.account(&a).verstoesse.len()
            );
        }
        // Und gezaehlt wird trotzdem richtig: zwei je Epoche ueber das Fenster.
        assert_eq!(
            state.verstoesse_im_fenster(&a, VERSTOSS_FENSTER),
            2 * VERSTOSS_FENSTER
        );
    }

    /// **Lesen veraendert nichts, auch nicht die Historie.**
    ///
    /// Raeumte das Lesen alte Eintraege weg, hinge der Zustand daran, wer
    /// wann gelesen hat, und zwei Knoten mit verschiedener
    /// Lesereihenfolge kaemen zu verschiedenen Verpflichtungen. Das ist
    /// der Grund, warum ausschliesslich der Uebergang kuerzt.
    #[test]
    fn lesen_veraendert_die_verpflichtung_nicht() {
        let mut state = LedgerState::genesis(100);
        let a = adresse(1);
        state.epoch = EpochId(3);
        state.verstoss_vermerken(&a);
        state.epoch = EpochId(80); // weit ausserhalb des Fensters

        let vorher = state.commitment();
        for fenster in [0u64, 1, VERSTOSS_FENSTER, u64::MAX] {
            let _ = state.verstoesse_im_fenster(&a, fenster);
        }
        assert_eq!(vorher, state.commitment(), "das Lesen hat den Zustand bewegt");
        // Der alte Eintrag liegt noch da und zaehlt trotzdem nicht mehr.
        assert_eq!(state.account(&a).verstoesse.len(), 1);
        assert_eq!(state.verstoesse_im_fenster(&a, VERSTOSS_FENSTER), 0);
    }

    /// **Die Historie ist Konsensfeld, nicht Beiwerk.**
    ///
    /// Zwei Zustaende, die sich nur in ihr unterscheiden, muessen
    /// verschiedene Verpflichtungen tragen — sonst koennten zwei Knoten
    /// mit verschiedenen Vorgeschichten denselben Block bestaetigen und
    /// beim naechsten Urteil verschieden hoch schlachten.
    #[test]
    fn die_historie_geht_in_die_verpflichtung_ein() {
        let mut ohne = LedgerState::genesis(100);
        ohne.account_mut(&adresse(1)).staked = 10;
        let mut mit = ohne.clone();
        assert_eq!(ohne.commitment(), mit.commitment());
        mit.verstoss_vermerken(&adresse(1));
        assert_ne!(
            ohne.commitment(),
            mit.commitment(),
            "ein vermerkter Verstoss blieb ohne Wirkung auf die Verpflichtung"
        );
    }

    /// Die Reihenfolge der Eintraege haengt nicht an der Reihenfolge der
    /// Epochen, in denen vermerkt wurde.
    #[test]
    fn die_historie_ist_nach_epoche_geordnet() {
        let mut state = LedgerState::genesis(100);
        let a = adresse(1);
        for e in [5u64, 2, 7, 1] {
            state.epoch = EpochId(e);
            state.verstoss_vermerken(&a);
        }
        let epochen: Vec<u64> =
            state.account(&a).verstoesse.iter().map(|v| v.epoche.0).collect();
        let mut sortiert = epochen.clone();
        sortiert.sort_unstable();
        assert_eq!(epochen, sortiert, "die Historie steht nicht aufsteigend");
    }

    #[test]
    fn kontenordnung_ist_unabhaengig_von_einfuegereihenfolge() {
        // BTreeMap: dieselbe Zielmenge ergibt dieselbe Serialisierung,
        // egal in welcher Reihenfolge die Konten angelegt wurden.
        let mut aufsteigend = LedgerState::genesis(100);
        for b in [1u8, 2, 3] {
            aufsteigend.account_mut(&adresse(b)).balance = b as u64;
        }
        let mut absteigend = LedgerState::genesis(100);
        for b in [3u8, 2, 1] {
            absteigend.account_mut(&adresse(b)).balance = b as u64;
        }
        assert_eq!(aufsteigend.commitment(), absteigend.commitment());
    }
}
