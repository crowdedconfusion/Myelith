//! Der Plan: die architektonische Trennung, Whitepaper Kap. 8.3.
//!
//! Kap. 8.3 verlangt wörtlich, dass **abgerufene Daten den Kontrollfluss
//! nicht beeinflussen können**, und zwar strukturell statt filterbasiert.
//!
//! ## ⚑ Die Trennung liegt nicht dort, wo die Frage sie vermutete
//!
//! Die offene Design-Entscheidung bot zwei Antworten an: zwei
//! Modellinstanzen auf getrennten Pods, oder ein erzwungener
//! Kontextwechsel innerhalb einer Session. **Beide beschreiben, wo das
//! Modell läuft, und die Anforderung handelt nicht davon.** Sie ist eine
//! Aussage über Datenfluss.
//!
//! Zwei Pods geben davon nichts: Bekommt der planende Teil den
//! abgerufenen Text als Zeichenkette zugestellt, steht er in seinem
//! Kontext, gleich auf wie vielen Maschinen. Und ein „erzwungener
//! Kontextwechsel" ist eine Zusage über den Prompt-Bau, also genau die
//! filterbasierte Absicherung, die das Kapitel ausschließt.
//!
//! ⚑ **Was zwei Instanzen tatsächlich verlangen, ist enger und
//! zugleich billiger: zwei Sessions mit getrennten KV-Caches.** Ein
//! Segment ist eine Position, und der KV-Cache ist sitzungsaffin;
//! teilten planender und verarbeitender Teil ihn, läge der abgerufene
//! Text buchstäblich in dem Cache, aus dem der Planer liest. Getrennte
//! Pods sind dafür weder nötig noch hinreichend.
//!
//! ## Was hier stattdessen steht
//!
//! **Der Plan ist eine Datenstruktur und kein Text.** Er nennt Schritte,
//! jeder Schritt ein Werkzeug und seine Argumente. Ein Argument ist
//! entweder ein Wert aus dem Auftrag oder die Ausgabe eines **früheren**
//! Schritts.
//!
//! ⚑ **Die Zusicherung folgt aus dem, was der Typ nicht kann.** Es gibt
//! keine Verzweigung, keine Schleife und keine Werkzeugwahl zur
//! Laufzeit. Damit steht die Folge der Werkzeugaufrufe fest, **bevor der
//! erste Aufruf geschieht**, und kein abgerufener Wert kann sie ändern:
//! Zwei Läufe, die sich nur in abgerufenen Inhalten unterscheiden,
//! rufen dieselben Werkzeuge in derselben Reihenfolge auf.
//!
//! **Das ist keine Prüfung, sondern eine Abwesenheit.** Eine Prüfung
//! kann man vergessen; ein Konstrukt, das es nicht gibt, kann man nicht
//! benutzen. [`Plan`] ist nach dem Bauen unveränderlich, seine
//! Schrittliste ist privat, und es gibt keinen Weg, sie zu erweitern.
//!
//! ## ⚑ Was ein getrübter Wert darf, und warum die strengere Regel falsch wäre
//!
//! Naheliegend wäre: Ein abgerufener Wert darf nie an eine
//! sicherheitsrelevante Stelle, also weder Empfänger noch Betrag.
//! **Diese Regel wäre zu streng und würde in der ersten Woche
//! aufgegeben.** „Finde den günstigsten Flug und buche ihn" liefert
//! Flugnummer und Preis aus einem Werkzeug; beide sind Argumente einer
//! Buchung. Wer sie verbietet, verbietet die Aufgabe.
//!
//! **Ein getrübter Wert darf deshalb in ein Werkzeugargument fließen.**
//! Er darf nicht bestimmen, **welches** Werkzeug läuft, **ob** es läuft
//! und **wie oft**. Empfänger und Betrag sind durch etwas anderes
//! gedeckt, nämlich den Session-Kontrakt, und der wird vom Konsens
//! durchgesetzt (Kap. 8.2). **Die beiden Mechanismen ergänzen sich:**
//! Die Trübung sperrt den Kontrollfluss, der Kontrakt den Schaden. Sie
//! zu vermengen machte den Agenten unbrauchbar und die Zusage nicht
//! stärker.
//!
//! ## ⚑ Trübung ist keine neue Achse
//!
//! Sie folgt aus dem Werkzeugmanifest, das Phase 1 schon führt: Was aus
//! einem [`Werkzeugart::Extern`] kommt oder aus einer Herkunft, die
//! niemand nachrechnen kann, ist getrübt, und Trübung erbt sich über
//! Argumente weiter. Ein zweites Etikett danebenzustellen hieße, dieselbe
//! Tatsache zweimal zu führen, und zwei Quellen für eine Aussage laufen
//! auseinander.
//!
//! ## Der Preis, und er wird hier genannt
//!
//! Ein gerader Plan kann **nicht auf ein Ergebnis reagieren**. „Wenn der
//! Preis unter 500 liegt, buche" ist nicht ausdrückbar. Für die
//! *Sicherheit* ist das kein Verlust, denn die Obergrenze steht im
//! Kontrakt; für die *Ergebnisqualität* ist es einer. Der Ausweg wäre,
//! den Planer erneut laufen zu lassen, und genau dabei liefe der
//! abgerufene Inhalt in seinen Kontext zurück. **Das ist eine eigene
//! Entscheidung und keine Lücke, die man nebenbei schließt.**

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::hash::Hash;
use myl_types::ids::MerkleRoot;

use crate::manifest::Werkzeugart;
use crate::registratur::{Benutzt, Registratur};

/// Trennzeichen für die Planadresse.
pub const DST_PLAN: &[u8] = b"MYELITH_AGENTENPLAN_v1";

/// Höchstzahl der Schritte eines Plans.
///
/// Ein Plan wandert in die Spur und wird von jedem Prüfer gelesen;
/// seine Größe darf nicht an einer Modellausgabe hängen. Zugleich ist
/// dies die Abbruchbedingung aus Kap. 8.4, hier ohne Zähler: Ein
/// gerader Plan endet nach seinem letzten Schritt.
pub const MAX_SCHRITTE: usize = 64;

/// Höchstzahl der Argumente eines Schritts.
pub const MAX_ARGUMENTE: usize = 16;

/// Woher ein Argument kommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Quelle {
    /// Aus dem Auftrag des Nutzers, als Hash über den Wert.
    ///
    /// **Ungetrübt**, denn der Nutzer ist nicht der Angreifer, gegen den
    /// Kap. 8.3 schützt.
    Auftrag(Hash),
    /// Die Ausgabe eines **früheren** Schritts.
    ///
    /// Nur rückwärts: Ein Vorwärtsverweis wäre ein Kreis, und ein Kreis
    /// wäre eine Schleife, deren Länge von einem Ergebnis abhinge.
    Schritt(u16),
}

/// Ein Schritt: ein Werkzeug und seine Argumente.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Schritt {
    /// Welches Werkzeug, als Manifestadresse.
    ///
    /// ⚑ **Fest im Plan und nicht zur Laufzeit gewählt.** Stünde hier
    /// eine Quelle statt einer Adresse, könnte ein abgerufener Wert
    /// bestimmen, was als Nächstes aufgerufen wird, und genau das
    /// schließt Kap. 8.3 aus.
    pub werkzeug: MerkleRoot,
    /// Die Argumente, in Reihenfolge.
    pub argumente: Vec<Quelle>,
}

/// Warum ein Plan nicht gilt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Planfehler {
    /// Mehr als [`MAX_SCHRITTE`] Schritte.
    ZuVieleSchritte {
        /// Wie viele übergeben wurden.
        hatte: usize,
    },
    /// Ein Schritt hat mehr als [`MAX_ARGUMENTE`] Argumente.
    ZuVieleArgumente {
        /// Welcher Schritt.
        schritt: usize,
        /// Wie viele.
        hatte: usize,
    },
    /// ⚑ Ein Argument verweist auf einen Schritt, der nicht vor ihm
    /// liegt.
    ///
    /// **Der einzige Weg, aus einem geraden Plan eine Schleife zu
    /// machen**, und deshalb der Fehler, auf den es hier ankommt.
    VerweisNichtRueckwaerts {
        /// Welcher Schritt verweist.
        schritt: usize,
        /// Wohin.
        auf: usize,
    },
}

impl std::fmt::Display for Planfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZuVieleSchritte { hatte } => {
                write!(f, "{hatte} Schritte, höchstens {MAX_SCHRITTE}")
            }
            Self::ZuVieleArgumente { schritt, hatte } => {
                write!(f, "Schritt {schritt} hat {hatte} Argumente, höchstens {MAX_ARGUMENTE}")
            }
            Self::VerweisNichtRueckwaerts { schritt, auf } => {
                write!(f, "Schritt {schritt} verweist auf {auf}, das ist nicht rückwärts")
            }
        }
    }
}

impl std::error::Error for Planfehler {}

/// Ob ein Wert aus der Außenwelt stammt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Truebung {
    /// Aus dem Auftrag oder aus nachrechenbaren Werkzeugen.
    Rein,
    /// Mittelbar oder unmittelbar aus einem externen Werkzeug.
    Getruebt,
}

/// Ein fester Ablauf aus Werkzeugaufrufen.
///
/// ⚑ **Nach dem Bauen unveränderlich, und das ist die Zusicherung.**
/// Die Schrittliste ist privat und es gibt keinen Weg, sie zu
/// erweitern; damit steht die Folge der Aufrufe fest, bevor der erste
/// geschieht.
///
/// **Ein Test kann das Fehlen einer Schnittstelle nicht beweisen.** Wer
/// diese Datei prüft, prüft daher auch, dass unterhalb kein `&mut` auf
/// `schritte` und kein `push` hinzugekommen ist. Die Tests zeigen, was
/// die Zusicherung wert ist, nicht dass sie besteht.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Plan {
    schritte: Vec<Schritt>,
}

impl Plan {
    /// Baut einen Plan und prüft seine Form.
    ///
    /// Geprüft wird, was strukturell sein muss: Länge, Argumentzahl und
    /// **dass jeder Verweis rückwärts zeigt**. Ob die Werkzeuge bekannt
    /// sind, entscheidet erst die Registratur ([`Plan::benutzt`]), denn
    /// ein Plan kann vor dem Laden der Manifeste entstehen.
    pub fn neu(schritte: Vec<Schritt>) -> Result<Self, Planfehler> {
        if schritte.len() > MAX_SCHRITTE {
            return Err(Planfehler::ZuVieleSchritte { hatte: schritte.len() });
        }
        for (i, s) in schritte.iter().enumerate() {
            if s.argumente.len() > MAX_ARGUMENTE {
                return Err(Planfehler::ZuVieleArgumente {
                    schritt: i,
                    hatte: s.argumente.len(),
                });
            }
            for a in &s.argumente {
                if let Quelle::Schritt(j) = a {
                    let j = *j as usize;
                    if j >= i {
                        return Err(Planfehler::VerweisNichtRueckwaerts { schritt: i, auf: j });
                    }
                }
            }
        }
        Ok(Self { schritte })
    }

    /// Der leere Plan: tut nichts, und das ist zulässig.
    pub fn leer() -> Self {
        Self { schritte: Vec::new() }
    }

    /// Die Schritte, nur lesend.
    pub fn schritte(&self) -> &[Schritt] {
        &self.schritte
    }

    /// Wie viele Schritte.
    pub fn len(&self) -> usize {
        self.schritte.len()
    }

    /// Ob der Plan nichts tut.
    pub fn is_empty(&self) -> bool {
        self.schritte.is_empty()
    }

    /// Die Adresse: Hash über Trennzeichen und kanonische Kodierung.
    ///
    /// ⚑ **Ohne sie ist die Zusage aus Kap. 8.3 nicht überprüfbar.**
    /// „Die Folge der Aufrufe stand fest, bevor der erste geschah" lässt
    /// sich von außen nur glauben, solange niemand belegen kann, **wann**
    /// sie feststand. Wer den Plan hinterher passend zu dem baut, was
    /// geschehen ist, erfüllt jede Prüfung an ihm.
    ///
    /// Mit der Adresse wird aus der Zusage eine Festlegung: Sie geht in
    /// den Anker der Segmentkette ein ([`crate::kette::anker`]), also in
    /// den ersten Schritt, und ein nachträglich geänderter Plan bricht
    /// die ganze Kette.
    ///
    /// Aufgefallen beim Bauen von Punkt 4.1, nicht beim Bauen von 3.1.
    pub fn adresse(&self) -> Hash {
        let mut daten = Vec::with_capacity(DST_PLAN.len() + 64);
        daten.extend_from_slice(DST_PLAN);
        daten.extend_from_slice(&borsh::to_vec(self).expect("Plan ist stets serialisierbar"));
        Hash::sha256(&daten)
    }

    /// Die Folge der Werkzeugaufrufe.
    ///
    /// ⚑ **Sie hängt von keinem Wert ab, sondern nur vom Plan**, und
    /// genau das ist die Aussage aus Kap. 8.3. Die Funktion nimmt
    /// deshalb keine Eingaben entgegen; könnte sie es, wäre die Zusage
    /// eine Behauptung über ihren Rumpf statt über ihre Signatur.
    pub fn werkzeugfolge(&self) -> Vec<MerkleRoot> {
        self.schritte.iter().map(|s| s.werkzeug).collect()
    }

    /// Was der Plan an Werkzeugen benutzt, für [`Registratur::stufe`].
    ///
    /// Sortiert und ohne Dubletten, damit die Ausgabe deterministisch
    /// ist: Sie wandert in die Spur.
    pub fn benutzt(&self) -> Benutzt {
        let mut werkzeuge: Vec<MerkleRoot> = self.werkzeugfolge();
        werkzeuge.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        werkzeuge.dedup();
        Benutzt { skills: Vec::new(), werkzeuge }
    }

    /// Die Trübung je Schritt.
    ///
    /// Ein Schritt ist getrübt, wenn sein Werkzeug die Außenwelt befragt
    /// oder nicht nachrechenbar ist, **oder** wenn eines seiner
    /// Argumente aus einem getrübten Schritt kommt. Ein unbekanntes
    /// Werkzeug gilt als getrübt: Wer nicht weiß, was es tut, weiß auch
    /// nicht, dass es rechnet.
    ///
    /// Ein Durchgang genügt, weil jeder Verweis rückwärts zeigt.
    pub fn truebung(&self, reg: &Registratur) -> Vec<Truebung> {
        let mut aus = Vec::with_capacity(self.schritte.len());
        for s in &self.schritte {
            let eigen = match reg.werkzeug(&s.werkzeug) {
                Some(m) => m.art == Werkzeugart::Extern || !m.nachrechenbar(),
                None => true,
            };
            let geerbt = s.argumente.iter().any(|a| match a {
                Quelle::Auftrag(_) => false,
                Quelle::Schritt(j) => aus[*j as usize] == Truebung::Getruebt,
            });
            aus.push(if eigen || geerbt { Truebung::Getruebt } else { Truebung::Rein });
        }
        aus
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Herkunft, Werkzeugmanifest};
    use borsh::{from_slice, to_vec};

    fn werkzeug(name: &str, art: Werkzeugart, herkunft: Herkunft) -> Werkzeugmanifest {
        Werkzeugmanifest {
            name: name.into(),
            anbieter: "prüfstand".into(),
            revision: "1".into(),
            lizenz: "Apache-2.0".into(),
            art,
            herkunft,
        }
    }

    /// Eine Registratur mit drei Werkzeugen: eines rechnet und ist
    /// verankert, eines befragt die Welt, eines liegt nur lokal.
    fn registratur() -> (Registratur, MerkleRoot, MerkleRoot, MerkleRoot) {
        let mut r = Registratur::neu();
        let rein = r
            .nimm_werkzeug(werkzeug("rechnen", Werkzeugart::Deterministisch, Herkunft::Verankert))
            .expect("gültig");
        let extern_ = r
            .nimm_werkzeug(werkzeug("abrufen", Werkzeugart::Extern, Herkunft::Bibliothek))
            .expect("gültig");
        let lokal = r
            .nimm_werkzeug(werkzeug("eigenes", Werkzeugart::Deterministisch, Herkunft::Lokal))
            .expect("gültig");
        (r, rein, extern_, lokal)
    }

    fn schritt(w: MerkleRoot, args: &[Quelle]) -> Schritt {
        Schritt { werkzeug: w, argumente: args.to_vec() }
    }

    /// ⚑ **Die Aussage aus Kap. 8.3, als Test.** Die Folge der
    /// Werkzeugaufrufe steht fest, bevor irgendetwas gerechnet wurde,
    /// und sie hängt von keinem Wert ab.
    ///
    /// Der Test kann das nur **vorführen**; getragen wird es davon,
    /// dass `werkzeugfolge` keine Eingaben entgegennimmt und `Plan`
    /// keine Schnittstelle zum Erweitern hat.
    #[test]
    fn die_werkzeugfolge_haengt_von_keinem_wert_ab() {
        let (_, rein, ext, _) = registratur();
        let p = Plan::neu(vec![
            schritt(ext, &[Quelle::Auftrag(Hash::sha256(b"frage"))]),
            schritt(rein, &[Quelle::Schritt(0)]),
            schritt(ext, &[Quelle::Schritt(1), Quelle::Auftrag(Hash::sha256(b"rest"))]),
        ])
        .expect("gültiger Plan");

        assert_eq!(p.werkzeugfolge(), vec![ext, rein, ext]);
        assert_eq!(p.len(), 3);

        // Derselbe Plan, egal was die Werkzeuge zurückgeben: Die Folge
        // steht in der Struktur und nicht in den Werten.
        let noch_einmal = p.clone();
        assert_eq!(p.werkzeugfolge(), noch_einmal.werkzeugfolge());
    }

    /// ⚑ **Der einzige Weg, aus einem geraden Plan eine Schleife zu
    /// machen, ist ein Verweis nach vorn.** Er wird abgewiesen, und
    /// damit ist die Terminierung strukturell und nicht gezählt.
    #[test]
    fn ein_verweis_nach_vorn_wird_abgewiesen() {
        let (_, rein, _, _) = registratur();
        assert_eq!(
            Plan::neu(vec![schritt(rein, &[Quelle::Schritt(1)]), schritt(rein, &[])]),
            Err(Planfehler::VerweisNichtRueckwaerts { schritt: 0, auf: 1 })
        );
        // Auch auf sich selbst.
        assert_eq!(
            Plan::neu(vec![schritt(rein, &[Quelle::Schritt(0)])]),
            Err(Planfehler::VerweisNichtRueckwaerts { schritt: 0, auf: 0 })
        );
        // Rückwärts ist erlaubt, auch über mehrere Stufen.
        assert!(Plan::neu(vec![
            schritt(rein, &[]),
            schritt(rein, &[Quelle::Schritt(0)]),
            schritt(rein, &[Quelle::Schritt(0), Quelle::Schritt(1)]),
        ])
        .is_ok());
    }

    /// ⚑ Trübung erbt sich über Argumente, und ein Auftragswert trübt
    /// nicht: Der Nutzer ist nicht der Angreifer aus Kap. 8.3.
    #[test]
    fn truebung_erbt_sich_ueber_die_argumente() {
        let (r, rein, ext, lokal) = registratur();
        let p = Plan::neu(vec![
            schritt(rein, &[Quelle::Auftrag(Hash::sha256(b"a"))]), // 0 rein
            schritt(ext, &[Quelle::Schritt(0)]),                   // 1 extern
            schritt(rein, &[Quelle::Schritt(1)]),                  // 2 geerbt
            schritt(rein, &[Quelle::Schritt(0)]),                  // 3 rein
            schritt(lokal, &[]),                                   // 4 nicht nachrechenbar
        ])
        .expect("gültig");

        assert_eq!(
            p.truebung(&r),
            vec![
                Truebung::Rein,
                Truebung::Getruebt,
                Truebung::Getruebt,
                Truebung::Rein,
                Truebung::Getruebt,
            ]
        );
    }

    /// ⚑ Ein unbekanntes Werkzeug gilt als getrübt. **Wer nicht weiß,
    /// was es tut, weiß auch nicht, dass es rechnet.**
    #[test]
    fn ein_unbekanntes_werkzeug_gilt_als_getruebt() {
        let (r, rein, _, _) = registratur();
        let fremd = MerkleRoot::new([9u8; 32]);
        let p = Plan::neu(vec![schritt(fremd, &[]), schritt(rein, &[Quelle::Schritt(0)])])
            .expect("gültig");
        assert_eq!(p.truebung(&r), vec![Truebung::Getruebt, Truebung::Getruebt]);

        // Und die Registratur sagt dasselbe eine Ebene höher.
        assert_eq!(
            r.stufe(&p.benutzt()),
            crate::registratur::Segmentstufe::Unbekannt { welche: vec![fremd] }
        );
    }

    /// Die Stufe eines Plans ist die Stufe seiner Werkzeuge: Dieselbe
    /// Rechnung wie für jedes andere Segment, keine zweite Quelle.
    #[test]
    fn die_stufe_eines_plans_kommt_aus_der_registratur() {
        use crate::registratur::Segmentstufe;
        let (r, rein, ext, _) = registratur();

        let sauber = Plan::neu(vec![schritt(rein, &[]), schritt(rein, &[Quelle::Schritt(0)])])
            .expect("gültig");
        assert_eq!(r.stufe(&sauber.benutzt()), Segmentstufe::Nachrechenbar);

        let mit_welt = Plan::neu(vec![schritt(rein, &[]), schritt(ext, &[Quelle::Schritt(0)])])
            .expect("gültig");
        assert_eq!(r.stufe(&mit_welt.benutzt()), Segmentstufe::Bezeugt { wegen: vec![ext] });
    }

    /// Die Werkzeugliste ist sortiert und ohne Dubletten, denn sie
    /// wandert in die Spur; zwei ehrliche Knoten müssen dieselbe
    /// schreiben.
    #[test]
    fn benutzt_ist_deterministisch_und_dublettenfrei() {
        let (_, rein, ext, lokal) = registratur();
        let p = Plan::neu(vec![
            schritt(lokal, &[]),
            schritt(ext, &[]),
            schritt(rein, &[]),
            schritt(ext, &[]),
            schritt(lokal, &[]),
        ])
        .expect("gültig");
        let b = p.benutzt();
        assert_eq!(b.werkzeuge.len(), 3, "fünf Schritte, drei Werkzeuge");
        assert!(b.skills.is_empty());
        let mut sortiert = b.werkzeuge.clone();
        sortiert.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        assert_eq!(b.werkzeuge, sortiert);
    }

    #[test]
    fn die_grenzen_werden_eingehalten() {
        let (_, rein, _, _) = registratur();
        let zu_lang: Vec<Schritt> =
            (0..=MAX_SCHRITTE).map(|_| schritt(rein, &[])).collect();
        assert_eq!(
            Plan::neu(zu_lang),
            Err(Planfehler::ZuVieleSchritte { hatte: MAX_SCHRITTE + 1 })
        );

        let viele: Vec<Quelle> = (0..=MAX_ARGUMENTE)
            .map(|i| Quelle::Auftrag(Hash::sha256(&[i as u8])))
            .collect();
        assert_eq!(
            Plan::neu(vec![schritt(rein, &viele)]),
            Err(Planfehler::ZuVieleArgumente { schritt: 0, hatte: MAX_ARGUMENTE + 1 })
        );

        assert!(Plan::leer().is_empty());
        assert_eq!(Plan::leer().werkzeugfolge(), Vec::new());
    }

    /// Ein Plan wandert über das Netz und muss den Rundweg überstehen.
    ///
    /// ⚑ **Und der Rundweg umgeht die Prüfung**, weil Borsh direkt in
    /// die private Liste schreibt. Deshalb prüft der Empfänger einen
    /// gelesenen Plan erneut, statt sich auf den Typ zu verlassen.
    #[test]
    fn borsh_ist_ein_rundweg_und_ersetzt_die_pruefung_nicht() {
        let (_, rein, _, _) = registratur();
        let p = Plan::neu(vec![schritt(rein, &[]), schritt(rein, &[Quelle::Schritt(0)])])
            .expect("gültig");
        let zurueck: Plan = from_slice(&to_vec(&p).expect("ser")).expect("de");
        assert_eq!(p, zurueck);

        // Ein zurechtgebogener Plan mit Vorwärtsverweis kommt durch die
        // Deserialisierung, und `neu` weist ihn ab. Genau dafür ist die
        // Wiederholung da.
        let boese = Plan { schritte: vec![schritt(rein, &[Quelle::Schritt(5)])] };
        let bytes = to_vec(&boese).expect("ser");
        let gelesen: Plan = from_slice(&bytes).expect("de");
        assert_eq!(
            Plan::neu(gelesen.schritte().to_vec()),
            Err(Planfehler::VerweisNichtRueckwaerts { schritt: 0, auf: 5 })
        );
    }
}
