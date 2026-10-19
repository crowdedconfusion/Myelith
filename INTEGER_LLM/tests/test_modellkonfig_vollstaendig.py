#!/usr/bin/env python3
"""**Stimmt der Modelleintrag mit dem echten `config.json` ueberein?**

## ⛔️ Wozu diese Probe da ist

Am 2026-09-21 haben zwei Angaben Stunden gekostet, und beide hatten
dieselbe Form:

| Angabe | im `config.json` | was ich einsetzte | Folge |
|---|---|---|---|
| `norm_topk_prob` | fehlt | `False` erfunden | Expertengewichte nicht normiert |
| `rope_theta` | `None` oben, **1e7 in `rope_scaling`** | θ_v-Konstante 1e6 | alle Drehwinkel falsch, Kauderwelsch |

📌 **Ein fehlendes Feld ist keine Erlaubnis, einen Vorgabewert zu
erfinden**, und ein Feld, das oben `None` ist, kann weiter unten einen
Wert haben. Beide Male habe ich an einer Stelle nachgesehen und den Rest
angenommen.

⚑ **Diese Probe sucht an allen Stellen.** Sie vergleicht jedes Feld des
Modelleintrags mit dem echten `config.json`, schaut dabei auch in
`text_config` und `rope_scaling`, und meldet zusaetzlich, welche Felder
des Config **niemand gelesen hat**.

⚠️ **Der zweite Teil ist der wichtigere.** Eine Abweichung faellt
irgendwann auf; ein Feld, das niemand liest, nie.

Aufruf:
    python3 INTEGER_LLM/tests/test_modellkonfig_vollstaendig.py [modell]
"""
import json
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
LLM = HIER.parent
REPO = LLM.parent
sys.path.insert(0, str(LLM / "calibrate"))

# ⚑ Die Tabellen und der Abgleich stehen seit 2026-10-19 in
#   `calibrate/src/konfigabgleich.py`, damit das Bauskript dieselbe Pruefung
#   faehrt.
from src.konfigabgleich import flach, feldabgleich  # noqa: E402


def main() -> int:
    from src.model_configs import get_export_model_config, MODEL_CONFIGS as MODELS

    modell = sys.argv[1] if len(sys.argv) > 1 else None
    namen = [modell] if modell else sorted(MODELS)
    fehler = 0

    for name in namen:
        # ⚑ **Nur exportfaehige Eintraege.** Ein Eintrag ohne die
        #   verifizierten Pflichtfelder ist ein Vermerk und kein Modell;
        #   ihn hier zu pruefen hiesse, eine Absicht gegen einen Config
        #   zu halten.
        try:
            eintrag = get_export_model_config(name)
        except ValueError:
            continue
        quelle = REPO / "MODELS" / "llm" / eintrag["hf_model_id"].split("/")[-1]
        pfad = quelle / "config.json"
        if not pfad.is_file():
            print(f"[konfigprobe] {name}: uebersprungen, {pfad} fehlt")
            continue

        echt = flach(json.loads(pfad.read_text(encoding="utf-8")))
        print(f"\n[konfigprobe] {name} gegen {pfad.relative_to(REPO)}")

        # 1. Jedes Feld des Eintrags gegen den Config, 1b die abgeleiteten.
        befund = feldabgleich(eintrag, echt)
        for ck, soll, ist, grund in befund["absichtlich"]:
            print(f"  ⚑ {ck}: {soll!r} gegen {ist!r}, absichtlich ({grund})")
        for ck, soll, ist in befund["abweichend"]:
            print(f"  ⛔️ {ck}: config sagt {soll!r}, Eintrag sagt {ist!r}")
        for meldung in befund["abgeleitet"]:
            print(f"  ⛔️ {meldung}")
        fehler += len(befund["abweichend"]) + len(befund["abgeleitet"])
        abweichend = [ck for ck, _, _ in befund["abweichend"]]

        # 1c. Die Tabellen des Artefakts gegen die Masse.
        #
        # ⛔️ **Der Fund vom 2026-09-22.** Die Drehtabellen des
        # Qwen3.6-35B-A3B hatten 1 310 720 Eintraege, also 40 960 mal 32,
        # waehrend das Artefakt 262 144 Positionen zusagte. Die Zahl stand
        # in `luts.json`, seit das erste Artefakt gebaut wurde, und war
        # mehrfach ausgegeben worden, **ohne sie gegen `max_context` zu
        # halten**.
        #
        # 📌 **Eine Zahl, die man ausgibt, hat man noch nicht geprueft.**
        artefakt = LLM / "artifacts" / name
        luts = artefakt / "luts.json"
        if luts.is_file():
            meta = json.loads(luts.read_text(encoding="utf-8"))
            paare = (eintrag.get("rotary_dim") or eintrag["head_dim"]) // 2
            soll = eintrag["max_context"] * paare
            for tabelle in ("cos", "sin"):
                if tabelle not in meta:
                    print(f"  ⛔️ {tabelle}.lut fehlt im Artefakt")
                    fehler += 1
                    continue
                ist = meta[tabelle]["length"]
                if ist != soll:
                    print(f"  ⛔️ {tabelle}.lut: {ist} Eintraege, aber "
                          f"max_context {eintrag['max_context']} x {paare} "
                          f"Paare = {soll}")
                    fehler += 1
            if all(meta.get(x, {}).get("length") == soll for x in ("cos", "sin")):
                print(f"  ⚑ Drehtabellen passen zu {eintrag['max_context']} "
                      f"Positionen x {paare} Paaren")
            # 1d. **Und ihr Inhalt**: die ersten Zeilen gegen das, was die
            # Masse des Modells ergeben (Basis, Drehbreite, Skalierung).
            #
            # ⛔️ **Der Fund vom 2026-10-13.** 1c pruefte nur die Laenge.
            # Die Tabellen des 35B waren mit der Basis 1e6 gerechnet, das
            # Modell verlangt 1e7; jede Position drehte leicht falsch, und
            # die Probe sagte „passen". Gemessen nach dem Berichtigen: die
            # Perplexitaet weit hinten im Kontext um 0,4 bis 0,85 % besser.
            #
            # 📌 **Eine Groesse zu pruefen ist nicht, den Inhalt zu pruefen.**
            import struct
            from src.luts import generate_rope_luts, load_nonlinear_spec, rope_masse
            masse = rope_masse(eintrag, load_nonlinear_spec())
            zeilen = min(64, eintrag["max_context"])
            sin_soll, cos_soll = generate_rope_luts(
                max_seq_len=zeilen, head_dim=masse["drehbreite"], rope_theta=masse["rope_theta"],
                frac_bits=masse["frac_bits"], frequenzen=masse["frequenzen"], amplitude=masse["amplitude"])
            for tabelle, soll_werte in (("sin", sin_soll), ("cos", cos_soll)):
                datei = artefakt / meta.get(tabelle, {}).get("file", f"{tabelle}.lut.bin")
                roh = datei.read_bytes()[: 2 * len(soll_werte)] if datei.is_file() else b""
                ist_werte = list(struct.unpack(f"<{len(roh) // 2}h", roh))
                if ist_werte != soll_werte:
                    erste = next((i for i, (a, b) in enumerate(zip(ist_werte, soll_werte)) if a != b), len(ist_werte))
                    print(f"  ⛔️ {tabelle}.lut weicht ab Zeile {erste // paare} von den Massen des Modells ab "
                          f"(Basis {masse['rope_theta']:g}); berichtigen: calibrate/src/rope_nachziehen.py --berichtigen")
                    fehler += 1
            print(f"  ⚑ die ersten {zeilen} Zeilen gegen Basis {masse['rope_theta']:g} geprueft")
        else:
            print(f"  ⚠️ kein Artefakt unter {artefakt.name}, Tabellen "
                  f"nicht geprueft")

        # 2. Was niemand gelesen hat.
        ungelesen = befund["ungelesen"]
        if ungelesen:
            print(f"  ⚠️ {len(ungelesen)} Felder des Config liest niemand:")
            for ck in ungelesen:
                print(f"       {ck} = {echt[ck]!r}")
            print("     ⚑ Jedes davon ist entweder gleichgueltig oder ein")
            print("       Fehler, der noch nicht aufgefallen ist. Wer es")
            print("       fuer gleichgueltig haelt, traegt es in")
            print("       NICHT_GEBRAUCHT ein und sagt damit, dass er")
            print("       nachgesehen hat.")
            fehler += len(ungelesen)
        elif not abweichend:
            print("  ⚑ alle Felder gelesen und in Uebereinstimmung")

    print()
    if fehler:
        print(f"[konfigprobe] ⛔️ FEHLGESCHLAGEN: {fehler} Punkte")
        return 1
    print("[konfigprobe] ⚑ BESTANDEN")
    return 0


if __name__ == "__main__":
    sys.exit(main())
