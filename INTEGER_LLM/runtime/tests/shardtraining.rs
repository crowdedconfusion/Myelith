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
    rueckwaerts, rueckwaerts_mit, sammlung_anwenden, vorwaerts, Fortschreibung, Sammlung,
    Shardgewichte, Shardvorgaben,
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
    let ms = vorwaerts(&m, &mut ganz_gew, &v_ganz, &hidden).expect("vorwaerts");
    let ganz = rueckwaerts(&m, &mut ganz_gew, &v_ganz, &ms, &g_aus).expect("rueckwaerts");

    // --- In zwei Shards --------------------------------------------
    let mut a_gew = Shardgewichte::aus_modell(&m, von, mitte).expect("Gewichte A");
    let mut b_gew = Shardgewichte::aus_modell(&m, mitte, bis).expect("Gewichte B");
    let v_a = Shardvorgaben { von, bis: mitte, schritt: 0, lr_zaehler: 1, lr_nenner: LR_NENNER };
    let v_b = Shardvorgaben { von: mitte, bis, schritt: 0, lr_zaehler: 1, lr_nenner: LR_NENNER };

    // Vorwaerts durch beide, der Ausgang von A ist der Eingang von B.
    let ms_a = vorwaerts(&m, &mut a_gew, &v_a, &hidden).expect("vorwaerts A");
    let ms_b = vorwaerts(&m, &mut b_gew, &v_b, &ms_a.ausgang).expect("vorwaerts B");
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
        assert_eq!(g.matrizen(), s.matrizen(), "Ebene {} des Bereichs weicht ab", von + i);
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
    let ms = vorwaerts(&m, &mut richtig, &v, &hidden).expect("vorwaerts");
    let _ = rueckwaerts(&m, &mut richtig, &v, &ms, &g_aus).expect("rueckwaerts");

    // Derselbe Lauf, aber der Wuerfel bekommt eine andere Ebenennummer:
    // nachgestellt ueber `schritt`, denn beide gehen gleichermassen in
    // die Kennung ein.
    let mut anders = Shardgewichte::aus_modell(&m, von, bis).expect("Gewichte");
    let v2 = Shardvorgaben { schritt: 1, ..v };
    let ms2 = vorwaerts(&m, &mut anders, &v2, &hidden).expect("vorwaerts");
    let _ = rueckwaerts(&m, &mut anders, &v2, &ms2, &g_aus).expect("rueckwaerts");

    let a: Vec<_> = richtig.master.iter().map(|e| e.matrizen()).collect();
    let b: Vec<_> = anders.master.iter().map(|e| e.matrizen()).collect();
    assert_ne!(
        a, b,
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
    let anfang: Vec<Vec<Vec<i32>>> = gew
        .master
        .iter()
        .map(|e| e.matrizen().iter().map(|m| m.to_vec()).collect())
        .collect();
    let mut nach_eins = Vec::new();
    for s in 0..2u64 {
        let v = Shardvorgaben { von, bis, schritt: s, lr_zaehler: 1, lr_nenner: LR_NENNER };
        let ms = vorwaerts(&m, &mut gew, &v, &hidden).expect("vorwaerts");
        let e = rueckwaerts(&m, &mut gew, &v, &ms, &g_aus).expect("rueckwaerts");
        if s == 0 {
            nach_eins = gew
                .master
                .iter()
                .map(|e| e.matrizen().iter().map(|m| m.to_vec()).collect::<Vec<Vec<i32>>>())
                .collect::<Vec<_>>();
            assert!(e.bewegte_gewichte > 0, "der erste Schritt bewegte nichts");
        } else {
            assert!(
                e.bewegte_gewichte > 0,
                "der zweite Schritt bewegte nichts gegenueber dem Anfang"
            );
        }
    }
    let nach_zwei: Vec<Vec<Vec<i32>>> = gew
        .master
        .iter()
        .map(|e| e.matrizen().iter().map(|m| m.to_vec()).collect())
        .collect();
    assert_ne!(anfang, nach_eins, "der erste Schritt wirkte nicht");
    assert_ne!(nach_eins, nach_zwei, "der zweite Schritt fing wieder beim Artefakt an");
}

// ===========================================================================
// Das Expertengemisch über Shardgrenzen
// ===========================================================================

fn moe_artefakte() -> std::path::PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    std::path::PathBuf::from(manifest).join("..").join("artifacts").join("qwen3-30b-a3b")
}

/// ⚑ **Dieselbe Frage wie oben, aber für Gemischebenen.**
///
/// Ein Bereich aus Gemischebenen, in zwei Shards zerlegt, muss dasselbe
/// ergeben wie derselbe Bereich am Stück. **Das ist bei einem Gemisch
/// weniger selbstverständlich als bei einer dichten Ebene**, denn hier
/// hängt der Versatz im Würfelraum an der Expertennummer, und welche
/// Experten überhaupt vorkommen, entscheidet der Router.
#[test]
#[ignore = "laedt 29 GB Artefakte; mit --ignored ausfuehren"]
fn zwei_shards_ueber_gemischebenen_rechnen_wie_einer() {
    let dir = moe_artefakte();
    if !dir.exists() {
        eprintln!("Artefakt fehlt, uebersprungen: {}", dir.display());
        return;
    }
    let m = load_model(&dir).expect("Modell laedt");
    let bis = m.num_layers;
    let von = bis - 4;
    let mitte = von + 2;
    let hidden = strom_vor(&m, von);
    let g_aus = gradient(m.hidden_size);

    let mut ganz_gew = Shardgewichte::aus_modell(&m, von, bis).expect("Gewichte");
    assert!(
        ganz_gew.master.iter().all(|e| e.ist_gemisch()),
        "dieses Modell hat keine Gemischebenen; der Test prueft nichts"
    );
    let v_ganz = Shardvorgaben { von, bis, schritt: 0, lr_zaehler: 1, lr_nenner: 1 << 18 };
    let ms = vorwaerts(&m, &mut ganz_gew, &v_ganz, &hidden).expect("vorwaerts");
    let ganz = rueckwaerts(&m, &mut ganz_gew, &v_ganz, &ms, &g_aus).expect("rueckwaerts");

    let mut a_gew = Shardgewichte::aus_modell(&m, von, mitte).expect("Gewichte A");
    let mut b_gew = Shardgewichte::aus_modell(&m, mitte, bis).expect("Gewichte B");
    let v_a = Shardvorgaben { von, bis: mitte, schritt: 0, lr_zaehler: 1, lr_nenner: 1 << 18 };
    let v_b = Shardvorgaben { von: mitte, bis, schritt: 0, lr_zaehler: 1, lr_nenner: 1 << 18 };

    let ms_a = vorwaerts(&m, &mut a_gew, &v_a, &hidden).expect("vorwaerts A");
    let ms_b = vorwaerts(&m, &mut b_gew, &v_b, &ms_a.ausgang).expect("vorwaerts B");
    assert_eq!(ms_b.ausgang, ms.ausgang, "der Vorwaertslauf ist schon verschieden");

    let erg_b = rueckwaerts(&m, &mut b_gew, &v_b, &ms_b, &g_aus).expect("rueckwaerts B");
    let erg_a =
        rueckwaerts(&m, &mut a_gew, &v_a, &ms_a, &erg_b.eingang).expect("rueckwaerts A");

    assert_eq!(erg_a.eingang, ganz.eingang, "der Eingangsgradient weicht ab");
    assert_eq!(erg_a.eingang_frac, ganz.eingang_frac, "die Skala weicht ab");

    let geteilt: Vec<_> = a_gew.master.iter().chain(b_gew.master.iter()).collect();
    let am_stueck: Vec<_> = ganz_gew.master.iter().collect();
    assert_eq!(geteilt.len(), am_stueck.len());
    for (i, (g, s)) in geteilt.iter().zip(am_stueck.iter()).enumerate() {
        assert_eq!(g.matrizen(), s.matrizen(), "Ebene {} weicht ab", von + i);
    }
    assert_eq!(
        erg_a.bewegte_gewichte + erg_b.bewegte_gewichte,
        ganz.bewegte_gewichte,
        "verschieden viele Gewichte bewegt"
    );
    assert!(ganz.bewegte_gewichte > 0, "der Schritt hat gar nichts bewegt");
    eprintln!(
        "\n--- Gemischebenen {von}..{bis} ueber zwei Shards ---\n  \
         {} von {} Gewichten bewegt, bitgleich ueber beide Zuschnitte",
        ganz.bewegte_gewichte, ganz.gewichte_gesamt
    );
}

/// ⚑ **Nur die gewählten Experten kommen in den Shard.**
///
/// Bei 128 Experten je Ebene und Top-8 über sechs Positionen dürfen es
/// höchstens 48 je Ebene sein. Hielte der Shard alle, wären es 2,4 GB
/// je Ebene, und ein Pod mit zwölf Ebenen liefe auf keiner Maschine.
#[test]
#[ignore = "laedt 29 GB Artefakte; mit --ignored ausfuehren"]
fn nur_gewaehlte_experten_werden_gehalten() {
    let dir = moe_artefakte();
    if !dir.exists() {
        return;
    }
    let m = load_model(&dir).expect("Modell laedt");
    let von = m.num_layers - 1;
    let hidden = strom_vor(&m, von);
    let mut gew = Shardgewichte::aus_modell(&m, von, m.num_layers).expect("Gewichte");
    let v = Shardvorgaben {
        von,
        bis: m.num_layers,
        schritt: 0,
        lr_zaehler: 1,
        lr_nenner: 1 << 18,
    };
    let _ = vorwaerts(&m, &mut gew, &v, &hidden).expect("vorwaerts");

    let integer_llm_runtime::shardtraining::Ebenenstand::Gemisch { experten, .. } =
        &gew.master[0]
    else {
        panic!("die letzte Ebene ist kein Gemisch");
    };
    let gehalten = experten.len();
    assert!(gehalten > 0, "kein einziger Experte wurde materialisiert");
    assert!(
        gehalten <= FOLGE.len() * 8,
        "{gehalten} Experten gehalten, hoechstens {} erwartet",
        FOLGE.len() * 8
    );
    eprintln!(
        "\n--- Materialisierte Experten ---\n  {gehalten} von 128, bei {} Positionen und Top-8",
        FOLGE.len()
    );
}

/// ⚑ **Der Kettenlauf muss dem Vorwärtslauf des Modells gleichen.**
///
/// `zwei_shards_rechnen_dasselbe_wie_einer` vergleicht den Shardweg mit
/// **sich selbst**, in zwei Zuschnitten. Das ist notwendig und nicht
/// hinreichend: Beide könnten dasselbe Falsche rechnen.
///
/// Hier steht der Shardweg gegen `run_layers_mit_mitschnitt`, also gegen
/// den Weg, den die Inferenz geht. **Wichen sie ab, träfe das Training
/// Gewichte für einen Vorwärtspass, den niemand rechnet**, und jede
/// Perplexitätsmessung wäre die eines anderen Modells.
#[test]
fn der_shardweg_rechnet_wie_das_modell() {
    let Some(m) = modell() else { return };
    let mut gew = Shardgewichte::aus_modell(&m, 0, m.num_layers).expect("Gewichte");
    let v = Shardvorgaben {
        von: 0,
        bis: m.num_layers,
        schritt: 0,
        lr_zaehler: 1,
        lr_nenner: LR_NENNER,
    };
    let strom: Vec<Vec<i16>> = FOLGE.iter().map(|t| m.embed_token(*t as usize)).collect();
    let ms = vorwaerts(&m, &mut gew, &v, &strom).expect("vorwaerts");

    // Der Weg des Modells, Position für Position mit KV-Cache.
    let mut cache = KVCache::for_range(0, m.num_layers, m.num_kv_heads);
    let mut modellstrom: Vec<Vec<i16>> = Vec::new();
    for (pos, tid) in FOLGE.iter().enumerate() {
        let start = m.embed_token(*tid as usize);
        let y = m.run_layers(start, pos, &mut cache, 0, m.num_layers);
        modellstrom.push(y);
    }

    assert_eq!(ms.ausgang.len(), modellstrom.len(), "verschieden viele Positionen");
    for (p, (a, b)) in ms.ausgang.iter().zip(modellstrom.iter()).enumerate() {
        assert_eq!(a, b, "Position {p}: der Shardweg weicht vom Modell ab");
    }
}

/// ⚑ **Auch gesammelt ist ein geteilter Lauf wie ein ungeteilter.**
///
/// # Warum das eine eigene Prüfung braucht
///
/// Die Sammlung führt Buch je **Ebene im Shard** und je Matrix, und der
/// Index einer Ebene im Shard ist nicht ihre Nummer im Modell: Ebene 12
/// des Modells ist im zweiten von zwei Shards die Ebene 0. Wer beim
/// Anwenden den lokalen Index in den Würfel gäbe, bekäme für dieselbe
/// Arbeit verschiedene Gewichte, je nachdem, wie der Pod geschnitten
/// war, und zwei ehrliche Pods liefen auseinander.
///
/// `sammlung_anwenden` rechnet deshalb `von + i` zurück auf die globale
/// Ebene. **Dieser Test hält das fest**, und die Zeile darunter ist
/// seine Gegenprobe.
#[test]
fn gesammelt_rechnen_zwei_shards_wie_einer() {
    let Some(m) = modell() else { return };
    let bis = m.num_layers;
    let von = bis - 4;
    let mitte = von + 2;
    let hidden = strom_vor(&m, von);
    let g_aus = gradient(m.hidden_size);

    // --- Am Stueck, gesammelt --------------------------------------
    let mut ganz_gew = Shardgewichte::aus_modell(&m, von, bis).expect("Gewichte");
    let v_ganz = Shardvorgaben { von, bis, schritt: 0, lr_zaehler: 1, lr_nenner: LR_NENNER };
    let mut s_ganz = Sammlung::neu();
    let ms = vorwaerts(&m, &mut ganz_gew, &v_ganz, &hidden).expect("vorwaerts");
    rueckwaerts_mit(
        &m,
        &mut ganz_gew,
        &v_ganz,
        &ms,
        &g_aus,
        &mut Fortschreibung::Sammeln(&mut s_ganz),
    )
    .expect("rueckwaerts");
    let erg_ganz = sammlung_anwenden(&s_ganz, &mut ganz_gew, &v_ganz);

    // --- In zwei Shards, jeder mit eigener Sammlung -----------------
    let mut a_gew = Shardgewichte::aus_modell(&m, von, mitte).expect("Gewichte A");
    let mut b_gew = Shardgewichte::aus_modell(&m, mitte, bis).expect("Gewichte B");
    let v_a = Shardvorgaben { von, bis: mitte, schritt: 0, lr_zaehler: 1, lr_nenner: LR_NENNER };
    let v_b = Shardvorgaben { von: mitte, bis, schritt: 0, lr_zaehler: 1, lr_nenner: LR_NENNER };
    let mut s_a = Sammlung::neu();
    let mut s_b = Sammlung::neu();

    let ms_a = vorwaerts(&m, &mut a_gew, &v_a, &hidden).expect("vorwaerts A");
    let ms_b = vorwaerts(&m, &mut b_gew, &v_b, &ms_a.ausgang).expect("vorwaerts B");
    let erg_b = rueckwaerts_mit(
        &m,
        &mut b_gew,
        &v_b,
        &ms_b,
        &g_aus,
        &mut Fortschreibung::Sammeln(&mut s_b),
    )
    .expect("rueckwaerts B");
    rueckwaerts_mit(
        &m,
        &mut a_gew,
        &v_a,
        &ms_a,
        &erg_b.eingang,
        &mut Fortschreibung::Sammeln(&mut s_a),
    )
    .expect("rueckwaerts A");
    let e_a = sammlung_anwenden(&s_a, &mut a_gew, &v_a);
    let e_b = sammlung_anwenden(&s_b, &mut b_gew, &v_b);

    // --- Der Vergleich ---------------------------------------------
    let geteilt: Vec<_> = a_gew.master.iter().chain(b_gew.master.iter()).collect();
    let am_stueck: Vec<_> = ganz_gew.master.iter().collect();
    assert_eq!(geteilt.len(), am_stueck.len(), "verschieden viele Ebenen");
    for (i, (g, s)) in geteilt.iter().zip(am_stueck.iter()).enumerate() {
        assert_eq!(
            g.matrizen(),
            s.matrizen(),
            "Ebene {} weicht ab: die Sammlung ist nicht shardtransparent",
            von + i
        );
    }
    assert_eq!(
        e_a.bewegte_gewichte + e_b.bewegte_gewichte,
        erg_ganz.bewegte_gewichte,
        "verschieden viele Gewichte bewegt"
    );
    assert!(erg_ganz.bewegte_gewichte > 0, "der Sammelschritt hat nichts bewegt");

    // ⚑ **Die Gegenprobe steckt im selben Test.** Wendet man die
    // Sammlung von A mit den Vorgaben von B an, also mit einer falschen
    // globalen Ebene, muss ein anderes Ergebnis herauskommen.
    let mut falsch = Shardgewichte::aus_modell(&m, von, mitte).expect("Gewichte A2");
    let v_falsch =
        Shardvorgaben { von: mitte, bis, schritt: 0, lr_zaehler: 1, lr_nenner: LR_NENNER };
    let e_falsch = sammlung_anwenden(&s_a, &mut falsch, &v_falsch);
    assert_ne!(
        e_falsch.delta_commitment, e_a.delta_commitment,
        "eine falsche globale Ebene muesste ein anderes Delta ergeben"
    );

    eprintln!(
        "\n--- Gesammeltes Training, Ebenen {von}..{bis} ---\n  \
         {} von {} Gewichten in EINEM Wurf bewegt, bitgleich ueber beide Zuschnitte",
        erg_ganz.bewegte_gewichte, erg_ganz.gewichte_gesamt
    );
}
