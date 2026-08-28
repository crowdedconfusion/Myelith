//! Die Naht zwischen Session-Kontrakt und Gateway-Beobachtung.
//!
//! Design-Entscheidung 1 ließ eine Zahl offen: **wie viele unabhängige
//! Gateways ein externes Werkzeugergebnis bezeugt haben müssen.** Die
//! Antwort war, dass sie nicht ins Protokoll gehört, sondern in den
//! Kontrakt, gekoppelt an den Betrag. [`Sitzungskontrakt`] trägt sie
//! seither als Leiter; hier wird sie benutzt.
//!
//! ⚑ **Und der Agent kann sie nicht senken.** Eine kleinere Zahl ist
//! ein anderer Kontrakt, ein anderer Kontrakt hat eine andere Adresse,
//! und die Session läuft unter der alten. Das ist derselbe Satz wie in
//! [`myl_types::sitzung`], nur eine Ebene weiter oben.
//!
//! ## Was die Zeitspanne hier tut, und was nicht
//!
//! Sie wird **durchgereicht und nicht ausgewertet**. Einigkeit über 200
//! Millisekunden bedeutet etwas anderes als Einigkeit über 30 Sekunden,
//! und wer das beurteilen kann, ist der Mensch oder der Agent, nicht
//! diese Funktion. Eine Frist hier hineinzuschreiben hieße, die
//! Entscheidung zu treffen, die Design 1 ausdrücklich nicht getroffen
//! hat.

use myl_types::hash::Hash;
use myl_types::sitzung::{Sitzungskontrakt, Waehrung};

use crate::beobachtung::{beobachte, Beobachtung, BeobachtungsFehler, GepruefteAttestierung};

/// Ob ein externes Ergebnis unter diesem Kontrakt benutzt werden darf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verwendbar {
    /// Genug Zeugen, und sie sagen dasselbe.
    Ja {
        /// Der bezeugte Inhalt.
        inhalt: Hash,
        /// Wie viele bezeugt haben.
        zeugen: usize,
        /// Über welche Zeitspanne. **Weitergereicht, nicht bewertet.**
        spanne_ms: u64,
    },
    /// Die Zeugen sagen Verschiedenes.
    ///
    /// ⚑ **Kein Mehrheitsentscheid, auch hier nicht.** Bei volatilen
    /// Daten ist die Mehrheit bedeutungslos, und bei zwei gegen eins
    /// kann die Minderheit die ehrliche sein.
    Uneinig {
        /// Wie viele verschiedene Antworten.
        varianten: usize,
        /// Über welche Zeitspanne.
        spanne_ms: u64,
    },
    /// Weniger Zeugen, als der Kontrakt für diesen Betrag verlangt.
    ZuWenigZeugen {
        /// Wie viele es waren.
        hatte: usize,
        /// Wie viele der Kontrakt verlangt.
        verlangt: usize,
    },
    /// Der Kontrakt verlangt für diesen Betrag keine Bezeugung.
    ///
    /// ⚑ **Das ist erlaubt und trotzdem keine Zusicherung.** Es wurde
    /// nichts geprüft, weil nichts verlangt war; das Ergebnis ist
    /// benutzbar, das Segment darum aber nicht nachrechenbar. Was das
    /// für die Verifikationsstufe heißt, sagt
    /// [`crate::Registratur::stufe`], nicht diese Funktion.
    OhneBezeugung,
}

impl Verwendbar {
    /// Darf es benutzt werden?
    pub fn ja(&self) -> bool {
        matches!(self, Self::Ja { .. } | Self::OhneBezeugung)
    }
}

/// Wie viele Zeugen dieser Kontrakt für diesen Betrag verlangt.
///
/// Reicht [`Sitzungskontrakt::zeugen_fuer`] durch und macht daraus die
/// `usize`, die [`beobachte`] erwartet.
pub fn verlangte_zeugen(kontrakt: &Sitzungskontrakt, waehrung: Waehrung, betrag: u64) -> usize {
    kontrakt.zeugen_fuer(waehrung, betrag) as usize
}

/// Entscheidet, ob ein externes Werkzeugergebnis unter diesem Kontrakt
/// für diesen Betrag benutzt werden darf.
///
/// **Die Zahl kommt aus dem Kontrakt**, die Aussagen von den Gateways,
/// und zusammengefasst werden sie von [`beobachte`], das seinerseits
/// nichts auflöst. Diese Funktion fügt genau eine Entscheidung hinzu,
/// und zwar die, die der Inhaber vorab getroffen hat: **genug Zeugen
/// und einig, sonst nicht.**
///
/// ⚑ **Verlangt der Kontrakt null Zeugen, wird gar nicht erst
/// beobachtet.** Aus einer leeren Liste folgt weder Einigkeit noch
/// Uneinigkeit, und [`beobachte`] weist sie deshalb ab. Wer keine
/// Bezeugung verlangt hat, bekommt hier [`Verwendbar::OhneBezeugung`]
/// und keine erfundene Übereinstimmung.
pub fn darf_verwendet_werden(
    kontrakt: &Sitzungskontrakt,
    waehrung: Waehrung,
    betrag: u64,
    anfrage: &Hash,
    attestierungen: &[GepruefteAttestierung],
) -> Result<Verwendbar, BeobachtungsFehler> {
    let verlangt = verlangte_zeugen(kontrakt, waehrung, betrag);
    if verlangt == 0 {
        return Ok(Verwendbar::OhneBezeugung);
    }
    if attestierungen.is_empty() {
        return Ok(Verwendbar::ZuWenigZeugen { hatte: 0, verlangt });
    }
    Ok(match beobachte(anfrage, attestierungen, verlangt)? {
        Beobachtung::Einig { inhalt, zeugen, spanne_ms } => Verwendbar::Ja {
            inhalt,
            zeugen: zeugen.len(),
            spanne_ms,
        },
        Beobachtung::Uneinig { varianten, spanne_ms } => Verwendbar::Uneinig {
            varianten: varianten.len(),
            spanne_ms,
        },
        Beobachtung::Zuwenig { hatte, verlangt } => Verwendbar::ZuWenigZeugen { hatte, verlangt },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beobachtung::{attestierungsbytes, Attestierung};
    use myl_types::bls::BlsSecretKey;
    use myl_types::ids::{Address, EpochId, MinerId};
    use myl_types::sitzung::{Grenzen, Zeugenstufe};

    fn geprueft(n: u8, anfrage: &Hash, zeit: u64, inhalt: &Hash) -> GepruefteAttestierung {
        let sk = BlsSecretKey::key_gen(&[n; 32]).expect("Schlüsselerzeugung");
        let pk = sk.public_key().expect("Schlüssel");
        let gateway = MinerId::new([n; 32]);
        let signatur = sk
            .sign(&attestierungsbytes(&gateway, anfrage, zeit, inhalt))
            .expect("signieren");
        Attestierung { gateway, anfrage: *anfrage, zeitpunkt_ms: zeit, inhalt: *inhalt, signatur }
            .pruefe(&pk)
            .expect("prüfen")
    }

    /// Kontrakt mit einer Leiter: bis 999 ein Zeuge, ab 1000 drei.
    fn kontrakt() -> Sitzungskontrakt {
        Sitzungskontrakt::neu(
            Address::new([1; 32]),
            Address::new([2; 32]),
            Grenzen {
                budget: 1_000_000,
                einzellimit: 1_000_000,
                schwelle: u64::MAX,
                zeugenleiter: vec![
                    Zeugenstufe { ab_betrag: 0, zeugen: 1 },
                    Zeugenstufe { ab_betrag: 1_000, zeugen: 3 },
                ],
            },
            Grenzen::gesperrt(),
            Vec::new(),
            EpochId(0),
            EpochId(100),
        )
        .expect("gültiger Kontrakt")
    }

    /// ⚑ **Dieselbe Beobachtung, zwei Beträge, zwei Antworten.** Genau
    /// das ist die Kopplung aus Design 1: Die Zahl skaliert mit dem,
    /// was auf dem Spiel steht.
    #[test]
    fn derselbe_abruf_reicht_fuer_wenig_und_nicht_fuer_viel() {
        let k = kontrakt();
        let anfrage = Hash::sha256(b"wie spaet ist es");
        let inhalt = Hash::sha256(b"zwoelf");
        let zwei = [geprueft(1, &anfrage, 100, &inhalt), geprueft(2, &anfrage, 140, &inhalt)];

        let klein =
            darf_verwendet_werden(&k, Waehrung::Credits, 999, &anfrage, &zwei).expect("gut");
        assert_eq!(klein, Verwendbar::Ja { inhalt, zeugen: 2, spanne_ms: 40 });
        assert!(klein.ja());

        let gross =
            darf_verwendet_werden(&k, Waehrung::Credits, 1_000, &anfrage, &zwei).expect("gut");
        assert_eq!(gross, Verwendbar::ZuWenigZeugen { hatte: 2, verlangt: 3 });
        assert!(!gross.ja());
    }

    /// Uneinigkeit ist nie verwendbar, und sie wird nicht aufgelöst.
    #[test]
    fn uneinigkeit_bleibt_uneinigkeit() {
        let k = kontrakt();
        let anfrage = Hash::sha256(b"kurs");
        let a = Hash::sha256(b"100");
        let b = Hash::sha256(b"101");
        let drei = [
            geprueft(1, &anfrage, 0, &a),
            geprueft(2, &anfrage, 10, &a),
            geprueft(3, &anfrage, 30, &b),
        ];
        let befund =
            darf_verwendet_werden(&k, Waehrung::Credits, 5_000, &anfrage, &drei).expect("gut");
        assert_eq!(befund, Verwendbar::Uneinig { varianten: 2, spanne_ms: 30 });
        assert!(!befund.ja());
    }

    /// ⚑ Der Agent kann die Zahl nicht senken: Ein Kontrakt mit einer
    /// milderen Leiter ist ein anderer Kontrakt.
    #[test]
    fn eine_mildere_leiter_ist_ein_anderer_kontrakt() {
        let streng = kontrakt();
        let mild = Sitzungskontrakt::neu(
            Address::new([1; 32]),
            Address::new([2; 32]),
            Grenzen {
                zeugenleiter: vec![Zeugenstufe { ab_betrag: 0, zeugen: 1 }],
                ..streng.credits.clone()
            },
            Grenzen::gesperrt(),
            Vec::new(),
            EpochId(0),
            EpochId(100),
        )
        .expect("gültig");
        assert_ne!(streng.adresse(), mild.adresse());
        assert_eq!(verlangte_zeugen(&streng, Waehrung::Credits, 1_000), 3);
        assert_eq!(verlangte_zeugen(&mild, Waehrung::Credits, 1_000), 1);
    }

    /// Verlangt der Kontrakt Zeugen und es kommt keiner, ist das zu
    /// wenig und kein Fehler: Der Kontrakt hat die Frage beantwortet.
    #[test]
    fn verlangte_zeugen_die_ausbleiben_sind_zu_wenige() {
        let k = kontrakt();
        let anfrage = Hash::sha256(b"leer");
        assert_eq!(
            darf_verwendet_werden(&k, Waehrung::Credits, 5_000, &anfrage, &[]).expect("gut"),
            Verwendbar::ZuWenigZeugen { hatte: 0, verlangt: 3 }
        );
    }

    /// Wer keine Bezeugung verlangt hat, bekommt keine erzwungen.
    #[test]
    fn ohne_leiter_genuegt_auch_nichts() {
        let ohne = Sitzungskontrakt::neu(
            Address::new([1; 32]),
            Address::new([2; 32]),
            Grenzen { budget: 10, einzellimit: 10, schwelle: u64::MAX, zeugenleiter: Vec::new() },
            Grenzen::gesperrt(),
            Vec::new(),
            EpochId(0),
            EpochId(1),
        )
        .expect("gültig");
        let anfrage = Hash::sha256(b"nichts");
        assert_eq!(verlangte_zeugen(&ohne, Waehrung::Credits, 5), 0);
        let befund =
            darf_verwendet_werden(&ohne, Waehrung::Credits, 5, &anfrage, &[]).expect("gut");
        // ⚑ Nicht „einig über die leere Menge", sondern „es war nichts
        // verlangt". Der Unterschied ist der zwischen einer geprüften
        // und einer ungeprüften Aussage.
        assert_eq!(befund, Verwendbar::OhneBezeugung);
        assert!(befund.ja());
    }
}
