//! Was ein Shard aushalten muss, wenn ihm jemand Unsinn schickt (K4).
//!
//! K4 verlangt „je Komponente eine adversariale Testebene" und „Fuzzing
//! über die Netzwerk-Deserialisierung". `myl-types` hat einen
//! Fuzz-Harness für seine Borsh-Pfade; **das Drahtformat des Pods hatte
//! keinen**, obwohl es dasjenige ist, das über das Netz kommt und
//! Aktivierungen trägt.
//!
//! ## Die Anforderung, in einem Satz
//!
//! Ein Shard darf an einer fremden Nachricht **niemals abstürzen**. Er
//! darf sie ablehnen, und er soll das oft tun; aber eine Panik ist im
//! offenen Netz ein Denial-of-Service, den jeder auslösen kann, der
//! Bytes schicken darf.
//!
//! Das ist eine schärfere Anforderung als „gibt `Err` zurück": Auch ein
//! `unwrap` auf einem leeren Vektor, ein Indexzugriff außerhalb der
//! Grenzen oder eine Subtraktion unter null wären Paniken, und keine
//! davon zeigt sich im Erfolgsfall.

use std::sync::Arc;

use myl_pod::wire::{self, PodMessage, FLAG_FEEDBACK, FLAG_SAMPLE, FLAG_TOKEN_INPUT};
use myl_types::ids::SegmentId;

/// SplitMix64, reproduzierbar und ohne Abhängigkeit.
struct Rng(u64);

impl Rng {
    fn neu(keim: u64) -> Self {
        Self(keim)
    }
    fn u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| (self.u64() & 0xff) as u8).collect()
    }
}

/// **Zufällige Bytes dürfen die Deserialisierung nie zum Absturz bringen.**
///
/// Der Erwartungswert ist, dass fast alles abgelehnt wird. Das ist in
/// Ordnung; geprüft wird die Abwesenheit einer Panik, nicht eine
/// Trefferquote.
#[test]
fn zufaellige_bytes_bringen_die_deserialisierung_nicht_zum_absturz() {
    // **Der erste Anlauf war ein grüner Haken ohne Aussage.** Er zog rein
    // zufällige Bytes, und davon deserialisierte in 50 000 Versuchen
    // **keiner einzige**: Borsh liest zuerst Längenfelder, und ein
    // zufälliges u32 verlangt gleich mehrere Gigabyte. Der Test prüfte
    // also nur, dass Ablehnen nicht abstürzt, und nie, was mit einer
    // angenommenen Nachricht geschieht. Dieselbe Falle wie bei Fund 33,
    // und gefunden nur, weil die Auskunftszeile die Null zeigte.
    //
    // Gezogen wird deshalb **strukturiert**: ein gültiger Kopf, ein
    // zufälliger Rest. Damit kommt ein erheblicher Teil durch.
    let vorlage = PodMessage::token_input(
        SegmentId::new([1u8; 32]),
        1,
        1,
        wire::pack_tokens(&[7]),
        FLAG_SAMPLE,
    );
    let gut = borsh::to_vec(&vorlage).expect("serialisierbar");

    let mut rng = Rng::neu(0xC0FF_EE00);
    let mut angenommen = 0usize;
    for _ in 0..50_000 {
        let mut roh = gut.clone();
        // Ab einer zufälligen Stelle alles überschreiben.
        let ab = (rng.u64() as usize) % roh.len();
        for b in roh.iter_mut().skip(ab) {
            *b = (rng.u64() & 0xff) as u8;
        }
        if let Ok(msg) = borsh::from_slice::<PodMessage>(&roh) {
            angenommen += 1;
            // Was durchkommt, muss auch weiterbehandelbar sein, ohne
            // panisch zu werden: Rahmenprüfung und Token-Auspacken.
            let _ = msg.is_valid_frame();
            let _ = msg.carries_tokens();
            let _ = wire::unpack_tokens(&msg.payload);
        }
    }
    eprintln!("  von 50.000 verstümmelten Nachrichten deserialisierten {angenommen}");
    assert!(
        angenommen > 500,
        "nur {angenommen} von 50.000 kamen durch; der Test prüft dann die \
         interessante Hälfte nicht und ist wertlos"
    );

    // Und zusätzlich reiner Zufall, denn auch der darf nicht abstürzen.
    for _ in 0..10_000 {
        let laenge = (rng.u64() % 512) as usize;
        let _ = borsh::from_slice::<PodMessage>(&rng.bytes(laenge));
    }
}

/// **Gültige Nachrichten mit gekippten Bits.** Näher am realen Angriff
/// als reiner Zufall: Der Rahmen stimmt, der Inhalt nicht.
#[test]
fn gekippte_bits_in_gueltigen_nachrichten_stuerzen_nicht_ab() {
    let vorlage = PodMessage::token_input(
        SegmentId::new([3u8; 32]),
        7,
        0,
        wire::pack_tokens(&[1234, 5678]),
        FLAG_SAMPLE,
    );
    let gut = borsh::to_vec(&vorlage).expect("serialisierbar");

    let mut rng = Rng::neu(0x0BAD_C0DE);
    for _ in 0..20_000 {
        let mut kaputt = gut.clone();
        // Ein bis drei gekippte Bits an zufälligen Stellen.
        for _ in 0..=(rng.u64() % 3) {
            let i = (rng.u64() as usize) % kaputt.len();
            kaputt[i] ^= 1 << (rng.u64() % 8);
        }
        if let Ok(msg) = borsh::from_slice::<PodMessage>(&kaputt) {
            let _ = msg.is_valid_frame();
            let _ = wire::unpack_tokens(&msg.payload);
        }
    }
}

/// **Abgeschnittene Nachrichten.** Der häufigste Netzfehler überhaupt,
/// und der, bei dem eine Längenangabe im Kopf gefährlich wird.
#[test]
fn abgeschnittene_nachrichten_stuerzen_nicht_ab() {
    let vorlage = PodMessage::token_input(
        SegmentId::new([9u8; 32]),
        1,
        5,
        wire::pack_tokens(&[42]),
        FLAG_TOKEN_INPUT | FLAG_FEEDBACK,
    );
    let gut = borsh::to_vec(&vorlage).expect("serialisierbar");
    for n in 0..gut.len() {
        let _ = borsh::from_slice::<PodMessage>(&gut[..n]);
    }
}

/// **`unpack_tokens` gegen jede Nutzlast**, die ihm untergeschoben
/// werden kann: leer, ungerade Länge, negative Werte, Extremwerte.
///
/// Die Funktion setzt zwei `i16` zu einem `u32` zusammen. Eine ungerade
/// Länge oder eine leere Folge sind der Normalfall bei einer
/// beschädigten Nachricht.
#[test]
fn unpack_tokens_haelt_jede_nutzlast_aus() {
    let mut rng = Rng::neu(0x5EED);
    for _ in 0..20_000 {
        let n = (rng.u64() % 40) as usize;
        let nutz: Vec<i16> = (0..n).map(|_| (rng.u64() as u16) as i16).collect();
        let _ = wire::unpack_tokens(&nutz);
    }
    // Und die benannten Randfälle, damit sie nicht dem Zufall überlassen
    // bleiben.
    for fall in [
        vec![],
        vec![0],
        vec![-1],
        vec![i16::MIN],
        vec![i16::MAX, i16::MIN],
        vec![0, 0, 0],
    ] {
        let _ = wire::unpack_tokens(&fall);
    }
}

/// **Ein Shard darf an einer fremden Nachricht nicht abstürzen.**
///
/// Das ist die eigentliche Zusicherung: Nicht der Parser allein, sondern
/// der ganze Weg bis in `process` hinein. Er läuft ohne Modell, weil
/// jede dieser Nachrichten vorher abgelehnt werden muss; käme eine
/// davon bis zur Rechnung, wäre **das** der Befund.
#[test]
fn ein_shard_lehnt_fremde_nachrichten_ab_statt_abzustuerzen() {
    let dir = {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        let mut p = std::path::PathBuf::from(manifest);
        p.push("..");
        p.push("..");
        p.push("INTEGER_LLM");
        p.push("artifacts");
        p.push("qwen2.5-0.5b");
        p
    };
    if !dir.exists() {
        eprintln!("SKIP: Artefakte fehlen: {:?}", dir);
        return;
    }
    let model = Arc::new(integer_llm_runtime::loader::load_model(&dir).expect("Modell"));
    let sk = myl_types::bls::BlsSecretKey::key_gen(&[17u8; 32]).expect("BLS");
    let shard = myl_pod::shard::ShardNode::new(
        1, // NICHT Shard 0: hat kein Embedding, muss Token-Eingang ablehnen
        6,
        12,
        false,
        false,
        model,
        sk,
        myl_pod::da::DaStore::new(Box::new(myl_pod::da::XorParityCoder::new(4))),
        8,
    );

    let mut rng = Rng::neu(0xDEAD_BEEF);
    let mut abgelehnt = 0usize;
    for _ in 0..2_000 {
        let msg = PodMessage {
            magic: if rng.u64() % 4 == 0 { wire::MAGIC } else { rng.bytes(8).try_into().unwrap() },
            segment_id: SegmentId::new(rng.bytes(32).try_into().unwrap()),
            session_id: rng.u64(),
            sender_shard: rng.u64(),
            position: rng.u64(),
            flags: rng.u64() & 0xff,
            trace: (0..(rng.u64() % 4))
                .map(|_| rng.bytes(32).try_into().unwrap())
                .collect(),
            signature: myl_types::bls::BlsSignature([0u8; 96]),
            payload: (0..(rng.u64() % 64)).map(|_| rng.u64() as i16).collect(),
        };
        if shard.process(&msg).is_err() {
            abgelehnt += 1;
        }
    }
    assert_eq!(
        abgelehnt, 2_000,
        "jede dieser Nachrichten muss abgelehnt werden; kam eine durch, \
         hat ein Shard fremde Aktivierungen gerechnet"
    );
}
