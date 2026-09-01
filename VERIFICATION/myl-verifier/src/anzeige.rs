//! Von der Abweichung zur Anzeige: wer beschuldigt wird und warum.
//!
//! # ⚑ Die Naht, die Stufe 2 folgenlos ließ
//!
//! Der Bisektionspfad steht seit Langem, das Topic
//! `/myelith/challenges/1` steht, `Challenge::validate_structure` wird in
//! der Netzschicht geprüft, **und niemand erzeugte eine `Challenge`.**
//! Damit fand Stufe 2 etwas und tat nichts damit.
//!
//! # ⚑ Die eigentliche Frage ist, wen es trifft
//!
//! Die Spur eines Segments läuft über **alle** Shards eines Pods: Ein
//! Hash je Layer, und die Layer sind auf die Shards aufgeteilt. Eine
//! Abweichung an Stelle `j` gehört deshalb dem Shard, dessen Bereich `j`
//! enthält, **und keinem anderen.**
//!
//! **Wer hier danebengreift, beschuldigt einen Unschuldigen**, und zwar
//! mit einem Beweisstück, das gegen ihn zählt. Deshalb gibt
//! [`beschuldigter`] `None` zurück, wenn kein Bereich passt, statt auf
//! den ersten zu zeigen: **Keine Anzeige ist besser als die falsche.**

use myl_types::bls::{BlsError, BlsSecretKey};
use myl_types::challenge::{Challenge, ChallengeStructureError};
use myl_types::hash::Hash;
use myl_types::ids::{MinerId, SegmentId};

/// Ein Shard mit dem Layerbereich, für den er einsteht.
///
/// `von` einschließlich, `bis` ausschließlich, wie überall im Projekt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zustaendigkeit {
    /// Wer rechnet.
    pub miner: MinerId,
    /// Erste Layer des Bereichs.
    pub von: u64,
    /// Erste Layer **nach** dem Bereich.
    pub bis: u64,
}

/// Warum eine Anzeige nicht zustande kommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anzeigefehler {
    /// Kein Shard steht für diese Stelle ein.
    ///
    /// ⚑ **Das ist ein Befund über die Zuständigkeiten, nicht über den
    /// Beschuldigten.** Entweder ist die Spur länger als die Summe der
    /// Bereiche, oder die Bereiche haben ein Loch. Beides ist ein Fehler
    /// beim Aufrufer, und beides darf **nicht** dazu führen, dass
    /// irgendwer angezeigt wird.
    NiemandZustaendig { stelle: usize },
    /// Die Anzeige hielt der eigenen Strukturprüfung nicht stand.
    Struktur(ChallengeStructureError),
    /// Die Unterschrift ließ sich nicht bilden.
    ///
    /// ⚑ **Eine unterschriebene Anzeige ist keine halbe Sache.** Ohne
    /// Bindung an den Herausforderer wäre sie „ein Hebel zum
    /// Schikanieren, den jeder ohne Einsatz ziehen kann"; der Angeklagte
    /// muss antworten, und das heißt seit der Umstellung auf
    /// Nachrechnen: eine ganze Folge neu rechnen.
    Unterschrift,
}

/// Wer für die Layer an Stelle `stelle` einsteht.
///
/// ⚑ **`None` statt eines Rückfalls.** Ein Rückfall auf den ersten
/// Shard sähe im Code harmlos aus und beschuldigte im Betrieb den
/// Falschen.
pub fn beschuldigter(zustaendig: &[Zustaendigkeit], stelle: usize) -> Option<MinerId> {
    let j = u64::try_from(stelle).ok()?;
    zustaendig
        .iter()
        .find(|z| z.von <= j && j < z.bis)
        .map(|z| z.miner)
}

/// Erhebt die Anzeige aus einem Befund.
///
/// `behauptet` ist der Hash aus dem Bündel, `nachgerechnet` der eigene.
///
/// # ⚑ Der Ankläger steht in der Anzeige
///
/// `Challenge` trägt zwei Miner und zwei Hashes, denn ohne die
/// Gegenseite wäre die Anzeige nicht nachprüfbar (Kap. 6.6). In der
/// Redundanzstufe sind das zwei Pods; **hier ist es der Angeklagte und
/// der Checker.** Die Feldnamen stammen aus der ersten Verwendung, die
/// Struktur des Streits ist dieselbe: zwei Parteien, zwei Hashes, eine
/// erste Abweichung, und eine Bisektion, die ihn entscheidet.
///
/// # ⚑ Sich selbst kann niemand anzeigen
///
/// `validate_structure` verlangt verschiedene Miner und verschiedene
/// Hashes. Beides ist hier keine Formalie: Gleiche Hashes sind **kein
/// Streit**, und wer sich selbst anzeigt, erzeugt einen, den keine
/// Bisektion entscheiden kann.
#[allow(clippy::too_many_arguments)]
pub fn anzeige_erheben(
    segment_id: SegmentId,
    stelle: usize,
    zustaendig: &[Zustaendigkeit],
    ankläger: MinerId,
    sk: &BlsSecretKey,
    behauptet: Hash,
    nachgerechnet: Hash,
    zeit_ms: u64,
) -> Result<Challenge, Anzeigefehler> {
    let angeklagter = beschuldigter(zustaendig, stelle)
        .ok_or(Anzeigefehler::NiemandZustaendig { stelle })?;
    let mut c = Challenge {
        segment_id,
        first_divergence: stelle,
        primary_miner: angeklagter,
        redundant_miner: ankläger,
        primary_hash: behauptet,
        redundant_hash: nachgerechnet,
        timestamp_ms: zeit_ms,
        signature: myl_types::bls::BlsSignature([0; 96]),
    };
    // ⚑ **Erst die Struktur, dann die Unterschrift.** Eine unterschriebene
    // Anzeige, die sich selbst widerspricht, wäre ein gültiger Beleg für
    // Unsinn; die Reihenfolge macht das unmöglich.
    c.validate_structure().map_err(Anzeigefehler::Struktur)?;
    c.signiere(sk).map_err(|_: BlsError| Anzeigefehler::Unterschrift)?;
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn miner(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }

    fn schluessel(b: u8) -> BlsSecretKey {
        BlsSecretKey::key_gen(&[b; 32]).expect("key_gen")
    }

    /// Die Kennung, die zu einem Schluessel gehoert.
    fn kennung(sk: &BlsSecretKey) -> MinerId {
        MinerId::aus_schluessel(&sk.public_key().expect("public_key"))
    }

    fn zustaendig() -> Vec<Zustaendigkeit> {
        vec![
            Zustaendigkeit { miner: miner(1), von: 0, bis: 6 },
            Zustaendigkeit { miner: miner(2), von: 6, bis: 12 },
            Zustaendigkeit { miner: miner(3), von: 12, bis: 18 },
        ]
    }

    /// ⚑ **Die Grenze gehoert dem naechsten Shard**, nicht dem vorigen.
    ///
    /// `bis` ist ausschliesslich, wie ueberall im Projekt. Ein Fehler um
    /// eins beschuldigt hier systematisch den Nachbarn.
    #[test]
    fn die_grenze_gehoert_dem_naechsten() {
        let z = zustaendig();
        assert_eq!(beschuldigter(&z, 5), Some(miner(1)));
        assert_eq!(beschuldigter(&z, 6), Some(miner(2)));
        assert_eq!(beschuldigter(&z, 11), Some(miner(2)));
        assert_eq!(beschuldigter(&z, 12), Some(miner(3)));
    }

    /// ⚑ **Ausserhalb aller Bereiche wird niemand beschuldigt.**
    #[test]
    fn ausserhalb_trifft_es_niemanden() {
        let z = zustaendig();
        assert_eq!(beschuldigter(&z, 18), None);
        assert_eq!(beschuldigter(&z, 999), None);
        assert_eq!(beschuldigter(&[], 0), None);
    }

    /// ⚑ **Ein Loch in den Bereichen fuehrt zu keiner Anzeige.**
    ///
    /// Es waere bequem, auf den naechstgelegenen zu zeigen. Bequem und
    /// falsch: Der Naechstgelegene hat die Stelle nicht gerechnet.
    #[test]
    fn ein_loch_beschuldigt_niemanden() {
        let z = vec![
            Zustaendigkeit { miner: miner(1), von: 0, bis: 6 },
            Zustaendigkeit { miner: miner(2), von: 8, bis: 12 },
        ];
        assert_eq!(beschuldigter(&z, 7), None);
        assert_eq!(
            anzeige_erheben(
                SegmentId::new([1; 32]),
                7,
                &z,
                miner(9),

                &schluessel(9),
                Hash::sha256(b"a"),
                Hash::sha256(b"b"),
                1,
            ),
            Err(Anzeigefehler::NiemandZustaendig { stelle: 7 })
        );
    }

    #[test]
    fn eine_gewoehnliche_anzeige_steht() {
        let c = anzeige_erheben(
            SegmentId::new([1; 32]),
            7,
            &zustaendig(),
            miner(9),

            &schluessel(9),
            Hash::sha256(b"behauptet"),
            Hash::sha256(b"nachgerechnet"),
            1_700_000_000_000,
        )
        .expect("Anzeige");
        assert_eq!(c.first_divergence, 7);
        assert_eq!(c.primary_miner, miner(2), "Stelle 7 liegt im zweiten Shard");
        assert_eq!(c.redundant_miner, miner(9), "der Ankläger steht daneben");
    }

    /// ⚑ **Gleiche Hashes sind kein Streit.**
    #[test]
    fn ohne_abweichung_gibt_es_nichts_anzuzeigen() {
        let h = Hash::sha256(b"gleich");
        assert!(matches!(
            anzeige_erheben(
                SegmentId::new([1; 32]),
                7,
                &zustaendig(),
                miner(9),
                &schluessel(9),
                h,
                h,
                1
            ),
            Err(Anzeigefehler::Struktur(_))
        ));
    }

    /// ⚑ **Die Unterschrift bindet den Herausforderer, und nur ihn.**
    ///
    /// Eine gueltige Unterschrift unter einer Anzeige, die einen
    /// **anderen** als Herausforderer nennt, belegt nur, dass irgendwer
    /// unterschrieben hat. Der Name muss aus dem Schluessel folgen.
    #[test]
    fn die_unterschrift_bindet_den_herausforderer() {
        let sk = schluessel(9);
        let c = anzeige_erheben(
            SegmentId::new([1; 32]),
            7,
            &zustaendig(),
            kennung(&sk),
            &sk,
            Hash::sha256(b"behauptet"),
            Hash::sha256(b"nachgerechnet"),
            1,
        )
        .expect("Anzeige");
        assert!(c.ist_vom_herausforderer(&sk.public_key().expect("public_key")));
        // ⚑ Und ein fremder Schluessel traegt sie nicht.
        assert!(!c.ist_vom_herausforderer(&schluessel(8).public_key().expect("public_key")));
    }

    /// ⚑ **Wer sich selbst anzeigt, erzeugt einen Streit, den keine
    /// Bisektion entscheiden kann.**
    #[test]
    fn niemand_zeigt_sich_selbst_an() {
        assert!(matches!(
            anzeige_erheben(
                SegmentId::new([1; 32]),
                7,
                &zustaendig(),
                miner(2), // derselbe wie der Zustaendige fuer Stelle 7
                &schluessel(2),
                Hash::sha256(b"a"),
                Hash::sha256(b"b"),
                1,
            ),
            Err(Anzeigefehler::Struktur(_))
        ));
    }
}
