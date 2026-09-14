//! Welche Farben dieses Programm benutzt.
//!
//! # ⚑ Fuenf Bilder, und eines davon ist keines
//!
//! `Standard` setzt **keine** eigene Farbe: Wer sein Terminal
//! eingerichtet hat, hat damit schon gewaehlt, und ein Programm, das
//! sich darueberlegt, nimmt ihm die Wahl wieder weg. Die anderen vier
//! setzen drei Toene, mehr nicht: eine Kante, ein Beiwerk, einen
//! Akzent.
//!
//! ⚑ **Drei Toene und nicht dreissig.** Ein Bild aus wenigen Farben
//! bleibt lesbar, wenn jemand einen hellen Hintergrund hat; eines aus
//! vielen sieht auf genau einem Schirm gut aus, naemlich dem, auf dem
//! es gebaut wurde.

use crossterm::style::Color;

use myl_client::einstellungen::Konsolendesign;

/// Die drei Toene eines Designs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toene {
    /// Rahmen und Linien.
    pub kante: Color,
    /// Fusszeilen, Hinweise, alles Nebensaechliche.
    pub beiwerk: Color,
    /// Was hervorgehoben wird.
    pub akzent: Color,
}

/// Die Toene dieses Designs.
///
/// ⚑ **`Color::Reset` ist die Vordergrundfarbe des Terminals**, nicht
/// Weiss. Genau das meint `Standard`.
pub fn toene(d: Konsolendesign) -> Toene {
    match d {
        Konsolendesign::Standard => Toene {
            // ⚑ Auch im Standard wird das Beiwerk gedaempft, aber ueber
            // die **ANSI-Graustufe** und nicht ueber einen eigenen
            // Farbwert: Sie folgt dem Schema des Terminals.
            kante: Color::AnsiValue(248),
            beiwerk: Color::AnsiValue(244),
            akzent: Color::Reset,
        },
        Konsolendesign::Myelith => Toene {
            kante: Color::AnsiValue(248),
            beiwerk: Color::AnsiValue(244),
            akzent: crate::farben::schlagwort(),
        },
        Konsolendesign::Bernstein => Toene {
            kante: Color::Rgb { r: 138, g: 86, b: 0 },
            beiwerk: Color::Rgb { r: 178, g: 116, b: 0 },
            akzent: Color::Rgb { r: 255, g: 176, b: 0 },
        },
        Konsolendesign::Tiefsee => Toene {
            kante: Color::Rgb { r: 42, g: 78, b: 120 },
            beiwerk: Color::Rgb { r: 96, g: 132, b: 168 },
            akzent: Color::Rgb { r: 92, g: 224, b: 232 },
        },
        Konsolendesign::Tinte => Toene {
            kante: Color::Rgb { r: 96, g: 96, b: 96 },
            beiwerk: Color::Rgb { r: 140, g: 140, b: 140 },
            akzent: Color::Rgb { r: 236, g: 236, b: 236 },
        },
    }
}

/// **Die Farbe des Ladetextes zu diesem Zeitpunkt.**
///
/// ⚑ **Im Regenbogen oder in einer Farbe**, je nach Design: Ein
/// Regenbogen in einem einfarbigen Bild ist kein Akzent, sondern ein
/// Fremdkoerper. Dort pulst stattdessen die Helle derselben Farbe, im
/// selben Takt von zwei Minuten.
pub fn ladefarbe(d: Konsolendesign, seit: std::time::Duration) -> Color {
    if d.regenbogen() {
        let (r, g, b) = crate::anzeige::regenbogen(seit);
        return Color::Rgb { r, g, b };
    }
    let Color::Rgb { r, g, b } = toene(d).akzent else {
        return toene(d).akzent;
    };
    let anteil = puls(seit);
    Color::Rgb {
        r: (r as u32 * anteil / 255) as u8,
        g: (g as u32 * anteil / 255) as u8,
        b: (b as u32 * anteil / 255) as u8,
    }
}

/// Ein Dreieckspuls zwischen einem Drittel und voller Helle.
///
/// ⚑ **Ganzzahlig und stetig.** Er steigt eine Minute lang und faellt
/// eine Minute lang; am Umkehrpunkt gibt es keinen Sprung, weil Anfang
/// und Ende denselben Wert haben.
pub fn puls(seit: std::time::Duration) -> u32 {
    const UNTEN: u32 = 85;
    const HALB: u128 = 60_000;
    let t = seit.as_millis() % (HALB * 2);
    let auf = if t < HALB { t } else { HALB * 2 - t };
    UNTEN + ((255 - UNTEN) as u128 * auf / HALB) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Standard setzt keinen eigenen Akzent.**
    ///
    /// ⚑ Die Zusage des Designs steht damit im Typ und nicht in einem
    /// Satz darueber.
    #[test]
    fn standard_nimmt_die_farbe_des_terminals() {
        assert_eq!(toene(Konsolendesign::Standard).akzent, Color::Reset);
    }

    /// **Jedes andere Design setzt drei verschiedene Toene.**
    ///
    /// 📌 Zwei gleiche Toene waeren ein Rahmen, den man von seiner
    /// Fusszeile nicht unterscheidet.
    #[test]
    fn jedes_design_hat_drei_unterscheidbare_toene() {
        for d in Konsolendesign::ALLE {
            if d == Konsolendesign::Standard {
                continue;
            }
            let t = toene(d);
            assert_ne!(t.kante, t.beiwerk, "{} hat Kante und Beiwerk gleich", d.name());
            assert_ne!(t.beiwerk, t.akzent, "{} hat Beiwerk und Akzent gleich", d.name());
        }
    }

    /// **Der Puls springt am Umkehrpunkt nicht.**
    ///
    /// 📌 Ein Sprung von voller Helle auf ein Drittel sieht aus wie ein
    /// Fehler und nicht wie eine Bewegung.
    #[test]
    fn der_puls_bleibt_stetig() {
        use std::time::Duration;
        let mut vorher = puls(Duration::ZERO);
        for ms in (0..240_000).step_by(500) {
            let jetzt = puls(Duration::from_millis(ms));
            let sprung = jetzt.abs_diff(vorher);
            assert!(sprung < 10, "bei {ms} ms springt der Puls um {sprung}");
            vorher = jetzt;
        }
    }

    /// **Und er bleibt in seinen Grenzen.**
    #[test]
    fn der_puls_bleibt_im_rahmen() {
        use std::time::Duration;
        for ms in (0..240_000).step_by(377) {
            let p = puls(Duration::from_millis(ms));
            assert!((85..=255).contains(&p), "bei {ms} ms ist der Puls {p}");
        }
    }

    /// **Ein einfarbiges Design bekommt keinen Regenbogen.**
    #[test]
    fn einfarbige_designs_pulsen_statt_zu_wandern() {
        assert!(!Konsolendesign::Bernstein.regenbogen());
        assert!(!Konsolendesign::Tinte.regenbogen());
        assert!(Konsolendesign::Myelith.regenbogen());
    }
}
