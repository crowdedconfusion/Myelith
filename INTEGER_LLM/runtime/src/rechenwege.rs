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

/// ⚑ **Der Freigabeplan** (Fund 705, Festlegung des Projektinhabers vom
/// 2026-11-16: das Programm waehlt die beste Ressourcenverteilung selbst,
/// je nach Hardwarefreigabe, wie spaeter in Produktion).
///
/// Der Klient kennt die Rechenwerke der Maschine und die Freigabe des
/// Besitzers und uebersetzt beides in Schalter (`hardware::schaltplan`).
/// Knoten und Shard kennen die Erhebung nicht und sollen sie nicht kennen
/// muessen: Sie bekommen den Plan als Text (`--rechenwerke`) und schalten
/// danach. **Eine Form, an einer Stelle gelesen und geschrieben**, damit
/// Klient, Knoten und Shard nicht drei Fassungen derselben Freigabe fuehren.
///
/// Was der Plan nicht nennt, bleibt, wie es ist: Ohne Plan rechnet alles,
/// was da ist und seine Selbstpruefung gegen die CPU bestanden hat.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Freigabeplan {
    /// Je Rechenweg (`metal`, `cuda`, `rocm`) an oder aus.
    pub wege: Vec<(String, bool)>,
    /// Je Karte, ueber ihre PCI-Adresse, an oder aus.
    pub karten: Vec<(String, bool)>,
}

/// Die Rechenwege, die einen Schalter haben koennen.
const SCHALTBAR: [&str; 3] = ["metal", "cuda", "rocm"];

impl Freigabeplan {
    /// Die Textform fuer `--rechenwerke`, etwa
    /// `metal=aus,cuda=an,karte:0000:01:00.0=an`; ein leerer Plan heisst
    /// `alle`.
    pub fn als_text(&self) -> String {
        let wort = |an: bool| if an { "an" } else { "aus" };
        let teile: Vec<String> = self
            .wege
            .iter()
            .map(|(w, an)| format!("{w}={}", wort(*an)))
            .chain(self.karten.iter().map(|(k, an)| format!("karte:{k}={}", wort(*an))))
            .collect();
        if teile.is_empty() {
            "alle".to_string()
        } else {
            teile.join(",")
        }
    }

    /// Liest die Textform. ⚑ **Ein unbekannter Weg oder Wert ist ein
    /// Fehler** und kein stilles Uebergehen: Ein Knoten, der eine Freigabe
    /// falsch versteht, nimmt sonst mehr Maschine, als der Besitzer gab.
    pub fn aus_text(text: &str) -> Result<Self, String> {
        let text = text.trim();
        let mut plan = Self::default();
        if text.is_empty() || text == "alle" {
            return Ok(plan);
        }
        for teil in text.split(',') {
            let (name, wert) = teil
                .rsplit_once('=')
                .ok_or_else(|| format!("`{teil}`: erwartet `name=an` oder `name=aus`"))?;
            let an = match wert.trim() {
                "an" => true,
                "aus" => false,
                w => return Err(format!("`{teil}`: `{w}` ist weder `an` noch `aus`")),
            };
            let name = name.trim();
            if let Some(pci) = name.strip_prefix("karte:") {
                if pci.trim().is_empty() {
                    return Err(format!("`{teil}`: die Karte braucht eine PCI-Adresse"));
                }
                plan.karten.push((pci.trim().to_string(), an));
            } else if SCHALTBAR.contains(&name) {
                plan.wege.push((name.to_string(), an));
            } else {
                return Err(format!("`{name}` ist kein schaltbarer Rechenweg (metal, cuda, rocm, karte:<pci>)"));
            }
        }
        Ok(plan)
    }

    /// **Schaltet nach dem Plan** und sagt je Schalter in einem Satz, was
    /// geschah, fuer das Protokoll. Ein Schalter, den es in dieser
    /// Uebersetzung nicht gibt, oder eine Karte, die nicht bereit ist, steht
    /// dabei; eine stille Zusage waere die Behauptung, etwas getan zu haben.
    pub fn anwenden(&self) -> Vec<String> {
        let wort = |an: bool| if an { "an" } else { "aus" };
        let mut aus = Vec::new();
        for (weg, an) in &self.wege {
            let geschaltet = setzen(weg, *an);
            aus.push(format!(
                "{weg} {}{}",
                wort(*an),
                if geschaltet { "" } else { " (in dieser Uebersetzung ohne Schalter)" }
            ));
        }
        for (pci, an) in &self.karten {
            let bereit = karte_setzen(pci, *an);
            aus.push(format!(
                "Karte {pci} {}{}",
                wort(*an),
                if bereit { "" } else { " (keine bereite Karte mit dieser Adresse)" }
            ));
        }
        aus
    }
}

/// **Welche Beschleuniger gerade rechnen duerfen**: vorhanden (Geraet,
/// Uebersetzer und Selbstpruefung gegen die CPU) und nicht abgeschaltet.
pub fn beschleuniger_an() -> Vec<&'static str> {
    use integer_llm_kernels::ressourcen::{karten, metal};
    let mut aus = Vec::new();
    if rechnet("metal") && metal::schwelle() > 0 {
        aus.push("metal");
    }
    for (weg, h) in [("cuda", karten::Hersteller::Nvidia), ("rocm", karten::Hersteller::Amd)] {
        if karten::uebersetzt_fuer(h) && karten::hersteller_an(h) && karten::schwelle() > 0 && karten::verfuegbar_fuer(h) {
            aus.push(weg);
        }
    }
    aus
}

/// Was die [`einmessen`] fand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Einmessung {
    /// Die Beschleuniger, die gemessen wurden.
    pub beschleuniger: Vec<&'static str>,
    /// Je Stapelgroesse: Eingaben, Zeit der CPU allein und Zeit mit den
    /// Beschleunigern, je in Mikrosekunden (die beste von zwei Messungen).
    pub zeilen: Vec<(usize, u128, u128)>,
    /// Die gesetzte Schwelle; `0` heisst: Die CPU rechnet allein.
    pub schwelle: usize,
}

impl Einmessung {
    /// Eine Zeile fuer das Protokoll.
    pub fn als_satz(&self) -> String {
        let zeilen: Vec<String> = self
            .zeilen
            .iter()
            .map(|(n, cpu, gpu)| format!("{n} Eingaben: CPU {} ms, {} {} ms", cpu / 1000, self.beschleuniger.join("+"), gpu / 1000))
            .collect();
        let ergebnis = if self.schwelle == 0 {
            "die CPU rechnet allein".to_string()
        } else {
            format!("{} ab {} Eingaben", self.beschleuniger.join("+"), self.schwelle)
        };
        format!("{}; {ergebnis}", zeilen.join("; "))
    }
}

/// ⚑ **Ab welcher Stapelgroesse die Beschleuniger rechnen sollen**, aus den
/// gemessenen Zeilen (Eingaben, CPU, Beschleuniger): die kleinste Groesse,
/// ab der sie bei dieser **und jeder groesseren gemessenen** klar schneller
/// sind, um mehr als ein Zwanzigstel, damit Messrauschen nicht hin und her
/// schaltet. `0` heisst: nie.
pub fn schwelle_aus(zeilen: &[(usize, u128, u128)]) -> usize {
    let mut z = zeilen.to_vec();
    z.sort_by_key(|(n, _, _)| *n);
    let klar_schneller = |(_, cpu, gpu): &(usize, u128, u128)| gpu.saturating_mul(20) < cpu.saturating_mul(19);
    let mut schwelle = 0;
    for zeile in z.iter().rev() {
        if !klar_schneller(zeile) {
            break;
        }
        schwelle = zeile.0;
    }
    schwelle
}

/// ⚑ **Die Einmessung** (Fund 705): misst die Arbeit des Aufrufers bei den
/// gegebenen Stapelgroessen einmal mit und einmal ohne Beschleuniger und
/// setzt die Schwelle danach (je Rechenweg ueber dessen eigene Schwelle).
///
/// 📌 **Warum gemessen und nicht vorgegeben:** Die Vorgabe von 16 Eingaben
/// kam von Metal bei grossen Prompts in einem Zug. Im Pod rechnet ein Shard
/// Stapel von 64 Token, und dort war Metal am 35B langsamer als die CPU
/// (80 s gegen 61 s fuer 3 563 Prompt-Token): Das Absetzen an die GPU kostete
/// mehr, als die gebuendelten Matrizen brachten. Welche Seite gewinnt,
/// haengt an Maschine, Modell, Bereich und Stapel; wissen kann es nur die
/// Maschine selbst.
///
/// ⚑ **Kein Ergebnis haengt daran**, nur wo gerechnet wird: Jeder
/// Beschleuniger hat seine Selbstpruefung gegen die CPU bestanden, und die
/// Konformitaet traegt auf allen Wegen denselben Digest.
///
/// `messen(n)` rechnet einen Stapel von `n` Eingaben und gibt die Dauer.
/// Ohne Beschleuniger kommt `None`, und nichts wurde gemessen.
pub fn einmessen(stapel: &[usize], mut messen: impl FnMut(usize) -> std::time::Duration) -> Option<Einmessung> {
    use integer_llm_kernels::ressourcen::{karten, metal};
    let beschleuniger = beschleuniger_an();
    if beschleuniger.is_empty() || stapel.is_empty() {
        return None;
    }
    let metall = beschleuniger.contains(&"metal");
    let karte = beschleuniger.iter().any(|w| *w == "cuda" || *w == "rocm");
    let setze = |n: usize| {
        if metall {
            metal::schwelle_setzen(n);
        }
        if karte {
            karten::schwelle_setzen(n);
        }
    };
    let mut bestes = |n: usize| (0..2).map(|_| messen(n).as_micros()).min().unwrap_or(u128::MAX);
    let mut zeilen = Vec::with_capacity(stapel.len());
    for &n in stapel {
        // Mit Beschleuniger zuerst, und einmal zum Aufwaermen: Die erste
        // Rechnung uebersetzt Shader und laedt Gewichte hoch.
        setze(1);
        let _ = bestes(n);
        let gpu = bestes(n);
        setze(0);
        let cpu = bestes(n);
        zeilen.push((n, cpu, gpu));
    }
    let schwelle = schwelle_aus(&zeilen);
    setze(schwelle);
    Some(Einmessung { beschleuniger, zeilen, schwelle })
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

    /// ⚑ **Der Plan kommt als Text so zurueck, wie er hinausging**, und
    /// die Textform ist die, die der Knoten an `myl-shard` weitergibt.
    #[test]
    fn der_freigabeplan_kommt_als_text_unveraendert_zurueck() {
        let plan = Freigabeplan {
            wege: vec![("metal".into(), false), ("cuda".into(), true)],
            karten: vec![("0000:01:00.0".into(), true), ("0000:02:00.0".into(), false)],
        };
        let text = plan.als_text();
        assert_eq!(text, "metal=aus,cuda=an,karte:0000:01:00.0=an,karte:0000:02:00.0=aus");
        assert_eq!(Freigabeplan::aus_text(&text), Ok(plan));
        assert_eq!(Freigabeplan::default().als_text(), "alle");
        assert_eq!(Freigabeplan::aus_text("alle"), Ok(Freigabeplan::default()));
        assert_eq!(Freigabeplan::aus_text(""), Ok(Freigabeplan::default()));
    }

    /// ⛔️ **Was der Plan nicht versteht, lehnt er ab.** Die Gegenprobe zur
    /// Probe oben: Ein Tippfehler darf nicht zur ganzen Maschine werden.
    #[test]
    fn ein_unverstaendlicher_freigabeplan_wird_abgelehnt() {
        for falsch in ["metal", "metal=ja", "vulkan=an", "karte:=an", "metal=an,,cuda=aus"] {
            assert!(Freigabeplan::aus_text(falsch).is_err(), "`{falsch}` wurde angenommen");
        }
    }

    /// ⚑ **Eine Karte, die es nicht gibt, sagt das**, und der Schalter
    /// steht danach wieder, wie er vorher stand.
    #[test]
    fn eine_fehlende_karte_steht_im_protokoll() {
        let adresse = "0000:fe:1f.7";
        let aus = Freigabeplan { wege: vec![], karten: vec![(adresse.into(), false)] }.anwenden();
        assert_eq!(aus, vec![format!("Karte {adresse} aus (keine bereite Karte mit dieser Adresse)")]);
        let an = Freigabeplan { wege: vec![], karten: vec![(adresse.into(), true)] }.anwenden();
        assert!(an[0].starts_with(&format!("Karte {adresse} an")), "{an:?}");
        assert!(integer_llm_kernels::ressourcen::karten::karte_an(adresse), "die Karte blieb aus");
    }

    /// ⚑ **Die Schwelle der Einmessung**: die kleinste Groesse, ab der die
    /// Beschleuniger bei dieser und jeder groesseren klar gewinnen.
    #[test]
    fn die_schwelle_beginnt_wo_der_beschleuniger_dauerhaft_gewinnt() {
        // Gewinnt ab 32: Schwelle 32.
        assert_eq!(schwelle_aus(&[(16, 100, 120), (32, 200, 150), (64, 400, 200)]), 32);
        // Gewinnt nur bei 16, verliert bei 64 (wie Metal im Pod): nie.
        assert_eq!(schwelle_aus(&[(16, 100, 50), (64, 400, 480)]), 0);
        // Gewinnt ueberall: ab der kleinsten.
        assert_eq!(schwelle_aus(&[(64, 400, 100), (16, 100, 60)]), 16);
        // Gleichstand ist kein Gewinn (weniger als ein Zwanzigstel).
        assert_eq!(schwelle_aus(&[(64, 1000, 960)]), 0);
        assert_eq!(schwelle_aus(&[]), 0);
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
