//! Der Lauf: die Teile, von Anfang bis Ende (AGENT_LAYER Phase 5).
//!
//! # ⚑ Was hier zum ersten Mal zusammenkommt
//!
//! Bis zum 2026-09-05 gab es die Teile und keinen Lauf: Die
//! [`crate::Erlaubnis`] erlaubte, die
//! [`crate::werkzeug::argumente_pruefen`] prüfte die Form,
//! [`crate::ausfuehrung::Werkzeugkasten`] konnte ausführen, und
//! **nichts rief sie in einer Reihenfolge**. Ein Harness aus Teilen, die
//! nie zusammen gelaufen sind, ist genau die Lage, aus der die Funde 173
//! bis 175 entstanden sind.
//!
//! # Die Reihenfolge, und jede Stufe hat ihren Grund
//!
//! ```text
//! 1. Schrittzahl        Sitzungskontrakt: max_schritte
//! 2. Modell fragen      die Tuer
//! 3. Vorschlaege lesen  aus der ANTWORT, aus nichts sonst
//! 4. Erlaubnis          steht der Name in der Positivliste?
//! 5. Argumentform       passt es zum erklaerten Schema?
//! 6. Betriebsart        darf ein Schritt dieser Stufe laufen?
//! 7. Ausfuehren         und erst jetzt
//! 8. In den Strom       samt allem, was abgelehnt wurde
//! ```
//!
//! ⚑ **Die Betriebsart steht auf Stufe 6 und nicht auf Stufe 2.** Der
//! Modellaufruf selbst benutzt kein Werkzeug; seine Stufe ist erst
//! bestimmt, wenn feststeht, **welches** Werkzeug laufen soll. Sie davor
//! zu prüfen hiesse, sie zu erraten.
//!
//! ⚑ **Und sie steht vor Stufe 7 und nicht danach.** Ein Segment, das
//! niemand nachrechnen kann, hinterher auszuweisen, ist zu spät.
//!
//! # ⚑ Was der Lauf nicht tut
//!
//! **Er filtert nichts.** Kein Schritt sieht sich an, ob ein Text
//! verdächtig aussieht. Jede Ablehnung folgt aus einer Liste oder einer
//! erklärten Form, und jede steht im Strom.

use myl_agent::registratur::{Benutzt, Registratur, Segmentstufe};
use myl_types::hash::Hash;

use crate::ausfuehrung::Werkzeugkasten;
use crate::betrieb::Betriebsart;
use crate::strom::{Entscheidung, Sitzungsstrom};
use crate::tuerklient::{Modellweg, Nachricht, Tuerfehler};
use crate::vollmacht_grenzen::{Grenzfehler, Sitzungsgrenzen};
use crate::werkzeug::{angebot, argumente_pruefen, vorschlaege, Erlaubnis, Werkzeugergebnis};

/// Was ein Lauf braucht.
pub struct Lauf<'a> {
    /// Woher die Modellantwort kommt: die Tuer eines Knotens oder ein
    /// Modell auf derselben Maschine.
    ///
    /// ⚑ **Seit dem 2026-09-08 ein Merkmal statt eines Klienten**, damit
    /// derselbe Agent mit und ohne Netz laeuft (CLIENT 0.2).
    pub klient: &'a dyn Modellweg,
    /// Welches Modell.
    pub modell: &'a str,
    /// Die Grenzen aus dem Sitzungskontrakt.
    pub grenzen: &'a Sitzungsgrenzen,
    /// Was der Nutzer zulässt.
    pub betriebsart: Betriebsart,
    /// Worauf die Werkzeuge zugreifen durften, fuer das Protokoll.
    ///
    /// ⚑ **`None` heisst: kein Dateizugriff**, nicht „unbekannt". Wer
    /// den Strom spaeter liest, soll den Unterschied sehen.
    pub einhaengung: Option<crate::strom::Einhaengungsmarke>,
    /// Die Werkzeuge samt Ausführung.
    pub kasten: &'a Werkzeugkasten,
    /// Wo die Manifeste stehen, für die Stufe je Schritt.
    pub registratur: &'a Registratur,
    /// Welche Manifestadresse zu welchem Werkzeugnamen gehört.
    ///
    /// ⚑ **Von aussen und nicht geraten.** Die Registratur kennt
    /// Adressen, das Modell kennt Namen, und die Zuordnung ist eine
    /// Angabe des Aufrufers. Wer sie hier erriete, könnte einem lokalen
    /// Werkzeug die Adresse eines verankerten zuordnen.
    pub adressen: &'a dyn Fn(&str) -> Option<myl_types::ids::MerkleRoot>,
    /// Der Anker der Kette.
    pub anker: Hash,
    /// Höchstzahl der Modellaufrufe, zusätzlich zur Schrittzahl.
    pub max_tokens: Option<u32>,
    /// In welcher Form die Werkzeuge angesagt werden.
    ///
    /// ⚑ **Die Vorgabe ist die amtliche Form**, also die des Modells
    /// selbst; siehe [`crate::werkzeug::Ansageform`] fuer die
    /// Herleitung und dafuer, warum die deutsche Fassung als Schalter
    /// stehen bleibt, bis die Agentenprobe gelaufen ist.
    pub ansageform: crate::werkzeug::Ansageform,
}

/// Warum ein Lauf endete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ende {
    /// Das Modell hat kein Werkzeug mehr vorgeschlagen.
    ///
    /// **Der normale Ausgang.**
    Fertig,
    /// Der Sitzungskontrakt lässt keine weiteren Schritte zu.
    Grenze(Grenzfehler),
    /// Die Tür hat nicht geantwortet.
    Tuer(Tuerfehler),
}

impl std::fmt::Display for Ende {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fertig => f.write_str("fertig"),
            Self::Grenze(g) => write!(f, "Grenze erreicht: {g}"),
            Self::Tuer(t) => write!(f, "die Tuer: {t}"),
        }
    }
}

/// Das Ergebnis eines Laufs.
pub struct Ergebnis {
    /// Der Beleg.
    pub strom: Sitzungsstrom,
    /// Die Unterhaltung, wie sie am Ende dastand.
    pub nachrichten: Vec<Nachricht>,
    /// Warum es aufhörte.
    pub ende: Ende,
}

impl<'a> Lauf<'a> {
    /// Fährt den Lauf.
    ///
    /// ⚑ **Der Strom entsteht dabei und wird auch bei einem Abbruch
    /// zurückgegeben.** Ein Lauf, der an einer Grenze endet, ist genau
    /// der interessante Fall; ein Beleg, den es nur bei Erfolg gibt, ist
    /// keiner.
    pub fn fahren(&self, auftrag: &str) -> Ergebnis {
        let mut strom = Sitzungsstrom::neu_mit_einhaengung(
            self.anker,
            self.betriebsart,
            self.einhaengung,
        );
        let erlaubnis = Erlaubnis::aus_angebot(self.kasten.angebote());
        let mut nachrichten =
            vec![angebot(self.kasten.angebote(), self.ansageform), Nachricht::nutzer(auftrag)];
        let mut getan: u32 = 0;

        let ende = loop {
            // 1. Schrittzahl.
            if let Err(g) = self.grenzen.schritt_erlaubt(getan) {
                break Ende::Grenze(g);
            }

            // 2. Das Modell fragen.
            let antwort =
                match self.klient.chat(self.modell, &nachrichten, self.max_tokens) {
                    Ok(a) => a,
                    Err(e) => break Ende::Tuer(e),
                };
            getan += 1;
            let gefragt = nachrichten.clone();
            nachrichten.push(Nachricht::modell(antwort.text.clone()));

            // 3. Vorschlaege, aus der ANTWORT und aus nichts sonst.
            let roh = vorschlaege(&antwort.text);
            let mut entschieden = Vec::new();
            let mut ergebnisse: Vec<String> = Vec::new();
            let mut benutzt: Vec<myl_types::ids::MerkleRoot> = Vec::new();

            for v in roh.into_iter().flatten() {
                // 4. Erlaubnis.
                if let Err(a) = erlaubnis.pruefen(&v) {
                    let text = a.to_string();
                    entschieden.push((v.clone(), Entscheidung::Abgelehnt(a)));
                    nachrichten.push(Werkzeugergebnis::nachricht(&v.name, &text));
                    ergebnisse.push(text);
                    continue;
                }
                // 5. Die Form der Argumente.
                let angebot = self.kasten.angebot(&v.name).expect("erlaubt heisst angeboten");
                if let Err(f) = argumente_pruefen(angebot, &v) {
                    let text = f.to_string();
                    entschieden.push((
                        v.clone(),
                        Entscheidung::Abgelehnt(crate::werkzeug::Abgelehnt {
                            name: v.name.clone(),
                            erlaubt: vec![format!("Form: {text}")],
                        }),
                    ));
                    nachrichten.push(Werkzeugergebnis::nachricht(&v.name, &text));
                    ergebnisse.push(text);
                    continue;
                }
                // 6. Die Betriebsart, VOR der Ausfuehrung.
                let adresse = (self.adressen)(&v.name);
                let stufe = match adresse {
                    Some(a) => self
                        .registratur
                        .stufe(&Benutzt { skills: Vec::new(), werkzeuge: vec![a] }),
                    // ⚑ Ohne Adresse ist die Herkunft unbekannt, und
                    // `Unbekannt` sperrt in JEDER Betriebsart.
                    None => Segmentstufe::Unbekannt { welche: Vec::new() },
                };
                if let Err(g) = self.betriebsart.pruefen(&stufe) {
                    let text = g.to_string();
                    entschieden.push((
                        v.clone(),
                        Entscheidung::Abgelehnt(crate::werkzeug::Abgelehnt {
                            name: v.name.clone(),
                            erlaubt: vec![format!("Betriebsart: {text}")],
                        }),
                    ));
                    nachrichten.push(Werkzeugergebnis::nachricht(&v.name, &text));
                    ergebnisse.push(text);
                    continue;
                }
                if let Some(a) = adresse {
                    benutzt.push(a);
                }

                // 7. Und erst jetzt ausfuehren.
                let text = match self.kasten.ausfuehren_ungeprueft(&v.name, &v.arguments) {
                    Some(Ok(t)) => t,
                    Some(Err(e)) => e.grund,
                    None => "das Werkzeug hat keine Ausfuehrung".to_string(),
                };
                entschieden.push((v.clone(), Entscheidung::Erlaubt));
                nachrichten.push(Werkzeugergebnis::nachricht(&v.name, &text));
                ergebnisse.push(text);
            }

            // 8. In den Strom, samt allem, was abgelehnt wurde.
            let stufe = self
                .registratur
                .stufe(&Benutzt { skills: Vec::new(), werkzeuge: benutzt });
            let fertig = entschieden.iter().all(|(_, e)| !e.erlaubt());
            strom.anhaengen(Sitzungsstrom::schritt_aus(
                &gefragt,
                &antwort,
                entschieden,
                &ergebnisse,
                stufe,
            ));

            // ⚑ **Fertig heisst: kein Werkzeug ist gelaufen.** Auch ein
            // Schritt, in dem alles abgelehnt wurde, endet den Lauf;
            // sonst liefe ein Modell, das immer wieder dasselbe
            // Verbotene vorschlaegt, bis die Schrittzahl aufgebraucht
            // ist, und der Nutzer bezahlte jede Runde.
            if fertig {
                break Ende::Fertig;
            }
        };

        Ergebnis { strom, nachrichten, ende }
    }
}
