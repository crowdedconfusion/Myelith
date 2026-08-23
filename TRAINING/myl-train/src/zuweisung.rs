//! VRF-gesteuerte Zuweisung von Korpusabschnitten (Whitepaper Kap. 7.3).
//!
//! **Warum die Zuweisung nicht dem Miner überlassen wird.** Wer keine
//! Daten fälschen kann, kann immer noch **auswählen**. Ein Angreifer mit
//! vierzig Prozent Kapazitätsanteil hätte bei freier Wahl auch vierzig
//! Prozent Einfluss auf die Datenzusammensetzung, und das Verfahren aus
//! [`crate::provenienz`] merkte davon nichts: Jedes einzelne Segment
//! wäre echt.
//!
//! Welcher Pod welche Abschnitte bearbeitet, ergibt sich deshalb aus dem
//! **Epochen-Seed**, nicht aus einer Wahl. Dem Miner bleibt nur, ein
//! zugewiesenes Bündel abzulehnen; das kostet Vergütung und wird über
//! die Ablehnungsquote sichtbar. Anhang B.6.5 beziffert den Resteinfluss
//! auf etwa zwei Prozent. Kap. 7.3 nennt diese Auflage ausdrücklich
//! **konstitutiv, nicht optional**.
//!
//! ## Der Seed kommt von außen
//!
//! Dieses Modul erzeugt **keinen** Seed. Es nimmt die 32 Bytes entgegen,
//! die `myl_scheduler::vrf_seed::EpochSeed::as_random_bytes()` liefert,
//! und leitet daraus die Zuweisung ab. Der Grund ist derselbe wie bei
//! Fund 34: Eine zweite Stelle, die Seeds erzeugt, wäre eine zweite
//! Quelle für dieselbe Aussage. Der Seed ist über eine VRF an den
//! finalisierten Block der Vorepoche und die Epochennummer gebunden
//! (Fund A20), und diese Bindung gehört genau einmal in den Scheduler.
//!
//! Die Abhängigkeit steht deshalb bewusst **nicht** im Manifest: Die
//! Schnittstelle sind 32 Bytes.

/// Ein zugewiesenes Bündel: zusammenhängende Segmente eines Korpus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zuweisung {
    pub start: u64,
    pub laenge: u64,
}

/// Fehler der Zuweisung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZuweisungsFehler {
    /// Ein Korpus ohne Segmente lässt sich nicht zuweisen.
    KorpusLeer,
    /// Bündelgröße null.
    BuendelLeer,
    /// Der Korpus trägt weniger Segmente, als ein Bündel groß ist.
    KorpusKleinerAlsBuendel { segmente: u64, buendel: u64 },
}

impl std::fmt::Display for ZuweisungsFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KorpusLeer => write!(f, "Korpus ohne Segmente"),
            Self::BuendelLeer => write!(f, "Bündelgröße muss größer als null sein"),
            Self::KorpusKleinerAlsBuendel { segmente, buendel } => write!(
                f,
                "Korpus hat {} Segmente, ein Bündel soll {} groß sein",
                segmente, buendel
            ),
        }
    }
}

impl std::error::Error for ZuweisungsFehler {}

/// Weist einem Pod ein Bündel zu.
///
/// Deterministisch aus `(epochen_seed, korpus_kennung, pod_index)`. Zwei
/// Knoten, die dieselben Eingaben haben, kommen zur selben Zuweisung;
/// das ist die Bedingung dafür, dass sie sie überhaupt prüfen können.
///
/// **Die Bündel liegen auf einem Raster** der Größe `buendel`, nicht auf
/// beliebigen Startpositionen. Zwei Gründe: Ein Bündel auf dem Raster
/// teilt sich den gemeinsamen Merkle-Teilbaum vollständig, worauf die
/// Kostenrechnung aus Anhang B.6.4 beruht, und die Zuweisung wird zu
/// einer Auswahl aus einer festen Menge, was sie prüfbar hält.
///
/// Der letzte Rest eines Korpus, der kein volles Bündel mehr füllt,
/// bleibt **unzugewiesen**. Ihn auf die Länge zu kürzen wäre einfach und
/// falsch: Bündel verschiedener Größe tragen verschieden viel Arbeit,
/// und die Vergütung hinge dann an der Position im Korpus.
pub fn zuweisen(
    epochen_seed: &[u8; 32],
    korpus_kennung: &str,
    segmente: u64,
    buendel: u64,
    pod_index: u64,
) -> Result<Zuweisung, ZuweisungsFehler> {
    if segmente == 0 {
        return Err(ZuweisungsFehler::KorpusLeer);
    }
    if buendel == 0 {
        return Err(ZuweisungsFehler::BuendelLeer);
    }
    let raster = segmente / buendel;
    if raster == 0 {
        return Err(ZuweisungsFehler::KorpusKleinerAlsBuendel { segmente, buendel });
    }

    let wahl = ziehung(epochen_seed, korpus_kennung, pod_index) % raster;
    Ok(Zuweisung {
        start: wahl * buendel,
        laenge: buendel,
    })
}

/// Ganzzahlige Ziehung aus Seed, Korpuskennung und Pod-Index.
///
/// SHA-256 über die drei Eingaben mit festen Feldbreiten und einem
/// Domänen-Präfix. Feste Breiten, damit `("ab", 1)` und `("a", "b1")`
/// nicht dieselbe Bytefolge ergeben; das Präfix, damit dieser Hash nicht
/// als irgendein anderer im Protokoll gelesen werden kann.
fn ziehung(seed: &[u8; 32], korpus: &str, pod_index: u64) -> u64 {
    use myl_types::hash::Hash;

    let mut botschaft = Vec::with_capacity(64 + korpus.len());
    botschaft.extend_from_slice(b"MYELITH_DATENZUWEISUNG_v1");
    botschaft.extend_from_slice(seed);
    botschaft.extend_from_slice(&(korpus.len() as u64).to_le_bytes());
    botschaft.extend_from_slice(korpus.as_bytes());
    botschaft.extend_from_slice(&pod_index.to_le_bytes());

    let h = Hash::sha256(&botschaft);
    let mut acht = [0u8; 8];
    acht.copy_from_slice(&h.as_bytes()[..8]);
    u64::from_le_bytes(acht)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: [u8; 32] = [7u8; 32];

    #[test]
    fn dieselben_eingaben_ergeben_dieselbe_zuweisung() {
        let a = zuweisen(&SEED, "wikitext2", 4096, 256, 3).unwrap();
        let b = zuweisen(&SEED, "wikitext2", 4096, 256, 3).unwrap();
        assert_eq!(a, b);
    }

    /// Ein anderer Seed, ein anderer Korpus oder ein anderer Pod: jedes
    /// für sich muss die Zuweisung bewegen können.
    #[test]
    fn jede_eingabe_geht_ein() {
        let grund = zuweisen(&SEED, "wikitext2", 4096, 256, 3).unwrap();

        let anderer_seed = zuweisen(&[9u8; 32], "wikitext2", 4096, 256, 3).unwrap();
        let anderer_korpus = zuweisen(&SEED, "wikitext103", 4096, 256, 3).unwrap();
        let anderer_pod = zuweisen(&SEED, "wikitext2", 4096, 256, 4).unwrap();

        assert!(
            anderer_seed != grund || anderer_korpus != grund || anderer_pod != grund,
            "keine der drei Eingaben bewegt die Zuweisung"
        );
        // Und einzeln, über mehrere Pods, damit ein zufälliger Treffer
        // den Test nicht bestehen lässt.
        let mit_seed_a: Vec<_> = (0..20).map(|i| zuweisen(&SEED, "k", 4096, 256, i).unwrap()).collect();
        let mit_seed_b: Vec<_> = (0..20).map(|i| zuweisen(&[9u8; 32], "k", 4096, 256, i).unwrap()).collect();
        assert_ne!(mit_seed_a, mit_seed_b);

        let korpus_a: Vec<_> = (0..20).map(|i| zuweisen(&SEED, "a", 4096, 256, i).unwrap()).collect();
        let korpus_b: Vec<_> = (0..20).map(|i| zuweisen(&SEED, "b", 4096, 256, i).unwrap()).collect();
        assert_ne!(korpus_a, korpus_b);
    }

    /// **Die Zusage des Verfahrens:** Der Miner wählt nicht. Über viele
    /// Pods hinweg muss die Zuweisung den Korpus überstreichen, sonst
    /// wäre sie vorhersagbar konzentriert.
    #[test]
    fn die_zuweisung_ueberstreicht_den_korpus() {
        let raster = 16u64;
        let mut getroffen = std::collections::BTreeSet::new();
        for pod in 0..500u64 {
            let z = zuweisen(&SEED, "wikitext2", raster * 256, 256, pod).unwrap();
            getroffen.insert(z.start / 256);
        }
        assert_eq!(
            getroffen.len() as u64,
            raster,
            "nach 500 Pods müssen alle {} Rasterplätze vorgekommen sein",
            raster
        );
    }

    /// Zuweisungen liegen auf dem Raster und nie über das Korpusende
    /// hinaus.
    #[test]
    fn zuweisungen_liegen_im_korpus_und_auf_dem_raster() {
        let segmente = 1000u64;
        let buendel = 256u64;
        for pod in 0..200u64 {
            let z = zuweisen(&SEED, "k", segmente, buendel, pod).unwrap();
            assert_eq!(z.laenge, buendel);
            assert_eq!(z.start % buendel, 0);
            assert!(z.start + z.laenge <= segmente);
        }
    }

    /// Der Rest, der kein volles Bündel mehr füllt, bleibt liegen.
    /// 1000 Segmente bei Bündelgröße 256 heißt: drei Rasterplätze,
    /// 232 Segmente ungenutzt.
    #[test]
    fn der_rest_bleibt_unzugewiesen() {
        let mut hoechste = 0;
        for pod in 0..200u64 {
            let z = zuweisen(&SEED, "k", 1000, 256, pod).unwrap();
            hoechste = hoechste.max(z.start + z.laenge);
        }
        assert_eq!(hoechste, 768);
    }

    #[test]
    fn unbrauchbare_eingaben_werden_abgelehnt() {
        assert_eq!(
            zuweisen(&SEED, "k", 0, 256, 0),
            Err(ZuweisungsFehler::KorpusLeer)
        );
        assert_eq!(
            zuweisen(&SEED, "k", 100, 0, 0),
            Err(ZuweisungsFehler::BuendelLeer)
        );
        assert_eq!(
            zuweisen(&SEED, "k", 100, 256, 0),
            Err(ZuweisungsFehler::KorpusKleinerAlsBuendel {
                segmente: 100,
                buendel: 256
            })
        );
    }

    /// Feste Feldbreiten: `("ab", 1)` und `("a", ...)` dürfen nicht
    /// dieselbe Bytefolge ergeben.
    #[test]
    fn die_botschaft_ist_eindeutig_zerlegbar() {
        assert_ne!(ziehung(&SEED, "ab", 1), ziehung(&SEED, "a", 1));
        assert_ne!(ziehung(&SEED, "a", 98), ziehung(&SEED, "ab", 0));
    }

    /// Die Zuweisung greift auf einen echten Korpus durch: Was
    /// zugewiesen wird, muss sich auch belegen lassen.
    #[test]
    fn eine_zuweisung_laesst_sich_belegen() {
        use crate::provenienz::Korpus;

        let segmente: Vec<Vec<u8>> = (0..64)
            .map(|i| format!("Abschnitt {}", i).into_bytes())
            .collect();
        let k = Korpus::verankern("klein", segmente).unwrap();
        let wurzel = k.wurzel();

        let z = zuweisen(&SEED, "klein", 64, 8, 0).unwrap();
        let b = k.buendel(z.start, z.laenge).unwrap();
        let daten: Vec<&[u8]> = (z.start..z.start + z.laenge)
            .map(|i| k.segment(i).unwrap())
            .collect();
        assert!(b.pruefen(&wurzel, &daten));
    }
}
