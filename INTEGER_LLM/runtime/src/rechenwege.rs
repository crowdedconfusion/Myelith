//! Welche Rechenwege auf dieser Maschine wirklich rechnen, und wie sich
//! einer abschalten laesst.
//!
//! # ⚑ Warum das hier steht und nicht beim Aufrufer
//!
//! Dieselbe Ueberlegung wie bei [`crate::kapazitaet`]: **Ein Aufrufer
//! soll die Kernkiste nicht kennen muessen.** Der Client zeigt je
//! gefundenem Rechenwerk einen Regler und muss dafuer wissen, ob ueber
//! dieses Geraet ueberhaupt ein Rechenweg fuehrt. Muesste er dafuer
//! `integer-llm-kernels` in sein eigenes Manifest schreiben, waere die
//! Naht keine.
//!
//! # ⚑ Gefragt wird, nicht wiederholt
//!
//! **Die Bedingung steht an einer Stelle**, naemlich dort, wo der Code
//! ausgewaehlt wird. Dieses Modul reicht sie durch und formuliert sie
//! nicht neu. Der Grund steht ausgeschrieben im Kopf von
//! `kernels/src/rechenpfad.rs`: Eine zweite Fassung derselben Bedingung
//! lief dort schon einmal auseinander und meldete einen Rechenpfad, den
//! es nicht gab.
//!
//! # ⚠️ Was „abschalten" heisst und was nicht
//!
//! **Drei Rechenwege haben einen Schalter**: `metal`
//! ([`integer_llm_kernels::ressourcen::metal::schwelle_setzen`]) und seit dem
//! 2026-10-14 `cuda` und `rocm`
//! ([`integer_llm_kernels::ressourcen::karten::hersteller_setzen`]), aber nur in
//! einer Uebersetzung mit dem jeweiligen Feature. Jeder nimmt seine Karten
//! aus dem Pfad, ohne ein Ergebnis zu aendern. [`setzen`] sagt mit seinem
//! Rueckgabewert, ob es den Schalter gab; **ein stilles Ja waere die
//! Zusage, etwas getan zu haben, das nicht geschehen ist.**

use std::sync::OnceLock;

/// Rechenwege, die auf dieser Uebersetzung und dieser Maschine einen
/// eigenen Rechenpfad haben.
///
/// `reference` steht immer darin, `cpu-simd` wenn vektorisiert wird,
/// `metal` wenn Geraet, Shader und Selbstpruefung gegen die CPU tragen.
pub fn vorhanden() -> Vec<&'static str> {
    integer_llm_kernels::rechenpfad::mit_rechenpfad()
}

/// Rechnet dieser Rechenweg hier selbst?
pub fn rechnet(backend: &str) -> bool {
    integer_llm_kernels::rechenpfad::rechnet(backend)
}

/// Die Schwelle, mit der `metal` in diesen Prozess gestartet ist.
///
/// ⚑ **Einmal gelesen, bevor irgendjemand sie setzt.** Ohne sie waere
/// das Wiedereinschalten ein Raten: `schwelle_setzen(0)` verwirft den
/// Wert, und die Vorgabe wieder hinzuschreiben ueberginge ein gesetztes
/// `MYL_METAL_AB`. **Ein Schalter muss den Zustand wiederherstellen,
/// den er vorgefunden hat**, sonst ist Ausschalten und wieder
/// Einschalten keine Nullbewegung.
static METALL_AUSGANG: OnceLock<usize> = OnceLock::new();

/// **Nimmt einen Rechenweg aus dem Pfad oder holt ihn zurueck.**
///
/// Gibt zurueck, ob dieser Rechenweg ueberhaupt einen Schalter hat.
/// `false` heisst: Es ist nichts geschehen, und der Aufrufer darf nicht
/// so tun als ob.
///
/// ⚑ **Es aendert kein Ergebnis, nur wo gerechnet wird.** Der Beleg ist
/// der Konformitaetslauf ueber `reference`, `cpu-simd` und `metal`: Alle
/// drei tragen denselben Digest.
pub fn setzen(backend: &str, an: bool) -> bool {
    match backend {
        "metal" => {
            let ausgang = *METALL_AUSGANG.get_or_init(integer_llm_kernels::ressourcen::metal::schwelle);
            integer_llm_kernels::ressourcen::metal::schwelle_setzen(if an { ausgang } else { 0 });
            true
        }
        "cuda" | "rocm" => {
            use integer_llm_kernels::ressourcen::karten::{hersteller_setzen, uebersetzt_fuer, Hersteller};
            let h = if backend == "cuda" { Hersteller::Nvidia } else { Hersteller::Amd };
            if !uebersetzt_fuer(h) {
                return false;
            }
            hersteller_setzen(h, an);
            true
        }
        _ => false,
    }
}

/// **Schaltet eine einzelne Karte (CUDA, ROCm) ab oder wieder an**, ueber
/// ihre PCI-Adresse. Siehe [`integer_llm_kernels::ressourcen::karten::karte_setzen`].
///
/// ⚑ **Je Karte, weil der Client je Karte freigibt.** Gibt zurueck, ob
/// eine bereite Karte diese Adresse traegt.
pub fn karte_setzen(pci: &str, an: bool) -> bool {
    integer_llm_kernels::ressourcen::karten::karte_setzen(pci, an)
}

#[cfg(test)]
mod proben {
    use super::*;

    /// Die Referenz ist der Massstab und rechnet immer.
    #[test]
    fn die_referenz_ist_immer_dabei() {
        assert!(vorhanden().contains(&"reference"));
        assert!(rechnet("reference"));
    }

    /// ⚑ **Ein Rechenweg ohne Schalter sagt das.** `cuda` und `rocm`
    /// haben einen genau dann, wenn ihr Feature uebersetzt ist; sonst
    /// kommt ein Nein und keine stille Zusage.
    #[test]
    fn ohne_schalter_kommt_ein_nein() {
        use integer_llm_kernels::ressourcen::karten::{hersteller_an, uebersetzt_fuer, Hersteller};
        for (weg, h) in [("cuda", Hersteller::Nvidia), ("rocm", Hersteller::Amd)] {
            let vorher = hersteller_an(h);
            assert_eq!(setzen(weg, false), uebersetzt_fuer(h), "{weg} meldet den Schalter falsch");
            if uebersetzt_fuer(h) {
                assert!(!hersteller_an(h), "{weg} ist nicht aus");
                assert!(setzen(weg, vorher));
            }
            assert_eq!(hersteller_an(h), vorher, "{weg}: der Ausgangszustand ist nicht zurueck");
        }
        assert!(!setzen("reference", false), "die Referenz laesst sich nicht abschalten");
        assert!(!setzen("gibt-es-nicht", true));
    }

    /// ⚑ **Aus und wieder an ist eine Nullbewegung.** Die Schwelle, mit
    /// der der Prozess gestartet ist, steht danach wieder da; sonst
    /// haette ein Regler, den jemand hin und her schiebt, die
    /// Einstellung stillschweigend geaendert.
    ///
    /// 📌 **Diese Pruefung aendert einen Zustand des ganzen Prozesses**,
    /// und `cargo test` laeuft nebenlaeufig. Sie ist trotzdem allein
    /// und braucht keinen Riegel: Die Schwelle entscheidet nur, **wo**
    /// gerechnet wird, nie **was** herauskommt, und keine zweite
    /// Pruefung dieser Kiste behauptet etwas ueber die GPU. Wer eine
    /// schreibt, die es tut, legt sie mit dieser zusammen.
    #[test]
    fn aus_und_wieder_an_stellt_die_schwelle_her() {
        let vorher = integer_llm_kernels::ressourcen::metal::schwelle();
        assert!(setzen("metal", false), "metal hat einen Schalter");
        assert_eq!(integer_llm_kernels::ressourcen::metal::schwelle(), 0, "die GPU ist nicht aus");
        assert!(setzen("metal", true));
        assert_eq!(
            integer_llm_kernels::ressourcen::metal::schwelle(),
            vorher,
            "die Ausgangsschwelle ist nicht zurueckgekommen"
        );
    }
}
