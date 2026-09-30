//! Die Werkzeugkiste `CAD` und ihr Skill.
//!
//! # ⚑ Zwei Sorten Proben, und warum die zweite `#[ignore]` traegt
//!
//! Was ohne FreeCAD pruefbar ist, laeuft immer: Die drei Manifeste lesen
//! sich, die Kiste bekommt die eingebauten Werkzeuge von `Advanced`, und
//! der Skill wird gefunden.
//!
//! Ob wirklich ein Teil entsteht, haengt an einem fremden Programm, das
//! nicht auf jeder Maschine liegt. Eine Probe, die dort still springt,
//! saehe aus wie bestanden; `ignored` steht in jeder Ausgabe. Gezielt:
//!
//! ```text
//! cargo test --test cadkiste -- --ignored
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

fn wurzel() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("Wurzel")
}

fn kiste() -> PathBuf {
    wurzel().join("CLIENT/werkzeugkisten/CAD")
}

#[test]
fn die_drei_werkzeuge_lesen_sich_und_rufen_den_einen_weg() {
    let mut warnungen = Vec::new();
    let manifeste = myl_client::kisten::manifeste_lesen(&kiste(), |w| warnungen.push(w));
    assert!(warnungen.is_empty(), "ein Manifest der Kiste liest sich nicht: {warnungen:?}");
    let namen: Vec<&str> = manifeste.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(namen, ["cad_bauen", "cad_parameter", "cad_pruefen"]);
    for m in &manifeste {
        // ⚑ Ueber `$MYL_KISTE` und nicht ueber einen festen Pfad, und alle
        //   drei ueber dasselbe Skript: Dort steht, wie FreeCAD gefunden wird.
        assert!(m.befehl.starts_with("sh \"$MYL_KISTE/cad.sh\" "), "{}: {}", m.name, m.befehl);
        // Ein kalter Start von FreeCAD braucht mehr als die Vorgabe.
        assert!(m.frist_s() >= 60, "{}: {} s reichen einem kalten Start nicht", m.name, m.frist_s());
    }
    for datei in ["cad.sh", "cad_lauf.py", "myl_cad.py", "README.md"] {
        assert!(kiste().join(datei).is_file(), "{datei} fehlt in der Kiste");
    }
    assert_eq!(
        myl_client::kisten::eingebaute_des_ordners(Some(&kiste())),
        Some(myl_client::werkzeuge::Werkzeugkiste::Advanced),
        "ohne `kiste.json` fiele ein Ordner namens CAD still auf Base zurueck"
    );
}

#[test]
fn der_skill_wird_unter_seinen_stichworten_gefunden() {
    let orte = myl_client::skills::Orte {
        projekt: None,
        eigene: PathBuf::from("/gibt/es/nicht"),
        mitgeliefert: Some(wurzel().join("CLIENT/myl-skills")),
    };
    for anfrage in ["freecad", "3d bauteil", "parametric part"] {
        let treffer = myl_client::skills::suchen_in(&orte, anfrage, 5);
        assert!(
            treffer.iter().any(|t| t.skill.name == "cad-erstellen"),
            "\"{anfrage}\" findet den Skill nicht"
        );
    }
    let gelernt = myl_client::skills::lernen_in(&orte, "cad-erstellen", Some("vorlagen/flansch.py")).expect("Vorlage");
    assert!(gelernt.text.contains("from myl_cad import *"));
    // Jeder Baustein, den der Skill nennt, steht in der Kiste.
    let bausteine = std::fs::read_to_string(kiste().join("myl_cad.py")).expect("myl_cad.py");
    let skill = myl_client::skills::lernen_in(&orte, "cad-erstellen", None).expect("Skill").text;
    for name in ["dokument", "parameter", "koerper", "quader", "zylinder", "ausschnitt", "bohrung", "verrunden", "fase", "reihe", "kreis"] {
        assert!(bausteine.contains(&format!("\ndef {name}(")), "der Baustein {name} fehlt in myl_cad.py");
        assert!(skill.contains(&format!("{name}(")), "der Skill nennt den Baustein {name} nicht");
    }
}

/// Ruft ein Werkzeug der Kiste, wie der Agent es riefe: die Vorlage des
/// Manifests mit den Argumenten, im Arbeitsordner, mit `MYL_KISTE`.
fn rufen(arbeit: &Path, name: &str, argumente: serde_json::Value) -> (bool, String) {
    let manifeste = myl_client::kisten::manifeste_lesen(&kiste(), |_| {});
    let m = manifeste.iter().find(|m| m.name == name).expect("Werkzeug");
    let befehl = myl_client::kisten::befehl_aus_vorlage(&m.befehl, &argumente).expect("Vorlage");
    let aus = Command::new("sh")
        .arg("-c")
        .arg(&befehl)
        .current_dir(arbeit)
        .env(myl_client::kisten::UMGEBUNG_KISTE, kiste())
        .output()
        .expect("sh");
    (aus.status.success(), String::from_utf8_lossy(&aus.stdout).to_string())
}

/// Der Python-Block aus dem Skill: Was dort als Beispiel steht, muss bauen.
fn beispiel_aus_dem_skill() -> String {
    let text = std::fs::read_to_string(wurzel().join("CLIENT/myl-skills/cad-erstellen/SKILL.md")).expect("SKILL.md");
    let ab = text.find("```python").expect("kein Python-Block im Skill") + "```python".len();
    let bis = ab + text[ab..].find("```").expect("der Block endet nicht");
    text[ab..bis].lines().map(|z| z.strip_prefix("   ").unwrap_or(z)).collect::<Vec<_>>().join("\n")
}

#[test]
#[ignore = "braucht FreeCAD; cargo test --test cadkiste -- --ignored"]
fn ein_teil_entsteht_laesst_sich_aendern_und_fehler_schreiben_nichts() {
    let ordner = tempfile::tempdir().expect("Ordner");
    let arbeit = ordner.path();
    std::fs::write(arbeit.join("halter.py"), beispiel_aus_dem_skill()).expect("schreiben");

    // Bauen: 60 x 40 x 6, zwei Bohrungen.
    let (gut, bericht) = rufen(arbeit, "cad_bauen", serde_json::json!({"skript": "halter.py", "name": "halter"}));
    assert!(gut, "das Beispiel aus dem Skill baut nicht:\n{bericht}");
    assert!(bericht.contains("60 x 40 x 6 mm"), "{bericht}");
    assert!(bericht.contains("breite = 60 mm") && bericht.contains("Lochreihe (PartDesign::LinearPattern)"), "{bericht}");
    // ⚑ Im Fenster von FreeCAD sichtbar: der Koerper und sein letzter Schritt
    //   (ohne die beigelegte Ansicht blendete FreeCAD alles aus).
    assert!(bericht.contains("SICHTBAR in FreeCAD: Halter, Lochreihe"), "{bericht}");
    assert!(bericht.trim_end().ends_with("ERGEBNIS: gelungen"));
    for datei in ["halter.FCStd", "halter.step"] {
        assert!(arbeit.join(datei).metadata().map(|m| m.len() > 0).unwrap_or(false), "{datei} fehlt");
    }

    // Aendern: Das Teil folgt dem Parameter, also ist es parametrisch.
    let (gut, bericht) =
        rufen(arbeit, "cad_parameter", serde_json::json!({"datei": "halter.FCStd", "werte": "breite=90; dicke=8 mm"}));
    assert!(gut, "{bericht}");
    assert!(bericht.contains("breite: 60 mm -> 90 mm"), "eine nackte Zahl behaelt ihre Einheit nicht:\n{bericht}");
    assert!(bericht.contains("90 x 40 x 8 mm"), "das Teil folgt dem Parameter nicht:\n{bericht}");

    // Ueberschreiben legt keine Sicherungsdatei von FreeCAD daneben.
    let reste: Vec<String> = std::fs::read_dir(arbeit)
        .expect("Ordner")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
        .filter(|n| n.ends_with(".FCBak") || n.contains("myl-neu"))
        .collect();
    assert!(reste.is_empty(), "liegen geblieben: {reste:?}");

    // Und es steht so in der Datei.
    let (gut, bericht) = rufen(arbeit, "cad_pruefen", serde_json::json!({"datei": "halter.FCStd"}));
    assert!(gut && bericht.contains("90 x 40 x 8 mm"), "{bericht}");

    // Ein Wert, mit dem sich das Teil nicht rechnen laesst, schreibt nichts.
    let vorher = std::fs::read(arbeit.join("halter.FCStd")).expect("lesen");
    let (gut, bericht) = rufen(arbeit, "cad_parameter", serde_json::json!({"datei": "halter.FCStd", "werte": "rand=500"}));
    assert!(!gut && bericht.contains("Nichts geschrieben"), "{bericht}");
    assert_eq!(std::fs::read(arbeit.join("halter.FCStd")).expect("lesen"), vorher, "die Datei ist angefasst");
    let (gut, bericht) = rufen(arbeit, "cad_parameter", serde_json::json!({"datei": "halter.FCStd", "werte": "gibtsnicht=1"}));
    assert!(!gut && bericht.contains("vorhanden: breite, tiefe, dicke, bohrung, rand"), "{bericht}");

    // Ein Fehler im Skript nennt die Zeile und schreibt nichts.
    std::fs::write(arbeit.join("kaputt.py"), "from myl_cad import *\ndoc = dokument(\"X\")\nk = koerper(doc)\nquader(k, \"A\", 1, 2)\n")
        .expect("schreiben");
    let (gut, bericht) = rufen(arbeit, "cad_bauen", serde_json::json!({"skript": "kaputt.py", "name": "kaputt"}));
    assert!(!gut && bericht.contains("FEHLER im Skript in Zeile 4"), "{bericht}");
    assert!(!arbeit.join("kaputt.FCStd").exists());

    // Der Ausgabename ist ein Name und kein Pfad, auch mit Leerzeichen und
    // Anfuehrungszeichen im Argument.
    for name in ["../draussen", "a b", "x\"; touch gehackt; \""] {
        let (gut, bericht) = rufen(arbeit, "cad_bauen", serde_json::json!({"skript": "halter.py", "name": name}));
        assert!(!gut && bericht.contains("FEHLER: name="), "{name}: {bericht}");
    }
    assert!(!arbeit.join("gehackt").exists() && !arbeit.parent().expect("Eltern").join("draussen.FCStd").exists());
}
