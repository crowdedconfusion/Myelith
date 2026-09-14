//! Welche Backends auf dieser Übersetzung wirklich rechnen.
//!
//! ## Der Befund, der dieses Modul erzwungen hat (2026-08-22)
//!
//! `conformance/run.sh cuda` meldete auf einem Mac ohne NVIDIA-Hardware
//! und ohne CUDA-Laufzeit **30/30 bestanden**. Der Prüflauf zertifizierte
//! also die Referenzimplementierung unter dem Etikett „cuda". Die Kette:
//!
//! - `run.sh` baut mit `--features cuda`.
//! - Die Features in `Cargo.toml` sind **alle leer** (`cuda = []`), sie
//!   schalten nur, ob `backends/cuda.rs` übersetzt wird.
//! - Der Rechenpfad (`linear.rs`, `mlp.rs`, `rmsnorm.rs`, …) enthält
//!   keine einzige `cfg(feature = "cuda")`. Die einzige echte Weiche im
//!   ganzen Rechenpfad steht in `dot.rs` und gilt `cpu-simd`.
//! - `golden_runner` nahm den Backend-Namen entgegen und verwarf ihn
//!   (`let _backend_name = &args[2];`).
//!
//! **Es ist derselbe Fehler, den der Kopf von `run.sh` für behoben
//! erklärt.** Dort steht, bis 2026-08-19 sei der Parameter nur ausgegeben
//! und dann ignoriert worden, womit sich der Prüflauf ausschließlich
//! selbst zertifizieren konnte. Die Behebung reichte den Parameter in den
//! Cargo-Aufruf. Da er die Rechnung nie erreichte, blieb die
//! Selbstzertifizierung bestehen und trug nur ein überzeugenderes
//! Etikett.
//!
//! ## Warum eine Liste und keine Prüfung zur Laufzeit
//!
//! Ob ein Backend rechnet, entscheidet sich beim **Übersetzen**: Es hängt
//! an Feature-Flags und an der Zielarchitektur. Zur Laufzeit ließe sich
//! das nur erraten. Die Liste **fragt deshalb dort nach, wo der Code
//! ausgewählt wird**, statt die Bedingung zu wiederholen: Wer einen
//! echten CUDA-Pfad schreibt, trägt ihn hier ein, und erst dann besteht
//! ein CUDA-Prüflauf.
//!
//! ## Fund 34 (2026-08-22): Diese Datei hatte denselben Fehler
//!
//! Bis zu diesem Datum stand die Bedingung für `cpu-simd` hier **noch
//! einmal**, als `any(target_arch = "x86_64", target_arch = "aarch64")`.
//! `dot.rs` vektorisiert aber nur unter `aarch64`. Auf x86_64 meldete
//! dieses Modul damit genau das, wogegen es geschrieben wurde: einen
//! Rechenpfad, den es nicht gibt. Der zugehörige Test prüfte gegen
//! dieselbe wiederholte Bedingung und konnte den Widerspruch nicht
//! finden.
//!
//! Die Bedingung steht jetzt einmal, am `cfg` von `dot::gewaehlt`;
//! `mit_rechenpfad` liest `dot::VEKTORISIERT`. Messung und Tragweite im
//! Kopf von `dot.rs`.
//!
//! ## ⚑ Die eine Ausnahme: `metal` fragt auch die Maschine
//!
//! Ob die GPU rechnet, entscheidet sich **nicht allein beim Übersetzen**.
//! Ein Bau mit `--features metal` kann auf einer Maschine ohne GPU laufen,
//! auf einem System ohne Shadersprache 4.0, oder auf einer GPU, deren
//! `matmul2d` anders rechnet als die CPU. In allen drei Fällen rechnet die
//! CPU, und `metal` darf dann nicht in der Liste stehen. Gefragt wird
//! deshalb `metal::verfuegbar`, und das heißt: Gerät da, Shader übersetzt,
//! Selbstprüfung gegen die CPU bestanden.
//!
//! ## Was das Modul nicht behauptet
//!
//! Es sagt **nicht**, dass ein Backend richtig rechnet. Dafür sind die
//! Golden Vectors da. Es sagt nur, dass überhaupt eigener Code läuft, und
//! verhindert damit die eine Aussage, die schlimmer ist als gar keine:
//! ein bestandener Prüflauf über einen Pfad, den es nicht gibt.

/// Backends, die auf **dieser** Übersetzung einen eigenen Rechenpfad
/// haben.
///
/// `reference` steht immer darin: Sie ist der Pfad, gegen den alles
/// andere geprüft wird.
///
/// `cpu-simd` steht darin, wenn `dot.rs` auf dieser Übersetzung
/// tatsächlich vektorisiert, und sonst nicht. **Gefragt wird dort, nicht
/// hier** (`dot::VEKTORISIERT`): Diese Datei hatte die Bedingung eigenständig
/// wiederholt, sie lief auseinander, und daraus wurde Fund 34. Der
/// Modulkopf von `dot.rs` führt die Messung.
///
/// `cuda` und `rocm` stehen bewusst **nicht** darin. Ihre Umsetzungen in
/// `backends/` delegieren jede Operation an die Referenzkernel; sie sind
/// Platzhalter mit dokumentierter Absicht, kein Rechenpfad.
pub fn mit_rechenpfad() -> Vec<&'static str> {
    let mut vorhanden = vec!["reference"];
    if crate::dot::VEKTORISIERT {
        vorhanden.push("cpu-simd");
    }
    if crate::metal::verfuegbar() {
        vorhanden.push("metal");
    }
    vorhanden
}

/// Backends, für die **diese Übersetzung konfiguriert** ist, ob sie
/// rechnen oder nicht.
///
/// ⚑ **Hier und nicht beim Aufrufer** (2026-09-14): Wer die Features
/// einschaltet, ist nicht mehr nur der eigene Bau. Der Testclient bekommt
/// `cpu-simd` und `metal` über seine Abhängigkeiten, und ein
/// `cfg!(feature = ...)` im Testclient sähe davon nichts. Gefragt wird
/// deshalb die Kiste, deren Features es sind.
pub fn uebersetzt() -> Vec<&'static str> {
    let mut b = vec!["reference"];
    for (an, name) in [
        (cfg!(feature = "cpu-simd"), "cpu-simd"),
        (cfg!(feature = "metal"), "metal"),
        (cfg!(feature = "cuda"), "cuda"),
        (cfg!(feature = "rocm"), "rocm"),
    ] {
        if an {
            b.push(name);
        }
    }
    b
}

/// Rechnet dieses Backend auf dieser Übersetzung selbst?
pub fn rechnet(backend: &str) -> bool {
    mit_rechenpfad().contains(&backend)
}

/// Die Begründung, warum ein Backend nicht zertifiziert werden kann.
///
/// **Ausformuliert und nicht nur ein Fehlercode**, aus demselben Grund
/// wie die Digest-Meldung im Testclient: Ohne den Satz sähe ein
/// abgelehnter Lauf wie ein technisches Problem aus, und jemand würde ihn
/// umgehen. Er ist aber kein Problem, sondern das Ergebnis.
pub fn ablehnung(backend: &str) -> String {
    let bekannt = ["reference", "cpu-simd", "metal", "cuda", "rocm"];
    if !bekannt.contains(&backend) {
        return format!(
            "Unbekanntes Backend {backend:?}. Bekannt sind: {}.",
            bekannt.join(", ")
        );
    }
    if backend == "metal" {
        return format!(
            "Backend \"metal\" rechnet in diesem Prozess NICHT.\n\
             Vorhanden sind: {}.\n\
             \n\
             Moegliche Gruende: ohne `--features metal` gebaut, nicht auf macOS,\n\
             keine Metal-GPU, Shadersprache 4.0 fehlt, oder die GPU hat die\n\
             Selbstpruefung gegen die CPU nicht bestanden (die Meldung dazu steht\n\
             weiter oben). Ein Prueflauf darueber zertifizierte die CPU unter dem\n\
             Namen der GPU.",
            mit_rechenpfad().join(", ")
        );
    }
    format!(
        "Backend {backend:?} hat auf dieser Übersetzung KEINEN eigenen Rechenpfad.\n\
         Vorhanden sind: {}.\n\
         \n\
         Ein Prüflauf darüber würde die Referenzimplementierung unter fremdem\n\
         Namen zertifizieren. Das Ergebnis wäre kein Backend-Nachweis, sondern\n\
         ein Nachweis über die Referenz, und er sähe genau wie ein bestandener\n\
         Backend-Nachweis aus.\n\
         \n\
         Bei {backend:?} liegt das daran, dass die Umsetzung in kernels/src/backends/\n\
         jede Operation an die Referenzkernel weiterreicht (Delegations-Stub).\n\
         Sobald dort echte Kernel stehen, gehört das Backend in\n\
         kernels/src/rechenpfad.rs, und erst dann besteht dieser Lauf.",
        mit_rechenpfad().join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Die Referenz ist der Maßstab und muss immer verfügbar sein.
    #[test]
    fn die_referenz_rechnet_immer() {
        assert!(rechnet("reference"));
        assert!(mit_rechenpfad().contains(&"reference"));
    }

    /// **Der Kern des Befunds.** Solange die Umsetzungen in `backends/`
    /// delegieren, darf kein Prüflauf sie zertifizieren. Fällt dieser
    /// Test eines Tages, ist das richtig: Dann gibt es echte Kernel, und
    /// wer sie schreibt, ändert ihn mit.
    #[test]
    fn delegierende_backends_werden_abgelehnt() {
        for backend in ["cuda", "rocm"] {
            assert!(
                !rechnet(backend),
                "{backend} gilt als Rechenpfad, obwohl es delegiert"
            );
            let text = ablehnung(backend);
            assert!(
                text.contains("KEINEN eigenen Rechenpfad"),
                "Ablehnung nennt den Grund nicht: {text}"
            );
            assert!(
                text.contains("Delegations-Stub"),
                "Ablehnung sagt nicht, woran es liegt: {text}"
            );
        }
    }

    /// **Fund 34 als Test.** Die Meldung über `cpu-simd` muss dem
    /// folgen, was `dot.rs` wirklich tut, und nicht einer eigenen
    /// Bedingung. Vorher stand hier `cfg!(all(feature = "cpu-simd",
    /// any(x86_64, aarch64)))`, also eine Wiederholung der Bedingung aus
    /// `dot.rs`, und weil dort nur `aarch64` vektorisiert, meldete
    /// dieser Prüfstand auf x86_64 einen Rechenpfad, den es nicht gibt.
    /// Der Test bestätigte das, weil er gegen dieselbe falsche Bedingung
    /// prüfte wie der Code.
    #[test]
    fn cpu_simd_gilt_nur_wo_dot_vektorisiert() {
        assert_eq!(
            rechnet("cpu-simd"),
            crate::dot::VEKTORISIERT,
            "cpu-simd wird anders gemeldet, als dot.rs es auswählt"
        );
    }

    /// Der Fall, der Fund 34 war: Das Feature ist gesetzt, der
    /// vektorisierte Pfad fehlt für dieses Ziel. Dann darf die Liste
    /// nichts außer der Referenz enthalten.
    #[test]
    fn feature_ohne_vektorpfad_bleibt_ohne_eintrag() {
        if cfg!(feature = "cpu-simd") && !crate::dot::VEKTORISIERT {
            assert_eq!(
                mit_rechenpfad(),
                vec!["reference"],
                "Feature gesetzt, aber dot.rs rechnet skalar: dann gibt es \
                 nur einen Rechenpfad"
            );
        }
    }

    /// **`metal` steht genau dann in der Liste, wenn die GPU rechnen
    /// darf**: Feature, Geraet, Shader und Selbstpruefung. Ohne das
    /// Feature ist die Antwort immer nein.
    #[test]
    fn metal_gilt_nur_mit_bereiter_gpu() {
        assert_eq!(rechnet("metal"), crate::metal::verfuegbar());
        if !cfg!(all(feature = "metal", target_os = "macos")) {
            assert!(!rechnet("metal"), "metal ohne Feature gemeldet");
        }
        let text = ablehnung("metal");
        assert!(!text.contains("Delegations-Stub"), "{text}");
        assert!(text.contains("Selbstpruefung"), "{text}");
    }

    /// **Was rechnet, ist auch übersetzt**, und die Referenz steht in
    /// beiden Listen.
    #[test]
    fn jeder_rechnende_weg_ist_uebersetzt() {
        let uebersetzt = uebersetzt();
        assert_eq!(uebersetzt[0], "reference");
        for weg in mit_rechenpfad() {
            assert!(uebersetzt.contains(&weg), "{weg} rechnet, ist aber nicht übersetzt");
        }
        assert_eq!(uebersetzt.contains(&"cpu-simd"), cfg!(feature = "cpu-simd"));
        assert_eq!(uebersetzt.contains(&"metal"), cfg!(feature = "metal"));
    }

    /// Ein Tippfehler im Backend-Namen darf nicht wie ein fehlender
    /// Rechenpfad aussehen: Das sind zwei verschiedene Fehler.
    #[test]
    fn unbekannte_namen_werden_als_solche_gemeldet() {
        let text = ablehnung("cudaa");
        assert!(text.contains("Unbekanntes Backend"), "{text}");
        assert!(!text.contains("Delegations-Stub"), "{text}");
    }
}
