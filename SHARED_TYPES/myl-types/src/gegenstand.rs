//! Das Format eines Gegenstands: Teile, Manifest, Ablage.
//!
//! # ⚑ Warum das hier liegt und nicht in `myl-store` (2026-08-31)
//!
//! **Ein gemeinsamer Vertrag gehört in die gemeinsame Kiste**, und zwar
//! aus demselben Grund wie beim Übergangs-Signaturvertrag zwei Tage
//! zuvor: Das Manifest wandert in den **Konsenszustand**, also muss der
//! Ledger es lesen können. `myl-ledger` an `myl-store` zu hängen hieße,
//! die ganze Store-Rolle an den Konsens zu hängen: Abruf, Auslieferung,
//! Rotation und später Netz-Ein- und -Ausgabe.
//!
//! Die Trennlinie ist damit: **das Format hier, die Rolle dort.** Was
//! zwei Halter ohne Absprache gleich sehen müssen, steht in dieser
//! Kiste; was ein Halter tut, steht in `myl-store`.
//!
//! Was ein gespeicherter Gegenstand ist: Teile, Hashes, Manifest.

use borsh::{BorshDeserialize, BorshSerialize};
use crate::hash::Hash;
use crate::ids::MerkleRoot;
use crate::merkle::{MerkleError, MerkleTree};

/// Feste Teilgröße in Bytes.
///
/// ⚑ **Fest und nicht wählbar, damit dieselbe Eingabe dasselbe Manifest
/// ergibt.** Wäre sie ein Parameter, hätte derselbe Gegenstand je nach
/// Aufrufer verschiedene Wurzeln, und zwei ehrliche Halter widersprächen
/// einander.
///
/// Ein Mebibyte: groß genug, dass die Zahl der Teile bei
/// Gigabyte-Gegenständen beherrschbar bleibt, klein genug, dass eine
/// Antwort im Verfügbarkeitsnachweis tragbar bleibt.
///
/// ⛑ **Hier stand bis zum 2026-08-30 das Gegenteil der Absicht:** „klein
/// genug, dass ein Verfügbarkeitsnachweis nicht ein Mebibyte Antwort
/// erzeugt". Der Satz beschreibt einen Nachweis, der ohne die Nutzdaten
/// auskommt, und ein solcher belegt keine Speicherung: Die Blätter des
/// Baums **sind** die Teil-Hashes, wer sie hält, antwortet für immer
/// richtig. Das ist Fund 106, siehe der Verfügbarkeitsnachweis in `myl-store`. Die Antwort
/// trägt seither den Teil selbst, und ein Mebibyte ist genau die Größe,
/// die das tragbar macht.
pub const TEILGROESSE: usize = 1024 * 1024;

/// Höchstzahl der Teile eines Gegenstands.
///
/// Bei [`TEILGROESSE`] sind das 64 GiB. Die Grenze steht, weil das
/// Manifest in den Konsenszustand wandert und seine Größe nicht an einer
/// Eingabe hängen darf.
pub const MAX_TEILE: usize = 65_536;

/// Wofür ein Gegenstand da ist.
///
/// ⚑ **Die Art entscheidet über die Redundanzform**, und deshalb steht
/// sie im Manifest und nicht in einer Konfigurationsdatei: Zwei Halter
/// müssen sich darüber einig sein, ohne sich abzusprechen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, BorshSerialize, BorshDeserialize)]
pub enum Gegenstandsart {
    /// Die Gewichte eines Modell-Shards. Groß, und **ganz** gelesen.
    Shardgewichte,
    /// Ein Skalenpaket. Klein (Größenordnung Megabyte).
    Skalenpaket,
    /// Ein Stück der Wissensdatenbank. **In kleinen Stücken gelesen**,
    /// mitten in einem Inferenzschritt.
    ///
    /// Eine **Einlage**: Sie hat einen Einleger, der für sie zahlt, und
    /// sie verfällt, wenn niemand mehr zahlt. Siehe
    /// das Speicherentgelt in `myl-store`.
    Wissensstueck,
    /// Sonstiges, das gehalten werden muss.
    Sonstiges,
    /// ⚑ **Netzwerkwissen: die Bibliothek, die immer verfügbar sein
    /// muss** (Festlegung des Projektinhabers, 2026-08-30).
    ///
    /// Kein privater Einlagerungsvorgang, sondern der gemeinsame
    /// Bestand: Er wird abgefragt **und speist das Training des
    /// Modells**. Deshalb hat er keinen Einleger, der ihn bezahlt, und
    /// deshalb darf er nicht verfallen.
    ///
    /// **Angehängt und nicht eingefügt**, damit die Marken der übrigen
    /// Arten ihre Borsh-Nummer behalten: Ein Manifest von gestern muss
    /// heute dasselbe bedeuten.
    Netzwerkwissen,
}

impl Gegenstandsart {
    /// Wo das Manifest dieser Art im Konsenszustand steht.
    ///
    /// Die Grenze wird nicht nur beschrieben, sondern **erzwungen**: Der
    /// Übergang, der einen Eintrag in den Zustand aufnimmt, weist die
    /// Wissensklassen mit benanntem Grund ab, statt sie stillschweigend
    /// wachsen zu lassen.
    pub fn ablage(self) -> Ablage {
        match self {
            Self::Shardgewichte | Self::Skalenpaket | Self::Sonstiges => Ablage::Direkt,
            Self::Wissensstueck | Self::Netzwerkwissen => Ablage::UeberWurzel,
        }
    }
}

/// Woraus die Vergütung eines Halters bezahlt wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finanzierung {
    /// Aus der Treasury: für alles, was vorliegen **muss** und keinen
    /// Einleger hat.
    Treasury,
    /// Aus dem Guthaben des Gegenstands, eingezahlt vom Einleger.
    Einleger,
}

impl Gegenstandsart {
    /// Wer die Vergütung für diese Art trägt.
    pub fn finanzierung(self) -> Finanzierung {
        match self {
            Self::Shardgewichte
            | Self::Skalenpaket
            | Self::Sonstiges
            | Self::Netzwerkwissen => Finanzierung::Treasury,
            Self::Wissensstueck => Finanzierung::Einleger,
        }
    }

    /// Ob ein Gegenstand dieser Art verfallen darf.
    ///
    /// **Genau die aus dem Guthaben eines Einlegers finanzierten.** Was
    /// die Allgemeinheit trägt, trägt sie, bis sie es abwählt; ein
    /// Verfall wäre dort ein stiller Verlust ohne Entscheidung.
    pub fn verfaellt(self) -> bool {
        matches!(self.finanzierung(), Finanzierung::Einleger)
    }
}

/// Wo das Manifest eines Gegenstands im Konsenszustand steht.
///
/// # ⚑ Warum es zwei Ablagen gibt und nicht eine
///
/// `LedgerState::commitment()` **serialisiert den ganzen Zustand und
/// hasht ihn**. Es gibt keinen Baum mit Teilbeweisen, sondern eine
/// Bytefolge über alles. Jede Zustandsänderung kostet damit
/// O(Zustandsgröße), und zwar je Block.
///
/// Daraus folgt unmittelbar: **Eine Menge, die unbegrenzt wächst, darf
/// nicht einzeln im Zustand stehen.** Die Wissensdatenbank wächst mit
/// der Nutzung; stünde jedes ihrer Manifeste dort, würde jeder Block die
/// ganze Datenbank serialisieren und hashen. Das ist keine Vorliebe,
/// sondern die Bauart des Commitments.
///
/// # Warum umgekehrt nicht alles über eine Wurzel läuft
///
/// **Ein beitretender Miner braucht die Shardgewichte, bevor er
/// irgendetwas beweisen kann.** Sie müssen ohne Beweis auffindbar sein,
/// sonst braucht der Beitritt genau das, was der Beitritt erst
/// herstellt. Die Infrastruktur ist zugleich klein und wächst nur durch
/// Governance-Akte, also ist sie im Zustand gut aufgehoben.
///
/// Dazu kommt eine schlichte Tatsache: `myl-types::merkle` baut Bäume
/// **statisch** aus einer Blattfolge. Einen Baum mit Aktualisierung gibt
/// es nicht, und ihn für eine Menge mit heute null Einträgen zu bauen,
/// wäre Maschinerie vor dem Bedarf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ablage {
    /// Das Manifest steht einzeln im Ledger-Zustand.
    ///
    /// Nur für Klassen, deren Zahl durch Governance begrenzt ist.
    Direkt,
    /// Nur eine Wurzel steht im Zustand, das einzelne Manifest wird
    /// gegen sie bewiesen.
    ///
    /// Für die Wissensdatenbank, deren Umfang an der Nutzung hängt.
    /// Die Wurzel ist κ_v, siehe [`myl-store`s κ_v]. **Der Weg dorthin ist
    /// heute nicht gebaut**, und das ist Absicht: Die Menge hat null
    /// Einträge, und die Aufnahme in sie ist ein Governance-Akt, der
    /// ebenfalls noch aussteht.
    UeberWurzel,
}

/// Wie ein Gegenstand vervielfältigt wird.
///
/// ⚑ **Welche Form für welche Art richtig ist, steht bewusst nicht
/// hier.** Die Frage ist Latenz gegen Platz, und beide Zahlen fehlen,
/// solange es keinen echten Abrufverkehr gibt. Was entschieden ist: Die
/// Wahl gehört in die Governance-Registry und ist **je Art**
/// einstellbar, damit sie der Messung folgen kann statt ihr
/// vorauszugehen.
///
/// **Eine Zahl vom Schreibtisch wäre hier keine Entscheidung, sondern
/// eine Behauptung mit Nachkommastelle.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Redundanzform {
    /// `n` vollständige Kopien. Ein Abruf braucht **einen** Halter.
    Kopien {
        /// Wie viele.
        anzahl: u8,
    },
    /// Reed-Solomon: `k` Datenfragmente, `m` Paritätsfragmente. Ein
    /// Abruf braucht **`k`** Halter, kostet dafür nur `(k+m)/k` Platz.
    Erasure {
        /// Datenfragmente.
        k: u8,
        /// Paritätsfragmente.
        m: u8,
    },
}

impl Redundanzform {
    /// Wie viele Halter ein vollständiger Abruf braucht.
    pub fn halter_je_abruf(&self) -> u32 {
        match self {
            Self::Kopien { .. } => 1,
            Self::Erasure { k, .. } => *k as u32,
        }
    }

    /// Wie viele Halter einem Gegenstand dieser Form **zugeteilt**
    /// werden.
    ///
    /// ⚑ **Nicht zu verwechseln mit [`Self::halter_je_abruf`].** Das
    /// eine ist die Untergrenze für einen vollständigen **Abruf**, das
    /// andere die Zahl der Halter, die es überhaupt geben muss. Bei
    /// Erasure k=8/m=6 sind das 8 gegen 14; wer das eine für das andere
    /// nimmt, teilt sechs Halter zu wenig zu und merkt es erst, wenn
    /// sechs ausfallen.
    pub fn halterzahl(&self) -> u32 {
        match self {
            Self::Kopien { anzahl } => *anzahl as u32,
            Self::Erasure { k, m } => (*k as u32) + (*m as u32),
        }
    }

    /// Wie viele Bytes ein einzelner Halter von `laenge` trägt.
    ///
    /// Bei Kopien die ganze Länge, bei Erasure ein Fragment. **Aufgerundet**:
    /// Ein Fragment, das rechnerisch 0,4 Bytes groß wäre, belegt trotzdem
    /// eines, und ein zu klein gerechneter Platzbedarf führt zu einer
    /// Zuteilung, die nicht passt.
    pub fn anteil_je_halter(&self, laenge: u64) -> u64 {
        match self {
            Self::Kopien { .. } => laenge,
            Self::Erasure { k, .. } => {
                let k = (*k as u64).max(1);
                laenge.div_ceil(k)
            }
        }
    }

    /// Wie viele Verluste die Form übersteht.
    pub fn vertraegt_verluste(&self) -> u32 {
        match self {
            Self::Kopien { anzahl } => anzahl.saturating_sub(1) as u32,
            Self::Erasure { m, .. } => *m as u32,
        }
    }

    /// Der Platzbedarf als Bruch, Zähler und Nenner.
    ///
    /// Ganzzahlig, weil dieselbe Zahl in Vergütung und Invarianten
    /// eingeht und dort kein Gleitkomma vorkommen darf.
    pub fn platz(&self) -> (u32, u32) {
        match self {
            Self::Kopien { anzahl } => (*anzahl as u32, 1),
            Self::Erasure { k, m } => ((*k as u32) + (*m as u32), *k as u32),
        }
    }
}

/// Ein Teil eines Gegenstands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Teil {
    /// Der wievielte, von null an.
    pub index: u32,
    /// Hash über den **Klartext** dieses Teils.
    pub klartext: Hash,
    /// Wie viele Bytes wirklich darin stehen.
    ///
    /// Der letzte Teil ist kürzer als [`TEILGROESSE`]. ⚑ **Ohne dieses
    /// Feld wäre er von einem aufgefüllten nicht zu unterscheiden**, und
    /// Auffüllen ist genau die Stelle, an der zwei Halter dieselbe
    /// Wurzel für verschiedene Inhalte bekämen.
    pub laenge: u32,
}

/// Warum ein Gegenstand kein Manifest bekommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestFehler {
    /// Nichts zu speichern.
    Leer,
    /// Mehr als [`MAX_TEILE`] Teile.
    ZuGross {
        /// Wie viele es wären.
        teile: usize,
    },
    /// Der Merkle-Baum ließ sich nicht bauen.
    Baum(MerkleError),
    /// Die Redundanzform ergibt keinen Sinn.
    Redundanz,
}

impl std::fmt::Display for ManifestFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Leer => write!(f, "leerer Gegenstand"),
            Self::ZuGross { teile } => write!(f, "{teile} Teile, höchstens {MAX_TEILE}"),
            Self::Baum(e) => write!(f, "Merkle-Baum: {e}"),
            Self::Redundanz => write!(f, "unbrauchbare Redundanzform"),
        }
    }
}

impl std::error::Error for ManifestFehler {}

/// Zerlegt einen Gegenstand in Teile.
///
/// ⚑ **Über den Klartext, in fester Größe, in kanonischer Reihenfolge.**
/// Alle drei zusammen ergeben die Zusage der Phase: Dieselbe Eingabe
/// ergibt dasselbe Manifest, Byte für Byte, unabhängig von Kompressor,
/// Dateisystem und Lesereihenfolge.
pub fn teile_bilden(daten: &[u8]) -> Result<Vec<Teil>, ManifestFehler> {
    if daten.is_empty() {
        return Err(ManifestFehler::Leer);
    }
    let anzahl = daten.len().div_ceil(TEILGROESSE);
    if anzahl > MAX_TEILE {
        return Err(ManifestFehler::ZuGross { teile: anzahl });
    }
    Ok(daten
        .chunks(TEILGROESSE)
        .enumerate()
        .map(|(i, stueck)| Teil {
            index: i as u32,
            klartext: Hash::sha256(stueck),
            laenge: stueck.len() as u32,
        })
        .collect())
}

/// Was über einen Gegenstand im Konsens steht.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Manifest {
    /// Wofür der Gegenstand da ist.
    pub art: Gegenstandsart,
    /// Fassung des Gegenstands selbst.
    pub fassung: u32,
    /// Wie viele Teile.
    pub teilzahl: u32,
    /// Merkle-Wurzel über die Teil-Hashes.
    pub wurzel: MerkleRoot,
    /// Wie vervielfältigt wird.
    pub redundanz: Redundanzform,
    /// Gesamtlänge des Klartexts in Bytes.
    pub laenge: u64,
}

impl Manifest {
    /// Baut das Manifest aus den Teilen.
    ///
    /// **Die Wurzel geht über die Klartext-Hashes**, nicht über die
    /// Teile als Struktur: Ein Prüfer, der ein Teil hat, rechnet dessen
    /// Hash aus und braucht den Pfad, nicht die Kodierung des
    /// Nachbarteils.
    pub fn neu(
        art: Gegenstandsart,
        fassung: u32,
        teile: &[Teil],
        redundanz: Redundanzform,
    ) -> Result<Self, ManifestFehler> {
        if teile.is_empty() {
            return Err(ManifestFehler::Leer);
        }
        match redundanz {
            Redundanzform::Kopien { anzahl: 0 } => return Err(ManifestFehler::Redundanz),
            Redundanzform::Erasure { k, m } if k == 0 || m == 0 => {
                return Err(ManifestFehler::Redundanz)
            }
            _ => {}
        }
        let blaetter: Vec<[u8; 32]> = teile.iter().map(|t| t.klartext.0).collect();
        let refs: Vec<&[u8]> = blaetter.iter().map(|b| b.as_slice()).collect();
        let baum = MerkleTree::new(&refs).map_err(ManifestFehler::Baum)?;
        Ok(Self {
            art,
            fassung,
            teilzahl: teile.len() as u32,
            wurzel: MerkleRoot::new(baum.root().0),
            redundanz,
            laenge: teile.iter().map(|t| t.laenge as u64).sum(),
        })
    }

    /// Wie viele Halter ein Gegenstand mindestens braucht, damit ein
    /// vollständiger Abruf möglich bleibt.
    pub fn mindesthalter(&self) -> u32 {
        self.redundanz.halter_je_abruf()
    }
}

#[cfg(test)]
mod ablage_tests {
    use super::*;

    /// Jede Art hat eine Ablage, und die beiden Wissensklassen sind
    /// genau die über eine Wurzel.
    #[test]
    fn jede_art_hat_eine_ablage_und_wissen_geht_ueber_die_wurzel() {
        for (art, erwartet) in [
            (Gegenstandsart::Shardgewichte, Ablage::Direkt),
            (Gegenstandsart::Skalenpaket, Ablage::Direkt),
            (Gegenstandsart::Sonstiges, Ablage::Direkt),
            (Gegenstandsart::Wissensstueck, Ablage::UeberWurzel),
            (Gegenstandsart::Netzwerkwissen, Ablage::UeberWurzel),
        ] {
            assert_eq!(art.ablage(), erwartet, "{art:?}");
        }
    }

    /// ⚑ **Was direkt im Zustand steht, trägt die Allgemeinheit.**
    ///
    /// Daraus folgt, dass das Speicherregister im Ledger **kein
    /// Guthaben** führt, und das ist kein Vergessen: Ein Guthaben
    /// braucht nur, was ein Einleger bezahlt, und das läuft über die
    /// Wurzel.
    ///
    /// Der Test steht hier, damit eine künftig ergänzte Art nicht still
    /// in die Lücke fällt. Wer eine Art als `Direkt` und zugleich
    /// `Einleger` einträgt, bekommt hier einen Fehlschlag und muss sich
    /// entscheiden, statt ein Guthaben zu erfinden, das niemand führt.
    #[test]
    fn was_direkt_im_zustand_steht_traegt_die_allgemeinheit() {
        for art in [
            Gegenstandsart::Shardgewichte,
            Gegenstandsart::Skalenpaket,
            Gegenstandsart::Wissensstueck,
            Gegenstandsart::Sonstiges,
            Gegenstandsart::Netzwerkwissen,
        ] {
            if art.ablage() == Ablage::Direkt {
                assert_eq!(
                    art.finanzierung(),
                    Finanzierung::Treasury,
                    "{art:?} steht direkt im Zustand und braeuchte ein Guthaben"
                );
            }
        }
    }

    /// ⚑ **Der Grund hinter der Zuordnung, als Zusicherung.**
    ///
    /// Direkt in den Zustand darf nur, was durch einen Governance-Akt
    /// hinzukommt und damit begrenzt ist. Was durch **Nutzung** wächst,
    /// darf es nicht, weil `commitment()` den ganzen Zustand
    /// serialisiert und hasht: Jeder Block zahlte sonst für die ganze
    /// Wissensdatenbank.
    ///
    /// Ohne diesen Test wäre die Zuordnung eine Liste, die jemand
    /// erweitert, ohne den Grund zu kennen.
    #[test]
    fn was_durch_nutzung_waechst_steht_nicht_einzeln_im_zustand() {
        let waechst_mit_der_nutzung = [
            Gegenstandsart::Wissensstueck,
            Gegenstandsart::Netzwerkwissen,
        ];
        for art in waechst_mit_der_nutzung {
            assert_eq!(
                art.ablage(),
                Ablage::UeberWurzel,
                "{art:?} waechst mit der Nutzung und darf nicht einzeln \
                 im Zustand stehen"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use borsh::{from_slice, to_vec};

    fn daten(n: usize, muster: u8) -> Vec<u8> {
        (0..n).map(|i| muster.wrapping_add(i as u8)).collect()
    }

    /// ⚑ **Das Akzeptanzkriterium der Phase, als Test:** Dieselbe
    /// Eingabe ergibt dasselbe Manifest, Byte für Byte.
    #[test]
    fn dieselbe_eingabe_ergibt_dasselbe_manifest() {
        let d = daten(TEILGROESSE * 2 + 500, 7);
        let a = Manifest::neu(
            Gegenstandsart::Wissensstueck,
            1,
            &teile_bilden(&d).expect("Teile"),
            Redundanzform::Kopien { anzahl: 3 },
        )
        .expect("Manifest");
        let b = Manifest::neu(
            Gegenstandsart::Wissensstueck,
            1,
            &teile_bilden(&d).expect("Teile"),
            Redundanzform::Kopien { anzahl: 3 },
        )
        .expect("Manifest");
        assert_eq!(a, b);
        assert_eq!(to_vec(&a).expect("ser"), to_vec(&b).expect("ser"));
        assert_eq!(a.teilzahl, 3);
        assert_eq!(a.laenge, (TEILGROESSE * 2 + 500) as u64);
    }

    /// ⚑ **Die Gegenprobe zu Entscheidung 1.** Gehasht wird der
    /// Klartext, also ändert eine andere Kodierung derselben Bytes
    /// nichts. Der Test spielt das nach, indem er dieselben Bytes über
    /// zwei verschiedene Wege einliest.
    #[test]
    fn die_kodierung_aendert_die_wurzel_nicht() {
        let d = daten(TEILGROESSE + 3, 42);
        // „Kompressor A": in einem Stück.
        let direkt = teile_bilden(&d).expect("Teile");
        // „Kompressor B": stückweise zusammengesetzt, andere
        // Puffergrenzen, dieselben Bytes.
        let mut zusammengesetzt = Vec::new();
        for stueck in d.chunks(7919) {
            zusammengesetzt.extend_from_slice(stueck);
        }
        let indirekt = teile_bilden(&zusammengesetzt).expect("Teile");
        assert_eq!(direkt, indirekt);

        let m = |t: &[Teil]| {
            Manifest::neu(Gegenstandsart::Shardgewichte, 1, t, Redundanzform::Erasure { k: 8, m: 6 })
                .expect("Manifest")
        };
        assert_eq!(m(&direkt).wurzel, m(&indirekt).wurzel);
    }

    /// Ein verändertes Byte in einem beliebigen Teil ändert die Wurzel.
    #[test]
    fn ein_verändertes_byte_aendert_die_wurzel() {
        let d = daten(TEILGROESSE * 3, 1);
        let form = Redundanzform::Kopien { anzahl: 2 };
        let vorher = Manifest::neu(
            Gegenstandsart::Sonstiges, 1, &teile_bilden(&d).expect("t"), form,
        )
        .expect("m");

        // Je einmal im ersten, mittleren und letzten Teil.
        for stelle in [0usize, TEILGROESSE + 5, TEILGROESSE * 3 - 1] {
            let mut geaendert = d.clone();
            geaendert[stelle] ^= 0xFF;
            let nachher = Manifest::neu(
                Gegenstandsart::Sonstiges, 1, &teile_bilden(&geaendert).expect("t"), form,
            )
            .expect("m");
            assert_ne!(vorher.wurzel, nachher.wurzel, "Stelle {stelle}");
        }
    }

    /// ⚑ Der letzte Teil ist kürzer, und seine Länge steht dabei.
    /// **Ohne sie wäre er von einem aufgefüllten nicht zu
    /// unterscheiden**, und Auffüllen ist die Stelle, an der zwei
    /// Inhalte dieselbe Wurzel bekämen.
    #[test]
    fn der_letzte_teil_traegt_seine_laenge() {
        let teile = teile_bilden(&daten(TEILGROESSE + 10, 3)).expect("t");
        assert_eq!(teile.len(), 2);
        assert_eq!(teile[0].laenge, TEILGROESSE as u32);
        assert_eq!(teile[1].laenge, 10);

        // Genau ein Teil voll: kein zweiter, leerer.
        let genau = teile_bilden(&daten(TEILGROESSE, 3)).expect("t");
        assert_eq!(genau.len(), 1);
        assert_eq!(genau[0].laenge, TEILGROESSE as u32);
    }

    #[test]
    fn leeres_und_zu_grosses_wird_abgewiesen() {
        assert_eq!(teile_bilden(&[]), Err(ManifestFehler::Leer));
        assert_eq!(
            Manifest::neu(Gegenstandsart::Sonstiges, 1, &[], Redundanzform::Kopien { anzahl: 1 }),
            Err(ManifestFehler::Leer)
        );
        let t = teile_bilden(&daten(100, 1)).expect("t");
        assert_eq!(
            Manifest::neu(Gegenstandsart::Sonstiges, 1, &t, Redundanzform::Kopien { anzahl: 0 }),
            Err(ManifestFehler::Redundanz)
        );
        assert_eq!(
            Manifest::neu(Gegenstandsart::Sonstiges, 1, &t, Redundanzform::Erasure { k: 8, m: 0 }),
            Err(ManifestFehler::Redundanz)
        );
    }

    /// ⚑ Die Zahlen hinter Entscheidung 3, als Test statt als Behauptung
    /// im Fließtext: **Sieben Kopien und k=8/m=6 überstehen dieselben
    /// sechs Verluste, und der Platzbedarf unterscheidet sich um das
    /// Vierfache.**
    #[test]
    fn kopien_und_erasure_kosten_verschieden_viel_fuer_dasselbe() {
        let kopien = Redundanzform::Kopien { anzahl: 7 };
        let erasure = Redundanzform::Erasure { k: 8, m: 6 };

        assert_eq!(kopien.vertraegt_verluste(), 6);
        assert_eq!(erasure.vertraegt_verluste(), 6);

        assert_eq!(kopien.platz(), (7, 1));
        assert_eq!(erasure.platz(), (14, 8)); // 1,75-fach

        // ⚑ Und der Preis dafür steht auf der anderen Seite: Ein Abruf
        // braucht einen Halter gegen acht.
        assert_eq!(kopien.halter_je_abruf(), 1);
        assert_eq!(erasure.halter_je_abruf(), 8);
    }

    #[test]
    fn borsh_ist_ein_rundweg() {
        let t = teile_bilden(&daten(2000, 9)).expect("t");
        let m = Manifest::neu(
            Gegenstandsart::Skalenpaket, 3, &t, Redundanzform::Erasure { k: 8, m: 4 },
        )
        .expect("m");
        let zurueck: Manifest = from_slice(&to_vec(&m).expect("ser")).expect("de");
        assert_eq!(m, zurueck);
        assert_eq!(zurueck.mindesthalter(), 8);
    }
}
