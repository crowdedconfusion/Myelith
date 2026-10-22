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

/// **Ein Artefakt, das auf der Netzliste steht**: die drei Dateien des
/// Abdrucks (`theta_v.json`, `model_config.json`, `tokenizer.json`) vom
/// echten `sparrow-0.6b-int8`. Der Abdruck deckt genau diese drei, also steht das
/// Probe-Artefakt mit demselben Abdruck auf der Liste.
///
/// ⚑ Ohne das echte Artefakt (CI) oder mit `MYL_OHNE_ARTEFAKTE`: `None`, und
/// die Probe prueft nur die andere Seite.
fn freigegebenes(marke: &str) -> Option<std::path::PathBuf> {
    let echt = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../INTEGER_LLM/artifacts/sparrow-0.6b-int8");
    if std::env::var_os("MYL_OHNE_ARTEFAKTE").is_some() || !echt.join("theta_v.json").is_file() {
        return None;
    }
    let ordner = std::env::temp_dir().join(format!("myl-modellwahl-{marke}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ordner);
    let ziel = ordner.join("sparrow-0.6b-int8");
    std::fs::create_dir_all(&ziel).expect("Artefaktordner");
    for d in integer_llm_runtime::netzfreigabe::ABDRUCK_DATEIEN {
        std::fs::copy(echt.join(d), ziel.join(d)).expect("Datei des Abdrucks");
    }
    Some(ordner)
}

fn mit(ordner: &std::path::Path, gewaehlt: &str) -> Einstellungen {
    let mut e = Einstellungen::default();
    e.modell.artefakt = ordner.join(gewaehlt).display().to_string();
    e
}

/// **Aufsteigend nach Groesse, nicht nach Verzeichnisnamen.**
///
/// 📌 Die Probe, an der die Festlegung des Projektinhabers vom
/// 2026-09-11 haengt. Nach Zeichenketten sortiert stuende
/// `magpie-30b-a3b-int8` **vor** `pigeon-4b-int8`, weil `3` vor `4` kommt;
/// genau diese Reihe ist deshalb ausgewaehlt.
///
/// ⛔️ **Bis zum 2026-09-12 stand hier `myelith-14b` als Beispiel**,
/// und es war das schaerfere: `1` vor `4`. Das Modell ist entfernt, und
/// eine Probe gegen ein Modell, das es nicht gibt, prueft nichts. Die
/// verbliebene Reihe traegt denselben Fall, nur eine Stelle weiter.
/// ⚑ **Auch ein Artefakt ohne Katalogeintrag heisst nach dem Schema**
/// (2026-10-17): In der Wahl stand bis CLIENT v0.168.0 der Ordnername.
#[test]
fn ein_artefakt_ohne_katalog_heisst_nach_dem_schema() {
    let ordner = artefakte("schema", &["crow-27b-ternaer"]);
    let e = mit(&ordner, "crow-27b-ternaer");
    let namen: Vec<String> = liste(&e).into_iter().filter(|m| !m.ueber_netz).map(|m| m.name).collect();
    // ⚑ Seit CLIENT v0.175.0 privat: Es steht nicht auf der Netzliste.
    assert_eq!(namen, vec!["Crow 27B - TERNÄR (privat)"]);
    let _ = std::fs::remove_dir_all(&ordner);
}

#[test]
fn die_wahl_steigt_an() {
    let ordner = artefakte(
        "aufsteigend",
        &["magpie-30b-a3b-int8", "pigeon-4b-int8", "sparrow-0.6b-int8"],
    );
    let e = mit(&ordner, "pigeon-4b-int8");

    let namen: Vec<String> =
        liste(&e).into_iter().filter(|m| !m.ueber_netz).map(|m| m.name).collect();

    assert_eq!(
        namen,
        // Die Probe-Artefakte stehen auf keiner Netzliste, also privat.
        vec!["Sparrow 0.6B - INT8 (privat)", "Pigeon 4B - INT8 (privat)", "Magpie 30B A3B - INT8 (privat)"],
        "die Wahl steht nicht aufsteigend"
    );
    let _ = std::fs::remove_dir_all(&ordner);
}

/// **Nur ein freigegebenes Modell steht auch als Netzeintrag da, gesperrt.**
///
/// ⛔️ **Seit CLIENT v0.175.0 (Fund 585):** Ein Artefakt, das nicht mit
/// seinem Abdruck auf der Netzliste steht, ist privat und bekommt keinen
/// Netzeintrag; bis dahin bekam ihn jedes.
#[test]
fn jedes_modell_steht_auch_im_netz() {
    let ordner = artefakte("netz", &["sparrow-0.6b-int8", "pigeon-4b-int8"]);
    let e = mit(&ordner, "pigeon-4b-int8");
    let alle = liste(&e);
    assert_eq!(alle.iter().filter(|m| !m.ueber_netz).count(), 2, "die beiden oertlichen Artefakte fehlen");
    assert_eq!(alle.iter().filter(|m| m.ueber_netz).count(), 0, "nicht freigegeben, also kein Netzeintrag");
    let _ = std::fs::remove_dir_all(&ordner);

    let Some(ordner) = freigegebenes("netz-frei") else { return };
    let e = mit(&ordner, "sparrow-0.6b-int8");
    let alle = liste(&e);

    let hier: Vec<&str> =
        alle.iter().filter(|m| !m.ueber_netz).map(|m| m.pfad.as_str()).collect();
    let dort: Vec<&myl_client::modelle::Modellwahl> =
        alle.iter().filter(|m| m.ueber_netz).collect();

    assert_eq!(hier.len(), 1, "das oertliche Artefakt fehlt");
    assert_eq!(dort.len(), 1, "zum freigegebenen Modell gehoert ein Netzeintrag");

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
/// 📌 Vorher stand genau ein Sammeleintrag `netz` in der Wahl. Seit dem
/// 2026-09-11 soll jeder Nutzer **sein** Modell anbieten koennen, und
/// dann muss der Wert sagen, welches gemeint ist.
#[test]
fn das_netzkennzeichen_nennt_das_modell() {
    // ⚑ Nur ein freigegebenes Modell hat einen Netzeintrag (Fund 585).
    let Some(ordner) = freigegebenes("kennung") else { return };
    let e = mit(&ordner, "sparrow-0.6b-int8");

    let netz: Vec<String> =
        liste(&e).into_iter().filter(|m| m.ueber_netz).map(|m| m.pfad).collect();

    assert_eq!(netz, vec![format!("{NETZ}sparrow-0.6b-int8")]);
    let _ = std::fs::remove_dir_all(&ordner);
}

/// **Was der Katalog kennt, traegt eine Hardwarezeile.**
///
/// ⚠️ Geprueft wird die **Eigenschaft** und nicht der Wortlaut: Die
/// Zahlen stehen im Katalog und duerfen sich mit einer Messung aendern,
/// ohne dass ein Test bricht.
#[test]
fn bekannte_modelle_nennen_ihre_mindestausstattung() {
    let ordner = artefakte("hardware", &["sparrow-0.6b-int8", "wasauchimmer"]);
    let e = mit(&ordner, "sparrow-0.6b-int8");
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
    assert_eq!(unbekannt.name, "wasauchimmer (privat)", "ohne Eintrag bleibt der Verzeichnisname, und es ist privat");
    assert!(unbekannt.hardware.is_empty(), "fuer ein unbekanntes Modell darf nichts dastehen");
    let _ = std::fs::remove_dir_all(&ordner);
}

/// **Das eingestellte Modell steht genau einmal da.**
///
/// 📌 Bis zum 2026-09-11 verglich die Wahl den eingestellten Pfad
/// **unaufgeloest** gegen die absoluten Pfade der Liste. Ein relativ
/// eingestelltes Artefakt stand deshalb zweimal in der Wahl: einmal
/// unter seinem Namen und einmal als „(eingestellt)".
#[test]
fn das_eingestellte_modell_steht_einmal_da() {
    let ordner = artefakte("einmal", &["sparrow-0.6b-int8", "pigeon-4b-int8"]);
    let mut e = Einstellungen::default();
    e.modell.artefakt = ordner.join("pigeon-4b-int8").display().to_string();

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
    let ordner = artefakte("fehlend", &["sparrow-0.6b-int8"]);
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
