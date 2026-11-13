//! **Einen Satz ueber den echten Vorleser sprechen und den ersten Ton
//! messen** (CLIENT 8.5), fuer die Abnahme eines Sprechwegs.
//!
//! ```text
//! MYL_SPRECHEN=eigen|fremd cargo run --release --example vorlesen -- --aus <ziel.wav> <text>
//! ```
//!
//! Nimmt den Weg, den die Sinne finden (`MYL_SPRECHEN` waehlt zwischen dem
//! eigenen Qwen3-TTS und einem Skript oder piper), waermt den Sprecher vor wie die
//! Oberflaeche, schickt den Text durch [`myl_senses::sprechen::Vorleser`]
//! und meldet, wann das erste Stueck klang und wann alles fertig war.
//! Alle Stuecke kommen hintereinander nach `--aus` (eine Abtastrate). ⚑ Mit
//! der Stimmprobe des Nutzers, also nichts davon ins Repositorium.

use std::sync::{Arc, Mutex};

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut aus, mut text) = (None, Vec::new());
    while let Some(a) = args.next() {
        match a.as_str() {
            "--aus" => aus = Some(args.next().unwrap_or_else(|| fehler("--aus ohne Datei"))),
            _ => text.push(a),
        }
    }
    let (Some(aus), false) = (aus, text.is_empty()) else { fehler("Aufruf: vorlesen --aus <ziel.wav> <text>") };
    let sinne = myl_senses::Sinne::finden();
    let zeug = sinne.sprechen.as_ref().unwrap_or_else(|m| fehler(&m.bericht())).clone();
    let geteilt: myl_senses::sprechen::Geteilter = Arc::default();
    let uhr = std::time::Instant::now();
    myl_senses::sprechen::vorwaermen(&zeug, &geteilt).unwrap_or_else(|f| fehler(&f));
    eprintln!("{}: vorgewaermt in {:.1} s", zeug.name(), uhr.elapsed().as_secs_f64());

    let stuecke: Arc<Mutex<Vec<(f64, myl_senses::ton::Ton)>>> = Arc::default();
    let sammler = Arc::clone(&stuecke);
    let start = std::time::Instant::now();
    let mut v = myl_senses::sprechen::Vorleser::neu_geteilt(
        &zeug,
        Box::new(move |wav: &std::path::Path| {
            let ton = myl_senses::ton::lesen(&std::fs::read(wav).map_err(|f| f.to_string())?).map_err(|f| format!("{f:?}"))?;
            sammler.lock().expect("Sammler").push((start.elapsed().as_secs_f64(), ton));
            Ok(())
        }),
        Some(Arc::clone(&geteilt)),
    );
    v.schub(&text.join(" "));
    let fehler_liste = v.abschliessen();
    let fertig = start.elapsed().as_secs_f64();
    if !fehler_liste.is_empty() {
        fehler(&format!("Fehler: {fehler_liste:?}"));
    }
    let stuecke = Arc::try_unwrap(stuecke).expect("allein").into_inner().expect("Sammler");
    let Some((erster, _)) = stuecke.first() else { fehler("kein Ton") };
    let rate = stuecke[0].1.rate;
    let proben: Vec<f32> = stuecke.iter().flat_map(|(_, t)| myl_senses::ton::umtasten(t, rate).proben).collect();
    let ton_s = proben.len() as f64 / f64::from(rate);
    // Abspielen in Echtzeit nachgestellt: Jedes Stueck beginnt, wenn es da
    // ist und das vorige verklungen; was dazwischen fehlt, ist eine Luecke.
    let (mut ende, mut luecken) = (*erster, 0f64);
    for (da, t) in &stuecke {
        luecken += (da - ende).max(0.0);
        ende = ende.max(*da) + t.proben.len() as f64 / f64::from(t.rate);
    }
    eprintln!(
        "erster Ton nach {erster:.2} s, fertig nach {fertig:.2} s, {} Stuecke, {ton_s:.2} s Ton, Verhaeltnis {:.2}, Luecken {luecken:.2} s",
        stuecke.len(),
        fertig / ton_s.max(1e-9)
    );
    std::fs::write(&aus, myl_senses::ton::schreiben(&myl_senses::ton::Ton { rate, proben })).unwrap_or_else(|f| fehler(&format!("{aus}: {f}")));
}

fn fehler(text: &str) -> ! {
    eprintln!("{text}");
    std::process::exit(2)
}
