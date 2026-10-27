//! Myelith in der Konsole.
//!
//! Man geht in ein Verzeichnis, tippt `myelith`, und der Agent arbeitet
//! **darin**. Kein Schalter, keine Einstellung, kein Pfad: Das
//! Arbeitsverzeichnis ist die Antwort auf die Frage, die im Fenster ein
//! Feld auf der Einstellungsseite ist.
//!
//! # ⚑ Warum das Verzeichnis nicht erfragt wird
//!
//! **Wer `myelith` in einem Verzeichnis tippt, hat die Frage schon
//! beantwortet.** Im Fenster ist der Arbeitsordner eine Einstellung,
//! weil ein Fenster nirgends steht; ein Programm in der Konsole steht
//! immer irgendwo, und das ist der Unterschied, aus dem dieses Programm
//! entsteht.
//!
//! ⚠️ **Und deshalb sagt es beim Start, worauf es zugreift.** Ein Agent
//! mit Dateiwerkzeugen in einem Verzeichnis, das der Nutzer nur
//! zufaellig betreten hat, ist genau der Fall, gegen den die
//! Einhaengegrenze gebaut ist. Der Kopf nennt den Ordner, bevor die
//! erste Eingabe moeglich ist.
//!
//! # ⚑ Was hier NICHT liegt
//!
//! Kein Agentenlauf, keine Werkzeuge, keine Einstellungslogik, keine
//! Modellwahl. Alles davon steht in `myl-client` und wird von hier
//! gerufen, genau wie das Fenster es tut. **Zwei Bedienoberflaechen,
//! eine Kiste darunter**; was hier stuende, muesste dort noch einmal
//! stehen.

mod animation;
mod antwort;
mod anzeige;
mod auswahl;
mod eingabe;
mod erhoehung;
mod einstellseite;
mod banner;
mod design;
mod farben;
mod ansi;
mod geometrie;
mod module;
mod modulbanner;
mod schimmer;
mod schirm;
mod janein;
mod miningseite;
mod walletseite;
mod miningzeile;
mod sitzung;
mod wahl;

fn main() {
    // ⛔️ **Der vorgegebene Systemprompt muss zu seiner Pruefsumme passen**,
    //    sonst startet die Konsole nicht (`myl_local_agent::systemprompt`).
    if let Err(f) = myl_local_agent::systemprompt::alle_pruefen() {
        eprintln!("{f}");
        std::process::exit(1);
    }
    let rc = sitzung::fahren();
    // ⚑ **Nach einem gelungenen `/update` startet die Konsole neu**, und
    //   zwar erst hier: Die Sitzung hat ihren Schirm schon aufgeraeumt.
    if sitzung::NEUSTART.load(std::sync::atomic::Ordering::SeqCst) {
        neu_starten();
    }
    std::process::exit(rc);
}

/// **Startet das neu installierte Programm an derselben Stelle**, mit
/// denselben Argumenten.
///
/// ⚑ Unter Unix ersetzt `exec` diesen Prozess: dasselbe Terminal, derselbe
/// Vordergrund. Unter Windows startet ein Kind, und dieser Prozess wartet
/// auf sein Ende; eine laufende `.exe` laesst sich dort nicht ersetzen.
fn neu_starten() {
    let Some(datei) = myl_client::aktualisierung::laufende_datei() else {
        eprintln!("  Das eigene Programm ist nicht auffindbar; bitte myelith neu starten.");
        return;
    };
    let argumente: Vec<String> = std::env::args().skip(1).collect();
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let fehler = std::process::Command::new(&datei).args(&argumente).exec();
        eprintln!("  Neustart ging nicht ({fehler}); bitte myelith neu starten.");
    }
    #[cfg(not(unix))]
    {
        match std::process::Command::new(&datei).args(&argumente).status() {
            Ok(s) => std::process::exit(s.code().unwrap_or(0)),
            Err(f) => eprintln!("  Neustart ging nicht ({f}); bitte myelith neu starten."),
        }
    }
}
