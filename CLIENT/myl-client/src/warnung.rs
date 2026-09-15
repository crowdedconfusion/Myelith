//! Die Warnung vor dem Agentenbetrieb, und die Regeln dazu.
//!
//! # ⚑ Warum der Text hier steht und nicht in der Oberflaeche
//!
//! **Fenster und Konsole muessen dasselbe sagen.** Eine Warnung, die im
//! einen Bedieninstrument strenger ist als im anderen, ist keine
//! Warnung, sondern eine Stichprobe. Und eine Warnung, die zweimal
//! getippt ist, laeuft auseinander, sobald einer sie verbessert; das ist
//! die haeufigste Fehlerklasse dieses Projekts.
//!
//! ⚑ **Uebersetzt wird einmal, an der Grenze**, genau wie bei
//! [`crate::einstellungen::Feld::in_sprache`]: Wer den Text holt, sagt
//! die Sprache, und was ankommt, ist fertig.
//!
//! # ⚠️ Was diese Warnung nicht ist
//!
//! Sie ist **keine Schranke**. Die Schranken sind die Einhaengegrenze,
//! die Schreiberlaubnis und die Betriebsart; sie wirken, ob jemand
//! gelesen hat oder nicht. Die Warnung sagt nur, was auf dem Spiel
//! steht, damit eine Zustimmung eine ist.

use crate::einstellungen::Sprache;

/// Ein Punkt der Anleitung: eine Regel und ihr Grund.
///
/// ⚑ **Der Grund steht dabei.** Eine Regel ohne Grund wird beim ersten
/// Mal befolgt und beim zweiten umgangen.
#[derive(Debug, Clone, Copy)]
pub struct Regel {
    pub regel: &'static str,
    pub grund: &'static str,
}

/// Die Warnung in einer Sprache, fertig zum Anzeigen.
#[derive(Debug, Clone)]
pub struct Warnung {
    /// Der eine Satz, der ueber allem steht, und zugleich die
    /// Ueberschrift.
    ///
    /// 📌 **Hier stand ein `titel` daneben** („Der Agent darf auf diesem
    /// Rechner handeln"). Der Satz sagte nichts, was der Kern darunter
    /// nicht ausfuehrt, und zwei Ueberschriften uebereinander sind eine
    /// zu viel (Auftrag des Projektinhabers, 2026-09-15).
    ///
    /// ⚑ **Eigenes Feld und nicht der erste Absatz des Kerns**
    /// (Auftrag des Projektinhabers, 2026-09-15): So kann jedes
    /// Bedieninstrument ihn hervorheben, statt ihn im Fliesstext
    /// untergehen zu lassen.
    pub achtung: &'static str,
    /// Der Kern, in zwei bis drei Saetzen.
    pub kern: &'static str,
    /// Die Ueberschrift ueber der Anleitung.
    pub anleitung_titel: &'static str,
    pub regeln: Vec<Regel>,
    /// Die Beschriftung der Zustimmung.
    pub zustimmung: &'static str,
    /// Die Beschriftung des Haekchens „nicht wieder zeigen".
    pub nicht_wieder: &'static str,
}

/// **Die Warnung, in der gewaehlten Sprache.**
pub fn warnung(s: Sprache) -> Warnung {
    match s {
        Sprache::De => Warnung {
            achtung: "⚠️  Achtung: KI-Systeme können Fehler machen  ⚠️",
            // 📌 **Zweimal „führt ... aus" stand hier**, und der Satz war
            // dadurch schwer zu lesen. Kürzere Sätze, ein Gedanke je
            // Satz, und am Ende steht, wer dafür geradesteht.
            kern: "Im Agentenbetrieb handelt ein Sprachmodell auf deinem \
                   Rechner. Es liest und schreibt Dateien im eingehängten \
                   Verzeichnis, und je nach Werkzeugkiste führt es auch \
                   Shell-Befehle aus. Dabei können Daten verändert oder \
                   gelöscht werden. Ein Modell kann sich irren, und es \
                   kann durch Inhalte, die es liest, in die Irre geführt \
                   werden. Was dabei geschieht, verantwortest du.",
            anleitung_titel: "Was du einhalten solltest:",
            regeln: REGELN_DE.to_vec(),
            zustimmung: "Verstanden, Agent benutzen",
            nicht_wieder: "Dieses Fenster nicht erneut anzeigen.",
        },
        Sprache::En => Warnung {
            achtung: "⚠️  Careful: AI systems can make mistakes  ⚠️",
            kern: "In agent mode a language model acts on your machine. \
                   It reads and writes files inside the mounted \
                   directory, and depending on the toolbox it also runs \
                   shell commands. Data can be changed or deleted. A \
                   model can be wrong, and it can be misled by content it \
                   reads. What happens is yours to answer for.",
            anleitung_titel: "What you should stick to:",
            regeln: REGELN_EN.to_vec(),
            zustimmung: "Understood, use the agent",
            nicht_wieder: "Do not show this window again.",
        },
    }
}

/// ⚑ **Die Regeln stehen in der Reihenfolge ihres Gewichts**, nicht
/// ihrer Bequemlichkeit: Was am meisten schuetzt, steht oben.
const REGELN_DE: [Regel; 7] = [
    Regel {
        regel: "Hänge nur ein Verzeichnis ein, dessen Verlust du verschmerzt.",
        grund: "Die Einhängegrenze hält die Dateiwerkzeuge darin fest. \
                Was drinnen liegt, ist erreichbar; was draußen liegt, nicht.",
    },
    Regel {
        regel: "Arbeite in einem Verzeichnis unter Versionsverwaltung oder mit Sicherung.",
        grund: "Ein falscher Schreibvorgang ist dann eine Rücknahme und kein Verlust.",
    },
    Regel {
        regel: "Gib die Schreiberlaubnis erst, wenn du sie brauchst.",
        grund: "Lesen und Schreiben sind getrennte Erlaubnisse. Ohne \
                Schreiberlaubnis kann ein Irrtum nichts anrichten.",
    },
    Regel {
        regel: "Bleib im manual mode, solange du dem Auftrag nicht traust.",
        grund: "Dann wird jede schreibende Handlung vorgelegt, mit \
                Werkzeugnamen und Argumenten, und läuft erst nach deiner Zustimmung.",
    },
    Regel {
        regel: "Benutze die Kiste Advanced nur bewusst.",
        grund: "Sie enthält run_command. Ein Shell-Befehl hält die \
                Einhängegrenze NICHT ein und kann alles, was du auch könntest.",
    },
    Regel {
        regel: "Lege nur Werkzeuge in eine Kiste, deren Befehl du gelesen hast.",
        grund: "Ein Manifest ist eine Befehlszeile. Wer es hineinlegt, gibt sie frei.",
    },
    Regel {
        regel: "Misstraue Inhalten, die der Agent liest.",
        grund: "Eine Datei kann Anweisungen enthalten, die an das Modell \
                gerichtet sind. Ein Modell unterscheidet Auftrag und \
                Inhalt nicht zuverlässig.",
    },
];

const REGELN_EN: [Regel; 7] = [
    Regel {
        regel: "Only mount a directory whose loss you could live with.",
        grund: "The mount boundary keeps the file tools inside it. What is \
                inside is reachable, what is outside is not.",
    },
    Regel {
        regel: "Work in a directory under version control or with a backup.",
        grund: "A wrong write is then something you undo, not something you lose.",
    },
    Regel {
        regel: "Grant write permission only when you need it.",
        grund: "Reading and writing are separate permissions. Without \
                writing, a mistake cannot do damage.",
    },
    Regel {
        regel: "Stay in manual mode as long as you do not trust the task.",
        grund: "Every writing action is then put to you, with tool name and \
                arguments, and runs only after you agree.",
    },
    Regel {
        regel: "Use the Advanced box deliberately.",
        grund: "It contains run_command. A shell command does NOT honour the \
                mount boundary and can do anything you could.",
    },
    Regel {
        regel: "Only put tools into a box whose command you have read.",
        grund: "A manifest is a command line. Whoever puts it there releases it.",
    },
    Regel {
        regel: "Distrust content the agent reads.",
        grund: "A file can contain instructions aimed at the model. A model \
                does not reliably tell a task from its material.",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// **Beide Sprachen sagen dasselbe, und zwar gleich viel.**
    ///
    /// ⚑ Eine Warnung, die in einer Sprache eine Regel weniger hat, ist
    /// in dieser Sprache eine andere Warnung.
    #[test]
    fn beide_sprachen_tragen_dieselben_regeln() {
        let d = warnung(Sprache::De);
        let e = warnung(Sprache::En);
        assert_eq!(d.regeln.len(), e.regeln.len(), "verschieden viele Regeln");
        assert!(!d.regeln.is_empty(), "eine Warnung ohne Regeln ist keine");
        for (a, b) in d.regeln.iter().zip(e.regeln.iter()) {
            assert!(!a.regel.trim().is_empty() && !a.grund.trim().is_empty());
            assert!(!b.regel.trim().is_empty() && !b.grund.trim().is_empty());
        }
        for w in [&d, &e] {
            assert!(!w.kern.trim().is_empty());
            assert!(!w.achtung.trim().is_empty());
            assert!(!w.nicht_wieder.trim().is_empty());
        }
    }

    /// ⛔️ **Die zwei Gefahren mit Namen kommen vor.** Wer die Warnung
    /// kuerzt, soll an dieser Pruefung merken, dass er ihr den Inhalt
    /// nimmt: die Shell ohne Einhaengegrenze und der fremde Inhalt, der
    /// das Modell lenkt.
    #[test]
    fn die_warnung_nennt_die_beiden_gefahren() {
        let d = warnung(Sprache::De);
        let alles: String = d
            .regeln
            .iter()
            .map(|r| format!("{} {}", r.regel, r.grund))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(alles.contains("run_command"), "die Shell fehlt: {alles}");
        assert!(
            alles.contains("Einhängegrenze NICHT ein"),
            "dass die Shell die Grenze nicht haelt, fehlt"
        );
        assert!(
            alles.contains("Anweisungen"),
            "der fremde Inhalt, der das Modell lenkt, fehlt"
        );
    }
}
