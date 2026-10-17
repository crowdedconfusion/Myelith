//! **Farben und Merkmale, die als Text immer ANSI schreiben.**
//!
//! # 📌 Warum es dieses Modul gibt (2026-10-17, unter Wine gefunden)
//!
//! Die Konsole baut ihre Zeilen als Zeichenketten: `format!("{}{text}{}",
//! SetForegroundColor(..), ResetColor)`. crossterm 0.29 setzt diese
//! Befehle beim Formatieren unter Windows **nicht als Text**, wenn es keine
//! ANSI-faehige Konsole findet, sondern ruft die Windows-API; gelingt das
//! nicht, gibt `Display` einen Fehler, und `format!` bricht mit einer Panik
//! ab (`a formatting trait implementation returned an error`). Ohne Konsole,
//! also in der CI oder bei umgeleiteter Ausgabe, fielen damit 15 Proben und
//! jede Zeile, die Farbe traegt.
//!
//! ⚑ **Die Huellen hier schreiben als Text immer den ANSI-Code**, und als
//! crossterm-Befehl (`queue!`, `execute!`) reichen sie alles weiter, auch
//! den Weg ueber die Windows-API. Im echten Terminal aendert sich damit
//! nichts: Dort schaltet crossterm unter Windows die ANSI-Verarbeitung ein,
//! und die Zeichenketten trugen schon bisher ANSI.

use std::fmt;

use crossterm::style::{Attribute, Color};
use crossterm::Command;

/// Schreibt einen crossterm-Befehl als ANSI-Text, ohne Rueckfrage beim
/// Betriebssystem.
fn als_ansi(befehl: &impl Command, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    befehl.write_ansi(f)
}

macro_rules! huelle {
    ($(#[$doku:meta])* $name:ident $(($feld:ty))?) => {
        $(#[$doku])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name $((pub $feld))?;

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                als_ansi(&self.innen(), f)
            }
        }

        impl Command for $name {
            fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
                self.innen().write_ansi(f)
            }
            #[cfg(windows)]
            fn execute_winapi(&self) -> std::io::Result<()> {
                self.innen().execute_winapi()
            }
            #[cfg(windows)]
            fn is_ansi_code_supported(&self) -> bool {
                self.innen().is_ansi_code_supported()
            }
        }
    };
}

huelle!(
    /// Wie `crossterm::style::SetForegroundColor`.
    SetForegroundColor(Color)
);
huelle!(
    /// Wie `crossterm::style::SetBackgroundColor`.
    SetBackgroundColor(Color)
);
huelle!(
    /// Wie `crossterm::style::SetAttribute`.
    SetAttribute(Attribute)
);
huelle!(
    /// Wie `crossterm::style::ResetColor`.
    ResetColor
);

impl SetForegroundColor {
    fn innen(&self) -> crossterm::style::SetForegroundColor {
        crossterm::style::SetForegroundColor(self.0)
    }
}
impl SetBackgroundColor {
    fn innen(&self) -> crossterm::style::SetBackgroundColor {
        crossterm::style::SetBackgroundColor(self.0)
    }
}
impl SetAttribute {
    fn innen(&self) -> crossterm::style::SetAttribute {
        crossterm::style::SetAttribute(self.0)
    }
}
impl ResetColor {
    fn innen(&self) -> crossterm::style::ResetColor {
        crossterm::style::ResetColor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Als Text derselbe Code wie crossterm**, nur ohne Rueckfrage.
    #[test]
    fn der_text_ist_der_ansi_code() {
        let mut erwartet = String::new();
        crossterm::style::SetForegroundColor(Color::Rgb { r: 1, g: 2, b: 3 }).write_ansi(&mut erwartet).unwrap();
        assert_eq!(format!("{}", SetForegroundColor(Color::Rgb { r: 1, g: 2, b: 3 })), erwartet);
        assert_eq!(format!("{ResetColor}"), "\u{1b}[0m");
        assert_eq!(format!("{}", SetAttribute(Attribute::Bold)), "\u{1b}[1m");
    }
}
