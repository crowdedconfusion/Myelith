//! **Die kausale Faltung der Zustandsschicht**, tiefenweise und ganzzahlig.
//!
//! Vor der Rekurrenz laeuft `q`, `k` und `v` gemeinsam durch eine
//! Faltung ueber [`KERN`] Stellen, je Kanal getrennt (tiefenweise, in der
//! Vorlage `groups = kanaele`), gefolgt von SiLU:
//!
//! ```text
//! aus[c][t] = silu( Summe_j  w[c][j] * ein[c][t - (KERN-1) + j] )
//! ```
//!
//! ⚑ **Tiefenweise heisst: kein Mischen ueber Kanaele.** Jeder Kanal hat
//! seine eigenen [`KERN`] Gewichte, und die Faltung ist damit kein GEMM,
//! sondern `kanaele` kleine Skalarprodukte. Beim grossen Modell sind es
//! 8192 Kanaele mal vier Stellen.
//!
//! # ⚑ Warum ein Fenster und keine zweite Fassung fuer den Vorlauf
//!
//! Die Faltung braucht die letzten `KERN - 1` Eingaenge. Beim Dekodieren
//! kommt je Aufruf ein Token, also muss der Rest irgendwo stehen; das ist
//! das [`Faltungsfenster`].
//!
//! ⛔️ **Es gibt bewusst nur diesen einen Weg.** Ein Vorlauf, der die
//! ganze Folge auf einmal faltet, waere schneller und **eine zweite
//! Umsetzung derselben Rechnung**. Genau dort entstehen Unterschiede
//! zwischen Vorlauf und Dekodieren, die niemand sieht, weil beide fuer
//! sich plausibel aussehen. Der Vorlauf ruft deshalb dieselbe Funktion in
//! einer Schleife.
//!
//! # ⚠️ Der Anfang einer Folge ist null und nicht der erste Wert
//!
//! Die Vorlage faltet mit `padding = KERN - 1`, fuellt also links mit
//! Nullen. Ein leeres Fenster traegt deshalb Nullen; eine Fortsetzung mit
//! dem ersten Wert waere eine andere Schicht.

use crate::fixed_point::clamp_i16_from_i64;
use crate::integer_math::silu_zerlegt;

/// Die Breite des Faltungskerns.
///
/// ⚑ **Als Konstante und nicht als Feld**, weil das Fenster sonst je
/// Kanal eine variable Laenge haette und die Schleife unten ihre
/// Grenzen zur Laufzeit holen muesste. Traegt ein Modell einen anderen
/// Kern, faellt die Zusicherung in [`Faltungsfenster::leer`] auf, statt
/// still falsch zu rechnen.
pub const KERN: usize = 4;

/// **Die letzten `KERN - 1` Eingaenge je Kanal.**
///
/// ⚑ **Aelteste zuerst**, damit die Schleife unten in derselben
/// Richtung laeuft wie die Gewichte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Faltungsfenster {
    pub kanaele: usize,
    werte: Vec<i16>,
}

impl Faltungsfenster {
    /// Ein leeres Fenster, wie es am Anfang einer Folge steht.
    pub fn leer(kanaele: usize, kern: usize) -> Self {
        assert_eq!(
            kern, KERN,
            "dieses Modul rechnet einen Kern von {KERN} Stellen, nicht {kern}"
        );
        Self { kanaele, werte: vec![0; kanaele * (KERN - 1)] }
    }

    /// Ein einzelner gemerkter Eingang, `alter = 0` ist der aelteste.
    #[inline]
    pub fn get(&self, kanal: usize, alter: usize) -> i16 {
        self.werte[kanal * (KERN - 1) + alter]
    }

    /// Setzt einen gemerkten Eingang. Fuer Proben und fuer das
    /// Wiederherstellen nach einer Uebernahme.
    #[inline]
    pub fn set(&mut self, kanal: usize, alter: usize, wert: i16) {
        self.werte[kanal * (KERN - 1) + alter] = wert;
    }

    /// Alles auf null, wie am Anfang einer Folge.
    pub fn leeren(&mut self) {
        self.werte.iter_mut().for_each(|w| *w = 0);
    }
}

/// **Ein Schritt der Faltung**, fuer ein Token ueber alle Kanaele.
///
/// `ein` liegt in `2^-ein_frac`, `gewicht` ist int8 mit einer Schiebung
/// je Kanal, und `aus` kommt in `2^-aus_frac`.
///
/// ⚑ **Die Gewichte liegen zeilenweise je Kanal**
/// (`gewicht[c * KERN + j]`), und `j = 0` gehoert zum **aeltesten**
/// Eingang. Dieselbe Richtung wie im Fenster.
///
/// ⚠️ **Das Fenster wird am Ende fortgeschrieben, nicht am Anfang.** Der
/// aktuelle Eingang gehoert in die Summe dieses Schritts; wer zuerst
/// schiebt, rechnet um eine Stelle versetzt.
#[allow(clippy::too_many_arguments)]
pub fn schritt(
    fenster: &mut Faltungsfenster,
    ein: &[i16],
    gewicht: &[i8],
    w_schiebungen: &[u8],
    ein_frac: u8,
    sigmoid_lut: &[i16],
    sigmoid_versatz: i16,
    sigmoid_ein_frac: u8,
    sigmoid_aus_frac: u8,
    // **Eine Ausgangsskala je Kanal** (Fund 421). Abfrage, Schluessel
    // und Wert kommen aus einer Matrixmultiplikation, sind aber drei
    // Groessen: `q` und `k` werden gleich darauf auf Einheitslaenge
    // gebracht und brauchen Aufloesung, `v` traegt als einziges eine
    // Groesse. Eine gemeinsame Skala richtet sich nach `v` und laesst
    // den beiden anderen drei bis vier Bit.
    aus_fracs: &[u8],
    aus: &mut [i16],
) {
    let c = fenster.kanaele;
    assert_eq!(ein.len(), c, "der Eingang passt nicht zur Kanalzahl");
    assert_eq!(aus.len(), c, "die Ausgabe passt nicht zur Kanalzahl");
    assert_eq!(gewicht.len(), c * KERN, "ein Kern von {KERN} Stellen je Kanal");
    assert_eq!(w_schiebungen.len(), c, "eine Schiebung je Kanal");
    assert_eq!(aus_fracs.len(), c, "eine Ausgangsskala je Kanal (Fund 421)");

    for kanal in 0..c {
        let w = &gewicht[kanal * KERN..(kanal + 1) * KERN];
        let basis = kanal * (KERN - 1);

        // --- 1. Die Summe ueber die letzten KERN Stellen.
        //
        // ⚑ **i64 und nicht i32.** Vier Summanden aus int8 mal int16
        // passen zwar in i32; die Summe geht aber danach durch
        // `rescale_i64`, und ein Wechsel des Typs mitten in der
        // Rechnung ist eine Stelle, an der jemand spaeter eine
        // Saettigung uebersieht.
        let mut akku: i64 = 0;
        for j in 0..KERN - 1 {
            akku += i64::from(w[j]) * i64::from(fenster.werte[basis + j]);
        }
        akku += i64::from(w[KERN - 1]) * i64::from(ein[kanal]);

        // --- 2. SiLU als Zerlegung, nicht als Tabelle ueber `x`.
        //
        // ⚑ **Nach der Summe und nicht davor.** Die Vorlage faltet
        // zuerst und aktiviert dann; umgekehrt waere es eine andere
        // Funktion.
        //
        // ⛔️ **Und `x` geht ungerastert hinein** (Fund 417). Bis zum
        // 2026-09-22 stand hier ein `silu_nachschlagen`, das `akku`
        // zuerst auf das Eingangsraster der SiLU-Tabelle brachte. Fuer
        // `q` und `k` liegt dieser Zweig zwei Rasterschritte ueber null;
        // die Tabelle loeschte sie damit aus. Die Begruendung steht bei
        // [`silu_zerlegt`].
        aus[kanal] = clamp_i16_from_i64(silu_zerlegt(
            akku,
            w_schiebungen[kanal] + ein_frac,
            sigmoid_lut,
            sigmoid_versatz,
            sigmoid_ein_frac,
            sigmoid_aus_frac,
            aus_fracs[kanal],
        ));

        // --- 4. Das Fenster nachziehen.
        //
        // ⚑ **Schieben statt Ringpuffer.** Bei `KERN - 1 = 3` sind es
        // drei Zuweisungen je Kanal; ein Ringpuffer spart sie und
        // handelt sich dafuer eine Positionsrechnung ein, die bei einer
        // Uebernahme mitgefuehrt werden muesste. **Die Rechnung ist hier
        // nicht der Engpass**, die Gewichte sind es.
        for j in 0..KERN - 2 {
            fenster.werte[basis + j] = fenster.werte[basis + j + 1];
        }
        fenster.werte[basis + KERN - 2] = ein[kanal];
    }
}

#[cfg(test)]
mod proben {
    use super::*;

    /// Eine Sigmoid-Tabelle, wie das Modell sie mitbringt.
    ///
    /// ⚑ **Sigmoid und nicht SiLU** (Fund 417): Die Faltung schlaegt
    /// seit dem 2026-09-22 nur noch den **Faktor** nach.
    fn sigmoid_tabelle(ein_frac: u8, aus_frac: u8, versatz: i16) -> Vec<i16> {
        let n = 1usize << 13;
        (0..n)
            .map(|i| {
                let x = (i as f64 - versatz as f64) / (1u32 << ein_frac) as f64;
                let y = 1.0 / (1.0 + (-x).exp());
                (y * (1u32 << aus_frac) as f64).round().clamp(-32768.0, 32767.0) as i16
            })
            .collect()
    }

    /// Was die Faltung fuer einen einzelnen Wert liefern muss.
    fn erwartet_aus(akku: i64, x_frac: u8, lut: &[i16]) -> i16 {
        clamp_i16_from_i64(silu_zerlegt(akku, x_frac, lut, 4096, 8, 8, 8))
    }

    /// **Ein leeres Fenster und ein Kern, der nur die letzte Stelle
    /// nimmt, ergibt silu(x).**
    ///
    /// ⚑ Das ist die Faltung an ihrem einfachsten Fall: Wenn der nicht
    /// stimmt, stimmt nichts.
    #[test]
    fn der_einfachste_kern_gibt_silu_zurueck() {
        let c = 3;
        let mut f = Faltungsfenster::leer(c, KERN);
        let mut aus = vec![0i16; c];
        // w = [0, 0, 0, 1] mit Schiebung 0, also Faktor 1.
        let mut w = vec![0i8; c * KERN];
        for kanal in 0..c {
            w[kanal * KERN + KERN - 1] = 1;
        }
        let lut = sigmoid_tabelle(8, 8, 4096);
        let ein = vec![256i16, -256, 0];
        schritt(&mut f, &ein, &w, &vec![0u8; c], 8, &lut, 4096, 8, 8, &vec![8u8; c], &mut aus);

        for (i, &x) in ein.iter().enumerate() {
            let erwartet = erwartet_aus(i64::from(x), 8, &lut);
            assert_eq!(aus[i], erwartet, "Kanal {i}");
        }
    }

    /// ⛔️ **Das Fenster wirkt, und zwar an der richtigen Stelle.**
    ///
    /// Ein Kern, der nur den **aeltesten** Eingang nimmt, muss einen
    /// Ausschlag um genau `KERN - 1` Schritte verzoegert wiedergeben.
    /// 📌 Ohne diese Probe bliebe die Faltung auch dann gruen, wenn das
    /// Fenster gar nicht gelesen wuerde.
    #[test]
    fn der_aelteste_eingang_kommt_verzoegert_an() {
        let c = 1;
        let mut f = Faltungsfenster::leer(c, KERN);
        let mut aus = vec![0i16; c];
        let mut w = vec![0i8; KERN];
        w[0] = 1; // nur die aelteste Stelle
        let lut = sigmoid_tabelle(8, 8, 4096);
        let schiebungen = vec![0u8; c];

        // Ein einzelner Ausschlag, danach Nullen.
        let mut gesehen = Vec::new();
        for t in 0..6 {
            let ein = vec![if t == 0 { 512i16 } else { 0 }];
            schritt(&mut f, &ein, &w, &schiebungen, 8, &lut, 4096, 8, 8, &vec![8u8; c], &mut aus);
            gesehen.push(aus[0]);
        }
        let silu_512 = erwartet_aus(512, 8, &lut);
        let silu_0 = erwartet_aus(0, 8, &lut);
        // Der Ausschlag erscheint bei t = KERN - 1 = 3.
        assert_eq!(gesehen[KERN - 1], silu_512, "der Ausschlag kommt zu frueh oder gar nicht");
        for (t, &g) in gesehen.iter().enumerate() {
            if t != KERN - 1 {
                assert_eq!(g, silu_0, "Schritt {t} traegt etwas, das dort nicht hingehoert");
            }
        }
    }

    /// **Der Anfang einer Folge ist null**, wie bei der Vorlage mit
    /// `padding = KERN - 1`.
    #[test]
    fn ein_leeres_fenster_traegt_nullen() {
        let f = Faltungsfenster::leer(5, KERN);
        for kanal in 0..5 {
            for alter in 0..KERN - 1 {
                assert_eq!(f.get(kanal, alter), 0);
            }
        }
    }

    /// **Zweimal derselbe Lauf ergibt dasselbe Fenster, Bit fuer Bit.**
    #[test]
    fn zwei_laeufe_ergeben_dasselbe_fenster() {
        let bauen = || {
            let c = 4;
            let mut f = Faltungsfenster::leer(c, KERN);
            let mut aus = vec![0i16; c];
            let w: Vec<i8> = (0..c * KERN).map(|i| ((i as i32 * 37) % 61 - 30) as i8).collect();
            let lut = sigmoid_tabelle(8, 8, 4096);
            for t in 0..32i16 {
                let ein: Vec<i16> = (0..c).map(|i| (t * 13 + i as i16 * 7) % 401 - 200).collect();
                schritt(&mut f, &ein, &w, &vec![3u8; c], 8, &lut, 4096, 8, 8, &vec![8u8; c], &mut aus);
            }
            f
        };
        assert_eq!(bauen(), bauen());
    }

    /// ⚠️ **Die Kanaele mischen sich nicht.**
    ///
    /// 📌 Ein Indexfehler in der Fensterbasis faellt sonst nur dann auf,
    /// wenn zufaellig zwei Kanaele verschiedene Werte tragen, und das ist
    /// keine Probe, sondern Glueck.
    #[test]
    fn ein_kanal_laesst_die_anderen_in_ruhe() {
        let c = 4;
        let mut f = Faltungsfenster::leer(c, KERN);
        let mut aus = vec![0i16; c];
        let mut w = vec![0i8; c * KERN];
        // Nur Kanal 1 hat ein Gewicht, und zwar auf der aeltesten Stelle.
        w[1 * KERN] = 1;
        let lut = sigmoid_tabelle(8, 8, 4096);
        let silu_0 = erwartet_aus(0, 8, &lut);

        for t in 0..KERN {
            // Alle Kanaele bekommen denselben Ausschlag.
            let ein = vec![if t == 0 { 512i16 } else { 0 }; c];
            schritt(&mut f, &ein, &w, &vec![0u8; c], 8, &lut, 4096, 8, 8, &vec![8u8; c], &mut aus);
        }
        // Nach KERN Schritten traegt nur Kanal 1 etwas.
        for kanal in 0..c {
            if kanal == 1 {
                assert_ne!(aus[kanal], silu_0, "Kanal 1 sollte den Ausschlag tragen");
            } else {
                assert_eq!(aus[kanal], silu_0, "Kanal {kanal} traegt etwas Fremdes");
            }
        }
    }
}
