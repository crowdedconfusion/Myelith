//! Datenprovenienz: Herkunft statt Inhalt (Whitepaper Kap. 7.3).
//!
//! **Die Frage, die das löst.** Ein Miner, der vergiftete Texte
//! einspeist, rechnet bitgleich korrekt und erzeugt trotzdem ein
//! verschobenes Modell. Der Bitvergleich aus Kap. 6 greift hier nicht:
//! Er prüft, ob richtig gerechnet wurde, nicht, ob die Daten legitim
//! waren.
//!
//! Eine **inhaltliche** Bewertung scheidet aus. Sie wäre subjektiv und
//! damit genau der Bewertungsspielraum, den dieses Protokoll überall
//! sonst vermeidet. Geprüft wird deshalb die **Herkunft**: Das Protokoll
//! führt kanonische Korpora, jedes mit einer Merkle-Wurzel im Konsens
//! verankert, und ein Trainingssegment referenziert keine Rohdaten,
//! sondern einen Beweis: *Dieser Abschnitt steht an Position i im Korpus
//! mit der Wurzel R.*
//!
//! Damit ist die Prüfung wieder objektiv und exakt so entscheidbar wie
//! eine Inferenz. Für eine nicht existierende Position gibt es keinen
//! gültigen Beweis.
//!
//! ## Was hier steht und was nicht
//!
//! Umgesetzt sind die drei objektiv entscheidbaren Teile von Kap. 7.3:
//! Verankerung, Referenz per Beweis und die **Bündelung**
//! zusammenhängender Segmente, deren Kostenwirkung Anhang B.6.4
//! beziffert.
//!
//! **Nicht hier:** die Ablehnungsquote für verweigerte Segmente. Sie ist
//! kein Provenienzproblem, sondern eine Buchführung über das Verhalten
//! eines Miners über Epochen hinweg, und die gehört zum Ledger. Sie
//! hier nachzubilden hieße, denselben Zustand zweimal zu führen.
//!
//! **Ebenfalls nicht hier:** der Nachweis, dass ein Korpus *inhaltlich*
//! in Ordnung ist. Wer ihn kanonisiert, entscheidet das; dieses Modul
//! stellt nur fest, dass ein Segment aus einem kanonisierten Korpus
//! stammt. Das ist der Punkt des Verfahrens, und es ist zugleich seine
//! Grenze: **Ein vergifteter kanonischer Korpus wird hier nicht
//! auffallen.**

use myl_types::hash::Hash;
use myl_types::merkle::{MerkleProof, MerkleTree};

/// Ein kanonischer Korpus, in Segmente zerlegt und über eine
/// Merkle-Wurzel verankert.
///
/// Die Wurzel geht in den Konsens; die Segmente selbst bleiben, wo sie
/// sind. Ein Knoten, der prüfen will, braucht die Wurzel und den Beweis,
/// nicht den Korpus.
#[derive(Debug, Clone)]
pub struct Korpus {
    /// Kennung, wie sie im Konsens geführt wird.
    pub kennung: String,
    baum: MerkleTree,
    segmente: Vec<Vec<u8>>,
}

/// Fehler der Provenienzprüfung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvenienzFehler {
    /// Ein Korpus ohne Segmente hat keine Wurzel und kann nichts belegen.
    KorpusLeer,
    /// Die verlangte Position liegt außerhalb des Korpus.
    PositionAusserhalb { position: u64, segmente: u64 },
    /// Ein Bündel ohne Segmente.
    BuendelLeer,
    /// Das Bündel reicht über das Korpusende hinaus.
    BuendelAusserhalb { start: u64, laenge: u64, segmente: u64 },
    /// Der Merkle-Baum ließ sich nicht bauen.
    BaumFehler(String),
}

impl std::fmt::Display for ProvenienzFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KorpusLeer => write!(
                f,
                "Korpus ohne Segmente: eine Wurzel darüber belegt nichts"
            ),
            Self::PositionAusserhalb {
                position,
                segmente,
            } => write!(
                f,
                "Position {} liegt außerhalb des Korpus ({} Segmente). \
                 Genau dafür gibt es keinen gültigen Beweis, und das ist \
                 der Zweck des Verfahrens",
                position, segmente
            ),
            Self::BuendelLeer => write!(f, "Bündel ohne Segmente"),
            Self::BuendelAusserhalb {
                start,
                laenge,
                segmente,
            } => write!(
                f,
                "Bündel ab {} über {} Segmente reicht über den Korpus \
                 ({} Segmente) hinaus",
                start, laenge, segmente
            ),
            Self::BaumFehler(e) => write!(f, "Merkle-Baum: {}", e),
        }
    }
}

impl std::error::Error for ProvenienzFehler {}

impl Korpus {
    /// Verankert einen Korpus: Segmente hinein, Merkle-Wurzel heraus.
    pub fn verankern(kennung: &str, segmente: Vec<Vec<u8>>) -> Result<Self, ProvenienzFehler> {
        if segmente.is_empty() {
            return Err(ProvenienzFehler::KorpusLeer);
        }
        let scheiben: Vec<&[u8]> = segmente.iter().map(|s| s.as_slice()).collect();
        let baum = MerkleTree::new(&scheiben)
            .map_err(|e| ProvenienzFehler::BaumFehler(format!("{:?}", e)))?;
        Ok(Self {
            kennung: kennung.to_string(),
            baum,
            segmente,
        })
    }

    /// Die im Konsens verankerte Wurzel.
    pub fn wurzel(&self) -> Hash {
        self.baum.root()
    }

    pub fn anzahl_segmente(&self) -> u64 {
        self.segmente.len() as u64
    }

    /// Tiefe des Baums, also die Länge eines Einzelbeweises in Knoten.
    pub fn tiefe(&self) -> usize {
        self.baum.depth()
    }

    /// Referenz auf ein einzelnes Segment.
    pub fn referenz(&self, position: u64) -> Result<SegmentReferenz, ProvenienzFehler> {
        if position >= self.anzahl_segmente() {
            return Err(ProvenienzFehler::PositionAusserhalb {
                position,
                segmente: self.anzahl_segmente(),
            });
        }
        let beweis = self
            .baum
            .proof(position as usize)
            .map_err(|e| ProvenienzFehler::BaumFehler(format!("{:?}", e)))?;
        Ok(SegmentReferenz {
            korpus: self.kennung.clone(),
            wurzel: self.wurzel(),
            position,
            beweis,
        })
    }

    /// Referenz auf ein Bündel zusammenhängender Segmente.
    ///
    /// **Warum gebündelt (Anhang B.6.4).** Ein Einzelbeweis über einen
    /// Korpus von einer Milliarde Dokumenten hat Tiefe 30, kostet also
    /// 960 Byte gegenüber 8192 Byte Nutzdaten, mithin 11,7 Prozent. Bei
    /// 256 zusammenhängenden Segmenten teilen sich diese den gemeinsamen
    /// Teilbaum, und der Anteil sinkt unter ein halbes Prozent.
    ///
    /// Hier werden die Einzelbeweise geführt, nicht der gemeinsame
    /// Teilbaum ausgerechnet: Die Ersparnis ist eine Eigenschaft der
    /// Übertragung, nicht der Prüfung, und ein Bündel mit einem
    /// gefälschten Segment darunter muss auffallen. Die Kostenrechnung
    /// steht in [`buendel_overhead_promille`] und ist dort gegen die
    /// Zahlen aus B.6.4 geprüft.
    pub fn buendel(&self, start: u64, laenge: u64) -> Result<BuendelReferenz, ProvenienzFehler> {
        if laenge == 0 {
            return Err(ProvenienzFehler::BuendelLeer);
        }
        if start.saturating_add(laenge) > self.anzahl_segmente() {
            return Err(ProvenienzFehler::BuendelAusserhalb {
                start,
                laenge,
                segmente: self.anzahl_segmente(),
            });
        }
        let mut referenzen = Vec::with_capacity(laenge as usize);
        for i in start..start + laenge {
            referenzen.push(self.referenz(i)?);
        }
        Ok(BuendelReferenz {
            korpus: self.kennung.clone(),
            wurzel: self.wurzel(),
            start,
            referenzen,
        })
    }

    /// Die Rohdaten eines Segments, für den, der den Korpus hat.
    pub fn segment(&self, position: u64) -> Option<&[u8]> {
        self.segmente.get(position as usize).map(|s| s.as_slice())
    }
}

/// Ein Trainingssegment, referenziert über seine Herkunft.
///
/// **Enthält bewusst keine Rohdaten.** Wer prüft, hat den Korpus und
/// braucht nur zu wissen, welche Position gemeint ist und dass sie
/// wirklich unter der verankerten Wurzel hängt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentReferenz {
    pub korpus: String,
    pub wurzel: Hash,
    pub position: u64,
    pub beweis: MerkleProof,
}

impl SegmentReferenz {
    /// Prüft die Referenz gegen die verankerte Wurzel und die Daten.
    ///
    /// `verankerte_wurzel` kommt aus dem Konsens, **nicht** aus der
    /// Referenz. Beide zu vergleichen ist der Kern: Eine Referenz, die
    /// ihre eigene Wurzel mitbringt und gegen sie geprüft wird, belegt
    /// nichts. Genau dieser Fehler steckte in Audit-Fund A11
    /// (`adjudicate` prüfte gegen den mitgelieferten Hash).
    pub fn pruefen(&self, verankerte_wurzel: &Hash, daten: &[u8]) -> bool {
        if self.wurzel != *verankerte_wurzel {
            return false;
        }
        self.beweis
            .verify(verankerte_wurzel, daten, self.position)
    }
}

/// Ein Bündel zusammenhängender Segmente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuendelReferenz {
    pub korpus: String,
    pub wurzel: Hash,
    pub start: u64,
    pub referenzen: Vec<SegmentReferenz>,
}

impl BuendelReferenz {
    pub fn laenge(&self) -> u64 {
        self.referenzen.len() as u64
    }

    /// Prüft das ganze Bündel.
    ///
    /// **Alles oder nichts, und die Positionen müssen lückenlos
    /// aufeinanderfolgen.** Ein Bündel, in das ein fremdes Segment
    /// eingeschoben wurde, ist kein Bündel mehr; ein Verfahren, das die
    /// gültigen Teile davon annimmt, wäre genau die Lücke, die Kap. 7.3
    /// schließen soll.
    pub fn pruefen(&self, verankerte_wurzel: &Hash, daten: &[&[u8]]) -> bool {
        if self.referenzen.is_empty() || daten.len() != self.referenzen.len() {
            return false;
        }
        for (i, referenz) in self.referenzen.iter().enumerate() {
            if referenz.position != self.start + i as u64 {
                return false;
            }
            if !referenz.pruefen(verankerte_wurzel, daten[i]) {
                return false;
            }
        }
        true
    }
}

/// Beweisknoten für ein Bündel aus `segmente` zusammenhängenden,
/// am Raster ausgerichteten Blättern in einem Baum der Tiefe `tiefe`.
///
/// **Die Herleitung, weil sie den Unterschied zu Anhang B.6.4 erklärt.**
/// Wer alle `n` Blätter eines vollständigen Teilbaums hat, kann dessen
/// Wurzel selbst ausrechnen; für die unteren `log2(n)` Ebenen braucht er
/// **keinen einzigen** Geschwisterknoten. Übertragen werden muss nur der
/// Weg von der Teilbaumwurzel zur Baumwurzel, also `tiefe − log2(n)`
/// Knoten. Insgesamt, nicht je Segment.
pub fn buendel_beweisknoten(tiefe: usize, segmente: u64) -> u64 {
    if segmente == 0 {
        return 0;
    }
    let geteilte_ebenen = (u64::BITS - segmente.leading_zeros() - 1) as u64;
    (tiefe as u64).saturating_sub(geteilte_ebenen)
}

/// Overhead eines Beweisbündels in Zehntelpromille der Nutzdaten.
///
/// `tiefe` ist die Merkle-Tiefe des Korpus, `segmente` die Bündelgröße,
/// `nutzbytes` die Nutzdaten je Segment. Ein Beweisknoten ist ein
/// SHA-256-Hash, also 32 Byte.
///
/// **Einheit ist Zehntelpromille**, nicht Promille: Bei 256 Segmenten
/// liegt der Anteil unter einem Promille, und eine Einheit, in der das
/// Ergebnis auf null gerundet wird, misst nicht mehr, sondern verschweigt.
///
/// ## Abweichung von Anhang B.6.4 (gefunden 2026-08-23)
///
/// Der Anhang nennt drei Werte. Der erste stimmt genau, die beiden
/// anderen sind zu hoch:
///
/// | Segmente | Knoten | Bytes | gerechnet | Anhang B.6.4 |
/// |---|---|---|---|---|
/// | 1 | 30 | 960 | 11,72 % | **11,7 %** ✅ |
/// | 16 | 26 | 832 | 0,63 % | **1 %** |
/// | 256 | 22 | 704 | **0,034 %** | **0,42 %** |
///
/// Beim Einzelbeweis gibt es keine Bündelung, dort kann sich nichts
/// unterscheiden. Sobald gebündelt wird, hängt alles daran, wie der
/// gemeinsame Teilbaum gezählt wird, und der Anhang zählt ihn ungünstiger
/// als nötig.
///
/// **Die Abweichung geht in die sichere Richtung:** Der Anhang gibt das
/// Verfahren teurer an, als es ist. Er überschätzt keine Eigenschaft des
/// Systems. Falsch ist er trotzdem, und für 256 Segmente um den Faktor
/// 12,5.
pub fn buendel_overhead_zehntelpromille(tiefe: usize, segmente: u64, nutzbytes: u64) -> u64 {
    if segmente == 0 || nutzbytes == 0 {
        return 0;
    }
    const KNOTEN_BYTES: u64 = 32;
    let beweis_bytes = buendel_beweisknoten(tiefe, segmente) * KNOTEN_BYTES;
    let nutz_bytes = segmente * nutzbytes;

    beweis_bytes * 10_000 / nutz_bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn korpus(n: usize) -> Korpus {
        let segmente: Vec<Vec<u8>> = (0..n)
            .map(|i| format!("Abschnitt {} des kanonischen Korpus", i).into_bytes())
            .collect();
        Korpus::verankern("wikitext2-kanonisch", segmente).expect("Verankerung")
    }

    #[test]
    fn ein_echtes_segment_belegt_seine_herkunft() {
        let k = korpus(64);
        let wurzel = k.wurzel();
        let r = k.referenz(17).expect("Referenz");
        assert!(r.pruefen(&wurzel, k.segment(17).unwrap()));
    }

    /// **Der Kern des Verfahrens.** Für eine nicht existierende Position
    /// gibt es keinen Beweis, und zwar nicht, weil die Prüfung ihn
    /// ablehnt, sondern weil er sich nicht erzeugen lässt.
    #[test]
    fn fuer_eine_nicht_existierende_position_gibt_es_keinen_beweis() {
        let k = korpus(64);
        assert_eq!(
            k.referenz(64),
            Err(ProvenienzFehler::PositionAusserhalb {
                position: 64,
                segmente: 64
            })
        );
        assert!(k.referenz(1_000_000).is_err());
    }

    /// Eigene Daten unter einem echten Beweis fallen auf.
    #[test]
    fn untergeschobene_daten_fallen_auf() {
        let k = korpus(64);
        let wurzel = k.wurzel();
        let r = k.referenz(17).expect("Referenz");
        assert!(!r.pruefen(&wurzel, b"vergifteter Text, aber echter Beweis"));
    }

    /// Ein echtes Segment an falscher Position fällt auf.
    #[test]
    fn verschobene_position_faellt_auf() {
        let k = korpus(64);
        let wurzel = k.wurzel();
        let mut r = k.referenz(17).expect("Referenz");
        r.position = 18;
        assert!(!r.pruefen(&wurzel, k.segment(17).unwrap()));
    }

    /// **Die Referenz darf nicht gegen ihre eigene Wurzel geprüft
    /// werden.** Ein Angreifer, der Daten und Wurzel selbst wählt, baut
    /// sich sonst einen gültigen Beweis. Das war Audit-Fund A11, eine
    /// Ebene höher.
    #[test]
    fn eine_selbst_mitgebrachte_wurzel_belegt_nichts() {
        let echt = korpus(64);
        let gefaelscht = Korpus::verankern("eigener-korpus", vec![b"eigener Text".to_vec()])
            .expect("Verankerung");

        let r = gefaelscht.referenz(0).expect("Referenz");
        // In sich stimmig, und trotzdem wertlos: Die verankerte Wurzel
        // ist eine andere.
        assert!(r.pruefen(&gefaelscht.wurzel(), b"eigener Text"));
        assert!(!r.pruefen(&echt.wurzel(), b"eigener Text"));
    }

    #[test]
    fn ein_buendel_belegt_alle_seine_segmente() {
        let k = korpus(256);
        let wurzel = k.wurzel();
        let b = k.buendel(32, 16).expect("Bündel");
        let daten: Vec<&[u8]> = (32..48).map(|i| k.segment(i).unwrap()).collect();
        assert!(b.pruefen(&wurzel, &daten));
        assert_eq!(b.laenge(), 16);
    }

    /// Ein eingeschobenes fremdes Segment macht das ganze Bündel
    /// ungültig, nicht nur sich selbst.
    #[test]
    fn ein_fremdes_segment_im_buendel_kippt_das_ganze_buendel() {
        let k = korpus(256);
        let wurzel = k.wurzel();
        let b = k.buendel(32, 16).expect("Bündel");
        let mut daten: Vec<&[u8]> = (32..48).map(|i| k.segment(i).unwrap()).collect();
        daten[7] = b"eingeschoben";
        assert!(!b.pruefen(&wurzel, &daten));
    }

    /// Ein Bündel mit einer Lücke in den Positionen ist keines.
    #[test]
    fn ein_buendel_mit_luecke_wird_abgelehnt() {
        let k = korpus(256);
        let wurzel = k.wurzel();
        let mut b = k.buendel(32, 4).expect("Bündel");
        b.referenzen[2] = k.referenz(200).expect("Referenz");
        let daten: Vec<&[u8]> = vec![
            k.segment(32).unwrap(),
            k.segment(33).unwrap(),
            k.segment(200).unwrap(),
            k.segment(35).unwrap(),
        ];
        assert!(!b.pruefen(&wurzel, &daten));
    }

    #[test]
    fn ein_buendel_ueber_das_korpusende_hinaus_wird_abgelehnt() {
        let k = korpus(64);
        assert!(matches!(
            k.buendel(60, 8),
            Err(ProvenienzFehler::BuendelAusserhalb { .. })
        ));
        assert_eq!(k.buendel(0, 0), Err(ProvenienzFehler::BuendelLeer));
    }

    #[test]
    fn ein_leerer_korpus_wird_abgelehnt() {
        assert_eq!(
            Korpus::verankern("leer", vec![]).unwrap_err(),
            ProvenienzFehler::KorpusLeer
        );
    }

    /// **Der Einzelbeweis aus Anhang B.6.4, nachgerechnet:** Milliarde
    /// Dokumente heißt Tiefe 30, also 960 Byte gegen 8192 Byte
    /// Nutzdaten, mithin 11,7 Prozent. Hier stimmt der Anhang genau.
    #[test]
    fn der_einzelbeweis_stimmt_mit_anhang_b_6_4() {
        const TIEFE: usize = 30;
        const NUTZ: u64 = 8192;

        assert_eq!(buendel_beweisknoten(TIEFE, 1), 30);
        // 960 / 8192 = 11,72 %
        assert_eq!(buendel_overhead_zehntelpromille(TIEFE, 1, NUTZ), 1171);
    }

    /// **Die gebündelten Werte weichen von Anhang B.6.4 ab, und zwar
    /// nach unten.** Festgehalten als Test, damit die Abweichung nicht
    /// wieder aus dem Blick gerät: Der Anhang gibt das Verfahren teurer
    /// an, als es ist.
    ///
    /// Wer alle Blätter eines vollständigen Teilbaums hat, braucht für
    /// dessen untere Ebenen **keinen** Geschwisterknoten; übertragen wird
    /// nur der Weg von der Teilbaumwurzel nach oben.
    #[test]
    fn gebuendelte_beweise_sind_guenstiger_als_im_anhang_angegeben() {
        const TIEFE: usize = 30;
        const NUTZ: u64 = 8192;

        // 16 Segmente: 26 Knoten, 832 Byte, 0,63 % statt „ein Prozent".
        assert_eq!(buendel_beweisknoten(TIEFE, 16), 26);
        assert_eq!(buendel_overhead_zehntelpromille(TIEFE, 16, NUTZ), 63);

        // 256 Segmente: 22 Knoten, 704 Byte, 0,034 % statt 0,42 %.
        assert_eq!(buendel_beweisknoten(TIEFE, 256), 22);
        assert_eq!(buendel_overhead_zehntelpromille(TIEFE, 256, NUTZ), 3);
    }

    /// Mehr Bündelung kostet nie mehr, und der Beweis schrumpft mit jeder
    /// Verdopplung um genau einen Knoten.
    #[test]
    fn mehr_buendelung_kostet_nie_mehr() {
        const TIEFE: usize = 30;
        const NUTZ: u64 = 8192;

        let mut vorher = u64::MAX;
        for n in [1u64, 2, 4, 16, 64, 256, 1024] {
            let jetzt = buendel_overhead_zehntelpromille(TIEFE, n, NUTZ);
            assert!(jetzt <= vorher, "n={n}: {jetzt} > {vorher}");
            vorher = jetzt;
        }
        for stufe in 0..10u32 {
            let n = 1u64 << stufe;
            assert_eq!(buendel_beweisknoten(TIEFE, n), 30 - stufe as u64);
        }
    }

    /// Ein Bündel, das den ganzen Baum umfasst, braucht keinen Beweis:
    /// Wer alle Blätter hat, rechnet die Wurzel selbst aus.
    #[test]
    fn ein_buendel_ueber_den_ganzen_baum_braucht_keinen_beweis() {
        assert_eq!(buendel_beweisknoten(4, 16), 0);
        assert_eq!(buendel_overhead_zehntelpromille(4, 16, 8192), 0);
    }

    /// Ein Korpus mit einem einzigen Segment hat die Wurzel gleich dem
    /// Blatt-Hash. Das ist eine Festlegung von `myl-types::merkle`, und
    /// die Provenienz muss auch dort tragen.
    #[test]
    fn ein_korpus_aus_einem_segment_traegt_auch() {
        let k = Korpus::verankern("winzig", vec![b"einziger Abschnitt".to_vec()]).unwrap();
        let wurzel = k.wurzel();
        let r = k.referenz(0).unwrap();
        assert!(r.pruefen(&wurzel, b"einziger Abschnitt"));
        assert!(!r.pruefen(&wurzel, b"anderer Abschnitt"));
    }
}
