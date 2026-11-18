use super::nachspielen;
use crate::abzug::Ablage;
use crate::kette::{probekonto, probeschluessel, Kette};
use crate::speicher::Kettenspeicher;
use myl_consensus::block::{Anweisung, Transaktion};

fn ordner(marke: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("myl-wiederanlauf-{marke}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("Ordner");
    p
}

/// Eine Kette mit Datei und Ablage über `n` Blöcke, mit einem Burn je
/// 50 Blöcke, damit der Zustand sich bewegt; `versatz` macht die Beträge
/// und damit die Kette verschieden.
fn gefuellt(pfad: &std::path::Path, n: u64, versatz: u64) -> Kette {
    let (speicher, _) = Kettenspeicher::oeffnen(pfad, Kette::startwert()).expect("Speicher");
    let mut k = Kette::probestand();
    k.speicher_setzen(speicher);
    k.ablage_setzen(Ablage::neben(pfad, Kette::startwert()));
    for i in 0..n {
        if i % 50 == 0 {
            let konto = (i % 8) as u8;
            let nonce = k.zustand().account(&probekonto(konto)).nonce;
            let tx = Transaktion::signiere(&Kette::startwert(), &probeschluessel(konto), nonce, Anweisung::Burn { betrag: 1_000 + i + versatz })
                .expect("signieren");
            k.aufnehmen(tx);
        }
        let _ = k.baue_block();
    }
    k
}

/// Nachspielen wie beim Start, mit dem eigenen Abzug `einsatz` (oder
/// keinem).
fn neu_geladen(pfad: &std::path::Path, einsatz: Option<u64>, ablage_von: &std::path::Path) -> (Kette, super::Nachgespielt) {
    let (mut speicher, _) = Kettenspeicher::oeffnen(pfad, Kette::startwert()).expect("Speicher");
    let mut k = Kette::probestand();
    k.ablage_setzen(Ablage::neben(pfad, Kette::startwert()));
    let einsatz = einsatz.map(|h| Ablage::neben(ablage_von, Kette::startwert()).lesen(h).expect("Abzug"));
    let n = nachspielen(&mut k, &mut speicher, einsatz);
    (k, n)
}

/// ⚑ **Der Wiederanlauf setzt beim jüngsten eigenen Abzug an** (Fund
/// 629): bis 3600 nur vorgespult, die hundert Blöcke danach übernommen,
/// und Höhe, letzter Hash und Zustandswurzel sind die des vollständigen
/// Nachrechnens. Die Blöcke nach dem Abzug gehen durch `uebernimm`; ein
/// falsch eingesetzter Zustand fiele dort an der ersten Wurzel auf.
#[test]
fn der_wiederanlauf_setzt_beim_juengsten_abzug_an() {
    let o = ordner("an");
    let pfad = o.join("kette.dat");
    let vorher = gefuellt(&pfad, 3700, 0);
    assert_eq!(Ablage::neben(&pfad, Kette::startwert()).hoehen(), vec![1800, 3600]);
    let (k, n) = neu_geladen(&pfad, Some(3600), &pfad);
    assert_eq!(n.verworfen, None);
    assert_eq!((n.abzug, n.vorgespult, n.uebernommen, n.abgelehnt), (Some(3600), 3600, 100, 0));
    assert_eq!((k.hoehe(), k.letzter_hash(), k.zustandswurzel()), (vorher.hoehe(), vorher.letzter_hash(), vorher.zustandswurzel()));
    let (voll, m) = neu_geladen(&pfad, None, &pfad);
    assert_eq!((m.abzug, m.uebernommen), (None, 3700));
    assert_eq!(voll.zustandswurzel(), k.zustandswurzel());
    let _ = std::fs::remove_dir_all(&o);
}

/// Ein Abzug, dessen Block nicht in der Datei steht, wird verworfen,
/// ebenso einer hinter ihrem Ende; der Aufrufer rechnet dann ab der
/// Basis.
#[test]
fn ein_fremder_abzug_wird_verworfen() {
    let o = ordner("fremd");
    let (eigen, fremd, lang) = (o.join("eigen.dat"), o.join("fremd.dat"), o.join("lang.dat"));
    gefuellt(&eigen, 2000, 0);
    gefuellt(&fremd, 2000, 7);
    gefuellt(&lang, 3700, 0);
    let (_, n) = neu_geladen(&eigen, Some(1800), &fremd);
    assert!(n.verworfen.as_deref().is_some_and(|g| g.contains("1800")), "{:?}", n.verworfen);
    let (_, n) = neu_geladen(&eigen, Some(3600), &lang);
    assert!(n.verworfen.as_deref().is_some_and(|g| g.contains("endet vor")), "{:?}", n.verworfen);
    let _ = std::fs::remove_dir_all(&o);
}
