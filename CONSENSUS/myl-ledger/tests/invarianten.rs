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

use myl_ledger::transitions::praegen;
use myl_ledger::{
    apply_verdict, burn_to_credits, credit_spend, transfer, LedgerState, SlashParams, Verdict,
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

/// **Invariante 1: MYL entsteht nicht aus dem Nichts, und wo doch, ist
/// die Quelle benannt.**
///
/// `burn_to_credits` verbrennt, `apply_verdict` schlachtet und verteilt
/// einen Teil weiter, `transfer` verschiebt. Genau ein Übergang erzeugt
/// MYL, nämlich `praegen`. Die Summe aus Guthaben und Stake darf deshalb
/// **nur durch Prägung steigen, und genau um den geprägten Betrag**.
///
/// Das ist die Invariante, deren Verletzung am teuersten wäre: Ein
/// Übergang, der Geld erzeugt, ist ein Loch in der Geldmenge.
///
/// # ⚑ Bis zum 2026-09-02 stand hier die halbe Aussage (Fund 136)
///
/// Der Test hieß `myl_steigt_niemals`, und der Kopf darüber behauptete:
/// „Kein Übergang in diesem Crate prägt MYL." **Das stimmte, als der
/// Test geschrieben wurde, und seit Punkt 38 nicht mehr:** `praegen`
/// gibt es seither, und der Knoten ruft es beim Epochenabschluss.
///
/// ⚑ **Aufgefallen ist es nicht an der Behauptung, sondern an der
/// Auswahl.** Der Zufallslauf würfelte über **drei** Übergänge, während
/// `myl-ledger` deren achtzehn kennt, und die beiden, die MYL bewegen
/// und erzeugen, waren nicht dabei. Die Invariante galt damit über eine
/// Zustandsmaschine, die ein Fünftel der echten war.
///
/// Der Satz auf der Beweisliste lautet vollständig: die Summe bleibt
/// gleich **oder die Quelle ist benannt**. Geprüft wurde die erste
/// Hälfte, über eine Menge, in der die zweite gar nicht vorkommen
/// konnte. Jetzt beide, und die zweite verlangt zusätzlich, dass der
/// Zuwachs **genau** dem geprägten Betrag entspricht.
#[test]
fn myl_steigt_nur_durch_praegung_und_genau_um_sie() {
    let mut gepraegte_schritte = 0usize;
    let mut gesunkene_schritte = 0usize;

    for keim in 1..=25u64 {
        let mut w = Wuerfel::neu(keim);
        let mut state = startzustand();
        let mut vorher = myl_gesamt(&state);

        for schritt in 0..200 {
            let s = zufaelliger_uebergang(&mut w, &mut state);
            let nachher = myl_gesamt(&state);

            if s.gepraegt > 0 {
                gepraegte_schritte += 1;
                // ⚑ Die Quelle ist benannt, **und die Zahl muss stimmen**.
                // Ein blosses „es wurde ja gepraegt" waere eine Ausrede,
                // unter der jeder Betrag durchginge.
                assert_eq!(
                    nachher,
                    vorher + s.gepraegt,
                    "Keim {keim}, Schritt {schritt}: gepraegt wurden {}, gestiegen ist die \
                     Menge um {}",
                    s.gepraegt,
                    nachher as i128 - vorher as i128
                );
            } else {
                assert!(
                    nachher <= vorher,
                    "Keim {keim}, Schritt {schritt}: MYL stieg von {vorher} auf {nachher}, \
                     ohne dass eine Quelle benannt war"
                );
                if nachher < vorher {
                    gesunkene_schritte += 1;
                }
            }
            vorher = nachher;
        }
    }

    println!(
        "[invarianten] {gepraegte_schritte} Schritte mit Praegung, \
         {gesunkene_schritte} mit Rueckgang"
    );
    // ⚑ Beide Zahlen muessen ueber null liegen. Ohne Praegung prueft die
    // zweite Haelfte der Invariante nie, ohne Rueckgang bewegt die Folge
    // nichts.
    assert!(
        gepraegte_schritte > 0,
        "kein einziger Schritt hat gepraegt; dann prueft dieser Test die benannte \
         Quelle nie"
    );
    assert!(
        gesunkene_schritte > 0,
        "die Menge ist nie gesunken; dann bewegt die Folge nichts"
    );
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
/// Wie viel MYL der letzte Übergang **erklärtermaßen** erzeugt hat.
///
/// ⚑ **Das ist die Hälfte der Invariante, die bis zum 2026-09-02
/// fehlte.** Der Satz auf der Beweisliste lautet „die Summe bleibt
/// gleich **oder die Quelle ist benannt**"; geprüft wurde nur die erste
/// Hälfte, und zwar über eine Übergangsmenge, in der es gar keine Quelle
/// gab.
#[derive(Debug, Default, PartialEq, Eq)]
struct Schritt {
    /// Betrag, den dieser Übergang neu geprägt hat. Null bei allen
    /// übrigen.
    gepraegt: u128,
}

/// Ein zufälliger Übergang, und er nennt seine Quelle.
///
/// ⚑ **Die Auswahl war der Fund (2026-09-02).** Diese Funktion würfelte
/// bis dahin über **drei** Übergänge plus einen direkten Schreibzugriff
/// auf die Epoche, während `myl-ledger` deren **achtzehn** kennt.
/// Ausgerechnet die beiden, die MYL **bewegen** und **erzeugen**,
/// `transfer` und `praegen`, waren nicht dabei. Die Invariante „MYL
/// steigt niemals" galt damit über eine Zustandsmaschine, die ein
/// Fünftel der echten war, und über eine, in der Prägung nicht vorkam.
///
/// Dieselbe Klasse wie überall hier: nicht die Behauptung war falsch,
/// sondern die **Auswahl**. Eine Prüfung, die nichts auswählt, sieht aus
/// wie eine, die nichts findet.
fn zufaelliger_uebergang(w: &mut Wuerfel, state: &mut LedgerState) -> Schritt {
    let addr = adresse((w.bis(4) + 1) as u8);
    match w.bis(6) {
        0 => {
            let _ = burn_to_credits(state, &addr, w.bis(5000), EpochId(w.bis(20)));
            Schritt::default()
        }
        1 => {
            let _ = credit_spend(state, &addr, w.bis(500));
            Schritt::default()
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
            Schritt::default()
        }
        3 => {
            // Überweisung: verschiebt MYL, erzeugt keines. Der
            // Empfänger wird bewusst auch mal derselbe sein, denn
            // `transfer` lehnt das ab, und ein abgelehnter Übergang
            // gehört ebenfalls in die Folge.
            let nach = adresse((w.bis(4) + 1) as u8);
            let _ = transfer(state, &addr, &nach, w.bis(3000));
            Schritt::default()
        }
        4 => {
            // ⚑ Der einzige Übergang, der MYL **erzeugt**, und deshalb
            // der einzige, der etwas zurückmeldet.
            let betrag = w.bis(2000);
            match praegen(state, &addr, betrag) {
                Ok(()) => Schritt {
                    gepraegt: betrag as u128,
                },
                Err(_) => Schritt::default(),
            }
        }
        _ => {
            state.epoch = EpochId(w.bis(50));
            Schritt::default()
        }
    }
}
