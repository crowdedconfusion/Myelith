//! Was die Ökonomie aushalten muss, wenn jemand lügt (K4).
//!
//! Dieses Crate rechnet aus, **wie viel Geld entsteht und wer es
//! bekommt**. Ein Fehler hier zahlt aus, statt nur etwas zuzulassen, und
//! er zahlt jede Epoche neu aus, bis ihn jemand bemerkt.
//!
//! Die Modultests belegen, dass die Formeln die vorgesehenen Werte
//! liefern; das ist der Erfolgsfall, den K4 als überrepräsentiert
//! benennt. Hier stehen die **Eigenschaften, die nach jeder Rechnung
//! gelten müssen**, geprüft über Eingaben, die niemand ausgesucht hat,
//! einschließlich der Ränder des Zahlbereichs.
//!
//! ## Warum die Ränder und nicht nur plausible Werte
//!
//! Alle Parameter dieses Crates sind für Governance vorgesehen
//! (Kap. 10.3). Eine Abstimmung kann jeden von ihnen auf jeden Wert
//! setzen, den der Typ hergibt. „So wird das niemand konfigurieren" ist
//! deshalb keine Zusicherung, sondern eine Hoffnung; was der Typ zulässt,
//! muss die Funktion aushalten.

use myl_tokenomics::distribute::{
    distribute_mint, redundancy_normalized_weight, split_proportional, SHARES_TOTAL_BPS,
};
use myl_tokenomics::ema::{ema_update, ema_update_with_alpha};
use myl_tokenomics::exp_approx::{exp_approx, update_price};
use myl_tokenomics::mint::{mint_amount, MintParams};
use myl_tokenomics::training::{capped_training_reward, training_reward_cap};
use myl_tokenomics::utilization::{calculate_utilization, UTILIZATION_SCALE};
use myl_tokenomics::vtfe::{vtfe_gutschrift, vtfe_voll, ModellProfil, ShardZuschnitt};
use myl_types::ids::Address;

/// xorshift64, reproduzierbar und ohne Abhängigkeit.
struct Wuerfel(u64);
impl Wuerfel {
    fn neu(keim: u64) -> Self {
        Self(keim | 1)
    }
    fn naechste(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn bis(&mut self, n: u64) -> u64 {
        self.naechste() % n
    }
}

fn adresse(b: u64) -> Address {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&b.to_le_bytes());
    Address::new(bytes)
}

fn qwen05b() -> ModellProfil {
    ModellProfil {
        hidden_size: 896,
        intermediate_size: 4864,
        num_experts: 0,
        num_experts_per_tok: 0,
        moe_intermediate_size: 0,
        num_layers: 24,
        vocab_size: 151_936,
        num_heads: 14,
        num_kv_heads: 2,
        head_dim: 64,
    }
}

// ---------------------------------------------------------------------
// Die Geldmenge
// ---------------------------------------------------------------------

/// **Invariante: Die Verteilung gibt genau das aus, was geprägt wurde.**
///
/// Nicht „ungefähr" und nicht „höchstens": Ein Rundungsrest, der
/// verschwindet, wäre Geld, das niemand bekommt; einer, der doppelt
/// vergeben wird, wäre Geld aus dem Nichts. Beides muss über den ganzen
/// Zahlbereich ausgeschlossen sein, `u64::MAX` eingeschlossen.
#[test]
fn die_verteilung_gibt_genau_die_praegung_aus() {
    let mut w = Wuerfel::neu(7);
    let mut faelle: Vec<u64> = vec![0, 1, 2, 3, 9_999, 10_000, 10_001, u64::MAX, u64::MAX - 1];
    for _ in 0..50_000 {
        faelle.push(w.naechste());
    }
    for m in faelle {
        let d = distribute_mint(m);
        assert_eq!(d.summe(), m, "Prägung {m} wurde nicht vollständig verteilt");
    }
}

/// **Invariante: Kein Empfänger bekommt mehr als seinen Schlüsselanteil.**
///
/// Das Treasury bekommt seinen Anteil **plus** den Rundungsrest; der Rest
/// ist kleiner als die Zahl der Gruppen und damit vernachlässigbar, aber
/// er muss nach oben begrenzt bleiben, sonst wäre „Rundungsrest" ein
/// Kanal.
#[test]
fn kein_empfaenger_bekommt_mehr_als_seinen_schluessel() {
    let mut w = Wuerfel::neu(11);
    for _ in 0..50_000 {
        let m = w.naechste();
        let d = distribute_mint(m);
        let anteil = |bps: u64| (m as u128 * bps as u128) / SHARES_TOTAL_BPS as u128;
        assert!(d.shard_miners as u128 <= anteil(7_800));
        assert!(d.coordinators as u128 <= anteil(500));
        assert!(d.validators as u128 <= anteil(1_000));
        assert!(d.checkers as u128 <= anteil(400));
        // Treasury: Grundanteil plus höchstens vier Rundungseinheiten.
        assert!(d.treasury as u128 <= anteil(300) + 4);
    }
}

/// **Invariante: Die Prägung übersteigt nie die Obergrenze.**
///
/// `M_max` ist der einzige harte Deckel der Geldmenge. Die
/// Epochensimulation aus K8 hat gezeigt, dass er im normalen Verlauf nie
/// greift; deshalb ist er dort **nicht geprüft, sondern nur nicht
/// verletzt worden**. Hier wird er gezielt getroffen.
#[test]
fn die_praegung_uebersteigt_nie_die_obergrenze() {
    let mut w = Wuerfel::neu(13);
    for _ in 0..50_000 {
        let params = MintParams {
            subsidy_num: w.bis(1_000),
            subsidy_den: w.bis(1_000) + 1,
            m_max: w.naechste(),
        };
        let m = mint_amount(w.naechste(), &params);
        assert!(
            m <= params.m_max,
            "Prägung {m} über der Obergrenze {}",
            params.m_max
        );
    }
}

/// **Angriff: die Subventionsrate so setzen, dass die Rechnung überläuft.**
///
/// `ema_burn · (den + num)` bildet `den + num` in `u64`. Alle drei sind
/// Governance-Parameter; eine Abstimmung kann sie an den Rand des
/// Zahlbereichs legen. Ein Überlauf wäre im Debug-Build eine Panik und im
/// Release-Build eine Prägung, die nicht der Formel entspricht, und damit
/// ein Konsensbruch zwischen Knoten mit verschiedenen Bauprofilen.
#[test]
fn extreme_subventionsparameter_praegen_nicht_aus_dem_nichts() {
    let faelle = [
        (u64::MAX, 0u64, 1u64, u64::MAX),
        (u64::MAX, 1, 1, u64::MAX),
        (u64::MAX, u64::MAX, u64::MAX, u64::MAX),
        (u64::MAX, u64::MAX - 1, 1, u64::MAX),
        (1, u64::MAX, 1, u64::MAX),
        (0, u64::MAX, u64::MAX, u64::MAX),
    ];
    for (ema, num, den, m_max) in faelle {
        let params = MintParams { subsidy_num: num, subsidy_den: den, m_max };
        let m = mint_amount(ema, &params);
        assert!(m <= m_max, "ema {ema}, s = {num}/{den}: Prägung {m} über {m_max}");
    }
}

/// **Invariante: Die proportionale Aufteilung zahlt exakt `total` aus.**
///
/// Weder mehr (Geld aus dem Nichts) noch weniger (verschwundenes Geld).
/// Geprüft über zufällige Gewichtslisten mit Doppeladressen, Nullgewichten
/// und Extremwerten, denn genau die kommen aus einer echten Epoche.
#[test]
fn die_proportionale_aufteilung_zahlt_exakt_aus() {
    let mut w = Wuerfel::neu(17);
    for _ in 0..20_000 {
        let n = (w.bis(12) + 1) as usize;
        let gewichte: Vec<(Address, u64)> = (0..n)
            .map(|_| {
                let a = adresse(w.bis(5)); // wenige Adressen ⇒ Doppelungen
                let g = match w.bis(4) {
                    0 => 0,
                    1 => u64::MAX,
                    _ => w.naechste(),
                };
                (a, g)
            })
            .collect();
        let total = match w.bis(4) {
            0 => 0,
            1 => u64::MAX,
            _ => w.naechste(),
        };

        match split_proportional(total, &gewichte) {
            Ok(auszahlung) => {
                let summe: u128 = auszahlung.values().map(|v| *v as u128).sum();
                assert_eq!(summe, total as u128, "Auszahlung ≠ Betrag");
                // Wer kein Gewicht hat, bekommt nichts.
                for (addr, betrag) in &auszahlung {
                    if *betrag > 0 {
                        let g: u64 = gewichte
                            .iter()
                            .filter(|(a, _)| a == addr)
                            .map(|(_, g)| *g)
                            .fold(0u64, |a, b| a.saturating_add(b));
                        assert!(g > 0, "Auszahlung an ein Gewicht von 0");
                    }
                }
            }
            Err(_) => {
                // Der einzige zulässige Fehlerfall: positiver Betrag,
                // aber alle Gewichte null.
                let summe: u128 = gewichte.iter().map(|(_, g)| *g as u128).sum();
                assert!(total > 0 && summe == 0);
            }
        }
    }
}

/// **Angriff: sich durch Doppelnennung eine zweite Auszahlung holen.**
///
/// Dieselbe Adresse mehrfach in der Gewichtsliste darf nicht zweimal
/// bedient werden. Die Gewichte werden zusammengeführt, das Ergebnis ist
/// dasselbe wie bei einer einzigen Nennung mit der Gewichtssumme.
#[test]
fn doppelte_adressen_zahlen_nicht_doppelt() {
    let einmal = split_proportional(
        1_000,
        &[(adresse(1), 60), (adresse(2), 40)],
    )
    .unwrap();
    let doppelt = split_proportional(
        1_000,
        &[(adresse(1), 30), (adresse(2), 40), (adresse(1), 30)],
    )
    .unwrap();
    assert_eq!(einmal, doppelt, "Doppelnennung ändert die Auszahlung");
}

// ---------------------------------------------------------------------
// Die EMA
// ---------------------------------------------------------------------

/// **Invariante: Ein EMA-Schritt geht nie über die Stichprobe hinaus.**
///
/// Das Ergebnis liegt stets zwischen `prev` und `sample`. Wäre es das
/// nicht, könnte eine einzelne Epoche mit hohem Verbrauch die EMA über
/// ihren eigenen Wert treiben, und die Prägung folgte einem Wert, den
/// niemand verbrannt hat.
#[test]
fn ein_ema_schritt_geht_nie_ueber_die_stichprobe_hinaus() {
    let mut w = Wuerfel::neu(19);
    let mut faelle: Vec<(u64, u64)> = vec![
        (0, 0),
        (0, u64::MAX),
        (u64::MAX, 0),
        (u64::MAX, u64::MAX),
        (u64::MAX - 1, u64::MAX),
    ];
    for _ in 0..50_000 {
        faelle.push((w.naechste(), w.naechste()));
    }
    for (prev, sample) in faelle {
        let neu = ema_update(prev, sample);
        let (lo, hi) = (prev.min(sample), prev.max(sample));
        assert!(
            (lo..=hi).contains(&neu),
            "EMA-Schritt von {prev} mit Stichprobe {sample} ergab {neu}"
        );
    }
}

/// **Angriff: einen α-Bruch über 1 durchsetzen** (⚑ Fund 47).
///
/// Für 0 < num ≤ den ist der Schritt ein Anteil der Differenz und damit
/// beschränkt. Für num > den ist er es nicht, und das ist als
/// Governance-Parameter erreichbar. Vor der Behebung geschah zweierlei:
/// ein `debug_assert!` ließ die Funktion **im Debug-Build abstürzen und
/// im Release-Build weiterrechnen**, und `−200 as u64` ergab einen Wert
/// nahe 2⁶⁴, der über `mint_amount` unmittelbar an die Prägeobergrenze
/// gegangen wäre.
///
/// Beides geprüft: kein Absturz auf keinem Bauprofil, und das Ergebnis
/// bleibt beschnitten statt umzulaufen.
#[test]
fn ein_alpha_ueber_eins_laeuft_nicht_um() {
    // 100 + (0 − 100) · 3/1 = 100 − 300 = −200, beschnitten auf 0.
    assert_eq!(ema_update_with_alpha(100, 0, 3, 1), 0);
    // Und nach oben ebenso.
    assert_eq!(ema_update_with_alpha(u64::MAX, u64::MAX, 5, 1), u64::MAX);

    let mut w = Wuerfel::neu(23);
    for _ in 0..20_000 {
        // Ausdrücklich auch jenseits von 1: Das ist der Parameter-Fehler,
        // der nicht in einen Umlauf führen darf.
        let den = w.bis(100) + 1;
        let num = w.bis(500) + 1;
        let (prev, sample) = (w.naechste(), w.naechste());
        let _ = ema_update_with_alpha(prev, sample, num, den);
    }

    // Im zulässigen Bereich bleibt der Schritt zwischen beiden Werten.
    for _ in 0..20_000 {
        let den = w.bis(1_000) + 1;
        let num = w.bis(den) + 1; // 1 ≤ num ≤ den
        let (prev, sample) = (w.naechste(), w.naechste());
        let neu = ema_update_with_alpha(prev, sample, num, den);
        assert!((prev.min(sample)..=prev.max(sample)).contains(&neu));
    }
}

// ---------------------------------------------------------------------
// Der Preis
// ---------------------------------------------------------------------

/// **Angriff: den Preis durch extreme Auslastung auf null oder ins
/// Unendliche treiben.**
///
/// Ein Preis von null wäre kostenlose Inferenz für alle; ein überlaufender
/// Preis wäre eine Denial-of-Service gegen die Nutzer. Geprüft wird, dass
/// die Rechnung an keiner Eingabe umläuft und der Preis nichtnegativ
/// bleibt.
#[test]
fn der_preis_laeuft_an_keiner_eingabe_um() {
    let mut w = Wuerfel::neu(29);
    let mut faelle: Vec<(i64, i64, i64, i64)> = vec![
        (i64::MAX, i64::MAX, i64::MAX, i64::MIN),
        (i64::MAX, 0, 0, 0),
        (1i64 << 32, i64::MAX, i64::MAX, i64::MIN),
        (1i64 << 32, i64::MIN, i64::MIN, i64::MAX),
        (0, 0, 0, 0),
    ];
    for _ in 0..50_000 {
        faelle.push((
            (w.naechste() >> 1) as i64,
            (w.naechste() % (1 << 20)) as i64,
            (w.naechste() % (4 * UTILIZATION_SCALE as u64)) as i64,
            (w.naechste() % (2 * UTILIZATION_SCALE as u64)) as i64,
        ));
    }
    for (p, k, u, ziel) in faelle {
        let neu = update_price(p, k, u, ziel);
        assert!(neu >= 0 || p < 0, "Preis {p} wurde zu {neu}");
    }
}

/// **`exp_approx` bleibt an jeder Eingabe im Bereich der Tabelle.**
///
/// Sie wird mit einem Exponenten aufgerufen, der aus Auslastung und κ
/// entsteht, beides Governance-nah. Ein Indexzugriff außerhalb der
/// Tabelle wäre eine Panik im Konsenspfad.
#[test]
fn exp_approx_haelt_jede_eingabe_aus() {
    let mut w = Wuerfel::neu(31);
    for x in [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX] {
        let y = exp_approx(x);
        assert!(y >= 0, "exp({x}) = {y} ist negativ");
    }
    for _ in 0..100_000 {
        let x = w.naechste() as i64;
        let y = exp_approx(x);
        assert!(y >= 0, "exp({x}) = {y} ist negativ");
    }
}

/// **Die Auslastung bleibt an jeder Eingabe endlich und nichtnegativ.**
#[test]
fn die_auslastung_haelt_jede_eingabe_aus() {
    let mut w = Wuerfel::neu(37);
    assert_eq!(calculate_utilization(1_000, 0), 0, "keine Kapazität, keine Auslastung");
    for _ in 0..50_000 {
        let u = calculate_utilization(w.naechste(), w.naechste());
        assert!(u >= 0);
    }
    // Der Extremfall: alles nachgefragt, ein Kleinstbetrag Kapazität.
    let _ = calculate_utilization(u64::MAX, 1);
}

// ---------------------------------------------------------------------
// Trainingsvergütung und Redundanz
// ---------------------------------------------------------------------

/// **Invariante: Die Trainingsvergütung übersteigt nie 70 % der
/// Inferenzvergütung.**
///
/// Kap. 5.6 begründet die Grenze damit, dass Training sonst attraktiver
/// wäre als Inferenz und das Netz seinen eigentlichen Dienst aufgäbe.
#[test]
fn die_trainingsverguetung_bleibt_unter_der_obergrenze() {
    let mut w = Wuerfel::neu(41);
    let mut faelle: Vec<(u64, u64)> = vec![(u64::MAX, u64::MAX), (0, u64::MAX), (u64::MAX, 0)];
    for _ in 0..50_000 {
        faelle.push((w.naechste(), w.naechste()));
    }
    for (angefragt, inferenz) in faelle {
        let gewaehrt = capped_training_reward(angefragt, inferenz);
        assert!(gewaehrt <= training_reward_cap(inferenz));
        assert!(gewaehrt <= angefragt, "mehr gewährt als angefragt");
        assert!(
            gewaehrt as u128 * 10_000 <= inferenz as u128 * 7_000,
            "Vergütung {gewaehrt} über 70 % von {inferenz}"
        );
    }
}

/// **Die Redundanz-Normierung halbiert, und zwar nach unten.**
///
/// Aufrunden hieße, dass zwei Pods zusammen mehr als eine volle
/// Gutschrift bekommen.
#[test]
fn die_redundanz_normierung_rundet_nach_unten() {
    let mut w = Wuerfel::neu(43);
    for v in [0u64, 1, 2, 3, u64::MAX, u64::MAX - 1]
        .into_iter()
        .chain((0..20_000).map(|_| w.naechste()))
    {
        let halb = redundancy_normalized_weight(v);
        assert!(halb.saturating_mul(2) <= v.max(halb.saturating_mul(2)));
        assert_eq!(halb, v / 2);
        assert!(2 * (v / 2) <= v, "zwei Pods bekämen zusammen mehr als eine volle Gutschrift");
    }
}

// ---------------------------------------------------------------------
// vTFE
// ---------------------------------------------------------------------

/// **Invariante: Ein Zuschnitt kann nie mehr beanspruchen als das ganze
/// Modell hergibt.**
///
/// Das ist die Abrechnungsgrundlage des ganzen Netzes. Wer hier mehr
/// beanspruchen kann als seinen Anteil, bekommt Geld für Arbeit, die er
/// nicht geleistet hat.
#[test]
fn ein_zuschnitt_beansprucht_nie_mehr_als_das_ganze_modell() {
    let profil = qwen05b();
    let mut w = Wuerfel::neu(47);
    for _ in 0..20_000 {
        let tokens = w.bis(10_000) + 1;
        let a = w.bis(profil.num_layers + 1);
        let b = w.bis(profil.num_layers + 1);
        let zuschnitt = ShardZuschnitt {
            layer_start: a.min(b),
            layer_end: a.max(b),
            hat_embedding: w.bis(2) == 0,
            hat_lm_kopf: w.bis(2) == 0,
        };
        let teil = vtfe_gutschrift(&profil, &zuschnitt, tokens).expect("gültiger Zuschnitt");
        assert!(
            teil <= vtfe_voll(tokens),
            "Zuschnitt {zuschnitt:?} beansprucht {teil} von höchstens {}",
            vtfe_voll(tokens)
        );
    }
}

/// **Angriff: einen Zuschnitt außerhalb des Modells beanspruchen.**
///
/// Wer `layer_end` über die Layerzahl hinaus setzt oder die Grenzen
/// vertauscht, dürfte sonst Arbeit abrechnen, die es nicht gibt.
#[test]
fn ein_zuschnitt_ausserhalb_des_modells_wird_abgelehnt() {
    let profil = qwen05b();
    let faelle = [
        (0u64, profil.num_layers + 1),
        (5, 4),
        (profil.num_layers, profil.num_layers + 10),
        (u64::MAX, u64::MAX),
        (0, u64::MAX),
    ];
    for (start, end) in faelle {
        let z = ShardZuschnitt {
            layer_start: start,
            layer_end: end,
            hat_embedding: false,
            hat_lm_kopf: false,
        };
        assert!(
            vtfe_gutschrift(&profil, &z, 100).is_err(),
            "Zuschnitt {start}..{end} muss abgelehnt werden"
        );
    }
}

/// **Angriff: ein Modellprofil ohne Rechenarbeit unterschieben.**
///
/// Der Nenner der Regel ist die Arbeit des vollen Vorwärtspasses. Ist er
/// null, wäre jede Gutschrift eine Division durch null.
#[test]
fn ein_modell_ohne_arbeit_wird_abgelehnt() {
    let leer = ModellProfil {
        hidden_size: 0,
        intermediate_size: 0,
        num_experts: 0,
        num_experts_per_tok: 0,
        moe_intermediate_size: 0,
        num_layers: 0,
        vocab_size: 0,
        num_heads: 0,
        num_kv_heads: 0,
        head_dim: 0,
    };
    let z = ShardZuschnitt {
        layer_start: 0,
        layer_end: 0,
        hat_embedding: true,
        hat_lm_kopf: true,
    };
    assert!(vtfe_gutschrift(&leer, &z, 1_000).is_err());
}

/// **Gegenprobe: Ein vollständiger Zuschnitt bekommt die volle
/// Gutschrift, und zwei Zuschnitte zusammen nie mehr als einer.**
///
/// Ohne diese Probe wäre jeder Test darüber wertlos: Eine Funktion, die
/// immer null zurückgibt, verletzt keine Obergrenze.
#[test]
fn ein_vollstaendiger_zuschnitt_bekommt_die_volle_gutschrift() {
    let profil = qwen05b();
    let tokens = 1_000u64;
    let ganz = ShardZuschnitt {
        layer_start: 0,
        layer_end: profil.num_layers,
        hat_embedding: true,
        hat_lm_kopf: true,
    };
    let voll = vtfe_gutschrift(&profil, &ganz, tokens).unwrap();
    assert!(voll > 0, "der volle Zuschnitt muss etwas bekommen");
    // Bis auf die Abrundung ist es die ganze Summe.
    let erwartet = vtfe_voll(tokens);
    assert!(
        voll <= erwartet && erwartet - voll < 1_000,
        "voller Zuschnitt bekam {voll} statt {erwartet}"
    );

    // Zwei disjunkte Hälften zusammen: nie mehr als das Ganze.
    let mitte = profil.num_layers / 2;
    let links = ShardZuschnitt { layer_start: 0, layer_end: mitte, hat_embedding: true, hat_lm_kopf: false };
    let rechts = ShardZuschnitt { layer_start: mitte, layer_end: profil.num_layers, hat_embedding: false, hat_lm_kopf: true };
    let summe = vtfe_gutschrift(&profil, &links, tokens).unwrap()
        + vtfe_gutschrift(&profil, &rechts, tokens).unwrap();
    assert!(summe <= voll, "zwei Hälften beanspruchen {summe}, das Ganze nur {voll}");
}
