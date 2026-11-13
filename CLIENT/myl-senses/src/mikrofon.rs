//! **Das Mikrofon ohne fremdes Programm**: eine duenne Anbindung je System.
//!
//! CLIENT 8.1 (Entscheidung vom 2026-11-11): eine eigene Anbindung je
//! System, zur Laufzeit geladen, und ffmpeg als Rueckfall, bis jede ihre
//! Abnahme besteht. `cpal` war Vorlage und kam nicht in den Vorrat: Unter
//! Linux haette es zum Bauen die Entwicklerdateien von ALSA verlangt, und
//! ein frischer Klon soll ueberall ohne Netz bauen.
//!
//! | System | Anbindung | Stand |
//! |---|---|---|
//! | macOS | AudioQueue aus AudioToolbox, Teil jedes macOS | gebaut |
//! | Linux | `libasound.so.2` zur Laufzeit geladen (`MYL_ALSA_GERAET`, sonst `default`) | gebaut |
//! | Windows | `winmm` (`waveIn`), Puffer abgefragt statt zurueckgerufen | geschrieben, nur in der CI uebersetzt |
//!
//! Geliefert wird immer dasselbe: 16 kHz, ein Kanal, 16 Bit, so wie das
//! Hoeren es braucht, in Stuecken von 0,1 s.
//!
//! ⚠️ **Die erste echte Aufnahme fragt nach der Freigabe des Mikrofons**
//! (macOS). Die Probe dafuer traegt deshalb `#[ignore]` und faehrt nur, wer
//! sie verlangt.

use std::sync::mpsc::Receiver;

/// Abtastrate, die geliefert wird.
pub const RATE: u32 = 16_000;
/// Proben je Stueck (0,1 s).
pub const STUECK: usize = 1_600;

/// Ein laufender Mitschnitt: Stuecke kommen ueber [`Mitschnitt::stuecke`].
pub struct Mitschnitt {
    stuecke: Receiver<Vec<i16>>,
    innen: system::Innen,
}

impl std::fmt::Debug for Mitschnitt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Mitschnitt")
    }
}

impl Mitschnitt {
    /// Die Stuecke, wie sie kommen.
    pub fn stuecke(&self) -> &Receiver<Vec<i16>> {
        &self.stuecke
    }

    /// **Haelt an** und gibt zurueck, was noch unterwegs war.
    pub fn anhalten(self) -> Vec<i16> {
        let Mitschnitt { stuecke, innen } = self;
        innen.anhalten();
        stuecke.try_iter().flatten().collect()
    }
}

/// **Oeffnet das Standardmikrofon.** Ein Fehler sagt, warum es nicht geht,
/// und dann gilt der Rueckfall.
pub fn oeffnen() -> Result<Mitschnitt, String> {
    let (sender, stuecke) = std::sync::mpsc::channel();
    let innen = system::oeffnen(sender)?;
    Ok(Mitschnitt { stuecke, innen })
}

/// Der Ausschlag eines Stuecks auf der Skala der Aufnahme (60 dB, 0 bis 1).
pub fn pegel(stueck: &[i16]) -> f32 {
    if stueck.is_empty() {
        return 0.0;
    }
    let summe: f64 = stueck.iter().map(|p| f64::from(*p).powi(2)).sum();
    let rms = (summe / stueck.len() as f64).sqrt() / 32_768.0;
    if rms <= 0.0 {
        return 0.0;
    }
    let db = 20.0 * rms.log10();
    (((db + 60.0) / 60.0) as f32).clamp(0.0, 1.0)
}

#[cfg(target_os = "macos")]
mod system {
    //! AudioQueue: Eingang mit eigenem Faden, drei Puffer zu je 0,1 s.
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::Sender;
    use std::sync::Mutex;

    #[repr(C)]
    struct Format {
        abtastrate: f64,
        format: u32,
        merkmale: u32,
        bytes_je_paket: u32,
        rahmen_je_paket: u32,
        bytes_je_rahmen: u32,
        kanaele: u32,
        bits: u32,
        frei: u32,
    }

    #[repr(C)]
    struct Puffer {
        kapazitaet: u32,
        daten: *mut c_void,
        groesse: u32,
        nutzer: *mut c_void,
        beschreibungen_kapazitaet: u32,
        beschreibungen: *mut c_void,
        beschreibungen_zahl: u32,
    }

    type Schlange = *mut c_void;
    type Rueckruf = extern "C" fn(*mut c_void, Schlange, *mut Puffer, *const c_void, u32, *const c_void);

    #[link(name = "AudioToolbox", kind = "framework")]
    extern "C" {
        fn AudioQueueNewInput(
            format: *const Format,
            rueckruf: Rueckruf,
            nutzer: *mut c_void,
            schleife: *const c_void,
            modus: *const c_void,
            merkmale: u32,
            aus: *mut Schlange,
        ) -> i32;
        fn AudioQueueAllocateBuffer(s: Schlange, groesse: u32, aus: *mut *mut Puffer) -> i32;
        fn AudioQueueEnqueueBuffer(s: Schlange, p: *mut Puffer, n: u32, b: *const c_void) -> i32;
        fn AudioQueueStart(s: Schlange, zeit: *const c_void) -> i32;
        fn AudioQueueStop(s: Schlange, sofort: u8) -> i32;
        fn AudioQueueDispose(s: Schlange, sofort: u8) -> i32;
    }

    /// `'lpcm'`
    const LINEAR_PCM: u32 = 0x6C70_636D;
    /// Vorzeichenbehaftet und dicht gepackt.
    const GANZZAHL_GEPACKT: u32 = 0x4 | 0x8;

    struct Geteilt {
        sender: Mutex<Sender<Vec<i16>>>,
        laeuft: AtomicBool,
    }

    pub(super) struct Innen {
        schlange: Schlange,
        geteilt: *mut Geteilt,
    }

    // Die Schlange gehoert dem Faden, der anhaelt; AudioToolbox ist
    // fadenfest fuer Start, Halt und Freigabe.
    unsafe impl Send for Innen {}

    extern "C" fn angekommen(nutzer: *mut c_void, s: Schlange, p: *mut Puffer, _: *const c_void, _: u32, _: *const c_void) {
        // SAFETY: `nutzer` ist das `Geteilt` aus `oeffnen`, das erst nach
        // `AudioQueueDispose` freigegeben wird; `p` ist ein Puffer dieser
        // Schlange mit `groesse` gueltigen Bytes.
        let (geteilt, puffer) = unsafe { (&*(nutzer as *const Geteilt), &*p) };
        let zahl = puffer.groesse as usize / 2;
        let proben = unsafe { std::slice::from_raw_parts(puffer.daten as *const i16, zahl) }.to_vec();
        if let Ok(s) = geteilt.sender.lock() {
            let _ = s.send(proben);
        }
        if geteilt.laeuft.load(Ordering::Acquire) {
            // SAFETY: Puffer und Schlange gehoeren zusammen und leben noch.
            unsafe { AudioQueueEnqueueBuffer(s, p, 0, std::ptr::null()) };
        }
    }

    pub(super) fn oeffnen(sender: Sender<Vec<i16>>) -> Result<Innen, String> {
        let format = Format {
            abtastrate: f64::from(super::RATE),
            format: LINEAR_PCM,
            merkmale: GANZZAHL_GEPACKT,
            bytes_je_paket: 2,
            rahmen_je_paket: 1,
            bytes_je_rahmen: 2,
            kanaele: 1,
            bits: 16,
            frei: 0,
        };
        let geteilt = Box::into_raw(Box::new(Geteilt { sender: Mutex::new(sender), laeuft: AtomicBool::new(true) }));
        let mut schlange: Schlange = std::ptr::null_mut();
        // SAFETY: Alle Zeiger zeigen auf lebende Werte; die Schlaufe ist
        // null, also ruft AudioToolbox auf einem eigenen Faden zurueck.
        let st = unsafe {
            AudioQueueNewInput(&format, angekommen, geteilt.cast(), std::ptr::null(), std::ptr::null(), 0, &mut schlange)
        };
        if st != 0 {
            // SAFETY: Noch kein Rueckruf kennt den Zeiger.
            drop(unsafe { Box::from_raw(geteilt) });
            return Err(format!("CoreAudio oeffnet keinen Eingang (OSStatus {st})"));
        }
        let innen = Innen { schlange, geteilt };
        for _ in 0..3 {
            let mut p: *mut Puffer = std::ptr::null_mut();
            // SAFETY: Die Schlange lebt, `p` wird beschrieben.
            let st = unsafe { AudioQueueAllocateBuffer(schlange, (super::STUECK * 2) as u32, &mut p) };
            if st != 0 || unsafe { AudioQueueEnqueueBuffer(schlange, p, 0, std::ptr::null()) } != 0 {
                innen.anhalten();
                return Err(format!("CoreAudio gibt keinen Puffer (OSStatus {st})"));
            }
        }
        // SAFETY: wie oben.
        let st = unsafe { AudioQueueStart(schlange, std::ptr::null()) };
        if st != 0 {
            innen.anhalten();
            return Err(format!("CoreAudio startet den Eingang nicht (OSStatus {st}); ist das Mikrofon freigegeben?"));
        }
        Ok(innen)
    }

    impl Innen {
        pub(super) fn anhalten(self) {
            // SAFETY: Erst kein Nachlegen mehr, dann anhalten und
            // freigeben; danach ruft AudioToolbox nicht mehr zurueck, und
            // `geteilt` darf gehen.
            unsafe {
                (*self.geteilt).laeuft.store(false, Ordering::Release);
                AudioQueueStop(self.schlange, 1);
                AudioQueueDispose(self.schlange, 1);
                drop(Box::from_raw(self.geteilt));
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod system {
    //! ALSA, zur Laufzeit geladen: Der Bau braucht weder `libasound2-dev`
    //! noch eine Kiste, und fehlt die Bibliothek, sagt es die Meldung.
    use std::ffi::{c_char, c_int, c_long, c_uint, c_void, CStr, CString};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::Sender;
    use std::sync::Arc;

    extern "C" {
        fn dlopen(name: *const c_char, flag: c_int) -> *mut c_void;
        fn dlsym(h: *mut c_void, name: *const c_char) -> *mut c_void;
    }
    const RTLD_NOW: c_int = 2;
    const CAPTURE: c_int = 1;
    const S16_LE: c_int = 2;
    const RW_INTERLEAVED: c_int = 3;

    type Open = unsafe extern "C" fn(*mut *mut c_void, *const c_char, c_int, c_int) -> c_int;
    type SetParams = unsafe extern "C" fn(*mut c_void, c_int, c_int, c_uint, c_uint, c_int, c_uint) -> c_int;
    type Readi = unsafe extern "C" fn(*mut c_void, *mut c_void, c_long) -> c_long;
    type Recover = unsafe extern "C" fn(*mut c_void, c_int, c_int) -> c_int;
    type Close = unsafe extern "C" fn(*mut c_void) -> c_int;
    type StrError = unsafe extern "C" fn(c_int) -> *const c_char;

    struct Alsa {
        open: Open,
        set_params: SetParams,
        readi: Readi,
        recover: Recover,
        close: Close,
        strerror: StrError,
    }

    fn laden() -> Result<Alsa, String> {
        // SAFETY: dlopen und dlsym mit gueltigen, nullterminierten Namen;
        // jedes Symbol wird vor dem Umwandeln auf null geprueft, und die
        // Bibliothek wird nie entladen.
        unsafe {
            let h = dlopen(c"libasound.so.2".as_ptr(), RTLD_NOW);
            if h.is_null() {
                return Err("ALSA fehlt (libasound.so.2 nicht ladbar); Rueckfall ist ffmpeg".into());
            }
            let sym = |name: &CStr| -> Result<*mut c_void, String> {
                let s = dlsym(h, name.as_ptr());
                if s.is_null() {
                    Err(format!("ALSA ohne {}", name.to_string_lossy()))
                } else {
                    Ok(s)
                }
            };
            Ok(Alsa {
                open: std::mem::transmute::<*mut c_void, Open>(sym(c"snd_pcm_open")?),
                set_params: std::mem::transmute::<*mut c_void, SetParams>(sym(c"snd_pcm_set_params")?),
                readi: std::mem::transmute::<*mut c_void, Readi>(sym(c"snd_pcm_readi")?),
                recover: std::mem::transmute::<*mut c_void, Recover>(sym(c"snd_pcm_recover")?),
                close: std::mem::transmute::<*mut c_void, Close>(sym(c"snd_pcm_close")?),
                strerror: std::mem::transmute::<*mut c_void, StrError>(sym(c"snd_strerror")?),
            })
        }
    }

    fn fehler(a: &Alsa, e: c_int) -> String {
        // SAFETY: snd_strerror liefert einen statischen, nullterminierten Text.
        unsafe { CStr::from_ptr((a.strerror)(e)) }.to_string_lossy().into_owned()
    }

    struct Geraet(*mut c_void);
    // Das Geraet benutzt nur der Lesefaden, nach dem Oeffnen.
    unsafe impl Send for Geraet {}

    pub(super) struct Innen {
        laeuft: Arc<AtomicBool>,
        faden: Option<std::thread::JoinHandle<()>>,
    }

    pub(super) fn oeffnen(sender: Sender<Vec<i16>>) -> Result<Innen, String> {
        let a = laden()?;
        let name = std::env::var("MYL_ALSA_GERAET").unwrap_or_else(|_| "default".into());
        let cname = CString::new(name.clone()).map_err(|_| "Geraetename mit Nullbyte".to_string())?;
        let mut pcm: *mut c_void = std::ptr::null_mut();
        // SAFETY: gueltige Zeiger; bei Erfolg ist `pcm` ein offenes Geraet.
        let e = unsafe { (a.open)(&mut pcm, cname.as_ptr(), CAPTURE, 0) };
        if e < 0 {
            return Err(format!("ALSA oeffnet {name} nicht: {}", fehler(&a, e)));
        }
        // 16 kHz, ein Kanal, 16 Bit; ALSA tastet selbst um, Latenz 0,5 s.
        let e = unsafe { (a.set_params)(pcm, S16_LE, RW_INTERLEAVED, 1, super::RATE, 1, 500_000) };
        if e < 0 {
            unsafe { (a.close)(pcm) };
            return Err(format!("ALSA nimmt das Format nicht an: {}", fehler(&a, e)));
        }
        let laeuft = Arc::new(AtomicBool::new(true));
        let l = Arc::clone(&laeuft);
        let geraet = Geraet(pcm);
        let faden = std::thread::spawn(move || {
            let geraet = geraet;
            let mut puffer = vec![0i16; super::STUECK];
            while l.load(Ordering::Acquire) {
                // SAFETY: Der Puffer fasst STUECK Rahmen zu je einer Probe.
                let n = unsafe { (a.readi)(geraet.0, puffer.as_mut_ptr().cast(), super::STUECK as c_long) };
                if n < 0 {
                    if unsafe { (a.recover)(geraet.0, n as c_int, 1) } < 0 {
                        break;
                    }
                    continue;
                }
                if sender.send(puffer[..n as usize].to_vec()).is_err() {
                    break;
                }
            }
            // SAFETY: Nur dieser Faden hielt das Geraet.
            unsafe { (a.close)(geraet.0) };
        });
        Ok(Innen { laeuft, faden: Some(faden) })
    }

    impl Innen {
        pub(super) fn anhalten(mut self) {
            self.laeuft.store(false, Ordering::Release);
            if let Some(f) = self.faden.take() {
                let _ = f.join();
            }
        }
    }
}

#[cfg(windows)]
mod system {
    //! `waveIn` aus `winmm`, Teil jedes Windows. Drei Puffer, ein Faden
    //! fragt ab, welcher fertig ist (`CALLBACK_NULL`): Ein Rueckruf liefe
    //! im Treiberfaden, und dort darf man nichts tun, was blockiert.
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::Sender;
    use std::sync::Arc;

    #[repr(C, packed)]
    struct Format {
        art: u16,
        kanaele: u16,
        rate: u32,
        bytes_je_s: u32,
        block: u16,
        bits: u16,
        zusatz: u16,
    }

    #[repr(C)]
    struct Kopf {
        daten: *mut u8,
        laenge: u32,
        aufgenommen: u32,
        nutzer: usize,
        merkmale: u32,
        schleifen: u32,
        naechster: *mut Kopf,
        frei: usize,
    }

    type Griff = *mut std::ffi::c_void;
    const WAVE_MAPPER: u32 = 0xFFFF_FFFF;
    const WHDR_DONE: u32 = 0x1;

    #[link(name = "winmm")]
    extern "system" {
        fn waveInOpen(g: *mut Griff, geraet: u32, f: *const Format, rueckruf: usize, inst: usize, art: u32) -> u32;
        fn waveInPrepareHeader(g: Griff, k: *mut Kopf, n: u32) -> u32;
        fn waveInUnprepareHeader(g: Griff, k: *mut Kopf, n: u32) -> u32;
        fn waveInAddBuffer(g: Griff, k: *mut Kopf, n: u32) -> u32;
        fn waveInStart(g: Griff) -> u32;
        fn waveInReset(g: Griff) -> u32;
        fn waveInClose(g: Griff) -> u32;
    }

    struct Offen(Griff);
    // Der Griff wird nach dem Oeffnen nur vom Abfragefaden benutzt.
    unsafe impl Send for Offen {}

    pub(super) struct Innen {
        laeuft: Arc<AtomicBool>,
        faden: Option<std::thread::JoinHandle<()>>,
    }

    pub(super) fn oeffnen(sender: Sender<Vec<i16>>) -> Result<Innen, String> {
        let format = Format { art: 1, kanaele: 1, rate: super::RATE, bytes_je_s: super::RATE * 2, block: 2, bits: 16, zusatz: 0 };
        let mut g: Griff = std::ptr::null_mut();
        // SAFETY: gueltige Zeiger, kein Rueckruf (CALLBACK_NULL).
        let e = unsafe { waveInOpen(&mut g, WAVE_MAPPER, &format, 0, 0, 0) };
        if e != 0 {
            return Err(format!("winmm oeffnet keinen Eingang (MMRESULT {e})"));
        }
        let laeuft = Arc::new(AtomicBool::new(true));
        let l = Arc::clone(&laeuft);
        let offen = Offen(g);
        let faden = std::thread::spawn(move || {
            let offen = offen;
            let groesse = std::mem::size_of::<Kopf>() as u32;
            let mut puffer: Vec<Vec<u8>> = (0..3).map(|_| vec![0u8; super::STUECK * 2]).collect();
            let mut koepfe: Vec<Kopf> = puffer
                .iter_mut()
                .map(|p| Kopf {
                    daten: p.as_mut_ptr(),
                    laenge: p.len() as u32,
                    aufgenommen: 0,
                    nutzer: 0,
                    merkmale: 0,
                    schleifen: 0,
                    naechster: std::ptr::null_mut(),
                    frei: 0,
                })
                .collect();
            // SAFETY: Koepfe und Puffer leben bis nach waveInReset und
            // waveInUnprepareHeader am Ende dieses Fadens.
            unsafe {
                for k in koepfe.iter_mut() {
                    waveInPrepareHeader(offen.0, k, groesse);
                    waveInAddBuffer(offen.0, k, groesse);
                }
                waveInStart(offen.0);
            }
            while l.load(Ordering::Acquire) {
                let mut nichts = true;
                for k in koepfe.iter_mut() {
                    // SAFETY: Das System schreibt die Merkmale aus einem
                    // anderen Faden; gelesen wird fluechtig.
                    let merkmale = unsafe { std::ptr::read_volatile(&k.merkmale) };
                    if merkmale & WHDR_DONE != 0 {
                        nichts = false;
                        let n = unsafe { std::ptr::read_volatile(&k.aufgenommen) } as usize / 2;
                        let proben = unsafe { std::slice::from_raw_parts(k.daten as *const i16, n) }.to_vec();
                        if sender.send(proben).is_err() {
                            l.store(false, Ordering::Release);
                        }
                        k.merkmale &= !WHDR_DONE;
                        unsafe { waveInAddBuffer(offen.0, k, groesse) };
                    }
                }
                if nichts {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
            unsafe {
                waveInReset(offen.0);
                for k in koepfe.iter_mut() {
                    waveInUnprepareHeader(offen.0, k, groesse);
                }
                waveInClose(offen.0);
            }
            drop(puffer);
        });
        Ok(Innen { laeuft, faden: Some(faden) })
    }

    impl Innen {
        pub(super) fn anhalten(mut self) {
            self.laeuft.store(false, Ordering::Release);
            if let Some(f) = self.faden.take() {
                let _ = f.join();
            }
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod system {
    pub(super) struct Innen;

    pub(super) fn oeffnen(_: std::sync::mpsc::Sender<Vec<i16>>) -> Result<Innen, String> {
        Err("auf diesem System gibt es noch keine eigene Anbindung an das Mikrofon".into())
    }

    impl Innen {
        pub(super) fn anhalten(self) {}
    }
}

#[cfg(test)]
mod proben {
    use super::*;

    /// Der Ausschlag liegt auf derselben Skala wie bei ffmpeg: Stille null,
    /// Vollaussteuerung eins, −30 dB in der Mitte.
    #[test]
    fn der_pegel_folgt_der_skala_der_aufnahme() {
        assert_eq!(pegel(&[]), 0.0);
        assert_eq!(pegel(&[0; 1600]), 0.0);
        assert!((pegel(&[i16::MAX; 1600]) - 1.0).abs() < 1e-3);
        let halb = (32_768.0 * 10f64.powf(-30.0 / 20.0)) as i16;
        assert!((pegel(&[halb; 1600]) - 0.5).abs() < 1e-2);
        // Rechteck mit wechselndem Vorzeichen: dieselbe Leistung.
        let wechsel: Vec<i16> = (0..1600).map(|i| if i % 2 == 0 { halb } else { -halb }).collect();
        assert!((pegel(&wechsel) - pegel(&[halb; 1600])).abs() < 1e-6);
    }

    /// Linux: zehn Stuecke vom ALSA-Geraet `null` (Stille ohne Hardware).
    /// Mit `#[ignore]`, weil er die Laufzeitbibliothek von ALSA braucht;
    /// gefahren im Behaelter mit `libasound2`.
    ///
    /// ⚠️ **`null` taktet nicht nach der Uhr**: Es lieferte in 0,5 s ueber
    /// eine Milliarde Proben. Die Probe zaehlt deshalb Stuecke, nicht Zeit;
    /// ein echtes Geraet blockiert bis zum naechsten Stueck.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "braucht libasound.so.2; im Behaelter mit MYL_ALSA_GERAET=null"]
    fn alsa_liefert_stuecke_vom_nullgeraet() {
        let m = oeffnen().expect("ALSA");
        let mut stuecke = 0usize;
        while stuecke < 10 {
            let s = m.stuecke().recv_timeout(std::time::Duration::from_secs(2)).expect("ein Stueck in zwei Sekunden");
            assert_eq!(s.len(), super::STUECK, "ein Stueck hat die falsche Laenge");
            stuecke += 1;
        }
        drop(m.anhalten());
    }

    /// Eine echte Aufnahme von einer Sekunde. ⚠️ Fragt beim ersten Mal nach
    /// der Freigabe des Mikrofons; danach `myl hoeren` oder diese Probe.
    #[test]
    #[ignore = "nimmt eine Sekunde vom Mikrofon auf; fragt beim ersten Mal nach der Freigabe"]
    fn eine_sekunde_vom_mikrofon() {
        let m = oeffnen().expect("Mikrofon");
        let mut proben = Vec::new();
        let ende = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while std::time::Instant::now() < ende {
            if let Ok(s) = m.stuecke().recv_timeout(std::time::Duration::from_millis(500)) {
                proben.extend(s);
            }
        }
        proben.extend(m.anhalten());
        eprintln!("[mikrofon] {} Proben, Pegel {:.2}", proben.len(), pegel(&proben));
        assert!(proben.len() >= 12_000, "nur {} Proben in einer Sekunde", proben.len());
    }
}
