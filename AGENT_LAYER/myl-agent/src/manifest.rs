//! Was ein Skill und was ein Werkzeug ist, und was ein Prüfer damit
//! anfangen kann.
//!
//! ## ⚑ Die tragende Unterscheidung: die Herkunftsstufe
//!
//! Kap. 8.1 kennt zwei Sorten Werkzeug: **deterministische**, die
//! vollständig verifiziert werden wie ein reguläres Segment, und
//! **externe**, deren Ergebnis attestiert und nicht nachgerechnet wird.
//! Die Unterscheidung ist dort eine Eigenschaft des Werkzeugs. **Hier
//! ist sie eine Eigenschaft des Manifests und wandert in die Spur**, und
//! das ist kein Detail:
//!
//! Ohne sie sieht ein Prüfer einem Segment **nicht an**, ob er es
//! nachrechnen kann oder nur glauben muss. Beides ist zulässig; beides
//! gleich aussehen zu lassen ist es nicht.
//!
//! ## Drei Herkünfte, drei Verifikationsstufen
//!
//! | Herkunft | Wer hat den Inhalt | Was ein Dritter kann |
//! |---|---|---|
//! | [`Herkunft::Verankert`] | alle, der Hash steht im Konsens | vollständig nachrechnen |
//! | [`Herkunft::Bibliothek`] | alle, kuratiert, Hash verankert | dito, solange der Hash im Konsens steht |
//! | [`Herkunft::Lokal`] | nur der Nutzer | **nichts** nachrechnen |
//!
//! ⚑ **Die dritte Zeile ist der Preis einer Freiheit, und er gehört dem
//! Nutzer gesagt.** Ein lokaler Skill lässt sich frei einpflegen, und
//! genau deshalb kann niemand sonst prüfen, was er getan hat. Ein
//! Segment, das ihn benutzt, ist ein **externer Eingang** im Sinne von
//! Kap. 8.1, gleich wie sorgfältig gehasht wird: Ein Hash belegt,
//! **welcher** Skill benutzt wurde, wenn man ihn schon hat. Er erlaubt
//! keinem Dritten, das Ergebnis nachzurechnen.
//!
//! **Das ist der Grund, warum die Stufe am Manifest hängt und nicht am
//! Aufrufer.** Wer sie beim Aufruf setzen dürfte, könnte einen lokalen
//! Skill als verankert ausgeben.
//!
//! ## Warum die Wurzel wie beim Modellmanifest gebaut ist
//!
//! Längenpräfix vor jedem Feld, eigener Domain-Trenner. Dieselbe Form
//! wie `myl_governance::modell::Modellmanifest::wurzel`, und aus
//! demselben Grund: Ohne Längenpräfix ergäben `("ab", "c")` und
//! `("a", "bc")` dieselben Bytes.

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::hash::Hash;
use myl_types::ids::MerkleRoot;

/// Domain-Trenner der Skill-Wurzel.
pub const DST_SKILLMANIFEST: &[u8] = b"MYELITH_SKILLMANIFEST_v1";
/// Domain-Trenner der Werkzeug-Wurzel.
pub const DST_WERKZEUGMANIFEST: &[u8] = b"MYELITH_WERKZEUGMANIFEST_v1";

/// Woher ein Skill oder Werkzeug stammt, und was daraus folgt.
///
/// ⚑ **Die Reihenfolge der Varianten ist die Ordnung der
/// Verifizierbarkeit**, von der schwächsten zur stärksten. `Ord` ist
/// deshalb abgeleitet und bedeutet etwas: Wer zwei Stufen vergleicht,
/// vergleicht, wie viel ein Dritter nachrechnen kann.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, BorshSerialize, BorshDeserialize)]
pub enum Herkunft {
    /// Der Nutzer bringt ihn mit, niemand sonst hat ihn.
    ///
    /// **Ein Segment, das ihn benutzt, ist nicht nachrechenbar.**
    Lokal,
    /// Aus der kuratierten Bibliothek, Hash im Konsens verankert.
    Bibliothek,
    /// Im Konsens verankert, wie ein deterministisches Werkzeug nach
    /// Kap. 8.1.
    Verankert,
}

impl Herkunft {
    /// Kann ein Dritter das Ergebnis nachrechnen?
    ///
    /// **Genau für `Lokal` ist die Antwort nein**, und daran hängt, ob
    /// ein Segment verifiziert oder nur attestiert wird.
    pub fn nachrechenbar(&self) -> bool {
        !matches!(self, Self::Lokal)
    }

    /// Der Name für Protokolle und Fehlermeldungen.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Lokal => "lokal",
            Self::Bibliothek => "Bibliothek",
            Self::Verankert => "verankert",
        }
    }
}

/// Ob ein Werkzeug rechnet oder die Welt befragt (Kap. 8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, BorshSerialize, BorshDeserialize)]
pub enum Werkzeugart {
    /// Rechnet aus seinen Eingaben, sonst nichts: Ledger-Abfragen,
    /// Berechnungen, verankerte Korpora. Verifizierbar wie ein
    /// reguläres Segment.
    Deterministisch,
    /// Fragt die Außenwelt. Das Ergebnis wird **attestiert**, nicht
    /// nachgerechnet; Kap. 8.1 nennt die Grenze ausdrücklich.
    Extern,
}

/// Ein Teil eines Skills: ein Abschnitt mit seinem Klartext-Hash.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Teil {
    /// Überschrift oder Fundstelle im Original.
    pub fundstelle: String,
    /// Hash über den Klartext dieses Teils.
    pub inhalt: Hash,
}

/// Warum ein Manifest nicht angenommen wurde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestFehler {
    /// Ein Pflichtfeld ist leer oder besteht aus Leerzeichen.
    FeldLeer { feld: &'static str },
    /// Ein Skill ohne Teile ist kein Skill.
    OhneTeile,
    /// Die Stufe passt nicht zur Art: Ein externes Werkzeug kann nicht
    /// verankert sein, denn verankert heißt nachrechenbar.
    StufePasstNichtZurArt {
        art: Werkzeugart,
        herkunft: Herkunft,
    },
}

impl std::fmt::Display for ManifestFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FeldLeer { feld } => write!(f, "das Feld {} ist leer", feld),
            Self::OhneTeile => write!(f, "ein Skill ohne Teile ist kein Skill"),
            Self::StufePasstNichtZurArt { art, herkunft } => write!(
                f,
                "ein {:?}es Werkzeug kann nicht {} sein: {} heißt nachrechenbar, \
                 und ein externes Ergebnis ist es nicht",
                art,
                herkunft.name(),
                herkunft.name()
            ),
        }
    }
}

impl std::error::Error for ManifestFehler {}

fn nicht_leer(feld: &'static str, wert: &str) -> Result<(), ManifestFehler> {
    if wert.trim().is_empty() {
        return Err(ManifestFehler::FeldLeer { feld });
    }
    Ok(())
}

/// Hängt ein Feld mit Längenpräfix an.
fn feld(daten: &mut Vec<u8>, roh: &[u8]) {
    daten.extend_from_slice(&(roh.len() as u64).to_le_bytes());
    daten.extend_from_slice(roh);
}

/// Ein Skill: abrufbares Wissen mit Herkunft und Lizenz.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Skillmanifest {
    /// Der Name, unter dem er angeboten wird.
    pub name: String,
    /// Woher der Inhalt kommt (Buch, Korpus, Sammlung).
    pub quelle: String,
    /// Die Ausgabe oder Revision der Quelle.
    ///
    /// **Ein leeres Feld hieße „was gerade dort liegt"** und wird
    /// zurückgewiesen, wie beim Modellmanifest.
    pub revision: String,
    /// Die Lizenz der Quelle (ETHICS G7).
    pub lizenz: String,
    /// Die Abschnitte mit ihren Klartext-Hashes.
    pub teile: Vec<Teil>,
    /// Die Herkunftsstufe.
    pub herkunft: Herkunft,
}

impl Skillmanifest {
    /// Prüft, dass jedes Pflichtfeld gefüllt ist.
    pub fn pruefe_vollstaendig(&self) -> Result<(), ManifestFehler> {
        nicht_leer("name", &self.name)?;
        nicht_leer("quelle", &self.quelle)?;
        nicht_leer("revision", &self.revision)?;
        nicht_leer("lizenz", &self.lizenz)?;
        if self.teile.is_empty() {
            return Err(ManifestFehler::OhneTeile);
        }
        for t in &self.teile {
            nicht_leer("teil.fundstelle", &t.fundstelle)?;
        }
        Ok(())
    }

    /// Die Adresse dieses Skills: der Hash über sein Manifest.
    ///
    /// ⚑ **Die Herkunftsstufe geht mit ein**, und das ist Absicht. Zwei
    /// Skills mit gleichem Inhalt und verschiedener Herkunft sind
    /// **verschiedene Gegenstände**, denn ein Prüfer kann mit ihnen
    /// Verschiedenes anfangen. Stünde die Stufe daneben statt darin,
    /// ließe sich ein lokaler Skill unter der Adresse eines verankerten
    /// ausgeben.
    pub fn wurzel(&self) -> MerkleRoot {
        let mut daten = Vec::new();
        daten.extend_from_slice(DST_SKILLMANIFEST);
        feld(&mut daten, self.name.as_bytes());
        feld(&mut daten, self.quelle.as_bytes());
        feld(&mut daten, self.revision.as_bytes());
        feld(&mut daten, self.lizenz.as_bytes());
        feld(&mut daten, &[self.herkunft as u8]);
        feld(&mut daten, &(self.teile.len() as u64).to_le_bytes());
        for t in &self.teile {
            feld(&mut daten, t.fundstelle.as_bytes());
            feld(&mut daten, t.inhalt.as_bytes());
        }
        MerkleRoot::new(Hash::sha256(&daten).0)
    }

    /// Kann ein Dritter ein Segment nachrechnen, das diesen Skill
    /// benutzt hat?
    pub fn nachrechenbar(&self) -> bool {
        self.herkunft.nachrechenbar()
    }
}

/// Ein Werkzeug: etwas, das ein Agent aufrufen kann.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Werkzeugmanifest {
    /// Der Name, unter dem es aufgerufen wird.
    pub name: String,
    /// Wer es bereitstellt.
    pub anbieter: String,
    /// Die Fassung.
    pub revision: String,
    /// Die Lizenz (ETHICS G7).
    pub lizenz: String,
    /// Rechnet es oder befragt es die Welt?
    pub art: Werkzeugart,
    /// Die Herkunftsstufe.
    pub herkunft: Herkunft,
}

impl Werkzeugmanifest {
    /// Prüft Pflichtfelder **und** die Verträglichkeit von Art und
    /// Stufe.
    ///
    /// ⚑ **Ein externes Werkzeug kann nicht verankert sein.** Verankert
    /// heißt nachrechenbar, und ein Ergebnis aus der Außenwelt ist es
    /// nicht, gleich wo sein Hash steht. Die beiden Felder sind nicht
    /// unabhängig, und ohne diese Prüfung ließe sich ein externer Abruf
    /// als deterministisches Segment ausgeben.
    pub fn pruefe_vollstaendig(&self) -> Result<(), ManifestFehler> {
        nicht_leer("name", &self.name)?;
        nicht_leer("anbieter", &self.anbieter)?;
        nicht_leer("revision", &self.revision)?;
        nicht_leer("lizenz", &self.lizenz)?;
        if self.art == Werkzeugart::Extern && self.herkunft == Herkunft::Verankert {
            return Err(ManifestFehler::StufePasstNichtZurArt {
                art: self.art,
                herkunft: self.herkunft,
            });
        }
        Ok(())
    }

    /// Die Adresse dieses Werkzeugs.
    pub fn wurzel(&self) -> MerkleRoot {
        let mut daten = Vec::new();
        daten.extend_from_slice(DST_WERKZEUGMANIFEST);
        feld(&mut daten, self.name.as_bytes());
        feld(&mut daten, self.anbieter.as_bytes());
        feld(&mut daten, self.revision.as_bytes());
        feld(&mut daten, self.lizenz.as_bytes());
        feld(&mut daten, &[self.art as u8]);
        feld(&mut daten, &[self.herkunft as u8]);
        MerkleRoot::new(Hash::sha256(&daten).0)
    }

    /// Wird ein Aufruf dieses Werkzeugs nachgerechnet oder bezeugt?
    ///
    /// **Beides muss stimmen:** Ein deterministisches Werkzeug aus
    /// lokaler Quelle ist so wenig nachrechenbar wie ein externes.
    pub fn nachrechenbar(&self) -> bool {
        self.art == Werkzeugart::Deterministisch && self.herkunft.nachrechenbar()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(herkunft: Herkunft) -> Skillmanifest {
        Skillmanifest {
            name: "Rustnomicon, Kapitel 3".into(),
            quelle: "The Rustonomicon".into(),
            revision: "2024-06".into(),
            lizenz: "MIT OR Apache-2.0".into(),
            teile: vec![Teil {
                fundstelle: "3.1 Aliasing".into(),
                inhalt: Hash::sha256(b"Aliasing"),
            }],
            herkunft,
        }
    }

    fn werkzeug(art: Werkzeugart, herkunft: Herkunft) -> Werkzeugmanifest {
        Werkzeugmanifest {
            name: "ledger_abfrage".into(),
            anbieter: "Myelith".into(),
            revision: "1".into(),
            lizenz: "PolyForm Shield License 1.0.0".into(),
            art,
            herkunft,
        }
    }

    /// ⚑ **Der tragende Satz dieses Moduls: Gleicher Inhalt, andere
    /// Herkunft, andere Adresse.**
    ///
    /// Zwei Skills mit identischen Teilen sind **verschiedene
    /// Gegenstände**, wenn ihre Herkunft verschieden ist, denn ein
    /// Prüfer kann mit ihnen Verschiedenes anfangen. Stünde die Stufe
    /// neben dem Manifest statt darin, ließe sich ein lokaler Skill
    /// unter der Adresse eines verankerten ausgeben.
    #[test]
    fn gleicher_inhalt_und_andere_herkunft_ergeben_andere_adressen() {
        let l = skill(Herkunft::Lokal).wurzel();
        let b = skill(Herkunft::Bibliothek).wurzel();
        let v = skill(Herkunft::Verankert).wurzel();
        assert_ne!(l, b);
        assert_ne!(b, v);
        assert_ne!(l, v);
        // Gegenprobe: dieselbe Herkunft, dieselbe Adresse. Sonst bewiese
        // der Test oben nur, dass die Wurzel von irgendetwas abhängt.
        assert_eq!(skill(Herkunft::Lokal).wurzel(), l);
    }

    /// ⚑ **Genau `Lokal` ist nicht nachrechenbar, und daran hängt die
    /// Verifikationsstufe des ganzen Segments.**
    #[test]
    fn genau_lokal_ist_nicht_nachrechenbar() {
        assert!(!skill(Herkunft::Lokal).nachrechenbar());
        assert!(skill(Herkunft::Bibliothek).nachrechenbar());
        assert!(skill(Herkunft::Verankert).nachrechenbar());
    }

    /// ⚑ **Ein externes Werkzeug kann nicht verankert sein.**
    ///
    /// Verankert heißt nachrechenbar, und ein Ergebnis aus der
    /// Außenwelt ist es nicht, gleich wo sein Hash steht. Ohne diese
    /// Prüfung ließe sich ein externer Abruf als deterministisches
    /// Segment ausgeben, und Kap. 8.1 verlöre seine Grenze.
    #[test]
    fn ein_externes_werkzeug_kann_nicht_verankert_sein() {
        assert!(matches!(
            werkzeug(Werkzeugart::Extern, Herkunft::Verankert).pruefe_vollstaendig(),
            Err(ManifestFehler::StufePasstNichtZurArt { .. })
        ));
        // Gegenprobe, beide Richtungen: Extern und Bibliothek geht,
        // deterministisch und verankert auch.
        assert!(werkzeug(Werkzeugart::Extern, Herkunft::Bibliothek)
            .pruefe_vollstaendig()
            .is_ok());
        assert!(werkzeug(Werkzeugart::Deterministisch, Herkunft::Verankert)
            .pruefe_vollstaendig()
            .is_ok());
    }

    /// ⚑ **Beides muss stimmen**: Ein deterministisches Werkzeug aus
    /// lokaler Quelle ist so wenig nachrechenbar wie ein externes. Der
    /// Test fährt alle vier Paarungen ab, damit keine durchrutscht.
    #[test]
    fn nachrechenbar_verlangt_art_und_herkunft() {
        use Herkunft::*;
        use Werkzeugart::*;
        let erwartet = [
            ((Deterministisch, Verankert), true),
            ((Deterministisch, Bibliothek), true),
            ((Deterministisch, Lokal), false),
            ((Extern, Bibliothek), false),
            ((Extern, Lokal), false),
        ];
        for ((art, h), soll) in erwartet {
            assert_eq!(
                werkzeug(art, h).nachrechenbar(),
                soll,
                "{:?} aus {} sollte {}nachrechenbar sein",
                art,
                h.name(),
                if soll { "" } else { "nicht " }
            );
        }
    }

    /// Ein leeres Pflichtfeld heißt „was gerade dort liegt" und wird
    /// zurückgewiesen, Leerzeichen eingeschlossen.
    #[test]
    fn leere_pflichtfelder_werden_zurueckgewiesen() {
        type Leeren = fn(&mut Skillmanifest);
        let leeren: [(&str, Leeren); 4] = [
            ("name", |s| s.name = "  ".into()),
            ("quelle", |s| s.quelle = String::new()),
            ("revision", |s| s.revision = "\t".into()),
            ("lizenz", |s| s.lizenz = String::new()),
        ];
        for (feld, leere) in leeren {
            let mut s = skill(Herkunft::Verankert);
            leere(&mut s);
            assert!(
                s.pruefe_vollstaendig().is_err(),
                "leeres Feld {feld} ging durch"
            );
        }
        // Und ein Skill ohne Teile ist kein Skill.
        let mut ohne = skill(Herkunft::Verankert);
        ohne.teile.clear();
        assert_eq!(ohne.pruefe_vollstaendig(), Err(ManifestFehler::OhneTeile));
        // Gegenprobe: der vollständige geht durch.
        assert!(skill(Herkunft::Verankert).pruefe_vollstaendig().is_ok());
    }

    /// ⚑ Das Längenpräfix trägt: Zwei Felder, die sich nur an der Grenze
    /// unterscheiden, ergeben verschiedene Wurzeln.
    ///
    /// Ohne Präfix ergäben `("ab", "c")` und `("a", "bc")` dieselben
    /// Bytes. Dieselbe Prüfung steht beim Modellmanifest, und aus
    /// demselben Grund.
    #[test]
    fn das_laengenpraefix_trennt_die_felder() {
        let mut a = skill(Herkunft::Verankert);
        a.name = "ab".into();
        a.quelle = "c".into();
        let mut b = skill(Herkunft::Verankert);
        b.name = "a".into();
        b.quelle = "bc".into();
        assert_ne!(a.wurzel(), b.wurzel());
    }

    /// Die Ordnung der Herkunft ist die Ordnung der Verifizierbarkeit,
    /// und sie bedeutet etwas.
    #[test]
    fn die_ordnung_der_herkunft_steigt_mit_der_verifizierbarkeit() {
        assert!(Herkunft::Lokal < Herkunft::Bibliothek);
        assert!(Herkunft::Bibliothek < Herkunft::Verankert);
    }

    #[test]
    fn manifeste_ueberleben_borsh() {
        let s = skill(Herkunft::Bibliothek);
        let roh = borsh::to_vec(&s).expect("serialisieren");
        assert_eq!(borsh::from_slice::<Skillmanifest>(&roh).expect("lesen"), s);

        let w = werkzeug(Werkzeugart::Extern, Herkunft::Lokal);
        let roh = borsh::to_vec(&w).expect("serialisieren");
        assert_eq!(
            borsh::from_slice::<Werkzeugmanifest>(&roh).expect("lesen"),
            w
        );
    }
}
