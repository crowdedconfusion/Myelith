//! Der Verfügbarkeitsnachweis: Wer zusagt zu halten, wird gefragt.
//!
//! ## ⚑ Fund 106: Der entworfene Nachweis belegte keine Speicherung
//!
//! Der Modulkopf von [`crate`] begründet, warum nicht nach einem Hash
//! über den ganzen Teil gefragt wird: „Wer ihn einmal gesehen hat,
//! wiederholt ihn für immer." Das ist richtig. Die daraus gezogene
//! Folgerung, gefragt werde nach einem Blattindex und geantwortet mit
//! „Blatt und Merkle-Pfad", **schließt die Lücke aber nicht**, wenn mit
//! Blatt der Blatt-*Hash* gemeint ist. Der Kommentar an
//! [`myl_types::gegenstand::TEILGROESSE`] sagte genau das: ein Mebibyte sei
//! „klein genug, dass ein Verfügbarkeitsnachweis nicht ein Mebibyte
//! Antwort erzeugt".
//!
//! Die Blätter des Baums **sind** die Teil-Hashes. Wer sie alle hält,
//! beantwortet jede Stichprobe fehlerfrei, für immer, ohne ein einziges
//! Byte Nutzdaten:
//!
//! | Gegenstand | Teile | Hashliste | Faktor |
//! |---|---|---|---|
//! | 1 GB | 954 | 0,03 MiB | 32 757 |
//! | 8 GB | 7 630 | 0,23 MiB | 32 765 |
//! | 30 GB | 28 611 | 0,87 MiB | **32 767** |
//!
//! Ein Halter des 30-GB-Gemischs käme mit 0,87 MiB aus und bekäme das
//! volle Speicherentgelt. **Das ist nicht ein schwacher Nachweis,
//! sondern gar keiner.**
//!
//! ## Die Antwort trägt die Bytes
//!
//! Verlangt wird der **Teil selbst**. Der Fragende hasht ihn, vergleicht
//! mit dem Blatt und prüft den Pfad gegen die Wurzel aus dem Manifest.
//! Er braucht dafür weiterhin nur das Manifest, also 32 Byte plus
//! Kopfdaten; hielte er die Daten, wäre der ganze Nachweis sinnlos.
//!
//! **Ein Mebibyte je Stichprobe ist der Preis, und er ist richtig
//! herum.** Die Befürchtung im alten Kommentar tauschte einen billigen
//! Nachweis gegen einen wertlosen. Je Epoche und Halter fällt genau eine
//! Antwort an; gemessen an einem Gegenstand von 30 GB sind das 0,003 %.
//!
//! ## ⚑ Jeder Halter wird nach einem anderen Teil gefragt
//!
//! Der Index hängt an der Kennung des Halters. Wäre er für alle gleich,
//! genügte **ein** Halter mit den Daten: Die übrigen könnten seine
//! Antwort abschreiben und würden für Speicher bezahlt, den nur einer
//! hat. Mit Bindung an den Halter nützt eine fremde Antwort niemandem,
//! denn sie steht für den falschen Teil.
//!
//! Der Seed kommt von außen und ist der Epochenseed, derselbe, aus dem
//! die Shard-Zuteilung gezogen wird. Er ist nicht vorhersehbar und für
//! jeden nachrechenbar, und beides braucht diese Stichprobe.

use myl_types::gegenstand::{Manifest, TEILGROESSE};
use myl_types::hash::Hash;
use myl_types::ids::{EpochId, MerkleRoot, MinerId};
use myl_types::merkle::{MerkleProof, MerkleTree};

/// Trennstring der Stichproben-Ableitung.
///
/// ⚑ **Wiederausfuhr, keine zweite Fassung** (2026-08-31). Die
/// Ableitung zog nach `myl-types`, weil jeder, der eine Quittung prüft,
/// dieselbe Zahl ausrechnen können muss, ohne an der Store-Rolle zu
/// hängen. Zwei Fassungen wären zwei Quellen für dieselbe Wahrheit.
pub use myl_types::zuteilung::DST_SPEICHER_STICHPROBE;

/// Welche Stelle eines Gegenstands von wem verlangt wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stichprobe {
    /// Wurzel des Gegenstands, aus dem Manifest.
    pub wurzel: MerkleRoot,
    /// Fassung des Gegenstands. Ohne sie wäre nach einem Wachstum der
    /// Wissensdatenbank offen, welcher Stand gemeint war.
    pub fassung: u32,
    /// Die Epoche, für die gefragt wird.
    pub epoche: EpochId,
    /// Wer antworten muss.
    pub halter: MinerId,
    /// Der verlangte Teil, von null an.
    pub teil: u32,
}

impl Stichprobe {
    /// Leitet die Stichprobe deterministisch ab.
    ///
    /// Jeder kann sie nachrechnen, niemand sie vorhersehen, solange der
    /// Seed es nicht ist. Gebunden wird an Wurzel, Fassung, Epoche und
    /// **Halter**, siehe Modulkopf.
    pub fn ableiten(
        manifest: &Manifest,
        epoche: EpochId,
        halter: &MinerId,
        seed: &[u8; 32],
    ) -> Self {
        let teil = myl_types::zuteilung::verlangter_teil(manifest, epoche, halter, seed);

        Self {
            wurzel: manifest.wurzel,
            fassung: manifest.fassung,
            epoche,
            halter: *halter,
            teil,
        }
    }
}

/// Die Antwort eines Halters: der Teil selbst, plus sein Pfad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Antwort {
    /// Der beantwortete Teil.
    pub teil: u32,
    /// ⚑ **Die Bytes, nicht ihr Hash.** Siehe Modulkopf, Fund 106.
    pub klartext: Vec<u8>,
    /// Pfad vom Blatt zur Wurzel.
    pub pfad: MerkleProof,
}

impl Antwort {
    /// Baut die Antwort aus den vollständigen Teilen des Gegenstands.
    ///
    /// **Nur ein Halter kann das**, und das ist der Zweck: Die Bytes
    /// stehen in der Antwort, ein Hash genügt nicht.
    pub fn erzeugen(teile: &[Vec<u8>], teil: u32) -> Result<Self, NachweisFehler> {
        let i = teil as usize;
        let bytes = teile.get(i).ok_or(NachweisFehler::TeilAusserhalb {
            teil,
            teilzahl: teile.len() as u32,
        })?;

        let blaetter: Vec<[u8; 32]> = teile.iter().map(|t| Hash::sha256(t).0).collect();
        let refs: Vec<&[u8]> = blaetter.iter().map(|b| b.as_slice()).collect();
        let baum = MerkleTree::new(&refs).map_err(|_| NachweisFehler::BaumUnbaubar)?;
        let pfad = baum
            .proof(i)
            .map_err(|_| NachweisFehler::BaumUnbaubar)?;

        Ok(Self {
            teil,
            klartext: bytes.clone(),
            pfad,
        })
    }
}

/// Warum eine Antwort den Nachweis nicht trägt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NachweisFehler {
    /// Beantwortet wurde ein anderer Teil als der verlangte.
    FalscherTeil {
        /// Verlangt war dieser.
        verlangt: u32,
        /// Geliefert wurde jener.
        geliefert: u32,
    },
    /// Der verlangte Teil liegt außerhalb des Gegenstands.
    TeilAusserhalb {
        /// Der verlangte Index.
        teil: u32,
        /// So viele Teile hat der Gegenstand.
        teilzahl: u32,
    },
    /// Die Stichprobe gilt einem anderen Gegenstand oder einer anderen
    /// Fassung.
    FremderGegenstand,
    /// Der Teil hat nicht die Länge, die das Manifest für ihn vorsieht.
    ///
    /// **Eigener Fehler und nicht bloß ein falscher Hash**, weil er den
    /// häufigsten Irrtum benennt: Wer den letzten Teil auffüllt oder
    /// einen mittleren kürzt, soll das lesen und nicht über einen
    /// Pfadfehler rätseln.
    FalscheLaenge {
        /// So lang hätte der Teil sein müssen.
        erwartet: u64,
        /// So lang war er.
        gemessen: u64,
    },
    /// Blatt und Pfad passen nicht zur Wurzel des Manifests.
    ///
    /// Das ist der Fall, in dem die Bytes fehlen oder verändert wurden.
    PfadFalsch,
    /// Aus den übergebenen Teilen ließ sich kein Baum bauen.
    BaumUnbaubar,
}

/// Wie lang der `teil`-te Teil eines Gegenstands sein muss.
///
/// Ergibt sich allein aus dem Manifest, der Fragende braucht die Daten
/// also nicht. Alle Teile bis auf den letzten sind
/// [`TEILGROESSE`] lang.
pub fn erwartete_laenge(manifest: &Manifest, teil: u32) -> Option<u64> {
    if teil >= manifest.teilzahl {
        return None;
    }
    let voll = TEILGROESSE as u64;
    if teil + 1 < manifest.teilzahl {
        return Some(voll);
    }
    // Der letzte Teil trägt den Rest. `laenge` ist die Summe aller
    // Teillängen, also ist der Rest nie negativ und nie null.
    let vorher = u64::from(manifest.teilzahl - 1) * voll;
    Some(manifest.laenge.saturating_sub(vorher))
}

/// Prüft eine Antwort gegen Manifest und Stichprobe.
///
/// **Der Fragende braucht nur das Manifest.** Geprüft wird in dieser
/// Reihenfolge, damit der Fehler benennt, was wirklich schiefging:
/// erst der Gegenstand, dann der Index, dann die Länge, zuletzt der
/// kryptografische Pfad.
pub fn pruefe(
    manifest: &Manifest,
    stichprobe: &Stichprobe,
    antwort: &Antwort,
) -> Result<(), NachweisFehler> {
    if stichprobe.wurzel != manifest.wurzel || stichprobe.fassung != manifest.fassung {
        return Err(NachweisFehler::FremderGegenstand);
    }
    if antwort.teil != stichprobe.teil {
        return Err(NachweisFehler::FalscherTeil {
            verlangt: stichprobe.teil,
            geliefert: antwort.teil,
        });
    }
    let erwartet = erwartete_laenge(manifest, stichprobe.teil).ok_or(
        NachweisFehler::TeilAusserhalb {
            teil: stichprobe.teil,
            teilzahl: manifest.teilzahl,
        },
    )?;
    let gemessen = antwort.klartext.len() as u64;
    if gemessen != erwartet {
        return Err(NachweisFehler::FalscheLaenge { erwartet, gemessen });
    }

    // ⚑ Hier steckt der ganze Nachweis: Das Blatt entsteht aus den
    // gelieferten Bytes und nicht aus einem mitgeschickten Hash.
    //
    // Das Blatt des Baums ist der **Teil-Hash als Blattdatum**, siehe
    // `Manifest::neu`. Deshalb `verify` und nicht `verify_hashed`: Die
    // Blatt-Hashung des Baums kommt noch darüber. `verify` bindet den
    // Beweis zugleich an den Index, auch bei einem Ein-Blatt-Baum.
    let blatt = Hash::sha256(&antwort.klartext);
    let wurzel = Hash(*manifest.wurzel.as_bytes());
    if !antwort
        .pfad
        .verify(&wurzel, &blatt.0, u64::from(stichprobe.teil))
    {
        return Err(NachweisFehler::PfadFalsch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::gegenstand::{teile_bilden, Gegenstandsart, Redundanzform};

    /// Drei Teile: zwei volle und ein kurzer letzter.
    fn gegenstand() -> (Manifest, Vec<Vec<u8>>) {
        let laenge = 2 * TEILGROESSE + 4096;
        let daten: Vec<u8> = (0..laenge).map(|i| (i % 251) as u8).collect();
        let teile = teile_bilden(&daten).expect("Teile");
        let manifest = Manifest::neu(
            Gegenstandsart::Shardgewichte,
            1,
            &teile,
            Redundanzform::Kopien { anzahl: 3 },
        )
        .expect("Manifest");
        let bytes: Vec<Vec<u8>> = daten.chunks(TEILGROESSE).map(<[u8]>::to_vec).collect();
        (manifest, bytes)
    }

    fn halter(byte: u8) -> MinerId {
        MinerId::new([byte; 32])
    }

    #[test]
    fn ein_ehrlicher_halter_weist_nach() {
        let (m, bytes) = gegenstand();
        let s = Stichprobe::ableiten(&m, EpochId(7), &halter(1), &[9u8; 32]);
        let a = Antwort::erzeugen(&bytes, s.teil).expect("Antwort");
        assert_eq!(pruefe(&m, &s, &a), Ok(()));
    }

    /// ⚑ **Fund 106 als Test.** Wer nur die Blatt-Hashes hält, kann den
    /// Pfad bauen, aber die Antwort nicht füllen.
    ///
    /// Der Baum entsteht **allein aus den Hashes**; der Betrüger kommt
    /// also bis zum Pfad. Was er nicht hat, sind die Bytes, und genau
    /// darauf prüft der Nachweis. Beide naheliegenden Versuche sind hier
    /// nachgestellt: den Hash als Klartext schicken, und irgendein
    /// Mebibyte schicken.
    #[test]
    fn wer_nur_die_hashliste_haelt_kann_nicht_antworten() {
        let (m, bytes) = gegenstand();
        let s = Stichprobe::ableiten(&m, EpochId(7), &halter(1), &[9u8; 32]);

        // Was der Betrüger hat: 3 mal 32 Byte statt 2 MiB.
        let hashliste: Vec<[u8; 32]> = bytes.iter().map(|t| Hash::sha256(t).0).collect();
        assert_eq!(hashliste.len() * 32, 96);
        let refs: Vec<&[u8]> = hashliste.iter().map(|b| b.as_slice()).collect();
        let baum = MerkleTree::new(&refs).expect("der Baum geht aus Hashes");
        let pfad = baum.proof(s.teil as usize).expect("Pfad");

        // Versuch 1: den Blatt-Hash als Klartext ausgeben.
        let versuch = Antwort {
            teil: s.teil,
            klartext: hashliste[s.teil as usize].to_vec(),
            pfad: pfad.clone(),
        };
        assert!(
            matches!(
                pruefe(&m, &s, &versuch),
                Err(NachweisFehler::FalscheLaenge { .. })
            ),
            "der Blatt-Hash als Antwort ging durch"
        );

        // Versuch 2: die richtige Länge, aber erfundene Bytes.
        let erwartet = erwartete_laenge(&m, s.teil).expect("Länge");
        let versuch = Antwort {
            teil: s.teil,
            klartext: vec![0u8; erwartet as usize],
            pfad,
        };
        assert_eq!(
            pruefe(&m, &s, &versuch),
            Err(NachweisFehler::PfadFalsch),
            "erfundene Bytes gingen durch"
        );
    }

    /// ⚑ **Zwei Halter werden nach verschiedenen Teilen gefragt.**
    ///
    /// Wäre der Index für alle gleich, genügte ein Halter mit den Daten;
    /// die übrigen schrieben seine Antwort ab. Der Test sucht einen
    /// Seed, bei dem sich die Indizes unterscheiden, und belegt damit,
    /// dass die Kennung überhaupt eingeht.
    #[test]
    fn zwei_halter_werden_nach_verschiedenen_teilen_gefragt() {
        let (m, _) = gegenstand();
        let mut verschieden = false;
        for k in 0..16u8 {
            let seed = [k; 32];
            let a = Stichprobe::ableiten(&m, EpochId(3), &halter(1), &seed);
            let b = Stichprobe::ableiten(&m, EpochId(3), &halter(2), &seed);
            if a.teil != b.teil {
                verschieden = true;
                break;
            }
        }
        assert!(
            verschieden,
            "die Kennung des Halters geht nicht in die Ableitung ein"
        );
    }

    /// Und die Antwort eines fremden Halters trägt den eigenen Nachweis
    /// nicht, sobald der Teil ein anderer ist.
    #[test]
    fn eine_abgeschriebene_antwort_passt_nicht_auf_die_eigene_stichprobe() {
        let (m, bytes) = gegenstand();
        let seed = [4u8; 32];
        let meine = Stichprobe::ableiten(&m, EpochId(3), &halter(1), &seed);
        for k in 2..40u8 {
            let fremde = Stichprobe::ableiten(&m, EpochId(3), &halter(k), &seed);
            if fremde.teil == meine.teil {
                continue;
            }
            let abgeschrieben = Antwort::erzeugen(&bytes, fremde.teil).expect("Antwort");
            assert_eq!(
                pruefe(&m, &meine, &abgeschrieben),
                Err(NachweisFehler::FalscherTeil {
                    verlangt: meine.teil,
                    geliefert: fremde.teil,
                })
            );
            return;
        }
        panic!("kein Halter mit abweichendem Teil gefunden");
    }

    #[test]
    fn dieselbe_ableitung_ergibt_dieselbe_stichprobe() {
        let (m, _) = gegenstand();
        let a = Stichprobe::ableiten(&m, EpochId(11), &halter(5), &[2u8; 32]);
        let b = Stichprobe::ableiten(&m, EpochId(11), &halter(5), &[2u8; 32]);
        assert_eq!(a, b);
        let c = Stichprobe::ableiten(&m, EpochId(12), &halter(5), &[2u8; 32]);
        assert_ne!((a.epoche, a.teil), (c.epoche, c.teil));
    }

    /// Der letzte Teil ist kürzer, und das muss der Fragende aus dem
    /// Manifest allein wissen.
    #[test]
    fn der_letzte_teil_ist_kuerzer_und_das_steht_im_manifest() {
        let (m, bytes) = gegenstand();
        assert_eq!(m.teilzahl, 3);
        assert_eq!(erwartete_laenge(&m, 0), Some(TEILGROESSE as u64));
        assert_eq!(erwartete_laenge(&m, 1), Some(TEILGROESSE as u64));
        assert_eq!(erwartete_laenge(&m, 2), Some(4096));
        assert_eq!(erwartete_laenge(&m, 3), None);
        assert_eq!(bytes[2].len(), 4096);
    }

    /// ⚑ **Ein aufgefüllter letzter Teil fällt auf.** Ohne die
    /// Längenprüfung wäre er von einem echten nicht zu unterscheiden,
    /// und genau davor warnt der Kommentar an `Teil::laenge`.
    #[test]
    fn ein_aufgefuellter_letzter_teil_faellt_auf() {
        let (m, bytes) = gegenstand();
        let s = Stichprobe {
            teil: 2,
            ..Stichprobe::ableiten(&m, EpochId(1), &halter(1), &[1u8; 32])
        };
        let mut a = Antwort::erzeugen(&bytes, 2).expect("Antwort");
        a.klartext.resize(TEILGROESSE, 0);
        assert_eq!(
            pruefe(&m, &s, &a),
            Err(NachweisFehler::FalscheLaenge {
                erwartet: 4096,
                gemessen: TEILGROESSE as u64,
            })
        );
    }

    #[test]
    fn eine_stichprobe_zu_einem_fremden_gegenstand_wird_abgelehnt() {
        let (m, bytes) = gegenstand();
        let s = Stichprobe {
            fassung: m.fassung + 1,
            ..Stichprobe::ableiten(&m, EpochId(1), &halter(1), &[1u8; 32])
        };
        let a = Antwort::erzeugen(&bytes, s.teil).expect("Antwort");
        assert_eq!(pruefe(&m, &s, &a), Err(NachweisFehler::FremderGegenstand));
    }

    #[test]
    fn ein_teil_ausserhalb_wird_abgelehnt() {
        let (_m, bytes) = gegenstand();
        assert!(matches!(
            Antwort::erzeugen(&bytes, 99),
            Err(NachweisFehler::TeilAusserhalb { teil: 99, .. })
        ));
    }
}
