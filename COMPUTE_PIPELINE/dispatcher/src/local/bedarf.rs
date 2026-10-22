//! **Was eine Arbeitslast braucht.**
//!
//! # ⚑ Ein eigenes Artefakt: auf das Byte, ohne zweite Formel
//!
//! Die Gewichte sind Dateien, also ist ihr Bedarf ihre Groesse, je Ebene
//! und je Experte getrennt: Die Namen tragen die Ebene
//! (`model_layers_<L>_…`) und den Experten (`…_mlp_experts_<E>_…`). Alles
//! ohne Ebene (Einbettung, Kopf, Tabellen) ist der Rand.
//!
//! ⚑ **Nicht jeder Rand wird beim Rechnen ganz gelesen** (seit 2026-10-15).
//! Traegt ein Artefakt einen eigenen Kopf in `i16` (`lm_head.bin`), rechnet
//! die Laufzeit die Logits damit (`IntegerModel::lm_head_int16` geht vor),
//! und dann
//!
//! - liest jedes Token von der **Einbettung** nur seine eine Zeile (2 KB
//!   beim 35B von 0,5 GB), und
//! - wird ein zweiter Kopf in `int8` (`lm_head_weight.bin`, bei den
//!   Gemischen) **gar nicht** gelesen.
//!
//! Beide stehen in [`Modellbedarf::rand_selten_bytes`] und nicht im
//! dichten Teil, der im Speicher bleiben muss. 📌 Bis dahin hielt der Plan
//! beim 35B 1 GB fuer fest, das das System in Wirklichkeit den Experten
//! gab. ⚠️ **Eine Festlegung der Laufzeit, die hier ein zweites Mal
//! steht**: Waehlt die Laufzeit einmal einen anderen Kopf, ist diese Regel
//! mitzuziehen; die Probe `der_seltene_rand` haelt sie fest.
//!
//! Was mit der Folge waechst, steht in `model_config.json`:
//!
//! - **KV je Token**: je achtsamer Ebene `num_kv_heads · head_dim` Werte
//!   fuer Schluessel und ebenso viele fuer Werte, je `i16`, also 2 Byte
//!   (so legt der KV-Speicher der Laufzeit sie ab).
//! - **Zustand je Folge**, nur bei rekurrenten Ebenen und unabhaengig von
//!   der Laenge: je Wertkopf eine Matrix `linear_key_head_dim ·
//!   linear_value_head_dim` aus `i64` (8 Byte), dazu ein Faltungsfenster
//!   `kanaele · (linear_conv_kernel_dim − 1)` aus `i16`, mit `kanaele = 2 ·
//!   linear_key_head_dim · linear_num_key_heads + linear_value_head_dim ·
//!   linear_num_value_heads`.
//!
//! ⚠️ **Die beiden Breiten (`i16`, `i64`) sind Festlegungen des
//! Rechenpfads und stehen hier ein zweites Mal**, weil diese Kiste den
//! Rechenpfad nicht mitbauen soll. Aendert sich dort eine, ist dieser Kopf
//! mitzuziehen; die Probe `der_bedarf_eines_hybriden_artefakts` rechnet
//! beide Groessen von Hand nach.
//!
//! # Ein fremdes Modell (Vision, Sprache)
//!
//! Es rechnet in einer fremden Laufzeit, und von aussen ist nur seine
//! Dateigroesse sicher. Der Rest kommt aus einer Eichung
//! ([`crate::local::eichung`]).

use std::collections::BTreeMap;
use std::path::Path;

/// Wie eine Ebene mischt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Ebenenart {
    /// Aufmerksamkeit: legt je Token einen Eintrag im KV-Speicher ab.
    Achtsam,
    /// Zustandsschicht: traegt einen Zustand fester Groesse.
    Rekurrent,
}

/// Der Bedarf einer Ebene.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Ebene {
    pub index: usize,
    pub art: Ebenenart,
    /// Alles ausser den gerouteten Experten: Mischer, Normen, Router, ein
    /// geteilter Experte, bei dichten Modellen der ganze MLP.
    pub dicht_bytes: u64,
    /// Je gerouteten Experten seine Bytes, in der Reihenfolge der Nummern.
    /// Leer bei dichten Modellen.
    pub experten_bytes: Vec<u64>,
}

/// Was ein eigenes Artefakt braucht.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Modellbedarf {
    pub name: String,
    /// Alles ohne Ebene, was jedes Token ganz liest: Kopf, Tabellen,
    /// Beschreibung, bei einem Modell ohne eigenen `i16`-Kopf auch die
    /// Einbettung.
    pub rand_bytes: u64,
    /// **Rand, den das Rechnen nicht oder nur zeilenweise liest**: die
    /// Einbettung und ein zweiter Kopf neben dem `i16`-Kopf (Modulkopf).
    pub rand_selten_bytes: u64,
    pub ebenen: Vec<Ebene>,
    pub kv_je_token: u64,
    pub zustand_je_folge: u64,
    pub hoechstkontext: u64,
}

impl Modellbedarf {
    /// Rand und alle dichten Teile.
    pub fn dicht_bytes(&self) -> u64 {
        self.rand_bytes + self.ebenen.iter().map(|e| e.dicht_bytes).sum::<u64>()
    }

    /// Alle gerouteten Experten zusammen.
    pub fn experten_bytes(&self) -> u64 {
        self.ebenen.iter().flat_map(|e| e.experten_bytes.iter()).sum()
    }

    /// Alle Gewichte, auch der seltene Rand.
    pub fn gewichte_bytes(&self) -> u64 {
        self.dicht_bytes() + self.experten_bytes() + self.rand_selten_bytes
    }

    /// Der KV-Speicher fuer `kontext` Token; `None` bei Ueberlauf.
    pub fn kv_bytes(&self, kontext: u64) -> Option<u64> {
        self.kv_je_token.checked_mul(kontext)
    }

    /// **Liest ein Artefakt**: Dateigroessen und Modellbeschreibung.
    ///
    /// Gelesen werden nur Verzeichniseintraege und `model_config.json`,
    /// keine Gewichte; das dauert auch beim 35B mit gut sechzigtausend
    /// Dateien unter einer Sekunde.
    ///
    /// ⚑ **Ein halbes Artefakt ist ein Fehler, kein kleiner Bedarf.** Fehlt
    /// eine Ebene oder ein Experte, kommt `Err` mit dem, was fehlt.
    pub fn aus_artefakt(ordner: &Path) -> Result<Self, String> {
        let konfig_pfad = ordner.join("model_config.json");
        let text = std::fs::read_to_string(&konfig_pfad).map_err(|e| format!("{}: {e}", konfig_pfad.display()))?;
        let k: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", konfig_pfad.display()))?;
        let mut dateien = Vec::new();
        for e in std::fs::read_dir(ordner).map_err(|e| format!("{}: {e}", ordner.display()))? {
            let e = e.map_err(|e| format!("{}: {e}", ordner.display()))?;
            let m = e.metadata().map_err(|err| format!("{}: {err}", e.path().display()))?;
            if m.is_file() {
                dateien.push((e.file_name().to_string_lossy().to_string(), m.len()));
            }
        }
        let name = ordner.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        Self::aus_teilen(&name, &k, &dateien)
    }

    /// Dasselbe aus schon gelesenen Teilen; die eigentliche Rechnung.
    pub fn aus_teilen(name: &str, k: &serde_json::Value, dateien: &[(String, u64)]) -> Result<Self, String> {
        let zahl = |feld: &str| -> Result<u64, String> {
            k.get(feld).and_then(|v| v.as_u64()).ok_or_else(|| format!("model_config.json: `{feld}` fehlt"))
        };
        let zahl_oder_null = |feld: &str| k.get(feld).and_then(|v| v.as_u64()).unwrap_or(0);

        let ebenenzahl = zahl("num_layers")? as usize;
        let experten = zahl_oder_null("num_experts") as usize;
        // ⚑ **Gepackte Experten stehen in zwei Tensoren je Ebene** statt in
        // Dateien je Experte. Hier gezaehlt, hielte sie der Bedarf fuer dicht;
        // kein Artefakt nutzt das heute, also lieber ein Fehler als eine
        // falsche Zahl.
        if k.get("experts_packed").and_then(|v| v.as_bool()) == Some(true) {
            return Err("Gepackte Experten (`experts_packed`) zählt dieser Bedarf nicht.".into());
        }
        let arten: Vec<Ebenenart> = match k.get("layer_types").and_then(|v| v.as_array()) {
            Some(liste) => {
                if liste.len() != ebenenzahl {
                    return Err(format!("`layer_types` nennt {} Ebenen, `num_layers` {ebenenzahl}", liste.len()));
                }
                liste
                    .iter()
                    .map(|t| match t.as_str() {
                        Some("full_attention") => Ok(Ebenenart::Achtsam),
                        Some("linear_attention") => Ok(Ebenenart::Rekurrent),
                        anders => Err(format!("unbekannte Ebenenart {anders:?}")),
                    })
                    .collect::<Result<_, _>>()?
            }
            None => vec![Ebenenart::Achtsam; ebenenzahl],
        };

        let mut rand = 0u64;
        let mut selten = 0u64;
        let eigener_kopf = dateien.iter().any(|(d, _)| d == "lm_head.bin");
        let mut dicht = vec![0u64; ebenenzahl];
        let mut gesehen = vec![false; ebenenzahl];
        let mut je_experte: Vec<BTreeMap<usize, u64>> = vec![BTreeMap::new(); ebenenzahl];
        for (datei, groesse) in dateien {
            let Some((ebene, rest)) = ebene_aus_name(datei) else {
                if eigener_kopf && matches!(datei.as_str(), "model_embed_tokens_weight.bin" | "lm_head_weight.bin") {
                    selten += groesse;
                } else {
                    rand += groesse;
                }
                continue;
            };
            if ebene >= ebenenzahl {
                return Err(format!("{datei}: Ebene {ebene}, das Modell hat {ebenenzahl}"));
            }
            gesehen[ebene] = true;
            match experte_aus_rest(rest) {
                Some(e) if e < experten => *je_experte[ebene].entry(e).or_insert(0) += groesse,
                Some(e) => return Err(format!("{datei}: Experte {e}, das Modell hat {experten}")),
                None => dicht[ebene] += groesse,
            }
        }
        if let Some(i) = gesehen.iter().position(|g| !g) {
            return Err(format!("Ebene {i} hat keine einzige Datei"));
        }

        let mut ebenen = Vec::with_capacity(ebenenzahl);
        for (i, art) in arten.iter().enumerate() {
            let karte = &je_experte[i];
            // ⚑ Ein Gemisch, dem in einer Ebene ein Experte fehlt, ist
            // unvollstaendig, nicht kleiner.
            if !karte.is_empty() && karte.len() != experten {
                let fehlt = (0..experten).find(|e| !karte.contains_key(e)).unwrap_or(experten);
                return Err(format!("Ebene {i}: Experte {fehlt} fehlt ({} von {experten})", karte.len()));
            }
            ebenen.push(Ebene {
                index: i,
                art: *art,
                dicht_bytes: dicht[i],
                experten_bytes: karte.values().copied().collect(),
            });
        }

        let achtsam = arten.iter().filter(|a| **a == Ebenenart::Achtsam).count() as u64;
        let rekurrent = ebenenzahl as u64 - achtsam;
        // Schluessel und Werte, je `i16`.
        let kv_je_token = achtsam * zahl("num_kv_heads")? * zahl("head_dim")? * 2 * 2;
        let zustand_je_folge = if rekurrent == 0 {
            0
        } else {
            let sk = zahl("linear_num_key_heads")?;
            let wk = zahl("linear_num_value_heads")?;
            let sd = zahl("linear_key_head_dim")?;
            let wd = zahl("linear_value_head_dim")?;
            let kern = zahl("linear_conv_kernel_dim")?;
            let kanaele = 2 * sd * sk + wd * wk;
            // Zustand je Wertkopf aus `i64`, Fenster aus `i16`.
            let je_ebene = wk * sd * wd * 8 + kanaele * kern.saturating_sub(1) * 2;
            rekurrent * je_ebene
        };

        Ok(Self {
            name: name.to_string(),
            rand_bytes: rand,
            rand_selten_bytes: selten,
            ebenen,
            kv_je_token,
            zustand_je_folge,
            hoechstkontext: zahl("max_context")?,
        })
    }
}

impl Modellbedarf {
    /// **Der Bedarf eines Ebenenbereichs** `[von, bis)` (Stufe 4, P8,
    /// 2026-10-22): was ein Shard hält, den das Netz diesem Rechner
    /// zugeteilt hat. KV-Speicher und Zustand anteilig der Ebenen im
    /// Bereich; der Rand (Kopf, Tabellen) nur am letzten Bereich, die
    /// Einbettung nur am ersten.
    ///
    /// ⚠️ **Der Rand ist dabei ungeteilt**: Er enthält außer dem Kopf
    /// Tabellen und Beschreibung, die jeder Shard braucht; sie sind klein
    /// gegen eine Ebene und zählen hier nur beim letzten.
    pub fn bereich(&self, von: usize, bis: usize, mit_einbettung: bool, mit_kopf: bool) -> Result<Self, String> {
        if von >= bis || bis > self.ebenen.len() {
            return Err(format!("Bereich {von} bis {bis} passt nicht zu {} Ebenen", self.ebenen.len()));
        }
        let zaehle = |art: Ebenenart, r: &[Ebene]| r.iter().filter(|e| e.art == art).count() as u64;
        let achtsam_alle = zaehle(Ebenenart::Achtsam, &self.ebenen).max(1);
        let rekurrent_alle = zaehle(Ebenenart::Rekurrent, &self.ebenen).max(1);
        let teil = &self.ebenen[von..bis];
        Ok(Self {
            name: format!("{} (Ebenen {von} bis {bis})", self.name),
            rand_bytes: if mit_kopf { self.rand_bytes } else { 0 },
            rand_selten_bytes: if mit_einbettung { self.rand_selten_bytes } else { 0 },
            ebenen: teil.to_vec(),
            kv_je_token: self.kv_je_token / achtsam_alle * zaehle(Ebenenart::Achtsam, teil),
            zustand_je_folge: self.zustand_je_folge / rekurrent_alle * zaehle(Ebenenart::Rekurrent, teil),
            hoechstkontext: self.hoechstkontext,
        })
    }
}

/// `model_layers_12_mlp_up_proj_weight.bin` → `(12, "mlp_up_proj_weight.bin")`.
fn ebene_aus_name(datei: &str) -> Option<(usize, &str)> {
    let rest = datei.strip_prefix("model_layers_")?;
    let ziffern = rest.find(|c: char| !c.is_ascii_digit())?;
    let ebene = rest[..ziffern].parse().ok()?;
    Some((ebene, rest[ziffern..].strip_prefix('_')?))
}

/// `mlp_experts_7_down_proj_weight.bin` → `7`; alles andere `None`.
///
/// ⚑ **`mlp_shared_expert_…` ist kein gerouteter Experte**: Er rechnet bei
/// jedem Token und gehoert zum dichten Teil.
fn experte_aus_rest(rest: &str) -> Option<usize> {
    let n = rest.strip_prefix("mlp_experts_")?;
    let ziffern = n.find(|c: char| !c.is_ascii_digit())?;
    n[..ziffern].parse().ok()
}

/// Was ein fremdes Modell braucht: die Summe seiner Dateien.
///
/// ⚑ **Eine fehlende Datei ist ein Fehler.** Ein Sehmodell ohne seinen
/// Projektor rechnet nicht, und sein Bedarf waere kleiner als der echte.
pub fn fremd_aus_dateien(dateien: &[&Path]) -> Result<u64, String> {
    let mut summe = 0u64;
    for d in dateien {
        summe += std::fs::metadata(d).map_err(|e| format!("{}: {e}", d.display()))?.len();
    }
    Ok(summe)
}

#[cfg(test)]
mod proben {
    use super::*;
    use serde_json::json;

    /// ⚑ **Ein hybrides Gemisch, von Hand nachgerechnet**: zwei Ebenen, eine
    /// rekurrent, eine achtsam, je drei Experten.
    #[test]
    fn der_bedarf_eines_hybriden_artefakts() {
        let k = json!({
            "num_layers": 2, "num_experts": 3, "num_kv_heads": 2, "head_dim": 256,
            "layer_types": ["linear_attention", "full_attention"],
            "linear_num_key_heads": 16, "linear_num_value_heads": 32,
            "linear_key_head_dim": 128, "linear_value_head_dim": 128,
            "linear_conv_kernel_dim": 4, "max_context": 262144
        });
        let mut dateien = vec![
            ("model_config.json".to_string(), 1),
            ("model_embed_tokens_weight.bin".to_string(), 100),
            ("lm_head.bin".to_string(), 50),
        ];
        for l in 0..2 {
            dateien.push((format!("model_layers_{l}_input_layernorm_weight.bin"), 10));
            dateien.push((format!("model_layers_{l}_mlp_shared_expert_up_proj_weight.bin"), 7));
            for e in 0..3 {
                dateien.push((format!("model_layers_{l}_mlp_experts_{e}_up_proj_weight.bin"), 1000 + e as u64));
                dateien.push((format!("model_layers_{l}_mlp_experts_{e}_up_proj_weight_shifts.bin"), 1));
            }
        }
        let b = Modellbedarf::aus_teilen("probe", &k, &dateien).expect("Bedarf");
        assert_eq!((b.rand_bytes, b.rand_selten_bytes), (51, 100), "mit eigenem Kopf liest jedes Token eine Zeile");
        assert_eq!(b.ebenen[0].dicht_bytes, 17, "der geteilte Experte zaehlt zum dichten Teil");
        assert_eq!(b.ebenen[1].experten_bytes, vec![1001, 1002, 1003]);
        assert_eq!(b.ebenen[0].art, Ebenenart::Rekurrent);
        // Eine achtsame Ebene: 2 Koepfe mal 256, Schluessel und Wert, je 2 Byte.
        assert_eq!(b.kv_je_token, 2 * 256 * 2 * 2);
        // Eine rekurrente: 32 Wertkoepfe mal 128 mal 128 mal 8, dazu das Fenster
        // mit 2·128·16 + 128·32 = 8192 Kanaelen mal 3 mal 2.
        assert_eq!(b.zustand_je_folge, 32 * 128 * 128 * 8 + 8192 * 3 * 2);
        assert_eq!(b.gewichte_bytes(), 151 + 2 * 17 + 2 * (1001 + 1002 + 1003));
        assert_eq!(b.kv_bytes(1000), Some(2048 * 1000));
    }

    /// ⚑ **Der seltene Rand**: Mit eigenem `i16`-Kopf sind Einbettung und
    /// `int8`-Kopf selten, ohne ihn zaehlt beides zum dichten Teil (die
    /// Einbettung kann dann der Kopf sein).
    #[test]
    fn der_seltene_rand() {
        let k = json!({"num_layers": 1, "num_kv_heads": 1, "head_dim": 8, "max_context": 16});
        let mit = |kopf: bool| {
            let mut d = vec![
                ("model_embed_tokens_weight.bin".to_string(), 100),
                ("model_embed_tokens_weight_shifts.bin".to_string(), 3),
                ("lm_head_weight.bin".to_string(), 100),
                ("model_layers_0_input_layernorm_weight.bin".to_string(), 1),
            ];
            if kopf {
                d.push(("lm_head.bin".to_string(), 200));
            }
            Modellbedarf::aus_teilen("p", &k, &d).expect("Bedarf")
        };
        let b = mit(true);
        assert_eq!((b.rand_bytes, b.rand_selten_bytes, b.dicht_bytes(), b.gewichte_bytes()), (203, 200, 204, 404));
        let b = mit(false);
        assert_eq!((b.rand_bytes, b.rand_selten_bytes, b.gewichte_bytes()), (203, 0, 204));
    }

    /// ⚑ **Ein halbes Artefakt ist ein Fehler**: ein fehlender Experte, eine
    /// fehlende Ebene, eine Ebene zu viel.
    #[test]
    fn ein_halbes_artefakt_wird_abgewiesen() {
        let k = json!({"num_layers": 2, "num_experts": 2, "num_kv_heads": 1, "head_dim": 8, "max_context": 16});
        let voll = |weg: Option<&str>| -> Vec<(String, u64)> {
            let mut d = Vec::new();
            for l in 0..2 {
                d.push((format!("model_layers_{l}_input_layernorm_weight.bin"), 1));
                for e in 0..2 {
                    d.push((format!("model_layers_{l}_mlp_experts_{e}_up_proj_weight.bin"), 1));
                }
            }
            d.retain(|(n, _)| Some(n.as_str()) != weg);
            d
        };
        assert!(Modellbedarf::aus_teilen("p", &k, &voll(None)).is_ok());
        let fehler = Modellbedarf::aus_teilen("p", &k, &voll(Some("model_layers_1_mlp_experts_1_up_proj_weight.bin")))
            .expect_err("ein fehlender Experte");
        assert!(fehler.contains("Experte 1 fehlt"), "{fehler}");
        let mut zu_viel = voll(None);
        zu_viel.push(("model_layers_2_input_layernorm_weight.bin".into(), 1));
        assert!(Modellbedarf::aus_teilen("p", &k, &zu_viel).is_err(), "eine Ebene ueber num_layers");
        let ohne_ebene: Vec<_> = voll(None).into_iter().filter(|(n, _)| !n.starts_with("model_layers_1_")).collect();
        assert!(Modellbedarf::aus_teilen("p", &k, &ohne_ebene).unwrap_err().contains("Ebene 1"));
        let mut gepackt = k.clone();
        gepackt["experts_packed"] = json!(true);
        assert!(Modellbedarf::aus_teilen("p", &gepackt, &voll(None)).unwrap_err().contains("Gepackte"));
    }

    /// Ein dichtes Modell ohne `layer_types`: jede Ebene achtsam, kein Zustand.
    #[test]
    fn ein_dichtes_modell_hat_keinen_zustand() {
        let k = json!({"num_layers": 1, "num_kv_heads": 8, "head_dim": 128, "max_context": 40960});
        let d = vec![("model_layers_0_mlp_up_proj_weight.bin".to_string(), 5)];
        let b = Modellbedarf::aus_teilen("p", &k, &d).expect("Bedarf");
        assert_eq!((b.zustand_je_folge, b.kv_je_token, b.experten_bytes()), (0, 8 * 128 * 4, 0));
    }

    #[test]
    fn namen_werden_genau_gelesen() {
        assert_eq!(ebene_aus_name("model_layers_12_mlp_up.bin"), Some((12, "mlp_up.bin")));
        assert_eq!(ebene_aus_name("model_embed_tokens_weight.bin"), None);
        assert_eq!(experte_aus_rest("mlp_experts_7_down.bin"), Some(7));
        assert_eq!(experte_aus_rest("mlp_shared_expert_down.bin"), None);
        assert_eq!(experte_aus_rest("mlp_experts_x_down.bin"), None);
        assert_eq!(experte_aus_rest("mlp_experts_gate_up_proj.bin"), None, "ein gepackter Name ist kein Experte");
    }
}
