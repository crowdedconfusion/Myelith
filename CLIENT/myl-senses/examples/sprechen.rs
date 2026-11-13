//! **Text mit dem eigenen Qwen3-TTS sprechen** (CLIENT 8.5).
//!
//! ```text
//! cargo run --release --example sprechen -- [--stufen] [--genau [sprecher|rueckweg]] [--rueckweg w8a16] [--gierig | --seed <zahl>] [--sprache german|english|auto] [--stimme <probe.wav> [--mitschrift <text>]] --aus <ziel.wav> <text>
//! ```
//!
//! Liest die Gewichte aus `MODELS/audio/qwen3-tts-0.6b-base` und spricht
//! wie im Betrieb mit frischem Samen ([`myl_senses::sprecher::sprechen_frei`]),
//! gibt ihn aus und rechnet mit `--seed <zahl>` einen Lauf nach; oder mit
//! `--gierig`. Ohne `--stimme` mit der Stimme des Modells; mit `--stimme` nur
//! mit dem Abdruck, mit `--mitschrift` dazu geklont (die Probe in 24 kHz).
//! Gibt die Zeit je Teil aus und das Verhaeltnis von Rechenzeit zu Tondauer;
//! mit `--stufen` dazu die Zeit je Stufe des Rueckwegs. `--genau` rechnet
//! Sprecher und Rueckweg in `f32` statt in int8, `--genau sprecher` oder
//! `--genau rueckweg` nur diesen Teil; `--rueckweg w8a16` rechnet den
//! Rueckweg mit zwei int8-Stellen statt W8A8.
//! ⚑ Fuer Stimmproben, die nicht ins Repositorium gehoeren: die eigene
//! Stimme, Systemstimmen mit unklarer Lizenz.

use myl_senses::sprecher::{bildgrenze, sprechen, sprechen_frei, Sprecher, Stimme, Wahl};
use myl_senses::stimmabdruck::Stimmabdruck;
use myl_senses::textzerleger::Textzerleger;
use myl_senses::tonzerleger::{hin, zurueck, zurueck_hinter, zurueck_mit, Hinweg, Tonzerleger, RATE};

fn main() {
    let mut args = std::env::args().skip(1).peekable();
    let (mut sprache, mut stimme, mut mitschrift, mut aus, mut text) = ("german".to_string(), None, None, None, Vec::new());
    let (mut stufen, mut genau_sprecher, mut genau_rueckweg, mut wahl) = (false, false, false, None);
    let mut w8a16 = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--sprache" => sprache = args.next().unwrap_or_else(|| fehler("--sprache ohne Wert")),
            "--stimme" => stimme = Some(args.next().unwrap_or_else(|| fehler("--stimme ohne Datei"))),
            "--mitschrift" => mitschrift = Some(args.next().unwrap_or_else(|| fehler("--mitschrift ohne Text"))),
            "--stufen" => stufen = true,
            // `--genau` allein: beides; `--genau sprecher` oder `--genau rueckweg`: nur dieser Teil.
            // 📌 Nur ein Teilname wird mitgenommen; zuerst schluckte `--genau`
            // jedes folgende Argument, auch `--seed`, und sprach es mit.
            "--genau" => match args.next_if(|t| t == "sprecher" || t == "rueckweg").as_deref() {
                Some("sprecher") => genau_sprecher = true,
                Some(_) => genau_rueckweg = true,
                None => (genau_sprecher, genau_rueckweg) = (true, true),
            },
            "--rueckweg" => w8a16 = args.next().is_some_and(|w| w == "w8a16"),
            "--gierig" => wahl = Some(Wahl::Gierig),
            "--seed" => wahl = Some(Wahl::Zufall(args.next().and_then(|z| z.parse().ok()).unwrap_or_else(|| fehler("--seed ohne Zahl")))),
            "--aus" => aus = Some(args.next().unwrap_or_else(|| fehler("--aus ohne Datei"))),
            _ => text.push(a),
        }
    }
    let (Some(aus), false) = (aus, text.is_empty()) else {
        fehler("Aufruf: sprechen [--sprache german|english|auto] [--stimme <probe.wav> [--mitschrift <text>]] --aus <ziel.wav> <text>")
    };
    if mitschrift.is_some() && stimme.is_none() {
        fehler("--mitschrift braucht --stimme");
    }
    let text = text.join(" ");
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../MODELS/audio/qwen3-tts-0.6b-base");
    let uhr = std::time::Instant::now();
    let zerleger = Textzerleger::laden(&d).unwrap_or_else(|f| fehler(&f));
    let s = if genau_sprecher { Sprecher::laden_genau(&d.join("model.safetensors")) } else { Sprecher::laden(&d.join("model.safetensors")) }.unwrap_or_else(|f| fehler(&f));
    let tz = d.join("speech_tokenizer/model.safetensors");
    let z = if genau_rueckweg {
        Tonzerleger::laden_genau(&tz)
    } else if w8a16 {
        Tonzerleger::laden_w8a16(&tz)
    } else {
        Tonzerleger::laden(&tz)
    }.unwrap_or_else(|f| fehler(&f));
    eprintln!("geladen in {:.1} s", uhr.elapsed().as_secs_f64());
    let uhr = std::time::Instant::now();
    let stimme = stimme.map(|datei| {
        let b = std::fs::read(&datei).unwrap_or_else(|f| fehler(&format!("{datei}: {f}")));
        let probe = myl_senses::ton::lesen(&b).unwrap_or_else(|f| fehler(&format!("{datei}: {f:?}")));
        let probe = myl_senses::ton::umtasten(&probe, RATE).proben;
        let abdruck = Stimmabdruck::laden(&d.join("model.safetensors")).unwrap_or_else(|f| fehler(&f)).abdruck(&probe);
        let vorbild = mitschrift.as_ref().map(|m| {
            let h = Hinweg::laden(&d.join("speech_tokenizer/model.safetensors")).unwrap_or_else(|f| fehler(&f));
            (zerleger.zerlegen(m), hin(&probe, &h))
        });
        Stimme { abdruck, vorbild }
    });
    if stimme.is_some() {
        eprintln!("Stimmprobe in {:.2} s", uhr.elapsed().as_secs_f64());
    }
    let sprachtoken = (sprache != "auto").then(|| s.sprache(&sprache).unwrap_or_else(|| fehler(&format!("unbekannte Sprache {sprache}"))));
    let ids = zerleger.vorlage(&text);
    let (codes, spur) = match wahl {
        Some(w) => sprechen(&s, &ids, sprachtoken, stimme.as_ref(), bildgrenze(&ids), w),
        None => {
            let (codes, spur, samen) = sprechen_frei(&s, &ids, sprachtoken, stimme.as_ref());
            eprintln!("Seed {samen} (nachrechnen mit --seed {samen})");
            (codes, spur)
        }
    };
    if codes.len() == bildgrenze(&ids) {
        eprintln!("kein Ende: {} Bilder, die Grenze", codes.len());
    }
    let uhr = std::time::Instant::now();
    let proben = match stimme.as_ref().and_then(|st| st.vorbild.as_ref()) {
        Some((_, vorbild)) => zurueck_hinter(vorbild, &codes, &z),
        None if stufen => {
            let mut vorher = std::time::Instant::now();
            zurueck_mit(&codes, &z, |name, _| {
                eprintln!("  Rueckweg {name}: {:.2} s", vorher.elapsed().as_secs_f64());
                vorher = std::time::Instant::now();
            })
        }
        None => zurueck(&codes, &z),
    };
    let dauer_zurueck = uhr.elapsed();
    let ton_s = proben.len() as f64 / f64::from(RATE);
    let rechnen = spur.dauer_sprecher + spur.dauer_vorhersage + dauer_zurueck;
    eprintln!(
        "{} Bilder, {ton_s:.2} s Ton: Sprecher {:.2} s, Vorhersage {:.2} s, Rueckweg {:.2} s; zusammen {:.2} s, Verhaeltnis {:.2}",
        codes.len(),
        spur.dauer_sprecher.as_secs_f64(),
        spur.dauer_vorhersage.as_secs_f64(),
        dauer_zurueck.as_secs_f64(),
        rechnen.as_secs_f64(),
        rechnen.as_secs_f64() / ton_s.max(1e-9),
    );
    std::fs::write(&aus, myl_senses::ton::schreiben(&myl_senses::ton::Ton { rate: RATE, proben })).unwrap_or_else(|f| fehler(&format!("{aus}: {f}")));
}

fn fehler(text: &str) -> ! {
    eprintln!("{text}");
    std::process::exit(2)
}
