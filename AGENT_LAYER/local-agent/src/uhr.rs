//! **Datum und Uhrzeit**, ganzzahlig und ohne Bibliothek, fuer die Uhr des
//! Agenten und fuer die Datumszeile im Auftrag.
//!
//! ⚑ **Eine Stelle.** Die Umrechnung stand bis zum 2026-09-29 in `bin/myl.rs`
//! bei der Uhr; seit der Auftrag das Datum ebenfalls nennt, wird sie an zwei
//! Stellen gebraucht, und zwei Umrechnungen liefen irgendwann auseinander.

/// **Datum und Uhrzeit in UTC** aus Sekunden seit 1970, ohne Bibliothek.
///
/// 📌 **Bis zum 2026-09-28 lieferte die Uhr nur die Sekundenzahl**
/// (`1790630712 Sekunden seit 1970`), unter dem deutschen Namen `zeit` mitten
/// in einer englischen Ansage. Ein Modell, das den Wochentag oder das Datum
/// braucht, muesste das selbst umrechnen, und das kann es nicht verlaesslich.
pub fn utc_text(sekunden: u64) -> String {
    let tage = (sekunden / 86_400) as i64;
    let rest = sekunden % 86_400;
    // Tage seit 1970 in ein Kalenderdatum (Verfahren nach H. Hinnant,
    // `civil_from_days`), ganzzahlig.
    let z = tage + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let tag = doy - (153 * mp + 2) / 5 + 1;
    let monat = if mp < 10 { mp + 3 } else { mp - 9 };
    let jahr = yoe + era * 400 + i64::from(monat <= 2);
    const WOCHENTAGE: [&str; 7] = ["Thursday", "Friday", "Saturday", "Sunday", "Monday", "Tuesday", "Wednesday"];
    format!(
        "{jahr:04}-{monat:02}-{tag:02} {:02}:{:02}:{:02} UTC, {}",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60,
        WOCHENTAGE[tage.rem_euclid(7) as usize]
    )
}

/// **Das heutige Datum als Zeile fuer den Auftrag.**
///
/// 📌 **Mehrstufige Auftraege, 30B, 2026-09-29:** Zwischen einer Unterlage
/// von 2024 und einer von 2026 verwarf das Modell die neuere mit der
/// Begruendung, das aktuelle Jahr sei 2023. Es kannte das Datum nicht; die
/// Uhr stand als Werkzeug bereit, gerufen hat es sie nicht. Agentensysteme
/// geben das Datum deshalb in den Kontext, statt auf den Aufruf zu warten.
pub fn heute_zeile(deutsch: bool) -> String {
    let jetzt = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let text = utc_text(jetzt);
    let datum = text.split(' ').next().unwrap_or_default();
    let tag = text.rsplit(", ").next().unwrap_or_default();
    if deutsch {
        format!("\n\nHeute ist {datum} ({tag}).")
    } else {
        format!("\n\nToday is {datum} ({tag}).")
    }
}

#[cfg(test)]
mod proben {
    use super::*;

    /// Die Uhr rechnet das Datum richtig, auch ueber Schaltjahre.
    #[test]
    fn die_uhr_nennt_datum_und_wochentag() {
        assert_eq!(utc_text(0), "1970-01-01 00:00:00 UTC, Thursday");
        assert_eq!(utc_text(951_782_400), "2000-02-29 00:00:00 UTC, Tuesday");
        assert_eq!(utc_text(1_790_630_712), "2026-09-28 21:25:12 UTC, Monday");
    }

    #[test]
    fn die_datumszeile_nennt_datum_und_tag() {
        let z = heute_zeile(false);
        assert!(z.starts_with("\n\nToday is 20") && z.ends_with("day)."), "{z}");
    }
}
