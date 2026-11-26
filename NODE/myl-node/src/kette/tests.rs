use super::*;
use myl_consensus::block::Anweisung;

fn zusage_direkt(k: &mut Kette, kennung: myl_types::ids::MinerId) {
    k.probezusage_setzen(kennung);
}

/// Die Zuteilung der laufenden Epoche, aber aus einem gegebenen Hash
/// statt der Saat im Zustand: die Gegenprobe zu Fund 143.
fn zuteilung_aus_hash(k: &Kette, hash: &Hash) -> myl_scheduler::shard_assignment::Zuteilung {
    let z = k.zustand();
    myl_scheduler::zonenzuteilung::zuteilung_der_epoche_mit_zusagen(
        &angemeldete_miner(z),
        z.epoch.0,
        hash,
        Kette::PROBE_SHARDS,
        &crate::netz::PROBEKETTE.podmodell,
        &|m| myl_ledger::transitions::modellzusage_der_epoche(z, m, z.epoch),
    )
}

/// Ein **wirksamer** Burn: Absender ist ein ausgestattetes
/// Testkonto. Ein Burn von einem leeren Konto scheitert still, und
/// ein Test damit belegte nichts.
///
/// ⚑ **Und er ist unterschrieben** (2026-08-28). Ohne Unterschrift
/// verwirft `anwenden` ihn, der Zustand bewegte sich nicht, und der
/// Test belegte wieder nichts: dieselbe Falle wie beim fehlenden
/// Guthaben, nur eine Ebene weiter.
fn burn_nr(wer: u8, betrag: u64, nonce: u64) -> Transaktion {
    Transaktion::signiere(
        &Kette::startwert(),
        &probeschluessel(wer),
        nonce,
        Anweisung::Burn { betrag },
    )
    .expect("signieren")
}

fn burn(wer: u8, betrag: u64) -> Transaktion {
    burn_nr(wer, betrag, 0)
}

/// Die Gebühr einer Transaktion, solange der Grundpreis am Boden der
/// Probekette steht (G1); kleine Blöcke heben ihn nicht.
fn gebuehr(tx: &Transaktion) -> u64 {
    crate::netz::PROBEKETTE.gebuehren.boden_je_byte * tx.gebuehreneinheiten()
}

/// ⚑ **Der Verlauf** trifft Absender und Empfänger, neueste zuerst, und
/// eine Seite setzt dort fort, wo die vorige endete.
#[test]
fn der_verlauf_nennt_ein_und_ausgang_und_blaettert() {
    let mut k = Kette::probestand();
    let von = probekonto(0);
    let nach = probekonto(1);
    let fern = probekonto(2);
    for n in 0..3u64 {
        k.aufnehmen(Transaktion::signiere(&Kette::startwert(), &probeschluessel(0), n, Anweisung::Ueberweisung { nach, betrag: 10 + n }).expect("signieren"));
        k.baue_block();
    }
    k.aufnehmen(burn_nr(0, 5, 3));
    k.baue_block();

    let lang = std::time::Duration::from_secs(5);
    let seite = k.verlauf_seite(&von, None, lang);
    let arten: Vec<(&str, u64, bool)> = seite.eintraege.iter().map(|e| (e.art, e.betrag, e.ausgehend)).collect();
    assert_eq!(arten, vec![("burn", 5, true), ("ueberweisung", 12, true), ("ueberweisung", 11, true), ("ueberweisung", 10, true)], "neueste zuerst");
    assert_eq!(seite.eintraege[1].gegen, Some(nach));
    assert_eq!(seite.weiter_bei, None, "alles auf einer Seite");

    let empfaenger = k.verlauf_seite(&nach, None, lang);
    assert_eq!(empfaenger.eintraege.len(), 3, "Eingänge");
    assert!(empfaenger.eintraege.iter().all(|e| !e.ausgehend && e.gegen == Some(von)), "Gegenseite ist der Absender");
    assert!(k.verlauf_seite(&fern, None, lang).eintraege.is_empty(), "Gegenprobe: unbeteiligt");

    // Blättern: Ab einer Höhe nur noch, was darunter liegt.
    let hoehe_zweite = seite.eintraege[2].hoehe;
    let rest = k.verlauf_seite(&von, Some(hoehe_zweite), lang);
    assert_eq!(rest.eintraege.first().map(|e| e.betrag), Some(11));
    assert_eq!(rest.eintraege.len(), 2);

    // Ohne Lesezeit endet die Seite nach dem ersten Schub mit einem Weiter.
    for _ in 0..VERLAUF_BLOECKE_JE_SCHUB {
        k.baue_block();
    }
    let knapp = k.verlauf_seite(&von, None, std::time::Duration::ZERO);
    assert!(knapp.weiter_bei.is_some(), "die Frist muss greifen");
}

/// ⚑ **Die Gegenprobe zu Fund 85: Eine Anweisung ohne gültige
/// Unterschrift wirkt nicht.**
///
/// Bis zum 2026-08-28 trug eine Transaktion keine Unterschrift, und
/// jeder konnte im Namen jedes Kontos anweisen. Bei `Burn` war das
/// Sachbeschädigung, bei einer Überweisung wäre es Diebstahl
/// gewesen.
#[test]
fn eine_gefaelschte_unterschrift_bewegt_nichts() {
    let mut k = Kette::probestand();
    // 📌 Seit der Historienwurzel bewegt jeder Block den Zustand (er
    // haengt den Hash seines Vorgaengers an). Verglichen wird deshalb
    // mit einem Zwilling, der einen leeren Block baut; beim ersten
    // Block ist der Vorgaenger in beiden derselbe Startblock.
    let mut leer = Kette::probestand();
    let _ = leer.baue_block();

    let mut tx = burn(0, 5_000);
    tx.signatur = myl_types::bls::BlsSignature([7u8; 96]);
    k.aufnehmen(tx);
    let _ = k.baue_block();
    assert_eq!(k.zustandswurzel(), leer.zustandswurzel(), "eine Fälschung darf nichts bewegen");

    // Und der Schlüssel eines anderen Kontos hilft auch nicht: Die
    // Adresse folgt aus dem Schlüssel, also belastet er sein eigenes
    // Konto und nicht das fremde.
    let echt = burn(1, 5_000);
    let konto_eins_vorher = k.zustand().account(&probekonto(1)).balance;
    let konto_null_vorher = k.zustand().account(&probekonto(0)).balance;
    k.aufnehmen(echt);
    let _ = k.baue_block();
    assert_eq!(k.zustand().account(&probekonto(0)).balance, konto_null_vorher);
    assert!(k.zustand().account(&probekonto(1)).balance < konto_eins_vorher);
}

/// ⚑ Dieselbe Transaktion zweimal wirkt einmal.
#[test]
fn eine_wiedereingespielte_transaktion_wirkt_nicht_zweimal() {
    let mut k = Kette::probestand();
    let tx = burn(2, 3_000);

    k.aufnehmen(tx.clone());
    let _ = k.baue_block();
    let nach_dem_ersten = k.zustand().account(&probekonto(2)).balance;
    assert_eq!(k.zustand().account(&probekonto(2)).nonce, 1);

    k.aufnehmen(tx);
    let _ = k.baue_block();
    assert_eq!(
        k.zustand().account(&probekonto(2)).balance,
        nach_dem_ersten,
        "dieselben Bytes ein zweites Mal duerfen nichts bewirken"
    );
    assert_eq!(k.zustand().account(&probekonto(2)).nonce, 1);

    // Mit der naechsten Nummer geht es weiter.
    k.aufnehmen(burn_nr(2, 3_000, 1));
    let _ = k.baue_block();
    assert!(k.zustand().account(&probekonto(2)).balance < nach_dem_ersten);
}

/// ⚑ Eine Ueberweisung ueber einen Block: Fund 83 geschlossen.
#[test]
fn eine_ueberweisung_kommt_ueber_einen_block_an() {
    use myl_consensus::block::Anweisung;
    let mut k = Kette::probestand();
    let von = probekonto(3);
    let nach = probekonto(4);
    let (v0, n0) = (
        k.zustand().account(&von).balance,
        k.zustand().account(&nach).balance,
    );

    let tx = Transaktion::signiere(
        &Kette::startwert(),
        &probeschluessel(3),
        0,
        Anweisung::Ueberweisung { nach, betrag: 2_500 },
    )
    .expect("signieren");
    let g = gebuehr(&tx);
    k.aufnehmen(tx);
    let _ = k.baue_block();

    assert_eq!(k.zustand().account(&von).balance, v0 - 2_500 - g);
    assert_eq!(k.zustand().account(&nach).balance, n0 + 2_500);
}

/// ⚑ Eine Session ueber Bloecke: eroeffnen, ausgeben, widerrufen.
/// **Der Kontrakt begrenzt, und zwar auf der Kette.**
#[test]
fn eine_session_wirkt_und_begrenzt_ueber_bloecke() {
    use myl_consensus::block::Anweisung;
    use myl_types::sitzung::{Grenzen, Sitzungskontrakt, Vorhaben, Waehrung};

    let mut k = Kette::probestand();
    let inhaber = probekonto(6);
    let agent = probekonto(7);
    let empfaenger = probekonto(0);

    let kontrakt = Sitzungskontrakt::neu(
        inhaber,
        agent,
        Grenzen::gesperrt(),
        Grenzen { budget: 5_000, einzellimit: 2_000, schwelle: u64::MAX, zeugenleiter: Vec::new() },
        vec![empfaenger],
        EpochId(0),
        EpochId(u64::MAX),16,
    )
    .expect("gueltiger Kontrakt");
    let id = kontrakt.adresse();

    let sig = |n: u8, nonce: u64, a: Anweisung| {
        Transaktion::signiere(&Kette::startwert(), &probeschluessel(n), nonce, a)
            .expect("signieren")
    };

    k.aufnehmen(sig(6, 0, Anweisung::SitzungEroeffnen { kontrakt }));
    let _ = k.baue_block();
    assert!(k.zustand().sitzung(&id).is_some(), "die Session steht im Zustand");

    // ⚑ **Mit steigender Nummer**, seit die Kette den Riegel gegen
    // die zweite Abbuchung führt.
    let zahlung = |betrag: u64, nummer: u64| Vorhaben {
        sitzung: id,
        handelnder: agent,
        waehrung: Waehrung::Myl,
        betrag,
        empfaenger,
        empfaenger_beweis: myl_types::sitzung::empfaengerbeweis(&[empfaenger], &empfaenger).expect("gelistet"),
        bestaetigt_ausgeliefert: false,
        nummer,
    };
    let empf_vorher = k.zustand().account(&empfaenger).balance;
    let inh_vorher = k.zustand().account(&inhaber).balance;

    // Ueber dem Einzellimit: wirkungslos.
    k.aufnehmen(sig(7, 0, Anweisung::SitzungAusgeben { vorhaben: zahlung(3_000, 1), vollmacht: None }));
    let _ = k.baue_block();
    assert_eq!(k.zustand().account(&empfaenger).balance, empf_vorher);

    // Darunter: es fliesst, und zwar vom Konto des Inhabers.
    k.aufnehmen(sig(7, 1, Anweisung::SitzungAusgeben { vorhaben: zahlung(1_500, 1), vollmacht: None }));
    let _ = k.baue_block();
    assert_eq!(k.zustand().account(&empfaenger).balance, empf_vorher + 1_500);
    assert_eq!(k.zustand().account(&inhaber).balance, inh_vorher - 1_500);

    // Der Agent kann nicht widerrufen, der Inhaber schon.
    k.aufnehmen(sig(7, 2, Anweisung::SitzungWiderrufen { sitzung: id }));
    let _ = k.baue_block();
    assert!(!k.zustand().sitzung(&id).expect("da").zustand.widerrufen);

    k.aufnehmen(sig(6, 1, Anweisung::SitzungWiderrufen { sitzung: id }));
    let _ = k.baue_block();
    assert!(k.zustand().sitzung(&id).expect("da").zustand.widerrufen);

    // Und danach fliesst nichts mehr.
    let nach_widerruf = k.zustand().account(&empfaenger).balance;
    k.aufnehmen(sig(7, 3, Anweisung::SitzungAusgeben { vorhaben: zahlung(100, 2), vollmacht: None }));
    let _ = k.baue_block();
    assert_eq!(k.zustand().account(&empfaenger).balance, nach_widerruf);
}

/// **Der Zustand ändert sich, wenn Transaktionen wirken.**
///
/// Der Test, der beim ersten echten Dreiknotenlauf gefehlt hat.
/// Damals stand die Zustandswurzel bei jeder Höhe auf demselben
/// Wert: Die Burns scheiterten an fehlender Deckung, wurden
/// übersprungen, und die Übereinstimmung der Wurzeln belegte nichts.
/// **Ein Lauf, dessen Zustand sich nie ändert, misst nichts.**
#[test]
fn ein_wirksamer_burn_veraendert_die_zustandswurzel() {
    let mut k = Kette::probestand();
    let vorher = k.zustandswurzel();
    k.aufnehmen(burn(0, 5_000));
    let _ = k.baue_block();
    assert_ne!(
        k.zustandswurzel(),
        vorher,
        "die Zustandswurzel hat sich nicht bewegt: die Transaktion war wirkungslos"
    );
}

#[test]
fn die_testkonten_haben_zu_genesis_guthaben() {
    let k = Kette::probestand();
    for n in 0..PROBEKONTEN {
        assert_eq!(
            k.zustand().account(&probekonto(n)).balance,
            PROBEGUTHABEN,
            "Testkonto {n} ohne Guthaben"
        );
    }
}

#[test]
fn das_konto_eines_knotens_bleibt_stabil() {
    // Ein wechselndes Konto machte zwei Läufe unvergleichbar.
    assert_eq!(konto_fuer("alpha"), konto_fuer("alpha"));
    assert!(
        (0..PROBEKONTEN).any(|n| probekonto(n) == konto_fuer("alpha")),
        "das Konto liegt außerhalb der ausgestatteten"
    );
}

// --- Höhe und Epoche ------------------------------------------

/// **Die Höhe wächst je Block, die Epoche nicht.**
///
/// Der Kern von Punkt 7. Bis zum 2026-08-27 war beides dasselbe
/// Feld; ein Block war eine Epoche, und jede Frist „je Epoche"
/// bedeutete in Wahrheit „je Block".
#[test]
fn die_hoehe_waechst_je_block_die_epoche_nicht() {
    let mut k = Kette::probestand();
    for erwartet in 1..=5u64 {
        let b = k.baue_block();
        assert_eq!(b.header.height, erwartet, "die Höhe zählt nicht mit");
        assert_eq!(b.header.epoch, 0, "die Epoche ist mitgewandert");
        assert_eq!(k.hoehe(), erwartet);
    }
}

/// **Und an der Epochengrenze springt sie, genau einmal.**
///
/// Ein Test, der nur die ersten Blöcke ansieht, bestünde auch dann,
/// wenn die Epoche **nie** wechselte. Und eine Epoche, die nie
/// wechselt, ist dieselbe Doppelbelegung mit umgekehrtem Vorzeichen.
#[test]
fn an_der_epochengrenze_springt_die_epoche() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    let mut k = Kette::probestand();
    let mut wechsel = 0usize;
    let mut vorige = 0u64;
    for _ in 0..(BLOECKE_JE_EPOCHE + 2) {
        let b = k.baue_block();
        if b.header.epoch != vorige {
            wechsel += 1;
            assert_eq!(
                b.header.height, BLOECKE_JE_EPOCHE,
                "der Wechsel liegt nicht auf der Grenze"
            );
            vorige = b.header.epoch;
        }
    }
    assert_eq!(wechsel, 1, "die Epoche wechselte {wechsel}-mal statt einmal");
    assert_eq!(vorige, 1);
}

/// **Die Epoche des Ledger-Zustands wandert mit.**
///
/// Sie stand bis zum 2026-08-27 auf 0 und blieb dort: `anwenden`
/// benutzte die Epoche nur zum Rechnen des Verfalls und setzte sie
/// nie. Jede Prüfung, die an der laufenden Epoche hängt (der
/// Verfall von Credits, das Fenster der Verstoßhistorie), lief
/// damit gegen null.
#[test]
fn die_epoche_des_zustands_wandert_mit() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    let mut k = Kette::probestand();
    assert_eq!(k.zustand().epoch.0, 0);
    for _ in 0..BLOECKE_JE_EPOCHE {
        let _ = k.baue_block();
    }
    assert_eq!(
        k.zustand().epoch.0,
        1,
        "der Zustand kennt die Epoche des Blocks nicht"
    );
}

/// Ein Block mit falscher Höhe wird abgelehnt, auch wenn er sonst
/// anschließt.
#[test]
fn eine_falsche_hoehe_wird_abgelehnt() {
    let mut erzeuger = Kette::probestand();
    let mut folger = Kette::probestand();
    let mut block = erzeuger.baue_block();
    block.header.height = 7;
    neu_unterschreiben(&mut block);
    assert!(matches!(
        folger.uebernimm(&block),
        Err(KettenFehler::HoeheWeichtAb { erwartet: 1, bekommen: 7 })
    ));
    // Und der Zustand blieb unberührt.
    assert_eq!(folger.hoehe(), 0);
}

/// Eine Epoche, die nicht aus der Höhe folgt, wird abgelehnt.
///
/// **Die Gegenprobe steht daneben:** Derselbe Block mit der
/// richtigen Epoche geht durch. Ein Test, der nur ablehnt, bestünde
/// auch dann, wenn gar nichts mehr angenommen würde.
#[test]
fn eine_epoche_die_nicht_aus_der_hoehe_folgt_wird_abgelehnt() {
    let mut erzeuger = Kette::probestand();
    let mut folger = Kette::probestand();
    let block = erzeuger.baue_block();

    let mut gefaelscht = block.clone();
    gefaelscht.header.epoch = 9;
    neu_unterschreiben(&mut gefaelscht);
    assert!(matches!(
        folger.uebernimm(&gefaelscht),
        Err(KettenFehler::EpocheWeichtAb { erwartet: 0, bekommen: 9 })
    ));
    assert_eq!(folger.hoehe(), 0);

    folger.uebernimm(&block).expect("der echte Block muss durchgehen");
    assert_eq!(folger.hoehe(), 1);
}

/// Zwei Ketten aus demselben Genesis sind gleich.
#[test]
fn zwei_frische_ketten_stimmen_ueberein() {
    // Wäre das nicht so, sähe jeder Lauf wie ein
    // Determinismusfehler aus, bevor überhaupt etwas passiert ist.
    let a = Kette::probestand();
    let b = Kette::probestand();
    assert_eq!(a.zustandswurzel(), b.zustandswurzel());
    assert_eq!(a.letzter_hash(), b.letzter_hash());
    assert_eq!(a.hoehe(), 0);
}

/// **Der Kern: Erzeuger und Übernehmer landen beim selben Zustand.**
#[test]
fn erzeuger_und_uebernehmer_kommen_zum_selben_zustand() {
    let mut erzeuger = Kette::probestand();
    let mut folger = Kette::probestand();

    erzeuger.aufnehmen(burn(1, 1_000));
    erzeuger.aufnehmen(burn(2, 2_500));
    let block = erzeuger.baue_block();

    folger.uebernimm(&block).expect("Block muss anschließen");
    assert_eq!(
        erzeuger.zustandswurzel(),
        folger.zustandswurzel(),
        "die Zustandswurzeln weichen ab"
    );
    assert_eq!(erzeuger.hoehe(), folger.hoehe());
    assert_eq!(erzeuger.letzter_hash(), folger.letzter_hash());
}

/// Mehrere Blöcke hintereinander bleiben gleich.
#[test]
fn eine_kette_aus_mehreren_bloecken_bleibt_gleich() {
    let mut erzeuger = Kette::probestand();
    let mut folger = Kette::probestand();
    for i in 1..=10u64 {
        erzeuger.aufnehmen(burn((i % 5) as u8 + 1, i * 300));
        let b = erzeuger.baue_block();
        folger.uebernimm(&b).expect("schließt an");
    }
    assert_eq!(erzeuger.hoehe(), 10);
    assert_eq!(erzeuger.zustandswurzel(), folger.zustandswurzel());
}

/// Ein Block, der nicht anschließt, lässt den Zustand unberührt.
#[test]
fn ein_verirrter_block_hinterlaesst_keine_spur() {
    let mut a = Kette::probestand();
    let mut fremd = Kette::probestand();
    fremd.aufnehmen(burn(9, 5_000));
    let _ = fremd.baue_block();
    fremd.aufnehmen(burn(9, 5_000));
    let zweiter = fremd.baue_block(); // schließt an fremds ersten an

    let vorher = a.zustandswurzel();
    let fehler = a.uebernimm(&zweiter).expect_err("darf nicht anschließen");
    assert!(matches!(fehler, KettenFehler::PasstNichtAn { .. }));
    assert_eq!(a.zustandswurzel(), vorher, "der Zustand wurde angefasst");
    assert_eq!(a.hoehe(), 0);
}

/// **Eine abweichende Zustandswurzel wird erkannt und nicht
/// übernommen.**
///
/// Der Fall, für den es diesen Aufbau gibt. Ein Knoten, der einem
/// abweichenden Block folgte, hätte den Befund verschluckt.
#[test]
fn eine_abweichende_zustandswurzel_wird_zurueckgewiesen() {
    let mut erzeuger = Kette::probestand();
    let mut folger = Kette::probestand();
    erzeuger.aufnehmen(burn(1, 1_000));
    let mut block = erzeuger.baue_block();

    // Der Erzeuger fälscht die Wurzel und unterschreibt.
    block.header.state_root = Hash::sha256(b"etwas anderes");
    neu_unterschreiben(&mut block);

    let vorher = folger.zustandswurzel();
    let fehler = folger.uebernimm(&block).expect_err("muss auffallen");
    assert!(matches!(fehler, KettenFehler::ZustandWeichtAb { .. }));
    assert_eq!(folger.zustandswurzel(), vorher, "Zustand wurde übernommen");
}

/// Gossip verbreitet mehrfach; derselbe Block darf nicht zweimal wirken.
#[test]
fn ein_doppelt_empfangener_block_wirkt_nur_einmal() {
    let mut erzeuger = Kette::probestand();
    let mut folger = Kette::probestand();
    erzeuger.aufnehmen(burn(1, 1_000));
    let block = erzeuger.baue_block();

    folger.uebernimm(&block).expect("erstes Mal");
    let nach_erstem = folger.zustandswurzel();
    assert_eq!(
        folger.uebernimm(&block),
        Err(KettenFehler::SchonBekannt),
        "die Dublette wurde nicht erkannt"
    );
    assert_eq!(folger.zustandswurzel(), nach_erstem);
    assert_eq!(folger.hoehe(), 1);
}

/// Eine Transaktion ohne Deckung bricht den Block nicht ab.
#[test]
fn eine_ungedeckte_transaktion_wird_uebersprungen_nicht_verworfen() {
    // Beide Seiten überspringen gleich, also stimmen die Wurzeln
    // trotzdem. Bräche der Erzeuger ab und der Folger nicht, wäre
    // genau das die Abweichung.
    let mut erzeuger = Kette::probestand();
    let mut folger = Kette::probestand();
    erzeuger.aufnehmen(burn(7, u64::MAX)); // mehr als jedes Konto hält
    let block = erzeuger.baue_block();
    folger.uebernimm(&block).expect("schließt an");
    assert_eq!(erzeuger.zustandswurzel(), folger.zustandswurzel());
}

/// ⚑ **Das Fenster hält, was es verspricht** (Fund 124).
///
/// Bis zum 2026-09-02 wuchs `verlauf` unbegrenzt; ein Knoten, der
/// lange genug lief, hielt die vollständige Kette im
/// Arbeitsspeicher. Der Test baut mehr Blöcke, als das Fenster
/// fasst, und sieht nach.
#[test]
fn der_verlauf_bleibt_im_fenster() {
    let mut k = Kette::probestand();
    let ueber = VERLAUFSFENSTER as u64 + 20;
    for i in 1..=ueber {
        k.aufnehmen(burn((i % 8) as u8, 100 + i));
        let _ = k.baue_block();
    }
    assert_eq!(k.hoehe(), ueber);
    assert_eq!(
        k.verlauf.len(),
        VERLAUFSFENSTER,
        "der Zwischenspeicher hält mehr, als das Fenster erlaubt"
    );
    // Und die Hashmenge wird mitgekürzt, sonst hätte das Fenster
    // nur die Hälfte des Speichers gebunden.
    assert_eq!(
        k.bekannt.len(),
        VERLAUFSFENSTER,
        "die Hashmenge wächst weiter, obwohl das Fenster kürzt"
    );
    // Die jüngsten sind da, die ältesten nicht.
    assert!(k.verlauf.contains_key(&ueber));
    assert!(!k.verlauf.contains_key(&1));
}

/// ⚑ **Was aus dem Fenster fällt, holt die Platte zurück**
/// (Fund 124).
///
/// Das ist die Bedingung, unter der das Fenster überhaupt zulässig
/// ist: Ohne den Rückgriff wäre ein Nachzügler, der weiter
/// zurückliegt als 256 Blöcke, von diesem Knoten nicht mehr zu
/// bedienen.
#[test]
fn was_aus_dem_fenster_faellt_kommt_von_der_platte() {
    let d = std::env::temp_dir().join(format!(
        "myelith-fenster-{}-{}",
        std::process::id(),
        crate::protokoll::jetzt_ms()
    ));
    std::fs::create_dir_all(&d).expect("Verzeichnis");
    let p = d.join("kette.log");

    let mut k = Kette::probestand();
    let (speicher, _) =
        crate::speicher::Kettenspeicher::oeffnen(&p, Kette::startwert()).expect("öffnen");
    k.speicher_setzen(speicher);

    let ueber = VERLAUFSFENSTER as u64 + 20;
    for i in 1..=ueber {
        k.aufnehmen(burn((i % 8) as u8, 100 + i));
        let _ = k.baue_block();
    }
    assert_eq!(k.schreibfehler(), 0, "die Datei nahm nicht alles an");

    // Höhe 1 liegt weit unter dem Fenster.
    assert!(!k.verlauf.contains_key(&1));
    let geliefert = k.bloecke_von_bis(1, 3);
    assert_eq!(geliefert.len(), 3, "die Platte lieferte nicht nach");
    assert_eq!(geliefert[0].header.height, 1);
    assert_eq!(geliefert[2].header.height, 3);
    assert_eq!(k.lesefehler(), 0);

    // Und eine Anfrage über die Fenstergrenze hinweg liefert
    // beides, in einem Stück und aufsteigend.
    let grenze = ueber - VERLAUFSFENSTER as u64;
    let ueberlappend = k.bloecke_von_bis(grenze - 2, grenze + 2);
    let hoehen: Vec<u64> = ueberlappend.iter().map(|b| b.header.height).collect();
    assert_eq!(
        hoehen,
        vec![grenze - 2, grenze - 1, grenze, grenze + 1, grenze + 2],
        "an der Fenstergrenze fehlt oder doppelt etwas"
    );

    std::fs::remove_dir_all(&d).ok();
}

/// ⚑ **Zurück über eine Epochengrenze** (2026-11-04): Was die Kette dort
/// neben dem Zustand ableitet (offene Zuteilungen), steht nach dem
/// Zurückrollen wie vorher, und dieselben Blöcke noch einmal ergeben
/// dieselbe Wurzel. Nur so viele Rückwege wie erlaubt.
#[test]
fn zurueck_ueber_eine_epochengrenze() {
    let mut k = Kette::probestand();
    let mut folger = Kette::probestand();
    bauen_und_folgen(&mut k, &mut folger, (BLOECKE_JE_EPOCHE - 3) as usize);
    let zuteilungen = folger.offene_zuteilungen.len();
    bauen_und_folgen(&mut k, &mut folger, 8);
    assert_eq!(folger.offene_zuteilungen.len(), zuteilungen + 1, "die Grenze hob eine Zuteilung auf");
    let wurzel = folger.wurzel();
    assert_eq!(folger.rueckwege.len(), RUECKWEG_HOECHSTENS);
    assert_eq!(folger.zurueckrollen_bis(BLOECKE_JE_EPOCHE - 3), Ok(8));
    assert_eq!(folger.offene_zuteilungen.len(), zuteilungen);
    assert_eq!(folger.zustand().epoch.0, epoche_fuer_hoehe(BLOECKE_JE_EPOCHE - 3));
    for h in BLOECKE_JE_EPOCHE - 2..=BLOECKE_JE_EPOCHE + 5 {
        folger.uebernimm(k.block_an(h).expect("im Fenster")).expect("derselbe Block noch einmal");
    }
    assert_eq!(folger.wurzel(), wurzel);
    assert_eq!(folger.offene_zuteilungen.len(), zuteilungen + 1);
}

/// ⚑ **Nach dem Zurückrollen spielt ein Neustart den neuen Zweig nach**
/// (2026-11-04): Die Kettendatei ist hinter dem Ziel gekürzt, der neue
/// Zweig angehängt, und eine frische Kette, die die Datei nachspielt,
/// steht bei derselben Höhe und Wurzel.
#[test]
fn nach_dem_zurueckrollen_spielt_ein_neustart_den_neuen_zweig() {
    let d = std::env::temp_dir().join(format!("myelith-zweig-{}-{}", std::process::id(), crate::protokoll::jetzt_ms()));
    std::fs::create_dir_all(&d).expect("Verzeichnis");
    let p = d.join("kette.log");
    let mut k = Kette::probestand();
    let (speicher, _) = crate::speicher::Kettenspeicher::oeffnen(&p, Kette::startwert()).expect("öffnen");
    k.speicher_setzen(speicher);
    for i in 1..=20u64 {
        k.aufnehmen(burn((i % 8) as u8, 100 + i));
        let _ = k.baue_block();
    }
    assert_eq!(k.zurueckrollen_bis(15), Ok(5));
    for i in 16..=18u64 {
        k.aufnehmen(burn((i % 8) as u8, 7_000 + i));
        let _ = k.baue_block();
    }
    assert_eq!(k.schreibfehler(), 0);
    let (mut wieder, w) = crate::speicher::Kettenspeicher::oeffnen(&p, Kette::startwert()).expect("wieder öffnen");
    assert_eq!(w.anzahl, 18, "die Sätze des verlassenen Zweigs sind weg");
    let mut neu = Kette::probestand();
    for b in wieder.alle_saetze().expect("lesen") {
        neu.uebernimm(&b).expect("der neue Zweig schliesst an");
    }
    assert_eq!((neu.hoehe(), neu.letzter_hash()), (k.hoehe(), k.letzter_hash()));
    assert_eq!(neu.wurzel(), k.wurzel());
    std::fs::remove_dir_all(&d).ok();
}

/// ⚑ **Eine Gabel tiefer als die Rückwege** (CONSENSUS 7.7a,
/// 2026-11-05): Die Kette baut den Stand am Gabelpunkt aus ihrem Abzug
/// an der Epochengrenze neu auf und folgt dem längeren Zweig; die
/// Kettendatei trägt danach genau ihn. Nach einem Neustart (keine
/// Rückwege) ebenso. Ein Zweig, dessen Erzeuger nicht trägt, löst keinen
/// Neuaufbau aus.
#[test]
fn eine_gabel_tiefer_als_die_rueckwege() {
    let d = std::env::temp_dir().join(format!("myelith-tief-{}-{}", std::process::id(), crate::protokoll::jetzt_ms()));
    std::fs::create_dir_all(&d).expect("Verzeichnis");
    let mit_datei = |name: &str| {
        let p = d.join(name);
        let mut k = Kette::probestand();
        let (speicher, _) = crate::speicher::Kettenspeicher::oeffnen(&p, Kette::startwert()).expect("öffnen");
        k.speicher_setzen(speicher);
        k.ablage_setzen(crate::abzug::Ablage::neben(&p, Kette::startwert()));
        (k, p)
    };
    let (mut a, p) = mit_datei("a.log");
    let (mut b, _) = mit_datei("b.log");
    let gabel = BLOECKE_JE_EPOCHE + 50;
    bauen_und_folgen(&mut a, &mut b, gabel as usize);
    assert!(a.ablage().is_some_and(|x| x.hoehen().contains(&BLOECKE_JE_EPOCHE)), "der Abzug an der Grenze");
    let tiefe = RUECKWEG_HOECHSTENS as u64 + 50;
    for _ in 0..tiefe {
        a.baue_block();
    }
    assert!(b.aufnehmen(burn(3, 3_000)));
    // 📌 Mit dem Zertifikat eines Prüfpunkts über der Gabel, wie im
    //    Nachtlauf vom 2026-11-06: Es zeigt auf einen Block des fremden
    //    Zweigs, nicht auf den eigenen an derselben Höhe.
    let pp = myl_consensus::pruefpunkt::BLOECKE_JE_PRUEFPUNKT;
    let ueber = (gabel / pp + 1) * pp;
    for _ in 0..tiefe + 50 {
        if b.hoehe() == ueber + 1 {
            let z = pruefpunkt_von(&b, ueber, PROBEERZEUGER);
            b.pruefpunkt_anbieten(z);
        }
        b.baue_block();
    }
    assert!(b.gerechtfertigt_bis() >= ueber, "der fremde Zweig trägt sein Zertifikat");
    assert!(a.rueckweg_boden() > gabel, "tiefer als die Rückwege");
    assert_eq!(a.zweig_boden(), 0);
    // Der längere Zweig, in Stücken, wie die Zweigsuche ihn holt.
    let mut fremd = Vec::new();
    let mut ab = gabel + 1;
    while ab <= b.hoehe() {
        let stueck = b.bloecke_von_bis(ab, b.hoehe());
        assert!(!stueck.is_empty());
        ab += stueck.len() as u64;
        fremd.extend(stueck);
    }
    assert!(fremd.iter().any(|x| x.header.pruefpunkt.as_ref().is_some_and(|z| z.hoehe == ueber)));
    let ihrer = fremd.iter().find(|x| x.header.height == ueber).map(|x| x.hash());
    assert!(ihrer.is_some() && a.eigener_hash(ueber).is_some());
    assert_ne!(a.eigener_hash(ueber), ihrer, "der eigene Block an der Höhe des Prüfpunkts ist ein anderer");
    // Die Vorprüfung selbst: Der zertifizierte Block ist der des Zweigs;
    // ⛔️ fehlt er im Zweig, oder läge er unter der Gabel (dann zählt der
    //    eigene, ein anderer), trägt das Zertifikat nicht.
    let mit = fremd.iter().find(|x| x.header.pruefpunkt.as_ref().is_some_and(|z| z.hoehe == ueber)).expect("mit Zertifikat").clone();
    assert!(a.pruefpunkt_traegt_im_zweig(&mit, gabel, &fremd));
    let ohne: Vec<Block> = fremd.iter().filter(|x| x.header.height != ueber).cloned().collect();
    assert!(!a.pruefpunkt_traegt_im_zweig(&mit, gabel, &ohne), "der zertifizierte Block fehlt im Zweig");
    assert!(!a.pruefpunkt_traegt_im_zweig(&mit, ueber, &fremd), "unter der Gabel zählt der eigene Block");
    assert!(!a.pruefpunkt_traegt(&mit), "die volle Prüfung gegen die eigene Kette trägt hier nicht");
    // ⛔️ Ein Zweig, dessen Erzeuger nicht trägt: kein Neuaufbau.
    let mut falsch = fremd.clone();
    falsch[5].unterschrift = None;
    let vorher = (a.hoehe(), a.letzter_hash());
    let ergebnis = a.zweig_folgen(&falsch);
    assert!(matches!(ergebnis, Err(Zweigfehler::TraegtNicht(_, KettenFehler::ErzeugerUnbelegt))), "{ergebnis:?}");
    assert_eq!((a.hoehe(), a.letzter_hash()), vorher);
    assert_eq!(a.zweig_folgen(&fremd), Ok(Zweigwahl::Gewechselt { gabel, zurueck: tiefe as usize, neu: (tiefe + 50) as usize }));
    assert_eq!((a.hoehe(), a.letzter_hash(), a.wurzel()), (b.hoehe(), b.letzter_hash(), b.wurzel()));
    assert_eq!(a.zustandswurzel(), b.wurzel(), "auch neu gerechnet");
    assert_eq!(a.schreibfehler(), 0);
    let (mut wieder, _) = crate::speicher::Kettenspeicher::oeffnen(&p, Kette::startwert()).expect("wieder öffnen");
    let mut neu = Kette::probestand();
    for x in wieder.alle_saetze().expect("lesen") {
        neu.uebernimm(&x).expect("die Datei trägt den neuen Zweig");
    }
    assert_eq!(neu.letzter_hash(), b.letzter_hash());

    // Nach einem Neustart: keine Rückwege, und doch derselbe Weg.
    let (mut c, _) = mit_datei("c.log");
    let mut e = Kette::probestand();
    bauen_und_folgen(&mut c, &mut e, gabel as usize);
    for _ in 0..20 {
        c.baue_block();
    }
    c.rueckwege.clear();
    assert!(e.aufnehmen(burn(4, 4_000)));
    for _ in 0..30 {
        e.baue_block();
    }
    assert_eq!(c.zweig_folgen(&e.bloecke_von_bis(gabel + 1, gabel + 30)), Ok(Zweigwahl::Gewechselt { gabel, zurueck: 20, neu: 30 }));
    assert_eq!(c.wurzel(), e.wurzel());
    std::fs::remove_dir_all(&d).ok();
}

/// ⛔️ **Ein gesetzter Anker laesst keinen Weg ab Genesis offen** (A43):
/// Fehlt sein Beleg noch, nimmt der Knoten nichts davor an, und er fragt
/// erst ab der Epoche des Ankers. Gegenprobe: ohne die Pruefung in
/// `folge` kommen die Belege davor durch.
#[test]
fn ein_gesetzter_anker_laesst_keinen_weg_ab_genesis_offen() {
    use crate::wechsel::probe::{beleg, satz_mit};
    use crate::wechsel::Folgefehler;
    let w = Kette::satzregeln(Kette::probestand().zustand()).mindestgewicht.max(1);
    let (a, b) = (satz_mit(1, w), satz_mit(11, w));
    let aussen = crate::wechsel::als_stimmsatz(&satz_mit(100, w));
    let davor = [beleg(1, &[100, 101, 102], None, Some(a.clone())), beleg(2, &[100, 101, 102], Some(a.clone()), Some(a.clone()))];
    let anker = beleg(9, &[11, 12, 13], Some(b.clone()), Some(b.clone()));
    let mut k = Kette::probestand();
    k.stimmsatz_setzen(aussen);
    k.anker_von_aussen_setzen(anker.zertifikat.hoehe, anker.zertifikat.block_hash, anker.zertifikat.bindung);
    assert_eq!(k.wechselbelege_bis(), 9, "gefragt wird ab dem Anker");
    assert_eq!(k.wechselbelege_aufnehmen(&davor), Err(Folgefehler::Unbekannt(9)));
    assert_eq!(k.wechselbelege_bis(), 9, "nichts davon liegt");
    assert!(k.anker_aus_zertifikat(&davor[1].abschluss).is_err(), "kein Anker aus einer Folge ab Genesis");
    let alle: Vec<_> = davor.iter().cloned().chain([anker]).collect();
    assert_eq!(k.wechselbelege_aufnehmen(&alle), Ok(1), "der Anker, was davor lag, zaehlt nicht");
}

/// ⚑ **Alte Belege fallen, die Folge beginnt beim aeltesten eigenen**
/// (A44), auch nach einem Neustart; der Beleg des Ankers bleibt.
/// Gegenprobe: ohne `belege_ab` beginnt die Folge bei Genesis und traegt
/// keinen jungen Beleg mehr.
#[test]
fn alte_belege_fallen_und_die_folge_beginnt_beim_aeltesten() {
    use crate::wechsel::probe::{beleg, satz_mit};
    let w = Kette::satzregeln(Kette::probestand().zustand()).mindestgewicht.max(1);
    let a = satz_mit(1, w);
    let aussen = crate::wechsel::als_stimmsatz(&satz_mit(100, w));
    let d = std::env::temp_dir().join(format!("myelith-belege-{}-{}", std::process::id(), crate::protokoll::jetzt_ms()));
    let ablage = crate::abzug::Ablage::neben(&d.join("k.log"), Kette::startwert());
    std::fs::create_dir_all(ablage.ordner()).expect("Verzeichnis");
    let datei = crate::wechsel::Belegablage::in_ordner(ablage.ordner());
    let belege: Vec<_> = (3..=240).map(|e| beleg(e, &[1, 2, 3], Some(a.clone()), Some(a.clone()))).collect();
    let mut k = Kette::probestand();
    k.stimmsatz_setzen(aussen.clone());
    k.belegablage = Some(datei.clone());
    for b in &belege {
        datei.anhaengen(b).expect("anhaengen");
        k.wechselbelege.insert(b.epoche(), b.clone());
    }
    let anker = &belege[0];
    k.anker_von_aussen_setzen(anker.zertifikat.hoehe, anker.zertifikat.block_hash, anker.zertifikat.bindung);
    k.belege_kuerzen(240);
    let grenze = 240 - crate::wechsel::WECHSELBELEGE_BEHALTEN;
    let erwartet = 1 + (240 - grenze + 1) as usize;
    assert_eq!(k.wechselbelege.len(), erwartet, "der Anker und die letzten");
    assert_eq!(k.belege_ab, Some(grenze));
    assert_eq!(datei.lesen().len(), erwartet, "die Ablage ist neu geschrieben");
    assert!(k.wechselbelege.contains_key(&anker.epoche()), "der Beleg des Ankers bleibt");
    let letzter = belege.last().expect("Beleg").clone();
    // Neustart ohne Anker: Die Folge beginnt beim aeltesten eigenen.
    let mut neu = Kette::probestand();
    neu.stimmsatz_setzen(aussen);
    neu.ablage_setzen(ablage);
    assert_eq!(neu.belege_ab, Some(grenze), "gekuerzt schon beim Laden");
    assert!(neu.anker_aus_zertifikat(&letzter.abschluss).is_ok(), "die Folge traegt ohne die alten Belege");
    std::fs::remove_dir_all(&d).ok();
}

/// ⚑ **Der Anker von aussen** (CONSENSUS 7.4a, 2026-11-07): Nach einer
/// Ruhe mit Ausschluss traegt der Beleg danach weder gewoehnlich noch
/// ueber die Bruecke (ein neuer Satz, den der alte nicht mit zwei
/// Dritteln traegt). Ein zustandsloser Knoten nimmt ihn dann nur, wenn
/// er der Anker von aussen ist; danach geht es gewoehnlich weiter, auch
/// nach einem Neustart. ⛔️ Gegenprobe: ohne Anker, und mit einem Anker,
/// der auf einen anderen Block zeigt, nicht.
#[test]
fn ein_anker_von_aussen_traegt_ueber_die_ruhe() {
    use crate::wechsel::probe::{beleg, satz_mit};
    let d = std::env::temp_dir().join(format!("myelith-anker-{}-{}", std::process::id(), crate::protokoll::jetzt_ms()));
    std::fs::create_dir_all(&d).expect("Verzeichnis");
    let p = d.join("k.log");
    let w = Kette::satzregeln(Kette::probestand().zustand()).mindestgewicht.max(1);
    let (a, b) = (satz_mit(1, w), satz_mit(11, w));
    let aussen = crate::wechsel::als_stimmsatz(&satz_mit(100, w));
    let vorher = [
        beleg(1, &[100, 101, 102], None, Some(a.clone())),
        beleg(2, &[100, 101, 102], Some(a.clone()), Some(a.clone())),
        beleg(3, &[1, 2, 3], Some(a.clone()), Some(a.clone())),
    ];
    let nach_der_ruhe = beleg(9, &[11, 12, 13], Some(b.clone()), Some(b.clone()));
    let danach = beleg(10, &[11, 12, 14], Some(b.clone()), Some(b.clone()));
    let alle: Vec<_> = vorher.iter().cloned().chain([nach_der_ruhe.clone(), danach.clone()]).collect();
    // ⛔️ Ohne Anker: bis zur Ruhe, dann nicht weiter.
    let mut ohne = Kette::probestand();
    ohne.stimmsatz_setzen(aussen.clone());
    assert_eq!(ohne.wechselbelege_aufnehmen(&alle), Err(crate::wechsel::Folgefehler::Unbekannt(9)));
    assert_eq!(ohne.wechselbelege_bis(), 4);
    // ⛔️ Ein Beleg mit dem Block des Ankers, aber eigener Bindung und
    //   eigenen Saetzen (die Unterschriften prueft beim Anker niemand),
    //   traegt nicht: Der Anker nennt auch die Bindung.
    let mut gefaelscht = nach_der_ruhe.clone();
    gefaelscht.naechster = Some(satz_mit(50, w));
    gefaelscht.uebernaechster = Some(satz_mit(50, w));
    gefaelscht.zertifikat.bindung = myl_ledger::stimmsatz::bindung_aus(gefaelscht.naechster.as_ref(), gefaelscht.uebernaechster.as_ref());
    let mut betrogen = Kette::probestand();
    betrogen.stimmsatz_setzen(aussen.clone());
    betrogen.anker_von_aussen_setzen(nach_der_ruhe.zertifikat.hoehe, nach_der_ruhe.zertifikat.block_hash, nach_der_ruhe.zertifikat.bindung);
    let mit_faelschung: Vec<_> = vorher.iter().cloned().chain([gefaelscht]).collect();
    assert_eq!(betrogen.wechselbelege_aufnehmen(&mit_faelschung), Err(crate::wechsel::Folgefehler::Unbekannt(9)));
    assert_eq!(betrogen.wechselbelege_bis(), 9, "die Fälschung liegt nicht, gefragt wird weiter ab dem Anker");
    // ⛔️ Ein Anker auf einen anderen Block hilft nicht.
    let mut fremd = Kette::probestand();
    fremd.stimmsatz_setzen(aussen.clone());
    fremd.anker_von_aussen_setzen(nach_der_ruhe.zertifikat.hoehe, Hash::sha256(b"anderer Block"), nach_der_ruhe.zertifikat.bindung);
    assert_eq!(fremd.wechselbelege_aufnehmen(&alle), Err(crate::wechsel::Folgefehler::Unbekannt(9)));
    // Mit dem Anker: der Beleg nach der Ruhe und der danach.
    let mut k = Kette::probestand();
    k.ablage_setzen(crate::abzug::Ablage::neben(&p, Kette::startwert()));
    k.stimmsatz_setzen(aussen.clone());
    k.anker_von_aussen_setzen(nach_der_ruhe.zertifikat.hoehe, nach_der_ruhe.zertifikat.block_hash, nach_der_ruhe.zertifikat.bindung);
    assert_eq!(k.wechselbelege_aufnehmen(&alle), Ok(2), "der Anker und der Beleg danach");
    assert_eq!(k.wechselbelege_bis(), 11);
    assert_eq!(k.juengster_wechselbeleg(), Some((10, danach.zertifikat.hoehe, danach.zertifikat.block_hash, danach.zertifikat.bindung)));
    let spaeter = beleg(12, &[11, 12, 13], None, None);
    assert!(k.anker_aus_zertifikat(&spaeter.abschluss).is_ok(), "ein Anker aus einem Zertifikat danach");
    assert_eq!(
        k.anker_aus_zertifikat(&nach_der_ruhe.abschluss).map(|a| a.hoehe),
        Ok(nach_der_ruhe.zertifikat.hoehe),
        "gleich nach der Ruhe: der Abschluss des Ankers selbst"
    );
    // Nach einem Neustart beginnt die Folge wieder beim Anker.
    let mut neu = Kette::probestand();
    neu.ablage_setzen(crate::abzug::Ablage::neben(&p, Kette::startwert()));
    neu.stimmsatz_setzen(aussen);
    neu.anker_von_aussen_setzen(nach_der_ruhe.zertifikat.hoehe, nach_der_ruhe.zertifikat.block_hash, nach_der_ruhe.zertifikat.bindung);
    assert!(neu.anker_aus_zertifikat(&spaeter.abschluss).is_ok());
    assert_eq!(neu.wechselbelege_aufnehmen(&[danach]), Ok(0), "schon bekannt");
    std::fs::remove_dir_all(&d).ok();
}

/// ⚑ **Nach einer Lücke** (2026-11-07): Ein gemerkter Beleg, der die
/// eigene Folge nicht trägt (ein Unterzeichner außerhalb des Satzes vor
/// der Lücke), wird durch einen späteren derselben Epoche ersetzt, der
/// sie trägt, auch nach einem Neustart; ein tragender nie.
#[test]
fn nach_einer_luecke_ersetzt_ein_tragender_beleg() {
    use crate::wechsel::probe::{beleg, satz_mit};
    let d = std::env::temp_dir().join(format!("myelith-luecke-{}-{}", std::process::id(), crate::protokoll::jetzt_ms()));
    std::fs::create_dir_all(&d).expect("Verzeichnis");
    let p = d.join("k.log");
    let mut k = Kette::probestand();
    k.ablage_setzen(crate::abzug::Ablage::neben(&p, Kette::startwert()));
    let w = Kette::satzregeln(k.zustand()).mindestgewicht.max(1);
    let a = satz_mit(1, w);
    k.stimmsatz_setzen(crate::wechsel::als_stimmsatz(&satz_mit(100, w)));
    for b in [
        beleg(1, &[100, 101, 102], None, Some(a.clone())),
        beleg(2, &[100, 101, 102], Some(a.clone()), Some(a.clone())),
        beleg(3, &[1, 2, 3], Some(a.clone()), Some(a.clone())),
    ] {
        k.wechselbelege.insert(b.epoche(), b);
    }
    // Epoche 9 nach der Lücke: zuerst eines mit einem Fremden.
    let fremd = beleg(9, &[1, 2, 3, 11], None, None);
    k.beleg_merken(fremd.zertifikat.clone(), fremd.abschluss.clone());
    assert_eq!(k.wechselbelege.get(&9), Some(&fremd), "der erste wird gemerkt");
    let gut = beleg(9, &[1, 2, 3], None, None);
    k.beleg_merken(gut.zertifikat.clone(), gut.abschluss.clone());
    assert_eq!(k.wechselbelege.get(&9), Some(&gut), "ersetzt durch einen, der trägt");
    let noch = beleg(9, &[1, 2, 4], None, None);
    k.beleg_merken(noch.zertifikat, noch.abschluss);
    assert_eq!(k.wechselbelege.get(&9), Some(&gut), "einen tragenden ersetzt keiner");
    // Nach einem Neustart gilt der spätere aus der Ablage.
    let mut neu = Kette::probestand();
    neu.ablage_setzen(crate::abzug::Ablage::neben(&p, Kette::startwert()));
    assert_eq!(neu.wechselbelege.get(&9), Some(&gut));
    // ⛔️ Gegenprobe: Ein zustandsloser Knoten nimmt den ersten nicht, den zweiten schon.
    let mut leicht = Kette::probestand();
    leicht.stimmsatz_setzen(crate::wechsel::als_stimmsatz(&satz_mit(100, w)));
    let bis_drei: Vec<_> = k.wechselbelege.values().filter(|b| b.epoche() < 9).cloned().collect();
    assert_eq!(leicht.wechselbelege_aufnehmen(&bis_drei), Ok(3), "bis zur Lücke");
    assert_eq!(leicht.wechselbelege_aufnehmen(&[fremd]), Err(crate::wechsel::Folgefehler::Unbekannt(9)));
    assert_eq!(leicht.wechselbelege_aufnehmen(&[gut]), Ok(1));
    std::fs::remove_dir_all(&d).ok();
}

/// ⚑ **Der Anker für einen vollen Einsteiger** (Z7, 2026-11-05): ein
/// Zertifikat, das eine Epochengrenze mit vollem Abzug endgültig macht;
/// eines, das nur einen gewöhnlichen Prüfpunkt endgültig macht, und eine
/// Kette ohne Ablage liefern keinen.
#[test]
fn der_anker_fuer_einen_vollen_einsteiger() {
    let d = std::env::temp_dir().join(format!("myelith-voll-{}-{}", std::process::id(), crate::protokoll::jetzt_ms()));
    std::fs::create_dir_all(&d).expect("Verzeichnis");
    let p = d.join("k.log");
    let mut k = Kette::probestand();
    let (speicher, _) = crate::speicher::Kettenspeicher::oeffnen(&p, Kette::startwert()).expect("öffnen");
    k.speicher_setzen(speicher);
    k.ablage_setzen(crate::abzug::Ablage::neben(&p, Kette::startwert()));
    let grenze = BLOECKE_JE_EPOCHE;
    let pp = myl_consensus::pruefpunkt::BLOECKE_JE_PRUEFPUNKT;
    for _ in 0..grenze - pp {
        k.baue_block();
    }
    // Erst ein gewöhnlicher Prüfpunkt, endgültig durch die Grenze.
    let z1 = pruefpunkt_von(&k, grenze - pp, PROBEERZEUGER);
    k.pruefpunkt_anbieten(z1);
    for _ in 0..pp {
        k.baue_block();
    }
    let z2 = pruefpunkt_von(&k, grenze, PROBEERZEUGER);
    k.pruefpunkt_anbieten(z2);
    for _ in 0..pp {
        k.baue_block();
    }
    assert_eq!(k.bestaetigt_bis(), grenze - pp);
    assert!(k.voller_anker().is_none(), "endgültig ist nur ein gewöhnlicher Prüfpunkt");
    // Dann die Grenze selbst.
    let z3 = pruefpunkt_von(&k, grenze + pp, PROBEERZEUGER);
    k.pruefpunkt_anbieten(z3);
    for _ in 0..5 {
        k.baue_block();
    }
    assert_eq!(k.bestaetigt_bis(), grenze);
    let anker = k.voller_anker().expect("ein Anker an der Grenze");
    assert_eq!((anker.hoehe, anker.quelle.hoehe), (grenze + pp, grenze));
    assert!(Kette::probestand().voller_anker().is_none(), "ohne Ablage keiner");
    std::fs::remove_dir_all(&d).ok();
}

/// ⚑ **Ohne Kettendatei bleibt nur das Fenster**, und das ist die
/// bewusst in Kauf genommene Folge von `--ohne-kette`.
///
/// Der Test hält sie fest, damit sie niemand für einen Fehler hält.
#[test]
fn ohne_kettendatei_liefert_nur_das_fenster() {
    let mut k = Kette::probestand();
    let ueber = VERLAUFSFENSTER as u64 + 20;
    for i in 1..=ueber {
        k.aufnehmen(burn((i % 8) as u8, 100 + i));
        let _ = k.baue_block();
    }
    assert!(
        k.bloecke_von_bis(1, 3).is_empty(),
        "ohne Datei kann nichts von unterhalb des Fensters kommen"
    );
    assert_eq!(k.lesefehler(), 0, "nicht vorhanden ist kein Lesefehler");
}

/// ⚑ **Eine Nachforderung von außen kann keinen Verstärker bauen**
/// (Fund 141).
///
/// `ab` und `bis` kommen über die Leitung. Ohne Deckel packte
/// `Bloecke { ab: 0, bis: u64::MAX }` alles, was der Knoten hat, in
/// **eine** Antwort: ein paar Bytes hinein, Megabyte hinaus.
#[test]
fn eine_masslose_nachforderung_wird_gedeckelt() {
    // ⚑ Mit Kettendatei: Seit der Deckel (512) über dem Fenster (256)
    //   liegt, haelt nur die Platte genug Bloecke, um ihn zu pruefen.
    let d = std::env::temp_dir().join(format!("myelith-deckel-{}-{}", std::process::id(), crate::protokoll::jetzt_ms()));
    std::fs::create_dir_all(&d).expect("Verzeichnis");
    let mut k = Kette::probestand();
    let (speicher, _) = crate::speicher::Kettenspeicher::oeffnen(&d.join("kette.log"), Kette::startwert()).expect("öffnen");
    k.speicher_setzen(speicher);
    let ueber = crate::nachschub::MAX_BLOECKE_JE_LIEFERUNG + 20;
    for i in 1..=ueber {
        k.aufnehmen(burn((i % 8) as u8, 100 + i));
        let _ = k.baue_block();
    }
    let geliefert = k.bloecke_von_bis(0, u64::MAX);
    assert!(
        geliefert.len() as u64 <= crate::nachschub::MAX_BLOECKE_JE_LIEFERUNG,
        "die Antwort trägt {} Blöcke, erlaubt sind {}",
        geliefert.len(),
        crate::nachschub::MAX_BLOECKE_JE_LIEFERUNG
    );
    // Gegenprobe zum Deckel: Die Kette hält mehr, als er erlaubt, und
    // er wird ausgeschöpft (ab Höhe 1; die Höhe 0 ist der Startwert).
    let voll = k.bloecke_von_bis(1, u64::MAX);
    assert_eq!(voll.len() as u64, crate::nachschub::MAX_BLOECKE_JE_LIEFERUNG, "der Deckel greift nicht oder es fehlt ein Block");
    let _ = std::fs::remove_dir_all(&d);
}


/// **Ein Neuling holt über den Verlauf auf.**
///
/// Der Fall, für den es [`crate::nachschub`] gibt: Beta verpasst
/// die ersten Blöcke, bekommt sie nachgeliefert und steht danach
/// beim selben Zustand wie Alpha.
#[test]
fn ein_nachzuegler_holt_ueber_den_verlauf_auf() {
    let mut erzeuger = Kette::probestand();
    for i in 1..=5u64 {
        erzeuger.aufnehmen(burn((i % 4) as u8, i * 400));
        let _ = erzeuger.baue_block();
    }
    // Der Neuling war die ganze Zeit nicht da.
    let mut neuling = Kette::probestand();
    assert_eq!(neuling.hoehe(), 0);

    // Er bekommt den Verlauf und wendet ihn ganz normal an.
    for b in erzeuger.bloecke_von_bis(1, 5) {
        neuling.uebernimm(&b).expect("nachgelieferter Block schließt an");
    }
    assert_eq!(neuling.hoehe(), 5);
    assert_eq!(
        neuling.zustandswurzel(),
        erzeuger.zustandswurzel(),
        "der Nachzügler steht bei einem anderen Zustand"
    );
}

/// Nachlieferung ist ein Transportweg, **kein Vertrauensweg**.
#[test]
fn ein_nachgelieferter_block_mit_falscher_wurzel_wird_genauso_abgewiesen() {
    // Wäre die Nachlieferung ein zweiter, schwächerer Weg in die
    // Kette, wäre sie das Loch: Wer einen Knoten zum Nachfordern
    // bringt, bekäme einen Block hineingelegt, ohne dass er
    // nachrechnet.
    let mut erzeuger = Kette::probestand();
    erzeuger.aufnehmen(burn(1, 900));
    let _ = erzeuger.baue_block();
    let mut geliefert = erzeuger.bloecke_von_bis(1, 1);
    geliefert[0].header.state_root = Hash::sha256(b"untergeschoben");
    neu_unterschreiben(&mut geliefert[0]);

    let mut neuling = Kette::probestand();
    let fehler = neuling.uebernimm(&geliefert[0]).expect_err("muss auffallen");
    assert!(matches!(fehler, KettenFehler::ZustandWeichtAb { .. }));
    assert_eq!(neuling.hoehe(), 0);
}

#[test]
fn der_verlauf_liefert_nur_was_da_ist() {
    // Lücken werden übersprungen, nicht aufgefüllt. Der Fragende
    // merkt es daran, dass sein Rückstand nicht ganz verschwindet.
    let mut k = Kette::probestand();
    for _ in 0..3 {
        let _ = k.baue_block();
    }
    assert_eq!(k.bloecke_von_bis(1, 3).len(), 3);
    assert_eq!(k.bloecke_von_bis(1, 99).len(), 3, "nicht Vorhandenes wird nicht erfunden");
    assert!(k.bloecke_von_bis(7, 9).is_empty());
}

/// **Der Mempool eines Nicht-Erzeugers wächst nicht ins Unendliche.**
///
/// Der Fund aus dem Dreiknoten-Probelauf: Beta nahm jede
/// Transaktion aus dem Gossip auf und leerte nie, weil es selbst
/// keine Blöcke baut.
#[test]
fn was_in_einem_block_ankommt_wartet_nicht_mehr() {
    let mut erzeuger = Kette::probestand();
    let mut folger = Kette::probestand();

    // Beide sehen dieselben Transaktionen aus dem Gossip.
    for i in 1..=3u64 {
        let t = burn((i % 4) as u8, i * 500);
        erzeuger.aufnehmen(t.clone());
        folger.aufnehmen(t);
    }
    assert_eq!(folger.wartend(), 3);

    let block = erzeuger.baue_block();
    folger.uebernimm(&block).expect("schließt an");
    assert_eq!(
        folger.wartend(),
        0,
        "der Mempool wurde nicht geleert: er wüchse für immer"
    );
}

#[test]
fn was_nicht_im_block_stand_wartet_weiter() {
    // Sonst verschwänden Transaktionen, die noch niemand
    // verarbeitet hat.
    let mut erzeuger = Kette::probestand();
    let mut folger = Kette::probestand();
    let drin = burn(1, 700);
    erzeuger.aufnehmen(drin.clone());
    folger.aufnehmen(drin);
    folger.aufnehmen(burn(2, 800)); // kennt der Erzeuger nicht

    let block = erzeuger.baue_block();
    folger.uebernimm(&block).expect("schließt an");
    assert_eq!(folger.wartend(), 1, "die unverarbeitete Transaktion ist weg");
}

#[test]
fn eine_doppelt_eingereichte_transaktion_faellt_mit_dem_block_ganz_heraus() {
    // 📌 Bis zum 2026-10-24 hiess dieser Test „verschwindet nur einmal":
    // Dieselbe Transaktion zweimal eingereicht sei zweimal eingereicht,
    // und die andere warte weiter. Sie trägt aber dieselbe Nummer, und
    // die ist mit dem Block verbraucht; die wartende hätte nie mehr
    // gegolten (Fund 627).
    let mut erzeuger = Kette::probestand();
    let mut folger = Kette::probestand();
    let t = burn(3, 600);
    erzeuger.aufnehmen(t.clone());
    folger.aufnehmen(t.clone());
    folger.aufnehmen(t);
    assert_eq!(folger.wartend(), 2);

    let block = erzeuger.baue_block();
    assert_eq!(block.txs.len(), 1);
    folger.uebernimm(&block).expect("schließt an");
    assert_eq!(folger.wartend(), 0);
}

/// Ein leerer Block ist gültig und bewegt die Kette weiter.
#[test]
fn ein_leerer_block_bewegt_die_kette() {
    let mut erzeuger = Kette::probestand();
    let mut folger = Kette::probestand();
    let block = erzeuger.baue_block();
    assert!(block.txs.is_empty());
    folger.uebernimm(&block).expect("auch leer schließt an");
    assert_eq!(folger.hoehe(), 1);
    assert_eq!(erzeuger.letzter_hash(), folger.letzter_hash());
}

/// Der Mempool wird beim Bauen geleert.
#[test]
fn der_mempool_wird_beim_bauen_geleert() {
    // Sonst stünde dieselbe Transaktion in jedem folgenden Block.
    let mut k = Kette::probestand();
    k.aufnehmen(burn(1, 500));
    assert_eq!(k.wartend(), 1);
    let b = k.baue_block();
    assert_eq!(b.txs.len(), 1);
    assert_eq!(k.wartend(), 0);
    assert!(k.baue_block().txs.is_empty());
}

/// ⚑ **Fund 627: Eine Transaktion, die ihre Vorgängerin im Gossip
/// überholt, geht nicht verloren.** Sie wartet im Mempool, statt im
/// Block an der Nummer zu scheitern, und kommt mit der Vorgängerin
/// nach. Gegenprobe: Vorher stand n+1 im ersten Block, scheiterte dort
/// und war weg; der zweite Block wandte nur n an, und die Nummer im
/// Zustand blieb bei 1 statt 2.
#[test]
fn eine_ueberholende_transaktion_wartet_auf_ihre_vorgaengerin() {
    let mut k = Kette::probestand();
    k.aufnehmen(burn_nr(3, 100, 1));
    let erster = k.baue_block();
    assert!(erster.txs.is_empty(), "n+1 ohne n gehört nicht in den Block");
    assert_eq!(k.wartend(), 1, "n+1 wartet");

    k.aufnehmen(burn_nr(3, 100, 0));
    let _ = k.baue_block();
    let _ = k.baue_block();
    assert_eq!(k.zustand().account(&probekonto(3)).nonce, 2, "beide sind angewandt");
    assert_eq!(k.wartend(), 0);
}

/// ⚑ **Fund 627: In den Block kommt nur, was seine Nummer verbraucht**,
/// und die Wurzel ist dieselbe wie mit allem. Ein Folger, der den
/// schlanken Block übernimmt, landet beim selben Zustand.
#[test]
fn ein_block_traegt_nur_was_seine_nummer_verbraucht() {
    let mut k = Kette::probestand();
    let mut folger = Kette::probestand();
    let mut gefaelscht = burn(4, 100);
    gefaelscht.signatur = myl_types::bls::BlsSignature([7u8; 96]);
    k.aufnehmen(gefaelscht);
    k.aufnehmen(burn(5, 100));
    k.aufnehmen(burn_nr(6, 100, 5));
    let b = k.baue_block();
    assert_eq!(b.txs.len(), 1, "nur die gültige");
    folger.uebernimm(&b).expect("schließt an und rechnet gleich");
    assert_eq!(folger.zustandswurzel(), k.zustandswurzel());
}

/// ⚑ **Fund 627: Der Mempool eines Folgers bleibt klein.** Eine
/// Transaktion, die nach ihrem Block ankommt, wird gar nicht erst
/// aufgenommen; eine, die vorher ankam, fällt mit dem Block heraus;
/// eine mit erfundener Nummer verfällt. Gegenprobe: Vorher nahm
/// `aufnehmen` alles, und die späte blieb für immer liegen; im
/// Archivlauf vom 2026-10-24 waren es über 5000.
#[test]
fn der_mempool_eines_folgers_bleibt_klein() {
    let mut erzeuger = Kette::probestand();
    let mut folger = Kette::probestand();
    let frueh = burn(1, 100);
    let spaet = burn(2, 100);
    erzeuger.aufnehmen(frueh.clone());
    erzeuger.aufnehmen(spaet.clone());
    folger.aufnehmen(frueh);
    folger.aufnehmen(burn_nr(7, 100, 9));
    let b = erzeuger.baue_block();
    folger.uebernimm(&b).expect("schließt an");
    assert_eq!(folger.wartend(), 1, "die frühe ist mit dem Block weg, die erfundene wartet");
    assert!(!folger.aufnehmen(spaet), "die späte gilt nie mehr");

    for _ in 0..WARTEN_HOECHSTENS_HOEHEN {
        let b = erzeuger.baue_block();
        folger.uebernimm(&b).expect("schließt an");
    }
    assert_eq!(folger.wartend(), 0, "die erfundene ist verfallen");
}

/// ⚑ **Fund 627: Der Mempool hat eine Obergrenze.**
#[test]
fn der_mempool_nimmt_hoechstens_seine_grenze_auf() {
    let mut k = Kette::probestand();
    // Verteilt auf so viele Absender, dass keiner an seine eigene
    // Grenze stösst (Fund 633).
    for i in 0..MEMPOOL_HOECHSTENS as u64 {
        let wer = i / MEMPOOL_JE_ABSENDER as u64;
        let sk = myl_types::bls::BlsSecretKey::key_gen(Hash::sha256(format!("fremd-{wer}").as_bytes()).as_bytes())
            .expect("Schlüssel");
        let tx = Transaktion::signiere(&Kette::startwert(), &sk, i % MEMPOOL_JE_ABSENDER as u64, Anweisung::Burn { betrag: 1 })
            .expect("signieren");
        // Mit Guthaben, sonst greift die Grenze für Neulinge (Fund 635).
        k.zustand_mut().account_mut(&tx.absender_adresse()).balance = 1;
        assert!(k.aufnehmen(tx));
    }
    assert!(!k.aufnehmen(burn(8, 1)), "über der Grenze wird nichts aufgenommen");
    assert_eq!(k.wartend(), MEMPOOL_HOECHSTENS);
}

/// Ein Archivbeleg mit `bytes` Nutzlast, wie ihn jedes Konto einreichen
/// kann: Er verbraucht seine Nummer und scheitert danach am Ledger.
fn grosser_beleg(wer: u8, nonce: u64, bytes: usize) -> Transaktion {
    Transaktion::signiere(
        &Kette::startwert(),
        &probeschluessel(wer),
        nonce,
        Anweisung::ArchivBelegen { bloecke: vec![vec![7u8; bytes]], beweise: vec![] },
    )
    .expect("signieren")
}

/// ⚑ **Ein Block bleibt unter seiner Bytegrenze** (Fund 633): Vierzig
/// Belege zu je 50 KB sind 2 MB, mehr als Gossip an einem Block annimmt.
/// Jeder Block trägt höchstens `BLOCK_TXS_HOECHSTENS_BYTES`, und alle
/// kommen nacheinander hinein.
#[test]
fn ein_block_bleibt_unter_seiner_bytegrenze() {
    let mut k = Kette::probestand();
    for i in 0..40u64 {
        assert!(k.aufnehmen(grosser_beleg(30 + (i % 4) as u8, i / 4, 50_000)));
    }
    let mut im_block = 0usize;
    for _ in 0..10 {
        let b = k.baue_block();
        let txs: usize = b.txs.iter().map(|t| borsh::to_vec(t).unwrap().len()).sum();
        assert!(txs <= BLOCK_TXS_HOECHSTENS_BYTES, "{txs} Bytes in einem Block");
        assert!(borsh::to_vec(&b).unwrap().len() <= myl_net::validation::MAX_BLOCKS_BYTES);
        im_block += b.txs.len();
    }
    assert_eq!(im_block, 40, "alle kommen nacheinander hinein");
    assert_eq!(k.wartend(), 0);
}

/// Eine Transaktion, die allein grösser ist als ein Block, hält die
/// übrigen nicht auf: Sie fällt heraus, die nächste kommt hinein.
#[test]
fn eine_uebergrosse_transaktion_haelt_den_block_nicht_auf() {
    let mut k = Kette::probestand();
    assert!(k.aufnehmen(grosser_beleg(30, 0, BLOCK_TXS_HOECHSTENS_BYTES + 1)));
    assert!(k.aufnehmen(burn(3, 900)));
    let b = k.baue_block();
    assert_eq!(b.txs.len(), 1, "der Burn steht im Block");
    assert_eq!(k.wartend(), 0, "die übergrosse ist gegangen");
}

/// ⚑ **Neulinge kommen nur gedrosselt hinein** (Fund 635): Hundert
/// frische Schlüssel ohne Guthaben bringen höchstens
/// `MEMPOOL_NEULINGE_HOECHSTENS` in den Mempool und höchstens
/// `BLOCK_NEULINGE_HOECHSTENS` in einen Block; ein Konto mit Guthaben
/// kommt weiter hinein. ⚑ Seit den Gebühren (G1) kommt ein Neuling ohne
/// Guthaben in keinen Block: Er kann sie nicht zahlen und wartet, bis er
/// verfällt.
#[test]
fn neulinge_kommen_nur_gedrosselt_hinein() {
    let mut k = Kette::probestand();
    let angenommen = (0..100u32)
        .filter(|i| {
            let sk = myl_types::bls::BlsSecretKey::key_gen(Hash::sha256(format!("neuling-{i}").as_bytes()).as_bytes())
                .expect("Schlüssel");
            let tx = Transaktion::signiere(&Kette::startwert(), &sk, 0, Anweisung::Burn { betrag: 1 }).expect("signieren");
            k.aufnehmen(tx)
        })
        .count();
    assert_eq!(angenommen, MEMPOOL_NEULINGE_HOECHSTENS);
    assert!(k.aufnehmen(burn(3, 900)), "ein Konto mit Guthaben kommt hinein");
    let b = k.baue_block();
    assert_eq!(b.txs.len(), 1, "nur der Burn, kein Neuling zahlt");
    assert_eq!(k.wartend(), MEMPOOL_NEULINGE_HOECHSTENS);
}

/// Ein einzelner Absender füllt den Mempool nicht: höchstens
/// `MEMPOOL_JE_ABSENDER`, und ein anderer kommt weiter hinein.
#[test]
fn ein_absender_fuellt_den_mempool_nicht() {
    let mut k = Kette::probestand();
    let angenommen = (0..200u64).filter(|&i| k.aufnehmen(burn_nr(8, 1, i))).count();
    assert_eq!(angenommen, MEMPOOL_JE_ABSENDER);
    assert!(k.aufnehmen(burn(3, 900)));
}

/// 📌 **Eine niedrigere Nummer geht vor** (`kurz-gabel`, 2026-11-05):
/// Ist das Kontingent eines Absenders voll, verdrängt eine frühere Nummer
/// seine höchste (nach einem Zweigwechsel kommt sie aus dem verlassenen
/// Block zurück); eine spätere, eine schon wartende und eine
/// verbrauchte nicht. Das Kontingent bleibt voll, nie darüber.
#[test]
fn eine_niedrigere_nummer_geht_vor() {
    let mut k = Kette::probestand();
    let n = MEMPOOL_JE_ABSENDER as u64;
    assert_eq!((1..=n).filter(|&i| k.aufnehmen(burn_nr(3, 1, i))).count(), MEMPOOL_JE_ABSENDER);
    assert!(!k.aufnehmen(burn_nr(3, 1, n + 1)), "eine spätere nicht");
    assert!(!k.aufnehmen(burn_nr(3, 1, 5)), "eine schon wartende nicht");
    assert!(k.aufnehmen(burn_nr(3, 1, 0)), "die fehlende frühere");
    assert_eq!(k.wartend(), MEMPOOL_JE_ABSENDER);
    let b = k.baue_block();
    assert_eq!(b.txs.first().map(|t| t.nonce), Some(0), "lückenlos ab null");
    assert!(b.txs.iter().all(|t| t.nonce < n), "die höchste ist gegangen");
}

/// Rohe Gossip-Bytes werden gelesen, Unsinn nicht.
#[test]
fn rohe_transaktionen_werden_gelesen_unsinn_nicht() {
    let mut k = Kette::probestand();
    let gut = borsh::to_vec(&burn(3, 900)).unwrap();
    assert!(k.aufnehmen_roh(&gut));
    assert_eq!(k.wartend(), 1);
    assert!(!k.aufnehmen_roh(&[0xFF; 7]));
    // Anhängsel: dieselbe Transaktion mit anderem Rahmen.
    let mut mit_anhang = gut.clone();
    mit_anhang.push(0);
    assert!(!k.aufnehmen_roh(&mit_anhang));
    assert_eq!(k.wartend(), 1);
}

// --- Punkt 38: der Epochenabschluss in der Kette ---

/// Summe aller Guthaben, um das Wachstum der Geldmenge zu messen.
fn geldmenge(k: &Kette) -> u128 {
    k.zustand().accounts.values().map(|a| a.balance as u128).sum()
}

/// ⚑ **Der Trainingsplan entsteht in der Kette und wird vermerkt.**
///
/// Ohne diesen Test wäre der Eintrag im Epochenwechsel eine Zeile,
/// die niemand erreicht, und `trainingsstand` bliebe leer: Die
/// Reihum-Ordnung hätte nichts zu ordnen, und niemandem fiele es
/// auf, weil die Auswahl dann still auf das blosse Los zurückfiele.
///
/// Zwölf Miner ergeben zwei Pods. Ohne Nachfrage ist die Auslastung
/// null, der Anteil 82 Prozent, und gerundet sind das **zwei**
/// Trainingspods, also **ein Redundanzpaar**.
///
/// ⚑ **Zwölf ist das Minimum eines Netzes, das trainieren kann.**
/// Mit acht Konten gäbe es einen Pod, kein Paar und damit keine
/// prüfbare Arbeit; genau daran fiel dieser Test am 2026-09-05, als
/// die Paarbildung dazukam, und das war richtig so.
#[test]
fn der_trainingsplan_entsteht_und_wird_vermerkt() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    use myl_types::miner::HardwareClass;
    use myl_types::node_metadata::GeoRegion;
    let mut k = Kette::probestand();
    let mut nonce = [0u64; 12];
    for w in 0..12u8 {
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::MinerAnmelden {
                    hardware: HardwareClass::MediumGpu,
                    zone: GeoRegion::Europe,
                    netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
        k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
        nonce[w as usize] += 1;
    }
    k.baue_block();
    assert_eq!(k.zustand().miner.len(), 12, "die Anmeldungen kamen nicht an");

    // Die Anmeldungen greifen erst nach dem Registrierungsschluss.
    for _ in 0..(3 * BLOECKE_JE_EPOCHE) {
        k.baue_block();
    }
    let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    assert_eq!(zuteilung.pods.len(), 2, "zwoelf Miner muessen zwei Pods ergeben");

    let plan = k.trainingsplan();
    assert_eq!(plan.auslastung, 0, "ohne Nachfrage ist die Auslastung null");
    assert_eq!(plan.anteil_bps, 8_200, "der Anteil bei leerem Netz");
    assert_eq!(plan.pods.len(), 2, "82 Prozent von zwei, gerundet");
    assert_eq!(plan.paare.len(), 1, "zwei Trainingspods ergeben ein Paar");
    // ⚑ **Und er bekommt ein Buendel**, sonst waere der Plan eine
    // Zuteilung ohne Aufgabe.
    let nr = *plan.pods.iter().next().expect("ein Pod");
    let buendel = plan.zuweisungen.get(&nr).expect("kein Buendel zugewiesen");
    assert_eq!(buendel.laenge, 256);
    assert_eq!(buendel.start % 256, 0, "das Buendel liegt nicht auf dem Raster");
    assert!(buendel.start + buendel.laenge <= 65_536, "es ragt aus dem Korpus");

    // ⚑ **Der Vermerk entsteht am Epochenwechsel**, also sobald die
    // Anmeldungen greifen und ein Pod entsteht.
    let nach_eins = k.zustand().trainingsstand.clone();
    assert!(!nach_eins.is_empty(), "der Epochenwechsel hat nichts vermerkt");
    assert!(nach_eins.len() >= 6, "ein Trainingspod hat sechs Mitglieder");
    let epoche_jetzt = k.zustand().epoch.0;
    assert!(
        nach_eins.values().all(|e| *e < epoche_jetzt),
        "vermerkt wird die abgelaufene Epoche, nicht die laufende"
    );
    // ⚑ **Zwoelf Miner, zwei Trainingspods zu je sechs Plaetzen.**
    // Die Pods werden
    // jede Epoche neu gemischt, also kommen ueber mehrere Epochen
    // alle acht an die Reihe. Genau deshalb haengt das Gedaechtnis
    // am Miner und nicht am Pod.
    assert!(nach_eins.len() <= 12, "mehr Vermerke als Miner");

    // ⚑ **Und der Vermerk rückt mit der Epoche vor**, statt auf dem
    // ersten Stand stehen zu bleiben.
    for _ in 0..BLOECKE_JE_EPOCHE {
        k.baue_block();
    }
    let nach_zwei = &k.zustand().trainingsstand;
    let juengster_vorher = nach_eins.values().max().copied().unwrap_or(0);
    let juengster_nachher = nach_zwei.values().max().copied().unwrap_or(0);
    assert!(
        juengster_nachher > juengster_vorher,
        "der Vermerk blieb stehen: vorher {juengster_vorher}, nachher {juengster_nachher}"
    );
    assert!(nach_zwei.len() >= nach_eins.len(), "der Stand hat Miner verloren");
}

/// ⚑ **Nur ein Paar, das sich einig ist, bewegt das Modell.**
///
/// Vier Fälle, und drei davon dürfen nichts bewegen: ein Paar, von
/// dem nur einer geliefert hat; ein Paar mit verschiedenen
/// Ergebnissen; ein Paar, das gar nicht denselben Auftrag hatte.
#[test]
fn nur_ein_einiges_paar_bestaetigt_ein_delta() {
    use myl_types::trainingssegment::Trainingssegment;

    let mut z = Kette::probestand().zustand().clone();
    let e = 7u64;
    let seg = |charge: u8, commitment: u8| Trainingssegment {
        id: myl_types::ids::SegmentId::new([1u8; 32]),
        modell_version: myl_types::ids::MerkleRoot::new([2u8; 32]),
        charge: Hash([charge; 32]),
        startschritt: 0,
        schrittzahl: 30,
        folgen: 1,
        lr_zaehler: 1,
        lr_nenner: 1 << 12,
        delta_commitment: Hash([commitment; 32]),
        bewegte_gewichte: 1,
        pod_pfad: vec![myl_types::ids::MinerId::new([3u8; 32])],
        signaturen: vec![myl_types::bls::BlsSignature([0u8; 96])],
    };
    let setzen = |z: &mut LedgerState, pod: u32, s: Trainingssegment| {
        z.trainingssegmente.insert(myl_types::miner::pod_kennung(e, pod), s);
    };

    // Paar (0, 1): einig.
    setzen(&mut z, 0, seg(9, 42));
    setzen(&mut z, 1, seg(9, 42));
    // Paar (2, 3): uneinig.
    setzen(&mut z, 2, seg(9, 42));
    setzen(&mut z, 3, seg(9, 43));
    // Paar (4, 5): nur einer hat geliefert.
    setzen(&mut z, 4, seg(9, 42));
    // Paar (6, 7): dasselbe Ergebnis, aber verschiedene Charge.
    setzen(&mut z, 6, seg(9, 42));
    setzen(&mut z, 7, seg(8, 42));

    let paare = [(0u32, 1u32), (2, 3), (4, 5), (6, 7)];
    let bestaetigt = Kette::bestaetigte_deltas(&z, &paare, e);
    assert_eq!(bestaetigt, vec![Hash([42; 32])], "es zaehlte mehr als das einige Paar");

    // ⚑ Und ohne einiges Paar bleibt die Fassung stehen.
    let leer = Kette::bestaetigte_deltas(&z, &[(2, 3), (4, 5)], e);
    assert!(leer.is_empty());
    let alt = myl_types::ids::MerkleRoot::new([5u8; 32]);
    assert_eq!(myl_types::modellversion::naechste_version(&alt, &leer), alt);
    assert_ne!(myl_types::modellversion::naechste_version(&alt, &bestaetigt), alt);
}

/// ⚑ **Die Ordnung haengt nicht an der Paarreihenfolge.**
#[test]
fn die_bestaetigten_deltas_stehen_kanonisch() {
    use myl_types::trainingssegment::Trainingssegment;
    let mut z = Kette::probestand().zustand().clone();
    let e = 3u64;
    let seg = |c: u8| Trainingssegment {
        id: myl_types::ids::SegmentId::new([1u8; 32]),
        modell_version: myl_types::ids::MerkleRoot::new([2u8; 32]),
        charge: Hash([9; 32]),
        startschritt: 0,
        schrittzahl: 30,
        folgen: 1,
        lr_zaehler: 1,
        lr_nenner: 1 << 12,
        delta_commitment: Hash([c; 32]),
        bewegte_gewichte: 1,
        pod_pfad: vec![myl_types::ids::MinerId::new([3u8; 32])],
        signaturen: vec![myl_types::bls::BlsSignature([0u8; 96])],
    };
    for (pod, c) in [(0u32, 9u8), (1, 9), (2, 1), (3, 1)] {
        z.trainingssegmente.insert(myl_types::miner::pod_kennung(e, pod), seg(c));
    }
    let vorwaerts = Kette::bestaetigte_deltas(&z, &[(0, 1), (2, 3)], e);
    let rueckwaerts = Kette::bestaetigte_deltas(&z, &[(2, 3), (0, 1)], e);
    assert_eq!(vorwaerts, rueckwaerts, "die Paarreihenfolge wirkt auf die Ordnung");
    assert_eq!(vorwaerts, vec![Hash([1; 32]), Hash([9; 32])]);
}

/// ⚑ **Der Weg vom Trainingsplan bis in den Zustand, ganz.**
///
/// Ein bestellter Pod reicht sein Segment ein und es steht danach im
/// Zustand; ein Segment mit fremder Charge wird abgewiesen. Ohne
/// diesen Test wäre `TrainingssegmentEinreichen` eine Variante, die
/// niemand geht, und `Trainingssegment` bliebe die Arbeitsklasse
/// ohne Weg, die sie bis heute war.
#[test]
fn ein_bestellter_pod_reicht_sein_trainingssegment_ein() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    use myl_types::miner::HardwareClass;
    use myl_types::node_metadata::GeoRegion;
    use myl_types::trainingssegment::Trainingssegment;

    let mut k = Kette::probestand();
    let mut nonce = [0u64; 12];
    for w in 0..12u8 {
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::MinerAnmelden {
                    hardware: HardwareClass::MediumGpu,
                    zone: GeoRegion::Europe,
                    netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
        k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
        nonce[w as usize] += 1;
    }
    k.baue_block();
    for _ in 0..(3 * BLOECKE_JE_EPOCHE) {
        k.baue_block();
    }

    let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    let plan = k.trainingsplan();
    let nr = *plan.pods.iter().next().expect("ein Trainingspod");
    let pod = zuteilung.pods.iter().find(|p| p.pod_index == nr).expect("Pod");
    let kennung = myl_types::miner::pod_kennung(k.zustand().epoch.0, nr);
    let zuweisung = plan.zuweisungen[&nr];
    let charge =
        k.zustand().korpus.as_ref().expect("Korpus").charge(zuweisung.start, zuweisung.laenge);

    // Welcher Probeschlüssel gehört zum Koordinator?
    let koordinator = pod.shards[0].miner.miner_id;
    let w = (0..12u8)
        .find(|w| {
            myl_types::ids::MinerId::new(*probekonto(*w).as_bytes()) == koordinator
        })
        .expect("der Koordinator ist ein Probekonto");

    let bauen = |charge: myl_types::hash::Hash| {
        let mut seg = Trainingssegment {
            id: myl_types::ids::SegmentId::new([3u8; 32]),
            modell_version: myl_types::ids::MerkleRoot::new([4u8; 32]),
            charge,
            startschritt: 0,
            schrittzahl: 30,
            folgen: 1,
            lr_zaehler: Kette::TRAININGS_LR_ZAEHLER,
            lr_nenner: Kette::trainings_lr_nenner(Kette::trainingstiefe(Kette::probestand().zustand()), 30),
            delta_commitment: myl_types::hash::Hash([5u8; 32]),
            bewegte_gewichte: 1,
            pod_pfad: pod.mitglieder().map(|m| m.miner_id).collect(),
            signaturen: Vec::new(),
        };
        // ⚑ **Eine Unterschrift je Mitglied**, in der Reihenfolge
        // des Pfades. Der Übergang fasst sie zusammen und prüft in
        // einem Pairing.
        let botschaft = seg.botschaft();
        seg.signaturen = pod
            .mitglieder()
            .map(|m| {
                let nr = (0..12u8)
                    .find(|w| {
                        myl_types::ids::MinerId::new(*probekonto(*w).as_bytes()) == m.miner_id
                    })
                    .expect("Mitglied ist ein Probekonto");
                probeschluessel(nr).sign(&botschaft).expect("signieren")
            })
            .collect();
        seg
    };

    // ⚑ **Erst die Fälschung**, damit der Erfolg danach nicht bloss
    // beweist, dass irgendetwas durchgeht.
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(w),
            nonce[w as usize],
            Anweisung::TrainingssegmentEinreichen {
                pod: kennung,
                segment: bauen(myl_types::hash::Hash([99u8; 32])),
            },
        )
        .expect("signieren"),
    );
    nonce[w as usize] += 1;
    k.baue_block();
    assert!(
        k.zustand().trainingssegmente.is_empty(),
        "ein Segment mit fremder Charge kam durch"
    );

    // Und jetzt das richtige.
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(w),
            nonce[w as usize],
            Anweisung::TrainingssegmentEinreichen {
                pod: kennung,
                segment: bauen(charge),
            },
        )
        .expect("signieren"),
    );
    k.baue_block();
    let abgelegt = k
        .zustand()
        .trainingssegmente
        .get(&kennung)
        .expect("das Segment steht nicht im Zustand");
    assert_eq!(abgelegt.charge, charge);
    assert_eq!(
        abgelegt.lr_nenner,
        Kette::trainings_lr_nenner(Kette::trainingstiefe(Kette::probestand().zustand()), abgelegt.gradienten())
    );
}

/// ⚑ **Der ganze Trainingsweg: Paar reicht ein, Kette bestätigt,
/// Modell rückt, Miner wird bezahlt.**
///
/// Ohne diesen Test wären die letzten drei Glieder Behauptungen: Ein
/// Segment käme in den Zustand, und ob daraus je eine
/// Modellfassung oder eine Gutschrift wird, sähe niemand.
#[test]
fn ein_einiges_paar_bewegt_das_modell_und_wird_bezahlt() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    use myl_types::miner::HardwareClass;
    use myl_types::node_metadata::GeoRegion;
    use myl_types::trainingssegment::Trainingssegment;

    let mut k = Kette::probestand();
    let mut nonce = [0u64; 12];
    for w in 0..12u8 {
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::MinerAnmelden {
                    hardware: HardwareClass::MediumGpu,
                    zone: GeoRegion::Europe,
                    netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
        k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
        nonce[w as usize] += 1;
    }
    // Auszahlungskonten, sonst gilt „ohne Eintrag kein Anteil".
    for w in 0..12u8 {
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::AuszahlungskontoEintragen {
                    kennung: myl_types::ids::MinerId::new(*probekonto(w).as_bytes()),
                    konto: probekonto(w),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
    }
    // Brennstoff, damit es überhaupt eine Prägung gibt.
    k.aufnehmen(burn_nr(0, 5_000_000, nonce[0]));
    nonce[0] += 1;
    k.baue_block();
    for _ in 0..(3 * BLOECKE_JE_EPOCHE) {
        k.baue_block();
    }

    let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    let plan = k.trainingsplan();
    assert_eq!(plan.paare.len(), 1, "zwoelf Miner ergeben ein Trainingspaar");
    let (a, b) = plan.paare[0];
    let zuweisung = plan.zuweisungen[&a];
    assert_eq!(zuweisung, plan.zuweisungen[&b], "das Paar hat verschiedene Buendel");
    let charge = k
        .zustand()
        .korpus
        .as_ref()
        .expect("Korpus")
        .charge(zuweisung.start, zuweisung.laenge);
    let epoche = k.zustand().epoch.0;
    let vorher = k.zustand().modell_version;

    let schluessel_von = |id: myl_types::ids::MinerId| -> u8 {
        (0..12u8)
            .find(|w| myl_types::ids::MinerId::new(*probekonto(*w).as_bytes()) == id)
            .expect("Mitglied ist ein Probekonto")
    };

    // ⚑ **Beide Pods des Paars reichen dasselbe Ergebnis ein.**
    for nr in [a, b] {
        let pod = zuteilung.pods.iter().find(|p| p.pod_index == nr).expect("Pod");
        let mut seg = Trainingssegment {
            id: myl_types::ids::SegmentId::new([3u8; 32]),
            modell_version: myl_types::ids::MerkleRoot::new([4u8; 32]),
            charge,
            startschritt: 0,
            schrittzahl: 30,
            folgen: 1,
            lr_zaehler: Kette::TRAININGS_LR_ZAEHLER,
            lr_nenner: Kette::trainings_lr_nenner(Kette::trainingstiefe(Kette::probestand().zustand()), 30),
            delta_commitment: Hash([5u8; 32]),
            bewegte_gewichte: 1,
            pod_pfad: pod.mitglieder().map(|m| m.miner_id).collect(),
            signaturen: Vec::new(),
        };
        let botschaft = seg.botschaft();
        seg.signaturen = pod
            .mitglieder()
            .map(|m| probeschluessel(schluessel_von(m.miner_id)).sign(&botschaft).expect("sig"))
            .collect();
        let w = schluessel_von(pod.shards[0].miner.miner_id);
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::TrainingssegmentEinreichen {
                    pod: myl_types::miner::pod_kennung(epoche, nr),
                    segment: seg,
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
    }
    k.baue_block();
    assert_eq!(
        k.zustand().trainingssegmente.len(),
        2,
        "beide Segmente muessen im Zustand stehen"
    );

    // Ein Mitglied des Paars, dessen Konto wir beobachten.
    let beobachtet = {
        let pod = zuteilung.pods.iter().find(|p| p.pod_index == a).expect("Pod");
        probekonto(schluessel_von(pod.shards[1].miner.miner_id))
    };
    let vor_geld = k.zustand().account(&beobachtet).balance;

    // Über die Epochengrenze.
    for _ in 0..BLOECKE_JE_EPOCHE {
        k.baue_block();
    }

    assert_ne!(
        k.zustand().modell_version,
        vorher,
        "ein einiges Paar hat die Modellfassung nicht bewegt"
    );
    assert!(
        k.zustand().trainingssegmente.is_empty(),
        "die Segmente wurden nicht geraeumt"
    );
    assert!(
        k.zustand().account(&beobachtet).balance > vor_geld,
        "der Trainingsminer wurde nicht bezahlt"
    );
}

/// ⚑ **Punkt 38, die Kernaussage in der Kette:** An der
/// Epochengrenze wird abgeschlossen, und ein Konto wächst.
#[test]
fn an_der_epochengrenze_wird_abgeschlossen() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    let mut k = Kette::probestand();
    // Erst verbrennen, damit es eine Bemessungsgrundlage gibt.
    k.aufnehmen(burn(0, 1_000_000));
    k.baue_block();
    assert!(k.zustand().burn_epoche > 0, "es wurde nichts verbrannt");

    let vor = geldmenge(&k);
    for _ in 1..=BLOECKE_JE_EPOCHE {
        k.baue_block();
    }
    assert_eq!(k.zustand().epoch.0, 1, "die Epoche wechselte nicht");
    assert_eq!(k.zustand().burn_epoche, 0, "der Zaehler wurde nicht zurueckgesetzt");
    assert!(k.zustand().burn_ema > 0, "der geglaettete Wert blieb null");
    assert!(geldmenge(&k) > vor, "es wurde nichts gepraegt");
}

/// ⚑ **Geprägt wird allein der Treasury-Anteil**, weil dieser Kette
/// keine bezeugte Arbeit vorliegt. Der Shard-Miner-Anteil bleibt
/// ungeprägt, statt irgendwohin zu fließen.
#[test]
fn ohne_bezeugte_arbeit_bekommt_nur_das_treasury() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    let mut k = Kette::probestand();
    k.aufnehmen(burn(0, 1_000_000));
    k.baue_block();
    let vor = geldmenge(&k);
    for _ in 1..=BLOECKE_JE_EPOCHE {
        k.baue_block();
    }
    let treasury = myl_types::treasury::treasury_adresse();
    let auf_treasury = k.zustand().account(&treasury).balance as u128;
    assert!(auf_treasury > 0, "das Treasury ging leer aus");
    assert_eq!(
        geldmenge(&k) - vor,
        auf_treasury,
        "es wuchs mehr als der Treasury-Anteil"
    );
}

/// ⚑ **W4: Der Burn-Deckel je Adresse wirkt, aber erst ab dem
/// Mindestvolumen.** Über ihm scheitert der Burn, die nächste Epoche
/// beginnt neu, und ohne Subvention gilt er nicht.
#[test]
fn der_burn_deckel_gilt_je_adresse_und_epoche() {
    use myl_tokenomics::{burn_im_deckel, burn_spielraum, BURN_DECKEL_AB};
    let a = Address::new([7; 32]);
    let mut z = LedgerState::genesis(PROBE_CREDIT_PREIS);
    z.account_mut(&a).balance = 1_000 * BURN_DECKEL_AB;

    // Unter dem Mindestvolumen kein Deckel.
    assert!(burn_im_deckel(&z, &a, 900 * BURN_DECKEL_AB, true));

    z.burn_ema = 20 * BURN_DECKEL_AB;
    let deckel = burn_spielraum(z.burn_ema, 0);
    assert_eq!(deckel, BURN_DECKEL_AB);
    assert!(burn_im_deckel(&z, &a, deckel, true), "genau der Deckel geht");
    assert!(!burn_im_deckel(&z, &a, deckel + 1, true), "Gegenprobe: einer darüber nicht");
    assert!(burn_im_deckel(&z, &a, deckel + 1, false), "ohne Subvention kein Deckel");

    myl_ledger::transitions::burn_vermerken(&mut z, &a, deckel);
    assert!(!burn_im_deckel(&z, &a, 1, true), "die Epoche ist ausgeschöpft");
    assert!(burn_im_deckel(&z, &Address::new([8; 32]), deckel, true), "eine andere Adresse hat ihren eigenen");
    z.epoch = EpochId(1);
    assert!(burn_im_deckel(&z, &a, deckel, true), "die nächste Epoche beginnt neu");
}

/// ⚑ **W4 in der Kette**: Ein Burn über dem Deckel verbrennt nichts,
/// und beide Seiten rechnen dasselbe (der Deckel steht in `anwenden`).
#[test]
fn ein_burn_ueber_dem_deckel_verbrennt_nichts() {
    use myl_tokenomics::BURN_DECKEL_AB;
    let mut k = Kette::probestand();
    k.zustand.burn_ema = 20 * BURN_DECKEL_AB;
    // Sonst scheiterte der Burn schon an der Deckung und bewiese nichts.
    k.zustand.account_mut(&probekonto(0)).balance = 10 * BURN_DECKEL_AB;
    k.aufnehmen(burn_nr(0, BURN_DECKEL_AB + 1, 0));
    k.baue_block();
    assert_eq!(k.zustand().burn_epoche, 0, "über dem Deckel darf nichts verbrannt sein");
    // Gegenprobe: Genau der Deckel geht, und danach nichts mehr.
    k.aufnehmen(burn_nr(0, BURN_DECKEL_AB, 1));
    k.baue_block();
    assert_eq!(k.zustand().burn_epoche, BURN_DECKEL_AB, "der Deckel selbst muss gehen");
    k.aufnehmen(burn_nr(0, 1, 2));
    k.baue_block();
    assert_eq!(k.zustand().burn_epoche, BURN_DECKEL_AB, "die Epoche ist ausgeschöpft");
}

/// ⚑ **Der Preis sieht die Nachfrage über die Epochengrenze**: Wer so
/// viele Credits kauft, wie die Pods in einer Epoche Pässe rechnen, hebt
/// ihn (Auslastung 1 über dem Ziel 0,8); dieselbe Kette mit dem halben
/// Kauf oder ohne Käufe senkt ihn. 📌 Fund 714: Vor der Umrechnung sah
/// der Preis diesen Kauf als ein Millionstel und fiel.
#[test]
fn die_gekauften_credits_heben_den_preis() {
    use myl_types::miner::HardwareClass;
    use myl_types::node_metadata::GeoRegion;
    // `teil` Hälften der Kapazität kaufen, in Pässen.
    let lauf = |teil: u64| {
        let mut k = Kette::probestand();
        for w in 0..12u8 {
            k.aufnehmen(
                Transaktion::signiere(
                    &Kette::startwert(),
                    &probeschluessel(w),
                    0,
                    Anweisung::MinerAnmelden {
                        hardware: HardwareClass::MediumGpu,
                        zone: GeoRegion::Europe,
                        netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                    },
                )
                .expect("signieren"),
            );
            k.aufnehmen(probezusage(w, EpochId(1), 1));
        }
        k.baue_block();
        while k.zustand().epoch.0 < 3 {
            k.baue_block();
        }
        let kapazitaet = k.trainingsplan().kapazitaet;
        assert!(kapazitaet > 0, "kein Pod, die Probe waere leer");
        if teil > 0 {
            let paesse = (kapazitaet / myl_tokenomics::VTFE_UNITS_PER_TFE) * teil / 2;
            let menge = paesse.saturating_mul(k.zustand().credit_price);
            k.zustand.account_mut(&probekonto(0)).balance = menge.saturating_mul(2);
            k.aufnehmen(burn_nr(0, menge, 2));
        }
        k.baue_block();
        let vor = k.zustand().credit_preis_fein;
        while k.zustand().epoch.0 < 4 {
            k.baue_block();
        }
        (vor, k.zustand().credit_preis_fein)
    };
    let (vor, mit) = lauf(2);
    assert!(mit > vor, "volle Nachfrage muss den Preis heben: {vor} zu {mit}");
    let (vor, halb) = lauf(1);
    assert!(halb < vor, "Gegenprobe: die halbe Kapazität liegt unter dem Ziel: {vor} zu {halb}");
    let (vor, ohne) = lauf(0);
    assert!(ohne < vor, "Gegenprobe: ohne Käufe faellt er: {vor} zu {ohne}");
}

/// Eine Probekette mit zwölf angemeldeten Minern in Epoche 3, also mit Pods.
fn probekette_mit_pods() -> Kette {
    use myl_types::miner::HardwareClass;
    use myl_types::node_metadata::GeoRegion;
    let mut k = Kette::probestand();
    for w in 0..12u8 {
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                0,
                Anweisung::MinerAnmelden {
                    hardware: HardwareClass::MediumGpu,
                    zone: GeoRegion::Europe,
                    netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                },
            )
            .expect("signieren"),
        );
        k.aufnehmen(probezusage(w, EpochId(1), 1));
    }
    k.baue_block();
    while k.zustand().epoch.0 < 3 {
        k.baue_block();
    }
    k
}

/// 📌 **Fund 732:** Auch abgebuchte Credits sind Pässe. Wer in der Vorepoche
/// so viele abbuchte, wie die Pods für die Nachfrage rechnen, lastet sie voll
/// aus; vor der Umrechnung sah der Plan ein Millionstel davon, also null.
#[test]
fn abgebuchte_credits_der_vorepoche_sind_auslastung() {
    let skala = myl_types::auslastung::AUSLASTUNG_SKALA;
    let mut k = probekette_mit_pods();
    let kapazitaet = k.trainingsplan().kapazitaet;
    assert!(kapazitaet > 0, "kein Pod, die Probe waere leer");
    let paesse = kapazitaet / myl_tokenomics::VTFE_UNITS_PER_TFE;
    k.zustand_mut().vtfe_vorepoche = paesse;
    assert_eq!(k.trainingsplan().auslastung, skala, "die volle Kapazität in Pässen ist volle Auslastung");
    k.zustand_mut().vtfe_vorepoche = paesse / 2;
    assert_eq!(k.trainingsplan().auslastung, skala / 2);
    k.zustand_mut().vtfe_vorepoche = 0;
    assert_eq!(k.trainingsplan().auslastung, 0, "Gegenprobe: ohne Abbuchung keine Auslastung");
}

/// 📌 **Fund 733:** Der Abschluss einer Epoche hält ihren Plan. Wird in ihr
/// so viel abgebucht, dass in der nächsten niemand trainiert, müssen ihre
/// Trainingspods trotzdem im Stand landen (und ihre Paare die Fassung
/// bewegen); vorher rechnete der Abschluss den Plan nach dem Rollen der
/// Nachfrage neu und fand keinen.
#[test]
fn der_abschluss_haelt_den_plan_der_abgelaufenen_epoche() {
    let mut k = probekette_mit_pods();
    let plan = k.trainingsplan();
    assert!(!plan.pods.is_empty(), "ohne Nachfrage trainiert niemand, die Probe waere leer");
    let e = k.zustand().epoch.0;
    // Das Hundertfache der Kapazität in Pässen: Danach trainiert niemand.
    let kapazitaet = plan.kapazitaet / myl_tokenomics::VTFE_UNITS_PER_TFE;
    k.zustand_mut().vtfe_epoche = kapazitaet * 100;
    while k.zustand().epoch.0 == e {
        k.baue_block();
    }
    assert!(k.trainingsplan().pods.is_empty(), "nach der Überlast trainiert noch jemand");
    let stand = k.zustand().trainingsstand.als_map();
    assert!(
        stand.values().any(|&ep| ep == e),
        "die Trainingspods der abgelaufenen Epoche fehlen im Stand: {stand:?}"
    );
}

/// ⚑ **Die Kapazität folgt Leistung, Takt und Redundanz** (2026-11-26): die
/// Leistung des Netzes je Sekunde mal der Dauer einer Epoche in diesem Takt,
/// halbiert für die beiden Zwillinge.
#[test]
fn die_kapazitaet_folgt_leistung_takt_und_redundanz() {
    let netz = &crate::netz::TESTNETZ_INT4;
    let leistung = netz.pod_leistung_vtfe_je_s;
    // 1 800 Blöcke zu zwei Sekunden sind eine Stunde, zu 0,8 s 24 Minuten.
    assert_eq!(kapazitaet_je_pod(netz, 2_000), leistung * 3_600 / 2);
    assert_eq!(kapazitaet_je_pod(netz, 800), leistung * 1_440 / 2);
    let mut k = Kette::im_netz(netz);
    assert_eq!(k.kapazitaet_je_pod(), kapazitaet_je_pod(netz, netz.slot_ms), "ohne Angabe der Takt des Netzes");
    k.takt_setzen(800);
    assert_eq!(k.kapazitaet_je_pod(), kapazitaet_je_pod(netz, 800), "der Takt des Laufs zählt");
    assert_eq!(kapazitaet_je_pod(netz, u64::MAX), u64::MAX / 2, "kein Ueberlauf");
}

/// Die Vorgabe je Sekunde ist die der Governance je Stunde.
#[test]
fn die_vorgabe_der_leistung_folgt_der_governance() {
    assert_eq!(crate::netz::POD_LEISTUNG_VORGABE * 3_600, Kette::pod_kapazitaet_vtfe());
    assert_eq!(crate::netz::PROBEKETTE.pod_leistung_vtfe_je_s, crate::netz::POD_LEISTUNG_VORGABE);
}

/// ⚑ **Der Auftragslauf vom 2026-11-25, nachgerechnet** (Entscheidung des
/// Projektinhabers vom 2026-11-26): Ein Kunde, Runde für Runde, liess über
/// zwei Pods des 35B rund 16 600 Pässe je Epoche abbuchen (Takt 800 ms). Mit
/// der gemessenen Leistung ist das eine Auslastung knapp unter eins, und der
/// Preis steigt leicht (Whitepaper 5.4: über dem Ziel 0,8). Gegenprobe: Mit
/// der Vorgabe der Governance und ohne Redundanz stand er am Deckel, +22 %
/// je Epoche.
#[test]
fn ein_einzelner_kunde_hebt_den_preis_nur_leicht() {
    let skala = myl_types::auslastung::AUSLASTUNG_SKALA;
    let netz = &crate::netz::TESTNETZ_INT4;
    let verbraucht = 16_600 * myl_tokenomics::VTFE_UNITS_PER_TFE;
    let fein = 88i64 << 32;
    let u = preis_auslastung(0, verbraucht, 2 * kapazitaet_je_pod(netz, 800));
    assert!(u > skala * 9 / 10 && u < skala, "Auslastung {u}");
    let (neu, _) = credit_preis_naechster(fein, u);
    assert!(neu > fein && neu < fein / 100 * 104, "Schritt {} Prozent", (neu - fein) * 100 / fein);
    let alt = preis_auslastung(0, verbraucht, 2 * Kette::pod_kapazitaet_vtfe());
    assert_eq!(alt, CREDIT_PREIS_AUSLASTUNG_HOECHSTENS, "Gegenprobe: der Deckel");
    let (am_deckel, _) = credit_preis_naechster(fein, alt);
    assert!(am_deckel > fein / 100 * 121, "Gegenprobe: {} Prozent", (am_deckel - fein) * 100 / fein);
}

/// Fund 714: Ein Credit ist ein Pass, also 10⁶ vTFE-Einheiten.
#[test]
fn ein_gekaufter_credit_sind_eine_million_vtfe_einheiten() {
    assert_eq!(nachgefragte_vtfe(1_000, 100), 10 * myl_tokenomics::VTFE_UNITS_PER_TFE);
    assert_eq!(nachgefragte_vtfe(99, 100), 0, "ein angefangener Credit ist keiner");
    assert_eq!(nachgefragte_vtfe(1_000, 0), 1_000 * myl_tokenomics::VTFE_UNITS_PER_TFE, "ein Preis von null teilt nicht");
    assert_eq!(nachgefragte_vtfe(u64::MAX, 1), u64::MAX, "kein Ueberlauf");
    // Die volle Kapazität eines Pods in Pässen ist volle Auslastung.
    let paesse = Kette::pod_kapazitaet_vtfe() / myl_tokenomics::VTFE_UNITS_PER_TFE;
    assert_eq!(
        myl_types::auslastung::auslastung(nachgefragte_vtfe(paesse * 100, 100), Kette::pod_kapazitaet_vtfe()),
        myl_types::auslastung::AUSLASTUNG_SKALA
    );
}

/// ⚑ **Whitepaper 5.4: Der Preis sieht die Nachfrage**, auch die, die
/// keinen Platz fand, und springt je Epoche nur begrenzt.
#[test]
fn der_preis_folgt_der_nachfrage_mit_deckel() {
    let skala = myl_types::auslastung::AUSLASTUNG_SKALA;
    let halb = skala / 2;
    assert_eq!(preis_auslastung(halb, 0, 1_000), halb, "ohne Käufe die bestätigte Arbeit");
    assert_eq!(preis_auslastung(halb, 900, 1_000), 900 * skala / 1_000, "Käufe über der Arbeit zählen");
    assert_eq!(preis_auslastung(halb, 100, 1_000), halb, "Gegenprobe: Käufe darunter nicht");
    assert_eq!(preis_auslastung(0, 1_500, 1_000), skala + halb, "Übernachfrage über 1");
    assert_eq!(preis_auslastung(0, u64::MAX, 3_600_000_000), CREDIT_PREIS_AUSLASTUNG_HOECHSTENS, "der Sprung ist begrenzt");
    assert_eq!(preis_auslastung(halb, 900, 0), halb, "ohne Kapazität nur die bestätigte Arbeit");
}

/// Die Kette prägt in der Anlaufphase über dem geglätteten Burn.
#[test]
fn in_der_anlaufphase_wird_mehr_gepraegt_als_verbrannt() {
    assert_eq!(Kette::praegeparameter(0).subsidy_num, Kette::praegeparameter(0).subsidy_den, "s = 1 zum Start");
    assert_eq!(myl_tokenomics::mint_amount(1_000, &Kette::praegeparameter(0)), 2_000);
    assert_eq!(myl_tokenomics::mint_amount(1_000, &Kette::praegeparameter(17_520)), 1_000, "danach neutral");
}

/// Innerhalb einer Epoche wird nicht abgeschlossen.
#[test]
fn innerhalb_einer_epoche_wird_nicht_abgeschlossen() {
    let mut k = Kette::probestand();
    k.aufnehmen(burn(0, 1_000_000));
    for _ in 0..5 {
        k.baue_block();
    }
    assert_eq!(k.zustand().epoch.0, 0);
    assert!(
        k.zustand().burn_epoche > 0,
        "der Zaehler wurde mitten in der Epoche zurueckgesetzt"
    );
}

/// ⚑ **Der entscheidende Test: Erzeuger und Übernehmer kommen zur
/// selben Zustandswurzel.**
///
/// Ein Abschluss, den nur der Erzeuger rechnet, wäre eine
/// abweichende Wurzel und damit ein Konsensbruch. Er steht deshalb
/// in `anwenden`, das beide Seiten durchlaufen; dieser Test hält es
/// über eine Epochengrenze hinweg fest.
///
/// ⚑ **Er prüft Übereinstimmung, nicht Richtigkeit.** Beide Seiten
/// laufen durch denselben Code; ein Abschluss, der falsch rechnet,
/// rechnet auf beiden Seiten gleich falsch, und dieser Test bliebe
/// grün. Zwei Gegenproben haben das bestätigt: Mit ausgebautem
/// Abschluss und mit vertauschter Reihenfolge fielen die beiden
/// Tests darüber, dieser nicht. **Dass er nicht alles prüft, ist
/// kein Mangel, sondern seine Aufgabe**, und es gehört
/// aufgeschrieben, damit niemand ihn für mehr hält.
#[test]
fn erzeuger_und_uebernehmer_stimmen_ueber_die_epochengrenze_ueberein() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    let mut erzeuger = Kette::probestand();
    let mut uebernehmer = Kette::probestand();
    erzeuger.aufnehmen(burn(0, 1_000_000));

    for _ in 0..=BLOECKE_JE_EPOCHE {
        let b = erzeuger.baue_block();
        uebernehmer.uebernimm(&b).expect("Uebernahme");
    }
    assert_eq!(erzeuger.zustand().epoch.0, 1, "die Grenze wurde nicht ueberschritten");
    assert_eq!(
        erzeuger.zustand().commitment(),
        uebernehmer.zustand().commitment(),
        "die Zustandswurzeln weichen ueber die Epochengrenze ab"
    );
}

/// ⚑ **Ein zustandsloser Folger rechnet mit Zeugen mit**
/// (zustandslose Validierung, 2026-10-29): Er hält keine Konten, nur
/// die Wurzel des grossen Baums, und übernimmt Blöcke mit dem Zeugen,
/// den der Erzeuger bereithält: Überweisungen an bestehende und neue
/// Konten, Burns, über die Epochengrenze. Dieselbe Wurzel, und der
/// Erzeuger fand jeden Block bezeugbar (kein Zugriff über eine ganze
/// grosse Sammlung, auch nicht am Epochenwechsel). Ohne Zeugen, oder
/// mit einem, dem ein berührtes Konto fehlt, wird abgewiesen, und
/// nichts bewegt sich.
#[test]
fn ein_zustandsloser_folger_rechnet_mit_zeugen_mit() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    let mut erzeuger = Kette::probestand();
    let mut folger = Kette::probestand();
    // Die eigenen Konten des Folgers: ein Absender, ein neuer
    // Empfänger, eines, das kein Block berührt.
    let eigene = [probekonto(3), probekonto(200), probekonto(6), probekonto(9)];
    folger.zustandslos_machen(&eigene);
    assert_eq!(folger.zustandswurzel(), erzeuger.zustandswurzel());
    assert_eq!(folger.zustand().accounts.anzahl(), 0);
    let zeuge_von = |k: &Kette, b: &Block| -> myl_ledger::state::Zeuge {
        borsh::from_slice(&k.eigener_zeuge(&b.hash()).expect("bereitgehalten")).expect("lesbar")
    };
    erzeuger.aufnehmen(burn(1, 1_000));
    erzeuger.aufnehmen(mit_grenze(3, 0, u64::MAX, Anweisung::Ueberweisung { nach: probekonto(200), betrag: 100 }));
    erzeuger.aufnehmen(mit_grenze(4, 0, u64::MAX, Anweisung::Ueberweisung { nach: probekonto(5), betrag: 50 }));
    let b = erzeuger.baue_block();
    assert_eq!(b.txs.len(), 3);
    assert!(matches!(folger.uebernimm(&b), Err(KettenFehler::ZeugeTraegtNicht(None))));
    let z = zeuge_von(&erzeuger, &b);
    let mut ohne = z.clone();
    ohne.accounts.retain(|(a, _)| *a != probekonto(200));
    let vorher = folger.zustandswurzel();
    let f = folger.uebernimm_mit_zeuge(&b, &ohne).expect_err("unvollständig");
    assert!(
        matches!(f, KettenFehler::ZeugeTraegtNicht(Some(myl_ledger::state::Zeugenfehler::Unvollstaendig))),
        "{f}"
    );
    assert_eq!(folger.zustandswurzel(), vorher, "nichts bewegt");
    assert_eq!(folger.hoehe(), 0);
    folger.uebernimm_mit_zeuge(&b, &z).expect("trägt");
    assert_eq!(folger.zustandswurzel(), erzeuger.zustandswurzel());
    for i in 0..BLOECKE_JE_EPOCHE {
        if i % 300 == 0 {
            erzeuger.aufnehmen(burn_nr(2, 10, i / 300));
            erzeuger.aufnehmen(mit_grenze(6, i / 300, u64::MAX, Anweisung::Ueberweisung { nach: probekonto(201 + (i / 300) as u8), betrag: 7 }));
        }
        let b = erzeuger.baue_block();
        folger.uebernimm_mit_zeuge(&b, &zeuge_von(&erzeuger, &b)).expect("trägt");
    }
    assert_eq!(erzeuger.zustand().epoch.0, 1, "über die Grenze");
    assert_eq!(folger.zustandswurzel(), erzeuger.zustandswurzel());
    assert_eq!(erzeuger.zeugenlos(), 0, "ein Block war nicht bezeugbar");
    assert_eq!(folger.zustand().accounts.anzahl(), 0, "der Folger hält keine Konten");
    assert!(erzeuger.zustand().accounts.anzahl() > 6);
    // ⚑ Seine eigenen kennt er, Nummer und Guthaben wie der Erzeuger:
    //    Damit reicht er seine Arbeit mit der richtigen Nummer ein.
    for a in &eigene {
        assert_eq!(folger.konto(a), erzeuger.zustand().account(a), "eigenes Konto {a:?}");
    }
    assert_eq!(folger.konto(&probekonto(6)).nonce, 6, "sechs Überweisungen");
}

// --- Punkt 40, Glied 3a: Anmeldung über die Kette ---

fn anmeldung(wer: u8, hardware: myl_types::miner::HardwareClass) -> Transaktion {
    Transaktion::signiere(
        &Kette::startwert(),
        &probeschluessel(wer),
        0,
        Anweisung::MinerAnmelden {
            hardware,
            zone: myl_types::node_metadata::GeoRegion::Europe,
            netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
        },
    )
    .expect("signieren")
}

/// ⚑ **Punkt 40, Glied 3a in der Kette:** Eine Anmeldung über eine
/// Transaktion steht danach im Register.
#[test]
fn eine_anmeldung_ueber_die_kette_steht_im_register() {
    use myl_types::miner::HardwareClass;
    let mut k = Kette::probestand();
    k.aufnehmen(anmeldung(0, HardwareClass::MediumGpu));
    k.baue_block();
    let kennung = myl_types::ids::MinerId::new(*probekonto(0).as_bytes());
    let eintrag = k.zustand().miner.get(&kennung).expect("angemeldet");
    assert_eq!(eintrag.hardware_class, HardwareClass::MediumGpu);
}

/// ⚑ **Die Zone kommt mit und steht im Zustand** (Entscheidung 3b).
/// Sie entscheidet, in welchem Topf gemischt wird, und ist deshalb
/// Konsensdatum und keine gegossipte Angabe.
#[test]
fn die_zone_kommt_mit_in_den_zustand() {
    use myl_types::miner::HardwareClass;
    use myl_types::node_metadata::GeoRegion;
    let mut k = Kette::probestand();
    let tx = Transaktion::signiere(
        &Kette::startwert(),
        &probeschluessel(0),
        0,
        Anweisung::MinerAnmelden {
            hardware: HardwareClass::MediumGpu,
            zone: GeoRegion::Asia,
            netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
        },
    )
    .expect("signieren");
    k.aufnehmen(tx);
    k.baue_block();
    let kennung = myl_types::ids::MinerId::new(*probekonto(0).as_bytes());
    assert_eq!(k.zustand().miner[&kennung].zone, GeoRegion::Asia);
}

// --- G1: Gebühren, Aufschlag, Hinterlegung ---

fn mit_grenze(wer: u8, nonce: u64, max_gebuehr: u64, a: Anweisung) -> Transaktion {
    Transaktion::signiere_mit_grenze(&Kette::startwert(), &probeschluessel(wer), nonce, max_gebuehr, a)
        .expect("signieren")
}

/// ⚑ **Wer weniger zusagt, als die Gebühr kostet, wartet** und behält
/// seine Nummer; mit genau der Gebühr kommt er hinein, und sie landet im
/// Topf der Epoche.
#[test]
fn wer_die_gebuehr_nicht_zusagt_wartet() {
    let mut k = Kette::probestand();
    let a = || Anweisung::Burn { betrag: 10 };
    let g = gebuehr(&mit_grenze(3, 0, 0, a()));
    k.aufnehmen(mit_grenze(3, 0, g - 1, a()));
    let b = k.baue_block();
    assert!(b.txs.is_empty(), "eine Einheit zu wenig zugesagt");
    assert_eq!(k.zustand().account(&probekonto(3)).nonce, 0, "die Nummer ist verbraucht");
    assert_eq!(k.zustand().gebuehren_epoche, 0);

    // Gegenprobe: genau die Gebühr.
    let vorher = k.zustand().account(&probekonto(3)).balance;
    k.aufnehmen(mit_grenze(3, 0, g, a()));
    assert_eq!(k.baue_block().txs.len(), 1);
    assert_eq!(k.zustand().account(&probekonto(3)).balance, vorher - 10 - g);
    assert_eq!(k.zustand().gebuehren_epoche, g);
}

/// ⚑ **Eine gescheiterte Anweisung zahlt trotzdem**: Ihre Bytes stehen
/// im Block. Wer die Gebühr nicht decken kann, kommt nicht hinein.
#[test]
fn eine_gescheiterte_anweisung_zahlt_die_gebuehr() {
    let mut k = Kette::probestand();
    let vorher = k.zustand().account(&probekonto(3)).balance;
    let tx = burn(3, u64::MAX);
    let g = gebuehr(&tx);
    k.aufnehmen(tx);
    assert_eq!(k.baue_block().txs.len(), 1, "die Nummer ist verbraucht");
    assert_eq!(k.zustand().account(&probekonto(3)).balance, vorher - g, "nur die Gebühr");

    // Gegenprobe: ein Konto mit weniger als der Gebühr.
    k.zustand_mut().account_mut(&probekonto(4)).balance = g - 1;
    k.aufnehmen(burn(4, 1));
    assert!(k.baue_block().txs.is_empty(), "ungedeckt im Block");
    assert_eq!(k.zustand().account(&probekonto(4)).balance, g - 1);
}

/// ⚑ **Ein neues Konto kostet einmal den Aufschlag**, an die Treasury;
/// die zweite Überweisung an dasselbe Konto nicht.
#[test]
fn ein_neues_konto_kostet_einmal_den_aufschlag() {
    let mut k = Kette::probestand();
    let aufschlag = crate::netz::PROBEKETTE.gebuehren.aufschlag_konto;
    let treasury = myl_types::treasury::treasury_adresse();
    let neu = Address::new(*Hash::sha256(b"ein-neues-konto").as_bytes());
    assert!(!k.zustand().accounts.contains_key(&neu));
    let (v0, t0) = (k.zustand().account(&probekonto(3)).balance, k.zustand().account(&treasury).balance);

    let tx = mit_grenze(3, 0, u64::MAX, Anweisung::Ueberweisung { nach: neu, betrag: 100 });
    let g1 = gebuehr(&tx);
    k.aufnehmen(tx);
    k.baue_block();
    assert_eq!(k.zustand().account(&neu).balance, 100);
    assert_eq!(k.zustand().account(&probekonto(3)).balance, v0 - 100 - g1 - aufschlag);
    assert_eq!(k.zustand().account(&treasury).balance, t0 + aufschlag);

    // Gegenprobe: dasselbe Konto ein zweites Mal.
    let tx = mit_grenze(3, 1, u64::MAX, Anweisung::Ueberweisung { nach: neu, betrag: 100 });
    let g2 = gebuehr(&tx);
    k.aufnehmen(tx);
    k.baue_block();
    assert_eq!(k.zustand().account(&probekonto(3)).balance, v0 - 200 - g1 - g2 - aufschlag);
    assert_eq!(k.zustand().account(&treasury).balance, t0 + aufschlag);
}

/// ⚑ **Die Anmeldung hinterlegt, die Abmeldung gibt zurück**; eine
/// zweite Anmeldung desselben Miners hinterlegt nicht noch einmal.
#[test]
fn die_anmeldung_hinterlegt_und_die_abmeldung_gibt_zurueck() {
    use myl_types::miner::HardwareClass;
    let mut k = Kette::probestand();
    let h = crate::netz::PROBEKETTE.gebuehren.hinterlegung_anmeldung;
    let kennung = *probekonto(0).as_bytes();
    let v0 = k.zustand().account(&probekonto(0)).balance;
    let mut gezahlt = 0;
    let mut senden = |k: &mut Kette, tx: Transaktion| {
        gezahlt += gebuehr(&tx);
        k.aufnehmen(tx);
        k.baue_block();
        gezahlt
    };

    let g = senden(&mut k, anmeldung(0, HardwareClass::MediumGpu));
    assert_eq!(k.zustand().account(&probekonto(0)).balance, v0 - g - h);
    assert_eq!(k.zustand().hinterlegungen.get(&(HINTERLEGT_ANMELDUNG, kennung)), Some(&(probekonto(0), h)));

    // Gegenprobe: dieselbe Anmeldung mit anderen Angaben hinterlegt nicht.
    let neu = Anweisung::MinerAnmelden {
        hardware: HardwareClass::LargeGpu,
        zone: myl_types::node_metadata::GeoRegion::Europe,
        netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
    };
    let g = senden(&mut k, mit_grenze(0, 1, u64::MAX, neu));
    assert_eq!(k.zustand().account(&probekonto(0)).balance, v0 - g - h);

    let g = senden(&mut k, mit_grenze(0, 2, u64::MAX, Anweisung::MinerAbmelden));
    assert_eq!(k.zustand().account(&probekonto(0)).balance, v0 - g, "die Hinterlegung kam nicht zurück");
    assert!(k.zustand().hinterlegungen.is_empty());
}

/// ⚑ **Eine Sitzung hinterlegt, und wenn sie aufgeräumt wird, kommt
/// die Hinterlegung zurück** (Fund 662: Vorher räumte die Kette nie auf).
/// Gegenprobe: vor dem Ende der Nachfrist bleibt beides.
#[test]
fn eine_abgelaufene_sitzung_gibt_ihre_hinterlegung_zurueck() {
    use myl_types::sitzung::{Grenzen, Sitzungskontrakt};
    let mut k = Kette::probestand();
    let h = crate::netz::PROBEKETTE.gebuehren.hinterlegung_sitzung;
    let kontrakt = Sitzungskontrakt::neu(
        probekonto(6),
        probekonto(7),
        Grenzen::gesperrt(),
        Grenzen::gesperrt(),
        Vec::new(),
        EpochId(0),
        EpochId(0),
        16,
    )
    .expect("gueltiger Kontrakt");
    let id = kontrakt.adresse();
    let v0 = k.zustand().account(&probekonto(6)).balance;
    let tx = mit_grenze(6, 0, u64::MAX, Anweisung::SitzungEroeffnen { kontrakt });
    let g = gebuehr(&tx);
    k.aufnehmen(tx);
    k.baue_block();
    assert!(k.zustand().sitzung(&id).is_some());
    assert_eq!(k.zustand().account(&probekonto(6)).balance, v0 - g - h);

    let nachfrist = myl_ledger::state::SITZUNG_NACHFRIST;
    let kap = k.kapazitaet_je_pod();
    let z = k.zustand_mut();
    z.epoch = EpochId(nachfrist);
    Kette::epochenwechsel_abschliessen(z, nachfrist + 1, &Hash([0; 32]), None, kap);
    assert!(k.zustand().sitzung(&id).is_some(), "vor dem Ende der Nachfrist aufgeräumt");
    assert_eq!(k.zustand().account(&probekonto(6)).balance, v0 - g - h);

    let z = k.zustand_mut();
    z.epoch = EpochId(nachfrist + 1);
    Kette::epochenwechsel_abschliessen(z, nachfrist + 2, &Hash([0; 32]), None, kap);
    assert!(k.zustand().sitzung(&id).is_none(), "die abgelaufene Sitzung blieb");
    assert_eq!(k.zustand().account(&probekonto(6)).balance, v0 - g, "die Hinterlegung kam nicht zurück");
}

/// ⚑ **Die Reserve vertritt eine Position** (Whitepaper 6.8): Ihr
/// Schlüssel tritt an die Stelle des Inhabers; ein Fremder, eine Position
/// ausserhalb des Pods, eine doppelte Position und ein doppelter Vertreter
/// tragen nicht.
#[test]
fn die_reserve_vertritt_eine_position() {
    let mut k = Kette::probestand();
    for w in 0..6u8 {
        k.aufnehmen(probeanmeldung(w));
        k.aufnehmen(probezusage(w, EpochId(1), 1));
    }
    k.baue_block();
    while k.zustand().epoch.0 < 3 {
        k.baue_block();
    }
    let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    let pod = &zuteilung.pods[0];
    assert_eq!((pod.shards.len(), pod.reserve.len()), (4, 2));
    let inhaber = Kette::unterzeichner_des_buendels(pod, &[]).expect("ohne Vertreter");
    let r0 = pod.reserve[0];
    let r1 = pod.reserve[1];
    let mit = Kette::unterzeichner_des_buendels(pod, &[(1, r0.miner_id)]).expect("ein Vertreter");
    assert_eq!(mit[1], (r0.miner_id, r0.schluessel), "die Reserve unterschreibt für Position 1");
    assert_eq!((mit[0], mit[2], mit[3]), (inhaber[0], inhaber[2], inhaber[3]), "die übrigen bleiben");
    assert!(Kette::unterzeichner_des_buendels(pod, &[(1, r0.miner_id), (3, r1.miner_id)]).is_some());
    // ⛔️ Gegenproben.
    let fremd = pod.shards[2].miner.miner_id;
    assert!(Kette::unterzeichner_des_buendels(pod, &[(1, fremd)]).is_none(), "keine Reserve");
    assert!(Kette::unterzeichner_des_buendels(pod, &[(4, r0.miner_id)]).is_none(), "keine solche Position");
    assert!(Kette::unterzeichner_des_buendels(pod, &[(1, r0.miner_id), (1, r1.miner_id)]).is_none(), "Position doppelt");
    assert!(Kette::unterzeichner_des_buendels(pod, &[(1, r0.miner_id), (2, r0.miner_id)]).is_none(), "Vertreter doppelt");
}

/// ⚑ **Stufe 1: vergütet wird nur, was zwei Zwillinge gleich gerechnet
/// haben** (Whitepaper 3.4 und 5.3), dann jeder zur Hälfte; uneinig oder
/// ohne Bündel des Zwillings keiner; ein Pod allein kommt zurück
/// (Anlaufregel), einer in Bereitschaft bekommt nichts.
#[test]
fn zwillinge_werden_nur_einig_vergutet() {
    let wurzel = |w: u8| myl_types::MerkleRoot::new([w; 32]);
    let buendel = |pod: u8, w: u8, segmente: u32| myl_types::PoIBundle {
        epoch: EpochId(3),
        pod: myl_types::ids::PodId::new([pod; 32]),
        segments_root: wurzel(pod),
        vtfe_claimed: 1_200,
        aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
        segmente,
        ausgabenwurzel: wurzel(w),
        vertreter: Vec::new(),
        sitzungen: Vec::new(),
    };
    let abrechnung = |vtfe: u64| myl_tokenomics::Podabrechnung {
        positionen: Vec::new(),
        reserve: Vec::new(),
        vtfe_pod: vtfe,
        verteilung: None,
    };
    let gueltig = |liste: &[(u32, myl_types::PoIBundle)]| {
        liste.iter().map(|(i, b)| (*i, (b.clone(), abrechnung(b.vtfe_claimed)))).collect::<std::collections::BTreeMap<_, _>>()
    };
    let vtfe = |n: &NachZwillingen| n.bezahlt.iter().map(|a| a.vtfe_pod).collect::<Vec<_>>();
    let paar = [vec![0u32, 1]];
    // Einig: beide zur Hälfte.
    let n = nach_zwillingen(gueltig(&[(0, buendel(1, 9, 4)), (1, buendel(2, 9, 4))]), &paar);
    assert_eq!((vtfe(&n), n.einig, n.uneinig), (vec![600, 600], 1, 0));
    // ⛔️ Gegenproben: andere Wurzel, andere Segmentzahl, Wurzel null,
    //    ein Bündel fehlt: keiner wird vergütet.
    for (a, b) in [
        (buendel(1, 9, 4), Some(buendel(2, 8, 4))),
        (buendel(1, 9, 4), Some(buendel(2, 9, 5))),
        (buendel(1, 0, 4), Some(buendel(2, 0, 4))),
        (buendel(1, 9, 4), None),
    ] {
        let beide = b.is_some();
        let mut liste = vec![(0, a)];
        liste.extend(b.map(|b| (1, b)));
        let n = nach_zwillingen(gueltig(&liste), &paar);
        assert!(n.bezahlt.is_empty() && n.allein.is_empty(), "uneinig vergütet: {n:?}");
        assert_eq!((n.einig, n.uneinig), (0, 1));
        // ⚑ Mit beiden Bündeln ist die Gruppe verschieden (ein
        //   Differenzbeweis darf kommen); mit einem fehlenden nicht.
        assert_eq!(n.verschieden.len(), usize::from(beide));
    }
    // Keiner von beiden hat etwas: nichts zu zählen.
    let n = nach_zwillingen(gueltig(&[]), &paar);
    assert_eq!((n.einig, n.uneinig), (0, 0));
    // ⚑ Ein Pod allein (Anlaufregel): nicht bezahlt, sondern zurück.
    let n = nach_zwillingen(gueltig(&[(2, buendel(3, 0, 4))]), &[vec![2]]);
    assert!(n.bezahlt.is_empty());
    assert_eq!(n.allein.iter().map(|(i, r)| (*i, r.vtfe_pod)).collect::<Vec<_>>(), vec![(2, 1_200)]);
    // ⛔️ Gegenprobe: Ein Pod in Bereitschaft (in keiner Gruppe), der
    //    trotzdem einreicht, bekommt weder Lohn noch Rückhalt.
    let n = nach_zwillingen(gueltig(&[(0, buendel(1, 9, 4)), (1, buendel(2, 9, 4)), (5, buendel(3, 9, 4))]), &paar);
    assert_eq!((vtfe(&n), n.allein.len(), n.bereitschaft), (vec![600, 600], 0, 1));
}

/// ⚑ **Stufe 1 je Sitzung** (Whitepaper 6.4, 2026-11-16): Ein Paar,
/// dessen Bündel sich nur in einzelnen Sitzungen unterscheiden, wird je
/// Sitzung abgerechnet: Gleiches sofort, Abweichendes vorbehalten und
/// anfechtbar, Fehlendes vorbehalten mit einem Pflichtauftrag, höchstens
/// vier je Paar.
#[test]
fn ein_paar_wird_je_sitzung_abgerechnet() {
    let wurzel = |w: u8| myl_types::MerkleRoot::new([w; 32]);
    let z = |s: u64, n: u32, w: u8| myl_types::Sitzungszeugnis { sitzung: s, segmente: n, wurzel: wurzel(w) };
    // Die flache Wurzel folgt hier aus der Liste, wie bei einem
    // ehrlichen Pod: gleiche Listen, gleiche Wurzel.
    let buendel = |pod: u32, liste: &[myl_types::Sitzungszeugnis]| {
        let stoff: Vec<u8> = liste.iter().flat_map(|e| [e.sitzung as u8, e.segmente as u8, e.wurzel.as_bytes()[0]]).collect();
        myl_types::PoIBundle {
            epoch: EpochId(3),
            pod: myl_types::pod_kennung(3, pod),
            segments_root: wurzel(pod as u8 + 100),
            vtfe_claimed: 1_200,
            aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
            segmente: liste.iter().map(|e| e.segmente).sum(),
            ausgabenwurzel: myl_types::MerkleRoot::new(myl_types::Hash::sha256(&stoff).0),
            vertreter: Vec::new(),
            sitzungen: liste.to_vec(),
        }
    };
    let gueltig = |liste: &[(u32, myl_types::PoIBundle)]| {
        liste
            .iter()
            .map(|(i, b)| {
                let r = myl_tokenomics::Podabrechnung { positionen: Vec::new(), reserve: Vec::new(), vtfe_pod: b.vtfe_claimed, verteilung: None };
                (*i, (b.clone(), r))
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let vtfe = |n: &NachZwillingen| n.bezahlt.iter().map(|a| a.vtfe_pod).collect::<Vec<_>>();
    let (pa, pb) = (myl_types::pod_kennung(3, 0), myl_types::pod_kennung(3, 1));
    let paar = [vec![0u32, 1]];

    // Ganz gleich: wie bisher, beide zur Hälfte.
    let alle = [z(1, 6, 1), z(2, 3, 2), z(3, 3, 3)];
    let n = nach_zwillingen(gueltig(&[(0, buendel(0, &alle)), (1, buendel(1, &alle))]), &paar);
    assert_eq!((vtfe(&n), n.einig, n.uneinig, n.pflicht.len(), n.vorbehalten.len()), (vec![600, 600], 1, 0, 0, 0));

    // ⚑ Dem Zwilling fehlt die letzte Sitzung (3 von 12 Segmenten).
    //   Vorher: beide nichts. Jetzt: A neun Zwölftel zur Hälfte sofort,
    //   B alles zur Hälfte, und A ein Pflichtauftrag über Sitzung 3.
    let n = nach_zwillingen(gueltig(&[(0, buendel(0, &alle)), (1, buendel(1, &alle[..2]))]), &paar);
    assert_eq!((vtfe(&n), n.einig, n.uneinig), (vec![450, 600], 1, 0));
    assert!(n.verschieden.is_empty() && n.vorbehalten.is_empty(), "nichts weicht ab");
    assert_eq!(n.pflicht.len(), 1);
    let (pod, zwilling, f, r) = &n.pflicht[0];
    assert_eq!((*pod, *zwilling, f.sitzung, f.segmente, f.stelle, r.vtfe_pod), (pa, pb, 3, 3, 11, 150));

    // ⚑ Eine Sitzung weicht ab, eine fehlt jedem: Gleiches bezahlt, das
    //   Abweichende vorbehalten und anfechtbar, je ein Pflichtauftrag.
    let a = [z(1, 6, 1), z(2, 3, 2), z(3, 3, 3)];
    let b = [z(1, 6, 1), z(2, 3, 9), z(4, 3, 4)];
    let n = nach_zwillingen(gueltig(&[(0, buendel(0, &a)), (1, buendel(1, &b))]), &paar);
    assert_eq!((vtfe(&n), n.einig, n.uneinig, n.verschieden.len()), (vec![300, 300], 0, 1, 1));
    let ((va, ra), (vb, rb)) = &n.vorbehalten[0];
    assert_eq!((*va, ra.vtfe_pod, *vb, rb.vtfe_pod), (pa, 150, pb, 150));
    let pflicht: Vec<_> = n.pflicht.iter().map(|(p, _, f, r)| (*p, f.sitzung, f.stelle, r.vtfe_pod)).collect();
    assert_eq!(pflicht, vec![(pa, 3, 11, 150), (pb, 4, 11, 150)]);
    // Nichts geht verloren und nichts entsteht: 600 je Pod.
    assert_eq!(300 + 150 + 150, 600);

    // ⚑ Das Bündel des Zwillings fehlt ganz: nichts sofort, die vier
    //   grössten Sitzungen bekommen einen Pflichtauftrag, der Rest nichts.
    let viele = [z(1, 1, 1), z(2, 5, 2), z(3, 2, 3), z(4, 4, 4), z(5, 3, 5), z(6, 5, 6)];
    let n = nach_zwillingen(gueltig(&[(0, buendel(0, &viele))]), &paar);
    assert!(n.bezahlt.is_empty() && n.verschieden.is_empty());
    assert_eq!((n.einig, n.uneinig), (0, 1));
    let gewaehlt: Vec<_> = n.pflicht.iter().map(|(p, zw, f, r)| (*p, *zw, f.sitzung, f.stelle, r.vtfe_pod)).collect();
    // 20 Segmente, 1 200 vTFE: je Segment 60, je Paar die Hälfte, 30.
    assert_eq!(gewaehlt, vec![(pa, pb, 2, 5, 150), (pa, pb, 6, 19, 150), (pa, pb, 4, 11, 120), (pa, pb, 5, 14, 90)]);

    // ⚑ Abwechselnd je Seite: Ein Pod mit vielen grossen Sitzungen
    //   nimmt dem anderen nicht alle Plätze.
    let a = [z(1, 2, 1), z(10, 9, 1), z(11, 9, 1), z(12, 9, 1), z(13, 9, 1)];
    let b = [z(1, 2, 1), z(20, 1, 1), z(21, 1, 1)];
    let n = nach_zwillingen(gueltig(&[(0, buendel(0, &a)), (1, buendel(1, &b))]), &paar);
    let seiten: Vec<_> = n.pflicht.iter().map(|(p, _, f, _)| (*p, f.sitzung)).collect();
    assert_eq!(seiten, vec![(pa, 10), (pa, 11), (pb, 20), (pb, 21)]);

    // ⛔️ Gegenprobe 1: Ein Bündel ohne Liste: das Paar als Ganzes, wie
    //    zuvor, keiner wird vergütet.
    let mut ohne = buendel(1, &alle[..2]);
    ohne.sitzungen.clear();
    let n = nach_zwillingen(gueltig(&[(0, buendel(0, &alle)), (1, ohne)]), &paar);
    assert!(n.bezahlt.is_empty() && n.pflicht.is_empty());
    assert_eq!((n.uneinig, n.verschieden.len(), n.vorbehalten.len()), (1, 1, 1));
    // ⛔️ Gegenprobe 2: Gleiche Listen, verschiedene flache Wurzeln (eine
    //    Liste passt nicht zu ihrem Bündel): nichts gilt als gleich.
    let mut schief = buendel(1, &alle);
    schief.ausgabenwurzel = wurzel(77);
    let n = nach_zwillingen(gueltig(&[(0, buendel(0, &alle)), (1, schief)]), &paar);
    assert!(n.bezahlt.is_empty() && n.pflicht.is_empty());
    assert_eq!((n.uneinig, n.vorbehalten[0].0 .1.vtfe_pod), (1, 600));
    // ⛔️ Gegenprobe 3: Eine Liste, deren Summe nicht die Segmentzahl
    //    ist, zählt nicht als Liste.
    let mut falsch = buendel(1, &alle[..2]);
    falsch.segmente = 12;
    let n = nach_zwillingen(gueltig(&[(0, buendel(0, &alle)), (1, falsch)]), &paar);
    assert!(n.bezahlt.is_empty() && n.pflicht.is_empty());
    // ⛔️ Gegenprobe 4: Fehlt das Bündel des Zwillings und trägt das
    //    eigene keine Liste, bleibt es bei nichts.
    let mut ohne = buendel(0, &alle);
    ohne.sitzungen.clear();
    let n = nach_zwillingen(gueltig(&[(0, ohne)]), &paar);
    assert!(n.bezahlt.is_empty() && n.pflicht.is_empty());
    assert_eq!(n.uneinig, 1);
}

/// ⚑ **Die Pflichtaufträge werden immer gezogen** (Stufe 1 je Sitzung):
/// nicht per Los, ohne Doppel zur Stichprobe, und ihr Auftrag trägt die
/// Bindung an die Sitzung. Schliesst er einig und ganz, wird der
/// Vorbehalt frei und am nächsten Wechsel fällig.
#[test]
fn pflichtauftraege_werden_immer_gezogen_und_gebunden() {
    let m = |b: u8| myl_types::ids::MinerId::new([b; 32]);
    let (pa, pb) = (myl_types::pod_kennung(3, 0), myl_types::pod_kennung(3, 1));
    let offen = |pflicht: Vec<(myl_types::ids::PodId, u32, u64, u32)>| myl_ledger::state::OffeneStichprobe {
        epoche: 3,
        // Zehn Segmente je Pod: Bei 1 bis 3 % zieht das Los meist nichts.
        anteile: vec![(pa, 10), (pb, 10)],
        wurzeln: vec![(pa, myl_types::MerkleRoot::new([1; 32]), 10), (pb, myl_types::MerkleRoot::new([2; 32]), 10)],
        voll: Vec::new(),
        pruefkandidaten: vec![m(1), m(2), m(3)],
        ersatz: Vec::new(),
        besetzung: Vec::new(),
        pflicht,
    };
    let ziehen = |o: myl_ledger::state::OffeneStichprobe| {
        let mut zustand = LedgerState::genesis(1);
        let mut gezogen = None;
        Kette::stichproben_ziehen(&mut zustand, vec![o], |_| [7u8; 32], &mut gezogen);
        (zustand, gezogen.map(|(_, s): (u64, Vec<crate::stichprobe::Segmentstichprobe>)| s).unwrap_or_default())
    };
    // ⛔️ Gegenprobe zuerst: ohne Pflicht zieht dieselbe Saat die Stellen
    //    9 und 4 nicht (sonst bewiese der Test nichts).
    let (ohne, los) = ziehen(offen(Vec::new()));
    assert!(!los.iter().any(|s| (s.pod == pa && s.segment == 9) || (s.pod == pb && s.segment == 4)), "das Los traf sie schon: {los:?}");
    assert!(ohne.pruefauftraege.iter().all(|a| a.gebunden.is_none()));
    // Mit Pflicht: beide Stellen gezogen, je ein gebundener Auftrag.
    let (mut zustand, mit) = ziehen(offen(vec![(pa, 9, 77, 4), (pb, 4, 78, 5)]));
    assert_eq!(mit.len(), los.len() + 2);
    assert!(mit.iter().any(|s| s.pod == pa && s.segment == 9) && mit.iter().any(|s| s.pod == pb && s.segment == 4));
    let gebunden: Vec<_> = zustand.pruefauftraege.iter().filter_map(|a| a.gebunden.map(|g| (a.pod, a.segment, g))).collect();
    assert_eq!(gebunden, vec![(pa, 9, (77, 4)), (pb, 4, (78, 5))]);
    assert!(zustand.pruefauftraege.iter().all(|a| a.pruefer.len() == 2 && a.epoche == 3));

    // Der Auftrag von A schliesst einig und ganz, der von B bleibt ohne
    // Aufdeckung: nur A wird frei.
    let vorbehalt = |pod, zwilling, stelle, wer: u8| myl_ledger::pruefung::Vorbehalt {
        epoche: 3,
        pod,
        zwilling,
        je_miner: vec![(m(wer), 150)],
        frei: false,
        bis_epoche: 9,
        pflicht: Some(stelle),
    };
    zustand.vorbehalte = vec![vorbehalt(pa, pb, 9, 40), vorbehalt(pb, pa, 4, 41)];
    let auftrag = zustand.pruefauftraege.iter().find(|a| a.pod == pa).cloned().expect("Auftrag");
    let befund = myl_ledger::pruefung::Pruefbefund { logits: Hash::sha256(b"l"), abweichung: None, positionen: 4 };
    let salz = [5u8; 32];
    let adresse = |w: &myl_types::ids::MinerId| myl_types::Address::new(*w.as_bytes());
    for w in &auftrag.pruefer {
        let f = myl_ledger::pruefung::festlegung(&befund, &salz, w);
        myl_ledger::pruefung::festlegen(&mut zustand, &adresse(w), 3, &pa, 9, f, auftrag.frist_festlegen).expect("festlegen");
    }
    for w in &auftrag.pruefer {
        myl_ledger::pruefung::aufdecken(&mut zustand, &adresse(w), 3, &pa, 9, befund.clone(), &salz, auftrag.frist_aufdecken).expect("aufdecken");
    }
    myl_ledger::pruefung::faellige_abschliessen(&mut zustand, auftrag.frist_aufdecken + 1, 30);
    assert_eq!(zustand.vorbehalte.iter().map(|v| (v.pod, v.frei)).collect::<Vec<_>>(), vec![(pa, true), (pb, false)]);
    assert_eq!(myl_ledger::pruefung::vorbehalte_faellig(&mut zustand, 4), vec![(m(40), 150)]);
    // B verfällt mit der Frist.
    assert!(myl_ledger::pruefung::vorbehalte_faellig(&mut zustand, 9).is_empty());
    assert!(zustand.vorbehalte.is_empty());
}

/// ⚑ **Anlaufregel:** Was ein Pod allein rechnete, wartet die
/// Streitfrist ab und kommt dann in die Zuschreibung; seine Segmente
/// werden alle gezogen.
#[test]
fn ein_pod_allein_wird_zurueckgehalten_und_voll_gezogen() {
    use myl_ledger::einsatz::SPERRFRIST_EPOCHEN;
    let m = |b: u8| myl_types::ids::MinerId::new([b; 32]);
    let mut gehalten = Vec::new();
    let mut je = std::collections::BTreeMap::new();
    zurueckhalten_und_freigeben(&mut gehalten, 5, &mut je, [(m(1), 300), (m(2), 0)].into_iter().collect());
    assert!(je.is_empty(), "nicht sofort");
    assert_eq!(gehalten, vec![(5 + SPERRFRIST_EPOCHEN, m(1), 300)], "ohne Nullen");
    // ⛔️ Gegenprobe: eine Epoche vor der Freigabe noch nichts.
    zurueckhalten_und_freigeben(&mut gehalten, 4 + SPERRFRIST_EPOCHEN, &mut je, Default::default());
    assert!(je.is_empty() && gehalten.len() == 1);
    // Fällig: zu dem, was der Miner in dieser Epoche ohnehin bekommt.
    je.insert(m(1), 50);
    zurueckhalten_und_freigeben(&mut gehalten, 5 + SPERRFRIST_EPOCHEN, &mut je, Default::default());
    assert_eq!(je.get(&m(1)), Some(&350));
    assert!(gehalten.is_empty());

    let a = myl_types::ids::PodId::new([1; 32]);
    let b = myl_types::ids::PodId::new([2; 32]);
    let mut gezogen = vec![crate::stichprobe::Segmentstichprobe { pod: a, segment: 1 }];
    voll_ziehen(&mut gezogen, &[a], &[(a, 3), (b, 5)]);
    gezogen.sort();
    assert_eq!(gezogen.iter().map(|s| (s.pod, s.segment)).collect::<Vec<_>>(), vec![(a, 0), (a, 1), (a, 2)], "alle, ohne Doppel, nur a");
    let mut viel = Vec::new();
    voll_ziehen(&mut viel, &[b], &[(b, 1_000)]);
    assert_eq!(viel.len() as u32, Kette::VOLL_GEZOGEN_HOECHSTENS, "gedeckelt");
}

/// ⚑ **Der Credit-Preis folgt der Auslastung** (Whitepaper 5.4, κ = 1/6,
/// u* = 0,8): ohne Nachfrage ein Achtel weniger, bei voller Auslastung
/// rund 3,4 % mehr, am Ziel gleich; nie unter 1.
#[test]
fn der_credit_preis_folgt_der_auslastung() {
    let voll = myl_types::auslastung::AUSLASTUNG_SKALA;
    let p = 100i64 << 32;
    let (_, ohne) = credit_preis_naechster(p, 0);
    assert!((87..=88).contains(&ohne), "ohne Nachfrage: {ohne}");
    let (_, hoch) = credit_preis_naechster(p, voll);
    assert!((103..=104).contains(&hoch), "voll: {hoch}");
    let (_, ziel) = credit_preis_naechster(p, CREDIT_PREIS_ZIEL);
    assert!((99..=101).contains(&ziel), "am Ziel: {ziel}");
    // Tausend Epochen ohne Nachfrage: unten begrenzt, abgerechnet 1.
    let mut f = p;
    for _ in 0..1_000 {
        f = credit_preis_naechster(f, 0).0;
    }
    assert!(f >= myl_tokenomics::PREIS_UNTERGRENZE_VORGABE);
    assert_eq!(credit_preis_naechster(f, 0).1, 1, "nie null");
}

/// Abgerechnet wird zur nächsten geraden Zahl gerundet, mindestens 1.
#[test]
fn der_credit_preis_rundet_zur_geraden_zahl() {
    let halb = 1i64 << 31;
    assert_eq!(credit_preis_abgerechnet((2 << 32) + halb), 2);
    assert_eq!(credit_preis_abgerechnet((3 << 32) + halb), 4);
    assert_eq!(credit_preis_abgerechnet((2 << 32) + halb + 1), 3);
    assert_eq!(credit_preis_abgerechnet((2 << 32) + halb - 1), 2);
    assert_eq!(credit_preis_abgerechnet(1), 1, "nie null");
    assert_eq!(credit_preis_abgerechnet(0), 1);
}

/// 📌 **Ein kleiner Credit-Preis bleibt beweglich**: Mit Feinauflösung
/// steigt er von 4 über sechzig volle Epochen. ⛔️ Gegenprobe: ganzzahlig
/// fortgeschrieben bleibt er bei 4 stehen.
#[test]
fn ein_kleiner_credit_preis_bleibt_beweglich() {
    let voll = myl_types::auslastung::AUSLASTUNG_SKALA;
    let (mut fein, mut ganz) = (4i64 << 32, 4u64);
    for _ in 0..60 {
        (fein, ganz) = credit_preis_naechster(fein, voll);
    }
    assert!(ganz >= 20, "mit Feinauflösung: {ganz}");
    let mut nur_ganz = 4u64;
    for _ in 0..60 {
        nur_ganz = credit_preis_naechster((nur_ganz as i64) << 32, voll).1;
    }
    assert_eq!(nur_ganz, 4, "ganzzahlig steht er");
}

/// Die Kette schreibt den Preis am Epochenwechsel fort: Ohne Nachfrage
/// liegt er danach unter dem Startwert, und beide Darstellungen passen.
#[test]
fn der_epochenwechsel_schreibt_den_credit_preis_fort() {
    let mut k = Kette::probestand();
    assert_eq!(k.zustand().credit_price, PROBE_CREDIT_PREIS);
    while k.zustand().epoch.0 < 1 {
        k.baue_block();
    }
    let z = k.zustand();
    assert!(z.credit_price < PROBE_CREDIT_PREIS, "{}", z.credit_price);
    assert_eq!(credit_preis_naechster(PROBE_CREDIT_PREIS as i64 * (1 << 32), 0).0, z.credit_preis_fein);
}

/// ⚑ **Ein Block trägt höchstens das Doppelte des Ziels in
/// Gebühreneinheiten**; der Rest eines vollen Mempools wartet.
#[test]
fn ein_block_traegt_hoechstens_das_doppelte_ziel() {
    // ⚑ 2026-10-28: Prüfeinheiten. Vorher passten 4 096 Burns (rund
    //   700 KiB) in einen Block, das Ziel lag bei rund 3 000.
    let mut k = Kette::probestand();
    let mut im_mempool = 0;
    for q in 0..64u32 {
        let sk = myl_types::bls::BlsSecretKey::key_gen(Hash::sha256(format!("einheiten-{q}").as_bytes()).as_bytes())
            .expect("Schlüssel");
        let a = Address::aus_schluessel(&sk.public_key().expect("pk"));
        k.zustand_mut().account_mut(&a).balance = 1_000_000_000;
        for n in 0..64u64 {
            let tx = Transaktion::signiere(&Kette::startwert(), &sk, n, Anweisung::Burn { betrag: 1 }).expect("signieren");
            im_mempool += usize::from(k.aufnehmen(tx));
        }
    }
    // 64 mal 64 (seit dem 2026-10-29 fasst der Mempool mehr).
    let alle = 64 * 64;
    assert_eq!(im_mempool, alle);
    let b = k.baue_block();
    let einheiten: u64 = b.txs.iter().map(|t| t.gebuehreneinheiten()).sum();
    assert!(einheiten <= BLOCK_EINHEITEN_HOECHSTENS, "{einheiten} Einheiten im Block");
    assert!(b.txs.len() < alle / 2, "{} Transaktionen im Block", b.txs.len());
    assert_eq!(k.wartend(), alle - b.txs.len(), "der Rest wartet");
}

/// ⚑ **Der Grundpreis wandert um höchstens ein Achtel**: am Ziel bleibt
/// er, ein voller Block hebt ihn um ein Achtel, ein leerer senkt ihn um
/// eins; bei eins steigt er dennoch um eins, und unter den Boden fällt
/// er nie.
#[test]
fn der_grundpreis_wandert_um_hoechstens_ein_achtel() {
    let ziel = GRUNDPREIS_ZIEL_EINHEITEN;
    assert_eq!(naechster_grundpreis(800, ziel, 1), 800);
    assert_eq!(naechster_grundpreis(800, 2 * ziel, 1), 900);
    assert_eq!(naechster_grundpreis(800, 0, 1), 700);
    assert_eq!(naechster_grundpreis(1, ziel + 1, 1), 2, "bei eins bliebe er sonst für immer");
    assert_eq!(naechster_grundpreis(1, 0, 1), 1, "unter den Boden");
    assert_eq!(naechster_grundpreis(800, 0, 750), 750, "unter den Boden");
    assert_eq!(naechster_grundpreis(u64::MAX, 2 * ziel, 1), u64::MAX);
}

/// ⚑ **Der Topf der Validatoren geht nach Stake mal Stimmen** (2026-10-28):
/// A (Stake 1, zwei Unterschriften) und B (Stake 3, eine) teilen 2 zu 3;
/// C steht im Satz, stimmt nicht und bekommt nichts; ein Unterzeichner
/// ausserhalb des Satzes bekommt nichts. Gegenproben: ohne Stimmen und
/// ohne Satz bleibt der Topf stehen; die Zähler sind danach leer.
#[test]
fn der_topf_der_validatoren_geht_nach_stake_mal_stimmen() {
    use myl_consensus::validator::{VotingMember, VotingSet};
    let schluessel = |s: &str| {
        myl_types::bls::BlsSecretKey::key_gen(Hash::sha256(s.as_bytes()).as_bytes())
            .expect("Schlüssel")
            .public_key()
            .expect("pk")
    };
    let (a, b, c, d) = (schluessel("a"), schluessel("b"), schluessel("c"), schluessel("d"));
    let id = |pk: &myl_types::bls::BlsPublicKey| myl_types::ids::MinerId::aus_schluessel(pk);
    let satz = VotingSet::from_members(
        [(a, 1u64), (b, 3), (c, 5)]
            .into_iter()
            .map(|(pk, weight)| (id(&pk), VotingMember { pubkey: pk, weight }))
            .collect(),
    );
    let mut z = Kette::probestand().zustand().clone();
    z.gebuehren_validatoren = 1_001;
    z.pruefpunkt_stimmen.insert(id(&a), 2);
    z.pruefpunkt_stimmen.insert(id(&b), 1);
    z.pruefpunkt_stimmen.insert(id(&d), 7);
    Kette::topf_auszahlen(&mut z, Some(&satz));
    assert_eq!(z.account(&Address::aus_schluessel(&a)).balance, 400, "2 von 5");
    assert_eq!(z.account(&Address::aus_schluessel(&b)).balance, 600, "3 von 5");
    assert_eq!(z.account(&Address::aus_schluessel(&c)).balance, 0, "ohne Stimme");
    assert_eq!(z.account(&Address::aus_schluessel(&d)).balance, 0, "nicht im Satz");
    assert_eq!(z.gebuehren_validatoren, 1, "der Rest bleibt im Topf");
    assert!(z.pruefpunkt_stimmen.is_empty());
    // Gegenproben: ohne Stimmen, ohne Satz.
    z.gebuehren_validatoren = 500;
    Kette::topf_auszahlen(&mut z, Some(&satz));
    assert_eq!(z.gebuehren_validatoren, 500, "ohne Stimmen ausgezahlt");
    z.pruefpunkt_stimmen.insert(id(&a), 1);
    Kette::topf_auszahlen(&mut z, None);
    assert_eq!(z.gebuehren_validatoren, 500, "ohne Satz ausgezahlt");
}

/// ⚑ **Die Gebühren einer Epoche teilen sich drei zu zehn**: Treasury
/// und Topf der Validatoren; der Rest der Teilung geht in den Topf, und
/// keine Einheit verschwindet.
#[test]
fn die_gebuehren_teilen_sich_drei_zu_zehn() {
    let mut z = Kette::probestand().zustand().clone();
    let treasury = myl_types::treasury::treasury_adresse();
    let t0 = z.account(&treasury).balance;
    z.gebuehren_epoche = 1_301;
    Kette::gebuehren_teilen(&mut z);
    assert_eq!(z.account(&treasury).balance - t0, 300);
    assert_eq!(z.gebuehren_validatoren, 1_001);
    assert_eq!(z.gebuehren_epoche, 0);
}

/// Welches Probekonto der Koordinator dieses Pods ist.
///
/// ⚑ **Der Koordinator ist Position null**, und welcher
/// Probeschlüssel das ist, entscheidet die Saat. Ihn zu raten hieße,
/// den Test an eine Permutation zu binden, und seit dem Mischen
/// (Fund 142) ist es nicht mehr Konto null.
fn koordinator_von(pod: &myl_scheduler::shard_assignment::Pod) -> u8 {
    (0..PROBEKONTEN)
        .find(|w| {
            myl_types::ids::MinerId::new(*probekonto(*w).as_bytes())
                == pod.shards[0].miner.miner_id
        })
        .expect("der Koordinator ist ein Probekonto")
}

/// Baut eine Probekette mit **einem echten Pod** und liefert sie
/// samt einem gültig unterschriebenen Bündel dafür.
///
/// ⚑ **Seit Fund 144 braucht jeder Bündeltest das.** Vorher genügte
/// eine erfundene Pod-Kennung und eine Nullsignatur: Die Aufnahme
/// prüfte beides nicht. Jetzt prüft sie Koordinator und Aggregat,
/// und ein Test, der das umgeht, prüfte den Weg nicht mehr, den ein
/// echtes Bündel geht.
///
/// Gibt die Kette, das Bündel und die nächste freie Nummer des
/// Koordinators zurück.
fn kette_mit_pod(vtfe: u64, segmente: u32) -> (Kette, myl_types::PoIBundle, u8, u64) {
    use myl_types::miner::HardwareClass;

    let mut k = Kette::probestand();
    let mut nonce = [0u64; 6];
    for w in 0..6u8 {
        k.aufnehmen(anmeldung(w, HardwareClass::MediumGpu));
        nonce[w as usize] += 1;
        k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
        nonce[w as usize] += 1;
    }
    k.baue_block();
    assert_eq!(k.zustand().miner.len(), 6, "die Anmeldungen kamen nicht an");
    // ⚑ Die Zusagen gelten ab der nächsten Epoche; erst dort gibt es
    //   einen Pod (kein Pod ohne Zusage).
    while k.zustand().epoch.0 < 1 {
        k.baue_block();
    }

    let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    assert_eq!(zuteilung.pods.len(), 1, "es entstand kein Pod");
    let pod = &zuteilung.pods[0];
    let koordinator = koordinator_von(pod);

    let mut b = myl_types::PoIBundle {
        epoch: k.zustand().epoch,
        pod: myl_types::pod_kennung(k.zustand().epoch.0, pod.pod_index),
        segments_root: myl_types::ids::MerkleRoot::new([7; 32]),
        vtfe_claimed: vtfe,
        aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
        segmente,
        ausgabenwurzel: myl_types::MerkleRoot::new([0u8; 32]),
        vertreter: Vec::new(),
        sitzungen: Vec::new(),
    };
    buendel_unterschreiben(&mut b, pod);
    (k, b, koordinator, nonce[koordinator as usize])
}

/// ⚑ **Die Entlohnung rechnet auch der zustandslose Folger nach**
/// (zustandslose Validierung, 2026-10-29): sechs Miner melden sich an,
/// sagen zu, ein Pod reicht ein Bündel ein, zwei Epochenwechsel buchen
/// die Ausschüttung. Der Folger hält keine Konten und kommt mit den
/// Zeugen zur selben Wurzel; jeder Block war bezeugbar.
#[test]
fn die_entlohnung_rechnet_auch_der_zustandslose_folger_nach() {
    use myl_types::miner::HardwareClass;
    let mut k = Kette::probestand();
    let mut folger = Kette::probestand();
    let eigene: Vec<Address> = (0..6u8).map(probekonto).collect();
    folger.zustandslos_machen(&eigene);
    let weiter = |k: &mut Kette, folger: &mut Kette| {
        let b = k.baue_block();
        let z: myl_ledger::state::Zeuge = borsh::from_slice(&k.eigener_zeuge(&b.hash()).expect("bereitgehalten")).expect("lesbar");
        folger.uebernimm_mit_zeuge(&b, &z).expect("trägt");
    };
    let mut nonce = [0u64; 6];
    for w in 0..6u8 {
        k.aufnehmen(anmeldung(w, HardwareClass::MediumGpu));
        nonce[w as usize] += 1;
        k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
        nonce[w as usize] += 1;
        // Ohne Auszahlungskonto kein Anteil.
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::AuszahlungskontoEintragen {
                    kennung: myl_types::ids::MinerId::new(*probekonto(w).as_bytes()),
                    konto: probekonto(w),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
    }
    weiter(&mut k, &mut folger);
    while k.zustand().epoch.0 < 1 {
        weiter(&mut k, &mut folger);
    }
    let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    let pod = &zuteilung.pods[0];
    let koordinator = koordinator_von(pod);
    let mut b = myl_types::PoIBundle {
        epoch: k.zustand().epoch,
        pod: myl_types::pod_kennung(k.zustand().epoch.0, pod.pod_index),
        segments_root: myl_types::ids::MerkleRoot::new([7; 32]),
        vtfe_claimed: 4_200,
        aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
        segmente: 1,
        ausgabenwurzel: myl_types::MerkleRoot::new([0u8; 32]),
        vertreter: Vec::new(),
        sitzungen: Vec::new(),
    };
    buendel_unterschreiben(&mut b, pod);
    k.aufnehmen(einreichung(koordinator, nonce[koordinator as usize], b));
    k.aufnehmen(burn_nr(7, 1_000_000, 0));
    weiter(&mut k, &mut folger);
    assert_eq!(k.zustand().buendel.anzahl(), 1, "das Bündel kam nicht an");
    while k.zustand().epoch.0 < 2 {
        weiter(&mut k, &mut folger);
    }
    // Ein Pod allein: gutgeschrieben erst nach der Streitfrist
    // (Anlaufregel). Der Rückhalt steht im Kopf, den beide ganz halten;
    // die Frist wird hier auf beiden Seiten gleich verkürzt.
    let gehalten: u64 = k.zustand().zurueckgehalten.iter().map(|(_, _, v)| *v).sum();
    assert!(gehalten > 0, "nichts zugeschrieben");
    assert_eq!(folger.zustand().zurueckgehalten, k.zustand().zurueckgehalten);
    for z in [k.zustand_mut(), folger.zustand_mut()] {
        for e in z.zurueckgehalten.iter_mut() {
            e.0 = 2;
        }
    }
    // Geprägt wird gegen Verbranntes (Burn-and-Mint, kein Zuschuss):
    // Ein Nutzer verbrennt in dieser Epoche.
    k.aufnehmen(burn_nr(7, 1_000_000, 1));
    let vorher: u128 = (0..6u8).map(|w| u128::from(k.zustand().account(&probekonto(w)).balance)).sum();
    while k.zustand().epoch.0 < 3 {
        weiter(&mut k, &mut folger);
    }
    assert!(k.zustand().zurueckgehalten.is_empty(), "nicht freigegeben");
    let nachher: u128 = (0..6u8).map(|w| u128::from(k.zustand().account(&probekonto(w)).balance)).sum();
    assert!(nachher > vorher, "keine Ausschüttung an die Miner: {vorher} → {nachher}");
    // Der zustandslose Miner sieht seinen Lohn auf dem eigenen Konto.
    for a in &eigene {
        assert_eq!(folger.konto(a), k.zustand().account(a));
    }
    assert_eq!(folger.zustandswurzel(), k.zustandswurzel(), "die Ausschüttung weicht ab");
    assert_eq!(k.zeugenlos(), 0, "ein Block war nicht bezeugbar");
    assert_eq!(folger.zustand().accounts.anzahl(), 0);
}

/// Die Transaktion, die ein Bündel einreicht.
fn einreichung(wer: u8, nonce: u64, buendel: myl_types::PoIBundle) -> Transaktion {
    Transaktion::signiere(
        &Kette::startwert(),
        &probeschluessel(wer),
        nonce,
        Anweisung::BuendelEinreichen { buendel },
    )
    .expect("signieren")
}

/// ⚑ **Der Einsatz über die Kette: hinterlegen, kündigen, holen**
/// (Punkt B11, Fund 145).
///
/// Bis zum 2026-09-03 war `staked` im Betrieb immer null, weil
/// keine Anweisung es schrieb. Der Test geht den ganzen Weg über
/// echte, unterschriebene Transaktionen.
#[test]
fn ein_einsatz_geht_ueber_die_kette() {
    let mut k = Kette::probestand();
    let vorher = k.zustand().account(&probekonto(0)).balance;
    assert!(vorher > 0, "das Probekonto hat kein Guthaben");
    // Die Gebühren laufen mit (G1).
    let mut gezahlt = 0;
    let mut senden = |k: &mut Kette, nonce: u64, anweisung: Anweisung| {
        let tx = Transaktion::signiere(&Kette::startwert(), &probeschluessel(0), nonce, anweisung).expect("signieren");
        gezahlt += gebuehr(&tx);
        k.aufnehmen(tx);
        k.baue_block();
        gezahlt
    };

    let g = senden(&mut k, 0, Anweisung::EinsatzHinterlegen { betrag: 5_000 });
    assert_eq!(
        k.zustand().account(&probekonto(0)).staked,
        5_000,
        "der Einsatz kam nicht an"
    );
    assert_eq!(k.zustand().account(&probekonto(0)).balance, vorher - 5_000 - g);

    // Kuendigen: aus `staked` heraus, aber noch nicht ins Guthaben.
    let g = senden(&mut k, 1, Anweisung::EinsatzKuendigen { betrag: 2_000 });
    let konto = k.zustand().account(&probekonto(0));
    assert_eq!(konto.staked, 3_000);
    assert_eq!(konto.balance, vorher - 5_000 - g, "das Geld kam zu frueh zurueck");
    assert_eq!(konto.gekuendigt.values().sum::<u64>(), 2_000);

    // Abholen vor der Freigabe bewirkt nichts.
    let g = senden(&mut k, 2, Anweisung::EinsatzAbholen);
    assert_eq!(
        k.zustand().account(&probekonto(0)).balance,
        vorher - 5_000 - g,
        "vor der Freigabe wurde ausgezahlt"
    );

    // ⚑ **Die Reife wird hier nicht geprüft, und der Grund ist
    // lehrreich.** Der erste Anlauf stellte `zustand.epoch` von
    // Hand auf die Freigabe-Epoche. Das hält nicht: `anwenden`
    // setzt die Epoche aus der **Blockhöhe**, bevor es die
    // Transaktionen anwendet, und überschreibt die gestellte
    // Zahl. **Das ist richtig so** und genau die Regel, die
    // Erzeuger und Übernehmer zusammenhält.
    //
    // Die Sperrfrist über echte Blöcke abzuwarten hiesse 168
    // Epochen zu bauen. **Die Reife steht deshalb dort, wo sie
    // hingehört**, bei den Übergängen: `gekuendigtes_liegt_bis_zur_freigabe`
    // in `myl-ledger` prüft sie samt Gegenprobe. Dieser Test hier
    // prüft, was nur er prüfen kann: dass die Anweisungen über
    // eine unterschriebene Transaktion in den Zustand gelangen.
    let konto = k.zustand().account(&probekonto(0));
    assert_eq!(konto.staked, 3_000, "der ungekuendigte Rest wurde angefasst");
}

/// ⚑ **Der Weg des Nutzers, von der Kette bis zum Beleg**
/// (B6-3, Stufe 4, erster Schnitt).
///
/// Ein Kontrakt kommt über eine unterschriebene Transaktion in die
/// Kette, der Knoten frischt seine Abschrift auf, und die Tür lässt
/// eine Vollmacht durch, die auf **diesen** Kontrakt zeigt.
///
/// **Der Test bleibt in einem Prozess**, denn die Tür über einen
/// echten Socket ist im Gateway geprüft; was hier geprüft wird, ist
/// die **Naht**: dass der Kontrakt aus der Kette dort ankommt, wo
/// die Tür ihn sucht. ⚑ **Genau diese Naht hat in dieser Woche
/// siebenmal gefehlt.**
#[test]
fn ein_kontrakt_aus_der_kette_erreicht_die_tuer() {
    use myl_gateway::zugang::Kontraktquelle;
    use myl_types::sitzung::{Grenzen, Sitzungskontrakt};

    let mut k = Kette::probestand();
    let agent = probeschluessel(1).public_key().expect("pk");
    let kontrakt = Sitzungskontrakt {
        inhaber: probekonto(0),
        agent: Address::aus_schluessel(&agent),
        credits: Grenzen {
            budget: 10_000,
            einzellimit: 1_000,
            schwelle: u64::MAX,
            zeugenleiter: Vec::new(),
        },
        myl: Grenzen::gesperrt(),
        empfaenger_wurzel: myl_types::sitzung::empfaengerwurzel(&[probekonto(2)]),
        gueltig_ab: EpochId(0),
        gueltig_bis: EpochId(100),
        max_schritte: 1_000,
    };
    let id = kontrakt.adresse();

    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(0),
            0,
            Anweisung::SitzungEroeffnen {
                kontrakt: kontrakt.clone(),
            },
        )
        .expect("signieren"),
    );
    k.baue_block();
    assert!(
        k.zustand().sitzung(&id).is_some(),
        "der Kontrakt kam nicht in die Kette"
    );

    // Der Knoten frischt die Abschrift auf, die Tür liest sie.
    let abschrift = crate::tuer::Kontraktabschrift::neu();
    abschrift.setzen(k.zustand());
    let (gefunden, _) = abschrift
        .nachschlagen(id)
        .expect("die Tuer findet den Kontrakt nicht");
    assert_eq!(gefunden, kontrakt);

    // ⚑ **Und eine Vollmacht darauf geht durch.** Das ist der
    // ganze Weg: Kette, Abschrift, Zugangsstelle, Befund.
    let vollmacht = myl_gateway::vollmacht::Vollmacht::ausstellen(
        &probeschluessel(1),
        vec![myl_gateway::vollmacht::Vorbehalt::NurSitzung(id)],
        [5u8; 32],
    )
    .expect("ausstellen");
    let rahmen = myl_gateway::vollmacht::Anfragerahmen {
        jetzt: EpochId(0),
        sitzung: id,
        credits: 1,
        modell: Hash::sha256(b"egal"),
    };
    let mut stelle = myl_gateway::zugang::Zugangsstelle::neu(abschrift.clone());
    assert_eq!(
        stelle.durchlassen_mit_vollmacht(&vollmacht, &rahmen, 1_000),
        myl_gateway::zugang::Zugangsbefund::Erlaubt,
        "die Vollmacht auf einen Kettenkontrakt wurde abgewiesen"
    );

    // ⚑ **Gegenprobe: ein Widerruf in der Kette schliesst die Tür**,
    // sobald die Abschrift aufgefrischt ist.
    k.zustand_mut()
        .sitzungen
        .get_mut(&id)
        .expect("da")
        .zustand
        .widerrufen = true;
    abschrift.setzen(k.zustand());
    let mut stelle = myl_gateway::zugang::Zugangsstelle::neu(abschrift);
    assert_eq!(
        stelle.durchlassen_mit_vollmacht(&vollmacht, &rahmen, 1_000),
        myl_gateway::zugang::Zugangsbefund::Abgelehnt,
        "der Widerruf erreichte die Tuer nicht"
    );
}

/// ⚑ **Punkt 40, Glied 1 in der Kette:** Ein Bündel eines
/// angemeldeten Miners kommt in den Zustand.
#[test]
fn ein_buendel_ueber_die_kette_kommt_an() {
    let (mut k, b, koordinator, nonce) = kette_mit_pod(4_200, 1);
    k.aufnehmen(einreichung(koordinator, nonce, b));
    k.baue_block();
    assert_eq!(k.zustand().buendel.len(), 1);
}

/// ⚑ **Gegenprobe zur Aufnahme: ein erfundener Pod kommt nicht
/// hinein** (Fund 144).
///
/// Bis zum 2026-09-02 kam er hinein und blieb bis zum
/// Epochenwechsel im Zustand. Die Kennung wählt der Einreichende
/// frei, also war das ein Weg, den Zustand wachsen zu lassen.
#[test]
fn ein_erfundener_pod_kommt_nicht_in_den_zustand() {
    let (mut k, b, koordinator, nonce) = kette_mit_pod(4_200, 1);

    // ⚑ **Die Kennung wird geändert und danach neu unterschrieben.**
    // Die Kennung steht in der Signierbotschaft; wer sie nach dem
    // Unterschreiben ändert, scheitert schon am Aggregat, und dann
    // prüfte dieser Test die Signatur und nicht die Kennungssuche.
    // Der erste Anlauf hat genau daran gelegen: Er bestand auch
    // dann, wenn man die Suche ausbaute.
    let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    let mut erfunden = b.clone();
    erfunden.pod = myl_types::ids::PodId::new([3; 32]);
    buendel_unterschreiben(&mut erfunden, &zuteilung.pods[0]);

    k.aufnehmen(einreichung(koordinator, nonce, erfunden));
    k.baue_block();
    assert!(
        k.zustand().buendel.is_empty(),
        "ein Buendel fuer einen Pod, den es nicht gibt, steht im Zustand"
    );

    // Gegenprobe zur Gegenprobe: Dasselbe Bündel mit der **echten**
    // Kennung geht durch. Sonst bewiese der Test nur, dass
    // irgendetwas scheitert.
    k.aufnehmen(einreichung(koordinator, nonce + 1, b));
    k.baue_block();
    assert_eq!(
        k.zustand().buendel.len(),
        1,
        "auch mit der echten Kennung kam nichts an"
    );
}

/// ⚑ **Und eine Attrappe als Unterschrift auch nicht** (Fund 144).
#[test]
fn eine_attrappe_kommt_nicht_in_den_zustand() {
    let (mut k, mut b, koordinator, nonce) = kette_mit_pod(4_200, 1);
    b.aggregate_sig = myl_types::bls::BlsSignature([0; 96]);
    k.aufnehmen(einreichung(koordinator, nonce, b));
    k.baue_block();
    assert!(
        k.zustand().buendel.is_empty(),
        "ein unsigniertes Buendel steht im Zustand"
    );
}

/// ⚑ **Nur der Koordinator reicht ein** (Fund 144).
#[test]
fn ein_anderes_mitglied_reicht_nicht_ein() {
    let (mut k, b, koordinator, _) = kette_mit_pod(4_200, 1);
    let anderer = (koordinator + 1) % 6;
    k.aufnehmen(einreichung(anderer, 1, b));
    k.baue_block();
    assert!(
        k.zustand().buendel.is_empty(),
        "ein Mitglied ohne Koordinatorrolle hat eingereicht"
    );
}

/// ⚑ **Und am Epochenwechsel fallen sie weg.** Ohne das wüchse der
/// Zustand unbegrenzt; die Historie steht in den Blöcken.
#[test]
fn am_epochenwechsel_fallen_die_buendel_weg() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    let (mut k, b, koordinator, nonce) = kette_mit_pod(4_200, 1);
    k.aufnehmen(einreichung(koordinator, nonce, b));
    k.baue_block();
    assert_eq!(k.zustand().buendel.len(), 1, "das Buendel kam nicht an");
    let start = k.zustand().epoch.0;
    for _ in 0..BLOECKE_JE_EPOCHE * 2 {
        k.baue_block();
        if k.zustand().epoch.0 == start + 1 {
            break;
        }
    }
    assert_eq!(k.zustand().epoch.0, start + 1, "die Epoche wechselte nicht");
    assert!(k.zustand().buendel.is_empty(), "die Buendel blieben stehen");
}

/// Unterschreibt ein Bündel mit den **Positionen** seines Pods.
///
/// ⚑ **Nur die Positionen, seit Fund 645** (2026-10-26): Die Reserve
/// rechnet nicht und kann nicht prüfen, was sie unterschriebe. Geprüft
/// wird gegen genau diese Menge ([`Kette::unterzeichner_des_buendels`]).
fn buendel_unterschreiben(
    buendel: &mut myl_types::PoIBundle,
    pod: &myl_scheduler::shard_assignment::Pod,
) {
    let msg = myl_consensus::poi::bundle_message(buendel);
    let mut teile = Vec::new();
    for m in pod.shards.iter().map(|s| &s.miner) {
        // Welcher Probeschlüssel gehört zu dieser Kennung?
        let w = (0..PROBEKONTEN)
            .find(|w| {
                myl_types::ids::MinerId::new(*probekonto(*w).as_bytes()) == m.miner_id
            })
            .expect("Mitglied ist ein Probekonto");
        teile.push(probeschluessel(w).sign(&msg).expect("Unterschrift"));
    }
    let aggregat =
        myl_types::bls::aggregate_signatures(&teile).expect("Aggregat");
    buendel.aggregate_sig = myl_types::bls::BlsSignature(aggregat.0);
}

/// Das Zertifikat des Prüfpunkts an `hoehe` über den eigenen Block,
/// unterschrieben vom Probeerzeuger, dem einzigen Mitglied einer
/// Probekette.
fn pruefpunkt_von(k: &Kette, hoehe: u64, wer: &str) -> myl_consensus::pruefpunkt::Pruefpunktzertifikat {
    pruefpunkt_mit(k, hoehe, wer, k.quelle_an(hoehe).expect("die Quelle ist vermerkt"))
}

/// Wie [`pruefpunkt_von`], mit einer beliebigen Quelle.
fn pruefpunkt_mit(
    k: &Kette,
    hoehe: u64,
    wer: &str,
    quelle: myl_consensus::pruefpunkt::Quelle,
) -> myl_consensus::pruefpunkt::Pruefpunktzertifikat {
    let schluessel = crate::schluessel::Konsensschluessel::probe(wer).expect("Schlüssel");
    let block_hash = k.hash_an(hoehe).expect("der Block liegt im Fenster");
    let bindung = myl_ledger::stimmsatz::bindung(k.zustand(), epoche_fuer_hoehe(hoehe));
    let signatur = schluessel
        .signiere(&myl_consensus::pruefpunkt::pruefpunktnachricht(hoehe, &block_hash, &bindung, &quelle))
        .expect("unterschreiben");
    let stimme =
        myl_consensus::pruefpunkt::Pruefpunktstimme { hoehe, block_hash, bindung, quelle, waehler: schluessel.kennung(), signatur };
    myl_consensus::pruefpunkt::Pruefpunktzertifikat::aus_stimmen(&[stimme]).expect("Zertifikat")
}

/// ⚑ **Ein Anker aus einem Zertifikat nur gegen den eigenen Stimmsatz**
/// (leichter Abzug, 2026-10-29): Ohne Satz keiner; mit Satz das echte
/// Zertifikat, nicht eines eines Fremden, nicht eines mit verändertem
/// Blockhash. ⚑ Seit 2026-11-04 ist der Anker die Quelle eines
/// Zertifikats, das sie endgültig macht; eines von Genesis aus ist keiner.
#[test]
fn ein_anker_kommt_nur_aus_einem_tragenden_zertifikat() {
    use myl_consensus::pruefpunkt::BLOECKE_JE_PRUEFPUNKT;
    let mut k = Kette::probestand();
    for _ in 0..BLOECKE_JE_PRUEFPUNKT {
        k.baue_block();
    }
    let erstes = pruefpunkt_von(&k, BLOECKE_JE_PRUEFPUNKT, PROBEERZEUGER);
    assert_eq!(k.anker_aus_zertifikat(&erstes).err(), Some(crate::wechsel::Folgefehler::Offen(0)), "nur gerechtfertigt");
    k.pruefpunkt_anbieten(erstes);
    for _ in 0..BLOECKE_JE_PRUEFPUNKT {
        k.baue_block();
    }
    let z = pruefpunkt_von(&k, 2 * BLOECKE_JE_PRUEFPUNKT, PROBEERZEUGER);
    assert_eq!(z.quelle.hoehe, BLOECKE_JE_PRUEFPUNKT, "die Quelle ist der gerechtfertigte");
    let mut ohne = Kette::probestand();
    ohne.stimmsatz = None;
    assert!(ohne.anker_aus_zertifikat(&z).is_err(), "ohne Stimmsatz kein Anker");
    let a = k.anker_aus_zertifikat(&z).expect("trägt gegen den eigenen Satz");
    assert_eq!((a.hoehe, a.blockhash), (BLOECKE_JE_PRUEFPUNKT, k.hash_an(BLOECKE_JE_PRUEFPUNKT).unwrap()));
    assert!(k.anker_aus_zertifikat(&pruefpunkt_von(&k, 2 * BLOECKE_JE_PRUEFPUNKT, "fremder")).is_err(), "ein Fremder");
    let mut falsch = z.clone();
    falsch.quelle.block_hash = Hash::sha256(b"anderer Block");
    assert!(k.anker_aus_zertifikat(&falsch).is_err(), "anderer Blockhash der Quelle");
}

/// Baut `n` Blöcke auf `k` und übernimmt sie in `folger`.
fn bauen_und_folgen(k: &mut Kette, folger: &mut Kette, n: usize) {
    for _ in 0..n {
        let b = k.baue_block();
        folger.uebernimm(&b).expect("ein gewöhnlicher Block");
    }
}

/// ⚑ **Ein Prüfpunkt wird bestätigt** (2026-10-27, CONSENSUS 7.2): Der
/// Erzeuger legt das Zertifikat in den nächsten Block, der Folger nimmt
/// es an, und beide stehen danach bei Höhe 150. Gegenproben: dasselbe
/// Zertifikat ein zweites Mal, keine Prüfpunkthöhe, ein anderer Block,
/// ein Fremder als Unterzeichner.
#[test]
fn ein_pruefpunkt_wird_bestaetigt() {
    let mut k = Kette::probestand();
    let mut folger = Kette::probestand();
    bauen_und_folgen(&mut k, &mut folger, 150);
    let z = pruefpunkt_von(&k, 150, PROBEERZEUGER);
    k.pruefpunkt_anbieten(z.clone());
    let mit = k.baue_block();
    assert_eq!(mit.header.pruefpunkt.as_ref(), Some(&z), "der Erzeuger legte das Zertifikat nicht in den Block");
    folger.uebernimm(&mit).expect("ein Block mit tragendem Zertifikat");
    // ⚑ Seit 2026-11-04 gerechtfertigt, noch nicht
    //   endgültig; das macht erst das Zertifikat von 300 mit 150 als
    //   Quelle.
    assert_eq!((k.gerechtfertigt_bis(), folger.gerechtfertigt_bis()), (150, 150));
    assert_eq!((k.bestaetigt_bis(), folger.bestaetigt_bis()), (0, 0));
    bauen_und_folgen(&mut k, &mut folger, 149);
    // ⛔️ Eine andere als die vermerkte Quelle trägt nicht, auch richtig
    //   unterschrieben.
    let genesis = pruefpunkt_mit(&k, 300, PROBEERZEUGER, myl_consensus::pruefpunkt::Quelle::GENESIS);
    let ohne = k.baue_block();
    let mut falsch = ohne.clone();
    falsch.header.pruefpunkt = Some(genesis);
    neu_unterschreiben(&mut falsch);
    assert_eq!(folger.uebernimm(&falsch), Err(KettenFehler::PruefpunktTraegtNicht), "andere Quelle");
    folger.uebernimm(&ohne).expect("derselbe Block ohne Zertifikat");
    let z300 = pruefpunkt_von(&k, 300, PROBEERZEUGER);
    assert_eq!(z300.quelle.hoehe, 150);
    k.pruefpunkt_anbieten(z300);
    folger.uebernimm(&k.baue_block()).expect("ein Block mit tragendem Zertifikat");
    assert_eq!((k.bestaetigt_bis(), folger.bestaetigt_bis()), (150, 150));
    assert_eq!(folger.gerechtfertigt_bis(), 300);

    // ⛔️ Dasselbe Zertifikat noch einmal: nichts Neues, abgelehnt.
    let mut nochmal = k.baue_block();
    nochmal.header.pruefpunkt = Some(z);
    neu_unterschreiben(&mut nochmal);
    assert_eq!(folger.uebernimm(&nochmal), Err(KettenFehler::PruefpunktTraegtNicht), "zweimal dasselbe");

    // Je Gegenprobe ein frisches Paar bei Höhe 150 und ein Block 151
    // mit einem veränderten Zertifikat.
    let pruefe = |aendern: &dyn Fn(&Kette) -> myl_consensus::pruefpunkt::Pruefpunktzertifikat| {
        let mut erz = Kette::probestand();
        let mut neu = Kette::probestand();
        bauen_und_folgen(&mut erz, &mut neu, 150);
        let zert = aendern(&erz);
        let mut b = erz.baue_block();
        b.header.pruefpunkt = Some(zert);
        neu_unterschreiben(&mut b);
        neu.uebernimm(&b)
    };
    // ⛔️ Ein richtiges Zertifikat, nachträglich in einen fertigen Block
    // gesetzt: Es schreibt den bestätigten Stand fort, die Wurzel im Kopf
    // weiss davon nichts.
    assert!(matches!(pruefe(&|e| pruefpunkt_von(e, 150, PROBEERZEUGER)), Err(KettenFehler::ZustandWeichtAb { .. })));
    // ⛔️ Ein anderer Block an Höhe 150.
    let anderer = |e: &Kette| {
        let mut x = pruefpunkt_von(e, 150, PROBEERZEUGER);
        x.block_hash = Hash::sha256(b"ein anderer Block");
        x
    };
    assert_eq!(pruefe(&anderer), Err(KettenFehler::PruefpunktTraegtNicht), "anderer Block");
    // ⛔️ Ein Fremder unterschreibt.
    assert_eq!(pruefe(&|e| pruefpunkt_von(e, 150, "fremder")), Err(KettenFehler::PruefpunktTraegtNicht), "Fremder");
    // ⛔️ Keine Prüfpunkthöhe.
    let schief = |e: &Kette| {
        let mut x = pruefpunkt_von(e, 150, PROBEERZEUGER);
        x.hoehe = 149;
        x
    };
    assert_eq!(pruefe(&schief), Err(KettenFehler::PruefpunktTraegtNicht), "keine Prüfpunkthöhe");
}

/// ⚑ **Zurückrollen und einem anderen Zweig folgen** (CONSENSUS 7.7
/// Schritt 2, 2026-11-04): Drei Blöcke zurück stehen Höhe, letzter Hash
/// und Wurzel wie vorher, und die Transaktion wartet wieder; danach
/// folgt der Folger einem neu gebauten Zweig bis zur selben Wurzel wie
/// der Erzeuger. ⛔️ Nie unter den endgültigen Prüfpunkt, und ohne
/// Rückweg (nach einem Neustart) nicht unter die eigene Höhe.
#[test]
fn zurueckrollen_und_einem_anderen_zweig_folgen() {
    let mut k = Kette::probestand();
    let mut folger = Kette::probestand();
    bauen_und_folgen(&mut k, &mut folger, 10);
    let (hash10, wurzel10) = (folger.letzter_hash(), folger.wurzel());
    assert!(k.aufnehmen(burn(1, 1_000)));
    bauen_und_folgen(&mut k, &mut folger, 3);
    assert_eq!(folger.wartend(), 0);
    assert_ne!(folger.wurzel(), wurzel10);
    assert_eq!(folger.zurueckrollen_bis(10), Ok(3));
    assert_eq!((folger.hoehe(), folger.letzter_hash()), (10, hash10));
    assert_eq!(folger.wurzel(), wurzel10);
    assert_eq!(folger.zustandswurzel(), wurzel10, "auch neu gerechnet");
    assert_eq!(folger.wartend(), 1, "die Transaktion wartet wieder");
    assert!(folger.block_an(11).is_none());
    assert_eq!(k.zurueckrollen_bis(10), Ok(3));
    bauen_und_folgen(&mut k, &mut folger, 4);
    assert_eq!(folger.hoehe(), 14);
    assert_eq!(folger.wurzel(), k.wurzel());
    assert_eq!(folger.wartend(), 0, "der neue Zweig trägt sie");

    // ⛔️ Der Boden: 150 endgültig durch das Zertifikat von 300.
    bauen_und_folgen(&mut k, &mut folger, 136);
    let z150 = pruefpunkt_von(&k, 150, PROBEERZEUGER);
    k.pruefpunkt_anbieten(z150);
    bauen_und_folgen(&mut k, &mut folger, 150);
    let z300 = pruefpunkt_von(&k, 300, PROBEERZEUGER);
    k.pruefpunkt_anbieten(z300);
    bauen_und_folgen(&mut k, &mut folger, 5);
    assert_eq!(folger.bestaetigt_bis(), 150);
    assert_eq!(folger.rueckweg_boden(), 150);
    // Fund 634: die Schranke der Höhe ab dem endgültigen Block.
    let s150 = folger.bloecke_von_bis(150, 150)[0].header.slot;
    assert_eq!(folger.hoehen_schranke(s150 + 10), 160);
    // ⚑ Die Vertrauensfrist misst dieselbe Schranke, ohne fremde Angabe (A46).
    let e = BLOECKE_JE_EPOCHE;
    assert!(folger.vertrauen_traegt_jetzt(s150 + 112 * e), "112 Epochen tragen");
    assert!(!folger.vertrauen_traegt_jetzt(s150 + 113 * e), "eine zu viel");
    assert!(Kette::probestand().vertrauen_traegt_jetzt(u64::MAX), "ohne endgültigen Prüfpunkt nichts zu prüfen");
    assert_eq!(Kette::probestand().hoehen_schranke(u64::MAX), u64::MAX, "ohne Endgültigkeit keine");
    assert_eq!(folger.zurueckrollen_bis(149), Err(150), "nie unter den endgültigen");
    assert_eq!(folger.hoehe(), 305, "nichts geändert");
    // Bis an ihn heran geht es. Der Zustand steht dann wie nach Block
    // 150, vor dem Block, der dessen Zertifikat trug; der Boden bleibt
    // trotzdem 150, weil die Rückwege darunter weg sind.
    assert_eq!(folger.zurueckrollen_bis(150), Ok(155));
    assert_eq!((folger.gerechtfertigt_bis(), folger.bestaetigt_bis()), (0, 0));
    assert_eq!(folger.rueckweg_boden(), 150, "die Rückwege darunter sind weg");
    assert_eq!(folger.zurueckrollen_bis(149), Err(150));
    let (mut erz, mut neu) = (Kette::probestand(), Kette::probestand());
    assert_eq!(neu.zurueckrollen_bis(0), Ok(0));
    bauen_und_folgen(&mut erz, &mut neu, 2);
    neu.rueckwege.clear();
    assert_eq!(neu.zurueckrollen_bis(1), Err(2), "ohne Rückweg nicht");
}

/// ⚑ **Die Wahl des Zweigs** (CONSENSUS 7.7 Schritt 4, 2026-11-04):
/// Der längere Zweig gewinnt, ein kürzerer oder gleich langer nicht; der
/// höhere gerechtfertigte Prüfpunkt geht vor der Länge; ein Zweig, dessen
/// Block nicht trägt, lässt die Kette, wie sie war; und unter den Boden
/// geht es nie.
#[test]
fn die_wahl_des_zweigs() {
    let mut a = Kette::probestand();
    let mut b = Kette::probestand();
    bauen_und_folgen(&mut a, &mut b, 148);
    assert!(a.aufnehmen(burn(1, 1_000)));
    a.baue_block();
    a.baue_block();
    assert!(b.aufnehmen(burn(2, 2_000)));
    for _ in 0..3 {
        b.baue_block();
    }
    let zweig_a = a.bloecke_von_bis(149, 150);
    let alles_b = b.bloecke_von_bis(140, 151);
    // Kürzer: Die Kette bleibt, unberührt.
    let vorher = (b.hoehe(), b.letzter_hash(), b.wurzel());
    assert_eq!(b.zweig_folgen(&zweig_a), Ok(Zweigwahl::Geblieben));
    assert_eq!((b.hoehe(), b.letzter_hash(), b.wurzel()), vorher);
    // Länger: gewechselt, der gemeinsame Anfang übersprungen.
    assert_eq!(a.zweig_folgen(&alles_b), Ok(Zweigwahl::Gewechselt { gabel: 148, zurueck: 2, neu: 3 }));
    assert_eq!((a.hoehe(), a.letzter_hash(), a.wurzel()), vorher);
    assert_eq!(a.zustandswurzel(), b.wurzel(), "auch neu gerechnet");
    assert_eq!(a.zweig_folgen(&alles_b), Err(Zweigfehler::Leer));
    // Gleich lang: Die Kette bleibt.
    let mut c = Kette::probestand();
    for x in a.bloecke_von_bis(1, 148) {
        c.uebernimm(&x).expect("der gemeinsame Anfang");
    }
    assert!(c.aufnehmen(burn(3, 3_000)));
    for _ in 0..3 {
        c.baue_block();
    }
    let zweig_c = c.bloecke_von_bis(149, 151);
    assert_eq!(a.zweig_folgen(&zweig_c), Ok(Zweigwahl::Geblieben));
    assert_eq!(a.letzter_hash(), vorher.1);
    // ⛔️ Ein längerer Zweig, dessen letzter Block nicht trägt: zurück
    // auf den eigenen, mit Befund.
    c.baue_block();
    let mut falsch = c.bloecke_von_bis(149, 152);
    falsch[3].txs.push(burn(4, 1));
    assert!(matches!(a.zweig_folgen(&falsch), Err(Zweigfehler::TraegtNicht(152, KettenFehler::ErzeugerUnbelegt))));
    assert_eq!((a.hoehe(), a.letzter_hash(), a.wurzel()), vorher);
    assert_eq!(a.zustandswurzel(), vorher.2);
    assert_eq!(a.zweig_folgen(&falsch[1..]), Err(Zweigfehler::PasstNirgends));

    // ⚑ Der höhere gerechtfertigte Prüfpunkt vor der Länge: Ein Zweig
    // mit dem Zertifikat von 150 schlägt einen um zwei Blöcke längeren
    // ohne.
    let mut d = Kette::probestand();
    let mut e = Kette::probestand();
    bauen_und_folgen(&mut d, &mut e, 150);
    let z150 = pruefpunkt_von(&d, 150, PROBEERZEUGER);
    d.pruefpunkt_anbieten(z150);
    d.baue_block();
    assert_eq!(d.gerechtfertigt_bis(), 150);
    for _ in 0..3 {
        e.baue_block();
    }
    let mit_zertifikat = d.bloecke_von_bis(151, 151);
    let laenger = e.bloecke_von_bis(151, 153);
    assert_eq!(d.zweig_folgen(&laenger), Ok(Zweigwahl::Geblieben));
    assert_eq!(e.zweig_folgen(&mit_zertifikat), Ok(Zweigwahl::Gewechselt { gabel: 150, zurueck: 3, neu: 1 }));
    assert_eq!((e.hoehe(), e.gerechtfertigt_bis(), e.wurzel()), (151, 150, d.wurzel()));
    // ⛔️ Unter den Boden nie: ohne Rückwege (nach einem Neustart) steht
    // er an der eigenen Höhe.
    e.rueckwege.clear();
    assert_eq!(e.zweig_folgen(&laenger), Err(Zweigfehler::ZuTief(151)));
    assert_eq!(e.hoehe(), 151);
}

/// 📌 **Ein fremder Block an der nächsten Höhe passt nicht an, er ist
/// nicht unbelegt** (Nachtlauf `nacht-rotation`, 2026-11-04): Der eigene
/// Zweig ist weiter in der Zeit als der fremde, also liegt der Slot des
/// fremden Blocks unter dem eigenen letzten. Der Plan gilt nur für einen
/// Block, der an die eigene Spitze anschliesst; sonst meldete die Kette
/// `ErzeugerUnbelegt`, der Knoten erkannte die Gabel nie und baute mit
/// wenigen anderen auf einem Zweig der Minderheit weiter.
#[test]
fn ein_fremder_zweig_passt_nicht_an() {
    let mut a = Kette::probestand();
    let mut b = Kette::probestand();
    bauen_und_folgen(&mut a, &mut b, 10);
    let s = a.letzter_slot();
    a.baue_block_im_slot(s + 1);
    a.baue_block_im_slot(s + 100);
    for i in 2..5 {
        b.baue_block_im_slot(s + i);
    }
    let fremd = b.bloecke_von_bis(13, 13).remove(0);
    assert!(matches!(a.uebernimm(&fremd), Err(KettenFehler::PasstNichtAn { .. })));
    assert_eq!(a.zweig_folgen(&b.bloecke_von_bis(11, 13)), Ok(Zweigwahl::Gewechselt { gabel: 10, zurueck: 2, neu: 3 }));
    assert_eq!(a.wurzel(), b.wurzel());
    // Ein Block, der anschliesst, braucht weiter einen Slot nach dem des
    // Kopfs.
    let (mut c, mut d) = (Kette::probestand(), Kette::probestand());
    for x in b.bloecke_von_bis(1, 12) {
        c.uebernimm(&x).expect("der gemeinsame Anfang");
        d.uebernimm(&x).expect("der gemeinsame Anfang");
    }
    let frueh = c.baue_block_im_slot(s + 50);
    d.zustand.letzter_slot = s + 60;
    assert!(matches!(d.uebernimm(&frueh), Err(KettenFehler::ErzeugerUnbelegt)), "Slot nicht nach dem des Kopfs");
}

/// 📌 **Eine Gabel an Block 1** (Podlauf `kurz-faelscher`, 2026-11-06):
/// Ein Validator baute nach einem späten Start Block 1 selbst. Der
/// gemeinsame Vorgänger ist Genesis; flach trägt der Rückweg von Block 1
/// ihn, ohne Rückwege hiess es `PasstNirgends`, obwohl der Startwert
/// feststeht. Jetzt ist es der Boden der Rückwege, wie bei jeder anderen
/// Gabel unter ihnen.
#[test]
fn eine_gabel_an_block_eins() {
    let mut a = Kette::probestand();
    let mut b = Kette::probestand();
    assert!(a.aufnehmen(burn(3, 3_000)));
    for _ in 0..3 {
        a.baue_block();
    }
    for _ in 0..5 {
        b.baue_block();
    }
    assert_ne!(a.bloecke_von_bis(1, 1)[0].hash(), b.bloecke_von_bis(1, 1)[0].hash(), "verschieden ab Block 1");
    let zweig = b.bloecke_von_bis(1, 5);
    let mut ohne = Kette::probestand();
    assert!(ohne.aufnehmen(burn(3, 3_000)));
    for _ in 0..3 {
        ohne.baue_block();
    }
    assert_eq!(a.zweig_folgen(&zweig), Ok(Zweigwahl::Gewechselt { gabel: 0, zurueck: 3, neu: 5 }));
    assert_eq!(a.wurzel(), b.wurzel());
    ohne.rueckwege.clear();
    assert_eq!(ohne.zweig_folgen(&zweig), Err(Zweigfehler::ZuTief(3)), "ohne Rückwege der Boden, nicht PasstNirgends");
}

/// ⚑ **Die Saat kommt aus dem Zertifikat** (2026-10-27): Zwei
/// Zertifikate mit verschiedenen Unterzeichnern ziehen verschieden, ein
/// und dasselbe zieht gleich, und jeder Knoten liest es aus dem Block.
#[test]
fn die_saat_kommt_aus_dem_zertifikat() {
    let anteile = vec![(myl_types::ids::PodId::new([5; 32]), 1_000u32)];
    let mut k = Kette::probestand();
    for _ in 0..150 {
        k.baue_block();
    }
    let a = pruefpunkt_von(&k, 150, PROBEERZEUGER);
    let b = pruefpunkt_von(&k, 150, "fremder");
    let zieh = |z: &myl_consensus::pruefpunkt::Pruefpunktzertifikat| {
        let saat = crate::stichprobe::saat_aus_zertifikat(&z.aggregat, &z.unterzeichner, 0);
        crate::stichprobe::stichprobe_aus_anteilen(&anteile, &saat, Kette::STICHPROBE_BP)
    };
    // ⚑ Aus der Konstante gerechnet (Fund 171).
    let erwartet = (1_000 * Kette::STICHPROBE_BP as usize).div_ceil(10_000);
    assert_eq!(zieh(&a).len(), erwartet);
    assert_ne!(zieh(&a), zieh(&b), "das Zertifikat muss die Ziehung ändern");
    assert_eq!(zieh(&a), zieh(&a.clone()), "dasselbe Zertifikat, dieselbe Ziehung");
}

/// ⚑ **Wer die Finalitaet traegt** (7.8 Schritt 2, 2026-11-04): in der
/// Startphase der Satz von aussen, danach nur der Satz der Epoche aus dem
/// Zustand; nie zurueck, nie fuer eine Epoche vor der Vorepoche, und ein
/// nicht tragfaehiger Satz laesst die Finalitaet ruhen.
#[test]
fn die_finalitaet_traegt_der_satz_aus_dem_zustand() {
    let pk = |b: u8| myl_types::bls::BlsSecretKey::key_gen(&[b; 32]).unwrap().public_key().unwrap();
    let id = |b: u8| myl_types::ids::MinerId::new([b; 32]);
    let mut aussen = std::collections::BTreeMap::new();
    aussen.insert(id(99), myl_consensus::validator::VotingMember { pubkey: pk(99), weight: 1 });
    let aussen = myl_consensus::validator::VotingSet::from_members(aussen);
    let mut z = LedgerState::genesis(1);
    z.epoch.0 = 5;
    assert!(Kette::finalitaetssatz_von(&z, Some(&aussen), 5).is_some_and(|s| s.contains(&id(99))), "Startphase");
    let w = crate::netz::PROBEKETTE.stimmsatz_mindesteinsatz.unwrap_or(1);
    let satz: myl_ledger::stimmsatz::Satz = (1..=4).map(|b| (id(b), pk(b), w)).collect();
    z.stimmsaetze.insert(7, satz.clone());
    assert!(Kette::finalitaetssatz_von(&z, Some(&aussen), 6).is_some_and(|s| s.contains(&id(99))), "vor dem ersten Satz");
    let s7 = Kette::finalitaetssatz_von(&z, Some(&aussen), 7).expect("Satz aus dem Zustand");
    assert!(s7.contains(&id(1)) && !s7.contains(&id(99)));
    z.epoch.0 = 8;
    z.stimmsaetze.insert(8, satz[..3].to_vec());
    assert!(Kette::finalitaetssatz_von(&z, Some(&aussen), 8).is_none(), "drei tragen nicht, und nie zurueck nach aussen");
    assert!(Kette::finalitaetssatz_von(&z, Some(&aussen), 7).is_some(), "die Vorepoche gilt");
    z.epoch.0 = 9;
    assert!(Kette::finalitaetssatz_von(&z, Some(&aussen), 7).is_none(), "aelter als die Vorepoche");
    assert!(Kette::finalitaetssatz_von(&z, Some(&aussen), 6).is_none(), "auch kein Rueckfall nach aussen");
}

/// ⛔️ **Fehlt `β`, wartet die Stichprobe** (2026-11-04): Mit einem
/// Saatschluessel zieht das Zertifikat nicht mehr. Kommt `β` der Epoche
/// nie zustande (zu viele Mitglieder fallen aus), zieht `β` der
/// naechsten alles Offene; einen Rueckfall gibt es nicht.
#[test]
fn ohne_beta_wartet_die_stichprobe_auf_das_naechste() {
    use myl_ledger::saat::{Saatrunde, Saatschluessel};
    let mut k = Kette::probestand();
    for _ in 0..150 {
        k.baue_block();
    }
    let mut z = pruefpunkt_von(&k, 150, PROBEERZEUGER);
    z.hoehe = BLOECKE_JE_EPOCHE;
    let sk = myl_types::bls::BlsSecretKey::key_gen(&[9; 32]).expect("Schluessel");
    let pk = sk.public_key().expect("oeffentlich");
    let schluessel = Saatschluessel {
        aus_epoche: 1,
        gruppe: pk,
        schwelle: 1,
        oeffentliche_anteile: vec![pk],
        inhaber: Vec::new(),
        qualifiziert: Vec::new(),
    };
    let mut zustand = LedgerState::genesis(1);
    for e in [1u64, 2] {
        zustand.saatrunden.insert(
            e,
            Saatrunde {
                mitglieder: Vec::new(),
                gesamtgewicht: 0,
                schwelle: 1,
                gesamt_sitze: 0,
                beschwerden: Vec::new(),
                offenlegungen: Vec::new(),
                schluessel: Some(schluessel.clone()),
                anteile: Vec::new(),
                beta: None,
            },
        );
    }
    zustand.stichproben_offen = vec![myl_ledger::state::OffeneStichprobe {
        epoche: 0,
        anteile: vec![(myl_types::ids::PodId::new([5; 32]), 1_000)],
        wurzeln: Vec::new(),
        voll: Vec::new(),
        pruefkandidaten: Vec::new(),
        ersatz: Vec::new(),
        besetzung: Vec::new(),
        pflicht: Vec::new(),
    }];
    let mut gezogen = None;
    Kette::offene_stichproben_ziehen(&mut zustand, &z, &mut gezogen);
    assert!(gezogen.is_none(), "das Zertifikat zieht nicht, wo ein Schluessel steht");
    assert_eq!(zustand.stichproben_offen.len(), 1);
    // `β_1` kam nie; `β_2` zieht die Stichprobe der Epoche 0.
    let beta = sk.sign(&myl_types::schwelle::saatbotschaft(2)).expect("signieren");
    Kette::beta_verwenden(&mut zustand, 2, &beta, 2, &mut gezogen);
    assert_eq!(gezogen.as_ref().map(|(e, _)| *e), Some(0));
    assert!(zustand.stichproben_offen.is_empty());
}

/// ⚑ **Die Zeugen kommen auch von der Platte** (2026-10-30): Ist der
/// Speicher leer, liefert die Ablage dieselben unterschriebenen Stücke.
#[test]
fn zeugenstuecke_kommen_auch_von_der_platte() {
    let mut erzeuger = Kette::probestand();
    let ordner = std::env::temp_dir().join(format!("myl-kette-zeugen-{}", std::process::id()));
    erzeuger.zeugen_auf_platte_setzen(ordner.clone()).expect("Ordner");
    let mut gehalten = Vec::new();
    for i in 0..5 {
        erzeuger.aufnehmen(burn_nr(2, 10, i));
        let b = erzeuger.baue_block();
        let s = erzeuger.zeugenstuecke(&b.hash()).expect("unterschrieben");
        gehalten.push((b.hash(), s));
    }
    erzeuger.zeugenstuecke.clear();
    for (h, s) in &gehalten {
        assert_eq!(erzeuger.zeugenstuecke_gehalten(h).as_deref(), Some(&**s), "von der Platte");
    }
    assert_eq!(erzeuger.zeugenablage_fehler, 0);
    assert!(erzeuger.zeugenstuecke_gehalten(&Hash::sha256(b"fremd")).is_none());
    std::fs::remove_dir_all(&ordner).ok();
}

/// Unterschreibt einen in einem Test veränderten Block neu, mit dem
/// Probeerzeuger: Der berechtigte Erzeuger hat einen falschen Block
/// gebaut, und die eigentliche Prüfung muss ihn fangen, nicht erst die
/// Unterschrift.
fn neu_unterschreiben(block: &mut Block) {
    let k = crate::schluessel::Konsensschluessel::probe(PROBEERZEUGER).expect("Schlüssel");
    let signatur = k.signiere(&block.erzeugernachricht()).expect("unterschreiben");
    block.unterschrift = Some(myl_consensus::block::Erzeugerunterschrift { schluessel: k.oeffentlich(), signatur });
}

/// ⚑ **Jeder Block braucht die Unterschrift eines Mitglieds** (Stimmsatz
/// ab Genesis, 2026-10-27). Mit Stimmsatz nur ein Mitglied unter seinem
/// Schlüssel; ohne Satz lernt eine Probekette den Erzeuger aus Block 1
/// und verlangt ihn danach; ein anderes Netz nimmt ohne Satz nichts.
/// ⚑ **Vor dem Lernen baut auf einer Probekette nur der ausdrückliche
/// Erzeuger** (2026-11-15, Probenetz): Ein Miner wartet auf Block 1 und
/// baut danach nur, wo ihn der Plan zieht; ein Erzeuger beginnt sofort.
#[test]
fn vor_dem_lernen_baut_nur_der_ausdrueckliche_erzeuger() {
    let erzeuger_k = crate::schluessel::Konsensschluessel::probe("erzeuger").expect("Schlüssel");
    let miner_k = crate::schluessel::Konsensschluessel::probe("mitlaeufer").expect("Schlüssel");
    let mut erzeuger = Kette::im_netz(&crate::netz::PROBEKETTE);
    erzeuger.erzeugerschluessel_setzen(erzeuger_k);
    let mut miner = Kette::im_netz(&crate::netz::PROBEKETTE);
    miner.miner_erzeugerschluessel_setzen(miner_k.clone());
    assert_eq!(miner.eigener_slot(0, 1_000), None, "der Miner baute vor dem ersten Block");
    assert!(erzeuger.eigener_slot(0, 1_000).is_some(), "der Erzeuger kam vor dem Lernen nicht dran");
    // Gegenprobe: derselbe Schlüssel als ausdrücklicher Erzeuger baut.
    let mut als_erzeuger = Kette::im_netz(&crate::netz::PROBEKETTE);
    als_erzeuger.erzeugerschluessel_setzen(miner_k);
    assert!(als_erzeuger.eigener_slot(0, 1_000).is_some());
    // Nach Block 1 des Erzeugers: Der Miner steht nicht im gelernten Satz.
    let b1 = erzeuger.baue_block();
    miner.uebernimm(&b1).expect("Block 1 des Erzeugers");
    let naechster = miner.letzter_slot() + 1;
    assert_eq!(miner.eigener_slot(naechster, naechster + 1_000), None);
    assert!(erzeuger.eigener_slot(naechster, naechster + 1_000).is_some(), "der Erzeuger baut weiter");
}

#[test]
fn nur_ein_belegter_erzeuger_kommt_durch() {
    use myl_consensus::validator::{VotingMember, VotingSet};
    let mitglied = crate::schluessel::Konsensschluessel::probe("erzeuger").expect("Schlüssel");
    let fremder = crate::schluessel::Konsensschluessel::probe("fremder").expect("Schlüssel");
    let satz = || {
        let mut m = std::collections::BTreeMap::new();
        m.insert(mitglied.kennung(), VotingMember { pubkey: mitglied.oeffentlich(), weight: 1 });
        VotingSet::from_members(m)
    };
    let erzeugt_von = |k: Option<&crate::schluessel::Konsensschluessel>| {
        let mut e = Kette::im_netz(&crate::netz::PROBEKETTE);
        if let Some(k) = k {
            e.erzeugerschluessel_setzen(k.clone());
        }
        e.baue_block()
    };
    let mit_satz = || {
        let mut l = Kette::probestand();
        l.stimmsatz_setzen(satz());
        l
    };

    // Das Mitglied: angenommen.
    let echt = erzeugt_von(Some(&mitglied));
    assert_eq!(echt.erzeuger(), Ok(Some(mitglied.kennung())));
    mit_satz().uebernimm(&echt).expect("ein Block des Mitglieds wurde abgewiesen");
    // ⛔️ Gegenproben mit Stimmsatz: unsigniert, von einem Fremden, und
    // die Unterschrift des Mitglieds unter einem anderen Schlüssel.
    assert_eq!(mit_satz().uebernimm(&erzeugt_von(None)), Err(KettenFehler::ErzeugerUnbelegt));
    assert_eq!(mit_satz().uebernimm(&erzeugt_von(Some(&fremder))), Err(KettenFehler::ErzeugerUnbelegt));
    let mut untergeschoben = echt.clone();
    if let Some(u) = untergeschoben.unterschrift.as_mut() {
        u.schluessel = fremder.oeffentlich();
    }
    assert_eq!(mit_satz().uebernimm(&untergeschoben), Err(KettenFehler::ErzeugerUnbelegt));

    // ⛔️ Ohne Satz auf der Probekette: unsigniert nie, und eine
    // Unterschrift, die nicht trägt, nie.
    assert_eq!(Kette::probestand().uebernimm(&erzeugt_von(None)), Err(KettenFehler::ErzeugerUnbelegt));
    assert_eq!(Kette::probestand().uebernimm(&untergeschoben), Err(KettenFehler::ErzeugerUnbelegt));
    // Der Erzeuger von Block 1 wird das einzige Mitglied; danach kommt
    // ein anderer nicht mehr durch.
    let mut e = Kette::im_netz(&crate::netz::PROBEKETTE);
    e.erzeugerschluessel_setzen(fremder.clone());
    let erster = e.baue_block();
    e.erzeugerschluessel_setzen(mitglied.clone());
    let zweiter = e.baue_block();
    let mut lerner = Kette::probestand();
    assert!(lerner.stimmsatz().is_none());
    lerner.uebernimm(&erster).expect("Block 1 auf der Probekette");
    assert_eq!(lerner.stimmsatz().and_then(|s| s.pubkey(&fremder.kennung())), Some(&fremder.oeffentlich()));
    assert_eq!(lerner.uebernimm(&zweiter), Err(KettenFehler::ErzeugerUnbelegt), "ein zweiter Erzeuger kam durch");

    // ⛔️ Ein Netz, das nicht lernt, nimmt ohne Satz nichts an.
    let mut t = Kette::im_netz(&crate::netz::TESTNETZ_INT4);
    t.erzeugerschluessel_setzen(mitglied.clone());
    let testblock = t.baue_block();
    assert_eq!(Kette::im_netz(&crate::netz::TESTNETZ_INT4).uebernimm(&testblock), Err(KettenFehler::ErzeugerUnbelegt));
}

/// ⚑ **Nur der für den Slot Gezogene** (Erzeugerplan, 2026-11-04): Drei
/// Mitglieder bauen, wer im nächsten Slot gezogen ist; ein anderes
/// Mitglied im selben Slot kommt nicht durch, ein Slot, der nicht nach
/// dem des Vorgängers liegt, auch nicht, und ein Block weiter vorn (zum
/// Nachholen) braucht nur ein Mitglied. Jedes kommt mal dran.
#[test]
fn nur_der_gezogene_erzeuger_kommt_durch() {
    use myl_consensus::validator::{VotingMember, VotingSet};
    let schluessel: Vec<_> =
        ["erste", "zweite", "dritte"].iter().map(|n| crate::schluessel::Konsensschluessel::probe(n).expect("Schlüssel")).collect();
    let mut m = std::collections::BTreeMap::new();
    for k in &schluessel {
        m.insert(k.kennung(), VotingMember { pubkey: k.oeffentlich(), weight: 1 });
    }
    let satz = VotingSet::from_members(m);
    let kette = || {
        let mut k = Kette::probestand();
        k.stimmsatz_setzen(satz.clone());
        k
    };
    let nachgebaut = |bis: &mut Kette, wer: &crate::schluessel::Konsensschluessel| {
        let mut k = kette();
        for b in bis.bloecke_von_bis(1, bis.hoehe()) {
            k.uebernimm(&b).expect("die Kette bis hier");
        }
        k.erzeugerschluessel_setzen(wer.clone());
        k
    };
    let mut erzeuger = kette();
    let mut folger = kette();
    let mut je = std::collections::BTreeMap::new();
    for _ in 0..24 {
        let hoehe = erzeuger.hoehe() + 1;
        let slot = erzeuger.letzter_slot() + 1;
        let gezogen = erzeuger.erzeuger_fuer(hoehe, slot).expect("gezogen");
        *je.entry(gezogen).or_insert(0) += 1;
        // ⛔️ Ein anderes Mitglied im selben Slot: abgewiesen.
        let anderer = schluessel.iter().find(|k| k.kennung() != gezogen).expect("ein anderer");
        let falsch = nachgebaut(&mut erzeuger, anderer).baue_block_im_slot(slot);
        assert_eq!(folger.uebernimm(&falsch), Err(KettenFehler::ErzeugerUnbelegt));
        // Der Gezogene: angenommen.
        let k = schluessel.iter().find(|k| k.kennung() == gezogen).expect("gezogen");
        erzeuger.erzeugerschluessel_setzen(k.clone());
        assert_eq!(erzeuger.eigener_slot(0, slot + 100), Some(slot));
        let block = erzeuger.baue_block();
        assert_eq!(block.header.slot, slot);
        folger.uebernimm(&block).expect("der Gezogene wurde abgewiesen");
    }
    assert_eq!(je.len(), 3, "nicht jeder kam dran: {je:?}");
    // ⛔️ Ein Slot, der nicht nach dem des Vorgängers liegt, auch vom
    // dort Gezogenen nicht.
    let alt = folger.letzter_slot();
    let wer = schluessel.iter().find(|k| Some(k.kennung()) == erzeuger.erzeuger_fuer(erzeuger.hoehe() + 1, alt)).expect("gezogen");
    let zurueck = nachgebaut(&mut erzeuger, wer).baue_block_im_slot(alt);
    assert_eq!(folger.uebernimm(&zurueck), Err(KettenFehler::ErzeugerUnbelegt));
    // Ein Mitglied, das im nächsten Slot nicht gezogen ist, baut in
    // seinem nächsten und kommt damit durch: Leere Slots sind erlaubt.
    let naechster = erzeuger.letzter_slot() + 1;
    let gezogen = erzeuger.erzeuger_fuer(erzeuger.hoehe() + 1, naechster).expect("gezogen");
    let spaeter = schluessel.iter().find(|k| k.kennung() != gezogen).expect("ein anderer");
    erzeuger.erzeugerschluessel_setzen(spaeter.clone());
    let s = erzeuger.eigener_slot(0, naechster + SLOTS_GESUCHT).expect("irgendwann dran");
    assert!(s > naechster);
    let block = erzeuger.baue_block();
    assert_eq!(block.header.slot, s);
    // Ein Block weiter vorn: nur ein Mitglied verlangt (das Nachholen).
    let voraus = erzeuger.baue_block();
    assert_eq!(folger.uebernimm(&voraus), Err(KettenFehler::PasstNichtAn { erwartet: folger.letzter_hash(), bekommen: block.hash() }));
    folger.uebernimm(&block).expect("ein späterer Slot wurde abgewiesen");
    folger.uebernimm(&voraus).expect("der Block danach");
}

/// ⚑ **Ein Erzeuger, der doppelt unterschreibt, wird geschlachtet**
/// (CONSENSUS 7.6): 30 % seines Einsatzes beim ersten Mal, das Kopfgeld
/// an den Melder. Gegenproben: derselbe Widerspruch noch einmal, ein
/// Täter ohne Einsatz, eine Tat vor der Sperrfrist, wer sich selbst
/// meldet.
#[test]
fn eine_doppelunterschrift_kostet_den_einsatz() {
    use myl_consensus::double_signing::{Belegblock, Doppelbeleg};
    let block = |zeit: u64, wer: u8| {
        let mut b = Block::new(BlockHeader {
            height: 5,
            epoch: 0,
            prev_block_hash: Hash::sha256(b"vorher"),
            timestamp_ms: zeit,
            state_root: Hash::sha256(b"wurzel"),
            pruefpunkt: None,
            slot: 0,
        });
        b.unterschreiben(&probeschluessel(wer)).expect("unterschreiben");
        Belegblock::aus_block(&b).expect("unterschrieben")
    };
    let beleg = |wer: u8| Doppelbeleg::Erzeuger(Box::new(block(1, wer)), Box::new(block(2, wer)));
    let taeter = Address::aus_schluessel(&probeschluessel(1).public_key().expect("pk"));
    let melder = probekonto(2);
    let mut z = Kette::probestand().zustand().clone();
    z.account_mut(&taeter).balance = 10_000;
    einsatz_hinterlegen(&mut z, &taeter, 10_000).expect("Einsatz");
    // Ein Grosser daneben: allein bleibt es beim Grundsatz (3 % mal drei).
    let gross = probekonto(4);
    z.account_mut(&gross).balance = 1_000_000;
    einsatz_hinterlegen(&mut z, &gross, 1_000_000).expect("Einsatz");
    let vorher_melder = z.account(&melder).balance;

    Kette::doppelbeleg_buchen(&mut z, &melder, &beleg(1)).expect("ein tragender Beleg");
    assert_eq!(z.account(&taeter).staked, 7_000, "30 % beim ersten Mal");
    assert!(z.account(&melder).balance > vorher_melder, "der Melder bekam kein Kopfgeld");

    // ⛔️ Derselbe Widerspruch noch einmal.
    assert!(Kette::doppelbeleg_buchen(&mut z, &melder, &beleg(1)).is_err());
    assert_eq!(z.account(&taeter).staked, 7_000);
    // ⛔️ Ein Täter ohne Einsatz: nichts zu schlachten.
    assert!(Kette::doppelbeleg_buchen(&mut z, &melder, &beleg(3)).is_err());
    // ⛔️ Wer sich selbst meldet.
    let mut z2 = Kette::probestand().zustand().clone();
    z2.account_mut(&taeter).balance = 10_000;
    einsatz_hinterlegen(&mut z2, &taeter, 10_000).expect("Einsatz");
    assert!(Kette::doppelbeleg_buchen(&mut z2, &taeter, &beleg(1)).is_err());
    // ⛔️ Eine Tat vor der Sperrfrist.
    z2.epoch = EpochId(myl_ledger::einsatz::SPERRFRIST_EPOCHEN + 1);
    assert!(Kette::doppelbeleg_buchen(&mut z2, &melder, &beleg(1)).is_err());
    assert_eq!(z2.account(&taeter).staked, 10_000);
    // ⚑ Korrelation: Hält der Täter allen Einsatz, verliert er alles,
    //   auch was er gekündigt hat; die Kündigung bleibt fest.
    let mut z3 = Kette::probestand().zustand().clone();
    z3.account_mut(&taeter).balance = 10_000;
    einsatz_hinterlegen(&mut z3, &taeter, 10_000).expect("Einsatz");
    myl_ledger::transitions::einsatz_kuendigen(&mut z3, &taeter, 4_000).expect("Kündigung");
    Kette::doppelbeleg_buchen(&mut z3, &melder, &beleg(1)).expect("ein tragender Beleg");
    assert_eq!(myl_ledger::einsatz::schlachtbar(z3.account(&taeter).staked, &z3.account(&taeter).gekuendigt), 0);
}

/// ⚑ **Eine Prüfpunktstimme als Transaktion** (7.8, 2026-11-03): zählt
/// mit dem Hash, den die Kette an der Höhe hat, je Prüfpunkt einmal.
/// Gegenproben: dieselbe Stimme noch einmal, ein falscher Hash, wer
/// nicht im Stimmsatz steht, eine Höhe ohne Prüfpunkt.
#[test]
fn eine_stimme_als_transaktion_zaehlt_einmal() {
    use myl_consensus::pruefpunkt::BLOECKE_JE_PRUEFPUNKT;
    let mut k = Kette::probestand();
    let mut folger = Kette::probestand();
    for i in 1..=3u8 {
        k.aufnehmen(probeanmeldung(i));
    }
    // Der erste Block lehrt beide den Erzeuger als Satz.
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    let mut mitglieder = std::collections::BTreeMap::new();
    if let Some(alt) = k.stimmsatz() {
        for (m, mitglied) in alt.members() {
            mitglieder.insert(*m, mitglied.clone());
        }
    }
    for i in 1..=2u8 {
        let pk = probeschluessel(i).public_key().expect("Schlüssel");
        mitglieder.insert(myl_types::ids::MinerId::aus_schluessel(&pk), myl_consensus::validator::VotingMember { pubkey: pk, weight: 1 });
    }
    assert!(!mitglieder.is_empty(), "der Erzeuger steht im Satz");
    let satz = myl_consensus::validator::VotingSet::from_members(mitglieder);
    k.stimmsatz_setzen(satz.clone());
    folger.stimmsatz_setzen(satz);
    while k.hoehe() <= BLOECKE_JE_PRUEFPUNKT + 1 {
        folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    }
    let h = BLOECKE_JE_PRUEFPUNKT;
    let x = k.hash_an(h).expect("Hash an der Höhe");
    assert!(k.zustand().pruefpunkt_hashes.contains(&(h, x)), "der Zustand kennt den Hash des Prüfpunkts");
    let id = |i: u8| myl_types::ids::MinerId::new(*probekonto(i).as_bytes());
    let senden = |k: &mut Kette, wer: u8, nonce: u64, hoehe: u64, block_hash: Hash| {
        k.aufnehmen(
            Transaktion::signiere(&Kette::startwert(), &probeschluessel(wer), nonce, Anweisung::PruefpunktBezeugen { hoehe, block_hash })
                .expect("signieren"),
        );
    };
    senden(&mut k, 1, 1, h, x);
    senden(&mut k, 1, 2, h, x);
    senden(&mut k, 2, 1, h, Hash::sha256(b"anderer Zweig"));
    senden(&mut k, 2, 2, h + 1, x);
    senden(&mut k, 3, 1, h, x);
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    let z = k.zustand();
    assert!(myl_ledger::stimmsatz::stimme_vermerkt(z, &id(1), 0, 1));
    assert_eq!(z.pruefpunkt_stimmen.get(&id(1)).copied(), Some(1), "einmal, nicht zweimal");
    assert!(!myl_ledger::stimmsatz::stimme_vermerkt(z, &id(2), 0, 1), "falscher Hash oder keine Prüfpunkthöhe");
    assert!(z.pruefpunkt_stimmen.get(&id(2)).is_none());
    assert!(z.pruefpunkt_stimmen.get(&id(3)).is_none(), "nicht im Stimmsatz");
    assert_eq!(folger.zustandswurzel(), k.zustandswurzel());
}

/// ⚑ **Die Vertrauensfrist** (CONSENSUS 7.5): bis 112 Epochen über dem
/// bestätigten Prüfpunkt trägt sie, eine darüber nicht; ein Netz, das
/// hinter dem eigenen Stand liegt, ist kein Grund zum Anhalten.
#[test]
fn die_vertrauensfrist_traegt_bis_112_epochen() {
    assert!(vertrauen_traegt(10, 10 + 112));
    assert!(!vertrauen_traegt(10, 10 + 113), "eine Epoche zu viel");
    assert!(vertrauen_traegt(50, 3));
    assert!(vertrauen_traegt(0, 0));
}

/// Ohne Zertifikat wird ein Block angenommen: Nicht jeder Block trägt
/// eines, nur der, in dem es gerade beisammen ist.
#[test]
fn ohne_zertifikat_wird_ein_block_angenommen() {
    let mut k = Kette::probestand();
    let block = k.baue_block();
    assert!(block.header.pruefpunkt.is_none());
    let mut leser = Kette::probestand();
    assert!(leser.uebernimm(&block).is_ok());
}


/// ⚑ **Die Zuteilung steht schon während ihrer Epoche fest**
/// (Fund 143).
///
/// Bis zum 2026-09-02 kam die Saat aus `self.letzter_hash`, und
/// beim Epochenabschluss ist das der **letzte** Block der Epoche,
/// die gerade abgerechnet wird. Die Zuteilung stand also erst fest,
/// wenn die Epoche vorbei war, während ein Bündel **während** ihr
/// eingereicht sein muss: Kein Pod konnte wissen, dass er einer
/// ist.
///
/// ⚑ **Warum der alte Fehler unentdeckt blieb:** Der große Test
/// dieses Punktes hat sechs Miner, also genau einen Pod, und der
/// enthält alle sechs, gleich welche Saat man nimmt. Die
/// Mitgliedschaft war saatunabhängig (Fund 142), und damit war auch
/// die falsche Saat unauffällig. **Dieser Test hat achtzehn Miner
/// und damit drei Pods**, und über drei Pods entscheidet die Saat.
#[test]
fn die_zuteilung_aendert_sich_waehrend_ihrer_epoche_nicht() {
    use myl_types::miner::HardwareClass;
    use myl_types::node_metadata::GeoRegion;

    let mut k = Kette::probestand();
    // ⚑ **Eigene Schlüssel, nicht die Probekonten.** Von denen gibt
    // es zwölf (`PROBEKONTEN`), und `probeschluessel` rechnet
    // modulo: Achtzehn Aufrufe ergäben achtzehn Anmeldungen unter
    // zwölf Kennungen, also zwölf Miner. Der Test bräuchte dann drei
    // Pods und bekäme zwei.
    for w in 0..18u8 {
        let geheim = myl_types::bls::BlsSecretKey::key_gen(&[w.wrapping_add(1); 32])
            .expect("32 Byte sind für key_gen gültig");
        let oeffentlich = geheim.public_key().expect("gültiger Punkt");
        let kennung = myl_types::ids::MinerId::aus_schluessel(&oeffentlich);
        myl_ledger::transitions::miner_anmelden(
            k.zustand_mut(),
            &myl_types::ids::Address::new(*kennung.as_bytes()),
            &kennung,
            HardwareClass::MediumGpu,
            GeoRegion::Europe,
            oeffentlich,
            myl_types::latency_attest::PeerIdBytes([0; 32]),
        )
        .expect("Anmeldung");
        zusage_direkt(&mut k, kennung);
    }
    assert_eq!(k.zustand().miner.len(), 18, "es entstanden nicht achtzehn Kennungen");

    let besetzung = |k: &Kette| -> Vec<Vec<[u8; 32]>> {
        Kette::zuteilung_der_laufenden_epoche(k.zustand())
            .pods
            .iter()
            .map(|p| {
                let mut m: Vec<[u8; 32]> =
                    p.mitglieder().map(|x| *x.miner_id.as_bytes()).collect();
                m.sort();
                m
            })
            .collect()
    };

    let am_anfang = besetzung(&k);
    // ⚑ Achtzehn Miner ergeben drei Pods zu sechs; der dritte hat keinen
    //   Zwilling und steht in Bereitschaft (W6), seine sechs ohne Pod.
    assert_eq!(am_anfang.len(), 2, "zwei Pods rechnen, einer steht in Bereitschaft");
    assert_eq!(Kette::zuteilung_der_laufenden_epoche(k.zustand()).ohne_pod.len(), 6, "die sechs in Bereitschaft");

    // Zehn Blöcke weiter, mitten in derselben Epoche.
    for _ in 0..10 {
        k.baue_block();
    }
    assert_eq!(k.zustand().epoch.0, 0, "die Epoche wechselte zu früh");
    assert_eq!(
        besetzung(&k),
        am_anfang,
        "die Besetzung hat sich innerhalb der Epoche geändert"
    );

    // ⚑ **Gegenprobe zur Zusicherung selbst:** Der letzte Hash hat
    // sich in diesen zehn Blöcken sehr wohl geändert. Wäre er die
    // Saat, sähe der Test oben etwas anderes, und dass er nichts
    // sieht, wäre nichts wert.
    let mit_letztem_hash: Vec<Vec<[u8; 32]>> =
        zuteilung_aus_hash(&k, &k.letzter_hash())
        .pods
        .iter()
        .map(|p| {
            let mut m: Vec<[u8; 32]> =
                p.mitglieder().map(|x| *x.miner_id.as_bytes()).collect();
            m.sort();
            m
        })
        .collect();
    assert_ne!(
        mit_letztem_hash, am_anfang,
        "der letzte Hash ergibt dieselbe Besetzung wie die Epochensaat, \
             dann prüft dieser Test nichts"
    );
}

/// ⚑ **Die Saat rückt um genau eine Epoche vor, mit zwei Epochen
/// Vorlauf** (Fund 143).
#[test]
fn die_saat_kommt_aus_der_vorletzten_epoche() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;

    let mut k = Kette::probestand();
    assert_eq!(k.zustand().epochensaat, Kette::startwert());

    // Bis zum Wechsel bauen und dabei den Hash **vor** jedem Block
    // merken. ⚑ Nicht die Höhe abzählen: Der Wechsel geschieht
    // **im** Block, dessen Epoche höher ist, und `letzter_hash` ist
    // dann schon der seine. Der erste Anlauf dieses Tests hat genau
    // daran gelegen, mit einer Zahl statt einer Wirkung.
    let mut ende_epoche_0 = k.letzter_hash();
    for _ in 0..BLOECKE_JE_EPOCHE * 2 {
        let vorher = k.letzter_hash();
        k.baue_block();
        if k.zustand().epoch.0 == 1 {
            ende_epoche_0 = vorher;
            break;
        }
    }
    assert_eq!(k.zustand().epoch.0, 1, "die Epoche wechselte nicht");
    // In Epoche 1 gilt noch der Startwert: Epoche `e` nimmt die Saat
    // von `e−2`, und `−1` gibt es nicht.
    assert_eq!(
        k.zustand().epochensaat,
        Kette::startwert(),
        "Epoche 1 nimmt schon eine Saat, die es noch nicht geben darf"
    );
    assert_eq!(k.zustand().epochensaat_naechste, ende_epoche_0);

    for _ in 0..BLOECKE_JE_EPOCHE * 2 {
        k.baue_block();
        if k.zustand().epoch.0 == 2 {
            break;
        }
    }
    assert_eq!(k.zustand().epoch.0, 2);
    assert_eq!(
        k.zustand().epochensaat,
        ende_epoche_0,
        "Epoche 2 muss die Saat vom Ende der Epoche 0 tragen"
    );
}

/// ⚑ **Punkt 40 als Ganzes: bezeugte Arbeit erreicht ein Konto.**
///
/// Sechs Miner melden sich an, tragen ein Auszahlungskonto ein,
/// jemand verbrennt, ein Bündel kommt in die Kette, und am
/// Epochenwechsel wächst das Konto eines Pod-Mitglieds.
///
/// ⚑ **Seit dem 2026-10-28 mit der Anlaufregel** (W6): Sechs Miner sind
/// ein Pod allein, ohne Zwilling. Seine Gutschrift wartet bis nach der
/// Streitfrist (`zurueckgehalten`, die Freigabe prüft
/// `ein_pod_allein_wird_zurueckgehalten_und_voll_gezogen`), und jedes
/// Segment wird gezogen, gedeckelt.
///
/// **Das ist der Test, an dem der ganze Punkt hängt.** Jeder
/// Zwischenschritt hat seine eigenen Tests; dieser prüft, dass sie
/// zusammen etwas ergeben.
#[test]
fn bezeugte_arbeit_erreicht_ein_konto() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    use myl_types::miner::HardwareClass;
    use myl_types::node_metadata::GeoRegion;

    // ⚑ **Keine Verteilung mehr zu setzen** (Fund 161): Sie wird
    // gerechnet und ist immer da. Das letzte Shard-Stück wiegt von
    // selbst schwerer, weil dort der LM-Kopf sitzt.
    let mut k = Kette::probestand();
    // Ein Folger, der jeden Block nur empfängt: Er muss dieselbe
    // Ziehung kennen wie der Erzeuger.
    let mut folger = Kette::probestand();

    // Sechs Miner: vier Positionen plus zwei Reserve.
    let mut nonce = [0u64; 6];
    for w in 0..6u8 {
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::MinerAnmelden {
                    hardware: HardwareClass::MediumGpu,
                    zone: GeoRegion::Europe,
                    netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
        k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
        nonce[w as usize] += 1;
    }
    // Und Brennstoff für die Prägung.
    k.aufnehmen(burn_nr(0, 5_000_000, nonce[0]));
    nonce[0] += 1;
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    for w in 0..6u8 {
        zusage_direkt(&mut k, myl_types::ids::MinerId::new(*probekonto(w).as_bytes()));
        zusage_direkt(&mut folger, myl_types::ids::MinerId::new(*probekonto(w).as_bytes()));
    }
    assert_eq!(k.zustand().miner.len(), 6, "die Anmeldungen kamen nicht an");
    assert!(k.zustand().burn_epoche > 0, "es wurde nichts verbrannt");

    // Auszahlungskonten eintragen, damit „ohne Eintrag kein Anteil"
    // nicht greift. Die erste Eintragung darf der Miner selbst.
    //
    // ⚑ **Über die Kette und nicht über den Übergang** (Fund 167,
    // 2026-09-03). Bis dahin rief dieser Test
    // `auszahlungskonto_eintragen` direkt, und das war der einzige
    // Weg, den es gab: Keine `Anweisung` trug ihn. Der Test war
    // grün und belegte einen Ablauf, den auf einem echten Netz
    // niemand gehen konnte. Jetzt geht er denselben Weg wie ein
    // Miner.
    for w in 0..6u8 {
        let kennung = myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::AuszahlungskontoEintragen {
                    kennung,
                    konto: kaltes_konto(w),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
    }
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    for w in 0..6u8 {
        let kennung = myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
        assert_eq!(
            k.zustand().auszahlung.get(&kennung),
            Some(&kaltes_konto(w)),
            "das Auszahlungskonto kam nicht ueber die Kette an"
        );
    }

    // Die Zuteilung dieser Epoche nachrechnen und für ihren Pod ein
    // Bündel einreichen.
    // ⚑ Die Saat aus dem Zustand, nicht der letzte Hash (Fund 143): Nur
    //   sie gilt die ganze Epoche, und nur gegen sie rechnet der
    //   Abschluss nach.
    let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    assert_eq!(zuteilung.pods.len(), 1, "es entstand kein Pod");
    let pod_index = zuteilung.pods[0].pod_index;
    let mut b = myl_types::PoIBundle {
        epoch: k.zustand().epoch,
        pod: myl_types::pod_kennung(k.zustand().epoch.0, pod_index),
        segments_root: myl_types::ids::MerkleRoot::new([7; 32]),
        vtfe_claimed: 1_000_000,
        aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
        // Tausend Segmente, damit die Ziehung ueberhaupt sichtbar ist
        // und die Ziehung ueberhaupt sichtbar ist.
        segmente: 1_000,
        ausgabenwurzel: myl_types::MerkleRoot::new([0u8; 32]),
        vertreter: Vec::new(),
        sitzungen: Vec::new(),
    };
    buendel_unterschreiben(&mut b, &zuteilung.pods[0]);
    // ⚑ **Eingereicht wird vom Koordinator**, seit die Aufnahme
    // ihn prüft (Fund 144). Vorher tat es Konto null, und das war
    // nur deshalb richtig, weil niemand hinsah.
    let koordinator = koordinator_von(&zuteilung.pods[0]);
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(koordinator),
            nonce[koordinator as usize],
            Anweisung::BuendelEinreichen { buendel: b },
        )
        .expect("signieren"),
    );
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    assert_eq!(k.zustand().buendel.len(), 1, "das Buendel kam nicht an");

    // Über die Epochengrenze.
    let vorher: Vec<u64> = (0..6u8)
        .map(|w| k.zustand().account(&kaltes_konto(w)).balance)
        .collect();
    for _ in 2..=BLOECKE_JE_EPOCHE {
        folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    }
    assert_eq!(k.zustand().epoch.0, 1, "die Epoche wechselte nicht");

    let nachher: Vec<u64> = (0..6u8)
        .map(|w| k.zustand().account(&kaltes_konto(w)).balance)
        .collect();
    // ⚑ **Gezogen wird erst mit dem Zertifikat** (2026-10-27, CONSENSUS
    //   7.2): Bis dahin hebt der Zustand die Anteile auf. ⛔️ Ohne
    //   Zertifikat keine Ziehung, und kein Rückfall auf den Blockhash.
    assert!(k.letzte_stichprobe().is_empty(), "gezogen ohne Zertifikat");
    assert_eq!(k.zustand().stichproben_offen.len(), 1);
    // 📌 Fund 680, Folge: Zwischen Wechsel und Ziehung kommen die Spuren;
    //    der Pod der abgelaufenen Epoche muss dann zu finden sein.
    assert_eq!(
        k.pod_der_kennung(0, &myl_types::pod_kennung(0, pod_index)).map(|p| p.pod_index),
        Some(pod_index),
        "der Pod der abgelaufenen Epoche vor der Ziehung"
    );
    let z = pruefpunkt_von(&k, BLOECKE_JE_EPOCHE, PROBEERZEUGER);
    k.pruefpunkt_anbieten(z);
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    assert!(k.zustand().stichproben_offen.is_empty(), "die Ziehung wartet noch");
    // ⚑ **Fund 114: Und die Stichprobe ist gezogen.**
    //
    // `sample_segments` und `check_segment` waren seit dem
    // 2026-08-17 gebaut, geprueft und abgehakt, und **hatten null
    // Aufrufer**. Damit war `p` aus Anhang B.1 im Betrieb null, und
    // `S_min = g/p^2` keine Schranke mehr.
    //
    // 📌 **Geprueft wird die Ziehung, nicht das Nachrechnen.** Das
    // braucht die Spur des Segments, und die liegt beim
    // Koordinator. Diese Grenze steht hier, damit niemand den
    // gruenen Test fuer mehr haelt, als er sagt.
    let kennung = myl_types::pod_kennung(0, pod_index);
    let stichprobe = k.letzte_stichprobe();
    // Die Rate zieht ihre 5 %, die Anlaufregel dazu jedes Segment bis
    // zum Deckel; ohne Doppel.
    let rate = (1_000 * Kette::STICHPROBE_BP as usize).div_ceil(10_000);
    let deckel = Kette::VOLL_GEZOGEN_HOECHSTENS as usize;
    assert!(
        (0..Kette::VOLL_GEZOGEN_HOECHSTENS).all(|s| stichprobe.iter().any(|x| x.segment == s)),
        "ein Pod allein: die ersten {deckel} Segmente alle gezogen"
    );
    assert!(
        (deckel..=deckel + rate).contains(&stichprobe.len()),
        "Rate {rate} und Deckel {deckel}, gezogen wurden {}",
        stichprobe.len()
    );
    assert!(
        stichprobe.iter().all(|x| x.pod == kennung && x.segment < 1_000),
        "eine Ziehung zeigte ins Leere"
    );
    // 📌 Fund 658: Der Pod jeder Ziehung ist nach dem Wechsel noch zu
    //    finden (die Probekonten hier tragen keine Netzadresse; die
    //    Adressen prüft `stichprobe.rs`).
    let pod = k.pod_der_kennung(0, &kennung).expect("Fund 658: der Pod der abgelaufenen Epoche fehlt");
    assert_eq!(pod.pod_index, pod_index);
    assert!(stichprobe.iter().all(|x| k.pod_der_kennung(0, &x.pod).is_some()));
    // ⛔️ Eine andere Epoche als die der Ziehung findet nichts.
    assert!(k.pod_der_kennung(5, &kennung).is_none());
    // 📌 Bis NODE 0.79.0 kannte nur der Erzeuger die Ziehung; ein
    //    Checker erfuhr nie, dass er zugeteilt war.
    assert_eq!(folger.letzte_stichprobe(), k.letzte_stichprobe(), "der Folger kennt die Ziehung nicht");
    assert_eq!(folger.pruefer_der_stichprobe(), k.pruefer_der_stichprobe());
    assert_eq!(folger.pod_der_kennung(0, &kennung).map(|p| p.pod_index), Some(pod_index));

    // ⚑ Die Probeprägung (2026-10-26) bekommt jedes eingetragene Konto
    //   obendrauf; geprüft wird, was die echte Ausschüttung zahlte.
    let probe = crate::netz::PROBEKETTE.probepraegung;
    assert!(
        (0..6).all(|i| nachher[i] >= vorher[i] + probe),
        "ein Konto bekam die Probeprägung nicht: {vorher:?} -> {nachher:?}"
    );
    // ⚑ Anlaufregel: in dieser Epoche nichts über die Probeprägung
    //   hinaus, sondern zurückgehalten bis nach der Streitfrist, für
    //   höchstens die vier Positionen.
    let gewachsen = (0..6).filter(|i| nachher[*i] > vorher[*i] + probe).count();
    assert_eq!(gewachsen, 0, "ein Pod allein wurde sofort bezahlt: {vorher:?} -> {nachher:?}");
    let gehalten = &k.zustand().zurueckgehalten;
    assert!((1..=4).contains(&gehalten.len()), "zurückgehalten: {gehalten:?}");
    assert!(
        gehalten.iter().all(|(ab, _, v)| *ab == myl_ledger::einsatz::SPERRFRIST_EPOCHEN && *v > 0),
        "Freigabe nach der Streitfrist der Epoche 0: {gehalten:?}"
    );
}

/// ⚑ **Stufe 2 auf der Kette** (2026-10-29): Ein Miner ohne Pod wird
/// Checker jeder Ziehung, legt sich fest, deckt auf, der Auftrag schliesst
/// nach der Frist einig ab, und der Checker-Pool bezahlt ihn am nächsten
/// Wechsel nach seinem geprüften Volumen. Ein Folger, der jeden Block nur
/// empfängt, kommt auf dieselbe Wurzel.
#[test]
fn ein_checker_ohne_pod_legt_fest_deckt_auf_und_wird_bezahlt() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    use myl_types::miner::HardwareClass;
    use myl_types::node_metadata::GeoRegion;

    let mut k = Kette::probestand();
    // Ein Folger, der jeden Block nur empfängt: Er muss dieselbe
    // Ziehung kennen wie der Erzeuger.
    let mut folger = Kette::probestand();

    // Sieben Miner: ein Pod aus vier Positionen und zwei Reserve, einer ohne Pod.
    let mut nonce = [0u64; 7];
    for w in 0..7u8 {
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::MinerAnmelden {
                    hardware: HardwareClass::MediumGpu,
                    zone: GeoRegion::Europe,
                    netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
        k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
        nonce[w as usize] += 1;
    }
    // Und Brennstoff für die Prägung.
    k.aufnehmen(burn_nr(0, 5_000_000, nonce[0]));
    nonce[0] += 1;
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    for w in 0..7u8 {
        zusage_direkt(&mut k, myl_types::ids::MinerId::new(*probekonto(w).as_bytes()));
        zusage_direkt(&mut folger, myl_types::ids::MinerId::new(*probekonto(w).as_bytes()));
    }
    assert_eq!(k.zustand().miner.len(), 7, "die Anmeldungen kamen nicht an");
    assert!(k.zustand().burn_epoche > 0, "es wurde nichts verbrannt");

    // Auszahlungskonten eintragen, damit „ohne Eintrag kein Anteil"
    // nicht greift. Die erste Eintragung darf der Miner selbst.
    //
    // ⚑ **Über die Kette und nicht über den Übergang** (Fund 167,
    // 2026-09-03). Bis dahin rief dieser Test
    // `auszahlungskonto_eintragen` direkt, und das war der einzige
    // Weg, den es gab: Keine `Anweisung` trug ihn. Der Test war
    // grün und belegte einen Ablauf, den auf einem echten Netz
    // niemand gehen konnte. Jetzt geht er denselben Weg wie ein
    // Miner.
    for w in 0..7u8 {
        let kennung = myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::AuszahlungskontoEintragen {
                    kennung,
                    konto: kaltes_konto(w),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
    }
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    for w in 0..7u8 {
        let kennung = myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
        assert_eq!(
            k.zustand().auszahlung.get(&kennung),
            Some(&kaltes_konto(w)),
            "das Auszahlungskonto kam nicht ueber die Kette an"
        );
    }

    // Die Zuteilung dieser Epoche nachrechnen und für ihren Pod ein
    // Bündel einreichen.
    // ⚑ Die Saat aus dem Zustand, nicht der letzte Hash (Fund 143): Nur
    //   sie gilt die ganze Epoche, und nur gegen sie rechnet der
    //   Abschluss nach.
    let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    assert_eq!(zuteilung.pods.len(), 1, "es entstand kein Pod");
    let pod_index = zuteilung.pods[0].pod_index;
    let mut b = myl_types::PoIBundle {
        epoch: k.zustand().epoch,
        pod: myl_types::pod_kennung(k.zustand().epoch.0, pod_index),
        segments_root: myl_types::ids::MerkleRoot::new([7; 32]),
        vtfe_claimed: 1_000_000,
        aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
        // Tausend Segmente, damit die Ziehung ueberhaupt sichtbar ist
        // und die Ziehung ueberhaupt sichtbar ist.
        segmente: 1_000,
        ausgabenwurzel: myl_types::MerkleRoot::new([0u8; 32]),
        vertreter: Vec::new(),
        sitzungen: Vec::new(),
    };
    buendel_unterschreiben(&mut b, &zuteilung.pods[0]);
    // ⚑ **Eingereicht wird vom Koordinator**, seit die Aufnahme
    // ihn prüft (Fund 144). Vorher tat es Konto null, und das war
    // nur deshalb richtig, weil niemand hinsah.
    let koordinator = koordinator_von(&zuteilung.pods[0]);
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(koordinator),
            nonce[koordinator as usize],
            Anweisung::BuendelEinreichen { buendel: b },
        )
        .expect("signieren"),
    );
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    assert_eq!(k.zustand().buendel.len(), 1, "das Buendel kam nicht an");

    // Über die Epochengrenze.
    for _ in 2..=BLOECKE_JE_EPOCHE {
        folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    }
    assert_eq!(k.zustand().epoch.0, 1, "die Epoche wechselte nicht");
    // ⚑ **Gezogen wird erst mit dem Zertifikat** (2026-10-27, CONSENSUS
    //   7.2): Bis dahin hebt der Zustand die Anteile auf. ⛔️ Ohne
    //   Zertifikat keine Ziehung, und kein Rückfall auf den Blockhash.
    assert!(k.letzte_stichprobe().is_empty(), "gezogen ohne Zertifikat");
    assert_eq!(k.zustand().stichproben_offen.len(), 1);
    let z = pruefpunkt_von(&k, BLOECKE_JE_EPOCHE, PROBEERZEUGER);
    k.pruefpunkt_anbieten(z);
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    assert!(k.zustand().stichproben_offen.is_empty(), "die Ziehung wartet noch");

    // Der Miner ohne Pod ist der einzige Kandidat, also Checker jeder Ziehung.
    let ohne = zuteilung.ohne_pod.iter().map(|m| m.miner_id).collect::<Vec<_>>();
    assert_eq!(ohne.len(), 1, "ein Miner ohne Pod");
    let w = (0..7u8).find(|w| myl_types::ids::MinerId::new(*probekonto(*w).as_bytes()) == ohne[0]).expect("Probekonto");
    let auftraege = k.zustand().pruefauftraege.clone();
    assert_eq!(auftraege.len(), k.letzte_stichprobe().len(), "je Ziehung ein Auftrag");
    assert!(auftraege.iter().all(|a| a.pruefer == ohne), "der Checker ist der Miner ohne Pod");
    assert_eq!(folger.pruefer_der_stichprobe(), k.pruefer_der_stichprobe());
    assert_eq!(k.pruefer_der_stichprobe().len(), auftraege.len());

    let befund = myl_ledger::pruefung::Pruefbefund {
        logits: Hash::sha256(b"logits"),
        abweichung: None,
        positionen: 5,
    };
    let salz = [7u8; 32];
    let senden = |k: &mut Kette, nonce: &mut [u64; 7], a: Anweisung| {
        k.aufnehmen(
            Transaktion::signiere(&Kette::startwert(), &probeschluessel(w), nonce[w as usize], a).expect("signieren"),
        );
        nonce[w as usize] += 1;
    };
    let festlegung = myl_ledger::pruefung::festlegung(&befund, &salz, &ohne[0]);
    for teil in auftraege.chunks(myl_consensus::block::PRUEFUNGEN_JE_TX) {
        let festlegungen = teil.iter().map(|a| (a.epoche, a.pod, a.segment, festlegung)).collect();
        senden(&mut k, &mut nonce, Anweisung::PruefungFestlegen { festlegungen });
    }
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    assert!(k.zustand().pruefauftraege.iter().all(|a| a.festlegungen.len() == 1), "festgelegt");
    for teil in auftraege.chunks(myl_consensus::block::PRUEFUNGEN_JE_TX) {
        let aufdeckungen = teil.iter().map(|a| (a.epoche, a.pod, a.segment, befund.clone(), salz)).collect();
        senden(&mut k, &mut nonce, Anweisung::PruefungAufdecken { aufdeckungen });
    }
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    assert!(k.zustand().pruefauftraege.iter().all(|a| a.aufdeckungen.len() == 1), "aufgedeckt");
    // Über die zweite Frist: einig abgeschlossen, Volumen gutgeschrieben.
    let frist = auftraege.iter().map(|a| a.frist_aufdecken).max().unwrap_or(0);
    while k.zustand().historie.anzahl() <= frist {
        folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    }
    assert!(k.zustand().pruefauftraege.is_empty(), "abgeschlossen");
    let abgeschlossen = k.pruefausgaben_abholen().abschluesse;
    assert_eq!(abgeschlossen.len(), auftraege.len());
    assert!(abgeschlossen.iter().all(|a| a.ausgang == myl_ledger::pruefung::Pruefausgang::Einig));
    assert_eq!(k.zustand().pruefvolumen.get(&ohne[0]), Some(&(5 * auftraege.len() as u64)));
    // Am nächsten Wechsel zahlt der Checker-Pool, und das Volumen beginnt neu.
    let vorher = k.zustand().account(&kaltes_konto(w)).balance;
    let epoche = k.zustand().epoch.0;
    while k.zustand().epoch.0 == epoche {
        folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    }
    let probe = crate::netz::PROBEKETTE.probepraegung;
    let nachher = k.zustand().account(&kaltes_konto(w)).balance;
    assert!(nachher > vorher + probe, "der Checker-Pool zahlte nicht: {vorher} -> {nachher} (Probeprägung {probe})");
    assert!(k.zustand().pruefvolumen.is_empty());
    assert_eq!(folger.zustandswurzel(), k.zustandswurzel());
    let _ = BLOECKE_JE_EPOCHE;
}

/// ⚑ **Stufe 1 je Sitzung auf der Kette** (2026-11-16): Zwei Zwillinge,
/// dem einen fehlt die letzte Sitzung. Am Wechsel wird sofort vergütet,
/// was beide gleich haben; für die fehlende Sitzung entsteht ein
/// Vorbehalt und ein gebundener Pflichtauftrag. Decken die Checker sie
/// einig und ganz auf, zahlt der nächste Wechsel den Vorbehalt aus. Ein
/// Folger kommt auf dieselbe Wurzel.
///
/// ⛔️ Gegenprobe: Decken sie nur einen Teil der Sitzung auf, bleibt der
/// Vorbehalt gesperrt, und der nächste Wechsel zahlt ihn nicht.
#[test]
fn eine_fehlende_sitzung_wird_nachgerechnet_und_dann_bezahlt() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    use myl_types::miner::HardwareClass;
    use myl_types::node_metadata::GeoRegion;
    // ⚑ **Zwei Miner über die Probekonten hinaus.** Zwölf Miner füllen
    // zwei Pods zu sechs, und keiner bleibt ohne Pod: Dann gibt es
    // keinen Checker, kein Pflichtauftrag entsteht, und der Vorbehalt
    // verfällt. Erst die beiden weiteren machen die Nachrechnung möglich.
    const N: u8 = PROBEKONTEN + 2;

    fn geheim(w: u8) -> myl_types::bls::BlsSecretKey {
        if w < PROBEKONTEN {
            return probeschluessel(w);
        }
        myl_types::bls::BlsSecretKey::key_gen(Hash::sha256(format!("pruefer-{w}").as_bytes()).as_bytes())
            .expect("32 Byte sind für key_gen gültig")
    }
    fn konto(w: u8) -> Address {
        Address::aus_schluessel(&geheim(w).public_key().expect("gültiger Punkt"))
    }
    fn senden(k: &mut Kette, nonce: &mut [u64; N as usize], w: u8, a: Anweisung) {
        assert!(
            k.aufnehmen(Transaktion::signiere(&Kette::startwert(), &geheim(w), nonce[w as usize], a).expect("signieren")),
            "der Mempool nahm die Transaktion von {w} nicht"
        );
        nonce[w as usize] += 1;
    }
    // Liefert, ob der Vorbehalt frei wurde, und je Miner des Vorbehalts
    // den Zuwachs am zweiten Wechsel über die Probeprägung hinaus.
    let lauf = |aufgedeckt: u32| -> (bool, Vec<u64>) {
        let mut k = Kette::probestand();
        let mut folger = Kette::probestand();
        let kennung = |w: u8| myl_types::ids::MinerId::new(*konto(w).as_bytes());
        let nummer = |m: &myl_types::ids::MinerId| (0..N).find(|w| kennung(*w) == *m).expect("einer der vierzehn");
        let mut nonce = [0u64; N as usize];
        // Zwei Pods und dazu Miner ohne Pod, die prüfen. Die beiden
        // weiteren werden unmittelbar angemeldet, auf beiden Ketten
        // gleich, und bekommen ihr Guthaben für die Gebühren überwiesen.
        for w in PROBEKONTEN..N {
            for kette in [&mut k, &mut folger] {
                myl_ledger::transitions::miner_anmelden(
                    kette.zustand_mut(),
                    &konto(w),
                    &kennung(w),
                    HardwareClass::MediumGpu,
                    GeoRegion::Europe,
                    geheim(w).public_key().expect("gültiger Punkt"),
                    myl_types::latency_attest::PeerIdBytes([0; 32]),
                )
                .expect("Anmeldung");
            }
            senden(&mut k, &mut nonce, 0, Anweisung::Ueberweisung { nach: konto(w), betrag: 1_000_000 });
        }
        for w in 0..PROBEKONTEN {
            let anmelden = Anweisung::MinerAnmelden {
                hardware: HardwareClass::MediumGpu,
                zone: GeoRegion::Europe,
                netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
            };
            senden(&mut k, &mut nonce, w, anmelden);
            k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
            nonce[w as usize] += 1;
        }
        senden(&mut k, &mut nonce, 0, Anweisung::Burn { betrag: 5_000_000 });
        folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
        for w in 0..N {
            zusage_direkt(&mut k, kennung(w));
            zusage_direkt(&mut folger, kennung(w));
            senden(&mut k, &mut nonce, w, Anweisung::AuszahlungskontoEintragen { kennung: kennung(w), konto: kaltes_konto(w) });
        }
        folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
        assert_eq!(k.zustand().auszahlung.len(), N as usize, "die Auszahlungskonten kamen nicht an");

        let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
        assert_eq!(zuteilung.pods.len(), 2, "es entstanden keine zwei Pods");
        assert_eq!(myl_scheduler::zwillingsgruppen(&zuteilung.pods).len(), 1, "die beiden sind kein Paar");
        assert!(!zuteilung.ohne_pod.is_empty(), "kein Miner ohne Pod, also kein Checker");
        // A rechnete die Sitzungen 1, 2 und 3, B nur 1 und 2.
        let zeugnis = |sitzung: u64, segmente: u32| myl_types::Sitzungszeugnis {
            sitzung,
            segmente,
            wurzel: myl_types::MerkleRoot::new([sitzung as u8; 32]),
        };
        let (pod_a, pod_b) = (&zuteilung.pods[0], &zuteilung.pods[1]);
        let (ka, kb) = (myl_types::pod_kennung(0, pod_a.pod_index), myl_types::pod_kennung(0, pod_b.pod_index));
        for (pod, pk, sitzungen, wurzel) in
            [(pod_a, ka, vec![zeugnis(1, 4), zeugnis(2, 3), zeugnis(3, 3)], 1u8), (pod_b, kb, vec![zeugnis(1, 4), zeugnis(2, 3)], 2u8)]
        {
            let segmente: u32 = sitzungen.iter().map(|s| s.segmente).sum();
            let mut b = myl_types::PoIBundle {
                epoch: k.zustand().epoch,
                pod: pk,
                segments_root: myl_types::ids::MerkleRoot::new([wurzel; 32]),
                vtfe_claimed: 100_000 * u64::from(segmente),
                aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
                segmente,
                ausgabenwurzel: myl_types::MerkleRoot::new([wurzel; 32]),
                vertreter: Vec::new(),
                sitzungen,
            };
            // Unterschrieben von den Positionen des Pods, wie
            // `buendel_unterschreiben`, nur über alle vierzehn Schlüssel.
            let msg = myl_consensus::poi::bundle_message(&b);
            let teile: Vec<_> = pod
                .shards
                .iter()
                .map(|s| geheim(nummer(&s.miner.miner_id)).sign(&msg).expect("Unterschrift"))
                .collect();
            b.aggregate_sig = myl_types::bls::BlsSignature(myl_types::bls::aggregate_signatures(&teile).expect("Aggregat").0);
            senden(&mut k, &mut nonce, nummer(&pod.shards[0].miner.miner_id), Anweisung::BuendelEinreichen { buendel: b });
        }
        folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
        assert_eq!(k.zustand().buendel.len(), 2, "die Buendel kamen nicht an");

        // Erster Wechsel: Gleiches sofort, die fehlende Sitzung als Vorbehalt.
        let stand = |k: &Kette| (0..N).map(|w| k.zustand().account(&kaltes_konto(w)).balance).collect::<Vec<u64>>();
        let probe = crate::netz::PROBEKETTE.probepraegung;
        let vorher = stand(&k);
        for _ in 2..=BLOECKE_JE_EPOCHE {
            folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
        }
        assert_eq!(k.zustand().epoch.0, 1, "die Epoche wechselte nicht");
        let nachher = stand(&k);
        for pod in [pod_a, pod_b] {
            let w = nummer(&pod.shards[0].miner.miner_id) as usize;
            assert!(nachher[w] > vorher[w] + probe, "die gleichen Sitzungen wurden nicht sofort bezahlt");
        }
        let pflicht: Vec<_> = k.zustand().vorbehalte.iter().filter(|v| v.pflicht.is_some()).cloned().collect();
        assert_eq!(pflicht.len(), 1, "ein Vorbehalt für die fehlende Sitzung: {:?}", k.zustand().vorbehalte);
        let v = &pflicht[0];
        // Die Stelle ist das letzte Segment der Sitzung 3: 4 + 3 + 3 - 1.
        assert_eq!((v.epoche, v.pod, v.zwilling, v.pflicht, v.frei), (0, ka, kb, Some(9), false));
        assert!(v.je_miner.iter().all(|(_, betrag)| *betrag > 0) && !v.je_miner.is_empty());
        assert_eq!(k.zustand().stichproben_offen.len(), 1);
        assert_eq!(k.zustand().stichproben_offen[0].pflicht, vec![(ka, 9, 3, 3)]);

        // Die Ziehung legt den gebundenen Auftrag an.
        let z = pruefpunkt_von(&k, BLOECKE_JE_EPOCHE, PROBEERZEUGER);
        k.pruefpunkt_anbieten(z);
        folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
        assert!(k.letzte_stichprobe().iter().any(|x| x.pod == ka && x.segment == 9), "die Pflichtstelle wurde nicht gezogen");
        let auftrag = k.zustand().pruefauftraege.iter().find(|a| a.pod == ka && a.segment == 9).cloned().expect("der Pflichtauftrag");
        assert_eq!(auftrag.gebunden, Some((3, 3)));
        assert!(!auftrag.pruefer.is_empty(), "kein Checker für den Pflichtauftrag");
        assert!(k.zustand().pruefauftraege.iter().filter(|a| a.gebunden.is_some()).count() == 1);

        // Die Checker legen fest und decken auf.
        let befund = myl_ledger::pruefung::Pruefbefund { logits: Hash::sha256(b"logits"), abweichung: None, positionen: aufgedeckt };
        let salz = [7u8; 32];
        for c in &auftrag.pruefer {
            let festlegungen = vec![(0, ka, 9, myl_ledger::pruefung::festlegung(&befund, &salz, c))];
            senden(&mut k, &mut nonce, nummer(c), Anweisung::PruefungFestlegen { festlegungen });
        }
        folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
        for c in &auftrag.pruefer {
            senden(&mut k, &mut nonce, nummer(c), Anweisung::PruefungAufdecken { aufdeckungen: vec![(0, ka, 9, befund.clone(), salz)] });
        }
        folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
        while k.zustand().historie.anzahl() <= auftrag.frist_aufdecken {
            folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
        }
        assert!(!k.zustand().pruefauftraege.iter().any(|a| a.pod == ka && a.segment == 9), "abgeschlossen");
        let frei = k.zustand().vorbehalte.iter().any(|x| x.pflicht == Some(9) && x.frei);

        // Zweiter Wechsel.
        let vorher = stand(&k);
        while k.zustand().epoch.0 == 1 {
            folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
        }
        let nachher = stand(&k);
        assert_eq!(folger.zustandswurzel(), k.zustandswurzel(), "der Folger weicht ab");
        assert_eq!(k.zustand().vorbehalte.iter().any(|x| x.pflicht == Some(9)), !frei, "bezahlt ist vergessen, gesperrt bleibt");
        let zuwachs = v
            .je_miner
            .iter()
            .map(|(m, _)| {
                let w = nummer(m) as usize;
                nachher[w].saturating_sub(vorher[w]).saturating_sub(probe)
            })
            .collect();
        (frei, zuwachs)
    };

    let (frei, zuwachs) = lauf(3);
    assert!(frei, "einig und ganz aufgedeckt, aber nicht frei");
    assert!(zuwachs.iter().all(|mehr| *mehr > 0), "der Vorbehalt wurde nicht ausgezahlt: {zuwachs:?}");
    // ⛔️ Gegenprobe: nur zwei der drei Positionen aufgedeckt.
    let (frei, zuwachs) = lauf(2);
    assert!(!frei, "ein Teil der Sitzung gab den Vorbehalt frei");
    assert!(zuwachs.iter().all(|mehr| *mehr == 0), "gesperrt und doch gezahlt: {zuwachs:?}");
}

/// ⚑ **T6d auf der Kette** (2026-10-29): Der Checker deckt bei einer
/// Ziehung eine Abweichung an Position 0, Eintrag 1 auf (Ausgang der
/// Layer 0). Der Abschluss öffnet einen Streitfall gegen das Mitglied mit
/// Layer 0, ein Validator stimmt „Pod falsch“, und die Kette schlachtet
/// den Beschuldigten nach der Matrix (100 % des Einsatzes); der
/// Anfechter bekommt das Kopfgeld. Ein Folger kommt auf dieselbe Wurzel.
#[test]
fn ein_streitfall_schlachtet_den_beschuldigten_und_zahlt_dem_anfechter_kopfgeld() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    use myl_types::miner::HardwareClass;
    use myl_types::node_metadata::GeoRegion;

    let mut k = Kette::probestand();
    // Ein Folger, der jeden Block nur empfängt: Er muss dieselbe
    // Ziehung kennen wie der Erzeuger.
    let mut folger = Kette::probestand();

    // Sieben Miner: ein Pod aus vier Positionen und zwei Reserve, einer ohne Pod.
    let mut nonce = [0u64; 7];
    for w in 0..7u8 {
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::MinerAnmelden {
                    hardware: HardwareClass::MediumGpu,
                    zone: GeoRegion::Europe,
                    netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
        k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
        nonce[w as usize] += 1;
    }
    // Und Brennstoff für die Prägung.
    k.aufnehmen(burn_nr(0, 5_000_000, nonce[0]));
    nonce[0] += 1;
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    for w in 0..7u8 {
        zusage_direkt(&mut k, myl_types::ids::MinerId::new(*probekonto(w).as_bytes()));
        zusage_direkt(&mut folger, myl_types::ids::MinerId::new(*probekonto(w).as_bytes()));
    }
    assert_eq!(k.zustand().miner.len(), 7, "die Anmeldungen kamen nicht an");
    assert!(k.zustand().burn_epoche > 0, "es wurde nichts verbrannt");

    // Auszahlungskonten eintragen, damit „ohne Eintrag kein Anteil"
    // nicht greift. Die erste Eintragung darf der Miner selbst.
    //
    // ⚑ **Über die Kette und nicht über den Übergang** (Fund 167,
    // 2026-09-03). Bis dahin rief dieser Test
    // `auszahlungskonto_eintragen` direkt, und das war der einzige
    // Weg, den es gab: Keine `Anweisung` trug ihn. Der Test war
    // grün und belegte einen Ablauf, den auf einem echten Netz
    // niemand gehen konnte. Jetzt geht er denselben Weg wie ein
    // Miner.
    for w in 0..7u8 {
        let kennung = myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::AuszahlungskontoEintragen {
                    kennung,
                    konto: kaltes_konto(w),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
    }
    // Jeder hinterlegt 2 MYL: Ohne Einsatz gäbe es nichts zu schlachten.
    for w in 0..7u8 {
        k.aufnehmen(
            Transaktion::signiere(&Kette::startwert(), &probeschluessel(w), nonce[w as usize], Anweisung::EinsatzHinterlegen { betrag: 2_000_000 })
                .expect("signieren"),
        );
        nonce[w as usize] += 1;
    }
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    for w in 0..7u8 {
        let kennung = myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
        assert_eq!(
            k.zustand().auszahlung.get(&kennung),
            Some(&kaltes_konto(w)),
            "das Auszahlungskonto kam nicht ueber die Kette an"
        );
    }

    // Die Zuteilung dieser Epoche nachrechnen und für ihren Pod ein
    // Bündel einreichen.
    // ⚑ Die Saat aus dem Zustand, nicht der letzte Hash (Fund 143): Nur
    //   sie gilt die ganze Epoche, und nur gegen sie rechnet der
    //   Abschluss nach.
    let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    assert_eq!(zuteilung.pods.len(), 1, "es entstand kein Pod");
    let pod_index = zuteilung.pods[0].pod_index;
    let mut b = myl_types::PoIBundle {
        epoch: k.zustand().epoch,
        pod: myl_types::pod_kennung(k.zustand().epoch.0, pod_index),
        segments_root: myl_types::ids::MerkleRoot::new([7; 32]),
        vtfe_claimed: 1_000_000,
        aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
        // Tausend Segmente, damit die Ziehung ueberhaupt sichtbar ist
        // und die Ziehung ueberhaupt sichtbar ist.
        segmente: 1_000,
        ausgabenwurzel: myl_types::MerkleRoot::new([0u8; 32]),
        vertreter: Vec::new(),
        sitzungen: Vec::new(),
    };
    buendel_unterschreiben(&mut b, &zuteilung.pods[0]);
    // ⚑ **Eingereicht wird vom Koordinator**, seit die Aufnahme
    // ihn prüft (Fund 144). Vorher tat es Konto null, und das war
    // nur deshalb richtig, weil niemand hinsah.
    let koordinator = koordinator_von(&zuteilung.pods[0]);
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(koordinator),
            nonce[koordinator as usize],
            Anweisung::BuendelEinreichen { buendel: b },
        )
        .expect("signieren"),
    );
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    assert_eq!(k.zustand().buendel.len(), 1, "das Buendel kam nicht an");

    // Über die Epochengrenze.
    for _ in 2..=BLOECKE_JE_EPOCHE {
        folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    }
    assert_eq!(k.zustand().epoch.0, 1, "die Epoche wechselte nicht");
    // ⚑ **Gezogen wird erst mit dem Zertifikat** (2026-10-27, CONSENSUS
    //   7.2): Bis dahin hebt der Zustand die Anteile auf. ⛔️ Ohne
    //   Zertifikat keine Ziehung, und kein Rückfall auf den Blockhash.
    assert!(k.letzte_stichprobe().is_empty(), "gezogen ohne Zertifikat");
    assert_eq!(k.zustand().stichproben_offen.len(), 1);
    let z = pruefpunkt_von(&k, BLOECKE_JE_EPOCHE, PROBEERZEUGER);
    k.pruefpunkt_anbieten(z);
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    assert!(k.zustand().stichproben_offen.is_empty(), "die Ziehung wartet noch");

    // Der Miner ohne Pod ist der einzige Kandidat, also Checker jeder Ziehung.
    let ohne = zuteilung.ohne_pod.iter().map(|m| m.miner_id).collect::<Vec<_>>();
    assert_eq!(ohne.len(), 1, "ein Miner ohne Pod");
    let w = (0..7u8).find(|w| myl_types::ids::MinerId::new(*probekonto(*w).as_bytes()) == ohne[0]).expect("Probekonto");
    let auftraege = k.zustand().pruefauftraege.clone();
    assert_eq!(auftraege.len(), k.letzte_stichprobe().len(), "je Ziehung ein Auftrag");
    assert!(auftraege.iter().all(|a| a.pruefer == ohne), "der Checker ist der Miner ohne Pod");
    assert_eq!(folger.pruefer_der_stichprobe(), k.pruefer_der_stichprobe());
    assert_eq!(k.pruefer_der_stichprobe().len(), auftraege.len());

    let gleich = myl_ledger::pruefung::Pruefbefund { logits: Hash::sha256(b"logits"), abweichung: None, positionen: 3 };
    let falsch = myl_ledger::pruefung::Pruefbefund { abweichung: Some((0, 1)), ..gleich.clone() };
    let salz = [9u8; 32];
    let befund_von = |i: usize| if i == 0 { falsch.clone() } else { gleich.clone() };
    let senden = |k: &mut Kette, nonce: &mut [u64; 7], wer: u8, a: Anweisung| {
        k.aufnehmen(
            Transaktion::signiere(&Kette::startwert(), &probeschluessel(wer), nonce[wer as usize], a).expect("signieren"),
        );
        nonce[wer as usize] += 1;
    };
    for (t, teil) in auftraege.chunks(myl_consensus::block::PRUEFUNGEN_JE_TX).enumerate() {
        let festlegungen = teil
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let b = befund_von(t * myl_consensus::block::PRUEFUNGEN_JE_TX + i);
                (a.epoche, a.pod, a.segment, myl_ledger::pruefung::festlegung(&b, &salz, &ohne[0]))
            })
            .collect();
        senden(&mut k, &mut nonce, w, Anweisung::PruefungFestlegen { festlegungen });
    }
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    for (t, teil) in auftraege.chunks(myl_consensus::block::PRUEFUNGEN_JE_TX).enumerate() {
        let aufdeckungen = teil
            .iter()
            .enumerate()
            .map(|(i, a)| (a.epoche, a.pod, a.segment, befund_von(t * myl_consensus::block::PRUEFUNGEN_JE_TX + i), salz))
            .collect();
        senden(&mut k, &mut nonce, w, Anweisung::PruefungAufdecken { aufdeckungen });
    }
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    let frist = auftraege.iter().map(|a| a.frist_aufdecken).max().unwrap_or(0);
    while k.zustand().historie.anzahl() <= frist {
        folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    }
    // Der Streitfall: gegen das Mitglied mit Layer 0.
    assert_eq!(k.zustand().streitfaelle.len(), 1, "ein Streitfall");
    let fall = k.zustand().streitfaelle[0].clone();
    assert_eq!((fall.position, fall.eintrag, fall.anfechter), (0, 1, ohne[0]));
    let erster = auftraege[0].besetzung.iter().find(|(_, von, _)| *von == 0).map(|(m, _, _)| *m);
    assert!(erster.is_some() && fall.beschuldigt == erster, "beschuldigt ist, wer Layer 0 rechnet");
    let beschuldigt = Address::new(*fall.beschuldigt.unwrap_or(ohne[0]).as_bytes());
    // Ein Validator, der weder Anfechter noch Beschuldigter ist, mit so
    // viel Gewicht, dass seine Stimme allein mehr als zwei Drittel ist.
    let v = (0..7u8)
        .find(|i| {
            let a = probekonto(*i);
            a != beschuldigt && a != probekonto(w)
        })
        .expect("ein Dritter");
    let mut mitglieder = std::collections::BTreeMap::new();
    if let Some(alt) = k.stimmsatz() {
        for (m, mitglied) in alt.members() {
            mitglieder.insert(*m, myl_consensus::validator::VotingMember { pubkey: mitglied.pubkey, weight: 1 });
        }
    }
    let vk = myl_types::ids::MinerId::new(*probekonto(v).as_bytes());
    mitglieder.insert(
        vk,
        myl_consensus::validator::VotingMember { pubkey: probeschluessel(v).public_key().expect("Schlüssel"), weight: 100 },
    );
    let satz = myl_consensus::validator::VotingSet::from_members(mitglieder);
    k.stimmsatz_setzen(satz.clone());
    folger.stimmsatz_setzen(satz);
    let vorher_einsatz = k.zustand().account(&beschuldigt).staked;
    let vorher_anfechter = k.zustand().account(&probekonto(w)).balance;
    assert!(vorher_einsatz > 0, "der Beschuldigte hat Einsatz");
    // ⛔️ Wer nicht im Stimmsatz steht, zählt nicht.
    let fremd = (0..7u8).find(|i| *i != v && *i != w).expect("ein Fremder");
    senden(&mut k, &mut nonce, fremd, Anweisung::SchiedsstimmeAbgeben { epoche: fall.epoche, pod: fall.pod, segment: fall.segment, pod_falsch: false });
    senden(&mut k, &mut nonce, v, Anweisung::SchiedsstimmeAbgeben { epoche: fall.epoche, pod: fall.pod, segment: fall.segment, pod_falsch: true });
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    assert_eq!(k.zustand().streitfaelle.first().map(|f| f.stimmen.len()), Some(1), "nur die berechtigte Stimme");
    folger.uebernimm(&k.baue_block()).expect("der Folger nimmt den Block");
    assert!(k.zustand().streitfaelle.is_empty(), "entschieden");
    let urteile = k.pruefausgaben_abholen().urteile;
    assert_eq!(urteile.len(), 1);
    assert_eq!(urteile[0].0.ausgang, myl_ledger::pruefung::Schiedsausgang::PodFalsch);
    assert_eq!(urteile[0].1, None, "gebucht");
    assert_eq!(k.zustand().account(&beschuldigt).staked, 0, "100 % des Einsatzes");
    let kopfgeld = k.zustand().account(&probekonto(w)).balance.saturating_sub(vorher_anfechter);
    assert!(kopfgeld >= vorher_einsatz * 3 / 10 - 10_000, "Kopfgeld 30 %: {kopfgeld} von {vorher_einsatz}");
    assert_eq!(folger.zustandswurzel(), k.zustandswurzel());
}


/// ⚑ **Punkt 40, Glied 2: Ein Bündel ohne gültige Aggregatsignatur
/// zahlt nichts aus.**
///
/// Der Pod ist echt, die Kennung stimmt, die Epoche stimmt, und der
/// Einreicher ist angemeldet. **Nur die Unterschrift der Mitglieder
/// fehlt**, und genau das schließt die Lücke, die bis hierher offen
/// war: Ein angemeldeter Miner konnte ein Bündel für einen fremden
/// Pod einreichen.
#[test]
fn ein_buendel_ohne_gueltige_unterschrift_zahlt_nichts_aus() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    use myl_types::miner::HardwareClass;
    use myl_types::node_metadata::GeoRegion;

    // ⚑ Keine Verteilung mehr zu setzen (Fund 161): sie ist immer da.
    let mut k = Kette::probestand();

    let mut nonce = [0u64; 6];
    for w in 0..6u8 {
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::MinerAnmelden {
                    hardware: HardwareClass::MediumGpu,
                    zone: GeoRegion::Europe,
                    netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
        k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
        nonce[w as usize] += 1;
    }
    k.aufnehmen(burn_nr(0, 5_000_000, nonce[0]));
    nonce[0] += 1;
    k.baue_block();
    for w in 0..6u8 {
        zusage_direkt(&mut k, myl_types::ids::MinerId::new(*probekonto(w).as_bytes()));
    }
    for w in 0..6u8 {
        let kennung = myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
        myl_ledger::transitions::auszahlungskonto_eintragen(
            k.zustand_mut(),
            &probekonto(w),
            &kennung,
            kaltes_konto(w),
        )
        .expect("Eintragung");
    }
    // ⚑ Die Saat aus dem Zustand, nicht der letzte Hash (Fund 143): Nur
    //   sie gilt die ganze Epoche, und nur gegen sie rechnet der
    //   Abschluss nach.
    let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    // ⚑ Echter Pod, echte Kennung, **Attrappe als Unterschrift**.
    let b = myl_types::PoIBundle {
        epoch: k.zustand().epoch,
        pod: myl_types::pod_kennung(k.zustand().epoch.0, zuteilung.pods[0].pod_index),
        segments_root: myl_types::ids::MerkleRoot::new([7; 32]),
        vtfe_claimed: 1_000_000,
        aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
        segmente: 1,
        ausgabenwurzel: myl_types::MerkleRoot::new([0u8; 32]),
        vertreter: Vec::new(),
        sitzungen: Vec::new(),
    };
    // ⚑ **Eingereicht wird vom Koordinator**, seit die Aufnahme
    // ihn prüft (Fund 144). Vorher tat es Konto null, und das war
    // nur deshalb richtig, weil niemand hinsah.
    let koordinator = koordinator_von(&zuteilung.pods[0]);
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(koordinator),
            nonce[koordinator as usize],
            Anweisung::BuendelEinreichen { buendel: b },
        )
        .expect("signieren"),
    );
    k.baue_block();
    // ⚑ **Seit Fund 144 kommt es gar nicht erst hinein.** Vorher
    // stand es im Zustand und zahlte dort nur nichts aus.
    assert!(
        k.zustand().buendel.is_empty(),
        "ein Buendel ohne gueltige Unterschrift kam in den Zustand"
    );

    for _ in 0..BLOECKE_JE_EPOCHE * 2 {
        k.baue_block();
        if k.zustand().epoch.0 == 1 {
            break;
        }
    }
    assert_eq!(k.zustand().epoch.0, 1, "die Epoche wechselte nicht");
    for w in 0..6u8 {
        // Nur die Probeprägung, nichts aus der Ausschüttung.
        assert_eq!(
            k.zustand().account(&kaltes_konto(w)).balance,
            crate::netz::PROBEKETTE.probepraegung,
            "ein Buendel ohne gueltige Unterschrift hat ausgezahlt"
        );
    }
}

// ⚑ **Der Test `ohne_arbeitsverteilung_bekommt_niemand_etwas` ist
// am 2026-09-03 entfernt worden** (Fund 161), zusammen mit dem
// Zustand, den er prüfte: Es gibt kein „ohne Verteilung" mehr,
// seit sie gerechnet statt gesetzt wird. Was von seiner Prüfung
// bleibt, deckt jetzt `ein_buendel_ohne_gueltige_unterschrift_zahlt_nichts_aus`
// ab: Ein Bündel ohne gültiges Aggregat zahlt nichts aus.

/// ⚑ **Ein Bündel mit erfundener Pod-Kennung zahlt nichts aus.**
///
/// 📌 Ohne diesen Test war die Kennungssuche in der Kette ungeprüft:
/// Die Gegenprobe „nimm einfach den ersten Pod" blieb grün, weil in
/// den übrigen Tests nur **ein** Pod entsteht. Hier scheitert sie.
///
/// ⚑ **Seit Fund 144 ist die Aussage schärfer geworden.** Bis zum
/// 2026-09-02 kam ein erfundenes Bündel in den Zustand und zahlte
/// dort nur nichts aus; jetzt kommt es gar nicht erst hinein, weil
/// die Aufnahme die Besetzung nachschlägt. Der Test prüft beides:
/// nicht im Zustand **und** kein Konto gewachsen.
#[test]
fn ein_buendel_mit_erfundener_kennung_zahlt_nichts_aus() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    use myl_types::miner::HardwareClass;
    use myl_types::node_metadata::GeoRegion;

    // ⚑ Keine Verteilung mehr zu setzen (Fund 161): sie ist immer da.
    let mut k = Kette::probestand();

    let mut nonce = [0u64; 6];
    for w in 0..6u8 {
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::MinerAnmelden {
                    hardware: HardwareClass::MediumGpu,
                    zone: GeoRegion::Europe,
                    netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
        k.aufnehmen(probezusage(w, EpochId(1), nonce[w as usize]));
        nonce[w as usize] += 1;
    }
    k.aufnehmen(burn_nr(0, 5_000_000, nonce[0]));
    nonce[0] += 1;
    k.baue_block();
    for w in 0..6u8 {
        zusage_direkt(&mut k, myl_types::ids::MinerId::new(*probekonto(w).as_bytes()));
    }
    for w in 0..6u8 {
        let kennung = myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
        myl_ledger::transitions::auszahlungskonto_eintragen(
            k.zustand_mut(),
            &probekonto(w),
            &kennung,
            kaltes_konto(w),
        )
        .expect("Eintragung");
    }

    // ⚑ Eine Kennung, die zu keiner Platznummer dieser Epoche gehört.
    let erfunden = myl_types::ids::PodId::new([0xAB; 32]);
    let b = myl_types::PoIBundle {
        epoch: k.zustand().epoch,
        pod: erfunden,
        segments_root: myl_types::ids::MerkleRoot::new([7; 32]),
        vtfe_claimed: 1_000_000,
        aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
        segmente: 1,
        ausgabenwurzel: myl_types::MerkleRoot::new([0u8; 32]),
        vertreter: Vec::new(),
        sitzungen: Vec::new(),
    };
    // Eingereicht wird von einem angemeldeten Miner, damit der
    // Test nicht schon an der Anmeldung scheitert.
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(0),
            nonce[0],
            Anweisung::BuendelEinreichen { buendel: b },
        )
        .expect("signieren"),
    );
    k.baue_block();
    assert!(
        k.zustand().buendel.is_empty(),
        "ein Buendel fuer einen Pod, den es nicht gibt, kam in den Zustand"
    );

    for _ in 0..BLOECKE_JE_EPOCHE * 2 {
        k.baue_block();
        if k.zustand().epoch.0 == 1 {
            break;
        }
    }
    assert_eq!(k.zustand().epoch.0, 1, "die Epoche wechselte nicht");
    for w in 0..6u8 {
        // Nur die Probeprägung, nichts aus der Ausschüttung.
        assert_eq!(
            k.zustand().account(&kaltes_konto(w)).balance,
            crate::netz::PROBEKETTE.probepraegung,
            "ein erfundenes Buendel hat ausgezahlt"
        );
    }
}

fn kaltes_konto(w: u8) -> Address {
    Address::new([200 + w; 32])
}

/// ⚑ **Die Probeprägung** (2026-10-26): Am Epochenwechsel bekommt das
/// Auszahlungskonto jedes angemeldeten Miners den Betrag der
/// Probekette, ein gesperrter in der Epoche seiner Sperre nichts, einer
/// ohne Eintrag nichts. Ohne Brennstoff und ohne Bündel, also nichts aus
/// der echten Ausschüttung.
#[test]
fn die_probekette_praegt_je_epoche_ins_auszahlungskonto() {
    use myl_consensus::block::BLOECKE_JE_EPOCHE;
    use myl_types::miner::HardwareClass;
    use myl_types::node_metadata::GeoRegion;

    let mut k = Kette::probestand();
    let kennung = |w: u8| myl_types::ids::MinerId::new(*probekonto(w).as_bytes());
    let mut nonce = [0u64; 4];
    for w in 0..4u8 {
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::MinerAnmelden {
                    hardware: HardwareClass::MediumGpu,
                    zone: GeoRegion::Europe,
                    netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
    }
    k.baue_block();
    // Drei tragen ein Konto ein, der vierte nicht.
    for w in 0..3u8 {
        k.aufnehmen(
            Transaktion::signiere(
                &Kette::startwert(),
                &probeschluessel(w),
                nonce[w as usize],
                Anweisung::AuszahlungskontoEintragen { kennung: kennung(w), konto: kaltes_konto(w) },
            )
            .expect("signieren"),
        );
        nonce[w as usize] += 1;
    }
    k.baue_block();
    assert_eq!(k.zustand().auszahlung.len(), 3, "die Auszahlungskonten kamen nicht an");
    // Der dritte ist in der laufenden Epoche gesperrt.
    k.zustand_mut().fristsperren.insert(kennung(2), EpochId(0));
    let vorher: Vec<u64> = (0..4u8)
        .map(|w| k.zustand().account(&kaltes_konto(w)).balance)
        .chain([k.zustand().account(&probekonto(3)).balance])
        .collect();
    while k.zustand().epoch.0 == 0 {
        k.baue_block();
    }
    assert!(k.hoehe() <= BLOECKE_JE_EPOCHE + 2, "die Epoche wechselte spät");
    let nachher: Vec<u64> = (0..4u8)
        .map(|w| k.zustand().account(&kaltes_konto(w)).balance)
        .chain([k.zustand().account(&probekonto(3)).balance])
        .collect();
    let betrag = crate::netz::PROBEKETTE.probepraegung;
    assert_eq!(nachher[0], vorher[0] + betrag, "das erste Konto bekam nicht den Betrag");
    assert_eq!(nachher[1], vorher[1] + betrag, "das zweite Konto bekam nicht den Betrag");
    assert_eq!(nachher[2], vorher[2], "der Gesperrte bekam etwas");
    assert_eq!(nachher[3], vorher[3], "ein Konto ohne Eintrag bekam etwas");
    assert_eq!(nachher[4], vorher[4], "der Miner ohne Eintrag bekam etwas");

    // ⛔️ Gegenprobe: derselbe Zustand im Testnetz prägt nichts.
    let mut z = k.zustand().clone();
    z.podmodell = Some(crate::netz::TESTNETZ_INT4.podmodell);
    let vor: Vec<u64> = (0..3u8).map(|w| z.account(&kaltes_konto(w)).balance).collect();
    Kette::probepraegung(&mut z, 1);
    let nach: Vec<u64> = (0..3u8).map(|w| z.account(&kaltes_konto(w)).balance).collect();
    assert_eq!(vor, nach, "das Testnetz prägte zur Probe");
    z.account_mut(&probekonto(3)).balance = 5;
    Kette::probepraegung(&mut z, 1);
    assert_eq!(z.account(&probekonto(3)).balance, 5, "das Testnetz fuellte ein Probekonto auf");
}

/// 📌 **Fund 675: Der Epochenwechsel fuellt ein Probekonto unter der
/// Haelfte auf.** Ein Konto knapp darueber bleibt, wie es ist, und mehr
/// als das Startguthaben entsteht nicht.
#[test]
fn der_wechsel_fuellt_ein_leeres_probekonto_auf() {
    let mut z = Kette::probestand().zustand().clone();
    z.account_mut(&probekonto(1)).balance = 68;
    z.account_mut(&probekonto(2)).balance = PROBEGUTHABEN / 2;
    Kette::probepraegung(&mut z, 1);
    assert_eq!(z.account(&probekonto(1)).balance, PROBEGUTHABEN, "das leere Konto blieb leer");
    assert_eq!(z.account(&probekonto(2)).balance, PROBEGUTHABEN / 2, "das halbvolle Konto wurde aufgefuellt");
    assert!((0..PROBEKONTEN).all(|n| z.account(&probekonto(n)).balance <= PROBEGUTHABEN));
}

/// ⚑ **Fund 167: das Auszahlungskonto hat einen Weg über die Kette,
/// und die Berechtigungsregel gilt auf ihm.**
///
/// Drei Lagen in einem Test, weil sie zusammen die Regel ergeben:
/// Die **erste** Eintragung darf der Miner selbst, jede **weitere**
/// nur das eingetragene kalte Konto, und ein **Fremder** nie.
///
/// 📌 Ohne diesen Test wäre die neue `Anweisung` eine Variante, die
/// der Übersetzer kennt und kein Block anwendet: genau die Klasse,
/// aus der der Fund kam.
#[test]
fn ein_auszahlungskonto_geht_ueber_die_kette_und_nur_der_richtige_darf() {
    let mut k = Kette::probestand();
    let kennung = myl_types::ids::MinerId::new(*probekonto(0).as_bytes());

    // ⚑ **Das kalte Konto ist hier ein Probekonto und keine nackte
    // Adresse**, denn der positive Fall „das kalte Konto ändert" ist
    // die Hälfte der Regel, und dafür muss jemand unterschreiben
    // können. Eine `Address::new([...])` kann das nicht.
    let kalt = probekonto(5);

    // 1. Die erste Eintragung, vom Miner selbst.
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(0),
            0,
            Anweisung::AuszahlungskontoEintragen { kennung, konto: kalt },
        )
        .expect("signieren"),
    );
    k.baue_block();
    assert_eq!(
        k.zustand().auszahlung.get(&kennung),
        Some(&kalt),
        "die erste Eintragung kam nicht an"
    );

    // 2. Ein Fremder will umleiten. ⚑ **Genau der Angriff, gegen den
    // die Trennung steht**: ein gestohlener Konsensschlüssel, der
    // den Ertrag woanders hin schickt.
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(3),
            0,
            Anweisung::AuszahlungskontoEintragen {
                kennung,
                konto: kaltes_konto(3),
            },
        )
        .expect("signieren"),
    );
    k.baue_block();
    assert_eq!(
        k.zustand().auszahlung.get(&kennung),
        Some(&kalt),
        "ein Fremder hat das Auszahlungskonto umgeleitet"
    );

    // ⚑ **Und der Miner selbst darf es jetzt auch nicht mehr.**
    // Nach der ersten Eintragung gehört die Änderung dem kalten
    // Konto; sonst nützte die Trennung nichts.
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(0),
            1,
            Anweisung::AuszahlungskontoEintragen {
                kennung,
                konto: kaltes_konto(4),
            },
        )
        .expect("signieren"),
    );
    k.baue_block();
    assert_eq!(
        k.zustand().auszahlung.get(&kennung),
        Some(&kalt),
        "der heisse Schluessel konnte nach der ersten Eintragung umleiten"
    );

    // 3. Das eingetragene kalte Konto darf. **Ohne diese Hälfte
    // wäre die Regel eine Sperre und keine Trennung:** Wer sein
    // kaltes Konto verliert, käme nie wieder an seine Erträge.
    k.aufnehmen(
        Transaktion::signiere(
            &Kette::startwert(),
            &probeschluessel(5),
            0,
            Anweisung::AuszahlungskontoEintragen {
                kennung,
                konto: kaltes_konto(9),
            },
        )
        .expect("signieren"),
    );
    k.baue_block();
    assert_eq!(
        k.zustand().auszahlung.get(&kennung),
        Some(&kaltes_konto(9)),
        "das eingetragene Konto durfte seine eigene Eintragung nicht aendern"
    );
}

/// ⚑ **Die Kapazitätszusage wirkt über die Kette** (Stufe 4,
/// 2026-10-21): angemeldet, dann eingereicht, steht sie im Zustand; eine
/// mit fremder Unterschrift nicht.
#[test]
fn eine_zusage_ueber_die_kette_wirkt() {
    use myl_types::zusage::{Kapazitaetszusage, Modellzusage};
    let mut k = Kette::probestand();
    k.aufnehmen(anmeldung(0, myl_types::miner::HardwareClass::SmallGpu));
    k.baue_block();
    let sk = probeschluessel(0);
    let halter = myl_types::ids::MinerId::aus_schluessel(&sk.public_key().expect("pk"));
    let ab = EpochId(k.zustand().epoch.0 + 1);
    let abdruck = crate::netz::PROBEKETTE.podmodell.abdruck;
    let modell = Modellzusage { abdruck, ebenen_hoechstens: 30, ebenenzeit_us: 900, gehalten: None };
    let mut gut = Kapazitaetszusage::neu(halter, ab, 0).mit_modell(modell);
    gut.signiere(&sk).expect("signieren");
    let mut fremd = Kapazitaetszusage::neu(halter, EpochId(ab.0 + 1), 0).mit_modell(modell);
    fremd.signiere(&probeschluessel(1)).expect("signieren");
    // ⚑ Ein Modell, das dieses Netz nicht rechnet (6.6).
    let mut anderes = Kapazitaetszusage::neu(halter, EpochId(ab.0 + 2), 0)
        .mit_modell(Modellzusage { abdruck: [7; 32], ..modell });
    anderes.signiere(&sk).expect("signieren");
    for (nonce, z) in [(1, gut), (2, fremd), (3, anderes)] {
        let tx = Transaktion::signiere(&Kette::startwert(), &sk, nonce, Anweisung::ZusageEinreichen { zusage: z })
            .expect("signieren");
        k.aufnehmen(tx);
    }
    k.baue_block();
    let liste = k.zustand().zusagen.get(&halter).expect("eingereicht");
    assert_eq!(liste.iter().map(|z| z.ab_epoche).collect::<Vec<_>>(), [ab], "nur die eigene Unterschrift und das Podmodell gelten");
}

/// Die Abmeldung wirkt über die Kette ebenso.
#[test]
fn eine_abmeldung_ueber_die_kette_wirkt() {
    use myl_types::miner::HardwareClass;
    let mut k = Kette::probestand();
    k.aufnehmen(anmeldung(0, HardwareClass::SmallGpu));
    k.baue_block();
    let ab = Transaktion::signiere(
        &Kette::startwert(),
        &probeschluessel(0),
        1,
        Anweisung::MinerAbmelden,
    )
    .expect("signieren");
    k.aufnehmen(ab);
    k.baue_block();
    assert!(k.zustand().miner.is_empty(), "die Abmeldung wirkte nicht");
}

/// ⚑ **Ohne gültige Unterschrift keine Anmeldung.** Dieselbe
/// Gegenprobe wie zu Fund 85, für die neue Anweisung.
#[test]
fn eine_anmeldung_ohne_unterschrift_wirkt_nicht() {
    use myl_types::miner::HardwareClass;
    let mut k = Kette::probestand();
    let mut tx = anmeldung(0, HardwareClass::SmallGpu);
    // Die Unterschrift verfälschen.
    tx.signatur.0[0] ^= 0xFF;
    k.aufnehmen(tx);
    k.baue_block();
    assert!(k.zustand().miner.is_empty(), "eine verfaelschte Anmeldung wirkte");
}

/// Erzeuger und Übernehmer kommen auch mit Anmeldungen zur selben
/// Wurzel.
#[test]
fn eine_anmeldung_aendert_bei_beiden_dieselbe_wurzel() {
    use myl_types::miner::HardwareClass;
    let mut erzeuger = Kette::probestand();
    let mut uebernehmer = Kette::probestand();
    erzeuger.aufnehmen(anmeldung(0, HardwareClass::LargeGpu));
    let b = erzeuger.baue_block();
    uebernehmer.uebernimm(&b).expect("Uebernahme");
    assert_eq!(
        erzeuger.zustand().commitment(),
        uebernehmer.zustand().commitment()
    );
}
