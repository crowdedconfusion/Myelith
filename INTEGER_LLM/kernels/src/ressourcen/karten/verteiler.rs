//! **Wer welchen Teil rechnet**, wenn mehrere Karten am selben System
//! haengen.
//!
//! # ⚑ Warum das bitgleich bleibt
//!
//! Jedes Ausgabeelement einer Matrix haengt nur an seiner eigenen Zeile und
//! an der Eingabe. Rechnet eine Karte die Zeilen `0..k` und eine andere den
//! Rest, wird das Ergebnis nur **zusammengesetzt**, nie verrechnet; es gibt
//! keine Summe ueber Karten hinweg. Deshalb ist jede Verteilung erlaubt,
//! und keine aendert eine Zahl.
//!
//! # ⚑ Zwei Arten zu teilen
//!
//! - **Viele Auftraege** (die Experten eines Gemischs, ab zwei je Karte):
//!   ganze Auftraege, die teuersten zuerst, jeweils an die Karte, die danach
//!   im Verhaeltnis zu ihrem Gewicht am wenigsten zu tun hat. Jede Karte
//!   bekommt so ganze Matrizen und laedt jede nur einmal hoch.
//! - **Wenige Auftraege** (eine grosse Matrix der Vorbereitung): jede
//!   Matrix nach Zeilen, im Verhaeltnis der Gewichte, auf ganze Kacheln
//!   ausgerichtet. Die Eingaben gehen dabei an jede beteiligte Karte.
//!
//! # ⚑ Ganzzahlig, auch hier
//!
//! Gewichte und Kosten sind ganze Zahlen, und verglichen wird ueber Kreuz
//! (`(a + c) * g_b < (b + c) * g_a`). Kein Gleitkomma, auch nicht in der
//! Planung: Dieselben Eingaben ergeben auf jeder Maschine dieselbe
//! Verteilung, und das macht einen Fehler wiederholbar.

/// Ein Teil der Arbeit: die Zeilen `von..bis` des Auftrags `auftrag`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Teil {
    pub auftrag: usize,
    pub von: usize,
    pub bis: usize,
}

/// Was ein Auftrag kostet: seine Zeilen und die Kosten je Zeile
/// (Spalten mal Eingaben).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Last {
    pub zeilen: usize,
    pub je_zeile: u64,
}

impl Last {
    fn kosten(&self) -> u128 {
        self.zeilen as u128 * self.je_zeile as u128
    }
}

/// Auf wie viele Zeilen ein Schnitt ausgerichtet wird: eine Kachel der
/// Kerne, damit keine Karte eine halbe Kachel rechnet.
pub const AUSRICHTUNG: usize = 64;

/// **Verteilt die Auftraege auf Karten mit diesen Gewichten.** Das
/// Ergebnis hat je Karte eine Liste von Teilen, in der Reihenfolge der
/// Auftraege. Eine Karte mit Gewicht null bekommt nichts.
///
/// Zugesichert, und in den Pruefungen nachgerechnet: **Jede Zeile jedes
/// Auftrags steht in genau einem Teil.**
pub fn verteilen(lasten: &[Last], gewichte: &[u32]) -> Vec<Vec<Teil>> {
    let mut plan: Vec<Vec<Teil>> = vec![Vec::new(); gewichte.len()];
    let aktive: Vec<usize> = (0..gewichte.len()).filter(|&d| gewichte[d] > 0).collect();
    if aktive.is_empty() {
        return plan;
    }
    let ganz = |a: usize| Teil { auftrag: a, von: 0, bis: lasten[a].zeilen };
    if aktive.len() == 1 {
        plan[aktive[0]] = (0..lasten.len()).filter(|&a| lasten[a].zeilen > 0).map(ganz).collect();
        return plan;
    }
    if lasten.len() >= 2 * aktive.len() {
        // Die teuersten zuerst; bei gleichen Kosten der fruehere Auftrag,
        // damit die Verteilung nicht von der Sortierung abhaengt.
        let mut reihe: Vec<usize> = (0..lasten.len()).filter(|&a| lasten[a].zeilen > 0).collect();
        reihe.sort_by(|&a, &b| lasten[b].kosten().cmp(&lasten[a].kosten()).then(a.cmp(&b)));
        let mut last = vec![0u128; gewichte.len()];
        for a in reihe {
            let c = lasten[a].kosten();
            let mut beste = aktive[0];
            for &d in &aktive[1..] {
                // (last[d] + c) / g[d] < (last[beste] + c) / g[beste], ueber Kreuz.
                if (last[d] + c) * (gewichte[beste] as u128) < (last[beste] + c) * (gewichte[d] as u128) {
                    beste = d;
                }
            }
            last[beste] += c;
            plan[beste].push(ganz(a));
        }
        for teile in plan.iter_mut() {
            teile.sort_by_key(|t| t.auftrag);
        }
        return plan;
    }
    let summe: u128 = aktive.iter().map(|&d| gewichte[d] as u128).sum();
    for (a, l) in lasten.iter().enumerate() {
        if l.zeilen == 0 {
            continue;
        }
        let kacheln = l.zeilen.div_ceil(AUSRICHTUNG) as u128;
        let mut bisher = 0u128;
        let mut von = 0usize;
        for &d in &aktive {
            bisher += gewichte[d] as u128;
            // Die Grenze nach dieser Karte, in ganzen Kacheln; die letzte
            // aktive Karte endet genau bei der Zeilenzahl.
            let bis = ((kacheln * bisher / summe) as usize * AUSRICHTUNG).min(l.zeilen);
            if bis > von {
                plan[d].push(Teil { auftrag: a, von, bis });
                von = bis;
            }
        }
        debug_assert_eq!(von, l.zeilen, "verteilen: Zeilen uebrig");
    }
    plan
}

#[cfg(test)]
mod proben {
    use super::*;

    /// Jede Zeile jedes Auftrags genau einmal.
    fn deckt_genau_ab(lasten: &[Last], plan: &[Vec<Teil>]) {
        for (a, l) in lasten.iter().enumerate() {
            let mut gezaehlt = vec![0u8; l.zeilen];
            for t in plan.iter().flatten().filter(|t| t.auftrag == a) {
                assert!(t.von < t.bis && t.bis <= l.zeilen, "{t:?}");
                for n in &mut gezaehlt[t.von..t.bis] {
                    *n += 1;
                }
            }
            assert!(gezaehlt.iter().all(|&n| n == 1), "Auftrag {a}: {gezaehlt:?}");
        }
    }

    /// ⚑ **Eine Matrix auf zwei gleiche Karten** wird halbiert, auf Kacheln
    /// ausgerichtet.
    #[test]
    fn eine_matrix_wird_nach_zeilen_geteilt() {
        let lasten = [Last { zeilen: 9_728, je_zeile: 2_560 * 128 }];
        let plan = verteilen(&lasten, &[1, 1]);
        deckt_genau_ab(&lasten, &plan);
        assert_eq!(plan[0], vec![Teil { auftrag: 0, von: 0, bis: 4_864 }]);
        assert_eq!(plan[1], vec![Teil { auftrag: 0, von: 4_864, bis: 9_728 }]);
        assert_eq!(plan[0][0].bis % AUSRICHTUNG, 0);
    }

    /// ⚑ **Die Gewichte bestimmen den Anteil**: 3 zu 1.
    #[test]
    fn die_gewichte_bestimmen_den_anteil() {
        let lasten = [Last { zeilen: 64 * 100, je_zeile: 1 }];
        let plan = verteilen(&lasten, &[3, 1]);
        deckt_genau_ab(&lasten, &plan);
        assert_eq!(plan[0][0].bis, 64 * 75);
    }

    /// ⚑ **Eine kleine Matrix bleibt ganz**, wenn sie nur eine Kachel hat;
    /// die zweite Karte bekommt dann nichts statt einer leeren Kachel.
    #[test]
    fn eine_kleine_matrix_bleibt_ganz() {
        let lasten = [Last { zeilen: 40, je_zeile: 10 }];
        let plan = verteilen(&lasten, &[1, 1]);
        deckt_genau_ab(&lasten, &plan);
        assert_eq!(plan.iter().filter(|p| !p.is_empty()).count(), 1);
    }

    /// ⚑ **Viele Auftraege gehen ganz**, und die Last ist ausgeglichen:
    /// 128 gleiche Experten auf vier Karten, je 32.
    #[test]
    fn viele_auftraege_gehen_ganz_und_gleich() {
        let lasten = vec![Last { zeilen: 768, je_zeile: 2_048 * 20 }; 128];
        let plan = verteilen(&lasten, &[1, 1, 1, 1]);
        deckt_genau_ab(&lasten, &plan);
        for p in &plan {
            assert_eq!(p.len(), 32);
            assert!(p.iter().all(|t| t.von == 0 && t.bis == 768));
            assert!(p.windows(2).all(|w| w[0].auftrag < w[1].auftrag), "nicht in Auftragsreihenfolge");
        }
    }

    /// ⚑ **Ungleiche Auftraege, ungleiche Karten**: Die Last je Gewicht
    /// liegt am Ende nah beieinander.
    #[test]
    fn ungleiche_auftraege_werden_ausgeglichen() {
        let lasten: Vec<Last> = (1..=20).map(|i| Last { zeilen: 64 * i, je_zeile: 1_000 }).collect();
        let gewichte = [2u32, 1, 1];
        let plan = verteilen(&lasten, &gewichte);
        deckt_genau_ab(&lasten, &plan);
        let je_gewicht: Vec<u128> = plan
            .iter()
            .zip(gewichte)
            .map(|(p, g)| p.iter().map(|t| lasten[t.auftrag].kosten()).sum::<u128>() / g as u128)
            .collect();
        let (min, max) = (je_gewicht.iter().min().unwrap(), je_gewicht.iter().max().unwrap());
        // Hoechstens ein grosser Auftrag Unterschied.
        assert!(max - min <= lasten[19].kosten(), "{je_gewicht:?}");
    }

    /// ⚑ **Ohne Gewicht keine Arbeit**, und ohne Karte kein Plan.
    #[test]
    fn eine_karte_ohne_gewicht_bekommt_nichts() {
        let lasten = [Last { zeilen: 1_000, je_zeile: 7 }, Last { zeilen: 0, je_zeile: 7 }];
        let plan = verteilen(&lasten, &[0, 5]);
        deckt_genau_ab(&lasten, &plan);
        assert!(plan[0].is_empty());
        assert_eq!(plan[1], vec![Teil { auftrag: 0, von: 0, bis: 1_000 }]);
        assert!(verteilen(&lasten, &[0, 0]).iter().all(Vec::is_empty));
        assert!(verteilen(&lasten, &[]).is_empty());
    }

    /// ⚑ **Dieselben Eingaben, derselbe Plan**, auch wenn Kosten gleich
    /// sind.
    #[test]
    fn der_plan_ist_wiederholbar() {
        let lasten: Vec<Last> = (0..37).map(|i| Last { zeilen: 64 + (i % 5) * 64, je_zeile: 11 }).collect();
        assert_eq!(verteilen(&lasten, &[1, 2, 1]), verteilen(&lasten, &[1, 2, 1]));
    }
}
