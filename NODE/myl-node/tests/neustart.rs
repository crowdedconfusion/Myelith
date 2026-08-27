//! Ein Knoten, der neu startet, macht dort weiter, wo er aufhörte.
//!
//! # Warum das ein eigener Test ist
//!
//! Bis zum 2026-08-26 begann **jeder Start bei null**. Der Chaos-Test
//! „Node-Restart" aus NETWORKING 4.1 war damit nicht durchführbar: Ein
//! neu gestarteter Knoten war nicht derselbe Knoten mit einer Lücke,
//! sondern ein neuer ohne Vergangenheit.
//!
//! # Was hier tatsächlich geprüft wird
//!
//! Nicht „die Datei lässt sich lesen". Das prüfen die Modultests in
//! `speicher.rs`. Hier geht es um die **Zusage der Kette**: dass Höhe,
//! letzter Hash und **Zustandswurzel** nach dem Neustart dieselben sind.
//!
//! Die Zustandswurzel ist der harte Teil. Sie wird nicht gespeichert,
//! sondern beim Nachspielen **neu gerechnet**, durch dieselbe
//! `Kette::uebernimm`, durch die auch Gossip-Blöcke gehen. Stimmt sie
//! überein, ist der Ledger-Pfad über einen Prozessneustart hinweg
//! deterministisch, und genau das ist die Frage.
//!
//! # ⚑ Der Abbruch wird nachgestellt, nicht gehofft
//!
//! Ein Test, der sauber beendet und wieder öffnet, prüft den bequemen
//! Fall. Der Fall, für den die Datei gebaut ist, ist `kill -9` mitten im
//! Schreiben. Er wird hier nachgestellt, indem hinter den letzten
//! vollständigen Satz eine halbe Nutzlast geschrieben wird.

use std::io::Write;
use std::path::PathBuf;

use myl_node::kette::Kette;
use myl_node::speicher::Kettenspeicher;
use myl_types::hash::Hash;

fn arbeitsverzeichnis(marke: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "myl-neustart-{marke}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Baut eine Kette mit Speicher und `n` Blöcken voller Transaktionen.
///
/// Gibt Höhe, letzten Hash und Zustandswurzel zurück, also genau das,
/// was der Neustart wiederherstellen muss.
fn kette_fuellen(pfad: &std::path::Path, n: u64) -> (u64, Hash, Hash) {
    use myl_consensus::block::{BurnTx, Transaction};
    use myl_node::kette::probekonto;

    let (speicher, anlauf) =
        Kettenspeicher::oeffnen(pfad, Kette::startwert()).expect("Speicher öffnen");
    assert!(anlauf.bloecke.is_empty(), "das Verzeichnis war nicht frisch");
    let mut k = Kette::probestand();
    k.speicher_setzen(speicher);

    for i in 0..n {
        // Echte Transaktionen, sonst bliebe der Zustand unverändert und
        // die übereinstimmende Wurzel belegte nichts.
        k.aufnehmen(Transaction::Burn(BurnTx {
            sender: probekonto((i % 8) as u8),
            amount: 1_000 + i,
        }));
        let _ = k.baue_block();
    }
    (k.hoehe(), k.letzter_hash(), k.zustandswurzel())
}

/// Öffnet die Datei und spielt sie nach, wie der Knoten es beim Start
/// tut: durch dieselbe `uebernimm`.
fn kette_nachspielen(pfad: &std::path::Path) -> (Kette, usize, u64) {
    let (speicher, anlauf) =
        Kettenspeicher::oeffnen(pfad, Kette::startwert()).expect("Speicher öffnen");
    let mut k = Kette::probestand();
    let mut uebernommen = 0usize;
    for b in &anlauf.bloecke {
        if k.uebernimm(b).is_ok() {
            uebernommen += 1;
        }
    }
    k.speicher_setzen(speicher);
    (k, uebernommen, anlauf.abgeschnitten)
}

#[test]
fn nach_dem_neustart_steht_die_kette_wo_sie_war() {
    let d = arbeitsverzeichnis("sauber");
    let p = d.join("kette.log");
    let (hoehe, letzter, wurzel) = kette_fuellen(&p, 7);
    assert_eq!(hoehe, 7);

    let (k, uebernommen, abgeschnitten) = kette_nachspielen(&p);
    assert_eq!(uebernommen, 7, "nicht alle Blöcke kamen durch");
    assert_eq!(abgeschnitten, 0);
    assert_eq!(k.hoehe(), hoehe);
    assert_eq!(k.letzter_hash(), letzter);
    // **Der eigentliche Punkt.** Die Wurzel wird nicht gespeichert,
    // sondern neu gerechnet. Stimmt sie, ist der Ledger-Pfad über einen
    // Prozessneustart hinweg deterministisch.
    assert_eq!(
        k.zustandswurzel(),
        wurzel,
        "die Zustandswurzel wich nach dem Neustart ab"
    );

    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn der_mempool_ueberlebt_den_neustart_bewusst_nicht() {
    // Wartende Transaktionen sind unbestätigt und kommen über den
    // Gossip wieder. Sie aufzuheben hieße, nach einem Tag Stillstand
    // alte Transaktionen einzuspeisen, deren Absender längst andere
    // geschickt hat.
    use myl_consensus::block::{BurnTx, Transaction};
    use myl_node::kette::probekonto;

    let d = arbeitsverzeichnis("mempool");
    let p = d.join("kette.log");
    {
        let (speicher, _) = Kettenspeicher::oeffnen(&p, Kette::startwert()).unwrap();
        let mut k = Kette::probestand();
        k.speicher_setzen(speicher);
        k.aufnehmen(Transaction::Burn(BurnTx {
            sender: probekonto(0),
            amount: 5,
        }));
        let _ = k.baue_block();
        // Diese hier landet in keinem Block mehr.
        k.aufnehmen(Transaction::Burn(BurnTx {
            sender: probekonto(1),
            amount: 7,
        }));
        assert_eq!(k.wartend(), 1);
    }
    let (k, _, _) = kette_nachspielen(&p);
    assert_eq!(k.hoehe(), 1);
    assert_eq!(k.wartend(), 0, "der Mempool wurde mitgeschleppt");
    std::fs::remove_dir_all(&d).ok();
}

/// ⚑ **Der Fall, für den die Datei gebaut ist.**
///
/// Ein `kill -9` mitten im Schreiben hinterlässt einen halben Satz. Der
/// Knoten muss danach starten, nicht scheitern, und mit dem letzten
/// **vollständigen** Block weiterrechnen.
#[test]
fn ein_abbruch_mitten_im_schreiben_kostet_hoechstens_den_letzten_block() {
    use myl_consensus::block::{BurnTx, Transaction};
    use myl_node::kette::probekonto;

    let d = arbeitsverzeichnis("abbruch");
    let p = d.join("kette.log");
    let (hoehe, letzter, wurzel) = kette_fuellen(&p, 4);
    assert_eq!(hoehe, 4);
    let laenge_vorher = std::fs::metadata(&p).unwrap().len();

    // Einen halben fünften Satz anhängen: Längenkopf plus halbe Nutzlast.
    let mut hilfs = Kette::probestand();
    for i in 0..4u64 {
        hilfs.aufnehmen(Transaction::Burn(BurnTx {
            sender: probekonto((i % 8) as u8),
            amount: 1_000 + i,
        }));
        let _ = hilfs.baue_block();
    }
    hilfs.aufnehmen(Transaction::Burn(BurnTx {
        sender: probekonto(4),
        amount: 4_711,
    }));
    let fuenfter = hilfs.baue_block();
    let nutz = borsh::to_vec(&fuenfter).unwrap();
    {
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        f.write_all(&(nutz.len() as u32).to_le_bytes()).unwrap();
        f.write_all(&nutz[..nutz.len() / 2]).unwrap();
    }
    assert!(std::fs::metadata(&p).unwrap().len() > laenge_vorher);

    // Der Knoten startet und steht beim vierten Block.
    let (k, uebernommen, abgeschnitten) = kette_nachspielen(&p);
    assert_eq!(uebernommen, 4);
    assert!(abgeschnitten > 0, "der halbe Satz wurde nicht bemerkt");
    assert_eq!(k.hoehe(), hoehe);
    assert_eq!(k.letzter_hash(), letzter);
    assert_eq!(k.zustandswurzel(), wurzel);
    assert_eq!(
        std::fs::metadata(&p).unwrap().len(),
        laenge_vorher,
        "die Datei wurde nicht gekürzt"
    );

    std::fs::remove_dir_all(&d).ok();
}

/// Nach dem Abbruch muss der nächste Block sauber anhängen.
///
/// Ohne das Kürzen stünde er hinter Datenmüll und wäre beim übernächsten
/// Start unerreichbar: Der Knoten verlöre bei jedem Absturz alles, was
/// danach kam, und das fiele erst beim dritten Start auf.
#[test]
fn nach_einem_abbruch_waechst_die_kette_normal_weiter() {
    use myl_consensus::block::{BurnTx, Transaction};
    use myl_node::kette::probekonto;

    let d = arbeitsverzeichnis("weiter");
    let p = d.join("kette.log");
    kette_fuellen(&p, 3);
    {
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        f.write_all(&[0x40, 0x00, 0x00, 0x00, 0xAB, 0xCD]).unwrap();
    }

    // Erster Neustart: kürzen und zwei Blöcke anhängen.
    {
        let (mut k, uebernommen, abgeschnitten) = kette_nachspielen(&p);
        assert_eq!(uebernommen, 3);
        assert_eq!(abgeschnitten, 6);
        for i in 0..2u64 {
            k.aufnehmen(Transaction::Burn(BurnTx {
                sender: probekonto(5),
                amount: 77 + i,
            }));
            let _ = k.baue_block();
        }
        assert_eq!(k.hoehe(), 5);
    }

    // Zweiter Neustart: alle fünf sind da.
    let (k, uebernommen, abgeschnitten) = kette_nachspielen(&p);
    assert_eq!(uebernommen, 5, "die nach dem Abbruch geschriebenen fehlen");
    assert_eq!(abgeschnitten, 0);
    assert_eq!(k.hoehe(), 5);

    std::fs::remove_dir_all(&d).ok();
}

/// Der Wiederanlauf geht durch dieselbe Prüfung wie der Gossip.
///
/// Ein zweiter Ladepfad mit eigenen Regeln wäre die Stelle, an der eine
/// veränderte Datei durchkäme. Hier wird ein gespeicherter Block
/// verfälscht, aber mit gültiger Prüfsumme: Der Speicher lässt ihn
/// durch, die **Kette** nicht.
#[test]
fn ein_veraenderter_block_faellt_beim_nachspielen_durch() {
    let d = arbeitsverzeichnis("veraendert");
    let p = d.join("kette.log");
    kette_fuellen(&p, 3);

    // Den zweiten Satz durch einen Block ersetzen, der für sich gültig
    // aussieht, aber nicht an den ersten anschließt.
    let (_, anlauf) = Kettenspeicher::oeffnen(&p, Kette::startwert()).unwrap();
    assert_eq!(anlauf.bloecke.len(), 3);
    let mut geaendert = anlauf.bloecke[1].clone();
    geaendert.header.prev_block_hash = Hash::sha256(b"etwas anderes");

    // Die Datei neu schreiben, mit korrekten Prüfsummen.
    std::fs::remove_file(&p).unwrap();
    {
        let (mut s, _) = Kettenspeicher::oeffnen(&p, Kette::startwert()).unwrap();
        s.anhaengen(&anlauf.bloecke[0]).unwrap();
        s.anhaengen(&geaendert).unwrap();
        s.anhaengen(&anlauf.bloecke[2]).unwrap();
    }

    let (k, uebernommen, abgeschnitten) = kette_nachspielen(&p);
    assert_eq!(abgeschnitten, 0, "die Prüfsumme stimmt, sie fängt das nicht");
    assert_eq!(
        uebernommen, 1,
        "die Kette nahm mehr als den ersten Block an, obwohl die Verkettung bricht"
    );
    assert_eq!(k.hoehe(), 1);
    std::fs::remove_dir_all(&d).ok();
}

// ── Über echte Knoten, mit echten Sockets ───────────────────────────

/// ⚑ **Der Beleg, den kein Modultest liefern kann.**
///
/// Ein Knoten wird abgeräumt und aus seiner Datei neu aufgebaut, während
/// ein **zweiter durchläuft**. Stimmen danach beide Zustandswurzeln
/// überein, hat der Wiederanlauf denselben Zustand hergestellt, den das
/// Netz die ganze Zeit gesehen hat.
///
/// Der Vergleich gegen den durchlaufenden Knoten ist der Kern. Ein
/// Vergleich gegen die eigene letzte Zustandsaufnahme wäre wertlos: Sie
/// liegt Sekunden und Blöcke vor dem Abbruch. Genau daran ist der erste
/// Versuch dieser Messung von Hand gescheitert.
#[tokio::test]
async fn ein_neu_gestarteter_knoten_stimmt_mit_dem_durchlaufenden_ueberein() {
    use myl_node::konfig::{KnotenKonfig, Rolle};
    use myl_node::Knoten;
    use std::time::Duration;

    let d = arbeitsverzeichnis("prozess");
    let kettendatei = d.join("erzeuger.kette");

    let konfig = |name: &str, erzeuger: bool, bootstrap: Vec<String>, kette: bool| KnotenKonfig {
        name: name.to_string(),
        schluesseldatei: d.join(format!("{name}.key")),
        protokollverzeichnis: d.join("logs"),
        horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
        bootstrap,
        rolle: Rolle::Teilnehmer,
        nat: Default::default(),
        aufnahme_sekunden: 1,
        testverkehr_sekunden: None,
        erzeugt_bloecke: erzeuger,
        teilnehmer: vec!["erzeuger".into(), "zeuge".into()],
        kettendatei: if kette { Some(kettendatei.clone()) } else { None },
        genesisdatei: None,
        konsensschluesseldatei: None,
    };

    // Der Zeuge läuft durch und startet nie neu.
    let mut zeuge = Knoten::starten(konfig("zeuge", false, vec![], false), false)
        .await
        .expect("Zeuge");
    let adresse = zeuge
        .warte_auf_adresse(Duration::from_secs(5))
        .await
        .expect("Adresse");
    let bootstrap = vec![format!("{}/p2p/{}", adresse, zeuge.peer_id())];

    let wurzel_nach_abbruch;
    let hoehe_nach_abbruch;
    {
        let mut erzeuger = Knoten::starten(konfig("erzeuger", true, bootstrap.clone(), true), false)
            .await
            .expect("Erzeuger");
        // Verbinden lassen: Der Erzeuger baut erst, wenn jemand zuhört.
        for _ in 0..20 {
            zeuge.laufe_fuer(Duration::from_millis(100)).await;
            erzeuger.laufe_fuer(Duration::from_millis(100)).await;
            if erzeuger.peers().await > 0 {
                break;
            }
        }
        assert!(erzeuger.peers().await > 0, "die beiden fanden einander nicht");

        // Transaktionen und Blöcke, damit sich der Zustand auch ändert.
        // Blieben die Blöcke leer, wäre die übereinstimmende Wurzel
        // nichtssagend: Sie wäre die des leeren Zustands.
        for _ in 0..6 {
            zeuge.sende_transaktion().await;
            zeuge.laufe_fuer(Duration::from_millis(120)).await;
            erzeuger.laufe_fuer(Duration::from_millis(120)).await;
            erzeuger.erzeuge_block().await;
            erzeuger.laufe_fuer(Duration::from_millis(120)).await;
            zeuge.laufe_fuer(Duration::from_millis(120)).await;
        }
        hoehe_nach_abbruch = erzeuger.kette().hoehe();
        wurzel_nach_abbruch = erzeuger.kette().zustandswurzel();
        assert!(hoehe_nach_abbruch >= 6, "zu wenige Blöcke: {hoehe_nach_abbruch}");
        assert_ne!(
            wurzel_nach_abbruch,
            myl_node::kette::Kette::probestand().zustandswurzel(),
            "der Zustand hat sich nicht geändert, die Wurzel belegt dann nichts"
        );
        assert_eq!(
            erzeuger.kette().gespeicherte_bloecke(),
            Some(hoehe_nach_abbruch)
        );
        assert_eq!(erzeuger.kette().schreibfehler(), 0);
        // Hier endet der Erzeuger. Kein sauberes Herunterfahren, kein
        // Abschlusseintrag: genau wie bei `kill -9`.
    }

    // Der Zeuge holt auf, was noch unterwegs war.
    for _ in 0..10 {
        zeuge.laufe_fuer(Duration::from_millis(120)).await;
    }

    // Neu aufbauen, aus derselben Datei.
    let neu = Knoten::starten(konfig("erzeuger", true, bootstrap, true), false)
        .await
        .expect("Wiederanlauf");
    assert_eq!(
        neu.kette().hoehe(),
        hoehe_nach_abbruch,
        "der Wiederanlauf steht auf einer anderen Höhe"
    );
    assert_eq!(
        neu.kette().zustandswurzel(),
        wurzel_nach_abbruch,
        "die Zustandswurzel wich nach dem Wiederanlauf ab"
    );
    assert_eq!(
        neu.kette().zustandswurzel(),
        zeuge.kette().zustandswurzel(),
        "der neu gestartete Knoten und der durchlaufende sind sich uneins. \
         Der Zeuge hat nie neu gestartet, also ist seine Sicht die Referenz"
    );
    assert_eq!(neu.kette().hoehe(), zeuge.kette().hoehe());

    std::fs::remove_dir_all(&d).ok();
}
