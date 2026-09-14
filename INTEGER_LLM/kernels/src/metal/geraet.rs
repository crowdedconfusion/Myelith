//! Die Anbindung an Metal: Geraet, Shader, Puffer, Befehlspuffer.
//!
//! ⚑ **Ueber `objc2-metal`.** Die fruehere Kiste `metal` erklaert sich in
//! ihrem eigenen README fuer veraltet und verweist auf diese; auch
//! `candle-metal-kernels` haengt an ihr. Uebersetzt wird sie nur fuer
//! `target_os = "macos"` und nur hinter dem Feature `metal`.

use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::{Mutex, OnceLock};

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{
    MTLBuffer, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue, MTLCompileOptions,
    MTLComputeCommandEncoder, MTLComputePipelineState, MTLCreateSystemDefaultDevice, MTLDevice,
    MTLLanguageVersion, MTLLibrary, MTLResourceOptions, MTLSize,
};

/// Die Shaderquelle, im Baum als eigene Datei, damit sie lesbar bleibt
/// und der Gleitkomma-Audit sie findet.
const QUELLE: &str = include_str!("w8a16.metal");

/// Das Kachelmass des Produkts, wie im Shader.
const KACHEL_EINGABEN: usize = 16;
const KACHEL_ZEILEN: usize = 64;
const SIMD_GRUPPEN: usize = 4;

type Puffer = Retained<ProtocolObject<dyn MTLBuffer>>;

/// Ein Stapel fuer [`Kontext::rechnen_viele`].
pub(super) struct Job<'a> {
    pub xs: &'a [&'a [i16]],
    pub w: &'a [i8],
    pub spalten: usize,
    pub abstaende: &'a [i8],
}

#[repr(C)]
struct Produktform {
    zeilen: i32,
    spalten: i32,
    eingaben: i32,
}

#[repr(C)]
struct Nachlaufform {
    zeilen: i32,
    stapel: i32,
}

struct Kontext {
    geraet: Retained<ProtocolObject<dyn MTLDevice>>,
    schlange: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    produkt: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
    nachlauf: Retained<ProtocolObject<dyn MTLComputePipelineState>>,
    /// ⚑ **Ein Befehlspuffer zur Zeit.** Die Puffer eines Aufrufs gehoeren
    /// ihm allein; die Sperre haelt zwei Aufrufer auseinander, die sonst
    /// dieselbe Schlange und dieselbe Speicherbandbreite teilten.
    sperre: Mutex<()>,
}

/// Warum es keinen Kontext gibt.
#[derive(Debug, Clone)]
pub(super) enum Ohne {
    /// Diese Maschine hat keine Metal-GPU.
    KeinGeraet,
    /// Es gibt eine GPU, aber der Aufbau ist gescheitert: Shader,
    /// Pipeline, Befehlsschlange.
    Aufbau(String),
    /// ⛔️ Es gibt eine GPU, und sie rechnet **anders** als die CPU.
    Selbstpruefung(String),
}

impl std::fmt::Display for Ohne {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ohne::KeinGeraet => write!(f, "keine Metal-GPU"),
            Ohne::Aufbau(grund) => write!(f, "Aufbau gescheitert: {grund}"),
            Ohne::Selbstpruefung(grund) => write!(f, "rechnet anders als die CPU: {grund}"),
        }
    }
}

/// Der Kontext, einmal je Prozess aufgebaut und geprueft.
///
/// ⚑ **Die drei Gruende bleiben getrennt.** Eine Maschine ohne GPU ist
/// kein Fehler; eine GPU, die anders rechnet, ist einer, und die
/// Pruefungen muessen das eine ueberspringen und am anderen scheitern.
fn zustand() -> &'static Result<Kontext, Ohne> {
    static K: OnceLock<Result<Kontext, Ohne>> = OnceLock::new();
    K.get_or_init(|| {
        let k = Kontext::neu().inspect_err(|grund| match grund {
            Ohne::KeinGeraet => {}
            anders => eprintln!("⚠️ Metal: {anders}; es rechnet die CPU"),
        })?;
        match k.selbstpruefung() {
            Ok(()) => Ok(k),
            Err(grund) => {
                eprintln!("⛔️ Metal: Selbstpruefung gescheitert ({grund}); es rechnet die CPU");
                Err(Ohne::Selbstpruefung(grund))
            }
        }
    })
}

fn kontext() -> Option<&'static Kontext> {
    zustand().as_ref().ok()
}

/// Fuer die Pruefungen: `Ok` mit GPU, `Err(KeinGeraet)` zum Ueberspringen,
/// jeder andere Fehler laesst sie scheitern.
#[cfg(test)]
pub(super) fn bereit() -> Result<(), Ohne> {
    zustand().as_ref().map(|_| ()).map_err(Clone::clone)
}

/// Rechnet ohne Schwelle und ohne Rueckfall, fuer die Pruefungen.
#[cfg(test)]
pub(super) fn direkt(xs: &[&[i16]], w: &[i8], in_features: usize, abstaende: &[i8]) -> Option<Result<Vec<Vec<i16>>, String>> {
    kontext().map(|k| k.rechnen(xs, w, in_features, abstaende))
}

pub(super) fn grund() -> Option<String> {
    zustand().as_ref().err().map(|o| o.to_string())
}

pub(super) fn verfuegbar() -> bool {
    kontext().is_some()
}

pub(super) fn stapel_viele(jobs: &[Job<'_>]) -> Option<Vec<Vec<Vec<i16>>>> {
    let k = kontext()?;
    match k.rechnen_viele(jobs) {
        Ok(aus) => {
            super::GERECHNET.fetch_add(jobs.len(), std::sync::atomic::Ordering::Relaxed);
            Some(aus)
        }
        Err(grund) => {
            eprintln!("⚠️ Metal: {grund}; diese Buendelung rechnet die CPU");
            None
        }
    }
}

pub(super) fn stapel(xs: &[&[i16]], w: &[i8], in_features: usize, abstaende: &[i8]) -> Option<Vec<Vec<i16>>> {
    let k = kontext()?;
    match k.rechnen(xs, w, in_features, abstaende) {
        Ok(aus) => {
            super::GERECHNET.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Some(aus)
        }
        Err(grund) => {
            eprintln!("⚠️ Metal: {grund}; diese Buendelung rechnet die CPU");
            None
        }
    }
}

impl Kontext {
    fn neu() -> Result<Self, Ohne> {
        let geraet = MTLCreateSystemDefaultDevice().ok_or(Ohne::KeinGeraet)?;
        Self::aufbauen(geraet).map_err(Ohne::Aufbau)
    }

    fn aufbauen(geraet: Retained<ProtocolObject<dyn MTLDevice>>) -> Result<Self, String> {
        let optionen = MTLCompileOptions::new();
        // ⚑ **Shadersprache 4.0**, sonst kennt der Uebersetzer weder
        // `metal_tensor` noch `matmul2d`.
        optionen.setLanguageVersion(MTLLanguageVersion::Version4_0);
        let bibliothek = geraet
            .newLibraryWithSource_options_error(&NSString::from_str(QUELLE), Some(&optionen))
            .map_err(|e| format!("Shader nicht uebersetzbar: {}", e.localizedDescription()))?;
        let pipeline = |name: &str| {
            let funktion = bibliothek
                .newFunctionWithName(&NSString::from_str(name))
                .ok_or_else(|| format!("Shaderfunktion {name} fehlt"))?;
            geraet
                .newComputePipelineStateWithFunction_error(&funktion)
                .map_err(|e| format!("Pipeline {name}: {}", e.localizedDescription()))
        };
        let produkt = pipeline("produkt")?;
        let nachlauf = pipeline("nachlauf")?;
        let schlange = geraet.newCommandQueue().ok_or("keine Befehlsschlange")?;
        Ok(Kontext { geraet, schlange, produkt, nachlauf, sperre: Mutex::new(()) })
    }

    fn neuer_puffer(&self, bytes: usize) -> Result<Puffer, String> {
        self.geraet
            .newBufferWithLength_options(bytes.max(1), MTLResourceOptions::StorageModeShared)
            .ok_or_else(|| format!("kein Puffer ueber {bytes} Byte"))
    }

    /// Die Gewichte als Puffer, **ohne Kopie**, wo es geht.
    ///
    /// ⚑ **Seitenweise gerundet.** Metal verlangt fuer einen Puffer ueber
    /// fremdem Speicher Seitengrenzen. Die Gewichte beginnen selten an
    /// einer; der Puffer umfasst deshalb die Seiten, in denen sie liegen,
    /// und der Shader liest ab dem Versatz. Diese Seiten gehoeren ganz zur
    /// Abbildung oder zum Heap, in dem die Gewichte liegen, denn eine
    /// Seite ist die kleinste Einheit, in der Speicher vergeben wird.
    ///
    /// ⚑ **Kein Zwischenspeicher je Adresse.** Der Puffer lebt genau so
    /// lange wie dieser Aufruf und damit kuerzer als die Ausleihe der
    /// Gewichte. Ein Speicher je Adresse haette nach dem Freigeben eines
    /// Vektors an derselben Stelle andere Zahlen gefunden.
    fn gewichtspuffer(&self, w: &[i8]) -> Result<(Puffer, usize), String> {
        let seite = seitengroesse();
        let anfang = w.as_ptr() as usize;
        let basis = anfang & !(seite - 1);
        let ende = (anfang + w.len()).div_ceil(seite) * seite;
        if let Some(zeiger) = NonNull::new(basis as *mut c_void) {
            // SAFETY: `basis..ende` umfasst genau die Seiten, in denen `w`
            // liegt; sie sind lesbar, solange `w` ausgeliehen ist, und der
            // Puffer wird vor dem Ende dieser Ausleihe freigegeben. Der
            // Shader schreibt nicht in Puffer 0.
            let puffer = unsafe {
                self.geraet.newBufferWithBytesNoCopy_length_options_deallocator(
                    zeiger,
                    ende - basis,
                    MTLResourceOptions::StorageModeShared,
                    None,
                )
            };
            if let Some(p) = puffer {
                return Ok((p, anfang - basis));
            }
        }
        // Rueckfall: kopieren. Gleiche Zahlen, mehr Speicher.
        let p = self.neuer_puffer(w.len())?;
        // SAFETY: `p` ist frisch, mindestens `w.len()` Byte gross und
        // gehoert nur diesem Aufruf.
        unsafe {
            std::ptr::copy_nonoverlapping(w.as_ptr(), p.contents().as_ptr() as *mut i8, w.len());
        }
        Ok((p, 0))
    }

    fn rechnen(
        &self,
        xs: &[&[i16]],
        w: &[i8],
        in_features: usize,
        abstaende: &[i8],
    ) -> Result<Vec<Vec<i16>>, String> {
        let mut aus = self.rechnen_viele(&[Job { xs, w, spalten: in_features, abstaende }])?;
        Ok(aus.pop().unwrap_or_default())
    }

    /// **Viele Stapel in einem Befehlspuffer.**
    ///
    /// ⚑ **Warum** (2026-09-14): Ein Befehlspuffer kostet 0,2 bis 0,3 ms fest,
    /// gleich wie klein die Matrix ist. Eine Gemischebene des 30B rechnet in
    /// der Vorbereitung 384 Expertenmatrizen; einzeln wären das rund 100 ms
    /// Verwaltung je Ebene. Hier teilen sich alle Aufträge je einen Puffer
    /// für Eingaben, Teilprodukte, Abstände und Ausgaben und liegen darin
    /// hintereinander; jeder Auftrag sieht über den Versatz nur seinen Teil.
    fn rechnen_viele(&self, jobs: &[Job<'_>]) -> Result<Vec<Vec<Vec<i16>>>, String> {
        struct Lage {
            x: usize,
            c: usize,
            a: usize,
            o: usize,
        }
        let zu_gross = |n: usize| n > i32::MAX as usize;
        let mut lagen = Vec::with_capacity(jobs.len());
        let (mut gx, mut gc, mut ga, mut go) = (0usize, 0usize, 0usize, 0usize);
        for job in jobs {
            let (zeilen, stapel, spalten) = (job.abstaende.len(), job.xs.len(), job.spalten);
            let eingaben = 2 * stapel + 1;
            if zu_gross(eingaben * spalten) || zu_gross(eingaben * zeilen) || zu_gross(zeilen * spalten) {
                return Err(format!("{zeilen} x {spalten} bei {stapel} Eingaben sprengt die Indizes des Shaders"));
            }
            for x in job.xs {
                if x.len() != spalten {
                    return Err(format!("Eingabe mit {} statt {spalten} Elementen", x.len()));
                }
            }
            lagen.push(Lage { x: gx, c: gc, a: ga, o: go });
            gx += eingaben * spalten;
            gc += eingaben * zeilen * 4;
            ga += zeilen;
            go += stapel * zeilen * 2;
        }
        let leer = |job: &Job<'_>| job.abstaende.is_empty() || job.xs.is_empty();

        let _allein = self.sperre.lock().unwrap_or_else(|e| e.into_inner());

        let gewichte: Vec<(Puffer, usize)> =
            jobs.iter().map(|j| self.gewichtspuffer(j.w)).collect::<Result<_, _>>()?;
        let x = self.neuer_puffer(gx)?;
        let c = self.neuer_puffer(gc)?;
        let abst = self.neuer_puffer(ga)?;
        let aus = self.neuer_puffer(go)?;

        // SAFETY: Alle Puffer sind frisch und gehoeren nur diesem Aufruf; jeder
        // Auftrag schreibt nur in seinen Abschnitt `lage.x .. lage.x + (2b+1)
        // * spalten` und `lage.a .. lage.a + zeilen`, und die Abschnitte
        // liegen nach Konstruktion hintereinander. Die GPU rechnet erst nach
        // `commit`.
        unsafe {
            for (job, lage) in jobs.iter().zip(&lagen) {
                let (zeilen, stapel, spalten) = (job.abstaende.len(), job.xs.len(), job.spalten);
                let ziel = std::slice::from_raw_parts_mut(
                    (x.contents().as_ptr() as *mut i8).add(lage.x),
                    (2 * stapel + 1) * spalten,
                );
                let (hoch, rest) = ziel.split_at_mut(stapel * spalten);
                let (niedrig, einsen) = rest.split_at_mut(stapel * spalten);
                for (b, eingabe) in job.xs.iter().enumerate() {
                    let h = &mut hoch[b * spalten..(b + 1) * spalten];
                    let l = &mut niedrig[b * spalten..(b + 1) * spalten];
                    for ((h, l), &wert) in h.iter_mut().zip(l.iter_mut()).zip(eingabe.iter()) {
                        (*h, *l) = super::zerlegen(wert);
                    }
                }
                einsen.fill(1);
                std::ptr::copy_nonoverlapping(
                    job.abstaende.as_ptr(),
                    (abst.contents().as_ptr() as *mut i8).add(lage.a),
                    zeilen,
                );
            }
        }

        let puffer = self.schlange.commandBuffer().ok_or("kein Befehlspuffer")?;
        let kodierer = puffer.computeCommandEncoder().ok_or("kein Kodierer")?;
        let breite = self.produkt.threadExecutionWidth() * SIMD_GRUPPEN;

        kodierer.setComputePipelineState(&self.produkt);
        for ((job, lage), (gew, versatz)) in jobs.iter().zip(&lagen).zip(&gewichte) {
            if leer(job) {
                continue;
            }
            let (zeilen, stapel, spalten) = (job.abstaende.len(), job.xs.len(), job.spalten);
            let eingaben = 2 * stapel + 1;
            let form = Produktform { zeilen: zeilen as i32, spalten: spalten as i32, eingaben: eingaben as i32 };
            // SAFETY: Die Indizes entsprechen den `[[buffer(n)]]` des Shaders,
            // die Versaetze liegen innerhalb der Puffer, und `setBytes`
            // kopiert die Form.
            unsafe {
                kodierer.setBuffer_offset_atIndex(Some(gew), *versatz, 0);
                kodierer.setBuffer_offset_atIndex(Some(&x), lage.x, 1);
                kodierer.setBuffer_offset_atIndex(Some(&c), lage.c, 2);
                kodierer.setBytes_length_atIndex(NonNull::from(&form).cast(), std::mem::size_of::<Produktform>(), 3);
            }
            kodierer.dispatchThreadgroups_threadsPerThreadgroup(
                MTLSize { width: zeilen.div_ceil(KACHEL_ZEILEN), height: eingaben.div_ceil(KACHEL_EINGABEN), depth: 1 },
                MTLSize { width: breite, height: 1, depth: 1 },
            );
        }

        kodierer.setComputePipelineState(&self.nachlauf);
        for (job, lage) in jobs.iter().zip(&lagen) {
            if leer(job) {
                continue;
            }
            let (zeilen, stapel) = (job.abstaende.len(), job.xs.len());
            let form = Nachlaufform { zeilen: zeilen as i32, stapel: stapel as i32 };
            // SAFETY: wie oben.
            unsafe {
                kodierer.setBuffer_offset_atIndex(Some(&c), lage.c, 0);
                kodierer.setBuffer_offset_atIndex(Some(&abst), lage.a, 1);
                kodierer.setBuffer_offset_atIndex(Some(&aus), lage.o, 2);
                kodierer.setBytes_length_atIndex(NonNull::from(&form).cast(), std::mem::size_of::<Nachlaufform>(), 3);
            }
            kodierer.dispatchThreads_threadsPerThreadgroup(
                MTLSize { width: zeilen, height: stapel, depth: 1 },
                MTLSize { width: 32.min(zeilen), height: 8.min(stapel), depth: 1 },
            );
        }
        kodierer.endEncoding();
        puffer.commit();
        puffer.waitUntilCompleted();
        if let Some(fehler) = puffer.error() {
            return Err(format!("Befehlspuffer gescheitert: {}", fehler.localizedDescription()));
        }

        Ok(jobs
            .iter()
            .zip(&lagen)
            .map(|(job, lage)| {
                let (zeilen, stapel) = (job.abstaende.len(), job.xs.len());
                if zeilen == 0 {
                    return vec![Vec::new(); stapel];
                }
                // SAFETY: Der Befehlspuffer ist fertig; der Abschnitt haelt
                // `stapel * zeilen` int16 dieses Auftrags.
                let werte = unsafe {
                    std::slice::from_raw_parts(
                        (aus.contents().as_ptr() as *const i16).add(lage.o / 2),
                        stapel * zeilen,
                    )
                };
                werte.chunks_exact(zeilen).map(|z| z.to_vec()).collect()
            })
            .collect())
    }

    /// **Rechnet die GPU dasselbe wie die CPU?** Einmal je Prozess, bevor
    /// sie zum ersten Mal rechnen darf.
    ///
    /// ⚑ **Weil `matmul2d` geschlossen ist** und ein Systemupdate seine
    /// Rechnung aendern kann. Ein Knoten, der danach anders rechnete,
    /// merkte es sonst erst an einem verlorenen Streit.
    ///
    /// Geprueft wird, was eine unauffaellige Messung nicht trifft:
    ///
    /// - **Die groessten Betraege beider Stellen.** Gewichte −128 und
    ///   127, Eingaben −32 768 und 32 767, 9 728 Spalten: Die
    ///   Teilprodukte erreichen 159 Millionen und liegen damit weit ueber
    ///   `2²⁴`, ab wo eine Rechnung in einfacher Gleitkommagenauigkeit
    ///   runden wuerde.
    /// - **Kachelraender** in beiden Richtungen: Zeilen und Eingaben, die
    ///   keine Vielfachen des Kachelmasses sind.
    /// - **Alle drei Faelle der Umskalierung**: Rechtsshift, kein Shift,
    ///   Linksshift.
    fn selbstpruefung(&self) -> Result<(), String> {
        let mut saat = 0x9e37_79b9_7f4a_7c15u64;
        let mut zufall = move || {
            saat ^= saat << 13;
            saat ^= saat >> 7;
            saat ^= saat << 17;
            saat
        };
        struct Fall {
            name: &'static str,
            zeilen: usize,
            spalten: usize,
            stapel: usize,
        }
        let faelle = [
            Fall { name: "Extremwerte", zeilen: 67, spalten: 9_728, stapel: 17 },
            Fall { name: "Zufall", zeilen: 130, spalten: 1_003, stapel: 33 },
            Fall { name: "klein", zeilen: 3, spalten: 5, stapel: 2 },
        ];
        for fall in &faelle {
            let (zeilen, spalten, stapel) = (fall.zeilen, fall.spalten, fall.stapel);
            let w: Vec<i8> = (0..zeilen * spalten)
                .map(|i| match fall.name {
                    "Extremwerte" => if (i / spalten) % 2 == 0 { -128 } else { 127 },
                    _ => zufall() as i8,
                })
                .collect();
            let eingaben: Vec<Vec<i16>> = (0..stapel)
                .map(|b| {
                    (0..spalten)
                        .map(|_| match fall.name {
                            "Extremwerte" => if b % 2 == 0 { i16::MIN } else { i16::MAX },
                            _ => zufall() as i16,
                        })
                        .collect()
                })
                .collect();
            let xs: Vec<&[i16]> = eingaben.iter().map(|e| e.as_slice()).collect();
            // Abstaende 20, 14, 0, -1 der Reihe nach: Rechtsshift, kein
            // Shift, Linksshift.
            let w_shifts: Vec<u8> = (0..zeilen).map(|z| [9u8, 3, 0, 0][z % 4]).collect();
            let ziele: Vec<u8> = (0..zeilen).map(|z| [0u8, 0, 11, 12][z % 4]).collect();
            let act = 11u8;
            let abstaende: Vec<i8> =
                (0..zeilen).map(|z| (w_shifts[z] + act) as i8 - ziele[z] as i8).collect();

            let gpu = self.rechnen(&xs, &w, spalten, &abstaende)?;
            let cpu = crate::linear::linear_w8a16_pc_stapel_cpu(&xs, &w, spalten, &w_shifts, act, &ziele);
            if gpu != cpu {
                let (b, z) = gpu
                    .iter()
                    .zip(cpu.iter())
                    .enumerate()
                    .find_map(|(b, (g, c))| g.iter().zip(c).position(|(g, c)| g != c).map(|z| (b, z)))
                    .unwrap_or((0, 0));
                return Err(format!(
                    "{}: {zeilen} x {spalten} bei {stapel} Eingaben weicht ab, zuerst Eingabe {b}, Zeile {z}: GPU {} gegen CPU {}",
                    fall.name, gpu[b][z], cpu[b][z]
                ));
            }
        }

        // ⚑ **Und mehrere Auftraege in einem Befehlspuffer**, mit
        // verschiedenen Formen, damit ein falscher Versatz auffaellt.
        let formen = [(40usize, 257usize, 3usize), (7, 64, 19), (130, 33, 1), (1, 1, 5)];
        // Je Auftrag: Gewichte, Eingaben, Abstaende.
        type Pruefauftrag = (Vec<i8>, Vec<Vec<i16>>, Vec<i8>);
        let daten: Vec<Pruefauftrag> = formen
            .iter()
            .map(|&(zeilen, spalten, stapel)| {
                let w: Vec<i8> = (0..zeilen * spalten).map(|_| zufall() as i8).collect();
                let e: Vec<Vec<i16>> =
                    (0..stapel).map(|_| (0..spalten).map(|_| zufall() as i16).collect()).collect();
                let a: Vec<i8> = (0..zeilen).map(|z| [20i8, 14, 0, -1][z % 4]).collect();
                (w, e, a)
            })
            .collect();
        let scheiben: Vec<Vec<&[i16]>> =
            daten.iter().map(|(_, e, _)| e.iter().map(|v| v.as_slice()).collect()).collect();
        let jobs: Vec<Job<'_>> = daten
            .iter()
            .zip(&scheiben)
            .zip(&formen)
            .map(|(((w, _, a), xs), &(_, spalten, _))| Job { xs, w, spalten, abstaende: a })
            .collect();
        let gpu = self.rechnen_viele(&jobs)?;
        for (i, job) in jobs.iter().enumerate() {
            let einzeln = self.rechnen(job.xs, job.w, job.spalten, job.abstaende)?;
            if gpu[i] != einzeln {
                return Err(format!("Auftrag {i} im gemeinsamen Befehlspuffer weicht vom einzelnen ab"));
            }
        }
        Ok(())
    }
}

/// Die Seitengroesse dieser Maschine.
fn seitengroesse() -> usize {
    extern "C" {
        static vm_page_size: usize;
    }
    // SAFETY: `vm_page_size` ist eine unveraenderliche Variable der
    // Systembibliothek, gesetzt vor `main`.
    unsafe { vm_page_size }
}
