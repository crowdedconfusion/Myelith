//! **Wie ein Pod seine Ebenen auf seine Mitglieder verteilt** (Stufe 4 des
//! Dispatchers, Festlegungen des Projektinhabers vom 2026-10-15 und
//! 2026-10-21).
//!
//! **Konsens-Feld:** Jeder Knoten rechnet aus denselben Zusagen und derselben
//! Saat dieselben Grenzen. Änderungen nur über Governance (Kap. 10.3).
//!
//! # Warum ungleich, und warum das keine Zahl ändert
//!
//! Bis heute schneidet ein Pod gleich, `ebenen · s / k`. Mitglieder sind aber
//! verschieden stark: Einer hält mehr Ebenen, ein anderer rechnet eine Ebene
//! schneller. Der Zuschnitt ändert keine Zahl des Modells (belegt in
//! `myl-pod`: dieselbe Spur für k = 1 bis 24 und ungleiche Grenzen), also darf
//! er nach Kapazität und Zeit schneiden. Ziel ist eine kurze **Engpasszeit**,
//! die Zeit des langsamsten Bereichs je Token.
//!
//! # Das Verfahren, ganz in ganzen Zahlen
//!
//! 1. **Machbar** nur, wenn jedes Mitglied mindestens eine Ebene hält, die
//!    Kapazitäten zusammen alle Ebenen tragen und das Mitglied mit dem Kopf
//!    ihn neben einer Ebene halten kann.
//! 2. **Der Kopf geht an das schnellste Mitglied**, das ihn tragen kann
//!    (kleinste Ebenenzeit, Gleichstand nach Kennung). Er zählt **zweimal,
//!    und verschieden**: im Platz als `kopf_platz` Ebenen (Bytes des Kopfs
//!    durch Bytes einer mittleren Ebene), in der Zeit als `kopf_zeit`
//!    Ebenenzeiten (gemessen). 📌 Gemessen am 2026-10-21: Raven 35B A3B int4
//!    Platz 4, Zeit 1 (eine Gemischebene liest mehr als ihr Anteil am Platz
//!    vermuten lässt); Sparrow 0.6B Zeit 5. Eine Zahl für beides hätte dem
//!    Mitglied mit Kopf falsch viele Ebenen gegeben.
//! 3. **Wasserfüllen**: Zu einer Zeit T trägt Mitglied i
//!    `min(c_i, max(1, T / t_i))` Ebenen, das Mitglied mit Kopf
//!    `min(c_i − kopf_platz, max(1, (T − kopf_zeit · t_i) / t_i))`. Gesucht wird per Binärsuche das
//!    kleinste T, bei dem alle zusammen mindestens alle Ebenen tragen. Bei
//!    T − 1 fehlen noch einige; sie gehen an die Mitglieder, die bei T eine
//!    Ebene mehr bekämen, die schnellsten zuerst, Gleichstand nach Kennung.
//!    Damit ist die Summe genau die Zahl der Ebenen.
//! 4. **Reihenfolge**: Das Mitglied mit dem Kopf rechnet den letzten
//!    Bereich; die übrigen kommen in der Folge, die die Saat mischt, damit
//!    niemand seine Stelle wählt.
//!
//! ⚑ **Kein Umplanen mit Schwelle** (2026-10-21, beim Bau geprüft): Geplant
//! war, nur bei echtem Gewinn umzuplanen (`4·neu ≤ 3·alt`). Die Pod-Bildung
//! mischt die Miner aber jede Epoche neu aus der Saat des Blocks; ein Pod
//! besteht eine Epoche, und es gibt kaum je einen alten Plan desselben Pods.
//! Darum wird jede Epoche frisch verteilt. Ein neuer Bereich ist mit dem
//! ganzen Artefakt auf der Platte billig: Es ändert sich, welche Seiten warm
//! sind.
//!
//! ⚑ **Die Zeit ist eine Frist und kein Nachweis.** Wer sie zu niedrig
//! ansagt, bekommt mehr Ebenen, bremst seinen Pod und verdient nicht mehr,
//! als er leistet; wer zu viele Ebenen ansagt, hält sie nicht und fällt aus.

use myl_types::ids::MinerId;

/// Ein Mitglied, wie seine Zusage es beschreibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mitglied {
    pub kennung: MinerId,
    /// Wie viele Ebenen es höchstens hält (`ebenen_hoechstens`), den Kopf in
    /// Ebenen-Äquivalenten mitgezählt.
    pub kapazitaet: u32,
    /// Mikrosekunden je Ebene und Token im Decode (`ebenenzeit_us`), ≥ 1.
    pub ebenenzeit_us: u64,
    /// ⚑ **Teilablage** (P10, 2026-10-22): Hält es nur `[von, bis)` auf der
    /// Platte, bekommt es nur Ebenen darin. `None`: das ganze Artefakt.
    pub gehalten: Option<(u32, u32)>,
}

impl Mitglied {
    /// Trägt es den Bereich `[von, bis)`, nach seinem gehaltenen Teil?
    pub fn haelt(&self, von: u32, bis: u32) -> bool {
        self.gehalten.is_none_or(|(g, h)| g <= von && bis <= h)
    }
}

/// Ein Bereich `[von, bis)` und wer ihn rechnet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bereich {
    pub kennung: MinerId,
    pub von: u32,
    pub bis: u32,
    /// Der letzte Bereich trägt den Kopf.
    pub mit_kopf: bool,
}

impl Bereich {
    pub fn ebenen(&self) -> u32 {
        self.bis - self.von
    }
}

/// Warum ein Pod nicht machbar ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verteilfehler {
    /// Kein Mitglied.
    Leer,
    /// Mehr Mitglieder als Ebenen: Jeder Bereich braucht eine Ebene.
    MehrMitgliederAlsEbenen { mitglieder: usize, ebenen: u32 },
    /// Eine Zusage über null Ebenen oder null Zeit.
    UngueltigeZusage(MinerId),
    /// Niemand kann den Kopf neben einer Ebene halten.
    KopfPasstNicht { kopf: u32 },
    /// Die Kapazitäten tragen nicht alle Ebenen.
    ZuWenigKapazitaet { summe: u64, ebenen: u32 },
    /// Die gehaltenen Bereiche decken in dieser Folge nicht alle Ebenen.
    TeilablageDecktNicht,
}

/// **Verteilt `ebenen` Ebenen auf die Mitglieder**; `kopf_platz` ist der Kopf
/// in Ebenen nach Bytes (aufgerundet), `kopf_zeit` in Ebenenzeiten
/// (gemessen), `saat` die Saat der Pod-Bildung.
///
/// Das Ergebnis ist in Rechenfolge: erster Bereich ab 0, letzter bis
/// `ebenen`, mit Kopf.
pub fn verteilen(
    mitglieder: &[Mitglied],
    ebenen: u32,
    kopf_platz: u32,
    kopf_zeit: u32,
    saat: &[u8; 32],
) -> Result<Vec<Bereich>, Verteilfehler> {
    pruefe_eingabe(mitglieder, ebenen)?;
    // 2. Der Kopf: das schnellste Mitglied, das ihn neben einer Ebene hält.
    let mut nach_tempo: Vec<usize> = (0..mitglieder.len()).collect();
    nach_tempo.sort_by_key(|&i| (mitglieder[i].ebenenzeit_us, mitglieder[i].kennung));
    let kopf_bei = *nach_tempo
        .iter()
        .find(|&&i| mitglieder[i].kapazitaet > kopf_platz)
        .ok_or(Verteilfehler::KopfPasstNicht { kopf: kopf_platz })?;
    // 4. Reihenfolge: gemischt nach der Saat, der Kopf zuletzt.
    let mut folge: Vec<usize> = (0..mitglieder.len()).filter(|&i| i != kopf_bei).collect();
    folge.sort_by_key(|&i| mitglieder[i].kennung);
    myl_types::seed_rng::deterministic_shuffle(&mut folge, saat);
    folge.push(kopf_bei);
    let geordnet: Vec<Mitglied> = folge.iter().map(|&i| mitglieder[i]).collect();
    verteilen_geordnet(&geordnet, ebenen, kopf_platz, kopf_zeit)
}

fn pruefe_eingabe(mitglieder: &[Mitglied], ebenen: u32) -> Result<(), Verteilfehler> {
    if mitglieder.is_empty() {
        return Err(Verteilfehler::Leer);
    }
    if mitglieder.len() as u64 > ebenen as u64 {
        return Err(Verteilfehler::MehrMitgliederAlsEbenen { mitglieder: mitglieder.len(), ebenen });
    }
    if let Some(m) = mitglieder.iter().find(|m| m.kapazitaet == 0 || m.ebenenzeit_us == 0) {
        return Err(Verteilfehler::UngueltigeZusage(m.kennung));
    }
    Ok(())
}

/// **Verteilt in der gegebenen Folge**: Der erste Bereich geht an das erste
/// Mitglied, der letzte mit dem Kopf an das letzte. Das ist der Weg der
/// Kette (siehe [`bereiche_des_pods`]): Dort steht die Folge der Positionen
/// schon fest, gemischt aus der Saat der Pod-Bildung, und an ihr hängen
/// Besetzung, Übernahme und Bündelprüfung.
pub fn verteilen_geordnet(
    mitglieder: &[Mitglied],
    ebenen: u32,
    kopf_platz: u32,
    kopf_zeit: u32,
) -> Result<Vec<Bereich>, Verteilfehler> {
    pruefe_eingabe(mitglieder, ebenen)?;
    let kopf = kopf_platz;
    let kopf_bei = mitglieder.len() - 1;
    if mitglieder[kopf_bei].kapazitaet <= kopf {
        return Err(Verteilfehler::KopfPasstNicht { kopf });
    }
    let mut nach_tempo: Vec<usize> = (0..mitglieder.len()).collect();
    nach_tempo.sort_by_key(|&i| (mitglieder[i].ebenenzeit_us, mitglieder[i].kennung));
    let kap: Vec<u32> = (0..mitglieder.len())
        .map(|i| if i == kopf_bei { mitglieder[i].kapazitaet - kopf } else { mitglieder[i].kapazitaet })
        .collect();
    let summe: u64 = kap.iter().map(|&c| c as u64).sum();
    if summe < ebenen as u64 {
        return Err(Verteilfehler::ZuWenigKapazitaet { summe, ebenen });
    }
    if mitglieder.iter().any(|m| m.gehalten.is_some()) {
        return verteilen_mit_teilablage(mitglieder, &kap, ebenen, kopf_zeit);
    }

    // 3. Wasserfüllen über die Zeit T.
    let traegt = |t: u64| -> Vec<u32> {
        (0..mitglieder.len())
            .map(|i| {
                let ti = mitglieder[i].ebenenzeit_us;
                let rest = if i == kopf_bei { t.saturating_sub((kopf_zeit as u64).saturating_mul(ti)) } else { t };
                let n = (rest / ti).max(1);
                n.min(kap[i] as u64) as u32
            })
            .collect()
    };
    let gesamt = |x: &[u32]| x.iter().map(|&n| n as u64).sum::<u64>();
    let langsamste = mitglieder.iter().map(|m| m.ebenenzeit_us).max().unwrap_or(1);
    let (mut unten, mut oben) = (0u64, langsamste.saturating_mul(ebenen as u64 + kopf_zeit as u64));
    while unten < oben {
        let mitte = unten + (oben - unten) / 2;
        if gesamt(&traegt(mitte)) >= ebenen as u64 {
            oben = mitte;
        } else {
            unten = mitte + 1;
        }
    }
    let t = unten;
    let mut x = if t == 0 { traegt(0) } else { traegt(t - 1) };
    let mut fehlt = (ebenen as u64).saturating_sub(gesamt(&x));
    if fehlt > 0 {
        let bei_t = traegt(t);
        for &i in &nach_tempo {
            if fehlt == 0 {
                break;
            }
            if bei_t[i] > x[i] {
                x[i] += 1;
                fehlt -= 1;
            }
        }
    }
    debug_assert_eq!(gesamt(&x), ebenen as u64);

    let mut von = 0u32;
    Ok((0..mitglieder.len())
        .map(|i| {
            let b = Bereich { kennung: mitglieder[i].kennung, von, bis: von + x[i], mit_kopf: i == kopf_bei };
            von = b.bis;
            b
        })
        .collect())
}

/// Wie viele Ebenen Mitglied `i` zur Zeit `t` trägt: höchstens seine
/// Kapazität, mindestens eine; das letzte rechnet den Kopf mit.
fn traegt_bei(mitglieder: &[Mitglied], kap: &[u32], kopf_zeit: u32, i: usize, t: u64) -> u32 {
    let ti = mitglieder[i].ebenenzeit_us;
    let rest = if i + 1 == mitglieder.len() { t.saturating_sub((kopf_zeit as u64).saturating_mul(ti)) } else { t };
    (rest / ti).max(1).min(kap[i] as u64) as u32
}

/// **Verteilt mit gehaltenen Bereichen** (P10, Teilablage nach Weg a,
/// 2026-10-22): Mitglied `i` bekommt nur Ebenen in seinem Teil.
///
/// # ⚑ Das Verfahren
///
/// Die Folge steht fest, die Bereiche sind lückenlos; gesucht sind die
/// Grenzen `0 = B₀ < B₁ < … < B_k = L`. Zu einer Zeit T trägt Mitglied i
/// höchstens `n_i(T)` Ebenen (wie beim Wasserfüllen). Ob T reicht, sagt ein
/// Lauf von vorn: Die erreichbaren Werte jeder Grenze bilden ein
/// Intervall, und jedes Mitglied schneidet es mit seinem Teil. Per
/// Binärsuche das kleinste T, bei dem `L` erreichbar ist; danach von hinten
/// die Grenzen, jedes Mitglied so viele Ebenen, wie es bei T trägt.
///
/// ⚑ **Ohne gehaltene Bereiche wird dieser Weg nicht genommen**: Dort
/// bleibt das Wasserfüllen mit seiner Restverteilung, bitgleich wie vor dem
/// 2026-10-22.
fn verteilen_mit_teilablage(
    mitglieder: &[Mitglied],
    kap: &[u32],
    ebenen: u32,
    kopf_zeit: u32,
) -> Result<Vec<Bereich>, Verteilfehler> {
    let k = mitglieder.len();
    // Je Mitglied das Intervall seiner Startgrenze, nach dem Lauf von vorn.
    let lauf = |t: u64| -> Option<Vec<(u32, u32)>> {
        let mut start = Vec::with_capacity(k);
        let (mut lo, mut hi) = (0u32, 0u32);
        for (i, m) in mitglieder.iter().enumerate() {
            let (g, h) = m.gehalten.unwrap_or((0, ebenen));
            let s_lo = lo.max(g);
            if s_lo > hi {
                return None;
            }
            start.push((s_lo, hi));
            let n = traegt_bei(mitglieder, kap, kopf_zeit, i, t);
            // Jedes spätere Mitglied braucht noch eine Ebene.
            let rand = ebenen - (k - 1 - i) as u32;
            lo = s_lo + 1;
            hi = (hi + n).min(h).min(rand);
            if lo > hi {
                return None;
            }
        }
        (lo <= ebenen && ebenen <= hi).then_some(start)
    };
    let langsamste = mitglieder.iter().map(|m| m.ebenenzeit_us).max().unwrap_or(1);
    let (mut unten, mut oben) = (0u64, langsamste.saturating_mul(ebenen as u64 + kopf_zeit as u64));
    if lauf(oben).is_none() {
        return Err(Verteilfehler::TeilablageDecktNicht);
    }
    while unten < oben {
        let mitte = unten + (oben - unten) / 2;
        if lauf(mitte).is_some() {
            oben = mitte;
        } else {
            unten = mitte + 1;
        }
    }
    let t = unten;
    let start = lauf(t).ok_or(Verteilfehler::TeilablageDecktNicht)?;
    let mut grenzen = vec![0u32; k + 1];
    grenzen[k] = ebenen;
    for i in (0..k).rev() {
        let n = traegt_bei(mitglieder, kap, kopf_zeit, i, t);
        let (s_lo, s_hi) = start[i];
        // So viele Ebenen, wie es bei T trägt, im erreichbaren Intervall.
        let b = grenzen[i + 1].saturating_sub(n).max(s_lo).min(s_hi).min(grenzen[i + 1] - 1);
        grenzen[i] = b;
    }
    debug_assert_eq!(grenzen[0], 0);
    Ok((0..k)
        .map(|i| Bereich { kennung: mitglieder[i].kennung, von: grenzen[i], bis: grenzen[i + 1], mit_kopf: i + 1 == k })
        .collect())
}

/// **Die Engpasszeit** eines Schnitts: der langsamste Bereich je Token, der
/// Kopf beim letzten mit `kopf_zeit` Ebenenzeiten, in Mikrosekunden.
pub fn engpass_us(bereiche: &[Bereich], mitglieder: &[Mitglied], kopf_zeit: u32) -> Option<u64> {
    let kopf = kopf_zeit;
    bereiche
        .iter()
        .map(|b| {
            let m = mitglieder.iter().find(|m| m.kennung == b.kennung)?;
            let n = b.ebenen() as u64 + if b.mit_kopf { kopf as u64 } else { 0 };
            Some(n.saturating_mul(m.ebenenzeit_us))
        })
        .try_fold(0u64, |a, x| Some(a.max(x?)))
}

/// Wie ein Pod seine Ebenen schneidet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Podschnitt {
    /// Nach Kapazität und Zeit, je Position.
    Verteilt(Vec<Bereich>),
    /// Mit den Zusagen nicht machbar; der Pod rechnet in dieser Epoche nicht.
    NichtMachbar(String),
}

impl Podschnitt {
    /// Die Grenzen je Position, oder `None`, wenn nicht machbar.
    pub fn grenzen(&self) -> Option<Vec<(u32, u32)>> {
        match self {
            Self::Verteilt(b) => Some(b.iter().map(|x| (x.von, x.bis)).collect()),
            Self::NichtMachbar(_) => None,
        }
    }
}

/// Der gleiche Schnitt von `ebenen` auf `k` Positionen, Rest nach hinten.
pub fn gleich_geschnitten(ebenen: u32, k: usize) -> Vec<(u32, u32)> {
    let k = k.max(1) as u64;
    (0..k).map(|s| ((ebenen as u64 * s / k) as u32, (ebenen as u64 * (s + 1) / k) as u32)).collect()
}

/// Ein Mitglied, wie seine Zusage es beschreibt.
pub fn mitglied_aus(kennung: MinerId, z: &myl_types::zusage::Modellzusage) -> Mitglied {
    Mitglied { kennung, kapazitaet: z.ebenen_hoechstens, ebenenzeit_us: z.ebenenzeit_us, gehalten: z.gehalten }
}

/// **Der Schnitt eines Pods der Kette** (P5, 2026-10-21; seit dem
/// 2026-10-22 ohne gleichen Rückfall): aus den geltenden Zusagen seiner
/// Mitglieder für das Podmodell, in der Folge seiner Positionen.
///
/// - ⚑ **Kein Pod ohne Zusage** (Festlegung des Projektinhabers vom
///   2026-10-22): Die Pod-Bildung nimmt nur Miner mit Zusage auf. Fehlt
///   einer Position doch eine, ist der Pod nicht machbar; bis heute
///   schnitt er dann gleich, und wer keine Zusage einreichte, umging so
///   die Prüfung seiner Kapazität.
/// - Gehaltene Bereiche (Teilablage, P10) schränken die Verteilung ein.
/// - ⚑ Festlegung E4: Mindestens ein Mitglied der Reserve muss jeden
///   Bereich übernehmen können, sonst ist der Pod nicht machbar.
pub fn bereiche_des_pods(
    positionen: &[MinerId],
    reserve: &[MinerId],
    zusage: &dyn Fn(&MinerId) -> Option<myl_types::zusage::Modellzusage>,
    podmodell: &myl_types::podmodell::Podmodell,
) -> Podschnitt {
    let als_mitglied = |m: &MinerId| zusage(m).map(|z| mitglied_aus(*m, &z));
    let Some(mitglieder) = positionen.iter().map(als_mitglied).collect::<Option<Vec<Mitglied>>>() else {
        return Podschnitt::NichtMachbar("eine Position ohne Zusage".into());
    };
    let bereiche = match verteilen_geordnet(&mitglieder, podmodell.ebenen, podmodell.kopf_platz, podmodell.kopf_zeit) {
        Ok(b) => b,
        Err(f) => return Podschnitt::NichtMachbar(format!("{f:?}")),
    };
    if !reserve.is_empty()
        && !reserve.iter().filter_map(als_mitglied).any(|r| reserve_traegt(&r, &bereiche, podmodell.kopf_platz))
    {
        return Podschnitt::NichtMachbar("keine Reserve traegt jeden Bereich".into());
    }
    Podschnitt::Verteilt(bereiche)
}

/// **Kann ein Reservemitglied jeden Bereich übernehmen?** (Festlegung E4 vom
/// 2026-10-21): seine Kapazität trägt jeden Bereich, den mit Kopf samt
/// Kopf (`kopf_platz`), und sein gehaltener Teil enthält ihn (P10).
pub fn reserve_traegt(reserve: &Mitglied, bereiche: &[Bereich], kopf_platz: u32) -> bool {
    bereiche.iter().all(|b| {
        let platz = b.ebenen() + if b.mit_kopf { kopf_platz } else { 0 };
        reserve.kapazitaet >= platz && reserve.haelt(b.von, b.bis)
    })
}

/// **Das schwächste Mitglied**, das weicht, wenn ein Pod nicht machbar ist
/// (6.7, 2026-10-22): die kleinste wirksame Kapazität (Kapazität, bei
/// Teilablage höchstens die Breite des gehaltenen Teils), bei Gleichstand
/// das langsamere, dann die größere Kennung. Ohne Zusage zählt es als
/// schwächstes überhaupt.
pub fn schwaechstes(
    mitglieder: &[MinerId],
    zusage: &dyn Fn(&MinerId) -> Option<myl_types::zusage::Modellzusage>,
) -> Option<MinerId> {
    mitglieder
        .iter()
        .min_by_key(|m| match zusage(m) {
            None => (0u32, std::cmp::Reverse(u64::MAX), std::cmp::Reverse(**m)),
            Some(z) => {
                let breite = z.gehalten.map_or(u32::MAX, |(v, b)| b.saturating_sub(v));
                (z.ebenen_hoechstens.min(breite), std::cmp::Reverse(z.ebenenzeit_us), std::cmp::Reverse(**m))
            }
        })
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(n: u8, kapazitaet: u32, ebenenzeit_us: u64) -> Mitglied {
        Mitglied { kennung: MinerId::new([n; 32]), kapazitaet, ebenenzeit_us, gehalten: None }
    }

    fn pruefe(b: &[Bereich], mitglieder: &[Mitglied], ebenen: u32, kopf: u32) {
        assert_eq!(b.len(), mitglieder.len());
        assert_eq!(b.first().map(|x| x.von), Some(0));
        assert_eq!(b.last().map(|x| x.bis), Some(ebenen));
        assert!(b.last().unwrap().mit_kopf && b.iter().filter(|x| x.mit_kopf).count() == 1);
        for w in b.windows(2) {
            assert_eq!(w[0].bis, w[1].von, "lueckenlos");
        }
        for x in b {
            let mi = mitglieder.iter().find(|q| q.kennung == x.kennung).unwrap();
            assert!(x.ebenen() >= 1);
            assert!(x.ebenen() + if x.mit_kopf { kopf } else { 0 } <= mi.kapazitaet, "{x:?} {mi:?}");
        }
    }

    /// ⚑ **Gleiche Mitglieder schneiden fast gleich**, wie heute, und der
    /// Kopf geht an das Mitglied mit der kleinsten Kennung (Gleichstand).
    #[test]
    fn gleiche_mitglieder_schneiden_gleich() {
        let ms = [m(1, 40, 900), m(2, 40, 900), m(3, 40, 900), m(4, 40, 900)];
        let b = verteilen(&ms, 40, 0, 0, &[7; 32]).unwrap();
        pruefe(&b, &ms, 40, 0);
        assert!(b.iter().all(|x| x.ebenen() == 10), "{b:?}");
        assert_eq!(b.last().unwrap().kennung, MinerId::new([1; 32]));
    }

    /// ⚑ **Ein doppelt so schnelles Mitglied bekommt etwa doppelt so viele
    /// Ebenen**, und die Engpasszeit sinkt gegen den gleichen Schnitt.
    #[test]
    fn schnelle_mitglieder_tragen_mehr() {
        let ms = [m(1, 40, 500), m(2, 40, 1000), m(3, 40, 1000), m(4, 40, 1000)];
        let b = verteilen(&ms, 40, 0, 0, &[1; 32]).unwrap();
        pruefe(&b, &ms, 40, 0);
        let schnell = b.iter().find(|x| x.kennung == MinerId::new([1; 32])).unwrap().ebenen();
        assert!((15..=17).contains(&schnell), "{b:?}");
        let gleich: Vec<Bereich> = (0..4)
            .map(|i| Bereich { kennung: ms[i].kennung, von: i as u32 * 10, bis: i as u32 * 10 + 10, mit_kopf: i == 3 })
            .collect();
        let vorher = engpass_us(&gleich, &ms, 0).unwrap();
        let nachher = engpass_us(&b, &ms, 0).unwrap();
        assert!(nachher < vorher, "{nachher} gegen {vorher}");
    }

    /// Die Kapazität begrenzt: Ein schnelles, aber kleines Mitglied bekommt
    /// nur, was es hält; der Kopf zählt mit.
    #[test]
    fn kapazitaet_und_kopf_begrenzen() {
        let ms = [m(1, 6, 100), m(2, 30, 1000), m(3, 30, 1000)];
        let b = verteilen(&ms, 40, 4, 4, &[2; 32]).unwrap();
        pruefe(&b, &ms, 40, 4);
        let kopf = b.last().unwrap();
        assert_eq!(kopf.kennung, MinerId::new([1; 32]), "das schnellste traegt den Kopf");
        assert_eq!(kopf.ebenen(), 2, "6 Plaetze, 4 fuer den Kopf");
    }

    /// ⚑ **Platz und Zeit des Kopfs zählen getrennt** (gemessen am Raven:
    /// Platz 4, Zeit 1). Mit kleiner Kopfzeit trägt das Mitglied mit Kopf
    /// mehr Ebenen als mit derselben Zahl für beides, und die Engpasszeit
    /// rechnet mit der Zeit.
    #[test]
    fn platz_und_zeit_des_kopfs_getrennt() {
        let ms = [m(1, 44, 900), m(2, 44, 900), m(3, 44, 900), m(4, 44, 900)];
        let getrennt = verteilen(&ms, 40, 4, 1, &[3; 32]).unwrap();
        let gleich = verteilen(&ms, 40, 4, 4, &[3; 32]).unwrap();
        pruefe(&getrennt, &ms, 40, 4);
        assert!(getrennt.last().unwrap().ebenen() > gleich.last().unwrap().ebenen(), "{getrennt:?} {gleich:?}");
        assert!(engpass_us(&getrennt, &ms, 1).unwrap() <= engpass_us(&gleich, &ms, 1).unwrap());
    }

    #[test]
    fn nicht_machbar_mit_grund() {
        assert_eq!(verteilen(&[], 40, 0, 0, &[0; 32]), Err(Verteilfehler::Leer));
        assert_eq!(
            verteilen(&[m(1, 10, 1), m(2, 10, 1)], 40, 0, 0, &[0; 32]),
            Err(Verteilfehler::ZuWenigKapazitaet { summe: 20, ebenen: 40 })
        );
        assert_eq!(verteilen(&[m(1, 4, 1), m(2, 40, 1)], 40, 40, 40, &[0; 32]), Err(Verteilfehler::KopfPasstNicht { kopf: 40 }));
        assert_eq!(verteilen(&[m(1, 0, 1)], 40, 0, 0, &[0; 32]), Err(Verteilfehler::UngueltigeZusage(MinerId::new([1; 32]))));
        assert!(matches!(verteilen(&[m(1, 9, 1), m(2, 9, 1), m(3, 9, 1)], 2, 0, 0, &[0; 32]), Err(Verteilfehler::MehrMitgliederAlsEbenen { .. })));
    }

    /// ⚑ **Dieselbe Eingabe in anderer Reihenfolge ergibt denselben
    /// Schnitt**, denn jeder Knoten liest die Zusagen in seiner Folge; und
    /// eine andere Saat mischt nur die Reihenfolge, nicht die Ebenenzahlen.
    #[test]
    fn reihenfolge_der_eingabe_aendert_nichts() {
        let ms = [m(5, 20, 700), m(2, 15, 400), m(9, 30, 1200), m(3, 12, 400)];
        let a = verteilen(&ms, 40, 3, 3, &[4; 32]).unwrap();
        let mut umgedreht = ms;
        umgedreht.reverse();
        assert_eq!(verteilen(&umgedreht, 40, 3, 3, &[4; 32]).unwrap(), a);
        let c = verteilen(&ms, 40, 3, 3, &[5; 32]).unwrap();
        let zahl = |b: &[Bereich], k: u8| b.iter().find(|x| x.kennung == MinerId::new([k; 32])).unwrap().ebenen();
        for k in [5, 2, 9, 3] {
            assert_eq!(zahl(&a, k), zahl(&c, k));
        }
    }

    /// ⚑ **Eigenschaften über viele Zufallspods**: lückenlos, in der
    /// Kapazität, Summe gleich, und ein schnelleres Mitglied mit gleicher
    /// Kapazität bekommt nie weniger Ebenen. Der Keim steht im Fehlertext.
    #[test]
    fn eigenschaften_ueber_zufallspods() {
        let mut z: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut naechste = |n: u64| {
            z ^= z << 13;
            z ^= z >> 7;
            z ^= z << 17;
            z % n
        };
        let mut machbar = 0;
        for keim in 0..2000u64 {
            let k = 1 + naechste(6) as usize;
            let ebenen = (k as u32).max(1) + naechste(60) as u32;
            let kopf = naechste(5) as u32;
            let ms: Vec<Mitglied> =
                (0..k).map(|i| m(i as u8 + 1, 1 + naechste(ebenen as u64 + kopf as u64 + 2) as u32, 1 + naechste(3000))).collect();
            let Ok(b) = verteilen(&ms, ebenen, kopf, kopf, &[keim as u8; 32]) else { continue };
            machbar += 1;
            pruefe(&b, &ms, ebenen, kopf);
            for p in &ms {
                for q in &ms {
                    if p.kapazitaet == q.kapazitaet && p.ebenenzeit_us < q.ebenenzeit_us {
                        let e = |x: &Mitglied| b.iter().find(|y| y.kennung == x.kennung).unwrap();
                        let (ep, eq) = (e(p), e(q));
                        if !ep.mit_kopf && !eq.mit_kopf {
                            assert!(ep.ebenen() >= eq.ebenen(), "Keim {keim}: {ms:?} -> {b:?}");
                        }
                    }
                }
            }
        }
        assert!(machbar >= 400, "nur {machbar} von 2000 Pods machbar; die Probe prueft zu wenig");
    }

    fn podmodell() -> myl_types::podmodell::Podmodell {
        myl_types::podmodell::Podmodell { abdruck: [7; 32], ebenen: 40, kopf_platz: 4, kopf_zeit: 1 }
    }

    /// ⚑ **Der Schnitt der Kette**: verteilt, wenn alle Positionen eine
    /// Zusage für das Modell haben und die Reserve trägt; sonst nicht
    /// machbar mit Grund, seit dem 2026-10-22 auch ohne Zusage (kein
    /// gleicher Rückfall mehr). Die Folge der Positionen bleibt.
    #[test]
    fn der_schnitt_eines_pods_der_kette() {
        use myl_types::zusage::Modellzusage;
        let id = |n: u8| MinerId::new([n; 32]);
        let mz = |k: u32, t: u64| Modellzusage { abdruck: [7; 32], ebenen_hoechstens: k, ebenenzeit_us: t, gehalten: None };
        let zusagen = |n: &MinerId| -> Option<Modellzusage> {
            match n.as_bytes()[0] {
                1 => Some(mz(44, 500)),
                2 | 3 => Some(mz(44, 1000)),
                4 => Some(mz(44, 1000)),
                5 => Some(mz(30, 900)),
                6 => Some(mz(5, 900)),
                _ => None,
            }
        };
        let pos = [id(1), id(2), id(3), id(4)];
        let s = bereiche_des_pods(&pos, &[id(5)], &zusagen, &podmodell());
        let Podschnitt::Verteilt(b) = &s else { panic!("{s:?}") };
        assert_eq!(b.iter().map(|x| x.kennung).collect::<Vec<_>>(), pos, "die Folge der Positionen bleibt");
        assert!(b[0].ebenen() > b[1].ebenen(), "das schnelle erste traegt mehr: {b:?}");
        assert_eq!(s.grenzen().unwrap().last().unwrap().1, 40);
        let fremd = [id(1), id(2), id(3), id(9)];
        assert!(matches!(bereiche_des_pods(&fremd, &[], &zusagen, &podmodell()), Podschnitt::NichtMachbar(_)), "ohne Zusage");
        assert!(matches!(bereiche_des_pods(&pos, &[id(6)], &zusagen, &podmodell()), Podschnitt::NichtMachbar(_)), "die Reserve traegt nicht");
        assert_eq!(gleich_geschnitten(40, 4), [(0, 10), (10, 20), (20, 30), (30, 40)]);
        assert_eq!(gleich_geschnitten(28, 3), [(0, 9), (9, 18), (18, 28)]);
    }

    /// ⚑ **Teilablage** (P10): Ein Mitglied bekommt nur Ebenen in seinem
    /// gehaltenen Teil; deckt die Folge nicht alles, ist der Pod nicht
    /// machbar. Ohne gehaltene Teile bleibt das Wasserfüllen bitgleich.
    #[test]
    fn die_teilablage_schraenkt_die_verteilung_ein() {
        let mut ms = vec![m(1, 40, 500), m(2, 40, 1000), m(3, 40, 1000), m(4, 44, 1000)];
        let frei = verteilen_geordnet(&ms, 40, 4, 1).unwrap();
        // Das schnelle erste hält nur die Ebenen 0 bis 6.
        ms[0].gehalten = Some((0, 6));
        let b = verteilen_geordnet(&ms, 40, 4, 1).unwrap();
        pruefe(&b, &ms, 40, 4);
        assert!(frei[0].ebenen() > 6, "ohne Teilablage mehr als sechs: {frei:?}");
        assert!(b[0].bis <= 6, "nur im gehaltenen Teil: {b:?}");
        for (x, mi) in b.iter().zip(&ms) {
            assert!(mi.haelt(x.von, x.bis), "{x:?} {mi:?}");
        }
        // Das zweite hält 20 bis 40: Die Ebenen 6 bis 20 deckt dann nur noch
        // das erste, und das hält sie nicht.
        ms[1].gehalten = Some((20, 40));
        assert_eq!(verteilen_geordnet(&ms, 40, 4, 1), Err(Verteilfehler::TeilablageDecktNicht));
    }

    /// ⚑ **Teilablage über viele Zufallspods**: Jeder Bereich liegt im
    /// gehaltenen Teil seines Mitglieds, lückenlos, in der Kapazität.
    #[test]
    fn teilablage_ueber_zufallspods() {
        let mut z: u64 = 0x2545_F491_4F6C_DD1D;
        let mut naechste = |n: u64| {
            z ^= z << 13;
            z ^= z >> 7;
            z ^= z << 17;
            z % n
        };
        let mut machbar = 0;
        for keim in 0..2000u64 {
            let k = 1 + naechste(5) as usize;
            let ebenen = k as u32 + naechste(40) as u32;
            let kopf = naechste(4) as u32;
            let ms: Vec<Mitglied> = (0..k)
                .map(|i| {
                    let mut x = m(i as u8 + 1, 1 + naechste(ebenen as u64 + kopf as u64 + 2) as u32, 1 + naechste(3000));
                    if naechste(2) == 0 {
                        let a = naechste(ebenen as u64) as u32;
                        let b = a + 1 + naechste((ebenen - a) as u64) as u32;
                        x.gehalten = Some((a, b));
                    }
                    x
                })
                .collect();
            let Ok(b) = verteilen_geordnet(&ms, ebenen, kopf, kopf) else { continue };
            machbar += 1;
            pruefe(&b, &ms, ebenen, kopf);
            for (x, mi) in b.iter().zip(&ms) {
                assert!(mi.haelt(x.von, x.bis), "Keim {keim}: {x:?} {mi:?}");
            }
        }
        assert!(machbar >= 100, "nur {machbar} machbar; die Probe prueft zu wenig");
    }

    /// ⚑ **Das schwächste Mitglied weicht** (6.7): kleinste wirksame
    /// Kapazität, die Teilablage zählt mit; ohne Zusage zuerst.
    #[test]
    fn das_schwaechste_mitglied() {
        use myl_types::zusage::Modellzusage;
        let id = |n: u8| MinerId::new([n; 32]);
        let zusagen = |n: &MinerId| -> Option<Modellzusage> {
            let b = n.as_bytes()[0];
            (b != 9).then(|| Modellzusage {
                abdruck: [7; 32],
                ebenen_hoechstens: 30 - b as u32,
                ebenenzeit_us: 100,
                gehalten: (b == 1).then_some((0, 3)),
            })
        };
        assert_eq!(schwaechstes(&[id(2), id(3), id(4)], &zusagen), Some(id(4)), "kleinste Kapazitaet");
        assert_eq!(schwaechstes(&[id(1), id(4)], &zusagen), Some(id(1)), "der gehaltene Teil ist schmaler");
        assert_eq!(schwaechstes(&[id(1), id(9)], &zusagen), Some(id(9)), "ohne Zusage zuerst");
    }

    #[test]
    fn die_reserve_muss_jeden_bereich_tragen() {
        let ms = [m(1, 20, 500), m(2, 20, 1000)];
        let b = verteilen(&ms, 30, 2, 2, &[0; 32]).unwrap();
        let groesster = b.iter().map(|x| x.ebenen() + if x.mit_kopf { 2 } else { 0 }).max().unwrap();
        assert!(reserve_traegt(&m(9, groesster, 1), &b, 2));
        assert!(!reserve_traegt(&m(9, groesster - 1, 1), &b, 2));
        // ⚑ Mit Teilablage muss der gehaltene Teil jeden Bereich enthalten.
        let teil = Mitglied { gehalten: Some((0, 29)), ..m(9, 40, 1) };
        assert!(!reserve_traegt(&teil, &b, 2), "die letzte Ebene fehlt ihr");
    }
}
