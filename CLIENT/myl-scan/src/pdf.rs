//! PDF-Seiten als Bild, ohne Fremdprogramm.

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{render, PixmapSettings, RenderCache, RenderSettings};
use image::RgbImage;
use std::sync::mpsc;
use std::time::Duration;

/// Aufloesung fuer alles: Sie traegt kleine Schrift, und feiner bringt nichts.
pub const DPI: f32 = 216.0;

/// ⚑ **Hoechstens so gross wird eine Seite als Bild** (2026-11-12): 16 384
/// Pixel je Kante und 64 Megapixel. A4 bei 216 dpi sind 4,5 Megapixel, A0
/// knapp 70; daruber wird verkleinert. 📌 Ohne Grenze wurde eine Seite mit
/// erfundener `MediaBox` von 20 000 Punkten ein Bild von 60 000 x 60 000
/// Pixeln (rund 25 GB), und eine von 200 000 Punkten brachte `vello_common`
/// zur Panik, weil `hayro` die Breite als `u16` kappt.
pub const KANTE_HOECHSTENS: f32 = 16_384.0;
pub const PIXEL_HOECHSTENS: f32 = 64_000_000.0;

/// Der Massstab fuer eine Seite von `breite` x `hoehe` Punkten: 216 dpi,
/// ausser das Bild wuerde groesser als erlaubt.
pub fn massstab(breite: f32, hoehe: f32) -> f32 {
    let (b, h) = (breite.max(1.0), hoehe.max(1.0));
    (DPI / 72.0).min(KANTE_HOECHSTENS / b.max(h)).min((PIXEL_HOECHSTENS / (b * h)).sqrt())
}

/// Wie viele Seiten das PDF hat.
pub fn seitenzahl(daten: Vec<u8>) -> Result<u32, String> {
    let pdf = Pdf::new(daten).map_err(|e| format!("kein lesbares PDF: {e:?}"))?;
    Ok(pdf.pages().len() as u32)
}

/// ⚑ **So lange darf eine Seite hoechstens rendern**, wenn der Auftrag nichts
/// anderes sagt (2026-11-13). Eine Buchseite braucht unter einer Sekunde,
/// eine dichte Karte einige; zwei Minuten trennen das sicher von einer
/// Schleife an einer praeparierten Seite. `hayro` kennt keinen Abbruch, also
/// wacht der Aufrufer.
pub const FRIST_JE_SEITE: Duration = Duration::from_secs(120);

/// Warum das Rendern endete, ohne alle Seiten zu liefern.
#[derive(Debug, PartialEq)]
pub enum Renderfehler {
    /// Unlesbares PDF, fehlende Seite, Absturz im Renderer oder ein Fehler
    /// aus `f`.
    Fehler(String),
    /// ⛔️ **Diese Seite hat die Frist ueberschritten.** Der Faden, der sie
    /// rendert, laeuft weiter und laesst sich nicht anhalten; der Aufrufer
    /// beendet den Prozess.
    Frist(u32),
}

impl From<String> for Renderfehler {
    fn from(m: String) -> Self {
        Self::Fehler(m)
    }
}

impl std::fmt::Display for Renderfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fehler(m) => f.write_str(m),
            Self::Frist(n) => write!(f, "Seite {n} rendert laenger als erlaubt"),
        }
    }
}

/// Eine Seite als Bild, mit dem Massstab aus [`massstab`].
fn seite_rendern<'a>(pdf: &'a Pdf, cache: &RenderCache<'a>, n: u32) -> Result<RgbImage, String> {
    let seite = pdf.pages().get((n as usize).wrapping_sub(1)).ok_or_else(|| format!("Seite {n} gibt es nicht"))?;
    let (breite, hoehe) = seite.render_dimensions();
    let m = massstab(breite, hoehe);
    let einst = PixmapSettings { x_scale: m, y_scale: m, bg_color: WHITE };
    let bild = render(seite, cache, &InterpreterSettings::default(), &RenderSettings::default(), &einst);
    let (b, h) = (u32::from(bild.width()), u32::from(bild.height()));
    let mut rgb = Vec::with_capacity(b as usize * h as usize * 3);
    for p in bild.data() {
        rgb.extend_from_slice(&[p.r, p.g, p.b]);
    }
    RgbImage::from_raw(b, h, rgb).ok_or_else(|| format!("Seite {n}: falsche Bildgroesse"))
}

/// Ruft `f` fuer jede genannte Seite (ab 1) in aufsteigender Folge; eine
/// Seite darf hoechstens `frist` rendern.
pub fn seiten_rendern(daten: Vec<u8>, seiten: &[u32], frist: Duration, f: impl FnMut(u32, RgbImage) -> Result<(), String>) -> Result<(), Renderfehler> {
    mit_frist(
        seiten,
        frist,
        move |d| match Pdf::new(daten) {
            Ok(pdf) => {
                let cache = RenderCache::new();
                d.bedienen(|n| seite_rendern(&pdf, &cache, n));
            }
            Err(e) => d.fehler(format!("kein lesbares PDF: {e:?}")),
        },
        f,
    )
}

/// Die Seite des Fadens: nimmt Seitennummern an und gibt Bilder ab.
struct Dienst {
    auftraege: mpsc::Receiver<u32>,
    abgabe: mpsc::Sender<Result<Option<RgbImage>, String>>,
}

impl Dienst {
    /// Meldet sich bereit und rendert dann jede gerufene Seite, bis der
    /// Aufrufer auflegt.
    fn bedienen(self, mut r: impl FnMut(u32) -> Result<RgbImage, String>) {
        if self.abgabe.send(Ok(None)).is_err() {
            return;
        }
        for n in self.auftraege {
            if self.abgabe.send(r(n).map(Some)).is_err() {
                return;
            }
        }
    }

    fn fehler(self, m: String) {
        let _ = self.abgabe.send(Err(m));
    }
}

/// Rendert in einem eigenen Faden, Seite fuer Seite auf Zuruf, und wartet je
/// Seite hoechstens `frist`. `arbeiter` laeuft im Faden und oeffnet dort das
/// PDF (ein `Pdf` und sein Cache muessen so nicht zwischen Faeden wandern),
/// unter derselben Frist. Das Bild entsteht erst,
/// wenn `f` das vorige verarbeitet hat; es liegt also nie mehr als eines im
/// Speicher als ohne Faden.
fn mit_frist(
    seiten: &[u32],
    frist: Duration,
    arbeiter: impl FnOnce(Dienst) + Send + 'static,
    mut f: impl FnMut(u32, RgbImage) -> Result<(), String>,
) -> Result<(), Renderfehler> {
    let (zuruf, auftraege) = mpsc::channel::<u32>();
    let (abgabe, bilder) = mpsc::channel::<Result<Option<RgbImage>, String>>();
    // ⚠️ Ein neuer Faden hat 2 MiB Stapel, der Hauptfaden 8 MiB; der
    // Renderer bekommt so viel wie vorher.
    std::thread::Builder::new()
        .name("myl-scan-seiten".into())
        .stack_size(8 << 20)
        .spawn(move || arbeiter(Dienst { auftraege, abgabe }))
        .map_err(|e| format!("Faden fuer die Seiten: {e}"))?;
    let warten = |n: Option<u32>| match bilder.recv_timeout(frist) {
        Ok(b) => b.map_err(Renderfehler::Fehler),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(match n {
            Some(n) => Renderfehler::Frist(n),
            None => Renderfehler::Fehler(format!("das PDF oeffnet sich nicht in {} s", frist.as_secs())),
        }),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(Renderfehler::Fehler(match n {
            Some(n) => format!("Seite {n}: der Renderer ist abgestuerzt"),
            None => "das PDF: der Renderer ist abgestuerzt".into(),
        })),
    };
    warten(None)?;
    for &n in seiten {
        zuruf.send(n).map_err(|_| format!("Seite {n}: der Renderer ist abgestuerzt"))?;
        let bild = warten(Some(n))?.ok_or_else(|| format!("Seite {n}: kein Bild"))?;
        f(n, bild)?;
    }
    Ok(())
}

#[cfg(test)]
mod grenzen {
    /// Eine PDF mit einer Seite von `breite` x `hoehe` Punkten, ohne Inhalt.
    fn pdf(breite: u32, hoehe: u32) -> Vec<u8> {
        let objekte = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {breite} {hoehe}] >>"),
        ];
        let mut b = b"%PDF-1.4\n".to_vec();
        let mut lagen = Vec::new();
        for (i, o) in objekte.iter().enumerate() {
            lagen.push(b.len());
            b.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let xref = b.len();
        b.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objekte.len() + 1).as_bytes());
        for l in lagen {
            b.extend_from_slice(format!("{l:010} 00000 n \n").as_bytes());
        }
        b.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objekte.len() + 1).as_bytes());
        b
    }

    fn groesse(breite: u32, hoehe: u32) -> (u32, u32) {
        let mut aus = (0, 0);
        super::seiten_rendern(pdf(breite, hoehe), &[1], super::FRIST_JE_SEITE, |_, bild| {
            aus = (bild.width(), bild.height());
            Ok(())
        })
        .expect("gerendert");
        aus
    }

    /// ⛔️ **Eine erfundene Seitengroesse sprengt nichts**: A4 bleibt bei 216
    /// dpi, eine Seite von 20 000 Punkten wird auf die Grenzen verkleinert,
    /// eine von 200 000 Punkten endet ohne Panik.
    #[test]
    fn eine_riesige_seite_wird_verkleinert() {
        assert_eq!(groesse(612, 792), (1836, 2376), "A4 unveraendert");
        for (b, h) in [(20_000, 20_000), (200_000, 200_000), (200_000, 10)] {
            let (w, hh) = groesse(b, h);
            assert!(w as f32 <= super::KANTE_HOECHSTENS + 1.0 && hh as f32 <= super::KANTE_HOECHSTENS + 1.0, "{b}x{h}: {w}x{hh}");
            assert!((w as f32) * (hh as f32) <= super::PIXEL_HOECHSTENS * 1.01, "{b}x{h}: {w}x{hh}");
        }
    }

    /// ⛔️ **Eine haengende Seite haelt den Lauf nicht fest**: Die Seiten davor
    /// kommen an, die haengende wird nach der Frist genannt, und `f` sieht
    /// sie nie. Ein Absturz im Renderer und ein PDF, das sich nicht oeffnet,
    /// enden als Fehler statt als Panik oder Warten.
    #[test]
    fn eine_haengende_seite_wird_nach_der_frist_genannt() {
        use super::{mit_frist, Renderfehler};
        use std::time::{Duration, Instant};
        let frist = Duration::from_millis(300);
        let haengt = |n: u32| {
            if n == 2 {
                std::thread::sleep(Duration::from_secs(3));
            }
            Ok(image::RgbImage::new(1, 1))
        };
        let mut gesehen = Vec::new();
        let start = Instant::now();
        let aus = mit_frist(&[1, 2, 3], frist, move |d| d.bedienen(haengt), |n, _| {
            gesehen.push(n);
            Ok(())
        });
        assert_eq!(aus, Err(Renderfehler::Frist(2)));
        assert_eq!(gesehen, [1]);
        assert!(start.elapsed() < Duration::from_secs(2), "{:?}", start.elapsed());

        let stuerzt = |n: u32| -> Result<image::RgbImage, String> {
            assert_ne!(n, 1, "Absturz der Probe im Renderer");
            Ok(image::RgbImage::new(1, 1))
        };
        assert!(matches!(mit_frist(&[1], frist, move |d| d.bedienen(stuerzt), |_, _| Ok(())), Err(Renderfehler::Fehler(m)) if m.contains("abgestuerzt")));

        let oeffnet_nie = |d: super::Dienst| {
            std::thread::sleep(Duration::from_secs(3));
            d.fehler("nie".into());
        };
        assert!(matches!(mit_frist(&[1], frist, oeffnet_nie, |_, _| Ok(())), Err(Renderfehler::Fehler(m)) if m.contains("oeffnet sich nicht")));
    }

    #[test]
    fn seite_null_ist_ein_fehler() {
        assert!(super::seiten_rendern(pdf(612, 792), &[0], super::FRIST_JE_SEITE, |_, _| Ok(())).is_err());
    }
}
