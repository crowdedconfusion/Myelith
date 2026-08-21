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
//! das nur erraten. Die Liste entsteht deshalb aus denselben `cfg`, die
//! auch den Code auswählen, und steht neben ihm: Wer einen echten
//! CUDA-Pfad schreibt, trägt ihn hier ein, und erst dann besteht ein
//! CUDA-Prüflauf.
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
/// `cpu-simd` nur mit Feature **und** passender Architektur. Ohne die
/// Architekturbedingung wäre die Angabe falsch: Auf einem Ziel ohne NEON
/// und ohne AVX2 übersetzt das Feature zwar, aber `dot.rs` fällt auf die
/// skalare Fassung zurück, und dann rechnet nichts anderes als bei
/// `reference`.
///
/// `cuda` und `rocm` stehen bewusst **nicht** darin. Ihre Umsetzungen in
/// `backends/` delegieren jede Operation an die Referenzkernel; sie sind
/// Platzhalter mit dokumentierter Absicht, kein Rechenpfad.
pub fn mit_rechenpfad() -> Vec<&'static str> {
    // `cfg!` statt `#[cfg]`: Der Zweig steht dann immer im Code und wird
    // nur zur Übersetzungszeit als wahr oder falsch ausgewertet. Mit dem
    // Attribut verschwände der `push` ganz, und `mut` wäre ohne
    // `cpu-simd` eine Warnung.
    let simd = cfg!(all(
        feature = "cpu-simd",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ));

    let mut vorhanden = vec!["reference"];
    if simd {
        vorhanden.push("cpu-simd");
    }
    vorhanden
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
    let bekannt = ["reference", "cpu-simd", "cuda", "rocm"];
    if !bekannt.contains(&backend) {
        return format!(
            "Unbekanntes Backend {backend:?}. Bekannt sind: {}.",
            bekannt.join(", ")
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

    /// `cpu-simd` hängt an Feature **und** Architektur. Der Test prüft
    /// beide Richtungen gegen dieselben Bedingungen, unter denen `dot.rs`
    /// seine vektorisierte Fassung wählt.
    #[test]
    fn cpu_simd_haengt_an_feature_und_architektur() {
        let erwartet = cfg!(all(
            feature = "cpu-simd",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ));
        assert_eq!(
            rechnet("cpu-simd"),
            erwartet,
            "cpu-simd wird anders gemeldet, als dot.rs es auswählt"
        );
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
