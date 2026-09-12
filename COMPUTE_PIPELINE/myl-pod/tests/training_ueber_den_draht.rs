//! Vier Prozesse trainieren denselben Pod wie einer.
//!
//! # ⚑ Was hier zum ersten Mal geprüft wird
//!
//! Fund 186 hat den Draht für die **Inferenz** geschlossen: vier
//! eigenständige Prozesse liefern dieselben Token wie ein Prozess. Für
//! das **Training** galt das nicht: `Trainingswerk` hielt alle vier
//! Shardgewichte in **einem** Adressraum, und damit war der Nachweis für
//! den Rückwärtspass nie erbracht.
//!
//! **Der Rückwärtspass ist der schwerere Fall.** Vorwärts fliesst ein
//! Residualstrom in eine Richtung; rückwärts muss der Gradient
//! **zurück**, und jeder Shard braucht dabei seinen Mitschnitt aus dem
//! Vorwärtslauf. Der Mitschnitt bleibt beim Shard, geht also nicht über
//! den Draht, und genau das kann still danebengehen: Ein Shard, der ihn
//! verliert oder verwechselt, rechnet einen Gradienten aus fremden
//! Zwischenwerten.
//!
//! ⚑ **Und die Gewichte müssen bitgleich sein, nicht nur ähnlich.**
//! Zwei Pods, die dasselbe Segment rechnen, müssen dasselbe
//! Δ-Commitment melden; sonst meldet der Redundanzvergleich zwei
//! ehrliche Pods als uneinig.

use std::io::BufRead;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

use myl_pod::shardweg::{Shardweg, UeberDenDraht};
use myl_pod::trainingswerk::{anwenden_ueber_den_draht, folge_ueber_den_draht};

const SHARDS: usize = 4;
const FOLGE: [usize; 6] = [9707, 374, 264, 1273, 315, 279];
const LR_NENNER: i64 = 1 << 16;

fn wurzel() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest).join("..").join("..")
}

fn artefakte() -> PathBuf {
    wurzel().join("INTEGER_LLM").join("artifacts").join("myelith-0.6b")
}

fn binary() -> PathBuf {
    let profil = if cfg!(debug_assertions) { "debug" } else { "release" };
    wurzel().join("target-shared").join(profil).join("myl-shard")
}

/// Startet einen Shard **mit Trainingshälfte**.
fn starten(nummer: usize) -> (Child, std::net::SocketAddr) {
    let mut kind = Command::new(binary())
        .arg("--artefakte")
        .arg(artefakte())
        .arg("--shard")
        .arg(nummer.to_string())
        .arg("--shards")
        .arg(SHARDS.to_string())
        .arg("--probeschluessel")
        .arg("--training")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("myl-shard startet");
    let aus = kind.stdout.take().expect("Standardausgabe");
    let mut leser = std::io::BufReader::new(aus);
    let mut zeile = String::new();
    if leser.read_line(&mut zeile).unwrap_or(0) == 0 {
        let mut fehler = String::new();
        if let Some(mut e) = kind.stderr.take() {
            use std::io::Read;
            let _ = e.read_to_string(&mut fehler);
        }
        let _ = kind.kill();
        panic!("Shard {nummer} nannte keine Adresse.\nFehlerausgabe:\n{fehler}");
    }
    let adresse = zeile
        .trim()
        .strip_prefix(&format!("SHARD {nummer} "))
        .unwrap_or_else(|| panic!("unerwartete Zeile von Shard {nummer}: {zeile:?}"))
        .parse()
        .expect("Adresse lesbar");
    (kind, adresse)
}

struct Aufraeumer(Vec<Child>);
impl Drop for Aufraeumer {
    fn drop(&mut self) {
        for k in &mut self.0 {
            let _ = k.kill();
            let _ = k.wait();
        }
    }
}

/// Der Gradient am Ausgang des letzten Shards, deterministisch.
///
/// ⚑ **Ein gerechneter Gradient wäre hier der falsche Aufbau.** Er käme
/// aus dem Verlust und damit aus einer zweiten Rechnung, die beide
/// Seiten gleich falsch machen könnten. Ein festgelegtes Muster prüft
/// genau das, worum es geht: dass **derselbe** Gradient über beide
/// Zuschnitte zu denselben Gewichten führt.
fn gradient(positionen: usize, breite: usize) -> Vec<Vec<i32>> {
    (0..positionen)
        .map(|p| {
            (0..breite)
                .map(|i| (((p * 31 + i * 17) % 401) as i32) - 200)
                .collect()
        })
        .collect()
}

#[test]
fn vier_prozesse_trainieren_wie_ein_prozess() {
    if !artefakte().exists() {
        eprintln!("Artefakt fehlt, uebersprungen");
        return;
    }
    assert!(
        binary().exists(),
        "myl-shard ist nicht gebaut.\nBauen mit:\n  \
         cd COMPUTE_PIPELINE/myl-pod && cargo build --bin myl-shard"
    );

    let modell = Arc::new(
        integer_llm_runtime::loader::load_model(&artefakte()).expect("Modell"),
    );
    let hidden: Vec<Vec<i16>> = FOLGE.iter().map(|t| modell.embed_token(*t)).collect();
    let g_aus = gradient(FOLGE.len(), modell.hidden_size);

    // --- In einem Prozess, aber in denselben vier Zuschnitten -----
    //
    // ⚑ **Vier Bereiche und nicht einer, und das ist der Punkt.** Ein
    // Lauf über alle Ebenen am Stück ergäbe **einen** Abdruck; der Draht
    // liefert **vier**. Zwei verschiedene Zusammenfassungen liessen sich
    // nur über die Zahl bewegter Gewichte vergleichen, und die ist eine
    // schwächere Aussage als Bitgleichheit. Hier entsteht dieselbe Spur
    // auf beiden Wegen, Eintrag für Eintrag.
    let (ein_spur, ein_bewegte, ein_gesamt) = {
        use integer_llm_runtime::shardtraining::*;
        let layer = modell.num_layers;
        let grenzen: Vec<usize> = (0..=SHARDS).map(|s| layer * s / SHARDS).collect();
        let mut gewichte: Vec<Shardgewichte> = (0..SHARDS)
            .map(|s| {
                Shardgewichte::aus_modell(&modell, grenzen[s], grenzen[s + 1]).expect("Gewichte")
            })
            .collect();
        // ⚑ **Normiert, wie der Pod es tut.** Ein Vergleich gegen die
        // unnormierte Rechnung praefte zwei verschiedene Verfahren und
        // meldete den Unterschied als Draht-Fehler.
        let mut sammlungen: Vec<Sammlung> =
            (0..SHARDS).map(|_| Sammlung::normiert()).collect();
        let vorgabe = |s: usize, schritt: u64| Shardvorgaben {
            von: grenzen[s],
            bis: grenzen[s + 1],
            schritt,
            lr_zaehler: 1,
            lr_nenner: LR_NENNER,
        };

        // Vorwaerts durch alle, jeder Mitschnitt bleibt liegen.
        let mut strom = hidden.clone();
        let mut mitschnitte = Vec::with_capacity(SHARDS);
        for (s, g) in gewichte.iter_mut().enumerate().take(SHARDS) {
            let ms = vorwaerts(&modell, g, &vorgabe(s, 0), &strom).expect("vorwaerts");
            strom = ms.ausgang.clone();
            mitschnitte.push(ms);
        }
        // Rueckwaerts zurueck, gesammelt.
        let mut grad = g_aus.clone();
        for s in (0..SHARDS).rev() {
            let erg = rueckwaerts_mit(
                &modell,
                &mut gewichte[s],
                &vorgabe(s, 0),
                &mitschnitte[s],
                &grad,
                &mut Fortschreibung::Sammeln(&mut sammlungen[s]),
            )
            .expect("rueckwaerts");
            sammlungen[s].folge_fertig();
            grad = erg.eingang;
        }
        // Anwenden, Shard fuer Shard.
        let mut spur = Vec::with_capacity(SHARDS);
        let mut bewegte = 0usize;
        let mut gesamt = 0usize;
        for s in 0..SHARDS {
            let e = sammlung_anwenden(&sammlungen[s], &mut gewichte[s], &vorgabe(s, 0));
            spur.push(e.delta_commitment);
            bewegte += e.bewegte_gewichte;
            gesamt += e.gewichte_gesamt;
        }
        (spur, bewegte, gesamt)
    };

    // --- Über vier Prozesse ---------------------------------------
    let mut kinder: Vec<Child> = Vec::new();
    let mut adressen = Vec::new();
    for n in 0..SHARDS {
        let (kind, adresse) = starten(n);
        kinder.push(kind);
        adressen.push(adresse);
    }
    let _wache = Aufraeumer(kinder);
    let weg = UeberDenDraht::neu(adressen);

    folge_ueber_den_draht(&weg as &dyn Shardweg, 7, hidden, g_aus, LR_NENNER)
        .expect("die Folge laeuft ueber den Draht");
    let ueber_draht = anwenden_ueber_den_draht(&weg as &dyn Shardweg, 0, LR_NENNER)
        .expect("anwenden ueber den Draht");

    // --- Der Vergleich --------------------------------------------
    assert_eq!(
        ueber_draht.bewegte_gewichte, ein_bewegte,
        "ueber vier Prozesse bewegen sich andere Gewichte als in einem"
    );
    assert_eq!(
        ueber_draht.gewichte_gesamt, ein_gesamt,
        "die Pods halten verschieden viele Gewichte"
    );
    // ⚑ **Die eigentliche Aussage: dieselbe Spur, Eintrag fuer Eintrag.**
    // Die Zahl bewegter Gewichte koennte zufaellig stimmen; vier gleiche
    // Abdruecke koennen es nicht.
    let draht_hex: Vec<String> = ueber_draht
        .delta_je_shard
        .iter()
        .map(|h| h.as_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>())
        .collect();
    assert_eq!(
        draht_hex, ein_spur,
        "die Spur ueber den Draht weicht von der im Prozess ab"
    );
    assert!(ueber_draht.bewegte_gewichte > 0, "ueber den Draht hat sich nichts bewegt");
    assert_eq!(ueber_draht.aus_der_form, None, "ein Shard hat die Uebertragungsform verlassen");
    assert_eq!(ueber_draht.delta_je_shard.len(), SHARDS, "die Spur hat nicht vier Eintraege");

    eprintln!(
        "\n--- Training ueber vier Prozesse ---\n  \
         {} von {} Gewichten bewegt, Spur ueber {} Shards\n  \
         Pod-Commitment: {}",
        ueber_draht.bewegte_gewichte,
        ueber_draht.gewichte_gesamt,
        ueber_draht.delta_je_shard.len(),
        ueber_draht.delta_commitment
    );
}
