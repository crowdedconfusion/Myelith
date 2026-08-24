//! Der vollständige Protokolldurchlauf und was er findet.

use myl_simulation::{Protokolllauf, Schwere};

/// **Der ehrliche Durchlauf: keine schweren Befunde.**
///
/// Die Gegenprobe zu allem darunter. Eine Simulation, die immer meldet,
/// meldet nichts.
#[test]
fn der_ehrliche_durchlauf_meldet_nichts_schweres() {
    let mut lauf = Protokolllauf::neu(8, 0).expect("Netz");

    let pods = lauf.naht_scheduler();
    assert!(pods.len() >= 2, "acht Teilnehmer müssen mindestens zwei Pods ergeben");

    let vergleich = lauf.naht_verifikation(true);
    assert_eq!(vergleich, myl_verifier::CompareResult::Match);

    let _ = lauf.naht_praegung(50);

    for b in lauf.bericht() {
        eprintln!("  [{:?}] {}: {}", b.schwere, b.stelle, b.beobachtung);
    }
    assert!(
        !lauf.schwerer_befund(),
        "der ehrliche Durchlauf darf keinen schweren Befund erzeugen"
    );
}

/// **Ein falsch rechnender Pod wird gefunden und gebucht.**
///
/// Die Kette Verifikation → Urteil → Ledger, an einem Stück.
#[test]
fn ein_falscher_pod_wird_gefunden_und_gebucht() {
    let mut lauf = Protokolllauf::neu(6, 2).expect("Netz");

    // Stufe 1 findet die Abweichung an der richtigen Stelle.
    let vergleich = lauf.naht_verifikation(false);
    assert_eq!(
        vergleich,
        myl_verifier::CompareResult::Mismatch { first_divergence: 9 },
        "der Vergleich muss die erste abweichende Position nennen"
    );

    // Der Schuldspruch wird gebucht.
    let schuldig = lauf.teilnehmer[6];
    let checker = lauf.teilnehmer[0];
    let stake_vorher = lauf.ledger.account(&schuldig.adresse).staked;
    let effekt = lauf.naht_slashing(&schuldig, &checker).expect("Buchung");

    assert_eq!(
        effekt.slashed, stake_vorher,
        "falsches Ergebnis kostet 100 % des Stakes (Kap. 5.5)"
    );
    assert!(effekt.bounty <= effekt.slashed, "Kopfgeld nie über dem Slash");
    assert_eq!(
        lauf.ledger.account(&schuldig.adresse).staked,
        0,
        "der Stake muss weg sein"
    );
    assert!(
        lauf.ledger.account(&checker.adresse).balance > 0,
        "der Checker bekommt sein Kopfgeld"
    );

    for b in lauf.bericht() {
        eprintln!("  [{:?}] {}: {}", b.schwere, b.stelle, b.beobachtung);
    }
    assert!(!lauf.schwerer_befund());
}

/// **Die Prägekurve über 200 Epochen, mit dem Burn-Cap.**
///
/// Der Cap aus Kap. 5.6 ist seit dem 2026-08-24 implementiert; hier läuft
/// er zum ersten Mal in einer Kette mit EMA, Prägung und Verteilung.
#[test]
fn die_praegekurve_bleibt_ueber_zweihundert_epochen_stimmig() {
    let mut lauf = Protokolllauf::neu(4, 0).expect("Netz");
    let zahlen = lauf.naht_praegung(200);

    eprintln!(
        "  geprägt {}, verbrannt {}, EMA {}",
        zahlen["gepraegt"], zahlen["verbrannt"], zahlen["ema"]
    );
    for b in lauf.bericht() {
        eprintln!("  [{:?}] {}: {} → {}", b.schwere, b.stelle, b.beobachtung, b.folge);
    }
    // Verteilt wird stets genau das Geprägte; ein Verstoß wäre ein
    // schwerer Befund und steht dann oben.
    assert!(!lauf.schwerer_befund());
    assert!(zahlen["ema"] > 0, "nach 200 Epochen Burn muss die EMA stehen");
}

/// **Ein Netz, das zu klein für Redundanz ist, meldet das.**
///
/// Zwei Teilnehmer ergeben einen Pod. Ohne zweiten Pod gibt es keinen
/// Vergleich, und Stufe 1 der Verifikation entfällt — das darf nicht
/// stillschweigend geschehen.
#[test]
fn ein_zu_kleines_netz_meldet_die_fehlende_redundanz() {
    let mut lauf = Protokolllauf::neu(2, 0).expect("Netz");
    let pods = lauf.naht_scheduler();
    assert!(pods.len() < 2);
    assert!(
        lauf.bericht().iter().any(|b| b.schwere == Schwere::Luecke),
        "die fehlende Redundanz muss gemeldet werden"
    );
}

/// **Der Verbrauchs-Stoß mit Ausstieg — der offene Punkt aus K8.**
///
/// Die K8-Simulation zeigte: Wer den Verbrauch hochtreibt und dann
/// aussteigt, lässt eine Prägung zurück, die der EMA nachläuft. Zwischen
/// Epoche 100 und 125 wuchs der Umlauf dort von 282 auf 30 222 MYL.
///
/// Seit dem 2026-08-24 gibt es den Burn-Cap je Adresse (Kap. 5.6). Hier
/// läuft er zum ersten Mal gegen den Angriff, für den er gedacht ist.
///
/// **Vorher aufgefallen:** Der Durchlauf oben erreicht den Cap **nie**.
/// Er greift ab 1000 MYL geglättetem Burn, und die EMA nähert sich diesem
/// Wert von unten, ohne ihn zu überschreiten. Ein Test, der den
/// interessanten Zweig nicht betritt, prüft ihn nicht — dieselbe Falle
/// wie beim ersten Pod-Fuzzer.
#[test]
fn der_burn_cap_bremst_den_verbrauchs_stoss() {
    use myl_tokenomics::{burn_spielraum, ema_update, UNITS_PER_MYL};

    // Ein Netz im Betrieb: die EMA steht bei 20 000 MYL.
    let ema = 20_000 * UNITS_PER_MYL;
    let deckel = burn_spielraum(ema, 0);
    assert!(
        deckel < u64::MAX,
        "bei 20 000 MYL geglättetem Burn muss der Deckel greifen"
    );
    assert_eq!(deckel, 1_000 * UNITS_PER_MYL, "ein Zwanzigstel");

    // Der Angreifer will das Zehnfache der EMA verbrennen.
    let wunsch = 200_000 * UNITS_PER_MYL;
    let tatsaechlich = wunsch.min(deckel);
    assert_eq!(
        tatsaechlich,
        1_000 * UNITS_PER_MYL,
        "der Stoß wird auf ein Zwanzigstel der EMA gestutzt"
    );

    // **Was der Deckel leistet, in Zahlen:** Ohne ihn hebt der Stoß die
    // EMA in einem Schritt spürbar; mit ihm kaum.
    let ohne = ema_update(ema, wunsch);
    let mit = ema_update(ema, tatsaechlich);
    let hub_ohne = ohne.saturating_sub(ema);
    let hub_mit = ema.saturating_sub(mit);
    eprintln!(
        "  EMA {} → ohne Deckel {} (+{}), mit Deckel {} (−{})",
        ema / UNITS_PER_MYL,
        ohne / UNITS_PER_MYL,
        hub_ohne / UNITS_PER_MYL,
        mit / UNITS_PER_MYL,
        hub_mit / UNITS_PER_MYL
    );
    assert!(
        ohne > ema && mit <= ema,
        "ohne Deckel hebt der Stoß die EMA, mit Deckel nicht"
    );

    // **Und was er nicht leistet, gehört dazu:** Zwanzig Adressen mit je
    // eigener Deckung erreichen denselben Stoß. Der Deckel macht daraus
    // eine Kapitalfrage, keine Sybil-Frage — die MYL müssen wirklich da
    // sein.
    let zwanzig = 20u64 * deckel;
    assert!(
        zwanzig >= ema,
        "zwanzig gedeckelte Adressen erreichen wieder die volle EMA; \
         der Deckel begrenzt den Einzelnen, nicht das Kartell"
    );
}
