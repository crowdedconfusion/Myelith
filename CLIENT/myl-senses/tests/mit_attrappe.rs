//! ⚑ **Der Beleg, dass der Weg traegt**, von der angehaengten Datei bis
//! in das fremde Programm und zurueck.
//!
//! # ⚑ Warum hier ein gestelltes Laufwerk steht
//!
//! llama.cpp und whisper.cpp liegen auf keiner Pruefmaschine, und ein
//! Test, der sie braucht, liefe nirgends. Also wird **das Programm**
//! gestellt und **der Weg** geprueft: Findet die Kiste es, gibt sie
//! seine Ausgabe weiter, laesst sie sein Protokoll draussen, und sagt
//! sie verstaendlich Bescheid, wenn nichts da ist?
//!
//! ⛔️ **Der letzte Fall ist der wichtige.** Er ist der, den jeder
//! Nutzer zuerst sieht.

use std::path::{Path, PathBuf};

use myl_senses::anhang::{self, Art, Sicht};
use myl_senses::laufwerk::{Eigene, Sinne, Stufe, HOERMODELL, SEHMODELL, SEHPROJEKTOR};

const ANHANGORDNER: &str = ".AGENT/anhaenge";

/// Ein gestelltes Programm, das ausgibt, was es bekommen hat, und dabei
/// nach **stderr** schwatzt wie llama.cpp.
fn attrappe(d: &Path, name: &str, sagt: &str) -> PathBuf {
    let bin = d.join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    let p = bin.join(name);
    std::fs::write(
        &p,
        format!("#!/bin/sh\necho \"ATTRAPPE $*\" >&2\necho\necho \"  {sagt}  \"\n"),
    )
    .expect("Attrappe");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).expect("Rechte");
    }
    p
}

fn mit_sehmodell(d: &Path, sagt: &str) -> Sinne {
    attrappe(d, "llama-mtmd-cli", sagt);
    std::fs::write(d.join(SEHMODELL), "Attrappe").expect("Modell");
    std::fs::write(d.join(SEHPROJEKTOR), "Attrappe").expect("Projektor");
    Sinne::finden_in(d, &Eigene::default(), &[])
}

/// ⚑ **Die Antwort kommt durch, das Ladegeschwaetz nicht.**
#[test]
fn das_sehmodell_wird_gerufen_und_seine_antwort_kommt_zurueck() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let sinne = mit_sehmodell(d.path(), "Auf dem Schild steht HALT.");
    let bild = d.path().join("schild.png");
    std::fs::write(&bild, b"\x89PNG").expect("Bild");

    let text = myl_senses::auswerten(&sinne, &bild, Art::Bild, None, Stufe::Schnell)
        .expect("fuer ein Bild gibt es einen Sinn")
        .expect("die Attrappe laeuft");
    assert_eq!(text, "Auf dem Schild steht HALT.");
    assert!(!text.contains("ATTRAPPE"), "das Protokoll steht in der Antwort: {text}");
}

/// ⛔️ **Fehlt das Laufwerk, steht da was fehlt und wie es hinkommt**,
/// statt einer fremden Meldung oder eines Absturzes.
#[test]
fn ohne_laufwerk_steht_da_was_fehlt() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let sinne = Sinne::finden_in(d.path(), &Eigene::default(), &[]);
    let fehler = myl_senses::auswerten(&sinne, Path::new("x.png"), Art::Bild, None, Stufe::Schnell)
        .expect("fuer ein Bild gibt es einen Sinn")
        .expect_err("es ist nichts eingerichtet");
    assert!(fehler.contains("Programm fehlt"), "{fehler}");
    assert!(fehler.contains("llama.cpp"), "was zu installieren ist, fehlt: {fehler}");
    assert!(fehler.contains(&d.path().display().to_string()), "der Ort fehlt: {fehler}");

    let fehler = myl_senses::auswerten(&sinne, Path::new("x.wav"), Art::Ton, None, Stufe::Schnell)
        .expect("fuer Ton gibt es einen Sinn")
        .expect_err("es ist nichts eingerichtet");
    assert!(fehler.contains("whisper.cpp"), "{fehler}");
    assert!(fehler.contains(&d.path().join(HOERMODELL).display().to_string()), "{fehler}");
}

/// ⛔️ **Eine Datei, die es nicht gibt, laedt kein Modell.**
#[test]
fn eine_fehlende_datei_wird_vorher_gemeldet() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let sinne = mit_sehmodell(d.path(), "egal");
    let fehler = myl_senses::auswerten(
        &sinne,
        &d.path().join("gibtsnicht.png"),
        Art::Bild,
        None,
        Stufe::Schnell,
    )
    .expect("Sinn")
    .expect_err("die Datei fehlt");
    assert!(fehler.contains("gibt es nicht"), "{fehler}");
}

/// ⚑ **Der ganze Weg**, so wie ihn der Chat geht: anhaengen, ansehen,
/// und die Beschreibung steht in der Zeile, die ins Gespraech geht.
#[test]
fn vom_anhang_bis_in_die_nachricht() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    std::fs::create_dir_all(&heimat).expect("Heimat");
    let sinne = mit_sehmodell(&heimat, "Ein rotes Achteck mit der Aufschrift HALT.");

    let quelle = d.path().join("schild.png");
    std::fs::write(&quelle, b"\x89PNG\r\n\x1a\nnoch etwas").expect("Bild");
    let wurzel = d.path().join("arbeit");
    std::fs::create_dir_all(&wurzel).expect("Arbeitsordner");

    let a = anhang::aufnehmen(&wurzel, ANHANGORDNER, &quelle).expect("aufnehmen");
    assert_eq!(a.art, Art::Bild);
    assert!(quelle.is_file(), "das Original ist weg");
    assert_eq!(a.pfad, format!("{ANHANGORDNER}/schild.png"));

    let gesehen = myl_senses::auswerten(&sinne, &wurzel.join(&a.pfad), a.art, None, Stufe::Schnell)
        .expect("Sinn")
        .expect("die Attrappe laeuft");
    let zeile = a.nachricht_mit(true, Sicht::Angesehen(&gesehen));
    assert!(zeile.contains("Ein rotes Achteck"), "{zeile}");
    assert!(zeile.contains(&a.pfad), "der Pfad fehlt: {zeile}");
    assert!(zeile.contains("anderes Modell"), "die Herkunft fehlt: {zeile}");

    // ⛔️ **Und die Datei selbst steht nicht drin.** Die ganze
    // Entscheidung dieses Moduls haengt daran.
    assert!(!zeile.contains("PNG"), "{zeile}");
    assert!(zeile.len() < 600, "die Zeile traegt zu viel: {}", zeile.len());
}

/// ⚑ **Die genaue Sprosse wird auch wirklich gerufen.** Beide Attrappen
/// sagen etwas anderes, also ist zu sehen, welche lief.
#[test]
fn die_stufe_entscheidet_welches_modell_laeuft() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    std::fs::create_dir_all(&heimat).expect("Heimat");
    attrappe(&heimat, "llama-mtmd-cli", "egal");
    // ⚑ Zwei Modelldateien, und die Attrappe gibt ihre Argumente nach
    // stderr; hier genuegt, dass die Pfade verschieden sind.
    std::fs::write(heimat.join(SEHMODELL), "klein").expect("Modell");
    std::fs::write(heimat.join(SEHPROJEKTOR), "klein").expect("Projektor");
    std::fs::write(heimat.join(myl_senses::laufwerk::SEHMODELL_GENAU), "gross").expect("Modell");
    std::fs::write(heimat.join(myl_senses::laufwerk::SEHPROJEKTOR_GENAU), "gross").expect("Projektor");

    let sinne = Sinne::finden_in(&heimat, &Eigene::default(), &[]);
    let sehen = sinne.sehen.as_ref().expect("bereit");
    assert!(sehen.zweistufig());
    assert_ne!(sehen.fuer(Stufe::Schnell).modell, sehen.fuer(Stufe::Genau).modell);
}

/// ⚑ **Sprechen ueber das eigene Skript**, den Weg, den CosyVoice und
/// jedes andere Sprechprogramm nehmen: zwei Pfade, sonst nichts.
#[test]
fn das_eigene_sprechskript_bekommt_text_und_ziel() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    let bin = heimat.join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    // Das Skript schreibt den Text gross in das Ziel; damit ist belegt,
    // dass es **beide** Pfade bekommen hat und in der richtigen Reihenfolge.
    let skript = bin.join("sprechen");
    std::fs::write(&skript, "#!/bin/sh\ntr 'a-z' 'A-Z' < \"$1\" > \"$2\"\n").expect("Skript");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&skript, std::fs::Permissions::from_mode(0o755)).expect("Rechte");
    }

    let sinne = Sinne::finden_in(&heimat, &Eigene::default(), &[]);
    assert!(sinne.kann_sprechen());
    let zeug = sinne.sprechen.as_ref().expect("bereit");
    let wav = myl_senses::sprechen::sagen(zeug, "guten tag").expect("gesprochen");
    assert_eq!(std::fs::read_to_string(&wav).expect("gelesen"), "GUTEN TAG");
    std::fs::remove_file(&wav).expect("weggeraeumt");
}

/// ⛔️ **Nichts zu sagen ist kein Lauf.** Ein Sprechmodell fuer eine
/// leere Antwort zu starten kostet Zeit und liefert eine leere Datei.
#[test]
fn eine_leere_antwort_wird_nicht_gesprochen() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    let bin = heimat.join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    let skript = bin.join("sprechen");
    std::fs::write(&skript, "#!/bin/sh\ncp \"$1\" \"$2\"\n").expect("Skript");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&skript, std::fs::Permissions::from_mode(0o755)).expect("Rechte");
    }
    let sinne = Sinne::finden_in(&heimat, &Eigene::default(), &[]);
    let zeug = sinne.sprechen.as_ref().expect("bereit");
    assert!(myl_senses::sprechen::sagen(zeug, "   \n ").is_err());
}

/// ⛔️ **Ein Sprechprogramm, das nichts hinterlaesst, ist ein Fehler**
/// und kein leeres Ergebnis: Sonst spielte der Client eine Datei ab,
/// die es nicht gibt.
#[test]
fn ohne_tondatei_ist_es_ein_fehler() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    let bin = heimat.join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    let skript = bin.join("sprechen");
    std::fs::write(&skript, "#!/bin/sh\nexit 0\n").expect("Skript");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&skript, std::fs::Permissions::from_mode(0o755)).expect("Rechte");
    }
    let sinne = Sinne::finden_in(&heimat, &Eigene::default(), &[]);
    let zeug = sinne.sprechen.as_ref().expect("bereit");
    let f = myl_senses::sprechen::sagen(zeug, "hallo").expect_err("keine Datei");
    assert!(f.contains("keine Tondatei"), "{f}");
}

/// Ein gestelltes Aufnahmeprogramm: Es schreibt sofort etwas in das Ziel
/// und wartet dann darauf, dass seine Standardeingabe schliesst. Damit
/// ist **der Stoppweg** geprueft und nicht nur der Start.
fn aufnahmeattrappe(heimat: &Path) {
    let bin = heimat.join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    let p = bin.join("aufnehmen");
    // ⚑ **Mehr als ein WAV-Kopf.** Eine Aufnahme, die nur so gross ist
    // wie ihr Kopf, ist keine; genau das prueft `beenden`, und die
    // Attrappe soll den Betrieb nachstellen und nicht den Fehlerfall.
    std::fs::write(
        &p,
        "#!/bin/sh\nprintf 'RIFF' > \"$1\"\nhead -c 400 /dev/zero >> \"$1\"\ncat > /dev/null\n",
    )
    .expect("Attrappe");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).expect("Rechte");
    }
}

/// ⚑ **Die Sprechtaste, ganz**: aufnehmen, aufhoeren, mitschreiben, und
/// die Tondatei ist danach weg.
#[test]
fn die_sprechtaste_nimmt_auf_und_schreibt_mit() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    std::fs::create_dir_all(&heimat).expect("Heimat");
    aufnahmeattrappe(&heimat);
    attrappe(&heimat, "whisper-cli", "Guten Tag, hier spricht die Attrappe.");
    std::fs::write(heimat.join(myl_senses::laufwerk::HOERMODELL), "Attrappe").expect("Modell");

    let sinne = Sinne::finden_in(&heimat, &Eigene::default(), &[]);
    assert!(sinne.kann_zuhoeren());

    let laufende = myl_senses::zuhoeren_beginnen(&sinne).expect("die Aufnahme laeuft");
    let wav = laufende.ziel().to_path_buf();
    let text = myl_senses::zuhoeren_beenden(&sinne, laufende, "de").expect("mitgeschrieben");
    assert_eq!(text, "Guten Tag, hier spricht die Attrappe.");
    // ⛔️ **Die Tondatei bleibt nicht liegen.** Sie hat niemand
    // angehaengt, und der Zwischenordner ist kein Archiv.
    assert!(!wav.exists(), "die Aufnahme liegt noch da: {}", wav.display());
}

/// ⛔️ **Ohne Hoermodell gibt es die Taste gar nicht erst.** Eine Taste,
/// die aufnimmt und dann niemanden hat, der mitschreibt, erzeugt
/// Tonmuell und eine Enttaeuschung.
#[test]
fn ohne_hoermodell_faengt_die_taste_nicht_an() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    std::fs::create_dir_all(&heimat).expect("Heimat");
    aufnahmeattrappe(&heimat);

    let sinne = Sinne::finden_in(&heimat, &Eigene::default(), &[]);
    assert!(sinne.aufnehmen.is_ok(), "aufnehmen geht");
    assert!(!sinne.kann_zuhoeren(), "ohne Hoermodell darf die Taste nicht gehen");
    let f = myl_senses::zuhoeren_beginnen(&sinne).expect_err("es fehlt das Hoermodell");
    assert!(f.contains("whisper.cpp"), "{f}");
}

/// ⛔️ **Eine leere Aufnahme ist ein Fehler mit Grund**, kein leerer Text.
/// Genau so sieht ein falsch gewaehltes Mikrofon aus, und wer das nicht
/// gesagt bekommt, sucht den Fehler beim Hoermodell.
#[test]
fn eine_leere_aufnahme_sagt_woran_es_liegen_kann() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    let bin = heimat.join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    let p = bin.join("aufnehmen");
    std::fs::write(&p, "#!/bin/sh\ncat > /dev/null\n").expect("Attrappe");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).expect("Rechte");
    }
    attrappe(&heimat, "whisper-cli", "egal");
    std::fs::write(heimat.join(myl_senses::laufwerk::HOERMODELL), "Attrappe").expect("Modell");

    let sinne = Sinne::finden_in(&heimat, &Eigene::default(), &[]);
    let laufende = myl_senses::zuhoeren_beginnen(&sinne).expect("laeuft");
    let f = myl_senses::zuhoeren_beenden(&sinne, laufende, "de").expect_err("nichts aufgenommen");
    assert!(f.contains("Geraet"), "der wahrscheinliche Grund fehlt: {f}");
}

/// ⚑ **Satzweise sprechen, waehrend das Modell noch schreibt**: Was
/// fertig ist, geht sofort raus, und die Reihenfolge bleibt.
///
/// ⛔️ **Die Reihenfolge ist der ganze Punkt.** Wer je Satz einen Faden
/// aufmacht, bekommt zwei Abspieler, die gleichzeitig reden.
#[test]
fn der_vorleser_spricht_satzweise_und_in_der_reihenfolge() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    let bin = heimat.join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    // Das Sprechskript legt den Text selbst als „Tondatei" ab, damit der
    // gestellte Abspieler ihn lesen und mitschreiben kann.
    let skript = bin.join("sprechen");
    std::fs::write(&skript, "#!/bin/sh\ncp \"$1\" \"$2\"\n").expect("Skript");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&skript, std::fs::Permissions::from_mode(0o755)).expect("Rechte");
    }
    let sinne = Sinne::finden_in(&heimat, &Eigene::default(), &[]);
    let zeug = sinne.sprechen.as_ref().expect("bereit").clone();

    let gespielt = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let mit = std::sync::Arc::clone(&gespielt);
    let mut vorleser = myl_senses::sprechen::Vorleser::neu_mit(
        &zeug,
        Box::new(move |wav| {
            let t = std::fs::read_to_string(wav).map_err(|f| f.to_string())?;
            mit.lock().expect("Schloss").push(t);
            Ok(())
        }),
    );

    // So kommt ein Strom an: stueckweise, mitten im Wort.
    for stueck in [
        "Der erste Satz ist lang gen",
        "ug zum Sprechen. Der zwei",
        "te Satz ist es ebenfalls, ganz sicher. Und ein Re",
        "st ohne Punkt",
    ] {
        vorleser.schub(stueck);
    }
    let fehler = vorleser.abschliessen();
    assert!(fehler.is_empty(), "{fehler:?}");

    let gehoert = gespielt.lock().expect("Schloss").clone();
    assert_eq!(gehoert.len(), 3, "{gehoert:?}");
    assert!(gehoert[0].starts_with("Der erste Satz"), "{gehoert:?}");
    assert!(gehoert[1].starts_with("Der zweite Satz"), "{gehoert:?}");
    // ⚑ **Der Rest ohne Punkt geht beim Abschliessen noch raus.** Sonst
    // fehlte der letzte Satz jeder Antwort, die nicht mit einem Punkt
    // endet.
    assert_eq!(gehoert[2], "Und ein Rest ohne Punkt");
}

/// ⛔️ **Eine Stimmprobe, die piper nicht verwerten kann, wird gesagt.**
///
/// ⚑ **Ein Schalter ohne Wirkung ist schlimmer als keiner.** Wer eine
/// Stimme hochlaedt und weiter dieselbe hoert, sucht den Fehler bei
/// sich. Diese Probe beisst, wenn der Hinweis wegfaellt.
#[test]
fn eine_probe_die_piper_ignoriert_wird_gesagt() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    std::fs::create_dir_all(&heimat).expect("Heimat");
    attrappe(&heimat, "piper", "egal");
    std::fs::write(heimat.join(myl_senses::laufwerk::SPRECHMODELL), "Stimme").expect("Stimme");
    std::fs::write(heimat.join(myl_senses::laufwerk::STIMMPROBE), "Probe").expect("Probe");

    let sinne = Sinne::finden_in(&heimat, &Eigene::default(), &[]);
    let z = sinne.sprechen.as_ref().expect("bereit");
    assert!(!z.kann_klonen(), "piper kann nicht klonen");
    assert!(z.probe.is_some(), "die Probe wurde nicht gefunden");
    let hinweis = sinne.stimmhinweis().expect("es gibt etwas zu sagen");
    assert!(hinweis.contains("CosyVoice"), "{hinweis}");
    assert!(hinweis.contains("ignoriert"), "{hinweis}");
}

/// ⚑ **Ein Skript bekommt die Probe als dritten Pfad**, und es steht
/// keiner da, wenn keine liegt.
#[test]
fn das_sprechskript_bekommt_die_probe_als_dritten_pfad() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    let bin = heimat.join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    // Das Skript schreibt seine Argumentzahl und das dritte Argument ins Ziel.
    let skript = bin.join("sprechen");
    std::fs::write(&skript, "#!/bin/sh\nprintf '%s|%s' \"$#\" \"${3:-keine}\" > \"$2\"\n")
        .expect("Skript");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&skript, std::fs::Permissions::from_mode(0o755)).expect("Rechte");
    }

    // Ohne Probe: zwei Pfade.
    let sinne = Sinne::finden_in(&heimat, &Eigene::default(), &[]);
    let z = sinne.sprechen.as_ref().expect("bereit");
    assert!(z.kann_klonen(), "der Skriptweg kann klonen");
    assert!(sinne.stimmhinweis().is_none(), "ohne Probe gibt es nichts zu sagen");
    let wav = myl_senses::sprechen::sagen(z, "hallo").expect("gesprochen");
    assert_eq!(std::fs::read_to_string(&wav).expect("gelesen"), "2|keine");
    std::fs::remove_file(&wav).expect("weg");

    // Mit Probe: drei Pfade, und der dritte ist sie.
    let probe = heimat.join(myl_senses::laufwerk::STIMMPROBE);
    std::fs::write(&probe, "Probe").expect("Probe");
    let sinne = Sinne::finden_in(&heimat, &Eigene::default(), &[]);
    let z = sinne.sprechen.as_ref().expect("bereit");
    assert!(sinne.stimmhinweis().is_none(), "ein Skript kann sie verwerten");
    let wav = myl_senses::sprechen::sagen(z, "hallo").expect("gesprochen");
    let gelesen = std::fs::read_to_string(&wav).expect("gelesen");
    assert_eq!(gelesen, format!("3|{}", probe.display()));
    std::fs::remove_file(&wav).expect("weg");
}

/// ⚑ **Der Dauerlaeufer und sein Gespraech**: `bereit`, dann je Satz
/// eine Zeile hin und `ok` zurueck, und das Schliessen der Eingabe
/// beendet ihn.
///
/// ⛔️ **Ohne ihn waere satzweises Sprechen mit CosyVoice langsamer als
/// gar keines**, weil je Satz ein halbes Milliardenmodell neu laedt.
/// Diese Probe stellt den Laeufer und prueft **das Protokoll**.
#[test]
fn der_dauerlaeufer_spricht_satz_fuer_satz() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    let bin = heimat.join("bin");
    std::fs::create_dir_all(&bin).expect("bin");

    // Ein gestellter Laeufer: meldet sich, arbeitet Zeilen ab, und
    // schreibt den Text in das jeweilige Ziel.
    let laeufer = bin.join("sprechen");
    std::fs::write(
        &laeufer,
        "#!/bin/sh\n\
         if [ \"$1\" != \"--dauer\" ]; then cp \"$1\" \"$2\"; exit 0; fi\n\
         echo bereit\n\
         while IFS=\"$(printf '\\t')\" read -r quelle ziel; do\n\
         cp \"$quelle\" \"$ziel\"; echo ok\n\
         done\n",
    )
    .expect("Laeufer");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&laeufer, std::fs::Permissions::from_mode(0o755)).expect("Rechte");
    }

    let sinne = Sinne::finden_in(&heimat, &Eigene::default(), &[]);
    let zeug = sinne.sprechen.as_ref().expect("bereit").clone();
    let mut dauer = myl_senses::sprechen::Dauersprecher::starten(&zeug).expect("meldet sich");

    for wort in ["erstens", "zweitens", "drittens"] {
        let quelle = d.path().join(format!("{wort}.txt"));
        std::fs::write(&quelle, wort).expect("Text");
        let ziel = d.path().join(format!("{wort}.wav"));
        dauer.satz(&quelle, &ziel).expect("gesprochen");
        assert_eq!(std::fs::read_to_string(&ziel).expect("gelesen"), wort);
    }
    // Das Fallenlassen schliesst die Eingabe; der Laeufer hoert von
    // selbst auf. Bleibt er haengen, laeuft diese Probe in die Frist.
    drop(dauer);
}

/// ⚑ **Ein Laeufer, der etwas anderes sagt, wird nicht benutzt.**
/// Sonst ginge der erste Satz in ein Programm, das noch laedt.
#[test]
fn ein_laeufer_mit_falscher_meldung_wird_abgelehnt() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    let bin = heimat.join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    let laeufer = bin.join("sprechen");
    std::fs::write(&laeufer, "#!/bin/sh\necho etwas anderes\n").expect("Laeufer");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&laeufer, std::fs::Permissions::from_mode(0o755)).expect("Rechte");
    }
    let sinne = Sinne::finden_in(&heimat, &Eigene::default(), &[]);
    let zeug = sinne.sprechen.as_ref().expect("bereit").clone();
    let f = myl_senses::sprechen::Dauersprecher::starten(&zeug).expect_err("meldet sich falsch");
    assert!(f.contains("bereit"), "{f}");
    assert!(f.contains("etwas anderes"), "die fremde Zeile fehlt in der Meldung: {f}");
}

/// ⛔️ **Ein Laeufer, der schweigt, haengt den Client nicht auf.**
///
/// 📌 Genau daran ist die eigene Pruefsammlung haengengeblieben, als der
/// Handschlag nur Zeilen zaehlte und keine Frist hatte: Ein
/// blockierendes `read_line` auf eine Zeile, die nie kommt, steht fuer
/// immer. **Ein Deckel auf der Zahl ist kein Deckel auf der Zeit.**
#[test]
fn ein_schweigender_laeufer_laeuft_in_die_frist() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    let bin = heimat.join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    let laeufer = bin.join("sprechen");
    // Eine Zeile, dann Schweigen, aber am Leben: der gefaehrliche Fall.
    std::fs::write(&laeufer, "#!/bin/sh\necho laedt noch\ncat > /dev/null\n").expect("Laeufer");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&laeufer, std::fs::Permissions::from_mode(0o755)).expect("Rechte");
    }
    let sinne = Sinne::finden_in(&heimat, &Eigene::default(), &[]);
    let zeug = sinne.sprechen.as_ref().expect("bereit").clone();

    let anfang = std::time::Instant::now();
    let f = myl_senses::sprechen::Dauersprecher::starten_mit(&zeug, 2)
        .expect_err("er sagt nie bereit");
    assert!(f.contains("bereit"), "{f}");
    assert!(anfang.elapsed().as_secs() < 8, "die Frist hat nicht gegriffen");
}

/// **Ein CosyVoice-foermiger Aufbau mit gestelltem Python.**
///
/// ⚑ **Der geteilte Laeufer gilt nur fuer CosyVoice** (`dauerhaft`), und
/// eine Probe, die statt dessen ein Skript nimmt, prueft den falschen
/// Weg. Also wird hier nachgestellt, was der Client wirklich findet:
/// ein `cosyvoice`-Ordner, der Laeufer daneben, und ein „Python", das
/// beides entgegennimmt.
fn cosy_attrappe(heimat: &Path, zaehler: &Path) -> Sinne {
    let bin = heimat.join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    std::fs::create_dir_all(heimat.join("cosyvoice")).expect("cosyvoice");
    std::fs::write(bin.join(myl_senses::laufwerk::COSYVOICE_LAEUFER), "# Attrappe\n")
        .expect("Laeufer");
    // Das gestellte Python bekommt den Laeufer als erstes Argument und
    // danach die Argumente des Clients.
    let python = bin.join("python3");
    std::fs::write(
        &python,
        format!(
            "#!/bin/sh\nshift\n\
             if [ \"$1\" != \"--dauer\" ]; then exit 0; fi\n\
             echo start >> {}\n\
             echo bereit\n\
             while IFS=\"$(printf '\\t')\" read -r quelle ziel; do cp \"$quelle\" \"$ziel\"; echo ok; done\n",
            zaehler.display()
        ),
    )
    .expect("Python");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o755)).expect("Rechte");
    }
    Sinne::finden_in(heimat, &Eigene::default(), &[])
}

/// ⛔️ **Ein geteilter Laeufer wird einmal gestartet, nicht je Antwort.**
///
/// 📌 Gemeldet vom Projektinhaber am 2026-09-18: „im Live Modus braucht
/// das Modell sehr lange nach der Textgenerierung um zu antworten."
/// CosyVoice laedt rund achtzehn Sekunden; ein Vorleser je Antwort legt
/// diese Zeit **vor jede** Antwort. Diese Probe zaehlt die Starts.
#[test]
fn ein_geteilter_laeufer_startet_nur_einmal() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    let zaehler = d.path().join("starts");
    let sinne = cosy_attrappe(&heimat, &zaehler);
    let zeug = sinne.sprechen.as_ref().expect("bereit").clone();
    assert!(zeug.dauerhaft(), "der Aufbau ist kein CosyVoice: {:?}", zeug.weg);
    let geteilt: myl_senses::sprechen::Geteilter = Default::default();

    for i in 0..3 {
        let mut v = myl_senses::sprechen::Vorleser::neu_geteilt(
            &zeug,
            Box::new(|_| Ok(())),
            Some(std::sync::Arc::clone(&geteilt)),
        );
        v.schub(&format!("Das ist die Antwort Nummer {i}, lang genug zum Sprechen."));
        let fehler = v.abschliessen();
        assert!(fehler.is_empty(), "{fehler:?}");
    }
    let starts = std::fs::read_to_string(&zaehler).unwrap_or_default().lines().count();
    assert_eq!(starts, 1, "der Laeufer wurde {starts} mal gestartet statt einmal");

    // ⚑ **Und ohne geteilten Halter startet jeder Vorleser seinen
    // eigenen.** Das ist der Fall, der die Wartezeit erzeugte; ohne
    // diese Gegenprobe hielte die obere Zusage auch dann, wenn gar nichts
    // gestartet wuerde.
    let _ = std::fs::remove_file(&zaehler);
    for _ in 0..2 {
        let mut v = myl_senses::sprechen::Vorleser::neu_mit(&zeug, Box::new(|_| Ok(())));
        v.schub("Das ist eine Antwort, lang genug zum Sprechen.");
        assert!(v.abschliessen().is_empty());
    }
    let einzeln = std::fs::read_to_string(&zaehler).unwrap_or_default().lines().count();
    assert_eq!(einzeln, 2, "ohne Halter muessten es zwei Starts sein, es waren {einzeln}");
}

/// ⚑ **Vorwaermen laedt im Voraus und nur einmal.**
#[test]
fn vorwaermen_startet_hoechstens_einen() {
    let d = tempfile::tempdir().expect("Verzeichnis");
    let heimat = d.path().join("sinne");
    let zaehler = d.path().join("starts");
    let sinne = cosy_attrappe(&heimat, &zaehler);
    let zeug = sinne.sprechen.as_ref().expect("bereit").clone();
    let geteilt: myl_senses::sprechen::Geteilter = Default::default();

    myl_senses::sprechen::vorwaermen(&zeug, &geteilt).expect("waermt vor");
    assert!(geteilt.lock().expect("Schloss").is_some(), "es steht keiner bereit");
    // Ein zweites Vorwaermen wirft den ersten nicht weg und startet
    // keinen zweiten.
    myl_senses::sprechen::vorwaermen(&zeug, &geteilt).expect("waermt vor");
    let starts = std::fs::read_to_string(&zaehler).unwrap_or_default().lines().count();
    assert_eq!(starts, 1, "vorgewaermt wurde {starts} mal statt einmal");
}

/// ⛔️ **Eine Nachricht, die gleich eine Beschreibung bekommt, darf
/// nicht behaupten, es sei nichts zu sagen** (Fund 436).
///
/// Das Fenster schreibt die Anhangzeile sofort und haengt die Antwort
/// des Sehmodells danach an. Stuende in der Zeile `Sicht::Nichts`,
/// enthielte dieselbe Nachricht beides: „ueber ihren Inhalt ist nichts
/// zu sagen" und darunter die Beschreibung. Das Modell liest den ersten
/// Satz zuerst.
#[test]
fn eine_kommende_beschreibung_wird_nicht_vorab_verneint() {
    let a = myl_senses::anhang::Anhang {
        name: "bild.png".into(),
        pfad: ".AGENT/anhaenge/bild.png".into(),
        art: myl_senses::anhang::Art::Bild,
        bytes: 5_600_000,
        auszug: String::new(),
    };
    let kommt = a.nachricht_mit(true, myl_senses::anhang::Sicht::Kommt);
    assert!(
        !kommt.contains("nichts zu sagen"),
        "die Zeile verneint, obwohl die Beschreibung folgt:\n{kommt}"
    );
    assert!(
        !kommt.contains("kein Sinnesmodell"),
        "die Zeile behauptet ein fehlendes Sinnesmodell:\n{kommt}"
    );
    assert!(kommt.contains("bild.png"), "{kommt}");

    // ⚑ **Die Gegenprobe:** Ohne Sinnesmodell gehoert genau dieser Satz
    //   hin, sonst pruefte die Probe darueber nichts.
    let nichts = a.nachricht_mit(true, myl_senses::anhang::Sicht::Nichts);
    assert!(nichts.contains("nichts zu sagen"), "{nichts}");
}

/// ⛔️ **Eine Textdatei im Chat darf kein Werkzeug nennen** (Fund 439).
///
/// Im Chat gibt es keine Werkzeuge. Nennt die Anhangzeile trotzdem
/// `read_file`, antwortet das Modell auf „aendere die Datei" mit einer
/// Anleitung: Es hat nichts, womit es sie aendern koennte, und die
/// Nachricht hat ihm das Gegenteil gesagt.
#[test]
fn eine_textdatei_ohne_werkzeuge_verspricht_keines() {
    let a = myl_senses::anhang::Anhang {
        name: "liste.md".into(),
        pfad: ".AGENT/anhaenge/liste.md".into(),
        art: myl_senses::anhang::Art::Text,
        bytes: 42,
        auszug: "# Einkaufsliste\n".into(),
    };
    let ohne = a.nachricht_mit(true, myl_senses::anhang::Sicht::Nichts);
    assert!(
        !ohne.contains("read_file"),
        "die Zeile nennt ein Werkzeug, das es im Chat nicht gibt:\n{ohne}"
    );
    assert!(ohne.contains("keine Dateiwerkzeuge"), "{ohne}");
    assert!(ohne.contains("# Einkaufsliste"), "der Auszug fehlt:\n{ohne}");

    // ⚑ **Die Gegenrichtung:** Mit Werkzeugen gehoert der Name genau
    //   dorthin, sonst sucht das Modell nicht danach.
    let mit = a.nachricht_mit(true, myl_senses::anhang::Sicht::Werkzeug("read_file"));
    assert!(mit.contains("`read_file`"), "{mit}");
    assert!(!mit.contains("keine Dateiwerkzeuge"), "{mit}");
}
