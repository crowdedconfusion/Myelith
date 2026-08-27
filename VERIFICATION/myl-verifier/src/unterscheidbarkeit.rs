//! Ununterscheidbarkeit **messen** statt sie zu behaupten (Punkt 3.2,
//! Kap. 6.7 Anforderung 1, Kap. 11 Forschungsfrage 5).
//!
//! # Was hier fehlte, und warum es kein Code-Problem ist
//!
//! [`crate::kontrollsegmente`] baut die Mechanik, [`crate::unterscheider`]
//! misst die Spur der Mechanik (Fund 58). Offen blieb die erste
//! Konstruktionsanforderung: Kontrollsegmente sollen der realen
//! Prompt-Verteilung entstammen und in **Länge, Timing und
//! Kontextprofil** unauffällig sein.
//!
//! Das ist eine Eigenschaft der Daten. Kein Datentyp erzwingt, dass ein
//! Prompt unauffällig ist, und echte Prompt-Verteilungen entstehen erst
//! im Betrieb. Genau deshalb steht in beiden Modulen „bleibt offen".
//!
//! **Was fehlte, war nicht die Antwort, sondern das Messgerät.** Ohne
//! es ist „wir messen das im Betrieb" ein Versprechen, das niemand
//! einlösen kann: Es sagt nicht, was gemessen wird, ab wann eine
//! Abweichung als Abweichung gilt, und wie viele Daten nötig sind,
//! damit ein Nein etwas bedeutet. Dieses Modul liefert das Gerät und
//! seine Eichung.
//!
//! # ⚑ Der Fallstrick sitzt im Akzeptanzkriterium selbst
//!
//! Das Kriterium der Phase lautet: „statistische Analyse zeigt **keinen
//! signifikanten Unterschied**". So formuliert ist es von einem
//! schlechten Test **leichter** zu erfüllen als von einem guten. Ein
//! Test mit einem einzigen Fach, oder mit dreißig Proben, findet nie
//! etwas und meldet immer Erfolg.
//!
//! „Kein Unterschied gefunden" ist keine Aussage, solange nicht
//! danebensteht, **was dieser Aufbau überhaupt hätte finden können**.
//!
//! Deshalb gibt es hier keinen Weg, ein Nein zu bekommen, ohne die
//! Trennschärfe mitzubekommen: [`Befund::KeinNachweis`] trägt
//! `erkennbar_ab` im Wert. Wer das Ergebnis weitergibt, gibt die Grenze
//! mit weiter. Dieselbe Bauart wie an anderen Stellen des Projekts: Die
//! Regel steht nicht im Kommentar, sie steht im Typ.
//!
//! # Das Verfahren: Vertauschungstest, ganzzahlig
//!
//! Zwei Stichproben ganzzahliger Merkmale, ein [`Raster`] von Fächern,
//! als Teststatistik der **totale Variationsabstand** der beiden
//! empirischen Verteilungen in Promille.
//!
//! Die Signifikanz kommt aus einem **Vertauschungstest**: Die Etiketten
//! „echt" und „Kontrolle" werden mehrfach neu gemischt, und gezählt
//! wird, wie oft der Zufall mindestens so weit auseinanderliegt wie die
//! Wirklichkeit. Der Anteil ist der p-Wert.
//!
//! Das ist hier nicht die schicke, sondern die passende Wahl:
//!
//! - **Kein Gleitkomma.** Der Abstand ist ein Bruch zweier Ganzzahlen,
//!   der p-Wert ist ein Zähler über einem Nenner. Diese Datei steht in
//!   der Liste des Gleitkomma-Audits, und sie muss nicht um eine
//!   Ausnahme bitten.
//! - **Keine Verteilungsannahme.** Ein χ²- oder KS-Test bringt Tabellen
//!   kritischer Werte und Annahmen mit, die für Prompt-Längen niemand
//!   geprüft hat. Der Vertauschungstest ist exakt für die Daten, die er
//!   bekommt.
//! - **Reproduzierbar.** Gemischt wird mit [`SeedRng`], also führt
//!   derselbe Seed zum selben p-Wert. Ein Messwert, den niemand
//!   nachrechnen kann, ist in diesem Projekt kein Messwert.
//!
//! # ⚑ Das Raster ist Teil des Tests, nicht Beiwerk
//!
//! Ein Raster mit einem Fach findet nie etwas: Beide Verteilungen legen
//! alles ins selbe Fach, der Abstand ist null. Ein Raster, dessen
//! Fächer feiner sind als die Stichprobe groß ist, findet immer etwas:
//! Jeder Wert steht allein, der Abstand geht gegen eins.
//!
//! Beides sind stille Fehler, und beide sehen an der Aufrufstelle
//! gleich aus. [`Raster::plausibel_fuer`] rechnet deshalb nach, ob
//! Fächerzahl und Stichprobengröße zueinander passen, und die Eichung
//! unten misst es an einem Fall, statt es zu versprechen.

use myl_types::hash::Hash;
use myl_types::seed_rng::SeedRng;

/// Vorgabe für die Zahl der Vertauschungen.
///
/// 999 und nicht 1 000, damit der p-Wert `(t+1)/(v+1)` einen runden
/// Nenner hat: Der kleinste erreichbare p-Wert ist dann genau 1/1000,
/// also ein Promille.
pub const VERTAUSCHUNGEN_VORGABE: usize = 999;

/// Vorgabe für die Signifikanzschranke: 50 Promille, also 5 %.
pub const SIGNIFIKANZ_PROMILLE: u64 = 50;

/// Anteil der Läufe, ab dem eine Verschiebung als „erkennbar" gilt.
///
/// 800 Promille, also 80 %. Die übliche Zielgröße für Trennschärfe.
/// Bewusst nicht 950: Ein Aufbau, der eine Abweichung in vier von fünf
/// Fällen findet, ist brauchbar; einer, der 95 % verlangt, braucht so
/// viele Daten, dass er nie zum Einsatz kommt.
pub const TRENNSCHAERFE_PROMILLE: u64 = 800;

/// Das Fächerraster, in das die Merkmalswerte fallen.
///
/// Werte unterhalb der Untergrenze landen im ersten Fach, Werte
/// oberhalb im letzten. Das ist Absicht: Ein Ausreißer soll den Test
/// nicht sprengen, sondern gezählt werden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Raster {
    /// Unterster Wert des ersten Fachs.
    pub untergrenze: u64,
    /// Breite eines Fachs, in Einheiten des Merkmals.
    pub breite: u64,
    /// Zahl der Fächer.
    pub faecher: usize,
}

impl Raster {
    /// Ein Raster, das `spanne` in `faecher` gleich breite Fächer teilt.
    ///
    /// Gibt `None` für ein Raster, das nicht messen kann: null Fächer,
    /// null Breite.
    pub fn neu(untergrenze: u64, breite: u64, faecher: usize) -> Option<Self> {
        if faecher == 0 || breite == 0 {
            return None;
        }
        Some(Self {
            untergrenze,
            breite,
            faecher,
        })
    }

    /// In welches Fach ein Wert fällt.
    pub fn fach(&self, wert: u64) -> usize {
        let ueber = wert.saturating_sub(self.untergrenze);
        ((ueber / self.breite) as usize).min(self.faecher - 1)
    }

    /// Ist dieses Raster für Stichproben dieser Größe brauchbar?
    ///
    /// Zwei Grenzen, beide aus dem Modulkopf:
    ///
    /// - **Mindestens zwei Fächer.** Mit einem findet der Test nie
    ///   etwas, und das sähe wie ein Beleg aus.
    /// - **Im Mittel mindestens fünf Werte je Fach.** Darunter besteht
    ///   die Statistik aus Rauschen, und der Abstand misst die
    ///   Stichprobengröße statt der Verteilung.
    ///
    /// Die fünf ist die übliche Faustregel für Häufigkeitstabellen. Sie
    /// ist eine Faustregel und wird hier auch so genannt; verlassen
    /// wird sich auf die Eichung, nicht auf sie.
    pub fn plausibel_fuer(&self, kleinste_stichprobe: usize) -> bool {
        self.faecher >= 2 && kleinste_stichprobe >= 5 * self.faecher
    }
}

/// Das Ergebnis eines Vertauschungstests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Testergebnis {
    /// Umfang der Stichprobe aus echtem Verkehr.
    pub echt: usize,
    /// Umfang der Stichprobe aus Kontrollsegmenten.
    pub kontrolle: usize,
    /// Totaler Variationsabstand der beiden Verteilungen, in Promille.
    pub abstand_promille: u64,
    /// Wie oft eine zufällige Vertauschung mindestens so weit lag.
    pub mindestens_so_weit: usize,
    /// Wie oft vertauscht wurde.
    pub vertauschungen: usize,
}

impl Testergebnis {
    /// Der p-Wert in Promille, aufgerundet.
    ///
    /// `(t + 1) / (v + 1)`, die übliche Zählweise für
    /// Vertauschungstests: Die beobachtete Anordnung zählt mit, sonst
    /// könnte ein p-Wert von exakt null herauskommen, und den gibt es
    /// nicht.
    pub fn p_promille(&self) -> u64 {
        let zaehler = (self.mindestens_so_weit as u64 + 1) * 1_000;
        let nenner = self.vertauschungen as u64 + 1;
        zaehler.div_ceil(nenner)
    }

    /// Liegt der p-Wert unter der Schranke?
    pub fn signifikant(&self, schranke_promille: u64) -> bool {
        self.p_promille() <= schranke_promille
    }
}

/// Wie empfindlich ein Messaufbau ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trennschaerfe {
    /// Die geprüfte Verschiebung zwischen den beiden Verteilungen.
    pub verschiebung: u64,
    /// Wie viele Läufe sie gefunden haben.
    pub erkannt: usize,
    /// Wie viele Läufe gefahren wurden.
    pub laeufe: usize,
}

impl Trennschaerfe {
    /// Erkennungsanteil in Promille.
    pub fn anteil_promille(&self) -> u64 {
        if self.laeufe == 0 {
            return 0;
        }
        (self.erkannt as u64 * 1_000) / self.laeufe as u64
    }

    /// Gilt diese Verschiebung als erkennbar?
    pub fn erkennbar(&self) -> bool {
        self.anteil_promille() >= TRENNSCHAERFE_PROMILLE
    }
}

/// Das Ergebnis einer Ununterscheidbarkeitsprüfung.
///
/// # ⚑ Warum es kein nacktes `bool` gibt
///
/// Ein `bool` ließe „kein Unterschied gefunden" sagen, ohne dazuzusagen,
/// was dieser Aufbau hätte finden können. Genau diese Auslassung macht
/// das Akzeptanzkriterium der Phase von einem schlechten Test leichter
/// erfüllbar als von einem guten.
///
/// [`Befund::KeinNachweis`] trägt die Grenze im Wert. Wer das Ergebnis
/// weiterreicht, reicht sie mit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Befund {
    /// Die beiden Ströme unterscheiden sich nachweisbar.
    Unterschied {
        /// p-Wert in Promille.
        p_promille: u64,
        /// Abstand der Verteilungen in Promille.
        abstand_promille: u64,
    },
    /// Kein Unterschied nachgewiesen, **und so scharf war das Gerät**.
    KeinNachweis {
        /// p-Wert in Promille.
        p_promille: u64,
        /// Kleinste Verschiebung, die dieser Aufbau in mindestens
        /// [`TRENNSCHAERFE_PROMILLE`] der Läufe gefunden hätte.
        ///
        /// `None` heißt: Der Aufbau hat im geprüften Bereich **gar
        /// nichts** gefunden. Dann ist das Nein wertlos, und zwar
        /// erkennbar.
        erkennbar_ab: Option<u64>,
    },
}

impl Befund {
    /// Darf aus diesem Befund „ununterscheidbar" gefolgert werden?
    ///
    /// Nur, wenn kein Unterschied gefunden wurde **und** der Aufbau
    /// überhaupt etwas hätte finden können.
    pub fn traegt_die_aussage(&self) -> bool {
        matches!(
            self,
            Befund::KeinNachweis {
                erkennbar_ab: Some(_),
                ..
            }
        )
    }
}

/// Zählt die Werte in die Fächer.
fn haeufigkeiten(raster: &Raster, werte: &[u64]) -> Vec<u64> {
    let mut zaehl = vec![0u64; raster.faecher];
    for &w in werte {
        zaehl[raster.fach(w)] += 1;
    }
    zaehl
}

/// Totaler Variationsabstand der beiden empirischen Verteilungen, in
/// Promille.
///
/// `½ · Σ |a_i/n_a − b_i/n_b|`, durchgehend ganzzahlig gerechnet: Die
/// Brüche werden auf den gemeinsamen Nenner `n_a · n_b` gebracht, die
/// Summe der Beträge in `u128` gebildet und erst am Ende geteilt.
/// Zwischenwerte bleiben damit exakt.
pub fn abstand_promille(raster: &Raster, echt: &[u64], kontrolle: &[u64]) -> u64 {
    if echt.is_empty() || kontrolle.is_empty() {
        return 0;
    }
    let a = haeufigkeiten(raster, echt);
    let b = haeufigkeiten(raster, kontrolle);
    let n_a = echt.len() as u128;
    let n_b = kontrolle.len() as u128;

    let mut summe: u128 = 0;
    for fach in 0..raster.faecher {
        let links = a[fach] as u128 * n_b;
        let rechts = b[fach] as u128 * n_a;
        summe += links.abs_diff(rechts);
    }
    // ½ · summe / (n_a · n_b), in Promille.
    ((summe * 1_000) / (2 * n_a * n_b)) as u64
}

/// Seed für die `i`-te Vertauschung: `sha256(seed ‖ i)`.
///
/// Abgeleitet statt hochgezählt, damit aufeinanderfolgende Läufe keine
/// verwandten Mischungen bekommen.
fn vertauschungs_seed(seed: &[u8; 32], i: usize) -> [u8; 32] {
    let mut daten = Vec::with_capacity(40);
    daten.extend_from_slice(seed);
    daten.extend_from_slice(&(i as u64).to_le_bytes());
    let h = Hash::sha256(&daten);
    let mut roh = [0u8; 32];
    roh.copy_from_slice(h.as_bytes());
    roh
}

/// Vergleicht zwei Stichproben mit einem Vertauschungstest.
pub fn vergleiche(
    raster: &Raster,
    echt: &[u64],
    kontrolle: &[u64],
    vertauschungen: usize,
    seed: &[u8; 32],
) -> Testergebnis {
    let beobachtet = abstand_promille(raster, echt, kontrolle);

    let mut topf: Vec<u64> = Vec::with_capacity(echt.len() + kontrolle.len());
    topf.extend_from_slice(echt);
    topf.extend_from_slice(kontrolle);

    let mut mindestens_so_weit = 0usize;
    for i in 0..vertauschungen {
        let mut gemischt = topf.clone();
        myl_types::seed_rng::deterministic_shuffle(&mut gemischt, &vertauschungs_seed(seed, i));
        let (links, rechts) = gemischt.split_at(echt.len());
        if abstand_promille(raster, links, rechts) >= beobachtet {
            mindestens_so_weit += 1;
        }
    }

    Testergebnis {
        echt: echt.len(),
        kontrolle: kontrolle.len(),
        abstand_promille: beobachtet,
        mindestens_so_weit,
        vertauschungen,
    }
}

/// Der Messaufbau, an dem die Trennschärfe geeicht wird.
///
/// # ⚑ Die Eichverteilung ist eine Annahme und wird als solche genannt
///
/// Geeicht wird gegen eine **gleichverteilte** Modellgröße mit einer
/// festen Spanne. Das ist keine Behauptung über echte Prompt-Längen; die
/// kennt niemand, und sie sind mit Sicherheit nicht gleichverteilt.
///
/// Was die Eichung liefert, ist eine Eigenschaft **des Geräts**: wie
/// viele Proben es braucht, um eine Verschiebung gegebener Größe zu
/// finden. Diese Zahl ist der Teil der Antwort, der schon heute
/// feststeht, und sie ist der Teil, den man im Betrieb braucht, **bevor**
/// man misst: Sie sagt, wie lange gesammelt werden muss, damit ein Nein
/// etwas bedeutet.
#[derive(Debug, Clone, Copy)]
pub struct Aufbau {
    /// Raster, in dem gemessen wird.
    pub raster: Raster,
    /// Umfang der Stichprobe aus echtem Verkehr.
    pub n_echt: usize,
    /// Umfang der Stichprobe aus Kontrollsegmenten.
    pub n_kontrolle: usize,
    /// Spanne der gleichverteilten Modellgröße.
    pub spanne: u64,
    /// Zahl der Vertauschungen je Test.
    pub vertauschungen: usize,
    /// Signifikanzschranke in Promille.
    pub schranke_promille: u64,
}

impl Aufbau {
    /// Ein Aufbau mit den Vorgaben dieses Moduls.
    pub fn vorgabe(raster: Raster, n_echt: usize, n_kontrolle: usize, spanne: u64) -> Self {
        Self {
            raster,
            n_echt,
            n_kontrolle,
            spanne,
            vertauschungen: VERTAUSCHUNGEN_VORGABE,
            schranke_promille: SIGNIFIKANZ_PROMILLE,
        }
    }
}

/// Zieht `n` gleichverteilte Werte aus `[versatz, versatz + spanne)`.
fn ziehe(rng: &mut SeedRng, n: usize, spanne: u64, versatz: u64) -> Vec<u64> {
    (0..n).map(|_| versatz + rng.next_below(spanne)).collect()
}

/// Wie oft dieser Aufbau eine Verschiebung um `verschiebung` findet.
pub fn trennschaerfe(
    aufbau: &Aufbau,
    verschiebung: u64,
    laeufe: usize,
    seed: &[u8; 32],
) -> Trennschaerfe {
    let mut erkannt = 0usize;
    for lauf in 0..laeufe {
        let mut rng = SeedRng::new(&vertauschungs_seed(seed, lauf));
        let echt = ziehe(&mut rng, aufbau.n_echt, aufbau.spanne, 0);
        let kontrolle = ziehe(&mut rng, aufbau.n_kontrolle, aufbau.spanne, verschiebung);
        let ergebnis = vergleiche(
            &aufbau.raster,
            &echt,
            &kontrolle,
            aufbau.vertauschungen,
            &vertauschungs_seed(seed, lauf + 1_000_000),
        );
        if ergebnis.signifikant(aufbau.schranke_promille) {
            erkannt += 1;
        }
    }
    Trennschaerfe {
        verschiebung,
        erkannt,
        laeufe,
    }
}

/// Die kleinste Verschiebung, die dieser Aufbau zuverlässig findet.
///
/// Sucht aufsteigend über `kandidaten` und gibt die erste zurück, die
/// [`Trennschaerfe::erkennbar`] erfüllt. `None` heißt: keine davon,
/// also ist der Aufbau für diesen Bereich blind.
pub fn erkennbar_ab(
    aufbau: &Aufbau,
    kandidaten: &[u64],
    laeufe: usize,
    seed: &[u8; 32],
) -> Option<u64> {
    let mut sortiert: Vec<u64> = kandidaten.to_vec();
    sortiert.sort_unstable();
    sortiert
        .into_iter()
        .find(|&v| trennschaerfe(aufbau, v, laeufe, seed).erkennbar())
}

/// Der vollständige Befund: Test **und** Trennschärfe.
///
/// # ⚑ Warum die Eichung mit im selben Aufruf steckt
///
/// Sie könnte danebenstehen, und dann würde sie irgendwann vergessen.
/// Ein Nein ohne Trennschärfe sieht an der Aufrufstelle genauso
/// überzeugend aus wie eines mit, und es ist wertlos. Also gibt es das
/// eine nur zusammen mit dem anderen.
///
/// Die Eichung läuft **nur**, wenn kein Unterschied gefunden wurde: Wo
/// etwas gefunden ist, braucht niemand zu wissen, was man sonst noch
/// gefunden hätte.
pub fn befund(
    aufbau: &Aufbau,
    echt: &[u64],
    kontrolle: &[u64],
    kandidaten: &[u64],
    eich_laeufe: usize,
    seed: &[u8; 32],
) -> Befund {
    let ergebnis = vergleiche(
        &aufbau.raster,
        echt,
        kontrolle,
        aufbau.vertauschungen,
        seed,
    );
    let p = ergebnis.p_promille();
    if ergebnis.signifikant(aufbau.schranke_promille) {
        return Befund::Unterschied {
            p_promille: p,
            abstand_promille: ergebnis.abstand_promille,
        };
    }
    Befund::KeinNachweis {
        p_promille: p,
        erkennbar_ab: erkennbar_ab(aufbau, kandidaten, eich_laeufe, seed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saat(n: u8) -> [u8; 32] {
        [n; 32]
    }

    /// Ein Aufbau, der klein genug für einen Testlauf ist und groß
    /// genug, um etwas auszusagen.
    fn kleiner_aufbau(faecher: usize, n: usize) -> Aufbau {
        Aufbau {
            raster: Raster::neu(0, 10, faecher).expect("Raster"),
            n_echt: n,
            n_kontrolle: n,
            spanne: 10 * faecher as u64,
            vertauschungen: 199,
            schranke_promille: SIGNIFIKANZ_PROMILLE,
        }
    }

    #[test]
    fn ein_raster_ohne_faecher_oder_breite_gibt_es_nicht() {
        assert!(Raster::neu(0, 10, 0).is_none());
        assert!(Raster::neu(0, 0, 8).is_none());
        assert!(Raster::neu(0, 1, 1).is_some());
    }

    #[test]
    fn das_raster_faengt_ausreisser_ab() {
        // Ein Ausreißer soll den Test nicht sprengen, sondern gezählt
        // werden.
        let r = Raster::neu(100, 10, 4).expect("Raster");
        assert_eq!(r.fach(0), 0, "unter der Untergrenze gehört ins erste Fach");
        assert_eq!(r.fach(100), 0);
        assert_eq!(r.fach(109), 0);
        assert_eq!(r.fach(110), 1);
        assert_eq!(r.fach(139), 3);
        assert_eq!(r.fach(u64::MAX), 3, "über der Obergrenze ins letzte Fach");
    }

    #[test]
    fn ein_raster_das_nichts_finden_kann_gilt_als_unbrauchbar() {
        // Ein Fach: beide Verteilungen legen alles hinein, der Abstand
        // ist immer null, und das sähe wie ein Beleg aus.
        assert!(!Raster::neu(0, 10, 1).unwrap().plausibel_fuer(10_000));
        // Zu fein für die Stichprobe: Jeder Wert steht allein, und der
        // Abstand misst die Stichprobengröße statt der Verteilung.
        assert!(!Raster::neu(0, 1, 100).unwrap().plausibel_fuer(400));
        assert!(Raster::neu(0, 1, 100).unwrap().plausibel_fuer(500));
    }

    #[test]
    fn gleiche_stichproben_haben_abstand_null() {
        let r = Raster::neu(0, 10, 8).expect("Raster");
        let werte: Vec<u64> = (0..80).collect();
        assert_eq!(abstand_promille(&r, &werte, &werte), 0);
    }

    #[test]
    fn voellig_getrennte_stichproben_haben_den_groessten_abstand() {
        // Totale Variation ist auf eins begrenzt, also auf 1000
        // Promille.
        let r = Raster::neu(0, 10, 8).expect("Raster");
        let links: Vec<u64> = vec![5; 50];
        let rechts: Vec<u64> = vec![75; 50];
        assert_eq!(abstand_promille(&r, &links, &rechts), 1_000);
    }

    #[test]
    fn der_abstand_ist_symmetrisch() {
        let r = Raster::neu(0, 10, 8).expect("Raster");
        let a: Vec<u64> = (0..40).map(|i| i * 2).collect();
        let b: Vec<u64> = (0..60).map(|i| i + 10).collect();
        assert_eq!(abstand_promille(&r, &a, &b), abstand_promille(&r, &b, &a));
    }

    #[test]
    fn eine_leere_stichprobe_stuerzt_nicht_ab() {
        let r = Raster::neu(0, 10, 8).expect("Raster");
        assert_eq!(abstand_promille(&r, &[], &[1, 2, 3]), 0);
        assert_eq!(abstand_promille(&r, &[1, 2, 3], &[]), 0);
    }

    #[test]
    fn der_p_wert_bleibt_in_seinen_grenzen() {
        // Kleiner als 1/(v+1) kann er nicht werden, größer als 1 auch
        // nicht.
        for (t, v) in [(0usize, 199usize), (199, 199), (100, 199)] {
            let e = Testergebnis {
                echt: 10,
                kontrolle: 10,
                abstand_promille: 0,
                mindestens_so_weit: t,
                vertauschungen: v,
            };
            assert!(e.p_promille() >= 5, "p unter 1/(v+1): {}", e.p_promille());
            assert!(e.p_promille() <= 1_000, "p über eins: {}", e.p_promille());
        }
    }

    #[test]
    fn derselbe_seed_ergibt_denselben_p_wert() {
        // Ein Messwert, den niemand nachrechnen kann, ist kein Messwert.
        let r = Raster::neu(0, 10, 8).expect("Raster");
        let a: Vec<u64> = (0..100).map(|i| i % 80).collect();
        let b: Vec<u64> = (0..100).map(|i| (i * 3) % 80).collect();
        let e1 = vergleiche(&r, &a, &b, 99, &saat(4));
        let e2 = vergleiche(&r, &a, &b, 99, &saat(4));
        assert_eq!(e1, e2);
    }

    #[test]
    fn unter_der_nullhypothese_schlaegt_der_test_selten_an() {
        // ⚑ Die wichtigste Eichung: Wenn beide Stichproben aus
        // derselben Verteilung kommen, darf der Test nur mit der
        // eingestellten Rate anschlagen. Ein Test, der zu oft anschlägt,
        // meldet Unterschiede, die es nicht gibt, und würde den
        // Kontrollsegment-Mechanismus grundlos verwerfen.
        //
        // Geprüft wird gegen eine großzügige Obergrenze, nicht gegen die
        // 5 % selbst: Bei 60 Läufen ist die Streuung erheblich, und ein
        // Test, der an der eigenen Streuung scheitert, ist ein
        // Flackertest.
        let aufbau = kleiner_aufbau(8, 200);
        let t = trennschaerfe(&aufbau, 0, 60, &saat(11));
        assert!(
            t.anteil_promille() <= 200,
            "Fehlalarmrate {} Promille, erwartet um 50",
            t.anteil_promille()
        );
    }

    #[test]
    fn eine_deutliche_verschiebung_wird_gefunden() {
        // Die Gegenprobe zur Eichung: Ohne sie hieße „selten anschlagen"
        // vielleicht nur „nie anschlagen".
        let aufbau = kleiner_aufbau(8, 200);
        let t = trennschaerfe(&aufbau, 40, 60, &saat(12));
        assert!(
            t.erkennbar(),
            "eine Verschiebung um die halbe Spanne wurde nicht gefunden ({} Promille)",
            t.anteil_promille()
        );
    }

    #[test]
    fn ein_blinder_aufbau_meldet_seine_blindheit() {
        // ⚑ Der Kern dieses Moduls. Ein Raster mit einem Fach findet
        // nichts, und genau deshalb würde es „kein Unterschied" melden.
        // Der Befund muss diese Blindheit mittragen, sonst liest sich
        // das Versagen des Geräts wie ein Ergebnis.
        let mut aufbau = kleiner_aufbau(8, 200);
        aufbau.raster = Raster::neu(0, 1_000_000, 1).expect("Raster");
        let echt: Vec<u64> = vec![1; 200];
        let kontrolle: Vec<u64> = vec![500_000; 200];

        let b = befund(&aufbau, &echt, &kontrolle, &[10, 40], 20, &saat(13));
        match b {
            Befund::KeinNachweis { erkennbar_ab, .. } => {
                assert_eq!(erkennbar_ab, None, "ein blindes Raster meldet Trennschärfe");
            }
            ref andere => panic!("ein Raster mit einem Fach fand einen Unterschied: {andere:?}"),
        }
        assert!(
            !b.traegt_die_aussage(),
            "ein blinder Aufbau trägt die Aussage ununterscheidbar"
        );
    }

    #[test]
    fn ein_taugliches_nein_traegt_seine_trennschaerfe() {
        // Der Gegenfall: gleiche Verteilungen, brauchbares Raster. Dann
        // ist „kein Unterschied" eine Aussage, weil danebensteht, was
        // der Aufbau gefunden hätte.
        let aufbau = kleiner_aufbau(8, 200);
        let mut rng = SeedRng::new(&saat(14));
        let echt = ziehe(&mut rng, 200, aufbau.spanne, 0);
        let kontrolle = ziehe(&mut rng, 200, aufbau.spanne, 0);

        let b = befund(&aufbau, &echt, &kontrolle, &[40], 20, &saat(15));
        match b {
            Befund::KeinNachweis { erkennbar_ab, .. } => {
                assert_eq!(erkennbar_ab, Some(40));
                assert!(b.traegt_die_aussage());
            }
            ref andere => panic!("gleiche Verteilungen wurden getrennt: {andere:?}"),
        }
    }

    #[test]
    fn ein_gefundener_unterschied_kommt_ohne_eichung() {
        // Wo etwas gefunden ist, braucht niemand zu wissen, was man
        // sonst noch gefunden hätte. Der Typ sagt das mit.
        let aufbau = kleiner_aufbau(8, 200);
        let echt: Vec<u64> = vec![5; 200];
        let kontrolle: Vec<u64> = vec![75; 200];
        let b = befund(&aufbau, &echt, &kontrolle, &[40], 20, &saat(16));
        assert!(matches!(b, Befund::Unterschied { .. }));
        assert!(!b.traegt_die_aussage(), "ein Unterschied trägt kein Nein");
    }

    #[test]
    fn die_trennschaerfe_waechst_mit_der_verschiebung() {
        // Ein Gerät, das bei größerer Abweichung nicht empfindlicher
        // wird, misst etwas anderes als die Abweichung.
        let aufbau = kleiner_aufbau(8, 200);
        let klein = trennschaerfe(&aufbau, 0, 40, &saat(17)).anteil_promille();
        let mittel = trennschaerfe(&aufbau, 20, 40, &saat(17)).anteil_promille();
        let gross = trennschaerfe(&aufbau, 60, 40, &saat(17)).anteil_promille();
        assert!(klein <= mittel, "{klein} > {mittel}");
        assert!(mittel <= gross, "{mittel} > {gross}");
        assert_eq!(
            gross, 1_000,
            "die größte Verschiebung wurde nicht immer gefunden"
        );
    }

    #[test]
    fn erkennbar_ab_gibt_die_kleinste_zurueck() {
        let aufbau = kleiner_aufbau(8, 200);
        // Unsortiert hereingegeben: Die Funktion sortiert selbst, sonst
        // hinge das Ergebnis an der Reihenfolge der Kandidaten.
        let gefunden = erkennbar_ab(&aufbau, &[60, 0, 40], 40, &saat(18));
        assert_eq!(gefunden, Some(40));
    }

    #[test]
    fn ohne_kandidaten_gibt_es_keine_trennschaerfe() {
        let aufbau = kleiner_aufbau(8, 200);
        assert_eq!(erkennbar_ab(&aufbau, &[], 10, &saat(19)), None);
    }
}
