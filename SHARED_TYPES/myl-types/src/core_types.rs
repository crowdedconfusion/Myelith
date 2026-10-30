//! Kern-Structs aus Whitepaper Anhang A.1 (Punkt 1.5):
//! `Segment`, `PoIBundle`, `InferenceCredit`.
//!
//! **Feldnamen und Feldreihenfolge sind Konsens-Vertrag:** Borsh
//! serialisiert in Deklarationsreihenfolge, jede Abweichung von
//! Anhang A.1 würde die Hashes über diesen Strukturen (und damit alle
//! Commitments und Bündel-Wurzeln) ändern. Änderungen nur über
//! Governance (Kap. 10.3).

use borsh::{BorshDeserialize, BorshSerialize};

use crate::bls::BlsSignature;
use crate::hash::Hash;
use crate::ids::{ActivationHash, Address, EpochId, MerkleRoot, MinerId, PodId, SegmentId};
use crate::merkle::{MerkleError, MerkleTree};

/// Ein Inferenz-Segment σ = (x, θ_v, π, y) als übertragbarer Datensatz
/// (Whitepaper Kap. 6.1, Anhang A.1).
///
/// - `id`: `h(session ‖ index)`
/// - `input_commitment`: `h(prompt_chunk ‖ kv_root)`
/// - `model_version`: Gewichts-Wurzel θ_v inkl. Ausführungsspezifikation
/// - `pod_path`: Pipeline-Reihenfolge der Shard-Miner
/// - `trace`: Berechnungsspur h(a_0), …, h(a_k)
/// - `signatures`: eine BLS-Signatur pro Shard-Übergang
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Segment {
    pub id: SegmentId,
    pub input_commitment: Hash,
    pub model_version: MerkleRoot,
    pub pod_path: Vec<MinerId>,
    pub output_commitment: Hash,
    pub trace: Vec<ActivationHash>,
    pub signatures: Vec<BlsSignature>,
}

/// PoI-Bündel: pro Epoche und Pod eingereicht, signiert und aggregiert
/// (Whitepaper Kap. 4.4, Anhang A.1).
///
/// - `segments_root`: Merkle-Wurzel über alle Segment-Ids der Epoche
/// - `vtfe_claimed`: beanspruchte Arbeit (vTFE)
/// - `aggregate_sig`: BLS-Aggregat über die Pod-Mitglieder
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct PoIBundle {
    pub epoch: EpochId,
    pub pod: PodId,
    pub segments_root: MerkleRoot,
    pub vtfe_claimed: u64,
    pub aggregate_sig: BlsSignature,
    /// Wie viele Segmente unter [`Self::segments_root`] hängen.
    ///
    /// # ⚑ Fund 115: Die Kette konnte nicht zählen, was sie bezahlt
    ///
    /// Bis zum 2026-09-01 trug ein Bündel nur die **Wurzel** über seine
    /// Segmentzeugnisse. Eine Wurzel sagt nichts über die Zahl der
    /// Blätter, und damit war aus dem Kettenzustand **nicht ableitbar,
    /// wie viele Segmente eine Epoche hatte**.
    ///
    /// Das war die stille Vorbedingung, an der Stufe 2 scheiterte:
    /// [`sample_segments`](../../../CONSENSUS/myl-scheduler) zieht aus
    /// `num_segments`, und diese Zahl gab es nirgends. **Erst mit ihr
    /// ist eine Stichprobe überhaupt herleitbar.**
    ///
    /// ⚑ **Sie gehört in die signierte Botschaft**, aus demselben Grund
    /// wie `vtfe_claimed`: Sonst erhöhte der Koordinator sie nach dem
    /// Einsammeln der Unterschriften und **verdünnte damit die
    /// Stichprobenwahrscheinlichkeit je Segment**, ohne das Aggregat
    /// ungültig zu machen.
    ///
    /// **Additiv angehängt, nie eingefügt:** Die Feldreihenfolge ist
    /// Konsensvertrag.
    pub segmente: u32,
    /// ⚑ **Die Ausgabenwurzel** (Stufe 1, Whitepaper 3.4; Festlegung des
    /// Projektinhabers vom 2026-10-28): je Segment Kennung, Eingang und
    /// letzte Ausgabe ([`ausgabenwurzel`]), **nach Kennung geordnet**. Zwei
    /// Zwillinge, die dieselben Segmente gleich gerechnet haben, tragen
    /// dieselbe, gleich in welcher Folge sie sie abschlossen, und nur dann
    /// vergütet die Kette. [`Self::segments_root`] taugt dafür nicht: Sie
    /// folgt der Folge des Abschlusses (die Spur selbst wird je Ebene geführt
    /// und hängt nicht am Schnitt, geprüft in `myl-pod`,
    /// `tests/grenzen_der_kette.rs`).
    ///
    /// Null heisst: keine (ein Pod ohne Zwilling). Sie wird mitunterschrieben,
    /// sobald sie nicht null ist (`poi_botschaft::bundle_message`).
    /// **Angehängt, nie eingefügt.**
    pub ausgabenwurzel: MerkleRoot,
    /// ⚑ **Wer für eine Position unterschrieben hat, weil ihr Inhaber
    /// ausfiel** (Whitepaper 6.8, Festlegung des Projektinhabers vom
    /// 2026-10-28): je Position das Reservemitglied des Pods, das sie
    /// übernahm. Die Kette prüft das Aggregat mit seinem Schlüssel statt dem
    /// des Inhabers und vergütet die Position an ihn. Leer: keine Übernahme.
    /// Mitunterschrieben, sobald nicht leer. Angehängt.
    pub vertreter: Vec<(u32, crate::ids::MinerId)>,
}

/// Inferenz-Credit: durch Burn erworbenes Guthaben an Inferenzarbeit
/// (Whitepaper Kap. 5, Anhang A.1).
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct InferenceCredit {
    pub owner: Address,
    pub vtfe: u64,
    pub expiry: EpochId,
}

/// Merkle-Wurzel über die **Spur** eines Segments.
///
/// Ein Blatt je Spur-Eintrag, damit sich ein **einzelner** Eintrag
/// beweisen lässt, ohne die ganze Spur zu zeigen. Genau das braucht die
/// Schiedsrunde: Sie streitet über eine Layer, nicht über ein Segment.
pub fn spurwurzel(spur: &[[u8; 32]]) -> Result<MerkleRoot, MerkleError> {
    let refs: Vec<&[u8]> = spur.iter().map(|h| h.as_slice()).collect();
    let tree = MerkleTree::new(&refs)?;
    Ok(MerkleRoot::new(tree.root().0))
}

/// Was ein Pod je Segment bezeugt: die Kennung **und** das Ergebnis.
///
/// # ⚑ Warum das Ergebnis dazugehört (Fund 100, 2026-08-30)
///
/// `segments_root` war bis dahin eine Wurzel über die bloßen
/// Segment-Ids, und eine `SegmentId` ist `(Sitzungsnummer, Position)`
/// mit Nullen aufgefüllt. Sie bindet **nichts**: weder die Spur noch
/// Ein- oder Ausgabe.
///
/// Damit beanspruchte ein PoI-Bündel Arbeit, **ohne zu sagen, was
/// gerechnet wurde**. Ein Pod konnte `n` Paare `(Sitzung, Position)`
/// aufzählen und dafür vergütet werden; die Spur lag nur örtlich beim
/// Koordinator und war an nichts gebunden.
///
/// Der ganze Streitpfad hing daran: Die Schiedsrunde will feststellen,
/// ob der Angeklagte **das** gerechnet hat, was er behauptet hat. Ohne
/// eine Zusicherung gibt es kein „behauptet", nur zwei einander
/// widersprechende Aussagen und keinen Grund, einer zu glauben.
///
/// Das Blatt ist jetzt `Id ‖ Spurwurzel`. Ein Beweis, dass der
/// Angeklagte für Segment `s` die Spur mit Wurzel `w` bezeugt hat, ist
/// damit ein Merkle-Pfad in dieser Wurzel; ein Beweis über einen
/// **einzelnen** Spur-Eintrag ein zweiter innerhalb von `w`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Segmentzeugnis {
    /// Die Kennung des Segments.
    pub id: SegmentId,
    /// Die Wurzel über seine Spur ([`spurwurzel`]).
    pub spurwurzel: MerkleRoot,
}

/// Was ein Pod je Segment über das Ergebnis bezeugt, unabhängig von seinem
/// Schnitt: Kennung, Eingang (Hash der Token-Nutzlast) und die letzte
/// Ausgabe der Spur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Ausgabenzeugnis {
    /// Die Kennung des Segments.
    pub id: SegmentId,
    /// Der Eingang des Segments.
    pub eingang: [u8; 32],
    /// Die letzte Ausgabe seiner Spur.
    pub ausgang: [u8; 32],
}

/// **Die Ausgabenwurzel eines Bündels** (Stufe 1): Merkle-Wurzel über
/// `Id ‖ Eingang ‖ Ausgang`, **nach Kennung geordnet**, damit zwei Zwillinge
/// dieselbe Wurzel haben, gleich in welcher Folge sie die Segmente
/// abschlossen.
pub fn ausgabenwurzel(zeugnisse: &[Ausgabenzeugnis]) -> Result<MerkleRoot, MerkleError> {
    let mut geordnet: Vec<&Ausgabenzeugnis> = zeugnisse.iter().collect();
    geordnet.sort_by(|a, b| a.id.as_ref().cmp(b.id.as_ref()));
    let blaetter: Vec<Vec<u8>> = geordnet
        .iter()
        .map(|z| {
            let mut b = Vec::with_capacity(96);
            b.extend_from_slice(z.id.as_ref());
            b.extend_from_slice(&z.eingang);
            b.extend_from_slice(&z.ausgang);
            b
        })
        .collect();
    let refs: Vec<&[u8]> = blaetter.iter().map(|b| b.as_slice()).collect();
    let tree = MerkleTree::new(&refs)?;
    Ok(MerkleRoot::new(tree.root().0))
}

/// Das Blatt eines [`Ausgabenzeugnis`] in der [`ausgabenwurzel`].
fn ausgabenblatt(z: &Ausgabenzeugnis) -> Vec<u8> {
    let mut b = Vec::with_capacity(96);
    b.extend_from_slice(z.id.as_ref());
    b.extend_from_slice(&z.eingang);
    b.extend_from_slice(&z.ausgang);
    b
}

/// Das Blatt eines [`Segmentzeugnis`] in [`segments_root`].
fn segmentblatt(z: &Segmentzeugnis) -> Vec<u8> {
    let mut b = Vec::with_capacity(64);
    b.extend_from_slice(z.id.as_ref());
    b.extend_from_slice(z.spurwurzel.as_ref());
    b
}

/// ⚑ **Eine Seite eines Differenzbeweises** (Stufe 1 auf Stufe 2,
/// Whitepaper 6.4 und 6.6, Festlegung des Projektinhabers vom 2026-10-29):
/// was ein Pod für ein Segment bezeugt hat, mit Beweis gegen seine
/// Ausgabenwurzel, und an welcher Stelle seines Bündels das Segment steht,
/// mit Beweis gegen `segments_root`.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Differenzseite {
    pub ausgabe: Ausgabenzeugnis,
    pub ausgabenbeweis: crate::merkle::MerkleProof,
    pub segment: Segmentzeugnis,
    pub segmentbeweis: crate::merkle::MerkleProof,
}

/// ⚑ **Der Differenzbeweis zweier Zwillinge**: dasselbe Segment, zwei
/// verschiedene Ausgaben. Er verortet die Abweichung, wie Verde und
/// Arbitrum BoLD einen Streit auf eine Stelle eingrenzen; gezogen wird dann
/// genau dieses Segment, in beiden Pods. Niemand muss dem glauben, der ihn
/// meldet: Jede Seite trägt sich selbst gegen die Wurzeln der Bündel.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Differenzbeweis {
    pub a: Differenzseite,
    pub b: Differenzseite,
}

/// Warum ein Differenzbeweis nicht trägt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Differenzfehler {
    /// Die Kennungen der Blätter sind nicht dieselben.
    AndereKennung,
    /// Beide Seiten bezeugen dasselbe: keine Differenz.
    KeineDifferenz,
    /// Ein Beweis trägt nicht gegen seine Wurzel (Seite `a` ist `false`).
    TraegtNicht { seite_b: bool },
}

/// **Eine Seite bauen**: aus den Ausgabenzeugnissen und den Segmentzeugnissen
/// (in Bündelfolge) eines Pods, für die Kennung `id`. `None`, wenn das
/// Segment fehlt.
pub fn differenzseite(ausgaben: &[Ausgabenzeugnis], segmente: &[Segmentzeugnis], id: &SegmentId) -> Option<Differenzseite> {
    let mut geordnet: Vec<&Ausgabenzeugnis> = ausgaben.iter().collect();
    geordnet.sort_by(|a, b| a.id.as_ref().cmp(b.id.as_ref()));
    let i = geordnet.iter().position(|z| z.id == *id)?;
    let blaetter: Vec<Vec<u8>> = geordnet.iter().map(|z| ausgabenblatt(z)).collect();
    let refs: Vec<&[u8]> = blaetter.iter().map(|b| b.as_slice()).collect();
    let ausgabenbeweis = MerkleTree::new(&refs).ok()?.proof(i).ok()?;
    let j = segmente.iter().position(|z| z.id == *id)?;
    let blaetter: Vec<Vec<u8>> = segmente.iter().map(segmentblatt).collect();
    let refs: Vec<&[u8]> = blaetter.iter().map(|b| b.as_slice()).collect();
    let segmentbeweis = MerkleTree::new(&refs).ok()?.proof(j).ok()?;
    Some(Differenzseite { ausgabe: *geordnet[i], ausgabenbeweis, segment: segmente[j], segmentbeweis })
}

/// **Einen Differenzbeweis prüfen** gegen (Ausgabenwurzel, `segments_root`,
/// Segmentzahl) der beiden Bündel. Ergebnis: die Stelle des Segments in
/// Bündel `a` und in Bündel `b`.
pub fn differenz_pruefen(
    beweis: &Differenzbeweis,
    a: (&MerkleRoot, &MerkleRoot, u32),
    b: (&MerkleRoot, &MerkleRoot, u32),
) -> Result<(u32, u32), Differenzfehler> {
    let id = beweis.a.ausgabe.id;
    if beweis.a.segment.id != id || beweis.b.ausgabe.id != id || beweis.b.segment.id != id {
        return Err(Differenzfehler::AndereKennung);
    }
    if beweis.a.ausgabe == beweis.b.ausgabe {
        return Err(Differenzfehler::KeineDifferenz);
    }
    let seite = |s: &Differenzseite, (ausgabenwurzel, segments_root, n): (&MerkleRoot, &MerkleRoot, u32)| {
        let ab = &s.ausgabenbeweis;
        let sb = &s.segmentbeweis;
        ab.leaf_count == u64::from(n)
            && sb.leaf_count == u64::from(n)
            && ab.verify(&Hash(*ausgabenwurzel.as_bytes()), &ausgabenblatt(&s.ausgabe), ab.leaf_index)
            && sb.verify(&Hash(*segments_root.as_bytes()), &segmentblatt(&s.segment), sb.leaf_index)
    };
    if !seite(&beweis.a, a) {
        return Err(Differenzfehler::TraegtNicht { seite_b: false });
    }
    if !seite(&beweis.b, b) {
        return Err(Differenzfehler::TraegtNicht { seite_b: true });
    }
    let stelle = |s: &Differenzseite| u32::try_from(s.segmentbeweis.leaf_index).unwrap_or(u32::MAX);
    Ok((stelle(&beweis.a), stelle(&beweis.b)))
}

/// Merkle-Wurzel über die Segmente einer Epoche, die
/// `segments_root`-Konstruktion aus [`PoIBundle`].
///
/// Blätter sind `Id ‖ Spurwurzel` in Bündel-Reihenfolge. Siehe
/// [`Segmentzeugnis`] dazu, warum die Spurwurzel dazugehört.
pub fn segments_root(zeugnisse: &[Segmentzeugnis]) -> Result<MerkleRoot, MerkleError> {
    let blaetter: Vec<Vec<u8>> = zeugnisse
        .iter()
        .map(|z| {
            let mut b = Vec::with_capacity(64);
            b.extend_from_slice(z.id.as_ref());
            b.extend_from_slice(z.spurwurzel.as_ref());
            b
        })
        .collect();
    let refs: Vec<&[u8]> = blaetter.iter().map(|b| b.as_slice()).collect();
    let tree = MerkleTree::new(&refs)?;
    Ok(MerkleRoot::new(tree.root().0))
}

#[cfg(test)]
mod tests {

    /// ⚑ **Stufe 2, die Lieferung:** Eine Ablage beantwortet jede Anfrage
    /// mit einem Beweis, der trägt; jede Veränderung fällt auf.
    #[test]
    fn eine_abgelegte_spur_traegt_gegen_die_wurzel() {
        let eintraege: Vec<(Segmentzeugnis, Vec<Hash>)> = (0..5u8)
            .map(|i| {
                let spur: Vec<Hash> = (0..4u8).map(|j| Hash::sha256(&[i, j])).collect();
                let roh: Vec<[u8; 32]> = spur.iter().map(|h| *h.as_bytes()).collect();
                (Segmentzeugnis { id: SegmentId::new([i; 32]), spurwurzel: spurwurzel(&roh).expect("Wurzel") }, spur)
            })
            .collect();
        let zeugnisse: Vec<Segmentzeugnis> = eintraege.iter().map(|(z, _)| *z).collect();
        let wurzel = segments_root(&zeugnisse).expect("Wurzel");
        let ablage = Spurablage { epoche: EpochId(4), pod: PodId::new([1; 32]), segments_root: wurzel, eintraege, eingaben: Vec::new() };
        let frage = |segment: u32| Spuranfrage { epoche: EpochId(4), pod: PodId::new([1; 32]), segment };
        for s in 0..5 {
            let a = ablage.antwort(&frage(s)).expect("Antwort");
            assert_eq!(lieferung_pruefen(&a, &wurzel, 5), Ok(()), "Segment {s}");
        }
        let a = ablage.antwort(&frage(2)).expect("Antwort");
        // ⛔️ Gegenproben: andere Segmentzahl, andere Wurzel, veränderte Spur,
        //    Beweis eines anderen Index, Anfrage an einen anderen Pod oder
        //    ausserhalb, Ablage mit falscher Wurzel.
        assert!(matches!(lieferung_pruefen(&a, &wurzel, 6), Err(Lieferfehler::BlattzahlPasstNicht { .. })));
        assert_eq!(lieferung_pruefen(&a, &MerkleRoot::new([9; 32]), 5), Err(Lieferfehler::BeweisTraegtNicht));
        let mut falsch = a.clone();
        falsch.spur[1] = Hash::sha256(b"anders");
        assert_eq!(lieferung_pruefen(&falsch, &wurzel, 5), Err(Lieferfehler::SpurPasstNichtZumZeugnis));
        let mut index = a.clone();
        index.anfrage.segment = 3;
        assert!(matches!(lieferung_pruefen(&index, &wurzel, 5), Err(Lieferfehler::FalschesSegment { .. })));
        assert!(ablage.antwort(&Spuranfrage { pod: PodId::new([2; 32]), ..frage(1) }).is_none());
        assert!(ablage.antwort(&frage(5)).is_none());
        assert!(Spurablage { segments_root: MerkleRoot::new([9; 32]), ..ablage.clone() }.antwort(&frage(1)).is_none());
    }

    /// ⚑ **Stufe 2, der Verlauf:** Eine Ablage mit zwei Sitzungen liefert zu
    /// einem gezogenen Segment genau die Segmente seiner Sitzung bis zu ihm,
    /// lückenlos ab null, mit Tokens, die an die Spur gebunden sind.
    #[test]
    fn der_verlauf_liefert_die_tokens_bis_zum_gezogenen_segment() {
        let id = |s: u64, p: u64| {
            let mut b = [0u8; 32];
            b[..8].copy_from_slice(&s.to_le_bytes());
            b[8..16].copy_from_slice(&p.to_le_bytes());
            SegmentId::new(b)
        };
        let packen = |t: u32| vec![(t & 0xFFFF) as i16, (t >> 16) as i16];
        // Zwei Sitzungen verschränkt, wie ein Pod sie abschliesst: 7 hat
        // Positionen 0..4, 9 hat 0..3; Token = 70 000 + 10 * Sitzung + Position.
        let folge = [(7, 0), (9, 0), (7, 1), (9, 1), (7, 2), (7, 3), (9, 2)];
        let mut eintraege = Vec::new();
        let mut eingaben = Vec::new();
        for (s, p) in folge {
            let eingabe = packen(70_000 + 10 * s as u32 + p as u32);
            let mut spur = vec![Hash(crate::uebergang::activation_hash(&eingabe))];
            spur.extend((0..3u8).map(|j| Hash::sha256(&[s as u8, p as u8, j])));
            let roh: Vec<[u8; 32]> = spur.iter().map(|h| *h.as_bytes()).collect();
            eintraege.push((Segmentzeugnis { id: id(s, p), spurwurzel: spurwurzel(&roh).expect("Wurzel") }, spur));
            eingaben.push(eingabe);
        }
        let zeugnisse: Vec<Segmentzeugnis> = eintraege.iter().map(|(z, _)| *z).collect();
        let wurzel = segments_root(&zeugnisse).expect("Wurzel");
        let ablage = Spurablage { epoche: EpochId(4), pod: PodId::new([1; 32]), segments_root: wurzel, eintraege, eingaben };
        let frage = |segment: u32| Spuranfrage { epoche: EpochId(4), pod: PodId::new([1; 32]), segment };

        // Gezogen: Sitzung 7, Position 2 (Index 4).
        let verlauf = ablage.verlauf(&frage(4)).expect("Verlauf");
        assert_eq!(verlauf_pruefen(&verlauf, &wurzel, 7), Ok(vec![70_070, 70_071, 70_072]));
        // Die einzelne Antwort trägt jetzt auch die Eingabe.
        let einzeln = ablage.antwort(&frage(4)).expect("Antwort");
        assert_eq!(einzeln, verlauf[2]);
        assert_eq!(verlauf_pruefen(&ablage.verlauf(&frage(1)).expect("Verlauf"), &wurzel, 7), Ok(vec![70_090]));

        // ⛔️ Gegenproben: ein untergeschobenes Token, eine Lücke, eine fremde
        //    Sitzung, eine falsche Wurzel, eine Ablage ohne Eingaben.
        let mut falsch = verlauf.clone();
        falsch[1].eingabe = eingabe_als_bytes(&packen(70_099));
        assert_eq!(verlauf_pruefen(&falsch, &wurzel, 7), Err(Verlauffehler::Eingabe { stelle: 1 }));
        let luecke = vec![verlauf[0].clone(), verlauf[2].clone()];
        assert_eq!(verlauf_pruefen(&luecke, &wurzel, 7), Err(Verlauffehler::Luecke { stelle: 1, position: 2 }));
        let mut fremd = verlauf.clone();
        fremd[1] = ablage.antwort(&frage(3)).expect("Antwort");
        assert_eq!(verlauf_pruefen(&fremd, &wurzel, 7), Err(Verlauffehler::FremdeSitzung { stelle: 1 }));
        assert!(matches!(
            verlauf_pruefen(&verlauf, &MerkleRoot::new([9; 32]), 7),
            Err(Verlauffehler::Lieferung { stelle: 0, .. })
        ));
        assert!(Spurablage { eingaben: Vec::new(), ..ablage.clone() }.verlauf(&frage(4)).is_none());
        assert_eq!(verlauf_pruefen(&[], &wurzel, 7), Err(Verlauffehler::Leer));
        assert_eq!(token_aus_eingabe(&packen(123_456)), Some(123_456));
        assert_eq!(token_aus_eingabe(&[1]), None);
    }

    /// ⚑ **Stufe 1:** Die Ausgabenwurzel hängt nicht an der Reihenfolge,
    /// aber an jedem Eingang, jedem Ausgang und jeder Kennung.
    #[test]
    fn die_ausgabenwurzel_ist_von_der_reihenfolge_frei() {
        let z = |i: u8, e: u8, a: u8| Ausgabenzeugnis { id: SegmentId::new([i; 32]), eingang: [e; 32], ausgang: [a; 32] };
        let w = ausgabenwurzel(&[z(1, 2, 3), z(4, 5, 6), z(7, 8, 9)]).expect("Wurzel");
        assert_eq!(w, ausgabenwurzel(&[z(7, 8, 9), z(1, 2, 3), z(4, 5, 6)]).expect("Wurzel"), "andere Folge, dieselbe Wurzel");
        // ⛔️ Gegenproben: ein anderer Ausgang, Eingang, eine andere Kennung,
        //    ein Segment weniger.
        assert_ne!(w, ausgabenwurzel(&[z(1, 2, 3), z(4, 5, 0), z(7, 8, 9)]).expect("Wurzel"));
        assert_ne!(w, ausgabenwurzel(&[z(1, 2, 3), z(4, 0, 6), z(7, 8, 9)]).expect("Wurzel"));
        assert_ne!(w, ausgabenwurzel(&[z(1, 2, 3), z(5, 5, 6), z(7, 8, 9)]).expect("Wurzel"));
        assert_ne!(w, ausgabenwurzel(&[z(1, 2, 3), z(4, 5, 6)]).expect("Wurzel"));
    }

    /// Die Botschaft eines Bündels trägt die Ausgabenwurzel, sobald sie
    /// nicht null ist; ohne sie bleibt sie, wie sie war.
    #[test]
    fn die_botschaft_traegt_die_ausgabenwurzel() {
        let b = PoIBundle {
            epoch: EpochId(3),
            pod: PodId::new([1; 32]),
            segments_root: MerkleRoot::new([2; 32]),
            vtfe_claimed: 9,
            aggregate_sig: BlsSignature([0; 96]),
            segmente: 4,
            ausgabenwurzel: MerkleRoot::new([0; 32]),
            vertreter: Vec::new(),
        };
        let ohne = crate::poi_botschaft::bundle_message(&b);
        assert_eq!(ohne, crate::poi_botschaft::poi_bundle_message(b.epoch, b.pod, &b.segments_root, 9, 4));
        let mit = PoIBundle { ausgabenwurzel: MerkleRoot::new([7; 32]), ..b.clone() };
        let m = crate::poi_botschaft::bundle_message(&mit);
        assert_eq!(m.len(), ohne.len() + 32);
        assert_ne!(m, crate::poi_botschaft::bundle_message(&PoIBundle { ausgabenwurzel: MerkleRoot::new([8; 32]), ..b }));
    }

    use super::*;
    use borsh::{from_slice, to_vec};

    /// Deterministischer PRNG (xorshift64) für die Property-Tests —
    /// die Testsuite soll ohne Zufalls-Abhängigkeit reproduzierbar sein.
    struct Xorshift(u64);

    impl Xorshift {
        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }

        fn fill<const N: usize>(&mut self) -> [u8; N] {
            let mut out = [0u8; N];
            for chunk in out.chunks_mut(8) {
                let v = self.next_u64().to_le_bytes();
                chunk.copy_from_slice(&v[..chunk.len()]);
            }
            out
        }
    }

    fn zufalls_segment(rng: &mut Xorshift) -> Segment {
        let n_miner = (rng.next_u64() % 4) as usize;
        let n_trace = (rng.next_u64() % 8) as usize;
        Segment {
            id: SegmentId::new(rng.fill()),
            input_commitment: Hash::from_bytes(rng.fill()),
            model_version: MerkleRoot::new(rng.fill()),
            pod_path: (0..n_miner).map(|_| MinerId::new(rng.fill())).collect(),
            output_commitment: Hash::from_bytes(rng.fill()),
            trace: (0..n_trace)
                .map(|_| ActivationHash::new(rng.fill()))
                .collect(),
            signatures: Vec::new(), // BLS-Signaturen sind separat getestet
        }
    }

    fn zufalls_bundle(rng: &mut Xorshift) -> PoIBundle {
        PoIBundle {
            epoch: EpochId(rng.next_u64()),
            pod: PodId::new(rng.fill()),
            segments_root: MerkleRoot::new(rng.fill()),
            vtfe_claimed: rng.next_u64(),
            aggregate_sig: BlsSignature(rng.fill()),
            segmente: 1,
            ausgabenwurzel: MerkleRoot::new([0u8; 32]),
            vertreter: Vec::new(),
        }
    }

    fn zufalls_credit(rng: &mut Xorshift) -> InferenceCredit {
        InferenceCredit {
            owner: Address::new(rng.fill()),
            vtfe: rng.next_u64(),
            expiry: EpochId(rng.next_u64()),
        }
    }

    /// Akzeptanzkriterium Phase 1: `serialize(deserialize(x)) == x` für
    /// zufällige Instanzen, mindestens 10.000 Fälle je Typ.
    #[test]
    fn roundtrip_zufaellige_instanzen() {
        let mut rng = Xorshift(0x5eed_5eed_5eed_5eed);
        for _ in 0..10_000 {
            let segment = zufalls_segment(&mut rng);
            let bytes = to_vec(&segment).expect("Serialisierung");
            let back: Segment = from_slice(&bytes).expect("Deserialisierung");
            assert_eq!(back, segment);
            assert_eq!(to_vec(&back).expect("Re-Serialisierung"), bytes);
        }
        for _ in 0..10_000 {
            let bundle = zufalls_bundle(&mut rng);
            let bytes = to_vec(&bundle).expect("Serialisierung");
            let back: PoIBundle = from_slice(&bytes).expect("Deserialisierung");
            assert_eq!(back, bundle);
            assert_eq!(to_vec(&back).expect("Re-Serialisierung"), bytes);
        }
        for _ in 0..10_000 {
            let credit = zufalls_credit(&mut rng);
            let bytes = to_vec(&credit).expect("Serialisierung");
            let back: InferenceCredit = from_slice(&bytes).expect("Deserialisierung");
            assert_eq!(back, credit);
            assert_eq!(to_vec(&back).expect("Re-Serialisierung"), bytes);
        }
    }

    #[test]
    fn serialisierung_ist_deterministisch() {
        let mut rng_a = Xorshift(42);
        let mut rng_b = Xorshift(42);
        let a = zufalls_segment(&mut rng_a);
        let b = zufalls_segment(&mut rng_b);
        assert_eq!(to_vec(&a).expect("ser"), to_vec(&b).expect("ser"));
    }

    fn zeugnisse(n: u8) -> Vec<Segmentzeugnis> {
        (0..n)
            .map(|i| {
                let mut bytes = [0u8; 32];
                bytes[0] = i;
                Segmentzeugnis {
                    id: SegmentId::new(bytes),
                    spurwurzel: spurwurzel(&[[i; 32], [i.wrapping_add(1); 32]]).expect("Wurzel"),
                }
            })
            .collect()
    }

    #[test]
    fn segments_root_stimmt_mit_merkle_baum_ueberein() {
        let z = zeugnisse(5);
        let root = segments_root(&z).expect("Wurzel");
        // Manuell über den Merkle-Baum mit denselben Blättern.
        let blaetter: Vec<Vec<u8>> = z
            .iter()
            .map(|e| {
                let mut b = e.id.as_ref().to_vec();
                b.extend_from_slice(e.spurwurzel.as_ref());
                b
            })
            .collect();
        let refs: Vec<&[u8]> = blaetter.iter().map(|b| b.as_slice()).collect();
        let tree = MerkleTree::new(&refs).expect("Baum");
        assert_eq!(root, MerkleRoot::new(tree.root().0));
        // Mitgliedschaftsbeweis für ein Blatt muss verifizieren.
        let proof = tree.proof(2).expect("Beweis");
        assert!(proof.verify_hashed(&tree.root(), &crate::merkle::leaf_hash(&blaetter[2])));
    }

    /// ⚑ **Der Kern von Fund 100: Die Wurzel muss sich ändern, wenn sich
    /// die Spur ändert.**
    ///
    /// Vorher war sie eine Wurzel über bloße Ids, und eine Id ist
    /// `(Sitzung, Position)`. Zwei Pods, die dieselben Positionen
    /// rechnen und **verschiedene Ergebnisse** bekommen, hatten damit
    /// dieselbe Wurzel: Das Bündel beanspruchte Arbeit, ohne zu sagen,
    /// was gerechnet wurde.
    #[test]
    fn eine_andere_spur_ergibt_eine_andere_wurzel() {
        let mut a = zeugnisse(3);
        let wurzel_a = segments_root(&a).expect("Wurzel");
        // Dieselben Ids, eine andere Spur.
        a[1].spurwurzel = spurwurzel(&[[99u8; 32]]).expect("Wurzel");
        let wurzel_b = segments_root(&a).expect("Wurzel");
        assert_ne!(
            wurzel_a, wurzel_b,
            "die Bündelwurzel bezeugt das Ergebnis nicht"
        );
    }

    /// Und die Spurwurzel selbst trägt jeden einzelnen Eintrag: Genau
    /// das braucht die Schiedsrunde, die über **eine** Layer streitet.
    #[test]
    fn ein_einzelner_spureintrag_ist_beweisbar() {
        let spur: Vec<[u8; 32]> = (0..7u8).map(|i| [i; 32]).collect();
        let refs: Vec<&[u8]> = spur.iter().map(|h| h.as_slice()).collect();
        let tree = MerkleTree::new(&refs).expect("Baum");
        assert_eq!(spurwurzel(&spur).expect("Wurzel"), MerkleRoot::new(tree.root().0));
        let proof = tree.proof(4).expect("Beweis");
        assert!(proof.verify_hashed(&tree.root(), &crate::merkle::leaf_hash(&spur[4])));
        // Ein anderer Eintrag passt nicht an diese Stelle.
        assert!(!proof.verify_hashed(&tree.root(), &crate::merkle::leaf_hash(&spur[5])));
    }

    #[test]
    fn segments_root_leer_wird_abgelehnt() {
        assert!(matches!(segments_root(&[]), Err(MerkleError::Empty)));
    }

    #[test]
    fn segment_feldreihenfolge_ist_fest() {
        // Anhang A.1 schreibt die Feldreihenfolge vor; Borsh kodiert in
        // Deklarationsreihenfolge. Dieser Test fixiert die Serialisierung
        // eines Minimal-Segments als Golden-Byte-Folge: Jede Änderung der
        // Feldreihenfolge oder der Typen bricht sie (und damit den
        // Konsens) und schlägt hier an.
        let segment = Segment {
            id: SegmentId::new([0u8; 32]),
            input_commitment: Hash::from_bytes([0u8; 32]),
            model_version: MerkleRoot::new([0u8; 32]),
            pod_path: Vec::new(),
            output_commitment: Hash::from_bytes([0u8; 32]),
            trace: Vec::new(),
            signatures: Vec::new(),
        };
        let bytes = to_vec(&segment).expect("Serialisierung");
        // 32 (id) + 32 (input) + 32 (version) + 4 (Vec-Länge) + 32 (output)
        // + 4 (Vec-Länge) + 4 (Vec-Länge) = 140 Bytes.
        assert_eq!(bytes.len(), 140);
        assert!(bytes[96..100].iter().all(|&b| b == 0)); // leere pod_path
        assert!(bytes[132..136].iter().all(|&b| b == 0)); // leere trace
        assert!(bytes[136..140].iter().all(|&b| b == 0)); // leere signatures
    }


    /// ⚑ **Der Differenzbeweis** (2026-10-29): zwei Zwillinge, dieselben drei
    /// Segmente, eines mit anderer Ausgabe. Der Beweis für dieses trägt und
    /// nennt die Stelle in jedem Bündel (die Folgen sind verschieden); ein
    /// gleiches Segment ist keine Differenz, ein vertauschtes Blatt trägt
    /// nicht, eine fremde Wurzel auch nicht.
    #[test]
    fn ein_differenzbeweis_verortet_die_abweichung() {
        let id = |b: u8| SegmentId::new([b; 32]);
        let zeugnis = |b: u8, aus: u8| Ausgabenzeugnis { id: id(b), eingang: [b; 32], ausgang: [aus; 32] };
        let seg = |b: u8| Segmentzeugnis { id: id(b), spurwurzel: MerkleRoot::new([b + 100; 32]) };
        let aus_a = vec![zeugnis(1, 1), zeugnis(2, 2), zeugnis(3, 3)];
        let aus_b = vec![zeugnis(1, 1), zeugnis(2, 9), zeugnis(3, 3)];
        let seg_a = vec![seg(1), seg(2), seg(3)];
        let seg_b = vec![seg(3), seg(1), seg(2)];
        let w = |aus: &[Ausgabenzeugnis], seg: &[Segmentzeugnis]| (ausgabenwurzel(aus).unwrap(), segments_root(seg).unwrap());
        let (wa, sa) = w(&aus_a, &seg_a);
        let (wb, sb) = w(&aus_b, &seg_b);
        let beweis = Differenzbeweis {
            a: differenzseite(&aus_a, &seg_a, &id(2)).unwrap(),
            b: differenzseite(&aus_b, &seg_b, &id(2)).unwrap(),
        };
        assert_eq!(differenz_pruefen(&beweis, (&wa, &sa, 3), (&wb, &sb, 3)), Ok((1, 2)));
        let gleich = Differenzbeweis {
            a: differenzseite(&aus_a, &seg_a, &id(1)).unwrap(),
            b: differenzseite(&aus_b, &seg_b, &id(1)).unwrap(),
        };
        assert_eq!(differenz_pruefen(&gleich, (&wa, &sa, 3), (&wb, &sb, 3)), Err(Differenzfehler::KeineDifferenz));
        let mut falsch = beweis.clone();
        falsch.b.ausgabe.ausgang = [7; 32];
        assert_eq!(differenz_pruefen(&falsch, (&wa, &sa, 3), (&wb, &sb, 3)), Err(Differenzfehler::TraegtNicht { seite_b: true }));
        assert_eq!(differenz_pruefen(&beweis, (&wb, &sa, 3), (&wb, &sb, 3)), Err(Differenzfehler::TraegtNicht { seite_b: false }));
        assert_eq!(differenz_pruefen(&beweis, (&wa, &sa, 4), (&wb, &sb, 3)), Err(Differenzfehler::TraegtNicht { seite_b: false }));
    }

}

/// Die Frage nach der Spur eines gezogenen Segments (Punkt 45, Stufe 2).
///
/// ⚑ **Die Kette hält nur eine Wurzel.** Wer ein Segment nachrechnen
/// will, braucht seine Eingabe und die behauptete Spur, und beide liegen
/// beim Koordinator des Pods. Ohne diesen Abruf ist die Ziehung eine
/// Lotterie ohne Ziehungsergebnis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Spuranfrage {
    /// Die Epoche, aus der das Bündel stammt.
    pub epoche: EpochId,
    /// Der Pod, dessen Bündel das Segment bezeugt.
    pub pod: PodId,
    /// Der Index des Segments **innerhalb dieses Bündels**.
    pub segment: u32,
}

/// Die Antwort darauf: das Zeugnis, sein Beweis, und was gerechnet wurde.
///
/// # ⚑ Warum der Beweis dazugehört
///
/// Ohne ihn reichte der Koordinator ein **anderes** Segment heraus,
/// nämlich eines, das er richtig gerechnet hat. Die Ziehung wäre dann
/// eine Frage, auf die der Gefragte die Antwort wählt. Der Beweis bindet
/// das Zeugnis an `segments_root` aus dem Bündel, und das Bündel ist
/// unterschrieben.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Spurantwort {
    /// Worauf geantwortet wird.
    pub anfrage: Spuranfrage,
    /// Das Zeugnis an dieser Stelle: Kennung und Spurwurzel.
    pub zeugnis: Segmentzeugnis,
    /// Merkle-Beweis des Zeugnisses gegen `segments_root`.
    pub beweis: crate::merkle::MerkleProof,
    /// Die Eingabe-Aktivierungen `a_0`.
    ///
    /// # ⚑ Fund 118: Dieses Feld kann heute niemand füllen
    ///
    /// **Die Entscheidung E10 (2026-08-30) hat das Archivieren der
    /// Aktivierungen abgeschafft.** Sie kostete über die Streitfrist
    /// zwischen 65 GiB und 1,8 TiB je Knoten, zusätzlich zur
    /// Modellgröße. Was bleibt, ist die **Spur**: 32 Byte je Layer statt
    /// 7 KiB je Eingang.
    ///
    /// Die Begründung dort lautet: Im Streitfall legt der **Ankläger**
    /// die Eingabe offen, denn „bei `j-1` sind sich beide einig, und er
    /// hat den Wert ohnehin, weil er das Segment gerade nachgerechnet
    /// hat."
    ///
    /// ⚑ **Das trägt für die Bisektion und nicht für die Stichprobe.**
    /// In der Bisektion streiten zwei, die beide gerechnet haben. Ein
    /// Checker der Stufe 2 hat **noch nichts gerechnet**; er will
    /// gerade erst anfangen und braucht dafür die Eingabe, die niemand
    /// mehr aufhebt.
    ///
    /// **Drei Wege, und keiner ist umsonst:**
    ///
    /// - Der Checker rechnet die **ganze Pipeline** vom Prompt an nach.
    ///   Selbstgenügsam, aber er zahlt `k`-mal statt einmal.
    /// - Die Eingabe wird **doch** aufgehoben, für eine kürzere Frist
    ///   als die Streitfrist. Das ist E10 mit anderer Zahl, nicht gegen
    ///   E10.
    /// - Stufe 2 prüft nur den **ersten** Shard eines Segments, dessen
    ///   Eingabe aus dem Prompt folgt. Billig, deckt aber `1/k` ab.
    ///
    /// **Das ist eine Entscheidung des Projektinhabers und keine
    /// Verdrahtung**, und sie ist als eigener Punkt offen; bis dahin
    /// bleibt das Feld im Typ, weil die Prüfung darauf aufbaut und das
    /// Bindungsgerüst darum herum vollständig und geprüft ist.
    pub eingabe: Vec<u8>,
    /// Die behaupteten Commitment-Hashes der Spur.
    pub spur: Vec<crate::hash::Hash>,
}

/// Warum eine gelieferte Spur nicht trägt: die Lieferung, bevor nachgerechnet
/// wird (Stufe 2, 2026-10-28). Dieselben vier Prüfungen wie am Anfang von
/// `myl_verifier::pruefe_spurantwort`, das sie von hier nimmt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lieferfehler {
    /// Der Beweis gehört zu einem anderen Index als dem gefragten.
    FalschesSegment { gefragt: u32, geliefert: u64 },
    /// Die Blattzahl des Beweises ist nicht die Segmentzahl des Bündels.
    BlattzahlPasstNicht { im_buendel: u32, im_beweis: u64 },
    /// Der Beweis trägt nicht gegen `segments_root`.
    BeweisTraegtNicht,
    /// Die gelieferte Spur ergibt nicht die bezeugte Spurwurzel.
    SpurPasstNichtZumZeugnis,
}

/// **Trägt die Lieferung?** Der Beweis gehört zum gefragten Index, seine
/// Blattzahl ist die Segmentzahl des Bündels, er trägt gegen die
/// unterschriebene `segments_root`, und die gelieferte Spur ergibt die
/// bezeugte Spurwurzel. Erst danach lohnt das Nachrechnen.
pub fn lieferung_pruefen(antwort: &Spurantwort, segments_root: &MerkleRoot, segmente_im_buendel: u32) -> Result<(), Lieferfehler> {
    if antwort.beweis.leaf_index != u64::from(antwort.anfrage.segment) {
        return Err(Lieferfehler::FalschesSegment { gefragt: antwort.anfrage.segment, geliefert: antwort.beweis.leaf_index });
    }
    if antwort.beweis.leaf_count != u64::from(segmente_im_buendel) {
        return Err(Lieferfehler::BlattzahlPasstNicht { im_buendel: segmente_im_buendel, im_beweis: antwort.beweis.leaf_count });
    }
    let mut blatt = Vec::with_capacity(64);
    blatt.extend_from_slice(antwort.zeugnis.id.as_ref());
    blatt.extend_from_slice(antwort.zeugnis.spurwurzel.as_ref());
    if !antwort.beweis.verify(&Hash(*segments_root.as_bytes()), &blatt, antwort.beweis.leaf_index) {
        return Err(Lieferfehler::BeweisTraegtNicht);
    }
    let roh: Vec<[u8; 32]> = antwort.spur.iter().map(|h| *h.as_bytes()).collect();
    match spurwurzel(&roh) {
        Ok(w) if w == antwort.zeugnis.spurwurzel => Ok(()),
        _ => Err(Lieferfehler::SpurPasstNichtZumZeugnis),
    }
}

/// ⚑ **Was ein Mitglied eines Pods für die Stichprobe aufhebt** (Stufe 2,
/// 2026-10-28): je Segment des Bündels Zeugnis und Spur (`[Eingang] ++
/// Spur`), in Bündelfolge, dazu die Wurzel, über die das Bündel
/// unterschrieben ist. Der Ortsdienst legt sie beim Einreichen bei jedem
/// Mitglied ab; der Shard selbst endet mit der Epoche, die Ziehung kommt
/// danach. Aktivierungen hebt niemand auf (Entscheidung E10); die Eingabe
/// eines Segments ist aber nur sein Token, vier Bytes, und die steht in
/// [`Self::eingaben`].
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Spurablage {
    /// Die Epoche des Bündels.
    pub epoche: EpochId,
    /// Der Pod des Bündels.
    pub pod: PodId,
    /// Die Wurzel des Bündels.
    pub segments_root: MerkleRoot,
    /// Je Segment Zeugnis und Spur, in Bündelfolge.
    pub eintraege: Vec<(Segmentzeugnis, Vec<Hash>)>,
    /// ⚑ **Je Segment die Nutzlast seines Eingangs** (Stufe 2, Nachrechnen,
    /// 2026-10-29), in derselben Folge wie [`Self::eintraege`]: das Token
    /// als zwei `i16`, deren Hash der erste Eintrag der Spur ist. Damit
    /// rechnet ein Checker die ganze Pipeline ab den Tokens nach
    /// (Entscheidung vom 2026-10-27), ohne dass jemand Aktivierungen
    /// aufhebt. Leer bei einer Ablage ohne Eingaben; eine andere Länge als
    /// die der Einträge macht die Ablage für den Verlauf unbrauchbar, sie
    /// wird nicht still gekürzt.
    pub eingaben: Vec<Vec<i16>>,
}

/// Sitzung und Position aus einer Segmentkennung: die ersten acht Bytes
/// und die nächsten acht, je little-endian (so bildet der Pod sie).
pub fn sitzung_und_position(id: &SegmentId) -> (u64, u64) {
    let b = id.as_ref();
    let mut s = [0u8; 8];
    let mut p = [0u8; 8];
    s.copy_from_slice(&b[..8]);
    p.copy_from_slice(&b[8..16]);
    (u64::from_le_bytes(s), u64::from_le_bytes(p))
}

/// Das Token aus der Nutzlast eines Eingangs: zwei `i16`, unten und oben,
/// wie der Pod sie packt. `None` bei jeder anderen Länge.
pub fn token_aus_eingabe(eingabe: &[i16]) -> Option<u32> {
    match eingabe {
        [lo, hi] => Some(u32::from(*lo as u16) | (u32::from(*hi as u16) << 16)),
        _ => None,
    }
}

/// Die Nutzlast als Bytes, wie sie in [`Spurantwort::eingabe`] steht:
/// je `i16` little-endian.
fn eingabe_als_bytes(eingabe: &[i16]) -> Vec<u8> {
    eingabe.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Zurück aus den Bytes; `None` bei ungerader Länge.
pub fn eingabe_aus_bytes(roh: &[u8]) -> Option<Vec<i16>> {
    if !roh.len().is_multiple_of(2) {
        return None;
    }
    Some(roh.as_chunks::<2>().0.iter().map(|c| i16::from_le_bytes(*c)).collect())
}

/// Warum ein gelieferter Verlauf nicht zum Nachrechnen taugt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verlauffehler {
    /// Leer.
    Leer,
    /// Eine Lieferung trägt nicht (siehe [`Lieferfehler`]).
    Lieferung { stelle: usize, fehler: Lieferfehler },
    /// Ein Segment gehört zu einer anderen Sitzung als das erste.
    FremdeSitzung { stelle: usize },
    /// Die Positionen laufen nicht lückenlos von null an.
    Luecke { stelle: usize, position: u64 },
    /// Die Eingabe fehlt, ist kein Token, oder ihr Hash ist nicht der erste
    /// Eintrag der Spur.
    Eingabe { stelle: usize },
}

/// ⚑ **Taugt der Verlauf zum Nachrechnen?** (Stufe 2, 2026-10-29) Jede
/// Lieferung trägt gegen die Wurzel des Bündels, alle gehören zu einer
/// Sitzung, die Positionen laufen lückenlos von null bis zum gezogenen
/// Segment, und jede Eingabe ist ein Token, dessen Hash der erste Eintrag
/// seiner Spur ist. Dann die Tokens in Folge.
///
/// ⚑ **Der Eingang ist gebunden, nicht nur geliefert**: Der Hash der
/// Nutzlast steht in der Spur, die Spur in der Spurwurzel, die Spurwurzel
/// unter der unterschriebenen `segments_root`. Ein Pod kann dem Checker
/// also keine anderen Tokens unterschieben als die, über die er gerechnet
/// zu haben behauptet.
pub fn verlauf_pruefen(
    antworten: &[Spurantwort],
    segments_root: &MerkleRoot,
    segmente_im_buendel: u32,
) -> Result<Vec<u32>, Verlauffehler> {
    let erste = antworten.first().ok_or(Verlauffehler::Leer)?;
    let (sitzung, _) = sitzung_und_position(&erste.zeugnis.id);
    let mut tokens = Vec::with_capacity(antworten.len());
    for (stelle, a) in antworten.iter().enumerate() {
        lieferung_pruefen(a, segments_root, segmente_im_buendel)
            .map_err(|fehler| Verlauffehler::Lieferung { stelle, fehler })?;
        let (s, p) = sitzung_und_position(&a.zeugnis.id);
        if s != sitzung {
            return Err(Verlauffehler::FremdeSitzung { stelle });
        }
        if p != stelle as u64 {
            return Err(Verlauffehler::Luecke { stelle, position: p });
        }
        let eingabe = eingabe_aus_bytes(&a.eingabe).ok_or(Verlauffehler::Eingabe { stelle })?;
        let token = token_aus_eingabe(&eingabe).ok_or(Verlauffehler::Eingabe { stelle })?;
        let gebunden = a.spur.first().is_some_and(|h| *h.as_bytes() == crate::uebergang::activation_hash(&eingabe));
        if !gebunden {
            return Err(Verlauffehler::Eingabe { stelle });
        }
        tokens.push(token);
    }
    Ok(tokens)
}

impl Spurablage {
    /// **Die Antwort auf eine Spuranfrage**: Zeugnis, Merkle-Beweis gegen
    /// `segments_root` und die Spur. `None`, wenn die Anfrage nicht zu dieser
    /// Ablage passt, der Index fehlt oder die Ablage ihre eigene Wurzel nicht
    /// ergibt.
    pub fn antwort(&self, anfrage: &Spuranfrage) -> Option<Spurantwort> {
        self.antworten(anfrage, &[anfrage.segment as usize])?.pop()
    }

    /// ⚑ **Der Verlauf eines gezogenen Segments** (Stufe 2, Nachrechnen,
    /// 2026-10-29): die Antworten auf alle Segmente derselben Sitzung bis
    /// zu seiner Position, nach Position geordnet, jede mit Beweis und
    /// Eingabe. Wer Position `t` nachrechnet, braucht die Tokens `0..=t`,
    /// denn die Aufmerksamkeit liest alle Positionen davor (Fund 373), und
    /// er braucht deren Spuren, damit die erste Abweichung auch vor `t`
    /// liegen darf. `None` wie bei [`Self::antwort`], oder ohne Eingaben.
    pub fn verlauf(&self, anfrage: &Spuranfrage) -> Option<Vec<Spurantwort>> {
        if self.eingaben.len() != self.eintraege.len() {
            return None;
        }
        let (ziel, _) = self.eintraege.get(anfrage.segment as usize)?;
        let (sitzung, bis) = sitzung_und_position(&ziel.id);
        let mut indizes: Vec<(u64, usize)> = self
            .eintraege
            .iter()
            .enumerate()
            .filter_map(|(i, (z, _))| {
                let (s, p) = sitzung_und_position(&z.id);
                (s == sitzung && p <= bis).then_some((p, i))
            })
            .collect();
        indizes.sort_unstable();
        let indizes: Vec<usize> = indizes.into_iter().map(|(_, i)| i).collect();
        self.antworten(anfrage, &indizes)
    }

    /// Die Antworten zu `indizes`, mit einem Baum für alle.
    fn antworten(&self, anfrage: &Spuranfrage, indizes: &[usize]) -> Option<Vec<Spurantwort>> {
        if anfrage.epoche != self.epoche || anfrage.pod != self.pod {
            return None;
        }
        let zeugnisse: Vec<Segmentzeugnis> = self.eintraege.iter().map(|(z, _)| *z).collect();
        if segments_root(&zeugnisse).ok()? != self.segments_root {
            return None;
        }
        let blaetter: Vec<Vec<u8>> = zeugnisse
            .iter()
            .map(|z| {
                let mut b = Vec::with_capacity(64);
                b.extend_from_slice(z.id.as_ref());
                b.extend_from_slice(z.spurwurzel.as_ref());
                b
            })
            .collect();
        let refs: Vec<&[u8]> = blaetter.iter().map(|b| b.as_slice()).collect();
        let baum = MerkleTree::new(&refs).ok()?;
        let mut aus = Vec::with_capacity(indizes.len());
        for &i in indizes {
            let beweis = baum.proof(i).ok()?;
            let (zeugnis, spur) = self.eintraege.get(i)?.clone();
            let eingabe = self.eingaben.get(i).map(|e| eingabe_als_bytes(e)).unwrap_or_default();
            aus.push(Spurantwort {
                anfrage: Spuranfrage { segment: i as u32, ..*anfrage },
                zeugnis,
                beweis,
                eingabe,
                spur,
            });
        }
        Some(aus)
    }
}
