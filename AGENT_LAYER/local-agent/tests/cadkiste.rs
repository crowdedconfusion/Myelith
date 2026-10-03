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
    wurzel().join("AGENT_LAYER/local-toolkits/CAD")
}

#[test]
fn die_drei_werkzeuge_lesen_sich_und_rufen_den_einen_weg() {
    let mut warnungen = Vec::new();
    let manifeste = myl_local_agent::kisten::manifeste_lesen(&kiste(), |w| warnungen.push(w));
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
        myl_local_agent::kisten::eingebaute_des_ordners(Some(&kiste())),
        Some(myl_local_agent::werkzeuge::Werkzeugkiste::Advanced),
        "ohne `kiste.json` fiele ein Ordner namens CAD still auf Base zurueck"
    );
}

#[test]
fn der_skill_wird_unter_seinen_stichworten_gefunden() {
    let orte = myl_local_agent::skills::Orte {
        projekt: None,
        eigene: PathBuf::from("/gibt/es/nicht"),
        mitgeliefert: Some(wurzel().join("AGENT_LAYER/local-skills")),
    };
    for anfrage in ["freecad", "3d bauteil", "parametric part"] {
        let treffer = myl_local_agent::skills::suchen_in(&orte, anfrage, 5);
        assert!(
            treffer.iter().any(|t| t.skill.name == "cad-erstellen"),
            "\"{anfrage}\" findet den Skill nicht"
        );
    }
    let gelernt = myl_local_agent::skills::lernen_in(&orte, "cad-erstellen", Some("vorlagen/flansch.py")).expect("Vorlage");
    assert!(gelernt.text.contains("from myl_cad import *"));
    // Jeder Baustein, den der Skill nennt, steht in der Kiste.
    let bausteine = std::fs::read_to_string(kiste().join("myl_cad.py")).expect("myl_cad.py");
    let skill = myl_local_agent::skills::lernen_in(&orte, "cad-erstellen", None).expect("Skill").text;
    for name in ["dokument", "parameter", "koerper", "quader", "zylinder", "ausschnitt", "bohrung", "verrunden", "fase", "reihe", "kreis"] {
        assert!(bausteine.contains(&format!("\ndef {name}(")), "der Baustein {name} fehlt in myl_cad.py");
        assert!(skill.contains(&format!("{name}(")), "der Skill nennt den Baustein {name} nicht");
    }
}

/// Ruft ein Werkzeug der Kiste, wie der Agent es riefe: die Vorlage des
/// Manifests mit den Argumenten, im Arbeitsordner, mit `MYL_KISTE`.
fn rufen(arbeit: &Path, name: &str, argumente: serde_json::Value) -> (bool, String) {
    let manifeste = myl_local_agent::kisten::manifeste_lesen(&kiste(), |_| {});
    let m = manifeste.iter().find(|m| m.name == name).expect("Werkzeug");
    let befehl = myl_local_agent::kisten::befehl_aus_vorlage(&m.befehl, &argumente).expect("Vorlage");
    let aus = Command::new("sh")
        .arg("-c")
        .arg(&befehl)
        .current_dir(arbeit)
        .env(myl_local_agent::kisten::UMGEBUNG_KISTE, kiste())
        .output()
        .expect("sh");
    (aus.status.success(), String::from_utf8_lossy(&aus.stdout).to_string())
}

/// Der Python-Block aus dem Skill: Was dort als Beispiel steht, muss bauen.
fn beispiel_aus_dem_skill() -> String {
    let text = std::fs::read_to_string(wurzel().join("AGENT_LAYER/local-skills/cad-erstellen/SKILL.md")).expect("SKILL.md");
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
    // Jede Bohrung liegt ganz im Material: keine Warnung, und die Bilanz steht da.
    assert!(!bericht.contains("WARNUNGEN"), "{bericht}");
    assert!(bericht.contains("Loch (PartDesign::SubtractiveCylinder): bei (10, 10, 0) entfernt"), "{bericht}");
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

/// ⛔️ **Ein Schnitt, der sein Volumen nicht entfernt, wird gemeldet**
/// (2026-10-01). Das 35B legte Bohrungen um (0, 0), als laege dort die Mitte
/// der Platte; drei schnitten nichts, eine ein Viertel, und das Modell hielt
/// sein Kopfrechnen fuer eine Pruefung. Gebaut wird trotzdem: Ein Loch am
/// Rand kann gewollt sein.
#[test]
#[ignore = "braucht FreeCAD; cargo test --test cadkiste -- --ignored"]
fn ein_schnitt_ausserhalb_des_materials_wird_gewarnt() {
    let ordner = tempfile::tempdir().expect("Ordner");
    let arbeit = ordner.path();
    std::fs::write(
        arbeit.join("platte.py"),
        "from myl_cad import *\n\
         doc = dokument(\"Platte\")\n\
         parameter(doc, laenge=60, breite=60, dicke=6)\n\
         k = koerper(doc, \"Platte\")\n\
         quader(k, \"Grund\", \"laenge\", \"breite\", \"dicke\")\n\
         bohrung(k, \"Mitte\", 5, \"dicke\", bei=(\"laenge / 2\", \"breite / 2\", 0))\n\
         bohrung(k, \"Ecke\", 5, \"dicke\", bei=(0, 0, 0))\n\
         bohrung(k, \"Daneben\", 2, \"dicke\", bei=(-20, 30, 0))\n\
         bohrung(k, \"Durch\", 3, \"dicke + 20 mm\", bei=(45, 45, -10))\n\
         quader(k, \"Block\", 20, 20, 30, bei=(20, 20, \"dicke\"))\n\
         bohrung(k, \"Halb\", 4, 24, bei=(30, 30, 21), achse=\"y\")\n",
    )
    .expect("schreiben");
    let (gut, bericht) = rufen(arbeit, "cad_bauen", serde_json::json!({"skript": "platte.py", "name": "platte"}));
    assert!(gut, "{bericht}");
    let warnungen = &bericht[bericht.find("WARNUNGEN:").unwrap_or_else(|| panic!("keine Warnung:\n{bericht}"))..];
    assert!(warnungen.contains("Ecke bei (0, 0, 0) liegt nur zu 25 % im Teil"), "{bericht}");
    // Und sie sagt, wo das Teil an dieser Stelle ist.
    assert!(warnungen.contains("an dieser Stelle reicht das Teil entlang z von 0 bis 6"), "{bericht}");
    assert!(warnungen.contains("Daneben bei (-20, 30, 0) liegt nur zu 0 % im Teil (ganz daneben, entfernt nichts); an dieser Stelle ist entlang z kein Material"), "{bericht}");
    assert!(warnungen.contains("Daneben bei (-20, 30, 0) liegt nur zu 0 % im Teil (ganz daneben, entfernt nichts)"), "{bericht}");
    // Gegenproben: Die Bohrung in der Mitte ist ganz im Material, und eine
    // durchgehende Bohrung mit Ueberlaenge ist gewollt; beide stehen nicht darunter.
    assert!(!warnungen.contains("Mitte bei"), "{bericht}");
    assert!(!warnungen.contains("Durch bei"), "{bericht}");
    assert!(bericht.contains("(ragt nur entlang seiner Achse ueber das Teil hinaus"), "{bericht}");
    // 📌 Lagerbock (2026-10-01): eine Querbohrung ab der Mitte des Blocks
    // geht nur halb durch und wird gemeldet, mit beiden Bereichen.
    assert!(
        warnungen.contains("Halb bei (30, 30, 21) ragt auf einer Seite aus dem Teil und endet auf der anderen im Material: Der Schnitt reicht entlang y von 30 bis 54, das Teil dort von 20 bis 40"),
        "{bericht}"
    );
    assert!(bericht.trim_end().ends_with("ERGEBNIS: gelungen"));
}

/// ⚑ **Ein runder Flansch mit allem, was die Bibliothek seit dem 2026-10-01
/// kann**: eine Fase auf einer Höhe, die nicht die oberste ist, nur am
/// äußeren Rand; ein Kreismuster aus Loch **und** Senkung; und eine Senkung
/// über einer Durchgangsbohrung, die nicht als Schnitt ins Leere gilt.
/// Volumen von Hand: 104 336,10 mm³.
#[test]
#[ignore = "braucht FreeCAD; cargo test --test cadkiste -- --ignored"]
fn ein_runder_flansch_mit_fase_auf_hoehe_und_lochkreis_mit_senkungen() {
    let ordner = tempfile::tempdir().expect("Ordner");
    let arbeit = ordner.path();
    std::fs::write(
        arbeit.join("flansch.py"),
        r#"from myl_cad import *
doc = dokument("Rundflansch")
parameter(doc, d_teller=100, dicke=12, d_bund=40, h_bund=20, d_mitte=20,
          lochkreis=75, d_loch=6.6, d_senk=11, t_senk=6.5, fase_mass=1, anzahl="6")
k = koerper(doc, "Flansch")
zylinder(k, "Teller", "d_teller / 2", "dicke")
fase(k, "Fase", "fase_mass", kanten="oben", hoehe="dicke", aussen=True)
zylinder(k, "Bund", "d_bund / 2", "h_bund", bei=(0, 0, "dicke"))
bohrung(k, "Mitte", "d_mitte / 2", "dicke + h_bund")
loch = bohrung(k, "Loch", "d_loch / 2", "dicke", bei=("lochkreis / 2", 0, 0))
senk = bohrung(k, "Senkung", "d_senk / 2", "t_senk", bei=("lochkreis / 2", 0, "dicke - t_senk"))
kreis(k, "Lochkreis", [loch, senk], "anzahl")
"#,
    )
    .expect("schreiben");
    let (gut, bericht) = rufen(arbeit, "cad_bauen", serde_json::json!({"skript": "flansch.py", "name": "flansch"}));
    assert!(gut, "{bericht}");
    assert!(bericht.contains("100 x 100 x 32 mm") && bericht.contains("Volumen 104336.104 mm3"), "{bericht}");
    assert!(bericht.contains("Fase (PartDesign::Chamfer): aendert das Volumen um -156.032 mm3"), "{bericht}");
    assert!(bericht.contains("(der Rest faellt in schon entferntes Material)"), "{bericht}");
    assert!(!bericht.contains("WARNUNGEN"), "Fehlalarm:\n{bericht}");
    // Und ein Parameter aendert die Zahl der Loecher. Von Hand: ein Loch mit
    // Senkung entfernt 410,543 + 617,716 - 222,378 = 805,881 mm3; das Muster
    // fuegt bei 6 Loechern 5 Kopien hinzu (-4 029,407), bei 8 sieben.
    assert!(bericht.contains("Lochkreis (PartDesign::PolarPattern): aendert das Volumen um -4029.407 mm3"), "{bericht}");
    let (gut, bericht) = rufen(arbeit, "cad_parameter", serde_json::json!({"datei": "flansch.FCStd", "werte": "anzahl=8"}));
    assert!(gut && bericht.contains("Lochkreis (PartDesign::PolarPattern): aendert das Volumen um -5641.169 mm3"), "{bericht}");
}

/// 📌 **Eine Zahl ohne Einheit in einer Laenge bekommt mm** (2026-10-01,
/// Lagerbock: drei Bauten fuer `"platte_hoehe + 35"` und Verwandte); in
/// einem Winkel sagt die Meldung, was zu tun ist.
#[test]
#[ignore = "braucht FreeCAD; cargo test --test cadkiste -- --ignored"]
fn eine_zahl_ohne_einheit_wird_ergaenzt_oder_erklaert() {
    let ordner = tempfile::tempdir().expect("Ordner");
    let arbeit = ordner.path();
    std::fs::write(
        arbeit.join("t.py"),
        "from myl_cad import *\ndoc = dokument(\"T\")\nparameter(doc, dicke=12)\nk = koerper(doc, \"K\")\n\
         zylinder(k, \"Z\", 5, \"dicke - 2\", bei=(\"dicke + 8\", 0, 0))\n",
    )
    .expect("schreiben");
    let (gut, bericht) = rufen(arbeit, "cad_bauen", serde_json::json!({"skript": "t.py", "name": "t"}));
    assert!(gut && bericht.contains("10 x 10 x 10 mm, von (15, -5, 0)"), "{bericht}");
    std::fs::write(
        arbeit.join("w.py"),
        "from myl_cad import *\ndoc = dokument(\"W\")\nparameter(doc, dicke=12, winkel=\"360 deg\")\nk = koerper(doc, \"K\")\n\
         zylinder(k, \"Z\", 50, \"dicke\")\nl = bohrung(k, \"L\", 3, \"dicke\", bei=(30, 0, 0))\n\
         kreis(k, \"Kreis\", l, \"4\", winkel=\"winkel - 10\")\n",
    )
    .expect("schreiben");
    let (gut, bericht) = rufen(arbeit, "cad_bauen", serde_json::json!({"skript": "w.py", "name": "w"}));
    assert!(!gut && bericht.contains("\"winkel - 10 deg\", nicht \"winkel - 10\""), "{bericht}");
}

/// ⚑ **Grossgeschrieben und mit Umlaut bauen dieselben Bausteine**
/// (2026-10-01, Montagewinkel: `Dokument` und `Koerper` kosteten zwei
/// Bauten); ein unbekannter Name nennt die Bausteine.
#[test]
#[ignore = "braucht FreeCAD; cargo test --test cadkiste -- --ignored"]
fn die_bausteine_heissen_auch_wie_ein_modell_sie_schreibt() {
    let ordner = tempfile::tempdir().expect("Ordner");
    let arbeit = ordner.path();
    std::fs::write(
        arbeit.join("a.py"),
        "from myl_cad import *\ndoc = Dokument(\"A\")\nParameter(doc, a=10)\nk = Körper(doc, \"K\")\nQuader(k, \"Q\", \"a\", \"a\", \"a\")\nBohrung(k, \"B\", 2, \"a\", bei=(5, 5, 0))\n",
    )
    .expect("schreiben");
    let (gut, bericht) = rufen(arbeit, "cad_bauen", serde_json::json!({"skript": "a.py", "name": "a"}));
    assert!(gut && bericht.contains("Volumen 874.336 mm3"), "{bericht}");
    std::fs::write(arbeit.join("b.py"), "from myl_cad import *\ndoc = dokument(\"B\")\nk = koerper(doc)\nwuerfel(k, \"W\", 1, 1, 1)\n")
        .expect("schreiben");
    let (gut, bericht) = rufen(arbeit, "cad_bauen", serde_json::json!({"skript": "b.py", "name": "b"}));
    assert!(!gut && bericht.contains("Die Bausteine heissen: dokument, parameter, koerper"), "{bericht}");
}

/// 📌 **Ein Python-Name in einem Ausdruck-Text wird beim Namen genannt**
/// (2026-10-01, NEMA-17 mit Saat 2: `h = "lochabstand / 2"`, dann
/// `"breite / 2 - h"`; FreeCAD sagte nur „Failed to parse expression“).
#[test]
#[ignore = "braucht FreeCAD; cargo test --test cadkiste -- --ignored"]
fn ein_fremder_name_im_ausdruck_wird_genannt() {
    let ordner = tempfile::tempdir().expect("Ordner");
    let arbeit = ordner.path();
    std::fs::write(
        arbeit.join("h.py"),
        "from myl_cad import *\ndoc = dokument(\"H\")\nparameter(doc, breite=60, lochabstand=31)\nk = koerper(doc)\n\
         quader(k, \"Q\", \"breite\", \"breite\", 5)\nh = \"lochabstand / 2\"\nbohrung(k, \"L\", 2, 5, bei=(\"breite / 2 - h\", 10, 0))\n",
    )
    .expect("schreiben");
    let (gut, bericht) = rufen(arbeit, "cad_bauen", serde_json::json!({"skript": "h.py", "name": "h"}));
    assert!(!gut && bericht.contains("h ist kein Parameter (Parameter: breite, lochabstand)"), "{bericht}");
}

/// 📌 **Eine Zeile, die nur speichert, wird uebergangen** (2026-10-01,
/// Montagewinkel mit Saat 2: `doc.speichern()`, dann `doc.save()`, zwei
/// Fehlbauten); gespeichert wird vom Werkzeug, und der Bericht sagt es.
#[test]
#[ignore = "braucht FreeCAD; cargo test --test cadkiste -- --ignored"]
fn eine_zeile_die_nur_speichert_wird_uebergangen() {
    let ordner = tempfile::tempdir().expect("Ordner");
    let arbeit = ordner.path();
    std::fs::write(
        arbeit.join("s.py"),
        "from myl_cad import *\ndoc = dokument(\"S\")\nk = koerper(doc)\nquader(k, \"Q\", 10, 10, 10)\ndoc.speichern()\ndoc.saveAs(\"fremd.FCStd\")\n",
    )
    .expect("schreiben");
    let (gut, bericht) = rufen(arbeit, "cad_bauen", serde_json::json!({"skript": "s.py", "name": "s"}));
    assert!(gut && bericht.contains("HINWEIS: Zeile 5 (doc.speichern()) uebergangen"), "{bericht}");
    assert!(arbeit.join("s.FCStd").exists() && !arbeit.join("fremd.FCStd").exists());
}

/// ⚑ **Rueckwaerts bohren** (2026-10-01, Gehaeuse mit Saat 3: ein Kernloch
/// von oben, `achse="-z"`). Von Hand: 8 000 - pi * 4 * (10 + 5 + 5).
#[test]
#[ignore = "braucht FreeCAD; cargo test --test cadkiste -- --ignored"]
fn eine_bohrung_laeuft_auch_rueckwaerts() {
    let ordner = tempfile::tempdir().expect("Ordner");
    let arbeit = ordner.path();
    std::fs::write(
        arbeit.join("r.py"),
        "from myl_cad import *\ndoc = dokument(\"R\")\nparameter(doc, a=20)\nk = koerper(doc)\nquader(k, \"Q\", \"a\", \"a\", \"a\")\n\
         bohrung(k, \"Z\", 2, 10, bei=(10, 10, \"a\"), achse=\"-z\")\nbohrung(k, \"X\", 2, 5, bei=(\"a\", 10, 10), achse=\"-x\")\n\
         bohrung(k, \"Y\", 2, 5, bei=(5, \"a\", 15), achse=\"-y\")\n",
    )
    .expect("schreiben");
    let (gut, bericht) = rufen(arbeit, "cad_bauen", serde_json::json!({"skript": "r.py", "name": "r"}));
    assert!(gut && bericht.contains("Volumen 7748.673 mm3") && !bericht.contains("WARNUNGEN"), "{bericht}");
}

/// ⚑ **Ein Raster statt einer Reihe einer Reihe** (2026-10-01, NEMA-17 mit
/// Saat 3: sieben Fehlbauten an `reihe` auf eine Reihe). Von Hand: 21 600 -
/// 4 * pi * 1,7^2 * 6 - 4 * (16 - 4 pi) * 6; nach a = 35 dasselbe Volumen,
/// die Loecher wandern.
#[test]
#[ignore = "braucht FreeCAD; cargo test --test cadkiste -- --ignored"]
fn ein_raster_ersetzt_die_reihe_einer_reihe() {
    let ordner = tempfile::tempdir().expect("Ordner");
    let arbeit = ordner.path();
    std::fs::write(
        arbeit.join("r.py"),
        "from myl_cad import *\ndoc = dokument(\"N\")\nparameter(doc, breite=60, tiefe=60, dicke=6, a=31, loch=3.4)\n\
         k = koerper(doc, \"Flansch\")\nquader(k, \"Platte\", \"breite\", \"tiefe\", \"dicke\")\n\
         loch = bohrung(k, \"Loch\", \"loch / 2\", \"dicke\", bei=(\"breite / 2 - a / 2\", \"tiefe / 2 - a / 2\", 0))\n\
         raster(k, \"Raster\", loch, \"2\", \"a\", \"2\", \"a\")\nverrunden(k, \"Ecken\", 4)\n",
    )
    .expect("schreiben");
    let (gut, bericht) = rufen(arbeit, "cad_bauen", serde_json::json!({"skript": "r.py", "name": "r"}));
    assert!(gut && bericht.contains("Volumen 21299.692 mm3") && !bericht.contains("Raster_x"), "{bericht}");
    let (gut, bericht) = rufen(arbeit, "cad_parameter", serde_json::json!({"datei": "r.FCStd", "werte": "a=35"}));
    assert!(gut && bericht.contains("Volumen 21299.692 mm3") && bericht.contains("bei (12.5, 12.5, 0)"), "{bericht}");
    // Die Reihe einer Reihe nennt den Weg.
    std::fs::write(
        arbeit.join("s.py"),
        "from myl_cad import *\ndoc = dokument(\"S\")\nk = koerper(doc)\nquader(k, \"P\", 60, 60, 6)\nl = bohrung(k, \"L\", 2, 6, bei=(10, 10, 0))\n\
         r = reihe(k, \"R\", l, \"2\", 30)\nreihe(k, \"R2\", r, \"2\", 30, richtung=\"y\")\n",
    )
    .expect("schreiben");
    let (gut, bericht) = rufen(arbeit, "cad_bauen", serde_json::json!({"skript": "s.py", "name": "s"}));
    assert!(!gut && bericht.contains("schon ein Muster") && bericht.contains("raster(k, name"), "{bericht}");
}
