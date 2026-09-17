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
use crate::werkzeug::{
    angebot_mit_regel, argumente_pruefen, vorschlaege, Erlaubnis, Werkzeugergebnis,
};

/// Was ein Lauf braucht.
pub struct Lauf<'a> {
    /// Woher die Modellantwort kommt: die Tuer eines Knotens oder ein
    /// Modell auf derselben Maschine.
    ///
    /// ⚑ **Seit dem 2026-09-08 ein Merkmal statt eines Klienten**, damit
    /// derselbe Agent mit und ohne Netz laeuft (CLIENT 0.2).
    pub klient: &'a dyn Modellweg,
    /// Eine Hausregel, die hinter der Werkzeugansage steht.
    ///
    /// ⚑ **`None` ist die Vorgabe und der Normalfall.** Sie ist
    /// gemessen worden und nicht geraten: Ohne sie ruft ein 0,6B in 27
    /// Laeufen kein einziges Werkzeug. ⛔️ **Im Netz muss sie fuer alle
    /// Knoten dieselbe sein**, sonst ruesten zwei Knoten am selben
    /// Auftrag verschieden.
    pub hausregel: Option<&'a str>,
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
    /// Wer zusehen will, waehrend es geschieht.
    ///
    /// # ⚑ Warum das eine Meldung ist und keine Rueckfrage
    ///
    /// **Der Melder bekommt zu sehen und entscheidet nichts.** Er gibt
    /// nichts zurueck, und er wird an Stellen gerufen, an denen die
    /// Entscheidung schon gefallen ist. Ein Haken, der den Lauf
    /// beeinflussen koennte, waere eine zweite Quelle fuer Erlaubnisse
    /// neben [`Erlaubnis`] und [`Betriebsart`], und genau die darf es
    /// nicht geben: Diese Kiste traegt eine Vollmacht.
    ///
    /// 📌 **`None` heisst: niemand sieht zu**, und der Lauf ist dann
    /// Zeichen fuer Zeichen derselbe. `ein_melder_aendert_den_lauf_nicht`
    /// haelt das fest.
    pub melder: Option<&'a dyn Fn(Meldung<'_>)>,
}

/// Was waehrend eines Laufs geschieht, waehrend es geschieht.
///
/// ⚑ **Fuer eine Anzeige und nicht fuer ein Protokoll.** Das Protokoll
/// ist der Sitzungsstrom; er ist vollstaendig, kommt am Ende und ist
/// die Grundlage jeder Nachrechnung. Diese Meldungen sind der Blick
/// waehrenddessen, und deshalb tragen sie geliehene Zeichenketten:
/// Wer sie behalten will, kopiert sie selbst.
#[derive(Debug, Clone, Copy)]
pub enum Meldung<'a> {
    /// Das Modell wird zum `n`-ten Mal gefragt.
    Schritt(u32),
    /// Ein Werkzeug wird jetzt ausgefuehrt.
    Aufruf {
        name: &'a str,
        argumente: &'a serde_json::Value,
    },
    /// Was es zurueckgab.
    Ergebnis { name: &'a str, text: &'a str },
    /// Ein Vorschlag wurde abgewiesen, mit dem Grund.
    ///
    /// ⚑ **Gehoert in die Anzeige.** Ein Agent, dessen Werkzeug
    /// abgelehnt wurde, sieht fuer den Nutzer aus wie einer, der nichts
    /// tut; der Grund steht sonst nur im Strom.
    Abgelehnt { name: &'a str, grund: &'a str },
    /// **Der Verlauf passte nicht mehr in den Kontext und wurde
    /// verdichtet**, mit den belegten Token davor und danach.
    ///
    /// ⚑ **Gehoert in die Anzeige**, denn der Agent weiss danach weniger
    /// woertlich als davor, und das soll niemand erst an einer Antwort
    /// merken.
    Verdichtet { vorher: usize, nachher: usize },
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
        self.fahren_mit_verlauf(auftrag, &[])
    }

    /// **Wie [`Lauf::fahren`], mit dem bisherigen Gespraech vor dem
    /// Auftrag.**
    ///
    /// # ⚑ Entscheidung C2, beantwortet am 2026-09-14
    ///
    /// Bis hierher stand jeder Auftrag fuer sich. Die Sorge dahinter: Ein
    /// Gespraech, das ueber Auftraege weiterlaeuft und den Schrittzaehler
    /// jedes Mal zuruecksetzt, macht aus der Obergrenze eine Empfehlung.
    ///
    /// **Das trifft nicht zu, solange jeder Auftrag ein eigener Lauf
    /// bleibt**, und genau so ist es gebaut: Schrittbudget und Belegkette
    /// beginnen hier neu, wie vorher. Die Obergrenze schuetzt vor einem
    /// Agenten, der **innerhalb** eines Auftrags nicht aufhoert; einen
    /// neuen Auftrag gibt nur der Mensch, und das ist eine neue Freigabe.
    /// Der Verlauf ist **Eingabe** dieses Laufs: Das Commitment des ersten
    /// Schritts ueber die Nachrichten an das Modell bindet ihn mit, wie jede
    /// andere Nachricht.
    ///
    /// ⚠️ **Systemnachrichten im Verlauf werden nicht uebernommen.** Die
    /// Werkzeugansage gehoert diesem Lauf und steht vorn; eine alte aus dem
    /// Verlauf versprache Werkzeuge, die es jetzt vielleicht nicht gibt.
    pub fn fahren_mit_verlauf(&self, auftrag: &str, verlauf: &[Nachricht]) -> Ergebnis {
        let mut strom = Sitzungsstrom::neu_mit_einhaengung(
            self.anker,
            self.betriebsart,
            self.einhaengung,
        );
        let erlaubnis = Erlaubnis::aus_angebot(self.kasten.angebote());
        let mut nachrichten =
            vec![angebot_mit_regel(self.kasten.angebote(), self.ansageform, self.hausregel)];
        nachrichten.extend(verlauf.iter().filter(|n| n.role != "system").cloned());
        // Wo der Auftrag steht; das Verdichten laesst ihn woertlich stehen.
        let mut auftrag_bei = nachrichten.len();
        nachrichten.push(Nachricht::nutzer(auftrag));
        let mut getan: u32 = 0;

        // ⚑ Der Melder wird ueber eine Hilfe gerufen und nicht an
        // jeder Stelle ausgepackt: Ein `if let Some(...)` je Meldung
        // waere fuenfmal dieselbe Zeile.
        let melden = |m: Meldung<'_>| {
            if let Some(f) = self.melder {
                f(m);
            }
        };

        let ende = loop {
            // 1. Schrittzahl.
            if let Err(g) = self.grenzen.schritt_erlaubt(getan) {
                break Ende::Grenze(g);
            }
            melden(Meldung::Schritt(getan + 1));

            // 1b. Passt der naechste Schritt noch in den Kontext?
            //
            // ⚑ **Hier und nicht erst am Fehler der Tuer**: Eine Antwort,
            // die an der Kontextgrenze abbricht, ist ein halber
            // Werkzeugaufruf. Freigehalten wird deshalb die ganze
            // Antwortlaenge.
            if let Err(e) = self.platz_schaffen(&mut nachrichten, &mut auftrag_bei, &melden) {
                break Ende::Tuer(e);
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
                    melden(Meldung::Abgelehnt { name: &v.name, grund: &text });
                    entschieden.push((v.clone(), Entscheidung::Abgelehnt(a)));
                    nachrichten.push(Werkzeugergebnis::nachricht(&v.name, &text));
                    ergebnisse.push(text);
                    continue;
                }
                // 5. Die Form der Argumente.
                let angebot = self.kasten.angebot(&v.name).expect("erlaubt heisst angeboten");
                if let Err(f) = argumente_pruefen(angebot, &v) {
                    let text = f.to_string();
                    melden(Meldung::Abgelehnt { name: &v.name, grund: &text });
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
                    melden(Meldung::Abgelehnt { name: &v.name, grund: &text });
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
                //
                // ⚑ **Gemeldet wird davor und danach**, und beides ist
                // noetig: Ein Werkzeug, das eine Datei durchsucht,
                // laeuft merklich lange, und wer nur das Ergebnis
                // meldet, zeigt in dieser Zeit ein Fenster, das
                // stillsteht.
                melden(Meldung::Aufruf { name: &v.name, argumente: &v.arguments });
                let text = match self.kasten.ausfuehren_ungeprueft(&v.name, &v.arguments) {
                    Some(Ok(t)) => t,
                    Some(Err(e)) => e.grund,
                    None => "das Werkzeug hat keine Ausfuehrung".to_string(),
                };
                melden(Meldung::Ergebnis { name: &v.name, text: &text });
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

    /// **Verdichtet den Verlauf, wenn der naechste Schritt nicht mehr
    /// hineinpasst.** Woertlich bleiben die Werkzeugansage, der Auftrag und
    /// der letzte Schritt; alles andere wird zusammengefasst, und die
    /// Zusammenfassung steht vor dem Auftrag.
    ///
    /// Ohne Auskunft ueber den Kontext (siehe [`Modellweg::kontext`])
    /// geschieht nichts.
    fn platz_schaffen(
        &self,
        nachrichten: &mut Vec<Nachricht>,
        auftrag_bei: &mut usize,
        melden: &dyn Fn(Meldung<'_>),
    ) -> Result<(), Tuerfehler> {
        let reserve = self.max_tokens.unwrap_or(0) as usize;
        let Some(vorher) = self.klient.kontext(nachrichten) else {
            return Ok(());
        };
        if vorher.passt(reserve) {
            return Ok(());
        }
        // Der letzte Schritt: ab der letzten Antwort des Modells, sofern sie
        // hinter dem Auftrag steht.
        let letzter = nachrichten
            .iter()
            .rposition(|n| n.role == "assistant")
            .filter(|&i| i > *auftrag_bei)
            .unwrap_or(nachrichten.len());
        let mitte: Vec<Nachricht> = nachrichten[1..*auftrag_bei]
            .iter()
            .chain(&nachrichten[*auftrag_bei + 1..letzter])
            .cloned()
            .collect();
        if !mitte.is_empty() {
            let text = crate::verdichtung::zusammenfassen(
                self.klient,
                self.modell,
                &mitte,
                crate::verdichtung::antwortlaenge(vorher.grenze),
            )?;
            let mut neu = vec![nachrichten[0].clone(), crate::verdichtung::als_nachricht(&text)];
            neu.push(nachrichten[*auftrag_bei].clone());
            neu.extend(nachrichten[letzter..].iter().cloned());
            *nachrichten = neu;
            *auftrag_bei = 2;
        }
        // 📌 **Passt der letzte Schritt allein nicht**, etwa weil ein
        // Werkzeug eine sehr lange Datei zurueckgab, wird seine laengste
        // Werkzeugantwort in der Mitte gekuerzt. Sehen koennte das Modell
        // sie ohnehin nicht, und ohne Kuerzung endete der Lauf hier. Der
        // Auftrag und die Antworten des Modells bleiben woertlich.
        let mut nachher = self.klient.kontext(nachrichten).unwrap_or(vorher);
        if !nachher.passt(reserve) {
            if let Some(i) = (*auftrag_bei + 1..nachrichten.len())
                .filter(|&i| nachrichten[i].role != "assistant")
                .max_by_key(|&i| nachrichten[i].content.len())
            {
                let davor = nachrichten[..i].to_vec();
                let danach = nachrichten[i + 1..].to_vec();
                let gekuerzt = crate::verdichtung::kuerzen_bis_es_passt(self.klient, &nachrichten[i], reserve, |n| {
                    let mut alle = davor.clone();
                    alle.push(n.clone());
                    alle.extend(danach.iter().cloned());
                    alle
                })?;
                nachrichten[i] = gekuerzt;
                nachher = self.klient.kontext(nachrichten).unwrap_or(nachher);
            }
        }
        if nachher == vorher {
            return Err(Tuerfehler::KontextVoll { belegt: vorher.belegt, grenze: vorher.grenze });
        }
        melden(Meldung::Verdichtet { vorher: vorher.belegt, nachher: nachher.belegt });
        if nachher.passt(reserve) {
            Ok(())
        } else {
            Err(Tuerfehler::KontextVoll { belegt: nachher.belegt, grenze: nachher.grenze })
        }
    }
}
