#!/usr/bin/env python3
"""Entscheidungstreue: waehlt der Ganzzahlpfad dieselben Token wie die
Gleitkomma-Referenz, und wo nicht, wie knapp war es?

# ⚑ Warum es diesen Lauf neben `perplexity.py` gibt (2026-09-16)

Perplexitaet misst, welche Wahrscheinlichkeit ein Pfad dem
**vorgegebenen** naechsten Token gibt, gemittelt ueber Positionen. Sie
beantwortet damit **nicht** die Frage, an der der ausgelieferte Client
haengt: Waehlt der Ganzzahlpfad dasselbe Token?

⛔️ **Und die beiden Antworten gehen weit auseinander.** Fuer
Qwen2.5-0,5B lagen am 2026-08-22 beide Messungen auf denselben
Sequenzen vor: Perplexitaetsabstand **+2,11 %**, also bequem unter dem
Kriterium von 5 %, und Top-1-Uebereinstimmung **89,7 %**, also an
jeder zehnten Position eine andere Wahl. Unter freier Generierung
kompensiert sich das nicht, es kompoundiert: 3 von 8 Texten waren
identisch.

**Der Grund ist strukturell.** Teacher-Forcing setzt den Kontext an
jeder Position auf die Wahrheit zurueck; eine Abweichung bei Token `t`
wirkt nicht auf `t+1`. Der Agent macht das Gegenteil, und seine
Werkzeugaufrufe sind JSON: Eine andere Entscheidung ist dort ein
anderer Dateipfad, kein Stilunterschied.

# ⚑ Was diese Probe kann, was eine Abweichungsquote allein nicht kann

**Sie misst mit, ob eine Abweichung ueberhaupt etwas bedeutet.** An
jeder Position wird der Abstand zwischen Rang 1 und Rang 2 erhoben.
Wo die Referenz selbst schwankt, ist eine andere Wahl eine Muenze, die
anders faellt; wo die Referenz sicher ist und der Ganzzahlpfad
abweicht, ist es ein Mangel. **Ohne diesen Abstand ist eine
Abweichungsquote nicht deutbar**, und genau daran krankt die Zahl von
2026-08-22.

⚑ **Und sie loest nebenbei das Stichprobenproblem.** Der
Perplexitaetsabstand ist eine gepoolte Zahl ueber alle Positionen; bei
435 Positionen ist ein Prozent nicht aufgeloest. Die Entscheidungstreue
ist eine **gepaarte** Messung je Position: Jede Position ist ein
Messwert, und der Fehlerbalken kommt aus der Binomialverteilung statt
aus vier Sequenzmittelwerten.

⛔️ **Was diese Probe NICHT ist.** Sie ist keine Konsensbedingung. Die
Bitgleichheit des Ganzzahlpfads mit sich selbst wird ueber die
Konformitaetsvektoren nachgewiesen, nicht hier. Naehe zur
Gleitkomma-Referenz ist eine Guetezahl: 100 % waeren kein Ziel,
sondern ein Hinweis darauf, dass die Quantisierung wirkungslos ist.

Gleitkomma ist nur im Referenzpfad im Einsatz (Messpfad, nicht
Inferenzpfad).

Aufruf aus der Wurzel:

    V=INTEGER_LLM/calibrate/.venv/bin/python3
    INTEGER_LLM_MODEL=myelith-0.6b $V BENCHMARKS/Inferenz/entscheidungstreue.py

Umgebung:
  ENT_SEQUENZEN   Wieviele Sequenzen (Vorgabe 4, wie die Reihe)
  ENT_SEQ_LEN     Tokens je Sequenz (Vorgabe 128)
"""

import json
import math
import os
import subprocess
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
sys.path.insert(0, str(HIER))

from wikitext_common import (  # noqa: E402
    ARTIFACTS_DIR as ARTIFACTS,
    MODEL_DIR,
    MODEL_NAME,
    ergebnis_pfad,
    select_sequences,
)

sys.path.insert(0, str(HIER.parent.parent / "INTEGER_LLM" / "tests"))
from cargo_paths import binary, fehlt_hinweis  # noqa: E402

PROBE = binary("runtime", "entscheidungsprobe")


def ganzzahl_entscheidungen(sequences, seq_datei):
    """Ruft die Ganzzahl-Probe und liest ihre Zeilen ein."""
    with open(seq_datei, "w") as f:
        for ids in sequences:
            f.write(" ".join(str(i) for i in ids) + "\n")

    erg = subprocess.run(
        [str(PROBE), str(ARTIFACTS), str(seq_datei)],
        capture_output=True,
        text=True,
    )
    if erg.returncode != 0:
        print(erg.stderr, file=sys.stderr)
        raise SystemExit("die Entscheidungsprobe ist gescheitert")

    aus = {}
    for zeile in erg.stdout.splitlines():
        if not zeile.startswith("ENT "):
            continue
        _, seq, pos, ziel, top1, lp1, top2, lp2, ziel_lp, rang = zeile.split()
        aus[(int(seq), int(pos))] = {
            "ziel": int(ziel),
            "top1": int(top1),
            "lp1": float(lp1),
            "lp2": float(lp2),
            "ziel_lp": float(ziel_lp),
            "rang": int(rang),
        }
    return aus


def referenz_entscheidungen(sequences):
    """Dasselbe aus der Gleitkomma-Referenz, auf denselben Sequenzen."""
    import torch
    from transformers import AutoModelForCausalLM

    modell = AutoModelForCausalLM.from_pretrained(
        MODEL_DIR, torch_dtype=torch.bfloat16, device_map="cpu"
    )
    modell.eval()

    aus = {}
    with torch.no_grad():
        for seq_idx, ids in enumerate(sequences):
            eingabe = torch.tensor([ids])
            logits = modell(eingabe).logits[0].float()
            logp = torch.log_softmax(logits, dim=-1)
            for pos in range(len(ids) - 1):
                zeile = logp[pos]
                werte, stellen = torch.topk(zeile, 2)
                ziel = ids[pos + 1]
                ziel_lp = zeile[ziel].item()
                # Rang: wie viele echt groesser sind (siehe die Probe).
                rang = 1 + int((zeile > zeile[ziel]).sum().item())
                aus[(seq_idx, pos)] = {
                    "ziel": ziel,
                    "top1": int(stellen[0].item()),
                    "lp1": float(werte[0].item()),
                    "lp2": float(werte[1].item()),
                    "ziel_lp": ziel_lp,
                    "rang": rang,
                }
    return aus


def anteil_mit_band(treffer, n):
    """Anteil samt Wald-Intervall (95 %), das bei 0 und 1 nicht ausbricht."""
    if n == 0:
        return 0.0, 0.0
    p = treffer / n
    band = 1.96 * math.sqrt(max(p * (1 - p), 1e-12) / n)
    return p, band


def main():
    if not PROBE.exists():
        print(f"[ent] {fehlt_hinweis('runtime', 'entscheidungsprobe')}", file=sys.stderr)
        return 1

    n_seq = int(os.environ.get("ENT_SEQUENZEN", "4"))
    seq_len = int(os.environ.get("ENT_SEQ_LEN", "128"))
    print(f"[ent] Modell {MODEL_NAME}, {n_seq} Sequenzen a {seq_len} Tokens")

    sequences = select_sequences(n_seq, seq_len)
    seq_datei = HIER / "results" / f".entscheidung_{MODEL_NAME}.txt"
    seq_datei.parent.mkdir(parents=True, exist_ok=True)

    print("[ent] Ganzzahlpfad ...")
    ganz = ganzzahl_entscheidungen(sequences, seq_datei)
    print(f"[ent]   {len(ganz)} Positionen")

    print("[ent] Gleitkomma-Referenz ...")
    ref = referenz_entscheidungen(sequences)
    print(f"[ent]   {len(ref)} Positionen")

    gemeinsam = sorted(set(ganz) & set(ref))
    if not gemeinsam:
        raise SystemExit("keine gemeinsamen Positionen")

    # ── Auswertung ────────────────────────────────────────────────────
    # ⚑ **Der Entscheidungsabstand der REFERENZ teilt die Positionen.**
    # Er sagt, wie sicher die Referenz an dieser Stelle war, und ist
    # damit unabhaengig davon, was der Ganzzahlpfad getan hat. Den
    # Abstand des Ganzzahlpfads dafuer zu nehmen waere ein Massstab, der
    # sich mit dem misst, was er messen soll.
    einig = 0
    abstaende_uneinig = []
    abstaende_einig = []
    rang_ziel_ganz = []
    rang_ziel_ref = []
    uneinig_sicher = 0   # Referenz war sicher (Abstand > 1 nat), trotzdem uneinig
    sicher_gesamt = 0

    for schluessel in gemeinsam:
        g, r = ganz[schluessel], ref[schluessel]
        abstand_ref = r["lp1"] - r["lp2"]
        sicher = abstand_ref > 1.0
        sicher_gesamt += 1 if sicher else 0
        if g["top1"] == r["top1"]:
            einig += 1
            abstaende_einig.append(abstand_ref)
        else:
            abstaende_uneinig.append(abstand_ref)
            if sicher:
                uneinig_sicher += 1
        rang_ziel_ganz.append(g["rang"])
        rang_ziel_ref.append(r["rang"])

    n = len(gemeinsam)
    treue, band = anteil_mit_band(einig, n)
    med = lambda xs: sorted(xs)[len(xs) // 2] if xs else float("nan")

    print()
    print(f"{'Positionen':<44} {n}")
    print(f"{'Top-1-Uebereinstimmung':<44} {100*treue:6.2f} %  (+/- {100*band:.2f})")
    print(f"{'davon Positionen mit sicherer Referenz':<44} {sicher_gesamt}")
    print(f"{'Abweichungen trotz sicherer Referenz':<44} {uneinig_sicher}")
    print()
    print(f"{'Entscheidungsabstand, wo einig (Median)':<44} {med(abstaende_einig):6.3f} nat")
    print(f"{'Entscheidungsabstand, wo uneinig (Median)':<44} {med(abstaende_uneinig):6.3f} nat")
    print()
    print(f"{'Rang des echten Tokens, Ganzzahl (Median)':<44} {med(rang_ziel_ganz):6.1f}")
    print(f"{'Rang des echten Tokens, Referenz (Median)':<44} {med(rang_ziel_ref):6.1f}")
    print()

    # ⛔️ **Das Urteil haengt an den Abweichungen bei sicherer Referenz**,
    # nicht an der Quote. Eine Abweichung dort, wo die Referenz selbst
    # schwankt, ist eine Muenze, die anders faellt; eine Abweichung, wo
    # die Referenz sicher war, ist ein Mangel des Pfades.
    anteil_sicher, band_sicher = anteil_mit_band(uneinig_sicher, max(sicher_gesamt, 1))
    if sicher_gesamt == 0:
        print("-> KEIN URTEIL: keine Position mit sicherer Referenz in dieser")
        print("   Stichprobe. Mehr Sequenzen messen.")
    elif anteil_sicher < 0.01:
        print(f"-> Die Abweichungen sind Muenzwuerfe: {100*anteil_sicher:.2f} % der")
        print("   sicheren Entscheidungen gehen anders aus. Wo die Referenz")
        print("   selbst schwankt, sagt eine andere Wahl nichts ueber den Pfad.")
    else:
        print(f"-> {100*anteil_sicher:.2f} % (+/- {100*band_sicher:.2f}) der SICHEREN")
        print("   Entscheidungen gehen anders aus. Das sind keine Muenzwuerfe;")
        print("   hier weicht der Ganzzahlpfad ab, wo die Referenz eindeutig war.")

    ziel = ergebnis_pfad("entscheidungstreue")
    ziel.write_text(
        json.dumps(
            {
                "modell": MODEL_NAME,
                "datensatz": "wikitext-2-raw-v1 (Testsplit)",
                "sequenzen": n_seq,
                "seq_len": seq_len,
                "positionen": n,
                "top1_uebereinstimmung": treue,
                "top1_band95": band,
                "positionen_sichere_referenz": sicher_gesamt,
                "abweichungen_bei_sicherer_referenz": uneinig_sicher,
                "anteil_abweichung_sicher": anteil_sicher,
                "abstand_median_einig": med(abstaende_einig),
                "abstand_median_uneinig": med(abstaende_uneinig),
                "rang_median_ganzzahl": med(rang_ziel_ganz),
                "rang_median_referenz": med(rang_ziel_ref),
            },
            indent=2,
            ensure_ascii=False,
        )
        + "\n"
    )
    print(f"\n[ent] abgelegt: {ziel}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
