//! Invarianten des Ledgers über zufällige Übergangsfolgen (Kritikpunkt K4).
//!
//! K4 lautet: *„Die Tests belegen überwiegend den Erfolgsfall."* Genau
//! das trifft auf `determinism.rs` zu: Es prüft, dass zwei Läufe
//! derselben Folge denselben Zustand ergeben, und das ist richtig und
//! wichtig, sagt aber **nichts darüber, ob der Zustand stimmt**. Zwei
//! Läufe derselben falschen Rechnung sind ebenso bitgleich.
//!
//! Diese Datei prüft deshalb nicht Gleichheit, sondern **Eigenschaften,
//! die nach jedem Übergang gelten müssen**, und zwar über Folgen, die
//! niemand von Hand ausgesucht hat.
//!
//! ## Warum ein eigener Würfel und keine Property-Test-Bibliothek
//!
//! `proptest` und `quickcheck` wären hier bequem. Beide sind aber eine
//! weitere Abhängigkeit in einem Crate, das den Konsens rechnet, und die
//! Kosten trägt jeder Teilnehmer, der das Repositorium baut. Ein
//! xorshift64 in zehn Zeilen leistet dasselbe, solange die Folge
//! **reproduzierbar** ist: Ein Test, der bei jedem Lauf andere Zahlen
//! zieht, meldet einen Fehler einmal und danach nie wieder.
//!
//! Was dabei verlorengeht, ist das automatische Verkleinern eines
//! Gegenbeispiels. Dafür nennt jeder Fehlschlag hier den Keim und den
//! Schritt, und damit ist der Fall von Hand nachzustellen.

use myl_ledger::{
    apply_verdict, burn_to_credits, credit_spend, LedgerState, SlashParams, Verdict,
    VerdictOutcome,
};
use myl_types::ids::{Address, EpochId, SegmentId};

const CREDIT_PREIS: u64 = 10;

fn adresse(byte: u8) -> Address {
    Address::new([byte; 32])
}

fn params() -> SlashParams {
    SlashParams {
        slash_fraction_num: 1,
        slash_fraction_den: 3,
        bounty_fraction_num: 1,
        bounty_fraction_den: 2,
    }
}

/// xorshift64, reproduzierbar und ohne Abhängigkeit.
struct Wuerfel(u64);

impl Wuerfel {
    fn neu(keim: u64) -> Self {
        Self(keim | 1)
    }
    fn naechste(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn bis(&mut self, n: u64) -> u64 {
        self.naechste() % n
    }
}

/// Summe aus Guthaben und Stake über alle Konten.
fn myl_gesamt(state: &LedgerState) -> u128 {
    state
        .accounts
        .values()
        .map(|a| a.balance as u128 + a.staked as u128)
        .sum()
}

/// Summe aller offenen Credits über alle Konten.
fn credits_gesamt(state: &LedgerState) -> u128 {
    state
        .accounts
        .values()
        .flat_map(|a| a.credits.iter())
        .map(|c| c.vtfe as u128)
        .sum()
}

/// Ein Ledger mit Startguthaben, damit die Übergänge etwas zu tun haben.
fn startzustand() -> LedgerState {
    let mut state = LedgerState::genesis(CREDIT_PREIS);
    for b in 1..=4u8 {
        let konto = state.account_mut(&adresse(b));
        konto.balance = 1_000_000;
        konto.staked = 500_000;
    }
    state
}

/// **Invariante 1: MYL entsteht nicht aus dem Nichts.**
///
/// Kein Übergang in diesem Crate prägt MYL. `burn_to_credits` verbrennt,
/// `apply_verdict` schlachtet und verteilt einen Teil davon weiter, der
/// Rest bleibt unverteilt. Die Summe aus Guthaben und Stake darf deshalb
/// **nie steigen**.
///
/// Das ist die Invariante, deren Verletzung am teuersten wäre: Ein
/// Übergang, der Geld erzeugt, ist ein Loch in der Geldmenge.
#[test]
fn myl_steigt_niemals() {
    for keim in 1..=25u64 {
        let mut w = Wuerfel::neu(keim);
        let mut state = startzustand();
        let mut vorher = myl_gesamt(&state);

        for schritt in 0..200 {
            zufaelliger_uebergang(&mut w, &mut state);
            let nachher = myl_gesamt(&state);
            assert!(
                nachher <= vorher,
                "Keim {keim}, Schritt {schritt}: MYL stieg von {vorher} auf {nachher}"
            );
            vorher = nachher;
        }
    }
}

/// **Invariante 2: Credits sind durch verbranntes MYL gedeckt.**
///
/// `burn_to_credits` prägt `floor(syn / preis)` Credits gegen `syn`
/// verbranntes MYL. Über eine ganze Folge muss deshalb gelten:
/// `Credits · Preis ≤ verbranntes MYL`. Die Abrundung geht zu Lasten des
/// Käufers, also nie zu Lasten der Deckung.
#[test]
fn credits_sind_durch_verbranntes_myl_gedeckt() {
    for keim in 1..=25u64 {
        let mut w = Wuerfel::neu(keim);
        let mut state = startzustand();
        let start_myl = myl_gesamt(&state);

        for _ in 0..200 {
            zufaelliger_uebergang(&mut w, &mut state);
        }

        // Alles, was an MYL verschwunden ist, ging entweder in Credits
        // oder in unverteilte Slash-Reste. Die Credits allein dürfen den
        // Schwund also nicht übersteigen.
        let geschwunden = start_myl - myl_gesamt(&state);
        let gedeckt = credits_gesamt(&state) * CREDIT_PREIS as u128;
        assert!(
            gedeckt <= geschwunden,
            "Keim {keim}: {gedeckt} an Credits gegen nur {geschwunden} verbranntes MYL"
        );
    }
}

/// **Invariante 3: Ein abgelehnter Übergang ändert nichts.**
///
/// Jeder Übergang prüft erst und ändert dann. Schlägt die Prüfung fehl,
/// muss der Zustand **bitgleich** bleiben, und nicht nur „im
/// Wesentlichen": Ein halb angewendeter Übergang wäre ein Konsensbruch,
/// weil zwei Knoten ihn an verschiedenen Stellen abbrechen könnten.
///
/// Genau das ist der Fall, den K4 vermisst: Hier wird ausschließlich der
/// **Fehlschlag** geprüft.
#[test]
fn ein_abgelehnter_uebergang_laesst_den_zustand_bitgleich() {
    let mut state = startzustand();
    let a = adresse(1);

    /// Ein benannter Fall, der fehlschlagen MUSS.
    type Fall = (&'static str, Box<dyn Fn(&mut LedgerState)>);

    let faelle: Vec<Fall> = vec![
        ("Burn ohne Deckung", Box::new(|s: &mut LedgerState| {
            assert!(burn_to_credits(s, &adresse(1), 999_999_999, EpochId(9)).is_err());
        })),
        ("Burn über null", Box::new(|s: &mut LedgerState| {
            assert!(burn_to_credits(s, &adresse(1), 0, EpochId(9)).is_err());
        })),
        ("Spend ohne Credits", Box::new(|s: &mut LedgerState| {
            assert!(credit_spend(s, &adresse(4), 1_000_000).is_err());
        })),
        ("Spend über null", Box::new(|s: &mut LedgerState| {
            assert!(credit_spend(s, &adresse(1), 0).is_err());
        })),
        ("Verdict mit unbrauchbaren Parametern", Box::new(|s: &mut LedgerState| {
            let kaputt = SlashParams {
                slash_fraction_num: 5,
                slash_fraction_den: 3, // Anteil > 1
                bounty_fraction_num: 1,
                bounty_fraction_den: 2,
            };
            let v = Verdict {
                segment_id: SegmentId::new([7u8; 32]),
                miner: adresse(1),
                checker: adresse(2),
                outcome: VerdictOutcome::SlashMiner,
            };
            assert!(apply_verdict(s, &v, &kaputt).is_err());
        })),
    ];

    // Etwas Vorgeschichte, damit nicht der leere Zustand geprüft wird.
    burn_to_credits(&mut state, &a, 1000, EpochId(9)).expect("Vorgeschichte");

    for (name, tun) in faelle {
        let vorher = state.commitment();
        tun(&mut state);
        assert_eq!(
            vorher,
            state.commitment(),
            "{name}: der abgelehnte Übergang hat den Zustand verändert"
        );
    }
}

/// **Invariante 4: Das Kopfgeld übersteigt nie den geschlachteten Betrag.**
///
/// Sonst zahlte das Protokoll mehr aus, als es eingezogen hat, und die
/// Differenz käme aus dem Nichts.
#[test]
fn das_kopfgeld_uebersteigt_nie_den_slash() {
    for keim in 1..=20u64 {
        let mut w = Wuerfel::neu(keim);
        let mut state = startzustand();

        for schritt in 0..100 {
            let v = Verdict {
                segment_id: SegmentId::new([(schritt % 251) as u8; 32]),
                miner: adresse((w.bis(4) + 1) as u8),
                checker: adresse((w.bis(4) + 1) as u8),
                outcome: if w.bis(2) == 0 {
                    VerdictOutcome::SlashMiner
                } else {
                    VerdictOutcome::SlashChecker
                },
            };
            if let Ok(effekt) = apply_verdict(&mut state, &v, &params()) {
                assert!(
                    effekt.bounty <= effekt.slashed,
                    "Keim {keim}, Schritt {schritt}: Kopfgeld {} > Slash {}",
                    effekt.bounty,
                    effekt.slashed
                );
            }
        }
    }
}

/// **Invariante 5: Guthaben und Stake bleiben in u64, ohne Umlauf.**
///
/// Ein Umlauf wäre im Debug-Build eine Panik und im Release-Build eine
/// stille Falschbuchung; zwei Knoten mit verschiedenen Bauprofilen kämen
/// zu verschiedenen Zuständen. Geprüft wird deshalb gegen absurde
/// Beträge nahe der Bereichsgrenze.
#[test]
fn extreme_betraege_laufen_nicht_um() {
    let mut state = LedgerState::genesis(1);
    let a = adresse(1);
    state.account_mut(&a).balance = u64::MAX;
    state.account_mut(&a).staked = u64::MAX;

    // Ein Burn über den vollen Bereich: Credits = syn / 1 = u64::MAX.
    let vorher = myl_gesamt(&state);
    let ergebnis = burn_to_credits(&mut state, &a, u64::MAX, EpochId(9));
    assert!(ergebnis.is_ok(), "voller Bereich muss buchbar sein");
    assert!(
        myl_gesamt(&state) <= vorher,
        "auch am Bereichsende darf MYL nicht steigen"
    );

    // Und die Gegenprobe: mehr ausgeben, als da ist.
    assert!(credit_spend(&mut state, &a, u64::MAX).is_ok());
    assert!(credit_spend(&mut state, &a, 1).is_err(), "leer ist leer");
}

/// Ein zufälliger, aber reproduzierbarer Übergang.
fn zufaelliger_uebergang(w: &mut Wuerfel, state: &mut LedgerState) {
    let addr = adresse((w.bis(4) + 1) as u8);
    match w.bis(4) {
        0 => {
            let _ = burn_to_credits(state, &addr, w.bis(5000), EpochId(w.bis(20)));
        }
        1 => {
            let _ = credit_spend(state, &addr, w.bis(500));
        }
        2 => {
            let v = Verdict {
                segment_id: SegmentId::new([(w.bis(251)) as u8; 32]),
                miner: addr,
                checker: adresse((w.bis(4) + 1) as u8),
                outcome: if w.bis(2) == 0 {
                    VerdictOutcome::SlashMiner
                } else {
                    VerdictOutcome::SlashChecker
                },
            };
            let _ = apply_verdict(state, &v, &params());
        }
        _ => {
            state.epoch = EpochId(w.bis(50));
        }
    }
}
