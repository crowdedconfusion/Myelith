//! Gegenproben zur Modellwahl: Ordnung, Hardwarezeile, Netzeintraege.
//!
//! # ⚑ Mit eigenen Artefakten, nicht mit den echten
//!
//! Die Proben legen leere Verzeichnisse mit einer `model_config.json`
//! an. **Was hier zur Pruefung steht, ist die Wahl und nicht der
//! Bestand dieser Maschine**: Ein Test, der die vorhandenen Artefakte
//! liest, faellt aus, sobald jemand eines baut oder loescht.
//!
//! ⚠️ Der Katalog wird dabei **nicht** nachgebildet. Er liegt im
//! Repositorium, und die Proben, die ihn brauchen, sagen es und pruefen
//! die Eigenschaft statt der Zahl.

use myl_client::einstellungen::Einstellungen;
use myl_client::modelle::{liste, NETZ};

/// Legt `n` Artefaktverzeichnisse an und gibt den Ordner zurueck.
fn artefakte(marke: &str, namen: &[&str]) -> std::path::PathBuf {
    let ordner = std::env::temp_dir().join(format!("myl-modellwahl-{marke}"));
    let _ = std::fs::remove_dir_all(&ordner);
    for n in namen {
        let p = ordner.join(n);
        std::fs::create_dir_all(&p).expect("Artefaktordner");
        std::fs::write(p.join("model_config.json"), "{}").expect("model_config.json");
    }
    ordner
}

fn mit(ordner: &std::path::Path, gewaehlt: &str) -> Einstellungen {
    let mut e = Einstellungen::default();
    e.modell.artefakt = ordner.join(gewaehlt).display().to_string();
    e
}

/// **Aufsteigend nach Groesse, nicht nach Verzeichnisnamen.**
///
/// ⛑ Die Probe, an der die Festlegung des Projektinhabers vom
/// 2026-09-11 haengt. Nach Zeichenketten sortiert stuende
/// `myelith-30b-a3b` **vor** `myelith-4b`, weil `3` vor `4` kommt;
/// genau diese Reihe ist deshalb ausgewaehlt.
///
/// ⛔️ **Bis zum 2026-09-12 stand hier `myelith-14b` als Beispiel**,
/// und es war das schaerfere: `1` vor `4`. Das Modell ist entfernt, und
/// eine Probe gegen ein Modell, das es nicht gibt, prueft nichts. Die
/// verbliebene Reihe traegt denselben Fall, nur eine Stelle weiter.
#[test]
fn die_wahl_steigt_an() {
    let ordner = artefakte(
        "aufsteigend",
        &["myelith-30b-a3b", "myelith-4b", "myelith-0.6b"],
    );
    let e = mit(&ordner, "myelith-4b");

    let namen: Vec<String> =
        liste(&e).into_iter().filter(|m| !m.ueber_netz).map(|m| m.name).collect();

    assert_eq!(
        namen,
        vec!["Myelith 0,6B", "Myelith 4B", "Myelith 30B-A3B"],
        "die Wahl steht nicht aufsteigend"
    );
    let _ = std::fs::remove_dir_all(&ordner);
}

/// **Jedes gefundene Modell steht auch als Netzeintrag da, gesperrt.**
#[test]
fn jedes_modell_steht_auch_im_netz() {
    let ordner = artefakte("netz", &["myelith-0.6b", "myelith-4b"]);
    let e = mit(&ordner, "myelith-4b");
    let alle = liste(&e);

    let hier: Vec<&str> =
        alle.iter().filter(|m| !m.ueber_netz).map(|m| m.pfad.as_str()).collect();
    let dort: Vec<&myl_client::modelle::Modellwahl> =
        alle.iter().filter(|m| m.ueber_netz).collect();

    assert_eq!(hier.len(), 2, "die beiden oertlichen Artefakte fehlen");
    assert_eq!(dort.len(), 2, "je Modell gehoert ein Netzeintrag dazu");

    for m in &dort {
        assert!(!m.offen, "der Netzeintrag {} ist waehlbar, obwohl nichts verdrahtet ist", m.name);
        assert!(!m.warum.is_empty(), "ein gesperrter Eintrag ohne Grund ist schlimmer als keiner");
        assert!(m.pfad.starts_with(NETZ), "{} traegt nicht das Netzkennzeichen", m.pfad);
        assert!(
            m.name.contains("(API), kostet Inferenz-Credits"),
            "{} nennt die Kosten nicht",
            m.name
        );
        // ⚠️ Im Netz rechnet eine fremde Maschine; eine Anforderung an
        // die eigene stuende dort falsch.
        assert!(m.hardware.is_empty(), "{} nennt Hardware fuer eine fremde Maschine", m.name);
    }

    // ⚑ **Und die Netzeintraege kommen nach den oertlichen.** Wer
    // waehlt, soll zuerst sehen, was sofort geht.
    let erste_netz = alle.iter().position(|m| m.ueber_netz).expect("Netzeintrag");
    assert!(
        alle[erste_netz..].iter().all(|m| m.ueber_netz),
        "oertliche und Netzeintraege sind vermischt"
    );
    let _ = std::fs::remove_dir_all(&ordner);
}

/// **Das Netzkennzeichen nennt das Modell, nicht nur „Netz".**
///
/// ⛑ Vorher stand genau ein Sammeleintrag `netz` in der Wahl. Seit dem
/// 2026-09-11 soll jeder Nutzer **sein** Modell anbieten koennen, und
/// dann muss der Wert sagen, welches gemeint ist.
#[test]
fn das_netzkennzeichen_nennt_das_modell() {
    let ordner = artefakte("kennung", &["myelith-0.6b"]);
    let e = mit(&ordner, "myelith-0.6b");

    let netz: Vec<String> =
        liste(&e).into_iter().filter(|m| m.ueber_netz).map(|m| m.pfad).collect();

    assert_eq!(netz, vec![format!("{NETZ}myelith-0.6b")]);
    let _ = std::fs::remove_dir_all(&ordner);
}

/// **Was der Katalog kennt, traegt eine Hardwarezeile.**
///
/// ⚠️ Geprueft wird die **Eigenschaft** und nicht der Wortlaut: Die
/// Zahlen stehen im Katalog und duerfen sich mit einer Messung aendern,
/// ohne dass ein Test bricht.
#[test]
fn bekannte_modelle_nennen_ihre_mindestausstattung() {
    let ordner = artefakte("hardware", &["myelith-0.6b", "wasauchimmer"]);
    let e = mit(&ordner, "myelith-0.6b");
    let alle = liste(&e);

    // ⚑ Ohne Katalog im Baum gibt es den Eintrag nicht, und das ist
    // richtig: erfundene Angaben waeren schlimmer als keine.
    if let Some(m) = alle.iter().find(|m| m.name.contains("0,6B") && !m.ueber_netz) {
        assert!(
            m.hardware.contains("Arbeitsspeicher") && m.hardware.contains("Platte"),
            "die Hardwarezeile nennt nicht beides: {:?}",
            m.hardware
        );
    }

    let unbekannt = alle
        .iter()
        .find(|m| m.pfad.ends_with("wasauchimmer") && !m.ueber_netz)
        .expect("auch ein Artefakt ohne Katalogeintrag steht in der Wahl");
    assert_eq!(unbekannt.name, "wasauchimmer", "ohne Eintrag bleibt der Verzeichnisname");
    assert!(unbekannt.hardware.is_empty(), "fuer ein unbekanntes Modell darf nichts dastehen");
    let _ = std::fs::remove_dir_all(&ordner);
}

/// **Das eingestellte Modell steht genau einmal da.**
///
/// ⛑ Bis zum 2026-09-11 verglich die Wahl den eingestellten Pfad
/// **unaufgeloest** gegen die absoluten Pfade der Liste. Ein relativ
/// eingestelltes Artefakt stand deshalb zweimal in der Wahl: einmal
/// unter seinem Namen und einmal als „(eingestellt)".
#[test]
fn das_eingestellte_modell_steht_einmal_da() {
    let ordner = artefakte("einmal", &["myelith-0.6b", "myelith-4b"]);
    let mut e = Einstellungen::default();
    e.modell.artefakt = ordner.join("myelith-4b").display().to_string();

    let oertlich: Vec<_> = liste(&e).into_iter().filter(|m| !m.ueber_netz).collect();
    assert_eq!(oertlich.len(), 2, "die Wahl zaehlt doppelt: {oertlich:?}");
    assert!(
        !oertlich.iter().any(|m| m.name.contains("(eingestellt)")),
        "das eingestellte Modell liegt in der Liste und braucht keinen Rueckfalleintrag"
    );
    let _ = std::fs::remove_dir_all(&ordner);
}

/// **Ein eingestellter Pfad, den es nicht gibt, verschwindet nicht.**
#[test]
fn ein_fehlender_pfad_bleibt_sichtbar() {
    let ordner = artefakte("fehlend", &["myelith-0.6b"]);
    let mut e = Einstellungen::default();
    e.modell.artefakt = ordner.join("gibtesnicht").display().to_string();

    let alle = liste(&e);
    let rueckfall = alle
        .iter()
        .find(|m| m.name.contains("(eingestellt)"))
        .expect("der eingestellte Pfad fehlt in der Wahl");
    assert!(!rueckfall.offen, "ein Pfad ohne `model_config.json` darf nicht waehlbar sein");
    assert!(
        !alle.iter().any(|m| m.ueber_netz && m.pfad.contains("gibtesnicht")),
        "was hier nicht liegt, ist auch im Netz keine Zusage"
    );
    let _ = std::fs::remove_dir_all(&ordner);
}
