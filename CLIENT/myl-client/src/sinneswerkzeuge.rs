//! **Sehen und Hoeren als Werkzeug fuer die Agentenschleife.**
//!
//! # ⚑ Warum diese beiden kompiliert sind und nicht als Manifest liegen
//!
//! Die erste Fassung war eine Werkzeugkiste mit `sh`-Skripten. Drei
//! Gruende haben das umgeworfen, und jeder einzelne haette gereicht:
//!
//! 1. ⛔️ **Ein Manifest laeuft ueber `sh`, und unter Windows gibt es
//!    keine.** Der Client wird fuer Windows ausgeliefert.
//! 2. ⛔️ **Ein Manifest haelt die Einhaengegrenze nicht ein.** Diese
//!    beiden tun es: Der Pfad geht durch [`Einhaengung::aufloesen`],
//!    also kommt kein `../../fremd` durch.
//! 3. ⛔️ **Ein Manifest braucht die Schreiberlaubnis**, weil eine Shell
//!    immer schreiben kann. Ein Bild anzusehen ist aber ein **Lesen**,
//!    und dafuer die Schreiberlaubnis zu verlangen waere eine Schranke
//!    an der falschen Stelle.
//!
//! ⚑ **Sie stehen nur im Angebot, wenn der Sinn eingerichtet ist.** Zwei
//! Werkzeuge, die immer „nicht installiert" antworten, kosten jedes
//! kleine Modell zwei Zeilen Ansage fuer nichts.

use myl_local_agent::ausfuehrung::{Werkzeugausfuehrung, Werkzeugfehler};
use myl_local_agent::werkzeug::{Ansageform, Werkzeug};
use myl_senses::{Sinne, Stufe};

use crate::werkzeuge::Einhaengung;

/// Welcher der beiden Sinne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sinneswerkzeug {
    Sehen,
    Hoeren,
}

impl Sinneswerkzeug {
    /// Wie es in dieser Form heisst.
    pub const fn name(&self, form: Ansageform) -> &'static str {
        match (self, form) {
            (Self::Sehen, Ansageform::Amtlich) => "describe_image",
            (Self::Hoeren, Ansageform::Amtlich) => "transcribe_audio",
            (Self::Sehen, Ansageform::Deutsch) => "bild_beschreiben",
            (Self::Hoeren, Ansageform::Deutsch) => "ton_mitschreiben",
        }
    }

    /// ⚠️ **Die Beschreibung sagt dem Modell, dass die Antwort nicht
    /// seine ist.** Ein Werkzeug, das das verschweigt, laedt dazu ein,
    /// die Auskunft fuer eigenes Sehen zu halten und darueber hinaus zu
    /// erfinden.
    pub fn beschreibung(&self, form: Ansageform) -> String {
        let deutsch = matches!(form, Ansageform::Deutsch);
        match (self, deutsch) {
            (Self::Sehen, true) => "Laesst ein Bild von einem anderen Modell beschreiben und den Text darin vorlesen. Der Pfad ist relativ zum Arbeitsordner; angehaengte Bilder liegen unter .AGENT/anhaenge/. Was die Antwort nicht erwaehnt, ist nicht zu erfragen.".into(),
            (Self::Sehen, false) => "Has a different model describe an image and read out any text in it. The path is relative to the working directory; attached images are under .AGENT/anhaenge/. What the answer does not mention cannot be asked for.".into(),
            (Self::Hoeren, true) => "Laesst eine Tonaufnahme von einem anderen Modell mitschreiben. Der Pfad ist relativ zum Arbeitsordner. Die Mitschrift kann sich verhoeren.".into(),
            (Self::Hoeren, false) => "Has a different model transcribe an audio recording. The path is relative to the working directory. The transcript may mishear.".into(),
        }
    }

    /// Das Argumentschema.
    pub fn parameter(&self, form: Ansageform) -> serde_json::Value {
        let deutsch = matches!(form, Ansageform::Deutsch);
        match self {
            Self::Sehen => serde_json::json!({
                "type": "object",
                "properties": {
                    "pfad": {"type": "string", "description": if deutsch { "Das Bild, relativ zum Arbeitsordner." } else { "The image, relative to the working directory." }},
                    "frage": {"type": "string", "description": if deutsch { "Worauf es ankommt, zum Beispiel: Was steht auf dem Schild?" } else { "What matters, for example: what does the sign say?" }}
                },
                "required": ["pfad"]
            }),
            Self::Hoeren => serde_json::json!({
                "type": "object",
                "properties": {
                    "pfad": {"type": "string", "description": if deutsch { "Die Aufnahme, relativ zum Arbeitsordner." } else { "The recording, relative to the working directory." }},
                    "sprache": {"type": "string", "description": if deutsch { "Sprachkuerzel wie de oder en, oder auto." } else { "Language code such as de or en, or auto." }}
                },
                "required": ["pfad"]
            }),
        }
    }
}

/// **Die Angebote, die dieser Rechner wirklich einloesen kann.**
///
/// ⚑ **Leer, wenn nichts eingerichtet ist.** Siehe Modulkopf.
pub fn angebote(
    sinne: &Sinne,
    ein: &Einhaengung,
    form: Ansageform,
) -> Vec<(Werkzeug, Box<dyn Werkzeugausfuehrung>)> {
    let mut aus: Vec<(Werkzeug, Box<dyn Werkzeugausfuehrung>)> = Vec::new();
    let mut nimm = |w: Sinneswerkzeug| {
        aus.push((
            Werkzeug {
                name: w.name(form).into(),
                beschreibung: w.beschreibung(form),
                parameter: w.parameter(form),
            },
            Box::new(Sinnesausfuehrung { was: w, ein: ein.clone(), form })
                as Box<dyn Werkzeugausfuehrung>,
        ));
    };
    if sinne.sehen.is_ok() {
        nimm(Sinneswerkzeug::Sehen);
    }
    if sinne.hoeren.is_ok() {
        nimm(Sinneswerkzeug::Hoeren);
    }
    aus
}

struct Sinnesausfuehrung {
    was: Sinneswerkzeug,
    ein: Einhaengung,
    form: Ansageform,
}

impl Werkzeugausfuehrung for Sinnesausfuehrung {
    fn name(&self) -> &str {
        self.was.name(self.form)
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let roh = a.get("pfad").and_then(|v| v.as_str()).ok_or_else(|| Werkzeugfehler {
            grund: "es fehlt das Feld `pfad`".into(),
        })?;
        // ⛔️ **Die Einhaengegrenze gilt auch fuers Hinsehen.** Ein
        // Werkzeug, das ein Bild ausserhalb des Arbeitsordners beschreibt,
        // liest hinaus, nur eben in Worten.
        let datei = self.ein.aufloesen(roh, true)?;

        // ⚑ **Frisch gefragt und nicht gemerkt.** Wer waehrend einer
        // Sitzung ein Modell hinlegt, soll es benutzen koennen, ohne den
        // Client neu zu starten; die Suche kostet ein paar Dateiabfragen.
        let sinne = Sinne::finden();
        let (art, frage) = match self.was {
            Sinneswerkzeug::Sehen => {
                (myl_senses::Art::Bild, a.get("frage").and_then(|v| v.as_str()))
            }
            Sinneswerkzeug::Hoeren => {
                (myl_senses::Art::Ton, a.get("sprache").and_then(|v| v.as_str()))
            }
        };
        // ⚑ **Hier die genaue Sprosse.** Wer ein Werkzeug ausdruecklich
        // mit einer Frage ruft, will die bessere Antwort; der Blick beim
        // Anhaengen nimmt die schnelle.
        myl_senses::auswerten(&sinne, &datei, art, frage, Stufe::Genau)
            .ok_or_else(|| Werkzeugfehler { grund: "fuer diese Art sieht hier niemand hin".into() })?
            .map_err(|grund| Werkzeugfehler { grund })
    }
}
