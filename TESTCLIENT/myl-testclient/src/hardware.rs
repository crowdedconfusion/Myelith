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
//!   erkannt, nicht zur Übersetzungszeit angenommen. Das ist eine
//!   Auskunft über die **CPU** und steht als solche im Protokoll.
//!
//!   **Nicht zu verwechseln mit dem genommenen Pfad** (Fund 34): Hier
//!   stand einmal, ein Bau mit `cpu-simd` nutze AVX2, sobald die CPU es
//!   habe. Das stimmte nie. Ob vektorisiert gerechnet wird, entscheidet
//!   allein `kernels/src/dot.rs`, und dort gibt es bisher nur NEON für
//!   aarch64. `simd_features` sagt, was die Maschine könnte;
//!   [`selected_backend`] sagt, was der Lauf getan hat. Die zweite
//!   Auskunft aus der ersten abzuleiten, war der Fehler.
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
    /// Sie bestimmen den Fingerabdruck, siehe [`Self::canonical_bytes`].
    pub entries: Vec<(String, String)>,
    /// Beschreibende Angaben zur Maschine (CPU-Modell, Speicher,
    /// Virtualisierung, GPU-Karte). Sie stehen im Protokoll, damit ein
    /// veröffentlichter Nachweis sagt, **welche** Maschine gemessen hat —
    /// aber sie gehen bewusst NICHT in den Fingerabdruck ein: Zwei
    /// baugleiche Mietmaschinen unterscheiden sich in keiner dieser
    /// Angaben, und der Nachweis verlangt verschiedene Fingerabdrücke.
    pub beschreibung: Vec<(String, String)>,
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

        // Die Beschreibung ist kein Teil des Fingerabdrucks: Sie benennt
        // die Maschine, nicht die Hardware-Klasse, und sie darf zwischen
        // zwei baugleichen Maschinen keinen Unterschied machen.
        let beschreibung: Vec<(String, String)> = vec![
            ("cpu_modell".into(), cpu_modell()),
            ("ram_bytes".into(), ram_bytes().to_string()),
            ("virtualisierung".into(), virtualisierung()),
            ("gpu_karte".into(), gpu_karte()),
        ];

        Self { entries, beschreibung }
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

    /// Kanonische Bytefolge für den Vergleich zweier **Maschinen**.
    ///
    /// Deckt **nur** die Felder aus [`MASCHINENFELDER`] ab. Nicht die
    /// [`Self::beschreibung`], denn zwei identische Mietkisten müssen
    /// denselben Fingerabdruck tragen, sonst hielte der Vergleich zwei
    /// gleiche Architekturen für zwei verschiedene und gäbe ein Urteil,
    /// das nichts belegt. **Und seit dem 2026-08-30 auch nicht die
    /// Felder aus [`RECHENPFADFELDER`]**, siehe dort.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        self.bytes_ueber(&MASCHINENFELDER)
    }

    /// Kanonische Bytefolge des **Rechenpfads**, also des Baus.
    ///
    /// Getrennt vom Maschinen-Fingerabdruck, weil beide verschiedene
    /// Fragen beantworten: „Ist das eine andere Maschine?" und „Ist das
    /// ein anderer Rechenweg?". Der Cross-Hardware-Nachweis hängt an der
    /// ersten, der Backend-Vergleich innerhalb einer Maschine an der
    /// zweiten.
    pub fn rechenpfad_bytes(&self) -> Vec<u8> {
        self.bytes_ueber(&RECHENPFADFELDER)
    }

    fn bytes_ueber(&self, felder: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        // Über die Feldliste laufen, nicht über `entries`: Die Reihenfolge
        // soll an der Liste hängen und nicht daran, in welcher Reihenfolge
        // `collect` die Werte erhebt.
        for feld in felder {
            let Some(wert) = self.get(feld) else { continue };
            out.extend_from_slice(feld.as_bytes());
            out.push(b'=');
            out.extend_from_slice(wert.as_bytes());
            out.push(b'\n');
        }
        out
    }
}

/// Die Felder, die die **Maschine** beschreiben. Nur sie tragen den
/// Cross-Hardware-Nachweis.
///
/// ⚑ **Fund 105 (2026-08-30): Der Nachweis ließ sich auf einer einzigen
/// Maschine erzeugen.** Bis zu diesem Tag lief `canonical_bytes` über
/// **alle** Einträge, also auch über `backends_compiled`,
/// `backends_rechnend` und `backend_selected`. Die beschreiben aber den
/// **Bau**, nicht die Maschine. Wer denselben Client ein zweites Mal mit
/// `--features cpu-simd` übersetzte und den Lauf wiederholte, bekam einen
/// zweiten Fingerabdruck, gleiche Vergleichswerte, und `vergleich`
/// urteilte:
///
/// > Urteil: NACHWEIS
/// > Die Fingerabdrücke unterscheiden sich, die Vergleichswerte stimmen
/// > überein. Das ist der Cross-Hardware-Determinismus-Nachweis für diese
/// > Einstellung.
///
/// Gemessen am 2026-08-30 auf einem einzigen MacBook, zwei Bauten,
/// dieselbe CPU. Der Modulkopf von `vergleich` nennt genau das die
/// gefährlichste Eigenschaft, die dieses Werkzeug haben kann: „Ein
/// Werkzeug, das einen Nachweis vortäuscht, ist schlimmer als keines,
/// weil sein Ergebnis geglaubt wird."
pub const MASCHINENFELDER: [&str; 7] = [
    "arch",
    "os",
    "family",
    "pointer_width",
    "endianness",
    "parallelism",
    "simd_features",
];

/// Die Felder, die den **Bau** beschreiben.
///
/// Ein Unterschied hier ist ein echter Befund, aber ein anderer: Zwei
/// Rechenpfade auf **derselben** Maschine, die bitgleich rechnen, sind
/// der Backend-Vergleich (TESTCLIENT 2.2) und nicht der
/// Cross-Hardware-Nachweis. `vergleich` unterscheidet beides seit dem
/// 2026-08-30 im Urteil.
pub const RECHENPFADFELDER: [&str; 3] = [
    "backends_compiled",
    "backends_rechnend",
    "backend_selected",
];

/// Kennzeichnet, **wie** der Fingerabdruck gebildet wurde.
///
/// Steht im Protokoll und wird vor jedem Urteil verglichen. Ohne diese
/// Marke wäre ein Protokoll von vor dem 2026-08-30 von einem danach nicht
/// zu unterscheiden: Beide tragen ein Feld `fingerprint_sha256`, aber es
/// deckt verschiedene Mengen ab, und derselbe Rechner ergäbe zwei
/// verschiedene Werte. Genau daraus entstünde wieder Fund 105, nur über
/// zwei Client-Fassungen statt über zwei Bauten.
pub const FINGERABDRUCK_SCHEMA: &str = "maschine/1";

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
    // **Gefragt wird der Code, nicht der Prozessor** (Fund 34).
    // Hier stand `is_x86_feature_detected!("avx2")`, und bei einem Treffer
    // ging `cpu-simd/avx2` ins Protokoll. Das war eine Auskunft darüber,
    // was die CPU kann, nicht darüber, was wir rechnen: `kernels/src/dot.rs`
    // vektorisiert bis heute nur unter aarch64. Ein Protokoll von einer
    // x86_64-Maschine hätte `cpu-simd/avx2` getragen und dabei denselben
    // skalaren Code ausgeführt wie ein Lauf mit `reference`.
    if integer_llm_kernels::dot::VEKTORISIERT {
        // Der einzige vektorisierte Pfad, den es gibt. Kommt AVX2 dazu,
        // gehört hier eine Fallunterscheidung nach `target_arch` hin, und
        // dann trägt sie auch etwas aus.
        "cpu-simd/neon"
    } else {
        "reference"
    }
}

// ---------------------------------------------------------------------------
// Beschreibung der Maschine (geht ins Protokoll, NICHT in den Fingerabdruck)
// ---------------------------------------------------------------------------

/// Liest einen `sysctl`-Wert als Zeichenkette. `None`, wenn der Wert
/// fehlt oder leer ist: Ein leerer String wäre eine Angabe, die keine ist.
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn sysctl_wert(key: &str) -> Option<String> {
    let out = std::process::Command::new("sysctl")
        .args(["-n", key])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Das CPU-Modell als lesbare Angabe.
///
/// Best-Effort je Betriebssystem; wo es nicht sicher ermittelbar ist,
/// steht „unbekannt" statt eines geratenen Werts.
fn cpu_modell() -> String {
    if cfg!(target_os = "macos") {
        #[cfg(target_os = "macos")]
        {
            // Apple Silicon meldet die Marke nicht immer über
            // `machdep.cpu.brand_string`; dann benennt `hw.model` das
            // Board, das ist besser als nichts.
            if let Some(s) = sysctl_wert("machdep.cpu.brand_string") {
                return s;
            }
            return sysctl_wert("hw.model").unwrap_or_else(|| "unbekannt".into());
        }
    }
    if cfg!(target_os = "linux") {
        if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
            for zeile in cpuinfo.lines() {
                if let Some(rest) = zeile.strip_prefix("model name") {
                    if let Some(wert) = rest.split(':').nth(1) {
                        let wert = wert.trim();
                        if !wert.is_empty() {
                            return wert.to_string();
                        }
                    }
                }
            }
        }
        return "unbekannt".into();
    }
    if cfg!(target_os = "windows") {
        // Kein `wmic` (auf neuen Windows-Versionen entfernt) und kein
        // Ratespiel: Der Prozessor-Identifikator ist eine ehrliche,
        // wenn auch grobe Angabe.
        return std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "unbekannt".into());
    }
    "unbekannt".into()
}

/// Der Arbeitsspeicher in Bytes, 0 wenn nicht ermittelbar.
fn ram_bytes() -> u64 {
    if cfg!(target_os = "macos") {
        #[cfg(target_os = "macos")]
        {
            if let Some(s) = sysctl_wert("hw.memsize") {
                if let Ok(n) = s.parse::<u64>() {
                    return n;
                }
            }
            return 0;
        }
    }
    if cfg!(target_os = "linux") {
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            for zeile in meminfo.lines() {
                if let Some(rest) = zeile.strip_prefix("MemTotal:") {
                    // `MemTotal:   16384 kB` — der Wert ist in KiB.
                    let zahl: String = rest
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    if let Ok(kib) = zahl.parse::<u64>() {
                        return kib.saturating_mul(1024);
                    }
                }
            }
        }
        return 0;
    }
    0
}

/// Läuft dieser Bau in einer Virtualisierung?
///
/// Die Angabe gehört in den Beleg, weil eine gemietete Maschine eine
/// andere Aussage trägt als die eigene: Ein Determinismus-Nachweis über
/// zwei VMs desselben Hosts ist weniger wert als einer über zwei
/// physische Architekturen.
fn virtualisierung() -> String {
    if cfg!(target_os = "macos") {
        #[cfg(target_os = "macos")]
        {
            // `kern.hv_vmm_present` ist 1, wenn ein Hypervisor aktiv ist.
            return match sysctl_wert("kern.hv_vmm_present").as_deref() {
                Some("1") => "hypervisor".to_string(),
                Some(_) => "keine".to_string(),
                None => "unbekannt".to_string(),
            };
        }
    }
    if cfg!(target_os = "linux") {
        // `systemd-detect-virt` meldet `none` auf nackter Hardware und
        // sonst die Technik (kvm, docker, …). Fehlt es, ist die Antwort
        // ehrlicherweise unbekannt.
        if let Ok(out) = std::process::Command::new("systemd-detect-virt").output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return s;
                }
            }
        }
        return "unbekannt".into();
    }
    "unbekannt".into()
}

/// Der GPU-Kartenname, wenn für ein GPU-Backend gebaut wurde.
///
/// Die Delegation der cuda/rocm-Backends rechnet nicht auf der Karte;
/// aber wer für ein GPU-Backend baut, will im Protokoll sehen, welche
/// Karte überhaupt da ist. Best-Effort über das jeweilige CLI-Werkzeug,
/// sonst „keine".
fn gpu_karte() -> String {
    let mut namen: Vec<String> = Vec::new();
    if cfg!(feature = "cuda") {
        if let Some(n) = nvidia_kartenname() {
            namen.push(n);
        }
    }
    if cfg!(feature = "rocm") {
        if let Some(n) = rocm_kartenname() {
            namen.push(n);
        }
    }
    if namen.is_empty() {
        "keine".into()
    } else {
        namen.join(",")
    }
}

#[cfg_attr(not(feature = "cuda"), allow(dead_code))]
fn nvidia_kartenname() -> Option<String> {
    let out = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=name", "--format=csv,noheader"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[cfg_attr(not(feature = "rocm"), allow(dead_code))]
fn rocm_kartenname() -> Option<String> {
    let out = std::process::Command::new("rocm-smi")
        .arg("--showproductname")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
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
    // `cpu-simd` steht seit Fund 34 in derselben Reihe wie `cuda` und
    // `rocm`, und aus demselben Grund: Auf x86_64 gibt es noch keinen
    // vektorisierten Pfad (`kernels/src/dot.rs`, Punkt AVX2 /
    // Fund A19). Ein Messlauf aus einem solchen Bau würde die Referenz
    // unter dem Namen `cpu-simd` protokollieren.
    //
    // Die Ablehnung ist hier freundlicher als bei `cuda`: Es fehlt nichts
    // am Lauf, nur am Bau. Ohne das Feature ist derselbe Lauf ein
    // vollwertiger Referenzlauf, und für den Cross-Hardware-Nachweis ist
    // er genau das, was gebraucht wird.
    for backend in ["cuda", "rocm", "cpu-simd"] {
        let konfiguriert = compiled_backends().iter().any(|b| b == backend);
        if konfiguriert && !integer_llm_kernels::rechenpfad::rechnet(backend) {
            if backend == "cpu-simd" {
                return Err(format!(
                    "Dieser Bau trägt `--features cpu-simd`, aber auf {} gibt es\n\
                     KEINEN vektorisierten Rechenpfad: kernels/src/dot.rs hat bisher\n\
                     nur eine NEON-Fassung für aarch64, AVX2 ist offener Punkt\n\
                     (Fund A19).\n\
                     \n\
                     Der Lauf würde die Referenzkernel unter dem Namen `cpu-simd`\n\
                     protokollieren. Ein Vergleich `Referenz gegen cpu-simd` sähe dann\n\
                     bestanden aus, obwohl beide Seiten denselben Code gerechnet haben.\n\
                     \n\
                     Abhilfe: ohne `--features cpu-simd` bauen.\n\
                     \n\
                     \x20\x20\x20\x20cargo build --release\n\
                     \n\
                     Der Lauf ist dann als `reference` vollwertig und für den\n\
                     Cross-Hardware-Nachweis genau das Richtige: Verglichen werden\n\
                     zwei Maschinen, nicht zwei Backends.",
                    std::env::consts::ARCH
                ));
            }
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

    /// **Jedes erhobene Feld gehört in genau eine der beiden Listen.**
    ///
    /// Ohne diese Probe fiele ein künftig ergänztes Feld stillschweigend
    /// aus beiden Fingerabdrücken heraus: `canonical_bytes` liefe über
    /// die Liste, das neue Feld stünde nicht darin, und niemand merkte
    /// es. Genau umgekehrt entstand Fund 105, nämlich dadurch, dass
    /// `canonical_bytes` über **alle** Felder lief und dabei drei
    /// mitnahm, die nicht die Maschine beschreiben.
    #[test]
    fn jedes_feld_ist_einer_seite_zugeordnet() {
        let fp = Fingerprint::collect();
        for (k, _) in &fp.entries {
            let maschine = MASCHINENFELDER.contains(&k.as_str());
            let rechenpfad = RECHENPFADFELDER.contains(&k.as_str());
            assert!(
                maschine ^ rechenpfad,
                "Feld {k:?} steht in {} Listen, es muss in genau einer stehen",
                u8::from(maschine) + u8::from(rechenpfad)
            );
        }
        // Und umgekehrt: keine Liste nennt ein Feld, das es nicht gibt.
        for feld in MASCHINENFELDER.iter().chain(RECHENPFADFELDER.iter()) {
            assert!(
                fp.entries.iter().any(|(k, _)| k == feld),
                "Liste nennt {feld:?}, erhoben wird es nicht"
            );
        }
    }

    /// ⚑ **Fund 105 in einem Test.** Der Maschinen-Fingerabdruck darf
    /// sich nicht ändern, wenn sich nur der Bau ändert.
    ///
    /// Nachgestellt wird das über die Felder selbst: Zwei Erhebungen
    /// derselben Maschine, bei denen die Rechenpfad-Felder verschiedene
    /// Werte tragen, müssen dieselbe kanonische Bytefolge ergeben. Ein
    /// zweiter Bau mit `--features cpu-simd` ändert genau diese drei
    /// Felder und sonst nichts.
    #[test]
    fn ein_zweiter_bau_ist_keine_zweite_maschine() {
        let mut a = Fingerprint::collect();
        let vorher = a.canonical_bytes();

        for (k, v) in a.entries.iter_mut() {
            if RECHENPFADFELDER.contains(&k.as_str()) {
                *v = format!("{v}-anders");
            }
        }

        assert_eq!(
            vorher,
            a.canonical_bytes(),
            "der Maschinen-Fingerabdruck hängt am Bau: das ist Fund 105"
        );
        assert_ne!(
            Fingerprint::collect().rechenpfad_bytes(),
            a.rechenpfad_bytes(),
            "der Rechenpfad-Fingerabdruck merkt den anderen Bau nicht"
        );
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
        // Ohne cuda/rocm bleibt der Fall aus Fund 34: ein Bau mit
        // `cpu-simd` auf einem Ziel ohne vektorisierten Pfad. Er wird
        // ebenfalls abgelehnt, mit eigenem Wortlaut, siehe den Test
        // darunter.
        if !konfiguriert.iter().any(|b| b == "cpu-simd")
            || integer_llm_kernels::dot::VEKTORISIERT
        {
            assert!(rechenpfad_pruefen().is_ok());
        }
    }

    /// **Fund 34 als Test.** Ein Bau mit `--features cpu-simd` auf einem
    /// Ziel, für das `kernels/src/dot.rs` keine vektorisierte Fassung
    /// hat, darf keinen Messlauf zulassen: Er würde die Referenzkernel
    /// unter dem Namen `cpu-simd` protokollieren, und ein Vergleich
    /// „Referenz gegen cpu-simd" sähe bestanden aus, obwohl beide Seiten
    /// denselben Code gerechnet haben.
    ///
    /// Der Test greift heute auf x86_64 und auf jedem Ziel ohne NEON.
    /// Sobald der AVX2-Pfad steht, greift er dort nicht mehr, und das ist
    /// richtig so.
    #[test]
    fn cpu_simd_ohne_vektorpfad_wird_abgelehnt() {
        if !cfg!(feature = "cpu-simd") || integer_llm_kernels::dot::VEKTORISIERT {
            return;
        }
        let fehler = rechenpfad_pruefen().expect_err("hätte ablehnen müssen");
        assert!(
            fehler.contains("KEINEN vektorisierten Rechenpfad"),
            "Ablehnung nennt den Grund nicht: {fehler}"
        );
        assert!(
            fehler.contains("cargo build --release"),
            "Ablehnung nennt die Abhilfe nicht: {fehler}"
        );
    }

    /// Das gemeldete Backend muss dem folgen, was gerechnet wird, und
    /// nicht dem, was die CPU könnte. Vor Fund 34 stand hier
    /// `is_x86_feature_detected!("avx2")`, und damit ging auf jeder
    /// halbwegs neuen x86_64-Maschine `cpu-simd/avx2` ins Protokoll,
    /// während skalar gerechnet wurde.
    #[test]
    fn gemeldetes_backend_folgt_dem_rechenpfad() {
        let gemeldet = selected_backend();
        if integer_llm_kernels::dot::VEKTORISIERT {
            assert_ne!(gemeldet, "reference");
            assert!(
                rechnende_backends().iter().any(|b| b == "cpu-simd"),
                "vektorisiert, aber cpu-simd fehlt in der Liste"
            );
        } else {
            assert_eq!(
                gemeldet, "reference",
                "ohne vektorisierten Pfad darf nichts anderes im Protokoll stehen"
            );
        }
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

    /// Die Beschreibungs-Felder (Punkt 4.2) müssen alle erhoben werden:
    /// Fehlt eines, sagt der Beleg nicht, welche Maschine gemessen hat.
    #[test]
    fn beschreibung_hat_alle_pflichtfelder() {
        let fp = Fingerprint::collect();
        let wert = |key: &str| {
            fp.beschreibung
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
        };
        for key in ["cpu_modell", "ram_bytes", "virtualisierung", "gpu_karte"] {
            assert!(wert(key).is_some(), "Beschreibungs-Feld {} fehlt", key);
            // Ein Feld darf vorhanden, aber nicht erfunden sein: leer ist
            // erlaubt nur als „keine", „unbekannt" oder die Zahl 0.
        }
        // ram_bytes ist eine Zahl (0 = nicht ermittelbar).
        let ram = wert("ram_bytes").unwrap();
        assert!(ram.parse::<u64>().is_ok(), "ram_bytes ist keine Zahl: {ram}");
    }

    /// **Die Kernbedingung von Punkt 4.2.** Die Beschreibungs-Felder gehen
    /// ins Protokoll, aber NICHT in den Fingerabdruck. Zwei baugleiche
    /// Mietmaschinen unterscheiden sich in keiner dieser Angaben; zählten
    /// sie mit, hielte der Vergleich zwei identische Kisten für zwei
    /// Architekturen und gäbe ein positives Urteil, das nichts belegt.
    #[test]
    fn beschreibung_veraendert_den_fingerabdruck_nicht() {
        let fp = Fingerprint::collect();

        // Die kanonischen Bytes nennen keinen der Beschreibungs-Schlüssel.
        let kanonisch = String::from_utf8(fp.canonical_bytes()).unwrap();
        for key in ["cpu_modell", "ram_bytes", "virtualisierung", "gpu_karte"] {
            assert!(
                !kanonisch.contains(key),
                "Fingerabdruck enthält das Beschreibungs-Feld {key}"
            );
            assert!(fp.get(key).is_none(), "{key} steht im Fingerabdruck-Teil");
        }

        // Der Rechenpfad-Teil darf sie ebenso wenig nennen.
        let pfad = String::from_utf8(fp.rechenpfad_bytes()).unwrap();
        for key in ["cpu_modell", "ram_bytes", "virtualisierung", "gpu_karte"] {
            assert!(
                !pfad.contains(key),
                "Rechenpfad enthält das Beschreibungs-Feld {key}"
            );
        }

        // Hier stand bis zum 2026-08-30: „jeder Entry-Schlüssel taucht
        // auf, kein fremder". Das galt, solange `canonical_bytes` über
        // alle Einträge lief, und genau das war Fund 105. Geprüft wird
        // jetzt die Zuordnung: Der Maschinen-Teil nennt die
        // Maschinenfelder und nur sie, der Rechenpfad-Teil die anderen.
        for feld in MASCHINENFELDER {
            assert!(kanonisch.contains(&format!("{feld}=")), "{feld} fehlt");
            assert!(!pfad.contains(&format!("{feld}=")), "{feld} doppelt");
        }
        for feld in RECHENPFADFELDER {
            assert!(pfad.contains(&format!("{feld}=")), "{feld} fehlt");
            assert!(
                !kanonisch.contains(&format!("{feld}=")),
                "{feld} zählt zur Maschine: das ist Fund 105"
            );
        }
    }

    /// Der Fingerabdruck bleibt über zwei Erhebungen hinweg bytegleich —
    /// auch jetzt, wo die Beschreibung mit erhoben wird. Ohne diese
    /// Zusicherung wäre jeder Lauf ein anderer Maßstab.
    #[test]
    fn fingerabdruck_stabil_unabhaengig_von_der_beschreibung() {
        let a = Fingerprint::collect();
        let b = Fingerprint::collect();
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    }
}
