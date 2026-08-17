# Konformitätspaket — SHARED_TYPES

> **myl-types-Version:** 0.2.3
> **Zweck:** Eigenständiges Artefakt, gegen das fremde Implementierungen
> sich prüfen können — ohne Kenntnis des Projektinneren.

## Was eine konforme Implementierung erfüllen muss

Eine Implementierung ist konform, wenn sie für jeden Golden Vector in
`vectors/` bei identischen Eingaben **bitgleiche** Ausgaben erzeugt.
Die Prüfung erfolgt über die hex-kodierten Hashes/Signaturen/Beweise.

### Vier Validierungsbereiche

| Bereich | Datei | Was geprüft wird |
|---|---|---|
| **Hash** | `vectors/hash.json` | SHA-256 über verschiedene Eingaben (leer, "abc", myelith-protocol-v1, 1000x "a") |
| **Merkle** | `vectors/merkle.json` | Merkle-Baum-Aufbau, Wurzel-Hashes, Beweise (1, 2, 3, 8 Blätter) |
| **VRF** | `vectors/vrf.json` | ECVRF-EDWARDS25519-SHA512-TAI (RFC 9381), Beweise und Outputs |
| **BLS** | `vectors/bls.json` | BLS12-381 Signaturen (min-pk), Einzelsignaturen und Aggregation |

### Golden-Vector-Format

Jede JSON-Datei enthält ein Array von Vektoren. Jeder Vektor hat:
- `name`: Bezeichner für den Vektor
- Eingabefelder (z.B. `input`, `alpha`, `message`, `leaves`)
- Ausgabefelder (z.B. `hash`, `root`, `proof`, `signature`, `output`)
- Alle Bytes sind hex-kodiert (lowercase, ohne Präfix)

### Anforderungen pro Bereich

**Hash:**
- SHA-256 (32 Bytes Ausgabe)
- Muss exakt den NIST-Testvektoren entsprechen

**Merkle:**
- Domain-Separation: Blätter = `SHA-256(0x00 || Daten)`, Knoten = `SHA-256(0x01 || links || rechts)`
- Ungerade Ebenen: letzter Knoten wird mit sich selbst gepaart
- Ein-Blatt-Baum: Wurzel = Blatt-Hash

**VRF:**
- ECVRF-EDWARDS25519-SHA512-TAI (RFC 9381, Suite 0x03)
- Deterministisch: gleicher Schlüssel + gleiche Eingabe = gleicher Beweis
- `proof_to_hash` liefert die VRF-Ausgabe (64 Bytes)

**BLS:**
- BLS12-381 min-pk (Public Key auf G1, Signatur auf G2)
- DST: `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_`
- Aggregation: mehrere Signaturen zu einer zusammenfassen

## Prüfung durchführen

```bash
# Referenz-Implementierung (Rust)
cargo test --test validate_conformance

# Eigene Implementierung: Vektoren laden und nachrechnen
# Die JSON-Dateien enthalten alle Eingaben und erwarteten Ausgaben
```

Exit-Code 0 = alle Vektoren bestanden, 1 = mindestens einer fehlgeschlagen.

## Dateien

```
conformance/
├── README.md          diese Datei
└── vectors/
    ├── hash.json      4 Hash-Vektoren
    ├── merkle.json    4 Merkle-Baum-Vektoren
    ├── vrf.json       5 VRF-Vektoren
    └── bls.json       5 BLS-Vektoren (inkl. Aggregation)
```

## Lizenz

Die Golden Vectors und dieses Konformitätspaket unterliegen derselben
Lizenz wie das Myelith-Projekt (PolyForm Shield License 1.0.0).
