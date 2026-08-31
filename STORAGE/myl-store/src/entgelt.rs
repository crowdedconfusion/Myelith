//! Das Speicherentgelt: bezahlt wird je bewiesenem Byte, unabhängig vom
//! Mining.
//!
//! # Warum unabhängig vom Mining
//!
//! **Festlegung des Projektinhabers, 2026-08-30.** Ein Knoten kann
//! Wissen halten, ohne eine einzige Token-Position zu rechnen. Käme das
//! Entgelt aus dem Anteil der Shard-Miner, bekäme genau dieser Knoten
//! nichts, und die Rolle Store wäre unbezahlt.
//!
//! Der Entwurf davor sah das Entgelt **innerhalb** der 78 Prozent der
//! Shard-Miner vor. Er war falsch, und zwar aus demselben Grund, aus dem
//! die Rollentabelle des Papiers eine siebte Rolle braucht: **Halten ist
//! eine eigene Leistung, nicht ein Nebenprodukt des Rechnens.**
//!
//! Strukturell steht das hier so: [`abrechnen`] kennt die Zahl der
//! Nachweisführenden und sonst nichts. Kein vTFE, keine Shard-Zuteilung,
//! keine Epochenarbeit. Wer nachweist, wird bezahlt.
//!
//! # Drei Klassen, drei Antworten
//!
//! | Art | Quelle | Verfall |
//! |---|---|---|
//! | Shardgewichte, Skalenpaket, Sonstiges der Infrastruktur | Treasury | nie |
//! | **Netzwerkwissen** | Treasury | nie |
//! | Wissensstück (Einlage) | Einleger | ja |
//!
//! **Protokollkritisches** muss vorliegen, sonst rechnet niemand. Es hat
//! keinen Einleger, also zahlt die Allgemeinheit.
//!
//! **Netzwerkwissen** ist die Bibliothek, die immer verfügbar sein muss.
//! Sie wird abgefragt und speist das Training. Auch sie hat keinen
//! Einleger.
//!
//! **Eine Einlage** hat einen. Sie kostet Byte-Epochen, und wenn keine
//! mehr da sind, endet die Haltepflicht.
//!
//! # ⚑ Wo die Schranke sitzt, und warum sie zweimal verschieden aussieht
//!
//! `speicherlast` in GOVERNANCE hat gerechnet, dass die Last je Knoten
//! `W · f / N` ist und dass **nichts W an N bindet**. Ohne Schranke
//! wächst W, bis Knoten gehen. Die Schranke sieht je Klasse anders aus,
//! und das ist kein Zufall, sondern folgt aus der Herkunft:
//!
//! - **Einlagen sind wirtschaftlich begrenzt.** Nichts kommt hinein,
//!   ohne dass jemand dafür Byte-Epochen erwirbt, und nichts bleibt
//!   liegen, wenn niemand nachzahlt. Das ist die Bauart, die Swarm mit
//!   seinen Frankierungen und Arweave mit seinem Fonds benutzen: Der
//!   Einleger trägt die Zukunft mit, nicht der Halter.
//! - **Netzwerkwissen ist verwaltungsmäßig begrenzt.** Es verfällt
//!   nicht, also muss die Schranke bei der **Aufnahme** sitzen. Was
//!   aufgenommen wird, ist eine Entscheidung, und sie gehört gegen die
//!   nachgewiesene Kapazität geprüft.
//!
//! ⚑ **Und daraus ergibt sich der Weg zwischen den beiden Klassen:**
//! Eine Einlage, die oft genug abgefragt wird, ist Wissen, das die
//! Allgemeinheit tragen will; sie kann in die Bibliothek aufgenommen
//! werden, und ihre Finanzierung wechselt mit. Wissen erwirbt sich
//! Dauerhaftigkeit durch Nutzung, statt sie zu kaufen.
//!
//! **Die Zahl dazu steht hier nicht**, denn sie braucht Abrufzählung und
//! damit Verkehr. Was hier steht, ist die Form: [`Finanzierung`] hängt
//! an der Art, und die Art ist änderbar.
//!
//! # Was hier nicht entschieden wird
//!
//! **Der Satz**, also wie viele Byte-Epochen eine verbrannte MYL ergibt
//! und wie viel eine Byte-Epoche einbringt. Das ist ein
//! Governance-Parameter und eine Festlegung des Projektinhabers; dieses
//! Modul rechnet in Byte-Epochen und kennt keinen Preis.

use myl_types::gegenstand::{Gegenstandsart, Manifest};

/// Das verbleibende Guthaben eines Gegenstands, in Byte-Epochen.
///
/// **Byte-Epochen und nicht Bytes**, weil Speicherung eine Leistung über
/// Zeit ist: Ein Gigabyte für eine Stunde und ein Megabyte für tausend
/// Stunden sind vergleichbare Größen, und nur so lässt sich beides aus
/// einem Topf bezahlen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Speicherguthaben {
    /// Verbleibende Byte-Epochen.
    pub byte_epochen: u128,
}

/// Was ein Gegenstand je Epoche kostet, in Byte-Epochen.
///
/// **Nutzdaten mal Platzfaktor.** Bezahlt wird, was das Netz wirklich
/// hält, nicht was hineingegeben wurde: Sieben Kopien kosten das
/// Siebenfache, Erasure k=8/m=6 das 1,75-fache. Genau dafür trägt
/// `Redundanzform::platz` seinen Bruch.
pub fn verbrauch_je_epoche(manifest: &Manifest) -> u128 {
    let (zaehler, nenner) = manifest.redundanz.platz();
    debug_assert!(nenner > 0, "ein Platzbruch ohne Nenner");
    u128::from(manifest.laenge) * u128::from(zaehler) / u128::from(nenner.max(1))
}

/// Das Ergebnis einer Epochenabrechnung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Abrechnung {
    /// Tatsächlich abgebuchte Byte-Epochen.
    pub verbraucht: u128,
    /// Was jeder Nachweisführende bekommt.
    pub je_halter: u128,
    /// Der unteilbare Rest, kleiner als die Zahl der Halter.
    ///
    /// **Er wird verschieden behandelt, und dafür gibt es einen
    /// Grund.** Solange der Gegenstand zahlungsfähig ist, bleibt er im
    /// Guthaben liegen: Nächste Epoche ist etwas mehr da, nichts geht
    /// verloren.
    ///
    /// ⚑ **In der letzten Epoche fällt er weg, und ohne das verfiele
    /// kein Gegenstand je.** Ein Rest ist immer kleiner als die
    /// Halterzahl. Bliebe er auch dann liegen, sänke das Guthaben
    /// monoton bis unter diese Schranke und stünde dort für immer: Der
    /// Gegenstand wäre bezahlt für nichts, verfiele nie und dürfte nie
    /// fallengelassen werden. Weniger als eine Handvoll Byte-Epochen
    /// wegzulassen ist der Preis dafür, dass die Schranke überhaupt
    /// greift.
    pub rest: u128,
    /// Ob das Guthaben danach leer ist.
    pub erschoepft: bool,
}

/// Bucht eine Epoche ab und verteilt sie auf die Nachweisführenden.
///
/// # ⚑ Ohne Nachweis wird nichts abgebucht
///
/// Erbringt niemand einen Nachweis, kostet die Epoche nichts. Das ist
/// nicht Milde gegenüber dem Einleger, sondern die einzige Fassung, die
/// stimmt: **Bezahlt wird Speicherung, und ob gespeichert wurde, weiß
/// man nur aus dem Nachweis.** Eine Abbuchung ohne Nachweis bezahlte
/// eine Behauptung, und dagegen ist der ganze Nachweis gebaut.
///
/// Nebenbei ist es die Regel, die den Haltern einen Grund gibt zu
/// antworten: Wer schweigt, bekommt nichts.
///
/// # Reicht das Guthaben nicht
///
/// Dann wird ausgezahlt, was noch da ist, und der Gegenstand gilt als
/// erschöpft. Ein halber Monat Speicherung ist eine halbe Leistung und
/// wird auch so vergütet; den Halter dafür leer ausgehen zu lassen wäre
/// eine Strafe für etwas, das der Einleger versäumt hat.
pub fn abrechnen(
    manifest: &Manifest,
    guthaben: &mut Speicherguthaben,
    nachweise: usize,
) -> Abrechnung {
    if nachweise == 0 {
        return Abrechnung {
            verbraucht: 0,
            je_halter: 0,
            rest: guthaben.byte_epochen,
            erschoepft: guthaben.byte_epochen == 0,
        };
    }

    let faellig = verbrauch_je_epoche(manifest);
    let halter = nachweise as u128;
    // Deckt das Guthaben diese Epoche nicht mehr, ist es die letzte.
    let erschoepfend = faellig >= guthaben.byte_epochen;
    let verfuegbar = faellig.min(guthaben.byte_epochen);
    let je_halter = verfuegbar / halter;
    let ausgezahlt = je_halter * halter;
    let rest = verfuegbar - ausgezahlt;

    if erschoepfend {
        // Letzte Epoche: alles weg, samt Rest. Siehe [`Abrechnung::rest`].
        guthaben.byte_epochen = 0;
    } else {
        guthaben.byte_epochen -= ausgezahlt;
    }

    Abrechnung {
        verbraucht: ausgezahlt,
        je_halter,
        rest,
        erschoepft: guthaben.byte_epochen == 0,
    }
}

/// Ob ein Gegenstand fallengelassen werden darf.
///
/// **Zwei Bedingungen, und beide müssen gelten:** Die Art muss verfallen
/// dürfen, und das Guthaben muss leer sein. Ein leeres Guthaben allein
/// genügt nicht, sonst verschwände Netzwerkwissen still, sobald jemand
/// vergisst nachzulegen.
pub fn darf_fallengelassen_werden(art: Gegenstandsart, guthaben: &Speicherguthaben) -> bool {
    art.verfaellt() && guthaben.byte_epochen == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::gegenstand::{teile_bilden, Finanzierung, Redundanzform, TEILGROESSE};

    fn manifest(art: Gegenstandsart, form: Redundanzform) -> Manifest {
        let daten = vec![7u8; 3 * TEILGROESSE];
        let teile = teile_bilden(&daten).expect("Teile");
        Manifest::neu(art, 1, &teile, form).expect("Manifest")
    }

    /// ⚑ **Die Festlegung des Projektinhabers, als Test.** Netzwerkwissen
    /// muss immer verfügbar sein, hat keinen Einleger und darf deshalb
    /// weder aus einem Guthaben bezahlt werden noch verfallen.
    #[test]
    fn netzwerkwissen_traegt_die_allgemeinheit_und_es_verfaellt_nicht() {
        let art = Gegenstandsart::Netzwerkwissen;
        assert_eq!(art.finanzierung(), Finanzierung::Treasury);
        assert!(!art.verfaellt());
        assert!(!darf_fallengelassen_werden(
            art,
            &Speicherguthaben { byte_epochen: 0 }
        ));
    }

    /// Und das Gegenstück: Eine Einlage zahlt ihr Einleger, und sie
    /// verfällt. Ohne beide Seiten wäre nicht zu sehen, dass die
    /// Unterscheidung überhaupt etwas bewirkt.
    #[test]
    fn eine_einlage_zahlt_der_einleger_und_sie_verfaellt() {
        let art = Gegenstandsart::Wissensstueck;
        assert_eq!(art.finanzierung(), Finanzierung::Einleger);
        assert!(art.verfaellt());
        assert!(darf_fallengelassen_werden(
            art,
            &Speicherguthaben { byte_epochen: 0 }
        ));
        assert!(!darf_fallengelassen_werden(
            art,
            &Speicherguthaben { byte_epochen: 1 }
        ));
    }

    /// Protokollkritisches trägt ebenfalls die Allgemeinheit: Ohne
    /// Shardgewichte rechnet niemand.
    #[test]
    fn protokollkritisches_traegt_die_allgemeinheit() {
        for art in [
            Gegenstandsart::Shardgewichte,
            Gegenstandsart::Skalenpaket,
            Gegenstandsart::Sonstiges,
        ] {
            assert_eq!(art.finanzierung(), Finanzierung::Treasury, "{art:?}");
            assert!(!art.verfaellt(), "{art:?}");
        }
    }

    /// ⚑ **Ohne Nachweis wird nichts abgebucht.** Bezahlt wird
    /// Speicherung, und ob gespeichert wurde, weiß man nur aus dem
    /// Nachweis.
    #[test]
    fn ohne_nachweis_wird_nichts_abgebucht() {
        let m = manifest(
            Gegenstandsart::Wissensstueck,
            Redundanzform::Kopien { anzahl: 7 },
        );
        let mut g = Speicherguthaben {
            byte_epochen: 1_000_000_000,
        };
        let a = abrechnen(&m, &mut g, 0);
        assert_eq!(a.verbraucht, 0);
        assert_eq!(a.je_halter, 0);
        assert_eq!(g.byte_epochen, 1_000_000_000, "es wurde abgebucht");
    }

    /// Bezahlt wird, was das Netz hält, nicht was hineingegeben wurde.
    #[test]
    fn der_verbrauch_folgt_dem_platzfaktor() {
        let kopien = manifest(
            Gegenstandsart::Wissensstueck,
            Redundanzform::Kopien { anzahl: 7 },
        );
        let erasure = manifest(
            Gegenstandsart::Wissensstueck,
            Redundanzform::Erasure { k: 8, m: 6 },
        );
        assert_eq!(verbrauch_je_epoche(&kopien), u128::from(kopien.laenge) * 7);
        assert_eq!(
            verbrauch_je_epoche(&erasure),
            u128::from(erasure.laenge) * 14 / 8
        );
        assert!(verbrauch_je_epoche(&kopien) > verbrauch_je_epoche(&erasure));
    }

    /// ⚑ **Die Invariante: Nichts entsteht und nichts verschwindet**,
    /// solange der Gegenstand zahlungsfähig ist.
    #[test]
    fn solange_zahlungsfaehig_geht_nichts_verloren() {
        let m = manifest(
            Gegenstandsart::Wissensstueck,
            Redundanzform::Kopien { anzahl: 7 },
        );
        let start = verbrauch_je_epoche(&m) * 100;
        for nachweise in 1..=9usize {
            let mut g = Speicherguthaben {
                byte_epochen: start,
            };
            let a = abrechnen(&m, &mut g, nachweise);
            assert!(!a.erschoepft);
            assert_eq!(
                a.je_halter * nachweise as u128 + g.byte_epochen,
                start,
                "bei {nachweise} Haltern"
            );
            assert!(a.rest < nachweise as u128, "der Rest ist nie teilbar");
        }
    }

    /// ⚑ **Die Staubfalle, gegen die die Sonderregel steht.**
    ///
    /// Bliebe der unteilbare Rest auch in der letzten Epoche liegen,
    /// sänke das Guthaben bis unter die Halterzahl und stünde dort für
    /// immer: Der Gegenstand verfiele nie.
    #[test]
    fn ein_gegenstand_verfaellt_wirklich_und_bleibt_nicht_im_staub_stecken() {
        let m = manifest(
            Gegenstandsart::Wissensstueck,
            Redundanzform::Kopien { anzahl: 7 },
        );
        let je_epoche = verbrauch_je_epoche(&m);
        let mut g = Speicherguthaben {
            byte_epochen: je_epoche * 2 + je_epoche / 2 + 1,
        };
        let mut epochen = 0;
        while !darf_fallengelassen_werden(Gegenstandsart::Wissensstueck, &g) {
            abrechnen(&m, &mut g, 3);
            epochen += 1;
            assert!(epochen < 10, "der Gegenstand verfaellt nicht");
        }
        assert_eq!(g.byte_epochen, 0);
        assert_eq!(epochen, 3, "zweieinhalb Epochen sind drei Abbuchungen");
    }

    /// Reicht das Guthaben nicht für eine volle Epoche, wird anteilig
    /// ausgezahlt statt gar nicht.
    #[test]
    fn bei_zu_wenig_guthaben_wird_anteilig_ausgezahlt() {
        let m = manifest(
            Gegenstandsart::Wissensstueck,
            Redundanzform::Kopien { anzahl: 7 },
        );
        let je_epoche = verbrauch_je_epoche(&m);
        let mut g = Speicherguthaben {
            byte_epochen: je_epoche / 4,
        };
        let a = abrechnen(&m, &mut g, 2);
        assert!(a.je_halter > 0, "der Halter ging leer aus");
        assert!(a.verbraucht < je_epoche);
        assert!(a.erschoepft);
        assert_eq!(g.byte_epochen, 0);
    }

    /// Jede Art ist einer Finanzierung zugeordnet, und Verfall folgt ihr.
    #[test]
    fn jede_art_ist_zugeordnet() {
        for art in [
            Gegenstandsart::Shardgewichte,
            Gegenstandsart::Skalenpaket,
            Gegenstandsart::Wissensstueck,
            Gegenstandsart::Sonstiges,
            Gegenstandsart::Netzwerkwissen,
        ] {
            let f = art.finanzierung();
            assert_eq!(art.verfaellt(), f == Finanzierung::Einleger, "{art:?}");
        }
    }

    /// ⚑ **Die Borsh-Nummern der alten Arten dürfen sich nicht
    /// verschoben haben.** Ein Manifest von gestern muss heute dasselbe
    /// bedeuten.
    #[test]
    fn die_neue_art_verschiebt_keine_alte() {
        use borsh::to_vec;
        assert_eq!(to_vec(&Gegenstandsart::Shardgewichte).unwrap(), vec![0]);
        assert_eq!(to_vec(&Gegenstandsart::Skalenpaket).unwrap(), vec![1]);
        assert_eq!(to_vec(&Gegenstandsart::Wissensstueck).unwrap(), vec![2]);
        assert_eq!(to_vec(&Gegenstandsart::Sonstiges).unwrap(), vec![3]);
        assert_eq!(to_vec(&Gegenstandsart::Netzwerkwissen).unwrap(), vec![4]);
    }
}
