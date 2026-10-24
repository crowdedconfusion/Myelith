//! Blocknachforderung: wie ein Neuling aufholt.
//!
//! # Das Problem, das dieses Modul löst
//!
//! Die Kette hängt am Vorgänger-Hash. Ein Knoten, der Block 1 nie
//! gesehen hat, weist Block 2 zurück, weil dessen Vorgänger ihm
//! unbekannt ist, und danach jeden weiteren aus demselben Grund. **Ein
//! einziger verpasster Block hängt ihn für den ganzen Lauf ab.**
//!
//! Der erste Dreiknoten-Probelauf lief genau so ins Leere: Der Erzeuger
//! war vier Sekunden früher da, baute acht Blöcke, und die beiden
//! anderen wiesen alle acht mit „passt nicht an" zurück.
//!
//! # ⚑ Woran ein Knoten seine Lücke abzählt (Stand 2026-08-27)
//!
//! **Am Höhenfeld des Blockkopfs**, und das gibt es seit dem
//! 2026-08-27. Ein Knoten auf Höhe 0, der Höhe 8 empfängt, weiß damit
//! genau, dass ihm 1 bis 7 fehlen.
//!
//! Zwei frühere Fassungen dieses Absatzes standen daneben, und beide
//! Male aus demselben Grund: Der Protokolltyp trug **kein** Höhenfeld,
//! die Probekette schrieb ihre Höhe in `epoch`, und dieselbe Zahl
//! bedeutete je nach Leser eine Höhe oder eine Epoche. Die erste
//! Fassung schloss daraus, die Lücke sei nicht benennbar; die zweite
//! benannte sie richtig und über ein Feld, dessen Bedeutung eine andere
//! war. **Aufgelöst ist die Doppelbelegung jetzt:** `height` zählt
//! Blöcke, `epoch` folgt aus der Höhe, und die Nachforderung hängt an
//! `height`.
//!
//! # Die Regel, die dieses Modul nicht bricht
//!
//! **Nachgelieferte Blöcke gehen denselben Weg wie verbreitete.** Sie
//! landen in [`crate::kette::Kette::uebernimm`], mit derselben
//! Anschlussprüfung und derselben Nachrechnung der Zustandswurzel.
//!
//! Das ist keine Formsache. Wäre die Nachlieferung ein zweiter,
//! schwächerer Weg in die Kette, wäre sie das Loch: Wer einen Knoten
//! dazu bringt, einen Block **nachzufordern**, bekäme ihn hineingelegt,
//! ohne dass er nachrechnet. Der Nachschub ist ein Transportweg, kein
//! Vertrauensweg.

use borsh::{BorshDeserialize, BorshSerialize};
use myl_consensus::block::Block;

/// Höchstzahl Blöcke je Nachlieferung.
///
/// **Herleitung:** Ein Neuling gegenüber einer langen Kette darf nicht
/// alles auf einmal anfordern; er würde den Gegenüber zum Senden
/// beliebiger Datenmengen bewegen, und das wäre ein Verstärker. 64
/// Blöcke sind bei der Größe eines Probeblocks (unter 200 Bytes)
/// bequem unter der Anfragegrenze und holen eine übliche Lücke in einem
/// Zug.
///
/// Größere Rückstände holt der Knoten in mehreren Runden auf: Jede
/// gelungene Nachlieferung hebt seine Höhe, und die nächste Anfrage
/// beginnt dort.
///
/// 📌 **Seit dem 2026-10-22: 512 statt 64, und dazu ein Deckel in Bytes**
/// ([`in_die_antwort`]). Die Herleitung oben sprach von Bytes, die
/// Grenze zählte Blöcke: Ein echter Block darf so groß sein wie die ganze
/// Antwort, 64 davon passten nicht hinein, und das Nachholen bliebe an
/// der ersten vollen Strecke stehen. Mit dem Deckel in Bytes ist die Zahl
/// keine Schutzgrenze mehr, nur noch eine Obergrenze für leichte Blöcke;
/// im Netzlauf holte ein Neuling 3 860 Blöcke in Schritten von 64.
pub const MAX_BLOECKE_JE_LIEFERUNG: u64 = 512;

/// Was eine Lieferung an Bytes um ihre Blöcke herum braucht (Marke der
/// Antwort, Länge der Liste), großzügig gerundet.
const LIEFERUNG_RAHMEN_BYTES: usize = 64;

/// **Wie viele der Längen vorn in eine Antwort passen**: mindestens eine,
/// denn ein Block, der allein nicht passt, passt nie, und ohne ihn käme
/// der Nachfragende nicht weiter.
pub fn passende_anzahl(laengen: &[usize], grenze: usize) -> usize {
    let mut summe = LIEFERUNG_RAHMEN_BYTES;
    let mut n = 0;
    for l in laengen {
        summe = summe.saturating_add(*l);
        if summe > grenze && n > 0 {
            break;
        }
        n += 1;
    }
    n
}

/// **Kürzt eine Lieferung auf das, was in eine Antwort passt**
/// ([`myl_types::protocol::MAX_ANFRAGE_BYTES`]). Der Nachfragende setzt an
/// der Höhe danach fort.
pub fn in_die_antwort(mut bloecke: Vec<Block>) -> Vec<Block> {
    let laengen: Vec<usize> = bloecke.iter().map(|b| borsh::to_vec(b).map(|v| v.len()).unwrap_or(usize::MAX)).collect();
    bloecke.truncate(passende_anzahl(&laengen, myl_types::protocol::MAX_ANFRAGE_BYTES));
    bloecke
}

/// So viel Platz hält eine Archivlieferung für ihren Beweis frei: höchstens
/// 64 Geschwister und 64 Gipfel zu 32 Byte, dazu Längen und Zahlen,
/// aufgerundet.
pub const BEWEIS_HOECHSTENS_BYTES: usize = 8 * 1024;

/// Wie [`in_die_antwort`], mit `frei` Bytes Platz für etwas, das neben den
/// Blöcken mitgeht.
pub fn in_die_antwort_mit(mut bloecke: Vec<Block>, frei: usize) -> Vec<Block> {
    let laengen: Vec<usize> = bloecke.iter().map(|b| borsh::to_vec(b).map(|v| v.len()).unwrap_or(usize::MAX)).collect();
    bloecke.truncate(passende_anzahl(&laengen, myl_types::protocol::MAX_ANFRAGE_BYTES.saturating_sub(frei)));
    bloecke
}

/// Was ein Knoten nachfordern kann.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Nachforderung {
    /// Blöcke der Höhen `ab` bis einschließlich `bis`.
    Bloecke { ab: u64, bis: u64 },
    /// Ein Inferenzauftrag an den Pod dieses Knotens (GATEWAY Stufe 4).
    ///
    /// ⚑ **Dieselbe Schiene wie die Blocknachforderung, und das ist
    /// eine Entscheidung.** Ein eigenes Protokoll dafür hätte einen
    /// zweiten Codec, eine zweite Größengrenze und eine zweite Stelle,
    /// an der eine Zerlegung auf fremde Eingaben trifft. Die Schiene
    /// hier trägt Längenpräfix und Deckel bereits.
    ///
    /// **Additiv angehängt, nie eingefügt:** Die Reihenfolge der
    /// Varianten ist Protokollvertrag; ein Knoten alter Fassung liest
    /// die neue Marke nicht und antwortet mit `Nichts`, statt etwas
    /// Falsches zu verstehen.
    Inferenz(myl_types::inferenzauftrag::Inferenzauftrag),
    /// ⚑ **Eine Zeitprobe an den Shard dieses Knotens** (Stufe 4, Frist,
    /// 2026-10-22): Wer fragt, nimmt die Zeit. Angehängt wie alle folgenden.
    Zeitprobe {
        /// Wie viele Positionen.
        positionen: u32,
    },
    /// Nur der Weg, ohne Rechnung: Gegenstück der Zeitprobe.
    Ping,
    /// ⚑ **Ein Stück einer Datei des Netzmodells** (Stufe 4, P11).
    Datei {
        /// Der Name im Artefaktordner, wie das Manifest ihn nennt.
        name: String,
        /// Ab welchem Byte.
        ab: u64,
        /// Wie viele, höchstens `dateidienst::STUECK_BYTES`.
        laenge: u64,
    },
    /// ⚑ **Ein Fristzeugnis an ein Mitglied desselben Pods** (6.8).
    Fristzeugnis {
        /// Die Epoche.
        epoche: myl_types::ids::EpochId,
        /// Der Pod.
        pod: u32,
        /// Wer seine Frist überschritten haben soll.
        gemeldeter: myl_types::ids::MinerId,
        /// Die Messung des Zeugen.
        zeugnis: myl_types::fristmeldung::Fristzeugnis,
    },
    /// ⚑ **Ein Stück eines Zustandsabzugs** (2026-10-22): der Abzug bei
    /// `hoehe`, ab Byte `ab` seines Zustands. Angehängt.
    Abzug {
        /// Die Epochengrenze.
        hoehe: u64,
        /// Ab welchem Byte des Zustands.
        ab: u64,
    },
    /// ⚑ **Blöcke der Geschichte mit Beweis** (Archivknoten, Stufe A4): die
    /// Höhen `ab` bis `bis`, und für die letzte gelieferte ein Beweis gegen
    /// die Historie mit `anzahl` Blöcken. Angehängt.
    Archiv {
        /// Ab welcher Höhe.
        ab: u64,
        /// Bis zu welcher Höhe, einschliesslich.
        bis: u64,
        /// Die Länge der Historie, gegen die bewiesen wird: die Höhe der
        /// Basis des Fragenden.
        anzahl: u64,
    },
    /// ⚑ **Ein Teil für einen Splitter** (Stufe A7): Teil `teil` der Gruppe
    /// an Stelle `stelle` des Abschnitts der Grenze `grenze`. Angehängt.
    Teil {
        /// Die Grenze des Abschnitts.
        grenze: u64,
        /// Die Stelle der Gruppe im Abschnitt.
        stelle: u64,
        /// Welcher der 64 Teile.
        teil: u64,
    },
}

/// Was auf eine Nachforderung zurückkommt.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Nachlieferung {
    /// Die angeforderten Blöcke, aufsteigend nach Höhe.
    Bloecke(Vec<Block>),
    /// Die Antwort auf einen Inferenzauftrag.
    Inferenz(myl_types::inferenzauftrag::Inferenzantwort),
    /// Nichts vorhanden. **Eine Antwort, kein Schweigen:** Der
    /// Fragende soll den Unterschied zwischen „habe ich nicht" und
    /// „habe nicht geantwortet" kennen, sonst wartet er auf eine
    /// Zeitüberschreitung, die nichts bedeutet.
    Nichts,
    /// Die Zeitprobe ist gerechnet: der Abdruck des letzten Ergebnisses.
    Zeitprobe([u8; 32]),
    /// Die Antwort auf einen Ping.
    Pong,
    /// Ein Stück einer Datei und die Größe der ganzen.
    Datei {
        /// Der Name.
        name: String,
        /// Ab welchem Byte.
        ab: u64,
        /// Die Bytes.
        daten: Vec<u8>,
        /// Wie groß die ganze Datei ist.
        groesse: u64,
    },
    /// Ein Fristzeugnis ist angekommen.
    Vermerkt,
    /// Ein Stück eines Zustandsabzugs: der Block bei H mit jedem Stück (der
    /// Fragende hält ihn beim ersten gegen seinen Anker), die Stelle, die
    /// Bytes und die Größe des ganzen Zustands.
    Abzug {
        /// Der Block bei H.
        block: Block,
        /// Ab welchem Byte.
        ab: u64,
        /// Die Bytes, höchstens `abzug::STUECK_BYTES`.
        daten: Vec<u8>,
        /// Wie groß der ganze Zustand ist.
        groesse: u64,
    },
    /// Blöcke der Geschichte, aufsteigend ab der erfragten Höhe, und der
    /// Beweis für den letzten.
    Archiv {
        /// Die Blöcke.
        bloecke: Vec<Block>,
        /// Der Beweis.
        beweis: myl_types::historie::Beweis,
    },
    /// Ein Teil und seine Pfade zur Teilwurzel.
    Teil {
        /// Die Grenze des Abschnitts.
        grenze: u64,
        /// Der Teil.
        stueck: Vec<u8>,
        /// Der Beweis.
        beweis: myl_types::archivteile::Teilbeweis,
    },
}

/// ⚑ **Der nächste Bereich beim Nachholen von mehreren Quellen** (2026-10-22).
///
/// Ab dem kleinsten Block über `eigene`, den weder ein laufender Bereich
/// (`laufend`, je `(ab, bis)`) noch der Vorausspeicher (`im_voraus`) deckt;
/// höchstens [`MAX_BLOECKE_JE_LIEFERUNG`] lang, nicht über `ziel` und nicht in
/// den nächsten laufenden Bereich hinein. `None`, wenn bis `ziel` alles gedeckt
/// ist.
///
/// 📌 Die erste Fassung begann hinter dem höchsten laufenden Bereich. Ging
/// die vorderste Lieferung verloren, schloss niemand mehr die Lücke davor,
/// und die Kette rückte nie vor, obwohl alles Spätere ankam.
pub fn naechster_bereich(eigene: u64, laufend: &[(u64, u64)], im_voraus: impl Fn(u64) -> bool, ziel: u64) -> Option<(u64, u64)> {
    let mut ab = eigene + 1;
    loop {
        if let Some(&(_, bis)) = laufend.iter().find(|(a, b)| *a <= ab && ab <= *b) {
            ab = bis + 1;
        } else if ab <= ziel && im_voraus(ab) {
            ab += 1;
        } else {
            break;
        }
    }
    if ab > ziel {
        return None;
    }
    let bis = ziel.min(ab + MAX_BLOECKE_JE_LIEFERUNG - 1);
    let bis = laufend.iter().filter(|(a, _)| *a > ab).map(|(a, _)| a - 1).min().unwrap_or(bis).min(bis);
    Some((ab, bis))
}

impl Nachforderung {
    /// Der Bereich, den ein Knoten auf Höhe `eigene` anfordern sollte,
    /// wenn er von Höhe `fremde` gehört hat.
    ///
    /// `None`, wenn nichts fehlt. Der Bereich ist auf
    /// [`MAX_BLOECKE_JE_LIEFERUNG`] gedeckelt.
    pub fn fuer_rueckstand(eigene: u64, fremde: u64) -> Option<Nachforderung> {
        if fremde <= eigene {
            return None;
        }
        let ab = eigene + 1;
        let bis = fremde.min(ab + MAX_BLOECKE_JE_LIEFERUNG - 1);
        Some(Nachforderung::Bloecke { ab, bis })
    }

    /// Als Bytes für den Anfragekanal.
    pub fn als_bytes(&self) -> Option<Vec<u8>> {
        borsh::to_vec(self).ok()
    }

    /// Aus Bytes zurück. `None` bei allem, was nicht passt, einschließlich
    /// Anhängseln.
    pub fn aus_bytes(daten: &[u8]) -> Option<Nachforderung> {
        let mut rest = daten;
        match Nachforderung::deserialize(&mut rest) {
            Ok(n) if rest.is_empty() => Some(n),
            _ => None,
        }
    }
}

impl Nachlieferung {
    pub fn als_bytes(&self) -> Option<Vec<u8>> {
        borsh::to_vec(self).ok()
    }

    pub fn aus_bytes(daten: &[u8]) -> Option<Nachlieferung> {
        let mut rest = daten;
        match Nachlieferung::deserialize(&mut rest) {
            Ok(n) if rest.is_empty() => Some(n),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn eine_lieferung_passt_in_die_antwort() {
        assert_eq!(super::passende_anzahl(&[100; 10], 1_000), 9, "64 Rahmen und neun Blöcke zu 100");
        assert_eq!(super::passende_anzahl(&[2_000], 1_000), 1, "ein zu großer Block geht allein");
        assert_eq!(super::passende_anzahl(&[400, 400, 400], 1_000), 2);
        assert_eq!(super::passende_anzahl(&[], 1_000), 0);
    }

    use super::*;

    #[test]
    fn wer_nichts_verpasst_hat_fordert_nichts() {
        assert_eq!(Nachforderung::fuer_rueckstand(8, 8), None);
        assert_eq!(Nachforderung::fuer_rueckstand(9, 8), None, "voraus, nicht zurück");
    }

    #[test]
    fn ein_neuling_fordert_ab_eins() {
        assert_eq!(
            Nachforderung::fuer_rueckstand(0, 5),
            Some(Nachforderung::Bloecke { ab: 1, bis: 5 })
        );
    }

    #[test]
    fn eine_luecke_in_der_mitte_beginnt_beim_naechsten() {
        assert_eq!(
            Nachforderung::fuer_rueckstand(3, 7),
            Some(Nachforderung::Bloecke { ab: 4, bis: 7 })
        );
    }

    #[test]
    fn ein_grosser_rueckstand_wird_gedeckelt() {
        // Sonst brächte ein Neuling den Gegenüber dazu, beliebige
        // Datenmengen zu senden: wenig Aufwand beim Fragenden, viel
        // beim Antwortenden.
        let n = Nachforderung::fuer_rueckstand(0, 10_000).unwrap();
        let Nachforderung::Bloecke { ab, bis } = n else {
            panic!("fuer_rueckstand baut nur Blockanfragen");
        };
        assert_eq!(ab, 1);
        assert_eq!(bis, MAX_BLOECKE_JE_LIEFERUNG);
        assert_eq!(bis - ab + 1, MAX_BLOECKE_JE_LIEFERUNG);
    }

    #[test]
    fn mehrere_runden_holen_auch_grosse_rueckstaende_auf() {
        // Jede gelungene Lieferung hebt die Höhe, die nächste Anfrage
        // beginnt dort. Nach genug Runden ist die Lücke zu.
        let ziel = 1_200u64;
        let mut hoehe = 0u64;
        let mut runden = 0;
        while let Some(Nachforderung::Bloecke { bis, .. }) =
            Nachforderung::fuer_rueckstand(hoehe, ziel)
        {
            hoehe = bis;
            runden += 1;
            assert!(runden < 20, "kommt nicht voran");
        }
        assert_eq!(hoehe, ziel);
        assert_eq!(runden, 3, "1200 Blöcke in Schritten von 512: drei Runden");
    }

    #[test]
    fn nachforderung_ueberlebt_den_weg() {
        let n = Nachforderung::Bloecke { ab: 4, bis: 9 };
        let bytes = n.als_bytes().unwrap();
        assert_eq!(Nachforderung::aus_bytes(&bytes), Some(n));
    }

    #[test]
    fn ein_anhaengsel_macht_die_nachforderung_ungueltig() {
        // Zwei Anfragen mit gleichem Inhalt und verschiedenem Anhang
        // wären zwei Anfragen. Das ist ein Kanal, kein Zufall.
        let mut bytes = Nachforderung::Bloecke { ab: 1, bis: 2 }.als_bytes().unwrap();
        bytes.push(0);
        assert_eq!(Nachforderung::aus_bytes(&bytes), None);
    }

    #[test]
    fn unsinn_wird_abgelehnt() {
        assert_eq!(Nachforderung::aus_bytes(&[0xFF; 3]), None);
        assert_eq!(Nachforderung::aus_bytes(&[]), None);
        assert_eq!(Nachlieferung::aus_bytes(&[0xAB; 9]), None);
    }

    /// ⚑ **Die neuen Marken stehen hinten** (2026-10-22): Die alten
    /// behalten ihre Nummern, und ein alter Knoten liest eine neue als
    /// unbekannt statt als etwas anderes.
    #[test]
    fn die_neuen_marken_stehen_hinten() {
        assert_eq!(Nachforderung::Bloecke { ab: 1, bis: 2 }.als_bytes().unwrap()[0], 0);
        assert_eq!(Nachforderung::Zeitprobe { positionen: 4 }.als_bytes().unwrap()[0], 2);
        assert_eq!(Nachforderung::Ping.als_bytes().unwrap()[0], 3);
        assert_eq!(Nachlieferung::Nichts.als_bytes().unwrap()[0], 2);
        assert_eq!(Nachlieferung::Pong.als_bytes().unwrap()[0], 4);
        let d = Nachlieferung::Datei { name: "a.bin".into(), ab: 3, daten: vec![1, 2], groesse: 5 };
        assert_eq!(Nachlieferung::aus_bytes(&d.als_bytes().unwrap()), Some(d));
    }

    #[test]
    fn eine_leere_lieferung_ist_eine_antwort() {
        // „Habe ich nicht" muss sich von „habe nicht geantwortet"
        // unterscheiden lassen, sonst wartet der Fragende auf eine
        // Zeitüberschreitung, die nichts bedeutet.
        let bytes = Nachlieferung::Nichts.als_bytes().unwrap();
        assert_eq!(Nachlieferung::aus_bytes(&bytes), Some(Nachlieferung::Nichts));
    }

    /// Der nächste Bereich beginnt beim kleinsten ungedeckten Block, auch
    /// wenn die vorderste Lieferung verloren ging und spätere noch laufen.
    #[test]
    fn der_naechste_bereich_schliesst_zuerst_die_vorderste_luecke() {
        let nichts = |_: u64| false;
        assert_eq!(naechster_bereich(100, &[], nichts, 2_000), Some((101, 612)));
        assert_eq!(naechster_bereich(100, &[(101, 612)], nichts, 2_000), Some((613, 1124)));
        // Die vorderste (101..612) ging verloren, 613..1124 läuft noch.
        assert_eq!(naechster_bereich(100, &[(613, 1124)], nichts, 2_000), Some((101, 612)));
        // Nicht in den nächsten laufenden hinein.
        assert_eq!(naechster_bereich(100, &[(301, 400)], nichts, 2_000), Some((101, 300)));
        // Was im Vorausspeicher liegt, ist gedeckt.
        assert_eq!(naechster_bereich(100, &[], |h| (101..=150).contains(&h), 2_000), Some((151, 662)));
        assert_eq!(naechster_bereich(100, &[(101, 200)], nichts, 200), None, "alles gedeckt");
        assert_eq!(naechster_bereich(200, &[], nichts, 200), None, "nichts fehlt");
    }

}
