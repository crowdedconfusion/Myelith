//! **Was eine Verbindung belegt, wenn sie voll ist** (Simulation vom
//! 2026-11-20, Ursache hinter Fund 715 im Knoten).
//!
//! Je Verbindung lässt `libp2p-request-response` höchstens
//! [`crate::MAX_ANFRAGEN_ZUGLEICH`] eigene Anfragen zugleich laufen; jede
//! weitere scheitert sofort mit „max sub-streams reached“. Im Auftragslauf
//! mit dem 35B geschah das tausendfach, und aus den Protokollen war nicht
//! abzulesen, **welche** Anfragen die Plätze hielten: Nachschub, Relais der
//! Shards, Spuren, Zeitproben?
//!
//! Hier steht je eigener Anfrage die Gegenstelle, die **Art** (das erste
//! Byte der Nachricht: Borsh schreibt dort die Variante, beim Knoten die von
//! `Nachforderung`) und der Beginn. Scheitert eine Anfrage am Stau, sagt
//! [`Unterwegs::bild`], was zu dieser Gegenstelle unterwegs ist, je Art mit
//! Anzahl und Alter der ältesten. Gemeldet wird je Gegenstelle höchstens
//! einmal je [`STAU_MELDEABSTAND`], damit die Mitschrift nicht selbst zur
//! Flut wird.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// ⚑ **Warten statt scheitern** (Simulation vom 2026-11-23): Das Bild
/// zeigte, wer die Plätze hielt: Salven von Spuranfragen der Stichprobe, 27
/// bis 35 auf einmal an dieselbe Gegenstelle, die älteste 1 bis 3 ms alt.
/// Alles über der Grenze scheiterte sofort, und der Nachschub des Knotens
/// lief dagegen (Fund 715). Jetzt wartet eine Anfrage ohne Zweck, wenn zur
/// Gegenstelle schon [`crate::MAX_ANFRAGEN_ZUGLEICH`] unterwegs sind, in
/// einer Schlange und geht hinaus, sobald eine Antwort oder ein Fehlschlag
/// einen Platz frei macht. ⛔️ **Eine Anfrage mit Zweck wartet nie:** Die
/// Zeitprobe misst ab dem Absenden im Knoten; eine Wartezeit hier zählte
/// als Rechenzeit des Gemessenen. Sie scheitert wie bisher und zählt dort
/// nicht (Fund 715).
///
/// ⚑ **Nach Bytes begrenzt, nicht nach Anzahl** (Fund 720, Auftragslauf vom
/// 2026-11-24): Mit höchstens 256 wartenden lief die Schlange am Wechsel
/// über. Die Stichprobe zieht 5 % der Segmente einer Epoche und fragt jedes
/// Mitglied einzeln, bei rund 12 500 Segmenten je Pod über 600 Spuranfragen
/// an jedes Mitglied auf einmal; der Rest scheiterte, und die Prüfung blieb
/// lückenhaft. Eine Spuranfrage hat 45 Byte; beim Deckel (100 000 Segmente)
/// sind es 5 000 je Pod, rund 225 KB. 16 MiB fassen das siebzigfach und ein
/// Stück einer Spurablage (3 MiB) fünfmal.
pub const WARTESCHLANGE_HOECHSTENS_BYTES: usize = 16 << 20;

/// ⚑ **Die Vorrangspur** (Fund 720): Anfragen, die der Knoten für seinen
/// eigenen Pod weiterreicht (Rechnen, Ablage, Bündel), warten getrennt und
/// gehen vor jeder wartenden Anfrage ohne Vorrang hinaus. 📌 Im Lauf vom
/// 24. standen die Stapel des Koordinators hinter der Stichprobe desselben
/// Knotens und wurden als „voll“ abgewiesen; die Runde ging verloren. Nur
/// der eigene Pod speist diese Spur (über die Relaisleitung mit Ausweis),
/// seine Nebenläufigkeit begrenzt sie; die Grenze hier ist ein Netz darunter.
pub const VORRANG_HOECHSTENS: usize = 256;

/// Der Grund, mit dem eine Anfrage scheitert, wenn die Schlange voll ist;
/// [`ist_stau`] erkennt ihn wie den der Bibliothek.
pub const WARTESCHLANGE_VOLL: &str = "die Warteschlange zu dieser Gegenstelle ist voll";

/// Höchstens so oft je Gegenstelle ein Bild des Staus.
pub const STAU_MELDEABSTAND: Duration = Duration::from_secs(10);

/// Was zu einer Gegenstelle unterwegs ist, je Art einer Anfrage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stauanteil {
    /// Das erste Byte der Anfrage.
    pub art: u8,
    /// Wie viele dieser Art unterwegs sind.
    pub anzahl: u32,
    /// Wie lange die älteste schon läuft, in Millisekunden.
    pub aeltest_ms: u64,
    /// Wie viele dieser Art in der Schlange warten.
    pub wartend: u32,
}

/// Die eigenen Anfragen, die noch keine Antwort und keinen Fehlschlag
/// hatten.
#[derive(Debug)]
pub struct Unterwegs<K: Hash + Eq> {
    anfragen: HashMap<K, (libp2p::PeerId, u8, Instant)>,
    gemeldet: HashMap<libp2p::PeerId, Instant>,
    /// Je Gegenstelle die wartenden Anfragen ohne Zweck, in Folge.
    warten: HashMap<libp2p::PeerId, Schlange>,
}

/// Die wartenden Anfragen zu einer Gegenstelle: zuerst die mit Vorrang,
/// jede mit ihrem Zweck (Fund 721).
#[derive(Debug, Default)]
struct Schlange {
    vorrang: std::collections::VecDeque<(Vec<u8>, u64)>,
    normal: std::collections::VecDeque<Vec<u8>>,
    /// Bytes in `normal`.
    bytes: usize,
}

impl Schlange {
    fn laenge(&self) -> usize {
        self.vorrang.len() + self.normal.len()
    }
}

impl<K: Hash + Eq> Default for Unterwegs<K> {
    fn default() -> Self {
        Self { anfragen: HashMap::new(), gemeldet: HashMap::new(), warten: HashMap::new() }
    }
}

/// Was mit einer neuen Anfrage ohne Zweck geschieht.
#[derive(Debug, PartialEq, Eq)]
pub enum Einreihen {
    /// Ein Platz ist frei: jetzt senden.
    Senden(Vec<u8>),
    /// Eingereiht; geht hinaus, wenn ein Platz frei wird.
    Wartet,
    /// Die Schlange ist voll ([`WARTESCHLANGE_HOECHSTENS_BYTES`], bei
    /// Vorrang [`VORRANG_HOECHSTENS`]): verwerfen.
    Voll,
}

impl<K: Hash + Eq> Unterwegs<K> {
    /// Eine Anfrage ist hinausgegangen; `daten` ist die Nachricht, ihr erstes
    /// Byte die Art (leer zählt als 255).
    pub fn beginnen(&mut self, kennung: K, an: libp2p::PeerId, daten: &[u8], jetzt: Instant) {
        self.anfragen.insert(kennung, (an, daten.first().copied().unwrap_or(u8::MAX), jetzt));
    }

    /// Antwort oder Fehlschlag: die Anfrage ist nicht mehr unterwegs.
    pub fn beenden(&mut self, kennung: &K) {
        self.anfragen.remove(kennung);
    }

    /// Wie viele zu `an` unterwegs sind.
    pub fn zu(&self, an: &libp2p::PeerId) -> usize {
        self.anfragen.values().filter(|(p, _, _)| p == an).count()
    }

    /// **Eine Anfrage ohne Zweck einreihen**: senden, wenn zu `an` weniger
    /// als `grenze` unterwegs sind und niemand vor ihr wartet; sonst warten.
    pub fn einreihen(&mut self, an: libp2p::PeerId, daten: Vec<u8>, grenze: usize) -> Einreihen {
        let frei = self.zu(&an) < grenze;
        let schlange = self.warten.entry(an).or_default();
        if schlange.laenge() == 0 && frei {
            self.leere_entfernen(&an);
            return Einreihen::Senden(daten);
        }
        if schlange.bytes + daten.len() > WARTESCHLANGE_HOECHSTENS_BYTES {
            self.leere_entfernen(&an);
            return Einreihen::Voll;
        }
        schlange.bytes += daten.len();
        schlange.normal.push_back(daten);
        Einreihen::Wartet
    }

    /// **Eine Anfrage mit Vorrang einreihen** (Fund 720): senden, wenn ein
    /// Platz frei ist und keine andere mit Vorrang wartet, auch wenn Anfragen
    /// ohne Vorrang warten; sonst vor diesen warten. Der `zweck` wartet mit
    /// und kommt mit [`Self::naechste`] oder [`Self::vergessen`] zurück.
    pub fn einreihen_vorrang(&mut self, an: libp2p::PeerId, daten: Vec<u8>, zweck: u64, grenze: usize) -> Einreihen {
        let frei = self.zu(&an) < grenze;
        let schlange = self.warten.entry(an).or_default();
        if schlange.vorrang.is_empty() && frei {
            self.leere_entfernen(&an);
            return Einreihen::Senden(daten);
        }
        if schlange.vorrang.len() >= VORRANG_HOECHSTENS {
            self.leere_entfernen(&an);
            return Einreihen::Voll;
        }
        schlange.vorrang.push_back((daten, zweck));
        Einreihen::Wartet
    }

    fn leere_entfernen(&mut self, an: &libp2p::PeerId) {
        if self.warten.get(an).is_some_and(|s| s.laenge() == 0) {
            self.warten.remove(an);
        }
    }

    /// Die nächste wartende Anfrage an `an`, wenn ein Platz frei ist, mit
    /// ihrem Zweck, wenn sie mit Vorrang wartete.
    pub fn naechste(&mut self, an: &libp2p::PeerId, grenze: usize) -> Option<(Vec<u8>, Option<u64>)> {
        if self.zu(an) >= grenze {
            return None;
        }
        let schlange = self.warten.get_mut(an)?;
        let d = match schlange.vorrang.pop_front() {
            Some((d, zweck)) => Some((d, Some(zweck))),
            None => {
                let d = schlange.normal.pop_front();
                schlange.bytes -= d.as_ref().map_or(0, Vec::len);
                d.map(|d| (d, None))
            }
        };
        self.leere_entfernen(an);
        d
    }

    /// Wie viele zu `an` warten, mit und ohne Vorrang.
    pub fn wartend(&self, an: &libp2p::PeerId) -> usize {
        self.warten.get(an).map_or(0, Schlange::laenge)
    }

    /// Wie viele Anfragen gerade unterwegs sind, über alle Gegenstellen.
    pub fn anzahl(&self) -> usize {
        self.anfragen.len()
    }

    /// **Das Bild des Staus zu `an`**, nach Art geordnet, oder `None`, wenn
    /// für diese Gegenstelle schon innerhalb von [`STAU_MELDEABSTAND`] eines
    /// gemeldet wurde. Eine Gegenstelle, die getrennt wurde, vergisst
    /// [`Self::vergessen`].
    pub fn bild(&mut self, an: &libp2p::PeerId, jetzt: Instant) -> Option<Vec<Stauanteil>> {
        if self.gemeldet.get(an).is_some_and(|t| jetzt.saturating_duration_since(*t) < STAU_MELDEABSTAND) {
            return None;
        }
        self.gemeldet.insert(*an, jetzt);
        let mut je_art: std::collections::BTreeMap<u8, (u32, u64, u32)> = std::collections::BTreeMap::new();
        for (p, art, seit) in self.anfragen.values() {
            if p != an {
                continue;
            }
            let alter = u64::try_from(jetzt.saturating_duration_since(*seit).as_millis()).unwrap_or(u64::MAX);
            let e = je_art.entry(*art).or_insert((0, 0, 0));
            e.0 += 1;
            e.1 = e.1.max(alter);
        }
        for d in self.warten.get(an).into_iter().flat_map(|s| s.vorrang.iter().map(|(d, _)| d).chain(s.normal.iter())) {
            je_art.entry(d.first().copied().unwrap_or(u8::MAX)).or_insert((0, 0, 0)).2 += 1;
        }
        Some(
            je_art
                .into_iter()
                .map(|(art, (anzahl, aeltest_ms, wartend))| Stauanteil { art, anzahl, aeltest_ms, wartend })
                .collect(),
        )
    }

    /// Eine getrennte Gegenstelle: ihre Meldesperre fällt weg. Ihre Anfragen
    /// unterwegs beendet libp2p selbst mit einem Fehlschlag; die wartenden
    /// kommen zurück, je mit ihrem Zweck, damit der Aufrufer sie als
    /// gescheitert meldet.
    pub fn vergessen(&mut self, an: &libp2p::PeerId) -> Vec<Option<u64>> {
        self.gemeldet.remove(an);
        self.warten.remove(an).map_or_else(Vec::new, |s| {
            s.vorrang.iter().map(|(_, z)| Some(*z)).chain(s.normal.iter().map(|_| None)).collect()
        })
    }
}

/// 📌 **Ein Stau, am Text der Bibliothek erkannt** (wie im Knoten). Der Text
/// stammt aus `libp2p-request-response` (`OutboundStreamFailed`).
pub fn ist_stau(grund: &str) -> bool {
    grund.ends_with("max sub-streams reached") || grund == WARTESCHLANGE_VOLL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn das_bild_zaehlt_je_art_und_nur_die_eine_gegenstelle() {
        let (a, b) = (libp2p::PeerId::random(), libp2p::PeerId::random());
        let t0 = Instant::now();
        let mut u: Unterwegs<u64> = Unterwegs::default();
        u.beginnen(1, a, &[9, 1, 2], t0);
        u.beginnen(2, a, &[9], t0 + Duration::from_secs(5));
        u.beginnen(3, a, &[0], t0 + Duration::from_secs(8));
        u.beginnen(4, b, &[9], t0);
        u.beginnen(5, a, &[], t0);
        u.beenden(&5);
        let jetzt = t0 + Duration::from_secs(10);
        let bild = u.bild(&a, jetzt).expect("erstes Bild");
        assert_eq!(
            bild,
            vec![
                Stauanteil { art: 0, anzahl: 1, aeltest_ms: 2_000, wartend: 0 },
                Stauanteil { art: 9, anzahl: 2, aeltest_ms: 10_000, wartend: 0 }
            ]
        );
        assert_eq!(u.anzahl(), 4);
    }

    #[test]
    fn je_gegenstelle_hoechstens_ein_bild_im_abstand() {
        let a = libp2p::PeerId::random();
        let t0 = Instant::now();
        let mut u: Unterwegs<u64> = Unterwegs::default();
        assert!(u.bild(&a, t0).is_some());
        assert!(u.bild(&a, t0 + Duration::from_secs(9)).is_none());
        assert!(u.bild(&a, t0 + STAU_MELDEABSTAND).is_some());
        u.vergessen(&a);
        assert!(u.bild(&a, t0 + STAU_MELDEABSTAND + Duration::from_secs(1)).is_some());
    }

    /// Bis zur Grenze wird gesendet, darüber gewartet, in Folge; ein freier
    /// Platz gibt die älteste wartende frei; eine andere Gegenstelle ist
    /// davon unberührt. Gegenprobe: ohne Schlange scheiterte die dritte.
    #[test]
    fn ueber_der_grenze_wird_gewartet_und_in_folge_nachgeschoben() {
        let (a, b) = (libp2p::PeerId::random(), libp2p::PeerId::random());
        let t0 = Instant::now();
        let mut u: Unterwegs<u64> = Unterwegs::default();
        for (k, d) in [(1u64, vec![9]), (2, vec![9])] {
            assert_eq!(u.einreihen(a, d.clone(), 2), Einreihen::Senden(d.clone()));
            u.beginnen(k, a, &d, t0);
        }
        assert_eq!(u.einreihen(a, vec![1], 2), Einreihen::Wartet);
        assert_eq!(u.einreihen(a, vec![2], 2), Einreihen::Wartet);
        assert_eq!(u.einreihen(b, vec![7], 2), Einreihen::Senden(vec![7]));
        assert_eq!(u.wartend(&a), 2);
        assert_eq!(u.naechste(&a, 2), None, "kein Platz frei");
        let bild = u.bild(&a, t0).expect("Bild");
        assert_eq!(bild.iter().map(|s| (s.art, s.anzahl, s.wartend)).collect::<Vec<_>>(), vec![(1, 0, 1), (2, 0, 1), (9, 2, 0)]);
        u.beenden(&1);
        assert_eq!(u.naechste(&a, 2), Some((vec![1], None)), "die aeltere zuerst");
        u.beginnen(3, a, &[1], t0);
        assert_eq!(u.naechste(&a, 2), None);
        assert_eq!(u.vergessen(&a), vec![None], "die wartende kommt als gescheitert zurueck");
        assert_eq!(u.wartend(&a), 0);
    }

    /// Die Schlange ist nach Bytes begrenzt: genau bis zur Grenze passt es,
    /// ein Byte mehr nicht; ein freier Platz macht wieder Raum.
    #[test]
    fn eine_volle_schlange_verwirft() {
        let a = libp2p::PeerId::random();
        let mut u: Unterwegs<u64> = Unterwegs::default();
        u.beginnen(1, a, &[0], Instant::now());
        let stueck = WARTESCHLANGE_HOECHSTENS_BYTES / 4;
        for _ in 0..4 {
            assert_eq!(u.einreihen(a, vec![0; stueck], 1), Einreihen::Wartet);
        }
        assert_eq!(u.einreihen(a, vec![0], 1), Einreihen::Voll);
        u.beenden(&1);
        assert_eq!(u.naechste(&a, 1).map(|(d, z)| (d.len(), z)), Some((stueck, None)));
        assert_eq!(u.einreihen(a, vec![0; stueck], 1), Einreihen::Wartet);
        assert_eq!(u.wartend(&a), 4);
    }

    /// 📌 Fund 720 als Probe: Die Stichprobe am Deckel (5 000 Spuranfragen
    /// zu 45 Byte an ein Mitglied) passt ganz in die Schlange.
    #[test]
    fn die_stichprobe_am_deckel_passt_in_die_schlange() {
        let a = libp2p::PeerId::random();
        let mut u: Unterwegs<u64> = Unterwegs::default();
        let mut gesendet = 0;
        for k in 0..5_000u64 {
            match u.einreihen(a, vec![11; 45], 16) {
                Einreihen::Senden(d) => {
                    u.beginnen(k, a, &d, Instant::now());
                    gesendet += 1;
                }
                Einreihen::Wartet => {}
                Einreihen::Voll => panic!("Spuranfrage {k} verworfen"),
            }
        }
        assert_eq!(gesendet, 16);
        assert_eq!(u.wartend(&a), 5_000 - 16);
    }

    /// Eine Anfrage mit Vorrang überholt die wartenden ohne Vorrang: Sie
    /// geht sofort hinaus, wenn ein Platz frei ist, und sonst als erste, wenn
    /// einer frei wird. Gegenprobe: ohne Vorrang wartete sie hinter allen.
    #[test]
    fn vorrang_ueberholt_die_wartenden() {
        let a = libp2p::PeerId::random();
        let t0 = Instant::now();
        let mut u: Unterwegs<u64> = Unterwegs::default();
        u.beginnen(1, a, &[11], t0);
        u.beginnen(2, a, &[11], t0);
        for _ in 0..3 {
            assert_eq!(u.einreihen(a, vec![11], 2), Einreihen::Wartet);
        }
        // Ein Platz wird frei (Antwort auf 1), drei ohne Vorrang warten noch.
        u.beenden(&1);
        assert_eq!(u.einreihen(a, vec![12], 2), Einreihen::Wartet, "ohne Vorrang hinten an");
        assert_eq!(u.einreihen_vorrang(a, vec![9], 70, 2), Einreihen::Senden(vec![9]), "mit Vorrang sofort");
        u.beginnen(3, a, &[9], t0);
        assert_eq!(u.einreihen_vorrang(a, vec![9, 1], 71, 2), Einreihen::Wartet);
        u.beenden(&2);
        assert_eq!(u.naechste(&a, 2), Some((vec![9, 1], Some(71))), "der Vorrang zuerst, mit seinem Zweck");
        u.beginnen(4, a, &[9], t0);
        let bild = u.bild(&a, t0).expect("Bild");
        assert_eq!(bild.iter().map(|s| (s.art, s.anzahl, s.wartend)).collect::<Vec<_>>(), vec![(9, 2, 0), (11, 0, 3), (12, 0, 1)]);
        assert_eq!(u.naechste(&a, 2), None, "beide Plaetze belegt");
        for z in 0..VORRANG_HOECHSTENS as u64 {
            assert_eq!(u.einreihen_vorrang(a, vec![9], z, 0), Einreihen::Wartet);
        }
        assert_eq!(u.einreihen_vorrang(a, vec![9], 999, 0), Einreihen::Voll);
        let zurueck = u.vergessen(&a);
        assert_eq!(zurueck.len(), VORRANG_HOECHSTENS + 4, "dazu die vier ohne Vorrang");
        assert_eq!(zurueck[0], Some(0), "die mit Vorrang kommen mit ihrem Zweck zurueck");
        assert_eq!(zurueck.last(), Some(&None));
    }

    #[test]
    fn der_stau_wird_am_text_erkannt() {
        assert!(ist_stau("IO error on outbound stream: max sub-streams reached"));
        assert!(ist_stau(WARTESCHLANGE_VOLL));
        assert!(!ist_stau("Timeout while waiting for a response"));
    }
}
