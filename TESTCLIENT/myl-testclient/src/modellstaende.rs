//! `modellstaende`: was sich beim Wechsel von θ_v A nach B geändert hat
//! (Punkt 3.3).
//!
//! **Die andere Frage.** `vergleich` fragt: Rechnen zwei *Maschinen*
//! dasselbe? Dort sind verschiedene Modellstände ein Ausschlussgrund,
//! und zwar zu Recht: Ein Digest-Unterschied zwischen zwei Modellen als
//! Determinismusfehler zu melden wäre die Verwechslung, gegen die es
//! `artefakte` gibt.
//!
//! Hier ist der Modellwechsel der **Gegenstand**. Nach einem θ_v-Wechsel
//! ändern sich die Vergleichswerte zwangsläufig, und die Frage lautet
//! nicht „gleich oder nicht", sondern **„erwartet oder nicht"**. Ein
//! einziger Aufruf beantwortet sie, statt dass jemand Protokolldateien
//! nebeneinanderlegt.
//!
//! ## Was das Werkzeug ausdrücklich nicht sagt
//!
//! Es fällt **kein Urteil über Determinismus**. Zwei Modellstände
//! *sollen* verschiedene Zahlen liefern; das ist kein Befund. Der
//! Nachweis entsteht in `vergleich`, über zwei Maschinen und **einen**
//! Modellstand.
//!
//! ## Was daran interessant ist
//!
//! Nicht die geänderten Werte, sondern die **unveränderten**. Ein
//! Vergleichswert, der einen θ_v-Wechsel unbeschadet übersteht, ist
//! entweder modellunabhängig (der `stack`-Durchlauf braucht kein Modell)
//! oder die Änderung hat ihn nicht erreicht. Der zweite Fall ist der
//! Grund für dieses Werkzeug.
//!
//! ## Was verglichen werden darf
//!
//! Nur Läufe mit **demselben Befehl und derselben Einstellungs-Kennung**.
//! Ein anderer Testplan misst etwas anderes, und ein Unterschied sagte
//! dann nichts über den Modellwechsel aus. Ebenso zählen nur
//! **abgeschlossene** Läufe: Ein abgebrochener Lauf stimmt in allem
//! überein, was er erreicht hat, und fehlt im Rest. Das war Fund 35, und
//! dieselbe Falle steht hier offen.
//!
//! Tragen zwei Maschinen **innerhalb desselben Modellstands**
//! verschiedene Werte, dann ist der Stand nicht einheitlich, und dieser
//! Wert wird für den Vergleich über Stände hinweg nicht benutzt. Er
//! gehört dann zu `vergleich`, nicht hierher.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::vergleich::{einlesen, Protokoll};

/// Ein Modellstand, so wie ihn die Protokolle ausweisen.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stand {
    pub theta_v: String,
    pub artefakt_digest: String,
    pub digest_umfang: String,
}

impl Stand {
    fn von(p: &Protokoll) -> Self {
        Self {
            theta_v: p.theta_v.clone(),
            artefakt_digest: p.artefakt_digest.clone(),
            digest_umfang: p.digest_umfang.clone(),
        }
    }

    /// Kurzform für die Anzeige: θ_v plus die ersten Zeichen des
    /// Artefaktdigests, denn zwei Stände können dieselbe θ_v tragen und
    /// trotzdem verschiedene Artefakte sein.
    pub fn kurz(&self) -> String {
        let tv = if self.theta_v.is_empty() {
            "θ_v?"
        } else {
            &self.theta_v
        };
        let art = self.artefakt_digest.chars().take(8).collect::<String>();
        if art.is_empty() {
            tv.to_string()
        } else {
            format!("{} / {}", tv, art)
        }
    }
}

/// Wie sich ein einzelner Vergleichswert über die Stände verhält.
///
/// **Warum es drei statt zwei Fälle sind.** Die erste Fassung dieses
/// Werkzeugs kannte nur „geändert" und „unverändert", jeweils über
/// **alle** Stände auf einmal. Beim ersten Lauf gegen echte Protokolle
/// fiel auf, dass das die eigentliche Frage verdeckt: Bei drei Ständen,
/// von denen zwei denselben Wert tragen und der dritte nicht, meldete es
/// „geändert" und verschwieg das Paar. Gefragt ist aber der Wechsel von
/// **A nach B**, nicht die Gesamtlage.
///
/// Dieselbe Klasse wie die Funde 33 bis 37: ein Messgerät, das mehr
/// behauptet, als es abdeckt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verhalten {
    /// In jedem Stand vorhanden, überall derselbe Wert.
    Unveraendert,
    /// In jedem Stand vorhanden, alle Werte paarweise verschieden.
    Geaendert,
    /// Mindestens zwei Stände teilen sich einen Wert, aber nicht alle.
    ///
    /// **Der Fall, auf den es ankommt.** Zwei Modellstände mit demselben
    /// Vergleichswert heißt: Die Änderung hat die Rechnung nicht
    /// erreicht. Genannt werden die Stände, die sich einen Wert teilen.
    TeilweiseGleich { gleiche: Vec<Vec<usize>> },
    /// Nicht in jedem Stand vorhanden.
    ///
    /// Kein Befund: Ein fehlender Wert ist keine Änderung. Dieselbe
    /// Unterscheidung wie in `vergleich` seit Fund 35.
    Unvollstaendig,
    /// In mindestens einem Stand tragen zwei Läufe verschiedene Werte.
    ///
    /// Der Stand ist dann nicht einheitlich, und der Wert taugt hier
    /// nicht. Das ist eine Frage für `vergleich`.
    StandUneinheitlich,
}

impl Verhalten {
    pub fn kurz(&self) -> &'static str {
        match self {
            Verhalten::Unveraendert => "unverändert",
            Verhalten::Geaendert => "geändert",
            Verhalten::TeilweiseGleich { .. } => "teils gleich",
            Verhalten::Unvollstaendig => "nicht überall",
            Verhalten::StandUneinheitlich => "Stand uneinheitlich",
        }
    }

    /// Teilen sich mindestens zwei **verschiedene** Stände einen Wert?
    pub fn hat_gleiches_paar(&self) -> bool {
        matches!(
            self,
            Verhalten::Unveraendert | Verhalten::TeilweiseGleich { .. }
        )
    }
}

/// Ein Vergleichswert über alle Stände hinweg.
#[derive(Debug, Clone)]
pub struct Zeile {
    pub name: String,
    /// Je Stand der Wert, falls einheitlich vorhanden.
    pub werte: BTreeMap<Stand, Option<String>>,
    pub verhalten: Verhalten,
}

/// Eine vergleichbare Menge: ein Befehl, eine Einstellungs-Kennung.
#[derive(Debug, Clone)]
pub struct Menge {
    pub befehl: String,
    pub einstellungen_id: String,
    pub staende: Vec<Stand>,
    pub zeilen: Vec<Zeile>,
    /// Protokolle, die wegen fehlenden Abschlusses ausgelassen wurden.
    pub ausgelassen: Vec<(String, &'static str)>,
}

/// Ordnet die Protokolle zu vergleichbaren Mengen und bewertet jeden
/// Vergleichswert über die Stände hinweg.
pub fn auswerten(protokolle: Vec<Protokoll>) -> Vec<Menge> {
    let mut nach_menge: BTreeMap<(String, String), Vec<Protokoll>> = BTreeMap::new();
    let mut ausgelassen: BTreeMap<(String, String), Vec<(String, &'static str)>> = BTreeMap::new();

    for p in protokolle {
        let schluessel = (p.befehl.clone(), p.einstellungen_id.clone());
        match p.mangel() {
            Some(grund) => ausgelassen
                .entry(schluessel)
                .or_default()
                .push((p.bezeichnung(), grund)),
            None => nach_menge.entry(schluessel).or_default().push(p),
        }
    }

    let mut mengen = Vec::new();
    for ((befehl, einstellungen_id), gruppe) in nach_menge {
        let staende: Vec<Stand> = gruppe
            .iter()
            .map(Stand::von)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        // Je Stand und Name alle vorkommenden Werte sammeln. Mehr als
        // einer heißt: der Stand ist nicht einheitlich.
        let mut gesammelt: BTreeMap<String, BTreeMap<Stand, BTreeSet<String>>> = BTreeMap::new();
        for p in &gruppe {
            let stand = Stand::von(p);
            for (name, digest) in &p.ergebnisse {
                gesammelt
                    .entry(name.clone())
                    .or_default()
                    .entry(stand.clone())
                    .or_default()
                    .insert(digest.clone());
            }
        }

        let mut zeilen = Vec::new();
        for (name, je_stand) in gesammelt {
            let mut werte: BTreeMap<Stand, Option<String>> = BTreeMap::new();
            let mut uneinheitlich = false;
            for stand in &staende {
                let wert = match je_stand.get(stand) {
                    None => None,
                    Some(menge) if menge.len() == 1 => menge.iter().next().cloned(),
                    Some(_) => {
                        uneinheitlich = true;
                        None
                    }
                };
                werte.insert(stand.clone(), wert);
            }

            let verhalten = if uneinheitlich {
                Verhalten::StandUneinheitlich
            } else if werte.values().any(|w| w.is_none()) {
                Verhalten::Unvollstaendig
            } else {
                // Stände nach ihrem Wert bündeln. Ein Bündel mit mehr als
                // einem Stand ist ein Paar, das sich nicht bewegt hat, und
                // genau danach wird gefragt.
                let mut nach_wert: BTreeMap<&String, Vec<usize>> = BTreeMap::new();
                for (i, stand) in staende.iter().enumerate() {
                    if let Some(Some(d)) = werte.get(stand) {
                        nach_wert.entry(d).or_default().push(i);
                    }
                }
                let gleiche: Vec<Vec<usize>> = nach_wert
                    .values()
                    .filter(|s| s.len() > 1)
                    .cloned()
                    .collect();

                match nach_wert.len() {
                    1 => Verhalten::Unveraendert,
                    _ if gleiche.is_empty() => Verhalten::Geaendert,
                    _ => Verhalten::TeilweiseGleich { gleiche },
                }
            };

            zeilen.push(Zeile {
                name,
                werte,
                verhalten,
            });
        }

        let key = (befehl.clone(), einstellungen_id.clone());
        mengen.push(Menge {
            befehl,
            einstellungen_id,
            staende,
            zeilen,
            ausgelassen: ausgelassen.remove(&key).unwrap_or_default(),
        });
    }

    mengen
}

fn kurz(digest: &str) -> String {
    digest.chars().take(16).collect()
}

/// Schreibt die Zusammenfassung auf das Terminal.
///
/// Liefert `true`, wenn überhaupt etwas zu berichten war. Ein
/// **Exit-Code über Änderungen wäre falsch**: Eine Änderung nach einem
/// θ_v-Wechsel ist der Normalfall, kein Fehler. Wer eine Erwartung
/// durchsetzen will, nimmt `--erwarte` am Messlauf.
pub fn berichten(mengen: &[Menge]) -> bool {
    if mengen.is_empty() {
        println!("  Keine auswertbaren Protokolle gefunden.");
        return false;
    }

    let mut etwas_gezeigt = false;

    for m in mengen {
        println!();
        println!(
            "  ── {} · Einstellungen {} ──",
            m.befehl,
            if m.einstellungen_id.is_empty() {
                "ohne-plan"
            } else {
                &m.einstellungen_id
            }
        );

        for (wer, grund) in &m.ausgelassen {
            println!("     ausgelassen: {} ({})", wer, grund);
        }

        if m.staende.len() < 2 {
            let stand = m
                .staende
                .first()
                .map(|s| s.kurz())
                .unwrap_or_else(|| "keiner".to_string());
            println!(
                "     Nur ein Modellstand ({}). Ein Vergleich über Stände braucht zwei.",
                stand
            );
            continue;
        }

        etwas_gezeigt = true;
        println!("     Stände:");
        for (i, s) in m.staende.iter().enumerate() {
            println!(
                "       [{}] {}{}",
                i + 1,
                s.kurz(),
                if s.digest_umfang.is_empty() {
                    String::new()
                } else {
                    format!("   Digest über {}", s.digest_umfang)
                }
            );
        }
        println!();

        let breite = m.zeilen.iter().map(|z| z.name.len()).max().unwrap_or(4).min(38);
        for z in &m.zeilen {
            let werte: Vec<String> = m
                .staende
                .iter()
                .map(|s| match z.werte.get(s).and_then(|w| w.as_ref()) {
                    Some(d) => kurz(d),
                    None => "·".to_string(),
                })
                .collect();
            println!(
                "     {:<breite$}  {:<20}  {}",
                z.name,
                z.verhalten.kurz(),
                werte.join("  "),
                breite = breite
            );
        }

        // **Gefragt ist der Wechsel von A nach B, nicht die Gesamtlage.**
        // Deshalb wird hier je Paar von Ständen aufgezählt, welche Werte
        // sich nicht bewegt haben. Eine Zusammenfassung über alle Stände
        // auf einmal verdeckte genau das, was das Werkzeug zeigen soll.
        println!();
        let mut irgendein_paar = false;
        for a in 0..m.staende.len() {
            for b in (a + 1)..m.staende.len() {
                let gleich: Vec<&str> = m
                    .zeilen
                    .iter()
                    .filter(|z| match &z.verhalten {
                        Verhalten::Unveraendert => true,
                        Verhalten::TeilweiseGleich { gleiche } => {
                            gleiche.iter().any(|g| g.contains(&a) && g.contains(&b))
                        }
                        _ => false,
                    })
                    .map(|z| z.name.as_str())
                    .collect();
                if gleich.is_empty() {
                    continue;
                }
                irgendein_paar = true;
                println!(
                    "     [{}] → [{}]  unverändert: {}",
                    a + 1,
                    b + 1,
                    gleich.join(", ")
                );
            }
        }

        if irgendein_paar {
            println!();
            println!(
                "     Das sind die interessanten Zeilen: Entweder hängt der Wert nicht \
                 am Modell, oder die Änderung hat ihn nicht erreicht."
            );
        } else {
            println!(
                "     Kein Vergleichswert überlebt einen Standwechsel unverändert. Das \
                 ist nach einem θ_v- oder Artefaktwechsel zu erwarten."
            );
        }
    }

    println!();
    println!(
        "  Kein Determinismusurteil. Zwei Modellstände sollen verschiedene Zahlen \
         liefern; das ist kein Befund. Der Nachweis entsteht in `vergleich`, über \
         zwei Maschinen und einen Stand."
    );
    etwas_gezeigt
}

/// `myl-test modellstaende`: Protokolle einlesen und zusammenfassen.
///
/// Liefert `false` **nur**, wenn die Protokolle nicht zu lesen waren.
/// Eine gefundene Änderung ist kein Fehlschlag: Sie ist nach einem
/// θ_v-Wechsel der Normalfall. Wer eine Erwartung durchsetzen will, gibt
/// sie am Messlauf mit `--erwarte` an.
pub fn run(dir: &Path) -> bool {
    let protokolle = match einlesen(dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("  {}", e);
            return false;
        }
    };
    println!("  {} Protokolle aus {}", protokolle.len(), dir.display());
    berichten(&auswerten(protokolle));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn protokoll(
        befehl: &str,
        wer: &str,
        theta_v: &str,
        artefakt: &str,
        ergebnisse: &[(&str, &str)],
    ) -> Protokoll {
        Protokoll {
            befehl: befehl.to_string(),
            teilnehmer: wer.to_string(),
            einstellungen_id: "plan1".to_string(),
            theta_v: theta_v.to_string(),
            artefakt_digest: artefakt.to_string(),
            digest_umfang: "logits+token".to_string(),
            ergebnisse: ergebnisse
                .iter()
                .map(|(n, d)| (n.to_string(), d.to_string()))
                .collect(),
            abgeschlossen: true,
            erfolgreich: true,
            ..Default::default()
        }
    }

    #[test]
    fn geaenderte_und_unveraenderte_werte_werden_getrennt() {
        let mengen = auswerten(vec![
            protokoll(
                "determinismus",
                "a",
                "0.17.0",
                "aaaa1111",
                &[("determinismus", "1111"), ("stack", "9999")],
            ),
            protokoll(
                "determinismus",
                "a",
                "0.18.0",
                "bbbb2222",
                &[("determinismus", "2222"), ("stack", "9999")],
            ),
        ]);
        assert_eq!(mengen.len(), 1);
        let m = &mengen[0];
        assert_eq!(m.staende.len(), 2);

        let det = m.zeilen.iter().find(|z| z.name == "determinismus").unwrap();
        assert_eq!(det.verhalten, Verhalten::Geaendert);
        let stack = m.zeilen.iter().find(|z| z.name == "stack").unwrap();
        assert_eq!(stack.verhalten, Verhalten::Unveraendert);
    }

    /// **Der Fall, der die erste Fassung dieses Moduls widerlegt hat.**
    /// Drei Stände, zwei davon mit demselben Wert: Die erste Fassung
    /// urteilte über alle Stände auf einmal, meldete „geändert" und
    /// verschwieg damit genau das Paar, nach dem die Prüfung fragt.
    /// Aufgefallen beim ersten Lauf gegen echte Protokolle, nicht im Test.
    #[test]
    fn zwei_von_drei_staenden_mit_gleichem_wert_werden_genannt() {
        let mengen = auswerten(vec![
            protokoll("determinismus", "a", "0.17.0", "aaaa", &[("d", "1111")]),
            protokoll("determinismus", "a", "0.17.0", "bbbb", &[("d", "2222")]),
            protokoll("determinismus", "a", "0.18.0", "bbbb", &[("d", "2222")]),
        ]);
        let m = &mengen[0];
        assert_eq!(m.staende.len(), 3);
        let z = &m.zeilen[0];
        match &z.verhalten {
            Verhalten::TeilweiseGleich { gleiche } => {
                assert_eq!(gleiche.len(), 1);
                // Stände 2 und 3 in der sortierten Reihenfolge.
                assert_eq!(gleiche[0], vec![1, 2]);
            }
            other => panic!("erwartet TeilweiseGleich, bekam {:?}", other),
        }
        assert!(z.verhalten.hat_gleiches_paar());
    }

    /// Drei paarweise verschiedene Werte bleiben „geändert".
    #[test]
    fn paarweise_verschiedene_werte_sind_geaendert() {
        let mengen = auswerten(vec![
            protokoll("determinismus", "a", "0.17.0", "aaaa", &[("d", "1111")]),
            protokoll("determinismus", "a", "0.18.0", "bbbb", &[("d", "2222")]),
            protokoll("determinismus", "a", "0.19.0", "cccc", &[("d", "3333")]),
        ]);
        assert_eq!(mengen[0].zeilen[0].verhalten, Verhalten::Geaendert);
        assert!(!mengen[0].zeilen[0].verhalten.hat_gleiches_paar());
    }

    /// Ein Wert, den nur einer der Stände trägt, ist **keine** Änderung.
    /// Dieselbe Unterscheidung, an der Fund 35 hing.
    #[test]
    fn fehlender_wert_ist_keine_aenderung() {
        let mengen = auswerten(vec![
            protokoll("determinismus", "a", "0.17.0", "aaaa", &[("neu", "1111")]),
            protokoll("determinismus", "a", "0.18.0", "bbbb", &[]),
        ]);
        let z = &mengen[0].zeilen[0];
        assert_eq!(z.verhalten, Verhalten::Unvollstaendig);
    }

    /// Zwei Maschinen im selben Stand mit verschiedenen Werten: Das ist
    /// eine Frage für `vergleich`, und der Wert taugt hier nicht.
    #[test]
    fn uneinheitlicher_stand_wird_nicht_verglichen() {
        let mengen = auswerten(vec![
            protokoll("determinismus", "a", "0.17.0", "aaaa", &[("d", "1111")]),
            protokoll("determinismus", "b", "0.17.0", "aaaa", &[("d", "3333")]),
            protokoll("determinismus", "a", "0.18.0", "bbbb", &[("d", "2222")]),
        ]);
        let z = &mengen[0].zeilen[0];
        assert_eq!(z.verhalten, Verhalten::StandUneinheitlich);
    }

    /// Verschiedene Testpläne sind verschiedene Messungen und landen
    /// nicht in derselben Tabelle.
    #[test]
    fn verschiedene_einstellungen_werden_getrennt() {
        let mut zweiter = protokoll("determinismus", "a", "0.18.0", "bbbb", &[("d", "2222")]);
        zweiter.einstellungen_id = "plan2".to_string();
        let mengen = auswerten(vec![
            protokoll("determinismus", "a", "0.17.0", "aaaa", &[("d", "1111")]),
            zweiter,
        ]);
        assert_eq!(mengen.len(), 2);
        for m in &mengen {
            assert_eq!(m.staende.len(), 1);
        }
    }

    /// Ein abgebrochener Lauf wird ausgelassen und benannt, nicht
    /// stillschweigend mitgezählt.
    #[test]
    fn abgebrochener_lauf_wird_ausgelassen() {
        let mut kaputt = protokoll("determinismus", "b", "0.18.0", "bbbb", &[("d", "2222")]);
        kaputt.abgeschlossen = false;
        let mengen = auswerten(vec![
            protokoll("determinismus", "a", "0.17.0", "aaaa", &[("d", "1111")]),
            kaputt,
        ]);
        assert_eq!(mengen.len(), 1);
        assert_eq!(mengen[0].ausgelassen.len(), 1);
        assert_eq!(mengen[0].staende.len(), 1);
    }

    #[test]
    fn ein_einzelner_stand_ergibt_keinen_vergleich() {
        let mengen = auswerten(vec![protokoll(
            "determinismus",
            "a",
            "0.17.0",
            "aaaa",
            &[("d", "1111")],
        )]);
        assert_eq!(mengen[0].staende.len(), 1);
        assert!(!berichten(&mengen));
    }

    /// Zwei Stände mit derselben θ_v, aber verschiedenen Artefakten
    /// müssen unterscheidbar bleiben.
    #[test]
    fn gleiche_theta_v_verschiedene_artefakte_sind_zwei_staende() {
        let mengen = auswerten(vec![
            protokoll("determinismus", "a", "0.17.0", "aaaa", &[("d", "1111")]),
            protokoll("determinismus", "a", "0.17.0", "bbbb", &[("d", "2222")]),
        ]);
        assert_eq!(mengen[0].staende.len(), 2);
        assert_eq!(mengen[0].zeilen[0].verhalten, Verhalten::Geaendert);
        assert_ne!(mengen[0].staende[0].kurz(), mengen[0].staende[1].kurz());
    }
}
