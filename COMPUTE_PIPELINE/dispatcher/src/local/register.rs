//! **Was in diesem Prozess schon laeuft**, und ob etwas daneben starten
//! darf.
//!
//! # ⚑ Wozu, im Betrieb
//!
//! Der Plan ([`crate::local::plan`]) verteilt eine Maschine, bevor etwas
//! laeuft. Im Betrieb kommt die Frage anders: Der Agent hat sein Modell
//! geladen, und jetzt soll ein Bild beschrieben oder ein Ton mitgeschrieben
//! werden, von einer fremden Laufzeit mit eigenen Gewichten. **Darf sie
//! daneben starten, ohne den Agenten zu verdraengen?**
//!
//! Ein Prozess meldet seine laufenden Lasten hier an ([`anmelden`]); die
//! Anmeldung gilt, solange ihr Wert lebt, und verschwindet mit ihm. Vor dem
//! Start einer niedrigeren Last fragt [`wache`].
//!
//! # ⚑ Was geschuetzt wird, und was nicht
//!
//! Das Freie, wie die Erhebung es misst, enthaelt den **Dateicache**, also
//! auch die per mmap gelesenen Gewichte des laufenden Agenten. Eine neue
//! Last darf davon nach dem Vorrang des Plans nicht alles nehmen:
//!
//! - **Die dichten Gewichte einer hoeheren Last bleiben geschuetzt**: Sie
//!   werden vom Freien abgezogen, denn sie liest jedes Token.
//! - **Ihre gerouteten Experten nicht, auch die heissen nicht**: Eine
//!   niedrigere Last, die hier fragt, ist ein Aufruf von Sehen oder Hoeren,
//!   und der darf sie sich leihen wie im Plan. Er verdraengt sie aus dem
//!   Cache, und der holt die haeufigen in den naechsten Token von selbst
//!   zurueck; das macht den Agenten kurz langsamer, nicht unfaehig.
//!   Gemessen in der Belegungssimulation vom 2026-10-15: Das int8-35B auf
//!   24 GiB verliert nach einem Bild rund 2 % Tempo; die heissen Experten
//!   festzuhalten hiesse dort, jedes Bild abzulehnen.
//! - Was die hoehere Last an anonymem Speicher haelt (KV, Zustand), steht
//!   ohnehin nicht im Freien.
//!
//! ⚑ **Ohne messbares Freies gibt es kein Urteil, also keine Ablehnung.**
//! Eine Wache, die auf einer Maschine ohne Auskunft alles verweigerte,
//! naehme dort den Sinnen jede Funktion, ohne etwas zu schuetzen.

use crate::local::eichung::VORGABE_ZUSCHLAG;
use crate::local::plan::{gib, Vorrang, VORGABE_RUECKLAGE};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Eine laufende Last.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Laufend {
    pub name: String,
    pub vorrang: Vorrang,
    /// Was von ihr im Speicher bleiben muss, damit sie fluessig laeuft: die
    /// dichten Gewichte.
    pub dicht_bytes: u64,
}

static LAUFEND: Mutex<Vec<(u64, Laufend)>> = Mutex::new(Vec::new());
static NAECHSTE: AtomicU64 = AtomicU64::new(1);

/// **Die Anmeldung einer laufenden Last**; sie endet, wenn dieser Wert
/// verworfen wird.
///
/// ⚑ **Eine Lebensdauer statt eines Abmeldeaufrufs**: Ein Aufruf liesse
/// sich auf einem Fehlerweg vergessen, ein Wert nicht.
#[derive(Debug)]
pub struct Anmeldung {
    kennung: u64,
}

impl Drop for Anmeldung {
    fn drop(&mut self) {
        // Ein vergifteter Riegel heisst: Ein anderer Faden brach mitten im
        // Eintragen ab. Die Liste ist trotzdem eine Liste.
        let mut l = LAUFEND.lock().unwrap_or_else(|e| e.into_inner());
        l.retain(|(k, _)| *k != self.kennung);
    }
}

/// Meldet eine laufende Last an.
pub fn anmelden(l: Laufend) -> Anmeldung {
    let kennung = NAECHSTE.fetch_add(1, Ordering::Relaxed);
    LAUFEND.lock().unwrap_or_else(|e| e.into_inner()).push((kennung, l));
    Anmeldung { kennung }
}

/// Was gerade angemeldet ist, in der Reihenfolge der Anmeldung.
pub fn laufende() -> Vec<Laufend> {
    LAUFEND.lock().unwrap_or_else(|e| e.into_inner()).iter().map(|(_, l)| l.clone()).collect()
}

/// **Ob eine neue Last daneben passt**: rein, aus dem, was laeuft, dem
/// Freien und dem Bedarf der neuen Last.
///
/// `frei` ist das Freie der Erhebung; `None` heisst nicht messbar, und dann
/// gibt es keine Ablehnung (siehe Modulkopf).
pub fn daneben(
    laufende: &[Laufend],
    frei: Option<u64>,
    name: &str,
    vorrang: Vorrang,
    braucht_bytes: u64,
    ruecklage_bytes: u64,
) -> Result<(), String> {
    let Some(frei) = frei else { return Ok(()) };
    let geschuetzt: u64 = laufende.iter().filter(|l| l.vorrang < vorrang).map(|l| l.dicht_bytes).sum();
    let bleibt = frei.saturating_sub(geschuetzt).saturating_sub(ruecklage_bytes);
    if braucht_bytes <= bleibt {
        return Ok(());
    }
    let hoehere: Vec<&str> = laufende.iter().filter(|l| l.vorrang < vorrang).map(|l| l.name.as_str()).collect();
    Err(format!(
        "{name} ({}) passt gerade nicht in den Arbeitsspeicher: braucht {}, frei sind {}, davon bleiben {} \
         für {} und {} Rücklage. Vorrang hat {}.",
        vorrang.name(),
        gib(braucht_bytes),
        gib(frei),
        gib(geschuetzt),
        if hoehere.is_empty() { "nichts".to_string() } else { hoehere.join(", ") },
        gib(ruecklage_bytes),
        if hoehere.is_empty() { "niemand".to_string() } else { "die laufende Last".to_string() },
    ))
}

/// **Fragt vor dem Start einer Last**: misst das Freie und urteilt mit
/// [`daneben`], mit der Vorgabe fuer Zuschlag und Ruecklage.
///
/// `dateien_bytes` ist die Summe der Gewichte, die die Last laden wird.
pub fn wache(name: &str, vorrang: Vorrang, dateien_bytes: u64) -> Result<(), String> {
    wache_mit(name, vorrang, dateien_bytes, VORGABE_ZUSCHLAG)
}

/// Dasselbe mit einem Zuschlag, etwa einem gemessenen
/// ([`crate::local::eichung::Eichbuch::zuschlag_fremd`]).
pub fn wache_mit(name: &str, vorrang: Vorrang, dateien_bytes: u64, zuschlag: u64) -> Result<(), String> {
    let braucht = dateien_bytes.saturating_add(zuschlag);
    daneben(&laufende(), crate::local::erhebung::speicher_frei_jetzt(), name, vorrang, braucht, VORGABE_RUECKLAGE)
}

#[cfg(test)]
mod proben {
    use super::*;
    use crate::local::erhebung::GIB;

    fn agent(dicht: u64) -> Laufend {
        Laufend { name: "raven-35b-a3b-int8".into(), vorrang: Vorrang::Agent, dicht_bytes: dicht }
    }

    /// ⚑ **Die dichten Gewichte der hoeheren Last bleiben**: Was daneben
    /// startet, bekommt nur das Freie ohne sie und ohne Ruecklage.
    #[test]
    fn die_dichten_gewichte_des_agenten_bleiben() {
        let l = [agent(3 * GIB)];
        assert!(daneben(&l, Some(10 * GIB), "Sehen", Vorrang::Vision, 6 * GIB, GIB).is_ok());
        let nein = daneben(&l, Some(10 * GIB), "Sehen", Vorrang::Vision, 6 * GIB + 1, GIB).expect_err("zu gross");
        assert!(nein.contains("raven-35b-a3b-int8") && nein.contains("Vorrang"), "{nein}");
    }

    /// ⚑ **Nur hoehere Lasten sind geschuetzt**: Eine gleich wichtige oder
    /// niedrigere, die laeuft, nimmt der neuen nichts weg.
    #[test]
    fn nur_hoeherer_vorrang_ist_geschuetzt() {
        let sprache = Laufend { name: "hoeren".into(), vorrang: Vorrang::Sprache, dicht_bytes: 5 * GIB };
        assert!(daneben(std::slice::from_ref(&sprache), Some(6 * GIB), "Sehen", Vorrang::Vision, 5 * GIB, GIB).is_ok());
        // Umgekehrt schuetzt Sehen sich vor dem Hoeren.
        let sehen = Laufend { name: "sehen".into(), vorrang: Vorrang::Vision, dicht_bytes: 5 * GIB };
        assert!(daneben(&[sehen], Some(6 * GIB), "Hoeren", Vorrang::Sprache, 1, GIB).is_err());
    }

    /// ⚑ **Ohne messbares Freies keine Ablehnung.**
    #[test]
    fn ohne_auskunft_kein_urteil() {
        assert!(daneben(&[agent(100 * GIB)], None, "Sehen", Vorrang::Vision, 100 * GIB, GIB).is_ok());
    }

    /// ⚑ **Eine Anmeldung gilt, solange ihr Wert lebt.**
    #[test]
    fn die_anmeldung_endet_mit_ihrem_wert() {
        let name = format!("probe-{}", std::process::id());
        let gibt = |n: &str| laufende().iter().any(|l| l.name == n);
        {
            let _a = anmelden(Laufend { name: name.clone(), vorrang: Vorrang::Agent, dicht_bytes: 1 });
            assert!(gibt(&name));
        }
        assert!(!gibt(&name), "die Anmeldung blieb nach ihrem Wert stehen");
    }
}
