//! Gleichstand zwischen NODE und NETWORKING.
//!
//! # Warum diese Datei nötig ist
//!
//! Zwei Stellen im Projekt rechnen dieselbe Ableitung aus, und keine
//! von beiden kann die andere sehen:
//!
//! - `myl_node::genesis::GenesisValidator::kennung` bildet aus dem
//!   BLS-Schlüssel eines Validators seine `MinerId`.
//! - `myl_net::endpunkt_aus_schluessel` bildet aus demselben Schlüssel
//!   den Endpunkt einer verschlüsselten Sitzung.
//!
//! Der Grund für die Trennung ist die Schichtung: `myl-net` ist L0 und
//! darf die Genesis-Datei nicht kennen. Der Preis ist eine
//! **Doppelrechnung**, und Doppelrechnungen laufen auseinander, sobald
//! jemand eine von beiden anfasst.
//!
//! ⚑ **Was auseinanderlaufen würde, wenn niemand hinsähe:** Die
//! Sitzungsschicht prüft eine Epochenankündigung, indem sie den
//! Endpunkt aus dem mitgeführten Schlüssel ableitet und mit dem
//! vergleicht, den der Pod-Pfad nennt. Der Pod-Pfad führt `MinerId`s.
//! Rechnen die beiden Seiten verschieden, passt **keine einzige**
//! Ankündigung mehr, und der Fehler sähe aus wie ein Angriff: Jede
//! Prüfung meldet „gehört zu einem anderen Endpunkt", und niemand käme
//! auf die Idee, dass beide Seiten recht haben.
//!
//! Diese Datei ist der Ort, an dem das auffällt, bevor es passiert.

use myl_node::genesis::GenesisValidator;
use myl_types::bls::BlsSecretKey;

fn validator(saat: u8, stake: u64) -> GenesisValidator {
    let sk = BlsSecretKey::key_gen(&[saat; 32]).expect("Schlüsselerzeugung");
    GenesisValidator {
        pubkey: sk.public_key().expect("Schlüssel"),
        pop: sk.prove_possession().expect("Besitznachweis"),
        stake,
    }
}

#[test]
fn die_minerid_der_genesis_ist_der_sitzungsendpunkt() {
    // Die eine Aussage, um die es geht: Wer in der Genesis-Datei steht,
    // ist im Sitzungskanal derselbe.
    for saat in [1u8, 7, 42, 200] {
        let v = validator(saat, 1_000);
        let aus_der_genesis = v.kennung();
        let aus_der_netzschicht = myl_net::endpunkt_aus_schluessel(&v.pubkey);
        assert_eq!(
            aus_der_netzschicht.bytes(),
            aus_der_genesis.as_bytes(),
            "MinerId und Sitzungsendpunkt sind auseinandergelaufen (Saat {saat})"
        );
    }
}

#[test]
fn verschiedene_schluessel_geben_verschiedene_endpunkte() {
    // Die Gegenprobe: Ohne sie hieße der Test oben auch dann „gleich",
    // wenn beide Seiten konstant dasselbe zurückgäben.
    let a = validator(1, 1_000);
    let b = validator(2, 1_000);
    assert_ne!(
        myl_net::endpunkt_aus_schluessel(&a.pubkey),
        myl_net::endpunkt_aus_schluessel(&b.pubkey)
    );
    assert_ne!(a.kennung(), b.kennung());
}

#[test]
fn eine_ankuendigung_wird_gegen_die_genesis_kennung_geprueft() {
    // Der Weg, wie er im Betrieb läuft: Der Pod-Pfad nennt eine
    // MinerId, die Ankündigung kommt vom Netz, und geprüft wird gegen
    // genau diese MinerId. Ohne Register, ohne Zuordnungstabelle.
    let sk = BlsSecretKey::key_gen(&[9u8; 32]).expect("Schlüsselerzeugung");
    let v = GenesisValidator {
        pubkey: sk.public_key().expect("Schlüssel"),
        pop: sk.prove_possession().expect("Besitznachweis"),
        stake: 1_000,
    };
    let aus_dem_pod_pfad = myl_net::Endpunkt::aus_bytes(*v.kennung().as_bytes());

    let epochenschluessel = myl_net::Epochenschluessel::probe(myl_types::ids::EpochId(9), [3u8; 32]);
    let ankuendigung =
        myl_net::Epochenankuendigung::neu(&sk, &epochenschluessel).expect("ankündigen");

    assert_eq!(
        ankuendigung
            .pruefe(aus_dem_pod_pfad, myl_types::ids::EpochId(9))
            .expect("prüfen")
            .punkt,
        epochenschluessel.punkt()
    );

    // Und ein anderer Validator kommt darüber nicht herein.
    let fremd = BlsSecretKey::key_gen(&[10u8; 32]).expect("Schlüsselerzeugung");
    let fremde = myl_net::Epochenankuendigung::neu(&fremd, &epochenschluessel).expect("ankündigen");
    assert!(fremde
        .pruefe(aus_dem_pod_pfad, myl_types::ids::EpochId(9))
        .is_err());
}
