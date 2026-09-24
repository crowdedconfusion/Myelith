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
    /// **Ein Blick auf den Bildschirm**, jetzt.
    Bildschirm,
    /// **Ein Blick durch die Kamera**, jetzt.
    Kamera,
}

impl Sinneswerkzeug {
    /// Wie es in dieser Form heisst.
    pub const fn name(&self, form: Ansageform) -> &'static str {
        match (self, form) {
            (Self::Sehen, Ansageform::Amtlich) => "describe_image",
            (Self::Hoeren, Ansageform::Amtlich) => "transcribe_audio",
            (Self::Bildschirm, Ansageform::Amtlich) => "look_at_screen",
            (Self::Kamera, Ansageform::Amtlich) => "look_through_camera",
            (Self::Sehen, Ansageform::Deutsch) => "bild_beschreiben",
            (Self::Hoeren, Ansageform::Deutsch) => "ton_mitschreiben",
            (Self::Bildschirm, Ansageform::Deutsch) => "bildschirm_ansehen",
            (Self::Kamera, Ansageform::Deutsch) => "kamera_ansehen",
        }
    }

    /// ⚠️ **Die Beschreibung sagt dem Modell, dass die Antwort nicht
    /// seine ist.** Ein Werkzeug, das das verschweigt, laedt dazu ein,
    /// die Auskunft fuer eigenes Sehen zu halten und darueber hinaus zu
    /// erfinden.
    pub fn beschreibung(&self, form: Ansageform) -> String {
        let deutsch = matches!(form, Ansageform::Deutsch);
        match (self, deutsch) {
            (Self::Sehen, true) => "Laesst ein Bild von einem anderen Modell ansehen. Der Pfad ist relativ zum Arbeitsordner; angehaengte Bilder liegen unter .AGENT/anhaenge/. Stelle in `frage` genau das, was du wissen willst, zum Beispiel 'Wie viele Finger?' oder 'Was steht auf dem Schild?'; ohne Frage kommt nur eine allgemeine Beschreibung zurueck.".into(),
            (Self::Sehen, false) => "Has a different model look at an image. The path is relative to the working directory; attached images are under .AGENT/anhaenge/. Put exactly what you want to know in `frage`, for example 'how many fingers?' or 'what does the sign say?'; without a question you only get a general description.".into(),
            (Self::Bildschirm, true) => "Nimmt den Bildschirm JETZT auf und laesst das Bild von einem anderen Modell ansehen. Stelle in `frage` genau das, was du wissen willst. Die Aufnahme bleibt unter .AGENT/blicke/ liegen.".into(),
            (Self::Bildschirm, false) => "Captures the screen NOW and has a different model look at it. Put exactly what you want to know in `frage`. The capture is kept under .AGENT/blicke/.".into(),
            (Self::Kamera, true) => "Nimmt ein Kamerabild JETZT auf und laesst es von einem anderen Modell ansehen. Stelle in `frage` genau das, was du wissen willst. Die Aufnahme bleibt unter .AGENT/blicke/ liegen.".into(),
            (Self::Kamera, false) => "Captures a camera image NOW and has a different model look at it. Put exactly what you want to know in `frage`. The capture is kept under .AGENT/blicke/.".into(),
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
            // ⛔️ **`frage` ist hier Pflicht und bei `Sehen` nicht.**
            //    Ein Bild liegt schon da und laesst sich noch einmal
            //    ansehen; ein Blick ist ein **Augenblick**, und wer ihn
            //    ohne Frage nimmt, bekommt eine allgemeine Beschreibung
            //    des Augenblicks, der dann vorbei ist. Gemessen: Auf
            //    „wie viele blaue Kreise" kam die Zahl, aus der
            //    allgemeinen Beschreibung desselben Bildes nie.
            Self::Bildschirm | Self::Kamera => serde_json::json!({
                "type": "object",
                "properties": {
                    "frage": {"type": "string", "description": if deutsch { "Genau das, was du wissen willst, zum Beispiel: Wie viele Finger halte ich hoch?" } else { "Exactly what you want to know, for example: how many fingers am I holding up?" }}
                },
                "required": ["frage"]
            }),
        }
    }
}

/// **Die Angebote, die dieser Rechner wirklich einloesen kann.**
///
/// ⚑ **Leer, wenn nichts eingerichtet ist.** Siehe Modulkopf.
/// **Ob der Agent von sich aus aufnehmen darf.**
///
/// # ⛔️ Warum das eine eigene Befugnis ist und nicht „noch ein Lesewerkzeug"
///
/// Ein Bild anzusehen, das schon im Arbeitsordner liegt, ist ein
/// **Lesen**, und die Einhaengegrenze fasst es ein. Den Bildschirm oder
/// die Kamera aufzunehmen ist etwas anderes: Es entsteht dabei etwas,
/// das vorher nicht da war, und es kommt aus einem Raum, den keine
/// Einhaengung begrenzt. **Wer eine Einhaengung setzt, hat damit nicht
/// gesagt, dass jemand ins Zimmer sehen darf.**
///
/// ⚑ **Festlegung des Projektinhabers vom 2026-09-23:** nur bei
/// ausdruecklicher Scharfstellung, und jede Aufnahme bleibt als Datei
/// liegen, damit nachsehbar ist, was gesehen wurde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Blickbefugnis {
    pub bildschirm: bool,
    pub kamera: bool,
}

impl Blickbefugnis {
    /// Nichts darf, die Vorgabe.
    pub fn keine() -> Self {
        Self::default()
    }

    /// Beides erlaubt.
    pub fn alles() -> Self {
        Self { bildschirm: true, kamera: true }
    }

    /// **Die Vorgabe, und die Umgebung darf sie uebersteuern.**
    ///
    /// ⚑ **Drei Zustaende, nicht zwei.** Die Variable ist **nicht
    /// gesetzt** (dann gilt die Vorgabe), sie sagt ausdruecklich ja,
    /// oder sie sagt etwas anderes. Ohne den dritten Fall koennte die
    /// Umgebung nur anschalten, und in der Konsole, wo die Vorgabe „an"
    /// ist, gaebe es keinen Weg, sie fuer einen Lauf abzuschalten.
    ///
    /// ⚠️ **„Etwas anderes" heisst nein, immer.** Ein Tippfehler faellt
    /// damit auf die sichere Seite, ganz gleich, wie die Vorgabe steht.
    pub fn fuer(vorgabe: Self) -> Self {
        let frage = |name: &str, vorgabe: bool| match std::env::var(name) {
            Err(_) => vorgabe,
            Ok(w) if w.trim().is_empty() => vorgabe,
            Ok(w) => matches!(w.trim().to_ascii_lowercase().as_str(), "1" | "ja" | "an"),
        };
        Self {
            bildschirm: frage("MYL_BLICK_BILDSCHIRM", vorgabe.bildschirm),
            kamera: frage("MYL_BLICK_KAMERA", vorgabe.kamera),
        }
    }

    /// **Was die Agenteneinstellung sagt**, samt Uebersteuerung.
    ///
    /// ⚑ **Eine Stelle, an der die Frage beantwortet wird.** Die
    /// Ruestung, die Werkzeugliste im Fenster und `myl sinne` fragen
    /// alle hier; drei eigene Ableitungen waeren drei Antworten auf
    /// dieselbe Frage.
    pub fn aus_einstellung(agent: &crate::einstellungen::Agenteneinstellung) -> Self {
        Self::fuer(Self { bildschirm: agent.blick_bildschirm, kamera: agent.blick_kamera })
    }
}

pub fn angebote(
    sinne: &Sinne,
    ein: &Einhaengung,
    form: Ansageform,
    befugnis: Blickbefugnis,
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
    // ⛔️ **Zwei Bedingungen, und beide muessen gelten:** Das Geraet muss
    // da sein UND die Befugnis erteilt. Ein Werkzeug, das im Angebot
    // steht und dann „nicht erlaubt" antwortet, kostet jedes kleine
    // Modell eine Zeile Ansage fuer nichts, genau wie eines ohne Geraet.
    if befugnis.bildschirm && sinne.bildschirm.is_ok() && sinne.sehen.is_ok() {
        nimm(Sinneswerkzeug::Bildschirm);
    }
    if befugnis.kamera && sinne.kamera.is_ok() && sinne.sehen.is_ok() {
        nimm(Sinneswerkzeug::Kamera);
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
        // ⚑ **Die beiden Blicke bringen ihr Bild selbst mit**, statt
        //   eines zu suchen. Alles danach ist derselbe Weg.
        if matches!(self.was, Sinneswerkzeug::Bildschirm | Sinneswerkzeug::Kamera) {
            return self.blicken(a);
        }
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
            // ⚑ **Hier unerreichbar, und trotzdem benannt.** Die beiden
            //   Blicke sind oben schon abgebogen; ein `_ =>` haette die
            //   Abbiegung stillschweigend mitgedeckt, falls sie einmal
            //   wegfaellt.
            Sinneswerkzeug::Bildschirm | Sinneswerkzeug::Kamera => {
                return Err(Werkzeugfehler {
                    grund: "ein Blick nimmt sein Bild selbst auf und kommt hier nicht an".into(),
                })
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

impl Sinnesausfuehrung {
    /// **Aufnehmen, ansehen, und die Aufnahme liegen lassen.**
    ///
    /// ⚑ **Die Aufnahme wird nicht weggeraeumt**, und das ist die
    /// zweite Haelfte der Befugnis (siehe [`Blickbefugnis`]): Wer
    /// nachsehen will, was der Agent gesehen hat, findet es unter
    /// `.AGENT/blicke/`. Eine Aufnahme, die nur im Speicher existiert,
    /// ist von aussen nicht nachpruefbar.
    fn blicken(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let frage = a
            .get("frage")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|f| !f.is_empty())
            .ok_or_else(|| Werkzeugfehler {
                grund: "es fehlt das Feld `frage`; sage genau, was du wissen willst".into(),
            })?;

        let ordner = self.ein.wurzel().join(".AGENT").join("blicke");
        std::fs::create_dir_all(&ordner)
            .map_err(|f| Werkzeugfehler { grund: format!("{} liess sich nicht anlegen: {f}", ordner.display()) })?;
        let stamm = match self.was {
            Sinneswerkzeug::Kamera => "kamera",
            _ => "bildschirm",
        };
        let ziel = ordner.join(
            myl_senses::zwischenname(stamm, "png")
                .file_name()
                .expect("Dateiname"),
        );

        let sinne = Sinne::finden();
        let genommen = match self.was {
            Sinneswerkzeug::Kamera => {
                let zeug = sinne.kamera.as_ref().map_err(|m| Werkzeugfehler { grund: m.bericht() })?;
                myl_senses::blick::kamera(zeug, &ziel)
            }
            _ => {
                let zeug =
                    sinne.bildschirm.as_ref().map_err(|m| Werkzeugfehler { grund: m.bericht() })?;
                myl_senses::blick::bildschirm(zeug, &ziel)
            }
        };
        genommen.map_err(|grund| Werkzeugfehler { grund })?;

        // ⚑ **Die genaue Sprosse, und dafuer gibt es eine Messung**
        //   (2026-09-23): Auf einer freigestellten Hand mit einem
        //   ausgestreckten Finger antwortete das genaue Modell „1", das
        //   schnelle „5", also die Vorannahme „eine Hand hat fuenf
        //   Finger". Ein Blick, der gezaehlt werden soll, braucht das
        //   genaue.
        let antwort = myl_senses::auswerten(&sinne, &ziel, myl_senses::Art::Bild, Some(frage), Stufe::Genau)
            .ok_or_else(|| Werkzeugfehler { grund: "fuer diese Art sieht hier niemand hin".into() })?
            .map_err(|grund| Werkzeugfehler { grund })?;

        // ⚠️ **Wo die Aufnahme liegt, gehoert in die Antwort.** Sonst
        //    steht im Verlauf eine Auskunft ueber ein Bild, das niemand
        //    mehr findet.
        let wo = ziel.strip_prefix(self.ein.wurzel()).unwrap_or(&ziel);
        Ok(format!("{antwort}\n\n(Aufnahme: {})", wo.display()))
    }
}

#[cfg(test)]
mod proben {
    use super::*;
    use myl_senses::laufwerk::{Bildschirmzeug, Kamerazeug, Mangel, Sehen, Sehzeug};
    use std::path::PathBuf;

    fn mangel() -> Mangel {
        Mangel { sinn: "Probe", fehlt: Vec::new(), anleitung: Vec::new() }
    }

    fn sehzeug() -> Sehzeug {
        Sehzeug {
            programm: PathBuf::from("/nirgends/llama-mtmd-cli"),
            modell: PathBuf::from("/nirgends/sehen.gguf"),
            projektor: PathBuf::from("/nirgends/sehen-mmproj.gguf"),
        }
    }

    /// Ein Rechner, auf dem alles da ist.
    fn alles_da() -> Sinne {
        Sinne {
            sehen: Ok(Sehen { schnell: Some(sehzeug()), genau: Some(sehzeug()) }),
            hoeren: Err(mangel()),
            sprechen: Err(mangel()),
            aufnehmen: Err(mangel()),
            bildschirm: Ok(Bildschirmzeug {
                programm: PathBuf::from("/usr/sbin/screencapture"),
                quelle: None,
            }),
            kamera: Ok(Kamerazeug {
                programm: PathBuf::from("/usr/bin/ffmpeg"),
                quelle: ("avfoundation".into(), "0".into()),
            }),
            // ⚑ Kein PDF-Leser: Diese Probenhilfe prueft die Sinne,
            //   und Schrift ist keiner von ihnen.
            schrift: Err(mangel()),
        }
    }

    fn namen(b: Blickbefugnis) -> Vec<String> {
        let ein = Einhaengung::neu(std::env::temp_dir(), false).expect("Einhaengung");
        angebote(&alles_da(), &ein, Ansageform::Deutsch, b)
            .into_iter()
            .map(|(w, _)| w.name)
            .collect()
    }

    /// ⛔️ **Ohne Scharfstellung wird nicht geblickt**, auch wenn das
    /// Geraet da ist. Das ist die ganze Befugnis; faellt diese Probe,
    /// nimmt der Agent ungefragt auf.
    #[test]
    fn ohne_befugnis_steht_kein_blick_im_angebot() {
        let n = namen(Blickbefugnis::keine());
        assert!(!n.iter().any(|x| x == "bildschirm_ansehen"), "{n:?}");
        assert!(!n.iter().any(|x| x == "kamera_ansehen"), "{n:?}");
        assert!(n.iter().any(|x| x == "bild_beschreiben"), "das Ansehen selbst fehlt: {n:?}");
    }

    /// ⚑ **Die Gegenrichtung, sonst prüfte die Probe darüber nichts.**
    #[test]
    fn mit_befugnis_steht_er_drin() {
        let n = namen(Blickbefugnis { bildschirm: true, kamera: true });
        assert!(n.iter().any(|x| x == "bildschirm_ansehen"), "{n:?}");
        assert!(n.iter().any(|x| x == "kamera_ansehen"), "{n:?}");
    }

    /// ⚑ **Jede der beiden fuer sich.** Wer den Bildschirm freigibt, hat
    /// die Kamera nicht freigegeben.
    #[test]
    fn die_beiden_befugnisse_sind_getrennt() {
        let n = namen(Blickbefugnis { bildschirm: true, kamera: false });
        assert!(n.iter().any(|x| x == "bildschirm_ansehen"), "{n:?}");
        assert!(!n.iter().any(|x| x == "kamera_ansehen"), "{n:?}");
    }

    /// ⛔️ **Ohne Geraet nuetzt die Befugnis nichts.** Ein Werkzeug, das
    /// im Angebot steht und dann „geht hier nicht" sagt, kostet jedes
    /// kleine Modell eine Zeile Ansage fuer nichts.
    fn ohne_geraet() -> Sinne {
        let mut s = alles_da();
        s.bildschirm = Err(mangel());
        s.kamera = Err(mangel());
        s
    }

    #[test]
    fn ohne_geraet_steht_kein_blick_im_angebot() {
        let ein = Einhaengung::neu(std::env::temp_dir(), false).expect("Einhaengung");
        let n: Vec<String> = angebote(
            &ohne_geraet(),
            &ein,
            Ansageform::Deutsch,
            Blickbefugnis { bildschirm: true, kamera: true },
        )
        .into_iter()
        .map(|(w, _)| w.name)
        .collect();
        assert!(!n.iter().any(|x| x.ends_with("_ansehen")), "{n:?}");
    }

    /// ⛔️ **Die Frage ist Pflicht.** Ein Blick ohne Frage liefert eine
    /// allgemeine Beschreibung eines Augenblicks, der dann vorbei ist.
    #[test]
    fn ein_blick_verlangt_eine_frage() {
        for w in [Sinneswerkzeug::Bildschirm, Sinneswerkzeug::Kamera] {
            let p = w.parameter(Ansageform::Deutsch);
            let pflicht = p["required"].as_array().expect("required");
            assert!(
                pflicht.iter().any(|v| v == "frage"),
                "{:?}: `frage` ist nicht Pflicht: {p}",
                w.name(Ansageform::Deutsch)
            );
        }
    }

    /// ⚠️ **Nur ausdrueckliche Zustimmung zaehlt.** Ein Tippfehler in der
    /// Umgebung darf die Kamera nicht anschalten.
    #[test]
    fn nur_ausdrueckliches_ja_schaltet_frei() {
        for (wert, erwartet) in [("1", true), ("ja", true), ("AN", true), ("", false), ("vielleicht", false), ("0", false)] {
            unsafe { std::env::set_var("MYL_BLICK_KAMERA", wert) };
            let b = Blickbefugnis::fuer(Blickbefugnis::keine());
            unsafe { std::env::remove_var("MYL_BLICK_KAMERA") };
            assert_eq!(b.kamera, erwartet, "MYL_BLICK_KAMERA={wert:?}");
        }
    }
}
