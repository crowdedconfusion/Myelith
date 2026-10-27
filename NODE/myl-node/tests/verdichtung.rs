//! **Verdichtung und Archivknoten** an echten Langzeitgrenzen (Funde 624 und
//! 625, Festlegungen des Projektinhabers vom 2026-10-23).
//!
//! Behalten wird immer alles ab dem **vorletzten** Langzeitabzug; die erste
//! Verdichtung ist also an der zweiten Langzeitgrenze möglich (Höhe
//! 230 400, Basis 115 200). Die Probe baut die Kette wirklich so weit, mit
//! leeren Blöcken (345 600 in rund drei Sekunden), und prüft Datei, Basis,
//! Abzüge und Neustart.
//!
//! ⚑ **Seit der Archivpflicht im Konsens (Stufen A5 und A6) verdichtet nur,
//! wer laut Zustand nicht hält**, und vor der ersten bestätigten Übergabe
//! hält jeder. Die Proben belegen deshalb wie echte Kopien: gleich nach
//! jeder Grenze die Vollständigkeit, aus der eigenen Kette und mit den
//! Schlüsseln der Probekonten.

use std::path::{Path, PathBuf};

use myl_node::abzug::{Ablage, LANGZEIT_EPOCHEN};
use myl_node::kette::Kette;
use myl_node::speicher::Kettenspeicher;
use myl_consensus::block::BLOECKE_JE_EPOCHE;
use myl_scheduler::miner_filter::{HardwareClass, MinerRegistration};
use myl_consensus::block::Transaktion;
use myl_node::kette::{probekonto, probeschluessel};
use myl_types::ids::MinerId;

const PERIODE: u64 = BLOECKE_JE_EPOCHE * LANGZEIT_EPOCHEN;

fn ordner(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("myl-verdichtung-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("Ordner");
    d
}

fn miner(b: u8) -> MinerRegistration {
    miner_ab(b, 0)
}

/// Die Kennung des Probekontos `b`.
fn kennung(b: u8) -> MinerId {
    MinerId::aus_schluessel(&probeschluessel(b).public_key().expect("Schlüssel"))
}

/// Ein Miner mit dem Schlüssel des Probekontos `b`, damit er belegen kann.
fn miner_ab(b: u8, epoche: u64) -> MinerRegistration {
    let pk = probeschluessel(b).public_key().expect("Schlüssel");
    MinerRegistration {
        miner_id: MinerId::aus_schluessel(&pk),
        hardware_class: HardwareClass::MediumGpu,
        registration_epoch: epoche,
        zone: myl_types::node_metadata::GeoRegion::Europe,
        schluessel: pk,
        netzadresse: myl_types::latency_attest::PeerIdBytes([b; 32]),
    }
}

/// **Die Kopien belegen**, soweit diese Kette ihre Geschichte hat: je
/// Kopie unter den Probekonten ein Beleg, unterschrieben mit ihrem Schlüssel.
fn belegen(k: &mut Kette) -> usize {
    let mut n = 0;
    for b in 1..=myl_node::kette::PROBEKONTEN {
        let id = kennung(b);
        let Some(anweisung) = k.archivbeleg(&id) else {
            continue;
        };
        let nonce = k.zustand().account(&probekonto(b)).nonce;
        let tx = Transaktion::signiere(&Kette::startwert(), &probeschluessel(b), nonce, anweisung).expect("signieren");
        k.aufnehmen(tx);
        n += 1;
    }
    n
}

/// Eine Probekette mit Datei und Ablage, `miner` registrierten Minern, der
/// eigenen Kennung `ich` und einem Budget von einem Byte.
fn kette(d: &Path, miner_zahl: u8, ich: Option<MinerId>) -> Kette {
    let mut k = Kette::probestand();
    let pfad = d.join("kette.dat");
    let (s, _) = Kettenspeicher::oeffnen(&pfad, k.netz().startwert_hash()).expect("Speicher");
    k.speicher_setzen(s);
    k.ablage_setzen(Ablage::neben(&pfad, k.netz().startwert_hash()));
    k.budget_setzen(1);
    k.archivkennung_setzen(ich);
    for b in 1..=miner_zahl {
        let m = miner(b);
        k.zustand_mut().miner.insert(m.miner_id, m);
    }
    k
}

/// Baut bis `hoehe`; am Anfang jeder Epoche belegen die Kopien, wie es
/// ihre Knoten täten (nach der Grenze zuerst die Vollständigkeit).
fn bis(k: &mut Kette, hoehe: u64) {
    while k.hoehe() < hoehe {
        if k.hoehe() >= PERIODE && k.hoehe().is_multiple_of(BLOECKE_JE_EPOCHE) {
            belegen(k);
        }
        k.baue_block();
    }
}

/// ⚑ **Ein Knoten, der nicht gezogen ist, verdichtet an jeder
/// Langzeitgrenze** auf den vorletzten Langzeitabzug, behält die Blöcke
/// danach, räumt die Langzeitabzüge davor und läuft aus der verdichteten
/// Datei wieder an.
#[test]
fn ein_nicht_gezogener_knoten_verdichtet() {
    let d = ordner("nicht-gezogen");
    let mut k = kette(&d, 10, None);
    bis(&mut k, 2 * PERIODE - 1);
    assert_eq!(k.basis(), 0, "vor der zweiten Langzeitgrenze darf nichts verdichtet sein");
    bis(&mut k, 2 * PERIODE);
    assert_eq!(k.basis(), PERIODE, "an der zweiten Grenze wird die erste zur Basis");
    assert!(k.verdichtung_nehmen().is_some());
    bis(&mut k, 3 * PERIODE);
    assert_eq!(k.basis(), 2 * PERIODE, "die Basis ist der vorletzte Langzeitabzug");
    let (basis, vorher, nachher) = k.verdichtung_nehmen().expect("verdichtet");
    assert_eq!(basis, 2 * PERIODE);
    // Vorher zwei Perioden, nachher eine: rund die Hälfte.
    assert!(nachher * 10 < vorher * 6, "die Datei schrumpfte nur von {vorher} auf {nachher}");
    let ablage = Ablage::neben(&d.join("kette.dat"), k.netz().startwert_hash());
    assert_eq!(ablage.basis(), Some(2 * PERIODE));
    assert!(ablage.bestaetigt(), "die selbst gerechnete Basis gilt als bestätigt");
    assert!(!ablage.hoehen().contains(&PERIODE), "der Langzeitabzug vor der Basis blieb liegen");
    let wurzel = k.zustandswurzel();
    let hoehe = k.hoehe();
    drop(k);

    // Neustart wie im Knoten: Abzug der Basis laden, Datei nachspielen.
    let mut neu = Kette::probestand();
    let (abzug, zustand) = ablage.lesen(2 * PERIODE).expect("Basis lesen");
    neu.auf_abzug_setzen(&abzug, zustand).expect("Basis setzen");
    let (mut s, w) = Kettenspeicher::oeffnen(&d.join("kette.dat"), neu.netz().startwert_hash()).expect("öffnen");
    assert_eq!(w.anzahl, hoehe - 2 * PERIODE);
    s.fuer_jeden_satz(|b| neu.uebernimm(&b).expect("nachspielen")).expect("lesen");
    assert_eq!(neu.hoehe(), hoehe);
    assert_eq!(neu.zustandswurzel(), wurzel);
    let _ = std::fs::remove_dir_all(&d);
}

/// ⚑ **Ohne registrierte Miner verdichtet niemand**: Es gibt niemanden,
/// unter dem gelost werden könnte, also ist jeder Archiv.
#[test]
fn ohne_miner_verdichtet_niemand() {
    let d = ordner("ohne-miner");
    let mut k = kette(&d, 0, None);
    bis(&mut k, 3 * PERIODE);
    assert_eq!(k.basis(), 0);
    assert_eq!(k.archiv(), (true, false), "jeder behält, keiner ist Kopie");
    assert!(k.verdichtung_nehmen().is_none());
    let _ = std::fs::remove_dir_all(&d);
}

/// ⚑ **Eine gezogene Kopie verdichtet nicht.** Bei genau sieben
/// berechtigten Minern ist jeder in jeder Periode Kopie.
#[test]
fn eine_gezogene_kopie_verdichtet_nicht() {
    let d = ordner("gezogen");
    let mut k = kette(&d, 7, Some(kennung(2)));
    bis(&mut k, 3 * PERIODE);
    assert_eq!(k.archivrolle(), myl_types::archivwahl::Archivrolle::Kopie);
    assert_eq!(k.archiv(), (true, true));
    assert_eq!(k.basis(), 0, "eine Kopie hat verdichtet");
    let _ = std::fs::remove_dir_all(&d);
}

/// ⚑ **Unter sieben Berechtigten behält jeder alles**, auch wer selbst
/// nicht Miner ist.
#[test]
fn unter_sieben_berechtigten_verdichtet_niemand() {
    let d = ordner("sechs");
    let mut k = kette(&d, 6, None);
    bis(&mut k, 3 * PERIODE);
    assert_eq!(k.archivrolle(), myl_types::archivwahl::Archivrolle::AlleBehalten);
    assert_eq!(k.basis(), 0);
    let _ = std::fs::remove_dir_all(&d);
}

/// ⛔️ **Junge Registrierungen zählen erst nach einer Periode.** Zehn
/// Miner aus Epoche 1 sind an der ersten Grenze (Epoche 64) nicht
/// berechtigt: Dort behält jeder alles, und weil die vorige Periode zählt,
/// verdichtet ein Unbeteiligter erst an der dritten Grenze statt an der
/// zweiten.
#[test]
fn junge_registrierungen_verschieben_die_verdichtung() {
    let d = ordner("jung");
    let mut k = kette(&d, 0, None);
    for b in 1..=10 {
        let m = miner_ab(b, 1);
        k.zustand_mut().miner.insert(m.miner_id, m);
    }
    bis(&mut k, 2 * PERIODE);
    assert_eq!(k.basis(), 0, "an der zweiten Grenze galt die erste noch als ungezogen");
    bis(&mut k, 3 * PERIODE);
    assert_eq!(k.basis(), 2 * PERIODE);
    let _ = std::fs::remove_dir_all(&d);
}

/// ⚑ **Die Geschichte kommt zurück**: Eine verdichtete Kette bekommt die
/// Blöcke ab Genesis vorangestellt, lebt danach ohne Basis, und ein
/// Neustart ab Genesis landet auf derselben Wurzel. So schliesst ein neu
/// gezogener Archivknoten seine Lücke, nachdem die Hintergrundprüfung die
/// Blöcke geprüft hat.
#[test]
fn die_geschichte_kommt_zurueck() {
    let d = ordner("geschichte");
    let mut k = kette(&d, 10, None);
    // Die eigenen Blöcke 1 bis zur späteren Basis, wie die Hintergrundprüfung
    // sie geprüft aufbewahrt. ⚠️ Nicht aus einer zweiten Kette: Die baut
    // andere Blöcke (Zeitstempel), und ihre Geschichte schlösse nicht an.
    let archiv = d.join("kette.dat.archiv");
    let (mut a, _) = Kettenspeicher::oeffnen(&archiv, k.netz().startwert_hash()).expect("Archiv");
    while k.hoehe() < 2 * PERIODE + 10 {
        if k.hoehe() >= PERIODE && k.hoehe().is_multiple_of(BLOECKE_JE_EPOCHE) {
            belegen(&mut k);
        }
        let b = k.baue_block();
        if b.header.height <= PERIODE {
            a.anhaengen(&b).expect("anhängen");
        }
    }
    drop(a);
    assert_eq!(k.basis(), PERIODE);

    let n = k.geschichte_einfuegen(&archiv).expect("einfügen");
    assert_eq!(n, PERIODE);
    assert_eq!(k.basis(), 0);
    assert!(!archiv.exists(), "die Archivdatei blieb liegen");
    let ablage = Ablage::neben(&d.join("kette.dat"), k.netz().startwert_hash());
    assert_eq!(ablage.basis(), None);
    let (wurzel, hoehe) = (k.zustandswurzel(), k.hoehe());
    drop(k);

    // Derselbe Anfangszustand wie oben, samt der direkt eingetragenen Miner.
    let mut neu = Kette::probestand();
    for b in 1..=10u8 {
        let m = miner(b);
        neu.zustand_mut().miner.insert(m.miner_id, m);
    }
    let (mut s, w) = Kettenspeicher::oeffnen(&d.join("kette.dat"), neu.netz().startwert_hash()).expect("öffnen");
    assert_eq!(w.anzahl, hoehe);
    s.fuer_jeden_satz(|b| neu.uebernimm(&b).expect("nachspielen ab Genesis")).expect("lesen");
    assert_eq!(neu.zustandswurzel(), wurzel);
    let _ = std::fs::remove_dir_all(&d);
}

/// ⚑ **Eine Kopie lädt ihre Geschichte mit Beweisen** (Stufe A4): Eine
/// verdichtete Kette holt die Blöcke vor ihrer Basis bei einer vollen,
/// Lieferung für Lieferung gegen die Historie ihrer Basis geprüft, setzt
/// sie vor und kann danach selbst liefern. Eine verdichtete Kette liefert
/// nichts.
#[test]
fn eine_kopie_laedt_ihre_geschichte_mit_beweisen() {
    use myl_node::archivabruf::Archivabruf;
    use myl_node::nachschub::naechster_bereich;
    let d = ordner("abruf");
    let dq = ordner("abruf-quelle");
    let mut quelle = kette(&dq, 10, None);
    quelle.budget_setzen(0);
    let mut kopie = kette(&d, 10, None);
    while quelle.hoehe() < 2 * PERIODE + 10 {
        if quelle.hoehe() == PERIODE {
            assert_eq!(belegen(&mut quelle), 7, "die Quelle belegt für alle Kopien");
        } else if quelle.hoehe() > PERIODE && quelle.hoehe().is_multiple_of(BLOECKE_JE_EPOCHE) {
            belegen(&mut quelle);
        }
        let b = quelle.baue_block();
        kopie.uebernimm(&b).expect("übernehmen");
    }
    assert_eq!(kopie.basis(), PERIODE);
    assert!(kopie.archivlieferung(1, 10, 100).is_none(), "eine verdichtete Kette lieferte Geschichte");

    let ablage = Ablage::neben(&d.join("kette.dat"), kopie.netz().startwert_hash());
    let (abzug, zustand) = ablage.lesen(PERIODE).expect("Basis");
    let mut a = Archivabruf::neu(
        &kopie.archivpfad().expect("Pfad"),
        kopie.netz().startwert_hash(),
        abzug.block,
        &zustand.historie,
    )
    .expect("Abruf");
    let mut lieferungen = 0;
    while let Some((ab, bis)) = naechster_bereich(a.geschrieben(), &[], |x| a.wartet(x), a.ziel()) {
        let (bloecke, beweis) = quelle.archivlieferung(ab, bis, a.anzahl()).expect("Lieferung");
        a.lieferung(bloecke, &beweis).expect("geprüft");
        lieferungen += 1;
    }
    assert!(lieferungen >= PERIODE / 512);
    let pfad = a.abschliessen().expect("abschliessen");
    let wurzel = kopie.zustandswurzel();
    assert_eq!(kopie.geschichte_einfuegen(&pfad).expect("einfügen"), PERIODE);
    assert_eq!(kopie.zustandswurzel(), wurzel);
    // Und jetzt liefert die Kopie selbst, mit demselben Beweis wie die Quelle.
    let n = kopie.hoehe() - 5;
    assert_eq!(kopie.archivlieferung(3000, 3100, n), quelle.archivlieferung(3000, 3100, n));
    assert!(kopie.archivlieferung(3000, 3100, n).is_some());
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&dq);
}


/// ⚑ **Splitter** (Stufe A7): Die Kopien nennen die Teilwurzel, die
/// Splitter holen ihre Teile, legen sie ab und belegen sie; und aus
/// beliebigen 32 der 64 Teile jeder Gruppe kommt die Geschichte der Periode
/// zurück, mit derselben Historie wie im Zustand.
#[test]
fn splitter_halten_teile_aus_denen_die_geschichte_zurueckkommt() {
    use myl_types::archivteile::{dekodieren, DATENTEILE, TEILE};
    let d = ordner("splitter");
    let mut k = kette(&d, myl_node::kette::PROBEKONTEN, None);
    while k.hoehe() < PERIODE {
        k.baue_block();
    }
    let a = k.zustand().archiv.clone();
    assert_eq!((a.kopien.len(), a.splitter.len()), (7, 5));
    assert_eq!((a.teil_ab, a.teil_bis), (0, PERIODE / 1024));
    // Die Kopien nennen die Teilwurzel.
    for b in 1..=myl_node::kette::PROBEKONTEN {
        let id = kennung(b);
        if let Some(anweisung) = k.teilwurzel_anweisung(&id) {
            let nonce = k.zustand().account(&probekonto(b)).nonce;
            k.aufnehmen(Transaktion::signiere(&Kette::startwert(), &probeschluessel(b), nonce, anweisung).expect("tx"));
        }
    }
    k.baue_block();
    let tw = k.zustand().archiv.teilwurzel.expect("die Teilwurzel gilt");
    let grenze = k.zustand().archiv.grenze;
    let anzahl = a.teil_bis - a.teil_ab;

    // Ein Splitter holt seine Teile, legt sie ab und belegt den gezogenen.
    let s = a.splitter[2];
    let b = (1..=myl_node::kette::PROBEKONTEN).find(|b| kennung(*b) == s).expect("Probekonto");
    let ablage = k.teilablage().expect("Ablage");
    for stelle in 0..anzahl {
        let (stueck, bw) = k.teilstueck(grenze, stelle, 2).expect("Teil");
        assert!(bw.pruefen(&tw, &stueck));
        ablage.ablegen(grenze, stelle, &stueck, &bw).expect("ablegen");
    }
    let z = k.zustand();
    let stelle = myl_ledger::archiv::teilprobe(&z.archiv, &z.epochensaat, z.epoch.0, &s).expect("Stelle");
    let (stueck, beweis) = ablage.lesen(grenze, stelle).expect("lesen");
    let nonce = k.zustand().account(&probekonto(b)).nonce;
    let tx = Transaktion::signiere(
        &Kette::startwert(),
        &probeschluessel(b),
        nonce,
        myl_consensus::block::Anweisung::ArchivTeilBelegen { stueck, beweis },
    )
    .expect("tx");
    k.aufnehmen(tx);
    k.baue_block();
    assert!(k.zustand().archiv.teil_belegt.contains(&s), "der Teilbeleg galt nicht");

    // Wiederherstellung: je Gruppe 32 andere Teile, die Blöcke zurück, und
    // ihre Hashes ergeben die Historie des Zustands an der Grenze.
    let mut historie = myl_types::historie::Historie::default();
    historie.anhaengen(&k.netz().startwert_hash());
    let mut x = 0x2545_f491_u64;
    for stelle in 0..anzahl {
        let mut wahl: Vec<usize> = (0..TEILE).collect();
        for i in (1..wahl.len()).rev() {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            wahl.swap(i, (x % (i as u64 + 1)) as usize);
        }
        let teile: Vec<(usize, Vec<u8>)> = wahl[..DATENTEILE]
            .iter()
            .map(|&t| (t, k.teilstueck(grenze, stelle, t as u64).expect("Teil").0))
            .collect();
        let bloecke = dekodieren(&teile).expect("aus 32 von 64");
        // Gelesen und gehasht wie jeder Block: Der Hash deckt die
        // Unterschrift des Erzeugers nicht, die rohen Bytes tun es.
        for roh in &bloecke {
            let b: myl_consensus::block::Block = borsh::from_slice(roh).expect("Block");
            historie.anhaengen(&b.hash());
        }
    }
    // Der Abschnitt endet an der letzten fertigen Gruppe; der Rest bis zur
    // Grenze steht in den Blöcken der Kette.
    let mut h = anzahl * 1024;
    while h < grenze {
        let bl = k.bloecke_von_bis(h, h).pop().expect("Block");
        historie.anhaengen(&bl.hash());
        h += 1;
    }
    let ablage_z = Ablage::neben(&d.join("kette.dat"), k.netz().startwert_hash());
    let (_, an_der_grenze) = ablage_z.lesen(grenze).expect("Abzug der Grenze");
    assert_eq!(historie, an_der_grenze.historie, "die wiederhergestellte Geschichte trifft die Historie nicht");
    let _ = std::fs::remove_dir_all(&d);
}

/// ⚑ **Eine kalte Ausfuhr kommt vor die verdichtete Kette** (Stufe A8):
/// geprüft gegen die Historienwurzel der Basis; danach lebt die Kette
/// wieder ab Genesis und rechnet auf dieselbe Wurzel. Eine Ausfuhr, die
/// nicht zur Basis führt, wird abgewiesen.
#[test]
fn eine_ausfuhr_kommt_vor_die_verdichtete_kette() {
    use myl_node::archivausfuhr::{ausfuhr, einfuhr_vor_basis};
    let d = ordner("ausfuhr");
    let mut k = kette(&d, 10, None);
    let mut eigene = Vec::new();
    while k.hoehe() < 2 * PERIODE + 10 {
        if k.hoehe() >= PERIODE && k.hoehe().is_multiple_of(BLOECKE_JE_EPOCHE) {
            belegen(&mut k);
        }
        let b = k.baue_block();
        if b.header.height <= PERIODE {
            eigene.push(b);
        }
    }
    assert_eq!(k.basis(), PERIODE);
    let startwert = k.netz().startwert_hash();
    let netz = k.netz();
    let (wurzel, hoehe) = (k.zustandswurzel(), k.hoehe());
    drop(k);
    let aus = d.join("kalt");
    ausfuhr(&aus, &startwert, PERIODE, true, |ab, bis| eigene[(ab - 1) as usize..bis.min(ab + 511) as usize].to_vec())
        .expect("Ausfuhr");
    // Eine Ausfuhr, die vor der Basis endet, reicht nicht.
    let kurz = d.join("kurz");
    ausfuhr(&kurz, &startwert, PERIODE - 1, false, |ab, bis| eigene[(ab - 1) as usize..bis.min(ab + 511) as usize].to_vec())
        .expect("Ausfuhr");
    assert!(einfuhr_vor_basis(&kurz, &d.join("kette.dat"), netz).is_err());

    assert_eq!(einfuhr_vor_basis(&aus, &d.join("kette.dat"), netz).expect("Einfuhr"), PERIODE);
    let mut neu = Kette::probestand();
    for b in 1..=10u8 {
        let m = miner(b);
        neu.zustand_mut().miner.insert(m.miner_id, m);
    }
    let (mut s, w) = Kettenspeicher::oeffnen(&d.join("kette.dat"), startwert).expect("öffnen");
    assert_eq!(w.anzahl, hoehe);
    s.fuer_jeden_satz(|b| neu.uebernimm(&b).expect("nachspielen ab Genesis")).expect("lesen");
    assert_eq!(neu.zustandswurzel(), wurzel);
    let _ = std::fs::remove_dir_all(&d);
}
