//! Fünf Knoten, eine BFT-Runde, ein Block.
//!
//! # Was dieser Test prüft, das kein Modultest prüfen kann
//!
//! `konsens.rs` fährt eine Runde über eine Nachrichtenschlange. Das ist
//! die Protokolllogik, und sie ist dort vollständig geprüft. Was dort
//! **nicht** geprüft werden kann, ist alles, was zwischen zwei Prozessen
//! liegt:
//!
//! - Ob die Nachricht ein Topic hat, das beide abonniert haben.
//! - Ob sie durch die Nutzlastprüfung der Netzschicht kommt.
//! - Ob Gossipsub sie überhaupt annimmt, oder ob das Mesh noch nicht
//!   steht.
//! - Ob ein Knoten seine **eigene** Stimme mitzählt, obwohl Gossipsub
//!   sie ihm nicht zurückschickt.
//!
//! Der letzte Punkt ist der Grund, warum dieser Test existiert: Er ist
//! über eine Schlange nicht zu prüfen, weil die Schlange sich anders
//! verhält als ein Netz.
//!
//! # ⚑ Warum fünf und nicht vier
//!
//! Die Ein-Drittel-Schranke der Stimmsatzdatei verlangt **mindestens
//! vier** Validatoren: Drei Werte unter je einem Drittel ergeben nie
//! ihre eigene Summe.
//!
//! Bei genau vier ist der interessante Fall aber nicht konstruierbar.
//! Damit drei von vier das Quorum verfehlen, müsste der ausgeschlossene
//! Vierte mehr als ein Drittel halten. Deshalb fünf, mit der
//! Verteilung 250/230/200/120/100, in der drei Köpfe je nach Gewicht
//! das Quorum erreichen **oder** verfehlen.
//!
//! # Was dieser Test nicht prüft
//!
//! **Keinen Ausfall.** Fällt der Leader aus, hängt die Runde, weil
//! `Konsensrunde` noch keinen Rundenwechsel fährt. Das ist der nächste
//! Punkt und braucht eine Uhr, also eine Entscheidung über GST.

use std::path::PathBuf;
use std::time::Duration;

use myl_node::stimmsatzdatei::Stimmsatzdatei;
use myl_node::konfig::{KnotenKonfig, Rolle};
use myl_node::schluessel::Konsensschluessel;
use myl_node::Knoten;
use myl_net::GossipTopic;
use myl_consensus::round_change::TimeoutConfig;
use myl_types::hash::Hash;

/// Fristen für die Tests.
///
/// **Großzügiger als die Vorgabe**, damit ein Rundenwechsel nicht
/// deshalb auftritt, weil der Testrahmen die Knoten reihum fährt statt
/// nebenläufig. Wo ein Wechsel gemessen werden soll, setzt der Test
/// eigene, knappe Fristen.
fn timeouts() -> TimeoutConfig {
    TimeoutConfig {
        propose_ms: 30_000,
        vote_ms: 30_000,
        commit_ms: 30_000,
        delta_ms: 5_000,
    }
}

/// Die fünf Teilnehmer und ihr Stake in MYL-Kleinstbeträgen.
///
/// **Konstruiert, nicht gegriffen.** Summe 900 MYL, Schwelle
/// `⌊2·900/3⌋ + 1` = 600 000 001. Drei Köpfe liegen je nach Auswahl
/// darunter (200+120+100 = 420), genau darauf (250+230+120 = 600) oder
/// darüber (250+230+200 = 680). Die Begründung steht in
/// `stimmsatzdatei.rs::die_verteilung_legt_drei_grenzfaelle_aus`.
const TEILNEHMER: [(&str, u64); 5] = [
    ("alpha", 250_000_000),
    ("beta", 230_000_000),
    ("gamma", 200_000_000),
    ("delta", 120_000_000),
    ("epsilon", 100_000_000),
];

fn arbeitsverzeichnis(marke: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "myl-bft-{marke}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn stimmsatz() -> Stimmsatzdatei {
    let mut text = String::from("netz myelith-probenetz-1\n");
    for (name, stake) in TEILNEHMER {
        let k = Konsensschluessel::probe(name).expect("Probeschlüssel");
        text.push_str(&k.genesiszeile(stake).expect("Stimmsatzdateizeile"));
        text.push('\n');
    }
    Stimmsatzdatei::aus_text(&text).expect("Stimmsatzdatei muss lesbar sein")
}

fn konfig(verzeichnis: &std::path::Path, name: &str, bootstrap: Vec<String>) -> KnotenKonfig {
    KnotenKonfig {
        name: name.to_string(),
        schluesseldatei: verzeichnis.join(format!("{name}.key")),
        protokollverzeichnis: verzeichnis.join("logs"),
        horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
        bootstrap,
        rolle: Rolle::Teilnehmer,
        kettendatei: None,
        stimmsatzdatei_pfad: None,
        konsensschluesseldatei: None,
        nat: Default::default(),
        aufnahme_sekunden: 1,
        // Kein Endpunkt: Ein fester Port kollidierte, sobald zwei
        // Testknoten nebeneinander laufen, und dieser Test prueft
        // ohnehin etwas anderes.
        beobachtung: None,
        // Dieselbe Begruendung: ein fester Port kollidiert, sobald zwei
        // Knoten nebeneinander laufen.
        tuer: None,
        ortsleitung: None,
        ortsausweis: None,
        testverkehr_sekunden: None,
        erzeugt_bloecke: false,
        teilnehmer: TEILNEHMER.iter().map(|(n, _)| n.to_string()).collect(),
    }
}

/// Fährt alle Knoten reihum für je eine Zeitscheibe.
///
/// **Reihum und nicht nebenläufig**, und das ist unbedenklich: Der Swarm
/// jedes Knotens läuft in einem eigenen Task weiter, auch während der
/// Knoten selbst gerade keine Ereignisse abholt. Sie sammeln sich im
/// Kanal und werden in der nächsten Scheibe verarbeitet.
async fn reihum(knoten: &mut [Knoten], runden: usize, scheibe: Duration) {
    for _ in 0..runden {
        for k in knoten.iter_mut() {
            k.laufe_fuer(scheibe).await;
        }
    }
}

/// Wartet, bis jeder Knoten für das Konsens-Topic ein Mesh hat.
///
/// ⚑ **Verbunden heißt nicht im Mesh.** Wer vor dem Mesh publiziert,
/// bekommt von Gossipsub ein „zu wenige Peers" zurück, und die Nachricht
/// ist weg. Beim Propose des Leaders wäre das das Ende der Runde, und im
/// Protokoll sähe es aus, als hätte niemand geantwortet.
async fn warte_auf_mesh(knoten: &mut [Knoten], frist: Duration) -> Vec<usize> {
    let ende = tokio::time::Instant::now() + frist;
    let mut groessen = vec![0usize; knoten.len()];
    while tokio::time::Instant::now() < ende {
        reihum(knoten, 1, Duration::from_millis(120)).await;
        for (i, k) in knoten.iter_mut().enumerate() {
            let z = k.zustand().await;
            groessen[i] = z
                .mesh
                .iter()
                .find(|(t, _)| *t == GossipTopic::Consensus)
                .map(|(_, n)| *n)
                .unwrap_or(0);
        }
        if groessen.iter().all(|n| *n >= 1) {
            return groessen;
        }
    }
    groessen
}

async fn starte_netz(verzeichnis: &std::path::Path) -> Vec<Knoten> {
    let mut knoten = Vec::new();
    // Der erste horcht, die übrigen steigen bei ihm ein. Ein Stern
    // genügt: Gossipsub leitet weiter, und genau das soll der Test
    // belasten.
    let mut erster = Knoten::starten(konfig(verzeichnis, TEILNEHMER[0].0, vec![]), false)
        .await
        .expect("erster Knoten");
    let adresse = erster
        .warte_auf_adresse(Duration::from_secs(5))
        .await
        .expect("Horchadresse des ersten Knotens");
    let bootstrap = vec![format!("{}/p2p/{}", adresse, erster.peer_id())];
    knoten.push(erster);

    for (name, _) in TEILNEHMER.iter().skip(1) {
        let k = Knoten::starten(konfig(verzeichnis, name, bootstrap.clone()), false)
            .await
            .expect("weiterer Knoten");
        knoten.push(k);
    }
    knoten
}

#[tokio::test]
async fn fuenf_knoten_commiten_denselben_block() {
    let verzeichnis = arbeitsverzeichnis("commit");
    let g = stimmsatz();
    let mut knoten = starte_netz(&verzeichnis).await;

    let mesh = warte_auf_mesh(&mut knoten, Duration::from_secs(20)).await;
    assert!(
        mesh.iter().all(|n| *n >= 1),
        "kein vollständiges Mesh auf {:?}: {mesh:?}",
        GossipTopic::Consensus
    );

    // Alle beginnen dieselbe Runde mit demselben Vorschlag. Der Hash
    // steht hier stellvertretend für einen Block; was er bezeichnet,
    // entscheidet die Kette, deren Persistenz ein eigener Punkt ist.
    let vorschlag = Hash::sha256(b"myelith testblock runde 0");
    for (i, (name, _)) in TEILNEHMER.iter().enumerate() {
        let schluessel = Konsensschluessel::probe(name).expect("Schlüssel");
        knoten[i]
            .beginne_konsensrunde(&g, schluessel, vorschlag, timeouts())
            .await
            .expect("Runde beginnen");
    }

    // Genug Scheiben für Propose, Vote und Commit über den Stern hinweg.
    for _ in 0..40 {
        reihum(&mut knoten, 1, Duration::from_millis(150)).await;
        if knoten
            .iter()
            .all(|k| k.konsens().map(|r| r.ist_commitet()).unwrap_or(false))
        {
            break;
        }
    }

    for (i, (name, _)) in TEILNEHMER.iter().enumerate() {
        let r = knoten[i].konsens().expect("laufende Runde");
        let (stimmen, commits, schwelle) = r.gewichte();
        println!(
            "[{name}] commitet={} stimmgewicht={stimmen} commitgewicht={commits} schwelle={schwelle}",
            r.ist_commitet()
        );
        assert!(
            r.ist_commitet(),
            "{name} hat nicht commitet: Stimmen {stimmen}, Commits {commits}, Schwelle {schwelle}"
        );
        assert_eq!(
            r.commiteter_block(),
            Some(vorschlag),
            "{name} commitete einen anderen Block"
        );
    }

    // Alle rechnen denselben Leader. Rechneten sie verschiedene,
    // verwürfe jeder den Propose des anderen, und die Runde bliebe
    // stehen, ohne dass jemand etwas Falsches getan hätte.
    let leader: Vec<_> = knoten
        .iter()
        .map(|k| k.konsens().unwrap().leader())
        .collect();
    assert!(
        leader.windows(2).all(|w| w[0] == w[1]),
        "die Knoten rechneten verschiedene Leader"
    );

    std::fs::remove_dir_all(&verzeichnis).ok();
}

/// ⚑ **Die eigene Stimme über das echte Netz.**
///
/// Gossipsub schickt einem Knoten seine eigene Veröffentlichung nicht
/// zurück. Ein Knoten, der nur veröffentlicht, käme also nie über
/// `n-1` Stimmen hinaus. Hier wird gemessen, dass das **volle**
/// Stimmgewicht ankommt, also 900 von 900, und nicht 900 minus das
/// eigene.
#[tokio::test]
async fn jeder_knoten_zaehlt_auch_sein_eigenes_gewicht() {
    let verzeichnis = arbeitsverzeichnis("eigengewicht");
    let g = stimmsatz();
    let gesamt = g.gesamtstake();
    let mut knoten = starte_netz(&verzeichnis).await;
    let mesh = warte_auf_mesh(&mut knoten, Duration::from_secs(20)).await;
    assert!(mesh.iter().all(|n| *n >= 1), "kein Mesh: {mesh:?}");

    let vorschlag = Hash::sha256(b"eigengewicht");
    for (i, (name, _)) in TEILNEHMER.iter().enumerate() {
        let schluessel = Konsensschluessel::probe(name).expect("Schlüssel");
        knoten[i]
            .beginne_konsensrunde(&g, schluessel, vorschlag, timeouts())
            .await
            .expect("Runde beginnen");
    }
    for _ in 0..40 {
        reihum(&mut knoten, 1, Duration::from_millis(150)).await;
        if knoten
            .iter()
            .all(|k| k.konsens().map(|r| r.gewichte().0).unwrap_or(0) == gesamt)
        {
            break;
        }
    }

    for (i, (name, stake)) in TEILNEHMER.iter().enumerate() {
        let (stimmen, _, _) = knoten[i].konsens().expect("Runde").gewichte();
        assert_eq!(
            stimmen, gesamt,
            "{name} zählte {stimmen} statt {gesamt}. Fehlt genau {stake}, \
             hat er seine eigene Stimme nicht mitgezählt"
        );
    }

    std::fs::remove_dir_all(&verzeichnis).ok();
}

/// Das Betriebsprotokoll muss die Runde rekonstruierbar machen.
///
/// Ein Lauf über mehrere Maschinen ist so viel wert wie das, was danach
/// aus den Protokollen hervorgeht. Für eine BFT-Runde heißt das: **wer
/// war Leader, welches Gewicht kam zusammen, welche Schwelle galt.**
#[tokio::test]
async fn das_protokoll_traegt_gewicht_und_schwelle() {
    let verzeichnis = arbeitsverzeichnis("protokoll");
    let g = stimmsatz();
    let mut knoten = starte_netz(&verzeichnis).await;
    let mesh = warte_auf_mesh(&mut knoten, Duration::from_secs(20)).await;
    assert!(mesh.iter().all(|n| *n >= 1), "kein Mesh: {mesh:?}");

    let vorschlag = Hash::sha256(b"protokolltest");
    for (i, (name, _)) in TEILNEHMER.iter().enumerate() {
        let schluessel = Konsensschluessel::probe(name).expect("Schlüssel");
        knoten[i]
            .beginne_konsensrunde(&g, schluessel, vorschlag, timeouts())
            .await
            .expect("Runde beginnen");
    }
    for _ in 0..40 {
        reihum(&mut knoten, 1, Duration::from_millis(150)).await;
        if knoten
            .iter()
            .all(|k| k.konsens().map(|r| r.ist_commitet()).unwrap_or(false))
        {
            break;
        }
    }

    let pfad = knoten[0].protokollpfad().to_path_buf();
    let text = std::fs::read_to_string(&pfad).expect("Protokoll lesen");

    assert!(
        text.contains("\"konsens_runde_beginnt\""),
        "der Rundenbeginn steht nicht im Protokoll"
    );
    assert!(
        text.contains("\"konsens_commitet\""),
        "der Abschluss steht nicht im Protokoll"
    );
    // ⚑ Gewicht und Schwelle, nicht Kopfzahl. Ein Protokoll, das
    // „3 von 5 Stimmen" meldete, verdeckte genau den Unterschied, für
    // den die Genesis-Verteilung gebaut wurde.
    assert!(
        text.contains("\"stimmgewicht\"") && text.contains("\"schwelle\""),
        "das Protokoll nennt Gewicht und Schwelle nicht"
    );
    // Die Herkunft des Schlüssels muss dastehen: Dieser Lauf benutzt
    // Probeschlüssel, und das darf im Nachhinein nicht strittig sein.
    assert!(
        text.contains("\"schluesselherkunft\":\"probelauf\""),
        "die Schlüsselherkunft fehlt im Protokoll"
    );
    assert!(
        text.contains("\"schluessel_geheim\":false"),
        "das Protokoll behauptet, ein Probeschlüssel sei geheim"
    );

    std::fs::remove_dir_all(&verzeichnis).ok();
}

/// ⚑ **Fund 63: Nachrichten, die vor der eigenen Runde ankommen.**
///
/// Dieser Test baut die Lage nach, die den ersten Lauf über fünf
/// Prozesse hat scheitern lassen: Der Leader beginnt seine Runde und
/// veröffentlicht sofort. Bei den anderen kommt der Propose an, **bevor**
/// sie ihre eigene Runde begonnen haben. In der ersten Fassung wurde er
/// dort verworfen, ohne eine Protokollzeile, und danach wartete das
/// ganze Netz auf einen Propose, den es längst bekommen hatte.
///
/// Gemessen waren es **417 Millisekunden** Abstand.
///
/// Der Modultest in `konsens.rs` kann das nicht sehen: Dort beginnen
/// alle Knoten ihre Runde, bevor die erste Nachricht fließt, weil eine
/// Schlange serialisiert, was ein Netz parallel macht.
#[tokio::test]
async fn ein_vorzeitiger_propose_geht_nicht_verloren() {
    let verzeichnis = arbeitsverzeichnis("vorlauf");
    let g = stimmsatz();
    let mut knoten = starte_netz(&verzeichnis).await;
    let mesh = warte_auf_mesh(&mut knoten, Duration::from_secs(20)).await;
    assert!(mesh.iter().all(|n| *n >= 1), "kein Mesh: {mesh:?}");

    let vorschlag = Hash::sha256(b"vorlauftest");

    // Wer ist Leader? Nur er sendet als Erster etwas.
    let leader = myl_consensus::select_leader(0, &g.kennungen()).expect("Leader");
    let leader_index = TEILNEHMER
        .iter()
        .position(|(n, _)| {
            Konsensschluessel::probe(n).expect("Schlüssel").kennung() == leader
        })
        .expect("Leader unter den Teilnehmern");

    // **Nur** der Leader beginnt und veröffentlicht.
    let schluessel = Konsensschluessel::probe(TEILNEHMER[leader_index].0).expect("Schlüssel");
    knoten[leader_index]
        .beginne_konsensrunde(&g, schluessel, vorschlag, timeouts())
        .await
        .expect("Runde des Leaders");

    // Zeit, damit Propose und Vote des Leaders bei allen ankommen,
    // solange dort noch keine Runde läuft. Genau die Lücke von Fund 63.
    for _ in 0..8 {
        reihum(&mut knoten, 1, Duration::from_millis(120)).await;
    }

    // Erst jetzt beginnen die übrigen.
    for (i, (name, _)) in TEILNEHMER.iter().enumerate() {
        if i == leader_index {
            continue;
        }
        let schluessel = Konsensschluessel::probe(name).expect("Schlüssel");
        knoten[i]
            .beginne_konsensrunde(&g, schluessel, vorschlag, timeouts())
            .await
            .expect("Runde beginnen");
    }

    for _ in 0..40 {
        reihum(&mut knoten, 1, Duration::from_millis(150)).await;
        if knoten
            .iter()
            .all(|k| k.konsens().map(|r| r.ist_commitet()).unwrap_or(false))
        {
            break;
        }
    }

    for (i, (name, _)) in TEILNEHMER.iter().enumerate() {
        let r = knoten[i].konsens().expect("laufende Runde");
        assert!(
            r.ist_commitet(),
            "{name} hat nicht commitet. Ohne den Vorlauf-Puffer wartet hier das \
             ganze Netz auf einen Propose, den es längst bekommen hat (Fund 63)"
        );
        assert_eq!(r.commiteter_block(), Some(vorschlag));
    }

    // Und die Zahl muss im Protokoll stehen: Ein Puffer, der still
    // arbeitet, ist dieselbe Stille wie vorher, nur an anderer Stelle.
    let text = std::fs::read_to_string(knoten[(leader_index + 1) % 5].protokollpfad())
        .expect("Protokoll lesen");
    assert!(
        text.contains("\"konsens_vorlauf_nachgereicht\""),
        "das Nachreichen steht nicht im Protokoll"
    );

    std::fs::remove_dir_all(&verzeichnis).ok();
}

/// ⚑ **Fund 64: keine doppelten Schlüssel in einer Protokollzeile.**
///
/// `konsens_gesendet` schrieb ein zweites Feld namens `art`. Solche
/// Zeilen schlagen nirgends fehl: Ein Leser nimmt das erste, ein anderer
/// das letzte. Eine Auswertung über mehrere Maschinen zählt dann je nach
/// Werkzeug verschiedene Dinge.
///
/// `debug_assert` in `protokoll.rs` fängt das beim Entwickeln. Dieser
/// Test prüft die **geschriebene Datei**, also auch im Freigabebau.
#[tokio::test]
async fn keine_protokollzeile_hat_einen_schluessel_zweimal() {
    let verzeichnis = arbeitsverzeichnis("schluessel");
    let g = stimmsatz();
    let mut knoten = starte_netz(&verzeichnis).await;
    let mesh = warte_auf_mesh(&mut knoten, Duration::from_secs(20)).await;
    assert!(mesh.iter().all(|n| *n >= 1), "kein Mesh: {mesh:?}");

    let vorschlag = Hash::sha256(b"schluesseltest");
    for (i, (name, _)) in TEILNEHMER.iter().enumerate() {
        let schluessel = Konsensschluessel::probe(name).expect("Schlüssel");
        knoten[i]
            .beginne_konsensrunde(&g, schluessel, vorschlag, timeouts())
            .await
            .expect("Runde beginnen");
    }
    for _ in 0..30 {
        reihum(&mut knoten, 1, Duration::from_millis(120)).await;
        if knoten
            .iter()
            .all(|k| k.konsens().map(|r| r.ist_commitet()).unwrap_or(false))
        {
            break;
        }
    }

    let mut zeilen = 0usize;
    for k in &knoten {
        let text = std::fs::read_to_string(k.protokollpfad()).expect("Protokoll lesen");
        for (nr, zeile) in text.lines().enumerate() {
            zeilen += 1;
            let mut namen = Vec::new();
            let mut rest = zeile;
            // Flache Objekte aus Zeichenketten, Zahlen und
            // Wahrheitswerten: Das verspricht das Format, und der Leser
            // in `zwei_knoten.rs` verlässt sich darauf ebenfalls.
            while let Some(i) = rest.find("\"") {
                let nach = &rest[i + 1..];
                let Some(j) = nach.find("\"") else { break };
                let wort = &nach[..j];
                let danach = &nach[j + 1..];
                if danach.starts_with(':') {
                    namen.push(wort.to_string());
                }
                rest = danach;
            }
            let mut gesehen = std::collections::BTreeSet::new();
            for n in &namen {
                assert!(
                    gesehen.insert(n.clone()),
                    "Zeile {} in {}: Feld {n:?} steht zweimal. Ein Leser nimmt das \
                     erste, ein anderer das letzte (Fund 64).\n{zeile}",
                    nr + 1,
                    k.protokollpfad().display()
                );
            }
        }
    }
    assert!(zeilen > 20, "nur {zeilen} Protokollzeilen geprüft");
    println!("[Messung] {zeilen} Protokollzeilen auf doppelte Schlüssel geprüft");

    std::fs::remove_dir_all(&verzeichnis).ok();
}

/// ⚑ **Der Rundenwechsel über echte Sockets.**
///
/// Der Leader von Runde 0 startet zwar als Netzknoten, **beginnt aber
/// keine Konsensrunde**. Er ist damit für den Konsens ausgefallen,
/// während sein Gossip weiterläuft: genau der Fall, den ein
/// abgestürzter Validator erzeugt.
///
/// Ohne Rundenwechsel wartet das Netz ewig auf einen Vorschlag, der nie
/// kommt. Mit ihm übernimmt der Leader von Runde 1.
///
/// **Die Fristen sind hier knapp gesetzt** (1,2 s), damit der Test nicht
/// minutenlang läuft. Die Vorgabe im Betrieb ist eine Sekunde plus eine
/// halbe je Runde.
#[tokio::test]
async fn ein_ausgefallener_leader_haelt_das_netz_nicht_auf() {
    let verzeichnis = arbeitsverzeichnis("ausfall");
    let g = stimmsatz();
    let mut knoten = starte_netz(&verzeichnis).await;
    let mesh = warte_auf_mesh(&mut knoten, Duration::from_secs(20)).await;
    assert!(mesh.iter().all(|n| *n >= 1), "kein Mesh: {mesh:?}");

    let leader = myl_consensus::select_leader(0, &g.kennungen()).expect("Leader");
    let leader_index = TEILNEHMER
        .iter()
        .position(|(n, _)| Konsensschluessel::probe(n).expect("Schlüssel").kennung() == leader)
        .expect("Leader unter den Teilnehmern");
    println!(
        "[test] Leader von Runde 0 ist {} und fällt aus",
        TEILNEHMER[leader_index].0
    );

    let knappe_fristen = TimeoutConfig {
        propose_ms: 1_200,
        vote_ms: 1_200,
        commit_ms: 1_200,
        delta_ms: 600,
    };
    let vorschlag = Hash::sha256(b"ausfalltest");

    // Alle **außer** dem Leader von Runde 0 beginnen.
    for (i, (name, _)) in TEILNEHMER.iter().enumerate() {
        if i == leader_index {
            continue;
        }
        let schluessel = Konsensschluessel::probe(name).expect("Schlüssel");
        knoten[i]
            .beginne_konsensrunde(&g, schluessel, vorschlag, knappe_fristen)
            .await
            .expect("Runde beginnen");
    }

    // Genug Zeit für: Frist verfällt, neuer Leader schlägt vor, alle
    // stimmen und commiten.
    for _ in 0..60 {
        reihum(&mut knoten, 1, Duration::from_millis(150)).await;
        let fertig = knoten
            .iter()
            .enumerate()
            .filter(|(i, k)| {
                *i != leader_index && k.konsens().map(|r| r.ist_commitet()).unwrap_or(false)
            })
            .count();
        if fertig == TEILNEHMER.len() - 1 {
            break;
        }
    }

    for (i, (name, _)) in TEILNEHMER.iter().enumerate() {
        if i == leader_index {
            assert!(
                knoten[i].konsens().is_none(),
                "{name} sollte gar keine Runde fahren"
            );
            continue;
        }
        let r = knoten[i].konsens().expect("laufende Runde");
        println!(
            "[{name}] runde={} wechsel={} commitet={}",
            r.runde(),
            r.wechsel(),
            r.ist_commitet()
        );
        assert!(
            r.wechsel() >= 1,
            "{name} hat die Runde nicht gewechselt, obwohl der Leader ausfiel"
        );
        assert!(
            r.ist_commitet(),
            "{name} hat nicht commitet. Ohne Rundenwechsel wartet hier das ganze \
             Netz auf einen Vorschlag, der nie kommt"
        );
        assert_eq!(r.commiteter_block(), Some(vorschlag), "{name}");
    }

    // Und das Protokoll muss den Wechsel benennen, mit dem Gewicht: Ein
    // Wechsel bei 0 Stimmen heißt „kein Vorschlag kam an", ein Wechsel
    // dicht unter der Schwelle heißt etwas ganz anderes.
    let beobachter = (leader_index + 1) % TEILNEHMER.len();
    let text = std::fs::read_to_string(knoten[beobachter].protokollpfad()).expect("Protokoll");
    assert!(
        text.contains("\"konsens_rundenwechsel\""),
        "der Rundenwechsel steht nicht im Protokoll"
    );
    assert!(
        text.contains("\"neuer_leader\"") && text.contains("\"stimmgewicht\""),
        "der Wechsel nennt nicht, wer übernimmt und mit welchem Gewicht"
    );

    std::fs::remove_dir_all(&verzeichnis).ok();
}
