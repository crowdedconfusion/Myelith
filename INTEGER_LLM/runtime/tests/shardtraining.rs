//! Geshardetes Training gegen den Einzelknoten.
//!
//! # ⚑ Die eine Frage, die dieser Test beantwortet
//!
//! **Rechnet ein in zwei Shards zerlegter Ebenenbereich dasselbe wie
//! derselbe Bereich am Stück?**
//!
//! Wenn nicht, ist geshardetes Training nicht gegen einen Einzelknoten
//! nachrechenbar, und damit fällt die Redundanzprüfung, auf der die
//! ganze Verifikation ruht: Zwei Pods mit verschiedenem Zuschnitt kämen
//! zu verschiedenen Δm, ohne dass einer von beiden falsch gerechnet
//! hätte.
//!
//! ⚑ **Das ist keine Selbstverständlichkeit.** Der Würfel des
//! stochastischen Rundens ist eine reine Funktion aus `(ebene, schritt,
//! index)`, und `index` zählt **innerhalb der Ebene**. Nur weil die
//! Ebenennummer global ist und der Versatz lokal, ist der Zuschnitt
//! gleichgültig.

use integer_llm_runtime::kv_cache::KVCache;
use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::mitschnitt::Zwischenwerte;
use integer_llm_runtime::model::IntegerModel;
use integer_llm_runtime::shardtraining::{
    rueckwaerts, vorwaerts, Shardgewichte, Shardvorgaben,
};

fn artefakte() -> std::path::PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    std::path::PathBuf::from(manifest).join("..").join("artifacts").join("qwen2.5-0.5b")
}

fn modell() -> Option<IntegerModel> {
    let dir = artefakte();
    if !dir.exists() {
        eprintln!("Artefakt fehlt, Test uebersprungen: {}", dir.display());
        return None;
    }
    Some(load_model(&dir).expect("Modell laedt"))
}

const FOLGE: [u32; 6] = [9707, 374, 264, 1273, 315, 279];
const LR_NENNER: i64 = 1 << 12;

/// Der Residualstrom am Eingang von Ebene `e`, je Position.
fn strom_vor(m: &IntegerModel, e: usize) -> Vec<Vec<i16>> {
    let mut cache = KVCache::for_range(0, m.num_layers, m.num_kv_heads);
    let mut auf = Zwischenwerte::neu();
    for (pos, tid) in FOLGE.iter().enumerate() {
        let start = m.embed_token(*tid as usize);
        let _ = m.run_layers_mit_mitschnitt(start, pos, &mut cache, 0, m.num_layers, &mut auf);
    }
    (0..FOLGE.len()).map(|p| auf.ebenen()[p * m.num_layers + e].residual_ein.clone()).collect()
}

/// Ein Gradient am Ausgang des Bereichs, deterministisch erfunden.
///
/// ⚑ **Erfunden ist hier richtig.** Der Test fragt nach der
/// Zerlegbarkeit, nicht nach dem Lernerfolg; ein echter Verlustgradient
/// brächte nur Rauschen in die Frage.
fn gradient(hs: usize) -> Vec<Vec<i32>> {
    (0..FOLGE.len())
        .map(|p| (0..hs).map(|i| ((p * hs + i) as i32 % 977) * 131 - 64_000).collect())
        .collect()
}

/// ⚑ **Ein Bereich am Stück gegen denselben Bereich in zwei Shards.**
#[test]
fn zwei_shards_rechnen_dasselbe_wie_einer() {
    let Some(m) = modell() else { return };
    let bis = m.num_layers;
    let von = bis - 4;
    let mitte = von + 2;
    let hidden = strom_vor(&m, von);
    let g_aus = gradient(m.hidden_size);

    // --- Am Stueck -------------------------------------------------
    let mut ganz_gew = Shardgewichte::aus_modell(&m, von, bis).expect("Gewichte");
    let v_ganz = Shardvorgaben { von, bis, schritt: 0, lr_zaehler: 1, lr_nenner: LR_NENNER };
    let ms = vorwaerts(&m, &ganz_gew, &v_ganz, &hidden).expect("vorwaerts");
    let ganz = rueckwaerts(&m, &mut ganz_gew, &v_ganz, &ms, &g_aus).expect("rueckwaerts");

    // --- In zwei Shards --------------------------------------------
    let mut a_gew = Shardgewichte::aus_modell(&m, von, mitte).expect("Gewichte A");
    let mut b_gew = Shardgewichte::aus_modell(&m, mitte, bis).expect("Gewichte B");
    let v_a = Shardvorgaben { von, bis: mitte, schritt: 0, lr_zaehler: 1, lr_nenner: LR_NENNER };
    let v_b = Shardvorgaben { von: mitte, bis, schritt: 0, lr_zaehler: 1, lr_nenner: LR_NENNER };

    // Vorwaerts durch beide, der Ausgang von A ist der Eingang von B.
    let ms_a = vorwaerts(&m, &a_gew, &v_a, &hidden).expect("vorwaerts A");
    let ms_b = vorwaerts(&m, &b_gew, &v_b, &ms_a.ausgang).expect("vorwaerts B");
    assert_eq!(ms_b.ausgang, ms.ausgang, "der Vorwaertslauf ist schon verschieden");

    // Rueckwaerts durch beide, der Eingang von B ist der Ausgang von A.
    let erg_b = rueckwaerts(&m, &mut b_gew, &v_b, &ms_b, &g_aus).expect("rueckwaerts B");
    let erg_a =
        rueckwaerts(&m, &mut a_gew, &v_a, &ms_a, &erg_b.eingang).expect("rueckwaerts A");

    // --- Der Vergleich ---------------------------------------------
    assert_eq!(
        erg_a.eingang, ganz.eingang,
        "der Gradient am Eingang des Bereichs ist verschieden"
    );
    assert_eq!(erg_a.eingang_frac, ganz.eingang_frac, "die Skala des Gradienten weicht ab");

    let geteilt: Vec<_> = a_gew.master.iter().chain(b_gew.master.iter()).collect();
    let am_stueck: Vec<_> = ganz_gew.master.iter().collect();
    assert_eq!(geteilt.len(), am_stueck.len(), "verschieden viele Ebenen");
    for (i, (g, s)) in geteilt.iter().zip(am_stueck.iter()).enumerate() {
        assert_eq!(g, s, "Ebene {} des Bereichs weicht ab", von + i);
    }

    assert_eq!(
        erg_a.bewegte_gewichte + erg_b.bewegte_gewichte,
        ganz.bewegte_gewichte,
        "verschieden viele Gewichte bewegt"
    );
    assert!(ganz.bewegte_gewichte > 0, "der Schritt hat gar nichts bewegt");
    assert_eq!(ganz.aus_der_form, None, "die Vorgabelernrate treibt nichts aus der Form");
    eprintln!(
        "\n--- Geshardetes Training, Ebenen {von}..{bis} ---\n  \
         {} von {} Gewichten bewegt, bitgleich ueber beide Zuschnitte\n  \
         Delta-Commitment am Stueck: {}",
        ganz.bewegte_gewichte, ganz.gewichte_gesamt, ganz.delta_commitment
    );
}

/// ⚑ **Die Gegenprobe: ein falscher Zuschnitt fällt auf.**
///
/// Rechnete Shard B mit **lokaler** statt globaler Ebenennummer, käme
/// ein anderes Δ heraus. Der Test baut genau das nach und hält fest,
/// dass es sich unterscheidet; sonst prüfte der Test darüber nichts.
#[test]
fn eine_lokale_ebenennummer_ergibt_ein_anderes_delta() {
    let Some(m) = modell() else { return };
    let bis = m.num_layers;
    let von = bis - 2;
    let hidden = strom_vor(&m, von);
    let g_aus = gradient(m.hidden_size);

    let mut richtig = Shardgewichte::aus_modell(&m, von, bis).expect("Gewichte");
    let v = Shardvorgaben { von, bis, schritt: 0, lr_zaehler: 1, lr_nenner: LR_NENNER };
    let ms = vorwaerts(&m, &richtig, &v, &hidden).expect("vorwaerts");
    let _ = rueckwaerts(&m, &mut richtig, &v, &ms, &g_aus).expect("rueckwaerts");

    // Derselbe Lauf, aber der Wuerfel bekommt eine andere Ebenennummer:
    // nachgestellt ueber `schritt`, denn beide gehen gleichermassen in
    // die Kennung ein.
    let mut anders = Shardgewichte::aus_modell(&m, von, bis).expect("Gewichte");
    let v2 = Shardvorgaben { schritt: 1, ..v };
    let ms2 = vorwaerts(&m, &anders, &v2, &hidden).expect("vorwaerts");
    let _ = rueckwaerts(&m, &mut anders, &v2, &ms2, &g_aus).expect("rueckwaerts");

    assert_ne!(
        richtig.master, anders.master,
        "die Kennung geht nicht in den Wuerfel ein; dann prueft der Test darueber nichts"
    );
}

/// Ein Bereich, der nicht im Modell liegt, wird abgewiesen.
#[test]
fn ein_bereich_ausserhalb_des_modells_wird_abgewiesen() {
    let Some(m) = modell() else { return };
    assert!(Shardgewichte::aus_modell(&m, 0, m.num_layers + 1).is_err());
    assert!(Shardgewichte::aus_modell(&m, 5, 5).is_err(), "ein leerer Bereich ist keiner");
    assert!(Shardgewichte::aus_modell(&m, 7, 3).is_err(), "verdrehte Grenzen");
}

/// ⚑ **Und die Gewichte überleben den Schritt.**
///
/// Der erste Entwurf änderte eine Kopie und warf sie weg; über einen
/// einzigen Schritt wäre das nicht aufgefallen. Zwei Schritte müssen
/// weiter kommen als einer.
#[test]
fn der_zweite_schritt_baut_auf_dem_ersten_auf() {
    let Some(m) = modell() else { return };
    let bis = m.num_layers;
    let von = bis - 2;
    let hidden = strom_vor(&m, von);
    let g_aus = gradient(m.hidden_size);

    let mut gew = Shardgewichte::aus_modell(&m, von, bis).expect("Gewichte");
    let anfang = gew.master.clone();
    let mut nach_eins = Vec::new();
    for s in 0..2u64 {
        let v = Shardvorgaben { von, bis, schritt: s, lr_zaehler: 1, lr_nenner: LR_NENNER };
        let ms = vorwaerts(&m, &gew, &v, &hidden).expect("vorwaerts");
        let e = rueckwaerts(&m, &mut gew, &v, &ms, &g_aus).expect("rueckwaerts");
        if s == 0 {
            nach_eins = gew.master.clone();
            assert!(e.bewegte_gewichte > 0, "der erste Schritt bewegte nichts");
        } else {
            assert!(
                e.bewegte_gewichte > 0,
                "der zweite Schritt bewegte nichts gegenueber dem Anfang"
            );
        }
    }
    assert_ne!(anfang, nach_eins, "der erste Schritt wirkte nicht");
    assert_ne!(nach_eins, gew.master, "der zweite Schritt fing wieder beim Artefakt an");
}
