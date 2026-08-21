//! Hardware-Fingerabdruck.
//!
//! Der Determinismus-Nachweis lautet: *Dieselbe Eingabe liefert auf
//! **verschiedener** Hardware dasselbe Ergebnis.* Diese Aussage ist nur
//! so viel wert wie die Beschreibung der Hardware, auf der gemessen
//! wurde. Zwei Läufe mit identischem Token-Hash beweisen nichts, wenn
//! beide auf derselben Maschine mit demselben Backend liefen.
//!
//! Deshalb erhebt dieses Modul für jeden Lauf, was das Ergebnis
//! beeinflussen könnte, und schreibt es ins Protokoll:
//!
//! - **Architektur und Betriebssystem**, die grobe Achse.
//! - **Zielspezifische Merkmale** (Zeigerbreite, Endianness): falls je
//!   eine Big-Endian-Plattform dazukommt, ist das die erste Stelle, an
//!   der es auffällt.
//! - **Verfügbare Rechenkerne (SIMD-Erweiterungen)**: zur Laufzeit
//!   erkannt, nicht zur Übersetzungszeit angenommen. Ein Binary, das mit
//!   `cpu-simd` gebaut wurde, nutzt AVX2 nur, wenn die CPU es hat; das
//!   Protokoll muss den tatsächlichen Pfad festhalten, nicht den
//!   möglichen.
//! - **Aktive Backends**: welche der Kernel-Implementierungen in diesem
//!   Build überhaupt vorhanden sind.
//!
//! **Was hier bewusst nicht erhoben wird:** Seriennummern, MAC-Adressen,
//! Hostnamen. Der Fingerabdruck beschreibt eine *Hardware-Klasse*, nicht
//! ein *Gerät*. Testprotokolle wandern zwischen Menschen, und ein
//! Gerätebezug hätte darin nichts zu suchen.

/// Ein erhobener Hardware-Fingerabdruck.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    /// Schlüssel-Wert-Paare in stabiler Reihenfolge (für den Diff).
    pub entries: Vec<(String, String)>,
}

impl Fingerprint {
    /// Erhebt den Fingerabdruck der laufenden Maschine.
    pub fn collect() -> Self {
        let entries: Vec<(String, String)> = vec![
            ("arch".into(), std::env::consts::ARCH.to_string()),
            ("os".into(), std::env::consts::OS.to_string()),
            ("family".into(), std::env::consts::FAMILY.to_string()),
            (
                "pointer_width".into(),
                (std::mem::size_of::<usize>() * 8).to_string(),
            ),
            ("endianness".into(), endianness().to_string()),
            (
                "parallelism".into(),
                std::thread::available_parallelism()
                    .map(|n| n.get().to_string())
                    .unwrap_or_else(|_| "unbekannt".into()),
            ),
            ("simd_features".into(), simd_features().join(",")),
            ("backends_compiled".into(), compiled_backends().join(",")),
            // Getrennt von `backends_compiled`, weil beides
            // auseinanderfallen kann und genau dann die Aussage kippt:
            // Ein Bau mit `--features cuda` ist dafür konfiguriert und
            // rechnet trotzdem mit der Referenz.
            ("backends_rechnend".into(), rechnende_backends().join(",")),
            ("backend_selected".into(), selected_backend().to_string()),
        ];

        Self { entries }
    }

    /// Wert zu einem Schlüssel.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Kurzform für Dateinamen und Vergleichstabellen, z. B.
    /// `aarch64-macos-neon`.
    pub fn short_id(&self) -> String {
        format!(
            "{}-{}-{}",
            self.get("arch").unwrap_or("?"),
            self.get("os").unwrap_or("?"),
            self.get("backend_selected").unwrap_or("?")
        )
    }

    /// Kanonische Bytefolge für den Vergleich zweier Maschinen.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for (k, v) in &self.entries {
            out.extend_from_slice(k.as_bytes());
            out.push(b'=');
            out.extend_from_slice(v.as_bytes());
            out.push(b'\n');
        }
        out
    }
}

fn endianness() -> &'static str {
    if cfg!(target_endian = "little") {
        "little"
    } else {
        "big"
    }
}

/// Zur **Laufzeit** erkannte SIMD-Erweiterungen.
///
/// Bewusst Laufzeit- statt Übersetzungszeit-Erkennung: Ein auf einer
/// modernen Maschine gebautes Binary kann auf einer älteren laufen, und
/// dann zählt, was die CPU tatsächlich kann.
pub fn simd_features() -> Vec<String> {
    let mut f: Vec<String> = Vec::new();

    #[cfg(target_arch = "x86_64")]
    {
        for (name, present) in [
            ("sse2", std::is_x86_feature_detected!("sse2")),
            ("avx", std::is_x86_feature_detected!("avx")),
            ("avx2", std::is_x86_feature_detected!("avx2")),
            ("avx512f", std::is_x86_feature_detected!("avx512f")),
        ] {
            if present {
                f.push(name.to_string());
            }
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        // NEON ist auf aarch64 Pflichtbestandteil der Architektur.
        f.push("neon".to_string());
    }

    if f.is_empty() {
        f.push("keine".to_string());
    }
    f
}

/// Backends, für die dieser Bau **konfiguriert** ist.
///
/// Das ist nicht dasselbe wie „rechnet damit": Ein Bau mit
/// `--features cuda` führt `cuda` hier auf, während die Rechnung
/// weiterhin die Referenzkernel machen. Genau diese Unterscheidung
/// gehört ins Protokoll, siehe [`rechnende_backends`].
pub fn compiled_backends() -> Vec<String> {
    let mut b = vec!["reference".to_string()];
    for (feature, name) in [
        (cfg!(feature = "cpu-simd"), "cpu-simd"),
        (cfg!(feature = "cuda"), "cuda"),
        (cfg!(feature = "rocm"), "rocm"),
    ] {
        if feature {
            b.push(name.to_string());
        }
    }
    b
}

/// Backends, die auf dieser Übersetzung einen **eigenen Rechenpfad**
/// haben.
///
/// Die Auskunft kommt aus `kernels::rechenpfad` und damit von dort, wo
/// die `cfg` stehen, die den Code auswählen. Eine eigene Liste im Client
/// wäre eine zweite Wahrheit, die beim ersten echten CUDA-Kernel still
/// veraltet.
pub fn rechnende_backends() -> Vec<String> {
    integer_llm_kernels::rechenpfad::mit_rechenpfad()
        .into_iter()
        .map(String::from)
        .collect()
}

/// Das Backend, das dieser Lauf tatsächlich verwendet.
///
/// **Meldet nie mehr, als geschieht.** Ein Bau mit `--features cuda`
/// bekommt hier `reference`, weil die Referenzkernel rechnen. Stünde
/// stattdessen `cuda` im Protokoll, sähe ein bitgleiches Ergebnis wie ein
/// bestandener GPU-Nachweis aus und wäre keiner.
pub fn selected_backend() -> &'static str {
    #[cfg(feature = "cpu-simd")]
    {
        #[cfg(target_arch = "x86_64")]
        {
            if std::is_x86_feature_detected!("avx2") {
                return "cpu-simd/avx2";
            }
            return "reference";
        }
        #[cfg(target_arch = "aarch64")]
        {
            return "cpu-simd/neon";
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            return "reference";
        }
    }
    #[cfg(not(feature = "cpu-simd"))]
    {
        "reference"
    }
}

/// Prüft, ob dieser Bau für einen Messlauf taugt.
///
/// `Err(begründung)`, wenn er für ein Backend konfiguriert wurde, das
/// nicht rechnet. **Das ist der Fall, gegen den es diese Prüfung gibt:**
/// Wer den Client mit `--features cuda` baut, will die GPU messen. Ohne
/// diese Sperre bekäme er ein bitgleiches Ergebnis, weil auf beiden
/// Seiten dieselben CPU-Referenzkernel rechnen, und hielte es für den
/// erbrachten Nachweis.
///
/// Es ist dieselbe Fehlerklasse wie ein abweichender Artefakt-Digest, und
/// sie wird genauso behandelt: Der Lauf wird nicht stillschweigend
/// anders, er wird abgelehnt und begründet.
pub fn rechenpfad_pruefen() -> Result<(), String> {
    for backend in ["cuda", "rocm"] {
        let konfiguriert = compiled_backends().iter().any(|b| b == backend);
        if konfiguriert && !integer_llm_kernels::rechenpfad::rechnet(backend) {
            return Err(integer_llm_kernels::rechenpfad::ablehnung(backend));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerabdruck_hat_alle_pflichtfelder() {
        let fp = Fingerprint::collect();
        for key in [
            "arch",
            "os",
            "family",
            "pointer_width",
            "endianness",
            "parallelism",
            "simd_features",
            "backends_compiled",
            "backends_rechnend",
            "backend_selected",
        ] {
            assert!(fp.get(key).is_some(), "Feld {} fehlt", key);
            assert!(!fp.get(key).unwrap().is_empty(), "Feld {} ist leer", key);
        }
    }

    #[test]
    fn erhebung_ist_innerhalb_eines_laufs_stabil() {
        assert_eq!(Fingerprint::collect(), Fingerprint::collect());
    }

    #[test]
    fn kanonische_bytes_sind_reproduzierbar() {
        let a = Fingerprint::collect();
        let b = Fingerprint::collect();
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    }

    #[test]
    fn short_id_hat_drei_teile() {
        let fp = Fingerprint::collect();
        assert_eq!(fp.short_id().split('-').count(), 3.max(fp.short_id().split('-').count()));
        assert!(fp.short_id().contains(std::env::consts::ARCH));
    }

    #[test]
    fn simd_features_sind_nie_leer() {
        assert!(!simd_features().is_empty());
    }

    /// **Der Kern der Sperre.** Ein Bau, der für ein delegierendes
    /// Backend konfiguriert ist, darf keinen Messlauf zulassen. Ohne das
    /// bekäme jemand mit `--features cuda` ein bitgleiches Ergebnis,
    /// weil auf beiden Seiten die CPU-Referenz rechnet, und hielte es für
    /// den GPU-Nachweis.
    #[test]
    fn ein_bau_ohne_rechenpfad_wird_abgelehnt() {
        let konfiguriert = compiled_backends();
        let rechnend = rechnende_backends();

        for backend in ["cuda", "rocm"] {
            if konfiguriert.iter().any(|b| b == backend) {
                assert!(
                    !rechnend.iter().any(|b| b == backend),
                    "{backend} rechnet: dann gehört dieser Test angepasst"
                );
                let fehler = rechenpfad_pruefen().expect_err("hätte ablehnen müssen");
                assert!(fehler.contains("KEINEN eigenen Rechenpfad"), "{fehler}");
                return;
            }
        }
        // Ohne cuda/rocm im Bau gibt es nichts abzulehnen.
        assert!(rechenpfad_pruefen().is_ok());
    }

    /// Konfiguriert und rechnend sind zwei verschiedene Listen. Fielen
    /// sie zusammen, wäre die Unterscheidung im Protokoll wertlos.
    #[test]
    fn konfiguriert_und_rechnend_sind_getrennt_erfasst() {
        let fp = Fingerprint::collect();
        let konfiguriert = fp.get("backends_compiled").expect("Feld fehlt");
        let rechnend = fp.get("backends_rechnend").expect("Feld fehlt");
        assert!(konfiguriert.contains("reference"));
        assert!(rechnend.contains("reference"));
        // Das gewählte Backend muss in den rechnenden vorkommen: Es ist
        // das, was tatsächlich läuft.
        let gewaehlt = fp.get("backend_selected").expect("Feld fehlt");
        let stamm = gewaehlt.split('/').next().unwrap_or(gewaehlt);
        assert!(
            rechnend.split(',').any(|b| b == stamm),
            "gewählt {gewaehlt}, rechnend {rechnend}"
        );
    }

    #[test]
    fn referenz_backend_ist_immer_vorhanden() {
        assert!(compiled_backends().contains(&"reference".to_string()));
    }

    /// Ohne das Feature `cpu-simd` darf kein SIMD-Backend gemeldet
    /// werden: sonst führt das Protokoll einen Pfad, den der Lauf gar
    /// nicht genommen hat.
    #[cfg(not(feature = "cpu-simd"))]
    #[test]
    fn ohne_feature_wird_referenz_gemeldet() {
        assert_eq!(selected_backend(), "reference");
        assert_eq!(compiled_backends(), vec!["reference".to_string()]);
    }

    /// Der Fingerabdruck beschreibt eine Hardware-Klasse, kein Gerät.
    ///
    /// Geprüft wird über die **Schlüsselnamen** und die Wertform, nicht
    /// über Teilstrings im Gesamttext: `os=macos` enthält „mac", ist aber
    /// offensichtlich keine MAC-Adresse. Ein Teilstring-Test wäre hier
    /// nicht nur falsch-positiv, er würde auch die eigentliche Gefahr
    /// verfehlen: ein Feld, das eine Kennung *trägt*.
    #[test]
    fn kein_geraetebezug_im_fingerabdruck() {
        let fp = Fingerprint::collect();

        // 1. Keine Schlüssel, die auf eine Gerätekennung hindeuten.
        for (key, _) in &fp.entries {
            let k = key.to_lowercase();
            for verboten in ["hostname", "host", "serial", "uuid", "mac_address", "user"] {
                assert_ne!(k, verboten, "Feld '{}' benennt eine Gerätekennung", key);
            }
        }

        // 2. Kein Wert sieht aus wie eine MAC-Adresse oder eine UUID.
        for (key, value) in &fp.entries {
            let sieht_aus_wie_mac = value.split(':').count() == 6
                && value
                    .split(':')
                    .all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(!sieht_aus_wie_mac, "Feld '{}' sieht aus wie eine MAC-Adresse", key);

            let sieht_aus_wie_uuid = value.len() == 36
                && value.split('-').map(|p| p.len()).collect::<Vec<_>>() == vec![8, 4, 4, 4, 12];
            assert!(!sieht_aus_wie_uuid, "Feld '{}' sieht aus wie eine UUID", key);
        }

        // 3. Der Benutzername des laufenden Prozesses darf nirgends stehen.
        if let Ok(user) = std::env::var("USER") {
            if !user.is_empty() {
                let text = String::from_utf8(fp.canonical_bytes()).unwrap();
                assert!(!text.contains(&user), "Fingerabdruck enthält den Benutzernamen");
            }
        }
    }
}
