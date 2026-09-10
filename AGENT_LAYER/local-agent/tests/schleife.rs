//! Der Lauf von Anfang bis Ende, gegen einen Stummel-Server.
//!
//! ⚑ **Gegen einen Stummel und nicht gegen ein Modell**, aus demselben
//! Grund wie in `harness_bis_modell.rs`: Ein 0,5B-Modell schlägt
//! unzuverlässig Werkzeuge vor, und ein Test, der davon abhinge, wäre
//! flatterig statt scharf. Der Stummel spielt genau die Antworten, die
//! geprüft werden sollen, **auch die bösartigen**.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use myl_agent::manifest::{Herkunft, Werkzeugart, Werkzeugmanifest};
use myl_agent::registratur::Registratur;
use myl_types::hash::Hash;
use myl_types::ids::{Address, EpochId, MerkleRoot};
use myl_types::sitzung::{Grenzen, Sitzungskontrakt};

use myl_local_agent::ausfuehrung::{Werkzeugausfuehrung, Werkzeugfehler, Werkzeugkasten};
use myl_local_agent::betrieb::Betriebsart;
use myl_local_agent::schleife::{Ende, Lauf};
use myl_local_agent::tuerklient::Tuerklient;
use myl_local_agent::vollmacht_grenzen::Sitzungsgrenzen;
use myl_local_agent::werkzeug::Werkzeug;

// --- Der Stummel ------------------------------------------------------

/// Spielt eine feste Folge von Modellantworten ab.
fn stummel(antworten: Vec<String>) -> (u16, Arc<Mutex<Vec<String>>>) {
    let horcher = TcpListener::bind("127.0.0.1:0").expect("binden");
    let port = horcher.local_addr().expect("Adresse").port();
    let gesehen = Arc::new(Mutex::new(Vec::new()));
    let mit = Arc::clone(&gesehen);
    thread::spawn(move || {
        for (i, text) in antworten.into_iter().enumerate() {
            let Ok((mut strom, _)) = horcher.accept() else { return };
            let anfrage = lesen(&mut strom);
            mit.lock().expect("Schloss").push(anfrage);
            let rumpf = format!(
                "{{\"id\":\"myl-{i}\",\"choices\":[{{\"index\":0,\"message\":{{\"role\":\
                 \"assistant\",\"content\":{}}},\"finish_reason\":\"stop\"}}],\
                 \"usage\":{{\"prompt_tokens\":10,\"completion_tokens\":5}},\
                 \"myelith_segment\":\"{}\"}}",
                serde_json::to_string(&text).expect("JSON"),
                "0".repeat(62) + &format!("{:02x}", i as u8)
            );
            let _ = strom.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{rumpf}",
                    rumpf.len()
                )
                .as_bytes(),
            );
        }
    });
    (port, gesehen)
}

fn lesen(strom: &mut TcpStream) -> String {
    let mut roh = Vec::new();
    let mut puffer = [0u8; 4096];
    loop {
        let Some(i) = roh.windows(4).position(|f| f == b"\r\n\r\n") else {
            match strom.read(&mut puffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    roh.extend_from_slice(&puffer[..n]);
                    continue;
                }
            }
        };
        let kopf = String::from_utf8_lossy(&roh[..i]).into_owned();
        let laenge: usize = kopf
            .lines()
            .find(|z| z.to_ascii_lowercase().starts_with("content-length:"))
            .and_then(|z| z.split(':').nth(1))
            .and_then(|w| w.trim().parse().ok())
            .unwrap_or(0);
        while roh.len() < i + 4 + laenge {
            match strom.read(&mut puffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => roh.extend_from_slice(&puffer[..n]),
            }
        }
        break;
    }
    String::from_utf8_lossy(&roh).into_owned()
}

// --- Werkzeuge --------------------------------------------------------

struct Zeit;
impl Werkzeugausfuehrung for Zeit {
    fn name(&self) -> &str {
        "zeit"
    }
    fn ausfuehren(&self, _a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        Ok("12:00".to_string())
    }
}

struct Kaputt;
impl Werkzeugausfuehrung for Kaputt {
    fn name(&self) -> &str {
        "kaputt"
    }
    fn ausfuehren(&self, _a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        Err(Werkzeugfehler { grund: "die Aussenwelt antwortet nicht".into() })
    }
}

fn manifest(name: &str, herkunft: Herkunft) -> Werkzeugmanifest {
    Werkzeugmanifest {
        name: name.to_string(),
        anbieter: "Myelith".into(),
        revision: "1".into(),
        lizenz: "PolyForm-Shield-1.0.0".into(),
        art: Werkzeugart::Deterministisch,
        herkunft,
    }
}

struct Aufbau {
    kasten: Werkzeugkasten,
    registratur: Registratur,
    adressen: Vec<(String, MerkleRoot)>,
}

fn aufbau() -> Aufbau {
    let mut kasten = Werkzeugkasten::neu();
    kasten
        .einhaengen(Werkzeug::ohne_parameter("zeit", "Die Zeit."), Box::new(Zeit))
        .expect("eingehaengt");
    kasten
        .einhaengen(Werkzeug::ohne_parameter("kaputt", "Geht schief."), Box::new(Kaputt))
        .expect("eingehaengt");
    let mut registratur = Registratur::neu();
    let a_zeit = registratur.nimm_werkzeug(manifest("zeit", Herkunft::Verankert)).expect("ok");
    let a_kaputt =
        registratur.nimm_werkzeug(manifest("kaputt", Herkunft::Lokal)).expect("ok");
    Aufbau {
        kasten,
        registratur,
        adressen: vec![("zeit".into(), a_zeit), ("kaputt".into(), a_kaputt)],
    }
}

fn kontrakt(max_schritte: u32) -> Sitzungskontrakt {
    Sitzungskontrakt::neu(
        Address::new([1u8; 32]),
        Address::new([2u8; 32]),
        Grenzen { budget: 1000, einzellimit: 100, schwelle: u64::MAX, zeugenleiter: Vec::new() },
        Grenzen { budget: 1000, einzellimit: 100, schwelle: u64::MAX, zeugenleiter: Vec::new() },
        vec![Address::new([9u8; 32])],
        EpochId(0),
        EpochId(100),
        max_schritte,
    )
    .expect("Kontrakt")
}

fn fahren(
    antworten: Vec<&str>,
    betriebsart: Betriebsart,
    max_schritte: u32,
) -> myl_local_agent::schleife::Ergebnis {
    fahren_mit_melder(antworten, betriebsart, max_schritte, None).0
}

/// Wie [`fahren`], aber mit einem Melder, dessen Meldungen mitkommen.
fn fahren_mit_melder(
    antworten: Vec<&str>,
    betriebsart: Betriebsart,
    max_schritte: u32,
    melder: Option<&dyn Fn(myl_local_agent::schleife::Meldung<'_>)>,
) -> (myl_local_agent::schleife::Ergebnis, ()) {
    let (port, _) = stummel(antworten.into_iter().map(String::from).collect());
    let a = aufbau();
    let klient = Tuerklient::neu("127.0.0.1", port, "vollmacht")
        .mit_frist(std::time::Duration::from_secs(10));
    let grenzen = Sitzungsgrenzen::neu(kontrakt(max_schritte), a.kasten.angebote());
    let adressen = a.adressen.clone();
    let finden = move |n: &str| -> Option<MerkleRoot> {
        adressen.iter().find(|(k, _)| k == n).map(|(_, v)| *v)
    };
    let erg = Lauf {
        einhaengung: None,
        klient: &klient,
        modell: "m",
        grenzen: &grenzen,
        betriebsart,
        kasten: &a.kasten,
        registratur: &a.registratur,
        adressen: &finden,
        anker: Hash::from_bytes([7u8; 32]),
        max_tokens: Some(32),
        ansageform: Default::default(),
        melder,
    }
    .fahren("Wie spaet ist es?");
    (erg, ())
}

// --- ⚑ Die Naht: ein Modellweg ohne Netz (CLIENT 0.2) -----------------

/// Ein Modellweg, der **keinen Netzverkehr** treibt.
///
/// ⚑ **Er steht hier als Gegenprobe der Naht.** Bis zum 2026-09-08
/// verlangte [`Lauf`] einen `Tuerklient`, also einen Knoten; der Agent
/// war ohne Netz nutzlos, obwohl das Modell auf derselben Maschine
/// liegen kann. Dass dieser Stummel ohne eine einzige Verbindung durch
/// dieselbe Schleife laeuft, ist der Beleg dafuer, dass die Trennung
/// eine ist.
///
/// ⛑ **Was er NICHT ist:** ein lokales Modell. Er antwortet aus einer
/// Liste. Das eigentliche Einhaengen des Ganzzahllaufwerks gehoert in
/// den Client und ausdruecklich nicht in diese Kiste, die eine
/// Vollmacht traegt und deshalb fast keine Abhaengigkeiten hat.
struct OertlicherWeg {
    antworten: std::cell::RefCell<std::collections::VecDeque<String>>,
    gerufen: std::cell::Cell<usize>,
}

impl OertlicherWeg {
    fn neu(antworten: Vec<&str>) -> Self {
        Self {
            antworten: std::cell::RefCell::new(
                antworten.into_iter().map(String::from).collect(),
            ),
            gerufen: std::cell::Cell::new(0),
        }
    }
}

impl myl_local_agent::Modellweg for OertlicherWeg {
    fn chat(
        &self,
        _modell: &str,
        _nachrichten: &[myl_local_agent::Nachricht],
        _max_tokens: Option<u32>,
    ) -> Result<myl_local_agent::Antwort, myl_local_agent::Tuerfehler> {
        self.gerufen.set(self.gerufen.get() + 1);
        let text = self.antworten.borrow_mut().pop_front().unwrap_or_default();
        Ok(myl_local_agent::Antwort {
            text,
            abschlussgrund: None,
            kennung: "oertlich".to_string(),
            segment: Default::default(),
            prompt_token: 0,
            antwort_token: 0,
        })
    }
}

/// ⚑ **Derselbe Lauf, ohne einen einzigen Netzaufruf.**
#[test]
fn die_schleife_laeuft_ohne_netz() {
    let weg = OertlicherWeg::neu(vec![
        "{\"werkzeug\":\"zeit\",\"argumente\":{}}",
        "Fertig.",
    ]);
    let a = aufbau();
    let grenzen = Sitzungsgrenzen::neu(kontrakt(4), a.kasten.angebote());
    let adressen = a.adressen.clone();
    let finden = move |n: &str| -> Option<MerkleRoot> {
        adressen.iter().find(|(k, _)| k == n).map(|(_, v)| *v)
    };
    let erg = Lauf {
        klient: &weg,
        modell: "oertlich",
        grenzen: &grenzen,
        betriebsart: Betriebsart::Alles,
        einhaengung: None,
        kasten: &a.kasten,
        registratur: &a.registratur,
        adressen: &finden,
        anker: Hash::from_bytes([7u8; 32]),
        max_tokens: Some(32),
        ansageform: Default::default(),
        melder: None,
    }
    .fahren("Wie spaet ist es?");

    assert!(
        weg.gerufen.get() >= 1,
        "der oertliche Weg wurde nie gerufen: die Naht traegt nicht"
    );
    assert!(
        !erg.nachrichten.is_empty(),
        "ohne Netz entstand kein Verlauf, die Schleife lief also nicht"
    );
}

// --- Die Tests --------------------------------------------------------

/// ⚑ **Der Grundfall: ein Werkzeug wird gerufen, dann ist Schluss.**
#[test]
fn ein_erlaubtes_werkzeug_laeuft_und_der_lauf_endet() {
    let e = fahren(
        vec![
            "<tool_call>{\"name\":\"zeit\",\"arguments\":{}}</tool_call>",
            "Es ist 12:00.",
        ],
        Betriebsart::NurVerankert,
        5,
    );
    assert_eq!(e.ende, Ende::Fertig, "{}", e.ende);
    assert_eq!(e.strom.schritte().len(), 2);
    assert_eq!(e.strom.abgelehnte(), 0);
    // Das Ergebnis ist im Gespraech gelandet.
    assert!(
        e.nachrichten.iter().any(|n| n.role == "tool" && n.content.contains("12:00")),
        "das Werkzeugergebnis fehlt"
    );
}

/// ⚑ **Der eingeschleuste Aufruf: er entsteht, wird abgelehnt, und
/// steht im Beleg.**
#[test]
fn ein_eingeschleuster_aufruf_wird_abgelehnt_und_steht_im_beleg() {
    let e = fahren(
        vec!["<tool_call>{\"name\":\"ueberweisen\",\"arguments\":{\"an\":\"0xboese\"}}</tool_call>"],
        Betriebsart::NurVerankert,
        5,
    );
    assert_eq!(e.ende, Ende::Fertig);
    assert_eq!(e.strom.abgelehnte(), 1, "die Ablehnung fehlt im Beleg");
    let bericht = e.strom.bericht();
    assert!(bericht.contains("ueberweisen"), "{bericht}");
    assert!(bericht.contains("⚑"), "ein Angriffsversuch ohne Flagge: {bericht}");
}

/// ⚑ **Die Betriebsart greift VOR der Ausführung.**
///
/// `kaputt` ist lokal, also nicht nachrechenbar. Unter `NurVerankert`
/// läuft es nicht, und das Werkzeug wird gar nicht erst gerufen.
#[test]
fn ein_lokales_werkzeug_laeuft_unter_nur_verankert_nicht() {
    let e = fahren(
        vec!["<tool_call>{\"name\":\"kaputt\",\"arguments\":{}}</tool_call>"],
        Betriebsart::NurVerankert,
        5,
    );
    assert_eq!(e.strom.abgelehnte(), 1);
    assert!(
        e.nachrichten.iter().any(|n| n.content.contains("nicht nachrechenbar")),
        "die Begruendung fehlt im Gespraech"
    );
    // ⚑ Und es lief wirklich nicht: sein Fehlertext taucht nirgends auf.
    assert!(
        !e.nachrichten.iter().any(|n| n.content.contains("Aussenwelt antwortet nicht")),
        "das Werkzeug wurde trotz Sperre gerufen"
    );
}

/// ⚑ **Unter `Alles` läuft es, und sein Fehler ist ein Ergebnis.**
#[test]
fn unter_alles_laeuft_es_und_der_fehler_kommt_zurueck() {
    let e = fahren(
        vec![
            "<tool_call>{\"name\":\"kaputt\",\"arguments\":{}}</tool_call>",
            "Dann eben nicht.",
        ],
        Betriebsart::Alles,
        5,
    );
    assert_eq!(e.strom.abgelehnte(), 0, "unter `alles` ist es erlaubt");
    assert!(
        e.nachrichten.iter().any(|n| n.content.contains("Aussenwelt antwortet nicht")),
        "der Fehler kam nicht als Ergebnis zurueck"
    );
    // ⚑ Und der Strom sagt, dass dieser Schritt nicht nachrechenbar war.
    assert_eq!(e.strom.nicht_nachrechenbare(), vec![0]);
}

/// ⚑ **Die Schrittzahl des Kontrakts hält.**
///
/// Ein Modell, das endlos Werkzeuge ruft, wird vom Kontrakt gestoppt und
/// nicht von einer Zahl im Harness.
#[test]
fn die_schrittzahl_des_kontrakts_haelt() {
    let ruf = "<tool_call>{\"name\":\"zeit\",\"arguments\":{}}</tool_call>";
    let e = fahren(vec![ruf, ruf, ruf, ruf, ruf], Betriebsart::NurVerankert, 2);
    assert!(matches!(e.ende, Ende::Grenze(_)), "{}", e.ende);
    assert_eq!(e.strom.schritte().len(), 2, "mehr Schritte als der Kontrakt zulaesst");
}

/// ⚑ **Ein Lauf, der an einer Grenze endet, hat trotzdem einen Beleg.**
///
/// Ein Beleg, den es nur bei Erfolg gibt, ist keiner.
#[test]
fn auch_ein_abgebrochener_lauf_hat_einen_beleg() {
    let ruf = "<tool_call>{\"name\":\"zeit\",\"arguments\":{}}</tool_call>";
    let e = fahren(vec![ruf, ruf], Betriebsart::NurVerankert, 1);
    assert!(matches!(e.ende, Ende::Grenze(_)));
    assert_eq!(e.strom.schritte().len(), 1);
    assert!(e.strom.kettenglieder().is_ok(), "der Beleg traegt keine Kette");
}

/// ⚑ **Ein Werkzeugkasten mit widersprüchlichen Namen fällt beim
/// Einhängen auf**, nicht beim Laufen.
#[test]
fn widerspruechliche_namen_fallen_beim_einhaengen_auf() {
    let mut k = Werkzeugkasten::neu();
    let fehler = k
        .einhaengen(Werkzeug::ohne_parameter("uhr", "Die Zeit."), Box::new(Zeit))
        .expect_err("Angebot `uhr`, Ausfuehrung `zeit`");
    assert!(fehler.to_string().contains("prueft die Erlaubnis einen anderen Namen"), "{fehler}");

    let mut k2 = Werkzeugkasten::neu();
    k2.einhaengen(Werkzeug::ohne_parameter("zeit", "a"), Box::new(Zeit)).expect("erst");
    assert!(
        k2.einhaengen(Werkzeug::ohne_parameter("zeit", "b"), Box::new(Zeit)).is_err(),
        "ein zweites unter demselben Namen ging durch"
    );
}

// --- ⚑ Der Melder: zusehen, ohne mitzureden --------------------------

/// ⚑ **Ein Melder aendert den Lauf nicht.**
///
/// # ⛑ Die Zusage, um die es geht
///
/// Diese Kiste traegt eine Vollmacht. Ein Haken, der den Lauf
/// beeinflussen koennte, waere eine **zweite Quelle fuer Erlaubnisse**
/// neben `Erlaubnis` und `Betriebsart`, und die darf es nicht geben.
/// Der Melder bekommt zu sehen und gibt nichts zurueck; **dass das
/// wirklich so ist, steht hier zur Pruefung** und nicht nur im
/// Kommentar.
#[test]
fn ein_melder_aendert_den_lauf_nicht() {
    let antworten = vec![
        "Ich sehe nach. <tool_call>{\"name\": \"zeit\", \"arguments\": {}}</tool_call>",
        "Fertig.",
    ];
    let ohne = fahren(antworten.clone(), Betriebsart::Alles, 4);

    let gesehen = std::cell::RefCell::new(Vec::<String>::new());
    let melder = |m: myl_local_agent::schleife::Meldung<'_>| {
        use myl_local_agent::schleife::Meldung as M;
        gesehen.borrow_mut().push(match m {
            M::Schritt(n) => format!("schritt {n}"),
            M::Aufruf { name, .. } => format!("aufruf {name}"),
            M::Ergebnis { name, .. } => format!("ergebnis {name}"),
            M::Abgelehnt { name, .. } => format!("abgelehnt {name}"),
        });
    };
    let (mit, ()) = fahren_mit_melder(antworten, Betriebsart::Alles, 4, Some(&melder));

    assert_eq!(ohne.ende, mit.ende, "der Melder hat das Ende veraendert");
    assert_eq!(
        ohne.nachrichten.len(),
        mit.nachrichten.len(),
        "der Melder hat den Nachrichtenverlauf veraendert"
    );
    for (a, b) in ohne.nachrichten.iter().zip(mit.nachrichten.iter()) {
        assert_eq!(a.role, b.role);
        assert_eq!(a.content, b.content, "der Melder hat eine Nachricht veraendert");
    }

    // ⚑ **Und er hat wirklich etwas gesehen.** Ohne diese Haelfte
    // pruefte der Test nur, dass ein Melder, der nie gerufen wird,
    // nichts kaputtmacht.
    let g = gesehen.borrow();
    assert!(g.contains(&"schritt 1".to_string()), "kein Schritt gemeldet: {g:?}");
    assert!(g.contains(&"aufruf zeit".to_string()), "kein Aufruf gemeldet: {g:?}");
    assert!(g.contains(&"ergebnis zeit".to_string()), "kein Ergebnis gemeldet: {g:?}");
    // Die Reihenfolge ist die Zusage: erst der Aufruf, dann das
    // Ergebnis. Umgekehrt zeigte das Fenster ein Ergebnis zu einem
    // Befehl, von dem es noch nichts weiss.
    let i = g.iter().position(|x| x == "aufruf zeit").expect("Aufruf");
    let j = g.iter().position(|x| x == "ergebnis zeit").expect("Ergebnis");
    assert!(i < j, "das Ergebnis kam vor dem Aufruf: {g:?}");
}

/// ⚑ **Eine Ablehnung wird gemeldet**, denn sonst sieht ein Agent, dem
/// ein Werkzeug verwehrt wurde, fuer den Nutzer aus wie einer, der
/// nichts tut.
#[test]
fn eine_abgelehnte_ausfuehrung_wird_gemeldet() {
    let gesehen = std::cell::RefCell::new(Vec::<String>::new());
    let melder = |m: myl_local_agent::schleife::Meldung<'_>| {
        if let myl_local_agent::schleife::Meldung::Abgelehnt { name, grund } = m {
            gesehen.borrow_mut().push(format!("{name}: {grund}"));
        }
    };
    // ⚑ `kaputt` ist lokal, also nicht nachrechenbar; `NurVerankert`
    // sperrt es **vor** der Ausfuehrung. Genau diesen Fall soll der
    // Nutzer sehen, denn sonst tut der Agent scheinbar nichts.
    let (_, ()) = fahren_mit_melder(
        vec!["<tool_call>{\"name\":\"kaputt\",\"arguments\":{}}</tool_call>", "Fertig."],
        Betriebsart::NurVerankert,
        4,
        Some(&melder),
    );
    let g = gesehen.borrow();
    assert!(!g.is_empty(), "eine Ablehnung wurde nicht gemeldet");
    assert!(g[0].starts_with("kaputt: "), "die Ablehnung nennt das Werkzeug nicht: {g:?}");
}
