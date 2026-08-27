//! Ununterscheidbarkeit messen (Sicherheitsaudit A11, Kap. 6.7).
//!
//! # Die Frage, und warum sie nicht im Code steht
//!
//! Kap. 6.7 nennt Ununterscheidbarkeit als **erste**
//! Konstruktionsanforderung an Kontrollsegmente: „Erkennt ein Miner sie
//! an statistischen Merkmalen, ist der Mechanismus wirkungslos."
//!
//! [`crate::kontrollsegmente`] sagt selbst, dass es diese Anforderung
//! nicht erfüllen kann, weil sie eine **Eigenschaft der Daten** ist und
//! kein Datentyp erzwingen kann, dass ein Prompt unauffällig ist.
//!
//! Was sich sehr wohl prüfen lässt: **ob die Einschleusung selbst eine
//! Spur hinterlässt**, unabhängig vom Inhalt der Prompts. Genau das tut
//! dieses Modul, und es tut es, indem es den Angriff baut statt ihn zu
//! beschreiben.
//!
//! # Was ein Miner sieht
//!
//! Ein Shard bekommt Segmente mit ihrer [`SegmentId`]. Er kann sich
//! merken, welche er schon gesehen hat. Mehr braucht der erste
//! Unterscheider nicht.
//!
//! # ⚑ Der Vorrat ist endlich, der Auftragsstrom nicht
//!
//! Kontrollsegmente kommen aus einem Vorrat begrenzter Größe. Über einen
//! langen Lauf wird derselbe Vorrat **wiederholt** eingeschleust, und
//! echte Arbeit wiederholt sich nie: Jeder reale Auftrag hat seine
//! eigene Id.
//!
//! **Damit ist jedes zweite Auftreten einer Id ein Beweis**, kein
//! Verdacht. Ein Miner, der eine Liste gesehener Ids führt, erkennt ab
//! dem zweiten Mal sicher, dass ein Segment eine Kontrolle ist, und
//! rechnet genau die ehrlich.
//!
//! Wie viele Kontrollen er damit erwischt, hängt an drei Zahlen: der
//! Vorratsgröße, der Einschleusungsrate und der Länge des Laufs. Diese
//! Abhängigkeit misst [`messe_wiederholung`], statt sie zu behaupten.
//!
//! # Was dieses Modul **nicht** misst
//!
//! Länge, Timing und Kontextprofil der Prompts. Dafür bräuchte es echte
//! Prompt-Verteilungen, und die entstehen erst im Betrieb. Das bleibt
//! die offene Messfrage aus Kap. 11 Punkt 5; hier wird der Teil
//! erledigt, der schon heute entscheidbar ist.

use std::collections::{HashMap, HashSet};

use myl_types::hash::Hash;
use myl_types::ids::SegmentId;

/// Das Ergebnis einer Unterscheidungsmessung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unterscheidungsergebnis {
    /// Länge des betrachteten Auftragsstroms.
    pub auftraege: usize,
    /// Wie viele davon Kontrollsegmente waren.
    pub kontrollen: usize,
    /// Wie viele Kontrollen der Unterscheider **sicher** erkannt hat.
    pub erkannt: usize,
    /// Wie viele reale Aufträge er fälschlich für Kontrollen hielt.
    ///
    /// **Beim Wiederholungsunterscheider immer null**, und das ist der
    /// Punkt: Er rät nicht, er beweist. Ein Fehlalarm wäre nur möglich,
    /// wenn echte Arbeit ihre Ids wiederverwendete.
    pub fehlalarme: usize,
}

impl Unterscheidungsergebnis {
    /// Anteil der erkannten Kontrollen, in Promille.
    ///
    /// Ganzzahlig, weil dieses Crate im Konsenspfad liegt und dort keine
    /// Gleitkomma-Arithmetik vorkommt. Promille statt Prozent, damit
    /// kleine Anteile nicht auf null runden.
    pub fn erkannt_promille(&self) -> u64 {
        if self.kontrollen == 0 {
            return 0;
        }
        (self.erkannt as u64 * 1000) / self.kontrollen as u64
    }

    /// Ob der Unterscheider besser ist als Raten.
    ///
    /// Ein Angreifer ohne Unterscheider trifft eine Kontrolle mit der
    /// Einschleusungsrate γ. Wer sie **sicher** erkennt, ist immer
    /// besser, sobald er überhaupt etwas erkennt: Er hat keine
    /// Fehlalarme.
    pub fn schlaegt_raten(&self) -> bool {
        self.erkannt > 0 && self.fehlalarme == 0
    }
}

/// Der Wiederholungsunterscheider: merkt sich jede gesehene Id.
///
/// Das ist der ganze Angriff. Er braucht keine Statistik, keine
/// Schwellenwerte und keine Annahmen über Prompts, nur ein Gedächtnis.
#[derive(Debug, Default)]
pub struct Wiederholungsunterscheider {
    gesehen: HashSet<SegmentId>,
}

impl Wiederholungsunterscheider {
    pub fn neu() -> Self {
        Self::default()
    }

    /// Meldet ein Segment und sagt, ob es **sicher** eine Kontrolle ist.
    ///
    /// `true` heißt: schon einmal gesehen, also aus einem Vorrat, also
    /// eine Kontrolle. `false` heißt **nicht** „keine Kontrolle",
    /// sondern nur „noch kein Beweis". Der Unterschied ist der Grund,
    /// warum es keine Fehlalarme gibt.
    pub fn melde(&mut self, id: &SegmentId) -> bool {
        !self.gesehen.insert(*id)
    }

    pub fn bekannte(&self) -> usize {
        self.gesehen.len()
    }
}

/// Baut einen Auftragsstrom und misst, wie viele Kontrollen der
/// Wiederholungsunterscheider erkennt.
///
/// `vorrat`: Zahl verschiedener Kontrollsegmente.
/// `auftraege`: Länge des Stroms.
/// `gamma_zaehler`/`gamma_nenner`: Einschleusungsrate.
///
/// Der Strom entsteht deterministisch aus `seed`, damit die Messung
/// wiederholbar ist.
pub fn messe_wiederholung(
    vorrat: usize,
    auftraege: usize,
    gamma_zaehler: u64,
    gamma_nenner: u64,
    seed: &[u8; 32],
) -> Unterscheidungsergebnis {
    let positionen = crate::kontrollsegmente::einschleusungsplan(
        auftraege,
        gamma_zaehler,
        gamma_nenner,
        seed,
    )
    .unwrap_or_default();
    let ist_kontrolle: HashSet<usize> = positionen.iter().copied().collect();

    let mut unterscheider = Wiederholungsunterscheider::neu();
    let mut erkannt = 0usize;
    let mut fehlalarme = 0usize;
    let mut kontrollzaehler = 0usize;

    for i in 0..auftraege {
        let id = if ist_kontrolle.contains(&i) {
            // Aus dem Vorrat: die Ids wiederholen sich zwangsläufig,
            // sobald mehr eingeschleust wird als der Vorrat hält.
            let n = kontrollzaehler % vorrat.max(1);
            kontrollzaehler += 1;
            kontroll_id(seed, n)
        } else {
            // Echte Arbeit: jede Id neu.
            echt_id(seed, i)
        };
        let verdacht = unterscheider.melde(&id);
        if verdacht {
            if ist_kontrolle.contains(&i) {
                erkannt += 1;
            } else {
                fehlalarme += 1;
            }
        }
    }

    Unterscheidungsergebnis {
        auftraege,
        kontrollen: positionen.len(),
        erkannt,
        fehlalarme,
    }
}

/// Was die **Kontingentgrenze** einem Miner zusätzlich verrät.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kontingentergebnis {
    /// Länge des betrachteten Auftragsstroms.
    pub auftraege: usize,
    /// Wie viele davon Kontrollsegmente waren.
    pub kontrollen: usize,
    /// Wie viele Kontrollen der Strom nach der Formel des Plans
    /// enthält. Das ist die Zahl, die ein Miner selbst ausrechnen kann.
    pub kontingent: usize,
    /// Wie viele Aufträge der Miner für Kontrollen hielt.
    pub verdaechtig: usize,
    /// Wie viele davon in Wahrheit echt waren.
    pub fehlalarme: usize,
    /// Wie viele Aufträge **sicher als echt** galten, weil das
    /// Kontingent des Stroms erschöpft war.
    pub sicher_echt: usize,
    /// Wie viele dieser Aufträge in Wahrheit Kontrollen waren.
    ///
    /// Muss null sein. Ist es das nicht, ist die Überlegung hinter der
    /// Kontingentgrenze falsch, und das wäre der interessantere Befund.
    pub sicher_echt_falsch: usize,
    /// Wie viele echte Aufträge es insgesamt gab.
    pub echte: usize,
}

impl Kontingentergebnis {
    /// Anteil der echten Aufträge, die sicher als echt erkannt wurden,
    /// in Promille.
    ///
    /// Das ist die Zahl, die zählt: Auf einem Auftrag, von dem ein Miner
    /// **weiß**, dass er keine Kontrolle ist, kostet Betrug ihn nichts
    /// mehr, jedenfalls nicht über diesen Mechanismus.
    pub fn sicher_echt_promille(&self) -> u64 {
        if self.echte == 0 {
            return 0;
        }
        (self.sicher_echt as u64 * 1_000) / self.echte as u64
    }
}

/// Misst, wie viel die **feste Zahl** von Kontrollen je Strom verrät.
///
/// # ⚑ Der Einschleusungsplan zieht ohne Zurücklegen
///
/// [`crate::kontrollsegmente::einschleusungsplan`] wählt aus `n`
/// Positionen **genau** `⌈n·γ⌉` aus. Ein echter Bernoulli-Prozess würde
/// je Auftrag unabhängig mit Wahrscheinlichkeit γ ziehen, und dann
/// schwankte die Zahl. Hier schwankt sie nicht.
///
/// γ ist ein Governance-Parameter und damit öffentlich, `n` sieht ein
/// Miner am Strom. **Also kennt er das Kontingent.** Hat er so viele
/// Kontrollen erkannt, wie hineinpassen, ist jeder weitere Auftrag des
/// Stroms mit Sicherheit echt: kein Verdacht, kein Schwellenwert, ein
/// Beweis.
///
/// # ⚑ Fund 72: Das Kontingent wird nie erschöpft, und das hat einen Grund
///
/// Gemessen wurde, ob die Kontingentgrenze ein eigenes Loch ist oder
/// ein bestehendes vertieft. Sie tut **weder das eine noch das
/// andere**, und die Null in der Messung ist kein glücklicher Zufall,
/// sondern folgt aus der Bauart des Unterscheiders:
///
/// **Die Wiederholung erkennt ein Segment frühestens beim zweiten
/// Auftreten.** Von `v` verschiedenen Vorratssegmenten bleiben die
/// ersten `min(v, k)` Auftreten also unerkannt, und damit gilt
/// zwangsläufig
///
/// ```text
/// verdächtig ≤ k − min(v, k) < k = Kontingent
/// ```
///
/// für jeden Vorrat `v ≥ 1`. Erschöpfen ließe sich das Kontingent nur
/// mit einem Unterscheider, der **jede** Kontrolle erkennt, und wer den
/// hat, braucht das Kontingent nicht mehr.
///
/// Gemessen bei γ = 2 % über 100 000 Aufträge, Kontingent 2 000:
///
/// | Vorrat | verdächtig | sicher echt |
/// |---|---|---|
/// | 64 | 1 936 | 0 |
/// | 256 | 1 744 | 0 |
/// | 1 024 | 976 | 0 |
/// | 2 048 | 0 | 0 |
///
/// Die verdächtigen Zahlen sind exakt `2 000 − Vorrat`, also der Satz
/// oben mit Gleichheit.
///
/// **Folgerung: `einschleusungsplan` bleibt, wie er ist.** Der
/// naheliegende Umbau wäre, je Position unabhängig mit
/// Wahrscheinlichkeit γ zu ziehen statt genau `⌈n·γ⌉` von `n`; dann
/// gäbe es kein Kontingent zu erschöpfen. Er würde ein Loch schließen,
/// das es nicht gibt, und dafür die exakte Einhaltung von γ je Strom
/// aufgeben, an der die Kostenbuchhaltung aus Punkt 3.4 hängt.
///
/// **Ein Nullergebnis mit Begründung ist mehr wert als eines ohne.**
/// Ohne den Satz stünde hier eine Messung, die für die geprüften fünf
/// Vorratsgrößen null ergab, und niemand wüsste, ob die sechste anders
/// ausfällt.
pub fn messe_kontingent(
    vorrat: usize,
    auftraege: usize,
    gamma_zaehler: u64,
    gamma_nenner: u64,
    seed: &[u8; 32],
) -> Kontingentergebnis {
    let positionen = crate::kontrollsegmente::einschleusungsplan(
        auftraege,
        gamma_zaehler,
        gamma_nenner,
        seed,
    )
    .unwrap_or_default();
    let ist_kontrolle: HashSet<usize> = positionen.iter().copied().collect();
    // Was der Miner ausrechnen kann: dieselbe Formel wie der Plan.
    let kontingent = positionen.len();

    let mut unterscheider = Wiederholungsunterscheider::neu();
    // ⚑ Gezählt wird, was der **Miner** zählt, nicht was ein Orakel
    // wüsste: jede Meldung, ob sie stimmt oder nicht. Ein Angreifer
    // kennt `ist_kontrolle` nicht. Dass die Zahl trotzdem der Wahrheit
    // entspricht, liegt an den null Fehlalarmen der Wiederholung, und
    // genau das prüft `fehlalarme` unten mit.
    let mut verdaechtig = 0usize;
    let mut fehlalarme = 0usize;
    let mut sicher_echt = 0usize;
    let mut sicher_echt_falsch = 0usize;
    let mut kontrollzaehler = 0usize;

    for i in 0..auftraege {
        // Vor der Meldung: Ist das Kontingent schon voll, steht die
        // Antwort für diesen Auftrag bereits fest.
        if verdaechtig >= kontingent {
            sicher_echt += 1;
            if ist_kontrolle.contains(&i) {
                sicher_echt_falsch += 1;
            }
        }

        let id = if ist_kontrolle.contains(&i) {
            let n = kontrollzaehler % vorrat.max(1);
            kontrollzaehler += 1;
            kontroll_id(seed, n)
        } else {
            echt_id(seed, i)
        };
        if unterscheider.melde(&id) {
            verdaechtig += 1;
            if !ist_kontrolle.contains(&i) {
                fehlalarme += 1;
            }
        }
    }

    Kontingentergebnis {
        auftraege,
        kontrollen: positionen.len(),
        kontingent,
        verdaechtig,
        fehlalarme,
        sicher_echt,
        sicher_echt_falsch,
        echte: auftraege - positionen.len(),
    }
}

/// Die Id des `n`-ten Kontrollsegments im Vorrat.
fn kontroll_id(seed: &[u8; 32], n: usize) -> SegmentId {
    let mut daten = Vec::with_capacity(48);
    daten.extend_from_slice(seed);
    daten.extend_from_slice(b"kontrolle");
    daten.extend_from_slice(&(n as u64).to_le_bytes());
    let h = Hash::sha256(&daten);
    let mut roh = [0u8; 32];
    roh.copy_from_slice(h.as_bytes());
    SegmentId::new(roh)
}

/// Die Id des `i`-ten echten Auftrags.
fn echt_id(seed: &[u8; 32], i: usize) -> SegmentId {
    let mut daten = Vec::with_capacity(48);
    daten.extend_from_slice(seed);
    daten.extend_from_slice(b"echt");
    daten.extend_from_slice(&(i as u64).to_le_bytes());
    let h = Hash::sha256(&daten);
    let mut roh = [0u8; 32];
    roh.copy_from_slice(h.as_bytes());
    SegmentId::new(roh)
}

/// Wie groß der Vorrat sein muss, damit über `auftraege` Aufträge bei
/// Rate γ **keine** Id zweimal eingeschleust wird.
///
/// Das ist die Untergrenze, unterhalb derer der
/// Wiederholungsunterscheider zwangsläufig greift: Wer öfter
/// einschleust, als er verschiedene Segmente hat, wiederholt sich.
///
/// **Eine notwendige, keine hinreichende Bedingung.** Auch ein großer
/// Vorrat schützt nicht gegen Unterscheidung an Länge, Timing oder
/// Inhalt; er beseitigt nur diese eine, sichere Spur.
/// **In `u64` und nicht in `usize`.** Der Wert wandert in die
/// Parameter-Registry der Governance, und die rechnet durchgehend in
/// `u64`; eine Umrechnung an der Grenze wäre genau die Stelle, an der
/// auf einer 32-Bit-Maschine eine Sicherheitsschranke stillschweigend
/// abgeschnitten würde.
pub fn noetiger_vorrat(auftraege: u64, gamma_zaehler: u64, gamma_nenner: u64) -> u64 {
    if gamma_nenner == 0 {
        return 0;
    }
    let n = (auftraege as u128 * gamma_zaehler as u128).div_ceil(gamma_nenner as u128);
    // Gesättigt statt abgeschnitten: Ein Fenster nahe `u64::MAX` mit
    // γ > 1 ergäbe sonst eine **kleine** Zahl, und die Schranke wäre
    // gerade dort wirkungslos, wo sie am nötigsten ist. γ > 1 lehnt die
    // Governance ohnehin ab; darauf verlassen sich soll sich diese
    // Funktion nicht.
    n.min(u64::MAX as u128) as u64
}

/// Wie viele Aufträge ein Vorrat trägt, bevor sich die erste Id
/// wiederholt.
///
/// Die Umkehrung von [`noetiger_vorrat`]; in `u64` aus demselben Grund.
pub fn reichweite(vorrat: u64, gamma_zaehler: u64, gamma_nenner: u64) -> u64 {
    if gamma_zaehler == 0 {
        return u64::MAX;
    }
    let n = (vorrat as u128 * gamma_nenner as u128) / gamma_zaehler as u128;
    n.min(u64::MAX as u128) as u64
}

/// Zählt, wie oft jede Id im Strom vorkommt. Für Diagnose.
pub fn haeufigkeiten(ids: &[SegmentId]) -> HashMap<SegmentId, usize> {
    let mut m = HashMap::new();
    for id in ids {
        *m.entry(*id).or_insert(0) += 1;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: [u8; 32] = [7u8; 32];

    /// **Der Unterscheider hat keine Fehlalarme.**
    ///
    /// Er rät nicht, er beweist: Echte Arbeit wiederholt ihre Ids nie.
    #[test]
    fn der_unterscheider_meldet_nie_einen_echten_auftrag() {
        let e = messe_wiederholung(1_000, 5_000, 1, 50, &SEED);
        assert_eq!(e.fehlalarme, 0, "ein echter Auftrag wurde für eine Kontrolle gehalten");
    }

    /// **⚑ Der Kern von A11: Ein kleiner Vorrat verrät sich vollständig.**
    #[test]
    fn ein_kleiner_vorrat_wird_fast_vollstaendig_erkannt() {
        // 64 Kontrollsegmente, 20 000 Aufträge, γ = 2 %: 400
        // Einschleusungen aus 64 Ids. Ab der 65. ist jede eine
        // Wiederholung.
        let e = messe_wiederholung(64, 20_000, 2, 100, &SEED);
        assert!(e.kontrollen >= 400);
        assert!(
            e.erkannt_promille() > 800,
            "nur {} Promille erkannt, erwartet über 800",
            e.erkannt_promille()
        );
        assert!(e.schlaegt_raten());
        println!(
            "[A11] Vorrat 64, {} Aufträge, γ=2%: {} von {} Kontrollen erkannt ({} Promille), \
             {} Fehlalarme",
            e.auftraege, e.erkannt, e.kontrollen, e.erkannt_promille(), e.fehlalarme
        );
    }

    /// **Ein hinreichend großer Vorrat schließt genau diese Spur.**
    #[test]
    fn ein_grosser_vorrat_hinterlaesst_keine_wiederholung() {
        // Vorrat >= Zahl der Einschleusungen: keine Id kommt zweimal.
        let auftraege = 20_000;
        let noetig = noetiger_vorrat(auftraege as u64, 2, 100) as usize;
        let e = messe_wiederholung(noetig, auftraege, 2, 100, &SEED);
        assert_eq!(
            e.erkannt, 0,
            "trotz ausreichendem Vorrat ({noetig}) wurden {} Kontrollen erkannt",
            e.erkannt
        );
        assert_eq!(e.erkannt_promille(), 0);
    }

    /// Die Grenze liegt genau dort, wo die Rechnung sie erwartet.
    #[test]
    fn die_reichweite_eines_vorrats_stimmt_mit_der_messung_ueberein() {
        // Vorrat 100, γ = 1 %: trägt 10 000 Aufträge.
        assert_eq!(reichweite(100, 1, 100), 10_000);
        let knapp_darunter = messe_wiederholung(100, 9_000, 1, 100, &SEED);
        assert_eq!(knapp_darunter.erkannt, 0, "vor der Grenze darf nichts auffallen");
        let darueber = messe_wiederholung(100, 30_000, 1, 100, &SEED);
        assert!(darueber.erkannt > 0, "nach der Grenze muss es auffallen");
    }

    #[test]
    fn ohne_einschleusung_gibt_es_nichts_zu_erkennen() {
        let e = messe_wiederholung(64, 1_000, 0, 100, &SEED);
        assert_eq!(e.kontrollen, 0);
        assert_eq!(e.erkannt, 0);
        assert!(!e.schlaegt_raten(), "ohne Kontrollen schlägt niemand das Raten");
    }

    #[test]
    fn der_noetige_vorrat_waechst_mit_strom_und_rate() {
        assert_eq!(noetiger_vorrat(10_000, 1, 100), 100);
        assert_eq!(noetiger_vorrat(10_000, 5, 100), 500);
        assert_eq!(noetiger_vorrat(100_000, 1, 100), 1_000);
        assert_eq!(noetiger_vorrat(0, 1, 100), 0);
    }

    /// **Kein stiller Überlauf an den Rändern.**
    ///
    /// Die Governance prüft γ ≤ 1, bevor sie diese Schranke auswertet.
    /// Verließe sich die Rechnung darauf, ergäbe ein Fenster nahe
    /// `u64::MAX` mit γ > 1 nach dem Abschneiden eine **kleine** Zahl —
    /// die Schranke wäre gerade dort wirkungslos, wo sie greifen müsste.
    #[test]
    fn die_schranke_saettigt_statt_abzuschneiden() {
        assert_eq!(noetiger_vorrat(u64::MAX, 1, 1), u64::MAX);
        assert_eq!(noetiger_vorrat(u64::MAX, 2, 1), u64::MAX);
        assert_eq!(reichweite(u64::MAX, 1, 2), u64::MAX);
        // Und ohne Einschleusung trägt jeder Vorrat alles.
        assert_eq!(reichweite(1, 0, 100), u64::MAX);
    }

    #[test]
    fn haeufigkeiten_zaehlen_richtig() {
        let a = kontroll_id(&SEED, 0);
        let b = kontroll_id(&SEED, 1);
        let h = haeufigkeiten(&[a, b, a]);
        assert_eq!(h.get(&a), Some(&2));
        assert_eq!(h.get(&b), Some(&1));
    }

    #[test]
    fn kontroll_und_echt_ids_kollidieren_nicht() {
        // Sonst zählte der Unterscheider eine echte Id als Wiederholung,
        // und die Messung wäre wertlos.
        let k: Vec<SegmentId> = (0..500).map(|n| kontroll_id(&SEED, n)).collect();
        let e: Vec<SegmentId> = (0..500).map(|i| echt_id(&SEED, i)).collect();
        for id in &k {
            assert!(!e.contains(id), "Id-Raum überlappt");
        }
    }
}

#[cfg(test)]
mod fund_72 {
    use super::*;

    /// **Das Kontingent wird nie erschöpft.**
    ///
    /// Über den ganzen Bereich von einem Vorrat, der nichts trägt, bis
    /// zu einem, der reichlich trägt.
    #[test]
    fn kein_auftrag_gilt_je_sicher_als_echt() {
        for vorrat in [1usize, 8, 64, 256, 1_024, 2_048, 4_096] {
            let k = messe_kontingent(vorrat, 20_000, 2, 100, &[3u8; 32]);
            assert_eq!(
                k.sicher_echt, 0,
                "Vorrat {vorrat}: das Kontingent war erschöpft"
            );
            assert_eq!(k.sicher_echt_falsch, 0);
        }
    }

    /// Der Satz aus dem Modulkopf, an Zahlen geprüft:
    /// `verdächtig ≤ Kontingent − min(Vorrat, Kontingent)`.
    ///
    /// Das ist die Aussage, die das Nullergebnis über die geprüften
    /// Vorratsgrößen hinaus trägt.
    #[test]
    fn die_verdachtszahl_bleibt_um_den_vorrat_unter_dem_kontingent() {
        for vorrat in [1usize, 8, 64, 256, 1_024, 2_048] {
            let k = messe_kontingent(vorrat, 20_000, 2, 100, &[3u8; 32]);
            let schranke = k.kontingent - vorrat.min(k.kontingent);
            assert!(
                k.verdaechtig <= schranke,
                "Vorrat {vorrat}: {} verdächtig, Schranke {schranke}",
                k.verdaechtig
            );
            assert!(
                k.verdaechtig < k.kontingent,
                "Vorrat {vorrat}: das Kontingent wurde erreicht"
            );
        }
    }

    /// Bei ausreichendem Vorrat ist die Schranke straff: Der Miner
    /// erkennt genau `Kontingent − Vorrat`, keinen mehr.
    ///
    /// Ohne diese Gegenprobe hieße die Ungleichung oben auch dann
    /// „erfüllt", wenn der Unterscheider gar nichts fände.
    #[test]
    fn die_schranke_wird_erreicht_solange_der_vorrat_umlaeuft() {
        for vorrat in [64usize, 256, 1_024] {
            let k = messe_kontingent(vorrat, 100_000, 2, 100, &[7u8; 32]);
            assert_eq!(
                k.verdaechtig,
                k.kontingent - vorrat,
                "Vorrat {vorrat}: die Schranke ist nicht straff"
            );
        }
    }

    /// Null Fehlalarme, und darauf beruht die ganze Rechnung.
    ///
    /// Gezählt wird, was der Miner für eine Kontrolle hält. Dass diese
    /// Zahl der Wahrheit entspricht, gilt nur, weil die Wiederholung
    /// nie danebengreift: Echte Arbeit wiederholt sich nicht.
    #[test]
    fn die_wiederholung_meldet_keine_fehlalarme() {
        for vorrat in [8usize, 64, 1_024] {
            let k = messe_kontingent(vorrat, 20_000, 2, 100, &[3u8; 32]);
            assert_eq!(k.fehlalarme, 0, "Vorrat {vorrat}");
        }
    }

    /// Das Kontingent ist genau die Zahl der Kontrollen im Strom.
    ///
    /// Ein Miner rechnet es aus `n` und γ aus, beides ist ihm bekannt.
    /// Ginge die Zahl auseinander, wäre die ganze Überlegung hinfällig,
    /// weil der Miner dann gar nicht wüsste, wann Schluss ist.
    #[test]
    fn das_kontingent_ist_die_zahl_der_kontrollen() {
        for (gz, gn) in [(2u64, 100u64), (1, 20), (5, 100)] {
            let k = messe_kontingent(512, 20_000, gz, gn, &[3u8; 32]);
            assert_eq!(k.kontingent, k.kontrollen);
            assert_eq!(k.echte + k.kontrollen, k.auftraege);
        }
    }
}
