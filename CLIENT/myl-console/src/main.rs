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
mod anzeige;
mod auswahl;
mod eingabe;
mod erhoehung;
mod einstellseite;
mod banner;
mod design;
mod farben;
mod schirm;
mod sitzung;
mod wahl;

fn main() {
    std::process::exit(sitzung::fahren());
}
