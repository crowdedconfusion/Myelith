//! Ein Vorhaben ueber mehrere Runden, mit echter Ruestung und echter
//! Agentenschleife; nur das Modell ist ein Drehbuch.

use std::cell::RefCell;

use myl_client::einstellungen::{Agenteneinstellung, Loopeinstellung, Sprache};
use myl_client::vorhaben::{self, Ablage, Zustand};
use myl_client::werkzeuge::Werkzeugkiste;
use myl_local_agent::tuerklient::{Antwort, Modellweg, Nachricht, Tuerfehler};
use myl_local_agent::werkzeug::Ansageform;

/// Liefert je Runde die naechste Zeile des Drehbuchs; den Pruefdurchgang
/// beantwortet es mit einem festen Urteil.
struct Drehbuch {
    zeilen: RefCell<Vec<String>>,
    /// Ein Urteil je Pruefung; das letzte gilt fuer alle weiteren.
    urteile: RefCell<Vec<String>>,
    gefragt: RefCell<Vec<String>>,
}

impl Modellweg for Drehbuch {
    fn chat(&self, _m: &str, nachrichten: &[Nachricht], _t: Option<u32>) -> Result<Antwort, Tuerfehler> {
        let letzte = nachrichten.last().map(|n| n.content.clone()).unwrap_or_default();
        self.gefragt.borrow_mut().push(letzte.clone());
        let text = if letzte.starts_with("Prüfe eine Runde") {
            let mut u = self.urteile.borrow_mut();
            if u.len() > 1 { u.remove(0) } else { u[0].clone() }
        } else {
            let mut z = self.zeilen.borrow_mut();
            if z.is_empty() { "Nichts mehr zu tun.".to_string() } else { z.remove(0) }
        };
        Ok(Antwort { text, abschlussgrund: Some("stop".into()), kennung: "probe".into(), segment: None, prompt_token: 0, antwort_token: 0 })
    }
}

fn aufruf(name: &str, argumente: serde_json::Value) -> String {
    format!("<tool_call>{}</tool_call>", serde_json::json!({"name": name, "arguments": argumente}))
}

fn ordner(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("myl-vorhaben-probe-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn ruester(
    wurzel: std::path::PathBuf,
) -> impl Fn(vorhaben::Zusatzwerkzeuge, &str) -> Result<myl_client::ruestung::Ruestung, String> {
    move |zusaetzlich, _saat| {
        let agent = Agenteneinstellung {
            wurzel: Some(wurzel.display().to_string()),
            schreiben: true,
            ..Agenteneinstellung::default()
        };
        myl_client::ruestung::ruesten(&agent, Ansageform::Amtlich, Werkzeugkiste::Base, zusaetzlich)
    }
}

#[test]
fn zwei_runden_notiz_wecken_fertig_und_kette() {
    let basis = ordner("zwei-runden");
    let ablage = Ablage::neu(basis.join("vorhaben"));
    let arbeit = basis.join("arbeit");
    std::fs::create_dir_all(&arbeit).unwrap();
    let mut v = ablage.anlegen("Drei Dateien anlegen", vorhaben::jetzt()).unwrap();

    let modell = Drehbuch {
        zeilen: RefCell::new(vec![
            // Runde 1: notieren, schreiben, in 30 Minuten weiter.
            aufruf("note_set", serde_json::json!({"key": "stand", "value": "a.md angelegt"})),
            aufruf("write_file", serde_json::json!({"pfad": "a.md", "inhalt": "eins"})),
            aufruf("wake_in", serde_json::json!({"minutes": 30})),
            "Ich habe a.md angelegt.".into(),
            // Runde 2: fertig, und danach ein Folgevorhaben.
            aufruf("chain_goal", serde_json::json!({"goal": "Die Dateien zusammenfassen"})),
            aufruf("finish_goal", serde_json::json!({"result": "a.md steht"})),
            "Fertig.".into(),
        ]),
        urteile: RefCell::new(vec![
            "FORTSCHRITT: JA\nERREICHT: NEIN\nGRUND: erst eine von drei".into(),
            "FORTSCHRITT: JA\nERREICHT: JA\nGRUND: a.md ist da".into(),
        ]),
        gefragt: RefCell::new(Vec::new()),
    };
    let grenzen = Loopeinstellung::default();
    let r = ruester(arbeit.clone());

    let erste = vorhaben::runde(&ablage, &mut v, &modell, &r, &grenzen, Sprache::De, 256, None).unwrap();
    assert_eq!(v.runden, 1);
    assert_eq!(v.zustand, Zustand::Schlaeft, "{:?}", erste);
    assert!(v.naechste >= vorhaben::jetzt() + 29 * 60);
    assert_eq!(v.notizen["stand"], "a.md angelegt");
    assert_eq!(std::fs::read_to_string(arbeit.join("a.md")).unwrap(), "eins");
    assert!(erste.pruefung.as_ref().is_some_and(|p| p.fortschritt));
    assert!(ablage.rundenstand(&v.kennung).is_none(), "nach der Runde ist der Rundenstand weg");
    assert_eq!(ablage.laden(&v.kennung).unwrap(), v, "gespeichert, wie es ist");

    // Die zweite Runde sieht Notizen und Tagebuch der ersten.
    let zweite = vorhaben::runde(&ablage, &mut v, &modell, &r, &grenzen, Sprache::De, 256, None).unwrap();
    let auftrag2 = modell.gefragt.borrow().iter().find(|t| t.contains("Runde 2 von")).cloned().expect("Auftrag der zweiten Runde");
    assert!(auftrag2.contains("stand: a.md angelegt"), "{auftrag2}");
    assert!(auftrag2.contains("Ich habe a.md angelegt."), "Tagebuch fehlt: {auftrag2}");
    assert!(auftrag2.contains("[System] Tatsächlich ausgeführt: note_set("), "Werkzeuge fehlen im Tagebuch: {auftrag2}");
    assert!(auftrag2.contains("write_file("), "{auftrag2}");
    assert_eq!(v.zustand, Zustand::Fertig);
    assert_eq!(v.ergebnis.as_deref(), Some("a.md steht"));
    assert_eq!(zweite.neue.len(), 1);
    assert_eq!(zweite.neue[0].ziel, "Die Dateien zusammenfassen");
    assert_eq!(zweite.neue[0].vorgaenger.as_deref(), Some(v.kennung.as_str()));
    assert_eq!(ablage.alle().len(), 2);
}

/// ⛔️ Die Pruefung widerspricht dem „fertig": Das Vorhaben laeuft weiter.
#[test]
fn pruefung_haelt_ein_falsches_fertig_auf() {
    let basis = ordner("falsches-fertig");
    let ablage = Ablage::neu(basis.join("vorhaben"));
    let arbeit = basis.join("arbeit");
    std::fs::create_dir_all(&arbeit).unwrap();
    let mut v = ablage.anlegen("Ein Bericht", vorhaben::jetzt()).unwrap();
    let modell = Drehbuch {
        zeilen: RefCell::new(vec![aufruf("finish_goal", serde_json::json!({"result": "alles da"})), "Fertig.".into()]),
        urteile: RefCell::new(vec!["FORTSCHRITT: NEIN\nERREICHT: NEIN\nGRUND: es gibt keinen Bericht".into()]),
        gefragt: RefCell::new(Vec::new()),
    };
    vorhaben::runde(&ablage, &mut v, &modell, &ruester(arbeit), &Loopeinstellung::default(), Sprache::De, 256, None).unwrap();
    assert_ne!(v.zustand, Zustand::Fertig);
    assert!(v.notizen["pruefung"].contains("keinen Bericht"));
    assert_eq!(v.ohne_fortschritt, 1);
}

/// ⚑ Ohne Pruefdurchgang fragt niemand nach, und das Wort des Agenten gilt.
#[test]
fn ohne_pruefung_kein_zweiter_durchgang() {
    let basis = ordner("ohne-pruefung");
    let ablage = Ablage::neu(basis.join("vorhaben"));
    let arbeit = basis.join("arbeit");
    std::fs::create_dir_all(&arbeit).unwrap();
    let mut v = ablage.anlegen("Kurz", vorhaben::jetzt()).unwrap();
    let modell = Drehbuch {
        zeilen: RefCell::new(vec![aufruf("finish_goal", serde_json::json!({"result": "ok"})), "Fertig.".into()]),
        urteile: RefCell::new(vec!["FORTSCHRITT: NEIN\nERREICHT: NEIN\nGRUND: -".into()]),
        gefragt: RefCell::new(Vec::new()),
    };
    let grenzen = Loopeinstellung { pruefen: false, ..Loopeinstellung::default() };
    vorhaben::runde(&ablage, &mut v, &modell, &ruester(arbeit), &grenzen, Sprache::De, 256, None).unwrap();
    assert_eq!(v.zustand, Zustand::Fertig);
    assert!(!modell.gefragt.borrow().iter().any(|t| t.starts_with("Prüfe eine Runde")));
}

/// Bricht beim dritten Aufruf ab, wie ein Fenster, das mitten in der
/// Runde geschlossen wird.
struct Abbrechend {
    aufrufe: RefCell<usize>,
}

impl Modellweg for Abbrechend {
    fn chat(&self, _m: &str, _n: &[Nachricht], _t: Option<u32>) -> Result<Antwort, Tuerfehler> {
        let mut a = self.aufrufe.borrow_mut();
        *a += 1;
        let text = match *a {
            1 => aufruf("note_set", serde_json::json!({"key": "stand", "value": "b.md geschrieben"})),
            2 => aufruf("write_file", serde_json::json!({"pfad": "b.md", "inhalt": "zwei"})),
            _ => return Err(Tuerfehler::Abgebrochen { bisher: String::new() }),
        };
        Ok(Antwort { text, abschlussgrund: Some("stop".into()), kennung: "probe".into(), segment: None, prompt_token: 0, antwort_token: 0 })
    }
}

/// ⚑ **Genau dort weiter, wo unterbrochen wurde.** Die fortgesetzte Runde
/// bekommt vorgelegt, was schon erledigt war, und die Notiz aus der
/// halben Runde ist nicht verloren.
#[test]
fn unterbrochene_runde_wird_mit_ihrem_stand_fortgesetzt() {
    let basis = ordner("unterbrochen");
    let ablage = Ablage::neu(basis.join("vorhaben"));
    let arbeit = basis.join("arbeit");
    std::fs::create_dir_all(&arbeit).unwrap();
    let mut v = ablage.anlegen("Zwei Dateien", vorhaben::jetzt()).unwrap();
    let grenzen = Loopeinstellung::default();
    let r = ruester(arbeit.clone());

    vorhaben::runde(&ablage, &mut v, &Abbrechend { aufrufe: RefCell::new(0) }, &r, &grenzen, Sprache::De, 256, None).unwrap();
    assert_eq!(v.runden, 0, "eine abgebrochene Runde zaehlt nicht");
    assert!(matches!(v.zustand, Zustand::Angehalten { .. }));
    let stand = ablage.rundenstand(&v.kennung).expect("der Rundenstand bleibt");
    assert_eq!(stand.erledigt.len(), 2, "{stand:?}");
    assert_eq!(stand.notizen["stand"], "b.md geschrieben");
    assert_eq!(std::fs::read_to_string(arbeit.join("b.md")).unwrap(), "zwei");

    // Der Mensch setzt fort. Die Datei wird von aussen geaendert: Liefe der
    // erledigte Aufruf noch einmal, stuende danach wieder „zwei" darin.
    v.zustand = Zustand::Bereit;
    std::fs::write(arbeit.join("b.md"), "von aussen").unwrap();
    let modell = Drehbuch {
        zeilen: RefCell::new(vec![
            // ⛔️ Das Modell wiederholt trotz Hinweis einen erledigten Aufruf.
            aufruf("write_file", serde_json::json!({"pfad": "b.md", "inhalt": "zwei"})),
            "Die Runde ist abgeschlossen.".into(),
        ]),
        urteile: RefCell::new(vec!["FORTSCHRITT: JA\nERREICHT: NEIN\nGRUND: weiter".into()]),
        gefragt: RefCell::new(Vec::new()),
    };
    vorhaben::runde(&ablage, &mut v, &modell, &r, &grenzen, Sprache::De, 256, None).unwrap();
    let auftrag = modell.gefragt.borrow()[0].clone();
    assert!(auftrag.contains("UNTERBROCHEN"), "{auftrag}");
    assert!(auftrag.contains("write_file("), "{auftrag}");
    assert!(auftrag.contains("stand: b.md geschrieben"), "{auftrag}");
    assert_eq!(v.runden, 1);
    assert_eq!(v.notizen["stand"], "b.md geschrieben");
    assert!(ablage.rundenstand(&v.kennung).is_none());
    assert_eq!(
        std::fs::read_to_string(arbeit.join("b.md")).unwrap(),
        "von aussen",
        "der erledigte Aufruf lief noch einmal"
    );
    let antwort = modell.gefragt.borrow().iter().find(|t| t.contains("nicht noch einmal ausgeführt")).cloned();
    assert!(antwort.is_some(), "das Modell erfuhr nicht, dass der Aufruf schon erledigt war");
}

/// ⚑ **Die Schlange im Antrieb**: `fahren` leiht das Modell je Runde,
/// faehrt in der Reihenfolge der Schlange, und eine Kette kommt direkt
/// nach ihrem Vorgaenger, vor allem, was schon wartete. Der Beginn einer
/// Runde wird erst gemeldet, wenn das Modell geliehen ist.
#[test]
fn fahren_leiht_je_runde_und_haelt_die_reihe() {
    let basis = ordner("schlange");
    let ablage = Ablage::neu(basis.join("vorhaben"));
    let arbeit = basis.join("arbeit");
    std::fs::create_dir_all(&arbeit).unwrap();
    let x = ablage.anlegen("X zuerst angelegt", vorhaben::jetzt()).unwrap();
    let y = ablage.anlegen("Y nach vorn gezogen", vorhaben::jetzt()).unwrap();
    ablage.reihe_setzen(&[y.kennung.clone(), x.kennung.clone()]).unwrap();

    let modell = Drehbuch {
        zeilen: RefCell::new(vec![
            // Y: Kette anstossen und fertig.
            aufruf("chain_goal", serde_json::json!({"goal": "Z aus der Kette"})),
            aufruf("finish_goal", serde_json::json!({"result": "Y erledigt"})),
            "Y fertig.".into(),
            // Z, dann X: je sofort fertig.
            aufruf("finish_goal", serde_json::json!({"result": "Z erledigt"})),
            "Z fertig.".into(),
            aufruf("finish_goal", serde_json::json!({"result": "X erledigt"})),
            "X fertig.".into(),
        ]),
        urteile: RefCell::new(vec!["FORTSCHRITT: JA\nERREICHT: JA\nGRUND: steht".into()]),
        gefragt: RefCell::new(Vec::new()),
    };
    let geliehen = std::cell::Cell::new(false);
    let leihen_gezaehlt = std::cell::Cell::new(0);
    let leihen = |runde: &mut dyn FnMut(&dyn Modellweg)| -> Result<(), String> {
        leihen_gezaehlt.set(leihen_gezaehlt.get() + 1);
        geliehen.set(true);
        runde(&modell);
        geliehen.set(false);
        Ok(())
    };
    let begonnen = RefCell::new(Vec::new());
    let melden = |e: vorhaben::Ereignis| {
        if let vorhaben::Ereignis::Beginnt { ziel, .. } = e {
            assert!(geliehen.get(), "„beginnt“ vor der Leihe: {ziel}");
            begonnen.borrow_mut().push(ziel.chars().next().unwrap());
        }
    };
    let laeufer = vorhaben::Laeufer::oeffnen(ablage.clone(), "probe").unwrap();
    let r = ruester(arbeit);
    vorhaben::fahren(&laeufer, &leihen, &r, &Loopeinstellung::default(), Sprache::De, 256, &melden, &|| false, None);
    assert_eq!(*begonnen.borrow(), ['Y', 'Z', 'X'], "Reihe, Kette direkt nach Y, dann X");
    assert_eq!(leihen_gezaehlt.get(), 3, "eine Leihe je Runde");
    assert!(ablage.alle().iter().all(|v| v.zustand == Zustand::Fertig));
}

/// Ohne Modell endet der Loop, und das Vorhaben bleibt, wie es war.
#[test]
fn ohne_modell_endet_der_loop_und_das_vorhaben_bleibt() {
    let basis = ordner("ohne-modell");
    let ablage = Ablage::neu(basis.join("vorhaben"));
    let v = ablage.anlegen("irgendwas", vorhaben::jetzt()).unwrap();
    let laeufer = vorhaben::Laeufer::oeffnen(ablage.clone(), "probe").unwrap();
    let gemeldet = RefCell::new(Vec::new());
    let melden = |e: vorhaben::Ereignis| gemeldet.borrow_mut().push(format!("{e:?}"));
    let leihen = |_: &mut dyn FnMut(&dyn Modellweg)| -> Result<(), String> { Err("kein Artefakt".into()) };
    let r = ruester(basis.clone());
    vorhaben::fahren(&laeufer, &leihen, &r, &Loopeinstellung::default(), Sprache::De, 256, &melden, &|| false, None);
    let g = gemeldet.borrow();
    assert_eq!(g.len(), 1, "{g:?}");
    assert!(g[0].contains("OhneModell") && g[0].contains("kein Artefakt"), "{g:?}");
    assert_eq!(ablage.laden(&v.kennung).unwrap().zustand, Zustand::Bereit, "nicht angehalten");
}

/// ⚑ **Punkt 4.7 ueber eine ganze Runde:** Das Modell meldet „fertig“, die
/// Abnahme sagt nein, und die naechste Runde sieht die echte Ausgabe. Dann
/// schreibt es die Datei, meldet sich nicht ab, und die Abnahme macht fertig.
#[test]
fn die_abnahme_entscheidet_ueber_ganze_runden() {
    let basis = ordner("abnahme");
    let ablage = Ablage::neu(basis.join("vorhaben"));
    let arbeit = basis.join("arbeit");
    std::fs::create_dir_all(&arbeit).unwrap();
    let mut v = ablage.anlegen("ergebnis/fertig.md anlegen", vorhaben::jetzt()).unwrap();
    v.abnahme = Some("test -s ergebnis/fertig.md && echo ABNAHME_OK".into());
    ablage.speichern(&v).unwrap();
    let modell = Drehbuch {
        zeilen: RefCell::new(vec![
            // Runde 1: behauptet fertig, ohne etwas zu tun.
            aufruf("finish_goal", serde_json::json!({"result": "alles erledigt"})),
            "Fertig.".into(),
            // Runde 2: schreibt die Datei, meldet sich nicht ab.
            aufruf("write_file", serde_json::json!({"pfad": "ergebnis/fertig.md", "inhalt": "ja"})),
            "Datei geschrieben.".into(),
        ]),
        urteile: RefCell::new(vec!["FORTSCHRITT: JA\nERREICHT: JA\nGRUND: glaube ich".into()]),
        gefragt: RefCell::new(Vec::new()),
    };
    let r = ruester(arbeit.clone());
    let grenzen = Loopeinstellung::default();
    let erste = vorhaben::runde(&ablage, &mut v, &modell, &r, &grenzen, Sprache::De, 256, None).unwrap();
    assert_ne!(v.zustand, Zustand::Fertig, "die Behauptung hat die Abnahme ueberstimmt");
    assert!(erste.abnahme.as_ref().is_some_and(|a| !a.bestanden));
    assert!(v.notizen["abnahme"].contains("endete mit 1"), "{:?}", v.notizen);
    let zweite = vorhaben::runde(&ablage, &mut v, &modell, &r, &grenzen, Sprache::De, 256, None).unwrap();
    let auftrag2 = modell.gefragt.borrow().iter().find(|t| t.contains("Runde 2 von")).cloned().unwrap();
    assert!(auftrag2.contains("ABNAHME:") && auftrag2.contains("endete mit 1"), "die Runde sah die Abnahme nicht: {auftrag2}");
    assert_eq!(v.zustand, Zustand::Fertig, "{:?}", zweite.abnahme);
    assert!(zweite.abnahme.as_ref().is_some_and(|a| a.bestanden && a.ausgabe.contains("ABNAHME_OK")));
}

/// ⛔️ **Eine Runde schreibt auch in vorhandene Dateien**, durch alle Hüllen
/// hindurch (Doppelsperre, Pendelwächter, Wiederholungsbremse).
#[test]
fn eine_runde_aendert_vorhandene_dateien() {
    let basis = ordner("vorhanden");
    let ablage = Ablage::neu(basis.join("vorhaben"));
    let arbeit = basis.join("arbeit");
    std::fs::create_dir_all(arbeit.join("daten")).unwrap();
    std::fs::write(arbeit.join("daten/a.py"), "x = 1\n").unwrap();
    std::fs::write(arbeit.join("daten/b.csv"), "alt\n").unwrap();
    let mut v = ablage.anlegen("zwei Dateien ändern", vorhaben::jetzt()).unwrap();
    let modell = Drehbuch {
        zeilen: RefCell::new(vec![
            aufruf("edit_file", serde_json::json!({"pfad": "daten/a.py", "aenderungen": [{"alt": "x = 1", "neu": "x = 2"}]})),
            aufruf("write_file", serde_json::json!({"pfad": "daten/b.csv", "inhalt": "neu\n"})),
            "Beide geändert.".into(),
        ]),
        urteile: RefCell::new(vec!["FORTSCHRITT: JA\nERREICHT: JA\nGRUND: steht".into()]),
        gefragt: RefCell::new(Vec::new()),
    };
    let r = ruester(arbeit.clone());
    let meldungen = RefCell::new(Vec::new());
    let melder = |m: myl_local_agent::schleife::Meldung<'_>| {
        if let myl_local_agent::schleife::Meldung::Ergebnis { name, text } = m {
            meldungen.borrow_mut().push(format!("{name}: {text}"));
        }
    };
    vorhaben::runde(&ablage, &mut v, &modell, &r, &Loopeinstellung::default(), Sprache::De, 256, Some(&melder)).unwrap();
    assert_eq!(std::fs::read_to_string(arbeit.join("daten/a.py")).unwrap(), "x = 2\n", "{:?}", meldungen.borrow());
    assert_eq!(std::fs::read_to_string(arbeit.join("daten/b.csv")).unwrap(), "neu\n", "{:?}", meldungen.borrow());
}

/// ⛔️ **Fund 491 über eine ganze Runde:** Das Modell schreibt „Ausgeführt:
/// write_file(…)“ in seine Antwort, ohne zu rufen. Nichts ist geschrieben,
/// das Tagebuch markiert die Behauptung und sagt, dass kein Werkzeug lief.
#[test]
fn eine_behauptete_ausfuehrung_steht_nicht_als_tatsache_da() {
    let basis = ordner("behauptet");
    let ablage = Ablage::neu(basis.join("vorhaben"));
    let arbeit = basis.join("arbeit");
    std::fs::create_dir_all(&arbeit).unwrap();
    let mut v = ablage.anlegen("x.md anlegen", vorhaben::jetzt()).unwrap();
    let modell = Drehbuch {
        zeilen: RefCell::new(vec!["Ich lege die Datei an.\n\nAusgeführt: write_file({\"pfad\":\"x.md\",\"inhalt\":\"ja\"})".into()]),
        urteile: RefCell::new(vec!["FORTSCHRITT: NEIN\nERREICHT: NEIN\nGRUND: nichts lief".into()]),
        gefragt: RefCell::new(Vec::new()),
    };
    let r = ruester(arbeit.clone());
    vorhaben::runde(&ablage, &mut v, &modell, &r, &Loopeinstellung::default(), Sprache::De, 256, None).unwrap();
    assert!(!arbeit.join("x.md").exists(), "eine Behauptung hat geschrieben");
    let tb = ablage.tagebuch_letzte(&v.kennung, 1).join("\n");
    assert!(tb.contains("(behauptet, nicht ausgeführt) Ausgeführt: write_file"), "{tb}");
    assert!(tb.contains("[System] In dieser Runde lief kein Werkzeug."), "{tb}");
}
