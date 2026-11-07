//! Schwellen-BLS mit verteilter Schluesselerzeugung: die Saat, die keiner
//! mahlen kann.
//!
//! ⚑ **Der Befund** (Angriffspruefung vom 2026-11-03): Bisher kam die Saat
//! der Epoche aus dem letzten Blockhash, die Saat der Stichprobe aus einem
//! Pruefpunktzertifikat. Beides waehlt jemand: Den Hash waehlt sein
//! Erzeuger, der beliebig viele Fassungen seines Blocks durchrechnen kann,
//! das Zertifikat sein Zusammensteller ueber die Unterzeichnermenge. Bei
//! einer Stichprobenrate von 2 % kommen 300 gefaelschte Segmente mit einer
//! festen Saat zu 0,23 % durch, mit 2¹⁰ durchprobierten zu 91 %.
//!
//! ⚑ **Die Antwort:** Die Saat ist eine BLS-Signatur unter einem Schluessel,
//! den der Stimmsatz gemeinsam haelt und keiner allein kennt. BLS ist
//! eindeutig: **Jede** Menge von `schwelle` gueltigen Anteilen ergibt
//! dieselbe Signatur, wer sie zusammenstellt, waehlt also nichts. Geprueft
//! wird sie wie jede BLS-Signatur ([`crate::bls::BlsPublicKey::verify`]).
//! Stand der Praxis: drand seit 2019, Aptos seit 2024.
//!
//! **Der Ablauf, wie drand:** Jeder Austeiler waehlt ein geheimes Polynom
//! vom Grad `schwelle − 1`, veroeffentlicht Feldman-Bindungen an dessen
//! Koeffizienten ([`Bindung`]) und schickt jedem Sitz seinen Wert
//! verschluesselt ([`crate::anteilchiffre`]). Wer einen falschen Wert
//! bekommt, beschwert sich; der Austeiler muss den Wert dann offenlegen
//! oder faellt heraus. Der Gruppenschluessel ist die Summe der
//! Bindungen an der Null, der Anteil eines Sitzes die Summe seiner Werte.
//! Dass ein Angreifer den Schluessel dabei verzerren kann, schadet einer
//! Schwellensignatur nicht (Gennaro, Jarecki, Krawczyk, Rabin, CT-RSA
//! 2003; fuer Schwellen-BLS Bacho, Loss, CCS 2022).
//!
//! ⛔️ **Der Kenntnisnachweis ist Pflicht** ([`Kenntnisnachweis`]). Ohne
//! ihn koennte ein Angreifer, der die ehrlichen Bindungen zuerst sieht,
//! seine Bindung an der Null als `g^x` minus die Summe der ehrlichen waehlen.
//! Dann ist der Gruppenschluessel `g^x`, und `x` kennt er. Seine Werte
//! fuer die ehrlichen Sitze passten zwar nicht, und die Beschwerden
//! wuerfen ihn hinaus; **zensiert ein fester Erzeuger die Beschwerden
//! aber, kennt der Angreifer jede kuenftige Saat**. Mit dem Nachweis muss
//! er den Logarithmus seiner Bindung kennen, also kann er die ehrliche
//! nicht aufheben, und Zensur kostet nur noch Lebendigkeit. Dieselbe
//! Abwehr wie der Besitznachweis bei der Registrierung (Fund 27).
//!
//! **Sitze:** Gewicht ist der Einsatz, verkleinert auf Sitze
//! ([`crate::sitze`]). Sitznummern laufen ab eins; an der Null liegt das
//! Geheimnis.
//!
//! **Konstante Zeit:** Mit geheimen Zahlen rechnen nur [`Skalar`] und die
//! Signier- und Schluesselfunktionen von `blst`. Die Mehrfachskalierung
//! (`MultiPoint::mult`) laeuft nicht in konstanter Zeit und bekommt deshalb
//! nur oeffentliche Zahlen: Sitznummern und Lagrange-Gewichte.

use blst::min_pk::{AggregatePublicKey, AggregateSignature, PublicKey, SecretKey, Signature};
use blst::{MultiPoint, BLST_ERROR};
use borsh::{BorshDeserialize, BorshSerialize};
use sha2::{Digest, Sha256, Sha512};

use crate::bls::{BlsPublicKey, BlsSignature, BLS_DST, BLS_SIG_LEN};
use crate::skalar::Skalar;

/// Domaene des Kenntnisnachweises: getrennt von Nachrichten- und
/// Besitznachweis-Signaturen, damit keiner als der andere durchgeht.
pub const DST_KENNTNIS: &[u8] = b"MYL_DKG_KENNTNIS_BLS12381G2_XMD:SHA-256_SSWU_RO_";

/// Domaene der Polynomableitung.
const DST_POLYNOM: &[u8] = b"myl-dkg-polynom-v1";

/// Domaene der signierten Saatbotschaft.
const DST_SAATBOTSCHAFT: &[u8] = b"myl-saat-botschaft-v1";

/// Domaene der Saat aus der Signatur.
const DST_SAAT: &[u8] = b"myl-saat-v1";

/// Bits der Skalare fuer die Mehrfachskalierung (`r < 2²⁵⁵`).
const SKALAR_BITS: usize = 255;

/// Feldman-Bindungen an die Koeffizienten eines Polynoms: `C_k = g₁^{a_k}`.
///
/// `punkte[0]` bindet das Geheimnis des Austeilers, die Laenge ist die
/// Schwelle.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Bindung {
    /// Die Punkte, Koeffizient fuer Koeffizient.
    pub punkte: Vec<BlsPublicKey>,
}

/// Nachweis, dass der Austeiler den Logarithmus von `punkte[0]` kennt: eine
/// Signatur mit `a_0` ueber Kontext und Bindung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Kenntnisnachweis(pub [u8; BLS_SIG_LEN]);

/// Ein geheimes Polynom; die Koeffizienten verlassen den Austeiler nie.
pub struct Polynom {
    koeffizienten: Vec<Skalar>,
}

impl std::fmt::Debug for Polynom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Polynom(Grad {})", self.koeffizienten.len().saturating_sub(1))
    }
}

fn geheimschluessel(s: &Skalar) -> Option<SecretKey> {
    SecretKey::from_bytes(&s.als_bytes_be()).ok()
}

fn oeffentlich(s: &Skalar) -> Option<BlsPublicKey> {
    Some(BlsPublicKey(geheimschluessel(s)?.sk_to_pk().compress()))
}

/// Dekodiert und prueft einen Punkt: gueltig, in der Untergruppe, nicht die
/// Identitaet.
fn punkt(p: &BlsPublicKey) -> Option<PublicKey> {
    let pk = PublicKey::from_bytes(&p.0).ok()?;
    pk.validate().ok()?;
    Some(pk)
}

impl Polynom {
    /// Leitet das Polynom deterministisch aus einem Geheimnis und dem
    /// Kontext ab (wie RFC 6979 die Nonce): kein Zufallsgenerator, und
    /// derselbe Kontext ergibt nach einem Neustart dasselbe Polynom, also
    /// keine zweite, widerspruechliche Austeilung. `schwelle` Koeffizienten,
    /// keiner null. `None` nur fuer `schwelle == 0`.
    pub fn ableiten(geheimnis: &[u8; 32], kontext: &[u8], schwelle: u64) -> Option<Polynom> {
        if schwelle == 0 {
            return None;
        }
        let mut koeffizienten = Vec::with_capacity(schwelle as usize);
        let mut zaehler: u64 = 0;
        while (koeffizienten.len() as u64) < schwelle {
            let mut h = Sha512::new();
            h.update(DST_POLYNOM);
            h.update((kontext.len() as u64).to_le_bytes());
            h.update(kontext);
            h.update(geheimnis);
            h.update(zaehler.to_le_bytes());
            zaehler += 1;
            let b: [u8; 64] = h.finalize().into();
            let a = Skalar::aus_breit(&b);
            // Null kommt praktisch nie vor; eine Null-Bindung waere aber die
            // Identitaet und wuerde abgewiesen.
            if !a.ist_null() {
                koeffizienten.push(a);
            }
        }
        Some(Polynom { koeffizienten })
    }

    /// Die Schwelle, fuer die das Polynom gebaut ist.
    pub fn schwelle(&self) -> u64 {
        self.koeffizienten.len() as u64
    }

    /// Die Feldman-Bindung.
    pub fn bindung(&self) -> Bindung {
        Bindung {
            punkte: self.koeffizienten.iter().map(|a| oeffentlich(a).expect("Koeffizient ungleich null")).collect(),
        }
    }

    /// Der Kenntnisnachweis zu [`Polynom::bindung`] in diesem Kontext.
    pub fn kenntnisnachweis(&self, kontext: &[u8]) -> Kenntnisnachweis {
        let botschaft = kenntnisbotschaft(kontext, &self.bindung());
        let sk = geheimschluessel(&self.koeffizienten[0]).expect("Koeffizient ungleich null");
        Kenntnisnachweis(sk.sign(&botschaft, DST_KENNTNIS, &[]).compress())
    }

    /// Der Wert an einer Sitznummer (Horner).
    pub fn anteil(&self, nummer: u64) -> Skalar {
        let x = Skalar::aus_u64(nummer);
        let mut erg = Skalar::NULL;
        for a in self.koeffizienten.iter().rev() {
            erg = erg.mal(&x).plus(a);
        }
        erg
    }
}

fn kenntnisbotschaft(kontext: &[u8], bindung: &Bindung) -> Vec<u8> {
    let mut b = Vec::with_capacity(8 + kontext.len() + 48 * bindung.punkte.len());
    b.extend_from_slice(&(kontext.len() as u64).to_le_bytes());
    b.extend_from_slice(kontext);
    for p in &bindung.punkte {
        b.extend_from_slice(&p.0);
    }
    b
}

/// Prueft eine fremde Bindung: richtige Laenge, jeder Punkt gueltig und
/// nicht die Identitaet, und der Kenntnisnachweis zu `punkte[0]` stimmt.
pub fn bindung_pruefen(bindung: &Bindung, schwelle: u64, kontext: &[u8], nachweis: &Kenntnisnachweis) -> bool {
    if schwelle == 0 || bindung.punkte.len() as u64 != schwelle {
        return false;
    }
    if !bindung.punkte.iter().all(|p| punkt(p).is_some()) {
        return false;
    }
    let Some(null) = punkt(&bindung.punkte[0]) else { return false };
    let Ok(sig) = Signature::from_bytes(&nachweis.0) else { return false };
    sig.verify(true, &kenntnisbotschaft(kontext, bindung), DST_KENNTNIS, &[], &null, true) == BLST_ERROR::BLST_SUCCESS
}

/// Der oeffentliche Wert einer Bindung an einer Sitznummer,
/// `Σ C_k · nummer^k`. `None`, wenn ein Punkt ungueltig ist oder das
/// Ergebnis die Identitaet waere.
pub fn oeffentlicher_anteil(bindung: &Bindung, nummer: u64) -> Option<BlsPublicKey> {
    let punkte: Vec<PublicKey> = bindung.punkte.iter().map(punkt).collect::<Option<_>>()?;
    let x = Skalar::aus_u64(nummer);
    let mut potenz = Skalar::EINS;
    let mut skalare = Vec::with_capacity(32 * punkte.len());
    for _ in 0..punkte.len() {
        skalare.extend_from_slice(&potenz.als_bytes_le());
        potenz = potenz.mal(&x);
    }
    let summe = punkte.as_slice().mult(&skalare, SKALAR_BITS).to_public_key();
    summe.validate().ok()?;
    Some(BlsPublicKey(summe.compress()))
}

/// Ob ein entschluesselter Wert zur Bindung passt.
pub fn anteil_pruefen(bindung: &Bindung, nummer: u64, anteil: &Skalar) -> bool {
    match (oeffentlicher_anteil(bindung, nummer), oeffentlich(anteil)) {
        (Some(soll), Some(ist)) => soll == ist,
        _ => false,
    }
}

/// Summiert die Bindungen der qualifizierten Austeiler Punkt fuer Punkt.
/// Das Ergebnis bindet das gemeinsame Polynom; `punkte[0]` ist der
/// Gruppenschluessel. `None` bei verschiedenen Laengen, leerer Eingabe oder
/// einem ungueltigen Punkt.
pub fn bindungen_summieren(bindungen: &[Bindung]) -> Option<Bindung> {
    let laenge = bindungen.first()?.punkte.len();
    if laenge == 0 || bindungen.iter().any(|b| b.punkte.len() != laenge) {
        return None;
    }
    let mut punkte = Vec::with_capacity(laenge);
    for k in 0..laenge {
        let einzeln: Vec<PublicKey> = bindungen.iter().map(|b| punkt(&b.punkte[k])).collect::<Option<_>>()?;
        let refs: Vec<&PublicKey> = einzeln.iter().collect();
        let summe = AggregatePublicKey::aggregate(&refs, false).ok()?.to_public_key();
        summe.validate().ok()?;
        punkte.push(BlsPublicKey(summe.compress()));
    }
    Some(Bindung { punkte })
}

/// Der Gruppenschluessel einer (summierten) Bindung.
pub fn gruppenschluessel(bindung: &Bindung) -> Option<BlsPublicKey> {
    bindung.punkte.first().copied()
}

/// Der Anteil eines Sitzes: die Summe der Werte aller qualifizierten
/// Austeiler.
pub fn anteile_summieren(werte: &[Skalar]) -> Skalar {
    werte.iter().fold(Skalar::NULL, |s, w| s.plus(w))
}

/// Die Botschaft, die der Satz fuer die Saat einer Epoche signiert.
pub fn saatbotschaft(epoche: u64) -> Vec<u8> {
    let mut b = Vec::with_capacity(DST_SAATBOTSCHAFT.len() + 8);
    b.extend_from_slice(DST_SAATBOTSCHAFT);
    b.extend_from_slice(&epoche.to_le_bytes());
    b
}

/// Signiert eine Botschaft mit dem Anteil eines Sitzes. Geprueft wird das
/// Ergebnis mit `BlsPublicKey::verify` gegen den oeffentlichen Anteil des
/// Sitzes.
pub fn anteil_signieren(anteil: &Skalar, botschaft: &[u8]) -> Option<BlsSignature> {
    let sk = geheimschluessel(anteil)?;
    Some(BlsSignature(sk.sign(botschaft, BLS_DST, &[]).compress()))
}

/// Die Lagrange-Gewichte an der Null fuer verschiedene Sitznummern ungleich
/// null. `None` bei einer Null oder einer doppelten Nummer.
pub fn lagrange_bei_null(nummern: &[u64]) -> Option<Vec<Skalar>> {
    let mut gewichte = Vec::with_capacity(nummern.len());
    for (i, &xi) in nummern.iter().enumerate() {
        if xi == 0 {
            return None;
        }
        let (mut zaehler, mut nenner) = (Skalar::EINS, Skalar::EINS);
        for (j, &xj) in nummern.iter().enumerate() {
            if i == j {
                continue;
            }
            if xi == xj {
                return None;
            }
            let sj = Skalar::aus_u64(xj);
            zaehler = zaehler.mal(&sj);
            nenner = nenner.mal(&sj.minus(&Skalar::aus_u64(xi)));
        }
        gewichte.push(zaehler.mal(&nenner.invers()?));
    }
    Some(gewichte)
}

/// Setzt gepruefte Signaturanteile zur Schwellensignatur zusammen.
///
/// Genommen werden die `schwelle` kleinsten verschiedenen Sitznummern; das
/// Ergebnis haengt davon nicht ab (Eindeutigkeit), aber so rechnet jeder
/// Knoten dasselbe. **Die Anteile muessen vorher einzeln gegen ihre
/// oeffentlichen Anteile geprueft sein**; ein falscher verdirbt die Summe,
/// und das faellt erst bei der Pruefung gegen den Gruppenschluessel auf.
pub fn zusammensetzen(teile: &[(u64, BlsSignature)], schwelle: u64) -> Option<BlsSignature> {
    let mut sortiert: Vec<(u64, BlsSignature)> = teile.to_vec();
    sortiert.sort_by_key(|(n, _)| *n);
    sortiert.dedup_by_key(|(n, _)| *n);
    if schwelle == 0 || (sortiert.len() as u64) < schwelle {
        return None;
    }
    sortiert.truncate(schwelle as usize);
    let nummern: Vec<u64> = sortiert.iter().map(|(n, _)| *n).collect();
    let gewichte = lagrange_bei_null(&nummern)?;
    let signaturen: Vec<Signature> =
        sortiert.iter().map(|(_, s)| Signature::from_bytes(&s.0).ok()).collect::<Option<_>>()?;
    let mut skalare = Vec::with_capacity(32 * gewichte.len());
    for g in &gewichte {
        skalare.extend_from_slice(&g.als_bytes_le());
    }
    let summe: AggregateSignature = signaturen.as_slice().mult(&skalare, SKALAR_BITS);
    Some(BlsSignature(summe.to_signature().compress()))
}

/// Die Saat aus einer geprueften Schwellensignatur.
pub fn saat_aus_signatur(signatur: &BlsSignature) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(DST_SAAT);
    h.update(signatur.0);
    h.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sitze;

    fn geheimnis(i: u8) -> [u8; 32] {
        [i; 32]
    }

    /// Eine vollstaendige Austeilung ohne Betrug: drei Austeiler, Sitze aus
    /// Einsatz.
    struct Lauf {
        vergabe: sitze::Sitzvergabe,
        summe: Bindung,
        anteile: Vec<Skalar>,
    }

    fn lauf(einsaetze: &[u64], austeiler: &[u8]) -> Lauf {
        let vergabe = sitze::vergeben(einsaetze).expect("Vergabe");
        let t = vergabe.schwelle;
        let polynome: Vec<Polynom> =
            austeiler.iter().map(|a| Polynom::ableiten(&geheimnis(*a), b"kontext", t).expect("Polynom")).collect();
        for (a, p) in austeiler.iter().zip(&polynome) {
            let k = [b"kontext".as_slice(), &[*a]].concat();
            assert!(bindung_pruefen(&p.bindung(), t, &k, &p.kenntnisnachweis(&k)));
        }
        let summe = bindungen_summieren(&polynome.iter().map(|p| p.bindung()).collect::<Vec<_>>()).expect("Summe");
        let anteile: Vec<Skalar> = (1..=vergabe.gesamt())
            .map(|n| {
                let werte: Vec<Skalar> = polynome.iter().map(|p| p.anteil(n)).collect();
                for (p, w) in polynome.iter().zip(&werte) {
                    assert!(anteil_pruefen(&p.bindung(), n, w));
                }
                anteile_summieren(&werte)
            })
            .collect();
        Lauf { vergabe, summe, anteile }
    }

    /// ⚑ Der Kern: jede Teilmenge ueber der Schwelle ergibt dieselben Bytes,
    /// und die bestehen die gewoehnliche BLS-Pruefung.
    #[test]
    fn jede_teilmenge_ergibt_dieselbe_saat() {
        let l = lauf(&[40, 30, 20, 10, 10], &[1, 2, 3]);
        let gruppe = gruppenschluessel(&l.summe).expect("Gruppenschluessel");
        let botschaft = saatbotschaft(7);
        let teile: Vec<(u64, BlsSignature)> = l
            .anteile
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let n = i as u64 + 1;
                let s = anteil_signieren(a, &botschaft).expect("Signatur");
                let oa = oeffentlicher_anteil(&l.summe, n).expect("oeffentlicher Anteil");
                assert!(oa.verify(&botschaft, &s), "Anteil {n}");
                (n, s)
            })
            .collect();
        let t = l.vergabe.schwelle as usize;
        let mut ergebnisse = Vec::new();
        // Vorne, hinten, verstreut, und alle.
        for auswahl in [teile[..t].to_vec(), teile[teile.len() - t..].to_vec(), teile.iter().step_by(2).cloned().collect(), teile.clone()] {
            if auswahl.len() < t {
                continue;
            }
            let sig = zusammensetzen(&auswahl, t as u64).expect("zusammengesetzt");
            assert!(gruppe.verify(&botschaft, &sig));
            ergebnisse.push(sig);
        }
        assert!(ergebnisse.len() >= 3);
        assert!(ergebnisse.windows(2).all(|w| w[0] == w[1]));
        // Eine andere Epoche, eine andere Saat.
        let andere = zusammensetzen(
            &l.anteile.iter().enumerate().map(|(i, a)| (i as u64 + 1, anteil_signieren(a, &saatbotschaft(8)).expect("Signatur"))).collect::<Vec<_>>(),
            t as u64,
        )
        .expect("zusammengesetzt");
        assert_ne!(saat_aus_signatur(&ergebnisse[0]), saat_aus_signatur(&andere));
    }

    #[test]
    fn unter_der_schwelle_gibt_es_nichts() {
        let l = lauf(&[1, 1, 1, 1], &[1, 2]);
        let t = l.vergabe.schwelle;
        let botschaft = saatbotschaft(1);
        let teile: Vec<(u64, BlsSignature)> =
            (0..(t - 1) as usize).map(|i| (i as u64 + 1, anteil_signieren(&l.anteile[i], &botschaft).expect("Signatur"))).collect();
        assert!(zusammensetzen(&teile, t).is_none());
        // Doppelte Nummern zaehlen einmal.
        let doppelt = vec![teile[0], teile[0]];
        assert!(zusammensetzen(&doppelt, 2).is_none());
    }

    /// Ein falscher Wert faellt gegen die Bindung durch.
    #[test]
    fn ein_falscher_wert_faellt_auf() {
        let p = Polynom::ableiten(&geheimnis(9), b"k", 3).expect("Polynom");
        let b = p.bindung();
        assert!(anteil_pruefen(&b, 4, &p.anteil(4)));
        assert!(!anteil_pruefen(&b, 4, &p.anteil(5)));
        assert!(!anteil_pruefen(&b, 4, &p.anteil(4).plus(&Skalar::EINS)));
    }

    /// ⛔️ Der Kenntnisnachweis gilt nur fuer seine Bindung und seinen Kontext,
    /// und eine Bindung, die eine fremde aufheben soll, hat keinen.
    #[test]
    fn ohne_kenntnis_keine_bindung() {
        let ehrlich = Polynom::ableiten(&geheimnis(1), b"k", 2).expect("Polynom");
        let eigen = Polynom::ableiten(&geheimnis(2), b"k", 2).expect("Polynom");
        let (be, ne) = (ehrlich.bindung(), ehrlich.kenntnisnachweis(b"k"));
        assert!(bindung_pruefen(&be, 2, b"k", &ne));
        assert!(!bindung_pruefen(&be, 2, b"anderer", &ne), "Kontext gebunden");
        assert!(!bindung_pruefen(&be, 3, b"k", &ne), "Laenge gebunden");
        // Der Aufhebe-Versuch: C0' = g^x − C0_ehrlich. Den Nachweis kann der
        // Angreifer nur mit x signieren, und x ist nicht der Logarithmus.
        let x = Skalar::aus_u64(12345);
        let minus_ehrlich = {
            let pk = PublicKey::from_bytes(&be.punkte[0].0).expect("Punkt");
            [pk].as_slice().mult(&Skalar::EINS.negiert().als_bytes_le(), SKALAR_BITS).to_public_key()
        };
        let gx = SecretKey::from_bytes(&x.als_bytes_be()).expect("Schluessel").sk_to_pk();
        let c0 = AggregatePublicKey::aggregate(&[&gx, &minus_ehrlich], true).expect("Summe").to_public_key();
        let boese = Bindung { punkte: vec![BlsPublicKey(c0.compress()), eigen.bindung().punkte[1]] };
        let mit_x = Kenntnisnachweis(
            SecretKey::from_bytes(&x.als_bytes_be())
                .expect("Schluessel")
                .sign(&kenntnisbotschaft(b"k", &boese), DST_KENNTNIS, &[])
                .compress(),
        );
        assert!(!bindung_pruefen(&boese, 2, b"k", &mit_x));
        // Und die Summe waere tatsaechlich g^x gewesen: Der Test prueft den
        // Angriff, nicht eine Attrappe.
        let summe = bindungen_summieren(&[be, boese]).expect("Summe");
        assert_eq!(summe.punkte[0].0, gx.compress());
    }

    #[test]
    fn identitaet_und_muell_werden_abgewiesen() {
        let p = Polynom::ableiten(&geheimnis(3), b"k", 2).expect("Polynom");
        let n = p.kenntnisnachweis(b"k");
        let mut identitaet = p.bindung();
        identitaet.punkte[1] = BlsPublicKey({
            let mut b = [0u8; 48];
            b[0] = 0xc0;
            b
        });
        assert!(!bindung_pruefen(&identitaet, 2, b"k", &n));
        let mut muell = p.bindung();
        muell.punkte[1] = BlsPublicKey([7; 48]);
        assert!(!bindung_pruefen(&muell, 2, b"k", &n));
        assert!(oeffentlicher_anteil(&muell, 1).is_none());
        assert!(bindungen_summieren(&[]).is_none());
        assert!(bindungen_summieren(&[p.bindung(), Polynom::ableiten(&geheimnis(4), b"k", 3).expect("Polynom").bindung()]).is_none());
    }

    #[test]
    fn das_polynom_ist_deterministisch_und_kontextgebunden() {
        let a = Polynom::ableiten(&geheimnis(5), b"e7", 3).expect("Polynom");
        let b = Polynom::ableiten(&geheimnis(5), b"e7", 3).expect("Polynom");
        let c = Polynom::ableiten(&geheimnis(5), b"e8", 3).expect("Polynom");
        assert_eq!(a.bindung(), b.bindung());
        assert_ne!(a.bindung(), c.bindung());
        assert!(Polynom::ableiten(&geheimnis(5), b"e7", 0).is_none());
        assert_eq!(format!("{a:?}"), "Polynom(Grad 2)");
    }

    #[test]
    fn lagrange_rekonstruiert_das_geheimnis() {
        let p = Polynom::ableiten(&geheimnis(6), b"k", 4).expect("Polynom");
        let nummern = [2u64, 5, 9, 11];
        let gewichte = lagrange_bei_null(&nummern).expect("Gewichte");
        let wert = nummern.iter().zip(&gewichte).fold(Skalar::NULL, |s, (n, g)| s.plus(&p.anteil(*n).mal(g)));
        assert_eq!(wert, p.anteil(0));
        assert!(lagrange_bei_null(&[1, 0]).is_none());
        assert!(lagrange_bei_null(&[3, 3]).is_none());
    }
}
