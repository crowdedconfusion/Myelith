//! **Der Zustandsbaum**: ein binärer, dünn besetzter Merkle-Baum über
//! 256-Bit-Schlüssel (Skalierung, Festlegung des Projektinhabers vom
//! 2026-10-29).
//!
//! # ⚑ Warum
//!
//! Bis hierher hashte die Kette je Block den ganzen Zustand: bei einer
//! Million Konten 127 ms, länger als ein Blocktakt. Dieser Baum hasht je
//! Block nur die Pfade der geänderten Einträge (O(k · log n)), und er
//! liefert Beweise für einzelne Einträge, auch für ihr Fehlen. Darauf bauen
//! die nächsten Schritte: Beweise im Block, prüfende Knoten mit nur den
//! oberen Ebenen, ein verteilt gehaltener Zustand (Ziel bis rund eine
//! Milliarde Teilnehmer).
//!
//! # ⚑ Die Gestalt
//!
//! Nach dem Vorbild des Jellyfish Merkle Tree (Aptos) und von EIP-7864
//! (binär, ein Schlüsselraum), eigene Umsetzung:
//!
//! - Ein leerer Teilbaum hasht zu [`LEER`] (32 Null-Bytes).
//! - **Ein Teilbaum mit genau einem Eintrag ist dieser Eintrag**:
//!   `SHA-256(0x00 ‖ Schlüssel ‖ Werthash)`, gleich in welcher Tiefe.
//! - Ein Zweig mit mindestens zwei Einträgen: `SHA-256(0x01 ‖ links ‖ rechts)`,
//!   geteilt nach dem Bit seiner Tiefe (höchstes Bit zuerst).
//!
//! Damit hängt die Wurzel nur an der Menge der Einträge, nicht an der Folge,
//! in der sie kamen; zwei Knoten, die denselben Zustand auf verschiedenen
//! Wegen erreichen, haben dieselbe Wurzel.
//!
//! # ⚑ Faul gehasht
//!
//! Ein Zweig merkt sich seinen Hash; eine Änderung löscht ihn auf dem Pfad,
//! und erst [`Zustandsbaum::wurzel`] rechnet nach. Was ein Block mehrfach
//! berührt, wird so einmal gehasht.

//!
//! # ⚑ Ausgelagert auf die Platte (2026-10-30)
//!
//! Nach dem Vorbild von NOMT (Thrum: ein binärer Merkle-Baum in Seiten auf
//! der SSD), eigene Umsetzung: Ein voller Knoten kann den Baum
//! **auslagern** ([`Zustandsbaum::mit_ablage`]). Teilbäume unterhalb einer
//! Tiefe, die mit der Zahl der Einträge wächst (rund 128 Einträge je
//! Seite), liegen dann als Seiten in einer Datei; im Speicher steht nur der
//! obere Baum und an jeder ausgelagerten Stelle ein
//! `Knoten::Ausgelagert { seite, hash }`. Vor jedem Zugriff wird der Pfad
//! eingelagert, nach jeder Wurzel werden die berührten Teilbäume wieder
//! ausgelagert. **Gehasht wird wie zuvor**, also gleiche Wurzel, gleiche
//! Beweise, gleiche Zeugen, unabhängig davon, was gerade wo liegt.
//!
//! ⚑ **Die Datei ist ein Zwischenspeicher, kein zweiter Zustand**: Der
//! Zustand wird beim Start aus Abzug und Blöcken hergeleitet, der Baum aus
//! ihm. Deshalb braucht sie keine Absturzsicherheit; Seiten werden an Ort
//! und Stelle wiederverwendet (Grössenklassen), die Datei wächst nur mit
//! dem lebenden Baum.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

/// Der Hash eines leeren Teilbaums.
pub const LEER: [u8; 32] = [0u8; 32];

fn blatthash(schluessel: &[u8; 32], wert: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([0u8]);
    h.update(schluessel);
    h.update(wert);
    h.finalize().into()
}

fn zweighash(links: &[u8; 32], rechts: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([1u8]);
    h.update(links);
    h.update(rechts);
    h.finalize().into()
}

/// Das Bit der Tiefe `t` (0 ist das höchste Bit des ersten Bytes).
fn bit(schluessel: &[u8; 32], t: usize) -> bool {
    schluessel[t / 8] & (0x80 >> (t % 8)) != 0
}

#[derive(Debug, Clone)]
enum Knoten {
    Leer,
    Blatt { schluessel: [u8; 32], wert: [u8; 32] },
    Zweig { links: Box<Knoten>, rechts: Box<Knoten>, hash: Option<[u8; 32]> },
    /// Nur im [`Teilbaum`]: ein Zweig mit mindestens zwei Einträgen, von dem
    /// nur der Hash vorliegt. Ein Blatt wird nie gekürzt, damit beim
    /// Entfernen entschieden werden kann, ob ein Zweig zusammenfällt.
    Gekuerzt([u8; 32]),
    /// Nur im ausgelagerten [`Zustandsbaum`]: ein Zweig (mindestens zwei
    /// Einträge), der als Seite in der Ablage liegt. ⛔️ Nie in einem Zeugen
    /// (`Teilbaum::aus_bytes` liest die Marke nicht).
    Ausgelagert { seite: Seite, hash: [u8; 32] },
}

/// Wo eine Seite in der Ablage liegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seite {
    ort: u64,
    laenge: u32,
}

/// ⚑ **Höchstens so viele Knoten hat ein gelesener Teilbaum** (2026-10-29):
/// Ein ehrlicher Zeuge hat bei einer Milliarde Einträgen und 8000 berührten
/// rund eine halbe Million. Die Schranke bindet den Speicher an etwas
/// anderes als die Länge der Bytes; Zweige mit leerem Geschwister kosten
/// im Draht nur zwei Bytes je Ebene und bleiben kanonisch.
pub const TEILBAUM_KNOTEN_HOECHSTENS: usize = 1 << 21;

/// Nach so vielen berührten Pfaden lagert ein ausgelagerter Baum auch
/// zwischen zwei Wurzeln aus.
const ZWISCHENDURCH_AUSLAGERN: usize = 1 << 16;

/// Ein Zugriff führte in einen Teil des Baums, der im Zeugen nur als Hash
/// steht: Der Zeuge ist für diesen Block unvollständig.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ausserhalb;

impl Knoten {
    fn hash(&mut self) -> [u8; 32] {
        match self {
            Knoten::Leer => LEER,
            Knoten::Gekuerzt(h) | Knoten::Ausgelagert { hash: h, .. } => *h,
            Knoten::Blatt { schluessel, wert } => blatthash(schluessel, wert),
            Knoten::Zweig { links, rechts, hash } => {
                if let Some(h) = hash {
                    return *h;
                }
                let h = zweighash(&links.hash(), &rechts.hash());
                *hash = Some(h);
                h
            }
        }
    }

    /// Setzt `schluessel` auf `wert` im Teilbaum der Tiefe `t`.
    fn setzen(self, schluessel: [u8; 32], wert: [u8; 32], t: usize) -> Result<Knoten, Ausserhalb> {
        Ok(match self {
            Knoten::Leer => Knoten::Blatt { schluessel, wert },
            // Ausgelagertes wird vorher eingelagert; sonst ist es wie
            // Gekürztes ausserhalb.
            Knoten::Gekuerzt(_) | Knoten::Ausgelagert { .. } => return Err(Ausserhalb),
            Knoten::Blatt { schluessel: s, .. } if s == schluessel => Knoten::Blatt { schluessel, wert },
            Knoten::Blatt { schluessel: s, wert: w } => {
                // Zwei Einträge: teilen, bis sich ihre Bits unterscheiden.
                let alt = Knoten::Blatt { schluessel: s, wert: w };
                let neu = Knoten::Blatt { schluessel, wert };
                Self::teilen(alt, &s, neu, &schluessel, t)
            }
            Knoten::Zweig { links, rechts, .. } => {
                if bit(&schluessel, t) {
                    Knoten::Zweig { links, rechts: Box::new(rechts.setzen(schluessel, wert, t + 1)?), hash: None }
                } else {
                    Knoten::Zweig { links: Box::new(links.setzen(schluessel, wert, t + 1)?), rechts, hash: None }
                }
            }
        })
    }

    fn teilen(a: Knoten, ka: &[u8; 32], b: Knoten, kb: &[u8; 32], t: usize) -> Knoten {
        let (ba, bb) = (bit(ka, t), bit(kb, t));
        if ba == bb {
            let innen = Self::teilen(a, ka, b, kb, t + 1);
            if ba {
                Knoten::Zweig { links: Box::new(Knoten::Leer), rechts: Box::new(innen), hash: None }
            } else {
                Knoten::Zweig { links: Box::new(innen), rechts: Box::new(Knoten::Leer), hash: None }
            }
        } else if ba {
            Knoten::Zweig { links: Box::new(b), rechts: Box::new(a), hash: None }
        } else {
            Knoten::Zweig { links: Box::new(a), rechts: Box::new(b), hash: None }
        }
    }

    /// Entfernt `schluessel`; ein Zweig, der danach höchstens einen Eintrag
    /// trägt, wird zu diesem Eintrag (die kanonische Gestalt).
    fn entfernen(self, schluessel: &[u8; 32], t: usize) -> Result<Knoten, Ausserhalb> {
        Ok(match self {
            Knoten::Gekuerzt(_) | Knoten::Ausgelagert { .. } => return Err(Ausserhalb),
            Knoten::Blatt { schluessel: s, .. } if s == *schluessel => Knoten::Leer,
            Knoten::Zweig { links, rechts, .. } => {
                let (links, rechts) = if bit(schluessel, t) {
                    (*links, rechts.entfernen(schluessel, t + 1)?)
                } else {
                    (links.entfernen(schluessel, t + 1)?, *rechts)
                };
                // Bleibt ein gekürzter Knoten neben Leerem, ist nicht
                // belegt, dass er ein Zweig ist: Sein Hash könnte der eines
                // einzelnen Blatts sein, das hier zusammenfallen müsste. Der
                // Zeuge muss ihn dann eine Ebene weit öffnen (siehe
                // `kuerzen`); ein geöffneter Zweig ist über die getrennten
                // Hash-Domänen als Zweig belegt, hat also im kanonischen
                // vollen Baum mindestens zwei Einträge.
                match (&links, &rechts) {
                    (Knoten::Leer, Knoten::Gekuerzt(_)) | (Knoten::Gekuerzt(_), Knoten::Leer) => return Err(Ausserhalb),
                    (Knoten::Leer, Knoten::Leer) => Knoten::Leer,
                    (Knoten::Leer, Knoten::Blatt { .. }) => rechts,
                    (Knoten::Blatt { .. }, Knoten::Leer) => links,
                    _ => Knoten::Zweig { links: Box::new(links), rechts: Box::new(rechts), hash: None },
                }
            }
            andere => andere,
        })
    }

    /// Der Teilbaum über die Pfade von `schluessel` (beide Listen sortiert):
    /// was abseits liegt, nur als Hash (Zweige) oder ganz (Blätter, Leeres). Neben dem Pfad eines
    /// Schlüssels aus `entfernt` wird ein Zweig eine Ebene weit geöffnet,
    /// damit belegt ist, dass er nicht zusammenfällt.
    fn kuerzen(&mut self, schluessel: &[[u8; 32]], entfernt: &[[u8; 32]], t: usize) -> Knoten {
        match self {
            Knoten::Zweig { links, rechts, .. } => {
                if schluessel.is_empty() {
                    return Knoten::Gekuerzt(self.hash());
                }
                // Sortiert, also liegen die mit Bit 0 vorn.
                let (sl, sr) = schluessel.split_at(schluessel.partition_point(|k| !bit(k, t)));
                let (el, er) = entfernt.split_at(entfernt.partition_point(|k| !bit(k, t)));
                let seite = |k: &mut Knoten, s: &[[u8; 32]], e: &[[u8; 32]], gegenueber: &[[u8; 32]]| {
                    if s.is_empty() && !gegenueber.is_empty() {
                        k.oeffnen()
                    } else {
                        k.kuerzen(s, e, t + 1)
                    }
                };
                Knoten::Zweig {
                    links: Box::new(seite(links, sl, el, er)),
                    rechts: Box::new(seite(rechts, sr, er, el)),
                    hash: None,
                }
            }
            // Ein ausgelagerter Zweig ist im Zeugen gekürzt (ein Pfad in
            // ihn hinein ist vorher eingelagert).
            Knoten::Ausgelagert { hash, .. } => Knoten::Gekuerzt(*hash),
            andere => andere.clone(),
        }
    }

    /// Ein Zweig mit beiden Kindern gekürzt (Blätter und Leeres ganz).
    fn oeffnen(&mut self) -> Knoten {
        match self {
            Knoten::Zweig { links, rechts, .. } => Knoten::Zweig {
                links: Box::new(links.kuerzen(&[], &[], 0)),
                rechts: Box::new(rechts.kuerzen(&[], &[], 0)),
                hash: None,
            },
            Knoten::Ausgelagert { hash, .. } => Knoten::Gekuerzt(*hash),
            andere => andere.clone(),
        }
    }

    fn lesen(&self, schluessel: &[u8; 32]) -> Result<Option<[u8; 32]>, Ausserhalb> {
        let mut k = self;
        let mut t = 0;
        loop {
            match k {
                Knoten::Leer => return Ok(None),
                Knoten::Gekuerzt(_) | Knoten::Ausgelagert { .. } => return Err(Ausserhalb),
                Knoten::Blatt { schluessel: s, wert } => return Ok((s == schluessel).then_some(*wert)),
                Knoten::Zweig { links, rechts, .. } => {
                    k = if bit(schluessel, t) { rechts } else { links };
                    t += 1;
                }
            }
        }
    }

    fn kodieren(&self, aus: &mut Vec<u8>) {
        match self {
            Knoten::Leer => aus.push(0),
            Knoten::Blatt { schluessel, wert } => {
                aus.push(1);
                aus.extend_from_slice(schluessel);
                aus.extend_from_slice(wert);
            }
            Knoten::Zweig { links, rechts, .. } => {
                aus.push(2);
                links.kodieren(aus);
                rechts.kodieren(aus);
            }
            Knoten::Gekuerzt(h) => {
                aus.push(3);
                aus.extend_from_slice(h);
            }
            Knoten::Ausgelagert { seite, hash } => {
                aus.push(4);
                aus.extend_from_slice(&seite.ort.to_le_bytes());
                aus.extend_from_slice(&seite.laenge.to_le_bytes());
                aus.extend_from_slice(hash);
            }
        }
    }

    /// Wie [`Self::kodieren`], für eine Seite der eigenen Ablage: Zweige
    /// tragen ihren Hash mit (Marke 5), damit eine eingelagerte Seite nicht
    /// ganz neu gehasht wird. 📌 Ohne das kostete ein Block mit 2000
    /// Überweisungen bei einer Million Konten 272 statt 34 ms. Vorher
    /// gehasht (nach `hash()`).
    fn kodieren_seite(&self, aus: &mut Vec<u8>) {
        match self {
            Knoten::Zweig { links, rechts, hash } => {
                aus.push(5);
                aus.extend_from_slice(&hash.expect("vor dem Auslagern gehasht"));
                links.kodieren_seite(aus);
                rechts.kodieren_seite(aus);
            }
            andere => andere.kodieren(aus),
        }
    }

    /// `seiten`: aus der eigenen Ablage (dort dürfen `Ausgelagert` und
    /// Zweige mit Hash stehen); sonst ein Zeuge aus fremder Hand, dessen
    /// Hashes immer selbst gerechnet werden.
    fn dekodieren(ein: &mut &[u8], tiefe: usize, budget: &mut usize, seiten: bool) -> Option<Knoten> {
        if tiefe > 256 || *budget == 0 {
            return None;
        }
        *budget -= 1;
        let (&marke, rest) = ein.split_first()?;
        *ein = rest;
        let nimm = |ein: &mut &[u8]| -> Option<[u8; 32]> {
            let (a, rest) = ein.split_at_checked(32)?;
            *ein = rest;
            a.try_into().ok()
        };
        Some(match marke {
            0 => Knoten::Leer,
            1 => Knoten::Blatt { schluessel: nimm(ein)?, wert: nimm(ein)? },
            2 => {
                let links = Self::dekodieren(ein, tiefe + 1, budget, seiten)?;
                let rechts = Self::dekodieren(ein, tiefe + 1, budget, seiten)?;
                // ⚑ **Nur Kanonisches** (2026-10-29): Ein Zweig über
                //   höchstens einem Eintrag kommt im Baum nicht vor. Ohne
                //   diese Prüfung kostete ein solcher Knoten im Draht zwei
                //   Bytes und im Speicher ein Vielfaches, bevor die Wurzel
                //   ihn verwerfen könnte.
                if matches!((&links, &rechts), (Knoten::Leer, Knoten::Leer) | (Knoten::Leer, Knoten::Blatt { .. }) | (Knoten::Blatt { .. }, Knoten::Leer)) {
                    return None;
                }
                Knoten::Zweig { links: Box::new(links), rechts: Box::new(rechts), hash: None }
            }
            3 => {
                let h = nimm(ein)?;
                // Der Hash des Leeren ist kein Zweig.
                if h == LEER {
                    return None;
                }
                Knoten::Gekuerzt(h)
            }
            // ⛔️ Nur aus der eigenen Ablage: ein Zweig mit seinem Hash.
            5 if seiten => {
                let h = nimm(ein)?;
                let links = Self::dekodieren(ein, tiefe + 1, budget, seiten)?;
                let rechts = Self::dekodieren(ein, tiefe + 1, budget, seiten)?;
                Knoten::Zweig { links: Box::new(links), rechts: Box::new(rechts), hash: Some(h) }
            }
            // ⛔️ Nur aus der eigenen Ablage.
            4 if seiten => {
                let (o, rest) = ein.split_at_checked(8)?;
                let (l, rest) = rest.split_at_checked(4)?;
                *ein = rest;
                let seite = Seite { ort: u64::from_le_bytes(o.try_into().ok()?), laenge: u32::from_le_bytes(l.try_into().ok()?) };
                Knoten::Ausgelagert { seite, hash: nimm(ein)? }
            }
            _ => return None,
        })
    }
}

/// **Ein Beweis** für einen Schlüssel: die Geschwister-Hashes von der Wurzel
/// abwärts bis zum Ende des Pfads, und was dort steht.
#[derive(Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct Baumbeweis {
    /// Die Geschwister von oben nach unten.
    pub geschwister: Vec<[u8; 32]>,
    /// Am Ende des Pfads: nichts, oder ein Blatt (Schlüssel, Werthash). Ein
    /// Blatt mit anderem Schlüssel beweist das Fehlen.
    pub ende: Option<([u8; 32], [u8; 32])>,
}

impl Baumbeweis {
    /// Prüft gegen `wurzel`: `Some(wert)` heisst, der Schlüssel trägt diesen
    /// Werthash; `None` heisst, der Schlüssel fehlt.
    pub fn pruefen(&self, wurzel: &[u8; 32], schluessel: &[u8; 32], wert: Option<&[u8; 32]>) -> bool {
        if self.geschwister.len() > 256 {
            return false;
        }
        match (&self.ende, wert) {
            (Some((s, w)), Some(soll)) if s == schluessel && w == soll => {}
            (Some((s, _)), None) if s != schluessel => {
                // Ein anderes Blatt am Ende muss denselben Pfad haben.
                if (0..self.geschwister.len()).any(|t| bit(s, t) != bit(schluessel, t)) {
                    return false;
                }
            }
            (None, None) => {}
            _ => return false,
        }
        let mut h = match &self.ende {
            Some((s, w)) => blatthash(s, w),
            None => LEER,
        };
        for (t, g) in self.geschwister.iter().enumerate().rev() {
            h = if bit(schluessel, t) { zweighash(g, &h) } else { zweighash(&h, g) };
        }
        h == *wurzel
    }
}

/// **Der Zustandsbaum** (siehe Modulkopf).
#[derive(Debug)]
pub struct Zustandsbaum {
    wurzel: Knoten,
    eintraege: usize,
    /// Ausgelagert: die Ablage, die eingelagerten Seiten je Hash (zum
    /// Wiederverwenden oder Freigeben), die seit dem letzten Auslagern
    /// berührten Pfade, und ob alles neu auszulagern ist.
    ablage: Option<Box<Seitenablage>>,
    herkunft: HashMap<[u8; 32], Seite>,
    /// Pfade, auf denen sich etwas geändert haben kann (bis zur
    /// Auslagerungstiefe abzugehen).
    beruehrt: Vec<[u8; 32]>,
    /// Eingelagerte Knoten: ein Schlüssel, dessen Pfad zu ihnen führt, und
    /// ihre Tiefe; ihr Teilbaum (eine Seite) wird ganz wieder ausgelagert.
    geladen: Vec<([u8; 32], usize)>,
    alles_auslagern: bool,
}

impl Default for Zustandsbaum {
    fn default() -> Self {
        Self {
            wurzel: Knoten::Leer,
            eintraege: 0,
            ablage: None,
            herkunft: HashMap::new(),
            beruehrt: Vec::new(),
            geladen: Vec::new(),
            alles_auslagern: false,
        }
    }
}

/// Eine Kopie liegt ganz im Speicher: Die Seiten gehören der Ablage des
/// Originals, das sie überschreibt.
impl Clone for Zustandsbaum {
    fn clone(&self) -> Self {
        let mut wurzel = self.wurzel.clone();
        if let Some(a) = &self.ablage {
            a.alles_einlagern(&mut wurzel);
        }
        Self { wurzel, eintraege: self.eintraege, ..Default::default() }
    }
}

/// ⚑ **Die Ablage der Seiten**: eine Datei, Seiten in Grössenklassen
/// (Zweierpotenzen ab 256 Bytes), freie Plätze je Klasse wiederverwendet.
/// Ein Zwischenspeicher (siehe Modulkopf): Ein Fehler beim Lesen oder
/// Schreiben hält den Knoten an, er baut beim Neustart neu.
#[derive(Debug)]
pub struct Seitenablage {
    datei: std::fs::File,
    frei: BTreeMap<u32, Vec<u64>>,
    ende: u64,
    /// Belegte Bytes (in Klassen), für Angaben.
    belegt: u64,
}

impl Seitenablage {
    /// Eine neue, leere Ablage (eine vorhandene Datei wird geleert).
    pub fn neu(pfad: &std::path::Path) -> std::io::Result<Self> {
        let datei = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(true).open(pfad)?;
        Ok(Self { datei, frei: BTreeMap::new(), ende: 0, belegt: 0 })
    }

    fn klasse(n: usize) -> u32 {
        (n.max(256).next_power_of_two()) as u32
    }

    fn schreiben(&mut self, bytes: &[u8]) -> Seite {
        use std::os::unix::fs::FileExt;
        let k = Self::klasse(bytes.len());
        let ort = match self.frei.get_mut(&k).and_then(Vec::pop) {
            Some(o) => o,
            None => {
                let o = self.ende;
                self.ende += u64::from(k);
                o
            }
        };
        self.datei.write_all_at(bytes, ort).expect("die Seitenablage des Zustandsbaums ist nicht beschreibbar");
        self.belegt += u64::from(k);
        Seite { ort, laenge: bytes.len() as u32 }
    }

    fn lesen(&self, s: Seite) -> Knoten {
        use std::os::unix::fs::FileExt;
        let mut b = vec![0u8; s.laenge as usize];
        self.datei.read_exact_at(&mut b, s.ort).expect("die Seitenablage des Zustandsbaums ist nicht lesbar");
        let mut ein = &b[..];
        let mut budget = usize::MAX;
        Knoten::dekodieren(&mut ein, 0, &mut budget, true).expect("eine Seite der eigenen Ablage ist kaputt")
    }

    fn freigeben(&mut self, s: Seite) {
        let k = Self::klasse(s.laenge as usize);
        self.frei.entry(k).or_default().push(s.ort);
        self.belegt -= u64::from(k);
    }

    /// Lagert alles unter `k` ein (für eine Kopie).
    fn alles_einlagern(&self, k: &mut Knoten) {
        if let Knoten::Ausgelagert { seite, hash } = k {
            let h = *hash;
            *k = self.lesen(*seite);
            if let Knoten::Zweig { hash, .. } = k {
                *hash = Some(h);
            }
        }
        if let Knoten::Zweig { links, rechts, .. } = k {
            self.alles_einlagern(links);
            self.alles_einlagern(rechts);
        }
    }

    /// Belegte Bytes der Ablage (in Grössenklassen).
    pub fn belegt(&self) -> u64 {
        self.belegt
    }
}

impl Zustandsbaum {
    /// Wie viele Einträge.
    pub fn len(&self) -> usize {
        self.eintraege
    }

    /// Ob leer.
    pub fn is_empty(&self) -> bool {
        self.eintraege == 0
    }

    /// ⚑ **Ein Baum, der auslagert** (siehe Modulkopf), mit einer neuen
    /// Ablage unter `pfad`.
    pub fn mit_ablage(pfad: &std::path::Path) -> std::io::Result<Self> {
        Ok(Self { ablage: Some(Box::new(Seitenablage::neu(pfad)?)), alles_auslagern: true, ..Default::default() })
    }

    /// Wie viele Knoten im Speicher stehen (für Angaben und Tests).
    pub fn knoten_im_speicher(&self) -> usize {
        fn zaehlen(k: &Knoten) -> usize {
            match k {
                Knoten::Zweig { links, rechts, .. } => 1 + zaehlen(links) + zaehlen(rechts),
                _ => 1,
            }
        }
        zaehlen(&self.wurzel)
    }

    /// Die Ablage, wenn ausgelagert wird.
    pub fn ablage(&self) -> Option<&Seitenablage> {
        self.ablage.as_deref()
    }

    /// Lagert den Pfad von `schluessel` ein; mit `geschwister` auch das
    /// Geschwister auf jeder Ebene (für den Zeugen eines entfernten
    /// Schlüssels, der es eine Ebene weit öffnet).
    fn einlagern_pfad(&mut self, schluessel: &[u8; 32], geschwister: bool) {
        let Some(ablage) = self.ablage.as_deref() else {
            return;
        };
        self.beruehrt.push(*schluessel);
        let laden = |k: &mut Knoten, herkunft: &mut HashMap<[u8; 32], Seite>| -> bool {
            if let Knoten::Ausgelagert { seite, hash } = *k {
                *k = ablage.lesen(seite);
                if let Knoten::Zweig { hash: h, .. } = k {
                    *h = Some(hash);
                }
                herkunft.insert(hash, seite);
                return true;
            }
            false
        };
        let mut k = &mut self.wurzel;
        let mut t = 0;
        loop {
            if laden(k, &mut self.herkunft) {
                self.geladen.push((*schluessel, t));
            }
            match k {
                Knoten::Zweig { links, rechts, .. } => {
                    let (weiter, daneben) = if bit(schluessel, t) { (rechts, links) } else { (links, rechts) };
                    if geschwister && laden(daneben, &mut self.herkunft) {
                        // Ein Schlüssel, dessen Pfad zum Geschwister führt.
                        let mut zu = *schluessel;
                        zu[t / 8] ^= 0x80 >> (t % 8);
                        self.geladen.push((zu, t + 1));
                    }
                    k = weiter;
                    t += 1;
                }
                _ => return,
            }
        }
    }

    /// ⚑ **Lagert die berührten Teilbäume aus** (nach der Wurzel): unterhalb
    /// der Tiefe, ab der ein Teilbaum rund 128 Einträge hat. Eine Seite, deren
    /// Teilbaum unverändert ist (gleicher Hash), wird wiederverwendet; die
    /// übrigen eingelagerten werden frei.
    fn auslagern(&mut self) {
        let Some(mut ablage) = self.ablage.take() else {
            return;
        };
        let tiefe = (usize::BITS - self.eintraege.max(1).leading_zeros()).saturating_sub(7).max(6) as usize;
        let mut herkunft = std::mem::take(&mut self.herkunft);
        if std::mem::take(&mut self.alles_auslagern) {
            Self::teilbaum_auslagern(&mut self.wurzel, 0, tiefe, &mut ablage, &mut herkunft);
        } else {
            // Eingelagerte Seiten ganz (je eine Seite gross), berührte Pfade
            // bis zur Auslagerungstiefe.
            for (schluessel, ab) in std::mem::take(&mut self.geladen) {
                Self::pfad_auslagern(&mut self.wurzel, &schluessel, ab, tiefe, &mut ablage, &mut herkunft);
            }
            for schluessel in std::mem::take(&mut self.beruehrt) {
                Self::pfad_auslagern(&mut self.wurzel, &schluessel, tiefe, tiefe, &mut ablage, &mut herkunft);
            }
        }
        self.beruehrt.clear();
        self.geladen.clear();
        for (_, s) in herkunft {
            ablage.freigeben(s);
        }
        self.ablage = Some(ablage);
    }

    /// Geht dem Pfad von `schluessel` bis zur Tiefe `ab` nach (höchstens bis
    /// zur Auslagerungstiefe) und lagert den Teilbaum dort aus.
    fn pfad_auslagern(
        wurzel: &mut Knoten,
        schluessel: &[u8; 32],
        ab: usize,
        tiefe: usize,
        ablage: &mut Seitenablage,
        herkunft: &mut HashMap<[u8; 32], Seite>,
    ) {
        let mut k = wurzel;
        let mut t = 0;
        while t < ab.min(tiefe) {
            match k {
                Knoten::Zweig { links, rechts, .. } => {
                    k = if bit(schluessel, t) { rechts } else { links };
                    t += 1;
                }
                _ => return,
            }
        }
        Self::teilbaum_auslagern(k, t, tiefe, ablage, herkunft);
    }

    /// Lagert unter `k` (in Tiefe `t`) alles aus, was in der Auslagerungstiefe
    /// als Zweig im Speicher steht.
    fn teilbaum_auslagern(k: &mut Knoten, t: usize, tiefe: usize, ablage: &mut Seitenablage, herkunft: &mut HashMap<[u8; 32], Seite>) {
        match k {
            Knoten::Zweig { links, rechts, .. } if t < tiefe => {
                Self::teilbaum_auslagern(links, t + 1, tiefe, ablage, herkunft);
                Self::teilbaum_auslagern(rechts, t + 1, tiefe, ablage, herkunft);
            }
            Knoten::Zweig { .. } => {
                let hash = k.hash();
                let seite = match herkunft.remove(&hash) {
                    Some(s) => s,
                    None => {
                        let mut b = Vec::new();
                        k.kodieren_seite(&mut b);
                        ablage.schreiben(&b)
                    }
                };
                *k = Knoten::Ausgelagert { seite, hash };
            }
            _ => {}
        }
    }

    /// Der Werthash zu `schluessel`, wenn es ihn gibt.
    pub fn lesen(&mut self, schluessel: &[u8; 32]) -> Option<[u8; 32]> {
        self.einlagern_pfad(schluessel, false);
        // Im vollen Baum ist nichts gekürzt.
        self.wurzel.lesen(schluessel).ok().flatten()
    }

    /// Setzt oder entfernt (`None`) einen Eintrag.
    pub fn setzen(&mut self, schluessel: [u8; 32], wert: Option<[u8; 32]>) {
        // ⚑ Ausgelagert auch zwischendurch: Sonst stünde beim ersten Bau
        //   (alle Einträge auf einmal) der ganze Baum im Speicher, bevor
        //   die erste Wurzel ihn auslagert.
        if self.ablage.is_some() && self.beruehrt.len() >= ZWISCHENDURCH_AUSLAGERN {
            self.wurzel();
        }
        let vorher = self.lesen(&schluessel).is_some();
        let alt = std::mem::replace(&mut self.wurzel, Knoten::Leer);
        // Im vollen Baum gibt es nichts Gekürztes; ein Fehler ist hier
        // unmöglich, und der alte Stand bliebe dann ohnehin nicht stehen.
        self.wurzel = match wert {
            Some(w) => {
                self.eintraege += usize::from(!vorher);
                alt.setzen(schluessel, w, 0).unwrap_or(Knoten::Leer)
            }
            None => {
                self.eintraege -= usize::from(vorher);
                alt.entfernen(&schluessel, 0).unwrap_or(Knoten::Leer)
            }
        };
    }

    /// Die Wurzel; hasht, was seit dem letzten Mal geändert wurde, und lagert
    /// danach die berührten Teilbäume aus.
    pub fn wurzel(&mut self) -> [u8; 32] {
        let h = self.wurzel.hash();
        self.auslagern();
        h
    }

    /// Die Wurzel, wenn sie schon gerechnet ist (nach [`Self::wurzel`]),
    /// ohne etwas zu ändern oder einzulagern.
    pub fn wurzel_gerechnet(&self) -> Option<[u8; 32]> {
        match &self.wurzel {
            Knoten::Leer => Some(LEER),
            Knoten::Blatt { schluessel, wert } => Some(blatthash(schluessel, wert)),
            Knoten::Zweig { hash, .. } => *hash,
            Knoten::Gekuerzt(h) | Knoten::Ausgelagert { hash: h, .. } => Some(*h),
        }
    }

    /// Ein Beweis für `schluessel` (vorhanden oder fehlend).
    pub fn beweis(&mut self, schluessel: &[u8; 32]) -> Baumbeweis {
        self.einlagern_pfad(schluessel, false);
        self.wurzel.hash();
        let mut geschwister = Vec::new();
        let mut k = &mut self.wurzel;
        let mut t = 0;
        loop {
            match k {
                // Der Pfad ist eingelagert; ein Ausgelagertes hier ergäbe
                // einen Beweis, der nicht trägt, keinen falschen.
                Knoten::Leer | Knoten::Gekuerzt(_) | Knoten::Ausgelagert { .. } => return Baumbeweis { geschwister, ende: None },
                Knoten::Blatt { schluessel: s, wert } => return Baumbeweis { geschwister, ende: Some((*s, *wert)) },
                Knoten::Zweig { links, rechts, .. } => {
                    if bit(schluessel, t) {
                        geschwister.push(links.hash());
                        k = rechts;
                    } else {
                        geschwister.push(rechts.hash());
                        k = links;
                    }
                    t += 1;
                }
            }
        }
    }

    /// ⚑ **Der Zeuge für `schluessel`** (zustandslose Validierung,
    /// 2026-10-29): ein [`Teilbaum`] über ihre Pfade, mit derselben Wurzel.
    /// Gemeinsame obere Pfade stehen nur einmal darin. `entfernt` sind die
    /// darunter, die der Block entfernt (vorher da, nachher nicht).
    pub fn teilbaum(&mut self, schluessel: &[[u8; 32]], entfernt: &[[u8; 32]]) -> Teilbaum {
        for k in schluessel {
            self.einlagern_pfad(k, entfernt.contains(k));
        }
        self.wurzel.hash();
        let geordnet = |m: &[[u8; 32]]| {
            let mut m = m.to_vec();
            m.sort_unstable();
            m.dedup();
            m
        };
        Teilbaum { wurzel: self.wurzel.kuerzen(&geordnet(schluessel), &geordnet(entfernt), 0) }
    }

    /// Die Wurzel einer Menge von Einträgen, ohne Baum: die Gegenprobe.
    pub fn wurzel_von(eintraege: &[([u8; 32], [u8; 32])]) -> [u8; 32] {
        fn rek(e: &[([u8; 32], [u8; 32])], t: usize) -> [u8; 32] {
            match e {
                [] => LEER,
                [(s, w)] => blatthash(s, w),
                _ => {
                    let (l, r): (Vec<_>, Vec<_>) = e.iter().partition(|(s, _)| !bit(s, t));
                    zweighash(&rek(&l, t + 1), &rek(&r, t + 1))
                }
            }
        }
        let mut v = eintraege.to_vec();
        v.sort();
        v.dedup_by(|a, b| a.0 == b.0);
        rek(&v, 0)
    }
}

/// ⚑ **Ein Teilbaum** (der Zeuge der zustandslosen Validierung): die Pfade
/// einiger Schlüssel, alles andere als Hash. Er hat dieselbe Wurzel wie der
/// volle Baum; lesen und ändern lässt sich darin alles auf diesen Pfaden,
/// und die neue Wurzel ist die des vollen Baums nach denselben Änderungen.
/// Ein Zugriff daneben ist [`Ausserhalb`].
///
/// ⚠️ Ein Zeuge aus fremder Hand belegt nur etwas, wenn seine Wurzel die
/// bekannte Wurzel vor dem Block ist; das prüft, wer ihn annimmt.
#[derive(Debug, Clone)]
pub struct Teilbaum {
    wurzel: Knoten,
}

impl Teilbaum {
    /// Die Wurzel.
    pub fn wurzel(&mut self) -> [u8; 32] {
        self.wurzel.hash()
    }

    /// Der Werthash zu `schluessel` (`None`: es gibt ihn nicht).
    pub fn lesen(&self, schluessel: &[u8; 32]) -> Result<Option<[u8; 32]>, Ausserhalb> {
        self.wurzel.lesen(schluessel)
    }

    /// Setzt oder entfernt (`None`) einen Eintrag auf einem Pfad des Zeugen.
    /// Das Entfernen eines Eintrags, den es nicht gibt, ändert nichts.
    pub fn setzen(&mut self, schluessel: [u8; 32], wert: Option<[u8; 32]>) -> Result<(), Ausserhalb> {
        if wert.is_none() && self.lesen(&schluessel)?.is_none() {
            return Ok(());
        }
        // Erst entscheiden, dann ändern: Ein Fehlschlag mitten im Ändern
        // liesse einen halben Baum zurück, und eine Sicherungskopie je
        // Änderung kostete bei 4000 Änderungen Sekunden (gemessen).
        self.traegt(&schluessel, wert.is_none())?;
        let alt = std::mem::replace(&mut self.wurzel, Knoten::Leer);
        let neu = match wert {
            Some(w) => alt.setzen(schluessel, w, 0),
            None => alt.entfernen(&schluessel, 0),
        };
        self.wurzel = neu.unwrap_or_else(|_| {
            debug_assert!(false, "die Vorprüfung ließ einen Zugriff ausserhalb durch");
            Knoten::Leer
        });
        Ok(())
    }

    /// Ob `setzen` auf dem Pfad von `schluessel` im Zeugen bleibt, ohne
    /// etwas zu ändern, in O(Tiefe): Der Pfad endet nicht an Gekürztem,
    /// und beim Entfernen bleibt beim Zusammenfallen von unten nie
    /// Gekürztes neben Leerem stehen (siehe `Knoten::entfernen`).
    fn traegt(&self, schluessel: &[u8; 32], entfernen: bool) -> Result<(), Ausserhalb> {
        let mut neben: Vec<&Knoten> = Vec::new();
        let mut k = &self.wurzel;
        let mut t = 0;
        loop {
            match k {
                Knoten::Gekuerzt(_) | Knoten::Ausgelagert { .. } => return Err(Ausserhalb),
                Knoten::Leer => return Ok(()),
                Knoten::Blatt { schluessel: s, .. } => {
                    if !entfernen || s != schluessel {
                        return Ok(());
                    }
                    break;
                }
                Knoten::Zweig { links, rechts, .. } => {
                    let (weiter, daneben) = if bit(schluessel, t) { (rechts, links) } else { (links, rechts) };
                    neben.push(daneben);
                    k = weiter;
                    t += 1;
                }
            }
        }
        // Was unten übrig bleibt: 0 leer, 1 ein Blatt, 2 ein Zweig (dann
        // fällt nichts mehr zusammen).
        let mut rest = 0u8;
        for daneben in neben.iter().rev() {
            rest = match (rest, daneben) {
                (2, _) => break,
                (0, Knoten::Gekuerzt(_)) => return Err(Ausserhalb),
                (0, Knoten::Leer) => 0,
                (0, Knoten::Blatt { .. }) | (1, Knoten::Leer) => 1,
                _ => 2,
            };
        }
        Ok(())
    }

    /// Die Bytes des Zeugen.
    pub fn als_bytes(&self) -> Vec<u8> {
        let mut aus = Vec::new();
        self.wurzel.kodieren(&mut aus);
        aus
    }

    /// Liest einen Zeugen; `None`, wenn die Bytes keiner sind (auch mit
    /// Rest dahinter, tiefer als 256, unkanonisch oder mit mehr als
    /// [`TEILBAUM_KNOTEN_HOECHSTENS`] Knoten).
    pub fn aus_bytes(mut ein: &[u8]) -> Option<Self> {
        let mut budget = TEILBAUM_KNOTEN_HOECHSTENS;
        let wurzel = Knoten::dekodieren(&mut ein, 0, &mut budget, false)?;
        ein.is_empty().then_some(Self { wurzel })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(i: u64) -> [u8; 32] {
        let h: [u8; 32] = Sha256::digest(i.to_le_bytes()).into();
        h
    }

    #[test]
    fn leer_und_ein_eintrag() {
        let mut b = Zustandsbaum::default();
        assert_eq!(b.wurzel(), LEER);
        b.setzen(k(1), Some(k(100)));
        assert_eq!(b.wurzel(), blatthash(&k(1), &k(100)));
        b.setzen(k(1), None);
        assert_eq!(b.wurzel(), LEER);
        assert!(b.is_empty());
    }

    /// ⚑ **Die Wurzel hängt nur an der Menge**: zwei Bäume, dieselben
    /// Einträge in verschiedener Folge, mit Änderungen und Entfernungen
    /// unterwegs, haben dieselbe Wurzel wie die Gegenprobe ohne Baum.
    #[test]
    fn die_wurzel_haengt_nur_an_der_menge() {
        let mut a = Zustandsbaum::default();
        let mut b = Zustandsbaum::default();
        for i in 0..500 {
            a.setzen(k(i), Some(k(i + 1000)));
        }
        for i in (0..700).rev() {
            b.setzen(k(i), Some(k(7)));
        }
        for i in 500..700 {
            b.setzen(k(i), None);
        }
        for i in 0..500 {
            b.setzen(k(i), Some(k(i + 1000)));
        }
        assert_eq!(a.len(), 500);
        assert_eq!(b.len(), 500);
        let erwartet = Zustandsbaum::wurzel_von(&(0..500).map(|i| (k(i), k(i + 1000))).collect::<Vec<_>>());
        assert_eq!(a.wurzel(), erwartet);
        assert_eq!(b.wurzel(), erwartet);
        // Faul gehasht: eine Änderung nach der Wurzel zählt.
        a.setzen(k(3), Some(k(3)));
        assert_ne!(a.wurzel(), erwartet);
        a.setzen(k(3), Some(k(1003)));
        assert_eq!(a.wurzel(), erwartet);
    }

    /// ⚑ **Beweise**: für jeden vorhandenen Eintrag mit seinem Wert, für
    /// fehlende das Fehlen; ein falscher Wert, ein falsches Fehlen und eine
    /// fremde Wurzel tragen nicht.
    #[test]
    fn beweise_fuer_vorhandene_und_fehlende() {
        let mut b = Zustandsbaum::default();
        for i in 0..300 {
            b.setzen(k(i), Some(k(i + 1)));
        }
        let w = b.wurzel();
        for i in (0..300).step_by(7) {
            let p = b.beweis(&k(i));
            assert!(p.pruefen(&w, &k(i), Some(&k(i + 1))), "vorhanden {i}");
            assert!(!p.pruefen(&w, &k(i), Some(&k(i + 2))), "falscher Wert {i}");
            assert!(!p.pruefen(&w, &k(i), None), "falsches Fehlen {i}");
            assert!(!p.pruefen(&k(9), &k(i), Some(&k(i + 1))), "fremde Wurzel {i}");
        }
        for i in 300..340 {
            let p = b.beweis(&k(i));
            assert!(p.pruefen(&w, &k(i), None), "fehlend {i}");
            assert!(!p.pruefen(&w, &k(i), Some(&k(1))), "erfundener Wert {i}");
        }
        // Die Pfadlänge bleibt logarithmisch.
        let tiefste = (0..300).map(|i| b.beweis(&k(i)).geschwister.len()).max().unwrap_or(0);
        assert!(tiefste < 30, "Tiefe {tiefste} bei 300 Einträgen");
    }

    /// ⚑ **Der Zeuge trägt wie der volle Baum** (zustandslose Validierung,
    /// 2026-10-29): Ein Teilbaum über einige Schlüssel hat dieselbe Wurzel;
    /// ändert man darin dieselben Einträge wie im vollen Baum (setzen,
    /// neu anlegen, entfernen bis zum Zusammenfallen), kommt dieselbe neue
    /// Wurzel heraus; nach dem Weg über die Bytes ebenso. Ein Zugriff
    /// abseits der Pfade ist `Ausserhalb`.
    #[test]
    fn der_zeuge_traegt_wie_der_volle_baum() {
        let mut voll = Zustandsbaum::default();
        for i in 0..400 {
            voll.setzen(k(i), Some(k(i + 9)));
        }
        let beruehrt: Vec<[u8; 32]> = [3u64, 50, 51, 399, 1000, 1001].iter().map(|i| k(*i)).collect();
        let entfernt = [k(50), k(51), k(399)];
        let mut zeuge = voll.teilbaum(&beruehrt, &entfernt);
        assert_eq!(zeuge.wurzel(), voll.wurzel());
        let bytes = zeuge.als_bytes();
        let mut zeuge = Teilbaum::aus_bytes(&bytes).expect("lesbar");
        assert_eq!(zeuge.wurzel(), voll.wurzel(), "auch nach den Bytes");
        assert!(bytes.len() < 400 * 65, "kleiner als alles: {} Bytes", bytes.len());
        assert_eq!(zeuge.lesen(&k(50)), Ok(Some(k(59))));
        assert_eq!(zeuge.lesen(&k(1000)), Ok(None), "das Fehlen ist belegt");
        // Dieselben Änderungen hier und dort.
        let aenderungen: Vec<([u8; 32], Option<[u8; 32]>)> =
            vec![(k(3), Some(k(77))), (k(50), None), (k(51), None), (k(1000), Some(k(1))), (k(1001), Some(k(2))), (k(399), None)];
        for (s, w) in &aenderungen {
            voll.setzen(*s, *w);
            zeuge.setzen(*s, *w).expect("auf einem Pfad");
        }
        assert_eq!(zeuge.wurzel(), voll.wurzel(), "dieselbe neue Wurzel");
        // Daneben: weder lesen noch setzen.
        let daneben = (0..400).map(k).find(|s| zeuge.lesen(s).is_err()).expect("ein Schlüssel abseits");
        assert_eq!(zeuge.setzen(daneben, Some(k(5))), Err(Ausserhalb));
        assert_eq!(zeuge.wurzel(), voll.wurzel(), "ein abgewiesener Zugriff ändert nichts");
        assert!(Teilbaum::aus_bytes(&bytes[..bytes.len() - 1]).is_none());
    }

    /// ⚑ **Ein gekürztes Blatt fällt nicht durch** (zustandslose
    /// Validierung, 2026-10-29): Der Hash eines einzelnen Blatts, als
    /// gekürzter Zweig ausgegeben, gibt dieselbe Vorwurzel. Entfernt der
    /// Block das Geschwister, müsste das Blatt im vollen Baum aufrücken;
    /// der Zeuge lehnt ab, statt eine falsche Wurzel zu rechnen. Ein
    /// ehrlicher Zeuge (mit `entfernt`) trägt dagegen.
    #[test]
    fn ein_als_zweig_getarntes_blatt_faellt_auf() {
        // Zwei Schlüssel, die sich im ersten Bit unterscheiden.
        let a = (0..).map(k).find(|s| !bit(s, 0)).unwrap();
        let b = (0..).map(k).find(|s| bit(s, 0)).unwrap();
        let mut voll = Zustandsbaum::default();
        voll.setzen(a, Some(k(1)));
        voll.setzen(b, Some(k(2)));
        let vorher = voll.wurzel();
        let getarnt = Teilbaum {
            wurzel: Knoten::Zweig {
                links: Box::new(Knoten::Blatt { schluessel: a, wert: k(1) }),
                rechts: Box::new(Knoten::Gekuerzt(blatthash(&b, &k(2)))),
                hash: None,
            },
        };
        let mut getarnt = Teilbaum::aus_bytes(&getarnt.als_bytes()).unwrap();
        assert_eq!(getarnt.wurzel(), vorher, "die Tarnung hält der Vorwurzel stand");
        assert_eq!(getarnt.setzen(a, None), Err(Ausserhalb));
        let mut ehrlich = voll.teilbaum(&[a], &[a]);
        ehrlich.setzen(a, None).expect("geöffnet");
        voll.setzen(a, None);
        assert_eq!(ehrlich.wurzel(), voll.wurzel());
        assert_eq!(voll.wurzel(), blatthash(&b, &k(2)), "das Blatt ist aufgerückt");
    }

    /// Viele Runden aus Setzen, Neuanlegen und Entfernen (auch ganzer
    /// Gruppen, bis Zweige zusammenfallen): Der Zeuge rechnet jedes Mal
    /// dieselbe Wurzel wie der volle Baum.
    #[test]
    fn der_zeuge_haelt_auch_gemischten_runden_stand() {
        let mut voll = Zustandsbaum::default();
        for i in 0..300 {
            voll.setzen(k(i), Some(k(i)));
        }
        for runde in 0..40u64 {
            // Bestimmt gemischt: Hash aus Runde und Platz.
            let wahl = |j: u64| u64::from_le_bytes(k(runde * 1000 + j)[..8].try_into().unwrap());
            let mut aenderungen: Vec<([u8; 32], Option<[u8; 32]>)> = Vec::new();
            for j in 0..12 {
                let s = k(wahl(j) % 400);
                let w = if wahl(j + 100) % 3 == 0 { None } else { Some(k(wahl(j + 200))) };
                if !aenderungen.iter().any(|(a, _)| *a == s) {
                    aenderungen.push((s, w));
                }
            }
            let schluessel: Vec<[u8; 32]> = aenderungen.iter().map(|(s, _)| *s).collect();
            let entfernt: Vec<[u8; 32]> =
                aenderungen.iter().filter(|(s, w)| w.is_none() && voll.lesen(s).is_some()).map(|(s, _)| *s).collect();
            let mut zeuge = Teilbaum::aus_bytes(&voll.teilbaum(&schluessel, &entfernt).als_bytes()).unwrap();
            assert_eq!(zeuge.wurzel(), voll.wurzel());
            for (s, w) in &aenderungen {
                assert_eq!(zeuge.lesen(s), Ok(voll.lesen(s)));
                voll.setzen(*s, *w);
                zeuge.setzen(*s, *w).unwrap_or_else(|_| panic!("Runde {runde}: abgewiesen"));
            }
            assert_eq!(zeuge.wurzel(), voll.wurzel(), "Runde {runde}");
        }
    }

    /// ⚑ **Unkanonisches wird gar nicht erst gelesen** (2026-10-29): ein
    /// Zweig über zwei Leeren, Leeres neben einem Blatt, ein gekürzter
    /// Knoten mit dem Hash des Leeren. Die Länge des Gelesenen bleibt so
    /// an die Zahl der Einträge gebunden.
    #[test]
    fn unkanonisches_wird_nicht_gelesen() {
        let blatt = |aus: &mut Vec<u8>| {
            aus.push(1);
            aus.extend_from_slice(&k(1));
            aus.extend_from_slice(&k(2));
        };
        let mut zwei_leere = vec![2, 0, 0];
        assert!(Teilbaum::aus_bytes(&zwei_leere).is_none());
        zwei_leere.clear();
        zwei_leere.push(2);
        zwei_leere.push(0);
        blatt(&mut zwei_leere);
        assert!(Teilbaum::aus_bytes(&zwei_leere).is_none(), "Leeres neben einem Blatt");
        let mut leer_gekuerzt = vec![3];
        leer_gekuerzt.extend_from_slice(&LEER);
        assert!(Teilbaum::aus_bytes(&leer_gekuerzt).is_none());
        // Gegenprobe: zwei Blätter sind ein Zweig.
        let mut zwei = vec![2];
        blatt(&mut zwei);
        blatt(&mut zwei);
        assert!(Teilbaum::aus_bytes(&zwei).is_some());
    }

    /// ⚑ **Ausgelagert rechnet wie im Speicher** (2026-10-30): viele Runden
    /// aus Setzen, Neuanlegen und Entfernen auf einem Baum im Speicher und
    /// einem ausgelagerten; jedes Mal dieselbe Wurzel, dieselben Werte,
    /// dieselben Beweise und Byte für Byte derselbe Zeuge. Ausgelagert steht
    /// danach nur ein Bruchteil im Speicher, eine Kopie liegt ganz im
    /// Speicher mit derselben Wurzel, und wer alles entfernt, hinterlässt
    /// keine belegte Seite.
    #[test]
    fn ausgelagert_rechnet_wie_im_speicher() {
        let pfad = std::env::temp_dir().join(format!("myl-seiten-{}-{}", std::process::id(), line!()));
        let mut speicher = Zustandsbaum::default();
        let mut platte = Zustandsbaum::mit_ablage(&pfad).expect("Ablage");
        for i in 0..5_000u64 {
            speicher.setzen(k(i), Some(k(i + 1)));
            platte.setzen(k(i), Some(k(i + 1)));
        }
        assert_eq!(platte.wurzel(), speicher.wurzel());
        let im_speicher = platte.knoten_im_speicher();
        assert!(im_speicher < 1_000, "ausgelagert stehen noch {im_speicher} Knoten im Speicher");
        assert!(platte.ablage().unwrap().belegt() > 0);
        for runde in 0..30u64 {
            let wahl = |j: u64| u64::from_le_bytes(k(runde * 977 + j)[..8].try_into().unwrap());
            let mut aenderungen = Vec::new();
            for j in 0..40 {
                let s = k(wahl(j) % 6_000);
                let w = if wahl(j + 100) % 3 == 0 { None } else { Some(k(wahl(j + 200))) };
                aenderungen.push((s, w));
            }
            let schluessel: Vec<[u8; 32]> = aenderungen.iter().map(|(s, _)| *s).collect();
            let entfernt: Vec<[u8; 32]> =
                aenderungen.iter().filter(|(s, w)| w.is_none() && speicher.lesen(s).is_some()).map(|(s, _)| *s).collect();
            // Der Zeuge vor dem Block, Byte für Byte.
            assert_eq!(platte.teilbaum(&schluessel, &entfernt).als_bytes(), speicher.teilbaum(&schluessel, &entfernt).als_bytes(), "Runde {runde}");
            for (s, w) in &aenderungen {
                speicher.setzen(*s, *w);
                platte.setzen(*s, *w);
            }
            assert_eq!(platte.wurzel(), speicher.wurzel(), "Runde {runde}");
            assert_eq!(platte.len(), speicher.len());
            let probe = k(wahl(7) % 6_000);
            assert_eq!(platte.lesen(&probe), speicher.lesen(&probe));
            assert_eq!(platte.beweis(&probe), speicher.beweis(&probe));
        }
        let mut kopie = platte.clone();
        assert!(kopie.ablage().is_none());
        assert_eq!(kopie.wurzel(), speicher.wurzel(), "die Kopie liegt ganz im Speicher");
        for i in 0..6_000u64 {
            platte.setzen(k(i), None);
            speicher.setzen(k(i), None);
        }
        assert_eq!(platte.wurzel(), LEER);
        assert_eq!(speicher.wurzel(), LEER);
        assert_eq!(platte.ablage().unwrap().belegt(), 0, "eine Seite blieb belegt");
        std::fs::remove_file(&pfad).ok();
    }

    /// ⛔️ **Die Marke einer Seite ist in einem Zeugen verboten**: Ein Zeuge
    /// aus fremder Hand mit `Ausgelagert` liest sich nicht.
    #[test]
    fn eine_seitenmarke_ist_kein_zeuge() {
        let mut b = vec![4u8];
        b.extend_from_slice(&0u64.to_le_bytes());
        b.extend_from_slice(&64u32.to_le_bytes());
        b.extend_from_slice(&k(1));
        assert!(Teilbaum::aus_bytes(&b).is_none());        // Ebenso ein Zweig mit mitgebrachtem Hash (Marke 5).
        let mut z = vec![5u8];
        z.extend_from_slice(&k(9));
        for _ in 0..2 {
            z.push(1);
            z.extend_from_slice(&k(1));
            z.extend_from_slice(&k(2));
        }
        assert!(Teilbaum::aus_bytes(&z).is_none());
    }
}
