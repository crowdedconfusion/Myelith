//! Der Trainingsschritt als Ganzes: vorwärts, rückwärts, fortschreiben.
//!
//! # ⚑ Warum es dieses Modul gibt
//!
//! `backward` liefert Gradienten, `optimierer` schreibt Gewichte fort,
//! und beide sind gegen Golden-Vektoren geprüft. **Bis zum 2026-09-01
//! hatte keiner der beiden einen Aufrufer**: `linear_backward`,
//! `moe_backward`, `attention_backward`, `rmsnorm_backward`,
//! `silu_backward` und `optimierer::schritt` wurden von nichts außerhalb
//! ihrer eigenen Tests gerufen.
//!
//! Die Verdrahtung zur Schleife stand als offener Punkt fest, war also
//! bekannt und kein Fund. **Aber es ist dieselbe
//! Lage**, und sie hat dieselbe Folge: Einzeln geprüfte Teile sagen
//! nichts über ihr Zusammenspiel. ⚑ **Wo Ganzzahltraining bricht, ist
//! nicht der einzelne Kern, sondern die Skala zwischen zweien.**
//!
//! # ⚑ Die Brücke, die fehlte: Master zu Übertragungsgewicht
//!
//! Der Optimierer rechnet auf `Master` (i32, mit [`FEIN_BITS`]
//! Bruchstellen darunter), der Vorwärtspass will `i8` mit einer Skala je
//! Zeile. **Diese Umrechnung gab es nirgends**, und ohne sie ist der
//! Kreis nicht zu schließen: Man kann fortschreiben oder rechnen, nicht
//! beides.
//!
//! # Was hier noch nicht steht
//!
//! Ein **Modell**. Dieses Modul schließt den Kreis für eine lineare
//! Ebene; die Schleife über ein ganzes Netz braucht einen Vorwärtspass,
//! der seine Zwischenwerte behält, und der Vorwärtspass der Laufzeit ist
//! auf Inferenz zugeschnitten und behält nichts. Das ist eigene Arbeit
//! und ein eigener offener Punkt.

use crate::backward::{linear_backward, Grad};
use crate::linear::linear_w8a16;
use crate::optimierer::{schritt, Master, Schrittkennung};

/// Wandelt Mastergewichte in die Übertragungsform: `i8` je Wert und eine
/// Verschiebung je Ausgabezeile.
///
/// # ⚑ Je Zeile eine Skala, nicht eine für alles
///
/// `linear_w8a16` erwartet `w_shifts` mit einem Eintrag je Ausgabezeile,
/// und das ist kein Zufall: Zwei Zeilen eines Gewichts können sich um
/// Größenordnungen unterscheiden. Eine gemeinsame Skala richtete sich
/// nach der größten und ließe die kleine auf null zusammenfallen.
///
/// # ⚑ Der Nullfall ist eine eigene Aussage
///
/// Eine Zeile, die nur Nullen enthält, hat keine sinnvolle Skala. Sie
/// bekommt Verschiebung `0`, und das ist richtig: Aus Nullen wird bei
/// jeder Verschiebung wieder null, und eine erfundene Skala wäre eine
/// Zahl ohne Deckung.
pub fn gewicht_aus_master(
    master: &[Master],
    in_features: usize,
) -> (Vec<i8>, Vec<u8>) {
    assert!(in_features > 0, "gewicht_aus_master: in_features muss > 0 sein");
    assert_eq!(
        master.len() % in_features,
        0,
        "gewicht_aus_master: {} Master passen nicht zu Zeilen à {}",
        master.len(),
        in_features
    );
    let zeilen = master.len() / in_features;
    let mut w = vec![0i8; master.len()];
    let mut shifts = vec![0u8; zeilen];

    for z in 0..zeilen {
        let bereich = &master[z * in_features..(z + 1) * in_features];
        let groesster = bereich.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
        if groesster == 0 {
            continue;
        }
        // Wie weit muss nach rechts geschoben werden, damit der groesste
        // Betrag in i8 passt? `127` ist der groesste darstellbare Betrag.
        let mut s = 0u32;
        while (groesster >> s) > 127 {
            s += 1;
        }
        shifts[z] = u8::try_from(s).unwrap_or(u8::MAX);
        for (i, v) in bereich.iter().enumerate() {
            let gerundet = crate::fixed_point::rshift_round_i64(
                i64::from(*v),
                u8::try_from(s).unwrap_or(u8::MAX),
            );
            w[z * in_features + i] = gerundet.clamp(-127, 127) as i8;
        }
    }
    (w, shifts)
}

/// Ein vollständiger Schritt auf **einer** linearen Ebene.
///
/// Vorwärts mit dem aus dem Master gewonnenen Gewicht, Verlustgradient
/// gegen das Ziel, rückwärts, fortschreiben. Gibt den quadratischen
/// Abstand **vor** dem Schritt zurück, damit ein Aufrufer sieht, wohin
/// es geht.
///
/// ⚑ **Der Verlustgradient ist hier `2·(y − ziel)`**, also der des
/// quadratischen Abstands. Das ist keine Trainingsvorschrift, sondern
/// die einfachste, an der sich zeigen lässt, **dass der Kreis
/// geschlossen ist**: Wenn er stimmt, sinkt der Abstand.
/// Die Zahlen, mit denen ein Schritt gerechnet wird.
///
/// ⚑ **Zusammengefasst, weil neun Argumente niemand richtig übergibt.**
/// Bei einer Reihe gleichartiger Zahlen fällt eine Vertauschung nicht
/// auf: `act_frac_bits` und `out_frac_bits` sind beide `u8`, Zähler und
/// Nenner beide `i64`. In einem Typ tragen sie ihren Namen mit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schrittvorgaben {
    /// Spaltenzahl des Gewichts.
    pub in_features: usize,
    /// Bruchstellen der Eingabeaktivierungen.
    pub act_frac_bits: u8,
    /// Bruchstellen der Ausgabe.
    pub out_frac_bits: u8,
    /// Zähler der Lernrate.
    pub lr_zaehler: i64,
    /// Nenner der Lernrate. **Muss größer als null sein.**
    pub lr_nenner: i64,
    /// Ebene, Schritt und Versatz für den Würfel.
    pub kennung: Schrittkennung,
}

pub fn schritt_auf_linear(
    master: &mut [Master],
    x: &[i16],
    ziel: &[i16],
    v: Schrittvorgaben,
) -> i64 {
    let Schrittvorgaben {
        in_features,
        act_frac_bits,
        out_frac_bits,
        lr_zaehler,
        lr_nenner,
        kennung,
    } = v;
    let (w, shifts) = gewicht_aus_master(master, in_features);
    let y = linear_w8a16(x, &w, in_features, &shifts, act_frac_bits, out_frac_bits);
    assert_eq!(y.len(), ziel.len(), "schritt_auf_linear: Ziel passt nicht zur Ausgabe");

    let mut abstand = 0i64;
    let mut g: Vec<Grad> = Vec::with_capacity(y.len());
    for (a, b) in y.iter().zip(ziel) {
        let d = i64::from(*a) - i64::from(*b);
        abstand += d * d;
        // Faktor zwei des quadratischen Abstands; die Lernrate traegt
        // ihn ohnehin mit, aber er gehoert in den Gradienten und nicht
        // in eine stillschweigend halbierte Rate.
        g.push((2 * d).clamp(i64::from(Grad::MIN), i64::from(Grad::MAX)) as Grad);
    }

    let (_gx, gw) = linear_backward(
        &g,
        x,
        &w,
        in_features,
        &shifts,
        out_frac_bits,
        act_frac_bits,
    );
    // ⚑ `linear_backward` liefert `dL/dW` als `i64` je Gewicht. Der
    // Optimierer nimmt `Grad = i32`; gesaettigt statt umlaufend, denn
    // ein umgelaufener Gradient zeigt in die **Gegenrichtung**.
    let gw32: Vec<Grad> = gw
        .iter()
        .map(|v| (*v).clamp(i64::from(Grad::MIN), i64::from(Grad::MAX)) as Grad)
        .collect();
    schritt(master, &gw32, kennung, lr_zaehler, lr_nenner);
    abstand
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Der Kreis ist geschlossen: Der Abstand sinkt.**
    ///
    /// Das ist die Aussage, die kein einzelner Golden-Vektor treffen
    /// kann. Jeder Kern kann fuer sich richtig sein und die Kette
    /// trotzdem in die falsche Richtung laufen, wenn eine Skala
    /// zwischen zweien nicht passt.
    #[test]
    fn der_abstand_sinkt_ueber_mehrere_schritte() {
        let in_features = 8usize;
        let zeilen = 4usize;
        // ⚑ **Ein Master steht in Rasterstufen, nicht in feinen
        // Einheiten.** Die erste Fassung dieses Tests schob ihn um
        // `FEIN_BITS` nach links; dann lagen die Werte bei fuenf
        // Millionen, `gewicht_aus_master` schob sie um sechzehn Stellen
        // zurueck, und ein Schritt von wenigen hundert war danach
        // unsichtbar. **Der Abstand blieb exakt stehen**, und der Test
        // hat damit eine Skalenverwechslung gefunden, die kein
        // Golden-Vektor je zeigen wuerde: `FEIN_BITS` sind
        // **unterhalb** der Rasterstufe und gehoeren in den Schritt,
        // nicht in die Darstellung.
        let mut master: Vec<Master> = (0..zeilen * in_features)
            .map(|i| (i as i32 * 37) % 11 - 5)
            .collect();
        let x: Vec<i16> = (0..in_features).map(|i| ((i * 13) % 9) as i16 - 4).collect();
        let ziel: Vec<i16> = (0..zeilen).map(|i| (i as i16 + 1) * 40).collect();

        // ⚑ **Die Lernrate ist gerechnet, nicht geraten.** Der Schritt
        // ist `g · lr · 2^FEIN_BITS / nenner` in feinen Einheiten, also
        // `g · lr / nenner` in Rasterstufen. Mit Gradienten in der
        // Groessenordnung zweihundert und einem Ziel von etwa einer
        // Fuenftel Rasterstufe je Schritt folgt `nenner ≈ 1000`.
        //
        // ⛑ Mit `1/4` stieg der Abstand von 64 214 auf 80 089: Der
        // Schritt sprang ueber das Ziel hinaus. **Das ist kein Fehler
        // der Kerne, sondern eine Lernrate, die nicht zur Skala passt**,
        // und es ist genau die Sorte Fehler, die einzeln gepruefte
        // Kerne nicht zeigen koennen.
        let mut erster = 0i64;
        let mut letzter = 0i64;
        for s in 0..300u64 {
            let a = schritt_auf_linear(
                &mut master,
                &x,
                &ziel,
                Schrittvorgaben {
                    in_features,
                    act_frac_bits: 8,
                    out_frac_bits: 8,
                    lr_zaehler: 1,
                    lr_nenner: 1_000,
                    kennung: Schrittkennung { ebene: 0, schritt: s, index_versatz: 0 },
                },
            );
            if s == 0 {
                erster = a;
            }
            letzter = a;
        }
        assert!(
            letzter < erster,
            "der Abstand stieg: {erster} auf {letzter}. Ein Kern kann fuer sich \
             richtig sein und die Kette trotzdem falsch herum laufen"
        );
    }

    /// Je Zeile eine eigene Skala, sonst faellt die kleine auf null.
    #[test]
    fn zwei_zeilen_bekommen_zwei_skalen() {
        let master = vec![
            1_000_000, 2_000_000, // grosse Zeile
            3, 4, // kleine Zeile
        ];
        let (w, s) = gewicht_aus_master(&master, 2);
        assert_ne!(s[0], s[1], "beide Zeilen bekamen dieselbe Skala");
        // ⚑ Die kleine Zeile ueberlebt: Mit gemeinsamer Skala waere sie
        // null.
        assert!(w[2] != 0 || w[3] != 0, "die kleine Zeile fiel auf null");
    }

    /// ⚑ **Eine Nullzeile bekommt keine erfundene Skala.**
    #[test]
    fn eine_nullzeile_bleibt_null() {
        let master = vec![0, 0, 0, 5];
        let (w, s) = gewicht_aus_master(&master, 2);
        assert_eq!(s[0], 0, "eine Nullzeile braucht keine Verschiebung");
        assert_eq!(&w[..2], &[0, 0]);
    }

    /// Der groesste Betrag passt nach der Verschiebung in `i8`.
    #[test]
    fn nach_der_verschiebung_passt_alles_in_i8() {
        for gross in [127i32, 128, 1_000, i32::MAX / 2] {
            let master = vec![gross, -gross, 1, 0];
            let (w, s) = gewicht_aus_master(&master, 4);
            assert!(
                w.iter().all(|v| (-127..=127).contains(&i32::from(*v))),
                "Verschiebung {} reichte fuer {gross} nicht",
                s[0]
            );
        }
    }
}
