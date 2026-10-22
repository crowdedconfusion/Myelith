//! **Die Ebenenzeit**: wie lange eine Ebene je Token im Decode braucht, auf
//! diesem Rechner und mit diesem Rechenweg.
//!
//! ⚑ **Wozu** (Stufe 4 des Dispatchers, Festlegung des Projektinhabers vom
//! 2026-10-21): Ein Knoten kuendigt dem Netz an, wie viele Ebenen eines
//! Modells er halten kann und wie schnell er sie rechnet; die Verteilung der
//! Ebenen in einem Pod gleicht damit ungleich schnelle Mitglieder aus. Die
//! Zeit ist eine Frist und kein Nachweis: Wer sie schoent, bekommt mehr
//! Ebenen, bremst seinen Pod und verdient nicht mehr, als er leistet.
//!
//! ⚑ **Was gemessen wird** (Festlegung E5): Decode, ein Token, Stapel 1, der
//! **Median** ueber [`PROBE_TOKEN`] Token nach [`ANWAERMEN`] Token zum
//! Anwaermen. Gemessen wird nur der Weg durch die Ebenen, ohne Kopf: Den
//! zaehlt die Verteilung getrennt als Gewicht. Bei einem Modell mit vier
//! Stroemen gibt es diesen Weg einzeln nicht; dort zaehlt `forward_token`
//! mit dem Kopf, also eher zu langsam, und das ist zu Ungunsten des Knotens.
//!
//! ⚑ **Und der Kopf getrennt** (2026-10-21, beim Bau der Simulation gefunden):
//! Fuer den Platz zaehlt er nach Bytes, beim 35B rund vier Ebenen; fuer die
//! Zeit viel mehr, denn eine Gemischebene liest je Token nur die gewaehlten
//! Experten, der Kopf jedes Mal ganz. Gemessen wird er als Unterschied von
//! `forward_token` zum Weg durch die Ebenen, in Ebenenzeiten ausgedrueckt.
//!
//! ⚑ **Ganze Zahlen**: Mikrosekunden, je Ebene aufgerundet und mindestens 1.
//! Die Zahl lenkt nur, sie geht in keine Rechnung des Modells ein.

use crate::kv_cache::KVCache;
use crate::model::IntegerModel;

/// Token zum Anwaermen, nicht gezaehlt.
pub const ANWAERMEN: usize = 4;
/// Token, ueber die der Median geht.
pub const PROBE_TOKEN: usize = 16;

/// Eine Messung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ebenenzeit {
    /// Der Median eines Tokens durch alle Ebenen, in Mikrosekunden.
    pub token_us: u64,
    /// Wie viele Ebenen das Modell hat.
    pub ebenen: usize,
    /// `token_us` je Ebene, aufgerundet, mindestens 1.
    pub je_ebene_us: u64,
    /// Der Median eines Tokens mit Kopf (`forward_token`), in Mikrosekunden.
    pub mit_kopf_us: u64,
    /// Der Kopf in Ebenenzeiten: `(mit_kopf_us − token_us) / je_ebene_us`,
    /// aufgerundet, mindestens 1.
    pub kopf_ebenenzeiten: u64,
}

/// Der Median, bei gerader Zahl der kleinere der beiden mittleren Werte
/// (eine ganze Zahl, kein Mittel zweier).
pub fn median(werte: &[u64]) -> Option<u64> {
    let mut w = werte.to_vec();
    w.sort_unstable();
    w.get(w.len().checked_sub(1)? / 2).copied()
}

/// Je Ebene, aufgerundet, mindestens 1.
pub fn je_ebene(token_us: u64, ebenen: usize) -> u64 {
    let e = ebenen.max(1) as u64;
    token_us.div_ceil(e).max(1)
}

/// **Misst die Ebenenzeit** an einem geladenen Modell.
///
/// Die Eingabe ist eine feste Folge von Token-Kennungen, damit jede Messung
/// dasselbe rechnet; bei einem Gemisch waehlt sie damit auch dieselben
/// Experten. Der KV-Speicher ist eigens fuer die Messung.
pub fn messen(modell: &IntegerModel) -> Result<Ebenenzeit, String> {
    let ebenen = modell.num_layers;
    if ebenen == 0 || modell.vocab_size == 0 {
        return Err("das Modell hat keine Ebenen".into());
    }
    // Zwei Laeufe ueber dieselben Token: einmal nur die Ebenen, einmal mit
    // Kopf, je mit eigenem KV-Speicher.
    let lauf = |mit_kopf: bool| -> Option<u64> {
        let mut cache = KVCache::new(modell.num_layers, modell.num_kv_heads);
        let mut zeiten = Vec::with_capacity(PROBE_TOKEN);
        for pos in 0..ANWAERMEN + PROBE_TOKEN {
            // Feste Kennungen ab 100, innerhalb des Vokabulars.
            let token = (100 + pos * 37) % modell.vocab_size;
            let anfang = std::time::Instant::now();
            if mit_kopf || modell.vierstrom.is_some() {
                let _ = modell.forward_token(token, pos, &mut cache);
            } else {
                let _ = modell.durch_die_ebenen(token, pos, &mut cache);
            }
            let us = anfang.elapsed().as_micros().min(u64::MAX as u128) as u64;
            if pos >= ANWAERMEN {
                zeiten.push(us);
            }
        }
        median(&zeiten)
    };
    let token_us = lauf(false).ok_or("keine Messung")?;
    let mit_kopf_us = lauf(true).ok_or("keine Messung")?;
    let je_ebene_us = je_ebene(token_us, ebenen);
    let kopf_ebenenzeiten = mit_kopf_us.saturating_sub(token_us).div_ceil(je_ebene_us).max(1);
    Ok(Ebenenzeit { token_us, ebenen, je_ebene_us, mit_kopf_us, kopf_ebenenzeiten })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_und_je_ebene_sind_ganzzahlig() {
        assert_eq!(median(&[5, 1, 9]), Some(5));
        assert_eq!(median(&[4, 1, 9, 7]), Some(4), "der kleinere der beiden mittleren");
        assert_eq!(median(&[]), None);
        assert_eq!(je_ebene(1000, 28), 36, "aufgerundet");
        assert_eq!(je_ebene(0, 28), 1, "mindestens 1");
        assert_eq!(je_ebene(10, 0), 10);
    }

    /// Am Anker des Projekts: eine Messung ergibt eine plausible Zahl je
    /// Ebene, und dieselbe Messung zweimal liegt in derselben Groessenordnung.
    #[test]
    fn die_ebenenzeit_des_ankers() {
        let pfad = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../artifacts/sparrow-0.6b-int8");
        if !pfad.join("model_config.json").is_file() {
            eprintln!("übersprungen: kein Artefakt unter {}", pfad.display());
            return;
        }
        let m = crate::loader::load_model(&pfad).expect("laden");
        let a = messen(&m).expect("messen");
        assert_eq!(a.ebenen, m.num_layers);
        assert!(a.je_ebene_us >= 1 && a.token_us >= a.je_ebene_us, "{a:?}");
        assert!(a.kopf_ebenenzeiten >= 1, "{a:?}");
        eprintln!("Ebenenzeit des Ankers: {a:?}");
        let b = messen(&m).expect("messen");
        assert!(b.token_us <= a.token_us * 10 + 1000 && a.token_us <= b.token_us * 10 + 1000, "{a:?} {b:?}");
    }
}
