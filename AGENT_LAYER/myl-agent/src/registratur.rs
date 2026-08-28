//! Was da ist, und wie viel ein Segment daraus wert ist.
//!
//! ## ⚑ Der tragende Satz: Ein Segment ist so nachrechenbar wie sein
//! schwächster Eingang
//!
//! [`crate::manifest`] hängt die Herkunftsstufe an den einzelnen Skill.
//! Ein Agentenschritt benutzt aber selten einen einzelnen: Er zieht
//! Wissen aus zwei Quellen, ruft ein Werkzeug, und was dabei
//! herauskommt, ist **ein** Segment mit **einer** Verifikationsstufe.
//!
//! **Diese Stufe ist das Minimum über alles Benutzte.** Ein verankerter
//! Skill neben einem lokalen ergibt ein Segment, das niemand nachrechnen
//! kann; der verankerte hilft dabei nichts. Das ist unbequem und es ist
//! richtig: Wer eine Kette prüft, prüft das schwächste Glied, nicht den
//! Durchschnitt.
//!
//! ⚑ **Und „unbekannt" ist nicht dasselbe wie „nur bezeugt".** Ein
//! Segment, das eine Adresse nennt, die hier niemand kennt, ist nicht
//! schwach belegt, sondern **gar nicht** belegt: Wir wissen nicht
//! einmal, was benutzt wurde. Das als „bezeugt" zu führen hieße zu
//! behaupten, man kenne den Eingang. Es bekommt deshalb einen eigenen
//! Zustand, und es ist der schlechteste.
//!
//! ## Die Adresse wird gerechnet, nicht geglaubt
//!
//! [`Registratur::nimm_skill`] legt unter der Adresse ab, die es
//! **selbst** aus dem Manifest rechnet. Nähme es eine mitgelieferte
//! Adresse, ließe sich ein lokaler Skill unter der Adresse eines
//! verankerten eintragen, und die ganze Stufenrechnung wäre wertlos.

use std::collections::BTreeMap;

use myl_types::hash::Hash;
use myl_types::ids::MerkleRoot;

use crate::manifest::{ManifestFehler, Skillmanifest, Werkzeugmanifest};

/// Was ein Segment an Skills und Werkzeugen benutzt hat.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Benutzt {
    /// Adressen der benutzten Skills.
    pub skills: Vec<MerkleRoot>,
    /// Adressen der benutzten Werkzeuge.
    pub werkzeuge: Vec<MerkleRoot>,
}

impl Benutzt {
    /// Nichts benutzt: reine Rechnung ohne Skill und ohne Werkzeug.
    pub fn leer() -> Self {
        Self::default()
    }
}

/// Wie viel ein Prüfer mit einem Segment anfangen kann.
///
/// **Drei Zustände und nicht zwei**, siehe den Modulkopf: „unbekannt"
/// ist keine schwache Form von „bezeugt", sondern das Fehlen jeder
/// Aussage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segmentstufe {
    /// Alles Benutzte ist nachrechenbar. Das Segment auch.
    Nachrechenbar,
    /// Mindestens ein Eingang ist nur bezeugt, mit den Adressen der
    /// Schuldigen — denn ein „nein" ohne Grund hilft niemandem.
    Bezeugt { wegen: Vec<MerkleRoot> },
    /// Mindestens eine benutzte Adresse ist hier unbekannt. **Der
    /// schlechteste Zustand**, weil nicht einmal feststeht, was benutzt
    /// wurde.
    Unbekannt { welche: Vec<MerkleRoot> },
}

impl Segmentstufe {
    /// Darf ein Prüfer dieses Segment nachrechnen?
    pub fn nachrechenbar(&self) -> bool {
        matches!(self, Self::Nachrechenbar)
    }

    /// Kurzform für Protokoll und Oberfläche.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Nachrechenbar => "nachrechenbar",
            Self::Bezeugt { .. } => "bezeugt",
            Self::Unbekannt { .. } => "unbekannt",
        }
    }
}

/// Was an Skills und Werkzeugen verfügbar ist.
///
/// **Geordnete Karten, damit die Ausgabe deterministisch ist.** Die
/// Listen in [`Segmentstufe`] wandern in die Spur, und eine Spur, deren
/// Reihenfolge von einer Hash-Karte abhängt, wäre zwischen zwei ehrlichen
/// Knoten verschieden.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registratur {
    skills: BTreeMap<MerkleRoot, Skillmanifest>,
    werkzeuge: BTreeMap<MerkleRoot, Werkzeugmanifest>,
}

impl Registratur {
    /// Leer.
    pub fn neu() -> Self {
        Self::default()
    }

    /// Nimmt einen Skill auf und gibt seine Adresse zurück.
    ///
    /// ⚑ **Die Adresse wird hier gerechnet, nicht entgegengenommen.**
    /// Nähme diese Funktion eine mitgelieferte, ließe sich ein lokaler
    /// Skill unter der Adresse eines verankerten eintragen.
    ///
    /// Ein unvollständiges Manifest wird abgewiesen; ein Skill ohne
    /// Lizenz oder ohne Revision kommt nicht herein (ETHICS G7).
    pub fn nimm_skill(&mut self, m: Skillmanifest) -> Result<MerkleRoot, ManifestFehler> {
        m.pruefe_vollstaendig()?;
        let adresse = m.wurzel();
        self.skills.insert(adresse, m);
        Ok(adresse)
    }

    /// Nimmt ein Werkzeug auf und gibt seine Adresse zurück.
    pub fn nimm_werkzeug(&mut self, m: Werkzeugmanifest) -> Result<MerkleRoot, ManifestFehler> {
        m.pruefe_vollstaendig()?;
        let adresse = m.wurzel();
        self.werkzeuge.insert(adresse, m);
        Ok(adresse)
    }

    /// Der Skill zu einer Adresse.
    pub fn skill(&self, adresse: &MerkleRoot) -> Option<&Skillmanifest> {
        self.skills.get(adresse)
    }

    /// Das Werkzeug zu einer Adresse.
    pub fn werkzeug(&self, adresse: &MerkleRoot) -> Option<&Werkzeugmanifest> {
        self.werkzeuge.get(adresse)
    }

    /// Wie viele Skills und Werkzeuge zusammen.
    pub fn anzahl(&self) -> usize {
        self.skills.len() + self.werkzeuge.len()
    }

    /// ⚑ **Die Verifikationsstufe eines Segments: das Minimum über alles
    /// Benutzte.**
    ///
    /// Ein verankerter Skill neben einem lokalen ergibt ein Segment, das
    /// niemand nachrechnen kann. Das ist unbequem und richtig: Wer eine
    /// Kette prüft, prüft das schwächste Glied.
    ///
    /// **Unbekanntes schlägt alles.** Steht auch nur eine Adresse nicht
    /// in dieser Registratur, ist das Ergebnis
    /// [`Segmentstufe::Unbekannt`] — nicht „bezeugt", denn wir wissen
    /// nicht, was benutzt wurde, und dürfen es nicht so aussehen lassen.
    pub fn stufe(&self, benutzt: &Benutzt) -> Segmentstufe {
        let mut unbekannt = Vec::new();
        let mut bezeugt = Vec::new();

        for a in &benutzt.skills {
            match self.skills.get(a) {
                None => unbekannt.push(*a),
                Some(s) if !s.nachrechenbar() => bezeugt.push(*a),
                Some(_) => {}
            }
        }
        for a in &benutzt.werkzeuge {
            match self.werkzeuge.get(a) {
                None => unbekannt.push(*a),
                Some(w) if !w.nachrechenbar() => bezeugt.push(*a),
                Some(_) => {}
            }
        }

        // Sortiert, damit die Spur nicht von der Aufrufreihenfolge
        // abhängt. Zwei ehrliche Knoten müssen dieselbe Liste schreiben.
        unbekannt.sort();
        unbekannt.dedup();
        bezeugt.sort();
        bezeugt.dedup();

        if !unbekannt.is_empty() {
            return Segmentstufe::Unbekannt { welche: unbekannt };
        }
        if !bezeugt.is_empty() {
            return Segmentstufe::Bezeugt { wegen: bezeugt };
        }
        Segmentstufe::Nachrechenbar
    }
}

/// Ein Werkzeugaufruf, wie er in die Spur geht.
///
/// **Nur Hashes.** Die Bytes von Ein- und Ausgabe liegen woanders; die
/// Spur trägt, was zum Nachrechnen und Vergleichen nötig ist, und nicht
/// den Inhalt.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Aufruf {
    /// Adresse des Werkzeugs in der Registratur.
    pub werkzeug: MerkleRoot,
    /// Hash über die Eingabe.
    pub eingabe: Hash,
    /// Hash über die Ausgabe.
    pub ausgabe: Hash,
}

/// Was ein Prüfer über eine Folge von Werkzeugaufrufen sagen kann.
///
/// ⚑ **Die Reihenfolge ist nach Schwere sortiert, und `Widerspruch`
/// steht oben.** Ein Widerspruch ist ein **Beleg für einen Defekt**;
/// „unbekannt" ist nur das Fehlen von Wissen. Beides macht ein Segment
/// unprüfbar, aber nur eines davon ist ein Fund, auf den jemand
/// reagieren kann.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aufrufbefund {
    /// ⚑ Ein **deterministisches** Werkzeug lieferte auf dieselbe
    /// Eingabe zwei verschiedene Ausgaben. Das widerspricht seiner
    /// eigenen Zusage.
    Widerspruch {
        werkzeug: MerkleRoot,
        eingabe: Hash,
        ausgaben: Vec<Hash>,
    },
    /// Ein aufgerufenes Werkzeug steht nicht in der Registratur.
    Unbekannt { welche: Vec<MerkleRoot> },
    /// Mindestens ein Aufruf ging an ein Werkzeug, das nur bezeugt.
    Bezeugt { wegen: Vec<MerkleRoot> },
    /// Alle Aufrufe gingen an nachrechenbare Werkzeuge und
    /// widersprechen sich nicht.
    Nachrechenbar,
}

impl Aufrufbefund {
    /// Darf ein Prüfer diese Aufruffolge nachrechnen?
    pub fn nachrechenbar(&self) -> bool {
        matches!(self, Self::Nachrechenbar)
    }

    /// Kurzform für Protokoll und Oberfläche.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Widerspruch { .. } => "widersprüchlich",
            Self::Unbekannt { .. } => "unbekannt",
            Self::Bezeugt { .. } => "bezeugt",
            Self::Nachrechenbar => "nachrechenbar",
        }
    }
}

impl Registratur {
    /// Prüft eine Folge von Werkzeugaufrufen.
    ///
    /// ## ⚑ Die Prüfung, die es geschenkt gibt
    ///
    /// Ein deterministisches Werkzeug sagt zu: **gleiche Eingabe,
    /// gleiche Ausgabe**. Kommt dieselbe Paarung aus Werkzeug und
    /// Eingabe in einer Spur zweimal mit **verschiedenen** Ausgaben
    /// vor, ist das ein Widerspruch, den man **ohne jede Ausführung**
    /// sieht. Entweder ist das Werkzeug nicht deterministisch, oder
    /// jemand hat eine Ausgabe erfunden; beides ist ein Fund.
    ///
    /// Das ist derselbe Gedanke wie der Redundanzvergleich eine Ebene
    /// höher: **zwei Aussagen über dieselbe Rechnung gegeneinander
    /// halten**, statt die Rechnung zu wiederholen.
    ///
    /// ⚑ **Und sie ist gelegentlich, nicht vollständig.** Sie kann nur
    /// zuschlagen, wenn dieselbe Paarung wirklich zweimal vorkommt.
    /// Kommt sie einmal vor, schweigt die Prüfung, und das heißt
    /// **nicht**, dass der Aufruf stimmt. Eine Prüfung, die manchmal
    /// etwas findet, ist wertvoll; eine, die man für vollständig hält,
    /// ist gefährlich.
    pub fn pruefe_aufrufe(&self, aufrufe: &[Aufruf]) -> Aufrufbefund {
        use std::collections::BTreeMap;

        let mut unbekannt = Vec::new();
        let mut bezeugt = Vec::new();
        // Je (Werkzeug, Eingabe) die gesehenen Ausgaben.
        let mut gesehen: BTreeMap<(MerkleRoot, Hash), Vec<Hash>> = BTreeMap::new();

        for a in aufrufe {
            match self.werkzeuge.get(&a.werkzeug) {
                None => {
                    unbekannt.push(a.werkzeug);
                    continue;
                }
                Some(w) if !w.nachrechenbar() => {
                    bezeugt.push(a.werkzeug);
                    // Ein bezeugtes Werkzeug darf verschiedene Ausgaben
                    // liefern; das ist der Sinn von „extern".
                    continue;
                }
                Some(_) => {}
            }
            let eintrag = gesehen.entry((a.werkzeug, a.eingabe)).or_default();
            if !eintrag.contains(&a.ausgabe) {
                eintrag.push(a.ausgabe);
            }
        }

        // Der Widerspruch zuerst: Er ist ein Beleg, nicht eine Lücke.
        for ((werkzeug, eingabe), mut ausgaben) in gesehen {
            if ausgaben.len() > 1 {
                ausgaben.sort();
                return Aufrufbefund::Widerspruch {
                    werkzeug,
                    eingabe,
                    ausgaben,
                };
            }
        }

        unbekannt.sort();
        unbekannt.dedup();
        if !unbekannt.is_empty() {
            return Aufrufbefund::Unbekannt { welche: unbekannt };
        }
        bezeugt.sort();
        bezeugt.dedup();
        if !bezeugt.is_empty() {
            return Aufrufbefund::Bezeugt { wegen: bezeugt };
        }
        Aufrufbefund::Nachrechenbar
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Herkunft, Teil, Werkzeugart};

    fn skill(name: &str, h: Herkunft) -> Skillmanifest {
        Skillmanifest {
            name: name.into(),
            quelle: "Sammlung".into(),
            revision: "1".into(),
            lizenz: "MIT".into(),
            teile: vec![Teil {
                fundstelle: "1".into(),
                inhalt: Hash::sha256(name.as_bytes()),
            }],
            herkunft: h,
        }
    }

    fn werkzeug(name: &str, art: Werkzeugart, h: Herkunft) -> Werkzeugmanifest {
        Werkzeugmanifest {
            name: name.into(),
            anbieter: "Myelith".into(),
            revision: "1".into(),
            lizenz: "MIT".into(),
            art,
            herkunft: h,
        }
    }

    /// ⚑ **Der tragende Satz: ein lokaler Skill zieht das ganze Segment
    /// herunter.**
    ///
    /// Zwei verankerte und ein lokaler ergeben ein Segment, das niemand
    /// nachrechnen kann. Die beiden verankerten helfen nichts. Wer eine
    /// Kette prüft, prüft das schwächste Glied und nicht den
    /// Durchschnitt.
    #[test]
    fn ein_lokaler_skill_zieht_das_ganze_segment_herunter() {
        let mut r = Registratur::neu();
        let a = r.nimm_skill(skill("a", Herkunft::Verankert)).expect("a");
        let b = r.nimm_skill(skill("b", Herkunft::Bibliothek)).expect("b");
        let c = r.nimm_skill(skill("c", Herkunft::Lokal)).expect("c");

        // Ohne den lokalen ist das Segment nachrechenbar.
        let ohne = Benutzt {
            skills: vec![a, b],
            werkzeuge: vec![],
        };
        assert_eq!(r.stufe(&ohne), Segmentstufe::Nachrechenbar);

        // Mit ihm nicht mehr, und die Meldung nennt ihn.
        let mit = Benutzt {
            skills: vec![a, b, c],
            werkzeuge: vec![],
        };
        assert_eq!(r.stufe(&mit), Segmentstufe::Bezeugt { wegen: vec![c] });
    }

    /// ⚑ **Unbekannt ist nicht dasselbe wie bezeugt, und es schlägt
    /// alles.**
    ///
    /// Eine Adresse, die hier niemand kennt, heißt nicht „schwach
    /// belegt", sondern „wir wissen nicht, was benutzt wurde". Das als
    /// bezeugt zu führen hieße zu behaupten, man kenne den Eingang.
    #[test]
    fn unbekannt_schlaegt_bezeugt() {
        let mut r = Registratur::neu();
        let lokal = r.nimm_skill(skill("l", Herkunft::Lokal)).expect("l");
        let fremd = skill("nie eingetragen", Herkunft::Verankert).wurzel();

        // Nur der lokale: bezeugt.
        assert!(matches!(
            r.stufe(&Benutzt { skills: vec![lokal], werkzeuge: vec![] }),
            Segmentstufe::Bezeugt { .. }
        ));
        // Mit dem unbekannten: unbekannt, obwohl der lokale auch dabei ist.
        assert_eq!(
            r.stufe(&Benutzt {
                skills: vec![lokal, fremd],
                werkzeuge: vec![]
            }),
            Segmentstufe::Unbekannt { welche: vec![fremd] }
        );
    }

    /// ⚑ **Die Adresse wird gerechnet, nicht geglaubt.**
    ///
    /// Zwei Skills mit gleichem Inhalt und verschiedener Herkunft
    /// bekommen verschiedene Adressen und liegen nebeneinander. Ohne das
    /// ließe sich ein lokaler Skill unter der Adresse eines verankerten
    /// eintragen und die ganze Stufenrechnung wäre wertlos.
    #[test]
    fn gleicher_inhalt_und_andere_herkunft_liegen_nebeneinander() {
        let mut r = Registratur::neu();
        let v = r.nimm_skill(skill("gleich", Herkunft::Verankert)).expect("v");
        let l = r.nimm_skill(skill("gleich", Herkunft::Lokal)).expect("l");
        assert_ne!(v, l, "die Adressen fallen zusammen");
        assert_eq!(r.anzahl(), 2, "einer hat den anderen überschrieben");
        assert_eq!(r.skill(&v).expect("v").herkunft, Herkunft::Verankert);
        assert_eq!(r.skill(&l).expect("l").herkunft, Herkunft::Lokal);
    }

    /// Ein Werkzeug zieht ebenso herunter, und zwar aus beiden Gründen:
    /// weil es extern ist oder weil es lokal ist.
    #[test]
    fn auch_werkzeuge_ziehen_herunter_und_zwar_aus_beiden_gruenden() {
        let mut r = Registratur::neu();
        let gut = r
            .nimm_werkzeug(werkzeug("rechnen", Werkzeugart::Deterministisch, Herkunft::Verankert))
            .expect("gut");
        let extern_ = r
            .nimm_werkzeug(werkzeug("abrufen", Werkzeugart::Extern, Herkunft::Bibliothek))
            .expect("extern");
        let lokal = r
            .nimm_werkzeug(werkzeug("eigenes", Werkzeugart::Deterministisch, Herkunft::Lokal))
            .expect("lokal");

        assert_eq!(
            r.stufe(&Benutzt { skills: vec![], werkzeuge: vec![gut] }),
            Segmentstufe::Nachrechenbar
        );
        for (name, a) in [("extern", extern_), ("lokal", lokal)] {
            assert!(
                matches!(
                    r.stufe(&Benutzt { skills: vec![], werkzeuge: vec![gut, a] }),
                    Segmentstufe::Bezeugt { .. }
                ),
                "{name} zog das Segment nicht herunter"
            );
        }
    }

    /// Ohne Eingänge ist ein Segment nachrechenbar. Eine reine Rechnung
    /// braucht keinen Skill, und das darf nicht als Mangel zählen.
    #[test]
    fn ohne_eingaenge_ist_ein_segment_nachrechenbar() {
        let r = Registratur::neu();
        assert_eq!(r.stufe(&Benutzt::leer()), Segmentstufe::Nachrechenbar);
    }

    /// ⚑ Die Liste in der Meldung ist **deterministisch**.
    ///
    /// Sie wandert in die Spur, und zwei ehrliche Knoten müssen dieselbe
    /// schreiben. Ohne Sortierung hinge sie an der Aufrufreihenfolge,
    /// und der Redundanzvergleich meldete zwei ehrliche Pods als
    /// abweichend — dieselbe Klasse wie das verbotene Token-Dropping.
    #[test]
    fn die_meldung_haengt_nicht_an_der_reihenfolge() {
        let mut r = Registratur::neu();
        let x = r.nimm_skill(skill("x", Herkunft::Lokal)).expect("x");
        let y = r.nimm_skill(skill("y", Herkunft::Lokal)).expect("y");

        let vorwaerts = r.stufe(&Benutzt { skills: vec![x, y], werkzeuge: vec![] });
        let rueckwaerts = r.stufe(&Benutzt { skills: vec![y, x], werkzeuge: vec![] });
        assert_eq!(vorwaerts, rueckwaerts);
        // Und doppelt genannt zählt einmal.
        let doppelt = r.stufe(&Benutzt { skills: vec![x, x, y], werkzeuge: vec![] });
        assert_eq!(doppelt, vorwaerts);
    }

    // ---- Werkzeugaufrufe ---------------------------------------------

    fn aufruf(w: MerkleRoot, ein: &str, aus: &str) -> Aufruf {
        Aufruf {
            werkzeug: w,
            eingabe: Hash::sha256(ein.as_bytes()),
            ausgabe: Hash::sha256(aus.as_bytes()),
        }
    }

    /// ⚑ **Die Prüfung, die es geschenkt gibt: derselbe deterministische
    /// Aufruf mit zwei Ausgaben ist ein Widerspruch.**
    ///
    /// Sichtbar **ohne jede Ausführung**, allein durch Vergleich. Das ist
    /// derselbe Gedanke wie der Redundanzvergleich eine Ebene höher.
    #[test]
    fn zwei_ausgaben_auf_dieselbe_eingabe_sind_ein_widerspruch() {
        let mut r = Registratur::neu();
        let w = r
            .nimm_werkzeug(werkzeug("rechnen", Werkzeugart::Deterministisch, Herkunft::Verankert))
            .expect("w");

        let stimmig = vec![aufruf(w, "2+2", "4"), aufruf(w, "3+3", "6")];
        assert_eq!(r.pruefe_aufrufe(&stimmig), Aufrufbefund::Nachrechenbar);

        // Dieselbe Eingabe, zwei Ausgaben.
        let kaputt = vec![aufruf(w, "2+2", "4"), aufruf(w, "2+2", "5")];
        match r.pruefe_aufrufe(&kaputt) {
            Aufrufbefund::Widerspruch {
                werkzeug: got,
                ausgaben,
                ..
            } => {
                assert_eq!(got, w);
                assert_eq!(ausgaben.len(), 2, "eine Ausgabe ist verschwunden");
            }
            anderes => panic!("erwartet Widerspruch, bekommen {anderes:?}"),
        }
    }

    /// ⚑ **Ein externes Werkzeug darf sich widersprechen.**
    ///
    /// Zwei Abrufe derselben Adresse liefern verschiedene Antworten, und
    /// das ist der Sinn von „extern", nicht ein Fehler. Ohne diese
    /// Ausnahme meldete die Prüfung jeden Wetterbericht als Defekt.
    #[test]
    fn ein_externes_werkzeug_darf_sich_widersprechen() {
        let mut r = Registratur::neu();
        let w = r
            .nimm_werkzeug(werkzeug("abrufen", Werkzeugart::Extern, Herkunft::Bibliothek))
            .expect("w");
        let zweimal = vec![aufruf(w, "kurs", "1,08"), aufruf(w, "kurs", "1,09")];
        assert!(
            matches!(r.pruefe_aufrufe(&zweimal), Aufrufbefund::Bezeugt { .. }),
            "ein externes Werkzeug wurde als widersprüchlich gemeldet"
        );
    }

    /// Der Widerspruch schlägt alles: Er ist ein Beleg, während
    /// „unbekannt" nur eine Lücke ist.
    #[test]
    fn der_widerspruch_schlaegt_das_unbekannte() {
        let mut r = Registratur::neu();
        let w = r
            .nimm_werkzeug(werkzeug("rechnen", Werkzeugart::Deterministisch, Herkunft::Verankert))
            .expect("w");
        let fremd = werkzeug("nie", Werkzeugart::Deterministisch, Herkunft::Verankert).wurzel();
        let beides = vec![
            aufruf(w, "x", "1"),
            aufruf(w, "x", "2"),
            aufruf(fremd, "y", "3"),
        ];
        assert!(matches!(
            r.pruefe_aufrufe(&beides),
            Aufrufbefund::Widerspruch { .. }
        ));
    }

    /// ⚑ Die Prüfung ist **gelegentlich**, nicht vollständig.
    ///
    /// Kommt eine Paarung nur einmal vor, schweigt sie, und das heißt
    /// **nicht**, dass der Aufruf stimmt. Der Test hält das fest, damit
    /// niemand die Prüfung für einen Beweis hält.
    #[test]
    fn ein_einzelner_aufruf_wird_nicht_geprueft_und_das_ist_kein_beleg() {
        let mut r = Registratur::neu();
        let w = r
            .nimm_werkzeug(werkzeug("rechnen", Werkzeugart::Deterministisch, Herkunft::Verankert))
            .expect("w");
        // Eine glatt erfundene Ausgabe fällt nicht auf, solange sie
        // allein steht.
        let erfunden = vec![aufruf(w, "2+2", "42")];
        assert_eq!(r.pruefe_aufrufe(&erfunden), Aufrufbefund::Nachrechenbar);
    }

    /// Ein unbekanntes Werkzeug macht die Folge unprüfbar.
    #[test]
    fn ein_unbekanntes_werkzeug_macht_die_folge_unpruefbar() {
        let r = Registratur::neu();
        let fremd = werkzeug("nie", Werkzeugart::Deterministisch, Herkunft::Verankert).wurzel();
        assert_eq!(
            r.pruefe_aufrufe(&[aufruf(fremd, "x", "y")]),
            Aufrufbefund::Unbekannt { welche: vec![fremd] }
        );
    }

    /// Keine Aufrufe heißt nachrechenbar: Eine reine Rechnung ohne
    /// Werkzeug ist kein Mangel.
    #[test]
    fn ohne_aufrufe_ist_die_folge_nachrechenbar() {
        let r = Registratur::neu();
        assert_eq!(r.pruefe_aufrufe(&[]), Aufrufbefund::Nachrechenbar);
    }

    /// Ein lokales deterministisches Werkzeug zieht herunter, obwohl es
    /// deterministisch ist: Nachrechnen kann es trotzdem niemand.
    #[test]
    fn ein_lokales_deterministisches_werkzeug_zieht_herunter() {
        let mut r = Registratur::neu();
        let w = r
            .nimm_werkzeug(werkzeug("eigenes", Werkzeugart::Deterministisch, Herkunft::Lokal))
            .expect("w");
        assert!(matches!(
            r.pruefe_aufrufe(&[aufruf(w, "x", "y")]),
            Aufrufbefund::Bezeugt { .. }
        ));
    }

    /// Ein unvollständiges Manifest kommt nicht herein. Sonst stünde in
    /// der Registratur ein Skill ohne Lizenz, und ETHICS G7 wäre eine
    /// Absichtserklärung statt einer Prüfung.
    #[test]
    fn ein_unvollstaendiges_manifest_kommt_nicht_herein() {
        let mut r = Registratur::neu();
        let mut ohne_lizenz = skill("x", Herkunft::Verankert);
        ohne_lizenz.lizenz = "  ".into();
        assert!(r.nimm_skill(ohne_lizenz).is_err());
        assert_eq!(r.anzahl(), 0, "es wurde trotzdem etwas eingetragen");

        // Und die verbotene Paarung aus dem Manifest gilt auch hier.
        assert!(r
            .nimm_werkzeug(werkzeug("x", Werkzeugart::Extern, Herkunft::Verankert))
            .is_err());
        assert_eq!(r.anzahl(), 0);
    }
}
