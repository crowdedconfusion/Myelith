//! Wer welchen Gegenstand hält: die deterministische Zuteilung.
//!
//! # Wozu
//!
//! Der Verfügbarkeitsnachweis fragt einen **Halter** nach einem Teil.
//! Solange niemand sagt, wer Halter ist, gibt es niemanden zu fragen.
//! Diese Zuteilung schließt die Lücke zwischen der Kapazitätszusage (wer
//! bietet wie viel Platz) und dem Register (welche Gegenstände es gibt).
//!
//! # ⚑ Sie wird gerechnet, nicht gespeichert
//!
//! Aus Register, Zusagen und Epochenseed ergibt sich dieselbe Zuteilung
//! bei jedem, der sie ausrechnet. Sie muss deshalb **nicht in den
//! Zustand**, und das ist wichtig: Der Zustandshash entsteht über eine
//! Serialisierung des ganzen Zustands, und eine Zuteilung über Tausende
//! Teile würde ihn je Epoche neu und groß machen.
//!
//! Aus demselben Grund liegt der Code hier und nicht in `myl-store`:
//! **Wer eine Abrechnung prüft, muss die Zuteilung nachrechnen können**,
//! ohne an der Store-Rolle zu hängen. Wer sie nur entgegennimmt,
//! überlässt dem Einreicher die Wahl, wer bezahlt wird, und das ist
//! genau der Fehler aus Fund 96.
//!
//! # Warum je Gegenstand ein eigener Seed
//!
//! Derselbe Grund wie beim Pod-Shuffle im Scheduler: Mit dem blanken
//! Epochenseed bekäme **jeder** Gegenstand dieselbe Reihenfolge, und
//! damit liefen immer dieselben Halter zuerst voll. Der Seed wird
//! deshalb aus Epochenseed und Wurzel abgeleitet.
//!
//! # ⚑ Was fehlender Platz auslöst
//!
//! Er wird **gemeldet, nicht verschwiegen**. `assign_redundant_pods`
//! überging fehlende Metadaten stillschweigend, und der Rückgabewert
//! sagte nicht, dass die Diversität ungeprüft blieb; das war ein
//! offener Punkt, bis es einer wurde. Hier steht die Unterbesetzung im
//! Ergebnis, mit Zahl und Grund.
//!
//! # Was hier noch nicht geschieht
//!
//! **Keine Diversitätsbedingung** über Geo-Zone und ASN (STORAGE 2.2),
//! **keine Rotation** (2.3) und **keine Nachbesetzung** bei
//! Unterschreitung (2.4). Alle drei setzen diese Zuteilung voraus und
//! kommen darauf; keiner davon ist hier stillschweigend weggelassen.

use crate::gegenstand::Manifest;
use crate::ids::{MerkleRoot, MinerId};
use crate::seed_rng::deterministic_shuffle;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Trennstring der Zuteilungs-Ableitung.
pub const DST_ZUTEILUNG: &[u8] = b"MYELITH_SPEICHER_ZUTEILUNG_v1";

/// Ein Halter mit dem Platz, den er zugesagt hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platzangebot {
    /// Wer anbietet.
    pub halter: MinerId,
    /// Wie viele Bytes noch frei sind.
    pub frei_bytes: u64,
}

/// Ein Gegenstand, für den der Platz nicht reichte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unterbesetzung {
    /// Welcher Gegenstand.
    pub wurzel: MerkleRoot,
    /// So viele Halter hätte er gebraucht.
    pub gebraucht: u32,
    /// So viele haben Platz gehabt.
    pub bekommen: u32,
}

/// Das Ergebnis einer Zuteilung.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Zuteilung {
    /// Je Gegenstand die zugeteilten Halter, aufsteigend geordnet.
    pub je_gegenstand: BTreeMap<MerkleRoot, Vec<MinerId>>,
    /// Gegenstände, die zu wenige Halter bekommen haben.
    ///
    /// **Leer heißt vollständig besetzt.** Ein Aufrufer, der das Feld
    /// übergeht, bekommt eine Zuteilung, die aussieht wie eine
    /// vollständige.
    pub unterbesetzt: Vec<Unterbesetzung>,
}

impl Zuteilung {
    /// Die Halter eines Gegenstands, falls er zugeteilt wurde.
    pub fn halter(&self, wurzel: &MerkleRoot) -> &[MinerId] {
        self.je_gegenstand
            .get(wurzel)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Ist jeder Gegenstand vollständig besetzt?
    pub fn vollstaendig(&self) -> bool {
        self.unterbesetzt.is_empty()
    }
}

/// Trennstring der Stichproben-Ableitung.
pub const DST_SPEICHER_STICHPROBE: &[u8] = b"MYELITH_SPEICHER_STICHPROBE_v1";

/// Welchen Teil ein Halter in dieser Epoche vorlegen muss.
///
/// # ⚑ Warum das hier liegt und nicht bei der Antwort
///
/// Die Antwort braucht die Bytes und gehört deshalb zum Halter, also in
/// `myl-store`. **Die Frage braucht sie nicht:** Sie folgt aus Wurzel,
/// Fassung, Epoche und Halterkennung, und jeder muss dieselbe ausrechnen
/// können, der eine Quittung prüft. Zwei Fassungen derselben Ableitung,
/// eine beim Halter und eine beim Prüfer, wären zwei Quellen für
/// dieselbe Wahrheit und trennten sich beim ersten Formatwechsel.
///
/// # Warum die Halterkennung eingeht
///
/// Sonst würden alle Halter nach demselben Teil gefragt, und **einer mit
/// den Daten genügte**: Die übrigen schrieben seine Antwort ab und
/// bekämen Speicherentgelt für Speicher, den nur einer hat.
pub fn verlangter_teil(
    manifest: &Manifest,
    epoche: crate::ids::EpochId,
    halter: &MinerId,
    seed: &[u8; 32],
) -> u32 {
    let mut vorlage = Vec::with_capacity(DST_SPEICHER_STICHPROBE.len() + 32 * 3 + 16);
    vorlage.extend_from_slice(DST_SPEICHER_STICHPROBE);
    vorlage.extend_from_slice(seed);
    vorlage.extend_from_slice(manifest.wurzel.as_bytes());
    vorlage.extend_from_slice(&manifest.fassung.to_le_bytes());
    vorlage.extend_from_slice(&epoche.0.to_le_bytes());
    vorlage.extend_from_slice(halter.as_bytes());

    let abgeleitet = crate::hash::Hash::sha256(&vorlage);
    let mut rng = crate::seed_rng::SeedRng::new(&abgeleitet.0);
    // `teilzahl` ist nach `Manifest::neu` niemals null.
    rng.next_below(u64::from(manifest.teilzahl.max(1))) as u32
}

/// Leitet den Seed für einen einzelnen Gegenstand ab.
fn gegenstands_seed(seed: &[u8; 32], wurzel: &MerkleRoot) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(DST_ZUTEILUNG);
    h.update(seed);
    h.update(wurzel.as_bytes());
    let d = h.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&d);
    out
}

/// Teilt Gegenstände auf Halter zu.
///
/// **Deterministisch**: Dieselben Eingaben ergeben dieselbe Ausgabe, auf
/// jedem Rechner. Die Gegenstände werden in kanonischer Reihenfolge
/// bearbeitet (aufsteigend nach Wurzel), damit das Ergebnis nicht an der
/// Reihenfolge der Eingabe hängt.
///
/// Der Platz eines Halters wird beim Zuteilen abgezogen; wer voll ist,
/// bekommt nichts mehr.
pub fn zuteilen(
    gegenstaende: &BTreeMap<MerkleRoot, Manifest>,
    angebote: &[Platzangebot],
    seed: &[u8; 32],
) -> Zuteilung {
    let mut frei: BTreeMap<MinerId, u64> = angebote
        .iter()
        .map(|a| (a.halter, a.frei_bytes))
        .collect();

    let mut ergebnis = Zuteilung::default();

    for (wurzel, manifest) in gegenstaende {
        let gebraucht = manifest.redundanz.halterzahl();
        let anteil = manifest.redundanz.anteil_je_halter(manifest.laenge);

        // Reihenfolge je Gegenstand, sonst laufen immer dieselben voll.
        let mut reihenfolge: Vec<MinerId> = frei.keys().copied().collect();
        deterministic_shuffle(&mut reihenfolge, &gegenstands_seed(seed, wurzel));

        let mut gewaehlt: Vec<MinerId> = Vec::with_capacity(gebraucht as usize);
        for halter in reihenfolge {
            if gewaehlt.len() as u32 == gebraucht {
                break;
            }
            let Some(platz) = frei.get_mut(&halter) else {
                continue;
            };
            if *platz < anteil {
                continue;
            }
            *platz -= anteil;
            gewaehlt.push(halter);
        }

        if (gewaehlt.len() as u32) < gebraucht {
            ergebnis.unterbesetzt.push(Unterbesetzung {
                wurzel: *wurzel,
                gebraucht,
                bekommen: gewaehlt.len() as u32,
            });
        }
        // ⛑ **Aufsteigend, und der Grund ist nicht der, der hier zuerst
        // stand.** „Damit zwei Rechner dieselbe Liste ausgeben" war
        // falsch: Das tun sie ohnehin, denn `reihenfolge` kommt aus
        // einer `BTreeMap` und der Shuffle ist deterministisch. Eine
        // Gegenprobe ohne diese Zeile fiel durch keinen einzigen Test.
        //
        // Der echte Grund ist Vorsorge: Sortiert ist die Ausgabe
        // **kanonisch unabhängig vom Auswahlverfahren**. Wer die
        // Schleife später ändert, etwa gewichtet zieht oder parallel
        // arbeitet, ändert damit nicht stillschweigend die Reihenfolge
        // einer Liste, die anderswo verglichen oder gehasht wird.
        gewaehlt.sort_unstable();
        ergebnis.je_gegenstand.insert(*wurzel, gewaehlt);
    }

    ergebnis
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gegenstand::{Gegenstandsart, Redundanzform};

    fn wurzel(b: u8) -> MerkleRoot {
        MerkleRoot::new([b; 32])
    }

    fn halter(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }

    /// Ein Manifest mit gewählter Länge und Redundanzform. Die Felder
    /// sind öffentlich; ein echter Gegenstand wäre für diese Frage
    /// unnötig groß.
    fn gegenstand(b: u8, laenge: u64, form: Redundanzform) -> (MerkleRoot, Manifest) {
        let w = wurzel(b);
        (
            w,
            Manifest {
                art: Gegenstandsart::Shardgewichte,
                fassung: 1,
                teilzahl: 1,
                wurzel: w,
                redundanz: form,
                laenge,
            },
        )
    }

    fn angebote(n: u8, frei: u64) -> Vec<Platzangebot> {
        (0..n)
            .map(|i| Platzangebot {
                halter: halter(i),
                frei_bytes: frei,
            })
            .collect()
    }

    #[test]
    fn dieselbe_eingabe_ergibt_dieselbe_zuteilung() {
        let mut g = BTreeMap::new();
        let (w, m) = gegenstand(1, 1000, Redundanzform::Kopien { anzahl: 3 });
        g.insert(w, m);
        let a = angebote(6, 10_000);
        assert_eq!(zuteilen(&g, &a, &[7u8; 32]), zuteilen(&g, &a, &[7u8; 32]));
    }

    /// ⚑ **Die Reihenfolge der Angebote darf nichts ändern.**
    ///
    /// Sie kommt aus einer Sammlung, deren Ordnung niemand zusichert.
    /// Hinge das Ergebnis daran, bekämen zwei Knoten verschiedene
    /// Zuteilungen und stritten über eine Abrechnung, ohne dass einer
    /// gelogen hätte.
    ///
    /// **Heute hält das die `BTreeMap`**, in die die Angebote laufen;
    /// der Test besteht also, ohne dass jemand etwas dafür tut. Er bleibt
    /// trotzdem stehen: Wer die Sammlung später gegen einen `Vec`
    /// tauscht, bekommt hier den Fehlschlag.
    #[test]
    fn die_reihenfolge_der_angebote_aendert_nichts() {
        let mut g = BTreeMap::new();
        let (w, m) = gegenstand(1, 1000, Redundanzform::Kopien { anzahl: 3 });
        g.insert(w, m);
        let vorwaerts = angebote(6, 10_000);
        let mut rueckwaerts = vorwaerts.clone();
        rueckwaerts.reverse();
        assert_eq!(
            zuteilen(&g, &vorwaerts, &[7u8; 32]),
            zuteilen(&g, &rueckwaerts, &[7u8; 32])
        );
    }

    #[test]
    fn jeder_gegenstand_bekommt_seine_halterzahl() {
        let mut g = BTreeMap::new();
        for i in 1..=3u8 {
            let (w, m) = gegenstand(i, 1000, Redundanzform::Kopien { anzahl: 3 });
            g.insert(w, m);
        }
        let z = zuteilen(&g, &angebote(9, 10_000), &[3u8; 32]);
        assert!(z.vollstaendig());
        for i in 1..=3u8 {
            assert_eq!(z.halter(&wurzel(i)).len(), 3, "Gegenstand {i}");
        }
    }

    /// ⚑ **Zu wenig Platz wird gemeldet, nicht verschwiegen.**
    ///
    /// Der Vorgänger `assign_redundant_pods` überging fehlende
    /// Metadaten stillschweigend, und der Rückgabewert sagte nicht, dass
    /// etwas ungeprüft blieb. Eine Zuteilung, die zu wenige Halter
    /// findet und trotzdem wie eine vollständige aussieht, ist
    /// dieselbe Falle.
    #[test]
    fn zu_wenig_platz_wird_gemeldet_nicht_verschwiegen() {
        let mut g = BTreeMap::new();
        let (w, m) = gegenstand(1, 1000, Redundanzform::Kopien { anzahl: 5 });
        g.insert(w, m);
        // Nur zwei Halter haben ueberhaupt genug Platz.
        let a = vec![
            Platzangebot { halter: halter(1), frei_bytes: 1000 },
            Platzangebot { halter: halter(2), frei_bytes: 1000 },
            Platzangebot { halter: halter(3), frei_bytes: 999 },
        ];
        let z = zuteilen(&g, &a, &[1u8; 32]);
        assert!(!z.vollstaendig());
        assert_eq!(
            z.unterbesetzt,
            vec![Unterbesetzung { wurzel: w, gebraucht: 5, bekommen: 2 }]
        );
        assert_eq!(z.halter(&w).len(), 2);
    }

    /// Ohne Angebote ist nichts besetzt, und das steht im Ergebnis.
    #[test]
    fn ohne_angebote_ist_alles_unterbesetzt() {
        let mut g = BTreeMap::new();
        let (w, m) = gegenstand(1, 1000, Redundanzform::Kopien { anzahl: 3 });
        g.insert(w, m);
        let z = zuteilen(&g, &[], &[1u8; 32]);
        assert!(!z.vollstaendig());
        assert_eq!(z.halter(&w), &[] as &[MinerId]);
    }

    /// ⚑ **Je Gegenstand ein eigener Seed, und zwar wirksam.**
    ///
    /// Mit dem blanken Epochenseed bekäme jeder Gegenstand **dieselbe**
    /// Reihenfolge, und bei ausreichendem Platz träfe die Auswahl damit
    /// zwangsläufig dieselben Halter. Genau das prüft dieser Test: Zwei
    /// Gegenstände, genug Platz für beide bei jedem, und die
    /// Haltermengen dürfen nicht gleich sein.
    ///
    /// ⛑ **Hier stand zuerst ein Test auf die Ableitungsfunktion
    /// selbst.** Der blieb grün, als der Aufruf versuchsweise durch den
    /// blanken Seed ersetzt wurde: Er prüfte, dass das Werkzeug
    /// funktioniert, nicht dass es benutzt wird. Dasselbe Muster wie bei
    /// Fund 42, wo drei grüne Tests neben einer Lücke standen.
    #[test]
    fn der_eigene_seed_je_gegenstand_wirkt_wirklich() {
        let mut g = BTreeMap::new();
        for i in 1..=2u8 {
            let (w, m) = gegenstand(i, 1000, Redundanzform::Kopien { anzahl: 3 });
            g.insert(w, m);
        }
        // Platz fuer beide Gegenstaende bei jedem Halter: Die Auswahl
        // haengt allein an der Reihenfolge, nicht am Platz.
        let z = zuteilen(&g, &angebote(12, 10_000), &[5u8; 32]);
        assert!(z.vollstaendig());
        assert_ne!(
            z.halter(&wurzel(1)),
            z.halter(&wurzel(2)),
            "beide Gegenstaende trafen dieselben Halter: der Seed wird \
             nicht je Gegenstand abgeleitet"
        );
    }

    /// Und die Ableitung selbst, als Ergänzung: verschieden je Wurzel,
    /// nicht der blanke Seed, und wiederholbar.
    #[test]
    fn die_seed_ableitung_ist_verschieden_und_wiederholbar() {
        let seed = [5u8; 32];
        let a = gegenstands_seed(&seed, &wurzel(1));
        let b = gegenstands_seed(&seed, &wurzel(2));
        assert_ne!(a, b);
        assert_ne!(a, seed);
        assert_eq!(a, gegenstands_seed(&seed, &wurzel(1)));
    }

    /// Die Halterliste ist aufsteigend, unabhängig davon, in welcher
    /// Reihenfolge die Auswahl sie gefunden hat.
    ///
    /// ⛑ Ohne diesen Test fiel die Sortierung durch **keine** einzige
    /// Gegenprobe, und ihr Kommentar behauptete einen Grund, den sie
    /// nicht hatte.
    #[test]
    fn die_halterliste_ist_kanonisch_geordnet() {
        let mut g = BTreeMap::new();
        for i in 1..=3u8 {
            let (w, m) = gegenstand(i, 1000, Redundanzform::Kopien { anzahl: 5 });
            g.insert(w, m);
        }
        let z = zuteilen(&g, &angebote(12, 10_000), &[8u8; 32]);
        for i in 1..=3u8 {
            let h = z.halter(&wurzel(i));
            let mut sortiert = h.to_vec();
            sortiert.sort_unstable();
            assert_eq!(h, sortiert.as_slice(), "Gegenstand {i} nicht geordnet");
        }
    }

    #[test]
    fn ein_halter_bekommt_denselben_gegenstand_nicht_zweimal() {
        let mut g = BTreeMap::new();
        let (w, m) = gegenstand(1, 100, Redundanzform::Kopien { anzahl: 4 });
        g.insert(w, m);
        let z = zuteilen(&g, &angebote(4, 10_000), &[2u8; 32]);
        let mut h = z.halter(&w).to_vec();
        let vorher = h.len();
        h.dedup();
        assert_eq!(h.len(), vorher, "ein Halter kam doppelt vor");
    }

    /// Erasure teilt mehr Haltern kleinere Stücke zu.
    #[test]
    fn erasure_teilt_mehr_haltern_kleinere_stuecke_zu() {
        let form = Redundanzform::Erasure { k: 8, m: 6 };
        assert_eq!(form.halterzahl(), 14);
        assert_eq!(form.anteil_je_halter(800), 100);
        // Aufgerundet: 801 durch 8 sind 101, nicht 100.
        assert_eq!(form.anteil_je_halter(801), 101);

        let mut g = BTreeMap::new();
        let (w, m) = gegenstand(1, 800, form);
        g.insert(w, m);
        let z = zuteilen(&g, &angebote(20, 100), &[4u8; 32]);
        assert!(z.vollstaendig());
        assert_eq!(z.halter(&w).len(), 14);
    }

    /// ⚑ **Die drei Größen müssen zueinander passen.**
    ///
    /// `halterzahl · anteil_je_halter` ist der Platz, den das Netz
    /// wirklich belegt, und `platz()` ist derselbe Wert als Bruch. Läuft
    /// das auseinander, rechnet die Zuteilung mit anderen Zahlen als das
    /// Speicherentgelt, und niemand merkt es an einem einzelnen Test.
    #[test]
    fn halterzahl_anteil_und_platzfaktor_passen_zusammen() {
        for form in [
            Redundanzform::Kopien { anzahl: 1 },
            Redundanzform::Kopien { anzahl: 7 },
            Redundanzform::Erasure { k: 8, m: 6 },
            Redundanzform::Erasure { k: 8, m: 4 },
        ] {
            let laenge = 8_000u64;
            let (zaehler, nenner) = form.platz();
            let aus_bruch = laenge * u64::from(zaehler) / u64::from(nenner);
            let aus_zuteilung =
                u64::from(form.halterzahl()) * form.anteil_je_halter(laenge);
            assert_eq!(aus_bruch, aus_zuteilung, "{form:?}");
        }
    }
}
