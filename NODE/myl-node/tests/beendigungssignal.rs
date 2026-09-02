//! Ein Knoten, der ein Beendigungssignal bekommt, schreibt es hin.
//!
//! # ⚑ Wogegen dieser Test geschrieben ist (Fund 123)
//!
//! Bis zum 2026-09-02 behandelte der Knoten allein `ctrl_c`, also
//! **SIGINT**, das Signal einer Tastatur. **Unter systemd, Docker und
//! Kubernetes kommt SIGTERM**, und daran starb er wortlos: kein
//! Abschlusseintrag, keine Zustandsaufnahme, nach der Schonfrist ein
//! SIGKILL.
//!
//! Damit fiel genau die Unterscheidung weg, für die der Abschlusseintrag
//! gebaut wurde. `knoten.rs` sagt es selbst: „absichtlich beendet" ließ
//! sich von „abgestürzt" nicht unterscheiden, und bei einem Lauf über
//! mehrere Maschinen ist das die erste Frage, wenn ein Protokoll kürzer
//! ist als die anderen. **Gelöst war sie für den Probelauf und offen im
//! Betrieb**, also dort, wo sie zählt.
//!
//! # Was dieser Test tut
//!
//! Er startet das echte Binary, schickt ihm ein echtes Signal und liest
//! das Protokoll. ⚑ **Ein Modultest könnte das nicht:** Signale gehen an
//! einen Prozess, und `cargo test` ist ein anderer.
//!
//! # ⚑ Und er wartet auf die richtige Wirkung, nicht auf die Uhr
//!
//! **Beim ersten Anlauf wartete er auf die Startzeile im Protokoll, und
//! das war die falsche Marke.** Der Knoten wartet vor seiner
//! Ereignisschleife bis zu **acht Sekunden auf eine QUIC-Adresse** und
//! danach bis zu **fünf auf irgendeine Horchadresse**; die Startzeile
//! steht lange davor. Ein Signal in diesem Fenster trifft auf **keinen
//! eingehängten Handler**, denn der wird erst in `laufen_bis`
//! eingehängt, und der Prozess stirbt an der Vorgabewirkung von SIGTERM.
//!
//! Der Test meldete damit einen Fehler, den es nicht gab, und zwar
//! zweimal, bis eine Spurausgabe zeigte, dass `laufen_bis` gar nicht
//! erreicht war.
//!
//! **Die richtige Marke ist die letzte Zeile des Starts auf der
//! Fehlerausgabe**, also „erreichbar unter" oder „noch keine
//! Horchadresse gemeldet". Danach folgt unmittelbar `laufen_bis`.
//!
//! ⚑ **Und das Fenster selbst ist ein Befund** (Fund 140): In den ersten
//! rund dreizehn Sekunden behandelt der Knoten **kein** Signal. Wer ihn
//! in einem Container startet und schnell wieder stoppt, bekommt einen
//! harten Abbruch ohne Abschlusseintrag.

#![cfg(unix)]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Großzügig: Die Frist belegt, dass etwas **kommt**, und zu langes
/// Warten kostet nur Laufzeit.
const FRIST: Duration = Duration::from_secs(60);

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("myl-beendigung-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("Verzeichnis");
    d
}

/// Wartet, bis `bedingung` wahr ist, oder bis die Frist um ist.
fn warte_auf(was: &str, mut bedingung: impl FnMut() -> bool) {
    let start = Instant::now();
    while start.elapsed() < FRIST {
        if bedingung() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("{was} ist innerhalb von {FRIST:?} nicht eingetreten");
}

fn protokolltext(verzeichnis: &Path) -> String {
    let mut alles = String::new();
    let Ok(inhalt) = std::fs::read_dir(verzeichnis) else {
        return alles;
    };
    for e in inhalt.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "jsonl") {
            if let Ok(mut f) = std::fs::File::open(&p) {
                let _ = f.read_to_string(&mut alles);
            }
        }
    }
    alles
}

fn starte(logs: &Path, schluessel: &Path, stderr: &Path) -> Child {
    let datei = std::fs::File::create(stderr).expect("stderr-Datei");
    Command::new(env!("CARGO_BIN_EXE_myl-node"))
        .args([
            "--name",
            "signalprobe",
            "--protokolle",
            logs.to_str().unwrap(),
            "--schluessel",
            schluessel.to_str().unwrap(),
            // ⚑ `--horche`, nicht `--horchen`. Der erste Anlauf nahm den
            // falschen Namen, der Knoten brach mit „unbekannte Angabe"
            // ab, und der Test wartete auf eine Startzeile, die nie kam.
            "--horche",
            "/ip4/127.0.0.1/tcp/0",
            // Lang genug, dass die Laufzeit den Test nicht beendet.
            "--laufzeit",
            "300",
            // ⚑ Das Konformitaetstor sucht die Vektoren **relativ zum
            // Arbeitsverzeichnis**, und das ist bei `cargo test` nicht
            // die Wurzel des Repositoriums. Ohne diese Angabe startet
            // der Knoten gar nicht, und zwar zu Recht: Ein Knoten, der
            // nicht weiss, ob er wie das Netz rechnet, gehoert nicht
            // ins Netz. Der zweite Anlauf dieses Tests ist genau daran
            // gescheitert.
            "--konformitaet",
            vektoren().to_str().unwrap(),
            // Dieser Test prueft Signale, nicht Metriken. Ein fester
            // Port kollidierte, sobald zwei Tests nebeneinander laufen.
            "--ohne-beobachtung",
            // ⚑ Die Kette in das eigene Verzeichnis, nicht ins
            // Arbeitsverzeichnis. Seit der Vorgabe aus Fund 122 legte
            // dieser Test sonst ein `kette.dat` neben die Quelldateien.
            "--kette",
            logs.join("kette.dat").to_str().unwrap(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::from(datei))
        .spawn()
        .expect("myl-node startet")
}

fn text(pfad: &Path) -> String {
    std::fs::read_to_string(pfad).unwrap_or_default()
}

/// Der Ort der Konformitaetsvektoren, absolut statt relativ.
fn vektoren() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../INTEGER_LLM/conformance/vectors/op")
}

/// Schickt `signal` an `pid`.
fn sende(pid: u32, signal: i32) {
    // Ohne `libc`-Abhaengigkeit: `kill` ist auf jedem Unix da, und der
    // Test laeuft ohnehin nur dort.
    let status = Command::new("kill")
        .args([format!("-{signal}"), pid.to_string()])
        .status()
        .expect("kill laeuft");
    assert!(status.success(), "kill -{signal} an {pid} ist gescheitert");
}

/// Der eigentliche Lauf: Signal schicken, Abschlusseintrag erwarten.
fn lauf(signal: i32, erwarteter_grund: &str, name: &str) {
    let logs = scratch(name);
    let schluessel = scratch(&format!("{name}-key")).join("knoten.key");
    let stderr = logs.join("stderr.txt");
    let mut kind = starte(&logs, &schluessel, &stderr);

    // ⚑ Auf die **richtige** Wirkung warten: Der Knoten ist erst in
    // seiner Ereignisschleife, wenn die Adressausgabe durch ist. Vorher
    // haengt kein Signalhandler, und ein Signal toetet ihn hart.
    warte_auf("das Ende des Startvorgangs", || {
        let s = text(&stderr);
        s.contains("erreichbar unter") || s.contains("noch keine Horchadresse gemeldet")
    });

    sende(kind.id(), signal);

    // Und auf die zweite Wirkung: Der Prozess ist weg.
    warte_auf("das Ende des Prozesses", || {
        matches!(kind.try_wait(), Ok(Some(_)))
    });

    let text = protokolltext(&logs);
    assert!(
        text.contains("\"ende\"") || text.contains("ende"),
        "nach Signal {signal} fehlt der Abschlusseintrag. Protokoll:\n{}",
        &text[text.len().saturating_sub(600)..]
    );
    assert!(
        text.contains(erwarteter_grund),
        "der Abschlusseintrag nennt nicht `{erwarteter_grund}`. Protokoll:\n{}",
        &text[text.len().saturating_sub(600)..]
    );

    let _ = std::fs::remove_dir_all(&logs);
}

/// **SIGTERM wird behandelt**, und das ist der Fund.
///
/// Signal 15 ist, was systemd, Docker und Kubernetes schicken.
#[test]
fn sigterm_schreibt_einen_abschlusseintrag() {
    lauf(15, "Beendigungssignal", "term");
}

/// **SIGINT weiterhin auch**, und mit eigenem Grund.
///
/// ⚑ Die Gegenprobe zur ersten Hälfte: Ein Umbau, der SIGTERM
/// hinzufügt und SIGINT dabei verliert, bestünde den ersten Test und
/// wäre eine Verschlechterung. Und die beiden Gründe müssen sich
/// **unterscheiden**, sonst sagt das Protokoll nicht, wer beendet hat.
#[test]
fn sigint_schreibt_einen_anderen_grund() {
    lauf(2, "Abbruchsignal", "int");
}

/// ⚑ **Auch im Startvorlauf**, und das ist Fund 140.
///
/// Der Knoten wartet vor seiner Ereignisschleife bis zu acht Sekunden
/// auf eine QUIC-Adresse und danach bis zu fuenf auf irgendeine
/// Horchadresse. Bis zum 2026-09-02 haengte in diesem Fenster **kein
/// Signalhandler**: Ein SIGTERM riss den Knoten hart heraus, ohne
/// Abschlusseintrag, und wer einen Knoten im Container startet und
/// schnell wieder stoppt, traf immer genau dieses Fenster.
///
/// Der Test schickt das Signal, **sobald der Prozess ueberhaupt
/// spricht**, also lange vor der Adressausgabe. Er ist die Umkehrung
/// der beiden anderen: Die warten ausdruecklich, bis der Vorlauf durch
/// ist, dieser ausdruecklich nicht.
#[test]
fn ein_signal_im_startvorlauf_wird_auch_behandelt() {
    let name = "vorlauf";
    let logs = scratch(name);
    let schluessel = scratch(&format!("{name}-key")).join("knoten.key");
    let stderr = logs.join("stderr.txt");
    let mut kind = starte(&logs, &schluessel, &stderr);

    // Die erste Zeile, die der Knoten schreibt, steht **vor** dem
    // Warten auf Adressen. Sobald sie da ist, ist der Knoten gebaut,
    // die Wache steht, und der Vorlauf hat begonnen.
    warte_auf("die erste Lebensaeusserung", || {
        text(&stderr).contains("myl-node: Peer-Id")
    });
    // Und ausdruecklich **nicht** bis zur Adressausgabe warten: Genau
    // das Fenster dazwischen ist der Fund.
    assert!(
        !text(&stderr).contains("erreichbar unter"),
        "der Vorlauf war schon durch, der Test prueft dann das Falsche"
    );

    let angefangen = std::time::Instant::now();
    sende(kind.id(), 15);

    warte_auf("das Ende des Prozesses", || {
        matches!(kind.try_wait(), Ok(Some(_)))
    });
    let gebraucht = angefangen.elapsed();

    // ⚑ **Und zwar zuegig**, nicht erst nach dem Vorlauf.
    //
    // Das Einhaengen der Wache allein reichte nicht: Danach ueberlebte
    // der Knoten das Signal zwar, arbeitete den Vorlauf aber zu Ende
    // und antwortete erst dann. **Gemessen: 8,6 Sekunden statt 58
    // Millisekunden.** Dockers Vorgabe fuer `stop` sind zehn Sekunden,
    // danach kommt SIGKILL; auf einer langsameren Maschine waere das
    // hart abgelaufen. Deshalb steht der Vorlauf selbst in einem
    // `select!` gegen die Wache.
    assert!(
        gebraucht < Duration::from_secs(3),
        "vom Signal bis zum Ende vergingen {gebraucht:?}. Der Vorlauf \
         laeuft offenbar zu Ende, statt abgebrochen zu werden"
    );

    let inhalt = protokolltext(&logs);
    assert!(
        inhalt.contains("Beendigungssignal"),
        "ein SIGTERM im Startvorlauf hinterlaesst keinen Abschlusseintrag. \
         Protokoll:\n{}\nAusgabe:\n{}",
        &inhalt[inhalt.len().saturating_sub(600)..],
        text(&stderr)
    );

    let _ = std::fs::remove_dir_all(&logs);
}
