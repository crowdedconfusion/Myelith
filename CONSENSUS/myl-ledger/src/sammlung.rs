//! **Eine Sammlung des Zustands, die ihre Änderungen kennt** (Skalierung,
//! Festlegung des Projektinhabers vom 2026-10-29).
//!
//! # ⚑ Warum
//!
//! Die Zustandswurzel kommt aus dem Zustandsbaum
//! ([`myl_types::zustandsbaum`]); in ihn geht je Block nur, was sich änderte.
//! Dafür merkt sich jede Sammlung die geänderten Schlüssel. Und statt den
//! ganzen Zustand zu kopieren, bevor ein fremder Block angewandt wird (bei
//! einer Million Konten 30 ms je Block), führt sie auf Wunsch ein Journal der
//! alten Werte, das ein abgewiesener Block zurückspielt.
//!
//! Gelesen und geändert wird nur über die Methoden hier, damit kein Zugriff
//! am Merken vorbeigeht. ⚑ **Kein `Deref` auf die `BTreeMap`** (2026-10-29):
//! Darüber wäre jede Methode erreichbar, die hier nicht steht
//! (`get_key_value`, `first_key_value` …), und ein solcher Lesezugriff ginge
//! still an Aufzeichnung und Abdeckung vorbei; ein Prüfer rechnete dann mit
//! einem Wert, den kein Zeuge belegt. Wer die ganze `BTreeMap` braucht,
//! nimmt [`Sammlung::als_map`], und das zählt als Zugriff auf das Ganze.
//!
//! # ⚑ Zeugen (zustandslose Validierung, 2026-10-29)
//!
//! Die grossen Sammlungen (Konten und was mit den Teilnehmern wächst) kann
//! ein prüfender Knoten nicht halten. Der Erzeuger eines Blocks **zeichnet
//! auf**, welche Einträge der Block berührt, mit ihrem Wert davor; daraus
//! wird der Zeuge. Der Prüfer hält die Sammlung dann **teilweise**: nur die
//! Einträge des Zeugen, und ein Zugriff auf anderes ist ein **Fehlgriff**,
//! der den Block abweist. Was über die ganze Sammlung läuft (`iter`, `len`,
//! `retain` …), ist in beiden Lagen ein Fehlgriff: Es lässt sich nicht
//! bezeugen. Die Lesemethoden hier gehen deshalb denen der `BTreeMap` vor.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use borsh::{BorshDeserialize, BorshSerialize};

/// Je berührtem Schlüssel der Wert davor (`None`: es gab ihn nicht).
pub type Aufzeichnung<K, V> = BTreeMap<K, Option<V>>;

/// Siehe Modulkopf. Gleich sind zwei Sammlungen mit gleichem Inhalt; was
/// sie sich merken, zählt nicht.
#[derive(Debug)]
pub struct Sammlung<K: Ord + Clone, V: Clone> {
    inhalt: BTreeMap<K, V>,
    /// Seit der letzten Wurzel geändert.
    geaendert: BTreeSet<K>,
    /// Während ein Journal läuft: je zuerst berührtem Schlüssel der alte Wert.
    journal: Option<Vec<(K, Option<V>)>>,
    /// Im laufenden Journal schon berührt.
    beruehrt: BTreeSet<K>,
    /// Beim Erzeuger, während aufgezeichnet wird: je berührtem Schlüssel
    /// (gelesen oder geändert) der Wert davor. Hinter einem Schloss, weil
    /// auch das Lesen (`&self`) aufzeichnet.
    aufzeichnung: Option<Box<Mutex<Aufzeichnung<K, V>>>>,
    /// Beim Prüfer: die Schlüssel, die der Zeuge abdeckt.
    abgedeckt: Option<BTreeSet<K>>,
    /// Ein Zugriff, der sich nicht bezeugen lässt (siehe Modulkopf).
    fehlgriff: AtomicBool,
}

impl<K: Ord + Clone, V: Clone> Clone for Sammlung<K, V> {
    fn clone(&self) -> Self {
        Self {
            inhalt: self.inhalt.clone(),
            geaendert: self.geaendert.clone(),
            journal: self.journal.clone(),
            beruehrt: self.beruehrt.clone(),
            aufzeichnung: self
                .aufzeichnung
                .as_ref()
                .map(|a| Box::new(Mutex::new(a.lock().unwrap_or_else(|e| e.into_inner()).clone()))),
            abgedeckt: self.abgedeckt.clone(),
            fehlgriff: AtomicBool::new(self.fehlgriff.load(Ordering::Relaxed)),
        }
    }
}

impl<K: Ord + Clone, V: Clone> Default for Sammlung<K, V> {
    fn default() -> Self {
        Self::aus(BTreeMap::new(), BTreeSet::new())
    }
}

impl<K: Ord + Clone, V: Clone + PartialEq> PartialEq for Sammlung<K, V> {
    fn eq(&self, andere: &Self) -> bool {
        self.inhalt == andere.inhalt
    }
}

impl<K: Ord + Clone, V: Clone + Eq> Eq for Sammlung<K, V> {}


impl<K: Ord + Clone, V: Clone> FromIterator<(K, V)> for Sammlung<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let inhalt: BTreeMap<K, V> = iter.into_iter().collect();
        let geaendert = inhalt.keys().cloned().collect();
        Self::aus(inhalt, geaendert)
    }
}

impl<K: Ord + Clone, V: Clone> Sammlung<K, V> {
    fn aus(inhalt: BTreeMap<K, V>, geaendert: BTreeSet<K>) -> Self {
        Self {
            inhalt,
            geaendert,
            journal: None,
            beruehrt: BTreeSet::new(),
            aufzeichnung: None,
            abgedeckt: None,
            fehlgriff: AtomicBool::new(false),
        }
    }

    /// Ein Zugriff auf `k`: aufzeichnen, und beim Prüfer die Abdeckung.
    fn zugriff(&self, k: &K) {
        if let Some(a) = &self.aufzeichnung {
            a.lock().unwrap_or_else(|e| e.into_inner()).entry(k.clone()).or_insert_with(|| self.inhalt.get(k).cloned());
        }
        if self.abgedeckt.as_ref().is_some_and(|d| !d.contains(k)) {
            self.fehlgriff.store(true, Ordering::Relaxed);
        }
    }

    /// Ein Zugriff auf die ganze Sammlung: nicht zu bezeugen.
    fn ganz(&self) {
        if self.aufzeichnung.is_some() || self.abgedeckt.is_some() {
            self.fehlgriff.store(true, Ordering::Relaxed);
        }
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        self.zugriff(k);
        self.inhalt.get(k)
    }

    pub fn contains_key(&self, k: &K) -> bool {
        self.zugriff(k);
        self.inhalt.contains_key(k)
    }

    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, K, V> {
        self.ganz();
        self.inhalt.iter()
    }

    pub fn keys(&self) -> std::collections::btree_map::Keys<'_, K, V> {
        self.ganz();
        self.inhalt.keys()
    }

    pub fn values(&self) -> std::collections::btree_map::Values<'_, K, V> {
        self.ganz();
        self.inhalt.values()
    }

    pub fn range<R: std::ops::RangeBounds<K>>(&self, r: R) -> std::collections::btree_map::Range<'_, K, V> {
        self.ganz();
        self.inhalt.range(r)
    }

    pub fn len(&self) -> usize {
        self.ganz();
        self.inhalt.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ganz();
        self.inhalt.is_empty()
    }

    /// Die ganze `BTreeMap`, für Code, der eine will; zählt als Zugriff auf
    /// das Ganze (siehe Modulkopf).
    pub fn als_map(&self) -> &BTreeMap<K, V> {
        self.ganz();
        &self.inhalt
    }

    /// Die Zahl der Einträge, ohne als Zugriff zu zählen (für die Wurzel
    /// und für Angaben über den Knoten selbst).
    pub fn anzahl(&self) -> usize {
        self.inhalt.len()
    }

    /// Ob `k` enthalten ist, ohne als Zugriff zu zählen. ⚑ Wie `wert_still`
    /// und `alle` nur in dieser Kiste, für den Baum: Im Blockpfad ginge ein
    /// stiller Zugriff an Aufzeichnung und Abdeckung vorbei.
    pub(crate) fn enthaelt_still(&self, k: &K) -> bool {
        self.inhalt.contains_key(k)
    }

    /// Der Wert zu `k`, ohne als Zugriff zu zählen (für den Baum).
    pub(crate) fn wert_still(&self, k: &K) -> Option<&V> {
        self.inhalt.get(k)
    }

    /// Alle Einträge, ohne als Zugriff zu zählen (für den Bau des Baums).
    pub(crate) fn alle(&self) -> std::collections::btree_map::Iter<'_, K, V> {
        self.inhalt.iter()
    }

    /// Vor der Änderung von `k`: merken, und im Journal den alten Wert.
    fn vor(&mut self, k: &K) {
        self.zugriff(k);
        self.geaendert.insert(k.clone());
        if let Some(j) = self.journal.as_mut() {
            if self.beruehrt.insert(k.clone()) {
                j.push((k.clone(), self.inhalt.get(k).cloned()));
            }
        }
    }

    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.vor(&k);
        self.inhalt.insert(k, v)
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        if !self.contains_key(k) {
            return None;
        }
        self.vor(k);
        self.inhalt.remove(k)
    }

    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        if !self.contains_key(k) {
            return None;
        }
        self.vor(k);
        self.inhalt.get_mut(k)
    }

    /// Wie `BTreeMap::entry`; der Schlüssel gilt als geändert.
    pub fn entry(&mut self, k: K) -> std::collections::btree_map::Entry<'_, K, V> {
        self.vor(&k);
        self.inhalt.entry(k)
    }

    /// Wie `BTreeMap::retain`; was wegfällt, gilt als geändert. ⚑ **Nur
    /// lesend**: Dürfte die Prüfung behaltene Einträge ändern, gingen diese
    /// Änderungen am Merken vorbei.
    pub fn retain(&mut self, mut f: impl FnMut(&K, &V) -> bool) {
        self.ganz();
        let weg: Vec<K> = self.inhalt.iter().filter(|(k, v)| !f(k, v)).map(|(k, _)| k.clone()).collect();
        for k in weg {
            self.remove(&k);
        }
    }

    /// Wie `BTreeMap::values_mut`: jeder Schlüssel gilt als geändert.
    pub fn values_mut(&mut self) -> std::collections::btree_map::ValuesMut<'_, K, V> {
        self.ganz();
        let alle: Vec<K> = self.inhalt.keys().cloned().collect();
        for k in &alle {
            self.vor(k);
        }
        self.inhalt.values_mut()
    }

    /// Wie `BTreeMap::iter_mut`: jeder Schlüssel gilt als geändert.
    pub fn iter_mut(&mut self) -> std::collections::btree_map::IterMut<'_, K, V> {
        self.ganz();
        let alle: Vec<K> = self.inhalt.keys().cloned().collect();
        for k in &alle {
            self.vor(k);
        }
        self.inhalt.iter_mut()
    }

    /// Nimmt den ganzen Inhalt heraus; jeder Schlüssel gilt als entfernt.
    /// ⛔️ Nie `std::mem::take` auf eine Sammlung: Das ginge am Merken vorbei.
    pub fn nehmen(&mut self) -> BTreeMap<K, V> {
        self.ganz();
        let alle: Vec<K> = self.inhalt.keys().cloned().collect();
        for k in &alle {
            self.vor(k);
        }
        std::mem::take(&mut self.inhalt)
    }

    pub fn clear(&mut self) {
        self.ganz();
        let alle: Vec<K> = self.inhalt.keys().cloned().collect();
        for k in &alle {
            self.vor(k);
        }
        self.inhalt.clear();
    }

    pub fn extend(&mut self, iter: impl IntoIterator<Item = (K, V)>) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }

    /// **Für Code, der eine `BTreeMap` will**: Alle Schlüssel vorher und
    /// nachher gelten als geändert; im Journal stehen die alten Werte und,
    /// für neue Schlüssel, ihr Fehlen.
    pub fn veraendern<R>(&mut self, f: impl FnOnce(&mut BTreeMap<K, V>) -> R) -> R {
        self.ganz();
        let vorher: Vec<K> = self.inhalt.keys().cloned().collect();
        for k in &vorher {
            self.vor(k);
        }
        let r = f(&mut self.inhalt);
        let neu: Vec<K> = self.inhalt.keys().filter(|k| !self.beruehrt.contains(*k) && !vorher.contains(k)).cloned().collect();
        for k in neu {
            self.geaendert.insert(k.clone());
            if let Some(j) = self.journal.as_mut() {
                self.beruehrt.insert(k.clone());
                j.push((k, None));
            }
        }
        // Ohne Journal: Die neuen sind oben schon gemerkt.
        let alle: Vec<K> = self.inhalt.keys().cloned().collect();
        self.geaendert.extend(alle);
        r
    }

    /// Die seit der letzten Wurzel geänderten Schlüssel, ohne sie zu nehmen.
    pub fn geaenderte(&self) -> &BTreeSet<K> {
        &self.geaendert
    }

    /// Nimmt die seit der letzten Wurzel geänderten Schlüssel.
    pub fn geaenderte_nehmen(&mut self) -> BTreeSet<K> {
        std::mem::take(&mut self.geaendert)
    }

    /// Alle Schlüssel gelten als geändert (für den ersten Bau des Baums).
    pub fn alle_als_geaendert(&mut self) {
        self.geaendert = self.inhalt.keys().cloned().collect();
    }

    /// ⚑ **Beginnt die Aufzeichnung** (beim Erzeuger, vor dem Block); eine
    /// laufende wird verworfen.
    pub fn aufzeichnen_beginnen(&mut self) {
        self.aufzeichnung = Some(Box::new(Mutex::new(BTreeMap::new())));
        self.fehlgriff.store(false, Ordering::Relaxed);
    }

    /// Beendet die Aufzeichnung: je berührtem Schlüssel der Wert davor.
    pub fn aufzeichnung_nehmen(&mut self) -> Aufzeichnung<K, V> {
        self.aufzeichnung.take().map(|a| a.into_inner().unwrap_or_else(|e| e.into_inner())).unwrap_or_default()
    }

    /// ⚑ **Hält die Sammlung teilweise** (beim Prüfer): nur die Einträge
    /// eines Zeugen; `None` belegt das Fehlen. Was vorher drin war, fällt
    /// ohne Merken weg.
    pub fn teilweise_setzen(&mut self, eintraege: Vec<(K, Option<V>)>) {
        self.abgedeckt = Some(eintraege.iter().map(|(k, _)| k.clone()).collect());
        self.inhalt = eintraege.into_iter().filter_map(|(k, v)| v.map(|v| (k, v))).collect();
        self.geaendert.clear();
        self.journal = None;
        self.beruehrt.clear();
        self.fehlgriff.store(false, Ordering::Relaxed);
    }

    /// Beendet die teilweise Haltung: leer, nichts gemerkt.
    pub fn teilweise_beenden(&mut self) {
        *self = Self::default();
    }

    /// Ob seit Beginn der Aufzeichnung oder der teilweisen Haltung ein
    /// Zugriff nicht zu bezeugen war.
    pub fn fehlgriff(&self) -> bool {
        self.fehlgriff.load(Ordering::Relaxed)
    }

    /// Beginnt ein Journal; ein laufendes wird verworfen.
    pub fn journal_beginnen(&mut self) {
        self.journal = Some(Vec::new());
        self.beruehrt.clear();
    }

    /// Schliesst das Journal ab: Die Änderungen bleiben.
    pub fn journal_abschliessen(&mut self) {
        self.journal = None;
        self.beruehrt.clear();
    }

    /// Spielt das Journal zurück: Jeder berührte Schlüssel bekommt seinen
    /// alten Wert (und gilt als geändert, damit der Baum ihm folgt).
    pub fn journal_zurueckspielen(&mut self) {
        let Some(j) = self.journal.take() else {
            return;
        };
        self.beruehrt.clear();
        for (k, alt) in j.into_iter().rev() {
            self.geaendert.insert(k.clone());
            match alt {
                Some(v) => {
                    self.inhalt.insert(k, v);
                }
                None => {
                    self.inhalt.remove(&k);
                }
            }
        }
    }
}

/// Wie bei der `BTreeMap`: `s[&k]` liest (und zählt als Zugriff), ein
/// fehlender Schlüssel ist ein Programmfehler.
impl<K: Ord + Clone, V: Clone> std::ops::Index<&K> for Sammlung<K, V> {
    type Output = V;
    fn index(&self, k: &K) -> &V {
        self.get(k).expect("Schlüssel in der Sammlung")
    }
}

// Kodiert wird nur der Inhalt, wie eine `BTreeMap`: Die Kettendatei und die
// Abzüge bleiben in ihrer Form.
impl<K: Ord + Clone + BorshSerialize, V: Clone + BorshSerialize> BorshSerialize for Sammlung<K, V> {
    fn serialize<W: std::io::Write>(&self, w: &mut W) -> std::io::Result<()> {
        self.inhalt.serialize(w)
    }
}

impl<K: Ord + Clone + BorshDeserialize, V: Clone + BorshDeserialize> BorshDeserialize for Sammlung<K, V> {
    fn deserialize_reader<R: std::io::Read>(r: &mut R) -> std::io::Result<Self> {
        let inhalt = BTreeMap::<K, V>::deserialize_reader(r)?;
        let geaendert = inhalt.keys().cloned().collect();
        Ok(Self::aus(inhalt, geaendert))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Das Journal spielt jede Art Änderung zurück**: Einfügen, Ändern,
    /// Entfernen, `entry`, `retain`; danach ist der Inhalt wie vorher, und
    /// alles Berührte gilt als geändert (der Baum folgt).
    #[test]
    fn das_journal_spielt_alles_zurueck() {
        let mut s: Sammlung<u8, u32> = [(1, 10), (2, 20), (3, 30)].into_iter().collect();
        let vorher = s.als_map().clone();
        s.geaenderte_nehmen();
        s.journal_beginnen();
        s.insert(4, 40);
        s.insert(1, 11);
        *s.get_mut(&2).unwrap() += 1;
        *s.entry(5).or_insert(0) += 50;
        s.retain(|k, _| *k != 3);
        s.remove(&1);
        assert_ne!(*s.als_map(), vorher);
        s.journal_zurueckspielen();
        assert_eq!(*s.als_map(), vorher);
        let g = s.geaenderte_nehmen();
        assert_eq!(g, [1, 2, 3, 4, 5].into_iter().collect());
        // Ohne Journal: nur gemerkt.
        s.insert(9, 90);
        assert_eq!(s.geaenderte_nehmen(), [9].into_iter().collect());
        s.journal_beginnen();
        s.insert(9, 91);
        s.journal_abschliessen();
        assert_eq!(s.get(&9), Some(&91));
    }

    #[test]
    fn kodiert_wie_eine_btreemap() {
        let s: Sammlung<u8, u32> = [(1, 10), (2, 20)].into_iter().collect();
        let m: BTreeMap<u8, u32> = [(1, 10), (2, 20)].into_iter().collect();
        assert_eq!(borsh::to_vec(&s).unwrap(), borsh::to_vec(&m).unwrap());
        let zurueck: Sammlung<u8, u32> = borsh::from_slice(&borsh::to_vec(&m).unwrap()).unwrap();
        assert_eq!(*zurueck.als_map(), m);
    }

    /// `veraendern` und `nehmen` gehen nicht am Merken vorbei: Alle
    /// Schlüssel davor und danach gelten als geändert, und das Journal spielt
    /// auch neu entstandene zurück.
    #[test]
    fn veraendern_und_nehmen_werden_gemerkt() {
        let mut s: Sammlung<u8, u32> = [(1, 10), (2, 20)].into_iter().collect();
        s.geaenderte_nehmen();
        s.journal_beginnen();
        s.veraendern(|m| {
            m.remove(&1);
            m.insert(3, 30);
            *m.get_mut(&2).unwrap() = 21;
        });
        assert_eq!(s.geaenderte_nehmen(), [1, 2, 3].into_iter().collect());
        s.journal_zurueckspielen();
        assert_eq!(*s.als_map(), [(1, 10), (2, 20)].into_iter().collect::<BTreeMap<_, _>>());
        s.geaenderte_nehmen();
        let alles = s.nehmen();
        assert_eq!(alles.len(), 2);
        assert!(s.is_empty());
        assert_eq!(s.geaenderte_nehmen(), [1, 2].into_iter().collect());
    }

    /// ⚑ **Aufzeichnung und teilweise Haltung** (zustandslose Validierung,
    /// 2026-10-29): Die Aufzeichnung kennt jeden berührten Schlüssel mit
    /// dem Wert davor, gelesen wie geändert, auch fehlende; wer teilweise
    /// hält, rechnet mit dem Aufgezeichneten dasselbe, und ein Zugriff
    /// daneben oder über das Ganze ist ein Fehlgriff.
    #[test]
    fn aufzeichnung_und_teilweise_haltung() {
        let mut voll: Sammlung<u8, u32> = (0..50).map(|i| (i, u32::from(i) * 10)).collect();
        voll.aufzeichnen_beginnen();
        let a = voll.get(&3).copied().unwrap_or(0);
        voll.insert(4, a + 1);
        voll.remove(&5);
        assert!(!voll.contains_key(&99), "fehlt");
        *voll.entry(98).or_insert(0) += 7;
        assert!(!voll.fehlgriff());
        let auf = voll.aufzeichnung_nehmen();
        assert_eq!(
            auf,
            [(3, Some(30)), (4, Some(40)), (5, Some(50)), (98, None), (99, None)].into_iter().collect::<BTreeMap<_, _>>()
        );
        let mut teil: Sammlung<u8, u32> = Sammlung::default();
        teil.teilweise_setzen(auf.into_iter().collect());
        let a = teil.get(&3).copied().unwrap_or(0);
        teil.insert(4, a + 1);
        teil.remove(&5);
        assert!(!teil.contains_key(&99));
        *teil.entry(98).or_insert(0) += 7;
        assert!(!teil.fehlgriff(), "alles abgedeckt");
        for k in teil.geaenderte() {
            assert_eq!(teil.inhalt.get(k), voll.inhalt.get(k), "Schlüssel {k}");
        }
        let _ = teil.get(&7);
        assert!(teil.fehlgriff(), "nicht abgedeckt");
        let mut teil2: Sammlung<u8, u32> = Sammlung::default();
        teil2.teilweise_setzen(vec![(1, Some(1))]);
        let _ = teil2.len();
        assert!(teil2.fehlgriff(), "über das Ganze");
        voll.aufzeichnen_beginnen();
        let _ = voll.iter().count();
        assert!(voll.fehlgriff(), "auch beim Erzeuger");
        voll.aufzeichnung_nehmen();
    }
}
