//! ⚑ **Dieselbe Anfrage an der Tür heißt eine Rechnung** (Fund 727,
//! Auftragslauf vom 2026-11-24).
//!
//! 📌 Die Tür wartet höchstens `ortsklient::TUERFRIST` (540 s) auf den
//! Ortsdienst und meldet dann „kein Pod hat gerechnet“; der Klient setzt nach
//! 15 s neu an (`myl_client::tuerweg::WIEDERHOLUNG`). Der Ortsdienst rechnete
//! die erste Anfrage aber weiter, und der neue Anlauf begann dieselbe Runde
//! ein zweites, dann ein drittes Mal: Im Kurzlauf rechneten Sitzung 4, 5 und
//! 6 mit derselben Eingabe gleichzeitig, die dreifache Last auf denselben
//! Pods, und keine wurde fertig.
//!
//! Hier läuft eine Rechnung je Schlüssel als eigene Aufgabe, unabhängig davon,
//! wer gerade wartet. ⚑ **Ihr Ergebnis geht an genau einen**: Wer mit demselben
//! Schlüssel kommt, nachdem der Wartende in seine Frist gelaufen und gegangen
//! ist, übernimmt sie, solange sie läuft oder ihr Ergebnis noch [`AUFHEBEN`]
//! lang liegt. Ist das Ergebnis ausgeliefert, ist es vergessen, und dieselbe
//! Frage danach ist eine neue Anfrage mit eigener Rechnung; ebenso eine, die
//! kommt, während ein anderer noch wartet. 📌 Die erste Fassung gab ein
//! Ergebnis 180 s lang an jeden mit demselben Schlüssel: Zwei Anfragen
//! teilten sich ein Segment und wurden zweimal abgebucht (Probe `gesamtlauf`
//! im Testclient). Das deckt sich mit Fund 717: Was angefangen ist, wird zu
//! Ende gerechnet; abgebucht wird, was ausgeliefert wird, einmal.

use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::watch;

/// So lange liegt ein Ergebnis, das niemand abgeholt hat, für einen neuen
/// Anlauf bereit: länger als die Pausen des Klienten zusammen (15, 30, 60 s).
///
/// ⛔️ **Fund 730 (2026-11-25): Solange auch läuft eine Rechnung ohne
/// Wartenden weiter, dann wird sie abgebrochen.** Bis hierher rechnete sie zu
/// Ende, auch wenn der Klient längst aufgegeben hatte: Im langen
/// Auftragslauf rechnete der Ortsdienst die letzte Runde des ersten
/// Auftrags noch 27 Minuten in den zweiten hinein, über zwei Epochen, auf
/// denselben Pods. Mit der Aufgabe fällt die Leitung zum Ortsdienst, und der
/// hält die Sitzung an.
pub const AUFHEBEN: Duration = Duration::from_secs(180);

/// Eine Rechnung, die läuft oder deren Ergebnis noch nicht ausgeliefert ist.
struct Lauf<V> {
    kennung: u64,
    rx: watch::Receiver<Option<V>>,
    /// Ob gerade jemand auf sie wartet.
    wartet: bool,
    /// Wie oft ein Wartender sie verlassen hat; die Uhr nach dem Verlassen
    /// bricht nur ab, wenn seitdem niemand kam und ging.
    verlassen: u64,
    /// Bricht die Rechnung ab (Fund 730).
    abbruch: Arc<tokio::sync::Notify>,
}

type Laeufe<K, V> = Arc<Mutex<HashMap<K, Lauf<V>>>>;

/// Laufende und noch nicht ausgelieferte Rechnungen je Schlüssel.
pub struct Einmal<K, V> {
    laufend: Laeufe<K, V>,
    aufheben: Duration,
    naechste: std::sync::atomic::AtomicU64,
}

impl<K, V> Default for Einmal<K, V> {
    fn default() -> Self {
        Self::mit_aufheben_intern(AUFHEBEN)
    }
}

impl<K, V> Einmal<K, V> {
    fn mit_aufheben_intern(aufheben: Duration) -> Self {
        Self { laufend: Arc::new(Mutex::new(HashMap::new())), aufheben, naechste: std::sync::atomic::AtomicU64::new(0) }
    }
}

fn sperre<K, V>(l: &Laeufe<K, V>) -> std::sync::MutexGuard<'_, HashMap<K, Lauf<V>>> {
    l.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Wie [`Einmal::holen`] ausging.
#[derive(Debug, PartialEq, Eq)]
pub enum Geholt<V> {
    /// Das Ergebnis, und ob diese Anfrage die Rechnung begonnen hat.
    Fertig { wert: V, begonnen: bool },
    /// Die Frist lief ab; die Rechnung läuft weiter, ein neuer Anlauf mit
    /// demselben Schlüssel wartet auf sie.
    Frist,
}

impl<K, V> Einmal<K, V>
where
    K: Hash + Eq + Clone + Send + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Mit einer anderen Aufbewahrung (für Proben).
    pub fn mit_aufheben(aufheben: Duration) -> Self {
        Self::mit_aufheben_intern(aufheben)
    }

    /// Das Ergebnis zu `schluessel`, höchstens `frist` lang gewartet. Liegt
    /// zu diesem Schlüssel eine Rechnung, auf die niemand mehr wartet,
    /// übernimmt diese Anfrage sie, und `rechnen` wird nicht gerufen; sonst
    /// beginnt `rechnen()` eine als eigene Aufgabe.
    pub async fn holen<F, Fut>(&self, schluessel: K, frist: Duration, rechnen: F) -> Geholt<V>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = V> + Send + 'static,
    {
        let (kennung, mut rx, begonnen, allein) = {
            let mut l = sperre(&self.laufend);
            match l.get_mut(&schluessel).filter(|lauf| !lauf.wartet) {
                Some(lauf) => {
                    lauf.wartet = true;
                    (Some(lauf.kennung), lauf.rx.clone(), false, None)
                }
                None => {
                    let (tx, rx) = watch::channel(None);
                    let kennung = self.naechste.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    // Wartet schon jemand auf eine andere Rechnung desselben
                    // Schlüssels, ist dies eine eigene Anfrage, und sie steht
                    // nicht zur Übernahme.
                    let eingetragen = !l.contains_key(&schluessel);
                    let abbruch = Arc::new(tokio::sync::Notify::new());
                    if eingetragen {
                        l.insert(schluessel.clone(), Lauf { kennung, rx: rx.clone(), wartet: true, verlassen: 0, abbruch: Arc::clone(&abbruch) });
                    }
                    let (laufend, aufheben, k) = (Arc::clone(&self.laufend), self.aufheben, schluessel.clone());
                    let fut = rechnen();
                    let a = Arc::clone(&abbruch);
                    tokio::spawn(async move {
                        // Abgebrochen: Die Rechnung fällt samt ihrer Leitung.
                        let wert = tokio::select! {
                            w = fut => w,
                            () = a.notified() => return,
                        };
                        let _ = tx.send(Some(wert));
                        if eingetragen {
                            tokio::time::sleep(aufheben).await;
                            let mut l = sperre(&laufend);
                            if l.get(&k).is_some_and(|lauf| lauf.kennung == kennung) {
                                l.remove(&k);
                            }
                        }
                    });
                    // Nicht eingetragen kann sie niemand übernehmen; geht
                    // ihr Wartender, bricht sie sofort ab.
                    (eingetragen.then_some(kennung), rx, true, (!eingetragen).then_some(abbruch))
                }
            }
        };
        let warten = async {
            loop {
                if let Some(v) = rx.borrow_and_update().clone() {
                    return Some(v);
                }
                if rx.changed().await.is_err() {
                    return rx.borrow().clone();
                }
            }
        };
        let geholt = match tokio::time::timeout(frist, warten).await {
            Ok(Some(wert)) => Geholt::Fertig { wert, begonnen },
            _ => Geholt::Frist,
        };
        // Ausgeliefert: vergessen. In die Frist gelaufen: frei zur Übernahme.
        if let Some(kennung) = kennung {
            let mut l = sperre(&self.laufend);
            if l.get(&schluessel).is_some_and(|lauf| lauf.kennung == kennung) {
                match geholt {
                    Geholt::Fertig { .. } => {
                        l.remove(&schluessel);
                    }
                    Geholt::Frist => {
                        if let Some(lauf) = l.get_mut(&schluessel) {
                            lauf.wartet = false;
                            lauf.verlassen += 1;
                            let (laufend, aufheben, k, verlassen) = (Arc::clone(&self.laufend), self.aufheben, schluessel.clone(), lauf.verlassen);
                            tokio::spawn(async move {
                                tokio::time::sleep(aufheben).await;
                                let mut l = sperre(&laufend);
                                let verwaist = l.get(&k).is_some_and(|lauf| lauf.kennung == kennung && !lauf.wartet && lauf.verlassen == verlassen);
                                if verwaist {
                                    if let Some(lauf) = l.remove(&k) {
                                        lauf.abbruch.notify_one();
                                    }
                                }
                            });
                        }
                    }
                }
            }
        }
        if let (Geholt::Frist, Some(a)) = (&geholt, allein) {
            a.notify_one();
        }
        geholt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Zwei gleichzeitige Anfragen mit demselben Schlüssel sind zwei
    /// Anfragen: Solange die erste wartet, rechnet die zweite für sich, und
    /// jede bekommt ihr eigenes Ergebnis. Nacheinander ebenso: Ein
    /// ausgeliefertes Ergebnis ist vergessen (Probe `gesamtlauf`).
    #[tokio::test]
    async fn zwei_anfragen_rechnen_zweimal() {
        let e: Arc<Einmal<u8, u32>> = Arc::new(Einmal::default());
        let gerechnet = Arc::new(AtomicUsize::new(0));
        let anfrage = |e: Arc<Einmal<u8, u32>>, g: Arc<AtomicUsize>| async move {
            e.holen(1, Duration::from_secs(5), move || async move {
                g.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(200)).await;
                42
            })
            .await
        };
        let (a, b) = tokio::join!(anfrage(Arc::clone(&e), Arc::clone(&gerechnet)), anfrage(Arc::clone(&e), Arc::clone(&gerechnet)));
        assert_eq!(gerechnet.load(Ordering::SeqCst), 2, "eine Rechnung für zwei wartende Anfragen");
        for g in [a, b] {
            assert_eq!(g, Geholt::Fertig { wert: 42, begonnen: true });
        }
        assert_eq!(anfrage(Arc::clone(&e), Arc::clone(&gerechnet)).await, Geholt::Fertig { wert: 42, begonnen: true });
        assert_eq!(gerechnet.load(Ordering::SeqCst), 3, "ein ausgeliefertes Ergebnis ging ein zweites Mal hinaus");
    }

    /// 📌 Fund 727 nachgestellt: Die erste Anfrage gibt nach ihrer Frist auf,
    /// die Rechnung läuft weiter, und der neue Anlauf nach einer Pause
    /// bekommt ihr Ergebnis, ohne eine zweite zu beginnen. Danach ist es
    /// vergessen; ein anderer Schlüssel rechnet für sich; ein nicht
    /// abgeholtes Ergebnis liegt nur die Aufbewahrung lang.
    #[tokio::test]
    async fn ein_neuer_anlauf_nach_der_frist_holt_die_laufende() {
        let e: Einmal<u8, u32> = Einmal::mit_aufheben(Duration::from_millis(400));
        let gerechnet = Arc::new(AtomicUsize::new(0));
        let rechne = |g: Arc<AtomicUsize>, wert: u32| {
            move || async move {
                g.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(300)).await;
                wert
            }
        };
        assert_eq!(e.holen(1, Duration::from_millis(50), rechne(Arc::clone(&gerechnet), 7)).await, Geholt::Frist);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            e.holen(1, Duration::from_secs(5), rechne(Arc::clone(&gerechnet), 99)).await,
            Geholt::Fertig { wert: 7, begonnen: false }
        );
        assert_eq!(gerechnet.load(Ordering::SeqCst), 1, "der neue Anlauf hat neu gerechnet");
        // Ausgeliefert ist vergessen: dieselbe Frage rechnet neu.
        assert_eq!(e.holen(1, Duration::from_secs(5), rechne(Arc::clone(&gerechnet), 9)).await, Geholt::Fertig { wert: 9, begonnen: true });
        assert_eq!(e.holen(2, Duration::from_secs(5), rechne(Arc::clone(&gerechnet), 8)).await, Geholt::Fertig { wert: 8, begonnen: true });
        // Ein Ergebnis, das niemand abholte, liegt nur [`AUFHEBEN`] lang.
        assert_eq!(e.holen(3, Duration::from_millis(50), rechne(Arc::clone(&gerechnet), 5)).await, Geholt::Frist);
        tokio::time::sleep(Duration::from_millis(1_000)).await;
        assert_eq!(e.holen(3, Duration::from_secs(5), rechne(Arc::clone(&gerechnet), 6)).await, Geholt::Fertig { wert: 6, begonnen: true });
        assert_eq!(gerechnet.load(Ordering::SeqCst), 5);
    }

    /// ⛔️ Fund 730: Eine Rechnung, die niemand mehr übernimmt, fällt nach
    /// der Aufbewahrung, samt dem, was sie hält (die Leitung zum Ortsdienst);
    /// eine, die nicht zur Übernahme steht, fällt mit ihrem Wartenden. Ein
    /// neuer Anlauf vor Ablauf hält sie am Leben. 📌 Gegenprobe ohne Abbruch:
    /// Beide Rechnungen laufen zu Ende.
    #[tokio::test]
    async fn eine_rechnung_ohne_wartenden_faellt_nach_der_aufbewahrung() {
        struct Leitung(Arc<AtomicUsize>);
        impl Drop for Leitung {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let e: Arc<Einmal<u8, u32>> = Arc::new(Einmal::mit_aufheben(Duration::from_millis(200)));
        let (zu_ende, gefallen) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let rechne = |dauer: u64| {
            let (z, g) = (Arc::clone(&zu_ende), Arc::clone(&gefallen));
            move || async move {
                let _leitung = Leitung(g);
                tokio::time::sleep(Duration::from_millis(dauer)).await;
                z.fetch_add(1, Ordering::SeqCst);
                1
            }
        };
        // Verlassen und nicht übernommen: fällt nach 200 ms, nicht erst nach
        // 800.
        assert_eq!(e.holen(1, Duration::from_millis(50), rechne(800)).await, Geholt::Frist);
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert_eq!((zu_ende.load(Ordering::SeqCst), gefallen.load(Ordering::SeqCst)), (0, 1), "die verwaiste Rechnung lief weiter");
        // Neu angelaufen vor Ablauf: Sie lebt und liefert.
        assert_eq!(e.holen(2, Duration::from_millis(50), rechne(400)).await, Geholt::Frist);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(e.holen(2, Duration::from_secs(5), rechne(400)).await, Geholt::Fertig { wert: 1, begonnen: false });
        assert_eq!(zu_ende.load(Ordering::SeqCst), 1);
        // Eine zweite Rechnung neben einer wartenden steht nicht zur
        // Übernahme: Geht ihr Wartender, fällt sie sofort.
        let (e2, r) = (Arc::clone(&e), rechne(800));
        let erste = tokio::spawn(async move { e2.holen(3, Duration::from_secs(5), r).await });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(e.holen(3, Duration::from_millis(50), rechne(800)).await, Geholt::Frist);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(gefallen.load(Ordering::SeqCst), 3, "die zweite Rechnung lief weiter");
        assert_eq!(erste.await.expect("erste"), Geholt::Fertig { wert: 1, begonnen: true });
    }
}
