//! Das Warten auf Peers, einmal statt viermal.
//!
//! # ⚑ Warum das hier steht: vier Kopien, die auseinandergelaufen waren
//!
//! `warte_auf_peers` stand am 2026-09-01 in `chaos.rs`, `nat.rs`,
//! `eclipse_sybil.rs` und `sitzung.rs`. Die vier waren **nicht** gleich,
//! und die Unterschiede betrafen genau das, worauf es ankommt:
//!
//! | Datei | Takt | Bei Fristablauf | Wenn der Knoten weg ist |
//! |---|---|---|---|
//! | `chaos` | 100 ms | gibt die Zahl zurück | **Abbruch** (`expect`) |
//! | `nat` | 100 ms | gibt die Zahl zurück | **null Peers** (`unwrap_or(0)`) |
//! | `eclipse_sybil` | **50 ms** | gibt die Zahl zurück | null Peers |
//! | `sitzung` | 100 ms | **`assert!`, der Test fällt** | Abbruch |
//!
//! ⚑ **Die letzte Spalte ist die gefährliche.** In einem Chaos-Test ist
//! ein verschwundener Knoten der **Versuchsaufbau** und kein Fehler; ein
//! `expect` macht daraus einen Abbruch mitten im Szenario. Umgekehrt
//! liest sich „null Peers" wie ein Befund, obwohl nur der Kanal zu ist.
//!
//! **Wer ein Muster aus einer Datei in die nächste übernahm, bekam ein
//! anderes Fehlerverhalten, ohne es zu merken.** Das ist der Preis von
//! vier Kopien, und er fällt nicht beim Schreiben an, sondern beim
//! Lesen eines Fehlschlags.
//!
//! # Die eine Fassung, und wie sie sich entscheidet
//!
//! - **Ein verschwundener Knoten zählt als null Peers.** In diesen Tests
//!   ist das Verschwinden ein Szenario. Wer daraus einen Fehler machen
//!   will, prüft die Zahl und sagt es selbst.
//! - **Bei Fristablauf kommt die Zahl zurück**, nicht ein Abbruch. Der
//!   Aufrufer weiß, ob er ein Erreichen oder ein Ausbleiben erwartet;
//!   diese Funktion weiß es nicht.
//! - **Ein Takt für alle.** 50 ms: Der schnellere der beiden gefundenen
//!   Werte, denn ein Takt kostet nur Aufwachen, und ein zu grober
//!   verlängert jeden Test um bis zu seine eigene Länge.

// Jede Testbinärdatei übersetzt dieses Modul für sich und benutzt
// davon, was sie braucht. Ungenutztes ist hier kein Befund, sondern die
// Bauart getrennter Testbinärdateien.
#![allow(dead_code)]

use std::time::Duration;

use myl_net::NodeCommand;
use tokio::sync::{mpsc, oneshot};

/// Abstand zweier Abfragen.
pub const TAKT: Duration = Duration::from_millis(50);

/// Die Zahl der verbundenen Peers, oder null, wenn der Knoten weg ist.
pub async fn peerzahl(kommandos: &mpsc::UnboundedSender<NodeCommand>) -> usize {
    let (tx, rx) = oneshot::channel();
    if kommandos.send(NodeCommand::PeerCount(tx)).is_err() {
        // Der Knoten ist fort. Das ist in diesen Tests ein Szenario.
        return 0;
    }
    rx.await.unwrap_or(0)
}

/// Wartet, bis mindestens `ziel` Peers verbunden sind, längstens `frist`.
///
/// **Gibt die zuletzt gelesene Zahl zurück**, auch wenn die Frist abläuft.
/// Ob das ein Fehlschlag ist, entscheidet der Aufrufer.
pub async fn warte_auf_peers(
    kommandos: &mpsc::UnboundedSender<NodeCommand>,
    ziel: usize,
    frist: Duration,
) -> usize {
    let ende = tokio::time::Instant::now() + frist;
    loop {
        let n = peerzahl(kommandos).await;
        if n >= ziel || tokio::time::Instant::now() >= ende {
            return n;
        }
        tokio::time::sleep(TAKT).await;
    }
}

/// Wartet, bis **höchstens** `ziel` Peers verbunden sind, längstens `frist`.
///
/// # ⚑ Die Gegenrichtung, und sie fehlte
///
/// [`warte_auf_peers`] wartet auf ein **Erreichen**. Nach einer Sperre
/// wartet man auf das Gegenteil: dass die bestehende Verbindung
/// wegfällt. Dafür stand in `chaos.rs` ein fester
/// `sleep(500 ms)`. ⚑ **Der ist am 2026-09-01 unter Last
/// umgefallen.** Der volle Lauf brauchte 28,6 Sekunden statt 3,6; in
/// der Zeit war die Sperre noch nicht wirksam, die Nachricht kam durch,
/// und der Partitionstest meldete einen Fehler, den es nicht gab.
///
/// **Ein Test, der nur auf einer unbeschäftigten Maschine besteht, ist
/// kein Test, sondern ein Wetterbericht.** Gewartet wird deshalb auf die
/// **Wirkung** und nicht auf die Uhr; wer früher fertig ist, wartet
/// nicht länger, und wer länger braucht, bekommt die Zeit.
pub async fn warte_auf_trennung(
    kommandos: &mpsc::UnboundedSender<NodeCommand>,
    ziel: usize,
    frist: Duration,
) -> usize {
    let ende = tokio::time::Instant::now() + frist;
    loop {
        let n = peerzahl(kommandos).await;
        if n <= ziel || tokio::time::Instant::now() >= ende {
            return n;
        }
        tokio::time::sleep(TAKT).await;
    }
}

/// Wartet, bis die Peerzahl `ruhe` lang unverändert bleibt, längstens `frist`.
///
/// Für Tests, die nicht auf ein Erreichen warten, sondern darauf, dass
/// sich nichts mehr tut.
pub async fn warte_auf_ruhe(
    kommandos: &mpsc::UnboundedSender<NodeCommand>,
    ruhe: Duration,
    frist: Duration,
) -> usize {
    let ende = tokio::time::Instant::now() + frist;
    let mut letzte = peerzahl(kommandos).await;
    let mut seit = tokio::time::Instant::now();
    loop {
        tokio::time::sleep(TAKT).await;
        let jetzt = peerzahl(kommandos).await;
        if jetzt != letzte {
            letzte = jetzt;
            seit = tokio::time::Instant::now();
        } else if tokio::time::Instant::now() - seit >= ruhe {
            return jetzt;
        }
        if tokio::time::Instant::now() >= ende {
            return jetzt;
        }
    }
}
