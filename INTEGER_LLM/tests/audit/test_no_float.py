#!/usr/bin/env python3
"""
Gleitkomma-Audit des Inferenzpfads (Punkt 12.22).

Eigenständiges Skript nach Projektkonvention (kein pytest). Die
Kerneigenschaft des Projekts — vollständig ganzzahlige Inferenz ohne
Gleitkomma im Rechenpfad — wird hier automatisch geprüft statt nur
implizit angenommen.

Ansatz (statische Quell-Analyse, deterministisch und CI-fähig):
  1. Die Heißpfad-Rust-Quellen werden gescannt (kernels ohne bin/,
     runtime ohne loader).
  2. Kommentare, String-Literale und #[cfg(test)]-Module werden entfernt
     (Test-Fixtures dürfen Gleitkomma verwenden, z. B. um Referenz-LUTs
     zu erzeugen; das ist nicht der Inferenzpfad).
  3. Verbleibende Gleitkomma-Nutzung (f32/f64-Typen, as f32/as f64-Casts,
     float-Literale, float-Methoden wie .exp()/.sqrt()) wird gemeldet.
  4. Fund im Heißpfad => Test failt.

Geprüft werden ZWEI Pfade und, seit dem 2026-08-28, die Liste selbst:
  A. **Inferenz-Heißpfad** (kernels, runtime) — die Ganzzahligkeit der
     Inferenz, Kernthese des Projekts (Kap. 6.2).
  B. **Konsenspfad** (myl-types, myl-ledger, myl-scheduler,
     myl-consensus, myl-tokenomics, myl-verifier) — dieselbe
     Anforderung auf der Protokollseite. Ein `f64` in der
     Preisformel oder der Komiteewahl bricht den Konsens genauso wie
     eines in der Inferenz; bis v0.2.9 war dieser Pfad ungeprüft, und
     genau dort lagen zwei reale Funde (die zur Laufzeit mit
     `f64::exp()` gebaute Preis-LUT und die `f64`-Sampling-Rate).

  C. **Die Vollständigkeit der Liste** (`pruefe_vollstaendigkeit`). Eine
     von Hand gepflegte Dateiliste hört leise auf, vollständig zu sein:
     Eine neue Datei wird nicht geprüft, der Lauf meldet trotzdem „null
     Treffer", und das klingt nach Vollständigkeit. So entstand Fund 44,
     und am 2026-08-28 fiel auf, dass `erasure.rs` und `pq.rs` seit
     ihrer Entstehung fehlten. Für `myl-types` und `myl-ledger` gilt
     deshalb: jede Datei ist gelistet oder trägt eine Begründung.

Dokumentiert erlaubte Zonen (kein Heiß-/Konsenspfad):
  - #[cfg(test)]-Module (Test-Fixtures, z. B. LUT-Erzeugung in
    kernels/src/{mlp,attention,rmsnorm}.rs, statistische Schranken in
    myl-types/src/seed_rng.rs)
  - kernels/src/bin/golden_runner.rs (Offline-Referenz-Erzeugung)
  - runtime/src/loader.rs (Kalibrier-Metadaten + Skalen-Validierung,
    Setup statt Inferenzpfad)
  - myl-tokenomics/src/utilization.rs (`utilization_to_f64` /
    `utilization_from_f64` sind ausdrücklich als Debug-/Logging-Helfer
    dokumentiert und gehen nicht in den Konsenswert ein)
  - myl-net (Netzschicht: die EMA-Latenzglättung ist Eingangsgröße für
    Attest-Erzeugung, nicht selbst Konsens-Feld — die Atteste tragen
    ganzzahlige Millisekunden)
  - myl-net/src/scoring.rs (Gossipsub-Peer-Scoring, 2026-08-24). Die
    Bewertung rechnet in f64, und das ist Absicht: Der Peer-Score hängt
    an lokalen Beobachtungen und Ankunftszeiten. Zwei ehrliche Knoten
    muessen hier zu VERSCHIEDENEN Ergebnissen kommen duerfen; eine
    Ganzzahlfassung wuerde Bitgleichheit suggerieren, wo keine erwuenscht
    ist. Kein Wert aus dem Modul geht in Block, Attest oder Ledger ein.
    Die Zahlengrenzen daneben (myl-net/src/limits.rs) sind ganzzahlig
    und werden geprueft.

Akzeptanzkriterium: null Gleitkomma-Treffer in beiden Pfaden.
"""

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent

# Heißpfad-Dateien (Inferenz-Rechenpfad). runtime/loader.rs ist bewusst
# ausgenommen (Setup/Metadaten), ebenso kernels/src/bin (Offline-Tools).
HOT_PATH = [
    REPO / "kernels" / "src" / "lib.rs",
    REPO / "kernels" / "src" / "fixed_point.rs",
    REPO / "kernels" / "src" / "rmsnorm.rs",
    REPO / "kernels" / "src" / "linear.rs",
    REPO / "kernels" / "src" / "rope.rs",
    REPO / "kernels" / "src" / "attention.rs",
    REPO / "kernels" / "src" / "mlp.rs",
    REPO / "kernels" / "src" / "moe.rs",
    REPO / "kernels" / "src" / "softmax.rs",
    REPO / "kernels" / "src" / "backend.rs",
    REPO / "kernels" / "src" / "integer_math.rs",
    REPO / "kernels" / "src" / "prng.rs",
    REPO / "kernels" / "src" / "sampling.rs",
    # 2026-08-29 nachgetragen, und ⚑ **`backward.rs` ist die schwerste
    # Luecke, die dieses Skript je hatte**: der ganze Rueckwaertspass,
    # also genau der Pfad, dessen Ganzzahligkeit die Trainingsthese
    # traegt. Er stand seit seiner Entstehung nicht in dieser Liste, und
    # der Lauf meldete trotzdem "null Treffer".
    #
    # Dieselbe Klasse wie Fund 84 und Fund 44, jetzt zum dritten Mal.
    # Die Vollstaendigkeitspruefung unten deckt seit heute auch
    # `kernels/src` und `runtime/src` ab; ohne sie waere es wieder nur
    # aufgefallen, weil jemand aus anderem Grund hinsah.
    REPO / "kernels" / "src" / "backward.rs",
    REPO / "kernels" / "src" / "optimierer.rs",
    REPO / "kernels" / "src" / "dot.rs",
    REPO / "kernels" / "src" / "rechenpfad.rs",
    REPO / "kernels" / "src" / "konformitaet.rs",
    REPO / "kernels" / "src" / "backends" / "mod.rs",
    REPO / "kernels" / "src" / "backends" / "reference.rs",
    REPO / "kernels" / "src" / "backends" / "simd.rs",
    REPO / "kernels" / "src" / "backends" / "cuda.rs",
    REPO / "kernels" / "src" / "backends" / "rocm.rs",
    REPO / "runtime" / "src" / "model.rs",
    REPO / "runtime" / "src" / "kv_cache.rs",
    REPO / "runtime" / "src" / "generate.rs",
    REPO / "runtime" / "src" / "lib.rs",
    REPO / "runtime" / "src" / "main.rs",
    REPO / "runtime" / "src" / "paths.rs",
    REPO / "runtime" / "src" / "tokenizer.rs",
    # Die Konformitaetspruefung, seit sie eine Bibliothek ist (2026-08-27).
    #
    # Vorher lag sie in kernels/src/bin/golden_runner.rs, und das ist als
    # Offline-Werkzeug ausgenommen. Dort steckte eine f64-Nachbildung der
    # exp-LUT als Rueckfall fuer Vektoren ohne LUT-Metadaten — genau die
    # Art Gleitkomma, gegen die dieses Skript geschrieben ist, nur an
    # einer Stelle, die es nicht ansah. Der Rueckfall ist beim Umzug
    # entfallen; damit er nicht zurueckkommt, stehen die beiden Module
    # jetzt hier. Dieselbe Luecke wie bei moe.rs, das als
    # Rechenpfad-Datei ebenfalls nicht in dieser Liste stand.
    REPO / "kernels" / "src" / "konformitaet.rs",
    REPO / "runtime" / "src" / "konformitaet.rs",
]

# Konsenspfad der Netzwerkkomponenten. Dieselbe Anforderung wie oben:
# jede dieser Dateien berechnet Werte, die alle Nodes bitgleich
# nachrechnen können müssen. `utilization.rs` ist bewusst nicht dabei
# (dokumentierte Debug-Helfer, siehe Modul-Doku dort).
ROOT = REPO.parent
CONSENSUS_PATH = [
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "hash.rs",
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "merkle.rs",
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "ids.rs",
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "core_types.rs",
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "challenge.rs",
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "seed_rng.rs",
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "bls.rs",
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "vrf.rs",
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "latency_attest.rs",
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "node_metadata.rs",
    # 2026-08-28 nachgetragen, und drei auf einmal: `erasure.rs` und
    # `pq.rs` fehlten seit ihrer Entstehung, `sitzung.rs` kam heute
    # dazu. Siehe die Vollstaendigkeitspruefung weiter unten, die genau
    # das kuenftig laut macht.
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "erasure.rs",
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "pq.rs",
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "protocol.rs",
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "sitzung.rs",
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "lib.rs",
    # Der Generator der Konformitaetsvektoren gehoert dazu, obwohl er
    # nicht laeuft, wenn das Netz laeuft: Er **erzeugt die Referenz**,
    # gegen die sich fremde Implementierungen pruefen. Gleitkomma hier
    # verdurbe die Vektoren, nicht nur einen Lauf.
    ROOT / "SHARED_TYPES" / "myl-types" / "src" / "bin" / "generate_golden_vectors.rs",
    ROOT / "CONSENSUS" / "myl-ledger" / "src" / "lib.rs",
    ROOT / "CONSENSUS" / "myl-ledger" / "src" / "state.rs",
    ROOT / "CONSENSUS" / "myl-ledger" / "src" / "transitions.rs",
    ROOT / "CONSENSUS" / "myl-scheduler" / "src" / "vrf_seed.rs",
    ROOT / "CONSENSUS" / "myl-scheduler" / "src" / "miner_filter.rs",
    ROOT / "CONSENSUS" / "myl-scheduler" / "src" / "geo_clustering.rs",
    ROOT / "CONSENSUS" / "myl-scheduler" / "src" / "shard_assignment.rs",
    ROOT / "CONSENSUS" / "myl-scheduler" / "src" / "redundancy.rs",
    ROOT / "CONSENSUS" / "myl-scheduler" / "src" / "sampling.rs",
    ROOT / "CONSENSUS" / "myl-consensus" / "src" / "bft.rs",
    ROOT / "CONSENSUS" / "myl-consensus" / "src" / "block.rs",
    ROOT / "CONSENSUS" / "myl-consensus" / "src" / "signing.rs",
    ROOT / "CONSENSUS" / "myl-consensus" / "src" / "validator.rs",
    ROOT / "CONSENSUS" / "myl-consensus" / "src" / "voting_weight.rs",
    ROOT / "CONSENSUS" / "myl-consensus" / "src" / "double_signing.rs",
    ROOT / "TOKENOMICS" / "myl-tokenomics" / "src" / "ema.rs",
    ROOT / "TOKENOMICS" / "myl-tokenomics" / "src" / "mint.rs",
    ROOT / "TOKENOMICS" / "myl-tokenomics" / "src" / "distribute.rs",
    ROOT / "TOKENOMICS" / "myl-tokenomics" / "src" / "training.rs",
    ROOT / "TOKENOMICS" / "myl-tokenomics" / "src" / "exp_approx.rs",
    ROOT / "TOKENOMICS" / "myl-tokenomics" / "src" / "exp_lut_table.rs",
    ROOT / "VERIFICATION" / "myl-verifier" / "src" / "redundancy.rs",
    ROOT / "VERIFICATION" / "myl-verifier" / "src" / "checker.rs",
    ROOT / "VERIFICATION" / "myl-verifier" / "src" / "challenge.rs",
    ROOT / "VERIFICATION" / "myl-verifier" / "src" / "bisection.rs",
    ROOT / "VERIFICATION" / "myl-verifier" / "src" / "adjudicate.rs",
    ROOT / "VERIFICATION" / "myl-verifier" / "src" / "slash.rs",
    ROOT / "VERIFICATION" / "myl-verifier" / "src" / "delivery.rs",
    ROOT / "VERIFICATION" / "myl-verifier" / "src" / "kontrollsegmente.rs",
    ROOT / "VERIFICATION" / "myl-verifier" / "src" / "unterscheider.rs",
    # 2026-08-27 mit dem Messgeraet fuer die Ununterscheidbarkeit.
    # Es misst Verteilungen und ist genau deshalb der Ort, an dem
    # Gleitkomma am naechsten liegt: p-Werte, Abstaende, Anteile.
    # Alles davon ist hier ein Bruch zweier Ganzzahlen, und die Datei
    # steht in der Liste, damit das so bleibt.
    ROOT / "VERIFICATION" / "myl-verifier" / "src" / "unterscheidbarkeit.rs",
    ROOT / "COMPUTE_PIPELINE" / "myl-pod" / "src" / "standby.rs",
    ROOT / "SIMULATION" / "myl-simulation" / "src" / "szenario.rs",
    # NETWORKING, nachgetragen 2026-08-23 (Fund 44).
    #
    # Diese Liste enthielt bis dahin **keine einzige Datei aus
    # `myl-net`**, und der Lauf meldete trotzdem "null Treffer" — über 57
    # Dateien, was nach Vollständigkeit klang und eine Auswahl war. In
    # `latency.rs` rechnete die Latenz-EMA in `f64`, obwohl der Kopf des
    # Crates seit dem ersten Tag Festkomma zusagt und `config.rs` die
    # ganzzahligen Konstanten dafür führt.
    #
    # `latency.rs` ist der Zulieferer des `LatencyGraph` und damit des
    # Geo-Clusterings der Pods (`myl-scheduler/src/geo_clustering.rs`,
    # eine Zeile höher in dieser Liste). Ein Zulieferer des Konsenspfads
    # gehört in den Konsenspfad.
    ROOT / "NETWORKING" / "myl-net" / "src" / "latency.rs",
    ROOT / "NETWORKING" / "myl-net" / "src" / "config.rs",
    ROOT / "NETWORKING" / "myl-net" / "src" / "validation.rs",
    ROOT / "NETWORKING" / "myl-net" / "src" / "gossip.rs",
    ROOT / "NETWORKING" / "myl-net" / "src" / "identity.rs",
    ROOT / "NETWORKING" / "myl-net" / "src" / "discovery.rs",
    ROOT / "NETWORKING" / "myl-net" / "src" / "node.rs",
    ROOT / "NETWORKING" / "myl-net" / "src" / "runtime.rs",
    # 2026-08-24: Die Verbindungsgrenzen sind reine Ganzzahlen und
    # gehoeren geprueft. `scoring.rs` daneben steht bewusst NICHT hier
    # (dokumentierte Ausnahme oben, Peer-Score ist lokal statt Konsens).
    ROOT / "NETWORKING" / "myl-net" / "src" / "limits.rs",
    ROOT / "NETWORKING" / "myl-net" / "src" / "nat.rs",
    ROOT / "NETWORKING" / "myl-net" / "src" / "anfrage.rs",
    # 2026-08-27 mit der Sitzungsschicht (Punkte 3.1 bis 3.3). Zaehler,
    # Laengen und Nonces sind ganzzahlig; ein Gleitkommawert im Nonce
    # oder in einer Groessengrenze waere kein Rundungsfehler, sondern
    # eine Nonce-Wiederholung. Aufgenommen mit der ersten Zeile, nicht
    # spaeter: Fund 44 entstand genau aus dem "spaeter".
    ROOT / "NETWORKING" / "myl-net" / "src" / "sitzung.rs",
    # NODE, aufgenommen 2026-08-24 mit dem Knoten-Binary. Die
    # Verdrahtung darf so wenig Gleitkomma enthalten wie das, was sie
    # verdrahtet, sonst wandert es genau hierher.
    ROOT / "NODE" / "myl-node" / "src" / "kette.rs",
    ROOT / "NODE" / "myl-node" / "src" / "probe.rs",
    ROOT / "NODE" / "myl-node" / "src" / "nachschub.rs",
    ROOT / "NODE" / "myl-node" / "src" / "knoten.rs",
    ROOT / "NODE" / "myl-node" / "src" / "konfig.rs",
    ROOT / "NODE" / "myl-node" / "src" / "protokoll.rs",
    ROOT / "NODE" / "myl-node" / "src" / "validator.rs",
    # GOVERNANCE, aufgenommen 2026-08-24 mit der ersten Zeile Code.
    # Die Registry hält die Parameter, die in Ledger-Zustandsübergänge
    # eingehen; ein Gleitkommawert hier wäre derselbe Konsensbruch wie
    # einer in TOKENOMICS.
    ROOT / "GOVERNANCE" / "myl-governance" / "src" / "registry.rs",
    ROOT / "GOVERNANCE" / "myl-governance" / "src" / "invarianten.rs",
    ROOT / "GOVERNANCE" / "myl-governance" / "src" / "vorschlag.rs",
    # 2026-08-28 mit der Abstimmung. Quoren, Mehrheiten und
    # Beteiligungen sind Anteile, und Anteile sind die Stelle, an der
    # Gleitkomma am naechsten liegt. Hier sind es Promille, also
    # Ganzzahlen, und die Datei steht in der Liste, damit das so
    # bleibt: Ein Stimmgewicht, das je Knoten anders rundet, ist ein
    # Konsensbruch.
    ROOT / "GOVERNANCE" / "myl-governance" / "src" / "abstimmung.rs",
    ROOT / "GOVERNANCE" / "myl-governance" / "src" / "modell.rs",
    ROOT / "TOKENOMICS" / "myl-tokenomics" / "src" / "sicherheit.rs",
    ROOT / "TOKENOMICS" / "myl-tokenomics" / "src" / "stake.rs",
    ROOT / "TOKENOMICS" / "myl-tokenomics" / "src" / "slashing.rs",
    ROOT / "TOKENOMICS" / "myl-tokenomics" / "src" / "anlauf.rs",
    ROOT / "TOKENOMICS" / "myl-tokenomics" / "src" / "genesis.rs",
]

# Gleitkomma-Indikatoren (angewandt nach Entfernen von Kommentaren,
# Strings und Test-Modulen).
FLOAT_PATTERNS = [
    (re.compile(r"\bf32\b"), "f32-Typ"),
    (re.compile(r"\bf64\b"), "f64-Typ"),
    (re.compile(r"\bas\s+f32\b"), "as-f32-Cast"),
    (re.compile(r"\bas\s+f64\b"), "as-f64-Cast"),
    (re.compile(r"\.exp\(\)"), ".exp()-Methode"),
    (re.compile(r"\.sqrt\(\)"), ".sqrt()-Methode"),
    (re.compile(r"\.ln\(\)"), ".ln()-Methode"),
    (re.compile(r"\.powf\("), ".powf()-Methode"),
    (re.compile(r"\.floor\(\)"), ".floor()-Methode"),
    (re.compile(r"\.ceil\(\)"), ".ceil()-Methode"),
    # float-Literal (z. B. 2.0, 0.5, 256.0) — nicht 0..129 (Range).
    (re.compile(r"\b\d+\.\d+(?:f32|f64)?\b"), "float-Literal"),
]


def strip_comments(src: str) -> str:
    """Entfernt Zeilen- (//) und Block-Kommentare (/* */)."""
    # Block-Kommentare (nicht-gierig, auch mehrzeilig).
    src = re.sub(r"/\*.*?\*/", "", src, flags=re.DOTALL)
    # Zeilen-Kommentare.
    src = re.sub(r"//[^\n]*", "", src)
    return src


def strip_strings(src: str) -> str:
    """Entfernt String-Literale (inkl. Escapes), behält Struktur.

    **DOTALL ist notwendig, nicht kosmetisch (2026-08-24).** Rust erlaubt
    im String-Literal einen Zeilenumbruch mit `\` am Zeilenende:

        "erster Teil, \
         zweiter Teil"

    Ohne DOTALL trifft `\\.` diesen Umbruch nicht (`.` schließt `\n`
    aus), und `[^"\\]` schließt den Backslash aus. Der ganze String
    blieb damit stehen und wurde als **Code** geprüft. Ein Verweis wie
    "Kap. 10.3" in einer Fehlermeldung sah dann aus wie ein
    Gleitkomma-Literal.

    Aufgefallen bei der Aufnahme von `myl-governance` in den Konsenspfad,
    wo zwei solche Meldungen als Treffer gemeldet wurden. Es sind nur
    Falschmeldungen und keine übersehenen Treffer, aber sie sind
    gefährlich: Wer sie sieht, nimmt eher die Datei aus der Liste, als das
    Muster zu prüfen. Genau so entstand der blinde Fleck, der Fund 44
    ermöglichte.
    """
    return re.sub(r'"(?:\\.|[^"\\])*"', '""', src, flags=re.DOTALL)


def strip_test_modules(src: str) -> str:
    """Entfernt #[cfg(test)]-Module per Brace-Matching."""
    out = []
    i = 0
    n = len(src)
    marker = "#[cfg(test)]"
    while i < n:
        pos = src.find(marker, i)
        if pos == -1:
            out.append(src[i:])
            break
        out.append(src[i:pos])
        # Finde das öffnende '{' des Moduls nach dem Attribut.
        j = pos + len(marker)
        # Überspringe optionale weitere Attribute/Whitespace bis 'mod'.
        brace = src.find("{", j)
        if brace == -1:
            out.append(src[pos:])
            break
        # Brace-Matching.
        depth = 0
        k = brace
        while k < n:
            if src[k] == "{":
                depth += 1
            elif src[k] == "}":
                depth -= 1
                if depth == 0:
                    break
            k += 1
        i = k + 1  # hinter das schließende '}' springen
    return "".join(out)


def audit_file(path: Path):
    """Liefert eine Liste von (Zeilennummer, Muster-Label, Zeile)-Treffern."""
    src = path.read_text(encoding="utf-8")
    src = strip_comments(src)
    src = strip_strings(src)
    src = strip_test_modules(src)
    findings = []
    for line_no, line in enumerate(src.splitlines(), start=1):
        for pat, label in FLOAT_PATTERNS:
            for _ in pat.finditer(line):
                findings.append((line_no, label, line.strip()))
    return findings


def audit_group(label: str, paths) -> int:
    """Prüft eine Dateigruppe; liefert die Anzahl der Treffer."""
    print(f"[no-float] {label}: {len(paths)} Dateien")

    missing = [p for p in paths if not p.exists()]
    if missing:
        for p in missing:
            print(f"[no-float] FEHLT: {p}")
        print(f"[no-float] FEHLGESCHLAGEN ({label}: Dateien fehlen)")
        sys.exit(1)

    total = 0
    for path in paths:
        findings = audit_file(path)
        for line_no, pattern_label, line in findings:
            print(f"[no-float] TREFFER {path.name}:{line_no} ({pattern_label}): {line}")
        total += len(findings)
    return total


# Crates, in denen **jede** Datei zum Konsenspfad gehoert.
#
# ⚑ **Warum diese Pruefung existiert (2026-08-28).** `CONSENSUS_PATH` ist
# eine von Hand gepflegte Liste, und eine solche Liste hoert leise auf,
# vollstaendig zu sein: Eine neue Datei wird nicht geprueft, der Lauf
# meldet trotzdem "null Treffer", und das klingt nach Vollstaendigkeit.
# Genau so entstand **Fund 44** (kein einziges `myl-net` in der Liste,
# waehrend `latency.rs` in `f64` rechnete).
#
# Am 2026-08-28 war es wieder so weit: `erasure.rs` und `pq.rs` standen
# seit ihrer Entstehung nicht darin. Aufgefallen ist es nur, weil
# jemand aus einem anderen Grund hinsah.
#
# **Fuer diese beiden Crates gilt die Regel ohne Ausnahme:** In
# `myl-types` ist jeder Typ Protokoll, in `myl-ledger` jede Datei ein
# Zustandsuebergang. Wer hier eine Datei anlegt, muss sie eintragen,
# sonst schlaegt der Lauf fehl. Fuer die uebrigen Crates (NETWORKING,
# VERIFICATION und so weiter) gilt das **nicht**, dort sind Auswahl und
# Begruendung je Datei noetig; diese Pruefung deckt sie deshalb nicht ab
# und behauptet es auch nicht.
VOLLSTAENDIG_ZU_PRUEFEN = [
    ROOT / "SHARED_TYPES" / "myl-types" / "src",
    ROOT / "CONSENSUS" / "myl-ledger" / "src",
    # 2026-08-29 dazugekommen, nachdem hier vier Dateien fehlten,
    # darunter der ganze Rueckwaertspass. In diesen beiden
    # Verzeichnissen ist jede Datei Heisspfad; die einzige Ausnahme ist
    # `bin/`, siehe BEWUSST_DRAUSSEN.
    REPO / "kernels" / "src",
    REPO / "runtime" / "src",
]

# Dateien aus den obigen Verzeichnissen, die bewusst **nicht** geprueft
# werden, je mit Begruendung.
#
# ⚑ **Diese Liste ist leer, und der Weg hierher ist der Sinn.** Wer eine
# Datei nicht pruefen lassen will, muss sie hier eintragen und
# hinschreiben, warum. Ohne diesen Weg waere die naheliegende Reaktion
# auf eine unbequeme Meldung, das ganze Verzeichnis aus
# `VOLLSTAENDIG_ZU_PRUEFEN` zu nehmen, und damit fiele die Pruefung
# still weg. Dieselbe Ueberlegung wie bei `strip_strings`: Eine
# Falschmeldung verleitet dazu, die Pruefung zu entschaerfen statt die
# Ursache anzusehen.
BEWUSST_DRAUSSEN: dict = {
    # ⚑ **Die einzige Ausnahme, und sie ist eng.** `loader.rs` haelt an
    # vier Stellen `f64`, und zwar ausschliesslich, um zu **pruefen**,
    # ob die im Artefakt angegebene `scale` zu ihrem `shift` passt
    # (`scale == 2^-shift`). Gerechnet wird mit `shift`, einer Ganzzahl;
    # die `f64` verlaesst die Pruefung nicht.
    #
    # **Warum die Datei trotzdem hier steht statt einfach zu fehlen:**
    # Eine ungelistete Datei ist unsichtbar, eine gelistete Ausnahme ist
    # bestreitbar. Wer spaeter eine Gleitkommazahl in den Rechenpfad
    # dieser Datei traegt, faellt hier nicht auf — **also gehoert diese
    # Zeile bei jeder Aenderung an `loader.rs` mitgelesen.**
    #
    # ⚑ **Und dahinter steckt ein Befund, der nicht hier zu beheben
    # ist:** `scale` ist aus `shift` ableitbar. Ein ableitbares Feld im
    # Artefakt ist eine zweite Quelle fuer dieselbe Aussage, und genau
    # deshalb muss der Loader die beiden gegeneinander pruefen. Es zu
    # entfernen hiesse, das Artefaktformat zu aendern und alle Artefakte
    # neu zu bauen; als eigener Punkt vermerkt.
    REPO
    / "runtime"
    / "src"
    / "loader.rs": "f64 nur zur Konsistenzpruefung von scale gegen shift, nie im Rechenpfad",
}

# Verzeichnisnamen, deren Inhalt bewusst nicht geprueft wird.
#
# ⚑ **`bin/` sind Offline-Werkzeuge**: Sie erzeugen Referenzvektoren,
# vermessen und schuetten Zwischenstaende aus, und keines von ihnen
# laeuft, waehrend das Netz laeuft.
#
# **Die Ausnahme ist nicht gratis.** Bis zum 2026-08-27 steckte in
# `kernels/src/bin/golden_runner.rs` eine f64-Nachbildung der exp-LUT
# als Rueckfall, und sie fiel nur auf, weil die Pruefliste beim Umzug
# von Hand durchgesehen wurde. **Wer hier ein Werkzeug ablegt, das doch
# im Betrieb laeuft, hat es der Pruefung entzogen, ohne es zu merken.**
BEWUSST_DRAUSSEN_ORDNER = {"bin"}


def pruefe_vollstaendigkeit() -> int:
    """Meldet Dateien, die in einem Konsens-Crate liegen und fehlen.

    Gegenprobe zur Liste selbst: Sie faengt nicht Gleitkomma, sondern
    das Vergessen.
    """
    # ⚑ Gegen **beide** Listen, nicht nur gegen den Konsenspfad. Der
    # erste Entwurf verglich nur mit CONSENSUS_PATH und meldete daraufhin
    # jede Heisspfad-Datei als fehlend: eine Pruefung, die 47 Treffer
    # meldet, liest niemand.
    gelistet = {p.resolve() for p in CONSENSUS_PATH} | {p.resolve() for p in HOT_PATH}
    fehlend = []
    for verzeichnis in VOLLSTAENDIG_ZU_PRUEFEN:
        if not verzeichnis.is_dir():
            print(f"[no-float] FEHLT (Verzeichnis): {verzeichnis}")
            return 1
        for datei in sorted(verzeichnis.rglob("*.rs")):
            if datei.resolve() in gelistet:
                continue
            if datei.resolve() in {k.resolve() for k in BEWUSST_DRAUSSEN}:
                continue
            if BEWUSST_DRAUSSEN_ORDNER & set(datei.parts):
                continue
            fehlend.append(datei)
    print(f"[no-float] Vollstaendigkeit: {len(VOLLSTAENDIG_ZU_PRUEFEN)} Verzeichnisse")
    for datei in fehlend:
        print(f"[no-float] NICHT GELISTET: {datei}")
    return len(fehlend)


def main():
    print("[no-float] Gleitkomma-Audit (Inferenz- und Konsenspfad)")

    total = audit_group("Inferenz-Heißpfad", HOT_PATH)
    total += audit_group("Konsenspfad", CONSENSUS_PATH)
    ungelistet = pruefe_vollstaendigkeit()
    if ungelistet:
        print(
            f"[no-float] FEHLGESCHLAGEN: {ungelistet} Datei(en) in einem "
            "Konsens-Crate stehen nicht in CONSENSUS_PATH"
        )
        sys.exit(1)

    if total == 0:
        print("[no-float] PASSED: null Gleitkomma-Treffer in Inferenz- und Konsenspfad")
        sys.exit(0)
    else:
        print(f"[no-float] FEHLGESCHLAGEN: {total} Gleitkomma-Treffer")
        sys.exit(1)


if __name__ == "__main__":
    main()
