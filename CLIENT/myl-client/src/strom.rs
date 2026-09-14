//! Aus rohen Token wird laufender Text, getrennt nach Denken und Antwort.
//!
//! # ⚑ Warum das hier steht und nicht im Fenster
//!
//! Ein Modell schreibt sein Denken und seine Antwort in **einen** Strom
//! und trennt sie durch Marken: `<think>` und `</think>`, dazu
//! `<tool_call>` fuer einen Werkzeugvorschlag. Wer das im Fenster
//! zerlegte, baute eine zweite Lesart derselben Antwort neben die, die
//! `werkzeug::vorschlaege` schon hat, und zwei Lesarten laufen
//! auseinander.
//!
//! # 📌 Die Schwierigkeit, die es hier ueberhaupt gibt
//!
//! **Eine Marke kommt nicht am Stueck.** Token sind Wortteile, und
//! `</think>` kann als `</`, `think`, `>` eintreffen. Wer den Zuwachs
//! einzeln durchsucht, findet sie nie; wer den ganzen Text jedes Mal
//! durchsucht, gibt Zeichen doppelt heraus.
//!
//! ⚑ **Deshalb haelt dieser Zerleger genau so viel zurueck, wie der
//! Anfang einer Marke lang sein kann**, und gibt alles davor frei. Was
//! stehen bleibt, ist hoechstens elf Zeichen, also der Bruchteil einer
//! Zeile; was herausgeht, ist endgueltig und wird nie zurueckgenommen.

/// Ein Stueck laufender Text, mit seiner Art.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stueck {
    /// Ueberlegung des Modells, zwischen `<think>` und `</think>`.
    Denken(String),
    /// Antworttext.
    Text(String),
}

/// Die Marken, an denen getrennt wird.
///
/// ⚑ **Dieselben, an denen `werkzeug::vorschlaege` liest.** Eine
/// abweichende Liste waere eine zweite Lesart derselben Antwort.
const MARKEN: [(&str, Umschalt); 4] = [
    ("<think>", Umschalt::DenkenAn),
    ("</think>", Umschalt::DenkenAus),
    ("<tool_call>", Umschalt::AufrufAn),
    ("</tool_call>", Umschalt::AufrufAus),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Umschalt {
    DenkenAn,
    DenkenAus,
    AufrufAn,
    AufrufAus,
}

/// Zerlegt einen ankommenden Tokenstrom.
#[derive(Debug, Default)]
pub struct Zerleger {
    /// Was noch nicht herausgegeben werden konnte, weil es der Anfang
    /// einer Marke sein koennte.
    rest: String,
    im_denken: bool,
    /// ⚑ **Ein Werkzeugvorschlag ist kein Antworttext.** Er wird
    /// verschluckt, und was daraus wird, meldet die Schleife mit dem
    /// **gelesenen** Namen. Wer ihn hier auch noch als Text ausgaebe,
    /// zeigte dem Nutzer rohes JSON neben einer Zeile, die dasselbe in
    /// lesbar sagt.
    im_aufruf: bool,
}

impl Zerleger {
    pub fn neu() -> Self {
        Self::default()
    }

    /// Nimmt den Zuwachs und gibt heraus, was endgueltig ist.
    pub fn schluck(&mut self, zuwachs: &str) -> Vec<Stueck> {
        self.rest.push_str(zuwachs);
        let mut aus = Vec::new();
        loop {
            // Die fruehste Marke im Puffer.
            let treffer = MARKEN
                .iter()
                .filter_map(|(m, u)| self.rest.find(m).map(|i| (i, *m, *u)))
                .min_by_key(|(i, _, _)| *i);
            let Some((i, marke, umschalt)) = treffer else { break };
            let davor = self.rest[..i].to_string();
            self.ausgeben(&davor, &mut aus);
            self.rest = self.rest[i + marke.len()..].to_string();
            match umschalt {
                Umschalt::DenkenAn => self.im_denken = true,
                Umschalt::DenkenAus => self.im_denken = false,
                Umschalt::AufrufAn => self.im_aufruf = true,
                Umschalt::AufrufAus => self.im_aufruf = false,
            }
        }
        // Was uebrig ist: alles freigeben ausser dem, was noch der
        // Anfang einer Marke sein koennte.
        let halten = self.moeglicher_markenanfang();
        let frei = self.rest[..self.rest.len() - halten].to_string();
        self.rest = self.rest[self.rest.len() - halten..].to_string();
        self.ausgeben(&frei, &mut aus);
        aus
    }

    /// Am Ende eines Laufs: auch der Rest geht heraus.
    ///
    /// ⚑ **Ohne diesen Schritt verschwaende eine abgebrochene Marke
    /// die letzten Zeichen.** Endet die Antwort auf `</thin`, ist das
    /// keine Marke, sondern Text, und der Nutzer haette ihn nie gesehen.
    pub fn abschluss(&mut self) -> Vec<Stueck> {
        let rest = std::mem::take(&mut self.rest);
        let mut aus = Vec::new();
        self.ausgeben(&rest, &mut aus);
        aus
    }

    fn ausgeben(&self, t: &str, aus: &mut Vec<Stueck>) {
        if t.is_empty() || self.im_aufruf {
            return;
        }
        aus.push(if self.im_denken {
            Stueck::Denken(t.to_string())
        } else {
            Stueck::Text(t.to_string())
        });
    }

    /// Wie viele Bytes am Ende des Puffers der Anfang einer Marke sein
    /// koennten.
    ///
    /// 📌 **Gerechnet auf Zeichengrenzen und nicht auf Bytes.** Alle
    /// Marken sind reines ASCII, ein Schnitt mitten in einer
    /// Mehrbytefolge waere trotzdem moeglich: Wer die letzten drei
    /// Bytes zurueckhaelt und dabei ein Zeichen zerteilt, gibt ein
    /// halbes Zeichen heraus.
    fn moeglicher_markenanfang(&self) -> usize {
        let laengste = MARKEN.iter().map(|(m, _)| m.len()).max().unwrap_or(0);
        for (i, _) in self.rest.char_indices() {
            let schwanz = &self.rest[i..];
            if schwanz.len() >= laengste {
                continue;
            }
            if MARKEN.iter().any(|(m, _)| m.starts_with(schwanz)) {
                return schwanz.len();
            }
        }
        0
    }
}

#[cfg(test)]
mod proben {
    use super::*;

    fn text(s: &[Stueck]) -> String {
        s.iter()
            .filter_map(|x| match x {
                Stueck::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    }
    fn denken(s: &[Stueck]) -> String {
        s.iter()
            .filter_map(|x| match x {
                Stueck::Denken(t) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Alles auf einmal, der einfache Fall.
    #[test]
    fn denken_und_antwort_werden_getrennt() {
        let mut z = Zerleger::neu();
        let mut s = z.schluck("<think>Ich ueberlege.</think>Die Antwort.");
        s.extend(z.abschluss());
        assert_eq!(denken(&s), "Ich ueberlege.");
        assert_eq!(text(&s), "Die Antwort.");
    }

    /// ⚑ **Der Fall, um den es wirklich geht:** Die Marke kommt in
    /// Stuecken, denn Token sind Wortteile.
    #[test]
    fn eine_zerrissene_marke_wird_trotzdem_erkannt() {
        let mut z = Zerleger::neu();
        let mut s = Vec::new();
        for stueck in ["<th", "ink", ">", "Ueber", "legung", "</", "thi", "nk>", "Antwort"] {
            s.extend(z.schluck(stueck));
        }
        s.extend(z.abschluss());
        assert_eq!(denken(&s), "Ueberlegung");
        assert_eq!(text(&s), "Antwort");
    }

    /// 📌 **Und nichts wird doppelt herausgegeben.** Der Puffer haelt
    /// zurueck, was noch eine Marke werden koennte, und gibt es genau
    /// einmal frei.
    #[test]
    fn nichts_kommt_zweimal_heraus() {
        let mut z = Zerleger::neu();
        let mut s = Vec::new();
        for stueck in ["Ant", "wort <", "th", "x"] {
            s.extend(z.schluck(stueck));
        }
        s.extend(z.abschluss());
        assert_eq!(text(&s), "Antwort <thx", "der Text kam nicht genau einmal heraus");
    }

    /// ⚑ **Ein Werkzeugvorschlag ist kein Antworttext.**
    #[test]
    fn ein_aufruf_wird_verschluckt() {
        let mut z = Zerleger::neu();
        let mut s = z.schluck("Ich sehe nach. <tool_call>{\"name\":\"datei_lesen\"}</tool_call> Fertig.");
        s.extend(z.abschluss());
        assert_eq!(text(&s), "Ich sehe nach.  Fertig.");
        assert!(!text(&s).contains("datei_lesen"), "rohes JSON steht im Antworttext");
    }

    /// 📌 **Eine abgebrochene Marke am Ende ist Text und kein Nichts.**
    #[test]
    fn eine_abgebrochene_marke_geht_am_ende_heraus() {
        let mut z = Zerleger::neu();
        let mut s = z.schluck("Antwort</thin");
        s.extend(z.abschluss());
        assert_eq!(text(&s), "Antwort</thin");
    }

    /// 📌 **Ein Umlaut darf nicht zerschnitten werden.** Die Marken
    /// sind ASCII, der Text ist es nicht.
    #[test]
    fn mehrbytezeichen_bleiben_ganz() {
        let mut z = Zerleger::neu();
        let mut s = Vec::new();
        for stueck in ["Grüße", " über", " Bäume<"] {
            s.extend(z.schluck(stueck));
        }
        s.extend(z.abschluss());
        let ganz = text(&s);
        assert_eq!(ganz, "Grüße über Bäume<");
        assert!(!ganz.contains('\u{FFFD}'), "ein Zeichen wurde zerschnitten");
    }

    /// ⚑ **Der laufende Text ist derselbe wie der fertige.** Wer die
    /// Stuecke aneinanderhaengt, bekommt genau das, was ein Lauf ohne
    /// Zerleger geliefert haette, abzueglich der Marken.
    #[test]
    fn stueckweise_ergibt_dasselbe_wie_am_stueck() {
        let ganz = "<think>a b c</think>Antwort mit <tool_call>{}</tool_call> Ende";
        let mut am_stueck = Zerleger::neu();
        let mut a = am_stueck.schluck(ganz);
        a.extend(am_stueck.abschluss());

        let mut einzeln = Zerleger::neu();
        let mut b = Vec::new();
        for c in ganz.chars() {
            b.extend(einzeln.schluck(&c.to_string()));
        }
        b.extend(einzeln.abschluss());

        assert_eq!(text(&a), text(&b), "zeichenweise kommt anderer Text heraus");
        assert_eq!(denken(&a), denken(&b), "zeichenweise kommt anderes Denken heraus");
    }
}
